use genesis_civilization::npc::Profession;
use genesis_economy::{CommodityType, RegionalMarket};
use genesis_sim::player::PlayerCharacter;
use glam::Vec2;

fn citizen() -> PlayerCharacter {
    PlayerCharacter::new_citizen(1, "Mira", 4, Vec2::new(3.0, 5.0))
}

#[test]
fn new_citizens_start_as_provisioned_merchants() {
    let p = citizen();
    assert_eq!(p.person_id, 1);
    assert_eq!(p.name, "Mira");
    assert_eq!(p.current_settlement_id, 4);
    assert_eq!(p.world_position, Vec2::new(3.0, 5.0));
    assert_eq!(p.profession, Profession::Merchant);
    assert_eq!(p.coin_purse, 250.0);
    assert_eq!(p.inventory[&CommodityType::Bread], 5.0);
    assert_eq!(p.needs.hunger, 10.0);
    assert!(p.is_alive);
}

#[test]
fn work_pays_according_to_profession_and_costs_stamina() {
    let mut farmer = citizen();
    farmer.profession = Profession::Farmer;
    let log = farmer.perform_work();
    assert!(log.contains("小麦"));
    assert_eq!(farmer.inventory[&CommodityType::Grain], 20.0);
    assert_eq!(farmer.coin_purse, 262.0);
    assert_eq!(farmer.needs.hunger, 25.0);

    let mut smith = citizen();
    smith.profession = Profession::Blacksmith;
    smith.perform_work();
    assert_eq!(smith.inventory[&CommodityType::Tools], 2.0);
    assert_eq!(smith.coin_purse, 275.0);

    let mut merchant = citizen();
    merchant.perform_work();
    assert_eq!(merchant.coin_purse, 285.0);

    let mut scholar = citizen();
    scholar.profession = Profession::Scholar;
    scholar.perform_work();
    assert_eq!(scholar.coin_purse, 265.0);
}

#[test]
fn hunger_from_work_saturates_at_the_maximum() {
    let mut p = citizen();
    for _ in 0..20 {
        p.perform_work();
    }
    assert_eq!(p.needs.hunger, 100.0);
}

#[test]
fn meals_consume_bread_and_restore_hunger() {
    let mut p = citizen();
    p.needs.hunger = 60.0;

    p.consume_meal().unwrap();
    assert_eq!(p.inventory[&CommodityType::Bread], 4.0);
    assert_eq!(p.needs.hunger, 20.0);

    p.consume_meal().unwrap();
    assert_eq!(p.needs.hunger, 0.0, "hunger never goes negative");
}

#[test]
fn eating_without_bread_fails() {
    let mut p = citizen();
    p.inventory.insert(CommodityType::Bread, 0.0);
    assert!(p.consume_meal().is_err());
    assert_eq!(p.needs.hunger, 10.0);
}

#[test]
fn buying_moves_coins_into_goods_and_registers_demand() {
    let mut p = citizen();
    let mut market = RegionalMarket::new();
    let price = market.listings[&CommodityType::Grain].last_clearing_price;
    let demand_before = market.listings[&CommodityType::Grain].demand;

    p.buy_commodity(&mut market, CommodityType::Grain, 4.0)
        .unwrap();

    assert_eq!(p.inventory[&CommodityType::Grain], 4.0);
    assert_eq!(p.coin_purse, 250.0 - (price * 4.0) as f64);
    assert_eq!(
        market.listings[&CommodityType::Grain].demand,
        demand_before + 4.0
    );
}

#[test]
fn buying_beyond_the_purse_is_rejected() {
    let mut p = citizen();
    p.coin_purse = 1.0;
    let mut market = RegionalMarket::new();

    assert!(p
        .buy_commodity(&mut market, CommodityType::Grain, 100.0)
        .is_err());
    assert_eq!(p.coin_purse, 1.0);
    assert!(!p.inventory.contains_key(&CommodityType::Grain));
}

#[test]
fn selling_takes_a_five_percent_fee_and_registers_supply() {
    let mut p = citizen();
    p.inventory.insert(CommodityType::Tools, 10.0);
    let mut market = RegionalMarket::new();
    let price = market.listings[&CommodityType::Tools].last_clearing_price;
    let supply_before = market.listings[&CommodityType::Tools].supply;

    p.sell_commodity(&mut market, CommodityType::Tools, 4.0)
        .unwrap();

    assert_eq!(p.inventory[&CommodityType::Tools], 6.0);
    assert_eq!(p.coin_purse, 250.0 + (price * 4.0 * 0.95) as f64);
    assert_eq!(
        market.listings[&CommodityType::Tools].supply,
        supply_before + 4.0
    );
}

#[test]
fn selling_goods_the_player_does_not_own_is_rejected() {
    let mut p = citizen();
    let mut market = RegionalMarket::new();

    assert!(p
        .sell_commodity(&mut market, CommodityType::Weapons, 1.0)
        .is_err());
    assert_eq!(p.coin_purse, 250.0);
}

#[test]
fn goods_without_a_listing_fall_back_to_the_default_price() {
    let mut p = citizen();
    let mut market = RegionalMarket::new();
    market.listings.remove(&CommodityType::Grain);

    p.buy_commodity(&mut market, CommodityType::Grain, 2.0)
        .unwrap();
    assert_eq!(p.coin_purse, 250.0 - 20.0);
}
