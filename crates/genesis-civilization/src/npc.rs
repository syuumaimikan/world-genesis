use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profession {
    Farmer,
    Blacksmith,
    Merchant,
    Soldier,
    Scholar,
    Ruler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcGoal {
    SatisfyHunger,
    EarnWages,
    Socialize,
    Revolt,
    FleeDanger,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcNeedHierarchy {
    pub hunger: f32, // 0.0 to 100.0
    pub wealth: f32,
    pub safety: f32,
    pub political_satisfaction: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcPerson {
    pub id: u64,
    pub settlement_id: u64,
    pub name: String,
    pub age: u16,
    pub profession: Profession,
    pub needs: NpcNeedHierarchy,
    pub current_goal: NpcGoal,
    pub position: Vec2,
}

impl NpcPerson {
    pub fn evaluate_utility_ai(&mut self) -> NpcGoal {
        if self.needs.hunger > 70.0 {
            self.current_goal = NpcGoal::SatisfyHunger;
        } else if self.needs.safety < 20.0 {
            self.current_goal = NpcGoal::FleeDanger;
        } else if self.needs.political_satisfaction < 15.0 && self.needs.hunger > 40.0 {
            self.current_goal = NpcGoal::Revolt;
        } else if self.needs.wealth < 50.0 {
            self.current_goal = NpcGoal::EarnWages;
        } else {
            self.current_goal = NpcGoal::Socialize;
        }
        self.current_goal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(needs: NpcNeedHierarchy) -> NpcPerson {
        NpcPerson {
            id: 1,
            settlement_id: 1,
            name: "Ryn".to_string(),
            age: 30,
            profession: Profession::Farmer,
            needs,
            current_goal: NpcGoal::Socialize,
            position: Vec2::ZERO,
        }
    }

    fn needs(hunger: f32, wealth: f32, safety: f32, politics: f32) -> NpcNeedHierarchy {
        NpcNeedHierarchy {
            hunger,
            wealth,
            safety,
            political_satisfaction: politics,
        }
    }

    #[test]
    fn hunger_outranks_every_other_need() {
        let mut p = person(needs(80.0, 0.0, 0.0, 0.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::SatisfyHunger);
        assert_eq!(p.current_goal, NpcGoal::SatisfyHunger);
    }

    #[test]
    fn danger_outranks_politics_and_wages() {
        let mut p = person(needs(50.0, 0.0, 10.0, 0.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::FleeDanger);
    }

    #[test]
    fn hungry_and_disenfranchised_people_revolt() {
        let mut p = person(needs(50.0, 0.0, 100.0, 10.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::Revolt);
    }

    #[test]
    fn well_fed_but_disenfranchised_people_only_seek_wages() {
        let mut p = person(needs(30.0, 0.0, 100.0, 10.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::EarnWages);
    }

    #[test]
    fn poor_but_safe_people_seek_wages() {
        let mut p = person(needs(10.0, 20.0, 100.0, 100.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::EarnWages);
    }

    #[test]
    fn satisfied_people_socialize() {
        let mut p = person(needs(10.0, 90.0, 100.0, 100.0));
        assert_eq!(p.evaluate_utility_ai(), NpcGoal::Socialize);
    }
}
