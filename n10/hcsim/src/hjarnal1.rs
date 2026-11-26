//! Alt 2323 (Steg L1 i LeCun-planen `Alt 2285`):
//! Korttidsminne med prioriterad återuppspelning (Del L1-A), exakt tvåstegsplanering (Del L1-B)
//! och epistemisk osäkerhet ur RLS-kovariansen `P_a` med ärvd försiktighetsgen `w_osak` (Del L1-C).
//! Håller `hjarna.rs`, `hjarna2008.rs` och `hjarnab1.rs` helt orörda.

use crate::hjarna::{gauss, lott_bland_max, NK, NP};
use crate::hjarnab1::{GenB1, S_B1, Z_B1};
use crate::rng::Rng;
use crate::tal::{A_N, S_N, Z_N};

/// Antal minnesposter i korttidsminnets cirkulära buffert (Hippocampus, Del L1-A).
pub const M_L1: usize = 64;
/// Steglängd för dämpad normaliserad kritiker-återuppspelning (rör aldrig `P_krit`).
pub const ETA_REPLAY: f64 = 0.35;
/// Glidande minnesspårets uppdateringstakt (`h_t = (1-β) h_{t-1} + β z_t`, ca 4 stegs minne).
pub const BETA_H: f32 = 0.25;
/// Standard diskonteringsfaktor för tvåstegsplanering (`T = 2`, Del L1-B).
pub const GAMMA_L1: f64 = 0.70;

/// Bitflaggor för att slå på/av L1:s tre delmekanismer individuellt.
pub const L1_REPLAY: u8 = 1;
pub const L1_PLAN2: u8 = 2;
pub const L1_OSAK: u8 = 4;
pub const L1_FULL: u8 = L1_REPLAY | L1_PLAN2 | L1_OSAK;

/// Symmetrisk noll-centrerad startfördelning för den ärvda försiktighetsgenen `w_osak` (inget förkodat tecken, Regel 10).
pub const SIGMA0_W_OSAK: f64 = 0.5;
pub const SIGMA_MUT_W_OSAK: f64 = 0.15;
pub const W_OSAK_MAX: f64 = 5.0;
pub const L1_GEN_XOR: u64 = 0x2323_0100_5A4B_0001;

const ETA_NORM: f64 = 0.02;
const ETA_ZNORM: f64 = 0.05;
const GRIND_GAMMA: f32 = 0.995;
const GRIND_MIN_N: f32 = 5.0;
const GRIND_STRAFF: f32 = 3.0;
const GRIND_MATTNAD: f64 = 0.35;
const GRIND_AKTIV_MIN: f64 = 0.20;
const LAMBDA_RLS: f64 = 0.99;
const LAMBDA_KRIT: f64 = 0.99;
const ALPHA_KRIT: f64 = 4.0;

/// Ärvd L1-tilläggsgen utöver `GenB1`: försiktighet mot osäkra handlingar (`> 0` försiktig, `< 0` nyfiken).
#[derive(Clone, Copy, Debug)]
pub struct GenL1 {
    pub w_osak: f64,
}

impl GenL1 {
    pub fn slump(g: &mut Rng) -> Self {
        Self {
            w_osak: (SIGMA0_W_OSAK * gauss(g)).clamp(-W_OSAK_MAX, W_OSAK_MAX),
        }
    }

