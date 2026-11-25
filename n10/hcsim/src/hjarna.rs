//! JEPA-liknande hjärna, kopierad från `n10/varld/hjarna.rs` med parametriserade storlekar.
//! Handlingar = 9; sinnen = 19 (regel 14; Alt 1107: avläsaren ärvd, förutsägaren lär sig).

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::tal::{self, A_N, ETA, S_N, SIGMA0, SIGMA_MUT, Z_N};

/// Lotta bland handlingar med samma (exakta) maxvärde.
pub fn lott_bland_max(varden: &[f64], g: &mut Rng) -> usize {
    assert!(!varden.is_empty());
    let mut bv = f64::NEG_INFINITY;
    for &v in varden {
        if v > bv {
            bv = v;
        }
    }
    let mut kand = Vec::with_capacity(varden.len());
    for (a, &v) in varden.iter().enumerate() {
        if v == bv {
            kand.push(a);
        }
    }
    kand[g.under(kand.len())]
}

/// Kodare W_k: Z × (S+1).
pub const NK: usize = Z_N * (S_N + 1);
/// Förutsägare W_a: A × Z × (Z+1).
pub const NP: usize = A_N * Z_N * (Z_N + 1);
/// Avläsare v: Z+1.
pub const NV: usize = Z_N + 1;
pub const N_GEN: usize = NK + NP + NV;

/// Räknesteg per steg (samma formel som hjarna.rs OPS, med våra S_N/A_N).
pub const OPS: usize = Z_N * (S_N + 1) + A_N * Z_N * (Z_N + 1) + A_N * (Z_N + 1) + Z_N * (Z_N + 1);

pub fn gauss(g: &mut Rng) -> f64 {
    let u = g.f().max(1e-300);
    let w = g.f();
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * w).cos()
}

/// Generna: [W_k, W_a-start, v]. Livet lär bara `wp` (förutsägaren).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hj {
    pub gen: Vec<f64>,
    pub wp: Vec<f64>,
    pub forra: Option<(Vec<f64>, usize, Vec<f64>)>,
    /// Fast del `b_a` vid förutsägelsen (samma vikter som gav `yf` i `forra`; Alt 1462).
    #[serde(default)]
    pub forra_ba: Option<Vec<f64>>,
    pub fel_sum: [f64; 2],
    pub fel_n: [u64; 2],
    pub steg: u64,
    /// Senaste förutsägelsefelet (kvadratsumma); None första steget.
    pub senaste_fel: Option<f64>,
    /// Handlingen som senaste_fel hör till (föregående val).
    pub senaste_fel_handling: Option<usize>,
    /// Senaste kopieringsfelet `|zf − z|²` (Alt 1462); None första steget.
    #[serde(default)]
    pub senaste_fel_kopiering: Option<f64>,
    /// Senaste felet mot bara den fasta delen `|b_a − z|²` (Alt 1462).
    #[serde(default)]
    pub senaste_fel_fast: Option<f64>,
}

impl Hj {
    pub fn fran_gen(gen: Vec<f64>) -> Hj {
        assert_eq!(gen.len(), N_GEN);
        let wp = gen[NK..NK + NP].to_vec();
        Hj {
            gen,
            wp,
            forra: None,
            forra_ba: None,
            fel_sum: [0.0; 2],
            fel_n: [0; 2],
            steg: 0,
            senaste_fel: None,
            senaste_fel_handling: None,
            senaste_fel_kopiering: None,
            senaste_fel_fast: None,
        }
    }

    pub fn slump(g: &mut Rng) -> Hj {
        Hj::fran_gen((0..N_GEN).map(|_| SIGMA0 * gauss(g)).collect())
    }

    /// Ungens gener: varje tal från modern eller fadern (hälften var), plus N(0, SIGMA_MUT).
    pub fn arv(mor: &[f64], far: &[f64], g: &mut Rng) -> Hj {
        Hj::fran_gen(
            mor.iter()
                .zip(far)
                .map(|(&m, &f)| (if g.f() < 0.5 { m } else { f }) + SIGMA_MUT * gauss(g))
                .collect(),
        )
    }

    pub fn kod(&self, s: &[f64]) -> Vec<f64> {
        assert_eq!(s.len(), S_N);
        let mut z = vec![0.0; Z_N];
        for i in 0..Z_N {
            let r = &self.gen[i * (S_N + 1)..(i + 1) * (S_N + 1)];
            z[i] = (r[S_N] + (0..S_N).map(|j| r[j] * s[j]).sum::<f64>()).tanh();
        }
        z
    }

