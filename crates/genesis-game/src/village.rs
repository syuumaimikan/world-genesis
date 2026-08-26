//! 集落の手続き的生成。
//!
//! 村は「置かれる」のではなく「成立する」。世界を 128×128 ブロックの地域に区切り、
//! 各地域についてバイオームの居住適性・地形の平坦さ・水の近さ・鉱脈の有無から
//! 立地スコアを計算する。スコアが閾値を超えた地域にだけ集落が生まれ、
//! そのスコアの大きさが野営地・村・町・都市という規模を決める。

use crate::biome::{biome_def, Biome};
use crate::blocks::{ids, BlockId};
use crate::chunk::{ChunkData, CHUNK_SX, CHUNK_SZ};
use crate::noise::{hash2i, hash_u64};
use crate::worldgen::WorldGenerator;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 1地域の一辺（ブロック）。
pub const REGION_SIZE: i32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettlementTier {
    Camp,
    Hamlet,
    Village,
    Town,
    City,
}

impl SettlementTier {
    pub fn display_name(self) -> &'static str {
        match self {
            SettlementTier::Camp => "野営地",
            SettlementTier::Hamlet => "集落",
            SettlementTier::Village => "村",
            SettlementTier::Town => "町",
            SettlementTier::City => "都市",
        }
    }

    pub fn building_count(self) -> i32 {
        match self {
            SettlementTier::Camp => 3,
            SettlementTier::Hamlet => 6,
            SettlementTier::Village => 11,
            SettlementTier::Town => 19,
            SettlementTier::City => 30,
        }
    }

    pub fn radius(self) -> i32 {
        match self {
            SettlementTier::Camp => 14,
            SettlementTier::Hamlet => 22,
            SettlementTier::Village => 32,
            SettlementTier::Town => 44,
            SettlementTier::City => 58,
        }
    }

    pub fn base_population(self) -> u32 {
        match self {
            SettlementTier::Camp => 12,
            SettlementTier::Hamlet => 45,
            SettlementTier::Village => 160,
            SettlementTier::Town => 900,
            SettlementTier::City => 4200,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildingKind {
    House,
    LargeHouse,
    Farmhouse,
    Smithy,
    Bakery,
    Market,
    Tavern,
    Temple,
    Watchtower,
    TownHall,
    Well,
    Granary,
    Barn,
    Mine,
    Dock,
}

impl BuildingKind {
    pub fn display_name(self) -> &'static str {
        match self {
            BuildingKind::House => "民家",
            BuildingKind::LargeHouse => "大きな民家",
            BuildingKind::Farmhouse => "農家",
            BuildingKind::Smithy => "鍛冶屋",
            BuildingKind::Bakery => "パン屋",
            BuildingKind::Market => "市場",
            BuildingKind::Tavern => "酒場",
            BuildingKind::Temple => "神殿",
            BuildingKind::Watchtower => "見張り塔",
            BuildingKind::TownHall => "庁舎",
            BuildingKind::Well => "井戸",
            BuildingKind::Granary => "穀物庫",
            BuildingKind::Barn => "納屋",
            BuildingKind::Mine => "坑道入口",
            BuildingKind::Dock => "船着き場",
        }
    }

    /// この建物に常駐する住人の職業。
    pub fn resident_profession(self) -> Option<&'static str> {
        match self {
            BuildingKind::House => Some("農民"),
            BuildingKind::LargeHouse => Some("商人"),
            BuildingKind::Farmhouse => Some("農家"),
            BuildingKind::Smithy => Some("鍛冶屋"),
            BuildingKind::Bakery => Some("パン職人"),
            BuildingKind::Market => Some("行商人"),
            BuildingKind::Tavern => Some("酒場の主人"),
            BuildingKind::Temple => Some("聖職者"),
            BuildingKind::Watchtower => Some("衛兵"),
            BuildingKind::TownHall => Some("代官"),
            BuildingKind::Granary => Some("穀物商"),
            BuildingKind::Barn => Some("牧夫"),
            BuildingKind::Mine => Some("鉱夫"),
            BuildingKind::Dock => Some("漁師"),
            BuildingKind::Well => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Building {
    pub kind: BuildingKind,
    /// 建物の footprint の最小角（ワールドブロック座標）。
    pub x: i32,
    pub z: i32,
    pub w: i32,
    pub d: i32,
    pub height: i32,
    /// 整地後の床の高さ。
    pub floor_y: i32,
    /// 玄関の向き 0=+Z 1=-Z 2=+X 3=-X
    pub facing: u8,
}

impl Building {
    #[inline]
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.z + self.d / 2)
    }
}

#[derive(Debug, Clone)]
pub struct VillagePlan {
    pub id: u64,
    pub name: String,
    pub center_x: i32,
    pub center_z: i32,
    pub ground_y: i32,
    pub tier: SettlementTier,
    pub biome: Biome,
    pub population: u32,
    pub buildings: Vec<Building>,
    /// 道路の中心線分 (x1,z1,x2,z2)。
    pub roads: Vec<(i32, i32, i32, i32)>,
    /// 農地区画 (x,z,w,d)。
    pub farms: Vec<(i32, i32, i32, i32)>,
    /// 城壁を持つか（町以上）。
    pub walled: bool,
    /// 建材セット。
    pub palette: BuildPalette,
}

impl VillagePlan {
    /// 集落の外接範囲（ブロック座標）。
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let r = self.tier.radius() + 6;
        (
            self.center_x - r,
            self.center_z - r,
            self.center_x + r,
            self.center_z + r,
        )
    }

    /// NPC の初期配置候補（建物の玄関前）。
    pub fn npc_spawns(&self) -> Vec<(i32, i32, i32, &'static str)> {
        self.buildings
            .iter()
            .filter_map(|b| {
                let prof = b.kind.resident_profession()?;
                let (cx, cz) = b.center();
                let (dx, dz) = match b.facing {
                    0 => (0, b.d / 2 + 1),
                    1 => (0, -(b.d / 2 + 1)),
                    2 => (b.w / 2 + 1, 0),
                    _ => (-(b.w / 2 + 1), 0),
                };
                Some((cx + dx, b.floor_y + 1, cz + dz, prof))
            })
            .collect()
    }
}

