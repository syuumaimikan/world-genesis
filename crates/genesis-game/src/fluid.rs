//! セル方式の流体シミュレーション（水・溶岩）。
//!
//! ナビエ・ストークスは解かない。Minecraft と同じく、各セルが「水位」を持ち、
//! 隣へ 1 段ずつ落ちながら広がる離散モデルにする。見た目に十分自然で、
//! 何万セルあっても軽い。
//!
//! 規則は 3 つだけ。
//!
//! 1. 下が空いていれば、まず落ちる（重力が最優先）。
//! 2. 落ちられなければ、水位が 1 段低い状態で水平に広がる。
//! 3. 供給元（水源、または自分より高い水位の隣）が無くなった流水は涸れる。
//!
//! 溶岩は同じ規則で動くが、広がる距離が短く、更新間隔も遅い。
//! 水と触れれば固まって石になる——これは現実の反応を再現しているのではなく、
//! 地形が変化する遊びの仕掛けとして抽象化したもの。

use crate::blocks::{ids, BlockId};
use std::collections::{HashSet, VecDeque};

/// 水位の最大段数。0 が水源（満杯）、数字が大きいほど浅い。
pub const MAX_LEVEL: u8 = 7;
/// 落下中の流体。満杯だが、上から供給され続けている間しか存在できない。
pub const FALLING: u8 = 8;
/// 流体が存在しないことを示す番兵。
pub const NO_FLUID: u8 = 255;

/// 流体の種類ごとの性質。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FluidKind {
    pub block: BlockId,
    /// 水平に広がるときに落ちる段数。大きいほど流れが短い。
    pub spread_cost: u8,
    /// 更新間隔（tick）。溶岩は水より鈍い。
    pub tick_rate: u32,
}

pub const WATER: FluidKind = FluidKind {
    block: ids::WATER,
    spread_cost: 1,
    tick_rate: 5,
};

pub const LAVA: FluidKind = FluidKind {
    block: ids::LAVA,
    // 溶岩は粘性が高く、水の半分ほどしか広がらない。
    spread_cost: 2,
    tick_rate: 20,
};

pub fn kind_of(block: BlockId) -> Option<FluidKind> {
    if block == ids::WATER {
        Some(WATER)
    } else if block == ids::LAVA {
        Some(LAVA)
    } else {
        None
    }
}

/// 広がる計算に使う実効水位。水源と落下中はどちらも「満杯」として扱う。
#[inline]
fn effective(level: u8) -> u8 {
    if level == FALLING {
        0
    } else {
        level
    }
}

/// 流体シミュレーションがボクセル世界を読み書きするための入り口。
pub trait FluidWorld {
    fn block_at(&self, x: i32, y: i32, z: i32) -> BlockId;
    /// 流体の水位。流体でなければ `NO_FLUID`。
    fn level_at(&self, x: i32, y: i32, z: i32) -> u8;
    fn set_fluid(&mut self, x: i32, y: i32, z: i32, block: BlockId, level: u8);
    fn set_solid(&mut self, x: i32, y: i32, z: i32, block: BlockId);
    fn clear_cell(&mut self, x: i32, y: i32, z: i32);
    /// 流体が入り込めないか（岩・木・建材など）。
    fn blocks_fluid(&self, x: i32, y: i32, z: i32) -> bool;
    /// 世界の外か（下端・上端）。
    fn out_of_bounds(&self, x: i32, y: i32, z: i32) -> bool;
}

