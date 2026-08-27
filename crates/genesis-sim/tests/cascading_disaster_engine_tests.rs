use genesis_climate::atmosphere::AtmosphericGrid;
use genesis_core::causality::CausalityGraph;
use genesis_core::events::{EventBus, WorldEvent};
use genesis_core::time::SimTick;
use genesis_geology::terrain::HeightField;
use genesis_sim::disasters::CascadingDisasterEngine;
use glam::Vec2;

fn sloped_terrain() -> HeightField {
    let mut hf = HeightField::new(16, 16, 0.0);
    for y in 0..16 {
        for x in 0..16 {
            let idx = hf.index(x, y);
            hf.elevation[idx] = x as f32 * 50.0;
        }
    }
    hf
}

#[test]
fn weak_quakes_only_record_the_tremor_itself() {
    let mut hf = HeightField::new(16, 16, 100.0);
    let before = hf.elevation.clone();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(10),
        Vec2::new(8.0, 8.0),
        40.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 1);
    assert_eq!(hf.elevation, before);

    let events = bus.drain();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].event,
        WorldEvent::Earthquake { magnitude, .. } if magnitude == 40.0
    ));
}

#[test]
fn strong_quakes_on_steep_slopes_cascade_into_landslide_and_flood() {
    let mut hf = sloped_terrain();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(20),
        Vec2::new(8.0, 8.0),
        90.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 3, "quake -> landslide -> flood");
    assert_eq!(hf.get_elevation(8, 8), 8.0 * 50.0 - 25.0);
    assert_eq!(hf.get_elevation(9, 8), 9.0 * 50.0 + 20.0, "natural dam");

    let events = bus.drain();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0].event, WorldEvent::Earthquake { .. }));
    assert!(matches!(
        events[1].event,
        WorldEvent::Flood {
            inundation_depth, ..
        } if inundation_depth == 6.5
    ));
}

#[test]
fn landslide_causality_chain_traces_back_to_the_quake() {
    let mut hf = sloped_terrain();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(20),
        Vec2::new(8.0, 8.0),
        90.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    let flood_id = genesis_core::causality::CausalityNodeId(3);
    let roots = causality.trace_root_causes(flood_id);
    assert_eq!(roots.len(), 1);
    assert_eq!(
        causality.get_node(roots[0]).unwrap().category,
        "Geology",
        "the flood ultimately stems from the earthquake"
    );
}

// The landslide check inspects `calculate_normal(..).z`, which is the meridional slope
// component rather than the vertical one (`y` is up), so level ground also cascades.
#[test]
fn strong_quakes_cascade_on_level_ground_too() {
    let mut hf = HeightField::new(16, 16, 100.0);
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(20),
        Vec2::new(8.0, 8.0),
        90.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 3);
    assert_eq!(hf.get_elevation(8, 8), 75.0);
}

#[test]
fn slopes_falling_away_to_the_south_are_considered_stable() {
    let mut hf = HeightField::new(16, 16, 0.0);
    for y in 0..16 {
        for x in 0..16 {
            let idx = hf.index(x, y);
            hf.elevation[idx] = (16 - y) as f32 * 20.0;
        }
    }
    let before = hf.elevation.clone();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(20),
        Vec2::new(8.0, 8.0),
        90.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 1);
    assert_eq!(hf.elevation, before);
}

#[test]
fn epicenters_outside_the_map_are_clamped_into_the_interior() {
    let mut hf = sloped_terrain();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_seismic_cascade(
        SimTick(20),
        Vec2::new(-100.0, 500.0),
        90.0,
        &mut hf,
        &mut causality,
        &bus,
    );

    assert_eq!(hf.get_elevation(1, 14), 50.0 - 25.0);
    assert!(hf.elevation.iter().all(|e| e.is_finite()));
}

#[test]
fn small_eruptions_are_recorded_without_cooling_the_climate() {
    let mut atmosphere = AtmosphericGrid::new(8, 8);
    let before: Vec<f32> = atmosphere.cells.iter().map(|c| c.temperature_c).collect();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_volcanic_eruption(
        SimTick(5),
        Vec2::new(2.0, 2.0),
        4,
        &mut atmosphere,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 1);
    let after: Vec<f32> = atmosphere.cells.iter().map(|c| c.temperature_c).collect();
    assert_eq!(after, before);

    let events = bus.drain();
    assert!(matches!(
        events[0].event,
        WorldEvent::VolcanicEruption { vei, ash_volume, .. } if vei == 4 && ash_volume == 1.0
    ));
}

#[test]
fn plinian_eruptions_trigger_a_volcanic_winter() {
    let mut atmosphere = AtmosphericGrid::new(8, 8);
    let before: Vec<f32> = atmosphere.cells.iter().map(|c| c.temperature_c).collect();
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();

    CascadingDisasterEngine::process_volcanic_eruption(
        SimTick(5),
        Vec2::new(2.0, 2.0),
        6,
        &mut atmosphere,
        &mut causality,
        &bus,
    );

    assert_eq!(causality.total_events(), 2, "eruption -> volcanic winter");
    for (cell, original) in atmosphere.cells.iter().zip(&before) {
        assert!((cell.temperature_c - (original - 100.0 * 0.4)).abs() < 1e-3);
    }
}

#[test]
fn ash_volume_grows_exponentially_with_the_eruption_index() {
    let mut causality = CausalityGraph::new();
    let bus = EventBus::new();
    let mut atmosphere = AtmosphericGrid::new(4, 4);

    for vei in [1u8, 5, 7] {
        CascadingDisasterEngine::process_volcanic_eruption(
            SimTick(1),
            Vec2::ZERO,
            vei,
            &mut atmosphere,
            &mut causality,
            &bus,
        );
    }

    let volumes: Vec<f32> = bus
        .drain()
        .into_iter()
        .filter_map(|e| match e.event {
            WorldEvent::VolcanicEruption { ash_volume, .. } => Some(ash_volume),
            _ => None,
        })
        .collect();

    assert_eq!(volumes, vec![0.1, 10.0, 1_000.0]);
}
