//! ボクセル世界での AABB 衝突解決。
//!
//! プレイヤーも NPC も動物も、軸に沿った直方体としてブロックへぶつかる。
//! 3軸をまとめて動かすと角へめり込むため、X → Z → Y の順に1軸ずつ動かして
//! 各段階で押し戻す。これにより壁に沿って滑り、段差では引っかからずに登れる。

use crate::streaming::VoxelWorld;
use bevy::prelude::*;

/// 動く物体の当たり判定。原点は足元（`position` は接地面の中心）。
#[derive(Debug, Clone, Copy)]
pub struct BodyShape {
    /// 水平方向の半径。
    pub half_width: f32,
    /// 全高。
    pub height: f32,
    /// 自動的に登れる段差の高さ。
    ///
    /// プレイヤーは 0.0（＝自動では登らない。ブロックへ上がるにはジャンプする）。
    /// 四足獣や経路追従する NPC は、蹄・歩幅の分だけ小さな段差を越えられる。
    pub step_height: f32,
}

impl Default for BodyShape {
    fn default() -> Self {
        Self {
            half_width: 0.3,
            height: 1.8,
            // 既定は「自動で登らない」。ブロック1段は必ずジャンプで越える。
            step_height: 0.0,
        }
    }
}

impl BodyShape {
    /// 段差を自動で越える個体（動物・NPC の経路追従）向けの形状を作る。
    pub fn with_step(mut self, step: f32) -> Self {
        self.step_height = step.max(0.0);
        self
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MoveResult {
    pub position: Vec3,
    pub hit_x: bool,
    pub hit_y: bool,
    pub hit_z: bool,
    pub grounded: bool,
    /// 体の中心が液体に浸かっているか。
    pub in_liquid: bool,
    /// 段差を自動的に登ったか。
    pub stepped_up: bool,
}

/// 指定した AABB がブロックと重なっているか。
pub fn overlaps_solid(world: &VoxelWorld, pos: Vec3, shape: BodyShape) -> bool {
    let min = Vec3::new(pos.x - shape.half_width, pos.y, pos.z - shape.half_width);
    let max = Vec3::new(
        pos.x + shape.half_width,
        pos.y + shape.height,
        pos.z + shape.half_width,
    );

    // AABB が触れているボクセルだけを見る。境界ちょうどで隣のブロックを
    // 拾わないよう、上限側はごく小さく内側へ寄せる。
    const EPS: f32 = 1e-4;
    let x0 = (min.x + EPS).floor() as i32;
    let x1 = (max.x - EPS).floor() as i32;
    let y0 = (min.y + EPS).floor() as i32;
    let y1 = (max.y - EPS).floor() as i32;
    let z0 = (min.z + EPS).floor() as i32;
    let z1 = (max.z - EPS).floor() as i32;

    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                if world.is_solid_at(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// 1フレーム分の移動を衝突解決しながら適用する。
pub fn move_body(world: &VoxelWorld, position: Vec3, shape: BodyShape, delta: Vec3) -> MoveResult {
    let mut result = MoveResult {
        position,
        ..Default::default()
    };
    let mut pos = position;

    // --- 水平方向：X ---
    if delta.x != 0.0 {
        let candidate = Vec3::new(pos.x + delta.x, pos.y, pos.z);
        if overlaps_solid(world, candidate, shape) {
            // 段差なら登る。
            let stepped = Vec3::new(candidate.x, candidate.y + shape.step_height, candidate.z);
            if shape.step_height > 0.0 && !overlaps_solid(world, stepped, shape) {
                pos = snap_down(world, stepped, shape);
                result.stepped_up = true;
            } else {
                result.hit_x = true;
            }
        } else {
            pos = candidate;
        }
    }

    // --- 水平方向：Z ---
    if delta.z != 0.0 {
        let candidate = Vec3::new(pos.x, pos.y, pos.z + delta.z);
        if overlaps_solid(world, candidate, shape) {
            let stepped = Vec3::new(candidate.x, candidate.y + shape.step_height, candidate.z);
            if shape.step_height > 0.0 && !overlaps_solid(world, stepped, shape) {
                pos = snap_down(world, stepped, shape);
                result.stepped_up = true;
            } else {
                result.hit_z = true;
            }
        } else {
            pos = candidate;
        }
    }

    // --- 垂直方向 ---
    if delta.y != 0.0 {
        let candidate = Vec3::new(pos.x, pos.y + delta.y, pos.z);
        if overlaps_solid(world, candidate, shape) {
            result.hit_y = true;
            if delta.y < 0.0 {
                // 落下してブロックに乗った：足元をブロックの上面へ揃える。
                pos.y = candidate.y.floor() + 1.0;
                // それでも埋まっているなら1ブロックずつ押し上げる（詰まり回復）。
                let mut guard = 0;
                while overlaps_solid(world, pos, shape) && guard < 8 {
                    pos.y += 1.0;
                    guard += 1;
                }
                result.grounded = true;
            }
            // 上方向に当たった場合は y を進めない（頭をぶつける）。
        } else {
            pos = candidate;
        }
    }

    // 接地判定：足元のごく薄い層を調べる。
    if !result.grounded {
        let probe = Vec3::new(pos.x, pos.y - 0.06, pos.z);
        result.grounded = overlaps_solid(world, probe, shape);
    }

    // 液体判定：胸の高さ。
    let chest = Vec3::new(pos.x, pos.y + shape.height * 0.55, pos.z);
    result.in_liquid = world.is_liquid_at(
        chest.x.floor() as i32,
        chest.y.floor() as i32,
        chest.z.floor() as i32,
    );

    result.position = pos;
    result
}

/// 段差を登ったあと、浮いた分だけ落とす。
fn snap_down(world: &VoxelWorld, pos: Vec3, shape: BodyShape) -> Vec3 {
    let mut p = pos;
    // 最大 step_height 分だけ、0.05 刻みで降ろす。
    let steps = (shape.step_height / 0.05).ceil() as i32;
    for _ in 0..steps {
        let lower = Vec3::new(p.x, p.y - 0.05, p.z);
        if overlaps_solid(world, lower, shape) {
            break;
        }
        p = lower;
    }
    p
}

/// 与えた (x,z) で安全に立てる Y 座標を探す。スポーン位置の決定に使う。
pub fn find_spawn_y(world: &VoxelWorld, wx: i32, wz: i32, shape: BodyShape) -> f32 {
    let ground = world.ground_height(wx, wz);
    let base = Vec3::new(wx as f32 + 0.5, 0.0, wz as f32 + 0.5);

    // 地表から上へ、体が収まる最初の高さを探す。
    for dy in 0..12 {
        let y = (ground + 1 + dy) as f32;
        let p = Vec3::new(base.x, y, base.z);
        if !overlaps_solid(world, p, shape) {
            return y;
        }
    }
    // 見つからなければ地表の少し上へ置く（詰まっていても落下で解決する）。
    (ground + 2) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{ids, BlockRegistry};
    use crate::chunk::{ChunkData, ChunkPos, SEA_LEVEL};
    use crate::worldgen::{GenParams, WorldGenerator};

    /// 手で作った、完全に予測できる小さな世界。
    fn flat_world() -> VoxelWorld {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let params = GenParams {
            flat_world: true,
            cave_density: 0.0,
            vegetation_density: 0.0,
            settlement_density: 0.0,
            ..GenParams::default()
        };
        let mut w = VoxelWorld::new(WorldGenerator::new(1, params), lookup);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let p = ChunkPos::new(cx, cz);
                let d = w.generator.generate_chunk(p, &w.lookup);
                w.chunks.insert(p, std::sync::Arc::new(d));
            }
        }
        w
    }

    /// 完全に空の世界（自分でブロックを置いて試す）。
    fn empty_world() -> VoxelWorld {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let params = GenParams { flat_world: true, ..GenParams::default() };
        let mut w = VoxelWorld::new(WorldGenerator::new(1, params), lookup);
        for cz in -1..=1 {
            for cx in -1..=1 {
                w.chunks.insert(ChunkPos::new(cx, cz), std::sync::Arc::new(ChunkData::empty(ChunkPos::new(cx, cz))));
            }
        }
        w
    }

    const GROUND: f32 = (SEA_LEVEL + 4) as f32;

    #[test]
    fn standing_on_the_ground_is_not_overlapping() {
        let w = flat_world();
        let shape = BodyShape::default();
        // 平坦世界の地表は SEA_LEVEL+4 のブロック。その上面は +1。
        let feet = Vec3::new(4.5, GROUND + 1.0, 4.5);
        assert!(!overlaps_solid(&w, feet, shape), "standing on the surface reports a collision");
        // 地表ブロックの中に立てば当然重なる。
        assert!(overlaps_solid(&w, Vec3::new(4.5, GROUND, 4.5), shape));
    }

    #[test]
    fn falling_lands_exactly_on_the_block_surface() {
        let w = flat_world();
        let shape = BodyShape::default();
        let start = Vec3::new(4.5, GROUND + 6.0, 4.5);
        let r = move_body(&w, start, shape, Vec3::new(0.0, -8.0, 0.0));
        assert!(r.hit_y && r.grounded);
        assert_eq!(r.position.y, GROUND + 1.0, "did not land flush with the surface");
        assert!(!overlaps_solid(&w, r.position, shape));
    }

    #[test]
    fn walking_into_a_two_block_wall_is_blocked() {
        let mut w = empty_world();
        // 床
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        // x=5 に高さ2の壁
        for z in 0..8 {
            w.set_block(5, 11, z, ids::STONE);
            w.set_block(5, 12, z, ids::STONE);
        }
        let shape = BodyShape::default();
        let start = Vec3::new(4.0, 11.0, 3.5);
        let r = move_body(&w, start, shape, Vec3::new(2.0, 0.0, 0.0));
        assert!(r.hit_x, "the wall did not stop movement");
        assert!(r.position.x <= 4.7, "walked through the wall (x={})", r.position.x);
    }

    #[test]
    fn the_player_does_not_auto_climb_a_block() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        // x>=5 を1段高くする。
        for z in 0..8 {
            for x in 5..8 {
                w.set_block(x, 11, z, ids::STONE);
            }
        }
        // 既定の形状（プレイヤー）は自動で登らない。
        let player = BodyShape::default();
        assert_eq!(player.step_height, 0.0);
        let r = move_body(&w, Vec3::new(4.0, 11.0, 3.5), player, Vec3::new(1.2, 0.0, 0.0));
        assert!(!r.stepped_up, "the player auto-climbed a block instead of needing to jump");
        assert!(r.hit_x, "the ledge should block horizontal movement");
        assert!(r.position.x < 4.8, "the player slid up onto the ledge (x={})", r.position.x);
    }

    #[test]
    fn jumping_gets_the_player_onto_a_one_block_ledge() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        for z in 0..8 {
            for x in 5..8 {
                w.set_block(x, 11, z, ids::STONE);
            }
        }
        let shape = BodyShape::default();
        // ジャンプ初速 8.6、重力 26 で 1 ブロックは十分越えられる。
        let mut pos = Vec3::new(4.0, 11.0, 3.5);
        let mut vy = 8.6f32;
        let dt = 1.0 / 60.0;
        let mut landed_on_ledge = false;
        for _ in 0..120 {
            vy -= 26.0 * dt;
            let r = move_body(&w, pos, shape, Vec3::new(1.6 * dt, vy * dt, 0.0));
            pos = r.position;
            if r.hit_y && vy < 0.0 {
                vy = 0.0;
            }
            if r.grounded && pos.y >= 12.0 && pos.x > 5.0 {
                landed_on_ledge = true;
                break;
            }
        }
        assert!(landed_on_ledge, "jumping failed to clear a single block (ended at {pos:?})");
    }

