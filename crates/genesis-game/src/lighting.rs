//! ボクセル照明。
//!
//! 光は 2 種類に分けて計算する。
//!
//! * **天空光（skylight）** — 空から降り注ぐ光。上から真っ直ぐ落ち、
//!   不透明ブロックに遮られるとそこで止まる。洞窟が暗いのはこれが届かないため。
//! * **ブロック光（blocklight）** — 松明・溶岩・ランタンなどが放つ光。
//!   光源から周囲へ 1 ブロックにつき 1 ずつ減衰しながら広がる。
//!
//! 天空光は「柱ごとに上から下へ」流すだけで求まるので、幅優先探索を使わない。
//! 広がりが必要なのは洞窟の入口など横方向だけなので、そこだけ短い距離の
//! 拡散をかける。ブロック光は光源が少ないため、素直な幅優先探索で十分速い。

use crate::blocks::{BlockId, BlockLookup};

/// 光の最大値。Minecraft と同じく 0〜15。
pub const MAX_LIGHT: u8 = 15;

/// 天空光が横方向へにじむ距離。洞窟の入口を自然に見せるためのもの。
const SKY_BLEED: u8 = 6;

/// 3 次元の光量グリッド。中心チャンクの外側 `pad` ブロックぶんも保持する。
pub struct LightVolume {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    /// 中心チャンクの原点がこのグリッド内で持つオフセット。
    pub pad: i32,
    sky: Vec<u8>,
    block: Vec<u8>,
}

impl LightVolume {
    pub fn new(width: i32, height: i32, depth: i32, pad: i32) -> Self {
        let n = (width * height * depth) as usize;
        Self {
            width,
            height,
            depth,
            pad,
            sky: vec![0; n],
            block: vec![0; n],
        }
    }

    #[inline]
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        if x < 0 || y < 0 || z < 0 || x >= self.width || y >= self.height || z >= self.depth {
            return None;
        }
        Some(((z * self.width + x) * self.height + y) as usize)
    }

    #[inline]
    pub fn sky_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.index(x, y, z).map(|i| self.sky[i]).unwrap_or(MAX_LIGHT)
    }

    #[inline]
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.index(x, y, z).map(|i| self.block[i]).unwrap_or(0)
    }

    #[inline]
    fn set_sky(&mut self, x: i32, y: i32, z: i32, v: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.sky[i] = v;
        }
    }

    #[inline]
    fn set_block(&mut self, x: i32, y: i32, z: i32, v: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.block[i] = v;
        }
    }

    /// チャンクローカル座標での光量（0.0〜1.0）。
    ///
    /// `day` は昼夜の進み具合（0=真夜中, 1=正午）。天空光はこれで弱まるが、
    /// ブロック光は夜でも変わらず光る。
    pub fn illumination(&self, lx: i32, y: i32, lz: i32, day: f32) -> f32 {
        let x = lx + self.pad;
        let z = lz + self.pad;
        let sky = self.sky_at(x, y, z) as f32 / MAX_LIGHT as f32;
        let blk = self.block_at(x, y, z) as f32 / MAX_LIGHT as f32;
        // 夜でも真っ暗にはせず、月明かりぶんを残す。
        let sky_strength = 0.12 + 0.88 * day.clamp(0.0, 1.0);
        (sky * sky_strength).max(blk).clamp(0.0, 1.0)
    }
}

/// 光の計算に必要な、ボクセルを読むための入り口。
pub trait LightSampler {
    /// グリッド座標（pad を含む）でのブロック。範囲外は不透明として扱う。
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId;
}

