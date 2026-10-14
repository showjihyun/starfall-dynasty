//! Deterministic RNG (SplitMix64). Same seed → same galaxy, same daily missions,
//! on server and client alike. No external crate so the sequence never drifts.

#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn from_parts(parts: &[u64]) -> Self {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        for p in parts {
            h ^= *p;
            h = h.wrapping_mul(0x0100_0000_01B3);
            h ^= h >> 29;
        }
        Rng::new(h)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as u32
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u32) as usize]
    }

    /// Point uniformly inside a unit sphere.
    pub fn in_sphere(&mut self) -> glam::Vec3 {
        loop {
            let v = glam::Vec3::new(self.range(-1., 1.), self.range(-1., 1.), self.range(-1., 1.));
            if v.length_squared() <= 1.0 {
                return v;
            }
        }
    }
}

pub fn hash_str(s: &str) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a: Vec<u64> = (0..5).map({ let mut r = Rng::new(7); move |_| r.next_u64() }).collect();
        let b: Vec<u64> = (0..5).map({ let mut r = Rng::new(7); move |_| r.next_u64() }).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn f32_in_unit_range() {
        let mut r = Rng::new(1);
        for _ in 0..10_000 {
            let x = r.f32();
            assert!((0.0..1.0).contains(&x));
        }
    }
}
