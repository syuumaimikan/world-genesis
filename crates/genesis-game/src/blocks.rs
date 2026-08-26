//! データ駆動ブロックレジストリ。
//!
//! 全てのボクセル種別はここに登録された `BlockDef` として存在する。組み込みブロックは
//! 固定IDを持ち、プラグイン（mods/*.json）が定義したブロックは実行時に末尾へ追加される。
//! ワールド生成・メッシュ生成・採掘・ドロップ判定は全てこのレジストリを参照するため、
//! プラグイン側は Rust を書かずに新しい鉱石・木材・植生を世界へ持ち込める。

use bevy::prelude::Resource;
use std::collections::HashMap;

/// ボクセル1つを表す識別子。0 は必ず空気。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BlockId(pub u16);

impl BlockId {
    #[inline]
    pub const fn is_air(self) -> bool {
        self.0 == 0
    }
}

/// 描画上の分類。メッシャがどのバッファへ面を追加するかを決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderClass {
    /// 不透明な立方体。隣接面はカリングされる。
    Opaque,
    /// 半透明の立方体（水・氷・ガラス）。同種同士は面を張らない。
    Translucent,
    /// 十字型のスプライト（草・花・苗）。当たり判定なし。
    Cross,
}

/// 採掘に適した道具の種別。適合しない道具では採掘速度が落ちる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolClass {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
}

#[derive(Debug, Clone)]
pub struct BlockDef {
    /// `genesis:stone` のような名前空間付き識別子。セーブとプラグインの参照キー。
    pub key: String,
    pub display_name: String,
    /// 上面 / 側面 / 底面の基本色（頂点カラーとして焼き込まれる）。
    pub color_top: [f32; 3],
    pub color_side: [f32; 3],
    pub color_bottom: [f32; 3],
    pub render: RenderClass,
    /// 立体としての当たり判定を持つか。
    pub solid: bool,
    /// 流体か（プレイヤーが泳げる）。
    pub liquid: bool,
    /// 採掘硬度（秒）。負値は破壊不能。
    pub hardness: f32,
    pub tool: ToolClass,
    /// 発光量 0..15。松明・溶岩などが正値を持つ。
    pub light: u8,
    /// 破壊時に得られるアイテムキー（None ならブロック自身）。
    pub drop_key: Option<String>,
    /// 頂点カラーへ加える疑似テクスチャノイズの強さ。
    pub grain: f32,
}

impl BlockDef {
    pub fn new(key: &str, name: &str, color: [f32; 3]) -> Self {
        Self {
            key: key.to_string(),
            display_name: name.to_string(),
            color_top: color,
            color_side: color,
            color_bottom: color,
            render: RenderClass::Opaque,
            solid: true,
            liquid: false,
            hardness: 1.0,
            tool: ToolClass::None,
            light: 0,
            drop_key: None,
            grain: 0.04,
        }
    }

    pub fn faces(mut self, top: [f32; 3], side: [f32; 3], bottom: [f32; 3]) -> Self {
        self.color_top = top;
        self.color_side = side;
        self.color_bottom = bottom;
        self
    }

    pub fn render(mut self, r: RenderClass) -> Self {
        self.render = r;
        if matches!(r, RenderClass::Cross) {
            self.solid = false;
        }
        self
    }

    pub fn liquid(mut self) -> Self {
        self.liquid = true;
        self.solid = false;
        self.render = RenderClass::Translucent;
        self.hardness = -1.0;
        self
    }

    pub fn hardness(mut self, h: f32, tool: ToolClass) -> Self {
        self.hardness = h;
        self.tool = tool;
        self
    }

    pub fn light(mut self, l: u8) -> Self {
        self.light = l;
        self
    }

    pub fn grain(mut self, g: f32) -> Self {
        self.grain = g;
        self
    }

    pub fn drops(mut self, key: &str) -> Self {
        self.drop_key = Some(key.to_string());
        self
    }
}

