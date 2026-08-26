//! 動物種のデータテーブル。
//!
//! バイオーム定義 (`biome.rs`) の `fauna` フィールドが参照するキーの実体。
//! 体格・色・食性・群れの大きさ・攻撃力などを全てここに集約してあるため、
//! 新しい生物を足すのはこの表に1行加えるだけで済む。プラグインの
//! `creatures` も同じ構造へ変換されて追加される。

use crate::biome::Biome;

/// 食性。生態系の栄養段階を決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diet {
    /// 植物食。草を食べ、捕食者から逃げる。
    Herbivore,
    /// 肉食。草食動物と、飢えれば人間も襲う。
    Carnivore,
    /// 雑食。基本は草食だが、追い詰められると反撃する。
    Omnivore,
    /// 濾過食・虫食など。他個体と相互作用しない。
    Filter,
}

/// 体の作り。ブロックモデルの組み立て方が変わる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPlan {
    /// 四足獣（牛・狼・鹿…）
    Quadruped,
    /// 鳥類（二足＋翼）
    Bird,
    /// 魚類
    Fish,
    /// 昆虫（6脚）
    Insect,
    /// 爬虫類（低い胴＋短い四肢）
    Reptile,
}

#[derive(Debug, Clone)]
pub struct SpeciesDef {
    pub key: &'static str,
    pub display_name: &'static str,
    pub body: BodyPlan,
    pub diet: Diet,
    /// 体長（ブロック）。モデルの全体スケールになる。
    pub length: f32,
    /// 肩高（ブロック）。
    pub height: f32,
    pub max_health: f32,
    /// 通常移動速度 (ブロック/秒)。
    pub speed: f32,
    /// 逃走・突進時の速度。
    pub sprint_speed: f32,
    /// 近接攻撃力 (毎秒ダメージ)。0 なら攻撃しない。
    pub attack: f32,
    /// 群れの標準頭数。
    pub herd_size: (u32, u32),
    /// 主色・副色・角/嘴などの差し色。
    pub color_primary: [f32; 3],
    pub color_secondary: [f32; 3],
    pub color_accent: [f32; 3],
    /// 倒したときに得られるアイテム。
    pub drops: &'static [(&'static str, u32)],
    /// 人間を恐れる距離（0 なら恐れない）。
    pub flee_distance: f32,
    /// 夜行性か。
    pub nocturnal: bool,
}

const fn s(
    key: &'static str,
    display_name: &'static str,
    body: BodyPlan,
    diet: Diet,
    length: f32,
    height: f32,
    max_health: f32,
    speed: f32,
    sprint_speed: f32,
    attack: f32,
    herd_size: (u32, u32),
    color_primary: [f32; 3],
    color_secondary: [f32; 3],
    color_accent: [f32; 3],
    drops: &'static [(&'static str, u32)],
    flee_distance: f32,
    nocturnal: bool,
) -> SpeciesDef {
    SpeciesDef {
        key,
        display_name,
        body,
        diet,
        length,
        height,
        max_health,
        speed,
        sprint_speed,
        attack,
        herd_size,
        color_primary,
        color_secondary,
        color_accent,
        drops,
        flee_distance,
        nocturnal,
    }
}

const D_MEAT: &[(&str, u32)] = &[("genesis:raw_meat", 2)];
const D_MEAT_HIDE: &[(&str, u32)] = &[("genesis:raw_meat", 3), ("genesis:leather", 2)];
const D_BIG_GAME: &[(&str, u32)] = &[("genesis:raw_meat", 5), ("genesis:leather", 3), ("genesis:bone", 2)];
const D_WOOL: &[(&str, u32)] = &[("genesis:raw_meat", 2), ("genesis:wool", 3)];
const D_FISH: &[(&str, u32)] = &[("genesis:raw_fish", 2)];
const D_PELT: &[(&str, u32)] = &[("genesis:pelt", 1), ("genesis:bone", 1)];
const D_FEATHER: &[(&str, u32)] = &[("genesis:feather", 3), ("genesis:raw_meat", 1)];
const D_NONE: &[(&str, u32)] = &[];

