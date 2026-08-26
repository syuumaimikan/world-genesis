//! アイテムとインベントリ。
//!
//! アイテムは「ブロックとして置けるもの」と「道具・食料など単体のもの」に
//! 分かれる。ブロック由来のアイテムはレジストリから自動生成されるため、
//! プラグインがブロックを追加すれば、その鉱石も自動的に持ち運べるようになる。

use crate::blocks::{BlockId, BlockRegistry, ToolClass};
use bevy::prelude::Resource;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemCategory {
    Block,
    Tool,
    Weapon,
    Food,
    Material,
    Misc,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub key: String,
    pub display_name: String,
    pub category: ItemCategory,
    /// 1スタックの上限。
    pub max_stack: u32,
    /// 食べたときに回復する満腹度。
    pub nutrition: f32,
    /// 設置できるブロック。
    pub places: Option<BlockId>,
    /// 道具としての種別と採掘倍率。
    pub tool: ToolClass,
    pub tool_power: f32,
    /// 近接攻撃力。
    pub damage: f32,
    /// 交易時の基準価格。
    pub value: f32,
}

impl ItemDef {
    fn tool(key: &str, name: &str, class: ToolClass, power: f32, damage: f32, value: f32) -> Self {
        Self {
            key: key.into(),
            display_name: name.into(),
            category: if damage >= 6.0 { ItemCategory::Weapon } else { ItemCategory::Tool },
            max_stack: 1,
            nutrition: 0.0,
            places: None,
            tool: class,
            tool_power: power,
            damage,
            value,
        }
    }

    fn food(key: &str, name: &str, nutrition: f32, value: f32) -> Self {
        Self {
            key: key.into(),
            display_name: name.into(),
            category: ItemCategory::Food,
            max_stack: 64,
            nutrition,
            places: None,
            tool: ToolClass::None,
            tool_power: 1.0,
            damage: 1.0,
            value,
        }
    }

    fn material(key: &str, name: &str, value: f32) -> Self {
        Self {
            key: key.into(),
            display_name: name.into(),
            category: ItemCategory::Material,
            max_stack: 64,
            nutrition: 0.0,
            places: None,
            tool: ToolClass::None,
            tool_power: 1.0,
            damage: 1.0,
            value,
        }
    }
}

#[derive(Resource)]
pub struct ItemRegistry {
    defs: Vec<ItemDef>,
    by_key: HashMap<String, usize>,
}

