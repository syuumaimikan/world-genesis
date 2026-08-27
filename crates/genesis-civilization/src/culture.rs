use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CulturalValues {
    pub martial_focus: f32,    // 0.0(平和主義) - 1.0(軍国主義)
    pub mercantile_focus: f32, // 0.0(自給自足) - 1.0(商業重視)
    pub traditionalism: f32,   // 0.0(革新志向) - 1.0(伝統固執)
    pub collectivism: f32,     // 0.0(個人主義) - 1.0(集団主義)
}

impl Default for CulturalValues {
    fn default() -> Self {
        Self {
            martial_focus: 0.5,
            mercantile_focus: 0.5,
            traditionalism: 0.5,
            collectivism: 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Culture {
    pub id: u32,
    pub name: String,
    pub language_dialect: String,
    pub values: CulturalValues,
}

impl Culture {
    pub fn drift(&mut self, is_at_war: bool, trade_volume: f32) {
        if is_at_war {
            self.values.martial_focus = (self.values.martial_focus + 0.02).min(1.0);
        } else {
            self.values.martial_focus = (self.values.martial_focus - 0.01).max(0.1);
        }

        if trade_volume > 100.0 {
            self.values.mercantile_focus = (self.values.mercantile_focus + 0.02).min(1.0);
            self.values.traditionalism = (self.values.traditionalism - 0.01).max(0.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn culture(values: CulturalValues) -> Culture {
        Culture {
            id: 1,
            name: "Valh".to_string(),
            language_dialect: "Old Valh".to_string(),
            values,
        }
    }

    #[test]
    fn default_values_are_perfectly_balanced() {
        let v = CulturalValues::default();
        assert_eq!(
            (
                v.martial_focus,
                v.mercantile_focus,
                v.traditionalism,
                v.collectivism
            ),
            (0.5, 0.5, 0.5, 0.5)
        );
    }

    #[test]
    fn war_militarises_a_culture_and_peace_demilitarises_it() {
        let mut warring = culture(CulturalValues::default());
        warring.drift(true, 0.0);
        assert!((warring.values.martial_focus - 0.52).abs() < 1e-6);

        let mut peaceful = culture(CulturalValues::default());
        peaceful.drift(false, 0.0);
        assert!((peaceful.values.martial_focus - 0.49).abs() < 1e-6);
    }

    #[test]
    fn martial_focus_saturates_at_both_extremes() {
        let mut hawk = culture(CulturalValues {
            martial_focus: 1.0,
            ..Default::default()
        });
        hawk.drift(true, 0.0);
        assert_eq!(hawk.values.martial_focus, 1.0);

        let mut dove = culture(CulturalValues {
            martial_focus: 0.1,
            ..Default::default()
        });
        dove.drift(false, 0.0);
        assert_eq!(dove.values.martial_focus, 0.1);
    }

    #[test]
    fn heavy_trade_makes_a_culture_mercantile_and_less_traditional() {
        let mut c = culture(CulturalValues::default());
        c.drift(false, 500.0);
        assert!((c.values.mercantile_focus - 0.52).abs() < 1e-6);
        assert!((c.values.traditionalism - 0.49).abs() < 1e-6);
    }

    #[test]
    fn light_trade_leaves_mercantile_values_untouched() {
        let mut c = culture(CulturalValues::default());
        c.drift(false, 100.0);
        assert_eq!(c.values.mercantile_focus, 0.5);
        assert_eq!(c.values.traditionalism, 0.5);
    }

    #[test]
    fn sustained_trade_saturates_mercantile_and_traditional_values() {
        let mut c = culture(CulturalValues::default());
        for _ in 0..200 {
            c.drift(false, 500.0);
        }
        assert_eq!(c.values.mercantile_focus, 1.0);
        assert_eq!(c.values.traditionalism, 0.1);
    }
}
