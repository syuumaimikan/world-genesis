//! 村人と野生動物の湧き出し・実行・消滅。
//!
//! プレイヤーの周囲だけに実体（エンティティ）を置き、遠ざかれば消す。
//! ただし世界そのものが消えるわけではない：村は `village.rs` の計画から
//! いつでも同じ姿で復元されるし、動物はバイオームの生息表から湧き直す。
//! これが仕様でいう Simulation LOD の最下層にあたる。

use crate::actors::*;
use crate::biome::{biome_def, Biome, ALL_BIOMES};
use crate::blocky::{build_creature, build_humanoid, BlockyAssets, HumanoidSkin, LimbAnimator};
use crate::chunk::{ChunkPos, CHUNK_SX, CHUNK_SZ};
use crate::noise::{hash2i, hash_u64};
use crate::physics::{find_spawn_y, move_body, BodyShape};
use crate::species::{species_by_key, SpeciesDef, SPECIES};
use crate::streaming::{StreamConfig, VoxelWorld};
use crate::village::VillagePlan;
use bevy::prelude::*;
use std::collections::HashSet;

/// 実体化した村・動物の管理。
#[derive(Resource, Default)]
pub struct PopulationTracker {
    /// 既に村人を配置した集落の ID。
    pub spawned_villages: HashSet<u64>,
    /// 動物を湧かせたチャンク。
    pub stocked_chunks: HashSet<ChunkPos>,
    pub npc_count: usize,
    pub creature_count: usize,
    /// 動物の総数上限。
    pub creature_cap: usize,
    pub npc_cap: usize,
}

impl PopulationTracker {
    pub fn new() -> Self {
        Self {
            creature_cap: 140,
            npc_cap: 180,
            ..Default::default()
        }
    }
}

/// この村の住人であることを示す。
#[derive(Component)]
pub struct VillageMember(pub u64);

// ======================================================================
// 村人の配置
// ======================================================================

#[allow(clippy::too_many_arguments)]
pub fn spawn_village_npcs_system(
    mut commands: Commands,
    world: Res<VoxelWorld>,
    config: Res<StreamConfig>,
    mut tracker: ResMut<PopulationTracker>,
    mut blocky: ResMut<BlockyAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(player_tf) = player.get_single() else { return };
    if tracker.npc_count >= tracker.npc_cap {
        return;
    }

    let px = player_tf.translation.x as i32;
    let pz = player_tf.translation.z as i32;
    // 描画範囲に入っている集落だけを実体化する。
    let plans = world
        .generator
        .villages
        .plans_around(&world.generator, px, pz, 1);

    let spawn_radius = (config.render_distance * CHUNK_SX) as f32;

    for plan in plans {
        if tracker.spawned_villages.contains(&plan.id) {
            continue;
        }
        let dist = Vec2::new(plan.center_x as f32 - px as f32, plan.center_z as f32 - pz as f32).length();
        if dist > spawn_radius {
            continue;
        }
        // 中心のチャンクがまだ生成されていないなら、地面の高さが確定しない。
        let center_chunk = ChunkPos::from_world(plan.center_x as f32, plan.center_z as f32);
        if !world.chunks.contains_key(&center_chunk) {
            continue;
        }

        spawn_one_village(
            &mut commands,
            &world,
            &mut blocky,
            &mut materials,
            &plan,
            &mut tracker,
        );
        tracker.spawned_villages.insert(plan.id);
    }
}

