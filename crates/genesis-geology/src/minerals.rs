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