const NEIGHBORS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// 1 セル分の流体を更新する。変化があった座標を `dirty` へ積む。
///
/// 戻り値は「このセルが変化したか」。
pub fn step_cell(
    world: &mut dyn FluidWorld,
    x: i32,
    y: i32,
    z: i32,
    dirty: &mut Vec<(i32, i32, i32)>,
) -> bool {
    let block = world.block_at(x, y, z);
    let Some(kind) = kind_of(block) else { return false };
    let level = world.level_at(x, y, z);
    if level == NO_FLUID {
        return false;
    }
    let is_source = level == 0;

    // --- 0. 水と溶岩が触れたら固まる ---
    if let Some(changed) = resolve_contact(world, x, y, z, kind, level, dirty) {
        return changed;
    }

    // --- 1. 供給が絶たれた流水・落下水は涸れる ---
    if !is_source && !has_supply(world, x, y, z, kind, level) {
        world.clear_cell(x, y, z);
        push_neighbors(x, y, z, dirty);
        return true;
    }

    // --- 2. 下が支えているか ---
    let below_blocked = world.out_of_bounds(x, y - 1, z) || world.blocks_fluid(x, y - 1, z);
    let below_block = world.block_at(x, y - 1, z);
    let below_level = world.level_at(x, y - 1, z);
    let below_is_full = below_block == kind.block && matches!(below_level, 0 | FALLING);
    let supported = below_blocked || below_is_full;

    if !supported {
        // --- 3. 落ちる。落ちている間は横に広がらない ---
        // （空中で水が板状に広がるのを防ぐ。これが無いと世界が水浸しになる）
        // 空きマス、または「まだ満杯でない同種の流体」の上へ落ちる。
        let needs_fill = below_block.is_air()
            || (below_block == kind.block
                && below_level != NO_FLUID
                && below_level != FALLING
                && below_level != 0);
        if needs_fill {
            world.set_fluid(x, y - 1, z, kind.block, FALLING);
            dirty.push((x, y - 1, z));
            push_neighbors(x, y - 1, z, dirty);
            return true;
        }
        return false;
    }

    // --- 4. 水平に広がる ---
    let next_level = effective(level).saturating_add(kind.spread_cost);
    if next_level > MAX_LEVEL {
        return false;
    }
    let mut changed = false;
    for (dx, dz) in NEIGHBORS {
        let (nx, nz) = (x + dx, z + dz);
        if world.out_of_bounds(nx, y, nz) || world.blocks_fluid(nx, y, nz) {
            continue;
        }
        let nb = world.block_at(nx, y, nz);
        let nl = world.level_at(nx, y, nz);
        // 空きマス、または自分より浅い同種の流体へ流し込む。
        let should_fill = nb.is_air()
            || (nb == kind.block && nl != NO_FLUID && nl != FALLING && nl > next_level);
        if should_fill {
            world.set_fluid(nx, y, nz, kind.block, next_level);
            dirty.push((nx, y, nz));
            push_neighbors(nx, y, nz, dirty);
            changed = true;
        }
    }

    changed
}

/// 流水を支える供給元があるか。
///
/// 真上に同じ流体があるか、水平の隣に自分より水位の高い（数値の小さい）
/// 同種の流体があれば supply あり。
fn has_supply(world: &dyn FluidWorld, x: i32, y: i32, z: i32, kind: FluidKind, level: u8) -> bool {
    // 真上から降ってきている。落下中の流体はこれだけが命綱。
    let fed_from_above =
        world.block_at(x, y + 1, z) == kind.block && world.level_at(x, y + 1, z) != NO_FLUID;
    if fed_from_above {
        return true;
    }
    if level == FALLING {
        // 上が切れた落下水は、横から支えられることはない。
        return false;
    }
    for (dx, dz) in NEIGHBORS {
        let (nx, nz) = (x + dx, z + dz);
        if world.block_at(nx, y, nz) != kind.block {
            continue;
        }
        let nl = world.level_at(nx, y, nz);
        if nl == NO_FLUID {
            continue;
        }
        if effective(nl) + kind.spread_cost <= level {
            return true;
        }
    }
    false
}

/// 水と溶岩が隣り合ったときの固化。
///
/// 現実の反応を再現するものではなく、地形が変わる遊びとしての抽象規則。
/// * 溶岩の**水源**が水に触れる → 黒曜石（硬い岩）
/// * 溶岩の**流れ**が水に触れる → 玄武岩
/// * 水が溶岩の真上から落ちる → その水は蒸発する
fn resolve_contact(
    world: &mut dyn FluidWorld,
    x: i32,
    y: i32,
    z: i32,
    kind: FluidKind,
    level: u8,
    dirty: &mut Vec<(i32, i32, i32)>,
) -> Option<bool> {
    let other = if kind.block == ids::LAVA { ids::WATER } else { ids::LAVA };

    let mut touching = false;
    for (dx, dy, dz) in [
        (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0),
    ] {
        if world.block_at(x + dx, y + dy, z + dz) == other {
            touching = true;
            break;
        }
    }
    if !touching {
        return None;
    }

    if kind.block == ids::LAVA {
        // 溶岩側が固まる。
        let solid = if level == 0 { ids::OBSIDIAN } else { ids::BASALT };
        world.set_solid(x, y, z, solid);
        push_neighbors(x, y, z, dirty);
        Some(true)
    } else {
        // 水は溶岩の真上にあるときだけ蒸発する（横並びなら残る）。
        if world.block_at(x, y - 1, z) == ids::LAVA && level != 0 {
            world.clear_cell(x, y, z);
            push_neighbors(x, y, z, dirty);
            Some(true)
        } else {
            None
        }
    }
}

