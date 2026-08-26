//! チャンクのストリーミング（生成 → メッシュ化 → 描画 → 破棄）。
//!
//! 生成もメッシュ化も `AsyncComputeTaskPool` のワーカースレッドで行い、
//! メインスレッドは「出来上がったものを受け取って GPU へ上げる」だけにする。
//! 1フレームに適用するメッシュ数には上限を設けてあるため、大量のチャンクが
//! 同時に届いてもフレームが飛ばない。

use crate::blocks::{BlockId, BlockLookup};
use crate::chunk::{ChunkData, ChunkPos, CHUNK_H, CHUNK_SX, CHUNK_SZ};
use crate::mesher::{build_chunk_meshes, ChunkMeshes, MeshBuffers, MeshInput};
use crate::worldgen::WorldGenerator;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// 1フレームに新規投入する生成タスクの上限。
const GEN_DISPATCH_PER_FRAME: usize = 6;
/// 同時に走らせる生成タスクの上限。
const MAX_INFLIGHT_GEN: usize = 24;
/// 同時に走らせるメッシュタスクの上限。
const MAX_INFLIGHT_MESH: usize = 12;

/// チャンク1つ分の描画エンティティ。
#[derive(Default)]
pub struct ChunkRender {
    pub opaque: Option<Entity>,
    pub translucent: Option<Entity>,
    pub cross: Option<Entity>,
}

impl ChunkRender {
    fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        [self.opaque, self.translucent, self.cross].into_iter().flatten()
    }
}

/// チャンクメッシュに付くマーカー（デバッグ表示・一括操作用）。
#[derive(Component)]
pub struct ChunkMeshMarker(pub ChunkPos);

/// 3種類のメッシュが共有するマテリアル。
#[derive(Resource)]
pub struct VoxelMaterials {
    pub opaque: Handle<StandardMaterial>,
    pub translucent: Handle<StandardMaterial>,
    pub cross: Handle<StandardMaterial>,
}

impl VoxelMaterials {
    pub fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        // 色はメッシュの頂点カラーに焼き込んであるので、マテリアルは白で良い。
        // これにより全チャンクが同一マテリアルとなり、描画がまとまる。
        Self {
            opaque: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.95,
                reflectance: 0.02,
                ..default()
            }),
            translucent: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                alpha_mode: AlphaMode::Blend,
                // 水面がぎらつくと不透明に見えてしまう。鏡面反射は控えめにし、
                // 透け具合（頂点カラーのアルファ）が素直に出るようにする。
                perceptual_roughness: 0.35,
                reflectance: 0.12,
                // 水面は裏側からも見える（水中から見上げたとき）。
                cull_mode: None,
                double_sided: true,
                ..default()
            }),
            cross: materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                cull_mode: None,
                double_sided: true,
                ..default()
            }),
        }
    }
}

/// ボクセル世界の実行時状態。
#[derive(Resource)]
pub struct VoxelWorld {
    pub generator: Arc<WorldGenerator>,
    pub lookup: BlockLookup,
    pub seed: u64,
    /// 生成済みのチャンクデータ。
    pub chunks: HashMap<ChunkPos, Arc<ChunkData>>,
    /// 描画中のチャンク。
    pub rendered: HashMap<ChunkPos, ChunkRender>,
    /// メッシュの再構築が必要なチャンク。
    pub dirty: HashSet<ChunkPos>,
    pending_gen: HashMap<ChunkPos, Task<ChunkData>>,
    pending_mesh: HashMap<ChunkPos, Task<ChunkMeshes>>,
    /// 直近フレームの統計（デバッグHUD用）。
    pub stats: StreamStats,
}

#[derive(Default, Clone, Copy)]
pub struct StreamStats {
    pub loaded_chunks: usize,
    pub rendered_chunks: usize,
    pub pending_gen: usize,
    pub pending_mesh: usize,
    pub quads_last_build: usize,
    pub modified_chunks: usize,
}

