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
