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

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> AnimalIndividual {
        AnimalIndividual {
            species_id: 1,
            position: Vec2::ZERO,
            health: 100.0,
            hunger: 0.0,
            thirst: 0.0,
            age_years: 0.0,
            current_action: FaunaAction::Foraging,
        }
    }

    #[test]
    fn metabolism_ages_the_animal_and_builds_up_needs() {
        let mut a = healthy();
        assert!(a.update_metabolism(1.0));
        assert!((a.age_years - 1.0 / 360.0).abs() < 1e-6);
        assert_eq!(a.hunger, 5.0);
        assert_eq!(a.thirst, 8.0);
        assert_eq!(a.health, 100.0);
    }

    #[test]
    fn hunger_and_thirst_saturate_at_their_maximum() {
        let mut a = healthy();
        a.update_metabolism(100.0);
        assert_eq!(a.hunger, 100.0);
        assert_eq!(a.thirst, 100.0);
    }

    #[test]
    fn starvation_drains_health() {
        let mut a = healthy();
        a.hunger = 90.0;
        assert!(a.update_metabolism(1.0));
        assert_eq!(a.health, 90.0);
    }

    #[test]
    fn dehydration_alone_also_drains_health() {
        let mut a = healthy();
        a.thirst = 95.0;
        a.update_metabolism(0.5);
        assert_eq!(a.health, 95.0);
    }

    #[test]
    fn metabolism_reports_death_when_health_runs_out() {
        let mut a = healthy();
        a.health = 5.0;
        a.hunger = 100.0;
        assert!(!a.update_metabolism(1.0));
        assert!(a.health <= 0.0);
    }

    #[test]
    fn species_carry_genetics_and_survive_serde() {
        let species = AnimalSpecies {
            id: 3,
            name: "Ridgeback".to_string(),
            is_carnivore: true,
            genetics: GeneticCode::default(),
        };
        let decoded: AnimalSpecies =
            serde_json::from_str(&serde_json::to_string(&species).unwrap()).unwrap();
        assert_eq!(decoded.name, "Ridgeback");
        assert!(decoded.is_carnivore);
        assert_eq!(
            decoded.genetics.longevity_years,
            GeneticCode::default().longevity_years
        );
    }
}