impl VoxelWorld {
    pub fn new(generator: WorldGenerator, lookup: BlockLookup) -> Self {
        let seed = generator.seed;
        Self {
            generator: Arc::new(generator),
            lookup,
            seed,
            chunks: HashMap::new(),
            rendered: HashMap::new(),
            dirty: HashSet::new(),
            pending_gen: HashMap::new(),
            pending_mesh: HashMap::new(),
            stats: StreamStats::default(),
        }
    }

    /// セーブから復元した改変済みチャンクを差し込む。
    /// 以後この座標は再生成されず、保存された姿のまま読み込まれる。
    pub fn inject_saved_chunk(&mut self, chunk: ChunkData) {
        let pos = chunk.pos;
        self.chunks.insert(pos, Arc::new(chunk));
        self.mark_dirty_with_neighbors(pos);
    }

    pub fn mark_dirty_with_neighbors(&mut self, pos: ChunkPos) {
        self.dirty.insert(pos);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            self.dirty.insert(ChunkPos::new(pos.x + dx, pos.z + dz));
        }
    }

    /// ワールド座標のブロックを読む。未生成なら None。
    pub fn block_at(&self, wx: i32, wy: i32, wz: i32) -> Option<BlockId> {
        if !(0..CHUNK_H).contains(&wy) {
            return Some(BlockId(0));
        }
        let cp = ChunkPos::new(wx.div_euclid(CHUNK_SX), wz.div_euclid(CHUNK_SZ));
        let chunk = self.chunks.get(&cp)?;
        Some(chunk.get(wx.rem_euclid(CHUNK_SX), wy, wz.rem_euclid(CHUNK_SZ)))
    }

    /// ブロックを書き換える。書き換えたチャンクは差分セーブの対象になる。
    /// 成功したら true。
    pub fn set_block(&mut self, wx: i32, wy: i32, wz: i32, id: BlockId) -> bool {
        if !(0..CHUNK_H).contains(&wy) {
            return false;
        }
        let cp = ChunkPos::new(wx.div_euclid(CHUNK_SX), wz.div_euclid(CHUNK_SZ));
        let Some(arc) = self.chunks.get_mut(&cp) else {
            return false;
        };
        let lx = wx.rem_euclid(CHUNK_SX);
        let lz = wz.rem_euclid(CHUNK_SZ);
        if arc.get(lx, wy, lz) == id {
            return false;
        }

        // 書き換えるときだけ実体を複製する（copy-on-write）。
        let chunk = Arc::make_mut(arc);
        chunk.set(lx, wy, lz, id);
        chunk.dirty_persist = true;

        // 地表高さを維持する。掘れば下がり、積めば上がる。
        let cur_h = chunk.height_at(lx, lz);
        if id.is_air() {
            if wy == cur_h {
                let mut nh = 0;
                for y in (0..wy).rev() {
                    if !chunk.get(lx, y, lz).is_air() {
                        nh = y;
                        break;
                    }
                }
                chunk.height_map[(lz * CHUNK_SX + lx) as usize] = nh as i16;
            }
        } else if wy > cur_h {
            chunk.height_map[(lz * CHUNK_SX + lx) as usize] = wy as i16;
        }

        self.dirty.insert(cp);
        // チャンク境界に接するブロックは隣のメッシュにも影響する。
        if lx == 0 {
            self.dirty.insert(ChunkPos::new(cp.x - 1, cp.z));
        }
        if lx == CHUNK_SX - 1 {
            self.dirty.insert(ChunkPos::new(cp.x + 1, cp.z));
        }
        if lz == 0 {
            self.dirty.insert(ChunkPos::new(cp.x, cp.z - 1));
        }
        if lz == CHUNK_SZ - 1 {
            self.dirty.insert(ChunkPos::new(cp.x, cp.z + 1));
        }
        true
    }

    /// その (x,z) 列で立てる地面の高さ。未生成ならワールド生成器へ直接問い合わせる。
    pub fn ground_height(&self, wx: i32, wz: i32) -> i32 {
        let cp = ChunkPos::new(wx.div_euclid(CHUNK_SX), wz.div_euclid(CHUNK_SZ));
        match self.chunks.get(&cp) {
            Some(c) => c.height_at(wx.rem_euclid(CHUNK_SX), wz.rem_euclid(CHUNK_SZ)),
            None => self.generator.terrain_height(wx as f32, wz as f32),
        }
    }

    /// 立体としての当たり判定があるか。
    pub fn is_solid_at(&self, wx: i32, wy: i32, wz: i32) -> bool {
        match self.block_at(wx, wy, wz) {
            Some(b) => self.lookup.is_solid(b),
            // 未生成の地面は「詰まっている」とみなす。すり抜け落下を防ぐ。
            None => wy <= self.generator.terrain_height(wx as f32, wz as f32),
        }
    }

    pub fn is_liquid_at(&self, wx: i32, wy: i32, wz: i32) -> bool {
        self.block_at(wx, wy, wz).map(|b| self.lookup.is_liquid(b)).unwrap_or(false)
    }

    /// 改変済みチャンク（セーブ対象）。
    pub fn modified_chunks(&self) -> impl Iterator<Item = &ChunkData> {
        self.chunks.values().filter(|c| c.dirty_persist).map(|a| a.as_ref())
    }
}