fn spawn_one_village(
    commands: &mut Commands,
    world: &VoxelWorld,
    blocky: &mut BlockyAssets,
    materials: &mut Assets<StandardMaterial>,
    plan: &VillagePlan,
    tracker: &mut PopulationTracker,
) {
    let spawns = plan.npc_spawns();
    if spawns.is_empty() {
        return;
    }

    let plaza = Vec3::new(
        plan.center_x as f32 + 0.5,
        plan.ground_y as f32 + 1.0,
        plan.center_z as f32 + 0.5,
    );
    // 井戸を水場とみなす（無ければ広場）。
    let water = plan
        .buildings
        .iter()
        .find(|b| b.kind == crate::village::BuildingKind::Well)
        .map(|b| {
            let (x, z) = b.center();
            Vec3::new(x as f32 + 0.5, b.floor_y as f32 + 1.0, z as f32 + 0.5)
        })
        .unwrap_or(plaza);

    for (i, (sx, _sy, sz, profession)) in spawns.iter().enumerate() {
        if tracker.npc_count >= tracker.npc_cap {
            break;
        }
        let person_id = hash_u64(plan.id ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));

        // 家族構成：世帯主のほかに配偶者・子供が住むことがある。
        let household = 1 + (person_id % 3) as usize;
        for member in 0..household {
            if tracker.npc_count >= tracker.npc_cap {
                break;
            }
            let mid = hash_u64(person_id ^ (member as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
            let gender = if mid % 2 == 0 { Gender::Male } else { Gender::Female };
            let is_child = member == 2;
            let age = if is_child {
                4 + (mid % 12) as u32
            } else {
                19 + (mid % 46) as u32
            };
            let personality = Personality::from_hash(mid >> 8);
            let prof: String = if is_child {
                "子供".to_string()
            } else if member == 0 {
                profession.to_string()
            } else {
                // 配偶者は別の仕事に就く。
                ["農民", "織り手", "木こり", "漁師", "行商人"][(mid >> 16) as usize % 5].to_string()
            };

            let home = Vec3::new(*sx as f32 + 0.5, world.ground_height(*sx, *sz) as f32 + 1.0, *sz as f32 + 0.5);
            // 職場：世帯主は自分の建物、それ以外は農地か広場。
            let workplace = if member == 0 {
                home
            } else if let Some(&(fx, fz, fw, fd)) = plan.farms.get(member % plan.farms.len().max(1)) {
                let (cx, cz) = (fx + fw / 2, fz + fd / 2);
                Vec3::new(cx as f32 + 0.5, world.ground_height(cx, cz) as f32 + 1.0, cz as f32 + 0.5)
            } else {
                plaza
            };

            let stature = if is_child { 1.15 + (mid % 20) as f32 * 0.01 } else { 1.70 + (mid % 22) as f32 * 0.008 };
            // NPC は経路探索を持たないため、段差を自動で越えられないと
            // 村の縁石や畑の畝で延々と足踏みしてしまう。プレイヤーと違い
            // 小さな段差だけは自動で上がれるようにしておく。
            let shape = BodyShape {
                half_width: 0.30,
                height: stature,
                step_height: 1.05,
            };
            let spawn_y = find_spawn_y(world, *sx, *sz, shape);
            let pos = Vec3::new(*sx as f32 + 0.5, spawn_y, *sz as f32 + 0.5);

            let skin = HumanoidSkin::from_hash(mid).with_profession(&prof);
            let name = generate_name(mid, gender);

            let mut npc = Npc {
                name: name.clone(),
                gender,
                personality,
                profession: prof.clone(),
                age,
                village_id: plan.id,
                village_name: plan.name.clone(),
                home,
                workplace,
                water,
                plaza,
                hunger: 55.0 + (mid % 40) as f32,
                fatigue: (mid >> 4) as f32 % 40.0,
                social: (mid >> 12) as f32 % 60.0,
                fear: 0.0,
                activity: Activity::Wander,
                decision_cooldown: (mid % 100) as f32 / 100.0,
                attack_cooldown: 0.0,
                memories: Vec::new(),
            };
            // 生まれ育ちに応じた記憶を持たせる。会話でこれが語られる。
            npc.remember(format!("私は{}の生まれだ。", plan.name));
            npc.remember(match plan.biome {
                Biome::RockyMountains | Biome::SnowyPeaks => "北の岩山では、冬に道が雪で閉ざされる。".to_string(),
                Biome::Desert | Biome::RedDesert => "井戸の水は貴重だ。無駄にしてはいけない。".to_string(),
                Biome::Jungle | Biome::BambooJungle => "森の奥からは、夜になると獣の声がする。".to_string(),
                Biome::Swamp | Biome::Mangrove => "湿地の霧は病を運ぶという。".to_string(),
                b => format!("この辺りは{}だ。作物はよく育つ。", biome_def(b).display_name),
            });

            let walk_speed = if is_child { 3.2 } else { 2.6 + (mid % 10) as f32 * 0.05 };

            commands
                .spawn((
                    SpatialBundle::from_transform(Transform::from_translation(pos)),
                    npc,
                    Actor::new(shape, walk_speed),
                    Health::new(if is_child { 45.0 } else { 100.0 }),
                    LimbAnimator::default(),
                    VillageMember(plan.id),
                    Nameplate {
                        text: format!("{name}（{prof}）"),
                        color: Color::rgb(0.92, 0.94, 0.98),
                    },
                ))
                .with_children(|parent| {
                    build_humanoid(parent, blocky, materials, skin, stature);
                });

            tracker.npc_count += 1;
        }
    }
}

/// 遠く離れた村人を片付ける。次に近づけば同じ村がまた実体化する。
pub fn despawn_far_npcs_system(
    mut commands: Commands,
    config: Res<StreamConfig>,
    mut tracker: ResMut<PopulationTracker>,
    player: Query<&Transform, With<Player>>,
    npcs: Query<(Entity, &Transform, &VillageMember), With<Npc>>,
) {
    let Ok(player_tf) = player.get_single() else { return };
    let limit = ((config.render_distance + 3) * CHUNK_SX) as f32;
    let limit_sq = limit * limit;

    let mut still_present: HashSet<u64> = HashSet::new();
    let mut removed = 0usize;

    for (entity, tf, member) in npcs.iter() {
        let d = tf.translation.distance_squared(player_tf.translation);
        if d > limit_sq {
            commands.entity(entity).despawn_recursive();
            removed += 1;
        } else {
            still_present.insert(member.0);
        }
    }

    tracker.npc_count = tracker.npc_count.saturating_sub(removed);
    // 全員消えた村は「未配置」に戻し、戻ってきたときに再び住民が現れるようにする。
    tracker.spawned_villages.retain(|id| still_present.contains(id));
}

// ======================================================================
// 野生動物の湧き出し
// ======================================================================

#[derive(Resource)]
pub struct SpawnTimers {
    pub wildlife: Timer,
}

impl Default for SpawnTimers {
    fn default() -> Self {
        Self {
            wildlife: Timer::from_seconds(1.5, TimerMode::Repeating),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_wildlife_system(
    mut commands: Commands,
    time: Res<Time>,
    world_time: Res<WorldTime>,
    world: Res<VoxelWorld>,
    config: Res<StreamConfig>,
    mut timers: ResMut<SpawnTimers>,
    mut tracker: ResMut<PopulationTracker>,
    mut blocky: ResMut<BlockyAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&Transform, With<Player>>,
) {
    if !timers.wildlife.tick(time.delta()).just_finished() {
        return;
    }
    let Ok(player_tf) = player.get_single() else { return };
    if tracker.creature_count >= tracker.creature_cap {
        return;
    }

    let center = ChunkPos::from_world(player_tf.translation.x, player_tf.translation.z);
    let r = config.render_distance.max(3);

    // 近すぎず遠すぎない環（プレイヤーの目の前に湧かない）。
    let mut candidates: Vec<ChunkPos> = Vec::new();
    for dz in -r..=r {
        for dx in -r..=r {
            let d2 = dx * dx + dz * dz;
            if !(9..=(r * r)).contains(&d2) {
                continue;
            }
            let p = ChunkPos::new(center.x + dx, center.z + dz);
            if world.chunks.contains_key(&p) && !tracker.stocked_chunks.contains(&p) {
                candidates.push(p);
            }
        }
    }
    if candidates.is_empty() {
        return;
    }

    // 1回の起動で最大 3 チャンクぶんだけ湧かせる。
    let pick_seed = world_time.tick ^ (center.x as u64) << 16 ^ (center.z as u64);
    for i in 0..3.min(candidates.len()) {
        let idx = (hash_u64(pick_seed ^ i as u64) % candidates.len() as u64) as usize;
        let cp = candidates[idx];
        tracker.stocked_chunks.insert(cp);
        stock_chunk(
            &mut commands,
            &world,
            &world_time,
            &mut blocky,
            &mut materials,
            cp,
            &mut tracker,
        );
        if tracker.creature_count >= tracker.creature_cap {
            break;
        }
    }
}

fn stock_chunk(
    commands: &mut Commands,
    world: &VoxelWorld,
    world_time: &WorldTime,
    blocky: &mut BlockyAssets,
    materials: &mut Assets<StandardMaterial>,
    cp: ChunkPos,
    tracker: &mut PopulationTracker,
) {
    let Some(chunk) = world.chunks.get(&cp) else { return };
    let (ox, oz) = cp.origin();
    let seed = world.seed;

    // このチャンクに群れが湧くかどうかを決定論的に決める。
    let h = hash2i(cp.x, cp.z, seed ^ 0xFA_11A);
    if (h % 100) > 42 {
        return; // 6 割弱のチャンクは無人
    }

    // 群れの中心となる列を選ぶ。
    let lx = ((h >> 8) % CHUNK_SX as u64) as i32;
    let lz = ((h >> 16) % CHUNK_SZ as u64) as i32;
    let biome = ALL_BIOMES
        .get(chunk.biome_at(lx, lz) as usize)
        .copied()
        .unwrap_or(Biome::Plains);
    let fauna = biome_def(biome).fauna;
    if fauna.is_empty() {
        return;
    }

    let key = fauna[((h >> 24) % fauna.len() as u64) as usize];
    let Some(sp) = species_by_key(key) else { return };
    let Some(species_idx) = crate::species::species_index(key) else { return };

    // 夜行性の動物は夜に、昼行性は昼に多く湧く。
    let night = world_time.is_night();
    if night != sp.nocturnal && (h >> 32) % 100 > 35 {
        return;
    }

    let herd = sp.herd_size.0 + ((h >> 40) % (sp.herd_size.1 - sp.herd_size.0 + 1) as u64) as u32;
    let anchor_x = ox + lx;
    let anchor_z = oz + lz;
    let anchor = Vec3::new(
        anchor_x as f32 + 0.5,
        world.ground_height(anchor_x, anchor_z) as f32 + 1.0,
        anchor_z as f32 + 0.5,
    );

    for k in 0..herd {
        if tracker.creature_count >= tracker.creature_cap {
            break;
        }
        let kh = hash_u64(h ^ (k as u64).wrapping_mul(0x1656_67B1_9E37_79F9));
        let jitter_x = (kh % 9) as i32 - 4;
        let jitter_z = ((kh >> 8) % 9) as i32 - 4;
        let wx = anchor_x + jitter_x;
        let wz = anchor_z + jitter_z;

        let surface = world.ground_height(wx, wz);
        // 水生動物は水中に、陸生動物は水面より上に。
        let underwater = surface < world.generator.params.sea_level;
        if sp.is_aquatic() != underwater {
            continue;
        }

        let shape = shape_for_species(sp);
        let y = if underwater {
            (surface + 1) as f32
        } else {
            find_spawn_y(world, wx, wz, shape)
        };
        let pos = Vec3::new(wx as f32 + 0.5, y, wz as f32 + 0.5);

        spawn_creature(commands, blocky, materials, sp, species_idx, pos, anchor, kh);
        tracker.creature_count += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_creature(
    commands: &mut Commands,
    blocky: &mut BlockyAssets,
    materials: &mut Assets<StandardMaterial>,
    sp: &'static SpeciesDef,
    species_idx: usize,
    pos: Vec3,
    anchor: Vec3,
    h: u64,
) {
    let shape = shape_for_species(sp);
    commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(pos)),
            Wildlife {
                species: species_idx,
                hunger: 25.0 + (h % 50) as f32,
                energy: 60.0 + (h >> 8) as f32 % 40.0,
                state: FaunaState::Wander,
                decision_cooldown: (h % 200) as f32 / 100.0,
                attack_cooldown: 0.0,
                home_anchor: anchor,
            },
            Actor::new(shape, sp.speed),
            Health::new(sp.max_health),
            LimbAnimator::default(),
            Nameplate {
                text: sp.display_name.to_string(),
                color: if sp.is_predator() {
                    Color::rgb(0.95, 0.55, 0.45)
                } else {
                    Color::rgb(0.80, 0.90, 0.80)
                },
            },
        ))
        .with_children(|parent| {
            build_creature(parent, blocky, materials, sp);
        });
}

pub fn despawn_far_wildlife_system(
    mut commands: Commands,
    config: Res<StreamConfig>,
    mut tracker: ResMut<PopulationTracker>,
    player: Query<&Transform, With<Player>>,
    creatures: Query<(Entity, &Transform), With<Wildlife>>,
) {
    let Ok(player_tf) = player.get_single() else { return };
    let limit = ((config.render_distance + 3) * CHUNK_SX) as f32;
    let limit_sq = limit * limit;

    let mut removed = 0;
    for (entity, tf) in creatures.iter() {
        if tf.translation.distance_squared(player_tf.translation) > limit_sq {
            commands.entity(entity).despawn_recursive();
            removed += 1;
        }
    }
    tracker.creature_count = tracker.creature_count.saturating_sub(removed);

    // 遠ざかったチャンクは「未補充」へ戻す。
    let center = ChunkPos::from_world(player_tf.translation.x, player_tf.translation.z);
    let keep = (config.render_distance + 4).pow(2);
    tracker.stocked_chunks.retain(|p| p.distance_sq_to(center) <= keep);
}

// ======================================================================
// AI の実行
// ======================================================================

/// 脅威（捕食者）の位置を集める。毎フレーム全探索しないよう、
/// 1度だけ集めて全 NPC / 動物で使い回す。
#[derive(Resource, Default)]
pub struct ThreatBoard {
    /// (位置, 攻撃力, エンティティ)
    pub predators: Vec<(Vec3, f32, Entity)>,
    /// 草食動物など、捕食者にとっての獲物。
    pub prey: Vec<(Vec3, Entity, usize)>,
    /// 村人と player の位置（捕食者が襲う対象）。
    pub humans: Vec<(Vec3, Entity)>,
}

pub fn collect_threats_system(
    mut board: ResMut<ThreatBoard>,
    creatures: Query<(Entity, &Transform, &Wildlife, &Health)>,
    npcs: Query<(Entity, &Transform), With<Npc>>,
    player: Query<(Entity, &Transform), With<Player>>,
) {
    board.predators.clear();
    board.prey.clear();
    board.humans.clear();

    for (e, tf, w, health) in creatures.iter() {
        if health.is_dead() {
            continue;
        }
        let sp = w.def();
        if sp.is_predator() {
            board.predators.push((tf.translation, sp.attack, e));
        } else {
            board.prey.push((tf.translation, e, w.species));
        }
    }
    for (e, tf) in npcs.iter() {
        board.humans.push((tf.translation, e));
    }
    for (e, tf) in player.iter() {
        board.humans.push((tf.translation, e));
    }
}

#[inline]
fn nearest(list: &[(Vec3, f32, Entity)], from: Vec3) -> (f32, Option<Vec3>) {
    let mut best = f32::INFINITY;
    let mut pos = None;
    for (p, _, _) in list {
        let d = p.distance(from);
        if d < best {
            best = d;
            pos = Some(*p);
        }
    }
    (best, pos)
}

/// 村人の思考と行動。
pub fn npc_ai_system(
    time: Res<Time>,
    world_time: Res<WorldTime>,
    board: Res<ThreatBoard>,
    mut npcs: Query<(&mut Npc, &mut Actor, &Transform, &Health)>,
) {
    let dt = time.delta_seconds();
    // ゲーム内では現実より時間が速く流れるので、欲求もそれに合わせて動かす。
    let game_hours = dt as f64 * TICKS_PER_REAL_SECOND * world_time.speed as f64 / 3600.0;
    let gh = game_hours as f32;

    for (mut npc, mut actor, tf, health) in npcs.iter_mut() {
        // --- 欲求の変化 ---
        npc.hunger = (npc.hunger - gh * 4.0).clamp(0.0, 100.0);
        npc.fatigue = (npc.fatigue + gh * 4.5).clamp(0.0, 100.0);
        npc.social = (npc.social + gh * 3.0).clamp(0.0, 100.0);
        npc.attack_cooldown = (npc.attack_cooldown - dt).max(0.0);

        // 行動中の効果。
        match npc.activity {
            Activity::Eat => npc.hunger = (npc.hunger + gh * 40.0).min(100.0),
            Activity::Sleep => npc.fatigue = (npc.fatigue - gh * 14.0).max(0.0),
            Activity::Socialize => npc.social = (npc.social - gh * 25.0).max(0.0),
            _ => {}
        }

        // --- 脅威の探索 ---
        let (threat_dist, threat_pos) = nearest(&board.predators, tf.translation);
        npc.fear = if threat_dist < 20.0 {
            (npc.fear + dt * 2.0).min(1.0)
        } else {
            (npc.fear - dt * 0.4).max(0.0)
        };

        // --- 意思決定（一定間隔で再評価） ---
        npc.decision_cooldown -= dt;
        let threat_changed = threat_dist < 14.0 && !matches!(npc.activity, Activity::Flee | Activity::Fight);
        if npc.decision_cooldown <= 0.0 || threat_changed {
            let ctx = NpcContext {
                hour: world_time.hour(),
                hunger: npc.hunger,
                fatigue: npc.fatigue,
                social: npc.social,
                threat_distance: threat_dist,
                courage: npc.personality.courage(),
                sociability: npc.personality.sociability(),
                work_ethic: npc.personality.work_ethic(),
                can_fight: npc.can_fight(),
                health_fraction: health.fraction(),
            };
            let new_activity = choose_activity(&ctx);
            if new_activity != npc.activity {
                // 印象に残る出来事だけを記憶する。
                if matches!(new_activity, Activity::Flee) && npc.activity != Activity::Flee {
                    npc.remember("獣に追われて必死に逃げたことがある。");
                }
                if matches!(new_activity, Activity::Fight) {
                    npc.remember("村を襲った獣と、この手で戦った。");
                }
                npc.activity = new_activity;
            }
            // 危険が近いほど短い間隔で考え直す。
            npc.decision_cooldown = if threat_dist < 20.0 { 0.4 } else { 2.5 };
        }

        // --- 目的地 ---
        let target = activity_target(&npc, npc.activity, threat_pos, tf.translation);
        actor.move_target = target;
        actor.speed = match npc.activity {
            Activity::Flee => 5.6,
            Activity::Fight => 4.2,
            Activity::Sleep => 0.0,
            _ => 2.6,
        };
    }
}

/// 動物の思考と行動。
pub fn wildlife_ai_system(
    time: Res<Time>,
    world_time: Res<WorldTime>,
    board: Res<ThreatBoard>,
    mut creatures: Query<(Entity, &mut Wildlife, &mut Actor, &Transform, &Health)>,
) {
    let dt = time.delta_seconds();
    let gh = (dt as f64 * TICKS_PER_REAL_SECOND * world_time.speed as f64 / 3600.0) as f32;
    let night = world_time.is_night();

    for (entity, mut fauna, mut actor, tf, health) in creatures.iter_mut() {
        let sp = fauna.def();
        let pos = tf.translation;

        fauna.hunger = (fauna.hunger + gh * 5.0).clamp(0.0, 100.0);
        fauna.attack_cooldown = (fauna.attack_cooldown - dt).max(0.0);
        match fauna.state {
            FaunaState::Graze => fauna.hunger = (fauna.hunger - gh * 35.0).max(0.0),
            FaunaState::Rest => fauna.energy = (fauna.energy + gh * 20.0).min(100.0),
            FaunaState::Drink => fauna.energy = (fauna.energy + gh * 14.0).min(100.0),
            _ => fauna.energy = (fauna.energy - gh * 6.0).max(0.0),
        }

        // --- 周囲の把握 ---
        // 恐れる相手：自分より強い捕食者と人間。
        let mut threat_dist = f32::INFINITY;
        let mut threat_pos = None;
        if !sp.is_predator() || sp.max_health < 60.0 {
            for (p, _, e) in &board.predators {
                if *e == entity {
                    continue;
                }
                let d = p.distance(pos);
                if d < threat_dist {
                    threat_dist = d;
                    threat_pos = Some(*p);
                }
            }
        }
        if sp.flee_distance > 0.0 {
            for (p, _) in &board.humans {
                let d = p.distance(pos);
                if d < threat_dist {
                    threat_dist = d;
                    threat_pos = Some(*p);
                }
            }
        }

        // 捕食者にとっての獲物。
        let mut prey_dist = f32::INFINITY;
        let mut prey_pos = None;
        if sp.is_predator() {
            for (p, e, idx) in &board.prey {
                if *e == entity {
                    continue;
                }
                let other = &SPECIES[*idx];
                if !is_prey_of(sp, other) {
                    continue;
                }
                let d = p.distance(pos);
                if d < prey_dist {
                    prey_dist = d;
                    prey_pos = Some(*p);
                }
            }
            // 獲物が見当たらなければ人間を狙う。
            for (p, _) in &board.humans {
                let d = p.distance(pos);
                if d < prey_dist {
                    prey_dist = d;
                    prey_pos = Some(*p);
                }
            }
        }

        // --- 状態選択 ---
        fauna.decision_cooldown -= dt;
        if fauna.decision_cooldown <= 0.0 {
            let ctx = FaunaContext {
                hunger: fauna.hunger,
                energy: fauna.energy,
                threat_distance: threat_dist,
                prey_distance: prey_dist,
                is_night: night,
                nocturnal: sp.nocturnal,
                flee_distance: sp.flee_distance,
                is_predator: sp.is_predator(),
                health_fraction: health.fraction(),
            };
            fauna.state = choose_fauna_state(&ctx);
            fauna.decision_cooldown = if matches!(fauna.state, FaunaState::Flee | FaunaState::Hunt) {
                0.35
            } else {
                1.6
            };
        }

        // --- 目的地と速度 ---
        let (target, speed) = match fauna.state {
            FaunaState::Flee => {
                let away = threat_pos
                    .map(|t| (pos - t).normalize_or_zero())
                    .unwrap_or(Vec3::Z);
                (Some(pos + away * 20.0), sp.sprint_speed)
            }
            FaunaState::Hunt => (prey_pos, sp.sprint_speed),
            FaunaState::Rest => (None, 0.0),
            FaunaState::Drink | FaunaState::Graze | FaunaState::Wander => {
                // 群れの中心から離れすぎたら戻る。それ以外はゆっくり彷徨う。
                let from_home = pos.distance(fauna.home_anchor);
                if from_home > 34.0 {
                    (Some(fauna.home_anchor), sp.speed)
                } else {
                    let seed = hash_u64(entity.index() as u64 ^ (world_time.tick / 8));
                    let ang = (seed % 360) as f32 * std::f32::consts::PI / 180.0;
                    let wander = fauna.home_anchor + Vec3::new(ang.cos(), 0.0, ang.sin()) * 12.0;
                    (Some(wander), sp.speed * 0.6)
                }
            }
        };

        actor.move_target = target;
        actor.speed = speed;
    }
}

// ======================================================================
// 移動の実行
// ======================================================================

/// 目標へ向かって歩き、重力と衝突を適用する。
pub fn actor_movement_system(
    time: Res<Time>,
    world: Res<VoxelWorld>,
    mut actors: Query<(&mut Transform, &mut Actor), Without<Player>>,
) {
    let dt = time.delta_seconds().min(0.1);

    for (mut tf, mut actor) in actors.iter_mut() {
        let pos = tf.translation;

        // --- 水平方向の意図 ---
        let mut horizontal = Vec3::ZERO;
        if let Some(target) = actor.move_target {
            let to = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
            if to.length_squared() > 0.36 {
                horizontal = to.normalize_or_zero() * actor.speed;
            }
        }

        // --- 重力と浮力 ---
        if actor.in_liquid {
            // 水中では沈みにくく、ゆっくり浮上する。
            actor.velocity.y = (actor.velocity.y + 6.0 * dt).clamp(-2.0, 1.6);
        } else if actor.grounded {
            actor.velocity.y = actor.velocity.y.max(0.0);
            // 進路が塞がれ続けているなら、跳んで乗り越えようとする。
            if actor.stuck_frames > 20 {
                actor.velocity.y = 7.5;
                actor.stuck_frames = 0;
            }
        } else {
            actor.velocity.y = (actor.velocity.y - 26.0 * dt).max(-52.0);
        }

        let delta = Vec3::new(
            horizontal.x * dt,
            actor.velocity.y * dt,
            horizontal.z * dt,
        );
        let result = move_body(&world, pos, actor.shape, delta);

        actor.grounded = result.grounded;
        actor.in_liquid = result.in_liquid;
        if result.hit_y && actor.velocity.y < 0.0 {
            actor.velocity.y = 0.0;
        }
        if result.hit_x || result.hit_z {
            actor.stuck_frames = actor.stuck_frames.saturating_add(1);
        } else {
            actor.stuck_frames = 0;
        }

        let moved = Vec3::new(result.position.x - pos.x, 0.0, result.position.z - pos.z);
        actor.last_speed = if dt > 0.0 { moved.length() / dt } else { 0.0 };

        tf.translation = result.position;

        // 進行方向を向く。
        if moved.length_squared() > 1e-5 {
            let dir = moved.normalize();
            // Bevy の前方は -Z。
            tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z) + std::f32::consts::PI);
        }
    }
}

// ======================================================================
// 戦闘
// ======================================================================

/// 捕食者が獲物・人間に噛みつき、戦える者が反撃する。
#[allow(clippy::too_many_arguments)]
pub fn combat_system(
    time: Res<Time>,
    mut commands: Commands,
    mut creatures: Query<(Entity, &Transform, &mut Wildlife, &mut Health, &mut LimbAnimator), Without<Npc>>,
    mut npcs: Query<(Entity, &Transform, &mut Npc, &mut Health, &mut LimbAnimator), Without<Wildlife>>,
    mut chronicle: ResMut<crate::chronicle::LocalChronicle>,
    world_time: Res<WorldTime>,
) {
    let dt = time.delta_seconds();

    // --- 捕食者の攻撃 ---
    let mut bites: Vec<(Entity, f32, Vec3)> = Vec::new(); // (被害者, ダメージ, 攻撃者位置)
    let mut attackers: Vec<(Entity, &'static str)> = Vec::new();

    for (entity, tf, mut fauna, health, mut anim) in creatures.iter_mut() {
        if health.is_dead() {
            continue;
        }
        let sp = fauna.def();
        if !sp.is_predator() || fauna.state != FaunaState::Hunt {
            continue;
        }
        if fauna.attack_cooldown > 0.0 {
            fauna.attack_cooldown -= dt;
            continue;
        }
        let reach = 1.2 + sp.length * 0.5;

        let mut target: Option<(Entity, Vec3)> = None;
        for (npc_entity, npc_tf, _, npc_health, _) in npcs.iter() {
            if npc_health.is_dead() {
                continue;
            }
            if npc_tf.translation.distance(tf.translation) <= reach {
                target = Some((npc_entity, tf.translation));
                break;
            }
        }
        if let Some((victim, from)) = target {
            bites.push((victim, sp.attack, from));
            attackers.push((entity, sp.display_name));
            fauna.attack_cooldown = 1.1;
            anim.attack_timer = 0.35;
        }
    }

    // --- 噛みつきの適用と、村人の反撃 ---
    for (victim, damage, from) in bites {
        if let Ok((_, npc_tf, mut npc, mut health, mut anim)) = npcs.get_mut(victim) {
            health.damage(damage);
            if health.is_dead() {
                chronicle.record(
                    world_time.tick,
                    format!("{}が獣に襲われて命を落とした", npc.name),
                    format!("{}の{}が、村の外れで獣に襲われた。", npc.village_name, npc.name),
                    0.7,
                );
                commands.entity(victim).insert(Dying { timer: 2.0 });
            } else if npc.can_fight() && npc.attack_cooldown <= 0.0 {
                npc.attack_cooldown = 0.9;
                anim.attack_timer = 0.35;
                // 反撃は個別に処理する（借用の都合で位置だけ記録しておく）。
                let _ = npc_tf;
                let _ = from;
            }
        }
    }

    // --- 村人の反撃 ---
    let armed: Vec<(Vec3, f32)> = npcs
        .iter()
        .filter(|(_, _, npc, health, _)| {
            npc.activity == Activity::Fight && npc.can_fight() && !health.is_dead()
        })
        .map(|(_, tf, npc, _, _)| {
            // 鍛冶屋と衛兵はよい武器を持っている。
            let power = match npc.profession.as_str() {
                "衛兵" => 26.0,
                "鍛冶屋" => 20.0,
                _ => 11.0,
            };
            (tf.translation, power)
        })
        .collect();

    for (entity, tf, fauna, mut health, mut anim) in creatures.iter_mut() {
        if health.is_dead() {
            continue;
        }
        let sp = fauna.def();
        let reach = 1.6 + sp.length * 0.5;
        for (pos, power) in &armed {
            if pos.distance(tf.translation) <= reach {
                health.damage(power * dt);
                anim.attack_timer = 0.25;
                if health.is_dead() {
                    chronicle.record(
                        world_time.tick,
                        format!("村人が{}を退けた", sp.display_name),
                        format!("村を襲った{}が、住民の手で討たれた。", sp.display_name),
                        0.35,
                    );
                    commands.entity(entity).insert(Dying { timer: 1.5 });
                }
                break;
            }
        }
    }
}

/// 倒れたものを片付ける。
pub fn dying_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tracker: ResMut<PopulationTracker>,
    mut dying: Query<(Entity, &mut Dying, &mut Transform, Option<&Wildlife>, Option<&Npc>)>,
) {
    let dt = time.delta_seconds();
    for (entity, mut d, mut tf, is_wildlife, is_npc) in dying.iter_mut() {
        d.timer -= dt;
        // 横倒しになりながら沈んでいく。
        let t = (1.0 - (d.timer / 2.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
        tf.rotation = Quat::from_rotation_y(tf.rotation.to_euler(EulerRot::YXZ).0)
            * Quat::from_rotation_x(t * std::f32::consts::FRAC_PI_2);
        if d.timer <= 0.0 {
            if is_wildlife.is_some() {
                tracker.creature_count = tracker.creature_count.saturating_sub(1);
            }
            if is_npc.is_some() {
                tracker.npc_count = tracker.npc_count.saturating_sub(1);
            }
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// 実体化した集落を数える（HUD 表示用）。
pub fn count_population_system(
    mut tracker: ResMut<PopulationTracker>,
    npcs: Query<(), With<Npc>>,
    creatures: Query<(), With<Wildlife>>,
) {
    tracker.npc_count = npcs.iter().count();
    tracker.creature_count = creatures.iter().count();
}

/// 近くの村人・動物を探す（対話・情報表示に使う）。
pub fn find_nearest_npc(
    npcs: &Query<(Entity, &Transform, &Npc)>,
    from: Vec3,
    max_distance: f32,
) -> Option<(Entity, f32)> {
    let mut best: Option<(Entity, f32)> = None;
    for (e, tf, _) in npcs.iter() {
        let d = tf.translation.distance(from);
        if d <= max_distance && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((e, d));
        }
    }
    best
}

/// 村人の返答を、その人の状態・記憶・環境から組み立てる。
///
/// LLM は使わない。性格・職業・今している行動・空腹度・時刻・記憶といった
/// 実際のシミュレーション状態から文章を選び、組み合わせる。
pub fn npc_dialogue(npc: &Npc, world_time: &WorldTime, threat_near: bool) -> String {
    let mut lines: Vec<String> = Vec::new();

    // 1. 挨拶は時刻と性格で変わる。
    let hour = world_time.hour();
    let greeting = match (hour, npc.personality) {
        (5..=10, Personality::Sociable) => "やあ、いい朝だね！",
        (5..=10, _) => "おはよう。",
        (11..=16, Personality::Gruff) => "……なんの用だ。",
        (11..=16, _) => "こんにちは。",
        (17..=21, Personality::Sociable) => "こんばんは！ 一杯どうだい？",
        (17..=21, _) => "こんばんは。",
        _ => "こんな時間に、何かあったのか？",
    };
    lines.push(greeting.to_string());

    // 2. 今していること。
    lines.push(match npc.activity {
        Activity::Work => format!("今は{}の仕事の最中でね。", npc.profession),
        Activity::Eat => "ちょうど食事をしていたところだ。".to_string(),
        Activity::Sleep => "……眠っていたんだ。用があるなら手短に。".to_string(),
        Activity::Socialize => "みんなと話していたところさ。".to_string(),
        Activity::FetchWater => "井戸まで水を汲みに行くところだ。".to_string(),
        Activity::GoHome => "そろそろ家に戻るよ。".to_string(),
        Activity::Flee => "逃げているんだ、話は後にしてくれ！".to_string(),
        Activity::Fight => "下がっていろ、危ない！".to_string(),
        Activity::Wander => "特に急ぐ用はないよ。".to_string(),
    });

    // 3. 体の具合。
    if npc.hunger < 25.0 {
        lines.push("腹が減って仕方がない。何か食べるものはないか。".to_string());
    } else if npc.fatigue > 80.0 {
        lines.push("正直、もうくたくただ。".to_string());
    }

    // 4. 危険が近ければそれを話す。
    if threat_near || npc.fear > 0.5 {
        lines.push(if npc.can_fight() {
            "近くに獣がいる。村の者は下がらせてある。".to_string()
        } else {
            "獣の気配がする……早く家に入ったほうがいい。".to_string()
        });
    }

    // 5. 記憶から一つ語る。tick でゆっくり切り替わるので、
    //    話しかけるたびに違う話が聞ける。
    if !npc.memories.is_empty() {
        let idx = ((world_time.tick / 97) as usize) % npc.memories.len();
        lines.push(npc.memories[idx].clone());
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_npc(activity: Activity, hunger: f32, fatigue: f32) -> Npc {
        Npc {
            name: "テストの誰か".into(),
            gender: Gender::Female,
            personality: Personality::Sociable,
            profession: "農民".into(),
            age: 30,
            village_id: 1,
            village_name: "テスト村".into(),
            home: Vec3::ZERO,
            workplace: Vec3::ZERO,
            water: Vec3::ZERO,
            plaza: Vec3::ZERO,
            hunger,
            fatigue,
            social: 0.0,
            fear: 0.0,
            activity,
            decision_cooldown: 0.0,
            attack_cooldown: 0.0,
            memories: vec!["北の山で鉄の鉱脈を見つけた。".into(), "去年の冬は厳しかった。".into()],
        }
    }

    #[test]
    fn dialogue_reflects_what_the_npc_is_actually_doing() {
        let t = WorldTime::default();
        let working = npc_dialogue(&make_npc(Activity::Work, 100.0, 0.0), &t, false);
        let sleeping = npc_dialogue(&make_npc(Activity::Sleep, 100.0, 0.0), &t, false);
        assert!(working.contains("仕事"), "{working}");
        assert!(sleeping.contains("眠"), "{sleeping}");
        assert_ne!(working, sleeping);
    }

    #[test]
    fn a_hungry_npc_says_so() {
        let t = WorldTime::default();
        let line = npc_dialogue(&make_npc(Activity::Wander, 10.0, 0.0), &t, false);
        assert!(line.contains("腹が減"), "{line}");
    }

    #[test]
    fn danger_changes_what_is_said() {
        let t = WorldTime::default();
        let calm = npc_dialogue(&make_npc(Activity::Wander, 100.0, 0.0), &t, false);
        let scared = npc_dialogue(&make_npc(Activity::Wander, 100.0, 0.0), &t, true);
        assert_ne!(calm, scared);
        assert!(scared.contains("獣"), "{scared}");
    }

    #[test]
    fn the_hour_changes_the_greeting() {
        let mut morning = WorldTime::default();
        morning.tick = 8 * 3600;
        let mut evening = WorldTime::default();
        evening.tick = 19 * 3600;
        let npc = make_npc(Activity::Wander, 100.0, 0.0);
        assert_ne!(npc_dialogue(&npc, &morning, false), npc_dialogue(&npc, &evening, false));
    }

    #[test]
    fn memories_rotate_so_conversation_is_not_a_loop() {
        let npc = make_npc(Activity::Wander, 100.0, 0.0);
        let mut seen = std::collections::HashSet::new();
        for i in 0..400u64 {
            let mut t = WorldTime::default();
            t.tick = i * 97;
            seen.insert(npc_dialogue(&npc, &t, false));
        }
        assert!(seen.len() > 1, "the NPC always says exactly the same thing");
    }

    #[test]
    fn dialogue_never_produces_an_empty_reply() {
        let t = WorldTime::default();
        for activity in [
            Activity::Sleep, Activity::Eat, Activity::Work, Activity::Socialize,
            Activity::FetchWater, Activity::GoHome, Activity::Flee, Activity::Fight, Activity::Wander,
        ] {
            for hour in [0u64, 6, 12, 18, 23] {
                let mut wt = WorldTime::default();
                wt.tick = hour * 3600;
                let line = npc_dialogue(&make_npc(activity, 60.0, 30.0), &wt, false);
                assert!(!line.trim().is_empty(), "{activity:?} at {hour}:00 produced no reply");
            }
        }
    }

    #[test]
    fn a_villager_with_no_memories_still_talks() {
        let mut npc = make_npc(Activity::Wander, 100.0, 0.0);
        npc.memories.clear();
        let line = npc_dialogue(&npc, &WorldTime::default(), false);
        assert!(!line.trim().is_empty());
    }
}
