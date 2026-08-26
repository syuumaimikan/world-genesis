use genesis_civilization::npc::{NpcNeedHierarchy, Profession};
use genesis_economy::{CommodityType, RegionalMarket};
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCharacter {
    pub person_id: u64,
    pub name: String,
    pub current_settlement_id: u64,
    pub world_position: Vec2,
    pub profession: Profession,
    pub coin_purse: f64,
    pub inventory: std::collections::HashMap<CommodityType, f32>,
    pub needs: NpcNeedHierarchy,
    pub is_alive: bool,
}

impl PlayerCharacter {
    pub fn new_citizen(id: u64, name: impl Into<String>, settlement_id: u64, pos: Vec2) -> Self {
        let mut inventory = std::collections::HashMap::new();
        inventory.insert(CommodityType::Bread, 5.0);

        Self {
            person_id: id,
            name: name.into(),
            current_settlement_id: settlement_id,
            world_position: pos,
            profession: Profession::Merchant,
            coin_purse: 250.0,
            inventory,
            needs: NpcNeedHierarchy {
                hunger: 10.0,
                wealth: 50.0,
                safety: 90.0,
                political_satisfaction: 80.0,
            },
            is_alive: true,
        }
    }

    /// 労働による収入と資源獲得
    pub fn perform_work(&mut self) -> String {
        self.needs.hunger = (self.needs.hunger + 15.0).min(100.0);

        match self.profession {
            Profession::Farmer => {
                let grain = self.inventory.entry(CommodityType::Grain).or_insert(0.0);
                *grain += 20.0;
                self.coin_purse += 12.0;
                "農作業を行い、小麦 20.0kg を収穫し、日当 12.0 コインを得ました。".to_string()
            }
            Profession::Blacksmith => {
                let tools = self.inventory.entry(CommodityType::Tools).or_insert(0.0);
                *tools += 2.0;
                self.coin_purse += 25.0;
                "鍛冶場で工具 2.0個 を鍛造し、賃金 25.0 コインを得ました。".to_string()
            }
            Profession::Merchant => {
                self.coin_purse += 35.0;
                "露店で取引を行い、売買マージン 35.0 コインを得ました。".to_string()
            }
            _ => {
                self.coin_purse += 15.0;
                "労働を行い、日当 15.0 コインを得ました。".to_string()
            }
        }
    }

    /// 食事をとって空腹を回復
    pub fn consume_meal(&mut self) -> Result<String, String> {
        let bread = self.inventory.entry(CommodityType::Bread).or_insert(0.0);
        if *bread >= 1.0 {
            *bread -= 1.0;
            self.needs.hunger = (self.needs.hunger - 40.0).max(0.0);
            Ok("パンを食べて空腹を満たしました。".to_string())
        } else {
            Err("所持品にパンがありません！".to_string())
        }
    }

    /// 市場で商品の購入
    pub fn buy_commodity(&mut self, market: &mut RegionalMarket, good: CommodityType, amount: f32) -> Result<String, String> {
        let price = market.listings.get(&good).map(|l| l.last_clearing_price).unwrap_or(10.0);
        let total_cost = (price * amount) as f64;

        if self.coin_purse < total_cost {
            return Err("所持金が足りません。".to_string());
        }

        self.coin_purse -= total_cost;
        let item = self.inventory.entry(good).or_insert(0.0);
        *item += amount;

        market.post_consumption(good, amount);
        Ok(format!("{:?} を {:.1}個 購入しました (費用: {:.2} コイン)", good, amount, total_cost))
    }

    /// 市場で商品の売却
    pub fn sell_commodity(&mut self, market: &mut RegionalMarket, good: CommodityType, amount: f32) -> Result<String, String> {
        let item = self.inventory.entry(good).or_insert(0.0);
        if *item < amount {
            return Err("売却する商品が手元にありません。".to_string());
        }

        let price = market.listings.get(&good).map(|l| l.last_clearing_price).unwrap_or(10.0);
        let revenue = (price * amount * 0.95) as f64; // 取引手数料5%

        *item -= amount;
        self.coin_purse += revenue;

        market.post_production(good, amount);
        Ok(format!("{:?} を {:.1}個 売却しました (受取: {:.2} コイン)", good, amount, revenue))
    }
}
