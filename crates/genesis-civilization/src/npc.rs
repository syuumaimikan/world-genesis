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
