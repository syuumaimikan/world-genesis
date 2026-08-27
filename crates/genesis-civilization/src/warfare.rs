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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diplomacy::CasusBelli;
    use crate::dynasty::PersonId;
    use crate::politics::{GovernmentForm, NationState, SuccessionLaw};
    use glam::Vec2;

    fn nation(id: u32) -> NationState {
        NationState {
            id,
            name: format!("Nation {id}"),
            government: GovernmentForm::FeudalMonarchy,
            succession: SuccessionLaw::Primogeniture,
            sovereign_ruler_id: PersonId(1),
            treasury_balance: 0.0,
            legitimacy_pct: 100.0,
            tax_rate: 0.1,
            is_at_war: true,
            capital_settlement_id: 1,
        }
    }

    fn war() -> WarState {
        WarState {
            attacker_id: 1,
            defender_id: 2,
            casus_belli: CasusBelli::TerritorialExpansion,
            casualties_attacker: 0,
            casualties_defender: 0,
            war_exhaustion: 0.0,
        }
    }

    fn army(soldiers: u32, morale: f32) -> ArmyRegiment {
        ArmyRegiment {
            id: 1,
            nation_id: 1,
            soldiers_count: soldiers,
            morale,
            supply_days: 30.0,
            target_settlement_id: Some(1),
        }
    }

    fn defended_town(nation_id: u32, population: u32) -> Settlement {
        let mut s = Settlement::new(1, "Karth", Vec2::ZERO, nation_id);
        s.population = population;
        s
    }

    #[test]
    fn sieges_against_a_settlement_of_another_nation_are_ignored() {
        let attacker = nation(1);
        let mut defender = nation(2);
        let mut settlement = defended_town(3, 1_000);
        let mut regiment = army(5_000, 1.0);
        let mut w = war();

        let result = WarfareEngine::resolve_siege_and_clash(
            SimTick(1),
            &mut w,
            &attacker,
            &mut defender,
            &mut settlement,
            &mut regiment,
        );

        assert!(result.is_none());
        assert_eq!(settlement.population, 1_000);
        assert_eq!(regiment.soldiers_count, 5_000);
        assert_eq!(w.casualties_attacker, 0);
    }

    #[test]
    fn an_overwhelming_army_captures_the_settlement() {
        let attacker = nation(1);
        let mut defender = nation(2);
        let mut settlement = defended_town(2, 1_000);
        let mut regiment = army(5_000, 1.0);
        let mut w = war();

        let previous_owner = WarfareEngine::resolve_siege_and_clash(
            SimTick(1),
            &mut w,
            &attacker,
            &mut defender,
            &mut settlement,
            &mut regiment,
        );

        assert_eq!(previous_owner, Some(2));
        assert_eq!(settlement.nation_id, 1);
        assert!((settlement.unrest_level - 0.80).abs() < 1e-6);
        assert!((settlement.infrastructure_health - 0.8).abs() < 1e-6);
        assert_eq!(regiment.soldiers_count, 5_000 - 60);
        assert_eq!(w.casualties_attacker, 60);
        assert_eq!(w.casualties_defender, 2_500);
        assert_eq!(
            settlement.population, 0,
            "losses cannot exceed the population"
        );
    }

    #[test]
    fn a_weak_army_bleeds_the_defenders_without_taking_the_city() {
        let attacker = nation(1);
        let mut defender = nation(2);
        let mut settlement = defended_town(2, 10_000);
        let mut regiment = army(600, 0.5);
        let mut w = war();

        let result = WarfareEngine::resolve_siege_and_clash(
            SimTick(1),
            &mut w,
            &attacker,
            &mut defender,
            &mut settlement,
            &mut regiment,
        );

        assert!(result.is_none());
        assert_eq!(settlement.nation_id, 2);
        assert_eq!(settlement.population, 10_000 - 150);
        assert_eq!(regiment.soldiers_count, 0, "the regiment is wiped out");
        assert_eq!(w.casualties_defender, 150);
    }

    #[test]
    fn infrastructure_damage_bottoms_out_after_repeated_sieges() {
        let attacker = nation(1);
        let mut defender = nation(2);
        let mut settlement = defended_town(2, 100_000);
        let mut regiment = army(100, 0.1);
        let mut w = war();

        for _ in 0..10 {
            regiment.soldiers_count = 100;
            WarfareEngine::resolve_siege_and_clash(
                SimTick(1),
                &mut w,
                &attacker,
                &mut defender,
                &mut settlement,
                &mut regiment,
            );
        }

        assert!((settlement.infrastructure_health - 0.1).abs() < 1e-6);
        assert_eq!(settlement.nation_id, 2);
    }
}
