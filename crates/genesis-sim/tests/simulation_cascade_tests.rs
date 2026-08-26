use genesis_civilization::Settlement;
use genesis_core::time::{SimTick, TICKS_PER_DAY, TICKS_PER_YEAR};
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::persistence::WorldSnapshotService;
use genesis_sim::world::WorldSimulation;
use glam::Vec2;

#[test]
fn test_world_genesis_deterministic_init() {
    let config = WorldGenesisConfig {
        seed: 42,
        map_width: 64,
        map_height: 64,
        plate_count: 8,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    let mut sim_a = WorldSimulation::new(config.clone());
    sim_a.bootstrap_genesis();

    let mut sim_b = WorldSimulation::new(config);
    sim_b.bootstrap_genesis();

    assert_eq!(sim_a.heightfield.elevation, sim_b.heightfield.elevation);
    assert_eq!(
        sim_a.causality.total_events(),
        sim_b.causality.total_events()
    );
}

#[test]
fn test_long_term_demographic_causality_chain() {
    let config = WorldGenesisConfig {
        seed: 1337,
        map_width: 32,
        map_height: 32,
        plate_count: 4,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    let mut sim = WorldSimulation::new(config);
    sim.bootstrap_genesis();

    // Fast-forward 10 simulation years
    for _ in 0..10 {
        sim.tick_step(TICKS_PER_YEAR);
    }

    // Verify system numerical sanity
    for cell in &sim.atmosphere.cells {
        assert!(!cell.temperature_c.is_nan(), "Atmosphere NaN detected!");
        assert!(cell.temperature_c >= -90.0 && cell.temperature_c <= 70.0);
    }

    for elev in &sim.heightfield.elevation {
        assert!(!elev.is_nan(), "Elevation NaN detected!");
    }

    assert!(sim.causality.total_events() > 0);
}

#[test]
fn test_famine_inflation_unrest_cascade() {
    let mut settlement = Settlement::new(1, "Oakhaven", Vec2::ZERO, 1);
    settlement.population = 1000;
    settlement.food_stockpile_kg = 0.0; // Total crop failure

    // Step demographics with 0 harvest
    settlement.step_demographics(0.0);

    assert!(
        settlement.population < 1000,
        "Starvation mortality must trigger"
    );
    assert!(settlement.unrest_level > 0.0, "Unrest must surge on famine");
}

#[test]
fn test_snapshot_compression_roundtrip() {
    let config = WorldGenesisConfig {
        seed: 999,
        map_width: 32,
        map_height: 32,
        plate_count: 4,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    let mut sim = WorldSimulation::new(config);
    sim.bootstrap_genesis();

    let temp_save_path = std::env::temp_dir().join("genesis_test_snapshot.bin.zst");
    WorldSnapshotService::save_world_compressed(&sim, &temp_save_path).expect("Save failed");
    let is_valid = WorldSnapshotService::verify_snapshot_integrity(&temp_save_path)
        .expect("Integrity check failed");

    assert!(is_valid);
    let _ = std::fs::remove_file(temp_save_path);
}
