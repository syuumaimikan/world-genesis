use glam::IVec2;
use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord(pub IVec2);

impl ChunkCoord {
    #[inline]
    pub fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }

    #[inline]
    pub fn from_world_pos(world_x: f32, world_y: f32) -> Self {
        let cx = (world_x / CHUNK_SIZE as f32).floor() as i32;
        let cy = (world_y / CHUNK_SIZE as f32).floor() as i32;
        Self::new(cx, cy)
    }

    #[inline]
    pub fn to_world_min(&self) -> (f32, f32) {
        (
            self.0.x as f32 * CHUNK_SIZE as f32,
            self.0.y as f32 * CHUNK_SIZE as f32,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimLOD {
    LOD0Immediate, // Active local entity GOAP / particle physics
    LOD1Local,     // Aggregated local market & pathing
    LOD2Regional,  // Regional ecology & bulk economic flows
    LOD3National,  // Abstract nation/statistical model
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkGrid<T> {
    pub width: usize,
    pub height: usize,
    pub data: Vec<T>,
}

impl<T: Default + Clone> ChunkGrid<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![T::default(); width * height],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        if x < self.width && y < self.height {
            Some(&self.data[y * self.width + x])
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        if x < self.width && y < self.height {
            Some(&mut self.data[y * self.width + x])
        } else {
            None
        }
    }

    #[inline]
    pub fn index_unchecked(&self, x: usize, y: usize) -> &T {
        &self.data[y * self.width + x]
    }

    #[inline]
    pub fn index_unchecked_mut(&mut self, x: usize, y: usize) -> &mut T {
        &mut self.data[y * self.width + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_coord_from_world_pos_floors_towards_negative_infinity() {
        assert_eq!(ChunkCoord::from_world_pos(0.0, 0.0), ChunkCoord::new(0, 0));
        assert_eq!(
            ChunkCoord::from_world_pos(31.9, 31.9),
            ChunkCoord::new(0, 0)
        );
        assert_eq!(
            ChunkCoord::from_world_pos(32.0, 64.0),
            ChunkCoord::new(1, 2)
        );
        assert_eq!(
            ChunkCoord::from_world_pos(-0.5, -32.0),
            ChunkCoord::new(-1, -1)
        );
        assert_eq!(
            ChunkCoord::from_world_pos(-33.0, -65.0),
            ChunkCoord::new(-2, -3)
        );
    }

    #[test]
    fn chunk_coord_world_min_is_inverse_of_from_world_pos() {
        for &(x, y) in &[(0, 0), (3, -7), (-4, 12)] {
            let coord = ChunkCoord::new(x, y);
            let (wx, wy) = coord.to_world_min();
            assert_eq!(wx, (x * CHUNK_SIZE as i32) as f32);
            assert_eq!(wy, (y * CHUNK_SIZE as i32) as f32);
            assert_eq!(ChunkCoord::from_world_pos(wx, wy), coord);
        }
    }

    #[test]
    fn grid_new_is_filled_with_defaults() {
        let grid: ChunkGrid<f32> = ChunkGrid::new(4, 3);
        assert_eq!(grid.data.len(), 12);
        assert!(grid.data.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn grid_get_is_row_major_and_bounds_checked() {
        let mut grid: ChunkGrid<u32> = ChunkGrid::new(4, 3);
        *grid.get_mut(2, 1).unwrap() = 7;
        assert_eq!(grid.data[1 * 4 + 2], 7);
        assert_eq!(grid.get(2, 1), Some(&7));
        assert_eq!(grid.get(4, 0), None);
        assert_eq!(grid.get(0, 3), None);
        assert!(grid.get_mut(9, 9).is_none());
    }

    #[test]
    fn grid_unchecked_accessors_match_checked_ones() {
        let mut grid: ChunkGrid<i32> = ChunkGrid::new(3, 3);
        *grid.index_unchecked_mut(1, 2) = -5;
        assert_eq!(*grid.index_unchecked(1, 2), -5);
        assert_eq!(grid.get(1, 2), Some(&-5));
    }

    #[test]
    fn lod_levels_compare_by_identity() {
        assert_eq!(SimLOD::LOD0Immediate, SimLOD::LOD0Immediate);
        assert_ne!(SimLOD::LOD1Local, SimLOD::LOD3National);
    }
}
