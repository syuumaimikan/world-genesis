use genesis_core::time::TICKS_PER_YEAR;
use genesis_economy::CommodityType;
use genesis_sim::config::WorldGenesisConfig;
use genesis_sim::world::WorldSimulation;

#[test]
fn test_1000_years_autonomous_invariants_and_numerical_stability() {
    let config = WorldGenesisConfig {
        seed: 0x1234_5678,
        map_width: 64,
        map_height: 64,
        plate_count: 8,
        sea_level: 0.0,
        solar_luminosity: 1.0,
        axial_tilt_deg: 23.44,
    };

    let mut world = WorldSimulation::new(config);
    world.bootstrap_genesis();

    // 1,000年間自律進行
    for _ in 0..1000 {
        world.tick_step(TICKS_PER_YEAR);
    }

    // 不変量1: 地形標高の数値健全性 (NaN / Infの排除 & 現実的範囲)
    for &elev in &world.heightfield.elevation {
        assert!(!elev.is_nan(), "標高にNaNが発生しています");
        assert!(!elev.is_infinite(), "標高が発散しています");
        assert!(elev >= -12000.0 && elev <= 12000.0, "標高が規定限界を超過: {}", elev);
    }

    // 不変量2: 気候・大気セルの数値健全性
    for cell in &world.atmosphere.cells {
        assert!(!cell.temperature_c.is_nan(), "気温にNaNが発生しています");
        assert!(cell.temperature_c >= -90.0 && cell.temperature_c <= 75.0, "気温が異常値: {}", cell.temperature_c);
        assert!(!cell.precipitation_rate.is_nan() && cell.precipitation_rate >= 0.0, "降水量が負またはNaN");
    }

    // 不変量3: 生態系バイオマスの非負性
    for flora in &world.ecology.flora {
        assert!(!flora.biomass_density.is_nan() && flora.biomass_density >= 0.0, "植生バイオマスが負またはNaN");
    }
    for &herb in &world.ecology.herbivore_density {
        assert!(!herb.is_nan() && herb >= 0.0, "草食獣密度が負またはNaN");
    }

    // 不変量4: 人口および都市パラメータの非負性
    for settlement in &world.settlements {
        assert!(settlement.food_stockpile_kg >= 0.0, "食料備蓄が負数");
        assert!(settlement.infrastructure_health >= 0.0 && settlement.infrastructure_health <= 1.0, "インフラ健全度が範囲外");
        assert!(settlement.unrest_level >= 0.0 && settlement.unrest_level <= 1.0, "不満度が範囲外");
    }

    // 不変量5: 市場価格の正数性と非発散
    for market in &world.markets {
        for listing in market.listings.values() {
            assert!(!listing.last_clearing_price.is_nan(), "市場価格にNaNが発生");
            assert!(listing.last_clearing_price >= 0.1 && listing.last_clearing_price <= 50_000.0, "市場価格が異常値: {}", listing.last_clearing_price);
        }
    }

    // 不変量6: 歴史因果グラフの整合性
    assert!(world.causality.total_events() > 0, "歴史因果ノードが1つも記録されていません");
}
