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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn biome_classification_covers_every_climate_band() {
        use FloraBiome::*;
        assert_eq!(FloraCell::classify_biome(-20.0, 100.0), BarrenWaste);
        assert_eq!(FloraCell::classify_biome(-20.0, 400.0), Tundra);
        assert_eq!(FloraCell::classify_biome(0.0, 400.0), TaigaBoreal);
        assert_eq!(FloraCell::classify_biome(15.0, 200.0), AridDesert);
        assert_eq!(FloraCell::classify_biome(15.0, 500.0), GrasslandSavanna);
        assert_eq!(FloraCell::classify_biome(15.0, 1_000.0), TemperateForest);
        assert_eq!(FloraCell::classify_biome(28.0, 100.0), AridDesert);
        assert_eq!(FloraCell::classify_biome(28.0, 800.0), GrasslandSavanna);
        assert_eq!(FloraCell::classify_biome(28.0, 3_000.0), TropicalRainforest);
    }

    #[test]
    fn growth_reclassifies_the_biome_from_the_local_climate() {
        let mut cell = FloraCell::default();
        cell.grow(28.0, 2_500.0);
        assert_eq!(cell.biome, FloraBiome::TropicalRainforest);
    }

    #[test]
    fn biomass_accumulates_up_to_the_biome_ceiling() {
        let mut cell = FloraCell {
            soil_fertility: 1.0,
            ..Default::default()
        };
        for _ in 0..1_000 {
            cell.grow(15.0, 1_500.0);
        }
        assert_eq!(cell.biome, FloraBiome::TemperateForest);
        assert_eq!(cell.biomass_density, 14.0);
        assert_eq!(cell.canopy_cover, 1.0);
    }

    #[test]
    fn switching_to_a_poorer_biome_caps_existing_biomass() {
        let mut cell = FloraCell {
            biomass_density: 20.0,
            ..Default::default()
        };
        cell.grow(-20.0, 100.0);
        assert_eq!(cell.biome, FloraBiome::BarrenWaste);
        assert_eq!(cell.biomass_density, 0.1);
        assert_eq!(cell.canopy_cover, 1.0);
    }

    #[test]
    fn barren_soil_does_not_gain_biomass() {
        let mut cell = FloraCell {
            soil_fertility: 0.0,
            biomass_density: 1.0,
            ..Default::default()
        };
        cell.grow(15.0, 500.0);
        assert_eq!(cell.biomass_density, 1.0);
    }

    #[test]
    fn canopy_cover_tracks_the_biomass_fraction() {
        let mut cell = FloraCell {
            biomass_density: 2.25,
            soil_fertility: 0.0,
            ..Default::default()
        };
        cell.grow(15.0, 500.0);
        assert_eq!(cell.biome, FloraBiome::GrasslandSavanna);
        assert_eq!(cell.canopy_cover, 0.5);
    }
}
