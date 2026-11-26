//! Alt 2282 (Skede B, Steg B1): 29-kanalig JEPA-världsmodell (`Z_B1 = 6`) med krontäcke, skadesignal och LeCuns lärda kritiker.
//! Håller `hjarna.rs` och `hjarna2008.rs` (bekräftad Skede A-referens `Lar2267Krit`) helt orörda.

use crate::hjarna::{gauss, lott_bland_max, NK, NP};
use crate::rng::Rng;
use crate::tal::{A_N, SIGMA0, SIGMA_MUT, S_N, Z_N};

/// Antal sinnen i B1: 19 ursprungliga (växt, antal, energi) + 9 krontäcken (egen + 8 grannar).
/// OBS: ingen post-angrepp `d.skada` i sinnesvektorn (annars läcker utfallskollidern träffen och stjäl vikten från krontäcket i kritikern).
pub const S_B1: usize = S_N + A_N; // 28
/// Inre bildens dimension i B1 (`Z_B1 = 6`, Neons R1-rekommendation när 9 nya aktiva kanaler tillkommer).
pub const Z_B1: usize = 6;

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

/// Symmetrisk noll-centrerad variation för den ärvda skadeviktsgenen `w_skada` (medelvärde exakt 0,0 — inget förkodat tecken!).
pub const SIGMA0_W_SKADA: f64 = 5.0;
pub const SIGMA_MUT_W_SKADA: f64 = 1.5;
pub const W_SKADA_MAX: f64 = 50.0;

/// Separat XOR för B1-genarv så världens `sim.g` inte ändras.
pub const B1_GEN_XOR: u64 = 0x2282_B100_5A4B_0001;

/// Ärvd B1-del av genomet: full kodare `wk_b1` för de 29 sinnena in i `Z_B1 = 6`, extra startinstinkt `v_extra`,
/// samt skadeviktsgenen `w_skada` (`Alt 1882`, medelvärde 0,0 utan förkodat tecken).
#[derive(Clone, Debug)]
pub struct GenB1 {
    /// Extra kodarvikter: 10 nya sinnen för axlar 0..4, samt hela 29+1 rader för axlar 4..6.
    pub wk_extra_04: [[f64; S_B1 - S_N]; Z_N],
    pub wk_rad_46: [[f64; S_B1 + 1]; Z_B1 - Z_N],
    /// Extra ärvd startinstinkt för axlar 4..6.
    pub v_extra: [f64; Z_B1 - Z_N],
    /// Ärvd vikt för smärta/skada i kroppens signal `r_t = ΔE_t − w_skada · skada_t`.
    /// Startar symmetriskt kring `0.0` (`N(0, 20)`) — 50 % positiva och 50 % negativa, utan förkodat tecken!
    pub w_skada: f64,
}

