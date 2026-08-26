//! 流体シミュレーションと実際のボクセル世界の接続。
//!
//! `fluid.rs` は Bevy を知らない純粋な規則だけを持つ。ここでその規則を
//! 実際の `VoxelWorld` へ適用し、1 フレームあたりの処理量に上限を設けて回す。

use crate::blocks::{ids, BlockId};
use crate::chunk::{ChunkPos, CHUNK_H, CHUNK_SX, CHUNK_SZ};
use crate::fluid::{kind_of, FluidScheduler, FluidWorld, NO_FLUID};
use crate::streaming::VoxelWorld;
use bevy::prelude::*;

/// 流体の更新待ち行列。
#[derive(Resource)]
pub struct FluidSim {
    pub scheduler: FluidScheduler,
    /// 1 フレームに処理するセル数の上限。
    pub budget: usize,
    /// 更新の間隔（秒）。毎フレーム回すと水が速すぎる。
    timer: Timer,
    /// 直近フレームで処理したセル数（デバッグ表示用）。
    pub last_processed: usize,
}

impl Default for FluidSim {
    fn default() -> Self {
        Self {
            scheduler: FluidScheduler::new(60_000),
            budget: 900,
            // 現実の 0.1 秒ごと。Minecraft の水の速さに近い。
            timer: Timer::from_seconds(0.1, TimerMode::Repeating),
            last_processed: 0,
        }
    }
}

/// `VoxelWorld` を流体シミュレーションから読み書きできるようにする。
struct VoxelFluidView<'a> {
    world: &'a mut VoxelWorld,
}

impl FluidWorld for VoxelFluidView<'_> {
    fn block_at(&self, x: i32, y: i32, z: i32) -> BlockId {
        // 未生成のチャンクは「岩」として扱う。まだ見えていない土地へ
        // 水が流れ込んで、後から生成された地形と食い違うのを防ぐ。
        self.world.block_at(x, y, z).unwrap_or(ids::STONE)
    }

    fn level_at(&self, x: i32, y: i32, z: i32) -> u8 {
        let b = self.block_at(x, y, z);
        if kind_of(b).is_none() {
            return NO_FLUID;
        }
        let cp = ChunkPos::new(x.div_euclid(CHUNK_SX), z.div_euclid(CHUNK_SZ));
        match self.world.chunks.get(&cp) {
            Some(c) => c
                .fluid_level(x.rem_euclid(CHUNK_SX), y, z.rem_euclid(CHUNK_SZ))
                .unwrap_or(NO_FLUID),
            None => NO_FLUID,
        }
    }

    fn set_fluid(&mut self, x: i32, y: i32, z: i32, block: BlockId, level: u8) {
        self.world.set_block(x, y, z, block);
        self.world.set_fluid_level(x, y, z, level);
    }

    fn set_solid(&mut self, x: i32, y: i32, z: i32, block: BlockId) {
        self.world.set_fluid_level(x, y, z, 0);
        self.world.set_block(x, y, z, block);
    }

    fn clear_cell(&mut self, x: i32, y: i32, z: i32) {
        self.world.set_fluid_level(x, y, z, 0);
        self.world.set_block(x, y, z, ids::AIR);
    }

    fn blocks_fluid(&self, x: i32, y: i32, z: i32) -> bool {
        let b = self.block_at(x, y, z);
        !b.is_air() && kind_of(b).is_none()
    }

    fn out_of_bounds(&self, _x: i32, y: i32, _z: i32) -> bool {
        // 水平方向は無限。上下だけが世界の端。
        !(0..CHUNK_H).contains(&y)
    }
}

/// 流体を 1 ステップ進める。
pub fn fluid_tick_system(
    time: Res<Time>,
    mut sim: ResMut<FluidSim>,
    mut world: ResMut<VoxelWorld>,
) {
    if !sim.timer.tick(time.delta()).just_finished() {
        return;
    }
    if sim.scheduler.is_empty() {
        sim.last_processed = 0;
        return;
    }
    let budget = sim.budget;
    let mut view = VoxelFluidView { world: &mut world };
    sim.last_processed = sim.scheduler.run(&mut view, budget);
}

/// ブロックが変化した位置の周りで、流体を起こす。
///
/// 掘って穴を開けたら水が流れ込む、松明を置いたら溶岩が固まる、といった
/// 反応はここが起点になる。
pub fn wake_fluid_around(sim: &mut FluidSim, x: i32, y: i32, z: i32) {
    for (dx, dy, dz) in [
        (0, 0, 0),
        (1, 0, 0), (-1, 0, 0),
        (0, 0, 1), (0, 0, -1),
        (0, 1, 0), (0, -1, 0),
        // 斜め上も見る。水面の縁が崩れたときに反応が伝わるように。
        (1, 1, 0), (-1, 1, 0), (0, 1, 1), (0, 1, -1),
    ] {
        sim.scheduler.schedule(x + dx, y + dy, z + dz);
    }
}

