//! 手続き的ワールド生成。
//!
//! 一切の固定配置を持たない。地形・気候・バイオーム・洞窟・鉱床・植生は
//! 全て (シード, 座標) の純関数として決まるため、どのチャンクをどの順番で
//! どのスレッドが生成しても、同じシードなら完全に同一の世界になる。

use crate::biome::{biome_def, Biome, TreeShape, ALL_BIOMES};
use crate::blocks::{ids, BlockId, BlockLookup};
use crate::chunk::{ChunkData, ChunkPos, CHUNK_H, CHUNK_SX, CHUNK_SZ, SEA_LEVEL};
use crate::noise::{fbm2, fbm3, hash2i, hash3i, rand01_2i, rand01_3i, ridged2, voronoi2};
use crate::village::VillagePlanner;

/// 気候場のサンプル。地形の形状ではなく「環境」を表す。
#[derive(Debug, Clone, Copy)]
pub struct Climate {
    /// 大陸度 (-1=外洋, +1=内陸中心)
    pub continent: f32,
    /// 侵食度 (-1=険しい, +1=平坦)
    pub erosion: f32,
    /// 起伏（尾根/谷）
    pub peaks_valleys: f32,
    /// 気温 (-1=極寒, +1=灼熱)。標高による気温減率を含む。
    pub temperature: f32,
    /// 湿度 (-1=乾燥, +1=多湿)
    pub humidity: f32,
    /// 特異度 [0,1]。稀少バイオームの出現を制御する。
    pub weirdness: f32,
}

/// 鉱床の生成規則。プラグインから追加できるよう完全にデータ化してある。
#[derive(Debug, Clone)]
pub struct OreRule {
    pub block: BlockId,
    /// 出現しうる高度帯。
    pub min_y: i32,
    pub max_y: i32,
    /// 最も濃く出る高度（正規分布の中心）。
    pub peak_y: i32,
    /// 相対出現重み。
    pub weight: f32,
    /// 鉱脈の最小・最大半径（ブロック）。
    pub size: (f32, f32),
    /// この鉱石が偏って出るバイオーム（None なら全域）。
    pub biome_affinity: Option<Biome>,
}

/// ワールド生成のチューニング値。ワールド作成画面とプラグインが書き換える。
#[derive(Debug, Clone)]
pub struct GenParams {
    pub sea_level: i32,
    /// 地形の起伏倍率。
    pub terrain_amplitude: f32,
    /// 洞窟の量 (0=無し, 1=標準, 2=蟻の巣状)。
    pub cave_density: f32,
    /// 鉱脈の量倍率。
    pub ore_richness: f32,
    /// 植生の量倍率。
    pub vegetation_density: f32,
    /// 集落の密度倍率。
    pub settlement_density: f32,
    /// 平坦な世界（デバッグ・建築用）。
    pub flat_world: bool,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            sea_level: SEA_LEVEL,
            terrain_amplitude: 1.0,
            cave_density: 1.0,
            ore_richness: 1.0,
            vegetation_density: 1.0,
            settlement_density: 1.0,
            flat_world: false,
        }
    }
}

pub struct WorldGenerator {
    pub seed: u64,
    pub params: GenParams,
    pub ore_rules: Vec<OreRule>,
    pub villages: VillagePlanner,
}

impl WorldGenerator {
    pub fn new(seed: u64, params: GenParams) -> Self {
        let settlement_density = params.settlement_density;
        Self {
            seed,
            params,
            ore_rules: default_ore_rules(),
            villages: VillagePlanner::new(seed, settlement_density),
        }
    }

    // ------------------------------------------------------------------
    // 気候場
    // ------------------------------------------------------------------

    /// ある地点の気候。標高を渡すと気温減率（高いほど寒い）が適用される。
    pub fn climate_at(&self, wx: f32, wz: f32, altitude_above_sea: f32) -> Climate {
        let s = self.seed;

        // 大陸はドメインワーピングで有機的な海岸線を持たせる。
        let cw_x = wx * 0.0016;
        let cw_z = wz * 0.0016;
        let warp_x = fbm2(cw_x * 0.6 + 13.7, cw_z * 0.6 - 4.1, s ^ 0xC0FF, 3, 2.0, 0.5) * 0.9;
        let warp_z = fbm2(cw_x * 0.6 - 8.3, cw_z * 0.6 + 21.9, s ^ 0xEE11, 3, 2.0, 0.5) * 0.9;
        let continent = fbm2(cw_x + warp_x, cw_z + warp_z, s ^ 0x1111, 5, 2.0, 0.5);

        let erosion = fbm2(wx * 0.00085, wz * 0.00085, s ^ 0x2222, 4, 2.0, 0.5);
        let peaks_valleys = ridged2(wx * 0.0060, wz * 0.0060, s ^ 0x3333, 5);

        // 気温帯は非常に緩やかな帯として広がり、そこへ標高による気温減率が乗る。
        let temp_band = fbm2(wx * 0.00045, wz * 0.00045, s ^ 0x4444, 3, 2.0, 0.55);
        let lapse = (altitude_above_sea.max(0.0) / 62.0).min(1.6);
        let temperature = (temp_band * 1.15 - lapse).clamp(-1.0, 1.0);

        // 湿度は海からの距離に強く依存する（内陸ほど乾く）。
        let humid_band = fbm2(wx * 0.00072, wz * 0.00072, s ^ 0x5555, 4, 2.0, 0.5);
        let maritime = (1.0 - continent.max(0.0)) * 0.55;
        let humidity = (humid_band * 0.95 + maritime - 0.22).clamp(-1.0, 1.0);

        let weirdness = ((fbm2(wx * 0.0021, wz * 0.0021, s ^ 0x6666, 3, 2.0, 0.5) + 1.0) * 0.5).clamp(0.0, 1.0);

        Climate {
            continent,
            erosion,
            peaks_valleys,
            temperature,
            humidity,
            weirdness,
        }
    }

