//! ゲーム本体：世界の読み込み・破棄、プレイヤー操作、採掘と設置、
//! 昼夜と天候、そして自動セーブ。

use crate::actors::*;
use crate::ai::PopulationTracker;
use crate::biome::{biome_def, Biome, ALL_BIOMES};
use crate::blocks::{ids, BlockRegistry, ToolClass};
use crate::blocky::{build_humanoid, BlockyAssets, HumanoidSkin, LimbAnimator};
use crate::chronicle::LocalChronicle;
use crate::chunk::{ChunkData, ChunkPos, SEA_LEVEL};
use crate::items::{Inventory, ItemRegistry, HOTBAR_SLOTS};
use crate::menu::{AppState, SaveManagerRes, SaveRequest, Toast, UiDirty};
use crate::physics::{find_spawn_y, move_body, BodyShape};
use crate::plugins::{degrade_unknown_blocks, PluginManager};
use crate::saves::{PlayerSave, WorldMeta, WorldSaveBody, SAVE_FORMAT_VERSION};
use crate::settings::GameSettings;
use crate::streaming::{raycast_blocks, StreamConfig, StreamOrigin, VoxelMaterials, VoxelWorld};
use crate::ui_theme::{C_ERR, C_OK};
use crate::worldgen::WorldGenerator;
use bevy::core_pipeline::tonemapping::Tonemapping;
use crate::keybinds::Action;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::ecs::system::SystemParam;
use bevy::pbr::{CascadeShadowConfigBuilder, FogFalloff, FogSettings};
use bevy::prelude::*;
use std::sync::Arc;

/// 三人称カメラを維持できる最小距離。これより詰まる場所では一人称にする。
const MIN_THIRD_PERSON_DISTANCE: f32 = 1.9;

/// 視点モード。F5 で巡回する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Perspective {
    /// 一人称。自分の体は描かない。
    #[default]
    First,
    /// 三人称（背後から）。
    ThirdBack,
    /// 二人称（正面から自分を見る）。
    ThirdFront,
}

impl Perspective {
    pub fn next(self) -> Self {
        match self {
            Perspective::First => Perspective::ThirdBack,
            Perspective::ThirdBack => Perspective::ThirdFront,
            Perspective::ThirdFront => Perspective::First,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Perspective::First => "一人称",
            Perspective::ThirdBack => "三人称（背面）",
            Perspective::ThirdFront => "二人称（正面）",
        }
    }

    /// カメラを引く向き。正面視点では前方へ回り込む。
    pub fn back_sign(self) -> f32 {
        match self {
            Perspective::ThirdFront => -1.0,
            _ => 1.0,
        }
    }
}

/// いま遊んでいる世界のメタ情報。
#[derive(Resource)]
pub struct ActiveWorld {
    pub meta: WorldMeta,
    /// このセッションでの累積プレイ秒。
    pub session_seconds: f64,
    pub autosave_timer: f32,
}

/// ゲーム世界に属するエンティティ（タイトルへ戻るとき一括で消す）。
#[derive(Component)]
pub struct WorldEntity;

/// 手に持った光源（松明・ランタン）が生む動的な明かり。
#[derive(Component)]
pub struct HeldLight;

/// 太陽と月。
#[derive(Component)]
pub struct SunLight;

#[derive(Component)]
pub struct MoonLight;

/// 対話中の相手。
#[derive(Resource, Default)]
pub struct DialogueState {
    pub speaker: Option<Entity>,
    pub name: String,
    pub text: String,
}

/// 採掘の進行状況。
#[derive(Resource, Default)]
pub struct MiningState {
    pub target: Option<IVec3>,
    pub progress: f32,
    pub required: f32,
}

// ======================================================================
// 世界の読み込み
// ======================================================================

