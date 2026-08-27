//! プラグイン（Mod）システム。
//!
//! `run/mods/*.json` に置かれた宣言的なプラグインを読み込み、
//! ブロック・鉱脈規則・アイテム・動物種・集落名を世界へ追加する。
//! Rust の再コンパイルを必要とせず、ゲーム内のプラグイン管理画面から
//! 個別に有効・無効を切り替えられる。
//!
//! ワールド生成はプラグインが定義した内容に依存するため、
//! 有効なプラグイン一覧はワールドごとに保存される（`WorldMeta::plugins`）。
//! 途中でプラグインを外しても、そのブロックは「不明ブロック」として
//! 残るのではなく石へ縮退させ、世界が壊れないようにしてある。

use crate::biome::Biome;
use crate::blocks::{BlockDef, BlockId, BlockRegistry, RenderClass, ToolClass};
use crate::worldgen::OreRule;
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// プラグイン1つ分のマニフェスト（JSON の実体）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// `author:plugin_name` 形式の一意なID。
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// 対応するゲーム側のAPIバージョン。
    #[serde(default = "default_api_version")]
    pub api_version: u32,
    #[serde(default)]
    pub blocks: Vec<PluginBlock>,
    #[serde(default)]
    pub ores: Vec<PluginOre>,
    #[serde(default)]
    pub items: Vec<PluginItem>,
    #[serde(default)]
    pub creatures: Vec<PluginCreature>,
    /// 集落名に追加される語。
    #[serde(default)]
    pub settlement_name_parts: Vec<String>,
}

fn default_api_version() -> u32 {
    PLUGIN_API_VERSION
}

/// 本体が受け付けるプラグインAPIのバージョン。
pub const PLUGIN_API_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginBlock {
    pub key: String,
    pub name: String,
    /// `[r, g, b]` を 0.0〜1.0 で指定。
    pub color: [f32; 3],
    #[serde(default)]
    pub color_top: Option<[f32; 3]>,
    #[serde(default)]
    pub color_bottom: Option<[f32; 3]>,
    /// "opaque" | "translucent" | "cross"
    #[serde(default = "default_render")]
    pub render: String,
    #[serde(default = "default_true")]
    pub solid: bool,
    #[serde(default)]
    pub liquid: bool,
    #[serde(default = "default_hardness")]
    pub hardness: f32,
    /// "none" | "pickaxe" | "axe" | "shovel" | "hoe"
    #[serde(default = "default_tool")]
    pub tool: String,
    #[serde(default)]
    pub light: u8,
    #[serde(default)]
    pub grain: Option<f32>,
}

