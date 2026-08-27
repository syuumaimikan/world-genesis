use crate::dynasty::{DemographyLedger, PersonId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernmentForm {
    TribalChieftaincy,
    FeudalMonarchy,
    MerchantRepublic,
    AbsoluteAutocracy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuccessionLaw {
    Primogeniture,
    ElectiveCouncil,
    MilitaryDictate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessionCrisis {
    pub claimant_a: PersonId,
    pub claimant_b: PersonId,
    pub faction_a_power: f32,
    pub faction_b_power: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NationState {
    pub id: u32,
    pub name: String,
    pub government: GovernmentForm,
    pub succession: SuccessionLaw,
    pub sovereign_ruler_id: PersonId,
    pub treasury_balance: f64,
    pub legitimacy_pct: f32,
    pub tax_rate: f32,
    pub is_at_war: bool,
    pub capital_settlement_id: u64,
}

impl NationState {
    pub fn handle_ruler_mortality(
        &mut self,
        demography: &mut DemographyLedger,
        death_tick: genesis_core::time::SimTick,
    ) -> Result<PersonId, SuccessionCrisis> {
        demography.record_death(self.sovereign_ruler_id, death_tick);

        let maybe_successor = match self.succession {
            SuccessionLaw::Primogeniture => {
                demography.resolve_primogeniture_successor(self.sovereign_ruler_id)
            }
            _ => None,
        };

        if let Some(successor_id) = maybe_successor {
            self.sovereign_ruler_id = successor_id;
            self.legitimacy_pct = (self.legitimacy_pct * 0.85).max(40.0);
            Ok(successor_id)
        } else {
            // 正当後継者不在による継承危機と派閥分裂の発生
            let claimant_a = self.sovereign_ruler_id; // 旧王血統
            let claimant_b = PersonId(self.sovereign_ruler_id.0 + 1); // 僭称軍閥

            Err(SuccessionCrisis {
                claimant_a,
                claimant_b,
                faction_a_power: 50.0,
                faction_b_power: 50.0,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynasty::DemographyLedger;
    use genesis_core::time::SimTick;

    fn nation(succession: SuccessionLaw, ruler: PersonId) -> NationState {
        NationState {
            id: 1,
            name: "Valenor".to_string(),
            government: GovernmentForm::FeudalMonarchy,
            succession,
            sovereign_ruler_id: ruler,
            treasury_balance: 1_000.0,
            legitimacy_pct: 100.0,
            tax_rate: 0.1,
            is_at_war: false,
            capital_settlement_id: 1,
        }
    }

    #[test]
    fn a_living_heir_inherits_the_throne_with_reduced_legitimacy() {
        let mut ledger = DemographyLedger::new();
        let ruler = ledger.birth_child(SimTick(0), "Ruler", 1, None, None);
        let heir = ledger.birth_child(SimTick(20), "Heir", 1, Some(ruler), None);
        let mut state = nation(SuccessionLaw::Primogeniture, ruler);

        let successor = state
            .handle_ruler_mortality(&mut ledger, SimTick(900))
            .unwrap();

        assert_eq!(successor, heir);
        assert_eq!(state.sovereign_ruler_id, heir);
        assert_eq!(state.legitimacy_pct, 85.0);
        assert!(!ledger.people[&ruler].is_alive());
    }

    #[test]
    fn legitimacy_never_falls_below_the_floor() {
        let mut ledger = DemographyLedger::new();
        let ruler = ledger.birth_child(SimTick(0), "Ruler", 1, None, None);
        ledger.birth_child(SimTick(20), "Heir", 1, Some(ruler), None);
        let mut state = nation(SuccessionLaw::Primogeniture, ruler);
        state.legitimacy_pct = 41.0;

        state
            .handle_ruler_mortality(&mut ledger, SimTick(900))
            .unwrap();
        assert_eq!(state.legitimacy_pct, 40.0);
    }

    #[test]
    fn an_heirless_monarchy_falls_into_a_succession_crisis() {
        let mut ledger = DemographyLedger::new();
        let ruler = ledger.birth_child(SimTick(0), "Ruler", 1, None, None);
        let mut state = nation(SuccessionLaw::Primogeniture, ruler);

        let crisis = state
            .handle_ruler_mortality(&mut ledger, SimTick(900))
            .unwrap_err();

        assert_eq!(crisis.claimant_a, ruler);
        assert_eq!(crisis.claimant_b, PersonId(ruler.0 + 1));
        assert_eq!(crisis.faction_a_power, crisis.faction_b_power);
        assert_eq!(
            state.sovereign_ruler_id, ruler,
            "the throne stays contested"
        );
    }

    #[test]
    fn non_hereditary_succession_laws_always_trigger_a_crisis() {
        for law in [
            SuccessionLaw::ElectiveCouncil,
            SuccessionLaw::MilitaryDictate,
        ] {
            let mut ledger = DemographyLedger::new();
            let ruler = ledger.birth_child(SimTick(0), "Ruler", 1, None, None);
            ledger.birth_child(SimTick(20), "Heir", 1, Some(ruler), None);
            let mut state = nation(law, ruler);

            assert!(state
                .handle_ruler_mortality(&mut ledger, SimTick(900))
                .is_err());
        }
    }
}
