use genesis_civilization::Settlement;
use genesis_economy::CommodityType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingType {
    Farmstead,     // 食料生産ボーナス
    SmithyWorkshop,// 工具・武器生産効率向上
    Marketplace,   // 交易・商品回転率向上
    FortressWall,  // 防御力・治安向上
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingStructure {
    pub id: u64,
    pub building_type: BuildingType,
    pub settlement_id: u64,
    pub level: u8,
    pub structural_integrity: f32, // 0.0 to 1.0
}

pub struct ConstructionService;

impl ConstructionService {
    pub fn get_building_cost(b_type: BuildingType) -> HashMap<CommodityType, f32> {
        let mut cost = HashMap::new();
        match b_type {
            BuildingType::Farmstead => {
                cost.insert(CommodityType::Timber, 20.0);
                cost.insert(CommodityType::Tools, 5.0);
            }
            BuildingType::SmithyWorkshop => {
                cost.insert(CommodityType::Timber, 35.0);
                cost.insert(CommodityType::IronOre, 15.0);
                cost.insert(CommodityType::Tools, 10.0);
            }
            BuildingType::Marketplace => {
                cost.insert(CommodityType::Timber, 50.0);
                cost.insert(CommodityType::Tools, 8.0);
            }
            BuildingType::FortressWall => {
                cost.insert(CommodityType::Timber, 80.0);
                cost.insert(CommodityType::IronOre, 30.0);
                cost.insert(CommodityType::Tools, 20.0);
            }
        }
        cost
    }

    pub fn construct_in_settlement(
        b_type: BuildingType,
        settlement: &mut Settlement,
        available_resources: &mut HashMap<CommodityType, f32>,
        player_coins: &mut f64,
    ) -> Result<BuildingStructure, String> {
        let cost = Self::get_building_cost(b_type);
        let monetary_fee = 50.0;

        if *player_coins < monetary_fee {
            return Err("所持金が不足しています。".to_string());
        }

        for (item, qty) in &cost {
            let stock = available_resources.get(item).cloned().unwrap_or(0.0);
            if stock < *qty {
                return Err(format!("資源 {:?} が不足しています (必要: {:.1})", item, qty));
            }
        }

        // 支払いと資源消費
        *player_coins -= monetary_fee;
        for (item, qty) in &cost {
            let stock = available_resources.get_mut(item).unwrap();
            *stock -= *qty;
        }

        // 都市インフラと治安の向上
        settlement.infrastructure_health = (settlement.infrastructure_health + 0.05).min(1.0);
        settlement.unrest_level = (settlement.unrest_level - 0.08).max(0.0);

        Ok(BuildingStructure {
            id: 1000 + settlement.id,
            building_type: b_type,
            settlement_id: settlement.id,
            level: 1,
            structural_integrity: 1.0,
        })
    }
}