impl ItemRegistry {
    /// ブロックレジストリを読み、設置可能アイテムと固有アイテムを揃える。
    pub fn build(blocks: &BlockRegistry) -> Self {
        let mut reg = Self {
            defs: Vec::new(),
            by_key: HashMap::new(),
        };

        // --- 道具・武器 ---
        for d in [
            ItemDef::tool("genesis:wooden_sword", "木の剣", ToolClass::None, 1.0, 9.0, 12.0),
            ItemDef::tool("genesis:iron_sword", "鉄の剣", ToolClass::None, 1.0, 18.0, 90.0),
            ItemDef::tool("genesis:bow", "狩猟弓", ToolClass::None, 1.0, 3.0, 40.0),
            ItemDef::tool("genesis:stone_pickaxe", "石のツルハシ", ToolClass::Pickaxe, 2.5, 4.0, 14.0),
            ItemDef::tool("genesis:iron_pickaxe", "鉄のツルハシ", ToolClass::Pickaxe, 5.0, 6.0, 80.0),
            ItemDef::tool("genesis:stone_axe", "石の斧", ToolClass::Axe, 2.5, 6.0, 14.0),
            ItemDef::tool("genesis:iron_axe", "鉄の斧", ToolClass::Axe, 5.0, 10.0, 78.0),
            ItemDef::tool("genesis:shovel", "シャベル", ToolClass::Shovel, 3.0, 3.0, 18.0),
            ItemDef::tool("genesis:stone_hoe", "石のクワ", ToolClass::Hoe, 2.0, 2.0, 12.0),
        ] {
            reg.insert(d);
        }

        // --- 食料 ---
        for d in [
            ItemDef::food("genesis:bread", "パン", 26.0, 6.0),
            ItemDef::food("genesis:raw_meat", "生肉", 12.0, 5.0),
            ItemDef::food("genesis:cooked_meat", "焼いた肉", 32.0, 12.0),
            ItemDef::food("genesis:raw_fish", "生魚", 10.0, 4.0),
            ItemDef::food("genesis:berries", "木の実", 8.0, 2.0),
            ItemDef::food("genesis:wheat", "小麦", 4.0, 3.0),
        ] {
            reg.insert(d);
        }

        // --- 素材 ---
        for d in [
            ItemDef::material("genesis:leather", "なめし革", 9.0),
            ItemDef::material("genesis:wool", "羊毛", 7.0),
            ItemDef::material("genesis:bone", "骨", 3.0),
            ItemDef::material("genesis:pelt", "毛皮", 16.0),
            ItemDef::material("genesis:feather", "羽根", 2.0),
            ItemDef::material("genesis:arrow", "矢", 1.5),
            ItemDef::material("genesis:stick", "棒", 1.0),
            ItemDef::material("genesis:iron_ingot", "鉄のインゴット", 34.0),
            ItemDef::material("genesis:gold_ingot", "金のインゴット", 120.0),
            ItemDef::material("genesis:coin", "硬貨", 1.0),
        ] {
            reg.insert(d);
        }

        // --- ブロック由来のアイテム ---
        // 破壊不能ブロック（岩盤）と液体は持ち歩けない。
        for i in 0..blocks.len() {
            let id = BlockId(i as u16);
            let def = blocks.get(id);
            if id.is_air() || def.hardness < 0.0 || def.liquid {
                continue;
            }
            // 既に同名の固有アイテムがあるならそちらを優先する。
            if reg.by_key.contains_key(&def.key) {
                continue;
            }
            reg.insert(ItemDef {
                key: def.key.clone(),
                display_name: def.display_name.clone(),
                category: ItemCategory::Block,
                max_stack: 64,
                nutrition: 0.0,
                places: Some(id),
                tool: ToolClass::None,
                tool_power: 1.0,
                damage: 1.0,
                value: (def.hardness * 3.0).max(1.0),
            });
        }

        reg
    }

    fn insert(&mut self, def: ItemDef) {
        if let Some(&idx) = self.by_key.get(&def.key) {
            self.defs[idx] = def;
            return;
        }
        self.by_key.insert(def.key.clone(), self.defs.len());
        self.defs.push(def);
    }

    pub fn get(&self, key: &str) -> Option<&ItemDef> {
        self.by_key.get(key).map(|&i| &self.defs[i])
    }

    pub fn display_name(&self, key: &str) -> String {
        self.get(key)
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| key.to_string())
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemDef> {
        self.defs.iter()
    }
}

/// 1つのスロット。
#[derive(Debug, Clone, PartialEq)]
pub struct ItemStack {
    pub key: String,
    pub count: u32,
}

/// 所持品。ホットバー 9 枠 + 収納 27 枠。
#[derive(Debug, Clone, bevy::prelude::Component)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
}

pub const HOTBAR_SLOTS: usize = 9;
pub const STORAGE_SLOTS: usize = 27;
pub const TOTAL_SLOTS: usize = HOTBAR_SLOTS + STORAGE_SLOTS;

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: vec![None; TOTAL_SLOTS],
        }
    }
}

impl Inventory {
    /// 初期装備。
    pub fn starter() -> Self {
        let mut inv = Self::default();
        inv.slots[0] = Some(ItemStack { key: "genesis:wooden_sword".into(), count: 1 });
        inv.slots[1] = Some(ItemStack { key: "genesis:stone_pickaxe".into(), count: 1 });
        inv.slots[2] = Some(ItemStack { key: "genesis:stone_axe".into(), count: 1 });
        inv.slots[3] = Some(ItemStack { key: "genesis:bow".into(), count: 1 });
        inv.slots[4] = Some(ItemStack { key: "genesis:arrow".into(), count: 32 });
        inv.slots[5] = Some(ItemStack { key: "genesis:bread".into(), count: 8 });
        inv.slots[6] = Some(ItemStack { key: "genesis:torch".into(), count: 16 });
        inv.slots[7] = Some(ItemStack { key: "genesis:stone_hoe".into(), count: 1 });
        inv
    }

    pub fn hotbar(&self) -> &[Option<ItemStack>] {
        &self.slots[..HOTBAR_SLOTS]
    }

