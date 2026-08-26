//! バイオーム定義テーブル。
//!
//! バイオームは手作業で塗り分けるのではなく、気候パラメータ
//! （気温・湿度・大陸度・侵食度・特異度）から分類される。これは現実の
//! ケッペン気候区分と同じ考え方で、緯度・標高・海からの距離が変われば
//! 自然に植生と地表が入れ替わる。

use crate::blocks::{ids, BlockId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {
    // --- 海洋 ---
    DeepOcean,
    Ocean,
    WarmShallows,
    FrozenOcean,
    Beach,
    StonyShore,
    // --- 温帯 ---
    Plains,
    Meadow,
    Forest,
    BirchForest,
    DarkForest,
    CherryGrove,
    // --- 寒帯 ---
    Taiga,
    SnowyTaiga,
    SnowyPlains,
    Tundra,
    IceSpikes,
    Glacier,
    // --- 乾燥帯 ---
    Savanna,
    Desert,
    RedDesert,
    Badlands,
    // --- 熱帯 ---
    Jungle,
    BambooJungle,
    Mangrove,
    Swamp,
    // --- 高地・特殊 ---
    Highlands,
    RockyMountains,
    SnowyPeaks,
    Volcanic,
    MushroomIsle,
}

pub const ALL_BIOMES: [Biome; 31] = [
    Biome::DeepOcean, Biome::Ocean, Biome::WarmShallows, Biome::FrozenOcean,
    Biome::Beach, Biome::StonyShore, Biome::Plains, Biome::Meadow, Biome::Forest,
    Biome::BirchForest, Biome::DarkForest, Biome::CherryGrove, Biome::Taiga,
    Biome::SnowyTaiga, Biome::SnowyPlains, Biome::Tundra, Biome::IceSpikes,
    Biome::Glacier, Biome::Savanna, Biome::Desert, Biome::RedDesert, Biome::Badlands,
    Biome::Jungle, Biome::BambooJungle, Biome::Mangrove, Biome::Swamp,
    Biome::Highlands, Biome::RockyMountains, Biome::SnowyPeaks, Biome::Volcanic,
    Biome::MushroomIsle,
];

/// 樹木の形状テンプレート。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeShape {
    None,
    /// 丸い樹冠（オーク・白樺・桜）
    Round,
    /// 円錐（トウヒ・モミ）
    Conifer,
    /// 巨大な多層樹冠（ジャングル）
    Giant,
    /// 傘状（アカシア）
    Umbrella,
    /// ヤシ（湾曲した幹＋放射状の葉）
    Palm,
    /// 支柱根つき（マングローブ）
    Mangrove,
    /// 枯木（枝のみ）
    Dead,
    /// サボテン柱
    Cactus,
    /// 竹林
    Bamboo,
    /// 巨大キノコ
    Mushroom,
}

#[derive(Debug, Clone)]
pub struct BiomeDef {
    pub biome: Biome,
    pub display_name: &'static str,
    /// 地表ブロック。
    pub surface: BlockId,
    /// 地表直下 (3〜4ブロック) の土壌。
    pub subsoil: BlockId,
    /// 深部の岩石。
    pub bedrock_stone: BlockId,
    /// 水面下に露出したときの地表。
    pub underwater: BlockId,
    /// 基準標高オフセット（海面基準ブロック数）。
    pub height_bias: f32,
    /// 地形の起伏の激しさ倍率。
    pub height_scale: f32,
    /// 樹木の生成確率（1ブロックあたり）。
    pub tree_density: f32,
    pub tree_shape: TreeShape,
    pub tree_log: BlockId,
    pub tree_leaves: BlockId,
    /// 下草の生成確率。
    pub grass_density: f32,
    pub grass_block: BlockId,
    /// 花の生成確率。
    pub flower_density: f32,
    /// このバイオームに咲く花の候補。
    pub flowers: &'static [BlockId],
    /// 集落が成立しうるか（居住適性 0.0〜1.0）。
    pub habitability: f32,
    /// 平均気温 (℃) — 体温シミュレーションと農業判定に使う。
    pub temperature_c: f32,
    /// 空の色（遠景フォグと同期）。
    pub sky_color: [f32; 3],
    /// 生息する動物種のキー。
    pub fauna: &'static [&'static str],
}

