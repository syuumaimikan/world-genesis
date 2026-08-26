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
