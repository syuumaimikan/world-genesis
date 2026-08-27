use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MineralType {
    IronOre,
    CopperOre,
    GoldOre,
    Coal,
    CrudeOil,
    Uraninite,
    RareEarthElements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MineralDeposit {
    pub mineral_type: MineralType,
    pub quantity_tons: f64,
    pub purity: f32,
    pub depth_meters: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MineralComposition {
    pub deposits: Vec<MineralDeposit>,
}

impl MineralComposition {
    pub fn synthesize_from_geology(
        temperature_c: f32,
        pressure_gpa: f32,
        volcanic_history: f32,
        sedimentary_age_my: f32,
    ) -> Self {
        let mut deposits = Vec::new();

        // Hydrothermal Vein: Iron & Copper
        if temperature_c > 150.0 && temperature_c < 400.0 && pressure_gpa > 0.2 {
            deposits.push(MineralDeposit {
                mineral_type: MineralType::CopperOre,
                quantity_tons: (temperature_c * 200.0) as f64,
                purity: 0.65,
                depth_meters: pressure_gpa * 800.0,
            });
            deposits.push(MineralDeposit {
                mineral_type: MineralType::IronOre,
                quantity_tons: (temperature_c * 800.0) as f64,
                purity: 0.55,
                depth_meters: pressure_gpa * 600.0,
            });
        }

        // Sedimentary Basins: Coal & Hydrocarbons
        if sedimentary_age_my > 50.0 && temperature_c < 200.0 && pressure_gpa < 0.5 {
            deposits.push(MineralDeposit {
                mineral_type: MineralType::Coal,
                quantity_tons: (sedimentary_age_my * 5000.0) as f64,
                purity: 0.85,
                depth_meters: 150.0,
            });
            if sedimentary_age_my > 120.0 {
                deposits.push(MineralDeposit {
                    mineral_type: MineralType::CrudeOil,
                    quantity_tons: (sedimentary_age_my * 2000.0) as f64,
                    purity: 0.90,
                    depth_meters: 1200.0,
                });
            }
        }

        // Volcanic Plumes: Gold & Rare Earths
        if volcanic_history > 0.7 {
            deposits.push(MineralDeposit {
                mineral_type: MineralType::GoldOre,
                quantity_tons: (volcanic_history * 50.0) as f64,
                purity: 0.30,
                depth_meters: 500.0,
            });
            deposits.push(MineralDeposit {
                mineral_type: MineralType::RareEarthElements,
                quantity_tons: (volcanic_history * 120.0) as f64,
                purity: 0.20,
                depth_meters: 800.0,
            });
        }

        Self { deposits }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(comp: &MineralComposition) -> Vec<MineralType> {
        let mut kinds: Vec<MineralType> = comp.deposits.iter().map(|d| d.mineral_type).collect();
        kinds.sort_by_key(|k| format!("{k:?}"));
        kinds
    }

    #[test]
    fn barren_geology_yields_no_deposits() {
        let comp = MineralComposition::synthesize_from_geology(20.0, 0.1, 0.0, 5.0);
        assert!(comp.deposits.is_empty());
    }

    #[test]
    fn hydrothermal_conditions_yield_copper_and_iron() {
        let comp = MineralComposition::synthesize_from_geology(300.0, 0.4, 0.0, 0.0);
        assert_eq!(
            kinds(&comp),
            vec![MineralType::CopperOre, MineralType::IronOre]
        );

        let iron = comp
            .deposits
            .iter()
            .find(|d| d.mineral_type == MineralType::IronOre)
            .unwrap();
        assert_eq!(iron.quantity_tons, 300.0 * 800.0);
        assert_eq!(iron.depth_meters, 0.4 * 600.0);
        assert_eq!(iron.purity, 0.55);
    }

    #[test]
    fn hydrothermal_needs_both_temperature_window_and_pressure() {
        assert!(
            MineralComposition::synthesize_from_geology(300.0, 0.1, 0.0, 0.0)
                .deposits
                .is_empty()
        );
        assert!(
            MineralComposition::synthesize_from_geology(500.0, 0.4, 0.0, 0.0)
                .deposits
                .is_empty()
        );
        assert!(
            MineralComposition::synthesize_from_geology(150.0, 0.4, 0.0, 0.0)
                .deposits
                .is_empty()
        );
    }

    #[test]
    fn young_sedimentary_basins_yield_coal_without_oil() {
        let comp = MineralComposition::synthesize_from_geology(100.0, 0.2, 0.0, 60.0);
        assert_eq!(kinds(&comp), vec![MineralType::Coal]);
        assert_eq!(comp.deposits[0].quantity_tons, 60.0 * 5000.0);
        assert_eq!(comp.deposits[0].depth_meters, 150.0);
    }

    #[test]
    fn ancient_sedimentary_basins_also_yield_crude_oil() {
        let comp = MineralComposition::synthesize_from_geology(100.0, 0.2, 0.0, 200.0);
        assert_eq!(kinds(&comp), vec![MineralType::Coal, MineralType::CrudeOil]);
        let oil = comp
            .deposits
            .iter()
            .find(|d| d.mineral_type == MineralType::CrudeOil)
            .unwrap();
        assert_eq!(oil.quantity_tons, 200.0 * 2000.0);
        assert_eq!(oil.depth_meters, 1200.0);
    }

    #[test]
    fn volcanic_history_yields_gold_and_rare_earths() {
        let comp = MineralComposition::synthesize_from_geology(20.0, 0.1, 0.9, 0.0);
        assert_eq!(
            kinds(&comp),
            vec![MineralType::GoldOre, MineralType::RareEarthElements]
        );
        assert!(comp.deposits.iter().all(|d| d.purity <= 0.30));
    }

    #[test]
    fn overlapping_conditions_accumulate_all_deposit_families() {
        // Hot hydrothermal veins plus an ancient basin plus volcanism.
        let comp = MineralComposition::synthesize_from_geology(180.0, 0.3, 0.95, 300.0);
        assert_eq!(
            kinds(&comp),
            vec![
                MineralType::Coal,
                MineralType::CopperOre,
                MineralType::CrudeOil,
                MineralType::GoldOre,
                MineralType::IronOre,
                MineralType::RareEarthElements,
            ]
        );
    }
}