/// バイオームに応じた建材の組。
#[derive(Debug, Clone, Copy)]
pub struct BuildPalette {
    pub wall: BlockId,
    pub accent: BlockId,
    pub frame: BlockId,
    pub roof: BlockId,
    pub floor: BlockId,
    pub foundation: BlockId,
    pub road: BlockId,
}

fn palette_for(biome: Biome) -> BuildPalette {
    use ids::*;
    match biome {
        Biome::Desert | Biome::RedDesert | Biome::Badlands => BuildPalette {
            wall: SANDSTONE, accent: PLASTER, frame: SANDSTONE, roof: PLASTER,
            floor: SANDSTONE, foundation: SANDSTONE, road: SANDSTONE,
        },
        Biome::Taiga | Biome::SnowyTaiga | Biome::SnowyPlains | Biome::Tundra => BuildPalette {
            wall: SPRUCE_PLANKS, accent: COBBLESTONE, frame: SPRUCE_LOG, roof: SPRUCE_PLANKS,
            floor: SPRUCE_PLANKS, foundation: COBBLESTONE, road: PATH,
        },
        Biome::RockyMountains | Biome::SnowyPeaks | Biome::Highlands | Biome::StonyShore => BuildPalette {
            wall: COBBLESTONE, accent: STONE_BRICK, frame: SPRUCE_LOG, roof: ROOF_TILE,
            floor: STONE_BRICK, foundation: STONE_BRICK, road: COBBLESTONE,
        },
        Biome::Savanna => BuildPalette {
            wall: ACACIA_LOG, accent: PLASTER, frame: ACACIA_LOG, roof: THATCH,
            floor: OAK_PLANKS, foundation: COBBLESTONE, road: PATH,
        },
        Biome::Jungle | Biome::BambooJungle | Biome::Mangrove | Biome::Swamp => BuildPalette {
            wall: JUNGLE_LOG, accent: OAK_PLANKS, frame: JUNGLE_LOG, roof: THATCH,
            floor: OAK_PLANKS, foundation: COBBLESTONE, road: PATH,
        },
        Biome::Volcanic => BuildPalette {
            wall: BASALT, accent: TUFF, frame: BASALT, roof: ROOF_TILE,
            floor: STONE_BRICK, foundation: BASALT, road: BASALT,
        },
        _ => BuildPalette {
            wall: PLASTER, accent: OAK_PLANKS, frame: OAK_LOG, roof: ROOF_TILE,
            floor: OAK_PLANKS, foundation: COBBLESTONE, road: PATH,
        },
    }
}

const NAME_PREFIX: [&str; 24] = [
    "Elden", "Riven", "Ash", "Bright", "Gold", "Stone", "Iron", "Fern", "North", "Aber",
    "Dun", "Green", "Mill", "Ravens", "White", "Black", "Ember", "Frost", "Wolf", "Silver",
    "Thorn", "Sun", "Moor", "Har",
];
const NAME_SUFFIX: [&str; 16] = [
    "dale", "ford", "bury", "wick", "holm", "gate", "field", "brook", "haven", "watch",
    "crest", "marsh", "reach", "hollow", "stead", "moor",
];

fn settlement_name(id: u64) -> String {
    let a = NAME_PREFIX[(id % NAME_PREFIX.len() as u64) as usize];
    let b = NAME_SUFFIX[((id >> 12) % NAME_SUFFIX.len() as u64) as usize];
    format!("{a}{b}")
}

pub struct VillagePlanner {
    seed: u64,
    density: f32,
    cache: RwLock<HashMap<(i32, i32), Arc<Option<VillagePlan>>>>,
}

