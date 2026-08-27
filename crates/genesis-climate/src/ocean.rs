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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_grid_uses_default_ocean_cells() {
        let grid = OceanGrid::new(4, 5);
        assert_eq!(grid.cells.len(), 20);
        assert!(grid.cells.iter().all(|c| c.salinity_psu == 35.0));
        assert!(grid.cells.iter().all(|c| c.surface_temperature_c == 18.0));
        assert!(grid.cells.iter().all(|c| c.current_velocity == Vec2::ZERO));
    }

    #[test]
    fn land_cells_keep_zero_current() {
        let mut grid = OceanGrid::new(8, 8);
        let elevation = vec![100.0f32; 64];
        grid.simulate_thermohaline_circulation(&elevation, 0.0);
        assert!(grid.cells.iter().all(|c| c.current_velocity == Vec2::ZERO));
    }

    #[test]
    fn submerged_cells_receive_gyre_currents() {
        let mut grid = OceanGrid::new(8, 8);
        let elevation = vec![-50.0f32; 64];
        grid.simulate_thermohaline_circulation(&elevation, 0.0);
        assert!(grid.cells.iter().any(|c| c.current_velocity != Vec2::ZERO));

        // Zonal flow reverses across the equator, meridional flow across the map centre.
        let north = grid.cells[1 * 8 + 4].current_velocity;
        let south = grid.cells[6 * 8 + 4].current_velocity;
        assert!(north.x > 0.0 && south.x < 0.0);
        assert!(grid.cells[4 * 8 + 0].current_velocity.y < 0.0);
        assert!(grid.cells[4 * 8 + 7].current_velocity.y > 0.0);
    }

    #[test]
    fn sea_level_decides_which_cells_are_ocean() {
        let mut grid = OceanGrid::new(4, 4);
        let elevation = vec![10.0f32; 16];

        grid.simulate_thermohaline_circulation(&elevation, 5.0);
        assert!(grid.cells.iter().all(|c| c.current_velocity == Vec2::ZERO));

        grid.simulate_thermohaline_circulation(&elevation, 20.0);
        assert!(grid.cells.iter().any(|c| c.current_velocity != Vec2::ZERO));
    }

    #[test]
    fn circulation_is_idempotent_for_a_static_world() {
        let mut grid = OceanGrid::new(6, 6);
        let elevation = vec![-10.0f32; 36];
        grid.simulate_thermohaline_circulation(&elevation, 0.0);
        let first: Vec<Vec2> = grid.cells.iter().map(|c| c.current_velocity).collect();
        grid.simulate_thermohaline_circulation(&elevation, 0.0);
        let second: Vec<Vec2> = grid.cells.iter().map(|c| c.current_velocity).collect();
        assert_eq!(first, second);
    }
}
