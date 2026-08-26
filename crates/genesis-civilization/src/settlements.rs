use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SettlementTier {
    Camp,
    Village,
    Town,
    City,
    Metropolis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settlement {
    pub id: u64,
    pub name: String,
    pub position: Vec2,
    pub nation_id: u32,
    pub tier: SettlementTier,
    pub population: u32,
    pub food_stockpile_kg: f32,
    pub infrastructure_health: f32, // 0.0 to 1.0
    pub unrest_level: f32,          // 0.0 to 1.0
}

impl Settlement {
    pub fn new(id: u64, name: impl Into<String>, position: Vec2, nation_id: u32) -> Self {
        Self {
            id,
            name: name.into(),
            position,
            nation_id,
            tier: SettlementTier::Camp,
            population: 40,
            food_stockpile_kg: 2000.0,
            infrastructure_health: 1.0,
            unrest_level: 0.0,
        }
    }

    pub fn step_demographics(&mut self, local_harvest_kg: f32) {
        self.food_stockpile_kg += local_harvest_kg;
        let required_food = self.population as f32 * 0.8 * 30.0; // Monthly need

        if self.food_stockpile_kg >= required_food {
            self.food_stockpile_kg -= required_food;
            let births = (self.population as f32 * 0.01) as u32;
            self.population = self.population.saturating_add(births);
            self.unrest_level = (self.unrest_level - 0.02).max(0.0);
        } else {
            let starvation_deficit = required_food - self.food_stockpile_kg;
            self.food_stockpile_kg = 0.0;
            let deaths = ((starvation_deficit / 24.0).ceil() as u32).min(self.population);
            self.population = self.population.saturating_sub(deaths);
            self.unrest_level = (self.unrest_level + 0.15).min(1.0);
        }

        // Tier Progression
        self.tier = match self.population {
            0..=100 => SettlementTier::Camp,
            101..=800 => SettlementTier::Village,
            801..=5_000 => SettlementTier::Town,
            5_001..=30_000 => SettlementTier::City,
            _ => SettlementTier::Metropolis,
        };
    }
}