/// 光量グリッドを構築する。
pub fn compute_lighting(
    sampler: &dyn LightSampler,
    lookup: &BlockLookup,
    width: i32,
    height: i32,
    depth: i32,
    pad: i32,
) -> LightVolume {
    let mut vol = LightVolume::new(width, height, depth, pad);

    // --- 1. 天空光：柱ごとに上から下へ落とす ---
    for z in 0..depth {
        for x in 0..width {
            let mut light = MAX_LIGHT;
            for y in (0..height).rev() {
                let b = sampler.block(x, y, z);
                if lookup.is_opaque(b) {
                    // 不透明ブロックに当たったらそこから下は完全に影。
                    light = 0;
                } else if !b.is_air() && !lookup.is_cross(b) {
                    // 半透明ブロック（水・氷・ガラス）は光を減衰させる。
                    light = light.saturating_sub(2);
                }
                vol.set_sky(x, y, z, light);
                if light == 0 {
                    // 以降はすべて影なので、残りをまとめて 0 にする。
                    for yy in 0..y {
                        vol.set_sky(x, yy, z, 0);
                    }
                    break;
                }
            }
        }
    }

    // --- 2. 天空光の横方向へのにじみ（洞窟の入口を自然に見せる） ---
    bleed_sky(&mut vol, sampler, lookup);

    // --- 3. ブロック光：光源から幅優先探索で広げる ---
    let mut queue: Vec<(i32, i32, i32, u8)> = Vec::new();
    for z in 0..depth {
        for x in 0..width {
            for y in 0..height {
                let b = sampler.block(x, y, z);
                let emission = lookup.entry(b).light;
                if emission > 0 {
                    vol.set_block(x, y, z, emission);
                    queue.push((x, y, z, emission));
                }
            }
        }
    }
    flood_block_light(&mut vol, sampler, lookup, queue);

    vol
}

/// 天空光を水平方向へ最大 `SKY_BLEED` ブロックだけ広げる。
fn bleed_sky(vol: &mut LightVolume, sampler: &dyn LightSampler, lookup: &BlockLookup) {
    let (w, h, d) = (vol.width, vol.height, vol.depth);
    // 明るいセルから順に、暗い隣接セルへ 1 ずつ落として広げる。
    let mut queue: Vec<(i32, i32, i32, u8)> = Vec::new();
    for z in 0..d {
        for x in 0..w {
            for y in 0..h {
                let v = vol.sky_at(x, y, z);
                if v > MAX_LIGHT - SKY_BLEED {
                    queue.push((x, y, z, v));
                }
            }
        }
    }

    let mut head = 0;
    while head < queue.len() {
        let (x, y, z, level) = queue[head];
        head += 1;
        if level <= MAX_LIGHT - SKY_BLEED {
            continue;
        }
        let next = level - 1;
        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if nx < 0 || ny < 0 || nz < 0 || nx >= w || ny >= h || nz >= d {
                continue;
            }
            if lookup.is_opaque(sampler.block(nx, ny, nz)) {
                continue;
            }
            if vol.sky_at(nx, ny, nz) >= next {
                continue;
            }
            vol.set_sky(nx, ny, nz, next);
            queue.push((nx, ny, nz, next));
        }
    }
}

/// 光源から周囲へブロック光を広げる。
fn flood_block_light(
    vol: &mut LightVolume,
    sampler: &dyn LightSampler,
    lookup: &BlockLookup,
    mut queue: Vec<(i32, i32, i32, u8)>,
) {
    let (w, h, d) = (vol.width, vol.height, vol.depth);
    let mut head = 0;
    while head < queue.len() {
        let (x, y, z, level) = queue[head];
        head += 1;
        if level <= 1 {
            continue;
        }
        // 現在値のほうが強ければ、この経路は既に上書きされている。
        if vol.block_at(x, y, z) > level {
            continue;
        }
        let next = level - 1;
        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if nx < 0 || ny < 0 || nz < 0 || nx >= w || ny >= h || nz >= d {
                continue;
            }
            if lookup.is_opaque(sampler.block(nx, ny, nz)) {
                continue;
            }
            if vol.block_at(nx, ny, nz) >= next {
                continue;
            }
            vol.set_block(nx, ny, nz, next);
            queue.push((nx, ny, nz, next));
        }
    }
}

/// 面の 4 隅における環境遮蔽（アンビエントオクルージョン）。
///
/// 隣り合う 2 辺と対角のブロックが埋まっているほど、その隅は暗くなる。
/// これがあるだけでブロックの角が立体的に見え、Minecraft らしい陰影になる。
///
/// 戻り値は 0（最も暗い）〜3（遮蔽なし）。
#[inline]
pub fn corner_ao(side1: bool, side2: bool, corner: bool) -> u8 {
    if side1 && side2 {
        // 両側が塞がっていれば、対角を見るまでもなく最も暗い。
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    }
}