#[inline]
fn push_neighbors(x: i32, y: i32, z: i32, dirty: &mut Vec<(i32, i32, i32)>) {
    for (dx, dy, dz) in [
        (1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0),
    ] {
        dirty.push((x + dx, y + dy, z + dz));
    }
}

/// 更新待ちセルの管理。1 tick あたりの処理数に上限を設け、
/// 大きな海が一度に溢れてもフレームが落ちないようにする。
#[derive(Default)]
pub struct FluidScheduler {
    queue: VecDeque<(i32, i32, i32)>,
    queued: HashSet<(i32, i32, i32)>,
    /// 溢れ防止の上限。これを超えたら古いものから捨てる。
    pub capacity: usize,
}

impl FluidScheduler {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            queued: HashSet::new(),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn schedule(&mut self, x: i32, y: i32, z: i32) {
        let key = (x, y, z);
        if self.queued.contains(&key) {
            return;
        }
        if self.queue.len() >= self.capacity {
            // 最古を捨てて暴走を防ぐ。捨てた分は次に水が動いたとき拾い直される。
            if let Some(old) = self.queue.pop_front() {
                self.queued.remove(&old);
            }
        }
        self.queue.push_back(key);
        self.queued.insert(key);
    }

    /// 最大 `budget` セルを更新する。処理したセル数を返す。
    pub fn run(&mut self, world: &mut dyn FluidWorld, budget: usize) -> usize {
        let mut processed = 0;
        let mut dirty: Vec<(i32, i32, i32)> = Vec::new();

        while processed < budget {
            let Some((x, y, z)) = self.queue.pop_front() else { break };
            self.queued.remove(&(x, y, z));
            processed += 1;
            step_cell(world, x, y, z, &mut dirty);
        }

        for (x, y, z) in dirty {
            self.schedule(x, y, z);
        }
        processed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 検証用の小さな箱庭。
    struct TestWorld {
        blocks: HashMap<(i32, i32, i32), BlockId>,
        levels: HashMap<(i32, i32, i32), u8>,
        size: i32,
        height: i32,
    }

    impl TestWorld {
        fn new(size: i32, height: i32) -> Self {
            Self {
                blocks: HashMap::new(),
                levels: HashMap::new(),
                size,
                height,
            }
        }
        fn floor(&mut self, y: i32) {
            for z in 0..self.size {
                for x in 0..self.size {
                    self.blocks.insert((x, y, z), ids::STONE);
                }
            }
        }
        fn source(&mut self, x: i32, y: i32, z: i32, block: BlockId) {
            self.blocks.insert((x, y, z), block);
            self.levels.insert((x, y, z), 0);
        }
        fn settle(&mut self, sched: &mut FluidScheduler) {
            for _ in 0..400 {
                if sched.is_empty() {
                    break;
                }
                sched.run(self, 4096);
            }
        }
        fn count(&self, block: BlockId) -> usize {
            self.blocks.values().filter(|b| **b == block).count()
        }
    }

    impl FluidWorld for TestWorld {
        fn block_at(&self, x: i32, y: i32, z: i32) -> BlockId {
            *self.blocks.get(&(x, y, z)).unwrap_or(&ids::AIR)
        }
        fn level_at(&self, x: i32, y: i32, z: i32) -> u8 {
            *self.levels.get(&(x, y, z)).unwrap_or(&NO_FLUID)
        }
        fn set_fluid(&mut self, x: i32, y: i32, z: i32, block: BlockId, level: u8) {
            self.blocks.insert((x, y, z), block);
            self.levels.insert((x, y, z), level);
        }
        fn set_solid(&mut self, x: i32, y: i32, z: i32, block: BlockId) {
            self.blocks.insert((x, y, z), block);
            self.levels.remove(&(x, y, z));
        }
        fn clear_cell(&mut self, x: i32, y: i32, z: i32) {
            self.blocks.insert((x, y, z), ids::AIR);
            self.levels.remove(&(x, y, z));
        }
        fn blocks_fluid(&self, x: i32, y: i32, z: i32) -> bool {
            let b = self.block_at(x, y, z);
            !b.is_air() && b != ids::WATER && b != ids::LAVA
        }
        fn out_of_bounds(&self, x: i32, y: i32, z: i32) -> bool {
            x < 0 || z < 0 || y < 0 || x >= self.size || z >= self.size || y >= self.height
        }
    }

    fn sched() -> FluidScheduler {
        FluidScheduler::new(200_000)
    }

    #[test]
    fn water_falls_before_it_spreads() {
        let mut w = TestWorld::new(9, 20);
        w.floor(0);
        // 高いところに水源を置く。
        w.source(4, 8, 4, ids::WATER);
        let mut s = sched();
        s.schedule(4, 8, 4);
        w.settle(&mut s);

        // 真下の列が水で埋まっている。
        for y in 1..8 {
            assert_eq!(w.block_at(4, y, 4), ids::WATER, "the falling column broke at y={y}");
        }
    }

    #[test]
    fn water_spreads_across_a_flat_floor_and_stops() {
        let mut w = TestWorld::new(31, 8);
        w.floor(0);
        w.source(15, 1, 15, ids::WATER);
        let mut s = sched();
        s.schedule(15, 1, 15);
        w.settle(&mut s);

        // 水源の隣は濡れている。
        assert_eq!(w.block_at(16, 1, 15), ids::WATER);
        // 広がりは有限。水は 7 段で尽きるので、8 マス以上先は乾いている。
        assert_eq!(w.block_at(15 + 8, 1, 15), ids::AIR, "water spread further than its level allows");
        assert_eq!(w.block_at(15, 1, 15 + 8), ids::AIR);
    }

    #[test]
    fn lava_spreads_less_far_than_water() {
        let reach = |block: BlockId| {
            let mut w = TestWorld::new(31, 8);
            w.floor(0);
            w.source(15, 1, 15, block);
            let mut s = sched();
            s.schedule(15, 1, 15);
            w.settle(&mut s);
            (1..15)
                .take_while(|d| w.block_at(15 + d, 1, 15) == block)
                .count()
        };
        let water = reach(ids::WATER);
        let lava = reach(ids::LAVA);
        assert!(water > 0 && lava > 0, "neither fluid spread at all");
        assert!(lava < water, "lava ({lava}) should not out-run water ({water})");
    }

    #[test]
    fn flowing_water_dries_up_when_its_source_is_removed() {
        let mut w = TestWorld::new(21, 8);
        w.floor(0);
        w.source(10, 1, 10, ids::WATER);
        let mut s = sched();
        s.schedule(10, 1, 10);
        w.settle(&mut s);
        assert!(w.count(ids::WATER) > 1, "no flow to remove");

        // 水源を消す。
        w.clear_cell(10, 1, 10);
        s.schedule(10, 1, 10);
        for (dx, dz) in NEIGHBORS {
            s.schedule(10 + dx, 1, 10 + dz);
        }
        w.settle(&mut s);

        assert_eq!(w.count(ids::WATER), 0, "flowing water outlived its source");
    }

    #[test]
    fn a_source_block_never_drains() {
        let mut w = TestWorld::new(9, 8);
        w.floor(0);
        w.source(4, 1, 4, ids::WATER);
        let mut s = sched();
        s.schedule(4, 1, 4);
        w.settle(&mut s);
        assert_eq!(w.block_at(4, 1, 4), ids::WATER);
        assert_eq!(w.level_at(4, 1, 4), 0, "the source lost its full level");
    }

    #[test]
    fn water_does_not_pass_through_walls() {
        let mut w = TestWorld::new(11, 8);
        w.floor(0);
        // x=6 に壁を立てる。
        for z in 0..11 {
            w.blocks.insert((6, 1, z), ids::STONE);
        }
        w.source(3, 1, 5, ids::WATER);
        let mut s = sched();
        s.schedule(3, 1, 5);
        w.settle(&mut s);

        assert_eq!(w.block_at(6, 1, 5), ids::STONE, "the wall was replaced");
        assert_eq!(w.block_at(7, 1, 5), ids::AIR, "water leaked through a solid wall");
    }

    #[test]
    fn water_fills_a_pit_from_the_bottom_up() {
        let mut w = TestWorld::new(11, 12);
        w.floor(0);
        // 3x3 の穴を囲む壁（高さ 1..3）。
        for y in 1..=3 {
            for z in 3..=7 {
                for x in 3..=7 {
                    let edge = x == 3 || x == 7 || z == 3 || z == 7;
                    if edge {
                        w.blocks.insert((x, y, z), ids::STONE);
                    }
                }
            }
        }
        // 穴の上から水を落とす。
        w.source(5, 6, 5, ids::WATER);
        let mut s = sched();
        s.schedule(5, 6, 5);
        w.settle(&mut s);

        // 底が濡れている。
        assert_eq!(w.block_at(5, 1, 5), ids::WATER, "the pit floor stayed dry");
    }

    #[test]
    fn lava_meeting_water_turns_to_stone() {
        let mut w = TestWorld::new(11, 8);
        w.floor(0);
        w.source(3, 1, 5, ids::LAVA);
        w.source(7, 1, 5, ids::WATER);
        let mut s = sched();
        s.schedule(3, 1, 5);
        s.schedule(7, 1, 5);
        w.settle(&mut s);

        // 接触点に岩ができている。
        let solidified = w.count(ids::OBSIDIAN) + w.count(ids::BASALT);
        assert!(solidified > 0, "lava and water met but nothing solidified");
    }

    #[test]
    fn a_lava_source_touching_water_becomes_obsidian() {
        let mut w = TestWorld::new(9, 8);
        w.floor(0);
        w.source(4, 1, 4, ids::LAVA);
        w.source(5, 1, 4, ids::WATER);
        let mut s = sched();
        s.schedule(4, 1, 4);
        w.settle(&mut s);
        assert_eq!(w.block_at(4, 1, 4), ids::OBSIDIAN);
    }

    #[test]
    fn the_scheduler_respects_its_budget() {
        let mut w = TestWorld::new(41, 8);
        w.floor(0);
        w.source(20, 1, 20, ids::WATER);
        let mut s = sched();
        s.schedule(20, 1, 20);
        let done = s.run(&mut w, 3);
        assert!(done <= 3, "the scheduler processed {done} cells with a budget of 3");
    }

    #[test]
    fn the_scheduler_never_grows_without_bound() {
        let mut s = FluidScheduler::new(64);
        for i in 0..10_000 {
            s.schedule(i, i % 7, i * 2);
        }
        assert!(s.len() <= 64, "queue grew to {} despite a cap of 64", s.len());
    }

    #[test]
    fn scheduling_the_same_cell_twice_queues_it_once() {
        let mut s = sched();
        s.schedule(1, 2, 3);
        s.schedule(1, 2, 3);
        s.schedule(1, 2, 3);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn simulation_terminates_and_is_stable() {
        let mut w = TestWorld::new(25, 10);
        w.floor(0);
        w.source(12, 5, 12, ids::WATER);
        let mut s = sched();
        s.schedule(12, 5, 12);
        w.settle(&mut s);

        // 落ち着いた後にもう一度全セルを起こしても、何も変わらないこと。
        let before: Vec<_> = {
            let mut v: Vec<_> = w.blocks.iter().map(|(k, b)| (*k, *b)).collect();
            v.sort();
            v
        };
        for (&(x, y, z), _) in w.blocks.clone().iter() {
            s.schedule(x, y, z);
        }
        w.settle(&mut s);
        let after: Vec<_> = {
            let mut v: Vec<_> = w.blocks.iter().map(|(k, b)| (*k, *b)).collect();
            v.sort();
            v
        };
        assert_eq!(before, after, "the fluid never reached a stable state");
    }

    #[test]
    fn water_does_not_escape_the_world_bounds() {
        let mut w = TestWorld::new(5, 6);
        w.floor(0);
        w.source(0, 1, 0, ids::WATER);
        let mut s = sched();
        s.schedule(0, 1, 0);
        w.settle(&mut s);
        for ((x, y, z), b) in w.blocks.iter() {
            if *b == ids::WATER {
                assert!(!w.out_of_bounds(*x, *y, *z), "water escaped the world at {x},{y},{z}");
            }
        }
    }
}
