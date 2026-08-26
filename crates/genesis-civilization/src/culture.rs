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
