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

#[cfg(test)]
mod tests {
    use super::*;

    fn stock(entries: &[(CommodityType, f32)]) -> HashMap<CommodityType, f32> {
        entries.iter().copied().collect()
    }

    #[test]
    fn single_input_recipe_consumes_inputs_and_yields_output() {
        let engine = ProductionChainEngine::default();
        let mut s = stock(&[(CommodityType::Grain, 6.0)]);

        let produced = engine.execute_production_cycle(CommodityType::Bread, &mut s, 10.0);

        assert_eq!(produced, 4.0);
        assert_eq!(s[&CommodityType::Grain], 0.0);
        assert_eq!(s[&CommodityType::Bread], 4.0);
    }

    #[test]
    fn batch_count_is_capped_by_the_scarcest_input() {
        let engine = ProductionChainEngine::default();
        let mut s = stock(&[(CommodityType::Timber, 10.0), (CommodityType::IronOre, 4.0)]);

        let produced = engine.execute_production_cycle(CommodityType::Tools, &mut s, 100.0);

        assert_eq!(produced, 2.0);
        assert_eq!(s[&CommodityType::IronOre], 0.0);
        assert_eq!(s[&CommodityType::Timber], 8.0);
    }

    #[test]
    fn max_batches_limits_production_even_with_plentiful_inputs() {
        let engine = ProductionChainEngine::default();
        let mut s = stock(&[
            (CommodityType::IronOre, 300.0),
            (CommodityType::Timber, 300.0),
        ]);

        let produced = engine.execute_production_cycle(CommodityType::Weapons, &mut s, 3.0);

        assert_eq!(produced, 3.0);
        assert_eq!(s[&CommodityType::IronOre], 291.0);
        assert_eq!(s[&CommodityType::Timber], 298.5);
    }

    #[test]
    fn missing_inputs_produce_nothing_and_leave_stock_untouched() {
        let engine = ProductionChainEngine::default();
        let mut s = stock(&[(CommodityType::Timber, 5.0)]);

        let produced = engine.execute_production_cycle(CommodityType::Tools, &mut s, 5.0);

        assert_eq!(produced, 0.0);
        assert_eq!(s[&CommodityType::Timber], 5.0);
        assert_eq!(s[&CommodityType::IronOre], 0.0);
    }

    #[test]
    fn goods_without_a_recipe_cannot_be_produced() {
        let engine = ProductionChainEngine::default();
        let mut s = stock(&[(CommodityType::Grain, 100.0)]);

        let produced = engine.execute_production_cycle(CommodityType::LuxuryTextiles, &mut s, 10.0);

        assert_eq!(produced, 0.0);
        assert_eq!(s.len(), 1);
    }
}