/// `LoadingWorld` 状態に入ったとき、世界を組み立てる。
#[allow(clippy::too_many_arguments)]
pub fn enter_world_system(
    mut commands: Commands,
    mut pending: ResMut<crate::menu::PendingLoad>,
    mut next_state: ResMut<NextState<AppState>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blocks: ResMut<BlockRegistry>,
    plugin_mgr: Res<PluginManager>,
    settings: Res<GameSettings>,
    save_mgr: Res<SaveManagerRes>,
    mut toast: ResMut<Toast>,
) {
    let Some(meta) = pending.0.take() else {
        // 読み込むものが無いならタイトルへ戻す（宙ぶらりんにしない）。
        next_state.set(AppState::Title);
        return;
    };

    // --- 1. プラグインを、この世界に記録された構成どおりに適用する ---
    blocks.reset_to_builtins();
    let contributions = plugin_mgr.apply(&mut blocks, Some(&meta.plugins));
    if !contributions.applied_plugin_ids.is_empty() {
        info!("プラグインを適用: {:?}", contributions.applied_plugin_ids);
    }
    let lookup = blocks.snapshot();
    let items = ItemRegistry::build(&blocks);

    // --- 2. ワールド生成器 ---
    let mut generator = WorldGenerator::new(meta.seed, meta.to_gen_params());
    generator.ore_rules.extend(contributions.ore_rules.iter().cloned());
    let mut voxel_world = VoxelWorld::new(generator, lookup.clone());

    // --- 3. セーブされた改変チャンクとプレイヤー状態を戻す ---
    let mut player_save = PlayerSave::default();
    let mut chronicle = LocalChronicle::new();
    let mut first_visit = true;

    match save_mgr.0.read_body(&meta.folder) {
        Ok(body) => {
            let is_solid = |b| lookup.is_solid(b);
            let mut degraded_total = 0usize;
            for packed in &body.modified_chunks {
                match ChunkData::from_palette_rle(packed, &is_solid) {
                    Some(mut chunk) => {
                        // 外されたプラグインのブロックは石へ縮退させる。
                        degraded_total += degrade_unknown_blocks(&mut chunk.voxels, &blocks);
                        voxel_world.inject_saved_chunk(chunk);
                    }
                    None => warn!("壊れたチャンクを読み飛ばしました ({}, {})", packed.x, packed.z),
                }
            }
            if degraded_total > 0 {
                toast.show(
                    format!("失われたプラグインのブロック {degraded_total} 個を石に置き換えました。"),
                    crate::ui_theme::C_WARN,
                );
            }
            if body.player.health > 0.0 {
                player_save = body.player;
                first_visit = false;
            }
            chronicle = LocalChronicle::from_save(&body.chronicle);
        }
        Err(e) => {
            // 新規作成直後は本体が空なので、これは異常ではない。
            info!("セーブ本体を読み込めませんでした（新規世界の可能性）: {e}");
        }
    }

    // --- 4. プレイヤーの配置 ---
    let spawn_shape = BodyShape::default();
    let (spawn_pos, yaw, pitch) = if first_visit {
        let (cx, cz) = find_habitable_spawn(&voxel_world);
        // スポーン地点のチャンクは同期生成する（落下防止）。
        prime_chunks_around(&mut voxel_world, ChunkPos::from_world(cx as f32, cz as f32), 2);
        // 実際のボクセルを見て、木の下や屋根の下を避ける。
        let (sx, sz) = find_open_column(&voxel_world, cx, cz);
        let y = find_spawn_y(&voxel_world, sx, sz, spawn_shape);
        (Vec3::new(sx as f32 + 0.5, y, sz as f32 + 0.5), 0.0, 0.25)
    } else {
        let p = Vec3::new(player_save.x, player_save.y, player_save.z);
        prime_chunks_around(&mut voxel_world, ChunkPos::from_world(p.x, p.z), 1);
        (p, player_save.yaw, player_save.pitch)
    };

    let inventory = if first_visit {
        Inventory::starter()
    } else {
        Inventory::from_save(&player_save.hotbar, &items)
    };

    let skin = HumanoidSkin::from_hash(meta.seed ^ 0x504C_4159);
    let mut blocky = BlockyAssets::new(&mut meshes);

    let player_entity = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(spawn_pos)),
            Player {
                hunger: if first_visit { 100.0 } else { player_save.hunger.max(1.0) },
                body_temp: if first_visit { 36.5 } else { player_save.body_temp.clamp(30.0, 40.0) },
                money: if first_visit { 120.0 } else { player_save.money },
                profession: if player_save.profession.is_empty() {
                    "放浪者".to_string()
                } else {
                    player_save.profession.clone()
                },
                reputation: player_save.reputation,
                age_days: if first_visit { 18.0 * 360.0 } else { player_save.age_days },
                selected_slot: player_save.selected_slot.min(HOTBAR_SLOTS - 1),
                ..Default::default()
            },
            Actor::new(spawn_shape, 5.6),
            Health {
                current: if first_visit { 100.0 } else { player_save.health.max(1.0) },
                max: 100.0,
            },
            inventory,
            LimbAnimator::default(),
            StreamOrigin,
            WorldEntity,
        ))
        .with_children(|parent| {
            build_humanoid(parent, &mut blocky, &mut materials, skin, 1.8);
            // 手に持った松明の明かり。既定では消えていて、
            // 光源アイテムを選んだときだけ点く。
            parent.spawn((
                PointLightBundle {
                    point_light: PointLight {
                        color: Color::rgb(1.0, 0.82, 0.52),
                        intensity: 0.0,
                        range: 18.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    transform: Transform::from_xyz(0.42, 1.25, -0.28),
                    ..default()
                },
                HeldLight,
            ));
        })
        .id();
    let _ = player_entity;

    // --- 5. カメラ・光源・空 ---
    let sky = Color::rgb(0.52, 0.72, 0.96);
    commands.spawn((
        Camera3dBundle {
            projection: PerspectiveProjection {
                fov: settings.fov_degrees.to_radians(),
                far: 2000.0,
                ..default()
            }
            .into(),
            transform: Transform::from_translation(spawn_pos + Vec3::Y * 2.0),
            // ブロックの色は無地の頂点カラーなので、既定のフィルミック曲線だと
            // 明るい面が一様に乳白色へ寄ってしまう。輝度だけを圧縮する
            // Reinhard のほうが、草の緑や桜の桃色をそのまま残せる。
            tonemapping: Tonemapping::ReinhardLuminance,
            ..default()
        },
        FogSettings {
            color: sky,
            falloff: FogFalloff::Linear {
                start: (settings.render_distance as f32 * 16.0) * 0.55,
                end: (settings.render_distance as f32 * 16.0) * 0.95,
            },
            ..default()
        },
        PlayerCamera {
            yaw,
            pitch,
            distance: 5.0,
            perspective: if settings.third_person {
                Perspective::ThirdBack
            } else {
                Perspective::First
            },
        },
        WorldEntity,
    ));

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                illuminance: 32_000.0,
                shadows_enabled: true,
                ..default()
            },
            cascade_shadow_config: CascadeShadowConfigBuilder {
                num_cascades: 3,
                maximum_distance: 220.0,
                ..default()
            }
            .into(),
            ..default()
        },
        SunLight,
        WorldEntity,
    ));
    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::rgb(0.62, 0.70, 0.95),
                illuminance: 900.0,
                shadows_enabled: false,
                ..default()
            },
            ..default()
        },
        MoonLight,
        WorldEntity,
    ));

    // --- 6. リソースの差し替え ---
    let mut world_time = WorldTime::default();
    world_time.tick = if meta.sim_tick == 0 { 9 * 3600 } else { meta.sim_tick };

    commands.insert_resource(voxel_world);
    commands.insert_resource(VoxelMaterials::new(&mut materials));
    commands.insert_resource(blocky);
    commands.insert_resource(items);
    commands.insert_resource(world_time);
    commands.insert_resource(chronicle);
    commands.insert_resource(PopulationTracker::new());
    commands.insert_resource(crate::ai::ThreatBoard::default());
    commands.insert_resource(crate::ai::SpawnTimers::default());
    commands.insert_resource(DialogueState::default());
    commands.insert_resource(MiningState::default());
    commands.insert_resource(StreamConfig {
        render_distance: settings.render_distance,
        upload_budget: settings.chunk_upload_budget,
    });
    commands.insert_resource(ClearColor(sky));
    commands.insert_resource(AmbientLight {
        color: Color::rgb(0.86, 0.90, 1.0),
        brightness: 260.0,
    });
    commands.insert_resource(ActiveWorld {
        meta,
        session_seconds: 0.0,
        autosave_timer: 0.0,
    });

    next_state.set(AppState::InGame);
}