/// 新しく読み込まれたチャンクの流体の縁を起こす。
///
/// 生成直後の海は水源だけで安定しているので、崖にかかった部分だけが動く。
/// 全セルを起こすと海全体が更新待ちに入って重くなるため、
/// 「下が空いている水」だけを拾う。
pub fn seed_chunk_fluids(sim: &mut FluidSim, world: &VoxelWorld, pos: ChunkPos) {
    let Some(chunk) = world.chunks.get(&pos) else { return };
    let (ox, oz) = pos.origin();

    for lz in 0..CHUNK_SZ {
        for lx in 0..CHUNK_SX {
            for y in 1..CHUNK_H {
                let b = chunk.get(lx, y, lz);
                if kind_of(b).is_none() {
                    continue;
                }
                // 真下が空いているなら落ちる余地がある。
                if chunk.get(lx, y - 1, lz).is_air() {
                    sim.scheduler.schedule(ox + lx, y, oz + lz);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BlockRegistry;
    use crate::chunk::ChunkData;
    use crate::worldgen::{GenParams, WorldGenerator};
    use std::sync::Arc;

    fn empty_world() -> VoxelWorld {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let params = GenParams { flat_world: true, ..GenParams::default() };
        let mut w = VoxelWorld::new(WorldGenerator::new(1, params), lookup);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let p = ChunkPos::new(cx, cz);
                w.chunks.insert(p, Arc::new(ChunkData::empty(p)));
            }
        }
        w
    }

    fn settle(world: &mut VoxelWorld, sim: &mut FluidSim) {
        for _ in 0..500 {
            if sim.scheduler.is_empty() {
                break;
            }
            let budget = sim.budget;
            let mut view = VoxelFluidView { world };
            sim.scheduler.run(&mut view, budget);
        }
    }

    #[test]
    fn water_poured_onto_a_floor_spreads_in_the_real_world() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        // 床を張る。
        for z in 0..16 {
            for x in 0..16 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        // 水源を1つ置く。
        w.set_block(8, 11, 8, ids::WATER);
        wake_fluid_around(&mut sim, 8, 11, 8);
        settle(&mut w, &mut sim);

        assert_eq!(w.block_at(9, 11, 8), Some(ids::WATER), "water did not spread");
        assert_eq!(w.block_at(7, 11, 8), Some(ids::WATER));
        // 広がりは有限。
        assert_eq!(w.block_at(8 + 8, 11, 8), Some(ids::AIR));
    }

    #[test]
    fn water_falls_down_a_shaft() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        for z in 0..16 {
            for x in 0..16 {
                w.set_block(x, 5, z, ids::STONE);
            }
        }
        w.set_block(8, 20, 8, ids::WATER);
        wake_fluid_around(&mut sim, 8, 20, 8);
        settle(&mut w, &mut sim);

        for y in 6..20 {
            assert_eq!(w.block_at(8, y, 8), Some(ids::WATER), "the column broke at y={y}");
        }
    }

    #[test]
    fn ungenerated_chunks_stop_the_flow() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        for z in 0..16 {
            for x in 0..16 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        // 生成済みチャンクは (-1..=1)。x=40 は未生成。
        let view = VoxelFluidView { world: &mut w };
        assert!(view.blocks_fluid(40, 10, 0), "an ungenerated chunk should stop fluid");
        let _ = &mut sim;
    }

    #[test]
    fn the_tick_budget_is_respected() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        sim.budget = 5;
        for z in 0..16 {
            for x in 0..16 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        w.set_block(8, 11, 8, ids::WATER);
        wake_fluid_around(&mut sim, 8, 11, 8);

        let budget = sim.budget;
        let mut view = VoxelFluidView { world: &mut w };
        let done = sim.scheduler.run(&mut view, budget);
        assert!(done <= 5, "processed {done} cells with a budget of 5");
    }

    #[test]
    fn seeding_a_calm_sea_does_not_flood_the_queue() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        // 底のある、完全に静かな水槽。
        let p = ChunkPos::new(0, 0);
        let mut chunk = ChunkData::empty(p);
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                chunk.set(lx, 10, lz, ids::STONE);
                for y in 11..=20 {
                    chunk.set(lx, y, lz, ids::WATER);
                }
            }
        }
        w.chunks.insert(p, Arc::new(chunk));

        seed_chunk_fluids(&mut sim, &w, p);
        // 下が空いている水は 1 つも無いので、何も起こされない。
        assert_eq!(sim.scheduler.len(), 0, "a calm sea queued {} cells", sim.scheduler.len());
    }

    #[test]
    fn seeding_finds_water_perched_over_a_drop() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        let p = ChunkPos::new(0, 0);
        let mut chunk = ChunkData::empty(p);
        // 宙に浮いた水（下は空気）。
        chunk.set(4, 30, 4, ids::WATER);
        w.chunks.insert(p, Arc::new(chunk));

        seed_chunk_fluids(&mut sim, &w, p);
        assert!(sim.scheduler.len() > 0, "water over a drop was not woken up");
    }

    #[test]
    fn lava_and_water_make_stone_in_the_real_world() {
        let mut w = empty_world();
        let mut sim = FluidSim::default();
        for z in 0..16 {
            for x in 0..16 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        w.set_block(4, 11, 4, ids::LAVA);
        w.set_block(6, 11, 4, ids::WATER);
        wake_fluid_around(&mut sim, 4, 11, 4);
        wake_fluid_around(&mut sim, 6, 11, 4);
        settle(&mut w, &mut sim);

        let mut solidified = 0;
        for x in 0..16 {
            for z in 0..16 {
                let b = w.block_at(x, 11, z).unwrap_or(ids::AIR);
                if b == ids::OBSIDIAN || b == ids::BASALT {
                    solidified += 1;
                }
            }
        }
        assert!(solidified > 0, "lava met water but nothing turned to stone");
    }
}