use BodyPlan::*;
use Diet::*;

/// 世界に存在する全動物種。
pub const SPECIES: &[SpeciesDef] = &[
    // --- 家畜・温帯の草食獣 ---
    s("cow", "牛", Quadruped, Herbivore, 1.5, 1.3, 40.0, 1.6, 3.0, 0.0, (3, 6),
      [0.35, 0.25, 0.18], [0.92, 0.90, 0.86], [0.20, 0.16, 0.14], D_MEAT_HIDE, 6.0, false),
    s("sheep", "羊", Quadruped, Herbivore, 1.1, 1.0, 28.0, 1.5, 3.2, 0.0, (4, 9),
      [0.92, 0.91, 0.88], [0.86, 0.84, 0.80], [0.30, 0.26, 0.24], D_WOOL, 7.0, false),
    s("horse", "馬", Quadruped, Herbivore, 1.9, 1.6, 55.0, 3.4, 8.5, 0.0, (2, 5),
      [0.42, 0.28, 0.16], [0.30, 0.20, 0.12], [0.18, 0.14, 0.10], D_MEAT_HIDE, 9.0, false),
    s("pig", "豚", Quadruped, Omnivore, 1.1, 0.9, 30.0, 1.4, 2.8, 0.0, (2, 5),
      [0.88, 0.62, 0.62], [0.78, 0.52, 0.52], [0.40, 0.28, 0.28], D_MEAT, 6.0, false),
    s("goat", "山羊", Quadruped, Herbivore, 1.1, 1.1, 30.0, 2.2, 5.0, 3.0, (3, 7),
      [0.80, 0.76, 0.70], [0.56, 0.52, 0.46], [0.30, 0.26, 0.20], D_MEAT_HIDE, 8.0, false),
    s("rabbit", "兎", Quadruped, Herbivore, 0.5, 0.4, 8.0, 2.6, 7.0, 0.0, (2, 6),
      [0.72, 0.64, 0.54], [0.90, 0.88, 0.84], [0.86, 0.60, 0.60], D_MEAT, 12.0, false),
    s("deer", "鹿", Quadruped, Herbivore, 1.5, 1.4, 34.0, 2.8, 8.0, 0.0, (3, 8),
      [0.66, 0.46, 0.26], [0.88, 0.82, 0.72], [0.52, 0.44, 0.32], D_MEAT_HIDE, 14.0, false),
    s("moose", "ヘラジカ", Quadruped, Herbivore, 2.3, 2.0, 80.0, 2.4, 6.5, 8.0, (1, 3),
      [0.32, 0.22, 0.14], [0.22, 0.16, 0.10], [0.62, 0.54, 0.40], D_BIG_GAME, 10.0, false),
    s("reindeer", "トナカイ", Quadruped, Herbivore, 1.7, 1.5, 46.0, 2.7, 7.5, 4.0, (5, 14),
      [0.62, 0.54, 0.44], [0.88, 0.86, 0.82], [0.50, 0.42, 0.30], D_BIG_GAME, 12.0, false),
    s("hare", "野兎", Quadruped, Herbivore, 0.6, 0.45, 9.0, 3.0, 8.0, 0.0, (1, 4),
      [0.90, 0.90, 0.92], [0.80, 0.80, 0.84], [0.60, 0.56, 0.56], D_MEAT, 13.0, false),
    s("boar", "猪", Quadruped, Omnivore, 1.3, 1.0, 42.0, 2.2, 6.5, 12.0, (2, 5),
      [0.30, 0.24, 0.20], [0.20, 0.16, 0.14], [0.86, 0.84, 0.78], D_MEAT_HIDE, 4.0, false),
    s("ibex", "アイベックス", Quadruped, Herbivore, 1.2, 1.2, 34.0, 2.6, 6.0, 5.0, (2, 6),
      [0.62, 0.52, 0.36], [0.44, 0.36, 0.24], [0.28, 0.22, 0.16], D_MEAT_HIDE, 11.0, false),

    // --- 捕食者 ---
    s("wolf", "狼", Quadruped, Carnivore, 1.2, 1.0, 45.0, 3.2, 9.0, 18.0, (3, 6),
      [0.36, 0.36, 0.40], [0.24, 0.24, 0.28], [0.86, 0.82, 0.30], D_PELT, 0.0, true),
    s("fox", "狐", Quadruped, Carnivore, 0.9, 0.7, 20.0, 3.0, 8.0, 8.0, (1, 2),
      [0.80, 0.44, 0.18], [0.92, 0.88, 0.82], [0.20, 0.18, 0.16], D_PELT, 9.0, true),
    s("lynx", "オオヤマネコ", Quadruped, Carnivore, 1.0, 0.9, 34.0, 3.1, 9.5, 16.0, (1, 2),
      [0.72, 0.66, 0.54], [0.52, 0.46, 0.36], [0.24, 0.22, 0.20], D_PELT, 6.0, true),
    s("bear", "熊", Quadruped, Omnivore, 2.0, 1.5, 95.0, 2.6, 8.0, 26.0, (1, 2),
      [0.28, 0.20, 0.14], [0.20, 0.14, 0.10], [0.44, 0.36, 0.28], D_BIG_GAME, 0.0, false),
    s("polar_bear", "ホッキョクグマ", Quadruped, Carnivore, 2.2, 1.6, 110.0, 2.8, 8.5, 30.0, (1, 2),
      [0.94, 0.94, 0.96], [0.84, 0.86, 0.90], [0.20, 0.20, 0.22], D_BIG_GAME, 0.0, false),
    s("lion", "獅子", Quadruped, Carnivore, 1.9, 1.3, 85.0, 3.0, 11.0, 28.0, (2, 5),
      [0.80, 0.66, 0.38], [0.56, 0.42, 0.22], [0.32, 0.22, 0.12], D_BIG_GAME, 0.0, true),
    s("tiger", "虎", Quadruped, Carnivore, 2.0, 1.3, 95.0, 3.2, 11.5, 32.0, (1, 2),
      [0.82, 0.52, 0.16], [0.94, 0.90, 0.84], [0.12, 0.10, 0.08], D_BIG_GAME, 0.0, true),
    s("jaguar", "ジャガー", Quadruped, Carnivore, 1.6, 1.1, 70.0, 3.3, 11.0, 24.0, (1, 1),
      [0.78, 0.62, 0.26], [0.20, 0.18, 0.14], [0.40, 0.32, 0.16], D_BIG_GAME, 0.0, true),
    s("jackal", "ジャッカル", Quadruped, Carnivore, 0.9, 0.7, 26.0, 3.2, 8.5, 10.0, (2, 5),
      [0.68, 0.56, 0.34], [0.46, 0.38, 0.24], [0.24, 0.20, 0.16], D_PELT, 5.0, true),
    s("crocodile", "ワニ", Reptile, Carnivore, 2.4, 0.5, 90.0, 1.2, 7.0, 34.0, (1, 3),
      [0.28, 0.36, 0.24], [0.20, 0.26, 0.18], [0.86, 0.84, 0.76], D_BIG_GAME, 0.0, false),
    s("python", "大蛇", Reptile, Carnivore, 2.6, 0.3, 40.0, 1.6, 5.0, 16.0, (1, 1),
      [0.44, 0.40, 0.22], [0.62, 0.56, 0.32], [0.20, 0.18, 0.12], D_MEAT, 0.0, true),
    s("salamander", "火トカゲ", Reptile, Carnivore, 0.8, 0.3, 22.0, 2.0, 5.0, 9.0, (1, 3),
      [0.72, 0.28, 0.14], [0.32, 0.18, 0.12], [0.94, 0.66, 0.22], D_NONE, 0.0, true),
    s("scorpion", "サソリ", Insect, Carnivore, 0.5, 0.2, 14.0, 2.2, 4.5, 11.0, (1, 3),
      [0.28, 0.22, 0.16], [0.18, 0.14, 0.10], [0.62, 0.54, 0.20], D_NONE, 0.0, true),

    // --- サバンナ・熱帯 ---
    s("zebra", "シマウマ", Quadruped, Herbivore, 1.8, 1.5, 52.0, 3.3, 9.0, 4.0, (4, 10),
      [0.94, 0.92, 0.88], [0.12, 0.12, 0.14], [0.30, 0.28, 0.26], D_MEAT_HIDE, 12.0, false),
    s("elephant", "象", Quadruped, Herbivore, 3.4, 2.8, 220.0, 2.0, 6.0, 30.0, (2, 6),
      [0.58, 0.56, 0.54], [0.44, 0.42, 0.40], [0.92, 0.90, 0.84], D_BIG_GAME, 0.0, false),
    s("giraffe", "キリン", Quadruped, Herbivore, 2.2, 3.6, 90.0, 2.6, 7.0, 8.0, (2, 6),
      [0.84, 0.70, 0.36], [0.56, 0.36, 0.16], [0.34, 0.28, 0.18], D_BIG_GAME, 11.0, false),
    s("gazelle", "ガゼル", Quadruped, Herbivore, 1.2, 1.1, 24.0, 3.6, 12.0, 0.0, (6, 16),
      [0.78, 0.62, 0.38], [0.94, 0.92, 0.86], [0.24, 0.20, 0.16], D_MEAT_HIDE, 16.0, false),
    s("camel", "駱駝", Quadruped, Herbivore, 2.0, 2.0, 70.0, 2.4, 5.5, 5.0, (2, 5),
      [0.78, 0.64, 0.42], [0.62, 0.50, 0.32], [0.36, 0.30, 0.20], D_MEAT_HIDE, 8.0, false),
    s("monkey", "猿", Quadruped, Omnivore, 0.7, 0.6, 18.0, 3.0, 7.0, 5.0, (4, 10),
      [0.44, 0.32, 0.20], [0.70, 0.56, 0.40], [0.24, 0.18, 0.14], D_MEAT, 7.0, false),
    s("panda", "パンダ", Quadruped, Herbivore, 1.5, 1.1, 60.0, 1.6, 4.0, 10.0, (1, 2),
      [0.94, 0.93, 0.90], [0.12, 0.12, 0.14], [0.30, 0.30, 0.32], D_MEAT_HIDE, 6.0, false),
    s("tapir", "バク", Quadruped, Herbivore, 1.6, 1.1, 48.0, 2.0, 5.5, 4.0, (1, 3),
      [0.24, 0.22, 0.24], [0.88, 0.86, 0.84], [0.16, 0.14, 0.16], D_MEAT_HIDE, 9.0, true),
    s("mooshroom", "菌牛", Quadruped, Herbivore, 1.5, 1.3, 40.0, 1.5, 2.8, 0.0, (2, 4),
      [0.62, 0.20, 0.18], [0.94, 0.92, 0.88], [0.86, 0.24, 0.22], D_MEAT_HIDE, 6.0, false),

    // --- 鳥類 ---
    s("eagle", "鷲", Bird, Carnivore, 0.9, 0.5, 22.0, 4.0, 12.0, 12.0, (1, 2),
      [0.36, 0.28, 0.20], [0.92, 0.90, 0.86], [0.94, 0.76, 0.18], D_FEATHER, 0.0, false),
    s("vulture", "禿鷲", Bird, Carnivore, 0.9, 0.6, 20.0, 3.4, 9.0, 6.0, (2, 6),
      [0.26, 0.22, 0.20], [0.66, 0.56, 0.46], [0.86, 0.62, 0.30], D_FEATHER, 5.0, false),
    s("owl", "梟", Bird, Carnivore, 0.5, 0.4, 12.0, 3.2, 8.0, 5.0, (1, 2),
      [0.52, 0.42, 0.30], [0.86, 0.80, 0.70], [0.94, 0.82, 0.24], D_FEATHER, 8.0, true),
    s("parrot", "オウム", Bird, Herbivore, 0.4, 0.35, 8.0, 3.4, 8.5, 0.0, (2, 8),
      [0.86, 0.22, 0.20], [0.24, 0.70, 0.30], [0.94, 0.84, 0.26], D_FEATHER, 10.0, false),
    s("ostrich", "駝鳥", Bird, Omnivore, 1.4, 2.2, 50.0, 4.0, 13.0, 9.0, (2, 6),
      [0.28, 0.26, 0.26], [0.90, 0.88, 0.84], [0.86, 0.66, 0.32], D_FEATHER, 12.0, false),
    s("heron", "鷺", Bird, Carnivore, 0.8, 1.0, 12.0, 2.4, 7.0, 3.0, (1, 3),
      [0.86, 0.86, 0.88], [0.56, 0.58, 0.62], [0.90, 0.78, 0.24], D_FEATHER, 11.0, false),
    s("chicken", "鶏", Bird, Omnivore, 0.4, 0.4, 6.0, 1.6, 3.5, 0.0, (3, 8),
      [0.94, 0.92, 0.88], [0.86, 0.84, 0.78], [0.88, 0.22, 0.18], D_FEATHER, 6.0, false),

    // --- 水生 ---
    s("cod", "タラ", Fish, Filter, 0.5, 0.3, 6.0, 2.2, 4.5, 0.0, (5, 14),
      [0.62, 0.58, 0.44], [0.86, 0.84, 0.76], [0.32, 0.30, 0.24], D_FISH, 8.0, false),
    s("salmon", "鮭", Fish, Filter, 0.6, 0.3, 8.0, 2.6, 5.5, 0.0, (5, 16),
      [0.72, 0.40, 0.34], [0.88, 0.86, 0.84], [0.36, 0.30, 0.28], D_FISH, 8.0, false),
    s("squid", "烏賊", Fish, Filter, 0.7, 0.5, 10.0, 1.8, 4.0, 0.0, (2, 6),
      [0.34, 0.28, 0.42], [0.52, 0.44, 0.60], [0.86, 0.84, 0.88], D_NONE, 6.0, true),
    s("turtle", "海亀", Reptile, Herbivore, 0.9, 0.4, 24.0, 0.8, 1.6, 0.0, (2, 5),
      [0.30, 0.52, 0.32], [0.62, 0.56, 0.36], [0.20, 0.34, 0.22], D_NONE, 7.0, false),
    s("seal", "アザラシ", Quadruped, Carnivore, 1.3, 0.5, 34.0, 1.4, 4.5, 6.0, (3, 9),
      [0.46, 0.46, 0.50], [0.70, 0.70, 0.74], [0.14, 0.14, 0.16], D_MEAT_HIDE, 7.0, false),
    s("crab", "蟹", Insect, Omnivore, 0.4, 0.25, 10.0, 1.4, 3.0, 4.0, (2, 7),
      [0.82, 0.32, 0.22], [0.62, 0.22, 0.16], [0.92, 0.88, 0.80], D_NONE, 5.0, false),
    s("frog", "蛙", Reptile, Carnivore, 0.35, 0.25, 6.0, 1.8, 5.0, 0.0, (2, 6),
      [0.36, 0.62, 0.30], [0.86, 0.88, 0.62], [0.20, 0.30, 0.16], D_NONE, 6.0, true),

    // --- 昆虫 ---
    s("bee", "蜜蜂", Insect, Herbivore, 0.25, 0.2, 4.0, 2.8, 5.0, 2.0, (4, 12),
      [0.90, 0.78, 0.22], [0.16, 0.14, 0.12], [0.86, 0.86, 0.90], D_NONE, 3.0, false),
];