/// 組み込みブロックの固定ID。ワールド生成コードはこの定数を使う。
pub mod ids {
    use super::BlockId;
    pub const AIR: BlockId = BlockId(0);
    pub const STONE: BlockId = BlockId(1);
    pub const GRANITE: BlockId = BlockId(2);
    pub const DIORITE: BlockId = BlockId(3);
    pub const BASALT: BlockId = BlockId(4);
    pub const LIMESTONE: BlockId = BlockId(5);
    pub const SANDSTONE: BlockId = BlockId(6);
    pub const DIRT: BlockId = BlockId(7);
    pub const COARSE_DIRT: BlockId = BlockId(8);
    pub const GRASS: BlockId = BlockId(9);
    pub const PODZOL: BlockId = BlockId(10);
    pub const MYCELIUM: BlockId = BlockId(11);
    pub const SAND: BlockId = BlockId(12);
    pub const RED_SAND: BlockId = BlockId(13);
    pub const GRAVEL: BlockId = BlockId(14);
    pub const CLAY: BlockId = BlockId(15);
    pub const SNOW: BlockId = BlockId(16);
    pub const ICE: BlockId = BlockId(17);
    pub const PACKED_ICE: BlockId = BlockId(18);
    pub const WATER: BlockId = BlockId(19);
    pub const LAVA: BlockId = BlockId(20);
    pub const BEDROCK: BlockId = BlockId(21);
    pub const TERRACOTTA: BlockId = BlockId(22);
    pub const OBSIDIAN: BlockId = BlockId(23);
    pub const TUFF: BlockId = BlockId(24);
    pub const MARBLE: BlockId = BlockId(25);
    pub const PEAT: BlockId = BlockId(26);

    // 樹木
    pub const OAK_LOG: BlockId = BlockId(30);
    pub const BIRCH_LOG: BlockId = BlockId(31);
    pub const SPRUCE_LOG: BlockId = BlockId(32);
    pub const JUNGLE_LOG: BlockId = BlockId(33);
    pub const ACACIA_LOG: BlockId = BlockId(34);
    pub const MANGROVE_LOG: BlockId = BlockId(35);
    pub const PALM_LOG: BlockId = BlockId(36);
    pub const OAK_LEAVES: BlockId = BlockId(37);
    pub const BIRCH_LEAVES: BlockId = BlockId(38);
    pub const SPRUCE_LEAVES: BlockId = BlockId(39);
    pub const JUNGLE_LEAVES: BlockId = BlockId(40);
    pub const ACACIA_LEAVES: BlockId = BlockId(41);
    pub const MANGROVE_LEAVES: BlockId = BlockId(42);
    pub const PALM_LEAVES: BlockId = BlockId(43);
    pub const CHERRY_LOG: BlockId = BlockId(44);
    pub const CHERRY_LEAVES: BlockId = BlockId(45);
    pub const DEAD_LOG: BlockId = BlockId(46);

    // 鉱石
    pub const COAL_ORE: BlockId = BlockId(50);
    pub const IRON_ORE: BlockId = BlockId(51);
    pub const COPPER_ORE: BlockId = BlockId(52);
    pub const TIN_ORE: BlockId = BlockId(53);
    pub const ZINC_ORE: BlockId = BlockId(54);
    pub const LEAD_ORE: BlockId = BlockId(55);
    pub const SILVER_ORE: BlockId = BlockId(56);
    pub const GOLD_ORE: BlockId = BlockId(57);
    pub const LAPIS_ORE: BlockId = BlockId(58);
    pub const EMERALD_ORE: BlockId = BlockId(59);
    pub const DIAMOND_ORE: BlockId = BlockId(60);
    pub const QUARTZ_ORE: BlockId = BlockId(61);
    pub const SULFUR_ORE: BlockId = BlockId(62);
    pub const SALT_ORE: BlockId = BlockId(63);
    pub const URANIUM_ORE: BlockId = BlockId(64);
    pub const OIL_SHALE: BlockId = BlockId(65);
    pub const AMBER_ORE: BlockId = BlockId(66);

