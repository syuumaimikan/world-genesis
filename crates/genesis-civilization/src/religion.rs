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

#[cfg(test)]
mod tests {
    use super::*;

    fn religion(zealotry: f32) -> Religion {
        Religion {
            id: 1,
            name: "Solar Covenant".to_string(),
            founded_tick: SimTick(0),
            holy_city_settlement_id: 9,
            zealotry,
            tolerance: 0.5,
            adherents_count: 10_000,
        }
    }

    #[test]
    fn calm_and_moderate_faiths_do_not_schism() {
        assert!(!religion(0.5).check_schism_risk(0.1));
    }

    #[test]
    fn fanatical_faiths_schism_under_social_unrest() {
        assert!(religion(0.9).check_schism_risk(0.6));
    }

    #[test]
    fn unrest_alone_cannot_split_a_moderate_faith() {
        assert!(!religion(0.2).check_schism_risk(1.0));
    }

    #[test]
    fn schism_risk_is_monotonic_in_unrest() {
        let r = religion(0.8);
        assert!(!r.check_schism_risk(0.2));
        assert!(r.check_schism_risk(0.8));
    }
}