    pub fn arv(mor: &GenL1, far: &GenL1, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ L1_GEN_XOR);
        let bas = if g.f() < 0.5 { mor.w_osak } else { far.w_osak };
        Self {
            w_osak: (bas + SIGMA_MUT_W_OSAK * gauss(&mut g)).clamp(-W_OSAK_MAX, W_OSAK_MAX),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HjL1Config {
    pub stryk_handling_i_val: bool,
    pub rls_alpha_tio: u16,
    pub med_kritiker: bool,
    pub anvand_w_skada: bool,
    pub r_klamp: f64,
    pub med_replay: bool,
    pub plan_steg: u8,
    pub gamma: f64,
    pub anvand_w_osak: bool,
}

impl HjL1Config {
    pub fn fran_mask(mask: u8, rls_alpha_tio: u16, r_klamp: f64) -> Self {
        Self {
            stryk_handling_i_val: false,
            rls_alpha_tio,
            med_kritiker: true,
            anvand_w_skada: true,
            r_klamp,
            med_replay: (mask & L1_REPLAY) != 0,
            plan_steg: if (mask & L1_PLAN2) != 0 { 2 } else { 1 },
            gamma: GAMMA_L1,
            anvand_w_osak: (mask & L1_OSAK) != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MinnePostL1 {
    pub z_prev: [f32; Z_B1],
    pub z_mal: [f32; Z_B1],
    pub r: f32,
    pub err_abs: f32,
    pub a_prev: u8,
}

#[derive(Clone, Debug)]
pub struct HjL1 {
    pub cfg: HjL1Config,
    pub wk: [[f64; S_B1 + 1]; Z_B1],
    pub w_rls_akt: [[[f64; Z_B1 + 1]; Z_B1]; A_N],
    pub p_rls_akt: [[[f64; Z_B1 + 1]; Z_B1 + 1]; A_N],
    pub v: [f64; Z_B1 + 1],
    pub w_krit: [f64; Z_B1 + 1],
    pub p_krit: [[f64; Z_B1 + 1]; Z_B1 + 1],
    pub w_osak: f64,
    pub vanta_beloning: Option<f64>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    pub skala_init: bool,
    pub mu: [f64; S_B1],
    pub var: [f64; S_B1],
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_B1],
    pub var_z: [f64; Z_B1],
    pub grind_n: [f32; A_N],
    pub grind_sy: [[f32; S_B1]; A_N],
    pub grind_sy2: [[f32; S_B1]; A_N],
    pub grind_sx: [[f32; S_B1]; A_N],
    pub grind_sx2: [[f32; S_B1]; A_N],
    pub grind_syx: Vec<[[f32; S_B1]; S_B1]>,
    pub grind_adj: [[f32; S_B1]; A_N],
    pub kanal_vikt: [f64; S_B1],
    pub forra_ra_sk: Option<[f32; S_B1]>,
    pub forra: Option<([f64; S_B1], [f64; Z_B1], usize, [f64; Z_B1])>,
    pub steg: u64,
    pub senaste_fel: Option<f64>,
    pub senaste_fel_kopiering: Option<f64>,
    /// Del L1-A: kompakt cirkulär hippocampus-buffert (64 poster × 60 B ≈ 3,8 KiB) och tidsminnesspår `h_t`.
    pub buffert: Vec<MinnePostL1>,
    pub buf_pos: usize,
    pub buf_len: usize,
    pub h_spar: [f32; Z_B1],
    pub replay_rng: Rng,
}

fn diag_p(alpha: f64) -> [[f64; Z_B1 + 1]; Z_B1 + 1] {
    let mut p = [[0.0; Z_B1 + 1]; Z_B1 + 1];
    let inv = 1.0 / alpha;
    for i in 0..=Z_B1 {
        p[i][i] = inv;
    }
    p
}

fn dot_zx(w_rad: &[f64; Z_B1 + 1], x: &[f64; Z_B1 + 1]) -> f64 {
    let mut s = 0.0;
    for j in 0..=Z_B1 {
        s += w_rad[j] * x[j];
    }
    s
}

/// Epistemisk osäkerhet `sqrt((xᵀ P x).clamp(0, 100))` direkt ur befintlig RLS-kovariansmatris `P`.
pub fn osakerhet_rls(p: &[[f64; Z_B1 + 1]; Z_B1 + 1], x: &[f64; Z_B1 + 1]) -> f64 {
    let mut xpx = 0.0;
    for m in 0..=Z_B1 {
        let mut s = 0.0;
        for n in 0..=Z_B1 {
            s += p[m][n] * x[n];
        }
        xpx += x[m] * s;
    }
    xpx.clamp(0.0, 100.0).sqrt()
}

fn rls_uppdatera(
    w: &mut [[f64; Z_B1 + 1]; Z_B1],
    p: &mut [[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    target: &[f64; Z_B1],
    pred: &[f64; Z_B1],
    lambda: f64,
) {
    let mut px = [0.0; Z_B1 + 1];
    for m in 0..=Z_B1 {
        let mut s = 0.0;
        for n in 0..=Z_B1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..=Z_B1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + xpx).max(1e-9);
    let mut gain = [0.0; Z_B1 + 1];
    for m in 0..=Z_B1 {
        gain[m] = px[m] / denom;
    }
    for r in 0..Z_B1 {
        let err = target[r] - pred[r];
        for m in 0..=Z_B1 {
            w[r][m] += err * gain[m];
        }
    }
    let inv_l = 1.0 / lambda;
    for m in 0..=Z_B1 {
        for n in 0..=Z_B1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..=Z_B1 {
        for n in 0..m {
            let sym = 0.5 * (p[m][n] + p[n][m]);
            p[m][n] = sym;
            p[n][m] = sym;
        }
        if p[m][m] > 1e3 {
            p[m][m] = 1e3;
        }
    }
}

fn rls_uppdatera_skalar(
    w: &mut [f64; Z_B1 + 1],
    p: &mut [[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    err: f64,
    lambda: f64,
) {
    let mut px = [0.0; Z_B1 + 1];
    for m in 0..=Z_B1 {
        let mut s = 0.0;
        for n in 0..=Z_B1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..=Z_B1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + xpx).max(1e-9);
    let mut gain = [0.0; Z_B1 + 1];
    for m in 0..=Z_B1 {
        gain[m] = px[m] / denom;
        w[m] += err * gain[m];
    }
    let inv_l = 1.0 / lambda;
    for m in 0..=Z_B1 {
        for n in 0..=Z_B1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..=Z_B1 {
        for n in 0..m {
            let sym = 0.5 * (p[m][n] + p[n][m]);
            p[m][n] = sym;
            p[n][m] = sym;
        }
        if p[m][m] > 1e3 {
            p[m][m] = 1e3;
        }
    }
}

/// Dämpad normaliserad projektionsuppdatering för replay — lämnar `P_krit` orörd så kovariansen aldrig exploderar.
fn replay_uppdatera_skalar(
    w: &mut [f64; Z_B1 + 1],
    p: &[[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    err: f64,
    eta: f64,
) {
    let mut px = [0.0; Z_B1 + 1];
    for m in 0..=Z_B1 {
        let mut s = 0.0;
        for n in 0..=Z_B1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..=Z_B1 {
        xpx += x[m] * px[m];
    }
    let denom = (1.0 + xpx.max(0.0)).max(1e-9);
    let skala = eta * err / denom;
    for m in 0..=Z_B1 {
        w[m] += skala * px[m];
    }
}

impl HjL1 {
    pub fn fran_gen(gen: &[f64], gen_b1: &GenB1, gen_l1: &GenL1, uid: u64, cfg: HjL1Config) -> Self {
        let inv_sqrt_s = 1.0 / (S_B1 as f64).sqrt();
        let mut wk = [[0.0; S_B1 + 1]; Z_B1];
        for i in 0..Z_N {
            let start = i * (S_N + 1);
            for j in 0..S_N {
                wk[i][j] = gen[start + j] * inv_sqrt_s;
            }
            for j in 0..(S_B1 - S_N) {
                wk[i][S_N + j] = gen_b1.wk_extra_04[i][j] * inv_sqrt_s;
            }
            wk[i][S_B1] = gen[start + S_N] * inv_sqrt_s;
        }
        for i in 0..(Z_B1 - Z_N) {
            for j in 0..=S_B1 {
                wk[Z_N + i][j] = gen_b1.wk_rad_46[i][j] * inv_sqrt_s;
            }
        }
        let alpha_akt = if cfg.rls_alpha_tio > 0 {
            cfg.rls_alpha_tio as f64 / 10.0
        } else {
            4.0
        };
        let mut v = [0.0; Z_B1 + 1];
        for i in 0..Z_N {
            v[i] = gen[NK + NP + i];
        }
        for i in 0..(Z_B1 - Z_N) {
            v[Z_N + i] = gen_b1.v_extra[i];
        }
        v[Z_B1] = gen[NK + NP + Z_N];
        let buffert = if cfg.med_replay {
            vec![MinnePostL1::default(); M_L1]
        } else {
            Vec::new()
        };
        Self {
            cfg,
            wk,
            w_rls_akt: [[[0.0; Z_B1 + 1]; Z_B1]; A_N],
            p_rls_akt: [diag_p(alpha_akt); A_N],
            v,
            w_krit: [0.0; Z_B1 + 1],
            p_krit: diag_p(ALPHA_KRIT),
            w_osak: gen_l1.w_osak,
            vanta_beloning: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_B1],
            var: [1.0; S_B1],
            z_skala_init: false,
            mu_z: [0.0; Z_B1],
            var_z: [0.25; Z_B1],
            grind_n: [0.0; A_N],
            grind_sy: [[0.0; S_B1]; A_N],
            grind_sy2: [[0.0; S_B1]; A_N],
            grind_sx: [[0.0; S_B1]; A_N],
            grind_sx2: [[0.0; S_B1]; A_N],
            grind_syx: vec![[[0.0; S_B1]; S_B1]; A_N],
            grind_adj: [[0.0; S_B1]; A_N],
            kanal_vikt: [1.0; S_B1],
            forra_ra_sk: None,
            forra: None,
            steg: 0,
            senaste_fel: None,
            senaste_fel_kopiering: None,
            buffert,
            buf_pos: 0,
            buf_len: 0,
            h_spar: [0.0; Z_B1],
            replay_rng: Rng::new(uid ^ 0x2323_4E91_0007),
        }
    }

    pub fn skala(&self, s: &[f64; S_B1]) -> [f64; S_B1] {
        let mut ut = [0.0; S_B1];
        if !self.skala_init {
            return ut;
        }
        for j in 0..S_B1 {
            ut[j] = ((s[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    pub fn uppdatera_skala(&mut self, s: &[f64; S_B1]) {
        if !self.skala_init {
            self.mu = *s;
            self.var = [1.0; S_B1];
            self.skala_init = true;
            return;
        }
        for j in 0..S_B1 {
            let d = s[j] - self.mu[j];
            self.mu[j] += ETA_NORM * d;
            self.var[j] = (1.0 - ETA_NORM) * self.var[j] + ETA_NORM * d * d;
        }
    }

    fn berakna_grind_rad(&self, a: usize) -> [f32; S_B1] {
        let mut rad = [0.0f32; S_B1];
        let n = self.grind_n[a];
        if n < GRIND_MIN_N {
            return rad;
        }
        let inv_n = 1.0f32 / n;
        let straff = GRIND_STRAFF * inv_n;
        let mut mx = [0.0f32; S_B1];
        let mut inv_vx = [0.0f32; S_B1];
        for k in 0..S_B1 {
            let m = self.grind_sx[a][k] * inv_n;
            mx[k] = m;
            let vx = self.grind_sx2[a][k] * inv_n - m * m;
            inv_vx[k] = if vx >= 1e-6 { 1.0 / vx } else { 0.0 };
        }
        let syx_a = &self.grind_syx[a];
        for j in 0..S_B1 {
            let my = self.grind_sy[a][j] * inv_n;
            let vy = self.grind_sy2[a][j] * inv_n - my * my;
            if vy < 1e-6 {
                continue;
            }
            let inv_vy = 1.0 / vy;
            let row = &syx_a[j];
            let mut best_r2 = 0.0f32;
            for k in 0..S_B1 {
                let cov = row[k] * inv_n - my * mx[k];
                let r2 = cov * cov * inv_vy * inv_vx[k];
                if r2 > best_r2 {
                    best_r2 = r2;
                }
            }
            rad[j] = (best_r2 - straff).max(0.0);
        }
        rad
    }

    fn uppdatera_kanalgrind(&mut self, s_sk_nu_f32: &[f32; S_B1]) {
        let (Some(s_prev), Some((_, _, a_prev, _))) = (self.forra_ra_sk, self.forra) else {
            return;
        };
        self.grind_n[a_prev] = GRIND_GAMMA * self.grind_n[a_prev] + 1.0;
        let sy = &mut self.grind_sy[a_prev];
        let sy2 = &mut self.grind_sy2[a_prev];
        let sx = &mut self.grind_sx[a_prev];
        let sx2 = &mut self.grind_sx2[a_prev];
        let syx = &mut self.grind_syx[a_prev];
        for j in 0..S_B1 {
            let yj = s_sk_nu_f32[j];
            let xj = s_prev[j];
            sy[j] = GRIND_GAMMA * sy[j] + yj;
            sy2[j] = GRIND_GAMMA * sy2[j] + yj * yj;
            sx[j] = GRIND_GAMMA * sx[j] + xj;
            sx2[j] = GRIND_GAMMA * sx2[j] + xj * xj;
            let row = &mut syx[j];
            for k in 0..S_B1 {
                row[k] = GRIND_GAMMA * row[k] + yj * s_prev[k];
            }
        }
        if self.steg < 300 {
            self.grind_adj[a_prev] = self.berakna_grind_rad(a_prev);
        } else if self.steg % 20 == 0 {
            for a in 0..A_N {
                self.grind_adj[a] = self.berakna_grind_rad(a);
            }
        } else {
            return;
        }
        let mut poang = [0.0f64; S_B1];
        let mut antal = 0usize;
        for a in 0..A_N {
            if self.grind_n[a] >= GRIND_MIN_N {
                antal += 1;
                for j in 0..S_B1 {
                    poang[j] += self.grind_adj[a][j] as f64;
                }
            }
        }
        if antal > 0 {
            let inv_a = 1.0 / antal as f64;
            for j in 0..S_B1 {
                poang[j] *= inv_a;
            }
        }
        let max_p = poang.iter().copied().fold(0.0f64, f64::max);
        if max_p > GRIND_AKTIV_MIN {
            for j in 0..S_B1 {
                self.kanal_vikt[j] = (poang[j] / GRIND_MATTNAD).clamp(0.0, 1.0).powi(3);
            }
        } else {
            self.kanal_vikt = [1.0; S_B1];
        }
    }

    pub fn kod_ra(&self, s_in: &[f64; S_B1]) -> [f64; Z_B1] {
        let mut z = [0.0; Z_B1];
        for i in 0..Z_B1 {
            let mut lin = self.wk[i][S_B1];
            for j in 0..S_B1 {
                lin += self.wk[i][j] * s_in[j];
            }
            z[i] = lin.tanh();
        }
        z
    }

    pub fn norm_z(&self, z_raw: &[f64; Z_B1]) -> [f64; Z_B1] {
        if !self.z_skala_init {
            return *z_raw;
        }
        let mut ut = [0.0; Z_B1];
        for i in 0..Z_B1 {
            ut[i] = ((z_raw[i] - self.mu_z[i]) / (self.var_z[i] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    pub fn uppdatera_z_skala(&mut self, z_raw: &[f64; Z_B1]) {
        if !self.z_skala_init {
            self.mu_z = *z_raw;
            self.var_z = [0.25; Z_B1];
            self.z_skala_init = true;
            return;
        }
        for i in 0..Z_B1 {
            let d = z_raw[i] - self.mu_z[i];
            self.mu_z[i] += ETA_ZNORM * d;
            self.var_z[i] = (1.0 - ETA_ZNORM) * self.var_z[i] + ETA_ZNORM * d * d;
        }
    }

    pub fn x_av(z: &[f64; Z_B1]) -> [f64; Z_B1 + 1] {
        let mut x = [1.0; Z_B1 + 1];
        x[..Z_B1].copy_from_slice(z);
        x
    }

    pub fn forutsag(&self, z: &[f64; Z_B1], a: usize) -> [f64; Z_B1] {
        let x = Self::x_av(z);
        let mut y = [0.0; Z_B1];
        for r in 0..Z_B1 {
            y[r] = z[r] + dot_zx(&self.w_rls_akt[a][r], &x);
        }
        y
    }

    pub fn forutsag_medel(&self, z: &[f64; Z_B1]) -> [f64; Z_B1] {
        let mut y = [0.0; Z_B1];
        for a in 0..A_N {
            let ya = self.forutsag(z, a);
            for r in 0..Z_B1 {
                y[r] += ya[r] / A_N as f64;
            }
        }
        y
    }

    pub fn v_eff(&self) -> [f64; Z_B1 + 1] {
        if self.cfg.med_kritiker {
            std::array::from_fn(|j| self.v[j] + self.w_krit[j])
        } else {
            self.v
        }
    }

    pub fn varde(&self, y: &[f64; Z_B1]) -> f64 {
        let ve = self.v_eff();
        let mut s = ve[Z_B1];
        for j in 0..Z_B1 {
            s += ve[j] * y[j];
        }
        s
    }

    pub fn osakerhet(&self, z: &[f64; Z_B1], a: usize) -> f64 {
        let x = Self::x_av(z);
        osakerhet_rls(&self.p_rls_akt[a], &x)
    }

    /// Exakt handlingsvärdering (1 steg eller exakt 2 steg med algebraiskt förberäknat andrasteg + valfri osäkerhetsterm).
    pub fn planera_varden(&self, z: &[f64; Z_B1]) -> ([f64; A_N], [[f64; Z_B1]; A_N]) {
        let mut ys = [[0.0; Z_B1]; A_N];
        let mut v1 = [0.0; A_N];
        let y_stryk = if self.cfg.stryk_handling_i_val {
            Some(self.forutsag_medel(z))
        } else {
            None
        };
        let x0 = Self::x_av(z);
        for a in 0..A_N {
            let y = y_stryk.unwrap_or_else(|| self.forutsag(z, a));
            let mut val = self.varde(&y);
            if self.cfg.anvand_w_osak && !self.cfg.stryk_handling_i_val {
                val -= self.w_osak * osakerhet_rls(&self.p_rls_akt[a], &x0);
            }
            ys[a] = y;
            v1[a] = val;
        }
        if self.cfg.plan_steg < 2 || self.cfg.stryk_handling_i_val || self.cfg.gamma == 0.0 {
            return (v1, ys);
        }
        // Exakt algebraisk komprimering för steg 2:
        // V(y1 + W_{a2}·[y1,1]) = (v_eff[6] + Σ_r v_eff[r] y1[r]) + Σ_m c2[a2][m] · [y1,1][m]
        // där c2[a2][m] = Σ_r v_eff[r] · W_{a2}[r][m] beräknas EN gång per steg.
        let ve = self.v_eff();
        let mut c2 = [[0.0; Z_B1 + 1]; A_N];
        for a2 in 0..A_N {
            for m in 0..=Z_B1 {
                let mut s = 0.0;
                for r in 0..Z_B1 {
                    s += ve[r] * self.w_rls_akt[a2][r][m];
                }
                c2[a2][m] = s;
            }
        }
        let mut v_tot = [0.0; A_N];
        for a1 in 0..A_N {
            let y1 = &ys[a1];
            let bas_v = self.varde(y1);
            let x1 = Self::x_av(y1);
            let mut max_v2 = f64::NEG_INFINITY;
            for a2 in 0..A_N {
                let v2 = bas_v + dot_zx(&c2[a2], &x1);
                if v2 > max_v2 {
                    max_v2 = v2;
                }
            }
            v_tot[a1] = v1[a1] + self.cfg.gamma * max_v2;
        }
        (v_tot, ys)
    }

    pub fn krit_r2(&self) -> f64 {
        if self.krit_n < 20 {
            return f64::NAN;
        }
        let n = self.krit_n as f64;
        let my = self.krit_sy / n;
        let ss_tot = self.krit_sy2 - n * my * my;
        if ss_tot <= 1e-9 {
            return f64::NAN;
        }
        1.0 - self.krit_se2 / ss_tot
    }

    pub fn g_vaxt(&self) -> f64 {
        (0..A_N).map(|k| self.kanal_vikt[k * 2]).sum::<f64>() / A_N as f64
    }

    pub fn g_djur(&self) -> f64 {
        (0..A_N).map(|k| self.kanal_vikt[k * 2 + 1]).sum::<f64>() / A_N as f64
    }

    pub fn g_energi(&self) -> f64 {
        self.kanal_vikt[18]
    }

    pub fn g_kron(&self) -> f64 {
        (0..A_N).map(|k| self.kanal_vikt[S_N + k]).sum::<f64>() / A_N as f64
    }

    pub fn minst_besokta_handling(&self, g: &mut Rng) -> usize {
        let mut neg = [0.0; A_N];
        for a in 0..A_N {
            neg[a] = -(self.grind_n[a] as f64);
        }
        lott_bland_max(&neg, g)
    }

    fn spara_och_spela_replay(&mut self, z_prev: &[f64; Z_B1], a_prev: usize, z_mal: &[f64; Z_B1], r: f64, err_abs: f64) {
        if !self.cfg.med_replay || self.buffert.is_empty() {
            return;
        }
        let post = MinnePostL1 {
            z_prev: std::array::from_fn(|i| z_prev[i] as f32),
            z_mal: std::array::from_fn(|i| z_mal[i] as f32),
            r: r as f32,
            err_abs: err_abs as f32,
            a_prev: a_prev as u8,
        };
        self.buffert[self.buf_pos] = post;
        self.buf_pos = (self.buf_pos + 1) % M_L1;
        if self.buf_len < M_L1 {
            self.buf_len += 1;
        }
        if self.buf_len < 4 {
            return;
        }
        // Replay-steg 1: topp-överraskningsminne bland 8 slumpade ur bufferten.
        let mut best_idx = self.replay_rng.under(self.buf_len);
        let mut best_e = self.buffert[best_idx].err_abs;
        for _ in 1..8 {
            let k = self.replay_rng.under(self.buf_len);
            if self.buffert[k].err_abs > best_e {
                best_e = self.buffert[k].err_abs;
                best_idx = k;
            }
        }
        self.replay_en_post(best_idx);
        // Replay-steg 2: likformigt slumpminne ur bufferten (håller minnet balanserat).
        let uni_idx = self.replay_rng.under(self.buf_len);
        self.replay_en_post(uni_idx);
    }

    fn replay_en_post(&mut self, idx: usize) {
        let post = self.buffert[idx];
        let z_f64: [f64; Z_B1] = std::array::from_fn(|i| post.z_mal[i] as f64);
        let pred = self.varde(&z_f64);
        let err = (post.r as f64) - pred;
        let x = Self::x_av(&z_f64);
        replay_uppdatera_skalar(&mut self.w_krit, &self.p_krit, &x, err, ETA_REPLAY);
        self.buffert[idx].err_abs = err.abs() as f32;
    }

    fn uppdatera_fran_forra(&mut self, s_in_now: &[f64; S_B1], lar: bool) {
        self.senaste_fel = None;
        self.senaste_fel_kopiering = None;
        let Some((_, z_prev, a_prev, y_prev)) = self.forra else {
            return;
        };
        let z_raw_now = self.kod_ra(s_in_now);
        let z_mal = self.norm_z(&z_raw_now);

        let mut kvad = 0.0;
        let mut kvad_kop = 0.0;
        for r in 0..Z_B1 {
            let ef = y_prev[r] - z_mal[r];
            kvad += ef * ef;
            let ek = z_prev[r] - z_mal[r];
            kvad_kop += ek * ek;
        }
        self.senaste_fel = Some(kvad);
        self.senaste_fel_kopiering = Some(kvad_kop);

        if !lar {
            return;
        }

        // 1. Lärd kritiker på nutidssteget + valfri prioriterad minnesåteruppspelning (Del L1-A).
        if self.cfg.med_kritiker {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let klamp = self.cfg.r_klamp.max(1.0);
                let r = r_raw.clamp(-klamp, klamp);
                let pred_k = self.varde(&z_mal);
                let err_k = r - pred_k;
                if self.steg >= 100 {
                    self.krit_n += 1;
                    self.krit_sy += r;
                    self.krit_sy2 += r * r;
                    self.krit_se2 += err_k * err_k;
                }
                let x_k = Self::x_av(&z_mal);
                rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
                self.spara_och_spela_replay(&z_prev, a_prev, &z_mal, r, err_k.abs());
            }
        } else {
            let _ = self.vanta_beloning.take();
        }

        // 2. Uppdatera de 9 fristående RLS-världsmodellerna.
        let x = Self::x_av(&z_prev);
        let delta_sann: [f64; Z_B1] = std::array::from_fn(|r| z_mal[r] - z_prev[r]);
        let res_pred: [f64; Z_B1] =
            std::array::from_fn(|r| dot_zx(&self.w_rls_akt[a_prev][r], &x));
        rls_uppdatera(
            &mut self.w_rls_akt[a_prev],
            &mut self.p_rls_akt[a_prev],
            &x,
            &delta_sann,
            &res_pred,
            LAMBDA_RLS,
        );
    }

    pub fn valj_lott(&mut self, s_nu: &[f64; S_B1], lar: bool, g: &mut Rng) -> usize {
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let s_sk_f32: [f32; S_B1] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B1];
        for j in 0..S_B1 {
            s_in[j] = self.kanal_vikt[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(&s_in, lar);
        let z_raw = self.kod_ra(&s_in);
        if !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);
        for i in 0..Z_B1 {
            self.h_spar[i] = (1.0 - BETA_H) * self.h_spar[i] + BETA_H * (z[i] as f32);
        }

        let (varden, ys) = self.planera_varden(&z);
        let ba = lott_bland_max(&varden, g);
        if lar {
            self.uppdatera_kanalgrind(&s_sk_f32);
            self.uppdatera_skala(s_nu);
            self.uppdatera_z_skala(&z_raw);
        }
        self.forra = Some((s_in, z, ba, ys[ba]));
        self.forra_ra_sk = Some(s_sk_f32);
        self.steg += 1;
        ba
    }

    pub fn lar_och_satt(&mut self, s_nu: &[f64; S_B1], lar: bool, a: usize) {
        assert!(a < A_N);
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let s_sk_f32: [f32; S_B1] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B1];
        for j in 0..S_B1 {
            s_in[j] = self.kanal_vikt[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(&s_in, lar);
        let z_raw = self.kod_ra(&s_in);
        if !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);
        for i in 0..Z_B1 {
            self.h_spar[i] = (1.0 - BETA_H) * self.h_spar[i] + BETA_H * (z[i] as f32);
        }
        let ya = self.forutsag(&z, a);
        if lar {
            self.uppdatera_kanalgrind(&s_sk_f32);
            self.uppdatera_skala(s_nu);
            self.uppdatera_z_skala(&z_raw);
        }
        self.forra = Some((s_in, z, a, ya));
        self.forra_ra_sk = Some(s_sk_f32);
        self.steg += 1;
    }
}

#[cfg(test)]
mod prov {
    use super::*;
    use crate::hjarna::Hj;
    use crate::hjarnab1::{HjB1, HjB1Config};

    #[test]
    fn l1_avstangd_matchar_b1_exakt() {
        let mut rng = Rng::new(232301);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1::slump(&mut rng);
        let cfg_b1 = HjB1Config {
            stryk_handling_i_val: false,
            rls_alpha_tio: 40,
            med_kritiker: true,
            anvand_w_skada: true,
            r_klamp: 20.0,
        };
        let cfg_l1 = HjL1Config::fran_mask(0, 40, 20.0);
        let mut b1 = HjB1::fran_gen(&hj.gen, &gb1, cfg_b1);
        let mut l1 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 42, cfg_l1);
        let mut g_b1 = Rng::new(999);
        let mut g_l1 = Rng::new(999);
        for t in 0..300 {
            let mut s = [0.0; S_B1];
            for j in 0..S_B1 {
                s[j] = ((t * 7 + j * 13) as f64).sin() * 0.5 + 0.5;
            }
            let r = (t as f64 * 0.13).cos() * 2.0;
            b1.vanta_beloning = Some(r);
            l1.vanta_beloning = Some(r);
            let a_b1 = b1.valj_lott(&s, true, &mut g_b1);
            let a_l1 = l1.valj_lott(&s, true, &mut g_l1);
            assert_eq!(a_b1, a_l1, "avstängd L1 avvek från B1 vid steg {t}");
        }
        for j in 0..=Z_B1 {
            assert!((b1.w_krit[j] - l1.w_krit[j]).abs() < 1e-12);
        }
    }

    #[test]
    fn l1_tva_stegs_planering_tar_omvag_och_matchar_direkt_rullning() {
        // Syntetisk 2-stegsfälla:
        // Från start z = [0, 0, ...]:
        //   - a = 0 (fälla): ger +0.5 direkt i steg 1, men sedan fast på +0.5 (steg 2 ger +0.5 -> summa 0.5 + 0.7*0.5 = 0.85)
        //   - a = 1 (omväg): ger 0.0 i steg 1 (z -> [1, 0, ...]), men från [1, 0, ...] ger a = 2 +3.0 i steg 2 (summa 0.0 + 0.7*3.0 = 2.10!)
        let mut rng = Rng::new(232302);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let bygg = |plan_steg: u8| {
            let mut h = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 1, HjL1Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: 40,
                med_kritiker: false,
                anvand_w_skada: false,
                r_klamp: 20.0,
                med_replay: false,
                plan_steg,
                gamma: GAMMA_L1,
                anvand_w_osak: false,
            });
            h.v = [0.0; Z_B1 + 1];
            h.v[1] = 1.0; // belöning ligger på axel 1
            h.w_rls_akt = [[[0.0; Z_B1 + 1]; Z_B1]; A_N];
            // a = 0: delta_z1 = +0.5 om z0 == 0, men -0.5*z0 så den ej staplar; sätt bara bias på z1 för a=0: +0.5 - 0.5*z1 (fast på 0.5)
            h.w_rls_akt[0][1][Z_B1] = 0.5;
            h.w_rls_akt[0][1][1] = -1.0; // y1_1 = 0.5 alltid
            // a = 1: flyttar z0 från 0 -> +1 (y1_0 = 1.0), ger 0 på axel 1
            h.w_rls_akt[1][0][Z_B1] = 1.0;
            h.w_rls_akt[1][0][0] = -1.0;
            // a = 2: om z0 == 1 ger +3.0 på axel 1!
            h.w_rls_akt[2][1][0] = 3.0;
            h
        };

        let z0 = [0.0; Z_B1];
        // Röd kontroll: 1-stegsplanering väljer fällan a = 0!
        let h1 = bygg(1);
        let (v1, _) = h1.planera_varden(&z0);
        let a_1steg = lott_bland_max(&v1, &mut Rng::new(7));
        assert_eq!(a_1steg, 0, "1-stegsplanering borde fastna i lokala fällan a=0, fick {a_1steg}");

        // Grön kontroll: 2-stegsplanering tar omvägen a = 1!
        let h2 = bygg(2);
        let (v2, ys) = h2.planera_varden(&z0);
        let a_2steg = lott_bland_max(&v2, &mut Rng::new(7));
        assert_eq!(a_2steg, 1, "2-stegsplanering borde ta omvägen a=1, fick {a_2steg} (v2={v2:?})");

        // Verifiera att vår algebraiska komprimering matchar direkt 2-stegsrullning < 1e-12!
        for a1 in 0..A_N {
            let mut max_direkt = f64::NEG_INFINITY;
            for a2 in 0..A_N {
                let y2 = h2.forutsag(&ys[a1], a2);
                max_direkt = max_direkt.max(h2.varde(&y2));
            }
            let exakt = h2.varde(&ys[a1]) + GAMMA_L1 * max_direkt;
            assert!((v2[a1] - exakt).abs() < 1e-12, "algebraisk 2-stegsplan avvek: {} vs {}", v2[a1], exakt);
        }
    }

    #[test]
    fn l1_replay_haller_pkrit_begransad_och_minns_sallsynta_smarthaendelser() {
        let mut rng = Rng::new(232303);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let mut med_rep = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 10, HjL1Config::fran_mask(L1_REPLAY, 40, 20.0));
        let mut utan_rep = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 10, HjL1Config::fran_mask(0, 40, 20.0));
        // Nollställ ärvd v så bara kritikern avgör
        med_rep.v = [0.0; Z_B1 + 1];
        utan_rep.v = [0.0; Z_B1 + 1];

        let z_fara = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let z_lugn = [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        // Mata in 3 sällsynta smärtangrepp i z_fara (r = -8.0) följt av 40 lugna steg i z_lugn (r = +0.2)
        for k in 0..43 {
            let (z_mal, r) = if k < 3 { (z_fara, -8.0) } else { (z_lugn, 0.2) };
            for h in [&mut med_rep, &mut utan_rep] {
                let pred = h.varde(&z_mal);
                let err = r - pred;
                let x = HjL1::x_av(&z_mal);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
                h.spara_och_spela_replay(&z_lugn, 0, &z_mal, r, err.abs());
            }
        }
        // 1. P_krit förblir identisk och ändlig (exploderar aldrig under replay!)
        for m in 0..=Z_B1 {
            for n in 0..=Z_B1 {
                assert!(med_rep.p_krit[m][n].is_finite());
                assert!((med_rep.p_krit[m][n] - utan_rep.p_krit[m][n]).abs() < 1e-12);
            }
        }
        // 2. Med prioriterad replay minns kritikern faran starkare (närmare -8.0) efter 40 lugna steg!
        let v_med = med_rep.varde(&z_fara);
        let v_utan = utan_rep.varde(&z_fara);
        assert!(
            (v_med - (-8.0)).abs() < (v_utan - (-8.0)).abs(),
            "replay borde bevara minnet av sällsynt smärta bättre: v_med={v_med:.3}, v_utan={v_utan:.3}"
        );
    }

    #[test]
    fn l1_osakerhet_sjunker_med_erfarenhet_och_w_osak_symmetrisk() {
        let mut rng = Rng::new(232304);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 1.0 };
        let mut h = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 5, HjL1Config::fran_mask(L1_OSAK, 40, 20.0));
        let z = [0.5, -0.3, 0.2, 0.1, -0.4, 0.6];
        let u_fore = h.osakerhet(&z, 3);
        let u_annan = h.osakerhet(&z, 4);
        assert!((u_fore - u_annan).abs() < 1e-12 && u_fore > 0.0);
        // Träna handling 3 tio gånger -> dess osäkerhet ska falla strikt under obesökta handling 4!
        let x = HjL1::x_av(&z);
        for _ in 0..10 {
            let delta = [0.1; Z_B1];
            let pred: [f64; Z_B1] = std::array::from_fn(|r| dot_zx(&h.w_rls_akt[3][r], &x));
            rls_uppdatera(&mut h.w_rls_akt[3], &mut h.p_rls_akt[3], &x, &delta, &pred, LAMBDA_RLS);
        }
        let u_efter = h.osakerhet(&z, 3);
        assert!(u_efter < u_fore * 0.6, "osäkerheten borde sjunka efter träning: före={u_fore:.3}, efter={u_efter:.3}");
        assert!(u_efter < h.osakerhet(&z, 4), "obesökt handling 4 ska ha högre osäkerhet än besökt 3");

        // Verifiera att startgenen w_osak är symmetrisk kring 0 utan förkodat tecken
        let mut g2 = Rng::new(232305);
        let mut pos = 0usize;
        let mut sum = 0.0;
        for _ in 0..1000 {
            let w = GenL1::slump(&mut g2).w_osak;
            if w > 0.0 { pos += 1; }
            sum += w;
        }
        assert!((440..=560).contains(&pos), "w_osak ej symmetrisk: pos={pos}/1000");
        assert!((sum / 1000.0).abs() < 0.1, "w_osak medelvärde ej nära 0: {}", sum / 1000.0);
    }
}
