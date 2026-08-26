use genesis_climate::atmosphere::PlanetaryState;
use genesis_core::time::{SimTick, TICKS_PER_DAY};
use glam::Vec3;

pub struct CelestialLighting;

impl CelestialLighting {
    /// 惑星の公転・自転・軸傾斜角から天頂太陽ベクトルおよび光量を計算
    pub fn compute_sun_direction(planet: &PlanetaryState, current_tick: SimTick) -> (Vec3, f32) {
        // 1日の時刻角度 (0.0 - 1.0)
        let time_of_day = (current_tick.0 % TICKS_PER_DAY) as f32 / TICKS_PER_DAY as f32;
        let day_angle = time_of_day * std::f32::consts::TAU;

        let season_angle = planet.orbital_progress * std::f32::consts::TAU;
        let tilt_rad = planet.axial_tilt_deg.to_radians();

        let sun_x = day_angle.cos();
        let sun_y = day_angle.sin() * (1.0 - (season_angle.sin() * tilt_rad).abs());
        let sun_z = (season_angle.cos() * tilt_rad).sin();

        let dir = Vec3::new(sun_x, sun_y.max(0.0), sun_z).normalize_or_zero();
        let intensity = (dir.y).max(0.0) * planet.solar_luminosity;

        (dir, intensity)
    }
}
