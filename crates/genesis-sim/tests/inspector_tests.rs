use genesis_core::time::TICKS_PER_YEAR;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::inspector::{ViewMode, WorldInspector};
use genesis_sim::world::WorldSimulation;

#[test]
fn every_view_mode_renders_a_downsampled_viewport() {
    let mut world = WorldSimulation::new(WorldGenesisConfig {
        seed: 7,
        map_width: 32,
        map_height: 32,
        plate_count: 4,
        ..WorldGenesisConfig::default()
    });
    world.bootstrap_genesis();
    world.tick_step(TICKS_PER_YEAR);

    for mode in [
        ViewMode::Elevation,
        ViewMode::Temperature,
        ViewMode::Vegetation,
        ViewMode::Political,
    ] {
        WorldInspector::render_ansi_viewport(&world, mode, 16, 8);
    }
}