impl GenB1 {
    pub fn slump(g: &mut Rng) -> Self {
        let mut wk_extra_04 = [[0.0; S_B1 - S_N]; Z_N];
        for rad in &mut wk_extra_04 {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut wk_rad_46 = [[0.0; S_B1 + 1]; Z_B1 - Z_N];
        for rad in &mut wk_rad_46 {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut v_extra = [0.0; Z_B1 - Z_N];
        for x in &mut v_extra {
            *x = SIGMA0 * gauss(g);
        }
        let w_skada = (SIGMA0_W_SKADA * gauss(g)).clamp(-W_SKADA_MAX, W_SKADA_MAX);
        Self {
            wk_extra_04,
            wk_rad_46,
            v_extra,
            w_skada,
        }
    }

    pub fn arv(mor: &GenB1, far: &GenB1, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ B1_GEN_XOR);
        let mut wk_extra_04 = [[0.0; S_B1 - S_N]; Z_N];
        for i in 0..Z_N {
            for j in 0..(S_B1 - S_N) {
                let bas = if g.f() < 0.5 {
                    mor.wk_extra_04[i][j]
                } else {
                    far.wk_extra_04[i][j]
                };
                wk_extra_04[i][j] = bas + SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut wk_rad_46 = [[0.0; S_B1 + 1]; Z_B1 - Z_N];
        for i in 0..(Z_B1 - Z_N) {
            for j in 0..=S_B1 {
                let bas = if g.f() < 0.5 {
                    mor.wk_rad_46[i][j]
                } else {
                    far.wk_rad_46[i][j]
                };
                wk_rad_46[i][j] = bas + SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut v_extra = [0.0; Z_B1 - Z_N];
        for i in 0..(Z_B1 - Z_N) {
            let bas = if g.f() < 0.5 {
                mor.v_extra[i]
            } else {
                far.v_extra[i]
            };
            v_extra[i] = bas + SIGMA_MUT * gauss(&mut g);
        }
        let bas_w = if g.f() < 0.5 { mor.w_skada } else { far.w_skada };
        let w_skada = (bas_w + SIGMA_MUT_W_SKADA * gauss(&mut g)).clamp(-W_SKADA_MAX, W_SKADA_MAX);
        Self {
            wk_extra_04,
            wk_rad_46,
            v_extra,
            w_skada,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HjB1Config {
    pub stryk_handling_i_val: bool,
    pub rls_alpha_tio: u16,
    pub med_kritiker: bool,
    pub anvand_w_skada: bool,
    pub r_klamp: f64,
}

#[derive(Clone, Debug)]
pub struct HjB1 {
    pub cfg: HjB1Config,
    /// Fryst ärvd kodare W_k: [Z_B1][S_B1 + 1].
    pub wk: [[f64; S_B1 + 1]; Z_B1],
    /// 9 fristående regulariserade RLS-världsmodeller (D2).
    pub w_rls_akt: [[[f64; Z_B1 + 1]; Z_B1]; A_N],
    pub p_rls_akt: [[[f64; Z_B1 + 1]; Z_B1 + 1]; A_N],
    /// Ärvd startinstinkt `v_0` (aldrig ändrad under livet).
    pub v: [f64; Z_B1 + 1],
    /// Lärd kritiker `w_krit` (1D-RLS residual ovanpå `v_0`).
    pub w_krit: [f64; Z_B1 + 1],
    pub p_krit: [[f64; Z_B1 + 1]; Z_B1 + 1],
    pub vanta_beloning: Option<f64>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    /// Kausal EMA-skalning av de 29 sinnena.
    pub skala_init: bool,
    pub mu: [f64; S_B1],
    pub var: [f64; S_B1],
    /// Kausal z-normering av kodarens utgång.
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_B1],
    pub var_z: [f64; Z_B1],
    /// Oförändrad skärpt blind KanalGrind över alla 29 kanaler (`f32`-ackumulatorer för halverat minne & SIMD).
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

impl HjB1 {
    pub fn fran_gen(gen: &[f64], gen_b1: &GenB1, cfg: HjB1Config) -> Self {
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
        Self {
            cfg,
            wk,
            w_rls_akt: [[[0.0; Z_B1 + 1]; Z_B1]; A_N],
            p_rls_akt: [diag_p(alpha_akt); A_N],
            v,
            w_krit: [0.0; Z_B1 + 1],
            p_krit: diag_p(ALPHA_KRIT),
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
            var_z: [1.0; Z_B1],
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

    fn x_av(z: &[f64; Z_B1]) -> [f64; Z_B1 + 1] {
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

    pub fn varde(&self, y: &[f64; Z_B1]) -> f64 {
        if self.cfg.med_kritiker {
            let mut s = self.v[Z_B1] + self.w_krit[Z_B1];
            for j in 0..Z_B1 {
                s += (self.v[j] + self.w_krit[j]) * y[j];
            }
            s
        } else {
            let mut s = self.v[Z_B1];
            for j in 0..Z_B1 {
                s += self.v[j] * y[j];
            }
            s
        }
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

    pub fn g_skada(&self) -> f64 {
        0.0
    }

    pub fn minst_besokta_handling(&self, g: &mut Rng) -> usize {
        let mut neg = [0.0; A_N];
        for a in 0..A_N {
            neg[a] = -(self.grind_n[a] as f64);
        }
        lott_bland_max(&neg, g)
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

        // 1. Lärd kritiker (om påslagen) på verklig ny inre bild z_mal och förra stegets kroppssignal r_t.
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

        let mut varden = [0.0; A_N];
        let mut ys = [[0.0; Z_B1]; A_N];
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

    #[test]
    fn alt2282_kanalgrind_slapper_igenom_krontacke_och_stryper_brus() {
        let mut rng = Rng::new(228201);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let cfg = HjB1Config {
            stryk_handling_i_val: false,
            rls_alpha_tio: 40,
            med_kritiker: true,
            anvand_w_skada: true,
            r_klamp: 100.0,
        };
        let mut b = HjB1::fran_gen(&hj.gen, &gb1, cfg);
        let w = 16usize;
        let mut vaxt = vec![0.0; w * w];
        let mut kron = vec![0.0; w * w];
        for y in 0..w {
            for x in 0..w {
                let fx = x as f64 * std::f64::consts::TAU / w as f64;
                let fy = y as f64 * std::f64::consts::TAU / w as f64;
                vaxt[y * w + x] = 0.5 + 0.35 * fx.sin() * fy.cos();
                kron[y * w + x] = 0.5 + 0.40 * (fx + 1.2).cos() * (fy - 0.7).sin();
            }
        }
        let grannar: [(i32, i32); 9] = [
            (0, 0), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1),
        ];
        let mut px = 4usize;
        let mut py = 4usize;
        let mut e = 0.8f64;
        for _ in 0..1200 {
            let mut s = [0.0; S_B1];
            for (k, &(dx, dy)) in grannar.iter().enumerate() {
                let nx = (px as i32 + dx).rem_euclid(w as i32) as usize;
                let ny = (py as i32 + dy).rem_euclid(w as i32) as usize;
                s[k * 2] = vaxt[ny * w + nx];
                s[k * 2 + 1] = rng.f();
                s[S_N + k] = kron[ny * w + nx];
            }
            s[18] = e;
            let a = if rng.f() < 0.25 {
                let am = b.minst_besokta_handling(&mut rng);
                b.lar_och_satt(&s, true, am);
                am
            } else {
                b.valj_lott(&s, true, &mut rng)
            };
            let (dx, dy) = grannar[a];
            px = (px as i32 + dx).rem_euclid(w as i32) as usize;
            py = (py as i32 + dy).rem_euclid(w as i32) as usize;
            let c = py * w + px;
            let de = vaxt[c] - 0.5;
            let hit = if rng.f() < 0.15 * (1.0 - kron[c]) { 0.30 } else { 0.0 };
            e = (e + 0.05 * de).clamp(0.1, 1.8);
            b.vanta_beloning = Some(de - 4.0 * hit);
        }
        assert!(b.g_vaxt() > 0.70, "g_vaxt för låg: {}", b.g_vaxt());
        assert!(b.g_kron() > 0.70, "g_kron för låg: {}", b.g_kron());
        assert!(b.g_djur() < 0.05, "g_djur för hög: {}", b.g_djur());
        assert!(b.krit_r2().is_finite() && b.krit_r2() > 0.08, "krit_r2 för låg: {}", b.krit_r2());

        // Verifiera att startgenen w_skada är symmetrisk kring 0 utan förkodat tecken
        let mut g2 = Rng::new(228202);
        let mut pos = 0usize;
        let mut sum_w = 0.0;
        for _ in 0..1000 {
            let wv = GenB1::slump(&mut g2).w_skada;
            if wv > 0.0 { pos += 1; }
            sum_w += wv;
        }
        assert!((440..=560).contains(&pos), "w_skada ej symmetrisk kring 0: pos={pos}/1000");
        assert!((sum_w / 1000.0).abs() < 2.5, "w_skada medelvärde ej nära 0: {}", sum_w / 1000.0);
    }

    #[test]
    fn alt2279_fadersplats_bugg_ger_exakt_noll_korsparning() {
        use crate::formaga::{Formaga, HjarnArm};
        use crate::sim::Sim;
        use crate::tal;
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};

        let n = 4 * 4;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -106.0, b: 4, h: 4, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.6; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let mut sim = Sim::fran_karta(&kart, 2279, 24);
        sim.parning_inom_linje = true;
        sim.hjarn_arm = Some(HjarnArm {
            namn: "iso_prov",
            markta: Formaga::Lar,
            omarkta: Formaga::Lar,
            spegel: false,
        });
        sim.markera_halften(2279);
        for (idx, d) in sim.djur.iter_mut().enumerate() {
            d.i = 1;
            d.j = 1;
            if d.hona {
                d.e = tal::E_DELA * 1.5;
                d.age = tal::LITEN + 100;
            } else {
                d.e = tal::E_START;
                d.age = if idx % 4 == 1 { tal::LIV - 1 } else { tal::LITEN + 100 };
            }
        }
        for _ in 0..30 {
            sim.steg();
        }
        assert!(sim.parningar_totalt > 0, "inga födslar skedde i provet");
        assert_eq!(
            sim.parningar_kors, 0,
            "fadersplats-buggen släppte igenom {} korsparningar under parning_inom_linje",
            sim.parningar_kors
        );
    }

    #[test]
    fn alt2282_rovdjur_b1_skada_kron_och_armar_utan_nan() {
        use crate::formaga::{Formaga, HjarnArm};
        use crate::sim::{RovdjurB1Param, Sim};
        use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};

        let n = 6 * 6;
        let kart = Karta {
            omrade: Omrade { lat0: 47.0, lon0: -106.0, b: 6, h: 6, ruta_m: 2000.0 },
            t: 500_000.0,
            hojd: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vatten_yta: Lager { varden: vec![100.0; n], marke: Marke::Last },
            vattentyp: Lager { varden: vec![Vatten::Land; n], marke: Marke::Last },
            mark: Lager { varden: vec![[0.25; 4]; n], marke: Marke::Simulering },
            vaxt: Lager { varden: vec![0.6; n], marke: Marke::Simulering },
            period: Lager { varden: vec![Period::Aldre; n], marke: Marke::Simulering },
        };
        let mut s1 = Sim::fran_karta(&kart, 2282, 20);
        let mut s2 = Sim::fran_karta(&kart, 2282, 20);
        for _ in 0..20 {
            s1.steg();
            s2.steg();
        }
        assert_eq!(s1.djur_summa(), s2.djur_summa());

        for &(m_f, o_f) in &[
            (Formaga::LarB1Krit(0.20, 40), Formaga::StrykB1Krit(0.20, 40)),
            (Formaga::FrystKritB1(0.20, 40), Formaga::LarB1UtanSkada(0.20, 40)),
            (Formaga::LarB1Krit(0.20, 40), Formaga::TabellB1),
        ] {
            let mut sim = Sim::fran_karta(&kart, 2282, 40);
            sim.hjarn_arm = Some(HjarnArm { namn: "b1_test", markta: m_f, omarkta: o_f, spegel: false });
            let mut param = RovdjurB1Param::default();
            param.p_bas = 0.05;
            sim.aktivera_rovdjur_b1(2282, param);
            sim.markera_halften(2282);
            for _ in 0..120 {
                sim.steg();
            }
            let rov = sim.rovdjur_b1.as_ref().unwrap();
            assert!(rov.angrepp_markt + rov.angrepp_omarkt > 0, "inga rovdjursangrepp skedde");
            for d in sim.djur.iter().filter(|d| d.lev) {
                assert!(d.e.is_finite() && d.skada.is_finite());
                if let Some(ref hb) = d.hj_b1 {
                    assert!(hb.w_krit.iter().all(|x| x.is_finite()));
                    assert!(hb.kanal_vikt.iter().all(|x| x.is_finite()));
                }
            }
        }
    }
}
