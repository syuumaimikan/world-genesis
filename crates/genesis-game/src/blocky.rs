//! マインクラフト風のブロック人形モデル。
//!
//! キャラクターはカプセル1個ではなく、頭・胴・両腕・両脚といった直方体の
//! 集合として組み立てる。全パーツが単一の 1×1×1 立方体メッシュを共有し、
//! 色ごとにマテリアルをキャッシュするため、パーツ数が増えても
//! ドローコールはマテリアル数までしか増えない。
//!
//! 手足は「関節でぶら下がる」構造にしてある。肩・股関節の位置に空の
//! ピボットを置き、その子として下方向へ半分ずらした直方体を吊るすことで、
//! ピボットを回すだけで自然な歩行アニメーションになる。

use crate::species::{BodyPlan, SpeciesDef};
use bevy::prelude::*;
use bevy::utils::HashMap;

/// 共有される立方体メッシュと、色ごとのマテリアルキャッシュ。
#[derive(Resource)]
pub struct BlockyAssets {
    pub cube: Handle<Mesh>,
    materials: HashMap<[u8; 3], Handle<StandardMaterial>>,
}

impl BlockyAssets {
    pub fn new(meshes: &mut Assets<Mesh>) -> Self {
        Self {
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            materials: HashMap::new(),
        }
    }

    /// 色に対応するマテリアルを返す。同じ色は必ず同じハンドルになるので、
    /// Bevy 側で自動的にバッチングされる。
    pub fn material(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        color: [f32; 3],
    ) -> Handle<StandardMaterial> {
        let key = [
            (color[0].clamp(0.0, 1.0) * 255.0) as u8,
            (color[1].clamp(0.0, 1.0) * 255.0) as u8,
            (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        ];
        self.materials
            .entry(key)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::rgb(color[0], color[1], color[2]),
                    perceptual_roughness: 0.92,
                    reflectance: 0.05,
                    ..default()
                })
            })
            .clone()
    }

    pub fn cached_material_count(&self) -> usize {
        self.materials.len()
    }
}

/// 歩行アニメーションの状態。親エンティティに付く。
#[derive(Component, Default)]
pub struct LimbAnimator {
    /// 歩行位相（ラジアン）。
    pub phase: f32,
    /// 現在の移動速度（ブロック/秒）。振幅と周期を決める。
    pub move_speed: f32,
    /// 攻撃モーションの残り時間。
    pub attack_timer: f32,
}

/// アニメーション対象の四肢。値は左右・前後の位相オフセット。
#[derive(Component, Clone, Copy)]
pub struct Limb {
    /// 位相オフセット（対角の脚が逆位相になる）。
    pub phase_offset: f32,
    /// 腕は攻撃モーションで振り上がる。
    pub is_arm: bool,
    /// 振れ幅の倍率。
    pub swing_scale: f32,
}

/// 頭。視線方向へ向く。
#[derive(Component)]
pub struct HeadPart;

/// 人型キャラクターの見た目。
#[derive(Debug, Clone, Copy)]
pub struct HumanoidSkin {
    pub skin: [f32; 3],
    pub hair: [f32; 3],
    pub shirt: [f32; 3],
    pub pants: [f32; 3],
    pub shoes: [f32; 3],
}

impl Default for HumanoidSkin {
    fn default() -> Self {
        Self {
            skin: [0.86, 0.68, 0.54],
            hair: [0.22, 0.16, 0.10],
            shirt: [0.30, 0.45, 0.72],
            pants: [0.26, 0.28, 0.34],
            shoes: [0.18, 0.14, 0.12],
        }
    }
}

const SKIN_TONES: [[f32; 3]; 6] = [
    [0.94, 0.80, 0.68],
    [0.86, 0.68, 0.54],
    [0.74, 0.56, 0.42],
    [0.58, 0.42, 0.30],
    [0.42, 0.30, 0.22],
    [0.30, 0.21, 0.16],
];
const HAIR_COLORS: [[f32; 3]; 6] = [
    [0.10, 0.08, 0.06],
    [0.28, 0.18, 0.10],
    [0.52, 0.36, 0.18],
    [0.78, 0.66, 0.36],
    [0.62, 0.30, 0.14],
    [0.82, 0.82, 0.84],
];
const SHIRT_COLORS: [[f32; 3]; 8] = [
    [0.72, 0.28, 0.24],
    [0.28, 0.44, 0.70],
    [0.30, 0.56, 0.34],
    [0.74, 0.66, 0.36],
    [0.52, 0.34, 0.60],
    [0.80, 0.78, 0.72],
    [0.36, 0.34, 0.32],
    [0.66, 0.44, 0.24],
];