impl VillagePlanner {
    pub fn new(seed: u64, density: f32) -> Self {
        Self {
            seed: seed ^ 0x5E77_1E11,
            density: density.clamp(0.0, 4.0),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// 地域 (rx, rz) の集落計画。存在しなければ None。結果はキャッシュされる。
    pub fn plan_for_region(&self, gen: &WorldGenerator, rx: i32, rz: i32) -> Arc<Option<VillagePlan>> {
        if let Some(p) = self.cache.read().get(&(rx, rz)) {
            return p.clone();
        }
        let plan = Arc::new(self.build_plan(gen, rx, rz));
        self.cache.write().insert((rx, rz), plan.clone());
        plan
    }

    /// 立地判定と実際の街割り。
    fn build_plan(&self, gen: &WorldGenerator, rx: i32, rz: i32) -> Option<VillagePlan> {
        if self.density <= 0.0 {
            return None;
        }
        let h = hash2i(rx, rz, self.seed);

        // 地域内で中心をずらし、格子状に並ばないようにする。
        let jitter_x = (h % (REGION_SIZE as u64 - 48)) as i32 + 24;
        let jitter_z = ((h >> 20) % (REGION_SIZE as u64 - 48)) as i32 + 24;
        let cx = rx * REGION_SIZE + jitter_x;
        let cz = rz * REGION_SIZE + jitter_z;

        let sea = gen.params.sea_level;
        let center_y = gen.terrain_height(cx as f32, cz as f32);
        if center_y <= sea + 1 {
            return None; // 水没地・海岸線ぎりぎりには作らない
        }

        let biome = gen.biome_at(cx as f32, cz as f32);
        let bdef = biome_def(biome);
        if bdef.habitability < 0.15 {
            return None;
        }

        // --- 立地スコア ---
        // 1) 地形の平坦さ：周囲16点の標高分散が小さいほど良い。
        let mut min_h = center_y;
        let mut max_h = center_y;
        let mut near_water = false;
        for (dx, dz) in [
            (-18, -18), (0, -18), (18, -18), (-18, 0), (18, 0), (-18, 18), (0, 18), (18, 18),
            (-34, 0), (34, 0), (0, -34), (0, 34), (-24, 24), (24, -24), (24, 24), (-24, -24),
        ] {
            let hh = gen.terrain_height((cx + dx) as f32, (cz + dz) as f32);
            min_h = min_h.min(hh);
            max_h = max_h.max(hh);
            if hh <= sea {
                near_water = true;
            }
        }
        let relief = (max_h - min_h) as f32;
        let flatness = (1.0 - relief / 26.0).clamp(0.0, 1.0);

        // 2) 居住適性 3) 水利 4) 地域固有の運
        let luck = ((h >> 40) % 1000) as f32 / 1000.0;
        let water_bonus = if near_water { 0.18 } else { 0.0 };
        let score = bdef.habitability * 0.45 + flatness * 0.40 + water_bonus + luck * 0.22;

        // 密度倍率は閾値を上下させる形で効かせる。
        let threshold = 0.62 / self.density.max(0.05);
        if score < threshold {
            return None;
        }

        let tier = if score > threshold + 0.30 {
            SettlementTier::City
        } else if score > threshold + 0.21 {
            SettlementTier::Town
        } else if score > threshold + 0.12 {
            SettlementTier::Village
        } else if score > threshold + 0.05 {
            SettlementTier::Hamlet
        } else {
            SettlementTier::Camp
        };

        let id = hash_u64(h ^ 0xABCD_1234);
        let palette = palette_for(biome);
        let mut plan = VillagePlan {
            id,
            name: settlement_name(id),
            center_x: cx,
            center_z: cz,
            ground_y: center_y,
            tier,
            biome,
            population: tier.base_population() + ((h >> 32) % 40) as u32,
            buildings: Vec::new(),
            roads: Vec::new(),
            farms: Vec::new(),
            walled: matches!(tier, SettlementTier::Town | SettlementTier::City),
            palette,
        };

        self.lay_out(gen, &mut plan, h);
        Some(plan)
    }

    /// 街割り：中央広場から放射する道と、その両脇に並ぶ建物。
    fn lay_out(&self, gen: &WorldGenerator, plan: &mut VillagePlan, h: u64) {
        let cx = plan.center_x;
        let cz = plan.center_z;
        let radius = plan.tier.radius();
        let count = plan.tier.building_count();

        // 中央に広場の核を置く。町以上は庁舎、それ未満は井戸。
        let core_kind = if plan.tier >= SettlementTier::Town {
            BuildingKind::TownHall
        } else {
            BuildingKind::Well
        };
        let core = self.make_building(gen, core_kind, cx - 2, cz - 2, 0);
        let (core_x, core_w, core_z) = (core.x, core.w, core.z);
        plan.buildings.push(core);

        // 村以上には共同井戸を置く。核の footprint の外側へずらして重なりを防ぐ。
        if plan.tier >= SettlementTier::Village && core_kind != BuildingKind::Well {
            plan.buildings
                .push(self.make_building(gen, BuildingKind::Well, core_x + core_w + 3, core_z, 0));
        }

        // 主要道路：東西南北 + 都市なら斜め。
        let arms: &[(i32, i32)] = if plan.tier >= SettlementTier::Town {
            &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)]
        } else {
            &[(1, 0), (-1, 0), (0, 1), (0, -1)]
        };
        for (ax, az) in arms {
            plan.roads.push((cx, cz, cx + ax * radius, cz + az * radius));
        }

        // 建物を道沿いへ並べる。角度と距離を決定的に振り分ける。
        let mut placed: Vec<(i32, i32, i32, i32)> = plan
            .buildings
            .iter()
            .map(|b| (b.x, b.z, b.w, b.d))
            .collect();

        let mut attempt = 0u64;
        while (plan.buildings.len() as i32) < count && attempt < 400 {
            let seed = hash_u64(h ^ attempt.wrapping_mul(0x9E37_79B9));
            attempt += 1;

            let arm = arms[(seed % arms.len() as u64) as usize];
            let along = 8 + ((seed >> 8) % (radius as u64 - 6)) as i32;
            let side = ((seed >> 16) % 2) as i32 * 2 - 1;
            let offset = 5 + ((seed >> 24) % 5) as i32;

            // 道に対して直交方向へずらす。
            let (px, pz) = (
                cx + arm.0 * along + (-arm.1) * side * offset,
                cz + arm.1 * along + arm.0 * side * offset,
            );

            let kind = self.pick_building_kind(plan, seed);
            let (w, d) = building_footprint(kind, seed);
            let bx = px - w / 2;
            let bz = pz - d / 2;

            // 重なりを避ける。
            let overlaps = placed.iter().any(|&(ox, oz, ow, od)| {
                bx < ox + ow + 2 && ox < bx + w + 2 && bz < oz + od + 2 && oz < bz + d + 2
            });
            if overlaps {
                continue;
            }

            // 急斜面には建てない。
            let corners = [
                gen.terrain_height(bx as f32, bz as f32),
                gen.terrain_height((bx + w) as f32, bz as f32),
                gen.terrain_height(bx as f32, (bz + d) as f32),
                gen.terrain_height((bx + w) as f32, (bz + d) as f32),
            ];
            let lo = corners.iter().copied().min().unwrap_or(0);
            let hi = corners.iter().copied().max().unwrap_or(0);
            if hi - lo > 6 || lo <= gen.params.sea_level {
                continue;
            }

            // 玄関は中央広場を向く。
            let facing = if arm.0 > 0 {
                3
            } else if arm.0 < 0 {
                2
            } else if arm.1 > 0 {
                1
            } else {
                0
            };

            placed.push((bx, bz, w, d));
            plan.buildings.push(self.make_building_sized(gen, kind, bx, bz, w, d, facing));
        }

        // 農地：集落の外縁に配置する。
        let farm_count = match plan.tier {
            SettlementTier::Camp => 1,
            SettlementTier::Hamlet => 2,
            SettlementTier::Village => 4,
            SettlementTier::Town => 6,
            SettlementTier::City => 9,
        };
        for f in 0..farm_count {
            let seed = hash_u64(h ^ (0xF00D_0000 + f as u64));
            let ang = (seed % 360) as f32 * std::f32::consts::PI / 180.0;
            let dist = (radius as f32 * 0.72) + ((seed >> 12) % 20) as f32;
            let fx = cx + (ang.cos() * dist) as i32;
            let fz = cz + (ang.sin() * dist) as i32;
            let fw = 7 + ((seed >> 20) % 6) as i32;
            let fd = 7 + ((seed >> 26) % 6) as i32;
            // 水没する農地は作らない。
            if gen.terrain_height(fx as f32, fz as f32) <= gen.params.sea_level {
                continue;
            }
            plan.farms.push((fx - fw / 2, fz - fd / 2, fw, fd));
        }
    }

