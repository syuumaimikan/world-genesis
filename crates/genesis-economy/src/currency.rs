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

#[cfg(test)]
mod tests {
    use super::*;

    fn currency(supply: f64, gold: f64) -> Currency {
        Currency {
            nation_id: 1,
            symbol: "AUR".to_string(),
            total_money_supply: supply,
            reserve_gold_tons: gold,
            inflation_rate_pct: 2.5,
        }
    }

    #[test]
    fn purchasing_power_counts_gdp_and_gold_reserves() {
        let c = currency(1_000.0, 2.0);
        assert_eq!(
            c.compute_purchasing_power(500.0),
            (500.0 + 100_000.0) / 1_000.0
        );
    }

    #[test]
    fn purchasing_power_falls_as_money_supply_grows() {
        let tight = currency(1_000.0, 0.0);
        let loose = currency(4_000.0, 0.0);
        assert!(tight.compute_purchasing_power(2_000.0) > loose.compute_purchasing_power(2_000.0));
    }

    #[test]
    fn currencies_without_money_supply_have_no_purchasing_power() {
        assert_eq!(currency(0.0, 10.0).compute_purchasing_power(1_000.0), 0.0);
        assert_eq!(currency(-5.0, 10.0).compute_purchasing_power(1_000.0), 0.0);
    }

    #[test]
    fn currency_roundtrips_through_json() {
        let c = currency(1_234.5, 6.0);
        let decoded: Currency = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(decoded.symbol, c.symbol);
        assert_eq!(decoded.total_money_supply, c.total_money_supply);
        assert_eq!(decoded.reserve_gold_tons, c.reserve_gold_tons);
    }
}
