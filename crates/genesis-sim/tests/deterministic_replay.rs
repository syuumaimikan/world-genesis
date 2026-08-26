use genesis_core::time::TICKS_PER_YEAR;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::world::WorldSimulation;

#[test]
fn test_seed_deterministic_exact_replay() {
    let config = WorldGenesisConfig {
        seed: 0x5EED_CAFE,
        map_width: 32,
        map_height: 32,
        plate_count: 4,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    // Run A
    let mut sim_a = WorldSimulation::new(config.clone());
    sim_a.bootstrap_genesis();
    for _ in 0..50 {
        sim_a.tick_step(TICKS_PER_YEAR);
    }

    // Run B
    let mut sim_b = WorldSimulation::new(config);
    sim_b.bootstrap_genesis();
    for _ in 0..50 {
        sim_b.tick_step(TICKS_PER_YEAR);
    }

    // 標高が1ビットの狂いもなく完全一致すること
    assert_eq!(sim_a.heightfield.elevation, sim_b.heightfield.elevation);
    // 気候温度が完全一致すること
    for (ca, cb) in sim_a.atmosphere.cells.iter().zip(sim_b.atmosphere.cells.iter()) {
        assert_eq!(ca.temperature_c, cb.temperature_c);
    }
    // 記録された歴史因果ノード数が完全一致すること
    assert_eq!(sim_a.causality.total_events(), sim_b.causality.total_events());
}
