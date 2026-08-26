use rand::RngCore;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::SeedableRng;

#[derive(Clone)]
pub struct DeterministicRng {
    rng: ChaCha8Rng,
}

impl DeterministicRng {
    pub fn seed_from_u64(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.rng.next_u32() as f32) / (u32::MAX as f32)
    }

    #[inline]
    pub fn next_f32_range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    #[inline]
    pub fn next_u32_range(&mut self, min: u32, max: u32) -> u32 {
        if min >= max {
            min
        } else {
            min + (self.rng.next_u32() % (max - min))
        }
    }
}