/// 視線に沿ってブロックを探すレイキャスト（Amanatides & Woo の DDA）。
pub struct RayHit {
    /// 当たったブロックの座標。
    pub block: IVec3,
    /// その手前の空きマス（設置位置）。
    pub adjacent: IVec3,
    pub distance: f32,
}

pub fn raycast_blocks(world: &VoxelWorld, origin: Vec3, dir: Vec3, max_distance: f32) -> Option<RayHit> {
    let dir = dir.normalize_or_zero();
    if dir.length_squared() < 0.5 {
        return None;
    }

    let mut voxel = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );
    let step = IVec3::new(
        if dir.x > 0.0 { 1 } else { -1 },
        if dir.y > 0.0 { 1 } else { -1 },
        if dir.z > 0.0 { 1 } else { -1 },
    );

    // 各軸で次の境界へ到達するまでの距離。
    let next_boundary = |o: f32, d: f32, v: i32| -> f32 {
        if d > 0.0 {
            (v as f32 + 1.0 - o) / d
        } else if d < 0.0 {
            (v as f32 - o) / d
        } else {
            f32::INFINITY
        }
    };
    let mut t_max = Vec3::new(
        next_boundary(origin.x, dir.x, voxel.x),
        next_boundary(origin.y, dir.y, voxel.y),
        next_boundary(origin.z, dir.z, voxel.z),
    );
    let t_delta = Vec3::new(
        if dir.x != 0.0 { (1.0 / dir.x).abs() } else { f32::INFINITY },
        if dir.y != 0.0 { (1.0 / dir.y).abs() } else { f32::INFINITY },
        if dir.z != 0.0 { (1.0 / dir.z).abs() } else { f32::INFINITY },
    );

    let mut previous = voxel;
    let mut travelled = 0.0f32;

    // 1ブロックずつしか進まないので、最大距離で必ず終わる。
    for _ in 0..(max_distance.ceil() as i32 * 3 + 3) {
        if let Some(b) = world.block_at(voxel.x, voxel.y, voxel.z) {
            if !b.is_air() && !world.lookup.is_liquid(b) {
                return Some(RayHit {
                    block: voxel,
                    adjacent: previous,
                    distance: travelled,
                });
            }
        }
        previous = voxel;

        // 最も近い境界の軸へ進む。
        if t_max.x < t_max.y && t_max.x < t_max.z {
            travelled = t_max.x;
            voxel.x += step.x;
            t_max.x += t_delta.x;
        } else if t_max.y < t_max.z {
            travelled = t_max.y;
            voxel.y += step.y;
            t_max.y += t_delta.y;
        } else {
            travelled = t_max.z;
            voxel.z += step.z;
            t_max.z += t_delta.z;
        }

        if travelled > max_distance {
            return None;
        }
    }
    None
}

/// ストリーミングの中心。プレイヤー（またはカメラ）の位置に付ける。
#[derive(Component)]
pub struct StreamOrigin;

/// 描画半径。設定画面から変更される。
#[derive(Resource)]
pub struct StreamConfig {
    pub render_distance: i32,
    pub upload_budget: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            render_distance: 8,
            upload_budget: 4,
        }
    }
}

