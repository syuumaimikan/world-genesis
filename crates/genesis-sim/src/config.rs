use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldGenesisConfig {
    pub seed: u64,
    pub map_width: usize,
    pub map_height: usize,
    pub plate_count: usize,
    pub sea_level: f32,
    pub solar_luminosity: f32,
    pub axial_tilt_deg: f32,
}

impl Default for WorldGenesisConfig {
    fn default() -> Self {
        Self {
            seed: 0xC0FFEE_BEEF,
            map_width: 128,
            map_height: 128,
            plate_count: 12,
            sea_level: 0.0,
            solar_luminosity: 1.0,
            axial_tilt_deg: 23.44,
        }
    }
}
