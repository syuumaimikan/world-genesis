use crate::plants::FloraCell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrophicLevelMetrics {
    pub total_primary_biomass_tons: f64,
    pub herbivore_population: u64,
    pub carnivore_population: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemGrid {
    pub width: usize,
    pub height: usize,
    pub flora: Vec<FloraCell>,
    pub herbivore_density: Vec<f32>,
    pub carnivore_density: Vec<f32>,
}

impl EcosystemGrid {
    pub fn new(width: usize, height: usize) -> Self {
        let size = width * height;
        Self {
            width,
            height,
            flora: vec![FloraCell::default(); size],
            herbivore_density: vec![10.0; size],
            carnivore_density: vec![1.0; size],
        }
    }

    pub fn step_lotka_volterra(
        &mut self,
        temperatures: &[f32],
        precipitations: &[f32],
        dt_years: f32,
    ) -> TrophicLevelMetrics {
        let mut total_biomass = 0.0f64;
        let mut total_herb = 0u64;
        let mut total_carn = 0u64;

        let flora = &mut self.flora;
        let herb = &mut self.herbivore_density;
        let carn = &mut self.carnivore_density;

        for i in 0..flora.len() {
            flora[i].grow(temperatures[i], precipitations[i]);

            let h = herb[i];
            let c = carn[i];
            let plant_food = flora[i].biomass_density;

            // Lotka-Volterra system with environmental carrying capacity
            let herb_birth = 0.4 * h * (plant_food / (plant_food + 2.0));
            let herb_predation = 0.08 * h * c;
            let herb_death = 0.15 * h;

            let carn_birth = 0.05 * h * c;
            let carn_death = 0.25 * c;

            herb[i] = (h + (herb_birth - herb_predation - herb_death) * dt_years).max(0.0);
            carn[i] = (c + (carn_birth - carn_death) * dt_years).max(0.0);

            // Deplete grazed vegetation
            let consumed_biomass = (herb_predation * 0.5 * dt_years).min(flora[i].biomass_density);
            flora[i].biomass_density -= consumed_biomass;

            total_biomass += flora[i].biomass_density as f64;
            total_herb += herb[i] as u64;
            total_carn += carn[i] as u64;
        }

        TrophicLevelMetrics {
            total_primary_biomass_tons: total_biomass,
            herbivore_population: total_herb,
            carnivore_population: total_carn,
        }
    }
}
