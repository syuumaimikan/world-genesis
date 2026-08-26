use crate::terrain::HeightField;
use glam::Vec2;
use parking_lot::Mutex;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct ErosionParameters {
    pub inertia: f32,
    pub sediment_capacity_factor: f32,
    pub min_sediment_capacity: f32,
    pub dissolve_rate: f32,
    pub deposit_rate: f32,
    pub evaporation_rate: f32,
    pub gravity: f32,
    pub max_droplet_lifetime: usize,
}

impl Default for ErosionParameters {
    fn default() -> Self {
        Self {
            inertia: 0.05,
            sediment_capacity_factor: 4.0,
            min_sediment_capacity: 0.01,
            dissolve_rate: 0.3,
            deposit_rate: 0.3,
            evaporation_rate: 0.01,
            gravity: 4.0,
            max_droplet_lifetime: 40,
        }
    }
}

pub struct HydraulicErosionSimulator {
    pub params: ErosionParameters,
}

impl HydraulicErosionSimulator {
    pub fn new(params: ErosionParameters) -> Self {
        Self { params }
    }

    pub fn simulate_droplets(
        &self,
        heightfield: &mut HeightField,
        droplet_seeds: &[(f32, f32)],
    ) {
        let w = heightfield.width;
        let h = heightfield.height;

        let delta_elevation: Vec<Mutex<f32>> = (0..w * h)
            .map(|_| Mutex::new(0.0f32))
            .collect();

        droplet_seeds.par_iter().for_each(|&(sx, sy)| {
            let mut pos = Vec2::new(sx, sy);
            let mut dir = Vec2::ZERO;
            let mut speed: f32 = 1.0;
            let mut water: f32 = 1.0;
            let mut sediment: f32 = 0.0;

            for _ in 0..self.params.max_droplet_lifetime {
                let px = pos.x.floor() as usize;
                let py = pos.y.floor() as usize;

                if px >= w - 1 || py >= h - 1 {
                    break;
                }

                let normal = heightfield.calculate_normal(px, py);
                dir = dir * self.params.inertia - Vec2::new(normal.x, normal.y) * (1.0 - self.params.inertia);
                let dir_len = dir.length();
                if dir_len < 1e-4 {
                    break;
                }
                dir /= dir_len;

                let next_pos = pos + dir;
                let nx = next_pos.x.floor() as usize;
                let ny = next_pos.y.floor() as usize;

                if nx >= w - 1 || ny >= h - 1 {
                    break;
                }

                let h_curr = heightfield.get_elevation(px, py);
                let h_next = heightfield.get_elevation(nx, ny);
                let diff = h_next - h_curr;

                let capacity = ((-diff).max(0.0f32) * speed * water * self.params.sediment_capacity_factor)
                    .max(self.params.min_sediment_capacity);

                let idx = py * w + px;
                if sediment > capacity || diff > 0.0 {
                    let to_deposit = if diff > 0.0 {
                        sediment.min(diff)
                    } else {
                        (sediment - capacity) * self.params.deposit_rate
                    };
                    sediment -= to_deposit;
                    let mut lock = delta_elevation[idx].lock();
                    *lock += to_deposit;
                } else {
                    let to_erode = ((capacity - sediment) * self.params.dissolve_rate)
                        .min(-diff)
                        .max(0.0f32);
                    sediment += to_erode;
                    let mut lock = delta_elevation[idx].lock();
                    *lock -= to_erode;
                }

                speed = (speed * speed + diff * self.params.gravity).max(0.0f32).sqrt();
                water *= 1.0 - self.params.evaporation_rate;
                pos = next_pos;
            }
        });

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let delta = *delta_elevation[idx].lock();
                heightfield.elevation[idx] += delta;
            }
        }
    }

    pub fn simulate_thermal_talus(&self, heightfield: &mut HeightField, talus_angle_rad: f32) {
        let w = heightfield.width;
        let h = heightfield.height;
        let tan_talus = talus_angle_rad.tan();

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = heightfield.index(x, y);
                let center_h = heightfield.elevation[idx];

                let neighbors = [
                    (x + 1, y),
                    (x - 1, y),
                    (x, y + 1),
                    (x, y - 1),
                ];

                for (nx, ny) in neighbors {
                    let n_idx = heightfield.index(nx, ny);
                    let n_h = heightfield.elevation[n_idx];
                    let diff = center_h - n_h;

                    if diff > tan_talus {
                        let transfer = (diff - tan_talus) * 0.25;
                        heightfield.elevation[idx] -= transfer;
                        heightfield.elevation[n_idx] += transfer;
                    }
                }
            }
        }
    }
}