impl HumanoidSkin {
    /// ハッシュ値から見た目を決める。同じNPCは常に同じ姿になる。
    ///
    /// NPC の識別子は 0, 1, 2 … と連番で渡されることがあるため、
    /// 生のビットをそのまま添字に使うと隣り合う個体が同じ姿になってしまう。
    /// 各選択ごとに雪崩ハッシュを掛け直して相関を断ち切る。
    pub fn from_hash(h: u64) -> Self {
        let pick = |salt: u64| crate::noise::hash_u64(h ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        Self {
            skin: SKIN_TONES[(pick(1) % SKIN_TONES.len() as u64) as usize],
            hair: HAIR_COLORS[(pick(2) % HAIR_COLORS.len() as u64) as usize],
            shirt: SHIRT_COLORS[(pick(3) % SHIRT_COLORS.len() as u64) as usize],
            pants: [
                0.18 + (pick(4) % 20) as f32 / 100.0,
                0.18 + (pick(5) % 16) as f32 / 100.0,
                0.22 + (pick(6) % 24) as f32 / 100.0,
            ],
            shoes: [0.16, 0.13, 0.11],
        }
    }

    /// 職業に応じた作業着。
    pub fn with_profession(mut self, profession: &str) -> Self {
        self.shirt = match profession {
            "鍛冶屋" => [0.32, 0.28, 0.26],
            "衛兵" => [0.42, 0.44, 0.50],
            "聖職者" => [0.88, 0.86, 0.80],
            "代官" => [0.44, 0.24, 0.48],
            "商人" | "行商人" => [0.66, 0.50, 0.20],
            "農民" | "農家" => [0.52, 0.44, 0.30],
            "鉱夫" => [0.34, 0.32, 0.30],
            "漁師" => [0.28, 0.46, 0.52],
            "パン職人" => [0.86, 0.82, 0.72],
            _ => self.shirt,
        };
        self
    }
}

/// 直方体パーツを1つ生成する。`size` はブロック単位、`center` は親からの相対位置。
fn part(
    commands: &mut ChildBuilder,
    assets: &BlockyAssets,
    material: Handle<StandardMaterial>,
    center: Vec3,
    size: Vec3,
) -> Entity {
    commands
        .spawn(PbrBundle {
            mesh: assets.cube.clone(),
            material,
            transform: Transform::from_translation(center).with_scale(size),
            ..default()
        })
        .id()
}

/// 回転する四肢を生成する。ピボット（関節）を親、直方体を子にする。
fn limb(
    commands: &mut ChildBuilder,
    assets: &BlockyAssets,
    material: Handle<StandardMaterial>,
    joint: Vec3,
    size: Vec3,
    limb_data: Limb,
) {
    commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(joint)),
            limb_data,
        ))
        .with_children(|pivot| {
            // 関節から下（-Y）へぶら下げる。
            pivot.spawn(PbrBundle {
                mesh: assets.cube.clone(),
                material,
                transform: Transform::from_xyz(0.0, -size.y * 0.5, 0.0).with_scale(size),
                ..default()
            });
        });
}

