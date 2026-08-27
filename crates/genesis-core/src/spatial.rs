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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_radius_on_empty_grid_returns_nothing() {
        let grid: SpatialHashGrid<u32> = SpatialHashGrid::new(4.0);
        assert!(grid.query_radius(Vec2::ZERO, 100.0).is_empty());
    }

    #[test]
    fn query_radius_only_returns_items_inside_the_circle() {
        let mut grid: SpatialHashGrid<&str> = SpatialHashGrid::new(4.0);
        grid.insert("origin", Vec2::new(0.0, 0.0));
        grid.insert("near", Vec2::new(2.0, 1.0));
        grid.insert("edge", Vec2::new(5.0, 0.0));
        grid.insert("far", Vec2::new(40.0, 40.0));

        let mut hits = grid.query_radius(Vec2::ZERO, 5.0);
        hits.sort();
        assert_eq!(hits, vec!["edge", "near", "origin"]);
    }

    #[test]
    fn query_radius_spans_multiple_cells_including_negative_coords() {
        let mut grid: SpatialHashGrid<u32> = SpatialHashGrid::new(2.0);
        for i in 0..10u32 {
            grid.insert(i, Vec2::new(-(i as f32), -(i as f32)));
        }
        let hits = grid.query_radius(Vec2::new(-4.0, -4.0), 3.0);
        let mut hits = hits;
        hits.sort();
        // Diagonal spacing is sqrt(2) per index, so indices within 2 of 4 qualify.
        assert_eq!(hits, vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn multiple_items_in_one_cell_are_all_returned() {
        let mut grid: SpatialHashGrid<u32> = SpatialHashGrid::new(8.0);
        grid.insert(1, Vec2::new(1.0, 1.0));
        grid.insert(2, Vec2::new(1.5, 1.5));
        grid.insert(3, Vec2::new(2.0, 2.0));
        assert_eq!(grid.query_radius(Vec2::new(1.5, 1.5), 4.0).len(), 3);
    }

    #[test]
    fn clear_removes_all_items() {
        let mut grid: SpatialHashGrid<u32> = SpatialHashGrid::new(4.0);
        grid.insert(1, Vec2::new(1.0, 1.0));
        grid.clear();
        assert!(grid.query_radius(Vec2::new(1.0, 1.0), 10.0).is_empty());
    }

    #[test]
    fn cell_coords_floor_towards_negative_infinity() {
        let grid: SpatialHashGrid<u32> = SpatialHashGrid::new(10.0);
        assert_eq!(
            grid.pos_to_cell(Vec2::new(0.0, 9.9)),
            SpatialCellCoord { cx: 0, cy: 0 }
        );
        assert_eq!(
            grid.pos_to_cell(Vec2::new(-0.1, 10.0)),
            SpatialCellCoord { cx: -1, cy: 1 }
        );
    }
}