    fn pick_building_kind(&self, plan: &VillagePlan, seed: u64) -> BuildingKind {
        use BuildingKind::*;
        let has = |k: BuildingKind| plan.buildings.iter().any(|b| b.kind == k);

        // 生活に必要な施設を優先的に一つずつ揃える。
        if plan.tier >= SettlementTier::Hamlet && !has(Farmhouse) {
            return Farmhouse;
        }
        if plan.tier >= SettlementTier::Village && !has(Smithy) {
            return Smithy;
        }
        if plan.tier >= SettlementTier::Village && !has(Granary) {
            return Granary;
        }
        if plan.tier >= SettlementTier::Village && !has(Bakery) {
            return Bakery;
        }
        if plan.tier >= SettlementTier::Town && !has(Market) {
            return Market;
        }
        if plan.tier >= SettlementTier::Town && !has(Tavern) {
            return Tavern;
        }
        if plan.tier >= SettlementTier::Town && !has(Temple) {
            return Temple;
        }
        if plan.walled && !has(Watchtower) {
            return Watchtower;
        }
        if plan.biome == Biome::RockyMountains && !has(Mine) {
            return Mine;
        }

        match seed % 10 {
            0 | 1 => LargeHouse,
            2 => Barn,
            3 => Farmhouse,
            _ => House,
        }
    }

    fn make_building(&self, gen: &WorldGenerator, kind: BuildingKind, x: i32, z: i32, facing: u8) -> Building {
        let (w, d) = building_footprint(kind, 0);
        self.make_building_sized(gen, kind, x, z, w, d, facing)
    }

    #[allow(clippy::too_many_arguments)]
    fn make_building_sized(
        &self,
        gen: &WorldGenerator,
        kind: BuildingKind,
        x: i32,
        z: i32,
        w: i32,
        d: i32,
        facing: u8,
    ) -> Building {
        // 4隅の平均で床の高さを決める（整地の基準）。
        let hs = [
            gen.terrain_height(x as f32, z as f32),
            gen.terrain_height((x + w - 1) as f32, z as f32),
            gen.terrain_height(x as f32, (z + d - 1) as f32),
            gen.terrain_height((x + w - 1) as f32, (z + d - 1) as f32),
        ];
        let floor_y = hs.iter().sum::<i32>() / 4;
        Building {
            kind,
            x,
            z,
            w,
            d,
            height: building_height(kind),
            floor_y,
            facing,
        }
    }

    /// チャンクへ、そこに重なる集落構造物を書き込む。
    pub fn stamp_into_chunk(&self, gen: &WorldGenerator, chunk: &mut ChunkData) {
        if self.density <= 0.0 {
            return;
        }
        let (ox, oz) = chunk.pos.origin();
        let rx = ox.div_euclid(REGION_SIZE);
        let rz = oz.div_euclid(REGION_SIZE);

        for drz in -1..=1 {
            for drx in -1..=1 {
                let plan = self.plan_for_region(gen, rx + drx, rz + drz);
                let Some(plan) = plan.as_ref() else { continue };

                let (bx0, bz0, bx1, bz1) = plan.bounds();
                // チャンクと集落の外接矩形が交差しないなら何もしない。
                if bx1 < ox || bx0 > ox + CHUNK_SX || bz1 < oz || bz0 > oz + CHUNK_SZ {
                    continue;
                }
                stamp_plan(plan, chunk);
            }
        }
    }

    /// 指定チャンク内に中心を持つ集落（NPC スポーン処理用）。
    pub fn plan_centered_in_chunk(&self, gen: &WorldGenerator, cx: i32, cz: i32) -> Option<VillagePlan> {
        let ox = cx * CHUNK_SX;
        let oz = cz * CHUNK_SZ;
        let rx = ox.div_euclid(REGION_SIZE);
        let rz = oz.div_euclid(REGION_SIZE);
        for drz in -1..=1 {
            for drx in -1..=1 {
                let plan = self.plan_for_region(gen, rx + drx, rz + drz);
                if let Some(p) = plan.as_ref() {
                    if p.center_x >= ox && p.center_x < ox + CHUNK_SX && p.center_z >= oz && p.center_z < oz + CHUNK_SZ {
                        return Some(p.clone());
                    }
                }
            }
        }
        None
    }