/// チャンクの生成タスクを投入し、完了したものを取り込む。
pub fn chunk_generation_system(
    mut world: ResMut<VoxelWorld>,
    config: Res<StreamConfig>,
    origin: Query<&Transform, With<StreamOrigin>>,
) {
    let Ok(tf) = origin.get_single() else { return };
    let center = ChunkPos::from_world(tf.translation.x, tf.translation.z);
    let pool = AsyncComputeTaskPool::get();

    // --- 完了したタスクを回収する ---
    // poll_once は値を一度しか返さないので、まず is_finished で座標だけ拾い、
    // 借用を切ってから取り出す。
    let finished: Vec<ChunkPos> = world
        .pending_gen
        .iter()
        .filter(|(_, task)| task.is_finished())
        .map(|(pos, _)| *pos)
        .collect();
    for pos in finished {
        let Some(mut task) = world.pending_gen.remove(&pos) else { continue };
        match block_on(future::poll_once(&mut task)) {
            Some(data) => {
                world.chunks.insert(pos, Arc::new(data));
                world.mark_dirty_with_neighbors(pos);
            }
            // 取りこぼした場合は次フレームへ持ち越す。
            None => {
                world.pending_gen.insert(pos, task);
            }
        }
    }

    // --- 必要なチャンクを近い順に投入する ---
    // メッシュ化には隣接チャンクが要るので、描画半径より1つ外までデータを作る。
    let data_radius = config.render_distance + 1;
    if world.pending_gen.len() < MAX_INFLIGHT_GEN {
        let mut wanted: Vec<ChunkPos> = Vec::new();
        for dz in -data_radius..=data_radius {
            for dx in -data_radius..=data_radius {
                if dx * dx + dz * dz > data_radius * data_radius {
                    continue;
                }
                let p = ChunkPos::new(center.x + dx, center.z + dz);
                if !world.chunks.contains_key(&p) && !world.pending_gen.contains_key(&p) {
                    wanted.push(p);
                }
            }
        }
        wanted.sort_by_key(|p| p.distance_sq_to(center));

        let budget = GEN_DISPATCH_PER_FRAME.min(MAX_INFLIGHT_GEN - world.pending_gen.len());
        for p in wanted.into_iter().take(budget) {
            let generator = world.generator.clone();
            let lookup = world.lookup.clone();
            let task = pool.spawn(async move { generator.generate_chunk(p, &lookup) });
            world.pending_gen.insert(p, task);
        }
    }

    world.stats.loaded_chunks = world.chunks.len();
    world.stats.pending_gen = world.pending_gen.len();
    world.stats.modified_chunks = world.chunks.values().filter(|c| c.dirty_persist).count();
}