fn default_render() -> String {
    "opaque".into()
}
fn default_tool() -> String {
    "none".into()
}
fn default_true() -> bool {
    true
}
fn default_hardness() -> f32 {
    1.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOre {
    /// 対象ブロックのキー（このプラグインが定義したものでも組み込みでもよい）。
    pub block: String,
    pub min_y: i32,
    pub max_y: i32,
    #[serde(default)]
    pub peak_y: Option<i32>,
    #[serde(default = "default_weight")]
    pub weight: f32,
    #[serde(default = "default_min_size")]
    pub min_size: f32,
    #[serde(default = "default_max_size")]
    pub max_size: f32,
    /// バイオーム名（`Biome` の英語名）。
    #[serde(default)]
    pub biome: Option<String>,
}

fn default_weight() -> f32 {
    3.0
}
fn default_min_size() -> f32 {
    1.0
}
fn default_max_size() -> f32 {
    2.4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginItem {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub base_value: f32,
    #[serde(default)]
    pub nutrition: f32,
    /// "tool" | "weapon" | "food" | "material"
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCreature {
    pub key: String,
    pub name: String,
    pub color: [f32; 3],
    #[serde(default = "default_health")]
    pub health: f32,
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// "herbivore" | "carnivore" | "omnivore"
    #[serde(default)]
    pub diet: String,
    #[serde(default)]
    pub size: f32,
    /// 出現バイオーム名。
    #[serde(default)]
    pub biomes: Vec<String>,
}

fn default_health() -> f32 {
    30.0
}
fn default_speed() -> f32 {
    3.0
}

/// 読み込み済みプラグインの状態。
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub source_path: PathBuf,
    pub enabled: bool,
    /// 読み込み時に検出された問題（管理画面に表示する）。
    pub problems: Vec<String>,
}

impl LoadedPlugin {
    pub fn is_compatible(&self) -> bool {
        self.manifest.api_version == PLUGIN_API_VERSION
    }
}

/// プラグイン適用の結果。ワールド生成へ渡す追加データ。
#[derive(Debug, Clone, Default)]
pub struct PluginContributions {
    pub ore_rules: Vec<OreRule>,
    pub creature_keys: Vec<String>,
    pub item_keys: Vec<String>,
    pub name_parts: Vec<String>,
    pub applied_plugin_ids: Vec<String>,
}

#[derive(Resource, Default)]
pub struct PluginManager {
    pub plugins: Vec<LoadedPlugin>,
    pub last_scan_error: Option<String>,
}

impl PluginManager {
    pub fn mods_dir(root: &Path) -> PathBuf {
        root.join("mods")
    }

    /// `mods/` を走査してマニフェストを読み込む。
    /// 壊れた JSON は「問題あり」として一覧に残し、ゲームは止めない。
    pub fn scan(root: &Path, enabled_ids: &[String]) -> Self {
        let dir = Self::mods_dir(root);
        let mut mgr = PluginManager::default();

        if let Err(e) = std::fs::create_dir_all(&dir) {
            mgr.last_scan_error = Some(format!("mods フォルダを作成できません: {e}"));
            return mgr;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                mgr.last_scan_error = Some(format!("mods フォルダを読めません: {e}"));
                return mgr;
            }
        };

        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
            .collect();
        files.sort();

        for path in files {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    mgr.plugins.push(broken_plugin(&path, format!("読み込み失敗: {e}")));
                    continue;
                }
            };
            let manifest: PluginManifest = match serde_json::from_str(&text) {
                Ok(m) => m,
                Err(e) => {
                    mgr.plugins.push(broken_plugin(&path, format!("JSON 構文エラー: {e}")));
                    continue;
                }
            };

            let mut problems = validate(&manifest);
            if manifest.api_version != PLUGIN_API_VERSION {
                problems.push(format!(
                    "API バージョン不一致 (プラグイン: v{}, 本体: v{PLUGIN_API_VERSION})",
                    manifest.api_version
                ));
            }
            if mgr.plugins.iter().any(|p| p.manifest.id == manifest.id) {
                problems.push(format!("ID '{}' が重複しています", manifest.id));
            }

            let enabled = enabled_ids.contains(&manifest.id) && problems.is_empty();
            mgr.plugins.push(LoadedPlugin {
                enabled,
                source_path: path,
                manifest,
                problems,
            });
        }

        mgr
    }

    pub fn enabled_ids(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.manifest.id.clone())
            .collect()
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut LoadedPlugin> {
        self.plugins.iter_mut().find(|p| p.manifest.id == id)
    }

    /// 問題のあるプラグインは有効化できない。
    pub fn set_enabled(&mut self, id: &str, on: bool) -> Result<(), String> {
        let Some(p) = self.find_mut(id) else {
            return Err(format!("プラグイン '{id}' が見つかりません"));
        };
        if on && !p.problems.is_empty() {
            return Err(format!("'{}' には未解決の問題があります: {}", p.manifest.name, p.problems.join(" / ")));
        }
        p.enabled = on;
        Ok(())
    }

    /// 有効なプラグインの内容をレジストリへ適用する。
    ///
    /// `only` が Some のときは、そのIDのプラグインだけを適用する
    /// （セーブデータに記録された構成を再現するため）。
    pub fn apply(&self, registry: &mut BlockRegistry, only: Option<&[String]>) -> PluginContributions {
        let mut out = PluginContributions::default();

        for plugin in &self.plugins {
            let wanted = match only {
                Some(list) => list.contains(&plugin.manifest.id),
                None => plugin.enabled,
            };
            if !wanted || !plugin.problems.is_empty() {
                continue;
            }

            for b in &plugin.manifest.blocks {
                registry.register(to_block_def(b));
            }
            // ブロック登録後でなければ鉱脈規則がIDを解決できない。
            for o in &plugin.manifest.ores {
                match to_ore_rule(o, registry) {
                    Some(rule) => out.ore_rules.push(rule),
                    None => bevy::log::warn!(
                        "プラグイン '{}': 鉱脈規則が参照するブロック '{}' が未定義のため無視しました",
                        plugin.manifest.id,
                        o.block
                    ),
                }
            }
            out.creature_keys.extend(plugin.manifest.creatures.iter().map(|c| c.key.clone()));
            out.item_keys.extend(plugin.manifest.items.iter().map(|i| i.key.clone()));
            out.name_parts.extend(plugin.manifest.settlement_name_parts.iter().cloned());
            out.applied_plugin_ids.push(plugin.manifest.id.clone());
        }

        out
    }

    /// 同梱のサンプルプラグインを書き出す（初回起動時）。
    /// 既にファイルがあるときは上書きしない（利用者の編集を壊さない）。
    pub fn write_example_plugin(root: &Path) -> std::io::Result<PathBuf> {
        let dir = Self::mods_dir(root);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("example_deepearth.json");
        if path.exists() {
            return Ok(path);
        }
        std::fs::write(&path, EXAMPLE_PLUGIN_JSON)?;
        Ok(path)
    }
}