const TEMPERATE_FLOWERS: &[BlockId] = &[
    ids::FLOWER_RED, ids::FLOWER_YELLOW, ids::FLOWER_WHITE, ids::FLOWER_BLUE, ids::FLOWER_PURPLE,
];
const SPARSE_FLOWERS: &[BlockId] = &[ids::FLOWER_YELLOW, ids::FLOWER_WHITE];
const NO_FLOWERS: &[BlockId] = &[];
const FUNGI: &[BlockId] = &[ids::MUSHROOM_RED, ids::MUSHROOM_BROWN];

const SKY_TEMPERATE: [f32; 3] = [0.52, 0.72, 0.96];
const SKY_COLD: [f32; 3] = [0.66, 0.78, 0.92];
const SKY_ARID: [f32; 3] = [0.72, 0.78, 0.90];
const SKY_TROPIC: [f32; 3] = [0.46, 0.72, 0.92];
const SKY_SWAMP: [f32; 3] = [0.52, 0.62, 0.62];
const SKY_VOLCANIC: [f32; 3] = [0.46, 0.40, 0.40];

macro_rules! biome_def {
    ($b:expr, $name:expr, $surf:expr, $sub:expr, $stone:expr, $under:expr,
     $bias:expr, $scale:expr, $td:expr, $shape:expr, $log:expr, $leaf:expr,
     $gd:expr, $gblk:expr, $fd:expr, $fl:expr, $hab:expr, $temp:expr, $sky:expr, $fauna:expr) => {
        BiomeDef {
            biome: $b,
            display_name: $name,
            surface: $surf,
            subsoil: $sub,
            bedrock_stone: $stone,
            underwater: $under,
            height_bias: $bias,
            height_scale: $scale,
            tree_density: $td,
            tree_shape: $shape,
            tree_log: $log,
            tree_leaves: $leaf,
            grass_density: $gd,
            grass_block: $gblk,
            flower_density: $fd,
            flowers: $fl,
            habitability: $hab,
            temperature_c: $temp,
            sky_color: $sky,
            fauna: $fauna,
        }
    };
}

