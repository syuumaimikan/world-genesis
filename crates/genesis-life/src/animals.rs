use crate::genetics::GeneticCode;
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaunaAction {
    Foraging,
    Hunting,
    Drinking,
    Mating,
    Fleeing,
    Sleeping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimalSpecies {
    pub id: u32,
    pub name: String,
    pub is_carnivore: bool,
    pub genetics: GeneticCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimalIndividual {
    pub species_id: u32,
    pub position: Vec2,
    pub health: f32, // 0.0 to 100.0
    pub hunger: f32, // 0.0 (full) to 100.0 (starving)
    pub thirst: f32,
    pub age_years: f32,
    pub current_action: FaunaAction,
}

impl AnimalIndividual {
    pub fn update_metabolism(&mut self, dt_days: f32) -> bool {
        self.age_years += dt_days / 360.0;
        self.hunger = (self.hunger + dt_days * 5.0).min(100.0);
        self.thirst = (self.thirst + dt_days * 8.0).min(100.0);

        if self.hunger > 85.0 || self.thirst > 90.0 {
            self.health -= dt_days * 10.0;
        }

        self.health > 0.0
    }
}