    // 植生（十字スプライト）
    pub const TALL_GRASS: BlockId = BlockId(70);
    pub const FERN: BlockId = BlockId(71);
    pub const DEAD_BUSH: BlockId = BlockId(72);
    pub const FLOWER_RED: BlockId = BlockId(73);
    pub const FLOWER_YELLOW: BlockId = BlockId(74);
    pub const FLOWER_BLUE: BlockId = BlockId(75);
    pub const FLOWER_WHITE: BlockId = BlockId(76);
    pub const FLOWER_PURPLE: BlockId = BlockId(77);
    pub const MUSHROOM_RED: BlockId = BlockId(78);
    pub const MUSHROOM_BROWN: BlockId = BlockId(79);
    pub const CACTUS: BlockId = BlockId(80);
    pub const REEDS: BlockId = BlockId(81);
    pub const BAMBOO: BlockId = BlockId(82);
    pub const BERRY_BUSH: BlockId = BlockId(83);
    pub const WHEAT_CROP: BlockId = BlockId(84);
    pub const LILYPAD: BlockId = BlockId(85);
    pub const CORAL: BlockId = BlockId(86);
    pub const SEAGRASS: BlockId = BlockId(87);
    pub const VINE: BlockId = BlockId(88);
    pub const SAPLING: BlockId = BlockId(89);

    // 建材（集落生成・プレイヤー建築）
    pub const OAK_PLANKS: BlockId = BlockId(95);
    pub const SPRUCE_PLANKS: BlockId = BlockId(96);
    pub const COBBLESTONE: BlockId = BlockId(97);
    pub const STONE_BRICK: BlockId = BlockId(98);
    pub const BRICK: BlockId = BlockId(99);
    pub const THATCH: BlockId = BlockId(100);
    pub const GLASS: BlockId = BlockId(101);
    pub const TORCH: BlockId = BlockId(102);
    pub const PLASTER: BlockId = BlockId(103);
    pub const ROOF_TILE: BlockId = BlockId(104);
    pub const PATH: BlockId = BlockId(105);
    pub const FARMLAND: BlockId = BlockId(106);
    pub const DOOR: BlockId = BlockId(107);
    pub const FENCE: BlockId = BlockId(108);
    pub const CAMPFIRE: BlockId = BlockId(109);
    pub const LANTERN: BlockId = BlockId(110);
    pub const WELL_STONE: BlockId = BlockId(111);
    pub const MARBLE_COLUMN: BlockId = BlockId(112);

    /// 組み込みブロックが占有するID空間の上限（プラグインはこれ以降を使う）。
    pub const BUILTIN_COUNT: u16 = 120;
}

