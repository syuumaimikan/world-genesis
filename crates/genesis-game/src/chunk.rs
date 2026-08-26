//! ボクセルチャンクのデータ表現。
//!
//! 1チャンクは 16 × 160 × 16 のボクセル列を平坦な `Vec<BlockId>` として保持する。
//! 描画用エンティティは1チャンクにつき最大2個（不透明メッシュ・半透明メッシュ）しか
//! 生成されない。ブロック1個 = 1エンティティという実装は数万エンティティを生み
//! フレームを潰すため、この設計が性能上の要となる。

use crate::blocks::BlockId;
use std::collections::HashMap;

pub const CHUNK_SX: i32 = 16;
pub const CHUNK_SZ: i32 = 16;
pub const CHUNK_H: i32 = 160;
pub const SEA_LEVEL: i32 = 64;
pub const CHUNK_VOLUME: usize = (CHUNK_SX * CHUNK_SZ * CHUNK_H) as usize;

/// チャンクのワールド座標（ブロック座標ではない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    #[inline]
    pub fn from_world(wx: f32, wz: f32) -> Self {
        Self {
            x: (wx / CHUNK_SX as f32).floor() as i32,
            z: (wz / CHUNK_SZ as f32).floor() as i32,
        }
    }

    /// チャンクの原点となるブロック座標。
    #[inline]
    pub fn origin(self) -> (i32, i32) {
        (self.x * CHUNK_SX, self.z * CHUNK_SZ)
    }

    #[inline]
    pub fn distance_sq_to(self, other: ChunkPos) -> i32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        dx * dx + dz * dz
    }
}

/// ボクセル配列のインデックス。Y が最も内側なので縦方向の走査が連続アクセスになる。
#[inline]
pub const fn voxel_index(lx: i32, y: i32, lz: i32) -> usize {
    ((lz * CHUNK_SX + lx) * CHUNK_H + y) as usize
}

#[derive(Clone)]
pub struct ChunkData {
    pub pos: ChunkPos,
    pub voxels: Vec<BlockId>,
    /// 各列の最上位の不透明ブロックの Y（地表探索・NPC 配置・スポーン判定に使う）。
    pub height_map: [i16; (CHUNK_SX * CHUNK_SZ) as usize],
    /// 列ごとのバイオームID（描画時の空色ブレンドと生態シミュレーションに使う）。
    pub biome_map: [u8; (CHUNK_SX * CHUNK_SZ) as usize],
    /// このチャンクにプレイヤー／シミュレーションによる改変が入っているか。
    /// true のチャンクだけをセーブへ書き出す（差分セーブ）。
    pub dirty_persist: bool,
    /// 満杯でない流体の水位。疎な表で持つ。
    ///
    /// 生成された海や湖はすべて水源（水位 0）なので、ここには何も入らない。
    /// プレイヤーが流した水や、掘って崩れた流れだけが記録される。
    /// これで、広大な海を抱えてもメモリを食わずに済む。
    pub fluid_levels: HashMap<u32, u8>,
}

impl ChunkData {
    pub fn empty(pos: ChunkPos) -> Self {
        Self {
            pos,
            voxels: vec![BlockId(0); CHUNK_VOLUME],
            height_map: [0; (CHUNK_SX * CHUNK_SZ) as usize],
            biome_map: [0; (CHUNK_SX * CHUNK_SZ) as usize],
            dirty_persist: false,
            fluid_levels: HashMap::new(),
        }
    }

    /// 流体の水位。流体でないマスは `None`。
    #[inline]
    pub fn fluid_level(&self, lx: i32, y: i32, lz: i32) -> Option<u8> {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ || y < 0 || y >= CHUNK_H {
            return None;
        }
        let idx = voxel_index(lx, y, lz);
        Some(self.fluid_levels.get(&(idx as u32)).copied().unwrap_or(0))
    }

    /// 流体の水位を設定する。0（水源）は表に載せない。
    #[inline]
    pub fn set_fluid_level(&mut self, lx: i32, y: i32, lz: i32, level: u8) {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ || y < 0 || y >= CHUNK_H {
            return;
        }
        let idx = voxel_index(lx, y, lz) as u32;
        if level == 0 {
            self.fluid_levels.remove(&idx);
        } else {
            self.fluid_levels.insert(idx, level);
        }
    }

    #[inline]
    pub fn get(&self, lx: i32, y: i32, lz: i32) -> BlockId {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ || y < 0 || y >= CHUNK_H {
            return BlockId(0);
        }
        self.voxels[voxel_index(lx, y, lz)]
    }

    #[inline]
    pub fn set(&mut self, lx: i32, y: i32, lz: i32, id: BlockId) {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ || y < 0 || y >= CHUNK_H {
            return;
        }
        self.voxels[voxel_index(lx, y, lz)] = id;
    }

    #[inline]
    pub fn height_at(&self, lx: i32, lz: i32) -> i32 {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ {
            return SEA_LEVEL;
        }
        self.height_map[(lz * CHUNK_SX + lx) as usize] as i32
    }

    #[inline]
    pub fn biome_at(&self, lx: i32, lz: i32) -> u8 {
        if lx < 0 || lz < 0 || lx >= CHUNK_SX || lz >= CHUNK_SZ {
            return 0;
        }
        self.biome_map[(lz * CHUNK_SX + lx) as usize]
    }

