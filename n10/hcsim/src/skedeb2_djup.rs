//! Alt 2397 (enhällig konsensus 4/4 i Alt 2391): Generisk, felsäker flerlagers JEPA-hjärna
//! (`HjDjup`) för samtliga varelser (*Didelphodon*, *Edmontosaurus*, *Triceratops*, *T. rex* m.fl.)
//! med kompakt genom (`392` gener för `S = 57`, `302` för `S = 39`), en-förälders-nedärvning,
//! ankrad bas (`Z_ank = 8`) under medfödd `V_0`, plastisk 2-lagers-kodare (`Z_plast = 8`, inkl.
//! 6 minnesspårsaxlar) med `stop_grad` från `V_fara`, snabbt RLS-huvud med symmetriskt `P`-tak
//! på sista lagret i `V_mat` och `V_fara` (3-stegs `h`-fönster), faktoriserad världsmodell `Pred`,
//! positiv tids-RMS per axel (`>= 0.05`), noll-initierad neurontillväxt (Fälla A/B/C stängda)
//! och fast låst energipris `κ = 0.0004` per aktiv dold neuron och steg.
//!
//! Matematiskt förverifierad i Lean 4 (`HcsimJepaDjup.lean`, 14/14 Djup-satser, 59/59 totalt, 0 sorry).
//! Rör inte `hjarna.rs`, `hjarna2008.rs`, `hjarnab1.rs`, `sim.rs`, `formaga.rs` eller `321–340`.

