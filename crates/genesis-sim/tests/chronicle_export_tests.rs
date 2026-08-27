use genesis_core::time::TICKS_PER_YEAR;
use genesis_sim::chronicle_export::WorldChronicleExporter;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::world::WorldSimulation;

fn small_world() -> WorldSimulation {
    let config = WorldGenesisConfig {
        seed: 0xFEED_FACE,
        map_width: 32,
        map_height: 32,
        plate_count: 4,
        ..WorldGenesisConfig::default()
    };
    let mut world = WorldSimulation::new(config);
    world.bootstrap_genesis();
    for _ in 0..20 {
        world.tick_step(TICKS_PER_YEAR);
    }
    world
}

#[test]
fn default_config_describes_a_playable_earthlike_world() {
    let c = WorldGenesisConfig::default();
    assert_eq!(c.seed, 0xC0_FFEE_BEEF);
    assert_eq!(c.map_width, 128);
    assert_eq!(c.map_height, 128);
    assert_eq!(c.plate_count, 12);
    assert_eq!(c.sea_level, 0.0);
    assert_eq!(c.solar_luminosity, 1.0);
    assert!((c.axial_tilt_deg - 23.44).abs() < 1e-6);
}

#[test]
fn config_roundtrips_through_json() {
    let c = WorldGenesisConfig {
        seed: 42,
        map_width: 64,
        ..WorldGenesisConfig::default()
    };
    let decoded: WorldGenesisConfig =
        serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(decoded.seed, 42);
    assert_eq!(decoded.map_width, 64);
    assert_eq!(decoded.map_height, 128);
}

#[test]
fn exported_chronicle_documents_the_world_seed_events_and_powers() {
    let world = small_world();
    let dir = std::env::temp_dir().join(format!("genesis-chronicle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("chronicle.md");

    WorldChronicleExporter::export_markdown_chronicle(&world, &path).unwrap();
    let markdown = std::fs::read_to_string(&path).unwrap();

    assert!(markdown.starts_with("# WORLD GENESIS"));
    assert!(markdown.contains(&format!("0x{:X}", world.config.seed)));
    assert!(markdown.contains("32x32"));
    assert!(markdown.contains("## 1."));
    assert!(markdown.contains("## 2."));

    for nation in &world.nations {
        assert!(markdown.contains(&nation.name));
    }
    for settlement in &world.settlements {
        assert!(markdown.contains(&settlement.name));
    }

    let event_lines = markdown.lines().filter(|l| l.contains("(Node #")).count();
    assert!(event_lines > 0, "recorded history must be listed");
    assert!(event_lines <= world.causality.total_events());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn exporting_to_an_unwritable_path_reports_an_error() {
    let world = WorldSimulation::new(WorldGenesisConfig {
        map_width: 8,
        map_height: 8,
        plate_count: 2,
        ..WorldGenesisConfig::default()
    });
    let missing_dir = std::env::temp_dir().join("genesis-does-not-exist/chronicle.md");
    assert!(WorldChronicleExporter::export_markdown_chronicle(&world, missing_dir).is_err());
}
