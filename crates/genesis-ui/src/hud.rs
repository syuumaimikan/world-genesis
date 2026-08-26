use genesis_civilization::Settlement;
use genesis_core::time::{SimCalendar, SimTick};
use genesis_economy::{CommodityType, RegionalMarket};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct MarketPriceHistory {
    pub history_points: HashMap<CommodityType, Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct HudViewModel {
    pub calendar_text: String,
    pub total_population: u32,
    pub active_events_count: usize,
    pub price_history: MarketPriceHistory,
}

impl HudViewModel {
    pub fn build(
        current_tick: SimTick,
        settlements: &[Settlement],
        markets: &[RegionalMarket],
        total_events_count: usize,
    ) -> Self {
        let cal = SimCalendar::from_tick(current_tick);
        let calendar_text = format!(
            "YEAR {:04} / MONTH {:02} / DAY {:02}",
            cal.year, cal.month, cal.day
        );

        let total_pop: u32 = settlements.iter().map(|s| s.population).sum();

        let mut history = MarketPriceHistory::default();
        if let Some(market) = markets.first() {
            for (&good, listing) in &market.listings {
                history
                    .history_points
                    .entry(good)
                    .or_default()
                    .push(listing.last_clearing_price);
            }
        }

        Self {
            calendar_text,
            total_population: total_pop,
            active_events_count: total_events_count,
            price_history: history,
        }
    }
}