    /// 与えた地点の周辺にある全集落（世界地図・HUD 用）。
    pub fn plans_around(&self, gen: &WorldGenerator, wx: i32, wz: i32, region_radius: i32) -> Vec<VillagePlan> {
        let rx = wx.div_euclid(REGION_SIZE);
        let rz = wz.div_euclid(REGION_SIZE);
        let mut out = Vec::new();
        for drz in -region_radius..=region_radius {
            for drx in -region_radius..=region_radius {
                if let Some(p) = self.plan_for_region(gen, rx + drx, rz + drz).as_ref() {
                    out.push(p.clone());
                }
            }
        }
        out
    }
}

fn building_footprint(kind: BuildingKind, seed: u64) -> (i32, i32) {
    let v = (seed % 3) as i32;
    match kind {
        BuildingKind::Well => (5, 5),
        BuildingKind::House => (6 + v, 6 + ((seed >> 4) % 3) as i32),
        BuildingKind::LargeHouse => (9 + v, 8 + v),
        BuildingKind::Farmhouse => (8, 7 + v),
        BuildingKind::Smithy => (8, 7),
        BuildingKind::Bakery => (7, 7),
        BuildingKind::Market => (11, 9),
        BuildingKind::Tavern => (10, 9),
        BuildingKind::Temple => (11, 13),
        BuildingKind::Watchtower => (6, 6),
        BuildingKind::TownHall => (13, 11),
        BuildingKind::Granary => (7, 9),
        BuildingKind::Barn => (10, 8),
        BuildingKind::Mine => (7, 7),
        BuildingKind::Dock => (9, 6),
    }
}

fn building_height(kind: BuildingKind) -> i32 {
    match kind {
        BuildingKind::Well => 2,
        BuildingKind::Watchtower => 13,
        BuildingKind::Temple => 8,
        BuildingKind::TownHall => 7,
        BuildingKind::LargeHouse | BuildingKind::Tavern | BuildingKind::Market => 6,
        BuildingKind::Barn | BuildingKind::Granary => 6,
        _ => 4,
    }
}

// ----------------------------------------------------------------------
// 実際のボクセル書き込み
// ----------------------------------------------------------------------

/// チャンクローカルへ変換して書き込む。範囲外は `ChunkData::set` が捨てる。
#[inline]
fn put(chunk: &mut ChunkData, wx: i32, y: i32, wz: i32, b: BlockId) {
    let (ox, oz) = chunk.pos.origin();
    let lx = wx - ox;
    let lz = wz - oz;
    if (0..CHUNK_SX).contains(&lx) && (0..CHUNK_SZ).contains(&lz) {
        chunk.set(lx, y, lz, b);
    }
}

/// 建物の下を整地する：床より上を空け、床より下を基礎で埋める。
fn level_ground(chunk: &mut ChunkData, x0: i32, z0: i32, x1: i32, z1: i32, floor_y: i32, foundation: BlockId) {
    let (ox, oz) = chunk.pos.origin();
    for wz in z0..z1 {
        for wx in x0..x1 {
            let (lx, lz) = (wx - ox, wz - oz);
            if !(0..CHUNK_SX).contains(&lx) || !(0..CHUNK_SZ).contains(&lz) {
                continue;
            }
            // 上を掘る。
            for y in floor_y + 1..floor_y + 16 {
                if !chunk.get(lx, y, lz).is_air() {
                    chunk.set(lx, y, lz, ids::AIR);
                }
            }
            // 下を埋める（宙に浮かせない）。
            for y in (floor_y - 12).max(3)..=floor_y {
                let cur = chunk.get(lx, y, lz);
                if cur.is_air() || cur == ids::WATER {
                    chunk.set(lx, y, lz, foundation);
                }
            }
            // 整地した以上、この列の地面は床面そのものになる。高さマップは
            // NPC・プレイヤーの接地判定に使われるため、ここで追従させる。
            chunk.height_map[(lz * CHUNK_SX + lx) as usize] = floor_y as i16;
        }
    }
}

fn stamp_plan(plan: &VillagePlan, chunk: &mut ChunkData) {
    let p = plan.palette;

    // --- 道路 ---
    for &(x1, z1, x2, z2) in &plan.roads {
        stamp_road(chunk, x1, z1, x2, z2, p.road, plan);
    }

    // --- 農地 ---
    for &(fx, fz, fw, fd) in &plan.farms {
        stamp_farm(chunk, fx, fz, fw, fd, plan);
    }

    // --- 城壁 ---
    if plan.walled {
        stamp_wall(chunk, plan);
    }

    // --- 建物 ---
    for b in &plan.buildings {
        // チャンクと交差しなければ捨てる（無駄な整地を避ける）。
        let (ox, oz) = chunk.pos.origin();
        if b.x + b.w < ox || b.x > ox + CHUNK_SX || b.z + b.d < oz || b.z > oz + CHUNK_SZ {
            continue;
        }
        match b.kind {
            BuildingKind::Well => stamp_well(chunk, b, &p),
            BuildingKind::Watchtower => stamp_tower(chunk, b, &p),
            _ => stamp_house(chunk, b, &p, plan),
        }
    }
}