    /// 大陸度から基準標高を求めるスプライン。海溝・大陸棚・海岸平野・内陸を作り分ける。
    fn continental_base(&self, c: f32) -> f32 {
        const PTS: [(f32, f32); 7] = [
            (-1.00, 20.0),
            (-0.58, 40.0),
            (-0.30, 56.0),
            (-0.12, 65.0),
            (0.08, 71.0),
            (0.45, 82.0),
            (1.00, 100.0),
        ];
        if c <= PTS[0].0 {
            return PTS[0].1;
        }
        for w in PTS.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            if c <= x1 {
                let t = ((c - x0) / (x1 - x0)).clamp(0.0, 1.0);
                // smoothstep でつなぎ、傾きの不連続（段差）を消す。
                let t = t * t * (3.0 - 2.0 * t);
                return y0 + (y1 - y0) * t;
            }
        }
        PTS[PTS.len() - 1].1
    }

    /// 地表の標高（ブロック単位）。
    pub fn terrain_height(&self, wx: f32, wz: f32) -> i32 {
        if self.params.flat_world {
            return self.params.sea_level + 4;
        }
        // 標高が気温へ影響するが、気温は標高計算に使わないので altitude=0 で十分。
        let c = self.climate_at(wx, wz, 0.0);
        self.terrain_height_from(wx, wz, &c)
    }

    fn terrain_height_from(&self, wx: f32, wz: f32, c: &Climate) -> i32 {
        if self.params.flat_world {
            return self.params.sea_level + 4;
        }
        let base = self.continental_base(c.continent);

        // 侵食度が低い(-1)ほど険しく、高い(+1)ほど削られて平坦になる。
        let ruggedness = ((1.0 - c.erosion) * 0.5).clamp(0.0, 1.0);
        // 山脈は内陸でのみ立ち上がる。
        let landness = ((c.continent + 0.10) / 0.9).clamp(0.0, 1.0);
        let pv_positive = (c.peaks_valleys * 0.5 + 0.5).clamp(0.0, 1.0);

        let mountains = ruggedness.powf(1.5) * landness * pv_positive.powf(1.7) * 78.0;
        let hills = fbm2(wx * 0.017, wz * 0.017, self.seed ^ 0x7777, 4, 2.0, 0.5) * 7.5 * (0.3 + ruggedness);
        let micro = fbm2(wx * 0.075, wz * 0.075, self.seed ^ 0x8888, 2, 2.0, 0.5) * 1.6;

        let raw = base + (mountains + hills + micro) * self.params.terrain_amplitude;
        let h = raw.round() as i32;
        h.clamp(4, CHUNK_H - 12)
    }

    pub fn biome_at(&self, wx: f32, wz: f32) -> Biome {
        let sea = self.params.sea_level;
        let c0 = self.climate_at(wx, wz, 0.0);
        let h = self.terrain_height_from(wx, wz, &c0);
        let c = self.climate_at(wx, wz, (h - sea) as f32);
        crate::biome::classify(
            c.continent,
            c.temperature,
            c.humidity,
            c.erosion,
            c.weirdness,
            h,
            sea,
        )
    }

    // ------------------------------------------------------------------
    // 洞窟
    // ------------------------------------------------------------------

    /// この座標が洞窟として掘り抜かれるか。
    fn is_cave(&self, wx: i32, y: i32, wz: i32, surface_y: i32) -> bool {
        if self.params.cave_density <= 0.0 {
            return false;
        }
        // 地表直下は残し、岩盤層は掘らない。
        if y < 6 || y > surface_y - 4 {
            return false;
        }
        let fx = wx as f32;
        let fy = y as f32;
        let fz = wz as f32;

        // 深いほど広がる補正。
        let depth_bonus = ((surface_y - y) as f32 / 90.0).clamp(0.0, 1.0);
        let d = self.params.cave_density;

        // 1) スパゲッティ状のトンネル：2枚の尾根状ノイズが同時にゼロ交差する位置。
        let t1 = fbm3(fx * 0.0135, fy * 0.0230, fz * 0.0135, self.seed ^ 0xAA01, 2);
        let t2 = fbm3(fx * 0.0135 + 31.7, fy * 0.0230 - 9.4, fz * 0.0135 + 5.2, self.seed ^ 0xAA02, 2);
        let tunnel_w = (0.055 + depth_bonus * 0.030) * d;
        if t1.abs() < tunnel_w && t2.abs() < tunnel_w {
            return true;
        }

        // 2) チーズ状の空洞：低頻度ノイズの高い部分。深部にのみ大空洞ができる。
        if y < surface_y - 18 {
            let cheese = fbm3(fx * 0.0088, fy * 0.0150, fz * 0.0088, self.seed ^ 0xAA03, 3);
            let threshold = 0.62 - depth_bonus * 0.13 * d;
            if cheese > threshold {
                return true;
            }
        }

        // 3) 鍾乳洞状の大空洞：ボロノイのセル中心付近に球状の部屋を開ける。
        if y < surface_y - 26 && y > 10 {
            let (dist, cell) = voronoi2(fx * 0.0042, fz * 0.0042, self.seed ^ 0xAA04);
            // セルごとに部屋の中心高度と大きさを変える。
            let room_y = 14 + ((cell >> 8) % 46) as i32;
            let room_r = 0.055 + ((cell >> 20) % 40) as f32 * 0.0016;
            let dy = (y - room_y) as f32 / 16.0;
            if dist < room_r * d && dy.abs() < 1.0 {
                return true;
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // チャンク生成
    // ------------------------------------------------------------------

    pub fn generate_chunk(&self, pos: ChunkPos, lookup: &BlockLookup) -> ChunkData {
        let mut chunk = ChunkData::empty(pos);
        let (ox, oz) = pos.origin();
        let sea = self.params.sea_level;

        // --- 1. 地形の柱を積む ---
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                let wx = ox + lx;
                let wz = oz + lz;
                let (fx, fz) = (wx as f32, wz as f32);

                let c0 = self.climate_at(fx, fz, 0.0);
                let surface_y = self.terrain_height_from(fx, fz, &c0);
                let c = self.climate_at(fx, fz, (surface_y - sea) as f32);
                let biome = crate::biome::classify(
                    c.continent, c.temperature, c.humidity, c.erosion, c.weirdness, surface_y, sea,
                );
                let def = biome_def(biome);

                let bi = ALL_BIOMES.iter().position(|&b| b == biome).unwrap_or(0) as u8;
                chunk.biome_map[(lz * CHUNK_SX + lx) as usize] = bi;

                // 土壌層の厚み（岩がちなバイオームは薄い）。
                let soil_depth = 3 + (rand01_2i(wx, wz, self.seed ^ 0xB00B) * 2.0) as i32;

                for y in 0..CHUNK_H {
                    if y > surface_y {
                        // 地表より上：海面以下なら水、それ以外は空気。
                        if y <= sea && !matches!(biome, Biome::FrozenOcean) {
                            chunk.set(lx, y, lz, ids::WATER);
                        } else if y <= sea {
                            // 氷海は表層が氷、その下は水。
                            chunk.set(lx, y, lz, if y == sea { ids::ICE } else { ids::WATER });
                        }
                        continue;
                    }

                    if y <= 2 {
                        chunk.set(lx, y, lz, ids::BEDROCK);
                        continue;
                    }

                    if self.is_cave(wx, y, wz, surface_y) {
                        // 掘り抜かれた空間。深部は溶岩湖、中層は地下水脈。
                        if y < 11 {
                            chunk.set(lx, y, lz, ids::LAVA);
                        } else {
                            let aquifer = fbm3(wx as f32 * 0.011, y as f32 * 0.05, wz as f32 * 0.011, self.seed ^ 0xC0DE, 2);
                            if aquifer > 0.42 && y < sea - 6 {
                                chunk.set(lx, y, lz, ids::WATER);
                            }
                            // それ以外は空気のまま（洞窟）。
                        }
                        continue;
                    }

                    let block = if y == surface_y {
                        if surface_y < sea {
                            def.underwater
                        } else {
                            def.surface
                        }
                    } else if y > surface_y - soil_depth {
                        if surface_y < sea { def.underwater } else { def.subsoil }
                    } else {
                        self.deep_stone(wx, y, wz, &def)
                    };
                    chunk.set(lx, y, lz, block);
                }

                chunk.height_map[(lz * CHUNK_SX + lx) as usize] = surface_y as i16;
            }
        }

        // --- 2. 鉱脈 ---
        self.place_ore_veins(&mut chunk, lookup);

        // --- 3. 集落（樹木より先に置き、家の中に木が生えないようにする） ---
        self.villages.stamp_into_chunk(self, &mut chunk);

        // --- 4. 植生 ---
        self.decorate(&mut chunk, lookup);

        chunk
    }

    /// 深部の岩石種。地質年代・深さ・貫入岩体で岩相が変わる。
    fn deep_stone(&self, wx: i32, y: i32, wz: i32, def: &crate::biome::BiomeDef) -> BlockId {
        // 岩体はブロック単位では変わらない大きな塊として分布する。
        let n = fbm3(wx as f32 * 0.021, y as f32 * 0.030, wz as f32 * 0.021, self.seed ^ 0xD1CE, 2);
        if y < 22 {
            // 深部：貫入した花崗岩・玄武岩が多い。
            if n > 0.44 {
                return ids::GRANITE;
            }
            if n < -0.46 {
                return ids::BASALT;
            }
            if n > 0.30 {
                return ids::DIORITE;
            }
            return def.bedrock_stone;
        }
        if n > 0.52 {
            return ids::GRANITE;
        }
        if n < -0.52 {
            return ids::TUFF;
        }
        if n > 0.34 {
            return ids::DIORITE;
        }
        if n < -0.34 {
            // 堆積岩層。石灰岩は洞窟と鍾乳洞の母岩になる。
            return ids::LIMESTONE;
        }
        if n.abs() < 0.03 {
            return ids::MARBLE;
        }
        def.bedrock_stone
    }

    // ------------------------------------------------------------------
    // 鉱脈
    // ------------------------------------------------------------------

    fn place_ore_veins(&self, chunk: &mut ChunkData, lookup: &BlockLookup) {
        if self.ore_rules.is_empty() || self.params.ore_richness <= 0.0 {
            return;
        }
        let total_weight: f32 = self.ore_rules.iter().map(|r| r.weight).sum();
        if total_weight <= 0.0 {
            return;
        }

        // 隣接チャンク由来の鉱脈も跨って生成されるよう、1チャンク分外側まで走査する。
        for cdz in -1..=1 {
            for cdx in -1..=1 {
                let cx = chunk.pos.x + cdx;
                let cz = chunk.pos.z + cdz;
                let veins = (26.0 * self.params.ore_richness).round().clamp(0.0, 200.0) as i32;

                for v in 0..veins {
                    let h = hash3i(cx, v, cz, self.seed ^ 0x0FE0_0FE0);
                    let vx = cx * CHUNK_SX + (h % CHUNK_SX as u64) as i32;
                    let vz = cz * CHUNK_SZ + ((h >> 8) % CHUNK_SZ as u64) as i32;
                    let vy = 3 + ((h >> 16) % (CHUNK_H as u64 - 8)) as i32;

                    // 深さ・バイオームで重み付けした抽選。
                    let biome = if cdx == 0 && cdz == 0 {
                        let lx = vx - chunk.pos.x * CHUNK_SX;
                        let lz = vz - chunk.pos.z * CHUNK_SZ;
                        ALL_BIOMES
                            .get(chunk.biome_at(lx, lz) as usize)
                            .copied()
                            .unwrap_or(Biome::Plains)
                    } else {
                        self.biome_at(vx as f32, vz as f32)
                    };

                    let Some(rule) = self.pick_ore(vy, biome, (h >> 32) as u32) else {
                        continue;
                    };

                    let t = ((h >> 48) % 1024) as f32 / 1024.0;
                    let radius = rule.size.0 + (rule.size.1 - rule.size.0) * t;
                    self.carve_vein(chunk, lookup, vx, vy, vz, radius, rule.block, h);
                }
            }
        }
    }

    fn pick_ore(&self, y: i32, biome: Biome, roll: u32) -> Option<&OreRule> {
        let mut weights: [f32; 64] = [0.0; 64];
        let mut total = 0.0f32;
        for (i, r) in self.ore_rules.iter().enumerate().take(64) {
            if y < r.min_y || y > r.max_y {
                continue;
            }
            // 出現ピークからの距離でガウシアンに減衰させる。
            let span = ((r.max_y - r.min_y) as f32 * 0.5).max(1.0);
            let d = (y - r.peak_y) as f32 / span;
            let mut w = r.weight * (-d * d * 2.0).exp();
            if let Some(af) = r.biome_affinity {
                if af == biome {
                    w *= 4.0;
                } else {
                    w *= 0.55;
                }
            }
            weights[i] = w;
            total += w;
        }
        if total <= 0.0 {
            return None;
        }
        let mut pick = (roll % 100_000) as f32 / 100_000.0 * total;
        for (i, w) in weights.iter().enumerate() {
            if *w <= 0.0 {
                continue;
            }
            if pick < *w {
                return self.ore_rules.get(i);
            }
            pick -= *w;
        }
        self.ore_rules.first()
    }

    /// 鉱脈をノイズで歪んだ楕円体として掘り込む。石系ブロックのみ置換する。
    fn carve_vein(
        &self,
        chunk: &mut ChunkData,
        lookup: &BlockLookup,
        vx: i32,
        vy: i32,
        vz: i32,
        radius: f32,
        ore: BlockId,
        h: u64,
    ) {
        let r = radius.clamp(0.6, 6.0);
        let ri = r.ceil() as i32;
        let (ox, oz) = chunk.pos.origin();

        // 鉱脈は等方的ではなく、層に沿って伸びる。
        let sx = 1.0 + ((h >> 4) % 100) as f32 / 100.0;
        let sy = 0.55 + ((h >> 12) % 60) as f32 / 100.0;
        let sz = 1.0 + ((h >> 20) % 100) as f32 / 100.0;

        for dy in -ri..=ri {
            let y = vy + dy;
            if !(3..CHUNK_H).contains(&y) {
                continue;
            }
            for dz in -ri..=ri {
                let lz = vz + dz - oz;
                if !(0..CHUNK_SZ).contains(&lz) {
                    continue;
                }
                for dx in -ri..=ri {
                    let lx = vx + dx - ox;
                    if !(0..CHUNK_SX).contains(&lx) {
                        continue;
                    }
                    let fx = dx as f32 / sx;
                    let fy = dy as f32 / sy;
                    let fz = dz as f32 / sz;
                    let d = (fx * fx + fy * fy + fz * fz).sqrt();
                    // 縁をノイズで崩して自然な形にする。
                    let jitter = rand01_3i(vx + dx, y, vz + dz, self.seed ^ 0x5EED) * 0.75;
                    if d - jitter > r {
                        continue;
                    }
                    let existing = chunk.get(lx, y, lz);
                    // 空洞・水・土壌は置換しない（岩の中にだけ鉱脈が入る）。
                    if is_replaceable_stone(existing) && lookup.is_opaque(existing) {
                        chunk.set(lx, y, lz, ore);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 植生
    // ------------------------------------------------------------------

    fn decorate(&self, chunk: &mut ChunkData, lookup: &BlockLookup) {
        let (ox, oz) = chunk.pos.origin();
        let sea = self.params.sea_level;
        let veg = self.params.vegetation_density;

        // 樹木は隣接チャンクからはみ出すため、外側 8 ブロックまで走査してクリップする。
        const MARGIN: i32 = 8;
        for lz in -MARGIN..CHUNK_SZ + MARGIN {
            for lx in -MARGIN..CHUNK_SX + MARGIN {
                let wx = ox + lx;
                let wz = oz + lz;
                let inside = (0..CHUNK_SX).contains(&lx) && (0..CHUNK_SZ).contains(&lz);

                // チャンク内なら生成済みの高さマップを使い、外側だけ再計算する。
                let (surface_y, biome) = if inside {
                    let b = ALL_BIOMES
                        .get(chunk.biome_at(lx, lz) as usize)
                        .copied()
                        .unwrap_or(Biome::Plains);
                    (chunk.height_at(lx, lz), b)
                } else {
                    let h = self.terrain_height(wx as f32, wz as f32);
                    (h, self.biome_at(wx as f32, wz as f32))
                };

                if surface_y < sea {
                    if inside {
                        self.decorate_seafloor(chunk, lx, lz, surface_y, biome, veg);
                    }
                    continue;
                }
                let def = biome_def(biome);

                // --- 樹木 ---
                if def.tree_density > 0.0 {
                    let roll = rand01_2i(wx, wz, self.seed ^ 0x7EEE);
                    if roll < def.tree_density * veg {
                        // 密生しすぎないよう、近傍で最も強い木だけを残す（簡易ポアソン間引き）。
                        if self.is_local_tree_winner(wx, wz, roll) {
                            self.place_tree(chunk, lx, surface_y + 1, lz, &def, wx, wz);
                        }
                    }
                }

                if !inside {
                    continue;
                }

                // --- 下草・花・作物（チャンク内のみ） ---
                let top = chunk.get(lx, surface_y, lz);
                if !lookup.is_opaque(top) {
                    continue;
                }
                let above = chunk.get(lx, surface_y + 1, lz);
                if !above.is_air() {
                    continue;
                }

                let g = rand01_2i(wx, wz, self.seed ^ 0x9A5F);
                if def.flower_density > 0.0 && g < def.flower_density * veg {
                    let idx = (hash2i(wx, wz, self.seed ^ 0x1F10) % def.flowers.len().max(1) as u64) as usize;
                    if let Some(&flower) = def.flowers.get(idx) {
                        chunk.set(lx, surface_y + 1, lz, flower);
                    }
                } else if def.grass_density > 0.0 && g < def.flower_density * veg + def.grass_density * veg {
                    chunk.set(lx, surface_y + 1, lz, def.grass_block);
                }
            }
        }
    }

    /// 近傍 5×5 の中で最も高い抽選値を持つ列だけが木を立てる。幹の重なりを防ぐ。
    fn is_local_tree_winner(&self, wx: i32, wz: i32, roll: f32) -> bool {
        for dz in -2..=2i32 {
            for dx in -2..=2i32 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let other = rand01_2i(wx + dx, wz + dz, self.seed ^ 0x7EEE);
                // 同値のときは座標順で決定的に決着させる。
                if other < roll || (other == roll && (dx, dz) < (0, 0)) {
                    return false;
                }
            }
        }
        true
    }

    fn decorate_seafloor(&self, chunk: &mut ChunkData, lx: i32, lz: i32, surface_y: i32, biome: Biome, veg: f32) {
        let def = biome_def(biome);
        if def.grass_density <= 0.0 || def.grass_block.is_air() {
            return;
        }
        let (ox, oz) = chunk.pos.origin();
        let g = rand01_2i(ox + lx, oz + lz, self.seed ^ 0x0CE4);
        if g < def.grass_density * veg && chunk.get(lx, surface_y + 1, lz) == ids::WATER {
            chunk.set(lx, surface_y + 1, lz, def.grass_block);
        }
    }

    /// バイオームの樹形テンプレートに従って樹木を組み立てる。
    /// チャンク外へはみ出した部分は `ChunkData::set` が自動的に捨てる。
    /// 幹も葉も空気の位置にしか書き込まないため、先に生成された家屋や道を
    /// 樹木が貫くことはない。
    #[allow(clippy::too_many_arguments)]
    fn place_tree(
        &self,
        chunk: &mut ChunkData,
        lx: i32,
        base_y: i32,
        lz: i32,
        def: &crate::biome::BiomeDef,
        wx: i32,
        wz: i32,
    ) {
        let h = hash2i(wx, wz, self.seed ^ 0x77A2);
        let log = def.tree_log;
        let leaves = def.tree_leaves;
        let var = (h % 5) as i32;

        match def.tree_shape {
            TreeShape::None => {}
            TreeShape::Round => {
                let trunk = 4 + var;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, log);
                }
                let top = base_y + trunk;
                let r = 2 + (var / 3);
                for dy in -2..=1i32 {
                    let rr = if dy >= 0 { r - 1 } else { r };
                    for dz in -rr..=rr {
                        for dx in -rr..=rr {
                            if dx * dx + dz * dz > rr * rr + 1 {
                                continue;
                            }
                            if dx == 0 && dz == 0 && dy < 1 {
                                continue;
                            }
                            let y = top + dy;
                            if chunk.get(lx + dx, y, lz + dz).is_air() {
                                set_if_air(chunk, lx + dx, y, lz + dz, leaves);
                            }
                        }
                    }
                }
            }
            TreeShape::Conifer => {
                let trunk = 7 + var * 2;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, log);
                }
                // 下から上へ細くなる円錐。
                let layers = trunk - 2;
                for i in 0..layers {
                    let y = base_y + 2 + i;
                    let t = 1.0 - i as f32 / layers as f32;
                    let r = (t * 3.0).round() as i32;
                    if r <= 0 {
                        continue;
                    }
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx * dx + dz * dz > r * r {
                                continue;
                            }
                            if dx == 0 && dz == 0 {
                                continue;
                            }
                            if chunk.get(lx + dx, y, lz + dz).is_air() {
                                set_if_air(chunk, lx + dx, y, lz + dz, leaves);
                            }
                        }
                    }
                }
                set_if_air(chunk, lx, base_y + trunk, lz, leaves);
            }
            TreeShape::Giant => {
                let trunk = 12 + var * 3;
                // 2×2 の太い幹。
                for y in 0..trunk {
                    for dz in 0..2 {
                        for dx in 0..2 {
                            set_if_air(chunk, lx + dx, base_y + y, lz + dz, log);
                        }
                    }
                }
                // 多層の樹冠。
                for (i, dy) in [(0i32, 0i32), (1, -4), (2, -8)] {
                    let r = 4 - i;
                    let y = base_y + trunk + dy;
                    for ddy in -1..=1i32 {
                        for dz in -r..=r + 1 {
                            for dx in -r..=r + 1 {
                                if dx * dx + dz * dz > r * r + 2 {
                                    continue;
                                }
                                if chunk.get(lx + dx, y + ddy, lz + dz).is_air() {
                                    set_if_air(chunk, lx + dx, y + ddy, lz + dz, leaves);
                                }
                            }
                        }
                    }
                }
                // 垂れ下がるツタ。
                for k in 0..6 {
                    let a = (hash2i(wx, wz + k, self.seed ^ 0x1122) % 8) as i32 - 4;
                    let b = (hash2i(wx + k, wz, self.seed ^ 0x3344) % 8) as i32 - 4;
                    for d in 0..(2 + (k % 4)) {
                        let y = base_y + trunk - 2 - d;
                        if chunk.get(lx + a, y, lz + b).is_air() {
                            set_if_air(chunk, lx + a, y, lz + b, ids::VINE);
                        }
                    }
                }
            }
            TreeShape::Umbrella => {
                let trunk = 5 + var;
                let lean_x = ((h >> 8) % 3) as i32 - 1;
                let lean_z = ((h >> 16) % 3) as i32 - 1;
                let mut cx = lx;
                let mut cz = lz;
                for y in 0..trunk {
                    set_if_air(chunk, cx, base_y + y, cz, log);
                    if y > trunk / 2 && y % 2 == 0 {
                        cx += lean_x;
                        cz += lean_z;
                    }
                }
                let y = base_y + trunk;
                for dz in -3..=3i32 {
                    for dx in -3..=3i32 {
                        if dx.abs() + dz.abs() > 4 {
                            continue;
                        }
                        set_if_air(chunk, cx + dx, y, cz + dz, leaves);
                        if dx.abs() + dz.abs() <= 2 {
                            set_if_air(chunk, cx + dx, y + 1, cz + dz, leaves);
                        }
                    }
                }
            }
            TreeShape::Palm => {
                let trunk = 6 + var;
                let mut cx = lx;
                let cz = lz;
                let bend = ((h >> 8) % 2) as i32 * 2 - 1;
                for y in 0..trunk {
                    set_if_air(chunk, cx, base_y + y, cz, log);
                    if y >= trunk - 3 {
                        cx += bend;
                    }
                }
                let y = base_y + trunk;
                for (dx, dz) in [(2i32, 0i32), (-2, 0), (0, 2), (0, -2), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                    set_if_air(chunk, cx + dx, y, cz + dz, leaves);
                    set_if_air(chunk, cx + dx / 2, y + 1, cz + dz / 2, leaves);
                }
                set_if_air(chunk, cx, y + 1, cz, leaves);
            }
            TreeShape::Mangrove => {
                // 支柱根：幹の周りに斜めの根を張る。
                for (dx, dz) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    set_if_air(chunk, lx + dx, base_y - 1, lz + dz, log);
                    set_if_air(chunk, lx + dx, base_y, lz + dz, log);
                }
                let trunk = 5 + var;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, log);
                }
                let top = base_y + trunk;
                for dy in -1..=1i32 {
                    let r = 3 - dy.abs();
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx * dx + dz * dz > r * r + 1 {
                                continue;
                            }
                            if chunk.get(lx + dx, top + dy, lz + dz).is_air() {
                                set_if_air(chunk, lx + dx, top + dy, lz + dz, leaves);
                            }
                        }
                    }
                }
            }
            TreeShape::Dead => {
                let trunk = 3 + var;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, log);
                }
                // 枝。
                for (i, (dx, dz)) in [(1i32, 0i32), (-1, 0), (0, 1)].iter().enumerate() {
                    let y = base_y + trunk - 1 - i as i32;
                    set_if_air(chunk, lx + dx, y, lz + dz, log);
                }
            }
            TreeShape::Cactus => {
                let trunk = 2 + (h % 3) as i32;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, ids::CACTUS);
                }
            }
            TreeShape::Bamboo => {
                // 竹は1本ずつではなく数本の株立ちにする。
                for k in 0..4 {
                    let dx = (hash2i(wx + k, wz, self.seed ^ 0x8811) % 3) as i32 - 1;
                    let dz = (hash2i(wx, wz + k, self.seed ^ 0x8822) % 3) as i32 - 1;
                    let hgt = 6 + (hash2i(wx + k, wz + k, self.seed ^ 0x8833) % 8) as i32;
                    for y in 0..hgt {
                        set_if_air(chunk, lx + dx, base_y + y, lz + dz, ids::BAMBOO);
                    }
                }
            }
            TreeShape::Mushroom => {
                let trunk = 3 + var;
                for y in 0..trunk {
                    set_if_air(chunk, lx, base_y + y, lz, def.tree_log);
                }
                let y = base_y + trunk;
                for dz in -2..=2i32 {
                    for dx in -2..=2i32 {
                        if dx.abs() + dz.abs() > 3 {
                            continue;
                        }
                        set_if_air(chunk, lx + dx, y, lz + dz, def.tree_leaves);
                    }
                }
            }
        }
    }
}

