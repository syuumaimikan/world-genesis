use crate::goods::CommodityType;
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaravanUnit {
    pub id: u64,
    pub origin_settlement_id: u64,
    pub destination_settlement_id: u64,
    pub current_position: Vec2,
    pub target_position: Vec2,
    pub cargo_type: CommodityType,
    pub cargo_quantity: f32,
    pub cost_basis_per_unit: f32,
    pub travel_progress: f32, // 0.0 to 1.0
    pub is_arrived: bool,
}

impl CaravanUnit {
    pub fn step_movement(&mut self, dt_speed: f32) {
        let dist = self.current_position.distance(self.target_position);
        if dist < 0.5 {
            self.is_arrived = true;
            self.travel_progress = 1.0;
        } else {
            let dir = (self.target_position - self.current_position).normalize_or_zero();
            self.current_position += dir * dt_speed;
        }
    }
}
