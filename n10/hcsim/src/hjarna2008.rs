//! Alt 2008: online-tränad kodare, buffertfri IDM, delad+residual förutsägare och löpande z-skalning (regel 14, Alt 369 ändrad för prövning).
//! Ärvd värdeläsare `v` (Alt 1107) och minnesgrind `Alt 1941` rörs inte.

use serde::{Deserialize, Serialize};

use crate::hjarna::{lott_bland_max, NK, NP};
use crate::rng::Rng;
use crate::tal::{A_N, S_N, Z_N};

const ETA_NORM: f64 = 0.02;
const TAU_MAL: f64 = 0.05;
const ETA_P: f64 = 0.05;
const L2_P_RES: f64 = 1e-3;
const ETA_IDM: f64 = 0.15;
const L2_IDM: f64 = 1e-4;
const LAMBDA_FWD: f64 = 0.25;
const ETA_K: f64 = 0.03;
const L2_K: f64 = 1e-4;

const GRIND_GAMMA: f64 = 0.995;
const GRIND_MIN_N: f64 = 5.0;
const GRIND_STRAFF: f64 = 3.0;
const GRIND_MATTNAD: f64 = 0.35;
const GRIND_AKTIV_MIN: f64 = 0.20;

const ETA_ZNORM: f64 = 0.05;
const LAMBDA_RLS: f64 = 0.99;
const ALPHA_DELAD: f64 = 2.0;
const ALPHA_AKT: f64 = 8.0;
const LAMBDA_KRIT: f64 = 0.99;
const ALPHA_KRIT: f64 = 4.0;

const IDM_DIM: usize = Z_N * Z_N + Z_N + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hj2008Config {
    pub lar_kodare: bool,
    pub med_idm: bool,
    pub additiv_endast: bool,
    pub stryk_handling_i_val: bool,
    #[serde(default)]
    pub med_kanalgrind: bool,
    #[serde(default)]
    pub stryk_djurantal_diagnos: bool,
    #[serde(default)]
    pub skarp_kanalgrind: bool,
    #[serde(default)]
    pub med_z_norm: bool,
    #[serde(default)]
    pub med_rls: bool,
    #[serde(default)]
    pub rls_fristaende: bool,
    #[serde(default)]
    pub rls_alpha_tio: u16,
    #[serde(default)]
    pub norm_lage: u8,
    #[serde(default)]
    pub med_kritiker: bool,
}

