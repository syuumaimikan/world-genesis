use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TraitAllele {
    ThermalResistance,
    DroughtTolerance,
    ApexPredation,
    FastMetabolism,
    Camouflage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneticCode {
    pub alleles: Vec<TraitAllele>,
    pub base_fertility_rate: f32,
    pub longevity_years: f32,
    pub body_mass_kg: f32,
}

impl Default for GeneticCode {
    fn default() -> Self {
        Self {
            alleles: vec![TraitAllele::Camouflage],
            base_fertility_rate: 0.35,
            longevity_years: 15.0,
            body_mass_kg: 45.0,
        }
    }
}

impl GeneticCode {
    pub fn mutate(&mut self, mutation_rate: f32, rng_val: f32) {
        if rng_val < mutation_rate {
            self.base_fertility_rate = (self.base_fertility_rate * 1.05).clamp(0.05, 0.95);
            self.body_mass_kg = (self.body_mass_kg * 0.98).clamp(1.0, 5000.0);
        }
    }
}