/// 空気の位置にだけ書き込む。樹木の生成が、先に建った家屋・道・農地を
/// 貫通して壊すことを防ぐ。
#[inline]
fn set_if_air(chunk: &mut ChunkData, lx: i32, y: i32, lz: i32, id: BlockId) {
    if chunk.get(lx, y, lz).is_air() {
        chunk.set(lx, y, lz, id);
    }
}

/// 鉱脈が置換してよいブロックか（岩石系のみ）。
#[inline]
fn is_replaceable_stone(id: BlockId) -> bool {
    matches!(
        id,
        ids::STONE | ids::GRANITE | ids::DIORITE | ids::BASALT | ids::LIMESTONE
            | ids::TUFF | ids::MARBLE | ids::SANDSTONE | ids::TERRACOTTA
    )
}

/// 現実の鉱床学に沿った既定の鉱脈規則。
/// - 石炭は堆積盆（浅い層）に広く分布する
/// - 熱水性鉱脈（銀・金・鉛・亜鉛）は中深度
/// - キンバーライト起源のダイヤは最深部にのみ
pub fn default_ore_rules() -> Vec<OreRule> {
    use ids::*;
    vec![
        OreRule { block: COAL_ORE,    min_y: 20, max_y: 130, peak_y: 78, weight: 20.0, size: (1.8, 3.6), biome_affinity: None },
        OreRule { block: IRON_ORE,    min_y: 6,  max_y: 96,  peak_y: 40, weight: 16.0, size: (1.4, 2.8), biome_affinity: None },
        OreRule { block: COPPER_ORE,  min_y: 20, max_y: 88,  peak_y: 52, weight: 12.0, size: (1.6, 3.2), biome_affinity: None },
        OreRule { block: TIN_ORE,     min_y: 12, max_y: 70,  peak_y: 38, weight: 7.0,  size: (1.2, 2.4), biome_affinity: None },
        OreRule { block: ZINC_ORE,    min_y: 10, max_y: 62,  peak_y: 34, weight: 6.0,  size: (1.2, 2.4), biome_affinity: None },
        OreRule { block: LEAD_ORE,    min_y: 8,  max_y: 56,  peak_y: 30, weight: 6.0,  size: (1.2, 2.2), biome_affinity: None },
        OreRule { block: SILVER_ORE,  min_y: 6,  max_y: 44,  peak_y: 24, weight: 4.0,  size: (1.0, 2.0), biome_affinity: Some(Biome::RockyMountains) },
        OreRule { block: GOLD_ORE,    min_y: 4,  max_y: 36,  peak_y: 18, weight: 3.2,  size: (0.9, 1.9), biome_affinity: Some(Biome::Badlands) },
        OreRule { block: LAPIS_ORE,   min_y: 5,  max_y: 40,  peak_y: 20, weight: 2.6,  size: (0.9, 1.8), biome_affinity: None },
        OreRule { block: EMERALD_ORE, min_y: 8,  max_y: 120, peak_y: 96, weight: 1.6,  size: (0.7, 1.3), biome_affinity: Some(Biome::RockyMountains) },
        OreRule { block: DIAMOND_ORE, min_y: 3,  max_y: 20,  peak_y: 9,  weight: 1.5,  size: (0.8, 1.6), biome_affinity: None },
        OreRule { block: QUARTZ_ORE,  min_y: 10, max_y: 90,  peak_y: 46, weight: 7.0,  size: (1.4, 2.8), biome_affinity: None },
        OreRule { block: SULFUR_ORE,  min_y: 12, max_y: 110, peak_y: 60, weight: 4.5,  size: (1.2, 2.6), biome_affinity: Some(Biome::Volcanic) },
        OreRule { block: SALT_ORE,    min_y: 24, max_y: 92,  peak_y: 58, weight: 4.5,  size: (1.6, 3.4), biome_affinity: Some(Biome::Desert) },
        OreRule { block: URANIUM_ORE, min_y: 3,  max_y: 26,  peak_y: 12, weight: 1.2,  size: (0.7, 1.4), biome_affinity: None },
        OreRule { block: OIL_SHALE,   min_y: 14, max_y: 64,  peak_y: 36, weight: 5.0,  size: (2.0, 4.2), biome_affinity: Some(Biome::Swamp) },
        OreRule { block: AMBER_ORE,   min_y: 40, max_y: 110, peak_y: 72, weight: 2.2,  size: (1.0, 2.0), biome_affinity: Some(Biome::Taiga) },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BlockRegistry;

    fn gen() -> WorldGenerator {
        WorldGenerator::new(0xDEAD_BEEF, GenParams::default())
    }

    #[test]
    fn chunk_generation_is_deterministic() {
        let lookup = BlockRegistry::with_builtins().snapshot();
        let g = gen();
        let a = g.generate_chunk(ChunkPos::new(3, -7), &lookup);
        let b = g.generate_chunk(ChunkPos::new(3, -7), &lookup);
        assert_eq!(a.voxels, b.voxels, "same seed + same coord must give identical voxels");
        assert_eq!(a.height_map, b.height_map);
        assert_eq!(a.biome_map, b.biome_map);
    }

    #[test]
    fn different_seeds_make_different_worlds() {
        let lookup = BlockRegistry::with_builtins().snapshot();
        let a = WorldGenerator::new(1, GenParams::default()).generate_chunk(ChunkPos::new(0, 0), &lookup);
        let b = WorldGenerator::new(2, GenParams::default()).generate_chunk(ChunkPos::new(0, 0), &lookup);
        assert_ne!(a.voxels, b.voxels);
    }

    #[test]
    fn terrain_stays_inside_the_chunk_column() {
        let g = gen();
        for i in -400..400i32 {
            let h = g.terrain_height(i as f32 * 7.3, i as f32 * -3.1);
            assert!(h >= 4 && h < CHUNK_H - 11, "height {h} escaped the world column");
        }
    }

    #[test]
    fn bedrock_floor_is_unbroken() {
        let lookup = BlockRegistry::with_builtins().snapshot();
        let g = gen();
        for (cx, cz) in [(0, 0), (5, 9), (-13, 4)] {
            let c = g.generate_chunk(ChunkPos::new(cx, cz), &lookup);
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    for y in 0..=2 {
                        assert_eq!(c.get(lx, y, lz), ids::BEDROCK, "hole in the bedrock floor");
                    }
                }
            }
        }
    }

    #[test]
    fn oceans_are_filled_to_sea_level_and_land_is_not_flooded() {
        let lookup = BlockRegistry::with_builtins().snapshot();
        let g = gen();
        let mut checked_ocean = 0;
        let mut checked_land = 0;
        for cx in -6..6 {
            let c = g.generate_chunk(ChunkPos::new(cx, cx * 3), &lookup);
            for lz in (0..CHUNK_SZ).step_by(4) {
                for lx in (0..CHUNK_SX).step_by(4) {
                    let h = c.height_at(lx, lz);
                    if h < SEA_LEVEL - 2 {
                        // 海面直下は水（または氷海の氷）でなければならない。
                        let b = c.get(lx, SEA_LEVEL - 1, lz);
                        assert!(b == ids::WATER || b == ids::ICE, "ocean column not filled: {b:?}");
                        checked_ocean += 1;
                    } else if h > SEA_LEVEL + 6 {
                        // 陸地の地表より上に海水があってはならない。
                        assert_ne!(c.get(lx, h + 1, lz), ids::WATER, "land was flooded");
                        checked_land += 1;
                    }
                }
            }
        }
        assert!(checked_ocean > 0 && checked_land > 0, "test did not sample both ocean and land");
    }

    #[test]
    fn height_map_points_at_solid_ground_everywhere() {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let g = gen();
        // 集落の整地が入るチャンクも含めて検査する。
        for (cx, cz) in [(2, 2), (0, 0), (-9, 14), (23, -6)] {
            let c = g.generate_chunk(ChunkPos::new(cx, cz), &lookup);
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    let h = c.height_at(lx, lz);
                    assert!(h > 0 && h < CHUNK_H, "height {h} out of range");
                    // 高さマップは常に「立てる地面」を指していなければならない。
                    // NPC とプレイヤーの接地判定がこれに依存する。
                    assert!(
                        !c.get(lx, h, lz).is_air(),
                        "height map at ({cx},{cz})/({lx},{lz}) points at air (y={h})"
                    );
                }
            }
        }
    }

    #[test]
    fn ores_only_replace_stone_never_air_or_water() {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let mut params = GenParams::default();
        params.ore_richness = 4.0; // 検出しやすいよう鉱脈を増やす
        // 鍛冶屋の内装が鉄鉱石を地上へ置くため、集落を切って鉱脈生成だけを見る。
        params.settlement_density = 0.0;
        let g = WorldGenerator::new(77, params);
        let c = g.generate_chunk(ChunkPos::new(0, 0), &lookup);

        let mut ore_count = 0;
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                let surface = c.height_at(lx, lz);
                for y in 0..CHUNK_H {
                    let b = c.get(lx, y, lz);
                    if (50..=66).contains(&b.0) {
                        ore_count += 1;
                        assert!(y >= 3, "ore replaced bedrock");
                        assert!(y <= surface, "ore appeared above the surface");
                    }
                }
            }
        }
        assert!(ore_count > 0, "no ore was generated at 4x richness");
    }

    #[test]
    fn caves_actually_carve_space_underground() {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let g = gen();
        let mut air_below_surface = 0;
        for cx in 0..4 {
            let c = g.generate_chunk(ChunkPos::new(cx, 0), &lookup);
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    let surface = c.height_at(lx, lz);
                    for y in 8..(surface - 6).max(9) {
                        if c.get(lx, y, lz).is_air() {
                            air_below_surface += 1;
                        }
                    }
                }
            }
        }
        assert!(air_below_surface > 200, "cave systems did not generate (only {air_below_surface} air voxels)");
    }

    #[test]
    fn flat_world_is_actually_flat() {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let mut p = GenParams::default();
        p.flat_world = true;
        p.cave_density = 0.0;
        let g = WorldGenerator::new(5, p);
        let c = g.generate_chunk(ChunkPos::new(1, 1), &lookup);
        for lz in 0..CHUNK_SZ {
            for lx in 0..CHUNK_SX {
                assert_eq!(c.height_at(lx, lz), SEA_LEVEL + 4);
            }
        }
    }

    #[test]
    fn ore_rules_have_sane_ranges() {
        for r in default_ore_rules() {
            assert!(r.min_y >= 3, "ore may not spawn inside bedrock");
            assert!(r.max_y < CHUNK_H);
            assert!(r.min_y < r.max_y);
            assert!((r.min_y..=r.max_y).contains(&r.peak_y), "peak outside range for {:?}", r.block);
            assert!(r.weight > 0.0);
            assert!(r.size.0 > 0.0 && r.size.1 >= r.size.0);
        }
    }
}
