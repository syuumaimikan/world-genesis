use crate::currency::Currency;
use std::collections::HashMap;

pub struct ForexMarket;

impl ForexMarket {
    pub fn calculate_exchange_rate(
        base: &Currency,
        base_gdp: f64,
        quote: &Currency,
        quote_gdp: f64,
    ) -> f64 {
        let base_power = base.compute_purchasing_power(base_gdp);
        let quote_power = quote.compute_purchasing_power(quote_gdp);

        if quote_power <= 0.0 {
            1.0
        } else {
            base_power / quote_power
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn currency(symbol: &str, supply: f64, gold: f64) -> Currency {
        Currency {
            nation_id: 0,
            symbol: symbol.to_string(),
            total_money_supply: supply,
            reserve_gold_tons: gold,
            inflation_rate_pct: 0.0,
        }
    }

    #[test]
    fn identical_economies_trade_at_parity() {
        let a = currency("A", 1_000.0, 1.0);
        let b = currency("B", 1_000.0, 1.0);
        assert_eq!(
            ForexMarket::calculate_exchange_rate(&a, 500.0, &b, 500.0),
            1.0
        );
    }

    #[test]
    fn stronger_base_currency_buys_more_of_the_quote() {
        let strong = currency("A", 1_000.0, 10.0);
        let weak = currency("B", 5_000.0, 0.0);
        let rate = ForexMarket::calculate_exchange_rate(&strong, 1_000.0, &weak, 1_000.0);
        assert!(rate > 1.0, "rate was {rate}");
        let inverse = ForexMarket::calculate_exchange_rate(&weak, 1_000.0, &strong, 1_000.0);
        assert!((rate * inverse - 1.0).abs() < 1e-9);
    }

    #[test]
    fn worthless_quote_currency_falls_back_to_parity() {
        let base = currency("A", 1_000.0, 1.0);
        let collapsed = currency("B", 0.0, 0.0);
        assert_eq!(
            ForexMarket::calculate_exchange_rate(&base, 1_000.0, &collapsed, 1_000.0),
            1.0
        );
    }
}