    /// 高さマップを実データから再構築する。
    pub fn rebuild_height_map(&mut self, is_solid: &dyn Fn(BlockId) -> bool) {
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                let mut h = 0i32;
                for y in (0..CHUNK_H).rev() {
                    if is_solid(self.voxels[voxel_index(lx, y, lz)]) {
                        h = y;
                        break;
                    }
                }
                self.height_map[(lz * CHUNK_SX + lx) as usize] = h as i16;
            }
        }
    }

    /// パレット化した圧縮表現へ変換する。
    /// 空気と石が支配的なので、実測でおよそ 1/20 前後まで縮む。
    pub fn to_palette_rle(&self) -> PaletteRleChunk {
        let mut palette: Vec<u16> = Vec::new();
        let mut lookup: HashMap<u16, u16> = HashMap::new();
        let mut runs: Vec<(u16, u32)> = Vec::new();

        let mut current: Option<(u16, u32)> = None;
        for v in &self.voxels {
            let pi = *lookup.entry(v.0).or_insert_with(|| {
                palette.push(v.0);
                (palette.len() - 1) as u16
            });
            match &mut current {
                Some((idx, count)) if *idx == pi && *count < u32::MAX => *count += 1,
                Some(run) => {
                    runs.push(*run);
                    current = Some((pi, 1));
                }
                None => current = Some((pi, 1)),
            }
        }
        if let Some(run) = current {
            runs.push(run);
        }

        PaletteRleChunk {
            x: self.pos.x,
            z: self.pos.z,
            palette,
            runs,
            biome_map: self.biome_map.to_vec(),
        }
    }

    pub fn from_palette_rle(src: &PaletteRleChunk, is_solid: &dyn Fn(BlockId) -> bool) -> Option<Self> {
        let mut chunk = ChunkData::empty(ChunkPos::new(src.x, src.z));
        let mut cursor = 0usize;
        for &(pi, count) in &src.runs {
            let block = *src.palette.get(pi as usize)?;
            let end = cursor.checked_add(count as usize)?;
            if end > CHUNK_VOLUME {
                return None;
            }
            for slot in &mut chunk.voxels[cursor..end] {
                *slot = BlockId(block);
            }
            cursor = end;
        }
        if cursor != CHUNK_VOLUME {
            return None;
        }
        if src.biome_map.len() == chunk.biome_map.len() {
            chunk.biome_map.copy_from_slice(&src.biome_map);
        }
        chunk.rebuild_height_map(is_solid);
        chunk.dirty_persist = true;
        Some(chunk)
    }
}

/// セーブファイル上のチャンク表現（パレット + ランレングス圧縮）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaletteRleChunk {
    pub x: i32,
    pub z: i32,
    /// パレット。値は `BlockId` の生値。
    pub palette: Vec<u16>,
    /// (パレット添字, 連続数) の列。
    pub runs: Vec<(u16, u32)>,
    pub biome_map: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(id: BlockId) -> bool {
        id.0 != 0
    }

    #[test]
    fn voxel_index_is_unique_and_in_range() {
        let mut seen = vec![false; CHUNK_VOLUME];
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                for y in 0..CHUNK_H {
                    let i = voxel_index(lx, y, lz);
                    assert!(i < CHUNK_VOLUME);
                    assert!(!seen[i], "index collision at {lx},{y},{lz}");
                    seen[i] = true;
                }
            }
        }
        assert!(seen.into_iter().all(|s| s));
    }

    #[test]
    fn out_of_bounds_access_is_air_and_ignored() {
        let mut c = ChunkData::empty(ChunkPos::new(0, 0));
        assert_eq!(c.get(-1, 5, 0), BlockId(0));
        assert_eq!(c.get(0, CHUNK_H + 4, 0), BlockId(0));
        c.set(-3, 5, 0, BlockId(7));
        c.set(0, -1, 0, BlockId(7));
        assert!(c.voxels.iter().all(|v| v.0 == 0), "OOB write leaked into the chunk");
    }

    #[test]
    fn palette_rle_round_trips() {
        let mut c = ChunkData::empty(ChunkPos::new(3, -4));
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                for y in 0..70 {
                    c.set(lx, y, lz, BlockId(if y < 60 { 1 } else { 9 }));
                }
            }
        }
        c.set(5, 100, 5, BlockId(51));
        c.rebuild_height_map(&solid);

        let packed = c.to_palette_rle();
        // 圧縮が効いているか（生の1/50以下になるはず）。
        assert!(packed.runs.len() < CHUNK_VOLUME / 50, "RLE did not compress: {} runs", packed.runs.len());

        let restored = ChunkData::from_palette_rle(&packed, &solid).expect("round trip failed");
        assert_eq!(restored.voxels, c.voxels);
        assert_eq!(restored.pos, c.pos);
        assert_eq!(restored.height_at(5, 5), 100);
    }

    #[test]
    fn truncated_rle_is_rejected_not_panicking() {
        let c = ChunkData::empty(ChunkPos::new(0, 0));
        let mut packed = c.to_palette_rle();
        packed.runs.pop();
        assert!(ChunkData::from_palette_rle(&packed, &solid).is_none());

        let mut overlong = c.to_palette_rle();
        overlong.runs.push((0, 999));
        assert!(ChunkData::from_palette_rle(&overlong, &solid).is_none());
    }

    #[test]
    fn chunk_pos_from_world_handles_negatives() {
        assert_eq!(ChunkPos::from_world(0.0, 0.0), ChunkPos::new(0, 0));
        assert_eq!(ChunkPos::from_world(15.9, 15.9), ChunkPos::new(0, 0));
        assert_eq!(ChunkPos::from_world(16.0, 0.0), ChunkPos::new(1, 0));
        assert_eq!(ChunkPos::from_world(-0.1, -0.1), ChunkPos::new(-1, -1));
        assert_eq!(ChunkPos::from_world(-16.0, -17.0), ChunkPos::new(-1, -2));
    }
}