    #[test]
    fn animals_may_still_step_over_small_ledges() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        // x>=5 を1段高くする
        for z in 0..8 {
            for x in 5..8 {
                w.set_block(x, 11, z, ids::STONE);
            }
        }
        // 動物・NPC は歩幅ぶんの段差を自動で越えられる。
        let shape = BodyShape::default().with_step(1.05);
        let start = Vec3::new(4.0, 11.0, 3.5);
        let r = move_body(&w, start, shape, Vec3::new(1.2, 0.0, 0.0));
        assert!(r.stepped_up, "the step was not climbed");
        assert!(r.position.x > 4.9, "movement was blocked by a climbable step");
        assert_eq!(r.position.y, 12.0, "ended at the wrong height after stepping up");
        assert!(!overlaps_solid(&w, r.position, shape));
    }

    #[test]
    fn sliding_along_a_wall_preserves_the_other_axis() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
            }
        }
        for z in 0..8 {
            for dy in 1..=3 {
                w.set_block(5, 10 + dy, z, ids::STONE);
            }
        }
        let shape = BodyShape::default();
        let start = Vec3::new(4.0, 11.0, 3.0);
        // 壁へ斜めに突っ込む。X は止まるが Z は進むはず。
        let r = move_body(&w, start, shape, Vec3::new(2.0, 0.0, 1.0));
        assert!(r.hit_x);
        assert!(!r.hit_z);
        assert!((r.position.z - 4.0).abs() < 1e-4, "z movement was lost: {}", r.position.z);
    }

    #[test]
    fn jumping_into_a_ceiling_stops_upward_motion() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
                w.set_block(x, 14, z, ids::STONE);
            }
        }
        let shape = BodyShape::default();
        let start = Vec3::new(3.5, 11.0, 3.5);
        let r = move_body(&w, start, shape, Vec3::new(0.0, 2.0, 0.0));
        assert!(r.hit_y);
        assert_eq!(r.position.y, 11.0, "moved through the ceiling");
    }

    #[test]
    fn water_does_not_block_movement_but_is_detected() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
                for y in 11..14 {
                    w.set_block(x, y, z, ids::WATER);
                }
            }
        }
        let shape = BodyShape::default();
        let r = move_body(&w, Vec3::new(3.5, 11.0, 3.5), shape, Vec3::new(1.0, 0.0, 0.0));
        assert!(!r.hit_x, "water blocked horizontal movement");
        assert!(r.in_liquid, "submerged body was not detected as being in liquid");
    }

    #[test]
    fn a_body_stuck_inside_blocks_is_pushed_out_upwards() {
        let mut w = empty_world();
        for y in 10..14 {
            for z in 0..8 {
                for x in 0..8 {
                    w.set_block(x, y, z, ids::STONE);
                }
            }
        }
        let shape = BodyShape::default();
        // 岩の中から下向きに動かす。埋まったままにならず、外へ出ること。
        let r = move_body(&w, Vec3::new(3.5, 11.0, 3.5), shape, Vec3::new(0.0, -1.0, 0.0));
        assert!(!overlaps_solid(&w, r.position, shape), "body stayed buried inside solid rock");
        assert!(r.position.y >= 14.0);
    }

    #[test]
    fn spawn_search_always_finds_free_space() {
        let w = flat_world();
        let shape = BodyShape::default();
        for (x, z) in [(0, 0), (7, 3), (-5, 11), (15, -9)] {
            let y = find_spawn_y(&w, x, z, shape);
            let p = Vec3::new(x as f32 + 0.5, y, z as f32 + 0.5);
            assert!(!overlaps_solid(&w, p, shape), "spawn at ({x},{z}) is inside a block");
            assert!(y > 0.0 && y.is_finite());
        }
    }

    #[test]
    fn zero_delta_is_a_no_op_apart_from_sensing() {
        let w = flat_world();
        let shape = BodyShape::default();
        let start = Vec3::new(4.5, GROUND + 1.0, 4.5);
        let r = move_body(&w, start, shape, Vec3::ZERO);
        assert_eq!(r.position, start);
        assert!(r.grounded, "standing still lost ground contact");
        assert!(!r.hit_x && !r.hit_y && !r.hit_z);
    }

    #[test]
    fn small_creatures_fit_through_one_block_gaps() {
        let mut w = empty_world();
        for z in 0..8 {
            for x in 0..8 {
                w.set_block(x, 10, z, ids::STONE);
                // 高さ1の隙間だけを残す天井
                w.set_block(x, 12, z, ids::STONE);
            }
        }
        let rabbit = BodyShape { half_width: 0.2, height: 0.5, step_height: 0.6 };
        let human = BodyShape::default();
        let start = Vec3::new(3.5, 11.0, 3.5);
        assert!(!overlaps_solid(&w, start, rabbit), "a small animal cannot fit in a 1-block gap");
        assert!(overlaps_solid(&w, start, human), "a human should not fit in a 1-block gap");
    }
}