/// 人型モデルを既存エンティティの子として組み立てる。
///
/// 呼び出し側はルートエンティティ（`Transform` の原点は足元）を用意し、
/// その `ChildBuilder` を渡す。`stature` は全体の背丈（ブロック単位。
/// 大人はおよそ 1.8、子供は 1.35）。
pub fn build_humanoid(
    parent: &mut ChildBuilder,
    assets: &mut BlockyAssets,
    materials: &mut Assets<StandardMaterial>,
    skin: HumanoidSkin,
    stature: f32,
) {
    // モデルは「2.0 ブロック基準」で組み、最後に stature/2.0 で縮める。
    let k = stature / 2.0;
    let skin_mat = assets.material(materials, skin.skin);
    let hair_mat = assets.material(materials, skin.hair);
    let shirt_mat = assets.material(materials, skin.shirt);
    let pants_mat = assets.material(materials, skin.pants);
    let shoes_mat = assets.material(materials, skin.shoes);

    // 胴（8×12×4 px）
    part(parent, assets, shirt_mat.clone(), Vec3::new(0.0, 1.125 * k, 0.0),
         Vec3::new(0.50 * k, 0.75 * k, 0.25 * k));

    // 頭（8×8×8 px）— 視線方向へ向くようマーカーを付ける。
    parent
        .spawn((
            PbrBundle {
                mesh: assets.cube.clone(),
                material: skin_mat.clone(),
                transform: Transform::from_xyz(0.0, 1.75 * k, 0.0)
                    .with_scale(Vec3::splat(0.50 * k)),
                ..default()
            },
            HeadPart,
        ));

    // 髪（頭より一回り大きい薄い箱を上と後ろへ）
    part(parent, assets, hair_mat.clone(), Vec3::new(0.0, 1.97 * k, 0.0),
         Vec3::new(0.54 * k, 0.10 * k, 0.54 * k));
    part(parent, assets, hair_mat, Vec3::new(0.0, 1.78 * k, 0.145 * k),
         Vec3::new(0.54 * k, 0.42 * k, 0.26 * k));

    // 腕（4×12×4 px）— 肩は y=1.5、体の左右 x=±0.375
    limb(parent, assets, shirt_mat.clone(),
         Vec3::new(-0.375 * k, 1.5 * k, 0.0),
         Vec3::new(0.25 * k, 0.75 * k, 0.25 * k),
         Limb { phase_offset: std::f32::consts::PI, is_arm: true, swing_scale: 1.0 });
    limb(parent, assets, shirt_mat,
         Vec3::new(0.375 * k, 1.5 * k, 0.0),
         Vec3::new(0.25 * k, 0.75 * k, 0.25 * k),
         Limb { phase_offset: 0.0, is_arm: true, swing_scale: 1.0 });

    // 手（袖から出た肌色の先端）は腕と一緒に回らないと不自然なので省き、
    // 代わりに脚と靴で足元の情報量を確保する。

    // 脚（4×12×4 px）— 股関節は y=0.75
    limb(parent, assets, pants_mat.clone(),
         Vec3::new(-0.125 * k, 0.75 * k, 0.0),
         Vec3::new(0.25 * k, 0.75 * k, 0.25 * k),
         Limb { phase_offset: 0.0, is_arm: false, swing_scale: 1.0 });
    limb(parent, assets, pants_mat,
         Vec3::new(0.125 * k, 0.75 * k, 0.0),
         Vec3::new(0.25 * k, 0.75 * k, 0.25 * k),
         Limb { phase_offset: std::f32::consts::PI, is_arm: false, swing_scale: 1.0 });

    // 靴
    part(parent, assets, shoes_mat.clone(), Vec3::new(-0.125 * k, 0.04 * k, 0.02 * k),
         Vec3::new(0.27 * k, 0.09 * k, 0.30 * k));
    part(parent, assets, shoes_mat, Vec3::new(0.125 * k, 0.04 * k, 0.02 * k),
         Vec3::new(0.27 * k, 0.09 * k, 0.30 * k));
}