fn stamp_road(chunk: &mut ChunkData, x1: i32, z1: i32, x2: i32, z2: i32, road: BlockId, plan: &VillagePlan) {
    let steps = (x2 - x1).abs().max((z2 - z1).abs()).max(1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = x1 + ((x2 - x1) as f32 * t).round() as i32;
        let z = z1 + ((z2 - z1) as f32 * t).round() as i32;
        // 道は中心から離れるほど細くする。
        let width = if t < 0.55 { 2 } else { 1 };
        for dz in -width..=width {
            for dx in -width..=width {
                let (wx, wz) = (x + dx, z + dz);
                let (ox, oz) = chunk.pos.origin();
                let (lx, lz) = (wx - ox, wz - oz);
                if !(0..CHUNK_SX).contains(&lx) || !(0..CHUNK_SZ).contains(&lz) {
                    continue;
                }
                let surface = chunk.height_at(lx, lz);
                // 集落中心付近の起伏は道の高さへ寄せる。
                if (surface - plan.ground_y).abs() > 10 {
                    continue;
                }
                // 地表を舗装し、その上の草木を除去する。
                if !chunk.get(lx, surface, lz).is_air() {
                    chunk.set(lx, surface, lz, road);
                }
                for y in surface + 1..surface + 4 {
                    let cur = chunk.get(lx, y, lz);
                    if cur.is_air() {
                        break;
                    }
                    // 木の幹は残さない（道が塞がるため）。
                    chunk.set(lx, y, lz, ids::AIR);
                }
            }
        }
    }
}

fn stamp_farm(chunk: &mut ChunkData, fx: i32, fz: i32, fw: i32, fd: i32, plan: &VillagePlan) {
    let (ox, oz) = chunk.pos.origin();
    if fx + fw < ox || fx > ox + CHUNK_SX || fz + fd < oz || fz > oz + CHUNK_SZ {
        return;
    }
    for wz in fz..fz + fd {
        for wx in fx..fx + fw {
            let (lx, lz) = (wx - ox, wz - oz);
            if !(0..CHUNK_SX).contains(&lx) || !(0..CHUNK_SZ).contains(&lz) {
                continue;
            }
            let surface = chunk.height_at(lx, lz);
            if (surface - plan.ground_y).abs() > 8 {
                continue;
            }
            // 畝の間に灌漑用の水路を通す。
            let is_channel = (wx - fx) % 5 == 2;
            for y in surface + 1..surface + 5 {
                if chunk.get(lx, y, lz).is_air() {
                    break;
                }
                chunk.set(lx, y, lz, ids::AIR);
            }
            if is_channel {
                chunk.set(lx, surface, lz, ids::WATER);
            } else {
                chunk.set(lx, surface, lz, ids::FARMLAND);
                chunk.set(lx, surface + 1, lz, ids::WHEAT_CROP);
            }
        }
    }
}

fn stamp_wall(chunk: &mut ChunkData, plan: &VillagePlan) {
    let r = plan.tier.radius();
    let (cx, cz) = (plan.center_x, plan.center_z);
    let (ox, oz) = chunk.pos.origin();
    let wall = plan.palette.foundation;
    let height = if plan.tier == SettlementTier::City { 6 } else { 4 };

    for lz in 0..CHUNK_SZ {
        for lx in 0..CHUNK_SX {
            let wx = ox + lx;
            let wz = oz + lz;
            let dx = wx - cx;
            let dz = wz - cz;
            let dist = ((dx * dx + dz * dz) as f32).sqrt();
            if (dist - r as f32).abs() > 0.75 {
                continue;
            }
            // 門：主要道路が壁を貫くところは開ける。
            if dx.abs() < 3 || dz.abs() < 3 {
                continue;
            }
            let base = chunk.height_at(lx, lz);
            if base <= 4 {
                continue;
            }
            for y in base..base + height {
                chunk.set(lx, y, lz, wall);
            }
            // 胸壁（狭間）。
            if (wx + wz) % 2 == 0 {
                chunk.set(lx, base + height, lz, wall);
            }
        }
    }
}

fn stamp_well(chunk: &mut ChunkData, b: &Building, p: &BuildPalette) {
    let y = b.floor_y;
    level_ground(chunk, b.x, b.z, b.x + b.w, b.z + b.d, y, p.foundation);
    for wz in b.z..b.z + b.d {
        for wx in b.x..b.x + b.w {
            let edge = wx == b.x || wx == b.x + b.w - 1 || wz == b.z || wz == b.z + b.d - 1;
            if edge {
                put(chunk, wx, y, wz, ids::COBBLESTONE);
            } else {
                put(chunk, wx, y, wz, ids::WELL_STONE);
                put(chunk, wx, y + 1, wz, ids::WELL_STONE);
                // 中央は水を湛える。
                if wx == b.x + b.w / 2 && wz == b.z + b.d / 2 {
                    for dy in 0..5 {
                        put(chunk, wx, y - dy, wz, ids::WATER);
                    }
                    put(chunk, wx, y + 1, wz, ids::WATER);
                }
            }
        }
    }
    // 屋根を支える4本の柱と傘。
    for (dx, dz) in [(1i32, 1i32), (b.w - 2, 1), (1, b.d - 2), (b.w - 2, b.d - 2)] {
        for dy in 2..5 {
            put(chunk, b.x + dx, y + dy, b.z + dz, p.frame);
        }
    }
    for wz in b.z..b.z + b.d {
        for wx in b.x..b.x + b.w {
            put(chunk, wx, y + 5, wz, p.roof);
        }
    }
}