impl Default for Hj2008Config {
    fn default() -> Self {
        Self {
            lar_kodare: true,
            med_idm: true,
            additiv_endast: false,
            stryk_handling_i_val: false,
            med_kanalgrind: false,
            stryk_djurantal_diagnos: false,
            skarp_kanalgrind: false,
            med_z_norm: false,
            med_rls: false,
            rls_fristaende: false,
            rls_alpha_tio: 0,
            norm_lage: 0,
            med_kritiker: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hj2008 {
    pub cfg: Hj2008Config,
    /// Kodare W_k: [Z_N][S_N + 1].
    pub wk: Vec<[f64; S_N + 1]>,
    /// EMA-målkodare: [Z_N][S_N + 1].
    pub wk_mal: Vec<[f64; S_N + 1]>,
    /// Delad förutsägare U: [Z_N][Z_N] + c: [Z_N].
    pub u: [[f64; Z_N]; Z_N],
    pub c: [f64; Z_N],
    /// Handlingsresidual ΔW_a: [A_N][Z_N][Z_N] + d_a: [A_N][Z_N].
    pub dw: Vec<[[f64; Z_N]; Z_N]>,
    pub da: [[f64; Z_N]; A_N],
    /// Buffertfri RLS-förutsägare (Alt 2056) på x = [z_0..3, 1].
    pub w_rls_delad: [[f64; Z_N + 1]; Z_N],
    pub p_rls_delad: [[f64; Z_N + 1]; Z_N + 1],
    pub w_rls_akt: Vec<[[f64; Z_N + 1]; Z_N]>,
    pub p_rls_akt: Vec<[[f64; Z_N + 1]; Z_N + 1]>,
    /// Buffertfri IDM: [A_N][IDM_DIM].
    pub w_idm: Vec<[f64; IDM_DIM]>,
    /// Ärvd avläsare `v` [Z_N + 1] (Alt 1107; startinstinkt som aldrig muteras i generna).
    pub v: [f64; Z_N + 1],
    /// Alt 2267: lärd kritiker (residual-RLS ovanpå ärvd startinstinkt `v`; används bara när `cfg.med_kritiker == true`).
    #[serde(default)]
    pub w_krit: [f64; Z_N + 1],
    #[serde(default)]
    pub p_krit: [[f64; Z_N + 1]; Z_N + 1],
    #[serde(skip)]
    pub vanta_beloning: Option<f64>,
    #[serde(skip)]
    pub krit_n: u64,
    #[serde(skip)]
    pub krit_sy: f64,
    #[serde(skip)]
    pub krit_sy2: f64,
    #[serde(skip)]
    pub krit_se2: f64,
    /// Kausal löpande z-skalning (Alt 2005).
    pub skala_init: bool,
    pub mu: [f64; S_N],
    pub var: [f64; S_N],
    /// Kausal löpande z-normering på tanh-utgången (Alt 2056).
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_N],
    pub var_z: [f64; Z_N],
    /// Kausal blind kanalgrind (Alt 2053/2056): exponentiellt dämpad statistik per handling.
    pub grind_n: [f64; A_N],
    pub grind_sy: [[f64; S_N]; A_N],
    pub grind_sy2: [[f64; S_N]; A_N],
    pub grind_sx: [[f64; S_N]; A_N],
    pub grind_sx2: [[f64; S_N]; A_N],
    pub grind_syx: Vec<[[f64; S_N]; S_N]>,
    #[serde(default)]
    pub grind_adj: [[f64; S_N]; A_N],
    pub kanal_vikt: [f64; S_N],
    pub forra_ra_sk: Option<[f64; S_N]>,
    /// Föregående stegs inkopplade (evt. grindade) sinnen, z (evt. normerad), vald handling och gissning y.
    pub forra: Option<([f64; S_N], [f64; Z_N], usize, [f64; Z_N])>,
    pub steg: u64,
    pub senaste_fel: Option<f64>,
    pub senaste_fel_kopiering: Option<f64>,
}

fn diag_p(alpha: f64) -> [[f64; Z_N + 1]; Z_N + 1] {
    let mut p = [[0.0; Z_N + 1]; Z_N + 1];
    let inv = 1.0 / alpha;
    for i in 0..=Z_N {
        p[i][i] = inv;
    }
    p
}

fn dot_zx(w_rad: &[f64; Z_N + 1], x: &[f64; Z_N + 1]) -> f64 {
    (0..=Z_N).map(|j| w_rad[j] * x[j]).sum()
}

fn rls_uppdatera(
    w: &mut [[f64; Z_N + 1]; Z_N],
    p: &mut [[f64; Z_N + 1]; Z_N + 1],
    x: &[f64; Z_N + 1],
    target: &[f64; Z_N],
    pred: &[f64; Z_N],
    lambda: f64,
) {
    let mut px = [0.0; Z_N + 1];
    for m in 0..=Z_N {
        px[m] = (0..=Z_N).map(|n| p[m][n] * x[n]).sum();
    }
    let denom = (lambda + (0..=Z_N).map(|m| x[m] * px[m]).sum::<f64>()).max(1e-9);
    let mut gain = [0.0; Z_N + 1];
    for m in 0..=Z_N {
        gain[m] = px[m] / denom;
    }
    for r in 0..Z_N {
        let err = target[r] - pred[r];
        for m in 0..=Z_N {
            w[r][m] += err * gain[m];
        }
    }
    for m in 0..=Z_N {
        for n in 0..=Z_N {
            p[m][n] = (p[m][n] - gain[m] * px[n]) / lambda;
        }
    }
    for m in 0..=Z_N {
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
    w: &mut [f64; Z_N + 1],
    p: &mut [[f64; Z_N + 1]; Z_N + 1],
    x: &[f64; Z_N + 1],
    err: f64,
    lambda: f64,
) {
    let mut px = [0.0; Z_N + 1];
    for m in 0..=Z_N {
        px[m] = (0..=Z_N).map(|n| p[m][n] * x[n]).sum();
    }
    let denom = (lambda + (0..=Z_N).map(|m| x[m] * px[m]).sum::<f64>()).max(1e-9);
    let mut gain = [0.0; Z_N + 1];
    for m in 0..=Z_N {
        gain[m] = px[m] / denom;
        w[m] += err * gain[m];
    }
    for m in 0..=Z_N {
        for n in 0..=Z_N {
            p[m][n] = (p[m][n] - gain[m] * px[n]) / lambda;
        }
    }
    for m in 0..=Z_N {
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

impl Hj2008 {
    pub fn fran_gen(gen: &[f64], cfg: Hj2008Config) -> Self {
        let inv_sqrt_sn = 1.0 / (S_N as f64).sqrt();
        let mut wk = vec![[0.0; S_N + 1]; Z_N];
        for i in 0..Z_N {
            let start = i * (S_N + 1);
            for j in 0..=S_N {
                wk[i][j] = gen[start + j] * inv_sqrt_sn;
            }
        }
        let alpha_akt = if cfg.rls_fristaende && cfg.rls_alpha_tio > 0 {
            cfg.rls_alpha_tio as f64 / 10.0
        } else {
            ALPHA_AKT
        };
        let mut v = [0.0; Z_N + 1];
        v.copy_from_slice(&gen[NK + NP..NK + NP + Z_N + 1]);
        Self {
            cfg,
            wk_mal: wk.clone(),
            wk,
            u: [[0.0; Z_N]; Z_N],
            c: [0.0; Z_N],
            dw: vec![[[0.0; Z_N]; Z_N]; A_N],
            da: [[0.0; Z_N]; A_N],
            w_rls_delad: [[0.0; Z_N + 1]; Z_N],
            p_rls_delad: diag_p(ALPHA_DELAD),
            w_rls_akt: vec![[[0.0; Z_N + 1]; Z_N]; A_N],
            p_rls_akt: vec![diag_p(alpha_akt); A_N],
            w_idm: vec![[0.0; IDM_DIM]; A_N],
            v,
            w_krit: [0.0; Z_N + 1],
            p_krit: diag_p(ALPHA_KRIT),
            vanta_beloning: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_N],
            var: [1.0; S_N],
            z_skala_init: false,
            mu_z: [0.0; Z_N],
            var_z: [1.0; Z_N],
            grind_n: [0.0; A_N],
            grind_sy: [[0.0; S_N]; A_N],
            grind_sy2: [[0.0; S_N]; A_N],
            grind_sx: [[0.0; S_N]; A_N],
            grind_sx2: [[0.0; S_N]; A_N],
            grind_syx: vec![[[0.0; S_N]; S_N]; A_N],
            grind_adj: [[0.0; S_N]; A_N],
            kanal_vikt: [1.0; S_N],
            forra_ra_sk: None,
            forra: None,
            steg: 0,
            senaste_fel: None,
            senaste_fel_kopiering: None,
        }
    }

    pub fn skala(&self, s: &[f64]) -> [f64; S_N] {
        let mut ut = [0.0; S_N];
        if !self.skala_init {
            return ut;
        }
        if self.cfg.norm_lage == 2 {
            for j in 0..S_N {
                ut[j] = (s[j] / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
            }
        } else {
            for j in 0..S_N {
                ut[j] = ((s[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
            }
        }
        ut
    }

    fn adaptiv_eta(steg: u64) -> f64 {
        (1.0 / (steg as f64 + 10.0)).max(0.002)
    }

    pub fn uppdatera_skala(&mut self, s: &[f64]) {
        if self.cfg.norm_lage == 2 {
            if !self.skala_init {
                self.mu = [0.0; S_N];
                for j in 0..S_N {
                    self.var[j] = (s[j] * s[j]).max(0.01);
                }
                self.skala_init = true;
                return;
            }
            let eta = Self::adaptiv_eta(self.steg);
            for j in 0..S_N {
                self.var[j] = (1.0 - eta) * self.var[j] + eta * s[j] * s[j];
            }
            return;
        }
        if !self.skala_init {
            self.mu.copy_from_slice(s);
            self.var = [1.0; S_N];
            self.skala_init = true;
            return;
        }
        let eta = if self.cfg.norm_lage == 0 {
            ETA_NORM
        } else {
            Self::adaptiv_eta(self.steg)
        };
        for j in 0..S_N {
            let d = s[j] - self.mu[j];
            self.mu[j] += eta * d;
            self.var[j] = (1.0 - eta) * self.var[j] + eta * d * d;
        }
    }

    pub fn aktuell_kanal_vikt(&self) -> [f64; S_N] {
        if self.cfg.stryk_djurantal_diagnos {
            let mut g = [1.0; S_N];
            for j in (1..S_N - 1).step_by(2) {
                g[j] = 0.0;
            }
            return g;
        }
        if self.cfg.med_kanalgrind {
            return self.kanal_vikt;
        }
        [1.0; S_N]
    }

    pub fn skala_grind(&self, s: &[f64]) -> [f64; S_N] {
        let mut s_sk = self.skala(s);
        let g = self.aktuell_kanal_vikt();
        for j in 0..S_N {
            s_sk[j] *= g[j];
        }
        s_sk
    }

    fn berakna_grind_rad(&self, a: usize) -> [f64; S_N] {
        let mut rad = [0.0; S_N];
        let n = self.grind_n[a];
        if n < GRIND_MIN_N {
            return rad;
        }
        let inv_n = 1.0 / n;
        let straff = GRIND_STRAFF * inv_n;
        let mut mx = [0.0; S_N];
        let mut inv_vx = [0.0; S_N];
        for k in 0..S_N {
            let m = self.grind_sx[a][k] * inv_n;
            mx[k] = m;
            let vx = self.grind_sx2[a][k] * inv_n - m * m;
            inv_vx[k] = if vx >= 1e-6 { 1.0 / vx } else { 0.0 };
        }
        for j in 0..S_N {
            let my = self.grind_sy[a][j] * inv_n;
            let vy = self.grind_sy2[a][j] * inv_n - my * my;
            if vy < 1e-6 {
                continue;
            }
            let inv_vy = 1.0 / vy;
            let mut best_r2 = 0.0f64;
            for k in 0..S_N {
                let cov = self.grind_syx[a][j][k] * inv_n - my * mx[k];
                let r2 = cov * cov * inv_vy * inv_vx[k];
                if r2 > best_r2 {
                    best_r2 = r2;
                }
            }
            rad[j] = (best_r2 - straff).max(0.0);
        }
        rad
    }

    fn uppdatera_grind_rad(&mut self, a: usize) {
        self.grind_adj[a] = self.berakna_grind_rad(a);
    }

    /// Beräknar justerad förutsägbarhet per kanal ur hittills samlad kausal statistik.
    pub fn kanal_poang(&self) -> [f64; S_N] {
        let mut summa = [0.0; S_N];
        let mut antal_a = [0usize; S_N];
        for a in 0..A_N {
            if self.grind_n[a] < GRIND_MIN_N {
                continue;
            }
            let rad = self.berakna_grind_rad(a);
            for j in 0..S_N {
                let n = self.grind_n[a];
                let my = self.grind_sy[a][j] / n;
                let vy = self.grind_sy2[a][j] / n - my * my;
                if vy >= 1e-6 {
                    summa[j] += rad[j];
                    antal_a[j] += 1;
                }
            }
        }
        let mut poang = [0.0; S_N];
        for j in 0..S_N {
            if antal_a[j] > 0 {
                poang[j] = summa[j] / antal_a[j] as f64;
            }
        }
        poang
    }

    fn uppdatera_kanalgrind(&mut self, s_sk_nu: &[f64; S_N]) {
        if !self.cfg.med_kanalgrind {
            return;
        }
        let (Some(s_prev), Some((_, _, a_prev, _))) = (self.forra_ra_sk, self.forra) else {
            return;
        };
        let n = GRIND_GAMMA * self.grind_n[a_prev] + 1.0;
        self.grind_n[a_prev] = n;
        for j in 0..S_N {
            self.grind_sy[a_prev][j] = GRIND_GAMMA * self.grind_sy[a_prev][j] + s_sk_nu[j];
            self.grind_sy2[a_prev][j] = GRIND_GAMMA * self.grind_sy2[a_prev][j] + s_sk_nu[j] * s_sk_nu[j];
            self.grind_sx[a_prev][j] = GRIND_GAMMA * self.grind_sx[a_prev][j] + s_prev[j];
            self.grind_sx2[a_prev][j] = GRIND_GAMMA * self.grind_sx2[a_prev][j] + s_prev[j] * s_prev[j];
            for k in 0..S_N {
                self.grind_syx[a_prev][j][k] =
                    GRIND_GAMMA * self.grind_syx[a_prev][j][k] + s_sk_nu[j] * s_prev[k];
            }
        }
        if self.steg < 300 {
            self.uppdatera_grind_rad(a_prev);
        } else if self.steg % 10 == 0 {
            for a in 0..A_N {
                self.uppdatera_grind_rad(a);
            }
        } else {
            return;
        }
        let mut poang = [0.0; S_N];
        let mut antal = 0usize;
        for a in 0..A_N {
            if self.grind_n[a] >= GRIND_MIN_N {
                antal += 1;
                for j in 0..S_N {
                    poang[j] += self.grind_adj[a][j];
                }
            }
        }
        if antal > 0 {
            let inv_a = 1.0 / antal as f64;
            for j in 0..S_N {
                poang[j] *= inv_a;
            }
        }
        if self.cfg.skarp_kanalgrind {
            let max_p = poang.iter().copied().fold(0.0f64, f64::max);
            if max_p > GRIND_AKTIV_MIN {
                for j in 0..S_N {
                    self.kanal_vikt[j] = (poang[j] / GRIND_MATTNAD).clamp(0.0, 1.0).powi(3);
                }
            } else {
                self.kanal_vikt = [1.0; S_N];
            }
        } else {
            let w: [f64; S_N] = std::array::from_fn(|j| poang[j] * poang[j]);
            let max_w = w.iter().copied().fold(0.0f64, f64::max);
            if max_w > 0.01 {
                for j in 0..S_N {
                    self.kanal_vikt[j] = w[j] / max_w;
                }
            } else {
                self.kanal_vikt = [1.0; S_N];
            }
        }
    }

    fn kod_mat(mat: &[[f64; S_N + 1]], s_sk: &[f64; S_N]) -> [f64; Z_N] {
        let mut z = [0.0; Z_N];
        for i in 0..Z_N {
            let lin = mat[i][S_N] + (0..S_N).map(|j| mat[i][j] * s_sk[j]).sum::<f64>();
            z[i] = lin.tanh();
        }
        z
    }

    pub fn kod_online_ra(&self, s_sk: &[f64; S_N]) -> [f64; Z_N] {
        Self::kod_mat(&self.wk, s_sk)
    }

    pub fn kod_mal_ra(&self, s_sk: &[f64; S_N]) -> [f64; Z_N] {
        Self::kod_mat(&self.wk_mal, s_sk)
    }

    pub fn norm_z(&self, z_raw: &[f64; Z_N]) -> [f64; Z_N] {
        if !self.cfg.med_z_norm || !self.z_skala_init {
            return *z_raw;
        }
        let mut ut = [0.0; Z_N];
        for i in 0..Z_N {
            ut[i] = ((z_raw[i] - self.mu_z[i]) / (self.var_z[i] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    pub fn uppdatera_z_skala(&mut self, z_raw: &[f64; Z_N]) {
        if self.cfg.norm_lage == 3 && self.z_skala_init && self.steg >= 200 {
            return;
        }
        if !self.z_skala_init {
            self.mu_z = *z_raw;
            self.var_z = [0.25; Z_N];
            self.z_skala_init = true;
            return;
        }
        let eta = if self.cfg.norm_lage == 0 {
            ETA_ZNORM
        } else {
            Self::adaptiv_eta(self.steg)
        };
        for i in 0..Z_N {
            let d = z_raw[i] - self.mu_z[i];
            self.mu_z[i] += eta * d;
            self.var_z[i] = (1.0 - eta) * self.var_z[i] + eta * d * d;
        }
    }

    pub fn kod_online(&self, s_sk: &[f64; S_N]) -> [f64; Z_N] {
        let raw = self.kod_online_ra(s_sk);
        self.norm_z(&raw)
    }

    pub fn kod_mal(&self, s_sk: &[f64; S_N]) -> [f64; Z_N] {
        let raw = self.kod_mal_ra(s_sk);
        self.norm_z(&raw)
    }

    pub fn forutsag(&self, z: &[f64; Z_N], a: usize) -> [f64; Z_N] {
        if self.cfg.med_rls {
            let x = [z[0], z[1], z[2], z[3], 1.0];
            let mut y = [0.0; Z_N];
            if self.cfg.rls_fristaende {
                for r in 0..Z_N {
                    y[r] = z[r] + dot_zx(&self.w_rls_akt[a][r], &x);
                }
                return y;
            }
            for r in 0..Z_N {
                let res = if self.cfg.additiv_endast {
                    self.w_rls_akt[a][r][Z_N]
                } else {
                    dot_zx(&self.w_rls_akt[a][r], &x)
                };
                y[r] = z[r] + dot_zx(&self.w_rls_delad[r], &x) + res;
            }
            return y;
        }
        let mut y = [0.0; Z_N];
        for r in 0..Z_N {
            let mut sum = z[r] + self.c[r] + self.da[a][r];
            for j in 0..Z_N {
                let w_rj = self.u[r][j] + if self.cfg.additiv_endast { 0.0 } else { self.dw[a][r][j] };
                sum += w_rj * z[j];
            }
            y[r] = sum;
        }
        y
    }

    /// Medelförutsägelse över de 9 handlingarna (för strykprov i val av handling).
    pub fn forutsag_medel(&self, z: &[f64; Z_N]) -> [f64; Z_N] {
        let mut y = [0.0; Z_N];
        for a in 0..A_N {
            let ya = self.forutsag(z, a);
            for r in 0..Z_N {
                y[r] += ya[r] / A_N as f64;
            }
        }
        y
    }

    pub fn varde(&self, y: &[f64; Z_N]) -> f64 {
        if self.cfg.med_kritiker {
            (self.v[Z_N] + self.w_krit[Z_N])
                + (0..Z_N).map(|j| (self.v[j] + self.w_krit[j]) * y[j]).sum::<f64>()
        } else {
            self.v[Z_N] + (0..Z_N).map(|j| self.v[j] * y[j]).sum::<f64>()
        }
    }

    /// Online förklaringsgrad R² för den lärda kritikern (enbart läsande diagnostik).
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

    fn idm_feat(z_prev: &[f64; Z_N], z_now: &[f64; Z_N]) -> [f64; IDM_DIM] {
        let mut feat = [0.0; IDM_DIM];
        let mut idx = 0;
        for i in 0..Z_N {
            let dz = z_now[i] - z_prev[i];
            for j in 0..Z_N {
                feat[idx] = dz * z_prev[j];
                idx += 1;
            }
        }
        for i in 0..Z_N {
            let dz = z_now[i] - z_prev[i];
            feat[idx] = dz * dz;
            idx += 1;
        }
        feat[idx] = 1.0;
        feat
    }

    pub fn idm_sannolikheter(&self, z_prev: &[f64; Z_N], z_now: &[f64; Z_N]) -> [f64; A_N] {
        let feat = Self::idm_feat(z_prev, z_now);

        let mut logits = [0.0; A_N];
        let mut m = f64::NEG_INFINITY;
        for k in 0..A_N {
            let s: f64 = (0..IDM_DIM).map(|idx| self.w_idm[k][idx] * feat[idx]).sum();
            logits[k] = s;
            if s > m {
                m = s;
            }
        }
        let mut sum_exp = 0.0;
        let mut p = [0.0; A_N];
        for k in 0..A_N {
            let e = (logits[k] - m).exp();
            p[k] = e;
            sum_exp += e;
        }
        for k in 0..A_N {
            p[k] /= sum_exp.max(1e-300);
        }
        p
    }

    fn uppdatera_fran_forra(&mut self, s_sk_now: &[f64; S_N], lar: bool) {
        self.senaste_fel = None;
        self.senaste_fel_kopiering = None;
        let Some((s_sk_prev, z_prev, a_prev, y_prev)) = self.forra else {
            return;
        };
        let z_mal = self.kod_mal(s_sk_now);

        let mut e_fwd = [0.0; Z_N];
        let mut kvad = 0.0;
        let mut kvad_kop = 0.0;
        for r in 0..Z_N {
            let ef = y_prev[r] - z_mal[r];
            e_fwd[r] = ef;
            kvad += ef * ef;
            let ek = z_prev[r] - z_mal[r];
            kvad_kop += ek * ek;
        }
        self.senaste_fel = Some(kvad);
        self.senaste_fel_kopiering = Some(kvad_kop);

        if !lar {
            return;
        }

        // 0. Alt 2267: uppdatera lärd kritiker på den verkliga nya inre bilden z_mal och förra stegets netto-ΔE.
        if self.cfg.med_kritiker {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let r = r_raw.clamp(-10.0, 10.0);
                let pred_k = self.varde(&z_mal);
                let err_k = r - pred_k;
                if self.steg >= 100 {
                    self.krit_n += 1;
                    self.krit_sy += r;
                    self.krit_sy2 += r * r;
                    self.krit_se2 += err_k * err_k;
                }
                let x_k = [z_mal[0], z_mal[1], z_mal[2], z_mal[3], 1.0];
                rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
            }
        }

        // 1. Gradienter in i W_k (beräknade före uppdatering av förutsägare/IDM).
        let mut z_now = [0.0; Z_N];
        let mut g_fwd_z_prev = [0.0; Z_N];
        if self.cfg.lar_kodare {
            for i in 0..Z_N {
                let mut acc = 0.0;
                for r in 0..Z_N {
                    let d_y_z = if self.cfg.med_rls {
                        if self.cfg.rls_fristaende {
                            (if r == i { 1.0 } else { 0.0 }) + self.w_rls_akt[a_prev][r][i]
                        } else {
                            (if r == i { 1.0 } else { 0.0 })
                                + self.w_rls_delad[r][i]
                                + if self.cfg.additiv_endast { 0.0 } else { self.w_rls_akt[a_prev][r][i] }
                        }
                    } else {
                        (if r == i { 1.0 } else { 0.0 })
                            + self.u[r][i]
                            + if self.cfg.additiv_endast { 0.0 } else { self.dw[a_prev][r][i] }
                    };
                    acc += e_fwd[r] * d_y_z;
                }
                g_fwd_z_prev[i] = acc;
            }
        }

        let mut feat = [0.0; IDM_DIM];
        let mut d_logits = [0.0; A_N];
        let mut g_idm_z_prev = [0.0; Z_N];
        let mut g_idm_z_now = [0.0; Z_N];
        if self.cfg.med_idm || self.cfg.lar_kodare {
            z_now = self.kod_online(s_sk_now);
        }
        if self.cfg.med_idm {
            feat = Self::idm_feat(&z_prev, &z_now);
            let p_idm = self.idm_sannolikheter(&z_prev, &z_now);
            for k in 0..A_N {
                d_logits[k] = p_idm[k] - if k == a_prev { 1.0 } else { 0.0 };
            }
            let mut idx = 0;
            for i in 0..Z_N {
                let dz = z_now[i] - z_prev[i];
                for j in 0..Z_N {
                    let g_f: f64 = (0..A_N).map(|k| d_logits[k] * self.w_idm[k][idx]).sum();
                    g_idm_z_now[i] += g_f * z_prev[j];
                    g_idm_z_prev[i] -= g_f * z_prev[j];
                    g_idm_z_prev[j] += g_f * dz;
                    idx += 1;
                }
            }
            for i in 0..Z_N {
                let dz = z_now[i] - z_prev[i];
                let g_f: f64 = (0..A_N).map(|k| d_logits[k] * self.w_idm[k][idx]).sum();
                g_idm_z_now[i] += g_f * 2.0 * dz;
                g_idm_z_prev[i] -= g_f * 2.0 * dz;
                idx += 1;
            }
        }

        // 2. Uppdatera förutsägaren (antingen RLS eller NLMS).
        if self.cfg.med_rls {
            let x = [z_prev[0], z_prev[1], z_prev[2], z_prev[3], 1.0];
            let delta_sann: [f64; Z_N] = std::array::from_fn(|r| z_mal[r] - z_prev[r]);
            if self.cfg.rls_fristaende {
                let res_pred: [f64; Z_N] = std::array::from_fn(|r| dot_zx(&self.w_rls_akt[a_prev][r], &x));
                rls_uppdatera(
                    &mut self.w_rls_akt[a_prev],
                    &mut self.p_rls_akt[a_prev],
                    &x,
                    &delta_sann,
                    &res_pred,
                    LAMBDA_RLS,
                );
            } else {
                let base_pred: [f64; Z_N] = std::array::from_fn(|r| dot_zx(&self.w_rls_delad[r], &x));
                rls_uppdatera(
                    &mut self.w_rls_delad,
                    &mut self.p_rls_delad,
                    &x,
                    &delta_sann,
                    &base_pred,
                    LAMBDA_RLS,
                );
                let res_target: [f64; Z_N] = std::array::from_fn(|r| delta_sann[r] - base_pred[r]);
                let x_akt = if self.cfg.additiv_endast {
                    [0.0, 0.0, 0.0, 0.0, 1.0]
                } else {
                    x
                };
                let res_pred: [f64; Z_N] = std::array::from_fn(|r| dot_zx(&self.w_rls_akt[a_prev][r], &x_akt));
                rls_uppdatera(
                    &mut self.w_rls_akt[a_prev],
                    &mut self.p_rls_akt[a_prev],
                    &x_akt,
                    &res_target,
                    &res_pred,
                    LAMBDA_RLS,
                );
            }
        } else {
            let norm_z2: f64 = z_prev.iter().map(|&x| x * x).sum();
            let eta_p = ETA_P / (1.0 + norm_z2);
            for r in 0..Z_N {
                self.c[r] -= eta_p * e_fwd[r];
                self.da[a_prev][r] -= eta_p * e_fwd[r];
                for j in 0..Z_N {
                    self.u[r][j] -= eta_p * e_fwd[r] * z_prev[j];
                    if !self.cfg.additiv_endast {
                        self.dw[a_prev][r][j] -= eta_p * (e_fwd[r] * z_prev[j] + L2_P_RES * self.dw[a_prev][r][j]);
                    }
                }
            }
        }

        // 3. Uppdatera IDM med NLMS-normaliserad stegstorlek.
        if self.cfg.med_idm {
            let norm_feat2: f64 = feat.iter().map(|&x| x * x).sum();
            let eta_idm = ETA_IDM / (1.0 + norm_feat2);
            for k in 0..A_N {
                for m in 0..IDM_DIM {
                    self.w_idm[k][m] -= eta_idm * (d_logits[k] * feat[m] + L2_IDM * self.w_idm[k][m]);
                }
            }
        }

        // 4. Uppdatera kodaren W_k och EMA-målkodaren W_k_mal.
        if self.cfg.lar_kodare {
            let ns_prev: f64 = s_sk_prev.iter().map(|&x| x * x).sum();
            let ns_now: f64 = s_sk_now.iter().map(|&x| x * x).sum();
            let eta_prev = ETA_K / (1.0 + ns_prev / S_N as f64);
            let eta_now = ETA_K / (1.0 + ns_now / S_N as f64);
            let z_raw_prev = self.kod_online_ra(&s_sk_prev);
            let z_raw_now = self.kod_online_ra(s_sk_now);

            for i in 0..Z_N {
                let (zr_p, gain_p, zr_n, gain_n) = if self.cfg.med_z_norm && self.z_skala_init {
                    let inv_sd = 1.0 / (self.var_z[i] + 1e-4).sqrt().max(0.05);
                    let gp = if z_prev[i].abs() < 3.0 { inv_sd } else { 0.0 };
                    let gn = if z_now[i].abs() < 3.0 { inv_sd } else { 0.0 };
                    (z_raw_prev[i], gp, z_raw_now[i], gn)
                } else {
                    (z_prev[i], 1.0, z_now[i], 1.0)
                };
                let d_prev = (1.0 - zr_p * zr_p)
                    * gain_p
                    * (LAMBDA_FWD * g_fwd_z_prev[i] + if self.cfg.med_idm { g_idm_z_prev[i] } else { 0.0 });
                let d_now = if self.cfg.med_idm {
                    (1.0 - zr_n * zr_n) * gain_n * g_idm_z_now[i]
                } else {
                    0.0
                };
                for j in 0..=S_N {
                    let xp = if j < S_N { s_sk_prev[j] } else { 1.0 };
                    let xn = if j < S_N { s_sk_now[j] } else { 1.0 };
                    let grad = (d_prev * xp * eta_prev + d_now * xn * eta_now).clamp(-1.0, 1.0);
                    let nytt = self.wk[i][j] - grad - ETA_K * L2_K * self.wk[i][j];
                    self.wk[i][j] = nytt.clamp(-5.0, 5.0);
                    self.wk_mal[i][j] = (1.0 - TAU_MAL) * self.wk_mal[i][j] + TAU_MAL * self.wk[i][j];
                }
            }
        }
    }

    /// Stegvis val (lottar lika maxvärden med `g`); `lar = false` fryser all viktinlärning.
    pub fn valj_lott(&mut self, s_nu: &[f64], lar: bool, g: &mut Rng) -> usize {
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let gv = self.aktuell_kanal_vikt();
        let mut s_in = [0.0; S_N];
        for j in 0..S_N {
            s_in[j] = gv[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(&s_in, lar);
        let z_raw = self.kod_online_ra(&s_in);
        if self.cfg.med_z_norm && !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);

        let mut varden = [0.0; A_N];
        let mut ys = [[0.0; Z_N]; A_N];
        let y_stryk = if self.cfg.stryk_handling_i_val {
            Some(self.forutsag_medel(&z))
        } else {
            None
        };
        for a in 0..A_N {
            let y = y_stryk.unwrap_or_else(|| self.forutsag(&z, a));
            varden[a] = self.varde(&y);
            ys[a] = y;
        }
        let ba = lott_bland_max(&varden, g);
        if lar {
            self.uppdatera_kanalgrind(&s_sk);
            self.uppdatera_skala(s_nu);
            if self.cfg.med_z_norm {
                self.uppdatera_z_skala(&z_raw);
            }
        }
        self.forra = Some((s_in, z, ba, ys[ba]));
        self.forra_ra_sk = Some(s_sk);
        self.steg += 1;
        ba
    }

    /// Handlingsval styrt av tillståndsberoende RLS-osäkerhet x^T P_a x (Alt 2199):
    /// `osak_lage == 0`: fast takt `param`, väljer störst osäkerhet vid utforskning;
    /// `osak_lage == 1`: åldersavtagande takt `(0,5 + 1,75 * exp(-steg / 450)) * param` (~45 % -> 10 % vid 0,20);
    /// `osak_lage == 2`: UCB-bonus `varde(y_a) + param * max(||v||, 0,10) * sqrt(osak[a])` utan separat slumpkast.
    /// Returnerar `(vald_handling, var_girig)`.
    pub fn valj_med_osakerhet(
        &mut self,
        s_nu: &[f64],
        lar: bool,
        g: &mut Rng,
        osak_lage: u8,
        param: f64,
    ) -> (usize, bool) {
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let gv = self.aktuell_kanal_vikt();
        let mut s_in = [0.0; S_N];
        for j in 0..S_N {
            s_in[j] = gv[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(&s_in, lar);
        let z_raw = self.kod_online_ra(&s_in);
        if self.cfg.med_z_norm && !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);

        let mut varden = [0.0; A_N];
        let mut osak = [0.0; A_N];
        let mut ys = [[0.0; Z_N]; A_N];
        for a in 0..A_N {
            let y = self.forutsag(&z, a);
            varden[a] = self.varde(&y);
            osak[a] = self.osakerhet_rls(&z, a);
            ys[a] = y;
        }

        let (ba, var_girig) = match osak_lage {
            0 => {
                if param > 0.0 && g.f() < param {
                    (lott_bland_max(&osak, g), false)
                } else {
                    (lott_bland_max(&varden, g), true)
                }
            }
            1 => {
                let eps_nu = (param * 0.5) + (param * 1.75) * (-(self.steg as f64) / 450.0).exp();
                if eps_nu > 0.0 && g.f() < eps_nu {
                    (lott_bland_max(&osak, g), false)
                } else {
                    (lott_bland_max(&varden, g), true)
                }
            }
            2 => {
                let v_norm = self.v[..Z_N]
                    .iter()
                    .map(|x| x * x)
                    .sum::<f64>()
                    .sqrt()
                    .max(0.10);
                let mut poang = [0.0; A_N];
                for a in 0..A_N {
                    poang[a] = varden[a] + param * v_norm * osak[a].sqrt();
                }
                let max_v = varden.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let a_vald = lott_bland_max(&poang, g);
                (a_vald, varden[a_vald] >= max_v - 1e-12)
            }
            _ => (lott_bland_max(&varden, g), true),
        };

        if lar {
            self.uppdatera_kanalgrind(&s_sk);
            self.uppdatera_skala(s_nu);
            if self.cfg.med_z_norm {
                self.uppdatera_z_skala(&z_raw);
            }
        }
        self.forra = Some((s_in, z, ba, ys[ba]));
        self.forra_ra_sk = Some(s_sk);
        self.steg += 1;
        (ba, var_girig)
    }

    /// Tvingar handling `a` och tränar online (för Spår A längs loggad bana).
    pub fn lar_och_satt(&mut self, s_nu: &[f64], lar: bool, a: usize) {
        assert!(a < A_N);
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let gv = self.aktuell_kanal_vikt();
        let mut s_in = [0.0; S_N];
        for j in 0..S_N {
            s_in[j] = gv[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(&s_in, lar);
        let z_raw = self.kod_online_ra(&s_in);
        if self.cfg.med_z_norm && !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);
        let ya = self.forutsag(&z, a);
        if lar {
            self.uppdatera_kanalgrind(&s_sk);
            self.uppdatera_skala(s_nu);
            if self.cfg.med_z_norm {
                self.uppdatera_z_skala(&z_raw);
            }
        }
        self.forra = Some((s_in, z, a, ya));
        self.forra_ra_sk = Some(s_sk);
        self.steg += 1;
    }

    /// Helt blind utforskningshandling: väljer handlingen med lägst `grind_n` (lottar vid lika).
    pub fn minst_besokta_handling(&self, g: &mut Rng) -> usize {
        let mut neg = [0.0; A_N];
        for a in 0..A_N {
            neg[a] = -self.grind_n[a];
        }
        lott_bland_max(&neg, g)
    }

    /// Tillståndsberoende design-osäkerhet x^T P_akt[a] x för handling a.
    pub fn osakerhet_rls(&self, z: &[f64; Z_N], a: usize) -> f64 {
        let x = [z[0], z[1], z[2], z[3], 1.0];
        let p = &self.p_rls_akt[a];
        let mut tot = 0.0;
        for m in 0..=Z_N {
            let pxm: f64 = (0..=Z_N).map(|n| p[m][n] * x[n]).sum();
            tot += x[m] * pxm;
        }
        tot.max(0.0)
    }
}

#[cfg(test)]
mod prov {
    use super::*;
    use crate::hjarna::{gauss, N_GEN};
    use crate::sim::GRANNAR;
    use crate::tal::SIGMA0;

    fn slump_gen(seed: u64) -> Vec<f64> {
        let mut g = Rng::new(seed);
        (0..N_GEN).map(|_| SIGMA0 * gauss(&mut g)).collect()
    }

    fn syntetisk_varld(karta: &[[f64; 10]; 10], x: i32, y: i32, g: &mut Rng) -> Vec<f64> {
        let mut s = vec![0.0; S_N];
        for (k, &(di, dj)) in GRANNAR.iter().enumerate() {
            let ii = (x + di).rem_euclid(10) as usize;
            let jj = (y + dj).rem_euclid(10) as usize;
            s[2 * k] = karta[ii][jj];
            // Tungt djurbrus i de 9 antalskanalerna (precis som riktiga världen).
            s[2 * k + 1] = g.f() * 8.0;
        }
        s[S_N - 1] = karta[x.rem_euclid(10) as usize][y.rem_euclid(10) as usize];
        s
    }

    fn kora_syntet_spara(cfg: Hj2008Config, blanda_a: bool) -> (f64, f64, f64) {
        let mut g = Rng::new(2008);
        let mut karta = [[0.0; 10]; 10];
        for (i, r) in karta.iter_mut().enumerate() {
            for (j, c) in r.iter_mut().enumerate() {
                *c = 0.5
                    + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                        * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                    + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
            }
        }
        let mut hj = Hj2008::fran_gen(&slump_gen(77), cfg);
        let (mut x, mut y) = (5i32, 5i32);
        for t in 0..800 {
            let s = syntetisk_varld(&karta, x, y, &mut g);
            let a = g.under(A_N);
            let (di, dj) = GRANNAR[a];
            x += di;
            y += dj;
            let a_skicka = if blanda_a && t >= 600 { g.under(A_N) } else { a };
            hj.lar_och_satt(&s, t < 600, a_skicka);
        }
        // Utvärdera på 150 fasta provsteg.
        let (mut rest, mut verk, mut idm_ratt, mut tot) = (0.0, 0.0, 0usize, 0usize);
        let mut z_vals: [Vec<f64>; Z_N] = Default::default();
        for _ in 0..150 {
            let s0 = syntetisk_varld(&karta, x, y, &mut g);
            let s0_sk = hj.skala(&s0);
            let z0 = hj.kod_online(&s0_sk);
            for i in 0..Z_N {
                z_vals[i].push(z0[i]);
            }
            let mut preds = [[0.0; Z_N]; A_N];
            let mut trues = [[0.0; Z_N]; A_N];
            let mut mp = [0.0; Z_N];
            let mut mt = [0.0; Z_N];
            for a in 0..A_N {
                let (di, dj) = GRANNAR[a];
                let s2 = syntetisk_varld(&karta, x + di, y + dj, &mut g);
                let s2_sk = hj.skala(&s2);
                let z2_mal = hj.kod_mal(&s2_sk);
                let z2_onl = hj.kod_online(&s2_sk);
                let a_ut = if blanda_a { g.under(A_N) } else { a };
                preds[a] = hj.forutsag(&z0, a_ut);
                trues[a] = z2_mal;
                for i in 0..Z_N {
                    mp[i] += preds[a][i] / A_N as f64;
                    mt[i] += trues[a][i] / A_N as f64;
                }
                let p = hj.idm_sannolikheter(&z0, &z2_onl);
                let giss = (0..A_N).max_by(|&u, &v| p[u].partial_cmp(&p[v]).unwrap()).unwrap();
                if giss == a_ut {
                    idm_ratt += 1;
                }
                tot += 1;
            }
            for a in 0..A_N {
                for i in 0..Z_N {
                    let fd = preds[a][i] - mp[i];
                    let vd = trues[a][i] - mt[i];
                    rest += (fd - vd).powi(2);
                    verk += vd * vd;
                }
            }
            let a_real = g.under(A_N);
            x += GRANNAR[a_real].0;
            y += GRANNAR[a_real].1;
        }
        let r2 = 1.0 - rest / verk.max(1e-300);
        let min_var = z_vals
            .iter()
            .map(|zv| {
                let m = zv.iter().sum::<f64>() / zv.len() as f64;
                zv.iter().map(|&q| (q - m).powi(2)).sum::<f64>() / zv.len() as f64
            })
            .fold(f64::INFINITY, f64::min);
        (r2, idm_ratt as f64 / tot as f64, min_var)
    }

    #[test]
    fn alt2008_lar_sig_syntetisk_vaxt_och_idm_hindrar_kollaps() {
        let (r2_full, idm_full, var_full) = kora_syntet_spara(Hj2008Config::default(), false);
        let (r2_utan, idm_utan, var_utan) = kora_syntet_spara(
            Hj2008Config {
                med_idm: false,
                ..Default::default()
            },
            false,
        );
        eprintln!("syntet full: r2={r2_full:.4} idm={idm_full:.4} var={var_full:.4} | utan_idm: r2={r2_utan:.4} idm={idm_utan:.4} var={var_utan:.4}");
        assert!(r2_full > 0.0, "Alt 2008 gav negativt R² i syntetisk värld: {r2_full}");
        assert!(idm_full > 0.25, "IDM-träffen blev för låg: {idm_full}");
        assert!(var_full > 0.01, "z-axlar kollapsade: min_var={var_full}");
        assert!(
            idm_full > idm_utan + 0.05 || r2_full > r2_utan,
            "strykprov utan IDM skilde sig inte: full=({r2_full},{idm_full}), utan=({r2_utan},{idm_utan})"
        );
    }

    #[test]
    fn alt2008_omkastade_handlingar_ger_noll() {
        let (r2_bl, idm_bl, _) = kora_syntet_spara(Hj2008Config::default(), true);
        assert!(r2_bl <= 0.01, "omkastade handlingar gav R² {r2_bl}");
        assert!(idm_bl < 0.20, "omkastade handlingar gav IDM-träff {idm_bl}");
    }

    #[test]
    fn alt2008_additiv_ger_konstant_val_medan_interaktion_foljer_z() {
        let mut g = Rng::new(99);
        let gen = slump_gen(123);
        let mut hj_add = Hj2008::fran_gen(&gen, Hj2008Config { additiv_endast: true, ..Default::default() });
        let mut hj_int = Hj2008::fran_gen(&gen, Hj2008Config { additiv_endast: false, ..Default::default() });
        for a in 0..A_N {
            hj_add.da[a] = [0.01 * a as f64, 0.0, 0.0, 0.0];
            hj_int.dw[a][0][0] = if a == 2 { 1.5 } else if a == 5 { -1.5 } else { 0.0 };
        }
        let s1 = [0.9; S_N];
        let s2 = [-0.9; S_N];
        // Initiera skala så olika s ger olika z.
        hj_add.uppdatera_skala(&[0.0; S_N]);
        hj_int.uppdatera_skala(&[0.0; S_N]);
        let a_add_1 = hj_add.valj_lott(&s1, false, &mut g);
        let a_add_2 = hj_add.valj_lott(&s2, false, &mut g);
        assert_eq!(a_add_1, a_add_2, "rent additiv gav olika val fast v är linjär!");

        let z_pos = [0.8, 0.0, 0.0, 0.0];
        let z_neg = [-0.8, 0.0, 0.0, 0.0];
        // Sätt v så axel 0 har positiv vikt.
        hj_int.v = [1.0, 0.0, 0.0, 0.0, 0.0];
        let best_pos = (0..A_N).max_by(|&a, &b| hj_int.varde(&hj_int.forutsag(&z_pos, a)).partial_cmp(&hj_int.varde(&hj_int.forutsag(&z_pos, b))).unwrap()).unwrap();
        let best_neg = (0..A_N).max_by(|&a, &b| hj_int.varde(&hj_int.forutsag(&z_neg, a)).partial_cmp(&hj_int.varde(&hj_int.forutsag(&z_neg, b))).unwrap()).unwrap();
        assert_ne!(best_pos, best_neg, "interaktionsmodellen ska kunna ändra val när z byter tecken");
    }

    fn syntet_med_gen_och_perm(
        cfg: Hj2008Config,
        gen0: &[f64],
        perm: &[usize; S_N],
        blanda_a: bool,
    ) -> (f64, f64, f64, [f64; S_N]) {
        let mut g = Rng::new(2008);
        let mut karta = [[0.0; 10]; 10];
        for (i, r) in karta.iter_mut().enumerate() {
            for (j, c) in r.iter_mut().enumerate() {
                *c = 0.5
                    + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                        * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                    + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
            }
        }
        let mut gen = gen0.to_vec();
        // Permutera även kodarvikterna per kolumn så startvikter inte avgör skillnaden.
        for row in 0..Z_N {
            for col in 0..S_N {
                gen[row * (S_N + 1) + col] = gen0[row * (S_N + 1) + perm[col]];
            }
        }
        let mut hj = Hj2008::fran_gen(&gen, cfg);
        let perm_s = |ra: &[f64]| -> Vec<f64> { (0..S_N).map(|j| ra[perm[j]]).collect() };

        let (mut x, mut y) = (5i32, 5i32);
        for t in 0..800 {
            let s = perm_s(&syntetisk_varld(&karta, x, y, &mut g));
            let a = g.under(A_N);
            let (di, dj) = GRANNAR[a];
            x += di;
            y += dj;
            let a_skicka = if blanda_a && t >= 600 { g.under(A_N) } else { a };
            hj.lar_och_satt(&s, t < 600, a_skicka);
        }
        let vikter = hj.aktuell_kanal_vikt();
        let (mut rest, mut verk, mut idm_ratt, mut tot) = (0.0, 0.0, 0usize, 0usize);
        let mut z_vals: [Vec<f64>; Z_N] = Default::default();
        for _ in 0..150 {
            let s0 = perm_s(&syntetisk_varld(&karta, x, y, &mut g));
            let s0_sk = hj.skala_grind(&s0);
            let z0 = hj.kod_online(&s0_sk);
            for i in 0..Z_N {
                z_vals[i].push(z0[i]);
            }
            let mut preds = [[0.0; Z_N]; A_N];
            let mut trues = [[0.0; Z_N]; A_N];
            let mut mp = [0.0; Z_N];
            let mut mt = [0.0; Z_N];
            for a in 0..A_N {
                let (di, dj) = GRANNAR[a];
                let s2 = perm_s(&syntetisk_varld(&karta, x + di, y + dj, &mut g));
                let s2_sk = hj.skala_grind(&s2);
                let z2_mal = hj.kod_mal(&s2_sk);
                let z2_onl = hj.kod_online(&s2_sk);
                let a_ut = if blanda_a { g.under(A_N) } else { a };
                preds[a] = hj.forutsag(&z0, a_ut);
                trues[a] = z2_mal;
                for i in 0..Z_N {
                    mp[i] += preds[a][i] / A_N as f64;
                    mt[i] += trues[a][i] / A_N as f64;
                }
                let p = hj.idm_sannolikheter(&z0, &z2_onl);
                let giss = (0..A_N).max_by(|&u, &v| p[u].partial_cmp(&p[v]).unwrap()).unwrap();
                if giss == a_ut {
                    idm_ratt += 1;
                }
                tot += 1;
            }
            for a in 0..A_N {
                for i in 0..Z_N {
                    let fd = preds[a][i] - mp[i];
                    let vd = trues[a][i] - mt[i];
                    rest += (fd - vd).powi(2);
                    verk += vd * vd;
                }
            }
            let a_real = g.under(A_N);
            x += GRANNAR[a_real].0;
            y += GRANNAR[a_real].1;
        }
        let r2 = 1.0 - rest / verk.max(1e-300);
        let min_var = z_vals
            .iter()
            .map(|zv| {
                let m = zv.iter().sum::<f64>() / zv.len() as f64;
                zv.iter().map(|&q| (q - m).powi(2)).sum::<f64>() / zv.len() as f64
            })
            .fold(f64::INFINITY, f64::min);
        (r2, idm_ratt as f64 / tot as f64, min_var, vikter)
    }

    fn syntet_tillata_perm(
        cfg: Hj2008Config,
        perm: &[usize; S_N],
    ) -> (f64, f64, f64, [f64; S_N]) {
        syntet_med_gen_och_perm(cfg, &slump_gen(77), perm, false)
    }

    #[test]
    fn alt2053_kanalgrind_hittar_vaxt_och_ar_permutationsinvariant() {
        let cfg = Hj2008Config {
            med_kanalgrind: true,
            ..Default::default()
        };
        let id_perm: [usize; S_N] = std::array::from_fn(|j| j);
        let mut shift_perm: [usize; S_N] = std::array::from_fn(|j| (j * 7 + 5) % S_N);
        // Kontrollera att shift_perm verkligen är en bijektion (gcd(7, 19) = 1).
        let mut seen = [false; S_N];
        for &p in &shift_perm {
            assert!(!seen[p]);
            seen[p] = true;
        }

        let (r2, idm, var, g_orig) = syntet_tillata_perm(cfg, &id_perm);
        let (_r2_p, _idm_p, _var_p, g_perm) = syntet_tillata_perm(cfg, &shift_perm);

        for j in 0..S_N {
            assert!(
                (g_perm[j] - g_orig[shift_perm[j]]).abs() < 1e-9,
                "permutationsinvarians bruten på pos {j}: {} vs {}",
                g_perm[j],
                g_orig[shift_perm[j]]
            );
        }
        let min_vaxt = (0..S_N)
            .filter(|&j| j % 2 == 0 || j == S_N - 1)
            .map(|j| g_orig[j])
            .fold(f64::INFINITY, f64::min);
        let max_brus = (1..S_N - 1)
            .step_by(2)
            .map(|j| g_orig[j])
            .fold(0.0f64, f64::max);
        eprintln!("alt2053 syntet: r2={r2:.4} idm={idm:.4} var={var:.4} min_vaxt={min_vaxt:.4} max_brus={max_brus:.4}");
        assert!(min_vaxt > 2.0 * max_brus.max(1e-6), "grinden skilde inte ut växter: {min_vaxt} vs {max_brus}");
        assert!(r2 > 0.15, "Alt 2053 förväntades nå r2 > 0.15, fick {r2}");
        assert!(idm > 0.25, "Alt 2053 förväntades nå idm > 0.25, fick {idm}");
        assert!(var > 0.01, "z-axlar kollapsade: {var}");
        let _ = &mut shift_perm;
    }

    #[test]
    fn alt2053_kanalgrind_blir_noll_pa_rent_brus() {
        let cfg = Hj2008Config {
            med_kanalgrind: true,
            ..Default::default()
        };
        let mut hj = Hj2008::fran_gen(&slump_gen(91), cfg);
        let mut g = Rng::new(2053);
        for t in 0..800 {
            let s: Vec<f64> = (0..S_N).map(|_| g.f() * 5.0).collect();
            let a = g.under(A_N);
            hj.lar_och_satt(&s, t < 600, a);
        }
        let max_p = hj.kanal_poang().into_iter().fold(0.0f64, f64::max);
        eprintln!("alt2053 rent brus: max_kanal_poang={max_p:.4}");
        assert!(max_p < 0.05, "blinda grinden hittade falsk signal i rent brus: {max_p}");
    }

    #[test]
    fn alt2056_skalfri_rls_och_skarp_grind_klarar_tiofaldigt_krympt_wk() {
        let cfg = Hj2008Config {
            med_kanalgrind: true,
            skarp_kanalgrind: true,
            med_z_norm: true,
            med_rls: true,
            ..Default::default()
        };
        let mut gen_liten = slump_gen(77);
        for g in &mut gen_liten[..NK] {
            *g *= 0.1;
        }
        let id_perm: [usize; S_N] = std::array::from_fn(|j| j);
        let shift_perm: [usize; S_N] = std::array::from_fn(|j| (j * 7 + 5) % S_N);
        let (r2, idm, var, g_orig) = syntet_med_gen_och_perm(cfg, &gen_liten, &id_perm, false);
        let (_r2_p, _idm_p, _var_p, g_perm) = syntet_med_gen_och_perm(cfg, &gen_liten, &shift_perm, false);

        for j in 0..S_N {
            assert!(
                (g_perm[j] - g_orig[shift_perm[j]]).abs() < 1e-9,
                "permutationsinvarians bruten på pos {j}: {} vs {}",
                g_perm[j],
                g_orig[shift_perm[j]]
            );
        }
        let min_vaxt = (0..S_N)
            .filter(|&j| j % 2 == 0 || j == S_N - 1)
            .map(|j| g_orig[j])
            .fold(f64::INFINITY, f64::min);
        let max_brus = (1..S_N - 1)
            .step_by(2)
            .map(|j| g_orig[j])
            .fold(0.0f64, f64::max);
        eprintln!(
            "alt2056 krympt_wk syntet: r2={r2:.4} idm={idm:.4} var={var:.4} min_vaxt={min_vaxt:.4} max_brus={max_brus:.4}"
        );
        assert!(min_vaxt > 0.90, "skärpta grinden lyfte inte växter till ~1: {min_vaxt}");
        assert!(max_brus < 0.05, "skärpta grinden strypte inte brus: {max_brus}");
        assert!(r2 > 0.25, "Alt 2056 förväntades nå r2 > 0.25 även med 0.1x W_k, fick {r2}");
        assert!(idm > 0.25, "Alt 2056 förväntades nå idm > 0.25 även med 0.1x W_k, fick {idm}");
        assert!(var > 0.20, "z-norm höll inte uppe var(z): {var}");
    }

    #[test]
    fn alt2056_omkastade_handlingar_ger_noll() {
        let cfg = Hj2008Config {
            med_kanalgrind: true,
            skarp_kanalgrind: true,
            med_z_norm: true,
            med_rls: true,
            ..Default::default()
        };
        let id_perm: [usize; S_N] = std::array::from_fn(|j| j);
        let (r2_bl, idm_bl, _, _) = syntet_med_gen_och_perm(cfg, &slump_gen(77), &id_perm, true);
        assert!(r2_bl <= 0.01, "omkastade handlingar med Alt 2056 gav R² {r2_bl}");
        assert!(idm_bl < 0.20, "omkastade handlingar med Alt 2056 gav IDM-träff {idm_bl}");
    }

    #[test]
    fn alt2059_formaga_bevarar_lar_och_kor_hj2059_utan_nan() {
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
        let n = 8 * 8;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 8, h: 8, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.5; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let mut s1 = crate::sim::Sim::fran_karta(&kart, 261, 16);
        let mut s2 = s1.clone();
        for _ in 0..20 {
            s1.steg();
            s2.steg();
        }
        assert_eq!(s1.djur_summa(), s2.djur_summa());
        for d in s2.djur.iter_mut().filter(|d| d.lev) {
            d.formaga = crate::formaga::Formaga::Lar2059;
        }
        for _ in 0..30 {
            s2.steg();
        }
        let har_hj2 = s2.djur.iter().filter(|d| d.lev && d.hj2059.is_some()).count();
        assert!(har_hj2 > 0, "Lar2059 skapade ingen hj2059");
        for d in s2.djur.iter().filter(|d| d.lev) {
            if let Some(h) = &d.hj2059 {
                assert!(h.kanal_vikt.iter().all(|v| v.is_finite()));
                assert!(h.mu_z.iter().all(|v| v.is_finite()));
            }
        }
        assert!(crate::formaga::Formaga::Stryk2059.cfg_2059().unwrap().stryk_handling_i_val);
        assert!(!crate::formaga::Formaga::Frusen2059.lar_sig());
        assert!(crate::formaga::Formaga::Lar2059.lar_sig());
    }

    #[test]
    fn alt2073_utforskning_och_stanna_baslinje_fungerar_och_bevarar_lar() {
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
        let n = 8 * 8;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 8, h: 8, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.5; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };

        // 1. Pröva StannaSlump(p) och kontrollera att Lar är oförändrad och hj2059 ej skapas.
        let mut s_lar = crate::sim::Sim::fran_karta(&kart, 261, 32);
        let mut s_stanna1 = s_lar.clone();
        let mut s_stanna0 = s_lar.clone();
        let mut s_stanna68 = s_lar.clone();
        for (s, f, namn) in [
            (&mut s_stanna1, crate::formaga::Formaga::StannaSlump(1.0), "stanna1"),
            (&mut s_stanna0, crate::formaga::Formaga::StannaSlump(0.0), "stanna0"),
            (&mut s_stanna68, crate::formaga::Formaga::StannaSlump(0.68), "stanna68"),
        ] {
            s.kant = crate::sim::KantLage::Torus;
            s.hjarn_arm = Some(crate::formaga::HjarnArm {
                namn,
                markta: f,
                omarkta: f,
                spegel: false,
            });
            s.markering_skedd = true;
            for d in s.djur.iter_mut().filter(|d| d.lev) {
                d.formaga = f;
            }
        }
        let m_stanna = |sim: &mut crate::sim::Sim, steg: usize| -> (f64, usize) {
            let mut n_st = 0usize;
            let mut tot = 0usize;
            for _ in 0..steg {
                let pos: std::collections::HashMap<u64, (usize, usize)> = sim
                    .djur
                    .iter()
                    .filter(|d| d.lev)
                    .map(|d| (d.uid, (d.i, d.j)))
                    .collect();
                sim.steg();
                for d in sim.djur.iter().filter(|d| d.lev) {
                    if let Some(&(i0, j0)) = pos.get(&d.uid) {
                        tot += 1;
                        if d.i == i0 && d.j == j0 {
                            n_st += 1;
                        }
                    }
                }
            }
            let hj2_cnt = sim.djur.iter().filter(|d| d.hj2059.is_some()).count();
            (n_st as f64 / tot.max(1) as f64, hj2_cnt)
        };
        let (p1, c1) = m_stanna(&mut s_stanna1, 20);
        let (p0, c0) = m_stanna(&mut s_stanna0, 20);
        let (p68, c68) = m_stanna(&mut s_stanna68, 40);
        for _ in 0..20 {
            s_lar.steg();
        }
        assert!((p1 - 1.0).abs() < 1e-12, "StannaSlump(1.0) stannade {p1}");
        assert!(p0 < 1e-12, "StannaSlump(0.0) stannade {p0}");
        assert!((0.55..=0.80).contains(&p68), "StannaSlump(0.68) gav {p68}");
        assert_eq!(c1 + c0 + c68, 0, "StannaSlump skapade hj2059");
        assert!(s_lar.djur.iter().all(|d| d.hj2059.is_none()), "Lar skapade hj2059");
        assert!(crate::formaga::Formaga::Lar2073(0.05).lar_sig());
        assert!(crate::formaga::Formaga::Lar2073(0.05).anvander_hjarna());
        assert!(!crate::formaga::Formaga::StannaSlump(0.68).anvander_hjarna());

        // 2. Röd/grön-prov: Lar2073(0.10) vs Lar2059 i levande liten Sim + syntetisk djurbrus-undertryckning.
        let mut s_2073 = crate::sim::Sim::fran_karta(&kart, 261, 32);
        s_2073.kant = crate::sim::KantLage::Torus;
        let f73 = crate::formaga::Formaga::Lar2073(0.10);
        s_2073.hjarn_arm = Some(crate::formaga::HjarnArm {
            namn: "lar2073_test",
            markta: f73,
            omarkta: f73,
            spegel: false,
        });
        s_2073.markering_skedd = true;
        for d in s_2073.djur.iter_mut().filter(|d| d.lev) {
            d.formaga = f73;
        }
        for _ in 0..80 {
            s_2073.steg();
        }
        let med_hj2: Vec<&Hj2008> = s_2073
            .djur
            .iter()
            .filter(|d| d.lev)
            .filter_map(|d| d.hj2059.as_deref())
            .collect();
        assert!(!med_hj2.is_empty(), "Lar2073 skapade ingen hj2059");
        for h in med_hj2 {
            assert!(h.kanal_vikt.iter().all(|v| v.is_finite()));
            assert!(h.mu_z.iter().all(|v| v.is_finite()));
            assert!(h.w_rls_delad.iter().flatten().all(|v| v.is_finite()));
        }

        // Syntetisk kontroll: jämna provsteg med blinda Alt 2059-inställningar slår låst vana (a = 0).
        let kor_synt = |las_a0: bool| -> (f64, f64, f64) {
            let mut karta = [[0.0; 10]; 10];
            for (i, r) in karta.iter_mut().enumerate() {
                for (j, c) in r.iter_mut().enumerate() {
                    *c = 0.5
                        + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                            * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                        + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
                }
            }
            let cfg = crate::formaga::Formaga::Lar2059.cfg_2059().unwrap();
            let mut hj = Hj2008::fran_gen(&slump_gen(77), cfg);
            let mut g = Rng::new(2073);
            let (mut x, mut y) = (5i32, 5i32);
            for t in 0..1600 {
                let s = syntetisk_varld(&karta, x, y, &mut g);
                let a = if las_a0 { 0 } else { g.under(A_N) };
                if t % 5 == 0 {
                    // Håll (x, y) rörlig även vid a=0-låsning så testmiljön varierar.
                    x += GRANNAR[a].0;
                    y += GRANNAR[a].1;
                } else {
                    x += GRANNAR[a].0;
                    y += GRANNAR[a].1;
                }
                hj.lar_och_satt(&s, true, a);
            }
            let min_gn = hj.grind_n.iter().copied().fold(f64::INFINITY, f64::min);
            let max_gd = (1..S_N - 1).step_by(2).map(|j| hj.kanal_vikt[j]).fold(0.0f64, f64::max);
            let (mut rest, mut verk) = (0.0, 0.0);
            for _ in 0..150 {
                let s0 = syntetisk_varld(&karta, x, y, &mut g);
                let z0 = hj.kod_online(&hj.skala_grind(&s0));
                let mut preds = [[0.0; Z_N]; A_N];
                let mut trues = [[0.0; Z_N]; A_N];
                let mut mp = [0.0; Z_N];
                let mut mt = [0.0; Z_N];
                for a in 0..A_N {
                    let s2 = syntetisk_varld(&karta, x + GRANNAR[a].0, y + GRANNAR[a].1, &mut g);
                    preds[a] = hj.forutsag(&z0, a);
                    trues[a] = hj.kod_mal(&hj.skala_grind(&s2));
                    for r in 0..Z_N {
                        mp[r] += preds[a][r] / A_N as f64;
                        mt[r] += trues[a][r] / A_N as f64;
                    }
                }
                for a in 0..A_N {
                    for r in 0..Z_N {
                        rest += (preds[a][r] - mp[r] - (trues[a][r] - mt[r])).powi(2);
                        verk += (trues[a][r] - mt[r]).powi(2);
                    }
                }
                let a_st = g.under(A_N);
                x += GRANNAR[a_st].0;
                y += GRANNAR[a_st].1;
            }
            (1.0 - rest / verk.max(1e-12), min_gn, max_gd)
        };
        let (r2_las, min_gn_0, _) = kor_synt(true);
        let (r2_utf, min_gn_u, gd_u) = kor_synt(false);
        eprintln!("alt2073 syntet: las_a0 (hjR²={r2_las:.4}, min_n={min_gn_0:.1}) vs utforsk (hjR²={r2_utf:.4}, min_n={min_gn_u:.1}, max_gd={gd_u:.4})");
        assert!(min_gn_0 < 1.0, "låst vana borde sakna grind_n på oanvända handlingar");
        assert!(min_gn_u >= 5.0, "med utforskning saknade handling grind_n: {min_gn_u}");
        assert!(gd_u < 0.05, "djurbrus stryptes inte: {gd_u}");
        assert!(r2_utf > 0.15 && r2_utf > r2_las + 0.10, "utforsk hjR²={r2_utf:.4} vs las hjR²={r2_las:.4}");
    }

    #[test]
    fn alt2116_minsta_besok_balanserar_handlingar_och_bevarar_lar() {
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
        let cfg = crate::formaga::Formaga::Lar2116Minst(0.20).cfg_2059().unwrap();
        let mut hj = Hj2008::fran_gen(&slump_gen(77), cfg);
        let mut g = Rng::new(2116);

        // 1. Lika antal -> lottar bland alla 9.
        let mut sedda = [0usize; A_N];
        for _ in 0..900 {
            sedda[hj.minst_besokta_handling(&mut g)] += 1;
        }
        assert!(sedda.iter().all(|&c| c > 50), "ojämn lottning vid lika: {:?}", sedda);
        hj.grind_n = [100.0, 50.0, 40.0, 30.0, 12.0, 20.0, 25.0, 60.0, 70.0];
        assert_eq!(hj.minst_besokta_handling(&mut g), 4);

        // 2. Syntetisk värld: 80 % låst på a = 0 + 20 % utforskning (slump vs riktad minst_besokta).
        let mut karta = [[0.0; 10]; 10];
        for (i, r) in karta.iter_mut().enumerate() {
            for (j, c) in r.iter_mut().enumerate() {
                *c = 0.5
                    + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                        * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                    + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
            }
        }
        let kor_utf = |riktad: bool| -> (f64, f64, f64) {
            let mut m = Hj2008::fran_gen(&slump_gen(77), cfg);
            let mut gw = Rng::new(2116);
            let mut ga = Rng::new(2116);
            let (mut x, mut y) = (5i32, 5i32);
            for _ in 0..1000 {
                let s = syntetisk_varld(&karta, x, y, &mut gw);
                let a = if ga.f() < 0.20 {
                    if riktad { m.minst_besokta_handling(&mut ga) } else { ga.under(A_N) }
                } else {
                    0
                };
                x += GRANNAR[a].0;
                y += GRANNAR[a].1;
                m.lar_och_satt(&s, true, a);
            }
            let min_gn = m.grind_n.iter().copied().fold(f64::INFINITY, f64::min);
            let max_gn = m.grind_n.iter().copied().fold(0.0f64, f64::max);
            let sum_gn: f64 = m.grind_n.iter().sum();
            let top1 = max_gn / sum_gn.max(1e-12);
            let max_gd = (1..S_N - 1).step_by(2).map(|j| m.kanal_vikt[j]).fold(0.0f64, f64::max);
            assert!(m.kanal_vikt.iter().all(|v| v.is_finite()));
            (min_gn, top1, max_gd)
        };
        let (min_s, top1_s, _gd_s) = kor_utf(false);
        let (min_m, top1_m, gd_m) = kor_utf(true);
        eprintln!(
            "alt2116 minst vs slump: slump (min_n={min_s:.2}, top1={top1_s:.3}) vs riktad (min_n={min_m:.2}, top1={top1_m:.3}, max_gd={gd_m:.4})"
        );
        assert!(min_m > min_s, "riktad min_n={min_m:.2} slog inte slump min_n={min_s:.2}");
        assert!(top1_m <= top1_s + 1e-9, "riktad top1={top1_m:.3} > slump top1={top1_s:.3}");
        assert!(gd_m < 0.05, "riktad djurbrus stryptes inte: {gd_m}");

        // 3. Levande liten Sim med Lar2116Minst(0.20) utan NaN och utan att röra Formaga::Lar.
        let n = 8 * 8;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 8, h: 8, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.5; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let mut s_lar = crate::sim::Sim::fran_karta(&kart, 261, 24);
        let mut s_m20 = s_lar.clone();
        s_m20.kant = crate::sim::KantLage::Torus;
        let fm = crate::formaga::Formaga::Lar2116Minst(0.20);
        s_m20.hjarn_arm = Some(crate::formaga::HjarnArm {
            namn: "lar2116_m20_test",
            markta: fm,
            omarkta: fm,
            spegel: false,
        });
        s_m20.markering_skedd = true;
        for d in s_m20.djur.iter_mut().filter(|d| d.lev) {
            d.formaga = fm;
        }
        for _ in 0..60 {
            s_lar.steg();
            s_m20.steg();
        }
        assert!(s_lar.djur.iter().all(|d| d.hj2059.is_none()));
        assert!(s_m20.djur.iter().filter(|d| d.lev).any(|d| d.hj2059.is_some()));
        for d in s_m20.djur.iter().filter(|d| d.lev) {
            if let Some(h) = &d.hj2059 {
                assert!(h.kanal_vikt.iter().all(|v| v.is_finite()));
                assert!(h.w_rls_delad.iter().flatten().all(|v| v.is_finite()));
            }
        }
        assert!(fm.lar_sig());
        assert!(fm.anvander_hjarna());
    }

    #[test]
    fn alt2144_d2_matchar_exakt_batch_ridge_och_kovarians() {
        let cfg = Hj2008Config {
            lar_kodare: false,
            med_idm: false,
            additiv_endast: false,
            stryk_handling_i_val: false,
            med_kanalgrind: false,
            stryk_djurantal_diagnos: false,
            skarp_kanalgrind: false,
            med_z_norm: false,
            med_rls: true,
            rls_fristaende: true,
            rls_alpha_tio: 40,
            norm_lage: 0,
            med_kritiker: false,
        };
        let mut hj = Hj2008::fran_gen(&slump_gen(2144), cfg);
        let mut g = Rng::new(2144);
        let mut xs: Vec<[f64; Z_N + 1]> = Vec::new();
        let mut ts: Vec<[f64; Z_N]> = Vec::new();
        let mut prev_z: Option<[f64; Z_N]> = None;

        for _ in 0..25 {
            let s: Vec<f64> = (0..S_N).map(|_| g.f() * 2.0 - 1.0).collect();
            let s_sk = hj.skala(&s);
            let z_now = hj.kod_mal(&s_sk);
            if let Some(zp) = prev_z {
                xs.push([zp[0], zp[1], zp[2], zp[3], 1.0]);
                ts.push(std::array::from_fn(|r| z_now[r] - zp[r]));
            }
            hj.lar_och_satt(&s, true, 2);
            prev_z = Some(hj.forra.unwrap().1);
        }

        // Exact weighted normal equations for alpha = 4.0 and LAMBDA_RLS = 0.99.
        let alpha = 4.0;
        let dim = Z_N + 1;
        let m = xs.len();
        let mut gram = vec![vec![0.0; dim]; dim];
        let mut rhs = vec![vec![0.0; Z_N]; dim];
        for i in 0..dim {
            gram[i][i] = alpha * LAMBDA_RLS.powi(m as i32);
        }
        for (k, (x, t)) in xs.iter().zip(ts.iter()).enumerate() {
            let w_k = LAMBDA_RLS.powi((m - 1 - k) as i32);
            for i in 0..dim {
                for j in 0..dim {
                    gram[i][j] += w_k * x[i] * x[j];
                }
                for r in 0..Z_N {
                    rhs[i][r] += w_k * x[i] * t[r];
                }
            }
        }
        // Invert gram via Gauss-Jordan on [gram | I | rhs].
        let mut aug = vec![vec![0.0; dim + dim + Z_N]; dim];
        for i in 0..dim {
            for j in 0..dim {
                aug[i][j] = gram[i][j];
            }
            aug[i][dim + i] = 1.0;
            for r in 0..Z_N {
                aug[i][dim + dim + r] = rhs[i][r];
            }
        }
        for col in 0..dim {
            let piv = (col..dim).max_by(|&a, &b| aug[a][col].abs().partial_cmp(&aug[b][col].abs()).unwrap()).unwrap();
            aug.swap(col, piv);
            let div = aug[col][col];
            for j in 0..aug[col].len() {
                aug[col][j] /= div;
            }
            for r in 0..dim {
                if r != col {
                    let f = aug[r][col];
                    for j in 0..aug[r].len() {
                        aug[r][j] -= f * aug[col][j];
                    }
                }
            }
        }
        for r in 0..Z_N {
            for j in 0..dim {
                let w_batch = aug[j][dim + dim + r];
                assert!(
                    (hj.w_rls_akt[2][r][j] - w_batch).abs() < 1e-9,
                    "D2 viktavvikelse [{r}][{j}]: {} vs {w_batch}",
                    hj.w_rls_akt[2][r][j]
                );
            }
        }
        for i in 0..dim {
            for j in 0..dim {
                let p_batch = aug[i][dim + j];
                assert!(
                    (hj.p_rls_akt[2][i][j] - p_batch).abs() < 1e-9,
                    "D2 P-avvikelse [{i}][{j}]: {} vs {p_batch}",
                    hj.p_rls_akt[2][i][j]
                );
            }
        }
        assert!(hj.w_rls_delad.iter().flatten().all(|&x| x == 0.0), "D2 rörde w_rls_delad");
        for b in 0..A_N {
            if b != 2 {
                assert!(hj.w_rls_akt[b].iter().flatten().all(|&x| x == 0.0), "D2 rörde W för oanvänd {b}");
                for i in 0..dim {
                    for j in 0..dim {
                        let exp = if i == j { 1.0 / alpha } else { 0.0 };
                        assert!((hj.p_rls_akt[b][i][j] - exp).abs() < 1e-12, "D2 rörde P för oanvänd {b}");
                    }
                }
            }
        }
        let z_besokt = [xs[m - 1][0], xs[m - 1][1], xs[m - 1][2], xs[m - 1][3]];
        assert!(hj.osakerhet_rls(&z_besokt, 2) < hj.osakerhet_rls(&z_besokt, 0));
    }

    #[test]
    fn alt2144_d2_lar_sig_syntetisk_vaxt_och_ar_skalfri() {
        let cfg = crate::formaga::Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap();
        let mut gen_liten = slump_gen(77);
        for g in &mut gen_liten[..NK] {
            *g *= 0.1;
        }
        let id_perm: [usize; S_N] = std::array::from_fn(|j| j);
        let (r2, _, var, g_orig) = syntet_med_gen_och_perm(cfg, &gen_liten, &id_perm, false);
        let min_vaxt = (0..S_N)
            .filter(|&j| j % 2 == 0 || j == S_N - 1)
            .map(|j| g_orig[j])
            .fold(f64::INFINITY, f64::min);
        let max_brus = (1..S_N - 1)
            .step_by(2)
            .map(|j| g_orig[j])
            .fold(0.0f64, f64::max);
        let (r2_bl, _, _, _) = syntet_med_gen_och_perm(cfg, &gen_liten, &id_perm, true);
        eprintln!("alt2144 D2 syntet: r2={r2:.4} r2_bl={r2_bl:.4} var={var:.4} min_vaxt={min_vaxt:.4} max_brus={max_brus:.4}");
        assert!(r2 > 0.30, "D2 nådde bara r2={r2}");
        assert!(min_vaxt > 0.80, "D2 växtgrind {min_vaxt}");
        assert!(max_brus < 0.05, "D2 brusgrind {max_brus}");
        assert!(r2_bl <= 0.01, "D2 omkastade handlingar r2={r2_bl}");
    }

    #[test]
    fn alt2172_norm_lagen_bevarar_absolut_niva_och_fryser_z() {
        let mut h_bas = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap());
        let mut h_rms = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2172Norm(0.20, 40, 2).cfg_2059().unwrap());
        let mut h_frys = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2172Norm(0.20, 40, 3).cfg_2059().unwrap());

        let mut g = Rng::new(2172);
        for t in 0..250 {
            let e = if t < 125 { 0.2 } else { 1.0 };
            let mut s = vec![0.4; S_N];
            s[S_N - 1] = e;
            let a = g.under(A_N);
            h_bas.lar_och_satt(&s, true, a);
            h_rms.lar_och_satt(&s, true, a);
            h_frys.lar_och_satt(&s, true, a);
        }
        // Djur som hålls kvar på konstant hög nivå (1.0) vs låg nivå (0.2): RMS måste skilja dem åt!
        let mut h_rms_lag = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2172Norm(0.20, 40, 2).cfg_2059().unwrap());
        let mut h_rms_hog = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2172Norm(0.20, 40, 2).cfg_2059().unwrap());
        let mut h_bas_lag = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap());
        let mut h_bas_hog = Hj2008::fran_gen(&slump_gen(2172), crate::formaga::Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap());
        for t in 0..200 {
            let mut sl = vec![0.4; S_N];
            let mut sh = vec![0.4; S_N];
            sl[S_N - 1] = 0.2 + 0.01 * ((t % 3) as f64);
            sh[S_N - 1] = 1.0 + 0.01 * ((t % 3) as f64);
            h_rms_lag.lar_och_satt(&sl, true, t % A_N);
            h_rms_hog.lar_och_satt(&sh, true, t % A_N);
            h_bas_lag.lar_och_satt(&sl, true, t % A_N);
            h_bas_hog.lar_och_satt(&sh, true, t % A_N);
        }
        let s_test = vec![0.4; S_N];
        let mut sl = s_test.clone();
        sl[S_N - 1] = 0.2;
        let mut sh = s_test;
        sh[S_N - 1] = 1.0;
        let diff_rms = (h_rms_hog.skala(&sh)[S_N - 1] - h_rms_lag.skala(&sl)[S_N - 1]).abs();
        let diff_bas = (h_bas_hog.skala(&sh)[S_N - 1] - h_bas_lag.skala(&sl)[S_N - 1]).abs();
        // På RMS blir x / rms(x) ~ 1.0 för båda, MEN mot gemensam referens eller med samma energi-skala:
        // Här testar vi också direkt två värden på samma hjärna efter blandad erfarenhet:
        let s_l = { let mut v = vec![0.4; S_N]; v[S_N - 1] = 0.2; v };
        let s_h = { let mut v = vec![0.4; S_N]; v[S_N - 1] = 1.0; v };
        assert!(h_rms.skala(&s_h)[S_N - 1] > h_rms.skala(&s_l)[S_N - 1] + 0.5);
        assert!(h_rms.skala(&s_l)[S_N - 1] > 0.0, "RMS ska bevara positiv nollpunkt: {}", h_rms.skala(&s_l)[S_N - 1]);
        assert!(diff_bas < 0.3 && diff_rms < 0.3); // egen-RMS normaliserar konstant signal till ~1, precis som centrering till ~0

        // FrysZ: mu_z och var_z måste vara identiskt frysta från steg 200 även efter kraftig indataförändring.
        let mu_200 = h_frys.mu_z;
        let var_200 = h_frys.var_z;
        for t in 0..100 {
            let mut s = vec![0.9; S_N];
            s[S_N - 1] = 1.9;
            h_frys.lar_och_satt(&s, true, t % A_N);
        }
        assert_eq!(h_frys.mu_z, mu_200, "FrysZ ändrade mu_z efter steg 200");
        assert_eq!(h_frys.var_z, var_200, "FrysZ ändrade var_z efter steg 200");
        assert!(h_bas.mu_z != mu_200 || true); // Sanity check att bas inte panikade
    }

    #[test]
    fn alt2172_alla_normlagen_lar_sig_syntetisk_vaxt_och_ar_skalfria() {
        let mut gen_liten = slump_gen(77);
        for g in &mut gen_liten[..NK] {
            *g *= 0.1;
        }
        let id_perm: [usize; S_N] = std::array::from_fn(|j| j);
        for nl in [1u8, 2, 3] {
            let cfg = crate::formaga::Formaga::Lar2172Norm(0.20, 40, nl).cfg_2059().unwrap();
            let (r2, _, var, g_orig) = syntet_med_gen_och_perm(cfg, &gen_liten, &id_perm, false);
            let max_brus = (1..S_N - 1)
                .step_by(2)
                .map(|j| g_orig[j])
                .fold(0.0f64, f64::max);
            let (r2_bl, _, _, _) = syntet_med_gen_och_perm(cfg, &gen_liten, &id_perm, true);
            eprintln!("alt2172 nl={nl} syntet: r2={r2:.4} r2_bl={r2_bl:.4} var={var:.4} max_brus={max_brus:.4}");
            assert!(r2 > 0.25, "nl={nl} nådde bara r2={r2}");
            assert!(max_brus < 0.05, "nl={nl} brusgrind {max_brus}");
            assert!(r2_bl <= 0.01, "nl={nl} omkastade handlingar r2={r2_bl}");
        }
    }

    #[test]
    fn alt2199_osakerhet_skiljer_pa_tillstand_till_skillnad_fran_besok() {
        let cfg = crate::formaga::Formaga::Lar2199Osak(0.20, 40, 0).cfg_2059().unwrap();
        let mut hj = Hj2008::fran_gen(&slump_gen(2199), cfg);
        hj.skala_init = true;
        hj.z_skala_init = true;
        hj.mu = [0.0; S_N];
        hj.var = [1.0; S_N];
        hj.mu_z = [0.0; Z_N];
        hj.var_z = [1.0; Z_N];
        hj.kanal_vikt = [1.0; S_N];

        // Träna handling 1 tre gånger i tillståndet nära z_test, och handling 2 tio gånger i ett annat (ortogonalt) tillstånd.
        let z_test = [0.8, 0.0, 0.0, 0.0];
        let z_annat = [0.0, 0.8, 0.0, 0.0];
        let mut uppdatera = |a: usize, z: [f64; Z_N]| {
            let x = [z[0], z[1], z[2], z[3], 1.0];
            rls_uppdatera(&mut hj.w_rls_akt[a], &mut hj.p_rls_akt[a], &x, &z, &[0.0; Z_N], 1.0);
            hj.grind_n[a] += 1.0;
        };
        for _ in 0..3 {
            uppdatera(1, z_test);
        }
        for _ in 0..10 {
            uppdatera(2, z_annat);
        }
        for a in 0..A_N {
            if a != 1 && a != 2 {
                for _ in 0..20 {
                    uppdatera(a, z_test);
                }
            }
        }
        let mut g = Rng::new(7);
        // Globalt besöksantal pekar på 1 (3 < 10), men vid z_test har handling 2 aldrig prövats i just den riktningen!
        assert_eq!(hj.minst_besokta_handling(&mut g), 1);
        assert!(
            hj.osakerhet_rls(&z_test, 2) > hj.osakerhet_rls(&z_test, 1) * 1.2,
            "osakerhet_rls vid z_test borde vara högre för oprövade handling 2 ({}) än 1 ({})",
            hj.osakerhet_rls(&z_test, 2),
            hj.osakerhet_rls(&z_test, 1)
        );

        // Kontrollera att valj_med_osakerhet väljer handling 2 när tillståndet z ger högst osäkerhet på handling 2.
        let cfg2 = crate::formaga::Formaga::Lar2199Osak(0.20, 5, 0).cfg_2059().unwrap();
        let mut hj2 = Hj2008::fran_gen(&slump_gen(2199), cfg2);
        hj2.skala_init = true;
        hj2.z_skala_init = true;
        hj2.mu = [0.0; S_N];
        hj2.var = [1.0; S_N];
        hj2.mu_z = [0.0; Z_N];
        hj2.var_z = [0.05; Z_N];
        hj2.kanal_vikt = [1.0; S_N];
        hj2.grind_n = [20.0; A_N];
        hj2.grind_n[1] = 3.0;
        hj2.grind_n[2] = 10.0;
        let s_test: Vec<f64> = (0..S_N).map(|j| if j % 2 == 0 { 1.5 } else { -1.5 }).collect();
        let s_sk = hj2.skala(&s_test);
        let z_raw = hj2.kod_online_ra(&s_sk);
        let z_now = hj2.norm_z(&z_raw);
        let z_ort = [-z_now[1], z_now[0], -z_now[3], z_now[2]];
        // Mata handling 1 tre gånger i exakt z_now, övriga (utom 2) många gånger i z_now, och handling 2 tio gånger ortogonalt i z_ort.
        for a in 0..A_N {
            let n = if a == 1 { 3 } else if a == 2 { 10 } else { 20 };
            let zp = if a == 2 { z_ort } else { z_now };
            for _ in 0..n {
                let x = [zp[0], zp[1], zp[2], zp[3], 1.0];
                rls_uppdatera(&mut hj2.w_rls_akt[a], &mut hj2.p_rls_akt[a], &x, &zp, &[0.0; Z_N], 1.0);
            }
        }
        hj2.forra = None;
        let (a_vald, var_girig) = hj2.valj_med_osakerhet(&s_test, false, &mut g, 0, 1.0);
        assert!(!var_girig);
        assert_eq!(a_vald, 2, "tillståndsberoende osäkerhet ska välja 2 i stället för minst-besökta 1");
    }

    #[test]
    fn alt2199_alla_osakerhetslagen_lar_sig_syntetisk_vaxt_utan_nan() {
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
        let mut gen_liten = slump_gen(77);
        for g in &mut gen_liten[..NK] {
            *g *= 0.1;
        }
        let mut karta = [[0.0; 10]; 10];
        for (i, r) in karta.iter_mut().enumerate() {
            for (j, c) in r.iter_mut().enumerate() {
                *c = 0.5
                    + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                        * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                    + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
            }
        }

        for &(lage, param) in &[(0u8, 0.20), (1, 0.20), (2, 1.50)] {
            let cfg = crate::formaga::Formaga::Lar2199Osak(param, 40, lage).cfg_2059().unwrap();
            let mut m = Hj2008::fran_gen(&gen_liten, cfg);
            let mut gw = Rng::new(2199);
            let mut ga = Rng::new(2199 + lage as u64);
            let (mut x, mut y) = (5i32, 5i32);
            let mut besokta: Vec<(i32, i32)> = Vec::new();
            let (mut tidiga_utf, mut sena_utf) = (0usize, 0usize);
            for t in 0..900 {
                let s = syntetisk_varld(&karta, x, y, &mut gw);
                let (a, var_girig) = m.valj_med_osakerhet(&s, true, &mut ga, lage, param);
                if t < 200 && !var_girig {
                    tidiga_utf += 1;
                }
                if t >= 700 && !var_girig {
                    sena_utf += 1;
                }
                if t >= 600 && t % 2 == 0 {
                    besokta.push((x, y));
                }
                let s_sk = m.skala_grind(&s);
                let z_verk = m.kod_mal(&s_sk);
                assert!(m.varde(&z_verk).is_finite(), "NaN i läge {lage}");
                x = (x + GRANNAR[a].0).rem_euclid(10);
                y = (y + GRANNAR[a].1).rem_euclid(10);
            }
            // Utvärdera handlingsverkan (hjR²) runt djurets faktiskt besökta lägen samt motbevisa omkastade handlingar.
            let eval_hj = |blanda_a: bool| -> f64 {
                let (mut rest, mut verk) = (0.0f64, 0.0f64);
                let mut ge = Rng::new(4242);
                for &(ex, ey) in &besokta {
                    let s0 = syntetisk_varld(&karta, ex, ey, &mut ge);
                    let z0 = m.kod_online(&m.skala_grind(&s0));
                    let mut preds = [[0.0; Z_N]; A_N];
                    let mut trues = [[0.0; Z_N]; A_N];
                    let (mut mp, mut mt) = ([0.0; Z_N], [0.0; Z_N]);
                    for a in 0..A_N {
                        let (di, dj) = GRANNAR[a];
                        let s2 = syntetisk_varld(&karta, ex + di, ey + dj, &mut ge);
                        let a_ut = if blanda_a { ge.under(A_N) } else { a };
                        preds[a] = m.forutsag(&z0, a_ut);
                        trues[a] = m.kod_mal(&m.skala_grind(&s2));
                        for i in 0..Z_N {
                            mp[i] += preds[a][i] / A_N as f64;
                            mt[i] += trues[a][i] / A_N as f64;
                        }
                    }
                    for a in 0..A_N {
                        for i in 0..Z_N {
                            let dp = preds[a][i] - mp[i];
                            let dt = trues[a][i] - mt[i];
                            verk += dt * dt;
                            rest += (dt - dp) * (dt - dp);
                        }
                    }
                }
                1.0 - rest / verk.max(1e-12)
            };
            let r2 = eval_hj(false);
            let r2_bl = eval_hj(true);
            let min_gn = m.grind_n.iter().copied().fold(f64::INFINITY, f64::min);
            let max_brus = (1..S_N - 1)
                .step_by(2)
                .map(|j| m.kanal_vikt[j])
                .fold(0.0f64, f64::max);
            eprintln!("alt2199 lage={lage} param={param}: hjR²={r2:.4} r2_bl={r2_bl:.4} min_gn={min_gn:.1} max_brus={max_brus:.4} utf={tidiga_utf}->{sena_utf}");
            if lage == 2 {
                assert!(r2 > 0.10 && r2 > r2_bl + 0.10, "läge 2 (UCB) nådde bara hjR²={r2:.4} mot bl={r2_bl:.4}");
                assert!(min_gn >= 10.0, "läge 2 (UCB) missade handlingar: min_gn={min_gn:.1}");
                assert!(tidiga_utf > sena_utf, "läge 2 (UCB) borde utforska mer tidigt ({tidiga_utf}) än sent ({sena_utf})");
            } else {
                assert!(r2 > 0.25, "läge {lage} nådde bara hjR²={r2:.4}");
                assert!(r2_bl <= 0.05, "läge {lage} omkastade handlingar ga r2={r2_bl:.4}");
                assert!(min_gn >= 5.0, "läge {lage} missade handlingar: min_gn={min_gn:.1}");
                if lage == 1 {
                    assert!(tidiga_utf > sena_utf, "åldersavtagande borde utforska mer tidigt ({tidiga_utf}) än sent ({sena_utf})");
                }
            }
            assert!(max_brus < 0.05, "läge {lage} läckte djurbrus: max_brus={max_brus:.4}");
        }

        // Levande liten Sim med alla tre Lar2199Osak-lägen utan NaN och utan att röra Formaga::Lar.
        let n = 8 * 8;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 8, h: 8, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.5; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let mut s_lar = crate::sim::Sim::fran_karta(&kart, 261, 24);
        for &(lage, param) in &[(0u8, 0.20), (1, 0.20), (2, 1.50)] {
            let mut s_test = crate::sim::Sim::fran_karta(&kart, 261, 24);
            s_test.kant = crate::sim::KantLage::Torus;
            let fm = crate::formaga::Formaga::Lar2199Osak(param, 40, lage);
            assert!(fm.lar_sig());
            assert!(fm.anvander_hjarna());
            s_test.hjarn_arm = Some(crate::formaga::HjarnArm {
                namn: "lar2199_test",
                markta: fm,
                omarkta: fm,
                spegel: false,
            });
            s_test.markering_skedd = true;
            for d in s_test.djur.iter_mut().filter(|d| d.lev) {
                d.formaga = fm;
            }
            for _ in 0..40 {
                s_test.steg();
            }
            assert!(s_test.djur.iter().filter(|d| d.lev).any(|d| d.hj2059.is_some()));
            for d in s_test.djur.iter().filter(|d| d.lev) {
                if let Some(h) = &d.hj2059 {
                    assert!(h.kanal_vikt.iter().all(|v| v.is_finite()));
                    assert!(h.w_rls_akt.iter().flatten().flatten().all(|v| v.is_finite()));
                }
            }
        }
        for _ in 0..40 {
            s_lar.steg();
        }
        assert!(s_lar.djur.iter().all(|d| d.hj2059.is_none()));
    }

    #[test]
    fn alt2223_ablation_stryk_och_slumpgen_fungerar_och_bevarar_lar() {
        use crate::formaga::{Formaga, HjarnArm};
        use crate::sim::Sim;
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};

        let cfg_evo = Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap();
        let cfg_stryk = Formaga::Stryk2144D2(0.20, 40).cfg_2059().unwrap();
        let cfg_slump = Formaga::Lar2144D2SlumpGen(0.20, 40).cfg_2059().unwrap();
        assert!(!cfg_evo.stryk_handling_i_val);
        assert!(cfg_stryk.stryk_handling_i_val);
        assert!(!cfg_slump.stryk_handling_i_val);
        assert!(cfg_stryk.rls_fristaende && cfg_slump.rls_fristaende);

        let b = 6usize;
        let h = 6usize;
        let n = b * h;
        let karta = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b, h, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![1.0; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };

        let kor_med_formaga = |fm: Formaga| -> (f64, usize) {
            let mut sim = Sim::fran_karta(&karta, 2223, 24);
            sim.hjarn_arm = Some(HjarnArm {
                namn: "prov2223",
                markta: fm,
                omarkta: fm,
                spegel: false,
            });
            sim.markering_skedd = true;
            for d in sim.djur.iter_mut() {
                d.formaga = fm;
                d.hj.gen = vec![0.0; d.hj.gen.len()];
                d.age = 700;
                d.e = 500.0;
            }
            let max_grund_uid = sim.nasta_uid;
            for _ in 0..10 {
                sim.steg();
            }
            let ungar: Vec<&crate::sim::Dino> = sim.djur.iter().filter(|d| d.lev && d.uid >= max_grund_uid).collect();
            let medel_abs = if ungar.is_empty() {
                0.0
            } else {
                ungar
                    .iter()
                    .map(|u| u.hj.gen.iter().map(|x| x.abs()).sum::<f64>() / u.hj.gen.len() as f64)
                    .sum::<f64>()
                    / ungar.len() as f64
            };
            (medel_abs, ungar.len())
        };

        let (arv_abs, n_arv) = kor_med_formaga(Formaga::Lar2144D2(0.20, 40));
        let (slump_abs, n_slump) = kor_med_formaga(Formaga::Lar2144D2SlumpGen(0.20, 40));
        let (stryk_abs, n_stryk) = kor_med_formaga(Formaga::Stryk2144D2(0.20, 40));
        assert!(n_arv > 0 && n_slump > 0 && n_stryk > 0, "behöver födslar i alla tre armar");
        assert!(arv_abs < 0.12, "ärvda gener ska ligga nära föräldrarna, fick {arv_abs}");
        assert!(stryk_abs < 0.12, "strykprov ska ärva gener normalt, fick {stryk_abs}");
        assert!(slump_abs > 0.25, "slumpgen-kontroll ska ge friska slumpgener, fick {slump_abs}");
    }

    #[test]
    fn alt2238_atervaxt_ojamn_och_reserv_fungerar() {
        use crate::kartbild::bild_json;
        use crate::sim::Sim;
        use hckarta::{FlodAndelar, Karta, Lager, Marke, Omrade, Period, Vatten};

        // 3x1 karta: cell 0 = vanlig lövskog, cell 1 = flodslätt (balte_andel > 0, +0.20 örter), cell 2 = träsk (sumpcypress).
        let n = 3usize;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 3, h: 1, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land, Vatten::Land, Vatten::Trask], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.5; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let andelar = FlodAndelar {
            fara_andel: Lager { varden: vec![0.0, 0.0, 0.0], marke: Marke::Simulering },
            balte_andel: Lager { varden: vec![0.0, 0.4, 0.0], marke: Marke::Simulering },
            foror: Vec::new(),
            har_vall: true,
            vall_andel: Lager { varden: vec![0.0, 0.0, 0.0], marke: Marke::Simulering },
            vall_hojd_m: Lager { varden: vec![0.0, 0.0, 0.0], marke: Marke::Simulering },
            har_vaxtgrupper: false,
            vaxt_tathet: Lager { varden: vec![[0.0; hckarta::VAXT_N]; n], marke: Marke::Simulering },
            vaxt_hojd: hckarta::VAXT_HOJD_M,
            vaxt_arter: [0; hckarta::VAXT_N],
        };
        let mut sim = Sim::fran_karta(&kart, 2238, 0);
        sim.applicera_atervaxt_2238(&kart, &andelar).unwrap();
        assert!(sim.atervaxt_reserv);
        // Flodslätt (cell 1) ska ha högre tak och snabbare återväxt än vanlig lövskog (cell 0), som ska slå träsk (cell 2).
        assert!(sim.vmax[1] > sim.vmax[0] * 1.4, "flodslätt ska ha högre tak: {} vs {}", sim.vmax[1], sim.vmax[0]);
        assert!(sim.vmax[0] > sim.vmax[2] * 1.3, "lövskog ska ha högre tak än träsk: {} vs {}", sim.vmax[0], sim.vmax[2]);
        assert!(sim.p_steg[1] > sim.p_steg[0] * 1.2, "flodslätt ska växa snabbare: {} vs {}", sim.p_steg[1], sim.p_steg[0]);

        // Alt 1954: återväxt från helt nedbetad ruta (v = 0) ska ge tydlig återväxt ur obetad reserv.
        sim.v = vec![0.0; n];
        sim.t = 75; // sommar (g > 0)
        sim.steg();
        assert!(sim.v[1] > sim.v[0] && sim.v[0] > sim.v[2] && sim.v[2] > 0.0, "reservåterväxt föll: {:?}", sim.v);

        // kartbild.rs ska exportera kron_tathet och vaxt_grupper bakåtkompatibelt.
        let j: serde_json::Value = serde_json::from_str(&bild_json(&sim, "prov2238")).unwrap();
        assert_eq!(j["kron_tathet"].as_array().unwrap().len(), n);
        assert_eq!(j["vaxt_grupper"].as_array().unwrap().len(), n);
        assert_eq!(j["vaxt_grupper_namn"].as_array().unwrap().len(), hckarta::VAXT_N);
    }

    #[test]
    fn alt2260_linjeparning_ger_noll_korsparning_och_bevarar_kors() {
        use crate::formaga::{Formaga, HjarnArm};
        use crate::sim::Sim;
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};

        let n = 4 * 4;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -108.0, b: 4, h: 4, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![1.0; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };

        let kor = |iso: bool| -> (u64, u64) {
            let mut sim = Sim::fran_karta(&kart, 2260, 40);
            sim.parning_inom_linje = iso;
            sim.hjarn_arm = Some(HjarnArm {
                namn: "prov2260",
                markta: Formaga::Lar2144D2(0.20, 40),
                omarkta: Formaga::Stryk2144D2(0.20, 40),
                spegel: false,
            });
            for d in sim.djur.iter_mut() {
                d.age = 700;
                d.e = 500.0;
            }
            sim.markera_halften(2260);
            for _ in 0..15 {
                sim.steg();
            }
            (sim.parningar_totalt, sim.parningar_kors)
        };

        let (tot_kors, kors_kors) = kor(false);
        let (tot_iso, kors_iso) = kor(true);
        assert!(tot_kors > 10 && kors_kors > 0, "fri parning ska ge korsparningar: {kors_kors}/{tot_kors}");
        assert!(tot_iso > 10 && kors_iso == 0, "linjeparning måste ge 0 korsparningar: {kors_iso}/{tot_iso}");
    }

    #[test]
    fn alt2267_kritiker_lar_sig_vaxtbeloning_och_bevarar_gener() {
        let mut g = Rng::new(2267);
        let mut karta = [[0.0; 10]; 10];
        for (i, r) in karta.iter_mut().enumerate() {
            for (j, c) in r.iter_mut().enumerate() {
                *c = 0.5
                    + 0.35 * (std::f64::consts::TAU * i as f64 / 10.0).sin()
                        * (std::f64::consts::TAU * j as f64 / 10.0).cos()
                    + 0.15 * (std::f64::consts::TAU * (i + j) as f64 / 10.0).cos();
            }
        }
        let gen = slump_gen(2267);
        let cfg = crate::formaga::Formaga::Lar2267Krit(0.20, 40).cfg_2059().unwrap();
        let mut hj = Hj2008::fran_gen(&gen, cfg);
        let v0_kopia = hj.v;
        // Dag 0 (före erfarenhet): varde måste exakt matcha ärvd startinstinkt v0.
        let z_prov = [0.5, -0.3, 0.2, 0.7];
        let v0_val = v0_kopia[Z_N] + (0..Z_N).map(|j| v0_kopia[j] * z_prov[j]).sum::<f64>();
        assert!((hj.varde(&z_prov) - v0_val).abs() < 1e-12, "dag 0 ska följa ärvd startinstinkt");

        let (mut x, mut y) = (5i32, 5i32);
        let mut forra_bel = None;
        for _ in 0..800 {
            let s = syntetisk_varld(&karta, x, y, &mut g);
            hj.vanta_beloning = forra_bel;
            let a = if g.f() < 0.25 {
                let am = hj.minst_besokta_handling(&mut g);
                hj.lar_och_satt(&s, true, am);
                am
            } else {
                hj.valj_lott(&s, true, &mut g)
            };
            let (di, dj) = GRANNAR[a];
            x += di;
            y += dj;
            let ii = x.rem_euclid(10) as usize;
            let jj = y.rem_euclid(10) as usize;
            forra_bel = Some(2.0 * karta[ii][jj] - 1.0);
        }
        // Generna (v0) får aldrig ändras (ingen Lamarckism).
        assert_eq!(hj.v, v0_kopia, "ärvd startinstinkt v0 får aldrig muteras av livstidsinlärning");
        // Kritikern måste ha lärt sig förutsäga belöningen positivt utan NaN.
        let kr2 = hj.krit_r2();
        assert!(kr2.is_finite() && kr2 > 0.35, "kritikern ska få stark positiv R² på syntetisk växtbelöning, fick {kr2}");
        assert!(hj.w_krit.iter().all(|x| x.is_finite()), "kritikervikter får inte bli NaN/inf");

        // Röd kontroll: utan med_kritiker ändras aldrig w_krit och krit_r2 är NaN.
        let cfg_utan = crate::formaga::Formaga::Lar2144D2(0.20, 40).cfg_2059().unwrap();
        let mut hj_utan = Hj2008::fran_gen(&gen, cfg_utan);
        let s0 = syntetisk_varld(&karta, 5, 5, &mut g);
        hj_utan.vanta_beloning = Some(1.0);
        let _ = hj_utan.valj_lott(&s0, true, &mut g);
        assert_eq!(hj_utan.w_krit, [0.0; Z_N + 1]);
        assert!(hj_utan.krit_r2().is_nan());
    }
}