pub fn biome_def(b: Biome) -> BiomeDef {
    use ids::*;
    use Biome::*;
    use TreeShape as T;
    match b {
        DeepOcean => biome_def!(DeepOcean, "深海", SAND, GRAVEL, STONE, GRAVEL, -34.0, 0.35, 0.0, T::None, AIR, AIR,
            0.02, SEAGRASS, 0.0, NO_FLOWERS, 0.0, 12.0, SKY_TEMPERATE, &["squid", "cod"]),
        Ocean => biome_def!(Ocean, "海洋", SAND, SAND, STONE, SAND, -16.0, 0.4, 0.0, T::None, AIR, AIR,
            0.04, SEAGRASS, 0.0, NO_FLOWERS, 0.0, 15.0, SKY_TEMPERATE, &["squid", "cod", "salmon"]),
        WarmShallows => biome_def!(WarmShallows, "浅瀬サンゴ礁", SAND, SAND, LIMESTONE, SAND, -7.0, 0.3, 0.0, T::None, AIR, AIR,
            0.14, CORAL, 0.0, NO_FLOWERS, 0.05, 26.0, SKY_TROPIC, &["cod", "turtle"]),
        FrozenOcean => biome_def!(FrozenOcean, "氷海", PACKED_ICE, GRAVEL, STONE, GRAVEL, -14.0, 0.4, 0.0, T::None, AIR, AIR,
            0.0, AIR, 0.0, NO_FLOWERS, 0.0, -8.0, SKY_COLD, &["polar_bear", "seal"]),
        Beach => biome_def!(Beach, "砂浜", SAND, SAND, SANDSTONE, SAND, 1.0, 0.25, 0.006, T::Palm, PALM_LOG, PALM_LEAVES,
            0.03, TALL_GRASS, 0.004, SPARSE_FLOWERS, 0.55, 22.0, SKY_TEMPERATE, &["turtle", "crab"]),
        StonyShore => biome_def!(StonyShore, "岩礁海岸", GRAVEL, STONE, STONE, GRAVEL, 2.0, 0.9, 0.0, T::None, AIR, AIR,
            0.01, TALL_GRASS, 0.0, NO_FLOWERS, 0.25, 12.0, SKY_COLD, &["goat", "crab"]),

        Plains => biome_def!(Plains, "平原", GRASS, DIRT, STONE, DIRT, 5.0, 0.7, 0.008, T::Round, OAK_LOG, OAK_LEAVES,
            0.34, TALL_GRASS, 0.030, TEMPERATE_FLOWERS, 1.0, 18.0, SKY_TEMPERATE, &["cow", "sheep", "horse", "rabbit", "boar"]),
        Meadow => biome_def!(Meadow, "高原草地", GRASS, DIRT, STONE, DIRT, 14.0, 0.9, 0.010, T::Round, OAK_LOG, OAK_LEAVES,
            0.42, TALL_GRASS, 0.070, TEMPERATE_FLOWERS, 0.85, 14.0, SKY_TEMPERATE, &["sheep", "goat", "rabbit", "bee"]),
        Forest => biome_def!(Forest, "温帯森林", GRASS, DIRT, STONE, DIRT, 6.0, 0.9, 0.075, T::Round, OAK_LOG, OAK_LEAVES,
            0.30, TALL_GRASS, 0.022, TEMPERATE_FLOWERS, 0.9, 15.0, SKY_TEMPERATE, &["deer", "wolf", "fox", "boar", "rabbit", "bear"]),
        BirchForest => biome_def!(BirchForest, "白樺林", GRASS, DIRT, STONE, DIRT, 7.0, 0.85, 0.068, T::Round, BIRCH_LOG, BIRCH_LEAVES,
            0.28, TALL_GRASS, 0.026, TEMPERATE_FLOWERS, 0.85, 12.0, SKY_TEMPERATE, &["deer", "fox", "rabbit", "wolf"]),
        DarkForest => biome_def!(DarkForest, "暗黒樹林", PODZOL, DIRT, STONE, DIRT, 6.0, 0.8, 0.115, T::Round, OAK_LOG, OAK_LEAVES,
            0.20, FERN, 0.030, FUNGI, 0.55, 13.0, SKY_TEMPERATE, &["wolf", "bear", "boar", "owl"]),
        CherryGrove => biome_def!(CherryGrove, "桜の丘", GRASS, DIRT, STONE, DIRT, 12.0, 0.8, 0.055, T::Round, CHERRY_LOG, CHERRY_LEAVES,
            0.36, TALL_GRASS, 0.090, TEMPERATE_FLOWERS, 0.9, 16.0, SKY_TEMPERATE, &["bee", "rabbit", "deer", "fox"]),

        Taiga => biome_def!(Taiga, "タイガ", GRASS, DIRT, STONE, DIRT, 8.0, 1.0, 0.085, T::Conifer, SPRUCE_LOG, SPRUCE_LEAVES,
            0.22, FERN, 0.010, SPARSE_FLOWERS, 0.7, 3.0, SKY_COLD, &["wolf", "bear", "moose", "fox", "lynx"]),
        SnowyTaiga => biome_def!(SnowyTaiga, "雪原タイガ", SNOW, DIRT, STONE, DIRT, 8.0, 1.0, 0.070, T::Conifer, SPRUCE_LOG, SPRUCE_LEAVES,
            0.10, FERN, 0.0, NO_FLOWERS, 0.45, -6.0, SKY_COLD, &["wolf", "moose", "lynx", "fox"]),
        SnowyPlains => biome_def!(SnowyPlains, "雪原", SNOW, DIRT, STONE, DIRT, 5.0, 0.6, 0.004, T::Conifer, SPRUCE_LOG, SPRUCE_LEAVES,
            0.06, TALL_GRASS, 0.0, NO_FLOWERS, 0.4, -10.0, SKY_COLD, &["rabbit", "fox", "wolf"]),
        Tundra => biome_def!(Tundra, "ツンドラ", COARSE_DIRT, DIRT, STONE, GRAVEL, 6.0, 0.7, 0.001, T::Dead, DEAD_LOG, AIR,
            0.12, TALL_GRASS, 0.002, SPARSE_FLOWERS, 0.3, -14.0, SKY_COLD, &["reindeer", "wolf", "hare"]),
        IceSpikes => biome_def!(IceSpikes, "氷尖塔地帯", SNOW, PACKED_ICE, STONE, PACKED_ICE, 9.0, 1.3, 0.0, T::None, AIR, AIR,
            0.0, AIR, 0.0, NO_FLOWERS, 0.05, -24.0, SKY_COLD, &["polar_bear"]),
        Glacier => biome_def!(Glacier, "氷河", PACKED_ICE, ICE, STONE, PACKED_ICE, 24.0, 1.6, 0.0, T::None, AIR, AIR,
            0.0, AIR, 0.0, NO_FLOWERS, 0.0, -28.0, SKY_COLD, &["polar_bear"]),

        Savanna => biome_def!(Savanna, "サバンナ", GRASS, DIRT, STONE, SAND, 6.0, 0.7, 0.016, T::Umbrella, ACACIA_LOG, ACACIA_LEAVES,
            0.40, TALL_GRASS, 0.008, SPARSE_FLOWERS, 0.75, 28.0, SKY_ARID, &["zebra", "lion", "elephant", "giraffe", "gazelle", "ostrich"]),
        Desert => biome_def!(Desert, "砂漠", SAND, SANDSTONE, SANDSTONE, SAND, 4.0, 0.5, 0.004, T::Cactus, CACTUS, AIR,
            0.02, DEAD_BUSH, 0.0, NO_FLOWERS, 0.25, 35.0, SKY_ARID, &["camel", "scorpion", "jackal", "vulture"]),
        RedDesert => biome_def!(RedDesert, "赤砂漠", RED_SAND, TERRACOTTA, SANDSTONE, RED_SAND, 5.0, 0.7, 0.003, T::Cactus, CACTUS, AIR,
            0.02, DEAD_BUSH, 0.0, NO_FLOWERS, 0.2, 37.0, SKY_ARID, &["camel", "scorpion", "vulture"]),
        Badlands => biome_def!(Badlands, "メサ荒野", RED_SAND, TERRACOTTA, TERRACOTTA, RED_SAND, 14.0, 1.8, 0.002, T::Dead, DEAD_LOG, AIR,
            0.03, DEAD_BUSH, 0.0, NO_FLOWERS, 0.2, 32.0, SKY_ARID, &["vulture", "jackal", "goat"]),

        Jungle => biome_def!(Jungle, "熱帯雨林", GRASS, DIRT, STONE, DIRT, 8.0, 1.0, 0.130, T::Giant, JUNGLE_LOG, JUNGLE_LEAVES,
            0.55, FERN, 0.035, TEMPERATE_FLOWERS, 0.55, 29.0, SKY_TROPIC, &["tiger", "monkey", "parrot", "tapir", "python", "jaguar"]),
        BambooJungle => biome_def!(BambooJungle, "竹林", GRASS, DIRT, STONE, DIRT, 8.0, 0.9, 0.140, T::Bamboo, BAMBOO, JUNGLE_LEAVES,
            0.45, FERN, 0.020, TEMPERATE_FLOWERS, 0.6, 26.0, SKY_TROPIC, &["panda", "monkey", "parrot"]),
        Mangrove => biome_def!(Mangrove, "マングローブ林", PEAT, CLAY, STONE, CLAY, 1.0, 0.35, 0.095, T::Mangrove, MANGROVE_LOG, MANGROVE_LEAVES,
            0.35, SEAGRASS, 0.010, TEMPERATE_FLOWERS, 0.4, 27.0, SKY_SWAMP, &["crocodile", "heron", "frog", "python"]),
        Swamp => biome_def!(Swamp, "湿地", PEAT, CLAY, STONE, CLAY, 1.5, 0.30, 0.045, T::Round, OAK_LOG, OAK_LEAVES,
            0.30, REEDS, 0.020, FUNGI, 0.45, 20.0, SKY_SWAMP, &["frog", "heron", "crocodile", "boar"]),

        Highlands => biome_def!(Highlands, "高地", GRASS, COARSE_DIRT, STONE, GRAVEL, 34.0, 1.7, 0.020, T::Conifer, SPRUCE_LOG, SPRUCE_LEAVES,
            0.20, TALL_GRASS, 0.012, SPARSE_FLOWERS, 0.55, 8.0, SKY_COLD, &["goat", "eagle", "ibex", "wolf"]),
        RockyMountains => biome_def!(RockyMountains, "岩山", STONE, GRAVEL, GRANITE, GRAVEL, 62.0, 2.6, 0.004, T::Conifer, SPRUCE_LOG, SPRUCE_LEAVES,
            0.05, TALL_GRASS, 0.002, SPARSE_FLOWERS, 0.2, -2.0, SKY_COLD, &["goat", "eagle", "ibex"]),
        SnowyPeaks => biome_def!(SnowyPeaks, "雪嶺", SNOW, STONE, GRANITE, GRAVEL, 88.0, 3.0, 0.0, T::None, AIR, AIR,
            0.0, AIR, 0.0, NO_FLOWERS, 0.05, -20.0, SKY_COLD, &["eagle", "ibex"]),
        Volcanic => biome_def!(Volcanic, "火山地帯", BASALT, TUFF, BASALT, BASALT, 46.0, 2.4, 0.0, T::Dead, DEAD_LOG, AIR,
            0.01, DEAD_BUSH, 0.0, NO_FLOWERS, 0.1, 46.0, SKY_VOLCANIC, &["salamander", "vulture"]),
        MushroomIsle => biome_def!(MushroomIsle, "菌糸島", MYCELIUM, DIRT, STONE, SAND, 7.0, 0.9, 0.045, T::Mushroom, MUSHROOM_BROWN, MUSHROOM_RED,
            0.10, MUSHROOM_BROWN, 0.020, FUNGI, 0.35, 17.0, SKY_TROPIC, &["mooshroom"]),
    }
}