/// キーから種別定義を引く。
pub fn species_by_key(key: &str) -> Option<&'static SpeciesDef> {
    SPECIES.iter().find(|s| s.key == key)
}

/// バイオームに生息する種のキー一覧。
pub fn fauna_of(biome: Biome) -> &'static [&'static str] {
    crate::biome::biome_def(biome).fauna
}

/// 種のインデックス（決定論的な抽選に使う）。
pub fn species_index(key: &str) -> Option<usize> {
    SPECIES.iter().position(|s| s.key == key)
}

impl SpeciesDef {
    /// 捕食者か（人間・草食獣を襲う）。
    pub fn is_predator(&self) -> bool {
        matches!(self.diet, Diet::Carnivore) && self.attack > 0.0
    }

    /// 水中でしか生きられないか。
    pub fn is_aquatic(&self) -> bool {
        matches!(self.body, BodyPlan::Fish) || matches!(self.key, "squid" | "seal" | "turtle")
    }

    /// 飛べるか。
    pub fn can_fly(&self) -> bool {
        matches!(self.body, BodyPlan::Bird) && !matches!(self.key, "ostrich" | "chicken")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biome::ALL_BIOMES;

    #[test]
    fn the_roster_is_large_and_varied() {
        assert!(SPECIES.len() >= 45, "only {} species defined", SPECIES.len());
        let predators = SPECIES.iter().filter(|s| s.is_predator()).count();
        let herbivores = SPECIES.iter().filter(|s| s.diet == Diet::Herbivore).count();
        assert!(predators >= 10, "too few predators: {predators}");
        assert!(herbivores >= 15, "too few herbivores: {herbivores}");
        for plan in [BodyPlan::Quadruped, BodyPlan::Bird, BodyPlan::Fish, BodyPlan::Insect, BodyPlan::Reptile] {
            assert!(
                SPECIES.iter().any(|s| s.body == plan),
                "no species uses body plan {plan:?}"
            );
        }
    }

    #[test]
    fn species_keys_are_unique() {
        let mut keys: Vec<&str> = SPECIES.iter().map(|s| s.key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before, "duplicate species keys in the roster");
    }

    #[test]
    fn every_species_has_sane_stats() {
        for sp in SPECIES {
            assert!(sp.length > 0.0 && sp.length < 8.0, "{}: bad length", sp.key);
            assert!(sp.height > 0.0 && sp.height < 6.0, "{}: bad height", sp.key);
            assert!(sp.max_health > 0.0, "{}: bad health", sp.key);
            assert!(sp.speed > 0.0, "{}: bad speed", sp.key);
            assert!(sp.sprint_speed >= sp.speed, "{}: sprint slower than walk", sp.key);
            assert!(sp.attack >= 0.0, "{}: negative attack", sp.key);
            assert!(sp.herd_size.0 >= 1 && sp.herd_size.1 >= sp.herd_size.0, "{}: bad herd size", sp.key);
            assert!(sp.flee_distance >= 0.0, "{}: bad flee distance", sp.key);
            for c in [sp.color_primary, sp.color_secondary, sp.color_accent] {
                assert!(c.iter().all(|v| (0.0..=1.0).contains(v)), "{}: colour out of range", sp.key);
            }
            for (item, qty) in sp.drops {
                assert!(item.contains(':'), "{}: drop '{item}' is not namespaced", sp.key);
                assert!(*qty > 0, "{}: zero-quantity drop", sp.key);
            }
        }
    }

    #[test]
    fn every_biome_fauna_key_resolves() {
        for b in ALL_BIOMES {
            for key in fauna_of(b) {
                assert!(
                    species_by_key(key).is_some(),
                    "biome {b:?} lists unknown species '{key}'"
                );
            }
        }
    }

    #[test]
    fn every_species_lives_somewhere() {
        for sp in SPECIES {
            let found = ALL_BIOMES.iter().any(|b| fauna_of(*b).contains(&sp.key));
            // 家畜は集落が連れてくるので、野生バイオームに載っていなくてよい。
            if matches!(sp.key, "pig" | "chicken") {
                continue;
            }
            assert!(found, "species '{}' has no habitat", sp.key);
        }
    }

    #[test]
    fn predators_are_faster_than_nothing_and_deal_damage() {
        for sp in SPECIES.iter().filter(|s| s.is_predator()) {
            assert!(sp.attack > 0.0);
            assert!(sp.sprint_speed >= 4.0, "{} cannot catch anything", sp.key);
        }
    }

    #[test]
    fn aquatic_species_are_not_land_predators_of_humans() {
        // 魚が陸上のNPCを襲うと世界が壊れるため、水生種は攻撃力を持たないか
        // 明示的に水辺の捕食者（ワニ）であること。
        for sp in SPECIES.iter().filter(|s| s.is_aquatic()) {
            assert!(sp.attack <= 8.0, "{} is an overpowered aquatic attacker", sp.key);
        }
    }
}
