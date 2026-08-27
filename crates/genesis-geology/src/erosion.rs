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

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(width: usize, height: usize, drop_per_cell: f32) -> HeightField {
        let mut hf = HeightField::new(width, height, 0.0);
        for y in 0..height {
            for x in 0..width {
                let idx = hf.index(x, y);
                hf.elevation[idx] = (width - x) as f32 * drop_per_cell;
            }
        }
        hf
    }

    #[test]
    fn default_parameters_are_physically_sane() {
        let p = ErosionParameters::default();
        assert!(p.inertia > 0.0 && p.inertia < 1.0);
        assert!(p.dissolve_rate > 0.0 && p.deposit_rate > 0.0);
        assert!(p.evaporation_rate > 0.0 && p.evaporation_rate < 1.0);
        assert!(p.max_droplet_lifetime > 0);
        assert_eq!(
            HydraulicErosionSimulator::new(p)
                .params
                .max_droplet_lifetime,
            p.max_droplet_lifetime
        );
    }

    #[test]
    fn droplets_carve_and_deposit_material_on_generated_terrain() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut hf = HeightField::new(64, 64, 0.0);
        hf.generate_continents(1234);
        let before = hf.elevation.clone();

        let seeds: Vec<(f32, f32)> = (2..62)
            .flat_map(|x| (2..62).map(move |y| (x as f32, y as f32)))
            .collect();
        sim.simulate_droplets(&mut hf, &seeds);

        let eroded = hf
            .elevation
            .iter()
            .zip(&before)
            .filter(|(after, before)| after < before)
            .count();
        let deposited = hf
            .elevation
            .iter()
            .zip(&before)
            .filter(|(after, before)| after > before)
            .count();

        assert!(eroded > 0, "droplets must dissolve material");
        assert!(deposited > 0, "dissolved material must be deposited again");
        assert!(hf.elevation.iter().all(|e| e.is_finite()));
    }

    #[test]
    fn droplets_on_flat_ground_leave_terrain_unchanged() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut hf = HeightField::new(16, 16, 50.0);
        let before = hf.elevation.clone();
        sim.simulate_droplets(&mut hf, &[(8.0, 8.0), (4.0, 12.0)]);
        assert_eq!(hf.elevation, before);
    }

    // Droplet deltas are accumulated in parallel, so repeated runs agree only up to
    // floating point summation order.
    #[test]
    fn droplet_erosion_reproduces_the_same_terrain_for_the_same_seeds() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let seeds: Vec<(f32, f32)> = (2..30).map(|i| (i as f32, i as f32)).collect();

        let mut a = HeightField::new(32, 32, 0.0);
        a.generate_continents(7);
        let mut b = a.clone();
        sim.simulate_droplets(&mut a, &seeds);
        sim.simulate_droplets(&mut b, &seeds);

        for (lhs, rhs) in a.elevation.iter().zip(&b.elevation) {
            assert!((lhs - rhs).abs() < 1e-3, "{lhs} vs {rhs}");
        }
    }

    #[test]
    fn droplets_starting_out_of_bounds_are_ignored() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut inside = HeightField::new(32, 32, 0.0);
        inside.generate_continents(3);
        let mut with_strays = inside.clone();

        sim.simulate_droplets(&mut inside, &[(10.0, 10.0)]);
        sim.simulate_droplets(
            &mut with_strays,
            &[(10.0, 10.0), (-5.0, 4.0), (100.0, 100.0), (32.0, 0.0)],
        );

        assert_eq!(inside.elevation, with_strays.elevation);
    }

    #[test]
    fn thermal_talus_flattens_slopes_steeper_than_the_talus_angle() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut hf = HeightField::new(5, 5, 0.0);
        let peak = hf.index(2, 2);
        hf.elevation[peak] = 100.0;

        let before_peak = hf.elevation[peak];
        sim.simulate_thermal_talus(&mut hf, 0.5);

        assert!(hf.elevation[peak] < before_peak, "the spike must collapse");
        assert!(
            hf.elevation[hf.index(2, 1)] > 0.0,
            "material must move to neighbours"
        );
    }

    #[test]
    fn thermal_talus_conserves_total_mass() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut hf = ramp(16, 16, 25.0);
        let total_before: f32 = hf.elevation.iter().sum();
        sim.simulate_thermal_talus(&mut hf, 0.3);
        let total_after: f32 = hf.elevation.iter().sum();
        assert!(
            (total_before - total_after).abs() < 1.0,
            "mass drifted: {total_before} -> {total_after}"
        );
    }

    #[test]
    fn thermal_talus_leaves_gentle_terrain_alone() {
        let sim = HydraulicErosionSimulator::new(ErosionParameters::default());
        let mut hf = ramp(8, 8, 0.1);
        let before = hf.elevation.clone();
        sim.simulate_thermal_talus(&mut hf, 1.0);
        assert_eq!(hf.elevation, before);
    }
}
