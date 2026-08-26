use genesis_core::math::sanitize_f32;
use glam::Vec2;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetaryState {
    pub axial_tilt_deg: f32,
    pub solar_luminosity: f32,
    pub orbital_progress: f32,
    pub sea_level: f32,
}

impl Default for PlanetaryState {
    fn default() -> Self {
        Self {
            axial_tilt_deg: 23.44,
            solar_luminosity: 1.0,
            orbital_progress: 0.0,
            sea_level: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateParameters {
    pub lapse_rate_c_per_km: f32,
    pub greenhouse_factor: f32,
}

impl Default for ClimateParameters {
    fn default() -> Self {
        Self {
            lapse_rate_c_per_km: 6.5,
            greenhouse_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmosphericCell {
    pub temperature_c: f32,
    pub humidity: f32,
    pub precipitation_rate: f32,
    pub wind_vector: Vec2,
    pub surface_pressure: f32,
}

impl Default for AtmosphericCell {
    fn default() -> Self {
        Self {
            temperature_c: 15.0,
            humidity: 0.5,
            precipitation_rate: 800.0,
            wind_vector: Vec2::ZERO,
            surface_pressure: 1013.25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmosphericGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<AtmosphericCell>,
}

impl AtmosphericGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![AtmosphericCell::default(); width * height],
        }
    }

    pub fn update_climate(
        &mut self,
        elevation: &[f32],
        planet: &PlanetaryState,
        params: &ClimateParameters,
    ) {
        let w = self.width;
        let h = self.height;

        let season_declination = (planet.orbital_progress * std::f32::consts::TAU).sin()
            * planet.axial_tilt_deg.to_radians();

        self.cells.par_iter_mut().enumerate().for_each(|(idx, cell)| {
            let y = idx / w;
            let lat_rad = ((y as f32 / h as f32) - 0.5) * std::f32::consts::PI;

            let zenith_angle = (lat_rad - season_declination).abs();
            let insolation = zenith_angle.cos().max(0.0) * planet.solar_luminosity * 1361.0;

            let base_temp = 32.0 * insolation / 1361.0 - 15.0;

            let alt_km = (elevation[idx] - planet.sea_level).max(0.0) / 1000.0;
            let temp_c = base_temp - (alt_km * params.lapse_rate_c_per_km);
            cell.temperature_c = sanitize_f32(temp_c, -80.0, 65.0);

            let lat_deg = lat_rad.to_degrees().abs();
            let (u_wind, v_wind) = if lat_deg < 30.0 {
                (-3.0, if lat_rad > 0.0 { -1.5 } else { 1.5 })
            } else if lat_deg < 60.0 {
                (5.0, if lat_rad > 0.0 { 2.0 } else { -2.0 })
            } else {
                (-2.0, if lat_rad > 0.0 { -1.0 } else { 1.0 })
            };

            cell.wind_vector = Vec2::new(u_wind, v_wind);

            let sat_vapor = 6.11 * 10.0f32.powf((7.5 * cell.temperature_c) / (237.3 + cell.temperature_c));

            if cell.humidity > 0.75 {
                let excess = (cell.humidity - 0.75) * sat_vapor * 45.0;
                cell.precipitation_rate = excess.max(0.0);
                cell.humidity = (cell.humidity - 0.05).max(0.1);
            } else {
                cell.precipitation_rate = 0.0;
            }
        });
    }
}