/// 居住に適したスポーン地点を探す。
///
/// 海の真ん中や崖の上、そして**建物の中**には降ろさない。
/// 近くに集落があれば、その外縁の開けた場所から村を望む位置に立たせる。
fn find_habitable_spawn(world: &VoxelWorld) -> (i32, i32) {
    let gen = &world.generator;
    let sea = gen.params.sea_level;

    let plans = gen.villages.plans_around(gen, 0, 0, 2);

    // 集落の外側の、開けた平地を探す。
    let mut best: Option<(i32, i32, i32)> = None; // (優先度, x, z)
    for plan in &plans {
        if plan.ground_y <= sea + 1 {
            continue;
        }
        let ring = plan.tier.radius() + 10;
        for step in 0..16 {
            let ang = step as f32 * std::f32::consts::TAU / 16.0;
            let x = plan.center_x + (ang.cos() * ring as f32) as i32;
            let z = plan.center_z + (ang.sin() * ring as f32) as i32;

            let h = gen.terrain_height(x as f32, z as f32);
            if h <= sea + 1 {
                continue;
            }
            // 建物・農地・城壁と重ならないこと。
            if is_inside_any_structure(&plans, x, z) {
                continue;
            }
            // 周囲が平坦であること（崖の縁に立たせない）。
            let relief = local_relief(gen, x, z);
            if relief > 5 {
                continue;
            }
            // 集落の中心に近いほど良い眺め。
            let score = -(relief * 100 + step);
            if best.map(|(s, _, _)| score > s).unwrap_or(true) {
                best = Some((score, x, z));
            }
        }
        if best.is_some() {
            break;
        }
    }
    if let Some((_, x, z)) = best {
        return (x, z);
    }

    // 集落が無ければ、原点から螺旋状に陸地を探す。
    for radius in (0..4000).step_by(40) {
        for step in 0..16 {
            let ang = step as f32 * std::f32::consts::TAU / 16.0;
            let x = (ang.cos() * radius as f32) as i32;
            let z = (ang.sin() * radius as f32) as i32;
            let h = gen.terrain_height(x as f32, z as f32);
            if h > sea + 2 && h < sea + 45 && local_relief(gen, x, z) <= 6 {
                let biome = gen.biome_at(x as f32, z as f32);
                if biome_def(biome).habitability > 0.3 && !is_inside_any_structure(&plans, x, z) {
                    return (x, z);
                }
            }
        }
    }
    (0, 0)
}

/// この座標が、集落の建物・農地の内側にあるか。
fn is_inside_any_structure(plans: &[crate::village::VillagePlan], x: i32, z: i32) -> bool {
    const MARGIN: i32 = 2;
    plans.iter().any(|plan| {
        plan.buildings.iter().any(|b| {
            x >= b.x - MARGIN
                && x < b.x + b.w + MARGIN
                && z >= b.z - MARGIN
                && z < b.z + b.d + MARGIN
        }) || plan.farms.iter().any(|&(fx, fz, fw, fd)| {
            x >= fx && x < fx + fw && z >= fz && z < fz + fd
        })
    })
}

/// 周囲 8 点との標高差。大きいほど急斜面。
fn local_relief(gen: &WorldGenerator, x: i32, z: i32) -> i32 {
    let center = gen.terrain_height(x as f32, z as f32);
    let mut lo = center;
    let mut hi = center;
    for (dx, dz) in [(-3, 0), (3, 0), (0, -3), (0, 3), (-2, -2), (2, 2), (-2, 2), (2, -2)] {
        let h = gen.terrain_height((x + dx) as f32, (z + dz) as f32);
        lo = lo.min(h);
        hi = hi.max(h);
    }
    hi - lo
}

/// 生成済みのボクセルを見て、頭上が開けた列を探す。
///
/// 立地スコアだけでは「木の真下」「屋根の下」に降りてしまうことがある。
/// 実際に空が見えるかどうかは、地形を作ってからでないと分からない。
fn find_open_column(world: &VoxelWorld, cx: i32, cz: i32) -> (i32, i32) {
    let is_open = |x: i32, z: i32| -> bool {
        let ground = world.ground_height(x, z);
        if ground <= world.generator.params.sea_level {
            return false;
        }
        // 頭上 14 ブロックが抜けていること。
        for dy in 1..=14 {
            match world.block_at(x, ground + dy, z) {
                Some(b) if b.is_air() => {}
                // 未生成なら判断できないので「開けていない」とみなす。
                _ => return false,
            }
        }
        true
    };

    if is_open(cx, cz) {
        return (cx, cz);
    }
    // 渦巻き状に近い順で探す。
    for r in 1..24 {
        for step in 0..(r * 8) {
            let ang = step as f32 * std::f32::consts::TAU / (r * 8) as f32;
            let x = cx + (ang.cos() * r as f32).round() as i32;
            let z = cz + (ang.sin() * r as f32).round() as i32;
            if is_open(x, z) {
                return (x, z);
            }
        }
    }
    (cx, cz)
}

/// 指定チャンクの周囲を同期生成する（読み込み直後の落下を防ぐ）。
fn prime_chunks_around(world: &mut VoxelWorld, center: ChunkPos, radius: i32) {
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let p = ChunkPos::new(center.x + dx, center.z + dz);
            if !world.chunks.contains_key(&p) {
                let data = world.generator.generate_chunk(p, &world.lookup);
                world.chunks.insert(p, Arc::new(data));
            }
        }
    }
}

/// タイトルへ戻るときに世界を片付ける。
pub fn exit_world_system(
    mut commands: Commands,
    entities: Query<Entity, With<WorldEntity>>,
    npcs: Query<Entity, With<Npc>>,
    creatures: Query<Entity, With<Wildlife>>,
    projectiles: Query<Entity, With<Projectile>>,
    chunk_meshes: Query<Entity, With<crate::streaming::ChunkMeshMarker>>,
) {
    for e in entities
        .iter()
        .chain(npcs.iter())
        .chain(creatures.iter())
        .chain(projectiles.iter())
        .chain(chunk_meshes.iter())
    {
        commands.entity(e).despawn_recursive();
    }
    commands.remove_resource::<VoxelWorld>();
    commands.remove_resource::<ActiveWorld>();
}

// ======================================================================
// 時間・空・光
// ======================================================================

pub fn advance_time_system(time: Res<Time>, mut world_time: ResMut<WorldTime>, mut active: ResMut<ActiveWorld>) {
    world_time.advance(time.delta_seconds());
    active.session_seconds += time.delta_seconds() as f64;
}