fn broken_plugin(path: &Path, problem: String) -> LoadedPlugin {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "?".into());
    LoadedPlugin {
        manifest: PluginManifest {
            id: format!("broken:{name}"),
            name: name.clone(),
            version: String::new(),
            author: String::new(),
            description: "読み込みに失敗したプラグイン".into(),
            api_version: 0,
            blocks: Vec::new(),
            ores: Vec::new(),
            items: Vec::new(),
            creatures: Vec::new(),
            settlement_name_parts: Vec::new(),
        },
        source_path: path.to_path_buf(),
        enabled: false,
        problems: vec![problem],
    }
}

fn validate(m: &PluginManifest) -> Vec<String> {
    let mut problems = Vec::new();

    if m.id.trim().is_empty() {
        problems.push("id が空です".into());
    } else if !m.id.contains(':') {
        problems.push(format!("id '{}' は 'author:name' 形式である必要があります", m.id));
    }
    if m.name.trim().is_empty() {
        problems.push("name が空です".into());
    }
    // 組み込みの名前空間は奪えない。
    if m.id.starts_with("genesis:") {
        problems.push("'genesis:' は本体予約の名前空間です".into());
    }

    for b in &m.blocks {
        if !b.key.contains(':') {
            problems.push(format!("ブロック '{}' のキーが 'ns:name' 形式ではありません", b.key));
        }
        if b.key.starts_with("genesis:") {
            problems.push(format!("ブロック '{}' は本体のブロックを上書きしようとしています", b.key));
        }
        for (i, c) in b.color.iter().enumerate() {
            if !c.is_finite() || !(0.0..=1.0).contains(c) {
                problems.push(format!("ブロック '{}' の color[{i}] が 0.0〜1.0 の範囲外です", b.key));
            }
        }
        if !matches!(b.render.as_str(), "opaque" | "translucent" | "cross") {
            problems.push(format!("ブロック '{}' の render '{}' は未知の値です", b.key, b.render));
        }
        if !matches!(b.tool.as_str(), "none" | "pickaxe" | "axe" | "shovel" | "hoe") {
            problems.push(format!("ブロック '{}' の tool '{}' は未知の値です", b.key, b.tool));
        }
        if b.light > 15 {
            problems.push(format!("ブロック '{}' の light は 0〜15 です", b.key));
        }
    }

    for o in &m.ores {
        if o.min_y >= o.max_y {
            problems.push(format!("鉱脈 '{}' の min_y は max_y より小さくなければなりません", o.block));
        }
        if o.min_y < 3 {
            problems.push(format!("鉱脈 '{}' は岩盤層(y<3)には配置できません", o.block));
        }
        if o.max_y >= crate::chunk::CHUNK_H {
            problems.push(format!("鉱脈 '{}' の max_y が世界の高さを超えています", o.block));
        }
        if let Some(p) = o.peak_y {
            if p < o.min_y || p > o.max_y {
                problems.push(format!("鉱脈 '{}' の peak_y が範囲外です", o.block));
            }
        }
        if o.weight <= 0.0 || !o.weight.is_finite() {
            problems.push(format!("鉱脈 '{}' の weight は正の有限値である必要があります", o.block));
        }
        if o.min_size <= 0.0 || o.max_size < o.min_size {
            problems.push(format!("鉱脈 '{}' のサイズ指定が不正です", o.block));
        }
        if let Some(b) = &o.biome {
            if parse_biome(b).is_none() {
                problems.push(format!("鉱脈 '{}' の biome '{b}' は未知のバイオームです", o.block));
            }
        }
    }

    for c in &m.creatures {
        if c.health <= 0.0 || !c.health.is_finite() {
            problems.push(format!("生物 '{}' の health が不正です", c.key));
        }
        if !c.speed.is_finite() || c.speed < 0.0 {
            problems.push(format!("生物 '{}' の speed が不正です", c.key));
        }
        for b in &c.biomes {
            if parse_biome(b).is_none() {
                problems.push(format!("生物 '{}' の biome '{b}' は未知のバイオームです", c.key));
            }
        }
    }

    problems
}