    pub fn forutsag(&self, z: &[f64], a: usize) -> Vec<f64> {
        let mut y = vec![0.0; Z_N];
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            let r = &self.wp[b + i * (Z_N + 1)..b + (i + 1) * (Z_N + 1)];
            y[i] = r[Z_N] + (0..Z_N).map(|j| r[j] * z[j]).sum::<f64>();
        }
        y
    }

    /// Fast del `b_a` för handlingen `a` (sista kolumnen i varje rad av `W_a`; läser bara).
    pub fn fast_del(&self, a: usize) -> Vec<f64> {
        let mut ba = vec![0.0; Z_N];
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            ba[i] = self.wp[b + i * (Z_N + 1) + Z_N];
        }
        ba
    }

    fn kvad_diff(a: &[f64], b: &[f64]) -> f64 {
        (0..Z_N).map(|i| {
            let d = a[i] - b[i];
            d * d
        }).sum()
    }

    pub fn varde(&self, y: &[f64]) -> f64 {
        let v = &self.gen[NK + NP..];
        v[Z_N] + (0..Z_N).map(|j| v[j] * y[j]).sum::<f64>()
    }

    /// Avläsarens v0 (samma för alla handlingar; Alt 1251).
    pub fn v0(&self) -> f64 {
        self.gen[NK + NP + Z_N]
    }

    /// Sätt v0 (prov; rör inte övriga gener).
    pub fn satt_v0(&mut self, v0: f64) {
        self.gen[NK + NP + Z_N] = v0;
    }

    /// Värdets delar per handling: `v · W_a · z`, `v · b_a`, och hela `v · y_a + v0`.
    /// Läser bara; ingen lärning. `v0` är samma för alla a.
    pub fn vardets_delar(&self, s: &[f64]) -> (Vec<f64>, [f64; A_N], [f64; A_N], [f64; A_N]) {
        let z = self.kod(s);
        let v = &self.gen[NK + NP..];
        let v0 = v[Z_N];
        let mut lage = [0.0; A_N];
        let mut fast = [0.0; A_N];
        let mut vard = [0.0; A_N];
        for a in 0..A_N {
            let b = a * Z_N * (Z_N + 1);
            let mut wz = [0.0; Z_N];
            let mut bias = [0.0; Z_N];
            for i in 0..Z_N {
                let r = &self.wp[b + i * (Z_N + 1)..b + (i + 1) * (Z_N + 1)];
                wz[i] = (0..Z_N).map(|j| r[j] * z[j]).sum::<f64>();
                bias[i] = r[Z_N];
            }
            lage[a] = (0..Z_N).map(|i| v[i] * wz[i]).sum::<f64>();
            fast[a] = (0..Z_N).map(|i| v[i] * bias[i]).sum::<f64>();
            vard[a] = lage[a] + fast[a] + v0;
        }
        (z, vard, lage, fast)
    }

    /// Uppdatera förutsägaren från föregående gissning (samma väg för `lar` och de nya förmågorna).
    /// De tre kvadratfelen (Alt 1462) räknas **före** viktuppdateringen; valet och vikternas
    /// uppdatering är oförändrade.
    fn lar_fran_forra(&mut self, z: &[f64], lar: bool, halva: usize) {
        self.senaste_fel = None;
        self.senaste_fel_handling = None;
        self.senaste_fel_kopiering = None;
        self.senaste_fel_fast = None;
        if let Some((zf, a, yf)) = &self.forra {
            let fel: Vec<f64> = (0..Z_N).map(|i| yf[i] - z[i]).collect();
            let kvad: f64 = fel.iter().map(|x| x * x).sum();
            let kvad_kop = Self::kvad_diff(zf, z);
            let kvad_fast = match &self.forra_ba {
                Some(ba) => Self::kvad_diff(ba, z),
                None => Self::kvad_diff(&self.fast_del(*a), z),
            };
            self.fel_sum[halva] += kvad;
            self.fel_n[halva] += 1;
            self.senaste_fel = Some(kvad);
            self.senaste_fel_handling = Some(*a);
            self.senaste_fel_kopiering = Some(kvad_kop);
            self.senaste_fel_fast = Some(kvad_fast);
            if lar {
                let b = a * Z_N * (Z_N + 1);
                for i in 0..Z_N {
                    for j in 0..=Z_N {
                        let x = if j < Z_N { zf[j] } else { 1.0 };
                        self.wp[b + i * (Z_N + 1) + j] -= ETA * fel[i] * x;
                    }
                }
            }
        }
    }

    /// Mutant för kontroll 4: felet räknas **efter** viktuppdateringen (omräknad `yf` ur nya vikter).
    /// Används bara i prov; rör inte produktionsvägen.
    pub fn lar_fran_forra_mutant_efter_uppdatering(&mut self, z: &[f64], lar: bool, halva: usize) {
        self.senaste_fel = None;
        self.senaste_fel_handling = None;
        self.senaste_fel_kopiering = None;
        self.senaste_fel_fast = None;
        if let Some((zf, a, yf)) = self.forra.clone() {
            let fel: Vec<f64> = (0..Z_N).map(|i| yf[i] - z[i]).collect();
            if lar {
                let b = a * Z_N * (Z_N + 1);
                for i in 0..Z_N {
                    for j in 0..=Z_N {
                        let x = if j < Z_N { zf[j] } else { 1.0 };
                        self.wp[b + i * (Z_N + 1) + j] -= ETA * fel[i] * x;
                    }
                }
            }
            // Efter uppdateringen: omräknad förutsägelse och fast del ur de nya vikterna.
            let y_ny = self.forutsag(&zf, a);
            let ba_ny = self.fast_del(a);
            let kvad = Self::kvad_diff(&y_ny, z);
            let kvad_kop = Self::kvad_diff(&zf, z);
            let kvad_fast = Self::kvad_diff(&ba_ny, z);
            self.fel_sum[halva] += kvad;
            self.fel_n[halva] += 1;
            self.senaste_fel = Some(kvad);
            self.senaste_fel_handling = Some(a);
            self.senaste_fel_kopiering = Some(kvad_kop);
            self.senaste_fel_fast = Some(kvad_fast);
        }
    }

    /// Lär förutsägaren (om `lar`), välj handling med högst värde; lika → stå kvar (0).
    /// Avläsaren `v` och lika-regeln ändras inte här (Alt 1107 / underlaget).
    pub fn valj(&mut self, s: &[f64], lar: bool, halva: usize) -> usize {
        let z = self.kod(s);
        self.lar_fran_forra(&z, lar, halva);
        let (mut ba, mut bv, mut by) = (0usize, f64::NEG_INFINITY, vec![0.0; Z_N]);
        for a in 0..A_N {
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

    /// Som `valj` men tvingar handling `a` (Alt 1463: `lar_utforsk`:s slumpgren).
    /// Förutsägaren lär sig av den valda handlingen precis som i `valj`; `lar`:s väg oförändrad.
    pub fn lar_och_satt(&mut self, s: &[f64], lar: bool, halva: usize, a: usize) {
        assert!(a < A_N);
        let z = self.kod(s);
        self.lar_fran_forra(&z, lar, halva);
        let by = self.forutsag(&z, a);
        self.forra_ba = Some(self.fast_del(a));
        self.forra = Some((z, a, by));
        self.steg += 1;
    }

    /// Som `valj`, men lika värden lottas med `g` (djurets egen följd).
    pub fn valj_lott(&mut self, s: &[f64], lar: bool, halva: usize, g: &mut Rng) -> usize {
        let z = self.kod(s);
        self.lar_fran_forra(&z, lar, halva);
        let mut varden = [0.0; A_N];
        let mut ys: Vec<Vec<f64>> = Vec::with_capacity(A_N);
        for a in 0..A_N {
            let y = self.forutsag(&z, a);
            varden[a] = self.varde(&y);
            ys.push(y);
        }
        let ba = lott_bland_max(&varden, g);
        self.forra_ba = Some(self.fast_del(ba));
        self.forra = Some((z, ba, ys[ba].clone()));
        self.steg += 1;
        ba
    }

    /// Förutsägaren lär sig som i `lar`; handlingen väljs av `poang(y)` med lottning vid lika.
    /// Returnerar (vald handling, den handling `v` hade valt på samma y:n, y för vald).
    pub fn valj_med_poang<F>(
        &mut self,
        s: &[f64],
        lar: bool,
        halva: usize,
        g: &mut Rng,
        mut poang: F,
    ) -> (usize, usize, Vec<f64>)
    where
        F: FnMut(&[f64]) -> f64,
    {
        let z = self.kod(s);
        self.lar_fran_forra(&z, lar, halva);
        let mut poang_y = [0.0; A_N];
        let mut v_y = [0.0; A_N];
        let mut ys: Vec<Vec<f64>> = Vec::with_capacity(A_N);
        for a in 0..A_N {
            let y = self.forutsag(&z, a);
            poang_y[a] = poang(&y);
            v_y[a] = self.varde(&y);
            ys.push(y);
        }
        let ba = lott_bland_max(&poang_y, g);
        // `v` använder samma lika-regel som `lar`: strikt störst, annars stå kvar (0).
        let mut v_ba = 0usize;
        let mut v_bv = f64::NEG_INFINITY;
        for a in 0..A_N {
            if v_y[a] > v_bv {
                v_ba = a;
                v_bv = v_y[a];
            }
        }
        let by = ys[ba].clone();
        self.forra_ba = Some(self.fast_del(ba));
        self.forra = Some((z, ba, by.clone()));
        self.steg += 1;
        (ba, v_ba, by)
    }
}

/// Energi för hjärnans räknesteg: `pris = HJ_ANDEL · kost / OPS`, totalt `OPS · pris = HJ_ANDEL · kost`.
pub fn hjarnkost(kost: f64) -> f64 {
    let pris = tal::HJ_ANDEL * kost / OPS as f64;
    OPS as f64 * pris
}
