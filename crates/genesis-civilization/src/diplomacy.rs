use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CasusBelli {
    TerritorialExpansion,
    ResourceScarcity,
    IdeologicalHolyWar,
    DynasticClaim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarState {
    pub attacker_id: u32,
    pub defender_id: u32,
    pub casus_belli: CasusBelli,
    pub casualties_attacker: u32,
    pub casualties_defender: u32,
    pub war_exhaustion: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub nation_a: u32,
    pub nation_b: u32,
    pub opinion: i32, // -100 to +100
    pub active_war: Option<WarState>,
}