    pub fn get(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot).and_then(|s| s.as_ref())
    }

    /// アイテムを加える。入り切らなかった数を返す。
    pub fn add(&mut self, key: &str, mut count: u32, max_stack: u32) -> u32 {
        let max_stack = max_stack.max(1);

        // まず既存のスタックへ積む。
        for slot in self.slots.iter_mut() {
            if count == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.key == key && stack.count < max_stack {
                    let space = max_stack - stack.count;
                    let moved = space.min(count);
                    stack.count += moved;
                    count -= moved;
                }
            }
        }
        // 次に空きスロットへ。
        for slot in self.slots.iter_mut() {
            if count == 0 {
                break;
            }
            if slot.is_none() {
                let moved = max_stack.min(count);
                *slot = Some(ItemStack { key: key.to_string(), count: moved });
                count -= moved;
            }
        }
        count
    }

    /// スロットから 1 個消費する。使い切ったらスロットを空にする。
    pub fn consume_one(&mut self, slot: usize) -> Option<String> {
        let entry = self.slots.get_mut(slot)?;
        let stack = entry.as_mut()?;
        let key = stack.key.clone();
        stack.count = stack.count.saturating_sub(1);
        if stack.count == 0 {
            *entry = None;
        }
        Some(key)
    }

    /// キーを指定して総数を数える。
    pub fn count_of(&self, key: &str) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.key == key)
            .map(|s| s.count)
            .sum()
    }

    /// キーを指定して 1 個取り出す。
    pub fn take_one(&mut self, key: &str) -> bool {
        for slot in self.slots.iter_mut() {
            let should_clear = match slot {
                Some(stack) if stack.key == key && stack.count > 0 => {
                    stack.count -= 1;
                    stack.count == 0
                }
                _ => continue,
            };
            if should_clear {
                *slot = None;
            }
            return true;
        }
        false
    }

    /// セーブ用に (キー, 個数) の並びへ。
    pub fn to_save(&self) -> Vec<Option<(String, u32)>> {
        self.slots
            .iter()
            .map(|s| s.as_ref().map(|st| (st.key.clone(), st.count)))
            .collect()
    }

    /// セーブから復元する。未知のアイテム（外したプラグインのもの）は捨てる。
    pub fn from_save(data: &[Option<(String, u32)>], registry: &ItemRegistry) -> Self {
        let mut inv = Self::default();
        for (i, entry) in data.iter().take(TOTAL_SLOTS).enumerate() {
            if let Some((key, count)) = entry {
                if *count == 0 || registry.get(key).is_none() {
                    continue;
                }
                inv.slots[i] = Some(ItemStack {
                    key: key.clone(),
                    count: *count,
                });
            }
        }
        inv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::ids;

    fn registry() -> ItemRegistry {
        ItemRegistry::build(&BlockRegistry::with_builtins())
    }

    #[test]
    fn blocks_become_placeable_items() {
        let r = registry();
        let stone = r.get("genesis:stone").expect("stone should be an item");
        assert_eq!(stone.places, Some(ids::STONE));
        assert_eq!(stone.category, ItemCategory::Block);
        assert_eq!(stone.max_stack, 64);
    }

    #[test]
    fn unbreakable_and_liquid_blocks_are_not_items() {
        let r = registry();
        assert!(r.get("genesis:bedrock").is_none(), "bedrock must not be carryable");
        assert!(r.get("genesis:water").is_none(), "water must not be carryable");
        assert!(r.get("genesis:lava").is_none());
    }

    #[test]
    fn tools_do_not_stack_and_have_power() {
        let r = registry();
        let pick = r.get("genesis:iron_pickaxe").unwrap();
        assert_eq!(pick.max_stack, 1);
        assert_eq!(pick.tool, ToolClass::Pickaxe);
        assert!(pick.tool_power > r.get("genesis:stone_pickaxe").unwrap().tool_power);
    }

    #[test]
    fn plugin_blocks_automatically_become_items() {
        let mut blocks = BlockRegistry::with_builtins();
        blocks.register(crate::blocks::BlockDef::new("mod:mithril_ore", "ミスリル鉱石", [0.5, 0.8, 0.9]));
        let r = ItemRegistry::build(&blocks);
        let item = r.get("mod:mithril_ore").expect("plugin block did not become an item");
        assert_eq!(item.display_name, "ミスリル鉱石");
        assert!(item.places.is_some());
    }

    #[test]
    fn adding_items_fills_existing_stacks_first() {
        let mut inv = Inventory::default();
        assert_eq!(inv.add("genesis:stone", 100, 64), 0);
        // 64 + 36 の2スタックになるはず。
        assert_eq!(inv.count_of("genesis:stone"), 100);
        let stacks: Vec<u32> = inv.slots.iter().flatten().map(|s| s.count).collect();
        assert_eq!(stacks, vec![64, 36]);

        // さらに 30 個。最初の空きではなく、既存の 36 のスタックが埋まる。
        inv.add("genesis:stone", 30, 64);
        let stacks: Vec<u32> = inv.slots.iter().flatten().map(|s| s.count).collect();
        assert_eq!(stacks, vec![64, 64, 2]);
    }

    #[test]
    fn a_full_inventory_reports_the_overflow() {
        let mut inv = Inventory::default();
        // 全スロットを埋める。
        let leftover = inv.add("genesis:stone", 64 * TOTAL_SLOTS as u32 + 25, 64);
        assert_eq!(leftover, 25, "overflow was silently discarded");
        assert!(inv.slots.iter().all(|s| s.is_some()));
    }

    #[test]
    fn consuming_the_last_item_clears_the_slot() {
        let mut inv = Inventory::default();
        inv.add("genesis:bread", 2, 64);
        assert_eq!(inv.consume_one(0).as_deref(), Some("genesis:bread"));
        assert_eq!(inv.consume_one(0).as_deref(), Some("genesis:bread"));
        assert!(inv.get(0).is_none(), "an emptied slot must become free");
        assert!(inv.consume_one(0).is_none());
    }

    #[test]
    fn take_one_finds_the_item_in_any_slot() {
        let mut inv = Inventory::default();
        inv.slots[5] = Some(ItemStack { key: "genesis:arrow".into(), count: 2 });
        assert!(inv.take_one("genesis:arrow"));
        assert_eq!(inv.count_of("genesis:arrow"), 1);
        assert!(inv.take_one("genesis:arrow"));
        assert!(inv.slots[5].is_none());
        assert!(!inv.take_one("genesis:arrow"), "took an arrow that was not there");
    }

    #[test]
    fn inventory_round_trips_through_a_save() {
        let r = registry();
        let mut inv = Inventory::starter();
        inv.add("genesis:diamond_ore", 5, 64);

        let saved = inv.to_save();
        let restored = Inventory::from_save(&saved, &r);
        assert_eq!(restored.slots, inv.slots);
        assert_eq!(restored.count_of("genesis:diamond_ore"), 5);
    }

    #[test]
    fn items_from_removed_plugins_are_dropped_on_load() {
        let r = registry();
        let saved = vec![
            Some(("genesis:bread".to_string(), 3)),
            Some(("gone:mystery_gem".to_string(), 9)),
            Some(("genesis:stone".to_string(), 0)),
        ];
        let inv = Inventory::from_save(&saved, &r);
        assert_eq!(inv.count_of("genesis:bread"), 3);
        assert_eq!(inv.count_of("gone:mystery_gem"), 0, "an item from a removed plugin survived");
        assert_eq!(inv.count_of("genesis:stone"), 0, "a zero-count stack was restored");
    }

    #[test]
    fn a_longer_save_than_the_inventory_does_not_panic() {
        let r = registry();
        let saved: Vec<Option<(String, u32)>> = (0..500)
            .map(|_| Some(("genesis:stone".to_string(), 1)))
            .collect();
        let inv = Inventory::from_save(&saved, &r);
        assert_eq!(inv.slots.len(), TOTAL_SLOTS);
    }

    #[test]
    fn the_starter_kit_is_usable() {
        let r = registry();
        let inv = Inventory::starter();
        assert!(inv.hotbar().iter().filter(|s| s.is_some()).count() >= 6);
        for stack in inv.slots.iter().flatten() {
            assert!(
                r.get(&stack.key).is_some(),
                "starter item '{}' is not in the registry",
                stack.key
            );
        }
    }

    #[test]
    fn every_creature_drop_exists_as_an_item() {
        let r = registry();
        for sp in crate::species::SPECIES {
            for (key, _) in sp.drops {
                assert!(r.get(key).is_some(), "{} drops unknown item '{key}'", sp.key);
            }
        }
    }
}