/// 太陽・月・空の色・フォグを時刻とバイオームに合わせる。
#[allow(clippy::too_many_arguments)]
pub fn sky_system(
    world_time: Res<WorldTime>,
    world: Option<Res<VoxelWorld>>,
    settings: Res<GameSettings>,
    config: Res<StreamConfig>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<AmbientLight>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), (With<SunLight>, Without<MoonLight>)>,
    mut moon: Query<(&mut Transform, &mut DirectionalLight), (With<MoonLight>, Without<SunLight>)>,
    mut fog: Query<&mut FogSettings>,
    player: Query<&Transform, (With<Player>, Without<SunLight>, Without<MoonLight>)>,
) {
    let elevation = world_time.sun_elevation();
    let t = world_time.day_fraction();
    // 太陽は東（+X）から昇り西へ沈む。
    let azimuth = t * std::f32::consts::TAU;
    let sun_dir = Vec3::new(azimuth.cos() * 0.6, elevation, azimuth.sin() * 0.6).normalize_or_zero();

    if let Ok((mut tf, mut light)) = sun.get_single_mut() {
        tf.translation = sun_dir * 400.0;
        tf.look_at(Vec3::ZERO, Vec3::Y);
        // 地平線近くでは弱く、赤みを帯びる。
        //
        // 照度は現実の値（快晴の昼で数万ルクス）ではなく、Bevy の既定露出
        // （EV100 = 9.7）で白飛びしない範囲に合わせる。実測で 3 万ルクスでは
        // 草地も葉も白く飛んでしまったため、1.3 万を上限にしている。
        let strength = elevation.max(0.0).powf(0.55);
        light.illuminance = 500.0 + 7_000.0 * strength;
        let warmth = (1.0 - elevation.max(0.0)).powf(2.0);
        light.color = Color::rgb(1.0, 1.0 - warmth * 0.32, 1.0 - warmth * 0.55);
        light.shadows_enabled = elevation > 0.02;
    }
    if let Ok((mut tf, mut light)) = moon.get_single_mut() {
        tf.translation = -sun_dir * 400.0;
        tf.look_at(Vec3::ZERO, Vec3::Y);
        // 月明かり。真っ暗にはせず、地形の輪郭が読める程度に残す。
        light.illuminance = if elevation < 0.05 { 1_500.0 } else { 0.0 };
    }

    // 空の色：バイオームの基調色を、昼夜と朝焼けで動かす。
    let base = match (world.as_ref(), player.get_single().ok()) {
        (Some(w), Some(p)) => {
            let cp = ChunkPos::from_world(p.translation.x, p.translation.z);
            w.chunks
                .get(&cp)
                .map(|c| {
                    let b = ALL_BIOMES
                        .get(c.biome_at(
                            p.translation.x.floor() as i32 - cp.origin().0,
                            p.translation.z.floor() as i32 - cp.origin().1,
                        ) as usize)
                        .copied()
                        .unwrap_or(Biome::Plains);
                    biome_def(b).sky_color
                })
                .unwrap_or([0.52, 0.72, 0.96])
        }
        _ => [0.52, 0.72, 0.96],
    };

    let day = elevation.max(0.0).powf(0.4);
    let dusk = (1.0 - (elevation.abs() * 5.0).min(1.0)).max(0.0);
    let night_col = Vec3::new(0.03, 0.04, 0.09);
    let day_col = Vec3::from_array(base);
    let dusk_col = Vec3::new(0.86, 0.44, 0.28);

    let mut sky = night_col.lerp(day_col, day);
    sky = sky.lerp(dusk_col, dusk * 0.45);
    let sky_color = Color::rgb(sky.x, sky.y, sky.z);

    clear.0 = sky_color;
    // 屋内は面ごとの光量計算を持たないため、環境光の下限を高めに取る。
    // これが低すぎると、家の中や坑道が真っ黒で何も見えなくなる。
    // 夜の下限を高めに取る。ここが低すぎると、月夜でも画面が真っ黒になる。
    ambient.brightness = 115.0 + 130.0 * day;
    ambient.color = Color::rgb(
        0.55 + 0.35 * day,
        0.60 + 0.32 * day,
        0.78 + 0.22 * day,
    );

    let reach = config.render_distance as f32 * 16.0;
    for mut f in fog.iter_mut() {
        f.color = sky_color;
        f.falloff = if settings.show_fog {
            FogFalloff::Linear {
                start: reach * 0.55,
                end: reach * 0.98,
            }
        } else {
            // 完全に切るのではなく、遥か遠方でだけ効かせる。
            FogFalloff::Linear {
                start: 4000.0,
                end: 6000.0,
            }
        };
    }
}

// ======================================================================
// プレイヤー操作
// ======================================================================

