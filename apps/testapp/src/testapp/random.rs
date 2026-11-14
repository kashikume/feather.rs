use std::time::{SystemTime, UNIX_EPOCH};

/// A small pseudo-random number generator (xorshift32), enough for demo settings.
pub struct Random(u32);

impl Random {
    /// Seeded from the clock, so demos differ every run.
    pub fn from_clock() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Self::from_seed(nanos)
    }

    /// The same numbers for the same seed.
    pub fn from_seed(seed: u32) -> Self {
        Self(seed | 1)
    }

    /// A number in `0.0..1.0`.
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A number in `min..max`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next()
    }
}

#[cfg(test)]
mod tests {
    use super::Random;

    #[test]
    fn random_numbers_are_in_range_and_vary() {
        let mut random = Random::from_seed(12345);
        let values: Vec<f32> = (0..1000).map(|_| random.next()).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((0.4..0.6).contains(&mean), "{mean}");
    }

    #[test]
    fn range_stays_inside() {
        let mut random = Random::from_seed(7);
        assert!((0..1000)
            .map(|_| random.range(-2.0, 3.0))
            .all(|v| (-2.0..3.0).contains(&v)));
    }

    #[test]
    fn same_seed_same_numbers() {
        let (mut a, mut b) = (Random::from_seed(5), Random::from_seed(5));
        assert!((0..10).all(|_| a.next() == b.next()));
    }
}
