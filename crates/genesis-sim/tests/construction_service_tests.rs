use genesis_civilization::Settlement;
use genesis_economy::CommodityType;
use genesis_sim::building::{BuildingType, ConstructionService};
use glam::Vec2;
use std::collections::HashMap;

fn town() -> Settlement {
    let mut s = Settlement::new(7, "Halbrook", Vec2::new(10.0, 10.0), 1);
    s.infrastructure_health = 0.5;
    s.unrest_level = 0.5;
    s
}

fn full_warehouse() -> HashMap<CommodityType, f32> {
    HashMap::from([
        (CommodityType::Timber, 500.0),
        (CommodityType::IronOre, 500.0),
        (CommodityType::Tools, 500.0),
    ])
}

#[test]
fn every_building_type_has_a_timber_based_cost() {
    for b_type in [
        BuildingType::Farmstead,
        BuildingType::SmithyWorkshop,
        BuildingType::Marketplace,
        BuildingType::FortressWall,
    ] {
        let cost = ConstructionService::get_building_cost(b_type);
        assert!(cost[&CommodityType::Timber] > 0.0);
        assert!(cost[&CommodityType::Tools] > 0.0);
        assert!(cost.values().all(|q| *q > 0.0));
    }

    let fortress = ConstructionService::get_building_cost(BuildingType::FortressWall);
    let farm = ConstructionService::get_building_cost(BuildingType::Farmstead);
    assert!(fortress[&CommodityType::Timber] > farm[&CommodityType::Timber]);
    assert!(fortress.contains_key(&CommodityType::IronOre));
    assert!(!farm.contains_key(&CommodityType::IronOre));
}

#[test]
fn construction_charges_coins_and_materials_and_improves_the_settlement() {
    let mut settlement = town();
    let mut resources = full_warehouse();
    let mut coins = 200.0;

    let building = ConstructionService::construct_in_settlement(
        BuildingType::Marketplace,
        &mut settlement,
        &mut resources,
        &mut coins,
    )
    .unwrap();

    assert_eq!(building.building_type, BuildingType::Marketplace);
    assert_eq!(building.settlement_id, settlement.id);
    assert_eq!(building.id, 1000 + settlement.id);
    assert_eq!(building.level, 1);
    assert_eq!(building.structural_integrity, 1.0);

    assert_eq!(coins, 150.0);
    assert_eq!(resources[&CommodityType::Timber], 450.0);
    assert_eq!(resources[&CommodityType::Tools], 492.0);
    assert!((settlement.infrastructure_health - 0.55).abs() < 1e-6);
    assert!((settlement.unrest_level - 0.42).abs() < 1e-6);
}

#[test]
fn construction_fails_without_enough_coins_and_changes_nothing() {
    let mut settlement = town();
    let mut resources = full_warehouse();
    let mut coins = 49.0;

    let err = ConstructionService::construct_in_settlement(
        BuildingType::Farmstead,
        &mut settlement,
        &mut resources,
        &mut coins,
    )
    .unwrap_err();

    assert!(err.contains("所持金"));
    assert_eq!(coins, 49.0);
    assert_eq!(resources[&CommodityType::Timber], 500.0);
    assert_eq!(settlement.infrastructure_health, 0.5);
}

#[test]
fn construction_fails_on_missing_materials_without_partial_payment() {
    let mut settlement = town();
    let mut resources = HashMap::from([(CommodityType::Timber, 500.0)]);
    let mut coins = 1_000.0;

    let err = ConstructionService::construct_in_settlement(
        BuildingType::SmithyWorkshop,
        &mut settlement,
        &mut resources,
        &mut coins,
    )
    .unwrap_err();

    assert!(err.contains("資源"));
    assert_eq!(coins, 1_000.0);
    assert_eq!(resources[&CommodityType::Timber], 500.0);
    assert_eq!(settlement.unrest_level, 0.5);
}

#[test]
fn infrastructure_and_unrest_saturate_after_many_projects() {
    let mut settlement = town();
    let mut resources = HashMap::from([
        (CommodityType::Timber, 100_000.0),
        (CommodityType::IronOre, 100_000.0),
        (CommodityType::Tools, 100_000.0),
    ]);
    let mut coins = 100_000.0;

    for _ in 0..30 {
        ConstructionService::construct_in_settlement(
            BuildingType::Farmstead,
            &mut settlement,
            &mut resources,
            &mut coins,
        )
        .unwrap();
    }

    assert_eq!(settlement.infrastructure_health, 1.0);
    assert_eq!(settlement.unrest_level, 0.0);
}