#[allow(clippy::too_many_arguments)]
pub fn player_control_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    settings: Res<GameSettings>,
    world: Res<VoxelWorld>,
    mut player: Query<
        (&mut Transform, &mut Actor, &mut Player, &mut LimbAnimator, &mut Visibility),
        Without<PlayerCamera>,
    >,
    mut camera: Query<(&mut Transform, &mut PlayerCamera), Without<Player>>,
) {
    let dt = time.delta_seconds().min(0.1);
    let Ok((mut tf, mut actor, mut player_state, mut anim, mut visibility)) = player.get_single_mut()
    else {
        return;
    };
    let Ok((mut cam_tf, mut cam)) = camera.get_single_mut() else { return };

    // --- 視点 ---
    let sens = settings.mouse_sensitivity;
    let invert = if settings.invert_mouse_y { 1.0 } else { -1.0 };
    for ev in mouse_motion.read() {
        cam.yaw -= ev.delta.x * sens;
        cam.pitch = (cam.pitch + ev.delta.y * sens * invert).clamp(-1.5, 1.5);
    }
    // ホイールで三人称の距離を調整する。
    for ev in wheel.read() {
        cam.distance = (cam.distance - ev.y * 0.6).clamp(1.5, 9.0);
    }

    let binds = &settings.keybinds;

    // --- 視点切替（F5）---
    if binds.just_pressed(Action::Perspective, &keys) {
        cam.perspective = cam.perspective.next();
    }

    // --- ホットバー選択 ---
    if let Some(slot) = binds.hotbar_pressed(&keys) {
        player_state.selected_slot = slot;
    }

    // --- 移動入力 ---
    let mut input = Vec3::ZERO;
    if binds.pressed(Action::Forward, &keys) { input.z -= 1.0; }
    if binds.pressed(Action::Backward, &keys) { input.z += 1.0; }
    if binds.pressed(Action::Left, &keys) { input.x -= 1.0; }
    if binds.pressed(Action::Right, &keys) { input.x += 1.0; }

    let sprinting = binds.pressed(Action::Sprint, &keys) && player_state.stamina > 5.0;
    let base_speed = if sprinting { 8.4 } else { 5.0 };
    if sprinting && input.length_squared() > 0.0 {
        player_state.stamina = (player_state.stamina - 16.0 * dt).max(0.0);
    } else {
        player_state.stamina = (player_state.stamina + 9.0 * dt).min(100.0);
    }

    let yaw_rot = Quat::from_rotation_y(cam.yaw);
    let mut horizontal = Vec3::ZERO;
    if input.length_squared() > 0.0 {
        horizontal = (yaw_rot * input.normalize()) * base_speed;
        if actor.in_liquid {
            horizontal *= 0.55;
        }
    }

    // --- 跳躍・浮上・重力 ---
    if actor.in_liquid {
        // 水中：スペースで浮上、それ以外はゆっくり沈む。
        actor.velocity.y = if binds.pressed(Action::Jump, &keys) {
            3.4
        } else {
            (actor.velocity.y - 3.0 * dt).max(-2.4)
        };
    } else if player_state.flying {
        // 飛行中は重力を受けない。Space で上昇、Ctrl で下降。
        actor.velocity.y = if binds.pressed(Action::Jump, &keys) {
            7.0
        } else if binds.pressed(Action::Crouch, &keys) {
            -7.0
        } else {
            0.0
        };
    } else if actor.grounded {
        actor.velocity.y = if binds.just_pressed(Action::Jump, &keys) { 8.6 } else { 0.0 };
    } else {
        actor.velocity.y = (actor.velocity.y - 26.0 * dt).max(-58.0);
    }

    // 飛行の切り替え（建築と地形確認のため）。
    if binds.just_pressed(Action::Fly, &keys) {
        player_state.flying = !player_state.flying;
        actor.velocity.y = 0.0;
    }

    let delta = Vec3::new(horizontal.x * dt, actor.velocity.y * dt, horizontal.z * dt);
    let before = tf.translation;
    let result = move_body(&world, before, actor.shape, delta);

    actor.grounded = result.grounded;
    actor.in_liquid = result.in_liquid;
    if result.hit_y && actor.velocity.y < 0.0 {
        actor.velocity.y = 0.0;
    }
    tf.translation = result.position;

    let moved = Vec3::new(result.position.x - before.x, 0.0, result.position.z - before.z);
    actor.last_speed = if dt > 0.0 { moved.length() / dt } else { 0.0 };
    anim.move_speed = actor.last_speed;

    // 体の向きは進行方向、止まっていれば視線方向。
    let facing = if moved.length_squared() > 1e-5 {
        moved.normalize()
    } else {
        (yaw_rot * Vec3::NEG_Z).normalize()
    };
    tf.rotation = Quat::from_rotation_y(facing.x.atan2(facing.z) + std::f32::consts::PI);

    // --- カメラ追従 ---
    let eye = result.position + Vec3::Y * 1.62;
    let look_rot = Quat::from_rotation_y(cam.yaw) * Quat::from_rotation_x(-cam.pitch);
    // 一人称のときは自分の体を隠す。カメラは目の高さ＝頭の内側にあるので、
    // 描いたままだと自分の頭の裏側で画面が埋まってしまう。
    let mut first_person = cam.perspective == Perspective::First;

    if first_person {
        cam_tf.translation = eye;
        cam_tf.rotation = look_rot;
    } else {
        // 三人称・二人称：壁にめり込まないよう、視線方向へ飛ばして手前で止める。
        // 二人称は前方へ回り込むだけなので、同じ処理を符号違いで使える。
        let back = (look_rot * Vec3::Z) * cam.perspective.back_sign();
        let mut dist = cam.distance;
        if let Some(hit) = raycast_blocks(&world, eye, back, cam.distance + 0.4) {
            dist = hit.distance - 0.35;
        }
        // 頭の揺れ。歩くとわずかに上下する。
        let bob = if settings.view_bobbing && actor.grounded {
            (time.elapsed_seconds() * 9.0).sin() * (actor.last_speed * 0.008).min(0.06)
        } else {
            0.0
        };

        if dist < MIN_THIRD_PERSON_DISTANCE {
            // 引ける余地が無い（屋内・狭い坑道）ときに無理へ引くと、
            // 自分の後頭部で画面が埋まってしまう。そういう場所では一人称へ落とす。
            cam_tf.translation = eye + Vec3::Y * bob;
            first_person = true;
        } else {
            cam_tf.translation = eye + back * dist + Vec3::Y * bob;
        }
        cam_tf.rotation = if cam.perspective == Perspective::ThirdFront {
            // 正面視点：プレイヤーの方を向き直す。
            let mut t = Transform::from_translation(cam_tf.translation);
            t.look_at(eye, Vec3::Y);
            t.rotation
        } else {
            look_rot
        };
    }

    let wanted = if first_person { Visibility::Hidden } else { Visibility::Inherited };
    if *visibility != wanted {
        *visibility = wanted;
    }
}

/// 空腹・体温・体力の推移。
pub fn player_vitals_system(
    time: Res<Time>,
    world_time: Res<WorldTime>,
    world: Res<VoxelWorld>,
    mut chronicle: ResMut<LocalChronicle>,
    mut query: Query<(&Transform, &mut Player, &mut Health, &Actor)>,
) {
    let dt = time.delta_seconds();
    let gh = (dt as f64 * TICKS_PER_REAL_SECOND * world_time.speed as f64 / 3600.0) as f32;

    let Ok((tf, mut player, mut health, actor)) = query.get_single_mut() else { return };

    player.age_days += gh / 24.0;
    player.hunger = (player.hunger - gh * 2.6).clamp(0.0, 100.0);

    // --- 体温 ---
    // 気温はバイオームの平均値に、標高と昼夜の差を乗せて求める。
    let pos = tf.translation;
    let cp = ChunkPos::from_world(pos.x, pos.z);
    let biome = world
        .chunks
        .get(&cp)
        .map(|c| {
            ALL_BIOMES
                .get(c.biome_at(pos.x.floor() as i32 - cp.origin().0, pos.z.floor() as i32 - cp.origin().1) as usize)
                .copied()
                .unwrap_or(Biome::Plains)
        })
        .unwrap_or(Biome::Plains);
    let bdef = biome_def(biome);

    let altitude = (pos.y - SEA_LEVEL as f32).max(0.0);
    let lapse = altitude * 0.065; // 100m でおよそ 6.5℃（現実の気温減率）
    let diurnal = world_time.sun_elevation() * 5.0;
    let mut ambient = bdef.temperature_c - lapse + diurnal;
    if actor.in_liquid {
        ambient -= 12.0;
    }

    // 体はゆっくり周囲の温度へ引かれ、代謝で 36.5℃ へ戻ろうとする。
    let toward_ambient = (ambient - 20.0) * 0.004;
    let metabolism = (36.5 - player.body_temp) * 0.25;
    player.body_temp = (player.body_temp + (toward_ambient + metabolism) * dt).clamp(28.0, 42.0);

    // --- 体力 ---
    if player.hunger <= 0.0 {
        health.damage(1.6 * dt);
    } else if player.hunger > 65.0 && health.current < health.max {
        health.heal(0.9 * dt);
    }
    if player.body_temp < 34.0 {
        health.damage((34.0 - player.body_temp) * 0.9 * dt);
    }
    if player.body_temp > 39.5 {
        health.damage((player.body_temp - 39.5) * 0.9 * dt);
    }

    if health.is_dead() {
        chronicle.record(
            world_time.tick,
            "あなたは死んだ",
            "だが世界は、何事もなかったかのように続いていく。",
            1.0,
        );
        // 死んでも世界は止まらない。近くの安全な場所で目を覚ます。
        health.current = health.max * 0.35;
        player.hunger = 45.0;
        player.body_temp = 36.5;
    }
}

