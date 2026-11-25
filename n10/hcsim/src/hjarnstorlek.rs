//! Hjärnans storlek per art (Håkans beslut 2026-11-22: "Hjärnkapaciteten ... måste vara efter storleken på hjärnan";
//! Alt 1679: storleken som tal per art och tiden mätt; Alt 1673: skalan från antal nervceller).
//!
//! Första steget: en hjärna med valfri storlek (`HjS`) som räknar exakt som `hjarna::Hj` (provet
//! `tests/hjarnstorlek1679.rs` kräver samma val och samma vikter bit för bit vid standardstorleken),
//! och skalan `z_ur_nervceller`. `sim.rs` använder fortfarande `Hj` med den fasta storleken; inget i världen ändras.

use crate::hjarna::gauss;
use crate::rng::Rng;
use crate::tal::{self, ETA, SIGMA0};

/// Antal sinnestal, tal i den inre bilden och handlingar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storlek {
    pub s: usize,
    pub z: usize,
    pub a: usize,
}

impl Storlek {
    /// Dagens storlek (Edmontosaurus i hcsim).
    pub const STANDARD: Storlek = Storlek { s: tal::S_N, z: tal::Z_N, a: tal::A_N };

    /// Kodaren W_k: Z × (S+1).
    pub fn nk(&self) -> usize {
        self.z * (self.s + 1)
    }
    /// Förutsägaren W_a: A × Z × (Z+1).
    pub fn np(&self) -> usize {
        self.a * self.z * (self.z + 1)
    }
    /// Avläsaren v: Z+1.
    pub fn nv(&self) -> usize {
        self.z + 1
    }
    pub fn n_gen(&self) -> usize {
        self.nk() + self.np() + self.nv()
    }
    /// Räknesteg per steg (samma formel som `hjarna::OPS`).
    pub fn ops(&self) -> usize {
        self.z * (self.s + 1) + self.a * self.z * (self.z + 1) + self.a * (self.z + 1) + self.z * (self.z + 1)
    }
}

/// Golv och tak för den inre bilden i skalan (Alt 1673, eget val).
pub const Z_GOLV: usize = 2;
pub const Z_TAK: usize = 8;

/// Skalan (Alt 1673, eget val): Z = 2 + (log10 N − 5), avrundat, begränsat till 2–8.
/// N är antalet nervceller i hela hjärnan. En rangordning, ingen avbildning: verkliga hjärnor skiljer sig med
/// många tiopotenser, skalan ger högst en faktor 4.
pub fn z_ur_nervceller(n: f64) -> usize {
    assert!(n.is_finite() && n > 0.0, "antalet nervceller ska vara större än 0");
    let z = (2.0 + (n.log10() - 5.0)).round();
    z.clamp(Z_GOLV as f64, Z_TAK as f64) as usize
}

/// Hjärna med valfri storlek. Räknar som `hjarna::Hj::valj` (samma uttryck i samma ordning), också bokföringen
/// av de tre kvadratfelen, så att den mätta tiden gäller samma arbete.
#[derive(Clone, Debug)]
pub struct HjS {
    pub st: Storlek,
    pub gen: Vec<f64>,
    pub wp: Vec<f64>,
    pub forra: Option<(Vec<f64>, usize, Vec<f64>)>,
    pub forra_ba: Option<Vec<f64>>,
    pub fel_sum: f64,
    pub fel_n: u64,
    pub senaste_fel: Option<f64>,
    pub senaste_fel_kopiering: Option<f64>,
    pub senaste_fel_fast: Option<f64>,
    pub steg: u64,
}

impl HjS {
    pub fn fran_gen(st: Storlek, gen: Vec<f64>) -> HjS {
        assert!(st.s > 0 && st.z > 0 && st.a > 0);
        assert_eq!(gen.len(), st.n_gen());
        let wp = gen[st.nk()..st.nk() + st.np()].to_vec();
        HjS {
            st,
            gen,
            wp,
            forra: None,
            forra_ba: None,
            fel_sum: 0.0,
            fel_n: 0,
            senaste_fel: None,
            senaste_fel_kopiering: None,
            senaste_fel_fast: None,
            steg: 0,
        }
    }

