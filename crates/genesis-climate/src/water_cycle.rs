use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiverNode {
    pub grid_index: usize,
    pub downstream_index: Option<usize>,
    pub flow_rate_m3_s: f32,
    pub sediment_load_kg_s: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WaterCycleSystem {
    pub rivers: Vec<RiverNode>,
}

impl WaterCycleSystem {
    pub fn generate_drainage_network(
        &mut self,
        width: usize,
        height: usize,
        elevation: &[f32],
        precipitation: &[f32],
        sea_level: f32,
    ) {
        self.rivers.clear();
        let total_cells = width * height;
        let mut accumulation = vec![0.0f32; total_cells];

        // Seed rainfall runoff
        for i in 0..total_cells {
            accumulation[i] = (precipitation[i] / 1000.0).max(0.01);
        }

        // Steepest descent flow routing
        let mut flow_directions = vec![None; total_cells];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = y * width + x;
                if elevation[idx] <= sea_level {
                    continue;
                }

                let current_e = elevation[idx];
                let mut min_e = current_e;
                let mut target = None;

                for dy in [-1, 0, 1] {
                    for dx in [-1, 0, 1] {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = (x as isize + dx) as usize;
                        let ny = (y as isize + dy) as usize;
                        let n_idx = ny * width + nx;
                        if elevation[n_idx] < min_e {
                            min_e = elevation[n_idx];
                            target = Some(n_idx);
                        }
                    }
                }

                flow_directions[idx] = target;
            }
        }

        // Accumulate flow downstream
        for _ in 0..12 {
            for idx in 0..total_cells {
                if let Some(downstream) = flow_directions[idx] {
                    accumulation[downstream] += accumulation[idx] * 0.9;
                }
            }
        }

        for idx in 0..total_cells {
            if accumulation[idx] > 5.0 {
                self.rivers.push(RiverNode {
                    grid_index: idx,
                    downstream_index: flow_directions[idx],
                    flow_rate_m3_s: accumulation[idx] * 12.5,
                    sediment_load_kg_s: accumulation[idx] * 2.0,
                });
            }
        }
    }
}
