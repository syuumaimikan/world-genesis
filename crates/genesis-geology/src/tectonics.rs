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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_simulator_assigns_every_cell_to_the_nearest_plate() {
        let sim = TectonicSimulator::new(24, 24, 5, 42);
        assert_eq!(sim.plates.len(), 5);
        assert_eq!(sim.plate_map.len(), 24 * 24);

        for y in 0..24 {
            for x in 0..24 {
                let pos = Vec2::new(x as f32, y as f32);
                let assigned = sim.plate_map[y * 24 + x];
                let assigned_dist = sim.plates[assigned as usize].center.distance_squared(pos);
                let nearest_dist = sim
                    .plates
                    .iter()
                    .map(|p| p.center.distance_squared(pos))
                    .fold(f32::MAX, f32::min);
                assert!((assigned_dist - nearest_dist).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn plate_properties_follow_crust_type() {
        let sim = TectonicSimulator::new(16, 16, 8, 7);
        for plate in &sim.plates {
            let expected_density = if plate.is_oceanic { 3.0 } else { 2.7 };
            assert_eq!(plate.crust_density, expected_density);
            assert_eq!(plate.accumulated_shear_stress, 0.0);
            let speed = plate.velocity.length();
            assert!((0.01..=0.08).contains(&speed), "speed = {speed}");
            assert!(plate.center.x >= 0.0 && plate.center.x <= 16.0);
        }
        let ids: Vec<u32> = sim.plates.iter().map(|p| p.id).collect();
        assert_eq!(ids, (0..8).collect::<Vec<u32>>());
    }

    #[test]
    fn simulator_construction_is_seed_deterministic() {
        let a = TectonicSimulator::new(16, 16, 6, 2024);
        let b = TectonicSimulator::new(16, 16, 6, 2024);
        let c = TectonicSimulator::new(16, 16, 6, 2025);
        assert_eq!(a.plate_map, b.plate_map);
        assert_ne!(a.plate_map, c.plate_map);
    }

    #[test]
    fn tectonic_step_only_modifies_terrain_near_plate_boundaries() {
        let mut sim = TectonicSimulator::new(32, 32, 6, 99);
        let mut hf = HeightField::new(32, 32, 100.0);
        sim.simulate_tectonic_step(&mut hf);

        for y in 1..31 {
            for x in 1..31 {
                let current = sim.plate_map[y * 32 + x];
                let is_boundary = [
                    sim.plate_map[y * 32 + x + 1],
                    sim.plate_map[y * 32 + x - 1],
                    sim.plate_map[(y + 1) * 32 + x],
                    sim.plate_map[(y - 1) * 32 + x],
                ]
                .iter()
                .any(|&n| n != current);

                if !is_boundary {
                    assert_eq!(
                        hf.get_elevation(x, y),
                        100.0,
                        "interior cell ({x},{y}) changed"
                    );
                }
            }
        }
    }

    #[test]
    fn convergent_boundaries_uplift_terrain_and_can_emit_earthquakes() {
        let mut sim = TectonicSimulator::new(4, 4, 2, 1);
        // Two plates colliding head-on along the vertical midline.
        sim.plates = vec![
            TectonicPlate {
                id: 0,
                center: Vec2::new(0.0, 2.0),
                velocity: Vec2::new(0.5, 0.0),
                crust_density: 3.0,
                is_oceanic: true,
                accumulated_shear_stress: 0.0,
            },
            TectonicPlate {
                id: 1,
                center: Vec2::new(4.0, 2.0),
                velocity: Vec2::new(-0.5, 0.0),
                crust_density: 2.7,
                is_oceanic: false,
                accumulated_shear_stress: 0.0,
            },
        ];
        sim.plate_map = (0..16).map(|i| if (i % 4) < 2 { 0 } else { 1 }).collect();

        let mut hf = HeightField::new(4, 4, 0.0);
        let quakes = sim.simulate_tectonic_step(&mut hf);

        assert!(
            hf.get_elevation(1, 1) > 0.0,
            "collision must raise mountains"
        );
        assert!(
            !quakes.is_empty(),
            "strong compression must emit earthquakes"
        );
        assert!(quakes.iter().all(|(_, magnitude)| *magnitude > 0.0));
    }

    #[test]
    fn divergent_boundaries_subside_terrain_without_earthquakes() {
        let mut sim = TectonicSimulator::new(4, 4, 2, 1);
        sim.plates = vec![
            TectonicPlate {
                id: 0,
                center: Vec2::new(0.0, 2.0),
                velocity: Vec2::new(-0.5, 0.0),
                crust_density: 3.0,
                is_oceanic: true,
                accumulated_shear_stress: 0.0,
            },
            TectonicPlate {
                id: 1,
                center: Vec2::new(4.0, 2.0),
                velocity: Vec2::new(0.5, 0.0),
                crust_density: 3.0,
                is_oceanic: true,
                accumulated_shear_stress: 0.0,
            },
        ];
        sim.plate_map = (0..16).map(|i| if (i % 4) < 2 { 0 } else { 1 }).collect();

        let mut hf = HeightField::new(4, 4, 0.0);
        let quakes = sim.simulate_tectonic_step(&mut hf);

        assert!(hf.get_elevation(1, 1) < 0.0, "rifting must create a basin");
        assert!(quakes.is_empty());
    }
}