// ======================================================================
// 採掘・設置・攻撃・対話
// ======================================================================

/// 操作系が触るリソースをひとまとめにする（システム引数の上限対策）。
#[derive(SystemParam)]
pub struct InteractionCtx<'w> {
    pub settings: Res<'w, GameSettings>,
    pub blocks: Res<'w, BlockRegistry>,
    pub items: Res<'w, ItemRegistry>,
    pub world_time: Res<'w, WorldTime>,
    pub mining: ResMut<'w, MiningState>,
    pub dialogue: ResMut<'w, DialogueState>,
    pub toast: ResMut<'w, Toast>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub materials: ResMut<'w, Assets<StandardMaterial>>,
}

#[allow(clippy::too_many_arguments)]
pub fn player_interaction_system(
    mut commands: Commands,
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut world: ResMut<VoxelWorld>,
    mut ctx: InteractionCtx,
    mut player: Query<(Entity, &Transform, &mut Player, &mut Inventory, &mut Health, &mut LimbAnimator)>,
    camera: Query<&Transform, (With<PlayerCamera>, Without<Player>)>,
    npcs: Query<(Entity, &Transform, &Npc)>,
    mut creatures: Query<(Entity, &Transform, &Wildlife, &mut Health), (Without<Player>, Without<Npc>)>,
) {
    let dt = time.delta_seconds();
    let InteractionCtx {
        settings,
        blocks,
        items,
        world_time,
        ref mut mining,
        ref mut dialogue,
        ref mut toast,
        ref mut meshes,
        ref mut materials,
    } = ctx;
    let Ok((player_entity, tf, mut player_state, mut inventory, mut health, mut anim)) = player.get_single_mut() else { return };
    let Ok(cam_tf) = camera.get_single() else { return };

    let eye = tf.translation + Vec3::Y * 1.62;
    let dir = *cam_tf.forward();
    let reach = 5.5;

    let held_key = inventory.get(player_state.selected_slot).map(|s| s.key.clone());
    let held = held_key.as_deref().and_then(|k| items.get(k));

    // --- [F] 会話 ---
    if settings.keybinds.just_pressed(Action::Interact, &keys) {
        if dialogue.speaker.is_some() {
            dialogue.speaker = None;
            dialogue.text.clear();
        } else if let Some((entity, _)) = crate::ai::find_nearest_npc(&npcs, tf.translation, 4.5) {
            if let Ok((_, _, npc)) = npcs.get(entity) {
                let threat_near = creatures.iter().any(|(_, ctf, w, h)| {
                    w.def().is_predator() && !h.is_dead() && ctf.translation.distance(tf.translation) < 22.0
                });
                dialogue.speaker = Some(entity);
                dialogue.name = format!("{}（{} / {}歳 / {}）", npc.name, npc.profession, npc.age, npc.personality.label());
                dialogue.text = crate::ai::npc_dialogue(npc, &world_time, threat_near);
            }
        } else {
            toast.show("話しかけられる相手が近くにいません。", crate::ui_theme::C_TEXT_DIM);
        }
    }

    // --- [左クリック] 採掘 / 攻撃 ---
    if mouse.pressed(MouseButton::Left) {
        // 生き物が射線上にいれば攻撃を優先する。
        let mut attacked = false;
        if mouse.just_pressed(MouseButton::Left) {
            anim.attack_timer = 0.3;
            let damage = held.map(|d| d.damage).unwrap_or(2.0);
            for (entity, ctf, w, mut chealth) in creatures.iter_mut() {
                if chealth.is_dead() {
                    continue;
                }
                let to = ctf.translation + Vec3::Y * w.def().height * 0.5 - eye;
                if to.length() < reach && to.normalize_or_zero().dot(dir) > 0.80 {
                    chealth.damage(damage);
                    attacked = true;
                    if chealth.is_dead() {
                        for (item, qty) in w.def().drops {
                            let max_stack = items.get(item).map(|d| d.max_stack).unwrap_or(64);
                            inventory.add(item, *qty, max_stack);
                        }
                        commands.entity(entity).insert(Dying { timer: 1.5 });
                        toast.show(format!("{}を仕留めた。", w.def().display_name), C_OK);
                    }
                    break;
                }
            }
        }

        if !attacked {
            if let Some(hit) = raycast_blocks(&world, eye, dir, reach) {
                let block = world
                    .block_at(hit.block.x, hit.block.y, hit.block.z)
                    .unwrap_or(ids::AIR);
                let def = blocks.get(block);

                if def.hardness < 0.0 {
                    mining.target = None;
                } else {
                    // 対象が変わったら進行をやり直す。
                    if mining.target != Some(hit.block) {
                        mining.target = Some(hit.block);
                        mining.progress = 0.0;
                        // 道具が合っていれば速く掘れる。
                        let power = match held {
                            Some(item) if item.tool == def.tool && def.tool != ToolClass::None => item.tool_power,
                            Some(item) if item.tool != ToolClass::None => 1.0,
                            _ => 0.7,
                        };
                        mining.required = (def.hardness / power).max(0.05);
                    }
                    mining.progress += dt;

                    if mining.progress >= mining.required {
                        let drop_key = def.drop_key.clone().unwrap_or_else(|| def.key.clone());
                        world.set_block(hit.block.x, hit.block.y, hit.block.z, ids::AIR);
                        if let Some(item) = items.get(&drop_key) {
                            let leftover = inventory.add(&drop_key, 1, item.max_stack);
                            if leftover > 0 {
                                toast.show("持ち物がいっぱいです。", crate::ui_theme::C_WARN);
                            }
                        }
                        mining.target = None;
                        mining.progress = 0.0;
                    }
                }
            } else {
                mining.target = None;
            }
        }
    } else {
        mining.target = None;
        mining.progress = 0.0;
    }

    // --- [右クリック] 設置 / 使用 ---
    if mouse.just_pressed(MouseButton::Right) {
        let mut consumed = false;

        if let Some(item) = held {
            // 食べる。
            if item.nutrition > 0.0 && player_state.hunger < 99.0 {
                player_state.hunger = (player_state.hunger + item.nutrition).min(100.0);
                health.heal(item.nutrition * 0.25);
                inventory.consume_one(player_state.selected_slot);
                toast.show(format!("{}を食べた。", item.display_name), C_OK);
                consumed = true;
            }
            // 弓を射る。
            else if item.key == "genesis:bow" {
                if inventory.take_one("genesis:arrow") {
                    anim.attack_timer = 0.35;
                    let spawn = eye + dir * 0.9;
                    commands.spawn((
                        PbrBundle {
                            mesh: meshes.add(Cuboid::new(0.06, 0.06, 0.9)),
                            material: materials.add(StandardMaterial {
                                base_color: Color::rgb(0.85, 0.76, 0.42),
                                ..default()
                            }),
                            transform: Transform::from_translation(spawn)
                                .looking_to(dir, Vec3::Y),
                            ..default()
                        },
                        Projectile {
                            velocity: dir * 46.0,
                            lifetime: 6.0,
                            damage: 22.0,
                            owner: Some(player_entity),
                        },
                        WorldEntity,
                    ));
                } else {
                    toast.show("矢がありません。", crate::ui_theme::C_WARN);
                }
                consumed = true;
            }
            // ブロックを置く。
            else if let Some(block) = item.places {
                let at = hit_place_target(&world, eye, dir, reach);
                if let Some(at) = at {
                    // 自分の体の中には置けない。
                    let feet = tf.translation;
                    let occupies = (at.x as f32 - feet.x).abs() < 0.9
                        && (at.z as f32 - feet.z).abs() < 0.9
                        && at.y as f32 >= feet.y - 1.0
                        && (at.y as f32) < feet.y + 1.9;
                    if occupies {
                        toast.show("そこには置けません。", crate::ui_theme::C_WARN);
                    } else if world.set_block(at.x, at.y, at.z, block) {
                        inventory.consume_one(player_state.selected_slot);
                    }
                }
                consumed = true;
            }
        }

        if !consumed && held.is_none() {
            toast.show("手に何も持っていません。", crate::ui_theme::C_TEXT_DIM);
        }
    }
}