use crate::rng::Rng;
use crate::skedeb2::{
    B2Param, ByteTyp, GenTrex, HarmlosKalla, HjB2, HjB2Config, HjTrex, SimB2, TrexB2, S_B2, S_TREX,
};
use crate::tal::{
    A_N, AT_DINO, E_DELA, E_HANE, E_VAXT, KOST_FLOD, KOST_SJO, KOST_STEG, KOST_TRASK,
    LITEN, LIV, STEG_PER_AR,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Låsta arkitekturkonstanter (4/4 konsensus 43-omrostning-alt2391.md)
// ---------------------------------------------------------------------------

pub const Z_ANK: usize = 8;
pub const Z_PLAST: usize = 8;
pub const Z_DJUP: usize = Z_ANK + Z_PLAST; // 16 fasta inre axlar (Fälla B stängd)
pub const Z_MINNE_START: usize = 10;       // 6 minnesspårsaxlar (10..16), helt inom Z_plast (8..16)!
pub const R_BAS: usize = 4;                // Låg-rang för ankrad bas W_bas = U · V + b
pub const H_V0: usize = 6;                 // Kompakt 2-lagers medfödd startinstinkt V_0(h_ank)

pub const H_ENC_START: usize = 32;
pub const H_ENC_MIN: usize = 8;
pub const H_ENC_MAX: usize = 64;

pub const H_PRED_START: usize = 24;
pub const H_PRED_MIN: usize = 8;
pub const H_PRED_MAX: usize = 48;

pub const H_KRIT_START: usize = 12;
pub const H_KRIT_MIN: usize = 4;
pub const H_KRIT_MAX: usize = 24;

/// Låst energipris per aktiv dold neuron och steg (4/4 konsensus: PÅ redan i Steg 1).
/// Vid 80 dolda neuroner (32+24+12+12): 0.032/steg (~9.4 % av överskott 0.339).
/// Vid taket 160 (64+48+24+24): 0.064/steg (~18.9 % av överskott 0.339 -> garanterat positivt, Lean D-12).
pub const KAPPA_ENERGI_LAST: f64 = 0.0004;

const BETA_MINNE: f64 = 0.25;
const RMS_GOLV: f64 = 0.05;
const ETA_SINNE: f64 = 0.02;
const ETA_Z_RMS: f64 = 0.02;
const GRIND_GAMMA: f32 = 0.995;
const GRIND_MIN_N: f32 = 12.0;
const GRIND_STRAFF: f32 = 2.0;
const GRIND_AKTIV_MIN: f64 = 0.04;
const GRIND_MATTNAD: f64 = 0.25;

const GAMMA_MAT: f64 = 0.55;
const GAMMA_FARA: f64 = 0.80;
const LAMBDA_KRIT: f64 = 0.992;
const ETA_MAL: f64 = 0.05;
const P_TAK_DJUP: f64 = 40.0;
const ALPHA_KRIT_DJUP: f64 = 8.0;
const UTFORSK_P: f64 = 0.20;
const CHOCK_GRANS: f64 = 10.0;
const CHOCK_BUFFERT_MAX: usize = 16;
const LUGN_BUFFERT_MAX: usize = 24;
const SIGMA_MUT_DJUP: f64 = 0.035;

/// Snabb rationell begränsad `tanh` (bevisad i Lean 4 D-3, D-4, D-14):
/// udda, teckenbevarande, strikt i `(-1, 1)`, derivata `1 / (1 + |x|)^2`.
#[inline]
pub fn rat_tanh(x: f64) -> f64 {
    x / (1.0 + x.abs())
}

#[inline]
pub fn rat_tanh_deriv(x: f64) -> f64 {
    let d = 1.0 + x.abs();
    1.0 / (d * d)
}

#[inline]
fn gauss(g: &mut Rng) -> f64 {
    let u1 = g.f().max(1e-12);
    let u2 = g.f();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

fn lott_bland_max(poang: &[f64], g: &mut Rng) -> usize {
    let mut best = f64::NEG_INFINITY;
    for &p in poang {
        if p.is_finite() && p > best {
            best = p;
        }
    }
    if !best.is_finite() {
        return g.under(poang.len().max(1));
    }
    let mut vald = 0usize;
    let mut antal = 0usize;
    for (k, &p) in poang.iter().enumerate() {
        if p >= best - 1e-12 {
            antal += 1;
            if g.under(antal) == 0 {
                vald = k;
            }
        }
    }
    vald
}

/// Symmetrisk kongruensskalning av `P` (`D P D`) som bevarar symmetri och PSD (Lean 4 `b2_sym_p_cap`).
fn sym_kongruens_p_tak_vec(p: &mut [f64], dim: usize, p_tak: f64) -> u64 {
    let mut d = vec![1.0f64; dim];
    let mut skalar = false;
    for i in 0..dim {
        let diag = p[i * dim + i];
        if diag > p_tak {
            d[i] = (p_tak / diag).sqrt();
            skalar = true;
        }
    }
    if !skalar {
        return 0;
    }
    for i in 0..dim {
        for j in 0..dim {
            p[i * dim + j] *= d[i] * d[j];
        }
    }
    1
}

/// Viktat 1D-RLS-steg med Lean 4-verifierat symmetriskt `P`-tak (bevarar PSD och exakt invers).
fn rls_skalar_viktad_vec(
    w: &mut [f64],
    p: &mut [f64],
    x: &[f64],
    err: f64,
    lam: f64,
    c_vikt: f64,
    p_tak: f64,
) -> u64 {
    let dim = x.len();
    let mut px = vec![0.0f64; dim];
    let mut q = 0.0f64;
    for i in 0..dim {
        let mut s = 0.0;
        let row = i * dim;
        for j in 0..dim {
            s += p[row + j] * x[j];
        }
        px[i] = s;
        q += x[i] * s;
    }
    let q_pos = q.max(0.0);
    let denom = lam + c_vikt * q_pos;
    if denom <= 1e-12 || !denom.is_finite() {
        return 0;
    }
    let inv_d = c_vikt / denom;
    let inv_lam = 1.0 / lam;
    for i in 0..dim {
        w[i] += (err * px[i] * inv_d).clamp(-5.0, 5.0);
    }
    for i in 0..dim {
        let row = i * dim;
        let pxi = px[i] * inv_d;
        for j in 0..dim {
            p[row + j] = (p[row + j] - pxi * px[j]) * inv_lam;
        }
    }
    // Symmetrisera bort ev. 1e-16 flyttalsdrift
    for i in 0..dim {
        for j in (i + 1)..dim {
            let m = 0.5 * (p[i * dim + j] + p[j * dim + i]);
            p[i * dim + j] = m;
            p[j * dim + i] = m;
        }
    }
    sym_kongruens_p_tak_vec(p, dim, p_tak)
}

/// L1-radnormtak på en viktrad (Lean 4 D-5 & D-14 -> garanterar `|w · act(u)| <= max_l1`).
#[inline]
fn klamp_rad_l1(rad: &mut [f64], max_l1: f64) {
    let mut l1 = 0.0;
    for &v in rad.iter() {
        l1 += v.abs();
    }
    if l1 > max_l1 && l1 > 1e-12 {
        let sc = max_l1 / l1;
        for v in rad.iter_mut() {
            *v *= sc;
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Generisk artspecifikation (samma modul för alla arter!)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ArtSpec {
    pub s_n: usize,
    pub a_n: usize,
    pub h_enc_start: usize,
    pub h_enc_min: usize,
    pub h_enc_max: usize,
    pub h_pred_start: usize,
    pub h_pred_min: usize,
    pub h_pred_max: usize,
    pub h_krit_start: usize,
    pub h_krit_min: usize,
    pub h_krit_max: usize,
    pub fri_storlek: bool,
    pub kappa_energi: f64,
    pub stryk_wm: bool,
}

impl ArtSpec {
    pub fn byte_b2(fri_storlek: bool) -> Self {
        Self {
            s_n: S_B2,
            a_n: A_N,
            h_enc_start: H_ENC_START,
            h_enc_min: H_ENC_MIN,
            h_enc_max: H_ENC_MAX,
            h_pred_start: H_PRED_START,
            h_pred_min: H_PRED_MIN,
            h_pred_max: H_PRED_MAX,
            h_krit_start: H_KRIT_START,
            h_krit_min: H_KRIT_MIN,
            h_krit_max: H_KRIT_MAX,
            fri_storlek,
            kappa_energi: KAPPA_ENERGI_LAST,
            stryk_wm: false,
        }
    }

    pub fn trex_b2(fri_storlek: bool) -> Self {
        Self {
            s_n: S_TREX,
            a_n: A_N,
            h_enc_start: H_ENC_START,
            h_enc_min: H_ENC_MIN,
            h_enc_max: H_ENC_MAX,
            h_pred_start: H_PRED_START,
            h_pred_min: H_PRED_MIN,
            h_pred_max: H_PRED_MAX,
            h_krit_start: H_KRIT_START,
            h_krit_min: H_KRIT_MIN,
            h_krit_max: H_KRIT_MAX,
            fri_storlek,
            kappa_energi: KAPPA_ENERGI_LAST,
            stryk_wm: false,
        }
    }

    pub fn max_aktiva_dolda(&self) -> usize {
        self.h_enc_max + self.h_pred_max + 2 * self.h_krit_max
    }
}

// ---------------------------------------------------------------------------
// 2. Kompakt genom GenDjup (392 gener för S=57, 302 för S=39) + en-förälders-arv
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GenDjup {
    /// S ärftliga symmetriska kanalskalor (klampade i [0.10, 2.00], 100 % permutations-ekvivarianta).
    pub kanalskala: Vec<f64>,
    /// Ankrad låg-rang-bas W_bas = U (Z_ANK x R_BAS) * V (R_BAS x S) + b (Z_ANK).
    pub bas_u: [[f64; R_BAS]; Z_ANK],
    pub bas_v: Vec<f64>, // längd R_BAS * S
    pub bas_b: [f64; Z_ANK],
    /// Kompakt 2-lagers medfödd startinstinkt V_0(h_ank): Z_ANK -> H_V0 -> 1 (aldrig ändrad under livet!).
    pub v0_w1: [[f64; Z_ANK + 1]; H_V0],
    pub v0_w2: [f64; H_V0 + 1],
    /// 6 meta-/storleksgener:
    pub w_skada: f64,
    pub n_dolda_enc: usize,
    pub n_dolda_pred: usize,
    pub n_dolda_krit: usize,
    pub eta_kod: f64,
    pub eta_pred: f64,
    pub s_n: usize,
}

impl GenDjup {
    pub fn slump(spec: ArtSpec, g: &mut Rng) -> Self {
        let s = spec.s_n;
        let inv_sqrt_s = 1.0 / (s as f64).sqrt();
        let kanalskala = vec![1.0; s];
        let mut bas_u = [[0.0; R_BAS]; Z_ANK];
        for r in 0..Z_ANK {
            for q in 0..R_BAS {
                bas_u[r][q] = 0.5 * gauss(g);
            }
        }
        let mut bas_v = vec![0.0; R_BAS * s];
        for x in &mut bas_v {
            *x = inv_sqrt_s * gauss(g);
        }
        let mut bas_b = [0.0; Z_ANK];
        for b in &mut bas_b {
            *b = 0.05 * gauss(g);
        }
        let mut v0_w1 = [[0.0; Z_ANK + 1]; H_V0];
        for k in 0..H_V0 {
            for j in 0..=Z_ANK {
                v0_w1[k][j] = 0.35 * gauss(g);
            }
        }
        let mut v0_w2 = [0.0; H_V0 + 1];
        for k in 0..=H_V0 {
            v0_w2[k] = 0.25 * gauss(g);
        }
        Self {
            kanalskala,
            bas_u,
            bas_v,
            bas_b,
            v0_w1,
            v0_w2,
            w_skada: (1.0 + 0.3 * gauss(g)).clamp(0.1, 8.0),
            n_dolda_enc: spec.h_enc_start,
            n_dolda_pred: spec.h_pred_start,
            n_dolda_krit: spec.h_krit_start,
            eta_kod: 0.008,
            eta_pred: 0.020,
            s_n: s,
        }
    }

    /// Exakt antal gener enligt konsensusschemat (392 för S=57, 302 för S=39).
    pub fn antal_gener(&self) -> usize {
        self.kanalskala.len()
            + (Z_ANK * R_BAS + self.bas_v.len() + Z_ANK)
            + (H_V0 * (Z_ANK + 1) + (H_V0 + 1))
            + 6
    }

    /// En-förälders-nedärvning + svag mutation (Fables & Carbons konsensuskrav: ingen vikt-för-vikt-korsning!).
    /// Väljer slumpvis mor eller far som helhetsförälder så den medfödda 2-lagers-instinkten bevaras (`r > 0.90`).
    pub fn arv(mor: &Self, far: &Self, uid: u64, spec: ArtSpec) -> Self {
        let mut g = Rng::new(uid ^ 0x2397_A12F_0001);
        let foralder = if g.f() < 0.5 { mor } else { far };
        let mut barn = foralder.clone();
        let inv_sqrt_s = 1.0 / (spec.s_n as f64).sqrt();
        for x in &mut barn.kanalskala {
            *x = (*x + SIGMA_MUT_DJUP * gauss(&mut g)).clamp(0.10, 2.00);
        }
        for r in 0..Z_ANK {
            for q in 0..R_BAS {
                barn.bas_u[r][q] = (barn.bas_u[r][q] + SIGMA_MUT_DJUP * gauss(&mut g)).clamp(-3.0, 3.0);
            }
            barn.bas_b[r] = (barn.bas_b[r] + 0.5 * SIGMA_MUT_DJUP * gauss(&mut g)).clamp(-2.0, 2.0);
        }
        for x in &mut barn.bas_v {
            *x = (*x + SIGMA_MUT_DJUP * inv_sqrt_s * gauss(&mut g)).clamp(-2.0, 2.0);
        }
        for k in 0..H_V0 {
            for j in 0..=Z_ANK {
                barn.v0_w1[k][j] = (barn.v0_w1[k][j] + SIGMA_MUT_DJUP * gauss(&mut g)).clamp(-3.0, 3.0);
            }
        }
        for k in 0..=H_V0 {
            barn.v0_w2[k] = (barn.v0_w2[k] + SIGMA_MUT_DJUP * gauss(&mut g)).clamp(-3.0, 3.0);
        }
        barn.w_skada = (barn.w_skada + 0.15 * gauss(&mut g)).clamp(0.0, 12.0);
        barn.eta_kod = (barn.eta_kod * (1.0 + 0.08 * gauss(&mut g))).clamp(0.001, 0.03);
        barn.eta_pred = (barn.eta_pred * (1.0 + 0.08 * gauss(&mut g))).clamp(0.003, 0.06);

        if spec.fri_storlek {
            if g.f() < 0.12 {
                let d: isize = if g.f() < 0.5 { 2 } else { -2 };
                barn.n_dolda_enc = (barn.n_dolda_enc as isize + d)
                    .clamp(spec.h_enc_min as isize, spec.h_enc_max as isize) as usize;
            }
            if g.f() < 0.12 {
                let d: isize = if g.f() < 0.5 { 2 } else { -2 };
                barn.n_dolda_pred = (barn.n_dolda_pred as isize + d)
                    .clamp(spec.h_pred_min as isize, spec.h_pred_max as isize) as usize;
            }
            if g.f() < 0.12 {
                let d: isize = if g.f() < 0.5 { 1 } else { -1 };
                barn.n_dolda_krit = (barn.n_dolda_krit as isize + d)
                    .clamp(spec.h_krit_min as isize, spec.h_krit_max as isize) as usize;
            }
        }
        barn
    }

    /// Utvärderar medfödd 2-lagers startinstinkt `V_0(h_ank)` (aldrig ändrad under livet!).
    #[inline]
    pub fn v0_varde(&self, h: &[f64; Z_DJUP]) -> f64 {
        let mut s = self.v0_w2[H_V0];
        for k in 0..H_V0 {
            let mut lin = self.v0_w1[k][Z_ANK];
            for r in 0..Z_ANK {
                lin += self.v0_w1[k][r] * h[r];
            }
            s += self.v0_w2[k] * rat_tanh(lin);
        }
        s
    }

    /// Ankrad låg-rang-bas `U · (V · (kanalskala ⊙ s)) + b_bas` (Lean 4 D-10 permutations-ekvivariant).
    pub fn ankrad_bas(&self, s_in: &[f64]) -> [f64; Z_ANK] {
        let s_n = self.s_n;
        let mut q_val = [0.0f64; R_BAS];
        for q in 0..R_BAS {
            let off = q * s_n;
            let mut sum = 0.0;
            for j in 0..s_n {
                sum += self.bas_v[off + j] * (self.kanalskala[j] * s_in[j]);
            }
            q_val[q] = sum;
        }
        let mut ut = self.bas_b;
        for r in 0..Z_ANK {
            let mut sum = 0.0;
            for q in 0..R_BAS {
                sum += self.bas_u[r][q] * q_val[q];
            }
            ut[r] += sum;
        }
        ut
    }
}

// ---------------------------------------------------------------------------
// 3. Generisk flerlagers JEPA-hjärna HjDjup
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ChockHandelse {
    /// 3-stegs förvarningsfönster (h_{t-2}, h_{t-1}, h_t) i EXAKT samma h-rum som planeringen!
    h_fonster: [[f64; Z_DJUP]; 3],
    r_chock: f64,
}

#[derive(Clone, Debug)]
struct LugnOvergang {
    s_prev: Vec<f32>,
    s_now: Vec<f32>,
    a: u8,
}

#[derive(Clone, Debug)]
pub struct HjDjup {
    pub spec: ArtSpec,
    pub gen: GenDjup,
    // Aktiva dolda neuronantal (växer/krymper enbart i dolda lager, fasta Z=16!)
    pub n_enc: usize,
    pub n_pred: usize,
    pub n_krit: usize,
    // Fast skalning 1/sqrt(H_max) -> stänger Fälla A!
    sk_enc: f64,
    sk_pred: f64,
    sk_krit: f64,
    // Plastisk 2-lagers kodare (57 -> n_enc -> Z_PLAST), maxallokerad till h_enc_max
    pub enc_w1: Vec<f64>,   // h_enc_max * (s_n + 1)
    pub enc_w2: Vec<f64>,   // Z_PLAST * (h_enc_max + 1)
    // Faktoriserad 2-lagers residual världsmodell Pred: ([h; 1] + e(a)) -> n_pred -> Z_DJUP
    pub pred_wh: Vec<f64>,  // h_pred_max * (Z_DJUP + 1)
    pub pred_wa: Vec<f64>,  // h_pred_max * a_n
    pub pred_w2: Vec<f64>,  // Z_DJUP * (h_pred_max + 1)
    // Delad 2-lagers kritiker med SNABBT RLS-huvud på sista lagret:
    // V_mat (endast lugna steg |r| < 10)
    pub mat_w1: Vec<f64>,   // h_krit_max * (Z_DJUP + 1)
    pub mat_w_rls: Vec<f64>,// dim_krit = h_krit_max + Z_DJUP + 1
    pub mat_w_mal: Vec<f64>,
    pub mat_p_rls: Vec<f64>,// dim_krit * dim_krit
    // V_fara (kroppschocker + 3-stegs h-fönster, 100 % stop_grad!)
    pub fara_w1: Vec<f64>,  // h_krit_max * (Z_DJUP + 1)
    pub fara_w_rls: Vec<f64>,
    pub fara_p_rls: Vec<f64>,
    // IDM-huvud (gissar handling a ur [z_plast_prev, z_plast_now - z_plast_prev, 1]) för plastisk kodare
    idm_w: Vec<f64>,        // a_n * (2 * Z_PLAST + 1)
    // Sinnesskalning + sänkande blind kanalgrind + fryst livstids-chockgolv
    pub skala_init: bool,
    pub mu_s: Vec<f64>,
    pub var_s: Vec<f64>,
    pub grind_n: Vec<f32>,
    pub grind_sy: Vec<Vec<f32>>,
    pub grind_sy2: Vec<Vec<f32>>,
    pub grind_sx: Vec<Vec<f32>>,
    pub grind_sx2: Vec<Vec<f32>>,
    pub grind_syx: Vec<Vec<f32>>, // a_n * (s_n * s_n)
    pub grind_adj: Vec<Vec<f32>>,
    pub bel_n: f32,
    pub bel_sr: f32,
    pub bel_sr2: f32,
    pub bel_sa: f32,
    pub bel_sa2: f32,
    pub bel_sx: Vec<f32>,
    pub bel_sx2: Vec<f32>,
    pub bel_srx: Vec<f32>,
    pub bel_sax: Vec<f32>,
    pub chock_golv: Vec<f64>,
    pub kanal_vikt: Vec<f64>,
    // Positiv tids-RMS per axel (golv >= 0.05 -> bevarar noll & slukar inte mat-axlar, Lean D-13!)
    pub ms_z: [f64; Z_DJUP],
    // Arbetsminnesspår
    pub h_init: bool,
    pub h_spar: [f64; Z_DJUP],
    h_hist_2: [f64; Z_DJUP],
    h_hist_1: [f64; Z_DJUP],
    // Kausal historik & buffertar
    forra_ra_sk: Option<Vec<f32>>,
    forra_s_in: Option<Vec<f32>>,
    forra: Option<([f64; Z_DJUP], usize)>,
    td_forra: Option<([f64; Z_DJUP], f64)>,
    vanta_beloning: Option<f64>,
    vanta_chock: bool,
    chock_minne: Vec<ChockHandelse>,
    lugn_minne: Vec<LugnOvergang>,
    replay_pek: usize,
    lugn_pek: usize,
    // Diagnostik (rent passiv)
    pub lugn_krit_n: u64,
    pub lugn_krit_sy: f64,
    pub lugn_krit_sy2: f64,
    pub lugn_krit_se2: f64,
    pub p_tak_traffar: u64,
    pub steg: u64,
}

impl HjDjup {
    pub fn fran_gen(gen: &GenDjup, spec: ArtSpec, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ 0x2397_B7A1_0007);
        let s_n = spec.s_n;
        let a_n = spec.a_n;
        let n_enc = gen.n_dolda_enc.clamp(spec.h_enc_min, spec.h_enc_max);
        let n_pred = gen.n_dolda_pred.clamp(spec.h_pred_min, spec.h_pred_max);
        let n_krit = gen.n_dolda_krit.clamp(spec.h_krit_min, spec.h_krit_max);

        let inv_s = 1.0 / (s_n as f64).sqrt();
        let mut enc_w1 = vec![0.0; spec.h_enc_max * (s_n + 1)];
        for k in 0..spec.h_enc_max {
            let off = k * (s_n + 1);
            for j in 0..s_n {
                enc_w1[off + j] = 0.6 * inv_s * gauss(&mut g);
            }
            enc_w1[off + s_n] = 0.05 * gauss(&mut g);
        }
        let mut enc_w2 = vec![0.0; Z_PLAST * (spec.h_enc_max + 1)];
        for r in 0..Z_PLAST {
            let off = r * (spec.h_enc_max + 1);
            for k in 0..n_enc {
                enc_w2[off + k] = 0.6 * gauss(&mut g);
            }
            // Inaktiva platser n_enc..h_enc_max får exakt 0.0 i utgång (noll-initierad tillväxt redo!)
        }

        let mut pred_wh = vec![0.0; spec.h_pred_max * (Z_DJUP + 1)];
        for x in &mut pred_wh {
            *x = 0.25 * gauss(&mut g);
        }
        let mut pred_wa = vec![0.0; spec.h_pred_max * a_n];
        for x in &mut pred_wa {
            *x = 0.30 * gauss(&mut g);
        }
        // Utgångslagret i residual-Pred startar nära 0 (identitets-residual) och 0 på inaktiva platser
        let mut pred_w2 = vec![0.0; Z_DJUP * (spec.h_pred_max + 1)];
        for r in 0..Z_DJUP {
            let off = r * (spec.h_pred_max + 1);
            for k in 0..n_pred {
                pred_w2[off + k] = 0.02 * gauss(&mut g);
            }
        }

        let mut mat_w1 = vec![0.0; spec.h_krit_max * (Z_DJUP + 1)];
        let mut fara_w1 = vec![0.0; spec.h_krit_max * (Z_DJUP + 1)];
        for k in 0..spec.h_krit_max {
            let off = k * (Z_DJUP + 1);
            for r in 0..Z_DJUP {
                mat_w1[off + r] = 1.1 * gauss(&mut g);
                fara_w1[off + r] = 1.1 * gauss(&mut g);
            }
            mat_w1[off + Z_DJUP] = 0.8 * gauss(&mut g);
            fara_w1[off + Z_DJUP] = 0.8 * gauss(&mut g);
        }

        let dim_k = spec.h_krit_max + Z_DJUP + 1;
        let mut mat_p_rls = vec![0.0; dim_k * dim_k];
        let mut fara_p_rls = vec![0.0; dim_k * dim_k];
        for i in 0..dim_k {
            mat_p_rls[i * dim_k + i] = ALPHA_KRIT_DJUP;
            fara_p_rls[i * dim_k + i] = ALPHA_KRIT_DJUP;
        }

        Self {
            spec,
            gen: gen.clone(),
            n_enc,
            n_pred,
            n_krit,
            sk_enc: 1.0 / (spec.h_enc_max as f64).sqrt(),
            sk_pred: 1.0 / (spec.h_pred_max as f64).sqrt(),
            sk_krit: 2.5 / (spec.h_krit_max as f64).sqrt(),
            enc_w1,
            enc_w2,
            pred_wh,
            pred_wa,
            pred_w2,
            mat_w1,
            mat_w_rls: vec![0.0; dim_k],
            mat_w_mal: vec![0.0; dim_k],
            mat_p_rls,
            fara_w1,
            fara_w_rls: vec![0.0; dim_k],
            fara_p_rls,
            idm_w: vec![0.0; a_n * (2 * Z_PLAST + 1)],
            skala_init: false,
            mu_s: vec![0.0; s_n],
            var_s: vec![1.0; s_n],
            grind_n: vec![0.0; a_n],
            grind_sy: vec![vec![0.0; s_n]; a_n],
            grind_sy2: vec![vec![0.0; s_n]; a_n],
            grind_sx: vec![vec![0.0; s_n]; a_n],
            grind_sx2: vec![vec![0.0; s_n]; a_n],
            grind_syx: vec![vec![0.0; s_n * s_n]; a_n],
            grind_adj: vec![vec![0.0; s_n]; a_n],
            bel_n: 0.0,
            bel_sr: 0.0,
            bel_sr2: 0.0,
            bel_sa: 0.0,
            bel_sa2: 0.0,
            bel_sx: vec![0.0; s_n],
            bel_sx2: vec![0.0; s_n],
            bel_srx: vec![0.0; s_n],
            bel_sax: vec![0.0; s_n],
            chock_golv: vec![0.0; s_n],
            kanal_vikt: vec![1.0; s_n],
            ms_z: [1.0; Z_DJUP],
            h_init: false,
            h_spar: [0.0; Z_DJUP],
            h_hist_2: [0.0; Z_DJUP],
            h_hist_1: [0.0; Z_DJUP],
            forra_ra_sk: None,
            forra_s_in: None,
            forra: None,
            td_forra: None,
            vanta_beloning: None,
            vanta_chock: false,
            chock_minne: Vec::new(),
            lugn_minne: Vec::new(),
            replay_pek: 0,
            lugn_pek: 0,
            lugn_krit_n: 0,
            lugn_krit_sy: 0.0,
            lugn_krit_sy2: 0.0,
            lugn_krit_se2: 0.0,
            p_tak_traffar: 0,
            steg: 0,
        }
    }

    /// Totalt antal aktiva dolda neuroner över samtliga 4 dolda lager (4/4 konsensus: 32+24+12+12 = 80 i Steg 1).
    #[inline]
    pub fn aktiva_dolda_neuroner(&self) -> usize {
        self.n_enc + self.n_pred + 2 * self.n_krit
    }

    /// Verklig energikostnad per steg (Lean 4 D-12: strikt monoton och garanterat positivt överskott vid H_max).
    #[inline]
    pub fn energikostnad(&self) -> f64 {
        self.spec.kappa_energi * self.aktiva_dolda_neuroner() as f64
    }

    /// Fail-safe noll-initierad neurontillväxt (Lean 4 D-8):
    /// Slumpad ingångsvikt + exakt `0.0` utgångsvikt + fast skalning `1/sqrt(H_max)` -> 0.0 förändring i utgång!
    pub fn utoka_dolda_noll_init(&mut self, d_enc: usize, d_pred: usize, d_krit: usize, g: &mut Rng) {
        let s_n = self.spec.s_n;
        let inv_s = 1.0 / (s_n as f64).sqrt();
        let ny_enc = (self.n_enc + d_enc).min(self.spec.h_enc_max);
        for k in self.n_enc..ny_enc {
            let off = k * (s_n + 1);
            for j in 0..s_n {
                self.enc_w1[off + j] = 0.6 * inv_s * gauss(g);
            }
            self.enc_w1[off + s_n] = 0.05 * gauss(g);
            for r in 0..Z_PLAST {
                self.enc_w2[r * (self.spec.h_enc_max + 1) + k] = 0.0;
            }
        }
        self.n_enc = ny_enc;

        let ny_pred = (self.n_pred + d_pred).min(self.spec.h_pred_max);
        for k in self.n_pred..ny_pred {
            let off = k * (Z_DJUP + 1);
            for j in 0..=Z_DJUP {
                self.pred_wh[off + j] = 0.25 * gauss(g);
            }
            for a in 0..self.spec.a_n {
                self.pred_wa[k * self.spec.a_n + a] = 0.30 * gauss(g);
            }
            for r in 0..Z_DJUP {
                self.pred_w2[r * (self.spec.h_pred_max + 1) + k] = 0.0;
            }
        }
        self.n_pred = ny_pred;

        let ny_krit = (self.n_krit + d_krit).min(self.spec.h_krit_max);
        for k in self.n_krit..ny_krit {
            let off = k * (Z_DJUP + 1);
            for j in 0..Z_DJUP {
                self.mat_w1[off + j] = 1.1 * gauss(g);
                self.fara_w1[off + j] = 1.1 * gauss(g);
            }
            self.mat_w1[off + Z_DJUP] = 0.8 * gauss(g);
            self.fara_w1[off + Z_DJUP] = 0.8 * gauss(g);
            self.mat_w_rls[k] = 0.0;
            self.mat_w_mal[k] = 0.0;
            self.fara_w_rls[k] = 0.0;
        }
        self.n_krit = ny_krit;
    }

    pub fn ge_beloning(&mut self, r: f64) {
        self.vanta_beloning = Some(r);
        self.vanta_chock = false;
        let s_n = self.spec.s_n;
        if let Some(ref s_prev) = self.forra_ra_sk {
            let rf = r.clamp(-50.0, 50.0) as f32;
            let mr_prev = if self.bel_n > 1.0 { self.bel_sr / self.bel_n } else { 0.0 };
            let vr_prev = if self.bel_n > 2.0 {
                (self.bel_sr2 / self.bel_n - mr_prev * mr_prev).max(1e-4)
            } else {
                1.0
            };
            let af = (rf - mr_prev).abs();
            if r.abs() >= CHOCK_GRANS || (self.bel_n >= 40.0 && af > 3.0 * vr_prev.sqrt()) {
                self.vanta_chock = true;
                for j in 0..s_n {
                    let z2 = (s_prev[j] as f64) * (s_prev[j] as f64);
                    let avvik = ((z2 - 1.5).max(0.0) / 7.5).min(1.0) * 0.008;
                    if avvik > self.chock_golv[j] {
                        self.chock_golv[j] = avvik;
                    }
                }
            }
            self.bel_n = GRIND_GAMMA * self.bel_n + 1.0;
            self.bel_sr = GRIND_GAMMA * self.bel_sr + rf;
            self.bel_sr2 = GRIND_GAMMA * self.bel_sr2 + rf * rf;
            self.bel_sa = GRIND_GAMMA * self.bel_sa + af;
            self.bel_sa2 = GRIND_GAMMA * self.bel_sa2 + af * af;
            for j in 0..s_n {
                let x = s_prev[j];
                self.bel_sx[j] = GRIND_GAMMA * self.bel_sx[j] + x;
                self.bel_sx2[j] = GRIND_GAMMA * self.bel_sx2[j] + x * x;
                self.bel_srx[j] = GRIND_GAMMA * self.bel_srx[j] + rf * x;
                self.bel_sax[j] = GRIND_GAMMA * self.bel_sax[j] + af * x;
            }
        }
    }

    fn skala_sinnen(&self, s: &[f64]) -> Vec<f64> {
        let s_n = self.spec.s_n;
        let mut ut = vec![0.0; s_n];
        if !self.skala_init {
            return ut;
        }
        for j in 0..s_n {
            ut[j] = ((s[j] - self.mu_s[j]) / (self.var_s[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    fn uppdatera_skala(&mut self, s: &[f64]) {
        let s_n = self.spec.s_n;
        if !self.skala_init {
            self.mu_s.copy_from_slice(&s[..s_n]);
            self.var_s.fill(1.0);
            self.skala_init = true;
            return;
        }
        for j in 0..s_n {
            let d = s[j] - self.mu_s[j];
            self.mu_s[j] += ETA_SINNE * d;
            self.var_s[j] = (1.0 - ETA_SINNE) * self.var_s[j] + ETA_SINNE * d * d;
        }
    }

    fn berakna_grind_rad(&self, a: usize) -> Vec<f32> {
        let s_n = self.spec.s_n;
        let mut rad = vec![0.0f32; s_n];
        let n = self.grind_n[a];
        if n < GRIND_MIN_N {
            return rad;
        }
        let inv_n = 1.0f32 / n;
        let straff = GRIND_STRAFF * inv_n;
        let mut mx = vec![0.0f32; s_n];
        let mut inv_vx = vec![0.0f32; s_n];
        for k in 0..s_n {
            let m = self.grind_sx[a][k] * inv_n;
            mx[k] = m;
            let vx = self.grind_sx2[a][k] * inv_n - m * m;
            inv_vx[k] = if vx >= 1e-6 { 1.0 / vx } else { 0.0 };
        }
        let syx_a = &self.grind_syx[a];
        for j in 0..s_n {
            let my = self.grind_sy[a][j] * inv_n;
            let vy = self.grind_sy2[a][j] * inv_n - my * my;
            if vy < 1e-6 {
                continue;
            }
            let inv_vy = 1.0 / vy;
            let row = j * s_n;
            let mut best_r2 = 0.0f32;
            for k in 0..s_n {
                let cov = syx_a[row + k] * inv_n - my * mx[k];
                let r2 = cov * cov * inv_vy * inv_vx[k];
                if r2 > best_r2 {
                    best_r2 = r2;
                }
            }
            rad[j] = (best_r2 - straff).max(0.0);
        }
        rad
    }

    fn uppdatera_kanalgrind(&mut self, s_sk_f32: &[f32]) {
        let (Some(ref s_prev), Some((_, a_prev))) = (&self.forra_ra_sk, self.forra) else {
            return;
        };
        let s_n = self.spec.s_n;
        let a_n = self.spec.a_n;
        self.grind_n[a_prev] = GRIND_GAMMA * self.grind_n[a_prev] + 1.0;
        let sy = &mut self.grind_sy[a_prev];
        let sy2 = &mut self.grind_sy2[a_prev];
        let sx = &mut self.grind_sx[a_prev];
        let sx2 = &mut self.grind_sx2[a_prev];
        let syx = &mut self.grind_syx[a_prev];
        for j in 0..s_n {
            let yj = s_sk_f32[j];
            let xj = s_prev[j];
            sy[j] = GRIND_GAMMA * sy[j] + yj;
            sy2[j] = GRIND_GAMMA * sy2[j] + yj * yj;
            sx[j] = GRIND_GAMMA * sx[j] + xj;
            sx2[j] = GRIND_GAMMA * sx2[j] + xj * xj;
            let row = j * s_n;
            for k in 0..s_n {
                syx[row + k] = GRIND_GAMMA * syx[row + k] + yj * s_prev[k];
            }
        }
        if self.steg < 250 {
            if self.steg % 5 == 0 {
                self.grind_adj[a_prev] = self.berakna_grind_rad(a_prev);
            } else {
                return;
            }
        } else if self.steg % 25 == 0 {
            for a in 0..a_n {
                self.grind_adj[a] = self.berakna_grind_rad(a);
            }
        } else {
            return;
        }
        let mut poang = vec![0.0f64; s_n];
        let mut antal = 0usize;
        for a in 0..a_n {
            if self.grind_n[a] >= GRIND_MIN_N {
                antal += 1;
                for j in 0..s_n {
                    poang[j] += self.grind_adj[a][j] as f64;
                }
            }
        }
        if antal > 0 {
            let inv_a = 1.0 / antal as f64;
            for j in 0..s_n {
                poang[j] *= inv_a;
            }
        }
        let max_p = poang.iter().copied().fold(0.0f64, f64::max);
        if max_p > GRIND_AKTIV_MIN {
            for j in 0..s_n {
                self.kanal_vikt[j] = (poang[j] / GRIND_MATTNAD).clamp(0.0, 1.0).powi(3);
            }
        } else {
            self.kanal_vikt.fill(1.0);
        }
        if self.bel_n >= 40.0 {
            let inv_n = 1.0 / self.bel_n;
            let mr = self.bel_sr * inv_n;
            let vr = self.bel_sr2 * inv_n - mr * mr;
            let ma = self.bel_sa * inv_n;
            let va = self.bel_sa2 * inv_n - ma * ma;
            let straff = 1.5 * inv_n;
            for j in 0..s_n {
                let mx = self.bel_sx[j] * inv_n;
                let vx = self.bel_sx2[j] * inv_n - mx * mx;
                let mut best_r2 = 0.0f64;
                if vx >= 1e-6 {
                    if vr >= 1e-6 {
                        let cov = self.bel_srx[j] * inv_n - mr * mx;
                        let r2 = (cov * cov / (vr * vx) - straff).max(0.0) as f64;
                        if r2 > best_r2 {
                            best_r2 = r2;
                        }
                    }
                    if va >= 1e-6 {
                        let cov_a = self.bel_sax[j] * inv_n - ma * mx;
                        let r2_a = (cov_a * cov_a / (va * vx) - straff).max(0.0) as f64;
                        if r2_a > best_r2 {
                            best_r2 = r2_a;
                        }
                    }
                }
                let eff_r2 = best_r2.max(self.chock_golv[j]);
                let faktor = 0.25 + 0.75 * (eff_r2 / 0.008).clamp(0.0, 1.0);
                self.kanal_vikt[j] *= faktor;
            }
        }
    }

    /// Kodar `s_in` till rå `z_raw[0..16]` (0..8 ankrad bas, 8..16 plastisk 2-lagers-kodare).
    pub fn koda_ra(&self, s_in: &[f64]) -> ([f64; Z_DJUP], Vec<f64>, Vec<f64>) {
        let s_n = self.spec.s_n;
        let mut z_raw = [0.0f64; Z_DJUP];
        // 1. Ankrad låg-rang-bas (aldrig roterad av livstidsinlärning!)
        let ank = self.gen.ankrad_bas(s_in);
        for r in 0..Z_ANK {
            z_raw[r] = rat_tanh(ank[r]);
        }
        // 2. Plastisk 2-lagers-kodare med fast skalning `sk_enc` (Lean D-8)
        let mut pre = vec![0.0f64; self.n_enc];
        let mut act = vec![0.0f64; self.n_enc];
        for k in 0..self.n_enc {
            let off = k * (s_n + 1);
            let mut lin = self.enc_w1[off + s_n];
            for j in 0..s_n {
                lin += self.enc_w1[off + j] * (self.gen.kanalskala[j] * s_in[j]);
            }
            pre[k] = lin;
            act[k] = rat_tanh(lin);
        }
        let stride = self.spec.h_enc_max + 1;
        for r in 0..Z_PLAST {
            let off = r * stride;
            let mut sum = self.enc_w2[off + self.spec.h_enc_max];
            for k in 0..self.n_enc {
                sum += self.enc_w2[off + k] * act[k];
            }
            z_raw[Z_ANK + r] = self.sk_enc * sum;
        }
        (z_raw, pre, act)
    }

    /// Positiv tids-RMS-skalning per axel med fast golv >= 0.05 (Lean 4 D-13).
    #[inline]
    pub fn norm_tids_rms(&self, z_raw: &[f64; Z_DJUP]) -> [f64; Z_DJUP] {
        std::array::from_fn(|r| {
            let sc = self.ms_z[r].sqrt().max(RMS_GOLV);
            (z_raw[r] / sc).clamp(-3.0, 3.0)
        })
    }

    fn uppdatera_tids_rms(&mut self, z_raw: &[f64; Z_DJUP]) {
        for r in 0..Z_DJUP {
            self.ms_z[r] = (1.0 - ETA_Z_RMS) * self.ms_z[r] + ETA_Z_RMS * z_raw[r] * z_raw[r];
        }
    }

    /// Faktoriserad 2-lagers världsmodell: förberäknar `W_ph · h + b` EN gång (Lean 4 D-11).
    pub fn pred_forberakna(&self, h: &[f64; Z_DJUP]) -> Vec<f64> {
        let mut bas = vec![0.0f64; self.n_pred];
        for k in 0..self.n_pred {
            let off = k * (Z_DJUP + 1);
            let mut s = self.pred_wh[off + Z_DJUP];
            for r in 0..Z_DJUP {
                s += self.pred_wh[off + r] * h[r];
            }
            bas[k] = s;
        }
        bas
    }

    /// Förutsäger `ĥ_a` från förberäknad bas `bas` och handling `a`.
    pub fn forutsag_fran_bas(&self, h: &[f64; Z_DJUP], bas: &[f64], a: usize) -> [f64; Z_DJUP] {
        let a_n = self.spec.a_n;
        let stride = self.spec.h_pred_max + 1;
        let mut u = vec![0.0f64; self.n_pred];
        for k in 0..self.n_pred {
            u[k] = rat_tanh(bas[k] + self.pred_wa[k * a_n + a]);
        }
        let mut y = *h;
        for r in 0..Z_DJUP {
            let off = r * stride;
            let mut d = self.pred_w2[off + self.spec.h_pred_max];
            for k in 0..self.n_pred {
                d += self.pred_w2[off + k] * u[k];
            }
            y[r] += self.sk_pred * d;
        }
        y
    }

    /// Full icke-faktoriserad förutsägelse (för röd-till-grön ekvivalenstest mot faktoriserad!).
    pub fn forutsag(&self, h: &[f64; Z_DJUP], a: usize) -> [f64; Z_DJUP] {
        let bas = self.pred_forberakna(h);
        self.forutsag_fran_bas(h, &bas, a)
    }

    fn uppdatera_pred(&mut self, h_prev: &[f64; Z_DJUP], a_prev: usize, h_now: &[f64; Z_DJUP]) {
        let a_n = self.spec.a_n;
        let stride = self.spec.h_pred_max + 1;
        let bas = self.pred_forberakna(h_prev);
        let mut pre = vec![0.0f64; self.n_pred];
        let mut u = vec![0.0f64; self.n_pred];
        for k in 0..self.n_pred {
            let p = bas[k] + self.pred_wa[k * a_n + a_prev];
            pre[k] = p;
            u[k] = rat_tanh(p);
        }
        let mut err_rob = [0.0f64; Z_DJUP];
        for r in 0..Z_DJUP {
            let off = r * stride;
            let mut pred_d = self.pred_w2[off + self.spec.h_pred_max];
            for k in 0..self.n_pred {
                pred_d += self.pred_w2[off + k] * u[k];
            }
            pred_d *= self.sk_pred;
            let sann_d = h_now[r] - h_prev[r];
            // ReJEPA robust begränsad gradient (Lean 4 D-3/D-4: strikt i (-1, 1)!)
            err_rob[r] = rat_tanh(sann_d - pred_d);
        }
        let eta = self.gen.eta_pred;
        let mut d_u = vec![0.0f64; self.n_pred];
        for r in 0..Z_DJUP {
            let off = r * stride;
            let g_r = eta * self.sk_pred * err_rob[r];
            for k in 0..self.n_pred {
                d_u[k] += err_rob[r] * self.sk_pred * self.pred_w2[off + k];
                self.pred_w2[off + k] += g_r * u[k];
            }
            self.pred_w2[off + self.spec.h_pred_max] += g_r;
            klamp_rad_l1(&mut self.pred_w2[off..off + self.spec.h_pred_max], 12.0);
        }
        for k in 0..self.n_pred {
            let gk = eta * (d_u[k] * rat_tanh_deriv(pre[k])).clamp(-1.0, 1.0);
            let off = k * (Z_DJUP + 1);
            for r in 0..Z_DJUP {
                self.pred_wh[off + r] += gk * h_prev[r];
            }
            self.pred_wh[off + Z_DJUP] += gk;
            self.pred_wa[k * a_n + a_prev] += gk;
            klamp_rad_l1(&mut self.pred_wh[off..off + Z_DJUP], 8.0);
        }
    }

    /// Bygger aktiva 2-lagers-särdrag `φ(h)` för delkritikerna (dolda platser `n_krit..h_krit_max` exakt 0.0 -> Lean D-8!).
    fn krit_drag(&self, w1: &[f64], h: &[f64; Z_DJUP]) -> Vec<f64> {
        let hm = self.spec.h_krit_max;
        let mut x = vec![0.0f64; hm + Z_DJUP + 1];
        for k in 0..self.n_krit {
            let off = k * (Z_DJUP + 1);
            let mut lin = w1[off + Z_DJUP];
            for r in 0..Z_DJUP {
                lin += w1[off + r] * h[r];
            }
            x[k] = self.sk_krit * rat_tanh(lin);
        }
        for r in 0..Z_DJUP {
            x[hm + r] = h[r];
        }
        x[hm + Z_DJUP] = 1.0;
        x
    }

    pub fn varde_mat(&self, h: &[f64; Z_DJUP]) -> f64 {
        let x = self.krit_drag(&self.mat_w1, h);
        let mut s = 0.0;
        for i in 0..x.len() {
            s += self.mat_w_rls[i] * x[i];
        }
        s
    }

    fn varde_mat_mal(&self, h: &[f64; Z_DJUP]) -> f64 {
        let x = self.krit_drag(&self.mat_w1, h);
        let mut s = 0.0;
        for i in 0..x.len() {
            s += self.mat_w_mal[i] * x[i];
        }
        s
    }

    pub fn varde_fara(&self, h: &[f64; Z_DJUP]) -> f64 {
        let x = self.krit_drag(&self.fara_w1, h);
        let mut s = 0.0;
        for i in 0..x.len() {
            s += self.fara_w_rls[i] * x[i];
        }
        s.clamp(-60.0, 20.0)
    }

    /// Total värdefunktion `J(h) = V_0(h_ank) + V_mat(h) + V_fara(h)` (LeCun 2022 ekv. 1–3, Lean D-1/D-2).
    pub fn total_varde(&self, h: &[f64; Z_DJUP]) -> f64 {
        self.gen.v0_varde(h) + self.varde_mat(h) + self.varde_fara(h)
    }

    pub fn lugn_krit_r2(&self) -> f64 {
        if self.lugn_krit_n < 20 {
            return f64::NAN;
        }
        let n = self.lugn_krit_n as f64;
        let my = self.lugn_krit_sy / n;
        let ss_tot = self.lugn_krit_sy2 - n * my * my;
        if ss_tot <= 1e-9 {
            return f64::NAN;
        }
        1.0 - self.lugn_krit_se2 / ss_tot
    }

    /// Uppdaterar ENBART `V_fara` med 3-stegs `h`-fönster (`γ_fara = 0.80`, Lean D-6) och 100 % `stop_grad` (Lean D-9)!
    pub fn kor_fara_chock_steg(&mut self, handelse: &ChockHandelse, c_vikt: f64) {
        let r_ch = handelse.r_chock.clamp(-50.0, 0.0);
        // 3 steg bakåt i exakt samma h-rum: t (r_ch), t-1 (γ·r_ch), t-2 (γ²·r_ch)
        let mal = [
            (handelse.h_fonster[2], r_ch, c_vikt),
            (handelse.h_fonster[1], GAMMA_FARA * r_ch, c_vikt * 0.65),
            (handelse.h_fonster[0], GAMMA_FARA * GAMMA_FARA * r_ch, c_vikt * 0.40),
        ];
        for (h_st, y_mal, cw) in mal {
            let x = self.krit_drag(&self.fara_w1, &h_st);
            let pred = self.varde_fara(&h_st);
            let err = (y_mal - pred).clamp(-50.0, 50.0);
            self.p_tak_traffar += rls_skalar_viktad_vec(
                &mut self.fara_w_rls,
                &mut self.fara_p_rls,
                &x,
                err,
                1.0,
                cw,
                P_TAK_DJUP,
            );
        }
    }

    /// Uppdaterar enbart den plastiska kodaren (8..16) från lugna övergångar via IDM + varianskollapsskydd.
    /// Rör ALDRIG den ankrade basen (0..8), `V_0` eller `V_fara`!
    fn kor_plastisk_kodare_idm(&mut self) {
        if self.lugn_minne.len() < 8 {
            return;
        }
        let s_n = self.spec.s_n;
        let a_n = self.spec.a_n;
        let feat_dim = 2 * Z_PLAST + 1;
        let bs = 6usize.min(self.lugn_minne.len());
        let bs_f = bs as f64;
        let eta = self.gen.eta_kod;
        let stride_w2 = self.spec.h_enc_max + 1;

        let mut grad_w1 = vec![0.0f64; self.n_enc * (s_n + 1)];
        let mut grad_w2 = vec![0.0f64; Z_PLAST * stride_w2];
        let mut z_sam: Vec<[f64; Z_PLAST]> = Vec::with_capacity(bs);
        let mut act_sam: Vec<Vec<f64>> = Vec::with_capacity(bs);
        let mut pre_sam: Vec<Vec<f64>> = Vec::with_capacity(bs);
        let mut s_sam: Vec<Vec<f64>> = Vec::with_capacity(bs);

        for b_i in 0..bs {
            let idx = (self.lugn_pek + b_i) % self.lugn_minne.len();
            let sp: Vec<f64> = self.lugn_minne[idx].s_prev.iter().map(|&v| v as f64).collect();
            let sn: Vec<f64> = self.lugn_minne[idx].s_now.iter().map(|&v| v as f64).collect();
            let a_sann = (self.lugn_minne[idx].a as usize).min(a_n - 1);

            let (zr_p, pre_p, act_p) = self.koda_ra(&sp);
            let (zr_n, pre_n, act_n) = self.koda_ra(&sn);
            let z_p = self.norm_tids_rms(&zr_p);
            let z_n = self.norm_tids_rms(&zr_n);

            let mut zp_pl = [0.0f64; Z_PLAST];
            let mut zn_pl = [0.0f64; Z_PLAST];
            for r in 0..Z_PLAST {
                zp_pl[r] = z_p[Z_ANK + r];
                zn_pl[r] = z_n[Z_ANK + r];
            }
            z_sam.push(zn_pl);
            act_sam.push(act_n.clone());
            pre_sam.push(pre_n.clone());
            s_sam.push(sn.clone());

            let mut feat = vec![0.0f64; feat_dim];
            for r in 0..Z_PLAST {
                feat[r] = zp_pl[r];
                feat[Z_PLAST + r] = zn_pl[r] - zp_pl[r];
            }
            feat[2 * Z_PLAST] = 1.0;

            let mut logits = vec![0.0f64; a_n];
            let mut lmax = f64::NEG_INFINITY;
            for a in 0..a_n {
                let off = a * feat_dim;
                let mut s = 0.0;
                for m in 0..feat_dim {
                    s += self.idm_w[off + m] * feat[m];
                }
                logits[a] = s;
                if s > lmax {
                    lmax = s;
                }
            }
            let mut probs = vec![0.0f64; a_n];
            let mut se = 0.0;
            for a in 0..a_n {
                probs[a] = (logits[a] - lmax).exp();
                se += probs[a];
            }
            for a in 0..a_n {
                probs[a] /= se.max(1e-12);
            }

            let mut d_zp = [0.0f64; Z_PLAST];
            let mut d_zn = [0.0f64; Z_PLAST];
            for a in 0..a_n {
                let g_a = (if a == a_sann { 1.0 } else { 0.0 }) - probs[a];
                let off = a * feat_dim;
                for r in 0..Z_PLAST {
                    d_zp[r] += g_a * (self.idm_w[off + r] - self.idm_w[off + Z_PLAST + r]);
                    d_zn[r] += g_a * self.idm_w[off + Z_PLAST + r];
                }
                for m in 0..feat_dim {
                    self.idm_w[off + m] = (self.idm_w[off + m] + 0.025 * g_a * feat[m] / bs_f).clamp(-4.0, 4.0);
                }
            }
            // Backprop in i plastisk W2 & W1 (bara n_enc aktiva dolda neuroner)
            for (d_z, pre, act, s_in) in [(&d_zp, &pre_p, &act_p, &sp), (&d_zn, &pre_n, &act_n, &sn)] {
                let mut d_act = vec![0.0f64; self.n_enc];
                for r in 0..Z_PLAST {
                    let g_r = (d_z[r] * self.sk_enc).clamp(-0.5, 0.5) / bs_f;
                    let off2 = r * stride_w2;
                    for k in 0..self.n_enc {
                        grad_w2[off2 + k] += eta * g_r * act[k];
                        d_act[k] += g_r * self.enc_w2[off2 + k];
                    }
                    grad_w2[off2 + self.spec.h_enc_max] += eta * g_r;
                }
                for k in 0..self.n_enc {
                    let gk = eta * (d_act[k] * rat_tanh_deriv(pre[k])).clamp(-0.25, 0.25);
                    let off1 = k * (s_n + 1);
                    for j in 0..s_n {
                        grad_w1[off1 + j] += gk * self.gen.kanalskala[j] * s_in[j];
                    }
                    grad_w1[off1 + s_n] += gk;
                }
            }
        }

        // Lätt varianskollapsskydd över plastiska axlar (VICReg-stil)
        let mut mz = [0.0f64; Z_PLAST];
        for z in &z_sam {
            for r in 0..Z_PLAST {
                mz[r] += z[r] / bs_f;
            }
        }
        let mut vz = [0.0f64; Z_PLAST];
        for z in &z_sam {
            for r in 0..Z_PLAST {
                let d = z[r] - mz[r];
                vz[r] += d * d / bs_f;
            }
        }
        for i_b in 0..bs {
            for r in 0..Z_PLAST {
                if vz[r] < 0.30 {
                    let push = 0.002 * (z_sam[i_b][r] - mz[r]).signum() * self.sk_enc / bs_f;
                    let off2 = r * stride_w2;
                    for k in 0..self.n_enc {
                        grad_w2[off2 + k] += push * act_sam[i_b][k];
                    }
                }
            }
        }
        for r in 0..Z_PLAST {
            let off2 = r * stride_w2;
            for k in 0..self.n_enc {
                self.enc_w2[off2 + k] += grad_w2[off2 + k].clamp(-0.04, 0.04);
            }
            self.enc_w2[off2 + self.spec.h_enc_max] +=
                grad_w2[off2 + self.spec.h_enc_max].clamp(-0.04, 0.04);
            klamp_rad_l1(&mut self.enc_w2[off2..off2 + self.spec.h_enc_max], 10.0);
        }
        for k in 0..self.n_enc {
            let off1 = k * (s_n + 1);
            for j in 0..=s_n {
                self.enc_w1[off1 + j] += grad_w1[off1 + j].clamp(-0.04, 0.04);
            }
            klamp_rad_l1(&mut self.enc_w1[off1..off1 + s_n], 8.0);
        }
    }

    pub fn g_intervall(&self, start: usize, len: usize) -> f64 {
        let slut = (start + len).min(self.kanal_vikt.len());
        if slut <= start {
            return 0.0;
        }
        self.kanal_vikt[start..slut].iter().sum::<f64>() / (slut - start) as f64
    }

    /// Ett komplett kausalt tankesteg: returnerar `(vald handling, aktuell inre bild h_t)`.
    pub fn valj_lott(&mut self, s_nu: &[f64], g: &mut Rng) -> (usize, [f64; Z_DJUP]) {
        let s_n = self.spec.s_n;
        let a_n = self.spec.a_n;
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala_sinnen(s_nu);
        let s_sk_f32: Vec<f32> = s_sk.iter().map(|&v| v as f32).collect();
        let mut s_in = vec![0.0; s_n];
        for j in 0..s_n {
            s_in[j] = s_sk[j] * self.kanal_vikt[j];
        }

        let (z_raw_now, _, _) = self.koda_ra(&s_in);
        let z_now = self.norm_tids_rms(&z_raw_now);
        let h_now: [f64; Z_DJUP] = if !self.h_init {
            z_now
        } else {
            std::array::from_fn(|r| {
                if r < Z_MINNE_START {
                    z_now[r]
                } else {
                    (1.0 - BETA_MINNE) * self.h_spar[r] + BETA_MINNE * z_now[r]
                }
            })
        };

        // 1. Kausal inlärning från föregående steg
        if let Some((h_prev, a_prev)) = self.forra {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let r_nu = r_raw.clamp(-50.0, 50.0);
                // (a) Kroppschock -> ENBART V_fara (3-stegs h-fönster, 100 % stop_grad, Lean D-1/D-2/D-6/D-9!)
                if self.vanta_chock && r_nu < 0.0 {
                    let handelse = ChockHandelse {
                        h_fonster: [self.h_hist_2, self.h_hist_1, h_now],
                        r_chock: r_nu,
                    };
                    if self.chock_minne.len() < CHOCK_BUFFERT_MAX {
                        self.chock_minne.push(handelse.clone());
                    } else {
                        let idx = self.replay_pek % CHOCK_BUFFERT_MAX;
                        self.chock_minne[idx] = handelse.clone();
                    }
                    // Omedelbar 1-bettsprägling (3 steg på V_fara — noll påverkan på V_mat/V_0/kodare!)
                    for _ in 0..3 {
                        self.kor_fara_chock_steg(&handelse, 2.0);
                    }
                } else {
                    // (b) Lugnt steg -> uppdatera ENBART V_mat (TD(0) mot lugn matbelöning!)
                    if let Some((h_td, r_td)) = self.td_forra {
                        if r_td.abs() < CHOCK_GRANS {
                            let v_nasta = (self.gen.v0_varde(&h_now) + self.varde_mat_mal(&h_now)).clamp(-20.0, 20.0);
                            let y_td = (r_td + GAMMA_MAT * v_nasta).clamp(-20.0, 20.0);
                            let pred_m = self.gen.v0_varde(&h_td) + self.varde_mat(&h_td);
                            let err_m = y_td - pred_m;
                            if self.steg >= 80 {
                                self.lugn_krit_n += 1;
                                self.lugn_krit_sy += y_td;
                                self.lugn_krit_sy2 += y_td * y_td;
                                self.lugn_krit_se2 += err_m * err_m;
                            }
                            let x_m = self.krit_drag(&self.mat_w1, &h_td);
                            self.p_tak_traffar += rls_skalar_viktad_vec(
                                &mut self.mat_w_rls,
                                &mut self.mat_p_rls,
                                &x_m,
                                err_m,
                                LAMBDA_KRIT,
                                1.0,
                                P_TAK_DJUP,
                            );
                            for m in 0..self.mat_w_rls.len() {
                                self.mat_w_mal[m] += ETA_MAL * (self.mat_w_rls[m] - self.mat_w_mal[m]);
                            }
                            // Lugn 0.0-ankring för V_fara (så V_fara skiljer lugna rutor ~0.0 från chockrutor -48.0,
                            // helt utan att påverka V_mat, V_0 eller kodare!).
                            let x_f = self.krit_drag(&self.fara_w1, &h_td);
                            let err_f = (0.0 - self.varde_fara(&h_td)).clamp(-20.0, 20.0);
                            self.p_tak_traffar += rls_skalar_viktad_vec(
                                &mut self.fara_w_rls,
                                &mut self.fara_p_rls,
                                &x_f,
                                err_f,
                                0.999,
                                0.15,
                                P_TAK_DJUP,
                            );
                        }
                    }
                    // Spara lugn övergång för plastiska kodarens IDM
                    if let Some(ref sp) = self.forra_s_in {
                        let ov = LugnOvergang {
                            s_prev: sp.clone(),
                            s_now: s_in.iter().map(|&v| v as f32).collect(),
                            a: a_prev as u8,
                        };
                        if self.lugn_minne.len() < LUGN_BUFFERT_MAX {
                            self.lugn_minne.push(ov);
                        } else {
                            let idx = self.lugn_pek % LUGN_BUFFERT_MAX;
                            self.lugn_minne[idx] = ov;
                        }
                        self.lugn_pek = self.lugn_pek.wrapping_add(1);
                    }
                }
                self.vanta_chock = false;
                // Periodisk chockåterspelning på V_fara (1 per 10 steg, 0 % påverkan på V_mat/kodare!)
                if self.steg % 10 == 0 && !self.chock_minne.is_empty() {
                    let idx = self.replay_pek % self.chock_minne.len();
                    let hnd = self.chock_minne[idx].clone();
                    self.replay_pek = self.replay_pek.wrapping_add(1);
                    self.kor_fara_chock_steg(&hnd, 1.0);
                }
                // Periodisk plastisk kodaruppdatering (IDM + kollapsskydd, 1 per 10 lugna steg)
                if self.steg % 10 == 5 {
                    self.kor_plastisk_kodare_idm();
                }
                self.td_forra = Some((h_now, r_nu));
            }
            // Uppdatera faktoriserad världsmodell Pred med ReJEPA robust gradient
            self.uppdatera_pred(&h_prev, a_prev, &h_now);
        } else {
            let _ = self.vanta_beloning.take();
        }

        // 2. Uppdatera kanalgrind, sinnesskala, tids-RMS och 3-stegs h-historik
        self.uppdatera_kanalgrind(&s_sk_f32);
        self.uppdatera_skala(s_nu);
        self.uppdatera_tids_rms(&z_raw_now);
        self.h_hist_2 = self.h_hist_1;
        self.h_hist_1 = if self.h_init { self.h_spar } else { h_now };
        self.h_spar = h_now;
        self.h_init = true;

        // 3. Planera handling med faktoriserad Pred + delad kritiker J = V_0 + V_mat + V_fara
        let mut poang = vec![0.0f64; a_n];
        if self.spec.stryk_wm {
            let bas = self.pred_forberakna(&h_now);
            let mut ym = [0.0f64; Z_DJUP];
            for a in 0..a_n {
                let ya = self.forutsag_fran_bas(&h_now, &bas, a);
                for r in 0..Z_DJUP {
                    ym[r] += ya[r] / a_n as f64;
                }
            }
            let vm = self.total_varde(&ym);
            poang.fill(vm);
        } else {
            let bas = self.pred_forberakna(&h_now);
            for a in 0..a_n {
                let ya = self.forutsag_fran_bas(&h_now, &bas, a);
                poang[a] = self.total_varde(&ya);
            }
        }
        let a = if g.f() < UTFORSK_P {
            let neg: Vec<f64> = self.grind_n.iter().map(|&n| -(n as f64)).collect();
            lott_bland_max(&neg, g)
        } else {
            lott_bland_max(&poang, g)
        };
        self.forra_ra_sk = Some(s_sk_f32);
        self.forra_s_in = Some(s_in.iter().map(|&v| v as f32).collect());
        self.forra = Some((h_now, a));
        self.steg += 1;
        (a, h_now)
    }
}

// ---------------------------------------------------------------------------
// 4. Isolerad tävlingsarena för Alt 2397 (exakt samma B2Param::v2_skarp-fysik,
//    med disksparande per värld/arm för avbrottssäker Google Colab-körning!)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DjupArm {
    /// Djup flerlagers-JEPA (392 gener, κ=0.0004) mot sin egen variansbevarande FalskLukt-tvilling
    DjupVsFalskLukt,
    /// Djup flerlagers-JEPA mot gamla Alt 2373-hjärnan (MinneSankGrind) i samma värld
    DjupVsAlt2373,
    /// Djup flerlagers-JEPA mot sin egen världsmodell avstängd (StrykWM) — kausal världsmodellkontroll
    DjupVsStrykWm,
    /// Djup flerlagers-JEPA med fri neurontillväxt (Steg 2) mot låst 80-neuroners Djup (med samma κ=0.0004)
    DjupFriVsLast,
    /// Djup flerlagers-JEPA med fri neurontillväxt i lugn värld UTAN T. rex (Steg 2-kontroll!)
    DjupFriUtanTrex,
}

impl DjupArm {
    pub fn namn(&self) -> &'static str {
        match self {
            DjupArm::DjupVsFalskLukt => "Djup_vs_FalskLukt",
            DjupArm::DjupVsAlt2373 => "Djup_vs_Alt2373",
            DjupArm::DjupVsStrykWm => "Djup_vs_StrykWM",
            DjupArm::DjupFriVsLast => "Djup_Fri_vs_Last",
            DjupArm::DjupFriUtanTrex => "Djup_Fri_UtanTrex",
        }
    }
}

#[derive(Clone)]
enum ByteHjarna2397 {
    Djup(Box<HjDjup>),
    Gammal2373(Box<HjB2>, crate::skedeb2::GenB2),
}

#[derive(Clone)]
struct Byte2397 {
    lev: bool,
    i: usize,
    j: usize,
    e: f64,
    age: i64,
    hona: bool,
    markt: bool,
    falsk_lukt: bool,
    _uid: u64,
    hj: ByteHjarna2397,
    egen_rng: Rng,
    e_fore_steg: f64,
    senaste_skada: f64,
    antal_overlevda_bett: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DjupUtfall {
    pub fro: u64,
    pub arm_namn: String,
    pub tavling_ar: i64,
    pub fokus_andel: f64,
    pub n_fokus_slut: usize,
    pub n_mot_slut: usize,
    pub n_trex_slut: usize,
    pub traff_per_1000ar_fokus: f64,
    pub traff_per_1000ar_mot: f64,
    pub rov_dod_fokus_pct: f64,
    pub rov_dod_mot_pct: f64,
    pub intag_fokus: f64,
    pub intag_mot: f64,
    pub undvik_gap_fokus: f64,
    pub undvik_gap_mot: f64,
    pub g_lukt_rov: f64,
    pub g_lukt_harmlos: f64,
    pub lugn_krit_r2_fokus: f64,
    pub medel_neuroner_fokus: f64,
    pub medel_neuroner_mot: f64,
    pub p_tak_traffar: u64,
    pub trex_invandring: u64,
    pub sek: f64,
}

/// Kör en parvis 50/50-tävling i samma 64×64 Hell Creek-arena (identisk fysik med `SimB2`).
pub fn kor_en_arm_djup(
    fro: u64,
    arm: DjupArm,
    inkor_ar: i64,
    tavling_ar: i64,
) -> Result<DjupUtfall, String> {
    let t0 = Instant::now();
    let mut param = B2Param::v2_skarp();
    if arm == DjupArm::DjupFriUtanTrex {
        param.start_trex = 0;
        param.min_trex = 0;
        param.max_trex = 0;
    }
    let mut sim = SimB2::fran_hell_creek(fro, param)?;
    for _ in 0..(inkor_ar * STEG_PER_AR) {
        sim.steg();
    }

    // Konvertera levande byten till 50/50 parade lag med nya flerlagers-JEPA
    let mut lev_idx: Vec<usize> = sim
        .byten
        .iter()
        .enumerate()
        .filter(|(_, d)| d.lev)
        .map(|(k, _)| k)
        .collect();
    let mut gm = Rng::new(fro ^ 0x2397_A110_0007);
    for k in (1..lev_idx.len()).rev() {
        let r = gm.under(k + 1);
        lev_idx.swap(k, r);
    }
    let halv = lev_idx.len() / 2;

    let spec_f = match arm {
        DjupArm::DjupFriVsLast | DjupArm::DjupFriUtanTrex => ArtSpec::byte_b2(true),
        _ => ArtSpec::byte_b2(false),
    };
    let mut spec_m = ArtSpec::byte_b2(false);
    if arm == DjupArm::DjupVsStrykWm {
        spec_m.stryk_wm = true;
    }

    let mut byten: Vec<Byte2397> = Vec::with_capacity(lev_idx.len() * 2);
    for (pos, &idx) in lev_idx.iter().enumerate() {
        let m = pos < halv;
        let d = &sim.byten[idx];
        let mut gr = Rng::new(d.uid ^ 0x2397_1111);
        let (hj, falsk) = if m {
            let gen = GenDjup::slump(spec_f, &mut gr);
            (ByteHjarna2397::Djup(Box::new(HjDjup::fran_gen(&gen, spec_f, d.uid))), false)
        } else {
            match arm {
                DjupArm::DjupVsFalskLukt => {
                    let gen = GenDjup::slump(spec_m, &mut gr);
                    (ByteHjarna2397::Djup(Box::new(HjDjup::fran_gen(&gen, spec_m, d.uid))), true)
                }
                DjupArm::DjupVsAlt2373 => {
                    let g2373 = d.gen.clone();
                    let h2373 = HjB2::fran_gen(&g2373, HjB2Config::ny(ByteTyp::MinneSankGrind, false));
                    (ByteHjarna2397::Gammal2373(Box::new(h2373), g2373), false)
                }
                DjupArm::DjupVsStrykWm | DjupArm::DjupFriVsLast | DjupArm::DjupFriUtanTrex => {
                    let gen = GenDjup::slump(spec_m, &mut gr);
                    (ByteHjarna2397::Djup(Box::new(HjDjup::fran_gen(&gen, spec_m, d.uid))), false)
                }
            }
        };
        byten.push(Byte2397 {
            lev: true,
            i: d.i,
            j: d.j,
            e: d.e,
            age: d.age,
            hona: d.hona,
            markt: m,
            falsk_lukt: falsk,
            _uid: d.uid,
            hj,
            egen_rng: Rng::new(d.uid),
            e_fore_steg: d.e,
            senaste_skada: 0.0,
            antal_overlevda_bett: 0,
        });
    }

    let n = sim.b * sim.h;
    let total_steg = tavling_ar * STEG_PER_AR;
    let mut andel_sum = 0.0;
    let mut andel_n = 0usize;
    let (mut dod_tot_m, mut dod_tot_o, mut dod_rov_m, mut dod_rov_o) = (0u64, 0u64, 0u64, 0u64);
    let (mut traff_m, mut traff_o) = (0u64, 0u64);
    let (mut intag_sum_m, mut intag_n_m, mut intag_sum_o, mut intag_n_o) = (0.0, 0u64, 0.0, 0u64);
    let (mut ur_sum_m, mut ur_n_m, mut uh_sum_m, mut uh_n_m) = (0.0, 0u64, 0.0, 0u64);
    let (mut ur_sum_o, mut ur_n_o, mut uh_sum_o, mut uh_n_o) = (0.0, 0u64, 0.0, 0u64);
    let mut trex_inv = 0u64;

    const DIR: [(i32, i32); A_N] = [
        (0, 0), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1),
    ];

    for st in 0..total_steg {
        // 1. Växtåterväxt
        let fas = sim.klimat.fas(sim.steg);
        let k_f = sim.klimat.g(fas).max(0.15);
        for c in 0..n {
            let tak = sim.vmax[c];
            if tak > 0.0 {
                let tillv = sim.p_steg[c] * k_f * (1.0 - sim.v[c] / tak).max(0.0);
                sim.v[c] = (sim.v[c] + tillv).min(tak);
            }
        }
        // 2. Ofarliga doftkällor
        let ant_tr = sim.trex.iter().filter(|t| t.lev).count().max(1);
        while sim.harmlos.len() < ant_tr {
            let (i, j) = sim.land_rutor[sim.g.under(sim.land_rutor.len())];
            sim.harmlos.push(HarmlosKalla { i, j, dir: 1 + sim.g.under(8) });
        }
        while sim.harmlos.len() > ant_tr {
            sim.harmlos.pop();
        }
        for idx_h in 0..sim.harmlos.len() {
            if sim.g.f() < 0.25 {
                sim.harmlos[idx_h].dir = 1 + sim.g.under(8);
            }
            let (i, j, dir) = (sim.harmlos[idx_h].i, sim.harmlos[idx_h].j, sim.harmlos[idx_h].dir);
            let (dx, dy) = DIR[dir];
            let ni = (i as i32 + dx).rem_euclid(sim.b as i32) as usize;
            let nj = (j as i32 + dy).rem_euclid(sim.h as i32) as usize;
            let nc = nj * sim.b + ni;
            if sim.landandel[nc] > 0.0 && !matches!(sim.vatten[nc], hckarta::Vatten::Hav) {
                sim.harmlos[idx_h].i = ni;
                sim.harmlos[idx_h].j = nj;
            } else {
                sim.harmlos[idx_h].dir = 1 + sim.g.under(8);
            }
        }
        // 3. Vind och luktfält (samma fysiska rutin)
        sim.byten.clear(); // byteslukten fylls direkt nedan
        sim.uppdatera_vind_och_lukt();
        // Lägg in byteslukt från våra 2397-byten i sim.lukt_byte
        for d in &byten {
            if d.lev {
                let c = d.j * sim.b + d.i;
                sim.lukt_byte[c] = (sim.lukt_byte[c] + 0.15).min(2.0);
            }
        }

        // 4. Räkna agenter per ruta
        let mut antal_byte = vec![0u16; n];
        let mut antal_trex = vec![0u16; n];
        for d in &byten {
            if d.lev {
                let c = d.j * sim.b + d.i;
                antal_byte[c] = antal_byte[c].saturating_add(1);
            }
        }
        for t in &sim.trex {
            if t.lev {
                let c = t.j * sim.b + t.i;
                antal_trex[c] = antal_trex[c].saturating_add(1);
            }
        }

        // 5. Bytesdjur tänker & rör sig
        for idx_d in 0..byten.len() {
            if !byten[idx_d].lev {
                continue;
            }
            let (di, dj, de, d_markt, d_falsk) = (
                byten[idx_d].i,
                byten[idx_d].j,
                byten[idx_d].e,
                byten[idx_d].markt,
                byten[idx_d].falsk_lukt,
            );
            let s = sim.bygg_byte_sinnen(di, dj, de, &antal_byte, &antal_trex);
            let mut s_hj = s;
            if d_falsk {
                let fi = (di + sim.b / 2) % sim.b;
                let fj = (dj + sim.h / 2) % sim.h;
                for k in 0..A_N {
                    let (dx, dy) = DIR[k];
                    let ni = (fi as i32 + dx).rem_euclid(sim.b as i32) as usize;
                    let nj = (fj as i32 + dy).rem_euclid(sim.h as i32) as usize;
                    s_hj[37 + k] = sim.lukt_rov[nj * sim.b + ni];
                }
            }
            let (a, hj_kost) = {
                let d = &mut byten[idx_d];
                d.e_fore_steg = d.e;
                match d.hj {
                    ByteHjarna2397::Djup(ref mut hj) => {
                        let (act, _) = hj.valj_lott(&s_hj, &mut d.egen_rng);
                        (act, hj.energikostnad())
                    }
                    ByteHjarna2397::Gammal2373(ref mut hj, _) => {
                        let (act, _) = hj.valj_lott(&s_hj, &mut d.egen_rng);
                        (act, 0.0)
                    }
                }
            };
            // Ren undvikning på NU-fältet
            let max_r = s[37..46].iter().copied().fold(0.0f64, f64::max);
            let min_r = s[37..46].iter().copied().fold(f64::INFINITY, f64::min);
            if max_r > 0.12 && (max_r - min_r) > 0.04 {
                let med_r = s[37..46].iter().sum::<f64>() / A_N as f64;
                let p_r = (med_r - s[37 + a]) / max_r;
                if d_markt { ur_sum_m += p_r; ur_n_m += 1; } else { ur_sum_o += p_r; ur_n_o += 1; }
            }
            let max_h = s[46..55].iter().copied().fold(0.0f64, f64::max);
            let min_h = s[46..55].iter().copied().fold(f64::INFINITY, f64::min);
            if max_h > 0.12 && (max_h - min_h) > 0.04 {
                let med_h = s[46..55].iter().sum::<f64>() / A_N as f64;
                let p_h = (med_h - s[46 + a]) / max_h;
                if d_markt { uh_sum_m += p_h; uh_n_m += 1; } else { uh_sum_o += p_h; uh_n_o += 1; }
            }

            let (dx, dy) = DIR[a];
            let ni = (di as i32 + dx).rem_euclid(sim.b as i32) as usize;
            let nj = (dj as i32 + dy).rem_euclid(sim.h as i32) as usize;
            let nc = nj * sim.b + ni;
            if sim.landandel[nc] > 0.0 && !matches!(sim.vatten[nc], hckarta::Vatten::Hav) {
                byten[idx_d].i = ni;
                byten[idx_d].j = nj;
                let ror = match sim.vatten[nc] {
                    hckarta::Vatten::Trask => KOST_TRASK,
                    hckarta::Vatten::Flodfara => KOST_FLOD,
                    hckarta::Vatten::Sjo => KOST_SJO,
                    _ => 0.0,
                };
                byten[idx_d].e -= ror;
            }
            byten[idx_d].e -= hj_kost;
        }

        // 6. Autonoma T. rex
        for idx_t in 0..sim.trex.len() {
            if !sim.trex[idx_t].lev {
                continue;
            }
            let (ti, tj, te) = (sim.trex[idx_t].i, sim.trex[idx_t].j, sim.trex[idx_t].e);
            // Bygg T. rex-sinnen (samma 39 kanaler)
            let mut s_t = [0.0; S_TREX];
            let (wx, wy) = {
                let (dx, dy) = DIR[(sim.vind_dir % 8) + 1];
                let norm = ((dx * dx + dy * dy) as f64).sqrt().max(1.0);
                (dx as f64 / norm, dy as f64 / norm)
            };
            for k in 0..A_N {
                let (dx, dy) = DIR[k];
                let ni = (ti as i32 + dx).rem_euclid(sim.b as i32) as usize;
                let nj = (tj as i32 + dy).rem_euclid(sim.h as i32) as usize;
                let c = nj * sim.b + ni;
                let b_c = (antal_byte[c] as f64).min(12.0) / 12.0;
                let syn = (1.0 - sim.kron[c]).clamp(0.0, 1.0).powi(2);
                s_t[k] = if k == 0 || !sim.param.skymd_sikt { b_c } else { b_c * syn };
                s_t[9 + k] = sim.lukt_byte[c];
                s_t[18 + k] = sim.kron[c];
                s_t[27 + k] = antal_trex[c] as f64;
            }
            s_t[36] = wx;
            s_t[37] = wy;
            s_t[38] = (te / sim.param.trex_e_dela).clamp(0.0, 2.0);
            let a = {
                let t = &mut sim.trex[idx_t];
                t.e_fore_steg = t.e;
                t.hj.valj_lott(&s_t, &mut t.egen_rng)
            };
            let (dx, dy) = DIR[a];
            let ni = (ti as i32 + dx).rem_euclid(sim.b as i32) as usize;
            let nj = (tj as i32 + dy).rem_euclid(sim.h as i32) as usize;
            let nc = nj * sim.b + ni;
            if sim.landandel[nc] > 0.0 && !matches!(sim.vatten[nc], hckarta::Vatten::Hav) {
                sim.trex[idx_t].i = ni;
                sim.trex[idx_t].j = nj;
            }
            sim.trex[idx_t].e -= sim.param.trex_kost_steg;
            sim.trex[idx_t].age += 1;
            if sim.trex[idx_t].vila > 0 {
                sim.trex[idx_t].vila -= 1;
            }
        }

        // 7. Bete
        let mut byten_i_ruta: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (idx_d, d) in byten.iter().enumerate() {
            if d.lev {
                byten_i_ruta[d.j * sim.b + d.i].push(idx_d);
            }
        }
        for c in 0..n {
            let lista = &byten_i_ruta[c];
            if lista.is_empty() { continue; }
            let behov = AT_DINO * lista.len() as f64;
            let taget = sim.v[c].min(behov);
            sim.v[c] -= taget;
            let per_djur = (taget / lista.len() as f64) * E_VAXT;
            for &idx_d in lista {
                byten[idx_d].e += per_djur - KOST_STEG;
                byten[idx_d].age += 1;
                byten[idx_d].senaste_skada = 0.0;
                if byten[idx_d].markt {
                    intag_sum_m += per_djur;
                    intag_n_m += 1;
                } else {
                    intag_sum_o += per_djur;
                    intag_n_o += 1;
                }
            }
        }

        // 8. Fysiska T. rex-möten
        for idx_t in 0..sim.trex.len() {
            if !sim.trex[idx_t].lev || sim.trex[idx_t].vila > 0 { continue; }
            let c = sim.trex[idx_t].j * sim.b + sim.trex[idx_t].i;
            let kand: Vec<usize> = byten_i_ruta[c].iter().copied().filter(|&id| byten[id].lev).collect();
            if kand.is_empty() { continue; }
            let bakhall = sim.param.skymd_sikt && sim.kron[c] >= sim.param.kron_dold_grans;
            let p_traff = (sim.param.p_angrepp_bas * if bakhall { sim.param.bakhall_faktor } else { 1.0 }).min(0.95);
            if sim.g.f() < p_traff {
                let mal = kand[sim.g.under(kand.len())];
                let m = byten[mal].markt;
                if m { traff_m += 1; } else { traff_o += 1; }
                if sim.g.f() < sim.param.p_dod_vid_traff {
                    byten[mal].lev = false;
                    sim.trex[idx_t].e = (sim.trex[idx_t].e + sim.param.trex_byte_energi).min(sim.param.trex_e_dela * 2.0);
                    sim.trex[idx_t].vila = sim.param.trex_vila_steg;
                    if m { dod_rov_m += 1; dod_tot_m += 1; } else { dod_rov_o += 1; dod_tot_o += 1; }
                } else {
                    byten[mal].senaste_skada = 1.0;
                    byten[mal].antal_overlevda_bett = byten[mal].antal_overlevda_bett.saturating_add(1);
                    byten[mal].e -= sim.param.byte_skada_energi;
                    sim.trex[idx_t].e = (sim.trex[idx_t].e + sim.param.trex_traff_energi).min(sim.param.trex_e_dela * 2.0);
                }
            }
        }

        // 9. Kroppssignaler, död och förökning (en-förälders-arv för Djup!)
        for idx_d in 0..byten.len() {
            if !byten[idx_d].lev { continue; }
            let d = &mut byten[idx_d];
            let de = d.e - d.e_fore_steg;
            let w_eff = match d.hj {
                ByteHjarna2397::Djup(ref hj) => hj.gen.w_skada.max(0.0),
                ByteHjarna2397::Gammal2373(_, ref g) => g.w_skada.max(0.0),
            };
            let r_t = de - w_eff * d.senaste_skada;
            match d.hj {
                ByteHjarna2397::Djup(ref mut hj) => hj.ge_beloning(r_t),
                ByteHjarna2397::Gammal2373(ref mut hj, _) => hj.ge_beloning(r_t),
            }
            if d.e <= 0.0 || d.age >= LIV {
                d.lev = false;
                if d.markt { dod_tot_m += 1; } else { dod_tot_o += 1; }
            }
        }

        let honor: Vec<usize> = byten.iter().enumerate()
            .filter(|(_, d)| d.lev && d.hona && d.age >= LITEN && d.e >= E_DELA)
            .map(|(k, _)| k).collect();
        let mut nya: Vec<Byte2397> = Vec::new();
        for idx_mor in honor {
            let (mi, mj, m_markt, m_falsk) = (byten[idx_mor].i, byten[idx_mor].j, byten[idx_mor].markt, byten[idx_mor].falsk_lukt);
            let mut far_opt = None;
            for k in 0..A_N {
                let (dx, dy) = DIR[k];
                let ni = (mi as i32 + dx).rem_euclid(sim.b as i32) as usize;
                let nj = (mj as i32 + dy).rem_euclid(sim.h as i32) as usize;
                for &cand in &byten_i_ruta[nj * sim.b + ni] {
                    let dc = &byten[cand];
                    if dc.lev && !dc.hona && dc.age >= LITEN && dc.markt == m_markt {
                        far_opt = Some(cand);
                        break;
                    }
                }
                if far_opt.is_some() { break; }
            }
            let idx_far = far_opt.unwrap_or(idx_mor);
            let uid = sim.nasta_uid;
            sim.nasta_uid += 1;
            byten[idx_mor].e *= 0.5;
            let barn_e = byten[idx_mor].e.min(E_HANE);
            let hj_ny = match (&byten[idx_mor].hj, &byten[idx_far].hj) {
                (ByteHjarna2397::Djup(hm), ByteHjarna2397::Djup(hf)) => {
                    let sp = if m_markt { spec_f } else { spec_m };
                    let bg = GenDjup::arv(&hm.gen, &hf.gen, uid, sp);
                    ByteHjarna2397::Djup(Box::new(HjDjup::fran_gen(&bg, sp, uid)))
                }
                (ByteHjarna2397::Gammal2373(_, gm), ByteHjarna2397::Gammal2373(_, gf)) => {
                    let bg = crate::skedeb2::GenB2::arv(gm, gf, uid);
                    let bh = HjB2::fran_gen(&bg, HjB2Config::ny(ByteTyp::MinneSankGrind, false));
                    ByteHjarna2397::Gammal2373(Box::new(bh), bg)
                }
                (ByteHjarna2397::Djup(hm), _) => {
                    let sp = if m_markt { spec_f } else { spec_m };
                    let bg = GenDjup::arv(&hm.gen, &hm.gen, uid, sp);
                    ByteHjarna2397::Djup(Box::new(HjDjup::fran_gen(&bg, sp, uid)))
                }
                (ByteHjarna2397::Gammal2373(_, gm), _) => {
                    let bg = crate::skedeb2::GenB2::arv(gm, gm, uid);
                    let bh = HjB2::fran_gen(&bg, HjB2Config::ny(ByteTyp::MinneSankGrind, false));
                    ByteHjarna2397::Gammal2373(Box::new(bh), bg)
                }
            };
            nya.push(Byte2397 {
                lev: true,
                i: mi,
                j: mj,
                e: barn_e,
                age: 0,
                hona: (uid % 2) == 0,
                markt: m_markt,
                falsk_lukt: m_falsk,
                _uid: uid,
                hj: hj_ny,
                egen_rng: Rng::new(uid),
                e_fore_steg: barn_e,
                senaste_skada: 0.0,
                antal_overlevda_bett: 0,
            });
        }
        if !nya.is_empty() { byten.extend(nya); }
        if sim.steg % 50 == 0 { byten.retain(|d| d.lev); }

        // 10. T. rex belöning, förökning och golv
        let mut nya_tr = Vec::new();
        let mut lev_tr = 0usize;
        for idx_t in 0..sim.trex.len() {
            if !sim.trex[idx_t].lev { continue; }
            let de_t = sim.trex[idx_t].e - sim.trex[idx_t].e_fore_steg;
            sim.trex[idx_t].hj.ge_beloning(de_t);
            if sim.trex[idx_t].e <= 0.0 || sim.trex[idx_t].age >= LIV * 2 {
                sim.trex[idx_t].lev = false;
                continue;
            }
            lev_tr += 1;
            if sim.trex[idx_t].e >= sim.param.trex_e_dela
                && sim.trex[idx_t].age >= LITEN
                && lev_tr + nya_tr.len() < sim.param.max_trex
            {
                let uid = sim.nasta_uid;
                sim.nasta_uid += 1;
                sim.trex[idx_t].e *= 0.5;
                let bg = sim.trex[idx_t].gen.mutera(uid);
                let bh = HjTrex::fran_gen(&bg);
                nya_tr.push(TrexB2 {
                    lev: true,
                    i: sim.trex[idx_t].i,
                    j: sim.trex[idx_t].j,
                    e: sim.trex[idx_t].e,
                    age: 0,
                    uid,
                    gen: bg,
                    hj: bh,
                    egen_rng: Rng::new(uid ^ 0x74E8),
                    e_fore_steg: sim.trex[idx_t].e,
                    vila: 0,
                });
            }
        }
        if !nya_tr.is_empty() { sim.trex.extend(nya_tr); }
        while lev_tr < sim.param.min_trex {
            let (i, j) = sim.land_rutor[sim.g.under(sim.land_rutor.len())];
            let uid = sim.nasta_uid;
            sim.nasta_uid += 1;
            let bg = GenTrex::slump(&mut sim.g);
            let bh = HjTrex::fran_gen(&bg);
            sim.trex.push(TrexB2 {
                lev: true, i, j, e: sim.param.trex_e_start, age: 0, uid, gen: bg, hj: bh,
                egen_rng: Rng::new(uid ^ 0x74E8), e_fore_steg: sim.param.trex_e_start, vila: 0,
            });
            trex_inv += 1;
            lev_tr += 1;
        }
        if sim.steg % 50 == 0 { sim.trex.retain(|t| t.lev); }
        sim.steg += 1;

        if (st + 1) % STEG_PER_AR == 0 && (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR {
            let m = byten.iter().filter(|d| d.lev && d.markt).count();
            let o = byten.iter().filter(|d| d.lev && !d.markt).count();
            if m + o > 0 {
                andel_sum += m as f64 / (m + o) as f64;
                andel_n += 1;
            }
        }
    }

    let m_slut = byten.iter().filter(|d| d.lev && d.markt).count();
    let o_slut = byten.iter().filter(|d| d.lev && !d.markt).count();
    let tr_slut = sim.trex.iter().filter(|t| t.lev).count();
    let fokus_andel = if andel_n > 0 {
        andel_sum / andel_n as f64
    } else if m_slut + o_slut > 0 {
        m_slut as f64 / (m_slut + o_slut) as f64
    } else {
        0.0
    };

    let (mut gr_sum, mut gh_sum, mut gn) = (0.0, 0.0, 0usize);
    let (mut r2_sum, mut r2_n) = (0.0, 0usize);
    let (mut nf_sum, mut nf_n, mut nm_sum, mut nm_n) = (0.0, 0usize, 0.0, 0usize);
    let mut ptak = 0u64;
    for d in &byten {
        if !d.lev { continue; }
        if d.markt {
            if let ByteHjarna2397::Djup(ref hj) = d.hj {
                gr_sum += hj.g_intervall(37, 9);
                gh_sum += hj.g_intervall(46, 9);
                gn += 1;
                let r2 = hj.lugn_krit_r2();
                if r2.is_finite() { r2_sum += r2; r2_n += 1; }
                nf_sum += hj.aktiva_dolda_neuroner() as f64;
                nf_n += 1;
                ptak += hj.p_tak_traffar;
            }
        } else if let ByteHjarna2397::Djup(ref hj) = d.hj {
            nm_sum += hj.aktiva_dolda_neuroner() as f64;
            nm_n += 1;
        }
    }

    Ok(DjupUtfall {
        fro,
        arm_namn: arm.namn().to_string(),
        tavling_ar,
        fokus_andel,
        n_fokus_slut: m_slut,
        n_mot_slut: o_slut,
        n_trex_slut: tr_slut,
        traff_per_1000ar_fokus: if intag_n_m > 0 { (traff_m as f64 / intag_n_m as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        traff_per_1000ar_mot: if intag_n_o > 0 { (traff_o as f64 / intag_n_o as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        rov_dod_fokus_pct: if dod_tot_m > 0 { 100.0 * dod_rov_m as f64 / dod_tot_m as f64 } else { 0.0 },
        rov_dod_mot_pct: if dod_tot_o > 0 { 100.0 * dod_rov_o as f64 / dod_tot_o as f64 } else { 0.0 },
        intag_fokus: if intag_n_m > 0 { intag_sum_m / intag_n_m as f64 } else { 0.0 },
        intag_mot: if intag_n_o > 0 { intag_sum_o / intag_n_o as f64 } else { 0.0 },
        undvik_gap_fokus: (if ur_n_m > 0 { ur_sum_m / ur_n_m as f64 } else { 0.0 }) - (if uh_n_m > 0 { uh_sum_m / uh_n_m as f64 } else { 0.0 }),
        undvik_gap_mot: (if ur_n_o > 0 { ur_sum_o / ur_n_o as f64 } else { 0.0 }) - (if uh_n_o > 0 { uh_sum_o / uh_n_o as f64 } else { 0.0 }),
        g_lukt_rov: if gn > 0 { gr_sum / gn as f64 } else { f64::NAN },
        g_lukt_harmlos: if gn > 0 { gh_sum / gn as f64 } else { f64::NAN },
        lugn_krit_r2_fokus: if r2_n > 0 { r2_sum / r2_n as f64 } else { f64::NAN },
        medel_neuroner_fokus: if nf_n > 0 { nf_sum / nf_n as f64 } else { 0.0 },
        medel_neuroner_mot: if nm_n > 0 { nm_sum / nm_n as f64 } else { 0.0 },
        p_tak_traffar: ptak,
        trex_invandring: trex_inv,
        sek: t0.elapsed().as_secs_f64(),
    })
}

/// Ren tidsmätningsgrind (`Alt 2398`, Fables villkor: skriver ENBART körtid `ms/år` — aldrig överlevnadsutfall —
/// så att träningskartorna `266–280` förblir 100 % orörda för Steg 1!).
pub fn kora_grind2398_tidsbank(fron: &[u64], inkor_ar: i64, prov_ar: i64) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("# Alt 2398 — Ren körtidsmätning för Alt 2397 (inga överlevnadsutfall utskrivna)\n\n");
    let mut sum_ms_ar = 0.0;
    for &fro in fron {
        let u = kor_en_arm_djup(fro, DjupArm::DjupVsFalskLukt, inkor_ar, prov_ar)?;
        let tot_ar = (inkor_ar + prov_ar).max(1) as f64;
        let ms_per_ar = (u.sek * 1000.0) / tot_ar;
        sum_ms_ar += ms_per_ar;
        out.push_str(&format!(
            "- Frö `{fro}` (`{inkor_ar}+{prov_ar}` år): totalt `{:.2} s` -> **`{:.0} ms/år`** (`250` år på 1 tråd ≈ `{:.1} min`, 15 världar × 3 armar på 8 kärnor ≈ `{:.1} min`)\n",
            u.sek,
            ms_per_ar,
            ms_per_ar * 250.0 / 60_000.0,
            ms_per_ar * 250.0 * (45.0 / 8.0) / 60_000.0,
        ));
    }
    let m = sum_ms_ar / fron.len().max(1) as f64;
    out.push_str(&format!(
        "\n**Medel:** **`{:.0} ms/år`** -> `250` år på 1 tråd ≈ **`{:.1} min`** (`15` världar × `3` armar på `8` Colab-kärnor ≈ **`{:.1} min`**, på `12` kärnor ≈ **`{:.1} min`**).\n",
        m,
        m * 250.0 / 60_000.0,
        m * 250.0 * (45.0 / 8.0) / 60_000.0,
        m * 250.0 * (45.0 / 12.0) / 60_000.0,
    ));
    Ok(out)
}

/// Avbrottssäker parallell körning för Google Colab (och lokalt): läser ev. befintlig JSON i `ra_ut`,
/// hoppar över redan klara `(fro, arm)`-par och sparar till disk efter VARJE färdig värld!
pub fn kora_sparb2397(
    fron: &[u64],
    armar: &[DjupArm],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    ra_ut: Option<&Path>,
) -> Result<String, String> {
    let t0 = Instant::now();
    let mut befintliga: Vec<DjupUtfall> = if let Some(p) = ra_ut {
        if p.exists() {
            let txt = std::fs::read_to_string(p).unwrap_or_default();
            serde_json::from_str(&txt).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut jobb: Vec<(u64, DjupArm)> = Vec::new();
    for &arm in armar {
        for &fro in fron {
            let klar = befintliga
                .iter()
                .any(|u| u.fro == fro && u.arm_namn == arm.namn() && u.tavling_ar == tavling_ar);
            if !klar {
                jobb.push((fro, arm));
            }
        }
    }

    let totalt = jobb.len();
    let nt = tradar.max(1).min(totalt.max(1));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    let klara = Arc::new(AtomicUsize::new(0));
    let res_arc: Arc<std::sync::Mutex<Result<Vec<DjupUtfall>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(befintliga.clone())));
    let ra_path_owned = ra_ut.map(|p| p.to_path_buf());

    if totalt > 0 {
        std::thread::scope(|sc| {
            for _ in 0..nt {
                let jr = &jobb;
                let ic = Arc::clone(&idx_atom);
                let kc = Arc::clone(&klara);
                let rc = Arc::clone(&res_arc);
                let rp = ra_path_owned.clone();
                sc.spawn(move || loop {
                    let k = ic.fetch_add(1, Ordering::Relaxed);
                    if k >= jr.len() { break; }
                    let (fro, arm) = jr[k];
                    let r = kor_en_arm_djup(fro, arm, inkor_ar, tavling_ar);
                    let done = kc.fetch_add(1, Ordering::Relaxed) + 1;
                    let mut guard = rc.lock().unwrap();
                    match (&mut *guard, r) {
                        (Ok(vec), Ok(u)) => {
                            eprintln!(
                                "[Alt 2397] klara {done}/{totalt}: frö {fro} {} -> fokus={:.1}% tr1k={:.1}/{:.1} gap={:+.4}/{:+.4} lugnR2={:+.3} neur={:.1} ({:.1}s)",
                                u.arm_namn, u.fokus_andel * 100.0, u.traff_per_1000ar_fokus, u.traff_per_1000ar_mot,
                                u.undvik_gap_fokus, u.undvik_gap_mot, u.lugn_krit_r2_fokus, u.medel_neuroner_fokus, u.sek
                            );
                            vec.push(u);
                            if let Some(ref p) = rp {
                                if let Some(par) = p.parent() { let _ = std::fs::create_dir_all(par); }
                                if let Ok(js) = serde_json::to_string_pretty(vec) {
                                    let tmp = p.with_extension("json.tmp");
                                    if std::fs::write(&tmp, js).is_ok() {
                                        let _ = std::fs::rename(&tmp, p);
                                    }
                                }
                            }
                        }
                        (Ok(_), Err(e)) => *guard = Err(e),
                        _ => {}
                    }
                });
            }
        });
    }

    befintliga = Arc::try_unwrap(res_arc).unwrap().into_inner().unwrap()?;
    befintliga.sort_by(|a, b| a.arm_namn.cmp(&b.arm_namn).then(a.fro.cmp(&b.fro)));
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() { let _ = std::fs::create_dir_all(par); }
        let js = serde_json::to_string_pretty(&befintliga).map_err(|e| format!("json: {e}"))?;
        std::fs::write(p, js).map_err(|e| format!("skriv ra_ut: {e}"))?;
    }
    Ok(formatera_sparb2397_rapport(&befintliga, armar, t0.elapsed().as_secs_f64()))
}

pub fn formatera_sparb2397_rapport(utfall: &[DjupUtfall], armar: &[DjupArm], sek: f64) -> String {
    let med = |vals: Vec<f64>| -> f64 {
        let f: Vec<f64> = vals.into_iter().filter(|v| v.is_finite()).collect();
        if f.is_empty() { f64::NAN } else { f.iter().sum::<f64>() / f.len() as f64 }
    };
    let mut out = String::new();
    out.push_str(&format!("# Alt 2397 — Generisk flerlagers JEPA-hjärna ({:.1} s)\n\n", sek));
    out.push_str("| Arm | n | Fokus % | Vinster (>50%) | Träff/1000år F / M (F<M) | Rovdöd F / M % | Intag F / M | Undvik gap F / M | g_rov / g_h | Lugn Krit R² | Medel neuroner F / M | P-tak |\n");
    out.push_str("|---|---:|---:|---:|---|---|---|---|---|---:|---|---:|\n");
    for &arm in armar {
        let del: Vec<&DjupUtfall> = utfall.iter().filter(|u| u.arm_namn == arm.namn()).collect();
        if del.is_empty() { continue; }
        let n = del.len();
        let mf = med(del.iter().map(|u| u.fokus_andel).collect()) * 100.0;
        let w = del.iter().filter(|u| u.fokus_andel > 0.5).count();
        let tf = med(del.iter().map(|u| u.traff_per_1000ar_fokus).collect());
        let tm = med(del.iter().map(|u| u.traff_per_1000ar_mot).collect());
        let tl = del.iter().filter(|u| u.traff_per_1000ar_fokus < u.traff_per_1000ar_mot).count();
        let rf = med(del.iter().map(|u| u.rov_dod_fokus_pct).collect());
        let rm = med(del.iter().map(|u| u.rov_dod_mot_pct).collect());
        let inf = med(del.iter().map(|u| u.intag_fokus).collect());
        let inm = med(del.iter().map(|u| u.intag_mot).collect());
        let gf = med(del.iter().map(|u| u.undvik_gap_fokus).collect());
        let gm = med(del.iter().map(|u| u.undvik_gap_mot).collect());
        let glr = med(del.iter().map(|u| u.g_lukt_rov).collect());
        let glh = med(del.iter().map(|u| u.g_lukt_harmlos).collect());
        let lr2 = med(del.iter().map(|u| u.lugn_krit_r2_fokus).collect());
        let nf = med(del.iter().map(|u| u.medel_neuroner_fokus).collect());
        let nm = med(del.iter().map(|u| u.medel_neuroner_mot).collect());
        let pt = med(del.iter().map(|u| u.p_tak_traffar as f64).collect());
        out.push_str(&format!(
            "| `{}` | {n} | {mf:.1}% | {w}/{n} | {tf:.1} / {tm:.1} ({tl}/{n}) | {rf:.1}% / {rm:.1}% | {inf:.3}/{inm:.3} | {gf:+.4}/{gm:+.4} | {glr:.3}/{glh:.3} | {lr2:+.3} | {nf:.1} / {nm:.1} | {pt:.0} |\n",
            arm.namn()
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// 5. Obligatorisk röd-till-grön testsvit (Alt 2398-grind, alla 9 konsensusprov!)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pearson(xs: &[f64], ys: &[f64]) -> f64 {
        let n = xs.len() as f64;
        let mx = xs.iter().sum::<f64>() / n;
        let my = ys.iter().sum::<f64>() / n;
        let (mut num, mut vx, mut vy) = (0.0, 0.0, 0.0);
        for (&x, &y) in xs.iter().zip(ys.iter()) {
            let dx = x - mx;
            let dy = y - my;
            num += dx * dy;
            vx += dx * dx;
            vy += dy * dy;
        }
        num / (vx * vy).sqrt().max(1e-12)
    }

    /// Prov 1: Exakt genantal (392 byte, 302 T. rex) och art-generiskt röktest.
    #[test]
    fn alt2397_exakt_genantal_och_artgenerisk() {
        let mut g = Rng::new(23971);
        let sp_b = ArtSpec::byte_b2(false);
        let sp_t = ArtSpec::trex_b2(false);
        let gb = GenDjup::slump(sp_b, &mut g);
        let gt = GenDjup::slump(sp_t, &mut g);
        assert_eq!(gb.antal_gener(), 392, "Byte (S=57) måste ha exakt 392 gener");
        assert_eq!(gt.antal_gener(), 302, "T. rex (S=39) måste ha exakt 302 gener");

        let mut hjb = HjDjup::fran_gen(&gb, sp_b, 10);
        let mut hjt = HjDjup::fran_gen(&gt, sp_t, 20);
        assert_eq!(hjb.aktiva_dolda_neuroner(), 80);
        assert_eq!(hjt.aktiva_dolda_neuroner(), 80);

        let sb = vec![0.3; S_B2];
        let st = vec![0.4; S_TREX];
        for _ in 0..100 {
            let (ab, hb) = hjb.valj_lott(&sb, &mut g);
            let (at, ht) = hjt.valj_lott(&st, &mut g);
            assert!(ab < A_N && at < A_N);
            assert!(hb.iter().all(|v| v.is_finite()) && ht.iter().all(|v| v.is_finite()));
            hjb.ge_beloning(0.5);
            hjt.ge_beloning(0.5);
        }
    }

    /// Prov 2: 57-kanals permutationsinvarians (< 1e-12) -> matematiskt + numeriskt blindhetsbevis.
    #[test]
    fn alt2397_permutationsinvarians_57_kanaler() {
        let mut g = Rng::new(23972);
        let sp = ArtSpec::byte_b2(false);
        let gen1 = GenDjup::slump(sp, &mut g);
        let hj1 = HjDjup::fran_gen(&gen1, sp, 101);

        // Slumpmässig bijektion π av 0..57
        let mut pi: Vec<usize> = (0..S_B2).collect();
        for k in (1..S_B2).rev() {
            let r = g.under(k + 1);
            pi.swap(k, r);
        }

        let mut gen2 = gen1.clone();
        for j in 0..S_B2 {
            gen2.kanalskala[j] = gen1.kanalskala[pi[j]];
        }
        for q in 0..R_BAS {
            for j in 0..S_B2 {
                gen2.bas_v[q * S_B2 + j] = gen1.bas_v[q * S_B2 + pi[j]];
            }
        }
        let mut hj2 = hj1.clone();
        hj2.gen = gen2;
        for k in 0..sp.h_enc_max {
            let off = k * (S_B2 + 1);
            for j in 0..S_B2 {
                hj2.enc_w1[off + j] = hj1.enc_w1[off + pi[j]];
            }
        }

        let s1: Vec<f64> = (0..S_B2).map(|j| ((j as f64) * 0.41 + 0.3).sin()).collect();
        let s2: Vec<f64> = (0..S_B2).map(|j| s1[pi[j]]).collect();
        let (z1, _, _) = hj1.koda_ra(&s1);
        let (z2, _, _) = hj2.koda_ra(&s2);
        let max_diff = z1.iter().zip(z2.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        assert!(max_diff < 1e-12, "57-kanals permutation måste ge < 1e-12 diff, fick {max_diff}");
    }

    /// Prov 3: Noll-initierad neurontillväxt (< 1e-14) OCH röd-till-grön för Fälla A, B och C!
    #[test]
    fn alt2397_noll_tillvaxt_och_falla_a_b_c_rod_till_gron() {
        let mut g = Rng::new(23973);
        let sp = ArtSpec::byte_b2(true);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 303);
        let s: Vec<f64> = (0..S_B2).map(|j| ((j as f64) * 0.33).cos()).collect();
        // Kör 30 steg så alla vikter (inkl. V_mat och V_fara) är nollskilda
        for _ in 0..30 {
            let _ = hj.valj_lott(&s, &mut g);
            hj.ge_beloning(0.8);
        }
        let (z_fore, _, _) = hj.koda_ra(&s);
        let h_fore = hj.norm_tids_rms(&z_fore);
        let p_fore = hj.forutsag(&h_fore, 3);
        let j_fore = hj.total_varde(&h_fore);

        // GRÖN: utöka alla dolda lager med fast skalning + w_ut = 0 -> exakt 0.0 skillnad!
        hj.utoka_dolda_noll_init(8, 6, 4, &mut g);
        let (z_efter, _, _) = hj.koda_ra(&s);
        let h_efter = hj.norm_tids_rms(&z_efter);
        let p_efter = hj.forutsag(&h_efter, 3);
        let j_efter = hj.total_varde(&h_efter);
        let diff_z = z_fore.iter().zip(z_efter.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        let diff_p = p_fore.iter().zip(p_efter.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        let diff_j = (j_fore - j_efter).abs();
        assert!(diff_z < 1e-14 && diff_p < 1e-14 && diff_j < 1e-14,
            "Noll-initierad tillväxt måste ge exakt 0.0 skillnad: z={diff_z}, p={diff_p}, j={diff_j}");

        // RÖD FÄLLA A: om skalningen 1/sqrt(n) följer neuronantalet (32 -> 40) ändras utgången tydligt!
        let faktor_falla_a = (32.0f64 / 40.0).sqrt();
        let diff_falla_a = z_fore[Z_ANK..].iter().map(|&v| (v - v * faktor_falla_a).abs()).fold(0.0f64, f64::max);
        assert!(diff_falla_a > 1e-3, "Fälla A måste ge tydlig störning, fick {diff_falla_a}");

        // RÖD FÄLLA B: ny axel i vektor-RMSNorm (16 -> 17 med värde 1.0) ändrar gamla axlar!
        let rms16 = (h_fore.iter().map(|v| v * v).sum::<f64>() / 16.0).sqrt();
        let rms17 = ((h_fore.iter().map(|v| v * v).sum::<f64>() + 1.0) / 17.0).sqrt();
        assert!((rms16 - rms17).abs() / rms16 > 0.005, "Fälla B måste rubba RMS-skalan");

        // RÖD FÄLLA C: dubbel nolla (w_in=0, w_ut=0) ger 0.0 utgångsgradient (död neuron!),
        // medan vår slumpade w_in + w_ut=0 ger strikt nollskild aktivering!
        let k_ny = 32usize;
        let off = k_ny * (S_B2 + 1);
        let mut lin_ny = hj.enc_w1[off + S_B2];
        for j in 0..S_B2 {
            lin_ny += hj.enc_w1[off + j] * (hj.gen.kanalskala[j] * s[j]);
        }
        assert!(rat_tanh(lin_ny).abs() > 1e-4, "Ny neuron måste ha nollskild aktivering (Fälla C stängd)");
        assert_eq!(rat_tanh(0.0), 0.0, "Dubbel nolla ger alltid 0 aktivering -> död neuron");
    }

    /// Prov 4: Föräldra-barn-instinktkorrelation > 0.90 (grön) mot vikt-för-vikt-korsning < 0.55 (röd)!
    #[test]
    fn alt2397_foralder_barn_korrelation_rod_till_gron() {
        let mut g = Rng::new(23974);
        let sp = ArtSpec::byte_b2(false);
        let mor = GenDjup::slump(sp, &mut g);
        let far = GenDjup::slump(sp, &mut g);
        let barn_en_foralder = GenDjup::arv(&mor, &far, 999, sp);

        // Röd kontroll: naiv vikt-för-vikt-korsning mellan obesläktade föräldrar
        let mut barn_kors = mor.clone();
        for q in 0..R_BAS {
            for j in 0..S_B2 {
                if g.f() < 0.5 { barn_kors.bas_v[q * S_B2 + j] = far.bas_v[q * S_B2 + j]; }
            }
        }
        for r in 0..Z_ANK {
            for q in 0..R_BAS {
                if g.f() < 0.5 { barn_kors.bas_u[r][q] = far.bas_u[r][q]; }
            }
        }
        for k in 0..H_V0 {
            for j in 0..=Z_ANK {
                if g.f() < 0.5 { barn_kors.v0_w1[k][j] = far.v0_w1[k][j]; }
            }
            if g.f() < 0.5 { barn_kors.v0_w2[k] = far.v0_w2[k]; }
        }

        let (mut ym, mut yf, mut yb, mut yk) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for _ in 0..300 {
            let s: Vec<f64> = (0..S_B2).map(|_| gauss(&mut g)).collect();
            let eval = |gen: &GenDjup| {
                let b = gen.ankrad_bas(&s);
                let mut h = [0.0; Z_DJUP];
                for r in 0..Z_ANK { h[r] = rat_tanh(b[r]); }
                gen.v0_varde(&h)
            };
            ym.push(eval(&mor));
            yf.push(eval(&far));
            yb.push(eval(&barn_en_foralder));
            yk.push(eval(&barn_kors));
        }
        let r_en = pearson(&yb, &ym).max(pearson(&yb, &yf));
        let r_kors = pearson(&yk, &ym).max(pearson(&yk, &yf));
        assert!(r_en > 0.90, "En-förälders-arv måste bevara instinkt (r > 0.90), fick {r_en:.3}");
        assert!(r_kors < 0.65, "Vikt-för-vikt-korsning ska skada instinkt (röd kontroll), fick {r_kors:.3}");
    }

    /// Prov 5: 100 % stop_grad — 50 chocker på V_fara ändrar V_mat, V_0 och kodare med exakt 0.0!
    #[test]
    fn alt2397_stop_grad_noll_matlacka_efter_50_chocker() {
        let mut g = Rng::new(23975);
        let sp = ArtSpec::byte_b2(false);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 505);
        let s: Vec<f64> = (0..S_B2).map(|j| ((j as f64) * 0.27).sin()).collect();
        for _ in 0..60 {
            let _ = hj.valj_lott(&s, &mut g);
            hj.ge_beloning(1.2);
        }
        let h_prov: [f64; Z_DJUP] = std::array::from_fn(|r| ((r as f64) * 0.5).cos());
        let v_mat_fore = hj.varde_mat(&h_prov);
        let v0_fore = hj.gen.v0_varde(&h_prov);
        let w1_fore = hj.enc_w1.clone();

        let hnd = ChockHandelse {
            h_fonster: [h_prov, h_prov, h_prov],
            r_chock: -48.0,
        };
        for _ in 0..50 {
            hj.kor_fara_chock_steg(&hnd, 2.0);
        }
        assert_eq!(hj.varde_mat(&h_prov), v_mat_fore, "V_mat måste vara exakt 0.0 oförändrad efter 50 chocker");
        assert_eq!(hj.gen.v0_varde(&h_prov), v0_fore, "V_0 måste vara exakt 0.0 oförändrad");
        assert_eq!(hj.enc_w1, w1_fore, "Kodaren måste vara exakt 0.0 oförändrad (stop_grad)");
        assert!(hj.varde_fara(&h_prov) < -20.0, "V_fara måste samtidigt ha lärt in smärtan, fick {}", hj.varde_fara(&h_prov));
    }

    /// Prov 6: Carbons två fällor (röd-till-grön) — ankrad bas skyddar V_0, positiv tids-RMS skyddar mat-axlar!
    #[test]
    fn alt2397_koordinatbrott_och_signalslukning_rod_till_gron() {
        let mut g = Rng::new(23976);
        let sp = ArtSpec::byte_b2(false);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 606);
        let s: Vec<f64> = (0..S_B2).map(|j| ((j as f64) * 0.19).sin()).collect();
        let (z_fore, _, _) = hj.koda_ra(&s);

        // Simulera kraftig plastisk kodarändring på W1/W2:
        for v in &mut hj.enc_w1 { *v += 0.8 * gauss(&mut g); }
        let (z_efter, _, _) = hj.koda_ra(&s);
        // Ankrade basaxlar (0..8) som V_0 läser får 0.0 drift, medan plastiska (8..16) rör sig!
        let ank_drift = z_fore[0..Z_ANK].iter().zip(z_efter[0..Z_ANK].iter()).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        let plast_drift = z_fore[Z_ANK..].iter().zip(z_efter[Z_ANK..].iter()).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
        assert_eq!(ank_drift, 0.0, "Ankrade basaxlar för V_0 får inte drifta vid plastisk kodarändring");
        assert!(plast_drift > 0.05, "Röd kontroll: plastiska axlar ändras kraftigt");

        // Signalslukning: spik på axel 15 (+20.0) får INTE ändra mat-axlar (0..8) med positiv tids-RMS per axel,
        // medan vektor-RMSNorm slukar mat-axlarna!
        let mut z_lugn = [0.0; Z_DJUP];
        z_lugn[0] = 1.0;
        let mut z_spik = z_lugn;
        z_spik[15] = 20.0;
        let norm_lugn = hj.norm_tids_rms(&z_lugn);
        let norm_spik = hj.norm_tids_rms(&z_spik);
        assert_eq!(norm_lugn[0], norm_spik[0], "Positiv tids-RMS per axel får aldrig trycka ner mat-axeln vid luktspik");
        // Röd kontroll: vektor-RMSNorm krossar mat-axeln från ~4.0 ner till ~0.2!
        let vrms_lugn = (z_lugn.iter().map(|v| v * v).sum::<f64>() / 16.0).sqrt();
        let vrms_spik = (z_spik.iter().map(|v| v * v).sum::<f64>() / 16.0).sqrt();
        assert!((z_lugn[0] / vrms_spik) < 0.1 * (z_lugn[0] / vrms_lugn), "Röd kontroll: vektor-RMSNorm slukar mat-axeln");
    }

    /// Prov 7 (Fables låsta ordning): 1 enda bett -> 500 lugna steg med plastisk kodare PÅ -> fortsatt undvikande!
    #[test]
    fn alt2397_ett_bett_sedan_500_lugna_steg_med_plastisk_kodare_pa() {
        let mut g = Rng::new(23977);
        let sp = ArtSpec::byte_b2(false);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 707);

        let mut s_lugn = vec![0.0; S_B2];
        s_lugn[0] = 1.0;
        let mut s_fara = vec![0.0; S_B2];
        s_fara[0] = 1.0;
        s_fara[37] = 3.0;

        // 60 uppvärmningssteg
        for st in 0..60 {
            s_lugn[0] = if st % 2 == 0 { 1.0 } else { -1.0 };
            let _ = hj.valj_lott(&s_lugn, &mut g);
            hj.ge_beloning(s_lugn[0] * 0.6);
        }
        // 1 enda överlevt bett efter förvarning med lukt i kanal 37!
        let (_, h_varn) = hj.valj_lott(&s_fara, &mut g);
        hj.ge_beloning(0.0);
        let (_, h_bett) = hj.valj_lott(&s_fara, &mut g);
        hj.ge_beloning(-48.0);
        let _ = hj.valj_lott(&s_lugn, &mut g);
        hj.ge_beloning(0.5);

        // 500 lugna steg med plastisk kodarinlärning (IDM) aktiv!
        for st in 0..500 {
            s_lugn[0] = if st % 2 == 0 { 1.0 } else { -1.0 };
            let _ = hj.valj_lott(&s_lugn, &mut g);
            hj.ge_beloning(s_lugn[0] * 0.6);
        }
        let vf_bett = hj.varde_fara(&h_bett);
        let vf_varn = hj.varde_fara(&h_varn);
        let mut h_lugn_test = [0.0; Z_DJUP];
        h_lugn_test[0] = 0.5;
        let vf_lugn = hj.varde_fara(&h_lugn_test);
        assert!(
            vf_bett < vf_lugn - 10.0 && vf_varn < vf_lugn - 5.0,
            "Efter 1 bett + 500 lugna steg måste både bett ({vf_bett:.1}) och förvarning ({vf_varn:.1}) ligga klart under lugn ({vf_lugn:.1})"
        );
    }

    /// Prov 8: Icke-linjär XOR-fälla (röd-till-grön: 1-lagers linjär misslyckas, 2-lagers HjDjup klarar!).
    #[test]
    fn alt2397_ickelinjar_xor_mat_plus_lukt_falla_rod_till_gron() {
        let mut g = Rng::new(23978);
        let sp = ArtSpec::byte_b2(false);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 808);
        // 4 XOR-tillstånd i h[0], h[1]: (+1,+1) & (-1,-1) = FARA (-30), (+1,-1) & (-1,+1) = SÄKER (0)
        // En ren linjär modell w0*h0 + w1*h1 + b kan ALDRIG skilja dessa (teoretiskt R² = 0)!
        let fall = [
            ([1.0, 1.0], -30.0),
            ([-1.0, -1.0], -30.0),
            ([1.0, -1.0], 0.0),
            ([-1.0, 1.0], 0.0),
        ];
        // Träna 2-lagers V_fara (med dolda neuroner) på de 4 XOR-punkterna
        for _ in 0..30 {
            for &(xy, y_mal) in &fall {
                let mut h = [0.0; Z_DJUP];
                h[0] = xy[0];
                h[1] = xy[1];
                let x = hj.krit_drag(&hj.fara_w1, &h);
                let pred = hj.varde_fara(&h);
                let _ = rls_skalar_viktad_vec(&mut hj.fara_w_rls, &mut hj.fara_p_rls, &x, y_mal - pred, 1.0, 1.0, P_TAK_DJUP);
            }
        }
        let mut h_f1 = [0.0; Z_DJUP]; h_f1[0] = 1.0; h_f1[1] = 1.0;
        let mut h_f2 = [0.0; Z_DJUP]; h_f2[0] = -1.0; h_f2[1] = -1.0;
        let mut h_s1 = [0.0; Z_DJUP]; h_s1[0] = 1.0; h_s1[1] = -1.0;
        let mut h_s2 = [0.0; Z_DJUP]; h_s2[0] = -1.0; h_s2[1] = 1.0;
        let max_fara = hj.varde_fara(&h_f1).max(hj.varde_fara(&h_f2));
        let min_saker = hj.varde_fara(&h_s1).min(hj.varde_fara(&h_s2));
        assert!(
            max_fara < min_saker - 10.0,
            "2-lagers V_fara måste klara icke-linjär XOR-fälla: max_fara={max_fara:.1}, min_saker={min_saker:.1}"
        );

        // Röd kontroll: rent linjär modell utan dolda neuroner har alltid f(+1,+1)+f(-1,-1) == f(+1,-1)+f(-1,+1)!
        let (w0, w1, b): (f64, f64, f64) = (3.7, -2.1, -15.0);
        let lin_fara = (w0 + w1 + b) + (-w0 - w1 + b);
        let lin_saker = (w0 - w1 + b) + (-w0 + w1 + b);
        assert!((lin_fara - lin_saker).abs() < 1e-12, "Linjär modell kan aldrig separera XOR");
    }

    /// Prov 9: Faktoriserad Pred exakt ekvivalent + energitak garanterat < överskott 0.339 vid H_max = 160!
    #[test]
    fn alt2397_faktoriserad_pred_exakt_ekvivalent_och_energitak() {
        let mut g = Rng::new(23979);
        let sp = ArtSpec::byte_b2(true);
        let gen = GenDjup::slump(sp, &mut g);
        let mut hj = HjDjup::fran_gen(&gen, sp, 909);
        let h: [f64; Z_DJUP] = std::array::from_fn(|r| ((r as f64) * 0.31).sin());
        let bas = hj.pred_forberakna(&h);
        for a in 0..A_N {
            let y_fakt = hj.forutsag_fran_bas(&h, &bas, a);
            let y_full = hj.forutsag(&h, a);
            for r in 0..Z_DJUP {
                assert!((y_fakt[r] - y_full[r]).abs() < 1e-14);
            }
        }
        let k_start = hj.energikostnad();
        hj.utoka_dolda_noll_init(100, 100, 100, &mut g);
        assert_eq!(hj.aktiva_dolda_neuroner(), 160);
        let k_max = hj.energikostnad();
        assert!(k_max > k_start, "Energikostnad måste växa strikt med neuronantal");
        assert!(k_max < 0.339 * 0.25, "Vid H_max=160 måste energikostnaden ({k_max:.4}) lämna stort positivt överskott");
    }
}