fn to_block_def(b: &PluginBlock) -> BlockDef {
    let render = match b.render.as_str() {
        "translucent" => RenderClass::Translucent,
        "cross" => RenderClass::Cross,
        _ => RenderClass::Opaque,
    };
    let tool = match b.tool.as_str() {
        "pickaxe" => ToolClass::Pickaxe,
        "axe" => ToolClass::Axe,
        "shovel" => ToolClass::Shovel,
        "hoe" => ToolClass::Hoe,
        _ => ToolClass::None,
    };
    let mut def = BlockDef::new(&b.key, &b.name, b.color);
    def.color_top = b.color_top.unwrap_or(b.color);
    def.color_bottom = b.color_bottom.unwrap_or(b.color);
    def.render = render;
    def.solid = if matches!(render, RenderClass::Cross) { false } else { b.solid };
    def.liquid = b.liquid;
    if b.liquid {
        def.solid = false;
    }
    def.hardness = b.hardness;
    def.tool = tool;
    def.light = b.light.min(15);
    def.grain = b.grain.unwrap_or(0.06).clamp(0.0, 0.5);
    def
}

fn to_ore_rule(o: &PluginOre, registry: &BlockRegistry) -> Option<OreRule> {
    let block = registry.id_of(&o.block)?;
    let peak = o.peak_y.unwrap_or((o.min_y + o.max_y) / 2).clamp(o.min_y, o.max_y);
    Some(OreRule {
        block,
        min_y: o.min_y.max(3),
        max_y: o.max_y.min(crate::chunk::CHUNK_H - 1),
        peak_y: peak,
        weight: o.weight,
        size: (o.min_size, o.max_size.max(o.min_size)),
        biome_affinity: o.biome.as_deref().and_then(parse_biome),
    })
}

/// バイオーム名（英語識別子）の解決。
pub fn parse_biome(name: &str) -> Option<Biome> {
    use Biome::*;
    let n = name.trim().to_ascii_lowercase();
    Some(match n.as_str() {
        "deepocean" | "deep_ocean" => DeepOcean,
        "ocean" => Ocean,
        "warmshallows" | "warm_shallows" => WarmShallows,
        "frozenocean" | "frozen_ocean" => FrozenOcean,
        "beach" => Beach,
        "stonyshore" | "stony_shore" => StonyShore,
        "plains" => Plains,
        "meadow" => Meadow,
        "forest" => Forest,
        "birchforest" | "birch_forest" => BirchForest,
        "darkforest" | "dark_forest" => DarkForest,
        "cherrygrove" | "cherry_grove" => CherryGrove,
        "taiga" => Taiga,
        "snowytaiga" | "snowy_taiga" => SnowyTaiga,
        "snowyplains" | "snowy_plains" => SnowyPlains,
        "tundra" => Tundra,
        "icespikes" | "ice_spikes" => IceSpikes,
        "glacier" => Glacier,
        "savanna" => Savanna,
        "desert" => Desert,
        "reddesert" | "red_desert" => RedDesert,
        "badlands" => Badlands,
        "jungle" => Jungle,
        "bamboojungle" | "bamboo_jungle" => BambooJungle,
        "mangrove" => Mangrove,
        "swamp" => Swamp,
        "highlands" => Highlands,
        "rockymountains" | "rocky_mountains" => RockyMountains,
        "snowypeaks" | "snowy_peaks" => SnowyPeaks,
        "volcanic" => Volcanic,
        "mushroomisle" | "mushroom_isle" => MushroomIsle,
        _ => return None,
    })
}

/// プラグインが定義したブロックIDのうち、現在のレジストリで解決できないものを
/// 石へ縮退させる（プラグインを外した後のセーブを開けるようにする）。
pub fn degrade_unknown_blocks(voxels: &mut [BlockId], registry: &BlockRegistry) -> usize {
    let max = registry.len() as u16;
    let mut degraded = 0;
    for v in voxels.iter_mut() {
        if v.0 >= max {
            *v = crate::blocks::ids::STONE;
            degraded += 1;
        }
    }
    degraded
}

