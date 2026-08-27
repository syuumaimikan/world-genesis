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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_reproduces_identical_streams() {
        let mut a = DeterministicRng::seed_from_u64(0xDEAD_BEEF);
        let mut b = DeterministicRng::seed_from_u64(0xDEAD_BEEF);
        for _ in 0..64 {
            assert_eq!(a.next_f32(), b.next_f32());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = DeterministicRng::seed_from_u64(1);
        let mut b = DeterministicRng::seed_from_u64(2);
        let sum_a: f32 = (0..32).map(|_| a.next_f32()).sum();
        let sum_b: f32 = (0..32).map(|_| b.next_f32()).sum();
        assert!((sum_a - sum_b).abs() > 1e-3);
    }

    #[test]
    fn next_f32_stays_in_unit_interval() {
        let mut rng = DeterministicRng::seed_from_u64(7);
        for _ in 0..2000 {
            let v = rng.next_f32();
            assert!((0.0..=1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn next_f32_range_stays_within_bounds() {
        let mut rng = DeterministicRng::seed_from_u64(11);
        for _ in 0..2000 {
            let v = rng.next_f32_range(-3.5, 8.25);
            assert!((-3.5..=8.25).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn next_u32_range_is_half_open_and_covers_bounds() {
        let mut rng = DeterministicRng::seed_from_u64(13);
        let mut saw_min = false;
        let mut saw_max_minus_one = false;
        for _ in 0..2000 {
            let v = rng.next_u32_range(5, 9);
            assert!((5..9).contains(&v), "out of range: {v}");
            saw_min |= v == 5;
            saw_max_minus_one |= v == 8;
        }
        assert!(saw_min && saw_max_minus_one);
    }

    #[test]
    fn next_u32_range_returns_min_for_degenerate_ranges() {
        let mut rng = DeterministicRng::seed_from_u64(17);
        assert_eq!(rng.next_u32_range(4, 4), 4);
        assert_eq!(rng.next_u32_range(9, 2), 9);
    }

    #[test]
    fn clone_continues_the_same_stream() {
        let mut rng = DeterministicRng::seed_from_u64(23);
        rng.next_f32();
        let mut forked = rng.clone();
        assert_eq!(rng.next_f32(), forked.next_f32());
    }
}