    pub fn slump(st: Storlek, g: &mut Rng) -> HjS {
        HjS::fran_gen(st, (0..st.n_gen()).map(|_| SIGMA0 * gauss(g)).collect())
    }

    pub fn kod(&self, s: &[f64]) -> Vec<f64> {
        let (sn, zn) = (self.st.s, self.st.z);
        assert_eq!(s.len(), sn);
        let mut z = vec![0.0; zn];
        for i in 0..zn {
            let r = &self.gen[i * (sn + 1)..(i + 1) * (sn + 1)];
            z[i] = (r[sn] + (0..sn).map(|j| r[j] * s[j]).sum::<f64>()).tanh();
        }
        z
    }

    pub fn forutsag(&self, z: &[f64], a: usize) -> Vec<f64> {
        let zn = self.st.z;
        let mut y = vec![0.0; zn];
        let b = a * zn * (zn + 1);
        for i in 0..zn {
            let r = &self.wp[b + i * (zn + 1)..b + (i + 1) * (zn + 1)];
            y[i] = r[zn] + (0..zn).map(|j| r[j] * z[j]).sum::<f64>();
        }
        y
    }

    pub fn fast_del(&self, a: usize) -> Vec<f64> {
        let zn = self.st.z;
        let b = a * zn * (zn + 1);
        (0..zn).map(|i| self.wp[b + i * (zn + 1) + zn]).collect()
    }

    pub fn varde(&self, y: &[f64]) -> f64 {
        let zn = self.st.z;
        let v = &self.gen[self.st.nk() + self.st.np()..];
        v[zn] + (0..zn).map(|j| v[j] * y[j]).sum::<f64>()
    }

    fn kvad_diff(a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| {
                let d = x - y;
                d * d
            })
            .sum()
    }

    fn lar_fran_forra(&mut self, z: &[f64], lar: bool) {
        let zn = self.st.z;
        self.senaste_fel = None;
        self.senaste_fel_kopiering = None;
        self.senaste_fel_fast = None;
        if let Some((zf, a, yf)) = &self.forra {
            let fel: Vec<f64> = (0..zn).map(|i| yf[i] - z[i]).collect();
            let kvad: f64 = fel.iter().map(|x| x * x).sum();
            let kvad_kop = Self::kvad_diff(zf, z);
            let kvad_fast = match &self.forra_ba {
                Some(ba) => Self::kvad_diff(ba, z),
                None => Self::kvad_diff(&self.fast_del(*a), z),
            };
            self.fel_sum += kvad;
            self.fel_n += 1;
            self.senaste_fel = Some(kvad);
            self.senaste_fel_kopiering = Some(kvad_kop);
            self.senaste_fel_fast = Some(kvad_fast);
            if lar {
                let b = a * zn * (zn + 1);
                for i in 0..zn {
                    for j in 0..=zn {
                        let x = if j < zn { zf[j] } else { 1.0 };
                        self.wp[b + i * (zn + 1) + j] -= ETA * fel[i] * x;
                    }
                }
            }
        }
    }

    /// Lär förutsägaren (om `lar`) och välj handlingen med högst värde; lika ger den första (0 = stå kvar).
    pub fn valj(&mut self, s: &[f64], lar: bool) -> usize {
        let z = self.kod(s);
        self.lar_fran_forra(&z, lar);
        let (mut ba, mut bv, mut by) = (0usize, f64::NEG_INFINITY, vec![0.0; self.st.z]);
        for a in 0..self.st.a {
            let y = self.forutsag(&z, a);
            let v = self.varde(&y);
            if v > bv {
                ba = a;
                bv = v;
                by = y;
            }
        }
        self.forra_ba = Some(self.fast_del(ba));
        self.forra = Some((z, ba, by));
        self.steg += 1;
        ba
    }
}
