use crate::settlements::Settlement;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechId {
    CropRotation,        // 食料収穫量 +25%
    IronSmelting,        // 工具・武器製造速度向上
    WatermillPower,      // 生産性向上
    DoubleEntryLedger,   // 税収・市場回転率向上
    PrintingPress,       // 技術伝播速度 3倍
    SteamPistonEngine,   // 産業革命コア
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettlementTechKnowledge {
    pub tech_progress: HashMap<TechId, f32>, // 0.0 to 100.0 (100で完全習得)
}

impl SettlementTechKnowledge {
    pub fn has_unlocked(&self, tech: TechId) -> bool {
        self.tech_progress.get(&tech).cloned().unwrap_or(0.0) >= 100.0
    }

    pub fn diffuse_from_neighbor(&mut self, neighbor_techs: &SettlementTechKnowledge, diffusion_rate: f32) {
        for (&tech, &neighbor_progress) in &neighbor_techs.tech_progress {
            if neighbor_progress >= 100.0 {
                let current = self.tech_progress.entry(tech).or_insert(0.0);
                if *current < 100.0 {
                    *current = (*current + diffusion_rate).min(100.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knowledge(entries: &[(TechId, f32)]) -> SettlementTechKnowledge {
        SettlementTechKnowledge {
            tech_progress: entries.iter().copied().collect(),
        }
    }

    #[test]
    fn techs_are_unlocked_only_at_full_progress() {
        let k = knowledge(&[(TechId::CropRotation, 100.0), (TechId::IronSmelting, 99.9)]);
        assert!(k.has_unlocked(TechId::CropRotation));
        assert!(!k.has_unlocked(TechId::IronSmelting));
        assert!(!k.has_unlocked(TechId::PrintingPress));
    }

    #[test]
    fn mastered_neighbour_techs_diffuse_at_the_given_rate() {
        let neighbor = knowledge(&[(TechId::WatermillPower, 100.0)]);
        let mut local = SettlementTechKnowledge::default();

        local.diffuse_from_neighbor(&neighbor, 30.0);
        assert_eq!(local.tech_progress[&TechId::WatermillPower], 30.0);

        local.diffuse_from_neighbor(&neighbor, 30.0);
        assert_eq!(local.tech_progress[&TechId::WatermillPower], 60.0);
    }

    #[test]
    fn diffusion_saturates_at_full_mastery() {
        let neighbor = knowledge(&[(TechId::DoubleEntryLedger, 100.0)]);
        let mut local = knowledge(&[(TechId::DoubleEntryLedger, 95.0)]);

        local.diffuse_from_neighbor(&neighbor, 30.0);
        assert_eq!(local.tech_progress[&TechId::DoubleEntryLedger], 100.0);
        assert!(local.has_unlocked(TechId::DoubleEntryLedger));

        local.diffuse_from_neighbor(&neighbor, 30.0);
        assert_eq!(local.tech_progress[&TechId::DoubleEntryLedger], 100.0);
    }

    #[test]
    fn partially_researched_neighbour_techs_do_not_diffuse() {
        let neighbor = knowledge(&[(TechId::SteamPistonEngine, 99.0)]);
        let mut local = SettlementTechKnowledge::default();
        local.diffuse_from_neighbor(&neighbor, 50.0);
        assert!(local.tech_progress.is_empty());
    }
}