/// 気候パラメータからバイオームを決定する。
///
/// * `continent` — 大陸度 (-1=深海, +1=内陸)
/// * `temperature` — 正規化気温 (-1=極寒, +1=灼熱)
/// * `humidity` — 湿度 (-1=乾燥, +1=多湿)
/// * `erosion` — 侵食度 (-1=険しい山岳, +1=平坦)
/// * `weirdness` — 特異度。稀少バイオームの出現を制御する。
/// * `surface_y` — 実際の地表標高（ブロック）
/// * `sea_level` — 海面標高（ブロック）
pub fn classify(
    continent: f32,
    temperature: f32,
    humidity: f32,
    erosion: f32,
    weirdness: f32,
    surface_y: i32,
    sea_level: i32,
) -> Biome {
    use Biome::*;
    let depth = sea_level - surface_y;

    // --- 水面下 ---
    if surface_y < sea_level {
        if temperature < -0.55 {
            return FrozenOcean;
        }
        if depth > 26 {
            return DeepOcean;
        }
        if depth <= 6 && temperature > 0.45 {
            return WarmShallows;
        }
        return Ocean;
    }

    // --- 海岸線 ---
    if surface_y <= sea_level + 2 && continent < 0.22 {
        if temperature < -0.5 {
            return SnowyPlains;
        }
        if erosion < -0.35 {
            return StonyShore;
        }
        if humidity > 0.55 && temperature > 0.25 {
            return Mangrove;
        }
        return Beach;
    }

    // --- 稀少バイオーム（孤立した島にのみ出現） ---
    if continent < 0.30 && weirdness > 0.86 && temperature > 0.0 {
        return MushroomIsle;
    }

    // --- 高標高帯（気温は標高で既に補正済み） ---
    let altitude = surface_y - sea_level;
    if altitude > 78 {
        return if temperature < -0.15 { SnowyPeaks } else { RockyMountains };
    }
    if altitude > 52 {
        if weirdness > 0.72 && temperature > 0.5 {
            return Volcanic;
        }
        if temperature < -0.45 {
            return Glacier;
        }
        return RockyMountains;
    }
    if altitude > 30 && erosion < -0.1 {
        if temperature < -0.35 {
            return SnowyTaiga;
        }
        return if humidity > 0.1 { Highlands } else { Meadow };
    }

    // --- 低地：気温 × 湿度のマトリクス ---
    match (temperature, humidity) {
        // 極寒
        (t, _) if t < -0.75 => {
            if weirdness > 0.8 {
                IceSpikes
            } else {
                SnowyPlains
            }
        }
        (t, h) if t < -0.45 => {
            if h > 0.0 {
                SnowyTaiga
            } else {
                Tundra
            }
        }
        // 冷涼
        (t, h) if t < -0.15 => {
            if h > 0.2 {
                Taiga
            } else if h > -0.3 {
                Meadow
            } else {
                SnowyPlains
            }
        }
        // 温帯
        (t, h) if t < 0.32 => {
            if h > 0.62 {
                Swamp
            } else if h > 0.34 {
                if weirdness > 0.55 {
                    DarkForest
                } else {
                    Forest
                }
            } else if h > 0.05 {
                if weirdness > 0.6 {
                    CherryGrove
                } else {
                    BirchForest
                }
            } else if h > -0.35 {
                Plains
            } else {
                Savanna
            }
        }
        // 亜熱帯〜熱帯
        (t, h) if t < 0.68 => {
            if h > 0.55 {
                Jungle
            } else if h > 0.2 {
                if weirdness > 0.5 {
                    BambooJungle
                } else {
                    Jungle
                }
            } else if h > -0.25 {
                Savanna
            } else {
                Desert
            }
        }
        // 灼熱
        (_, h) => {
            if h > 0.45 {
                Jungle
            } else if h > -0.1 {
                Savanna
            } else if weirdness > 0.55 {
                Badlands
            } else if weirdness > 0.15 {
                RedDesert
            } else {
                Desert
            }
        }
    }
}

