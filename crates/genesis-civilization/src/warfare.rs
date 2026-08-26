use crate::diplomacy::WarState;
use crate::politics::NationState;
use crate::settlements::Settlement;
use genesis_core::time::SimTick;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmyRegiment {
    pub id: u64,
    pub nation_id: u32,
    pub soldiers_count: u32,
    pub morale: f32, // 0.0 to 1.0
    pub supply_days: f32,
    pub target_settlement_id: Option<u64>,
}

pub struct WarfareEngine;

impl WarfareEngine {
    pub fn resolve_siege_and_clash(
        _tick: SimTick,
        war: &mut WarState,
        attacker_nation: &NationState,
        defender_nation: &mut NationState,
        settlement: &mut Settlement,
        attacker_army: &mut ArmyRegiment,
    ) -> Option<u32> {
        if settlement.nation_id != defender_nation.id {
            return None;
        }

        // 戦闘計算
        let defense_power = settlement.population as f32 * 0.15 * settlement.infrastructure_health;
        let attack_power = attacker_army.soldiers_count as f32 * attacker_army.morale;

        let attacker_losses = (defense_power * 0.4) as u32;
        let defender_losses = (attack_power * 0.5) as u32;

        attacker_army.soldiers_count = attacker_army.soldiers_count.saturating_sub(attacker_losses);
        settlement.population = settlement.population.saturating_sub(defender_losses);
        settlement.infrastructure_health = (settlement.infrastructure_health - 0.2).max(0.1);

        war.casualties_attacker += attacker_losses;
        war.casualties_defender += defender_losses;

        // 都市陥落と主権移譲の判定
        if attack_power > defense_power * 2.0 && attacker_army.soldiers_count > 50 {
            let prev_owner = settlement.nation_id;
            settlement.nation_id = attacker_nation.id;
            settlement.unrest_level = 0.80;
            Some(prev_owner)
        } else {
            None
        }
    }
}
