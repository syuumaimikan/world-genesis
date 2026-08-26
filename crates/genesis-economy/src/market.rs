use crate::goods::CommodityType;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOrder {
    pub is_buy: bool,
    pub volume: f32,
    pub target_price: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketListing {
    pub supply: f32,
    pub demand: f32,
    pub last_clearing_price: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegionalMarket {
    pub listings: HashMap<CommodityType, MarketListing>,
}

impl RegionalMarket {
    pub fn new() -> Self {
        let mut listings = HashMap::new();
        for &good in &[
            CommodityType::Grain,
            CommodityType::Timber,
            CommodityType::IronOre,
            CommodityType::Tools,
            CommodityType::Weapons,
            CommodityType::LuxuryTextiles,
        ] {
            listings.insert(
                good,
                MarketListing {
                    supply: 100.0,
                    demand: 100.0,
                    last_clearing_price: 10.0,
                },
            );
        }
        Self { listings }
    }

    pub fn clear_market(&mut self) {
        for listing in self.listings.values_mut() {
            let ratio = (listing.demand / listing.supply.max(1.0)).clamp(0.1, 10.0);
            listing.last_clearing_price = (listing.last_clearing_price * 0.8 + (listing.last_clearing_price * ratio) * 0.2)
                .clamp(0.5, 10_000.0);

            // Decay excess buffers
            listing.supply = (listing.supply * 0.5).max(1.0);
            listing.demand = (listing.demand * 0.5).max(1.0);
        }
    }

    pub fn post_production(&mut self, good: CommodityType, amount: f32) {
        if let Some(listing) = self.listings.get_mut(&good) {
            listing.supply += amount;
        }
    }

    pub fn post_consumption(&mut self, good: CommodityType, amount: f32) {
        if let Some(listing) = self.listings.get_mut(&good) {
            listing.demand += amount;
        }
    }
}
