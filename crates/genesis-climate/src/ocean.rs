use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanCell {
    pub surface_temperature_c: f32,
    pub salinity_psu: f32, // Practical Salinity Units (~35 standard)
    pub current_velocity: Vec2,
}

impl Default for OceanCell {
    fn default() -> Self {
        Self {
            surface_temperature_c: 18.0,
            salinity_psu: 35.0,
            current_velocity: Vec2::ZERO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OceanGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<OceanCell>,
}

impl OceanGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![OceanCell::default(); width * height],
        }
    }

    pub fn simulate_thermohaline_circulation(&mut self, elevation: &[f32], sea_level: f32) {
        let w = self.width;
        let h = self.height;

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if elevation[idx] < sea_level {
                    let lat = ((y as f32 / h as f32) - 0.5) * 2.0; // -1.0 to 1.0
                    // Gyre circulation currents
                    let gyre_u = -lat * 2.0;
                    let gyre_v = ((x as f32 / w as f32) - 0.5) * 1.5;
                    self.cells[idx].current_velocity = Vec2::new(gyre_u, gyre_v);
                }
            }
        }
    }
}