/// メッシュ化タスクを投入し、完成したメッシュを GPU へ上げる。
pub fn chunk_meshing_system(
    mut commands: Commands,
    mut world: ResMut<VoxelWorld>,
    config: Res<StreamConfig>,
    materials: Res<VoxelMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    origin: Query<&Transform, With<StreamOrigin>>,
) {
    let Ok(tf) = origin.get_single() else { return };
    let center = ChunkPos::from_world(tf.translation.x, tf.translation.z);
    let pool = AsyncComputeTaskPool::get();
    let r = config.render_distance;

    // --- 完成したメッシュを適用する（1フレームあたりの上限つき） ---
    let ready: Vec<ChunkPos> = world
        .pending_mesh
        .iter_mut()
        .filter_map(|(pos, task)| {
            if task.is_finished() {
                Some(*pos)
            } else {
                None
            }
        })
        .take(config.upload_budget as usize)
        .collect();

    let mut quads = 0usize;
    for pos in ready {
        let Some(mut task) = world.pending_mesh.remove(&pos) else { continue };
        let Some(built) = block_on(future::poll_once(&mut task)) else {
            // まだ終わっていなかった場合は戻す。
            world.pending_mesh.insert(pos, task);
            continue;
        };
        quads += built.opaque.quad_count() + built.translucent.quad_count() + built.cross.quad_count();

        // 既存のメッシュエンティティを片付ける。
        if let Some(old) = world.rendered.remove(&pos) {
            for e in old.entities() {
                commands.entity(e).despawn_recursive();
            }
        }

        let (ox, oz) = pos.origin();
        let translation = Vec3::new(ox as f32, 0.0, oz as f32);
        let mut render = ChunkRender::default();

        render.opaque = spawn_mesh_entity(&mut commands, &mut meshes, &built.opaque, &materials.opaque, translation, pos);
        render.translucent = spawn_mesh_entity(&mut commands, &mut meshes, &built.translucent, &materials.translucent, translation, pos);
        render.cross = spawn_mesh_entity(&mut commands, &mut meshes, &built.cross, &materials.cross, translation, pos);

        world.rendered.insert(pos, render);
    }
    if quads > 0 {
        world.stats.quads_last_build = quads;
    }

    // --- 新しいメッシュタスクを投入する ---
    if world.pending_mesh.len() < MAX_INFLIGHT_MESH {
        let mut candidates: Vec<ChunkPos> = Vec::new();
        for dz in -r..=r {
            for dx in -r..=r {
                if dx * dx + dz * dz > r * r {
                    continue;
                }
                let p = ChunkPos::new(center.x + dx, center.z + dz);
                if world.pending_mesh.contains_key(&p) {
                    continue;
                }
                let needs = world.dirty.contains(&p) || !world.rendered.contains_key(&p);
                if !needs || !world.chunks.contains_key(&p) {
                    continue;
                }
                // 4方向の隣が揃うまで待つ。揃わないまま作ると境界に壁が出る。
                let ready = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .all(|(dx, dz)| world.chunks.contains_key(&ChunkPos::new(p.x + dx, p.z + dz)));
                if ready {
                    candidates.push(p);
                }
            }
        }
        candidates.sort_by_key(|p| p.distance_sq_to(center));

        let budget = (MAX_INFLIGHT_MESH - world.pending_mesh.len()).min(6);
        for p in candidates.into_iter().take(budget) {
            let Some(center_chunk) = world.chunks.get(&p).cloned() else { continue };
            let neighbors = [
                world.chunks.get(&ChunkPos::new(p.x - 1, p.z)).cloned(),
                world.chunks.get(&ChunkPos::new(p.x + 1, p.z)).cloned(),
                world.chunks.get(&ChunkPos::new(p.x, p.z - 1)).cloned(),
                world.chunks.get(&ChunkPos::new(p.x, p.z + 1)).cloned(),
            ];
            let input = MeshInput {
                center: center_chunk,
                neighbors,
                lookup: world.lookup.clone(),
                seed: world.seed,
            };
            let task = pool.spawn(async move { build_chunk_meshes(&input) });
            world.pending_mesh.insert(p, task);
            world.dirty.remove(&p);
        }
    }

    world.stats.rendered_chunks = world.rendered.len();
    world.stats.pending_mesh = world.pending_mesh.len();
}

/// 遠くなったチャンクを破棄する。改変済みチャンクのデータは保持し続ける
/// （セーブ時に失われないようにするため）。
pub fn chunk_unload_system(
    mut commands: Commands,
    mut world: ResMut<VoxelWorld>,
    config: Res<StreamConfig>,
    origin: Query<&Transform, With<StreamOrigin>>,
) {
    let Ok(tf) = origin.get_single() else { return };
    let center = ChunkPos::from_world(tf.translation.x, tf.translation.z);

    let render_limit = (config.render_distance + 2).pow(2);
    let data_limit = (config.render_distance + 5).pow(2);

    let to_unrender: Vec<ChunkPos> = world
        .rendered
        .keys()
        .copied()
        .filter(|p| p.distance_sq_to(center) > render_limit)
        .collect();
    for p in to_unrender {
        if let Some(r) = world.rendered.remove(&p) {
            for e in r.entities() {
                commands.entity(e).despawn_recursive();
            }
        }
    }

    let to_drop: Vec<ChunkPos> = world
        .chunks
        .iter()
        .filter(|(p, c)| p.distance_sq_to(center) > data_limit && !c.dirty_persist)
        .map(|(p, _)| *p)
        .collect();
    for p in to_drop {
        world.chunks.remove(&p);
        world.dirty.remove(&p);
    }
}

