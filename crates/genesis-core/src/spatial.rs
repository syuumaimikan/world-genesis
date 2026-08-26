use glam::Vec2;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpatialCellCoord {
    pub cx: i32,
    pub cy: i32,
}

pub struct SpatialHashGrid<T> {
    cell_size: f32,
    cells: HashMap<SpatialCellCoord, Vec<(T, Vec2)>>,
}

impl<T: Clone> SpatialHashGrid<T> {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    #[inline]
    fn pos_to_cell(&self, pos: Vec2) -> SpatialCellCoord {
        SpatialCellCoord {
            cx: (pos.x / self.cell_size).floor() as i32,
            cy: (pos.y / self.cell_size).floor() as i32,
        }
    }

    pub fn insert(&mut self, item: T, pos: Vec2) {
        let coord = self.pos_to_cell(pos);
        self.cells.entry(coord).or_default().push((item, pos));
    }

    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<T> {
        let mut results = Vec::new();
        let r_sq = radius * radius;

        let min_coord = self.pos_to_cell(center - Vec2::splat(radius));
        let max_coord = self.pos_to_cell(center + Vec2::splat(radius));

        for cy in min_coord.cy..=max_coord.cy {
            for cx in min_coord.cx..=max_coord.cx {
                if let Some(bucket) = self.cells.get(&SpatialCellCoord { cx, cy }) {
                    for (item, pos) in bucket {
                        if center.distance_squared(*pos) <= r_sq {
                            results.push(item.clone());
                        }
                    }
                }
            }
        }

        results
    }
}