fn stamp_tower(chunk: &mut ChunkData, b: &Building, p: &BuildPalette) {
    let y = b.floor_y;
    level_ground(chunk, b.x, b.z, b.x + b.w, b.z + b.d, y, p.foundation);
    for dy in 0..b.height {
        for wz in b.z..b.z + b.d {
            for wx in b.x..b.x + b.w {
                let edge = wx == b.x || wx == b.x + b.w - 1 || wz == b.z || wz == b.z + b.d - 1;
                if dy == 0 {
                    put(chunk, wx, y, wz, p.floor);
                } else if edge {
                    // 最上部は狭間にする。
                    if dy == b.height - 1 && (wx + wz) % 2 == 0 {
                        put(chunk, wx, y + dy, wz, ids::AIR);
                    } else {
                        put(chunk, wx, y + dy, wz, p.wall);
                    }
                } else {
                    put(chunk, wx, y + dy, wz, ids::AIR);
                }
            }
        }
    }
    // 見張り台の床と灯り。
    let (cx, cz) = b.center();
    for wz in b.z + 1..b.z + b.d - 1 {
        for wx in b.x + 1..b.x + b.w - 1 {
            put(chunk, wx, y + b.height - 3, wz, p.floor);
        }
    }
    put(chunk, cx, y + b.height - 2, cz, ids::LANTERN);
    // 出入口。
    put(chunk, b.x + b.w / 2, y + 1, b.z, ids::AIR);
    put(chunk, b.x + b.w / 2, y + 2, b.z, ids::AIR);
}