#[derive(Resource)]
pub struct BlockRegistry {
    defs: Vec<BlockDef>,
    by_key: HashMap<String, BlockId>,
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl BlockRegistry {
    pub fn with_builtins() -> Self {
        use ids::*;
        // 未使用スロットは空気で埋めておき、固定IDへ上書きしていく。
        let placeholder = BlockDef::new("genesis:air", "空気", [0.0, 0.0, 0.0]);
        let mut defs = vec![placeholder; BUILTIN_COUNT as usize];

        macro_rules! put {
            ($id:expr, $def:expr) => {
                defs[$id.0 as usize] = $def;
            };
        }

        let mut air = BlockDef::new("genesis:air", "空気", [0.0; 3]);
        air.solid = false;
        air.render = RenderClass::Translucent;
        air.hardness = -1.0;
        put!(AIR, air);

        // --- 岩石・土壌 ---
        put!(STONE, BlockDef::new("genesis:stone", "石", [0.47, 0.47, 0.49]).hardness(1.5, ToolClass::Pickaxe).drops("genesis:cobblestone"));
        put!(GRANITE, BlockDef::new("genesis:granite", "花崗岩", [0.63, 0.46, 0.40]).hardness(1.6, ToolClass::Pickaxe));
        put!(DIORITE, BlockDef::new("genesis:diorite", "閃緑岩", [0.72, 0.72, 0.72]).hardness(1.6, ToolClass::Pickaxe));
        put!(BASALT, BlockDef::new("genesis:basalt", "玄武岩", [0.24, 0.23, 0.26]).hardness(1.8, ToolClass::Pickaxe));
        put!(LIMESTONE, BlockDef::new("genesis:limestone", "石灰岩", [0.78, 0.76, 0.66]).hardness(1.2, ToolClass::Pickaxe));
        put!(SANDSTONE, BlockDef::new("genesis:sandstone", "砂岩", [0.85, 0.79, 0.58]).hardness(1.1, ToolClass::Pickaxe));
        put!(MARBLE, BlockDef::new("genesis:marble", "大理石", [0.90, 0.89, 0.87]).hardness(1.6, ToolClass::Pickaxe));
        put!(TUFF, BlockDef::new("genesis:tuff", "凝灰岩", [0.42, 0.44, 0.40]).hardness(1.4, ToolClass::Pickaxe));
        put!(TERRACOTTA, BlockDef::new("genesis:terracotta", "赤土", [0.70, 0.40, 0.26]).hardness(1.3, ToolClass::Pickaxe));
        put!(OBSIDIAN, BlockDef::new("genesis:obsidian", "黒曜石", [0.10, 0.08, 0.16]).hardness(9.0, ToolClass::Pickaxe));
        put!(BEDROCK, {
            let mut b = BlockDef::new("genesis:bedrock", "岩盤", [0.12, 0.12, 0.14]);
            b.hardness = -1.0;
            b.grain = 0.16;
            b
        });
        put!(DIRT, BlockDef::new("genesis:dirt", "土", [0.42, 0.30, 0.18]).hardness(0.5, ToolClass::Shovel));
        put!(COARSE_DIRT, BlockDef::new("genesis:coarse_dirt", "粗い土", [0.38, 0.27, 0.16]).hardness(0.5, ToolClass::Shovel));
        put!(PEAT, BlockDef::new("genesis:peat", "泥炭", [0.24, 0.19, 0.13]).hardness(0.6, ToolClass::Shovel));
        put!(GRASS, BlockDef::new("genesis:grass_block", "草ブロック", [0.36, 0.62, 0.24])
            .faces([0.36, 0.66, 0.24], [0.40, 0.44, 0.20], [0.42, 0.30, 0.18])
            .hardness(0.6, ToolClass::Shovel).drops("genesis:dirt"));
        put!(PODZOL, BlockDef::new("genesis:podzol", "ポドゾル", [0.35, 0.24, 0.11])
            .faces([0.35, 0.26, 0.10], [0.38, 0.28, 0.15], [0.42, 0.30, 0.18])
            .hardness(0.6, ToolClass::Shovel));
        put!(MYCELIUM, BlockDef::new("genesis:mycelium", "菌糸", [0.47, 0.40, 0.45])
            .faces([0.50, 0.42, 0.48], [0.44, 0.36, 0.38], [0.42, 0.30, 0.18])
            .hardness(0.6, ToolClass::Shovel));
        put!(SAND, BlockDef::new("genesis:sand", "砂", [0.87, 0.82, 0.60]).hardness(0.5, ToolClass::Shovel).grain(0.06));
        put!(RED_SAND, BlockDef::new("genesis:red_sand", "赤い砂", [0.78, 0.47, 0.24]).hardness(0.5, ToolClass::Shovel).grain(0.06));
        put!(GRAVEL, BlockDef::new("genesis:gravel", "砂利", [0.52, 0.50, 0.49]).hardness(0.6, ToolClass::Shovel).grain(0.11));
        put!(CLAY, BlockDef::new("genesis:clay", "粘土", [0.62, 0.65, 0.70]).hardness(0.6, ToolClass::Shovel));
        put!(SNOW, BlockDef::new("genesis:snow", "雪", [0.95, 0.96, 0.99]).hardness(0.3, ToolClass::Shovel).grain(0.02));
        put!(ICE, BlockDef::new("genesis:ice", "氷", [0.63, 0.79, 0.95]).render(RenderClass::Translucent).hardness(0.5, ToolClass::Pickaxe));
        put!(PACKED_ICE, BlockDef::new("genesis:packed_ice", "氷塊", [0.55, 0.74, 0.93]).hardness(0.8, ToolClass::Pickaxe));
        put!(WATER, BlockDef::new("genesis:water", "水", [0.17, 0.36, 0.70]).liquid());
        put!(LAVA, BlockDef::new("genesis:lava", "溶岩", [0.95, 0.42, 0.08]).liquid().light(15));

        // --- 樹木 ---
        let logs: [(BlockId, &str, &str, [f32; 3], [f32; 3]); 9] = [
            (OAK_LOG, "genesis:oak_log", "オークの原木", [0.45, 0.33, 0.19], [0.55, 0.44, 0.28]),
            (BIRCH_LOG, "genesis:birch_log", "白樺の原木", [0.85, 0.83, 0.76], [0.72, 0.70, 0.62]),
            (SPRUCE_LOG, "genesis:spruce_log", "トウヒの原木", [0.31, 0.22, 0.12], [0.40, 0.30, 0.18]),
            (JUNGLE_LOG, "genesis:jungle_log", "ジャングルの原木", [0.52, 0.39, 0.24], [0.42, 0.34, 0.22]),
            (ACACIA_LOG, "genesis:acacia_log", "アカシアの原木", [0.60, 0.36, 0.20], [0.48, 0.30, 0.17]),
            (MANGROVE_LOG, "genesis:mangrove_log", "マングローブの原木", [0.44, 0.22, 0.20], [0.36, 0.19, 0.17]),
            (PALM_LOG, "genesis:palm_log", "ヤシの原木", [0.58, 0.46, 0.30], [0.50, 0.40, 0.26]),
            (CHERRY_LOG, "genesis:cherry_log", "桜の原木", [0.42, 0.28, 0.28], [0.36, 0.24, 0.24]),
            (DEAD_LOG, "genesis:dead_log", "枯木", [0.42, 0.40, 0.36], [0.36, 0.34, 0.30]),
        ];
        for (id, key, name, top, side) in logs {
            put!(id, BlockDef::new(key, name, side).faces(top, side, top).hardness(2.0, ToolClass::Axe).grain(0.06));
        }

        let leaves: [(BlockId, &str, &str, [f32; 3]); 8] = [
            (OAK_LEAVES, "genesis:oak_leaves", "オークの葉", [0.24, 0.55, 0.20]),
            (BIRCH_LEAVES, "genesis:birch_leaves", "白樺の葉", [0.44, 0.66, 0.30]),
            (SPRUCE_LEAVES, "genesis:spruce_leaves", "トウヒの葉", [0.15, 0.38, 0.24]),
            (JUNGLE_LEAVES, "genesis:jungle_leaves", "ジャングルの葉", [0.19, 0.52, 0.14]),
            (ACACIA_LEAVES, "genesis:acacia_leaves", "アカシアの葉", [0.44, 0.60, 0.22]),
            (MANGROVE_LEAVES, "genesis:mangrove_leaves", "マングローブの葉", [0.22, 0.48, 0.28]),
            (PALM_LEAVES, "genesis:palm_leaves", "ヤシの葉", [0.30, 0.60, 0.26]),
            (CHERRY_LEAVES, "genesis:cherry_leaves", "桜の花", [0.94, 0.66, 0.78]),
        ];
        for (id, key, name, c) in leaves {
            put!(id, BlockDef::new(key, name, c).hardness(0.2, ToolClass::Axe).grain(0.11));
        }

        // --- 鉱石 ---
        let ores: [(BlockId, &str, &str, [f32; 3], f32); 17] = [
            (COAL_ORE, "genesis:coal_ore", "石炭鉱石", [0.24, 0.23, 0.24], 3.0),
            (IRON_ORE, "genesis:iron_ore", "鉄鉱石", [0.66, 0.55, 0.47], 3.0),
            (COPPER_ORE, "genesis:copper_ore", "銅鉱石", [0.55, 0.60, 0.48], 3.0),
            (TIN_ORE, "genesis:tin_ore", "錫鉱石", [0.58, 0.60, 0.63], 3.0),
            (ZINC_ORE, "genesis:zinc_ore", "亜鉛鉱石", [0.60, 0.62, 0.58], 3.0),
            (LEAD_ORE, "genesis:lead_ore", "鉛鉱石", [0.42, 0.42, 0.48], 3.0),
            (SILVER_ORE, "genesis:silver_ore", "銀鉱石", [0.72, 0.74, 0.78], 3.5),
            (GOLD_ORE, "genesis:gold_ore", "金鉱石", [0.78, 0.68, 0.32], 3.5),
            (LAPIS_ORE, "genesis:lapis_ore", "ラピスラズリ鉱石", [0.30, 0.40, 0.70], 3.5),
            (EMERALD_ORE, "genesis:emerald_ore", "エメラルド鉱石", [0.30, 0.70, 0.44], 4.0),
            (DIAMOND_ORE, "genesis:diamond_ore", "ダイヤモンド鉱石", [0.44, 0.78, 0.80], 4.5),
            (QUARTZ_ORE, "genesis:quartz_ore", "石英鉱石", [0.82, 0.80, 0.76], 2.5),
            (SULFUR_ORE, "genesis:sulfur_ore", "硫黄鉱石", [0.78, 0.74, 0.28], 2.5),
            (SALT_ORE, "genesis:salt_ore", "岩塩", [0.88, 0.86, 0.84], 2.0),
            (URANIUM_ORE, "genesis:uranium_ore", "ウラン鉱石", [0.36, 0.60, 0.34], 5.0),
            (OIL_SHALE, "genesis:oil_shale", "油母頁岩", [0.28, 0.26, 0.22], 2.5),
            (AMBER_ORE, "genesis:amber_ore", "琥珀鉱脈", [0.82, 0.52, 0.16], 2.0),
        ];
        for (id, key, name, c, hard) in ores {
            put!(id, BlockDef::new(key, name, c).hardness(hard, ToolClass::Pickaxe).grain(0.16));
        }

        // --- 植生（十字スプライト） ---
        let crosses: [(BlockId, &str, &str, [f32; 3]); 20] = [
            (TALL_GRASS, "genesis:tall_grass", "草", [0.40, 0.70, 0.26]),
            (FERN, "genesis:fern", "シダ", [0.30, 0.56, 0.28]),
            (DEAD_BUSH, "genesis:dead_bush", "枯れ木の茂み", [0.55, 0.42, 0.22]),
            (FLOWER_RED, "genesis:flower_red", "赤い花", [0.86, 0.22, 0.20]),
            (FLOWER_YELLOW, "genesis:flower_yellow", "黄色い花", [0.94, 0.84, 0.24]),
            (FLOWER_BLUE, "genesis:flower_blue", "青い花", [0.34, 0.44, 0.88]),
            (FLOWER_WHITE, "genesis:flower_white", "白い花", [0.94, 0.94, 0.92]),
            (FLOWER_PURPLE, "genesis:flower_purple", "紫の花", [0.62, 0.34, 0.80]),
            (MUSHROOM_RED, "genesis:mushroom_red", "赤キノコ", [0.82, 0.20, 0.18]),
            (MUSHROOM_BROWN, "genesis:mushroom_brown", "茶キノコ", [0.60, 0.44, 0.30]),
            (CACTUS, "genesis:cactus", "サボテン", [0.28, 0.54, 0.24]),
            (REEDS, "genesis:reeds", "葦", [0.52, 0.72, 0.36]),
            (BAMBOO, "genesis:bamboo", "竹", [0.56, 0.74, 0.28]),
            (BERRY_BUSH, "genesis:berry_bush", "ベリーの茂み", [0.30, 0.46, 0.22]),
            (WHEAT_CROP, "genesis:wheat_crop", "小麦", [0.80, 0.74, 0.30]),
            (LILYPAD, "genesis:lilypad", "睡蓮の葉", [0.24, 0.52, 0.26]),
            (CORAL, "genesis:coral", "サンゴ", [0.92, 0.40, 0.56]),
            (SEAGRASS, "genesis:seagrass", "海草", [0.22, 0.54, 0.40]),
            (VINE, "genesis:vine", "ツタ", [0.26, 0.50, 0.22]),
            (SAPLING, "genesis:sapling", "苗木", [0.34, 0.60, 0.26]),
        ];
        for (id, key, name, c) in crosses {
            put!(id, BlockDef::new(key, name, c).render(RenderClass::Cross).hardness(0.05, ToolClass::None).grain(0.08));
        }

        // --- 建材 ---
        put!(OAK_PLANKS, BlockDef::new("genesis:oak_planks", "オークの板材", [0.66, 0.52, 0.33]).hardness(1.8, ToolClass::Axe));
        put!(SPRUCE_PLANKS, BlockDef::new("genesis:spruce_planks", "トウヒの板材", [0.50, 0.38, 0.24]).hardness(1.8, ToolClass::Axe));
        put!(COBBLESTONE, BlockDef::new("genesis:cobblestone", "丸石", [0.44, 0.44, 0.46]).hardness(1.8, ToolClass::Pickaxe).grain(0.12));
        put!(STONE_BRICK, BlockDef::new("genesis:stone_brick", "石レンガ", [0.52, 0.52, 0.53]).hardness(1.9, ToolClass::Pickaxe).grain(0.05));
        put!(BRICK, BlockDef::new("genesis:brick", "レンガ", [0.62, 0.32, 0.26]).hardness(1.9, ToolClass::Pickaxe).grain(0.05));
        put!(THATCH, BlockDef::new("genesis:thatch", "藁ぶき", [0.76, 0.63, 0.28]).hardness(0.6, ToolClass::None).grain(0.10));
        put!(GLASS, BlockDef::new("genesis:glass", "ガラス", [0.78, 0.88, 0.92]).render(RenderClass::Translucent).hardness(0.3, ToolClass::None));
        put!(TORCH, BlockDef::new("genesis:torch", "松明", [0.96, 0.78, 0.34]).render(RenderClass::Cross).hardness(0.05, ToolClass::None).light(14));
        put!(LANTERN, BlockDef::new("genesis:lantern", "ランタン", [0.94, 0.80, 0.42]).hardness(0.5, ToolClass::None).light(15));
        put!(CAMPFIRE, BlockDef::new("genesis:campfire", "焚き火", [0.72, 0.36, 0.16]).hardness(0.6, ToolClass::Axe).light(13));
        put!(PLASTER, BlockDef::new("genesis:plaster", "漆喰", [0.88, 0.85, 0.78]).hardness(1.2, ToolClass::Pickaxe));
        put!(ROOF_TILE, BlockDef::new("genesis:roof_tile", "屋根瓦", [0.45, 0.24, 0.22]).hardness(1.5, ToolClass::Pickaxe));
        put!(PATH, BlockDef::new("genesis:path", "小道", [0.55, 0.45, 0.30])
            .faces([0.58, 0.48, 0.32], [0.45, 0.34, 0.20], [0.42, 0.30, 0.18])
            .hardness(0.5, ToolClass::Shovel));
        put!(FARMLAND, BlockDef::new("genesis:farmland", "耕地", [0.34, 0.22, 0.12])
            .faces([0.32, 0.20, 0.10], [0.40, 0.28, 0.16], [0.42, 0.30, 0.18])
            .hardness(0.5, ToolClass::Shovel));
        put!(DOOR, BlockDef::new("genesis:door", "扉", [0.52, 0.36, 0.20]).hardness(1.5, ToolClass::Axe));
        put!(FENCE, BlockDef::new("genesis:fence", "柵", [0.56, 0.42, 0.24]).render(RenderClass::Cross).hardness(1.5, ToolClass::Axe));
        put!(WELL_STONE, BlockDef::new("genesis:well_stone", "井戸石", [0.48, 0.48, 0.50]).hardness(2.0, ToolClass::Pickaxe));
        put!(MARBLE_COLUMN, BlockDef::new("genesis:marble_column", "大理石柱", [0.92, 0.91, 0.88]).hardness(2.0, ToolClass::Pickaxe));

        let mut by_key = HashMap::new();
        for (i, d) in defs.iter().enumerate() {
            by_key.entry(d.key.clone()).or_insert(BlockId(i as u16));
        }

        Self { defs, by_key }
    }

    #[inline]
    pub fn get(&self, id: BlockId) -> &BlockDef {
        self.defs.get(id.0 as usize).unwrap_or(&self.defs[0])
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn id_of(&self, key: &str) -> Option<BlockId> {
        self.by_key.get(key).copied()
    }

    /// プラグインからのブロック追加。既存キーなら上書き（プラグインによる調整を許可）。
    pub fn register(&mut self, def: BlockDef) -> BlockId {
        if let Some(&existing) = self.by_key.get(&def.key) {
            self.defs[existing.0 as usize] = def;
            return existing;
        }
        let id = BlockId(self.defs.len() as u16);
        self.by_key.insert(def.key.clone(), id);
        self.defs.push(def);
        id
    }

    /// 組み込み状態へ戻す（プラグイン設定を変更してワールドを作り直すときに使う）。
    pub fn reset_to_builtins(&mut self) {
        *self = Self::with_builtins();
    }

    /// メッシャ用の高速参照テーブル。ワーカースレッドへ move できる。
    pub fn snapshot(&self) -> BlockLookup {
        BlockLookup {
            entries: self
                .defs
                .iter()
                .map(|d| BlockLookupEntry {
                    color_top: d.color_top,
                    color_side: d.color_side,
                    color_bottom: d.color_bottom,
                    render: d.render,
                    solid: d.solid,
                    liquid: d.liquid,
                    grain: d.grain,
                    light: d.light,
                })
                .collect(),
        }
    }
}

/// メッシュ生成スレッドへ渡す軽量スナップショット。
#[derive(Clone)]
pub struct BlockLookup {
    pub entries: Vec<BlockLookupEntry>,
}

#[derive(Clone, Copy)]
pub struct BlockLookupEntry {
    pub color_top: [f32; 3],
    pub color_side: [f32; 3],
    pub color_bottom: [f32; 3],
    pub render: RenderClass,
    pub solid: bool,
    pub liquid: bool,
    pub grain: f32,
    pub light: u8,
}

impl BlockLookup {
    #[inline]
    pub fn entry(&self, id: BlockId) -> BlockLookupEntry {
        self.entries[(id.0 as usize).min(self.entries.len() - 1)]
    }

    #[inline]
    pub fn is_opaque(&self, id: BlockId) -> bool {
        !id.is_air() && matches!(self.entry(id).render, RenderClass::Opaque)
    }

    #[inline]
    pub fn is_solid(&self, id: BlockId) -> bool {
        !id.is_air() && self.entry(id).solid
    }

    #[inline]
    pub fn is_liquid(&self, id: BlockId) -> bool {
        !id.is_air() && self.entry(id).liquid
    }

    #[inline]
    pub fn is_cross(&self, id: BlockId) -> bool {
        !id.is_air() && matches!(self.entry(id).render, RenderClass::Cross)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids_resolve_to_their_own_keys() {
        let reg = BlockRegistry::with_builtins();
        assert_eq!(reg.id_of("genesis:stone"), Some(ids::STONE));
        assert_eq!(reg.id_of("genesis:diamond_ore"), Some(ids::DIAMOND_ORE));
        assert_eq!(reg.id_of("genesis:water"), Some(ids::WATER));
        assert_eq!(reg.get(ids::GRASS).display_name, "草ブロック");
    }

    #[test]
    fn water_is_liquid_and_not_solid() {
        let reg = BlockRegistry::with_builtins();
        let look = reg.snapshot();
        assert!(look.is_liquid(ids::WATER));
        assert!(!look.is_solid(ids::WATER));
        assert!(look.is_solid(ids::STONE));
        assert!(!look.is_solid(ids::TALL_GRASS));
    }

    #[test]
    fn plugin_blocks_get_ids_after_builtins() {
        let mut reg = BlockRegistry::with_builtins();
        let id = reg.register(BlockDef::new("testmod:mithril_ore", "ミスリル鉱石", [0.5, 0.8, 0.9]));
        assert!(id.0 >= ids::BUILTIN_COUNT);
        assert_eq!(reg.id_of("testmod:mithril_ore"), Some(id));
        // 同じキーを再登録しても ID は増えない（上書き）。
        let again = reg.register(BlockDef::new("testmod:mithril_ore", "改", [0.1, 0.1, 0.1]));
        assert_eq!(again, id);
    }
}
