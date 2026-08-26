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
