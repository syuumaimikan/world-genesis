use genesis_core::time::SimTick;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Religion {
    pub id: u32,
    pub name: String,
    pub founded_tick: SimTick,
    pub holy_city_settlement_id: u64,
    pub zealotry: f32,  // 0.0 to 1.0 (狂信度)
    pub tolerance: f32, // 0.0 to 1.0 (他教への寛容度)
    pub adherents_count: u64,
}

impl Religion {
    pub fn check_schism_risk(&self, avg_unrest: f32) -> bool {
        // 狂信度が高く、社会不安が高い場合に宗教分裂・異端派閥発生
        (self.zealotry * 0.6 + avg_unrest * 0.4) > 0.75
    }
}