/// 種別定義から動物のブロックモデルを組み立てる。
pub fn build_creature(
    parent: &mut ChildBuilder,
    assets: &mut BlockyAssets,
    materials: &mut Assets<StandardMaterial>,
    sp: &SpeciesDef,
) {
    let primary = assets.material(materials, sp.color_primary);
    let secondary = assets.material(materials, sp.color_secondary);
    let accent = assets.material(materials, sp.color_accent);

    let len = sp.length;
    let hgt = sp.height;

    match sp.body {
        BodyPlan::Quadruped => {
            let body_w = hgt * 0.42;
            let body_h = hgt * 0.38;
            let body_y = hgt * 0.62;
            let body_l = len * 0.62;

            // 胴。前方は -Z。
            part(parent, assets, primary.clone(), Vec3::new(0.0, body_y, 0.0),
                 Vec3::new(body_w, body_h, body_l));

            // 頭
            let head = hgt * 0.34;
            part(parent, assets, primary.clone(),
                 Vec3::new(0.0, body_y + hgt * 0.13, -body_l * 0.5 - head * 0.45),
                 Vec3::splat(head));
            // 鼻先
            part(parent, assets, secondary.clone(),
                 Vec3::new(0.0, body_y + hgt * 0.08, -body_l * 0.5 - head * 0.95),
                 Vec3::new(head * 0.5, head * 0.4, head * 0.45));
            // 耳・角
            for sx in [-1.0f32, 1.0] {
                part(parent, assets, accent.clone(),
                     Vec3::new(sx * head * 0.32, body_y + hgt * 0.30, -body_l * 0.5 - head * 0.35),
                     Vec3::new(head * 0.16, head * 0.34, head * 0.16));
            }

            // 4本の脚。対角が逆位相になるよう位相をずらす。
            let leg_h = hgt * 0.55;
            let leg_t = hgt * 0.14;
            let joint_y = body_y - body_h * 0.4;
            let px = body_w * 0.36;
            let pz = body_l * 0.34;
            for (i, (sx, sz)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].iter().enumerate() {
                let offset = if (i == 0) || (i == 3) { 0.0 } else { std::f32::consts::PI };
                limb(parent, assets, primary.clone(),
                     Vec3::new(sx * px, joint_y, sz * pz),
                     Vec3::new(leg_t, leg_h, leg_t),
                     Limb { phase_offset: offset, is_arm: false, swing_scale: 1.0 });
            }

            // 尾
            part(parent, assets, secondary,
                 Vec3::new(0.0, body_y + body_h * 0.3, body_l * 0.5 + len * 0.06),
                 Vec3::new(hgt * 0.10, hgt * 0.10, len * 0.16));
        }

        BodyPlan::Bird => {
            let body = hgt * 0.42;
            part(parent, assets, primary.clone(), Vec3::new(0.0, hgt * 0.55, 0.0),
                 Vec3::new(body * 0.8, body, len * 0.55));
            // 頭と嘴
            part(parent, assets, primary.clone(),
                 Vec3::new(0.0, hgt * 0.80, -len * 0.28),
                 Vec3::splat(body * 0.55));
            part(parent, assets, accent.clone(),
                 Vec3::new(0.0, hgt * 0.78, -len * 0.42),
                 Vec3::new(body * 0.18, body * 0.18, body * 0.35));
            // 翼（腕として振れるようにする）
            for sx in [-1.0f32, 1.0] {
                limb(parent, assets, secondary.clone(),
                     Vec3::new(sx * body * 0.45, hgt * 0.60, 0.0),
                     Vec3::new(body * 0.14, len * 0.5, len * 0.34),
                     Limb { phase_offset: 0.0, is_arm: true, swing_scale: 0.6 });
            }
            // 脚
            for sx in [-1.0f32, 1.0] {
                limb(parent, assets, accent.clone(),
                     Vec3::new(sx * body * 0.22, hgt * 0.36, 0.0),
                     Vec3::new(body * 0.10, hgt * 0.36, body * 0.10),
                     Limb { phase_offset: if sx < 0.0 { 0.0 } else { std::f32::consts::PI }, is_arm: false, swing_scale: 1.0 });
            }
            // 尾羽
            part(parent, assets, secondary,
                 Vec3::new(0.0, hgt * 0.58, len * 0.34),
                 Vec3::new(body * 0.7, body * 0.14, len * 0.28));
        }

        BodyPlan::Fish => {
            part(parent, assets, primary.clone(), Vec3::new(0.0, hgt * 0.5, 0.0),
                 Vec3::new(hgt * 0.35, hgt * 0.8, len * 0.7));
            // 尾びれ（振れるように四肢として付ける）
            limb(parent, assets, secondary.clone(),
                 Vec3::new(0.0, hgt * 0.5, len * 0.36),
                 Vec3::new(hgt * 0.08, hgt * 0.7, len * 0.28),
                 Limb { phase_offset: 0.0, is_arm: true, swing_scale: 0.5 });
            // 背びれ・胸びれ
            part(parent, assets, accent.clone(),
                 Vec3::new(0.0, hgt * 0.95, -len * 0.05),
                 Vec3::new(hgt * 0.06, hgt * 0.32, len * 0.30));
            for sx in [-1.0f32, 1.0] {
                part(parent, assets, secondary.clone(),
                     Vec3::new(sx * hgt * 0.2, hgt * 0.45, -len * 0.15),
                     Vec3::new(hgt * 0.18, hgt * 0.06, len * 0.16));
            }
            // 目
            part(parent, assets, accent, Vec3::new(0.0, hgt * 0.62, -len * 0.33),
                 Vec3::new(hgt * 0.30, hgt * 0.12, hgt * 0.06));
        }

        BodyPlan::Insect => {
            let seg = len * 0.34;
            part(parent, assets, primary.clone(), Vec3::new(0.0, hgt * 0.6, seg * 0.4),
                 Vec3::new(seg * 0.9, hgt * 0.7, seg));
            part(parent, assets, secondary.clone(), Vec3::new(0.0, hgt * 0.6, -seg * 0.5),
                 Vec3::new(seg * 0.7, hgt * 0.6, seg * 0.7));
            // 6脚
            for (i, sz) in [-0.6f32, 0.0, 0.6].iter().enumerate() {
                for sx in [-1.0f32, 1.0] {
                    let offset = if (i % 2 == 0) == (sx < 0.0) { 0.0 } else { std::f32::consts::PI };
                    limb(parent, assets, accent.clone(),
                         Vec3::new(sx * seg * 0.45, hgt * 0.55, sz * seg * 0.5),
                         Vec3::new(seg * 0.10, hgt * 0.5, seg * 0.10),
                         Limb { phase_offset: offset, is_arm: false, swing_scale: 1.4 });
                }
            }
            // 触角
            for sx in [-1.0f32, 1.0] {
                part(parent, assets, accent.clone(),
                     Vec3::new(sx * seg * 0.18, hgt * 0.85, -seg * 0.85),
                     Vec3::new(seg * 0.06, seg * 0.06, seg * 0.4));
            }
        }

        BodyPlan::Reptile => {
            let body_h = hgt * 0.7;
            part(parent, assets, primary.clone(), Vec3::new(0.0, body_h * 0.55, 0.0),
                 Vec3::new(len * 0.20, body_h, len * 0.52));
            // 頭
            part(parent, assets, primary.clone(),
                 Vec3::new(0.0, body_h * 0.55, -len * 0.36),
                 Vec3::new(len * 0.16, body_h * 0.8, len * 0.24));
            // 顎
            part(parent, assets, accent.clone(),
                 Vec3::new(0.0, body_h * 0.34, -len * 0.46),
                 Vec3::new(len * 0.13, body_h * 0.28, len * 0.18));
            // 尾（3節でテーパーさせる）
            for i in 1..=3 {
                let t = i as f32;
                part(parent, assets, secondary.clone(),
                     Vec3::new(0.0, body_h * 0.55, len * (0.26 + 0.14 * t)),
                     Vec3::new(len * 0.18 / t.max(1.0), body_h * 0.8 / t.max(1.0), len * 0.14));
            }
            // 短い四肢
            for (i, (sx, sz)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].iter().enumerate() {
                let offset = if (i == 0) || (i == 3) { 0.0 } else { std::f32::consts::PI };
                limb(parent, assets, primary.clone(),
                     Vec3::new(sx * len * 0.13, body_h * 0.35, sz * len * 0.20),
                     Vec3::new(len * 0.07, body_h * 0.5, len * 0.07),
                     Limb { phase_offset: offset, is_arm: false, swing_scale: 1.2 });
            }
        }
    }
}

