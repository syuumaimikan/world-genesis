use crate::goods::CommodityType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub inputs: Vec<(CommodityType, f32)>,
    pub output: (CommodityType, f32),
    pub labor_cost: f32,
}

pub struct ProductionChainEngine {
    recipes: HashMap<CommodityType, Recipe>,
}

impl Default for ProductionChainEngine {
    fn default() -> Self {
        let mut recipes = HashMap::new();

        // 農業連鎖: 小麦 -> パン
        recipes.insert(
            CommodityType::Bread,
            Recipe {
                inputs: vec![(CommodityType::Grain, 1.5)],
                output: (CommodityType::Bread, 1.0),
                labor_cost: 2.0,
            },
        );

        // 冶金連鎖: 原木 + 鉄鉱石 -> 工具
        recipes.insert(
            CommodityType::Tools,
            Recipe {
                inputs: vec![(CommodityType::Timber, 1.0), (CommodityType::IronOre, 2.0)],
                output: (CommodityType::Tools, 1.0),
                labor_cost: 5.0,
            },
        );

        // 軍需連鎖: 鉄鉱石 + 原木 -> 武器
        recipes.insert(
            CommodityType::Weapons,
            Recipe {
                inputs: vec![(CommodityType::IronOre, 3.0), (CommodityType::Timber, 0.5)],
                output: (CommodityType::Weapons, 1.0),
                labor_cost: 8.0,
            },
        );

        Self { recipes }
    }
}

impl ProductionChainEngine {
    pub fn execute_production_cycle(
        &self,
        target_good: CommodityType,
        available_stock: &mut HashMap<CommodityType, f32>,
        max_batches: f32,
    ) -> f32 {
        if let Some(recipe) = self.recipes.get(&target_good) {
            let mut batches = max_batches;
            for (input_good, input_qty) in &recipe.inputs {
                let current_stock = available_stock.entry(*input_good).or_insert(0.0);
                batches = batches.min(*current_stock / input_qty);
            }

            if batches <= 0.001 {
                return 0.0;
            }

            // 原材料消費
            for (input_good, input_qty) in &recipe.inputs {
                let stock = available_stock.get_mut(input_good).unwrap();
                *stock -= input_qty * batches;
            }

            // 完成品生産
            let (out_good, out_qty) = recipe.output;
            let final_produced = out_qty * batches;
            let output_stock = available_stock.entry(out_good).or_insert(0.0);
            *output_stock += final_produced;

            final_produced
        } else {
            0.0
        }
    }
}