fn hit_place_target(world: &VoxelWorld, eye: Vec3, dir: Vec3, reach: f32) -> Option<IVec3> {
    let hit = raycast_blocks(world, eye, dir, reach)?;
    // 手前の空きマスへ置く。
    let at = hit.adjacent;
    match world.block_at(at.x, at.y, at.z) {
        Some(b) if b.is_air() || world.lookup.is_liquid(b) => Some(at),
        _ => None,
    }
}

/// 矢の飛翔と命中。
pub fn projectile_system(
    mut commands: Commands,
    time: Res<Time>,
    world: Res<VoxelWorld>,
    items: Res<ItemRegistry>,
    mut toast: ResMut<Toast>,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    mut targets: Query<(Entity, &Transform, Option<&Wildlife>, &mut Health), Without<Projectile>>,
    mut inventory: Query<&mut Inventory, With<Player>>,
) {
    let dt = time.delta_seconds().min(0.05);

    for (entity, mut tf, mut proj) in projectiles.iter_mut() {
        proj.lifetime -= dt;
        proj.velocity.y -= 19.0 * dt;

        let step = proj.velocity * dt;
        let next = tf.translation + step;

        // 地形への命中。
        let blocked = world.is_solid_at(
            next.x.floor() as i32,
            next.y.floor() as i32,
            next.z.floor() as i32,
        );

        // 生き物への命中。
        let mut hit_something = false;
        for (target, target_tf, wildlife, mut health) in targets.iter_mut() {
            if Some(target) == proj.owner || health.is_dead() {
                continue;
            }
            let height = wildlife.map(|w| w.def().height).unwrap_or(1.8);
            let center = target_tf.translation + Vec3::Y * height * 0.5;
            if center.distance(next) < 0.6 + height * 0.35 {
                health.damage(proj.damage);
                hit_something = true;
                if health.is_dead() {
                    if let Some(w) = wildlife {
                        if let Ok(mut inv) = inventory.get_single_mut() {
                            for (item, qty) in w.def().drops {
                                let max_stack = items.get(item).map(|d| d.max_stack).unwrap_or(64);
                                inv.add(item, *qty, max_stack);
                            }
                        }
                        toast.show(format!("{}を射止めた。", w.def().display_name), C_OK);
                    }
                    commands.entity(target).insert(Dying { timer: 1.5 });
                }
                break;
            }
        }

        if hit_something || blocked || proj.lifetime <= 0.0 || next.y < 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        tf.translation = next;
        if proj.velocity.length_squared() > 1e-4 {
            tf.look_to(proj.velocity.normalize(), Vec3::Y);
        }
    }
}

// ======================================================================
// 設定の反映とセーブ
// ======================================================================

/// 手に持っている物が光るなら、その明かりを灯す。
///
/// 一人称ではプレイヤーモデルごと隠れてしまうため、明かりだけは
/// 見た目に依らず常に点けておく（暗い坑道で松明が効かないと理不尽なので）。
pub fn held_light_system(
    blocks: Res<BlockRegistry>,
    items: Res<ItemRegistry>,
    player: Query<(&Player, &Inventory)>,
    mut lights: Query<(&mut PointLight, &mut Visibility), With<HeldLight>>,
) {
    let Ok((state, inventory)) = player.get_single() else { return };

    // 選択中のスロットにある物の発光量を調べる。
    let emission = inventory
        .get(state.selected_slot)
        .and_then(|stack| items.get(&stack.key))
        .and_then(|item| item.places)
        .map(|block| blocks.get(block).light)
        .unwrap_or(0);

    for (mut light, mut visibility) in lights.iter_mut() {
        // 発光量 0〜15 を明るさへ写す。
        light.intensity = if emission > 0 {
            120_000.0 * (emission as f32 / 15.0)
        } else {
            0.0
        };
        light.range = 6.0 + emission as f32;
        // 親（プレイヤーモデル）が一人称で隠れても、明かりは灯し続ける。
        *visibility = Visibility::Visible;
    }
}

/// 設定画面で変えた値を実行中の世界へ反映する。
pub fn apply_settings_system(
    settings: Res<GameSettings>,
    mut config: ResMut<StreamConfig>,
    mut camera: Query<(&mut Projection, &mut PlayerCamera)>,
) {
    if !settings.is_changed() {
        return;
    }
    config.render_distance = settings.render_distance;
    config.upload_budget = settings.chunk_upload_budget;

    for (mut projection, mut cam) in camera.iter_mut() {
        if let Projection::Perspective(p) = projection.as_mut() {
            p.fov = settings.fov_degrees.to_radians();
        }
        // 設定側の三人称トグルは「既定の視点」を決めるだけで、
        // ゲーム中の F5 による切り替えを上書きしない。
        cam.distance = cam.distance.clamp(1.5, 9.0);
    }
}

