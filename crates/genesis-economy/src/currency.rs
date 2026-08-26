use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currency {
    pub nation_id: u32,
    pub symbol: String,
    pub total_money_supply: f64,
    pub reserve_gold_tons: f64,
    pub inflation_rate_pct: f32,
}

impl Currency {
    pub fn compute_purchasing_power(&self, national_gdp: f64) -> f64 {
        if self.total_money_supply <= 0.0 {
            0.0
        } else {
            (national_gdp + self.reserve_gold_tons * 50_000.0) / self.total_money_supply
        }
    }
}
