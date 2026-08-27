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

#[cfg(test)]
mod tests {
    use super::*;

    fn caravan(from: Vec2, to: Vec2) -> CaravanUnit {
        CaravanUnit {
            id: 7,
            origin_settlement_id: 1,
            destination_settlement_id: 2,
            current_position: from,
            target_position: to,
            cargo_type: CommodityType::Grain,
            cargo_quantity: 50.0,
            cost_basis_per_unit: 1.5,
            travel_progress: 0.0,
            is_arrived: false,
        }
    }

    #[test]
    fn caravans_move_towards_their_destination() {
        let mut c = caravan(Vec2::ZERO, Vec2::new(10.0, 0.0));
        c.step_movement(2.0);
        assert_eq!(c.current_position, Vec2::new(2.0, 0.0));
        assert!(!c.is_arrived);
    }

    #[test]
    fn caravans_arrive_once_within_half_a_cell() {
        let mut c = caravan(Vec2::new(9.8, 0.0), Vec2::new(10.0, 0.0));
        c.step_movement(1.0);
        assert!(c.is_arrived);
        assert_eq!(c.travel_progress, 1.0);
        assert_eq!(c.current_position, Vec2::new(9.8, 0.0));
    }

    #[test]
    fn repeated_steps_eventually_reach_the_destination() {
        let mut c = caravan(Vec2::ZERO, Vec2::new(3.0, 4.0));
        for _ in 0..20 {
            c.step_movement(0.5);
        }
        assert!(c.is_arrived);
        assert!(c.current_position.distance(Vec2::new(3.0, 4.0)) < 0.5);
    }
}
