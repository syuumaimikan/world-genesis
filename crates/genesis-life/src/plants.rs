use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FloraBiome {
    BarrenWaste,
    Tundra,
    TaigaBoreal,
    TemperateForest,
    GrasslandSavanna,
    TropicalRainforest,
    AridDesert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloraCell {
    pub biome: FloraBiome,
    pub biomass_density: f32, // kg/m^2
    pub canopy_cover: f32,    // 0.0 to 1.0
    pub soil_fertility: f32,  // 0.0 to 1.0
}

impl Default for FloraCell {
    fn default() -> Self {
        Self {
            biome: FloraBiome::GrasslandSavanna,
            biomass_density: 2.5,
            canopy_cover: 0.2,
            soil_fertility: 0.6,
        }
    }
}

impl FloraCell {
    pub fn classify_biome(temp_c: f32, precipitation_mm: f32) -> FloraBiome {
        if temp_c < -5.0 {
            if precipitation_mm < 250.0 {
                FloraBiome::BarrenWaste
            } else {
                FloraBiome::Tundra
            }
        } else if temp_c < 8.0 {
            FloraBiome::TaigaBoreal
        } else if temp_c < 20.0 {
            if precipitation_mm < 300.0 {
                FloraBiome::AridDesert
            } else if precipitation_mm < 800.0 {
                FloraBiome::GrasslandSavanna
            } else {
                FloraBiome::TemperateForest
            }
        } else {
            if precipitation_mm < 250.0 {
                FloraBiome::AridDesert
            } else if precipitation_mm < 1200.0 {
                FloraBiome::GrasslandSavanna
            } else {
                FloraBiome::TropicalRainforest
            }
        }
    }

    pub fn grow(&mut self, temp_c: f32, precipitation_mm: f32) {
        self.biome = Self::classify_biome(temp_c, precipitation_mm);
        let max_biomass = match self.biome {
            FloraBiome::BarrenWaste => 0.1,
            FloraBiome::Tundra => 1.5,
            FloraBiome::AridDesert => 0.4,
            FloraBiome::TaigaBoreal => 8.0,
            FloraBiome::GrasslandSavanna => 4.5,
            FloraBiome::TemperateForest => 14.0,
            FloraBiome::TropicalRainforest => 25.0,
        };

        let growth_potential = (self.soil_fertility * (precipitation_mm / 1000.0).clamp(0.1, 2.0)) * 0.1;
        self.biomass_density = (self.biomass_density + growth_potential).min(max_biomass);
        self.canopy_cover = (self.biomass_density / max_biomass).clamp(0.0, 1.0);
    }
}
