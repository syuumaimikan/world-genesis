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