impl Biome {
    pub fn display_name(self) -> &'static str {
        biome_def(self).display_name
    }

    /// 海洋バイオームか。
    pub fn is_oceanic(self) -> bool {
        matches!(
            self,
            Biome::DeepOcean | Biome::Ocean | Biome::WarmShallows | Biome::FrozenOcean
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_biome_has_a_complete_definition() {
        for b in ALL_BIOMES {
            let d = biome_def(b);
            assert_eq!(d.biome, b, "biome_def returned a mismatched biome");
            assert!(!d.display_name.is_empty());
            assert!(d.tree_density >= 0.0 && d.tree_density <= 1.0);
            assert!(d.grass_density >= 0.0 && d.grass_density <= 1.0);
            assert!(d.flower_density >= 0.0 && d.flower_density <= 1.0);
            assert!(d.habitability >= 0.0 && d.habitability <= 1.0);
            assert!(d.height_scale > 0.0);
            // 木を生やすと宣言しているなら幹ブロックが必要。
            if d.tree_density > 0.0 {
                assert_ne!(d.tree_shape, TreeShape::None, "{:?} has density but no shape", b);
                assert!(!d.tree_log.is_air(), "{:?} has density but no log block", b);
            }
            // 花を咲かせると宣言しているなら候補が必要。
            if d.flower_density > 0.0 {
                assert!(!d.flowers.is_empty(), "{:?} has flower density but no flowers", b);
            }
        }
    }

    #[test]
    fn underwater_always_classifies_as_ocean() {
        for t in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            for h in [-1.0f32, 0.0, 1.0] {
                let b = classify(0.5, t, h, 0.0, 0.0, 30, 64);
                assert!(b.is_oceanic(), "y<sea_level gave non-ocean biome {b:?}");
            }
        }
    }

    #[test]
    fn high_altitude_is_always_mountainous() {
        let b = classify(0.9, 0.0, 0.0, -0.5, 0.0, 64 + 100, 64);
        assert!(matches!(b, Biome::SnowyPeaks | Biome::RockyMountains));
    }

    #[test]
    fn hot_and_dry_gives_desert_family() {
        let b = classify(0.9, 0.9, -0.9, 0.5, 0.0, 70, 64);
        assert!(matches!(b, Biome::Desert | Biome::RedDesert | Biome::Badlands), "got {b:?}");
    }

    #[test]
    fn temperate_and_wet_gives_forest_family() {
        let b = classify(0.9, 0.0, 0.45, 0.5, 0.0, 70, 64);
        assert!(matches!(b, Biome::Forest | Biome::DarkForest | Biome::Swamp), "got {b:?}");
    }

    #[test]
    fn classification_is_pure() {
        let a = classify(0.4, 0.2, 0.3, 0.1, 0.5, 80, 64);
        let b = classify(0.4, 0.2, 0.3, 0.1, 0.5, 80, 64);
        assert_eq!(a, b);
    }
}