/// 自動セーブと、明示的な保存要求の処理。
#[allow(clippy::too_many_arguments)]
pub fn save_system(
    time: Res<Time>,
    settings: Res<GameSettings>,
    save_mgr: Res<SaveManagerRes>,
    world: Res<VoxelWorld>,
    world_time: Res<WorldTime>,
    chronicle: Res<LocalChronicle>,
    mut active: ResMut<ActiveWorld>,
    mut request: ResMut<SaveRequest>,
    mut toast: ResMut<Toast>,
    mut next_state: ResMut<NextState<AppState>>,
    mut dirty: ResMut<UiDirty>,
    player: Query<(&Transform, &Player, &Health, &Inventory)>,
    camera: Query<&PlayerCamera>,
) {
    // --- 自動セーブのタイマー ---
    let mut should_save = request.save;
    if settings.autosave_minutes > 0.0 {
        active.autosave_timer += time.delta_seconds();
        if active.autosave_timer >= settings.autosave_minutes * 60.0 {
            active.autosave_timer = 0.0;
            should_save = true;
        }
    }
    if !should_save {
        return;
    }
    request.save = false;

    let Ok((tf, player_state, health, inventory)) = player.get_single() else { return };
    let cam = camera.get_single().ok();

    let player_save = PlayerSave {
        x: tf.translation.x,
        y: tf.translation.y,
        z: tf.translation.z,
        yaw: cam.map(|c| c.yaw).unwrap_or(0.0),
        pitch: cam.map(|c| c.pitch).unwrap_or(0.0),
        health: health.current,
        hunger: player_state.hunger,
        body_temp: player_state.body_temp,
        age_days: player_state.age_days,
        money: player_state.money,
        arrows: inventory.count_of("genesis:arrow"),
        selected_slot: player_state.selected_slot,
        hotbar: inventory.to_save(),
        inventory: Vec::new(),
        profession: player_state.profession.clone(),
        reputation: player_state.reputation,
        discovered_settlements: Vec::new(),
    };

    let modified: Vec<_> = world.modified_chunks().map(|c| c.to_palette_rle()).collect();
    let chunk_count = modified.len();

    let body = WorldSaveBody {
        format_version: SAVE_FORMAT_VERSION,
        player: player_save,
        modified_chunks: modified,
        chronicle: chronicle.to_save(),
    };

    active.meta.sim_tick = world_time.tick;
    active.meta.played_seconds += active.session_seconds;
    active.session_seconds = 0.0;
    active.meta.last_played_unix = crate::saves::now_unix();

    let folder = active.meta.folder.clone();
    let mut ok = true;
    if let Err(e) = save_mgr.0.write_body(&folder, &body) {
        toast.show(format!("保存に失敗しました: {e}"), C_ERR);
        ok = false;
    }
    if let Err(e) = save_mgr.0.write_meta(&active.meta) {
        toast.show(format!("世界情報の保存に失敗しました: {e}"), C_ERR);
        ok = false;
    }
    if ok {
        toast.show(
            format!("保存しました（改変チャンク {chunk_count} 個）。"),
            C_OK,
        );
    }

    if request.quit_after {
        request.quit_after = false;
        dirty.0 = true;
        next_state.set(AppState::Title);
    }
}

/// 読み込み画面での進捗表示。
pub fn loading_progress_system(
    world: Option<Res<VoxelWorld>>,
    mut text: Query<&mut Text, With<crate::menu::LoadingProgressText>>,
) {
    let Some(world) = world else { return };
    for mut t in text.iter_mut() {
        if let Some(section) = t.sections.first_mut() {
            section.value = format!(
                "チャンク生成 {} / 描画 {} ・ 待機中 {}",
                world.stats.loaded_chunks, world.stats.rendered_chunks, world.stats.pending_gen
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::GenParams;

    fn world_with(params: GenParams, seed: u64) -> VoxelWorld {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        VoxelWorld::new(WorldGenerator::new(seed, params), lookup)
    }

    #[test]
    fn spawn_point_is_always_on_habitable_land() {
        for seed in [1u64, 42, 9999, 0xDEAD_BEEF, 777_777] {
            let w = world_with(GenParams::default(), seed);
            let (x, z) = find_habitable_spawn(&w);
            let h = w.generator.terrain_height(x as f32, z as f32);
            assert!(
                h > w.generator.params.sea_level,
                "seed {seed} spawned the player under water at ({x},{z}), h={h}"
            );
        }
    }

    #[test]
    fn spawn_point_is_reachable_even_with_no_settlements() {
        let params = GenParams {
            settlement_density: 0.0,
            ..GenParams::default()
        };
        let w = world_with(params, 31337);
        let (x, z) = find_habitable_spawn(&w);
        let h = w.generator.terrain_height(x as f32, z as f32);
        assert!(h > w.generator.params.sea_level);
    }

    #[test]
    fn priming_generates_the_chunks_around_the_spawn() {
        let mut w = world_with(GenParams::default(), 5);
        assert!(w.chunks.is_empty());
        prime_chunks_around(&mut w, ChunkPos::new(0, 0), 1);
        assert_eq!(w.chunks.len(), 9);
        // 中心と 4 隣接が揃っていること（メッシュ生成の前提）。
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            assert!(w.chunks.contains_key(&ChunkPos::new(dx, dz)));
        }
    }

    #[test]
    fn priming_is_idempotent() {
        let mut w = world_with(GenParams::default(), 5);
        prime_chunks_around(&mut w, ChunkPos::new(0, 0), 1);
        let first = w.chunks[&ChunkPos::new(0, 0)].clone();
        prime_chunks_around(&mut w, ChunkPos::new(0, 0), 1);
        let second = w.chunks[&ChunkPos::new(0, 0)].clone();
        assert!(Arc::ptr_eq(&first, &second), "priming regenerated an existing chunk");
    }

    #[test]
    fn placement_target_is_the_free_cell_in_front_of_the_hit() {
        let params = GenParams {
            flat_world: true,
            cave_density: 0.0,
            vegetation_density: 0.0,
            settlement_density: 0.0,
            ..GenParams::default()
        };
        let mut w = world_with(params, 1);
        prime_chunks_around(&mut w, ChunkPos::new(0, 0), 1);

        let ground = w.ground_height(4, 4);
        let eye = Vec3::new(4.5, ground as f32 + 4.0, 4.5);
        let at = hit_place_target(&w, eye, Vec3::NEG_Y, 8.0).expect("should find a placement cell");
        assert_eq!(at.y, ground + 1, "block would be placed inside the ground");
        assert_eq!(w.block_at(at.x, at.y, at.z), Some(ids::AIR));
    }

    #[test]
    fn placement_finds_nothing_when_aiming_at_the_sky() {
        let params = GenParams {
            flat_world: true,
            cave_density: 0.0,
            vegetation_density: 0.0,
            settlement_density: 0.0,
            ..GenParams::default()
        };
        let mut w = world_with(params, 1);
        prime_chunks_around(&mut w, ChunkPos::new(0, 0), 1);
        let ground = w.ground_height(4, 4);
        let eye = Vec3::new(4.5, ground as f32 + 2.0, 4.5);
        assert!(hit_place_target(&w, eye, Vec3::Y, 6.0).is_none());
    }
}
