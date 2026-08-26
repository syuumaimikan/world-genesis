use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommodityType {
    Grain,
    Bread,
    Timber,
    IronOre,
    Tools,
    Weapons,
    LuxuryTextiles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeCargo {
    pub commodity: CommodityType,
    pub quantity: f32,
    pub purchase_price_per_unit: f32,
}
