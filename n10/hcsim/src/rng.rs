//! SplitMix64-sådd xoshiro256**-liknande generator, kopierad från `n10/varld/flockdal.rs` rad 29–57
//! (egen kopia i hcsim; varld rörs inte).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(fro: u64) -> Rng {
        let mut z = fro ^ 0x5EED_F10C_DA15_0000;
        let mut s = [0u64; 4];
        for x in s.iter_mut() {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut y = z;
            y = (y ^ (y >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            y = (y ^ (y >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *x = y ^ (y >> 31);
        }
        Rng { s }
    }

    pub fn next(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    pub fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    pub fn under(&mut self, n: usize) -> usize {
        assert!(n > 0);
        ((self.f() * n as f64) as usize).min(n - 1)
    }

    pub fn blanda<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.under(i + 1);
            v.swap(i, j);
        }
    }

    pub fn lotta(&mut self, v: &[usize]) -> usize {
        v[self.under(v.len())]
    }

    /// Inre läge (för prov: turordningens lotteri ska inte röra världens följd).
    pub fn tillstand(&self) -> [u64; 4] {
        self.s
    }
}