/// 四肢を歩行位相に合わせて回転させる。
///
/// 手足のピボットを X 軸まわりに振るだけで歩いて見える。振幅は速度に比例し、
/// 静止時はゆっくり 0 へ戻るので、急に硬直したようには見えない。
pub fn animate_limbs(
    time: Res<Time>,
    mut animators: Query<(&mut LimbAnimator, &Children)>,
    mut limbs: Query<(&Limb, &mut Transform)>,
) {
    let dt = time.delta_seconds();
    for (mut anim, children) in animators.iter_mut() {
        // 速度が上がるほど歩調が速くなる。
        let cadence = (anim.move_speed * 1.9).clamp(0.0, 22.0);
        anim.phase = (anim.phase + cadence * dt) % std::f32::consts::TAU;
        anim.attack_timer = (anim.attack_timer - dt).max(0.0);

        // 振幅は速度に比例させ、0.9 rad で頭打ちにする。
        let amplitude = (anim.move_speed * 0.16).clamp(0.0, 0.9);
        let phase = anim.phase;
        let attacking = anim.attack_timer > 0.0;
        let attack_swing = if attacking {
            // 0→1→0 の一振り。
            let t = 1.0 - anim.attack_timer.min(0.35) / 0.35;
            (t * std::f32::consts::PI).sin() * -1.7
        } else {
            0.0
        };

        for &child in children.iter() {
            let Ok((limb, mut tf)) = limbs.get_mut(child) else {
                continue;
            };
            let mut angle = (phase + limb.phase_offset).sin() * amplitude * limb.swing_scale;
            if limb.is_arm && attacking {
                angle = attack_swing;
            }
            tf.rotation = Quat::from_rotation_x(angle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::SPECIES;

    #[test]
    fn skins_are_deterministic_and_in_range() {
        for h in [0u64, 1, 12345, u64::MAX, 0xDEAD_BEEF] {
            let a = HumanoidSkin::from_hash(h);
            let b = HumanoidSkin::from_hash(h);
            assert_eq!(a.skin, b.skin);
            assert_eq!(a.shirt, b.shirt);
            for c in [a.skin, a.hair, a.shirt, a.pants, a.shoes] {
                assert!(c.iter().all(|v| (0.0..=1.0).contains(v)), "colour out of range: {c:?}");
            }
        }
    }

    #[test]
    fn consecutive_ids_give_visibly_different_people() {
        // NPC の ID は連番で渡されるので、小さな値でも見た目が散ることが重要。
        let looks: Vec<(([f32; 3]), [f32; 3], [f32; 3])> = (0..24u64)
            .map(|i| {
                let s = HumanoidSkin::from_hash(i);
                (s.skin, s.hair, s.shirt)
            })
            .collect();
        let mut unique = looks.clone();
        unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
        unique.dedup();
        assert!(
            unique.len() >= 18,
            "24 consecutive NPCs produced only {} distinct looks",
            unique.len()
        );
    }

    #[test]
    fn professions_change_the_outfit() {
        let base = HumanoidSkin::from_hash(3);
        let smith = base.with_profession("鍛冶屋");
        let guard = base.with_profession("衛兵");
        assert_ne!(smith.shirt, guard.shirt);
        // 未知の職業は元の服のまま。
        assert_eq!(base.with_profession("未知の職").shirt, base.shirt);
    }

    /// モデルの寸法計算が、どの種でも有限で正の値になることを確かめる。
    /// 0 や NaN のスケールは Bevy の変換行列を壊し、画面が真っ黒になる。
    #[test]
    fn creature_dimensions_never_degenerate() {
        for sp in SPECIES {
            let len = sp.length;
            let hgt = sp.height;
            let dims = [
                hgt * 0.42, hgt * 0.38, len * 0.62, hgt * 0.34,
                hgt * 0.55, hgt * 0.14, len * 0.20, hgt * 0.7,
                len * 0.34, len * 0.55, hgt * 0.8,
            ];
            for d in dims {
                assert!(d.is_finite() && d > 0.0, "{} produced a degenerate dimension {d}", sp.key);
            }
        }
    }

    #[test]
    fn humanoid_scale_factor_is_positive_for_all_statures() {
        for stature in [1.2f32, 1.35, 1.8, 2.1] {
            let k = stature / 2.0;
            assert!(k > 0.0);
            // 最小のパーツ（靴の厚み）が潰れないこと。
            assert!(0.09 * k > 1e-3);
        }
    }
}