fn stamp_house(chunk: &mut ChunkData, b: &Building, p: &BuildPalette, plan: &VillagePlan) {
    let y = b.floor_y;
    level_ground(chunk, b.x - 1, b.z - 1, b.x + b.w + 1, b.z + b.d + 1, y, p.foundation);

    let wall_h = b.height;
    let (x0, z0) = (b.x, b.z);
    let (x1, z1) = (b.x + b.w - 1, b.z + b.d - 1);

    // 床。
    for wz in z0..=z1 {
        for wx in x0..=x1 {
            put(chunk, wx, y, wz, p.floor);
        }
    }

    // 壁と内部の空洞。
    for dy in 1..=wall_h {
        for wz in z0..=z1 {
            for wx in x0..=x1 {
                let corner = (wx == x0 || wx == x1) && (wz == z0 || wz == z1);
                let edge = wx == x0 || wx == x1 || wz == z0 || wz == z1;
                if corner {
                    put(chunk, wx, y + dy, wz, p.frame);
                } else if edge {
                    // 窓：壁の中ほどに等間隔で開ける。
                    let window_row = dy == wall_h - 2 || (wall_h >= 6 && dy == 2);
                    let window_col = (wx + wz) % 3 == 0;
                    if window_row && window_col {
                        put(chunk, wx, y + dy, wz, ids::GLASS);
                    } else if dy == wall_h {
                        put(chunk, wx, y + dy, wz, p.frame);
                    } else {
                        put(chunk, wx, y + dy, wz, p.wall);
                    }
                } else {
                    put(chunk, wx, y + dy, wz, ids::AIR);
                }
            }
        }
    }

    // 玄関。
    let (dx, dz) = match b.facing {
        0 => (b.w / 2, b.d - 1),
        1 => (b.w / 2, 0),
        2 => (b.w - 1, b.d / 2),
        _ => (0, b.d / 2),
    };
    put(chunk, x0 + dx, y + 1, z0 + dz, ids::DOOR);
    put(chunk, x0 + dx, y + 2, z0 + dz, ids::AIR);

    // 屋根：切妻。棟に向かって段状に狭める。
    let roof_span = (b.w.min(b.d) / 2).max(1);
    for step in 0..=roof_span {
        let ry = y + wall_h + 1 + step;
        for wz in (z0 - 1 + step)..=(z1 + 1 - step) {
            for wx in (x0 - 1 + step)..=(x1 + 1 - step) {
                let edge = wx == x0 - 1 + step
                    || wx == x1 + 1 - step
                    || wz == z0 - 1 + step
                    || wz == z1 + 1 - step;
                if edge || step == roof_span {
                    put(chunk, wx, ry, wz, p.roof);
                } else {
                    put(chunk, wx, ry, wz, ids::AIR);
                }
            }
        }
    }

    // 内装：明かりと、用途ごとの設備。
    let (cx, cz) = b.center();
    put(chunk, x0 + 1, y + wall_h - 1, z0 + 1, ids::TORCH);
    match b.kind {
        BuildingKind::Smithy => {
            put(chunk, cx, y + 1, cz, ids::CAMPFIRE);
            put(chunk, cx + 1, y + 1, cz, ids::IRON_ORE);
        }
        BuildingKind::Bakery => {
            put(chunk, cx, y + 1, cz, ids::CAMPFIRE);
            put(chunk, cx - 1, y + 1, cz, ids::THATCH);
        }
        BuildingKind::Granary => {
            for wz in z0 + 1..z1 {
                for wx in x0 + 1..x1 {
                    put(chunk, wx, y + 1, wz, ids::THATCH);
                }
            }
        }
        BuildingKind::Temple => {
            for wz in [z0 + 1, z1 - 1] {
                for wx in [x0 + 1, x1 - 1] {
                    for dy in 1..wall_h {
                        put(chunk, wx, y + dy, wz, ids::MARBLE_COLUMN);
                    }
                }
            }
            put(chunk, cx, y + 1, cz, ids::LANTERN);
        }
        BuildingKind::Market | BuildingKind::Tavern | BuildingKind::TownHall => {
            put(chunk, cx, y + 1, cz, ids::CAMPFIRE);
            put(chunk, cx, y + wall_h - 1, cz, ids::LANTERN);
        }
        BuildingKind::Mine => {
            // 坑道の入口：地下へ向かう縦坑。
            for dy in 1..14 {
                put(chunk, cx, y - dy, cz, ids::AIR);
                if dy % 4 == 0 {
                    put(chunk, cx + 1, y - dy, cz, ids::TORCH);
                }
            }
        }
        _ => {}
    }

    // 集落の格を示す装飾（都市の建物には外灯が立つ）。
    if plan.tier >= SettlementTier::Town {
        put(chunk, x0 - 1, y + 1, z0 - 1, ids::LANTERN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BlockRegistry;
    use crate::chunk::ChunkPos;
    use crate::worldgen::{GenParams, WorldGenerator};

    fn gen(density: f32) -> WorldGenerator {
        let mut p = GenParams::default();
        p.settlement_density = density;
        WorldGenerator::new(0x5EED_1234, p)
    }

    #[test]
    fn planning_is_deterministic_and_cached() {
        let g = gen(1.0);
        let a = g.villages.plan_for_region(&g, 2, 3);
        let b = g.villages.plan_for_region(&g, 2, 3);
        match (a.as_ref(), b.as_ref()) {
            (Some(x), Some(y)) => {
                assert_eq!(x.id, y.id);
                assert_eq!(x.buildings.len(), y.buildings.len());
                assert_eq!(x.center_x, y.center_x);
            }
            (None, None) => {}
            _ => panic!("planner returned inconsistent results for the same region"),
        }
    }

    #[test]
    fn settlements_actually_appear_across_the_world() {
        let g = gen(1.0);
        let mut found = 0;
        let mut tiers = Vec::new();
        for rz in -8..8 {
            for rx in -8..8 {
                if let Some(p) = g.villages.plan_for_region(&g, rx, rz).as_ref() {
                    found += 1;
                    tiers.push(p.tier);
                    assert!(!p.buildings.is_empty(), "{} has no buildings", p.name);
                    assert!(p.population > 0);
                }
            }
        }
        assert!(found >= 12, "only {found} settlements in 256 regions - the world is too empty");
        assert!(tiers.iter().any(|t| *t >= SettlementTier::Village), "no settlement grew past a hamlet");
    }

    #[test]
    fn settlements_are_never_placed_underwater() {
        let g = gen(2.0);
        for rz in -6..6 {
            for rx in -6..6 {
                if let Some(p) = g.villages.plan_for_region(&g, rx, rz).as_ref() {
                    assert!(p.ground_y > g.params.sea_level, "{} was founded below sea level", p.name);
                    for b in &p.buildings {
                        assert!(b.floor_y > g.params.sea_level - 2, "building {:?} is underwater", b.kind);
                    }
                }
            }
        }
    }

    #[test]
    fn buildings_never_overlap() {
        let g = gen(2.0);
        for rz in -4..4 {
            for rx in -4..4 {
                let Some(p) = g.villages.plan_for_region(&g, rx, rz).as_ref().clone() else { continue };
                for (i, a) in p.buildings.iter().enumerate() {
                    for b in &p.buildings[i + 1..] {
                        let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.z < b.z + b.d && b.z < a.z + a.d;
                        assert!(!overlap, "{:?} and {:?} overlap in {}", a.kind, b.kind, p.name);
                    }
                }
            }
        }
    }

    #[test]
    fn density_zero_produces_no_settlements() {
        let g = gen(0.0);
        for rz in -4..4 {
            for rx in -4..4 {
                assert!(g.villages.plan_for_region(&g, rx, rz).is_none());
            }
        }
    }

    #[test]
    fn higher_density_yields_more_settlements() {
        let count = |d: f32| {
            let g = gen(d);
            (-6..6)
                .flat_map(|rz| (-6..6).map(move |rx| (rx, rz)))
                .filter(|&(rx, rz)| g.villages.plan_for_region(&g, rx, rz).is_some())
                .count()
        };
        let low = count(0.6);
        let high = count(2.5);
        assert!(high > low, "density had no effect: {low} vs {high}");
    }

    #[test]
    fn stamped_village_leaves_buildings_in_the_chunk() {
        let reg = BlockRegistry::with_builtins();
        let lookup = reg.snapshot();
        let g = gen(2.0);
        // 実際に集落がある地域を探し、その中心チャンクを生成する。
        let mut checked = false;
        'outer: for rz in -6..6 {
            for rx in -6..6 {
                let plan = g.villages.plan_for_region(&g, rx, rz);
                let Some(p) = plan.as_ref() else { continue };
                let cpos = ChunkPos::from_world(p.center_x as f32, p.center_z as f32);
                let chunk = g.generate_chunk(cpos, &lookup);
                // 建材ブロックが1つでも書き込まれていること。
                let has_built = chunk.voxels.iter().any(|b| {
                    matches!(
                        *b,
                        ids::OAK_PLANKS | ids::SPRUCE_PLANKS | ids::COBBLESTONE | ids::STONE_BRICK
                            | ids::PLASTER | ids::ROOF_TILE | ids::THATCH | ids::WELL_STONE
                            | ids::SANDSTONE | ids::FARMLAND
                    )
                });
                assert!(has_built, "no construction materials found at the centre of {}", p.name);
                checked = true;
                break 'outer;
            }
        }
        assert!(checked, "no settlement was available to test stamping");
    }

    #[test]
    fn npc_spawn_points_sit_above_the_floor() {
        let g = gen(2.0);
        for rz in -4..4 {
            for rx in -4..4 {
                let Some(p) = g.villages.plan_for_region(&g, rx, rz).as_ref().clone() else { continue };
                let spawns = p.npc_spawns();
                for (_, y, _, prof) in &spawns {
                    assert!(*y > 4, "spawn for {prof} is inside bedrock");
                    assert!(!prof.is_empty());
                }
                if p.tier >= SettlementTier::Village {
                    assert!(!spawns.is_empty(), "{} has no residents", p.name);
                }
            }
        }
    }
}
