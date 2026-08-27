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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_code_is_a_camouflaged_mid_sized_animal() {
        let g = GeneticCode::default();
        assert_eq!(g.alleles, vec![TraitAllele::Camouflage]);
        assert_eq!(g.base_fertility_rate, 0.35);
        assert_eq!(g.longevity_years, 15.0);
        assert_eq!(g.body_mass_kg, 45.0);
    }

    #[test]
    fn mutation_fires_when_the_roll_is_below_the_rate() {
        let mut g = GeneticCode::default();
        g.mutate(0.5, 0.1);
        assert!((g.base_fertility_rate - 0.35 * 1.05).abs() < 1e-6);
        assert!((g.body_mass_kg - 45.0 * 0.98).abs() < 1e-4);
    }

    #[test]
    fn mutation_is_skipped_when_the_roll_is_at_or_above_the_rate() {
        let mut g = GeneticCode::default();
        g.mutate(0.5, 0.5);
        assert_eq!(g.base_fertility_rate, 0.35);
        assert_eq!(g.body_mass_kg, 45.0);
    }

    #[test]
    fn repeated_mutation_stays_inside_biological_limits() {
        let mut g = GeneticCode::default();
        for _ in 0..500 {
            g.mutate(1.0, 0.0);
        }
        assert!(g.base_fertility_rate <= 0.95);
        assert!(g.body_mass_kg >= 1.0);
    }

    #[test]
    fn genetic_code_roundtrips_through_json() {
        let mut g = GeneticCode::default();
        g.alleles = vec![TraitAllele::ApexPredation, TraitAllele::FastMetabolism];
        let decoded: GeneticCode =
            serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(decoded.alleles, g.alleles);
    }
}
