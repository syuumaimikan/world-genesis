use crate::terrain::HeightField;
use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TectonicBoundaryType {
    Divergent,
    Convergent,
    Transform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TectonicPlate {
    pub id: u32,
    pub center: Vec2,
    pub velocity: Vec2,
    pub crust_density: f32,
    pub is_oceanic: bool,
    pub accumulated_shear_stress: f32,
}

pub struct TectonicSimulator {
    pub plates: Vec<TectonicPlate>,
    pub plate_map: Vec<u32>,
    pub width: usize,
    pub height: usize,
}

impl TectonicSimulator {
    pub fn new(width: usize, height: usize, plate_count: usize, seed: u64) -> Self {
        let mut rng = genesis_core::prng::DeterministicRng::seed_from_u64(seed);
        let mut plates = Vec::with_capacity(plate_count);

        for id in 0..plate_count as u32 {
            let cx = rng.next_f32() * width as f32;
            let cy = rng.next_f32() * height as f32;
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let speed = rng.next_f32_range(0.01, 0.08);
            let is_oceanic = rng.next_f32() < 0.65;
            let density = if is_oceanic { 3.0 } else { 2.7 };

            plates.push(TectonicPlate {
                id,
                center: Vec2::new(cx, cy),
                velocity: Vec2::new(angle.cos() * speed, angle.sin() * speed),
                crust_density: density,
                is_oceanic,
                accumulated_shear_stress: 0.0,
            });
        }

        let mut plate_map = vec![0u32; width * height];
        for y in 0..height {
            for x in 0..width {
                let pos = Vec2::new(x as f32, y as f32);
                let mut best_dist = f32::MAX;
                let mut best_id = 0;
                for plate in &plates {
                    let d = pos.distance_squared(plate.center);
                    if d < best_dist {
                        best_dist = d;
                        best_id = plate.id;
                    }
                }
                plate_map[y * width + x] = best_id;
            }
        }

        Self {
            plates,
            plate_map,
            width,
            height,
        }
    }

    pub fn simulate_tectonic_step(&mut self, heightfield: &mut HeightField) -> Vec<(Vec2, f32)> {
        let mut earthquakes = Vec::new();

        for y in 1..self.height - 1 {
            for x in 1..self.width - 1 {
                let current_plate = self.plate_map[y * self.width + x];
                let neighbors = [
                    self.plate_map[y * self.width + (x + 1)],
                    self.plate_map[y * self.width + (x - 1)],
                    self.plate_map[(y + 1) * self.width + x],
                    self.plate_map[(y - 1) * self.width + x],
                ];

                for &n_plate in &neighbors {
                    if n_plate != current_plate {
                        let p_a = &self.plates[current_plate as usize];
                        let p_b = &self.plates[n_plate as usize];

                        let rel_vel = p_a.velocity - p_b.velocity;
                        let normal = (p_b.center - p_a.center).normalize_or_zero();
                        let compression = rel_vel.dot(normal);

                        let idx = heightfield.index(x, y);
                        if compression > 0.02 {
                            let uplift = compression * (p_b.crust_density / p_a.crust_density) * 1.5;
                            heightfield.elevation[idx] += uplift;

                            if compression > 0.05 {
                                earthquakes.push((Vec2::new(x as f32, y as f32), compression * 80.0));
                            }
                        } else if compression < -0.02 {
                            heightfield.elevation[idx] += compression * 0.8;
                        }
                        break;
                    }
                }
            }
        }

        earthquakes
    }
}
