use crate::config::WorldGenesisConfig;
use crate::world::WorldSimulation;
use genesis_core::time::TICKS_PER_YEAR;
use std::time::Instant;

pub struct BenchmarkResult {
    pub elapsed_millis: u128,
    pub simulated_years: u32,
    pub years_per_sec: f64,
    pub total_causality_nodes: usize,
    pub peak_memory_estimate_mb: f32,
}

pub struct SimulationBenchmarkRunner;

impl SimulationBenchmarkRunner {
    pub fn run_stress_benchmark(years: u32, map_size: usize) -> BenchmarkResult {
        let config = WorldGenesisConfig {
            seed: 0xBEEF_1337,
            map_width: map_size,
            map_height: map_size,
            plate_count: 12,
            sea_level: 0.0,
            solar_luminosity: 1.0,
            axial_tilt_deg: 23.44,
        };

        let mut world = WorldSimulation::new(config);
        world.bootstrap_genesis();

        let start = Instant::now();
        for _ in 0..years {
            world.tick_step(TICKS_PER_YEAR);
        }
        let elapsed = start.elapsed();
        let elapsed_millis = elapsed.as_millis().max(1);
        let years_per_sec = (years as f64) / (elapsed.as_secs_f64().max(0.0001));

        let total_nodes = world.causality.total_events();
        let cells = (map_size * map_size) as f32;
        let memory_estimate_mb = (cells * 4.0 * 8.0) / (1024.0 * 1024.0);

        BenchmarkResult {
            elapsed_millis,
            simulated_years: years,
            years_per_sec,
            total_causality_nodes: total_nodes,
            peak_memory_estimate_mb: memory_estimate_mb,
        }
    }
}