/// AO 値を明度の係数へ変換する。
#[inline]
pub fn ao_factor(ao: u8) -> f32 {
    match ao {
        0 => 0.55,
        1 => 0.72,
        2 => 0.86,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{ids, BlockRegistry};

    /// テスト用の単純なボクセル空間。
    struct Grid {
        w: i32,
        h: i32,
        d: i32,
        cells: Vec<BlockId>,
    }

    impl Grid {
        fn new(w: i32, h: i32, d: i32) -> Self {
            Self {
                w,
                h,
                d,
                cells: vec![BlockId(0); (w * h * d) as usize],
            }
        }
        fn set(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
            if x >= 0 && y >= 0 && z >= 0 && x < self.w && y < self.h && z < self.d {
                let i = ((z * self.w + x) * self.h + y) as usize;
                self.cells[i] = b;
            }
        }
    }

    impl LightSampler for Grid {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if x < 0 || y < 0 || z < 0 || x >= self.w || y >= self.h || z >= self.d {
                return ids::STONE; // 範囲外は岩＝不透明
            }
            self.cells[((z * self.w + x) * self.h + y) as usize]
        }
    }

    fn lookup() -> BlockLookup {
        BlockRegistry::with_builtins().snapshot()
    }

    #[test]
    fn open_sky_is_fully_lit() {
        let g = Grid::new(4, 20, 4);
        let l = lookup();
        let vol = compute_lighting(&g, &l, 4, 20, 4, 0);
        assert_eq!(vol.sky_at(2, 19, 2), MAX_LIGHT);
        assert_eq!(vol.sky_at(2, 5, 2), MAX_LIGHT, "nothing blocks the sky here");
    }

    #[test]
    fn a_roof_casts_a_shadow_underneath() {
        let mut g = Grid::new(8, 20, 8);
        // y=10 に全面の屋根を張る。
        for z in 0..8 {
            for x in 0..8 {
                g.set(x, 10, z, ids::STONE);
            }
        }
        let l = lookup();
        let vol = compute_lighting(&g, &l, 8, 20, 8, 0);
        assert_eq!(vol.sky_at(4, 15, 4), MAX_LIGHT, "above the roof should be lit");
        assert_eq!(vol.sky_at(4, 9, 4), 0, "under a solid roof must be pitch dark");
        assert_eq!(vol.sky_at(4, 2, 4), 0);
    }

    #[test]
    fn a_deep_cave_is_dark() {
        let mut g = Grid::new(16, 40, 16);
        // y<=30 を岩で埋め、y=5 に横穴を掘る。
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..=30 {
                    g.set(x, y, z, ids::STONE);
                }
            }
        }
        for x in 1..15 {
            g.set(x, 5, 8, ids::AIR);
        }
        let l = lookup();
        let vol = compute_lighting(&g, &l, 16, 40, 16, 0);
        // 洞窟の奥は天空光ゼロ。
        assert_eq!(vol.sky_at(8, 5, 8), 0, "a sealed cave must be dark");
        assert_eq!(vol.illumination(8, 5, 8, 1.0), 0.0);
    }

    #[test]
    fn a_torch_lights_up_its_surroundings() {
        let mut g = Grid::new(16, 40, 16);
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..=30 {
                    g.set(x, y, z, ids::STONE);
                }
            }
        }
        // 横穴と、その中央に松明。
        for x in 1..15 {
            g.set(x, 5, 8, ids::AIR);
        }
        g.set(8, 5, 8, ids::TORCH);

        let l = lookup();
        let vol = compute_lighting(&g, &l, 16, 40, 16, 0);
        assert!(vol.block_at(8, 5, 8) >= 14, "the torch itself must be bright");
        assert!(vol.block_at(10, 5, 8) > 0, "light should reach 2 blocks away");
        // 距離とともに暗くなる。
        assert!(vol.block_at(10, 5, 8) > vol.block_at(13, 5, 8));
        // 岩の向こう側へは漏れない。
        assert_eq!(vol.block_at(8, 5, 12), 0, "light leaked through solid rock");
    }

    #[test]
    fn block_light_works_at_night_but_skylight_does_not() {
        let mut g = Grid::new(8, 20, 8);
        g.set(4, 5, 4, ids::TORCH);
        let l = lookup();
        let vol = compute_lighting(&g, &l, 8, 20, 8, 0);

        // 真夜中（day=0）でも松明の周りは明るい。
        assert!(vol.illumination(4, 5, 4, 0.0) > 0.8, "a torch should still light the night");
        // 松明から離れた場所は、夜には月明かり程度まで落ちる。
        let far = vol.illumination(0, 19, 0, 0.0);
        assert!(far < 0.25, "open ground at midnight should be dim, got {far}");
        // 同じ場所も昼なら明るい。
        assert!(vol.illumination(0, 19, 0, 1.0) > 0.9);
    }

    #[test]
    fn lava_emits_light() {
        let mut g = Grid::new(8, 20, 8);
        for z in 0..8 {
            for x in 0..8 {
                for y in 0..=10 {
                    g.set(x, y, z, ids::STONE);
                }
            }
        }
        g.set(4, 5, 4, ids::AIR);
        g.set(4, 4, 4, ids::LAVA);
        let l = lookup();
        let vol = compute_lighting(&g, &l, 8, 20, 8, 0);
        assert!(vol.block_at(4, 4, 4) >= 14, "lava should be a strong light source");
        assert!(vol.block_at(4, 5, 4) > 0, "lava should light the space above it");
    }

    #[test]
    fn water_attenuates_skylight_with_depth() {
        let mut g = Grid::new(8, 30, 8);
        // y=10..20 を水で満たす。
        for z in 0..8 {
            for x in 0..8 {
                for y in 10..=20 {
                    g.set(x, y, z, ids::WATER);
                }
            }
        }
        let l = lookup();
        let vol = compute_lighting(&g, &l, 8, 30, 8, 0);
        let surface = vol.sky_at(4, 20, 4);
        let deep = vol.sky_at(4, 11, 4);
        assert!(surface > deep, "light should fade with depth ({surface} vs {deep})");
        assert!(deep < MAX_LIGHT);
    }

    #[test]
    fn light_never_exceeds_the_maximum() {
        let mut g = Grid::new(8, 20, 8);
        for z in 3..6 {
            for x in 3..6 {
                g.set(x, 5, z, ids::LAVA);
            }
        }
        let l = lookup();
        let vol = compute_lighting(&g, &l, 8, 20, 8, 0);
        for z in 0..8 {
            for x in 0..8 {
                for y in 0..20 {
                    assert!(vol.sky_at(x, y, z) <= MAX_LIGHT);
                    assert!(vol.block_at(x, y, z) <= MAX_LIGHT);
                    let i = vol.illumination(x, y, z, 0.5);
                    assert!((0.0..=1.0).contains(&i), "illumination out of range: {i}");
                }
            }
        }
    }

    #[test]
    fn ambient_occlusion_darkens_corners() {
        // 遮蔽なし＝最も明るい。
        assert_eq!(corner_ao(false, false, false), 3);
        // 対角だけ埋まっている。
        assert_eq!(corner_ao(false, false, true), 2);
        // 片側だけ。
        assert_eq!(corner_ao(true, false, false), 2);
        // 両側が埋まっていれば、対角に関係なく最も暗い。
        assert_eq!(corner_ao(true, true, false), 0);
        assert_eq!(corner_ao(true, true, true), 0);
        // 明度は単調に増える。
        assert!(ao_factor(0) < ao_factor(1));
        assert!(ao_factor(1) < ao_factor(2));
        assert!(ao_factor(2) < ao_factor(3));
        assert_eq!(ao_factor(3), 1.0);
    }

    #[test]
    fn lighting_is_deterministic() {
        let mut g = Grid::new(8, 20, 8);
        g.set(4, 5, 4, ids::TORCH);
        let l = lookup();
        let a = compute_lighting(&g, &l, 8, 20, 8, 0);
        let b = compute_lighting(&g, &l, 8, 20, 8, 0);
        for z in 0..8 {
            for x in 0..8 {
                for y in 0..20 {
                    assert_eq!(a.sky_at(x, y, z), b.sky_at(x, y, z));
                    assert_eq!(a.block_at(x, y, z), b.block_at(x, y, z));
                }
            }
        }
    }
}