const EXAMPLE_PLUGIN_JSON: &str = r#"{
  "id": "example:deepearth",
  "name": "Deep Earth Minerals",
  "version": "1.0.0",
  "author": "World Genesis Team",
  "description": "地底深部に希少鉱物を追加するサンプルプラグイン。このファイルを複製して自分のプラグインを作れます。",
  "api_version": 1,
  "blocks": [
    {
      "key": "example:mithril_ore",
      "name": "ミスリル鉱石",
      "color": [0.55, 0.80, 0.88],
      "render": "opaque",
      "hardness": 6.0,
      "tool": "pickaxe",
      "grain": 0.18
    },
    {
      "key": "example:glowshroom",
      "name": "光キノコ",
      "color": [0.45, 0.90, 0.70],
      "render": "cross",
      "solid": false,
      "hardness": 0.05,
      "light": 11
    }
  ],
  "ores": [
    {
      "block": "example:mithril_ore",
      "min_y": 4,
      "max_y": 24,
      "peak_y": 11,
      "weight": 1.1,
      "min_size": 0.8,
      "max_size": 1.6
    }
  ],
  "items": [
    { "key": "example:mithril_ingot", "name": "ミスリルのインゴット", "base_value": 220.0, "category": "material" }
  ],
  "creatures": [
    {
      "key": "example:cave_lizard",
      "name": "洞窟トカゲ",
      "color": [0.35, 0.45, 0.40],
      "health": 24.0,
      "speed": 4.2,
      "diet": "carnivore",
      "size": 0.7,
      "biomes": ["rocky_mountains", "volcanic"]
    }
  ],
  "settlement_name_parts": ["Deep", "delve"]
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::temp_dir as temp_root;

    fn write_mod(root: &Path, file: &str, json: &str) {
        let dir = PluginManager::mods_dir(root);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), json).unwrap();
    }

    #[test]
    fn example_plugin_is_valid_and_loadable() {
        let root = temp_root("example");
        PluginManager::write_example_plugin(&root).unwrap();
        let mgr = PluginManager::scan(&root, &["example:deepearth".to_string()]);

        assert_eq!(mgr.plugins.len(), 1);
        let p = &mgr.plugins[0];
        assert!(p.problems.is_empty(), "bundled example has problems: {:?}", p.problems);
        assert!(p.enabled);
        assert!(p.is_compatible());

        let mut reg = BlockRegistry::with_builtins();
        let before = reg.len();
        let contrib = mgr.apply(&mut reg, None);
        assert_eq!(reg.len(), before + 2, "plugin blocks were not registered");
        assert!(reg.id_of("example:mithril_ore").is_some());
        assert_eq!(contrib.ore_rules.len(), 1);
        assert_eq!(contrib.creature_keys, vec!["example:cave_lizard".to_string()]);
        assert_eq!(contrib.applied_plugin_ids, vec!["example:deepearth".to_string()]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn writing_the_example_never_overwrites_user_edits() {
        let root = temp_root("nooverwrite");
        let path = PluginManager::write_example_plugin(&root).unwrap();
        std::fs::write(&path, r#"{"id":"me:mine","name":"Mine","api_version":1}"#).unwrap();
        PluginManager::write_example_plugin(&root).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("me:mine"), "the bundled example clobbered a user's file");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn broken_json_is_reported_not_fatal() {
        let root = temp_root("broken");
        write_mod(&root, "bad.json", "{ nope");
        write_mod(&root, "good.json", r#"{"id":"a:b","name":"Good","api_version":1}"#);

        let mgr = PluginManager::scan(&root, &[]);
        assert_eq!(mgr.plugins.len(), 2);
        assert!(mgr.plugins.iter().any(|p| !p.problems.is_empty()));
        assert!(mgr.plugins.iter().any(|p| p.manifest.id == "a:b" && p.problems.is_empty()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plugins_may_not_hijack_the_core_namespace() {
        let root = temp_root("namespace");
        write_mod(
            &root,
            "evil.json",
            r#"{"id":"genesis:core","name":"Evil","api_version":1,
                "blocks":[{"key":"genesis:stone","name":"Hijacked","color":[1,0,0]}]}"#,
        );
        let mgr = PluginManager::scan(&root, &["genesis:core".to_string()]);
        let p = &mgr.plugins[0];
        assert!(!p.problems.is_empty());
        assert!(!p.enabled, "a plugin with problems must not auto-enable");

        // 有効化しようとしても拒否される。
        let mut mgr = mgr;
        assert!(mgr.set_enabled("genesis:core", true).is_err());

        // 適用しても組み込みブロックは書き換わらない。
        let mut reg = BlockRegistry::with_builtins();
        mgr.apply(&mut reg, None);
        assert_eq!(reg.get(crate::blocks::ids::STONE).display_name, "石");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_ore_ranges_are_caught() {
        let root = temp_root("ore");
        write_mod(
            &root,
            "ore.json",
            r#"{"id":"a:ore","name":"O","api_version":1,
                "blocks":[{"key":"a:x","name":"X","color":[0.5,0.5,0.5]}],
                "ores":[{"block":"a:x","min_y":100,"max_y":10},
                        {"block":"a:x","min_y":1,"max_y":50},
                        {"block":"a:x","min_y":5,"max_y":40,"biome":"atlantis"}]}"#,
        );
        let mgr = PluginManager::scan(&root, &[]);
        let problems = &mgr.plugins[0].problems;
        assert!(problems.iter().any(|p| p.contains("min_y")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("岩盤")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("atlantis")), "{problems:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn api_version_mismatch_blocks_activation() {
        let root = temp_root("api");
        write_mod(&root, "old.json", r#"{"id":"a:old","name":"Old","api_version":99}"#);
        let mgr = PluginManager::scan(&root, &["a:old".to_string()]);
        assert!(!mgr.plugins[0].enabled);
        assert!(!mgr.plugins[0].is_compatible());
        assert!(mgr.plugins[0].problems.iter().any(|p| p.contains("API")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ore_rule_referencing_a_missing_block_is_dropped_not_panicking() {
        let root = temp_root("missing");
        write_mod(
            &root,
            "m.json",
            r#"{"id":"a:m","name":"M","api_version":1,
                "ores":[{"block":"nothing:here","min_y":5,"max_y":40}]}"#,
        );
        let mgr = PluginManager::scan(&root, &["a:m".to_string()]);
        assert!(mgr.plugins[0].problems.is_empty());
        let mut reg = BlockRegistry::with_builtins();
        let contrib = mgr.apply(&mut reg, None);
        assert!(contrib.ore_rules.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_can_target_an_explicit_plugin_set() {
        let root = temp_root("explicit");
        write_mod(&root, "a.json", r#"{"id":"x:a","name":"A","api_version":1,
            "blocks":[{"key":"x:a_block","name":"A","color":[0.1,0.2,0.3]}]}"#);
        write_mod(&root, "b.json", r#"{"id":"x:b","name":"B","api_version":1,
            "blocks":[{"key":"x:b_block","name":"B","color":[0.3,0.2,0.1]}]}"#);

        // どちらも無効な状態でスキャンする。
        let mgr = PluginManager::scan(&root, &[]);
        assert!(mgr.plugins.iter().all(|p| !p.enabled));

        // セーブに記録された構成だけを再現する。
        let mut reg = BlockRegistry::with_builtins();
        let contrib = mgr.apply(&mut reg, Some(&["x:b".to_string()]));
        assert_eq!(contrib.applied_plugin_ids, vec!["x:b".to_string()]);
        assert!(reg.id_of("x:b_block").is_some());
        assert!(reg.id_of("x:a_block").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_blocks_degrade_to_stone() {
        let reg = BlockRegistry::with_builtins();
        let mut voxels = vec![
            crate::blocks::ids::GRASS,
            BlockId(reg.len() as u16 + 5),
            crate::blocks::ids::WATER,
            BlockId(60000),
        ];
        let n = degrade_unknown_blocks(&mut voxels, &reg);
        assert_eq!(n, 2);
        assert_eq!(voxels[0], crate::blocks::ids::GRASS);
        assert_eq!(voxels[1], crate::blocks::ids::STONE);
        assert_eq!(voxels[2], crate::blocks::ids::WATER);
        assert_eq!(voxels[3], crate::blocks::ids::STONE);
    }

    #[test]
    fn biome_names_parse_in_both_spellings() {
        assert_eq!(parse_biome("rocky_mountains"), Some(Biome::RockyMountains));
        assert_eq!(parse_biome("RockyMountains"), Some(Biome::RockyMountains));
        assert_eq!(parse_biome("  DESERT "), Some(Biome::Desert));
        assert_eq!(parse_biome("nope"), None);
    }
}