fn spawn_mesh_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    buffers: &MeshBuffers,
    material: &Handle<StandardMaterial>,
    translation: Vec3,
    pos: ChunkPos,
) -> Option<Entity> {
    if buffers.is_empty() {
        return None;
    }
    let mesh = to_bevy_mesh(buffers);
    let handle = meshes.add(mesh);
    Some(
        commands
            .spawn((
                PbrBundle {
                    mesh: handle,
                    material: material.clone(),
                    transform: Transform::from_translation(translation),
                    ..default()
                },
                ChunkMeshMarker(pos),
            ))
            .id(),
    )
}

/// CPU 側バッファを Bevy のメッシュへ変換する。
/// 色は頂点カラーとして持たせるため、テクスチャも UV も不要。
pub fn to_bevy_mesh(buffers: &MeshBuffers) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffers.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, buffers.colors.clone());
    mesh.insert_indices(Indices::U32(buffers.indices.clone()));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{ids, BlockRegistry};
    use crate::worldgen::GenParams;

    /// 検査しやすいよう、草木・洞窟・集落を切った平坦な世界を使う。
    /// レイキャストの期待値が地形の起伏に左右されないようにするため。
    fn test_world() -> VoxelWorld {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let params = GenParams {
            flat_world: true,
            cave_density: 0.0,
            vegetation_density: 0.0,
            settlement_density: 0.0,
            ..GenParams::default()
        };
        let gen = WorldGenerator::new(4242, params);
        let mut w = VoxelWorld::new(gen, lookup);
        // 3×3 チャンクを同期生成しておく。
        for cz in -1..=1 {
            for cx in -1..=1 {
                let p = ChunkPos::new(cx, cz);
                let data = w.generator.generate_chunk(p, &w.lookup);
                w.chunks.insert(p, Arc::new(data));
            }
        }
        w
    }

    #[test]
    fn block_reads_and_writes_hit_the_right_chunk() {
        let mut w = test_world();
        // 負座標が別チャンクへ正しく写ること。
        assert!(w.block_at(-1, 5, -1).is_some());
        assert!(w.block_at(9999, 5, 0).is_none(), "ungenerated chunk must read as None");

        assert!(w.set_block(-1, 5, -1, ids::DIAMOND_ORE));
        assert_eq!(w.block_at(-1, 5, -1), Some(ids::DIAMOND_ORE));
        // 同じ値の再設定は「変更なし」。
        assert!(!w.set_block(-1, 5, -1, ids::DIAMOND_ORE));
    }

    #[test]
    fn writes_mark_the_chunk_for_saving_and_remeshing() {
        let mut w = test_world();
        assert_eq!(w.modified_chunks().count(), 0);
        w.dirty.clear();

        w.set_block(4, 30, 4, ids::COBBLESTONE);
        assert_eq!(w.modified_chunks().count(), 1);
        assert!(w.dirty.contains(&ChunkPos::new(0, 0)));
    }

    #[test]
    fn writes_on_a_chunk_edge_also_dirty_the_neighbour() {
        let mut w = test_world();
        w.dirty.clear();
        // lx == 0 のブロックは -X 隣のメッシュにも影響する。
        w.set_block(0, 40, 5, ids::COBBLESTONE);
        assert!(w.dirty.contains(&ChunkPos::new(0, 0)));
        assert!(w.dirty.contains(&ChunkPos::new(-1, 0)), "neighbour was not remeshed");

        w.dirty.clear();
        w.set_block(CHUNK_SX - 1, 40, 5, ids::COBBLESTONE);
        assert!(w.dirty.contains(&ChunkPos::new(1, 0)));
    }

    #[test]
    fn copy_on_write_does_not_disturb_other_chunks() {
        let mut w = test_world();
        let before = w.chunks[&ChunkPos::new(1, 0)].clone();
        w.set_block(4, 30, 4, ids::COBBLESTONE);
        let after = w.chunks[&ChunkPos::new(1, 0)].clone();
        assert!(Arc::ptr_eq(&before, &after), "an unrelated chunk was cloned");
    }

    #[test]
    fn digging_and_stacking_keeps_the_height_map_correct() {
        let mut w = test_world();
        let h = w.ground_height(4, 4);
        assert!(h > 0);

        // 地表を掘ると地面が下がる。
        w.set_block(4, h, 4, ids::AIR);
        assert!(w.ground_height(4, 4) < h, "height map did not drop after digging");

        // 上に積むと地面が上がる。
        w.set_block(4, h + 5, 4, ids::COBBLESTONE);
        assert_eq!(w.ground_height(4, 4), h + 5);
    }

    #[test]
    fn raycast_finds_the_ground_below_the_camera() {
        let w = test_world();
        let h = w.ground_height(8, 8) as f32;
        let origin = Vec3::new(8.5, h + 6.0, 8.5);
        let hit = raycast_blocks(&w, origin, Vec3::NEG_Y, 20.0).expect("ray should hit the ground");
        assert_eq!(hit.block.x, 8);
        assert_eq!(hit.block.z, 8);
        assert!(hit.block.y <= h as i32);
        // 手前の空きマスは、当たったブロックの1つ上。
        assert_eq!(hit.adjacent.y, hit.block.y + 1);
        assert!(hit.distance > 0.0 && hit.distance <= 20.0);
    }

    #[test]
    fn raycast_into_open_sky_finds_nothing() {
        let w = test_world();
        let h = w.ground_height(8, 8) as f32;
        assert!(raycast_blocks(&w, Vec3::new(8.5, h + 4.0, 8.5), Vec3::Y, 30.0).is_none());
    }

    #[test]
    fn raycast_terminates_on_a_zero_direction() {
        let w = test_world();
        assert!(raycast_blocks(&w, Vec3::new(8.5, 90.0, 8.5), Vec3::ZERO, 20.0).is_none());
    }

    #[test]
    fn raycast_never_exceeds_its_reach() {
        let w = test_world();
        // 上空から真下を短い距離で撃つと届かない。
        let h = w.ground_height(8, 8) as f32;
        assert!(raycast_blocks(&w, Vec3::new(8.5, h + 50.0, 8.5), Vec3::NEG_Y, 3.0).is_none());
    }

    #[test]
    fn ungenerated_ground_is_treated_as_solid_not_as_a_hole() {
        let w = test_world();
        // 遠方の未生成チャンクでも地面判定が返る（落下バグ防止）。
        let far_h = w.generator.terrain_height(50_000.0, 50_000.0);
        assert!(w.is_solid_at(50_000, far_h - 1, 50_000));
        assert!(!w.is_solid_at(50_000, far_h + 30, 50_000));
    }

    #[test]
    fn injected_saved_chunks_replace_generated_ones() {
        let mut w = test_world();
        let mut saved = ChunkData::empty(ChunkPos::new(0, 0));
        saved.set(1, 1, 1, ids::BEDROCK);
        saved.set(1, 20, 1, ids::GOLD_ORE);
        saved.dirty_persist = true;
        w.inject_saved_chunk(saved);

        assert_eq!(w.block_at(1, 20, 1), Some(ids::GOLD_ORE));
        assert_eq!(w.modified_chunks().count(), 1);
        assert!(w.dirty.contains(&ChunkPos::new(0, 0)));
        assert!(w.dirty.contains(&ChunkPos::new(1, 0)));
    }

    #[test]
    fn bevy_mesh_conversion_preserves_the_buffers() {
        let mut buffers = MeshBuffers::default();
        buffers.positions = vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
        buffers.normals = vec![[0.0, 0.0, 1.0]; 4];
        buffers.colors = vec![[1.0, 0.5, 0.25, 1.0]; 4];
        buffers.indices = vec![0, 1, 2, 0, 2, 3];

        let mesh = to_bevy_mesh(&buffers);
        assert_eq!(mesh.count_vertices(), 4);
        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
        assert_eq!(mesh.indices().map(|i| i.len()), Some(6));
    }
}
