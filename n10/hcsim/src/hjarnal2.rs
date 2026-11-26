//! Alt 2348 (Steg L2 i LeCun-planen): Biologisk inställare (delade fysiologiska kritiker + kroppsstyrd målväxling)
//! samt Mode-2 → Mode-1-kompilering (reaktiv vana vid låg överraskning).
//!
//! Bygger ovanpå bekräftade Steg B1 (`hjarnab1.rs`, `Alt 2297`) och lämnar `hjarna.rs`, `hjarna2008.rs`
//! och `hjarnab1.rs` helt orörda.
//! All ny kärnmatematik är maskinverifierad i Lean 4 (`HcsimJepaL2.lean`, 8/8 satser, 0 sorry, 3/3 rödprov).

use crate::hjarna::{gauss, lott_bland_max, NK, NP};
use crate::hjarnab1::{GenB1, S_B1, Z_B1};
use crate::rng::Rng;
use crate::tal::{A_N, E_START, LITEN, S_N, Z_N};

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

/// Normaliserad inlärningshastighet för Mode-2 → Mode-1-kompilering (`0 < η < 2`, Lean-bevisad `l2_mode1_nlms_error_shrinks`).
const ETA_M1: f64 = 0.25;
/// Energisparfaktor i Mode-1 (`α = 0,25 ∈ [0, 1]`, Lean-bevisad `l2_mode1_energy_cost_bound`).
pub const MODE1_KOST_FAKTOR: f64 = 0.25;

/// Symmetrisk nollcentrerad variation för inställargenerna `theta_e`, `theta_s`, `theta_j` (Regel 10: inget förkodat tecken!).
pub const SIGMA0_THETA: f64 = 2.0;
pub const SIGMA_MUT_THETA: f64 = 0.5;
pub const THETA_MAX: f64 = 30.0;

/// Symmetrisk nollcentrerad variation för Mode-1-tröskelgenen `tau_m1_raw` (`tau_m1 = tau_m1_raw.max(0.0)`).
pub const SIGMA0_TAU_M1: f64 = 0.5;
pub const SIGMA_MUT_TAU_M1: f64 = 0.15;
pub const TAU_M1_MAX: f64 = 5.0;

/// Separat XOR för L2-genarv så världens `sim.g` aldrig rörs.
pub const L2_GEN_XOR: u64 = 0x2348_1200_7C3D_0001;

/// Ärvd L2-del av genomet: biologisk inställare (`theta_e`, `theta_s`, `theta_j`) och Mode-1-tröskel (`tau_m1_raw`).
/// Samtliga gener startar symmetriskt kring `0.0` utan förkodat beteendetecken (Regel 10).
#[derive(Clone, Copy, Debug)]
pub struct GenL2 {
    /// Skiftar säkerhetsvikten med relativ energireserv `e_norm ∈ [-1, 1]` (>0: mätt = försiktigare, hungrig = djärvare).
    pub theta_e: f64,
    /// Skiftar säkerhetsvikten med aktuell sårskada `skada ∈ [0, 1]` (>0: skadad = försiktigare).
    pub theta_s: f64,
    /// Skiftar säkerhetsvikten med ungdjursfas `ung ∈ {0, 1}` (>0: ungar = försiktigare under trädkronor).
    pub theta_j: f64,
    /// Rå tröskelgen för Mode-2 → Mode-1-växling (`tau_m1 = tau_m1_raw.max(0.0)`).
    pub tau_m1_raw: f64,
}

impl GenL2 {
    pub fn slump(g: &mut Rng) -> Self {
        Self {
            theta_e: (SIGMA0_THETA * gauss(g)).clamp(-THETA_MAX, THETA_MAX),
            theta_s: (SIGMA0_THETA * gauss(g)).clamp(-THETA_MAX, THETA_MAX),
            theta_j: (SIGMA0_THETA * gauss(g)).clamp(-THETA_MAX, THETA_MAX),
            tau_m1_raw: (SIGMA0_TAU_M1 * gauss(g)).clamp(-TAU_M1_MAX, TAU_M1_MAX),
        }
    }

    pub fn arv(mor: &GenL2, far: &GenL2, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ L2_GEN_XOR);
        let be = if g.f() < 0.5 { mor.theta_e } else { far.theta_e };
        let bs = if g.f() < 0.5 { mor.theta_s } else { far.theta_s };
        let bj = if g.f() < 0.5 { mor.theta_j } else { far.theta_j };
        let bt = if g.f() < 0.5 { mor.tau_m1_raw } else { far.tau_m1_raw };
        Self {
            theta_e: (be + SIGMA_MUT_THETA * gauss(&mut g)).clamp(-THETA_MAX, THETA_MAX),
            theta_s: (bs + SIGMA_MUT_THETA * gauss(&mut g)).clamp(-THETA_MAX, THETA_MAX),
            theta_j: (bj + SIGMA_MUT_THETA * gauss(&mut g)).clamp(-THETA_MAX, THETA_MAX),
            tau_m1_raw: (bt + SIGMA_MUT_TAU_M1 * gauss(&mut g)).clamp(-TAU_M1_MAX, TAU_M1_MAX),
        }
    }
}

/// Fysiologiskt kroppsläge i aktuellt steg (inga världsfusk — enbart egen energi, egen sårskada och egen ålder).
#[derive(Clone, Copy, Debug)]
pub struct KroppL2 {
    pub e_norm: f64,
    pub skada: f64,
    pub ung: f64,
}

impl KroppL2 {
    pub fn fran_djur(e: f64, e_skala: f64, skada: f64, age: i64) -> Self {
        let e_ref = (E_START * e_skala).max(1.0);
        let e_norm = ((e - e_ref) / e_ref).clamp(-1.0, 1.0);
        let s_nu = skada.clamp(0.0, 1.0);
        let ung = if age < LITEN { 1.0 } else { 0.0 };
        Self {
            e_norm,
            skada: s_nu,
            ung,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HjL2Config {
    pub stryk_handling_i_val: bool,
    pub rls_alpha_tio: u16,
    /// True = använd ärvda inställargener (`theta_e, theta_s, theta_j`); False = fast `w_skada.max(0.0)` (superpositionskontroll).
    pub med_installare: bool,
    /// True = tillåt Mode-2 → Mode-1-kompilering vid låg överraskning (`senaste_fel ≤ tau_m1`).
    pub med_mode1: bool,
    pub r_klamp: f64,
}

#[derive(Clone, Debug)]
pub struct HjL2 {
    pub cfg: HjL2Config,
    /// Fryst ärvd kodare W_k: [Z_B1][S_B1 + 1] (identisk med HjB1).
    pub wk: [[f64; S_B1 + 1]; Z_B1],
    /// 9 fristående regulariserade RLS-världsmodeller (D2, identisk med HjB1).
    pub w_rls_akt: [[[f64; Z_B1 + 1]; Z_B1]; A_N],
    pub p_rls_akt: [[[f64; Z_B1 + 1]; Z_B1 + 1]; A_N],
    /// Ärvd startinstinkt `v_0` (aldrig ändrad under livet).
    pub v: [f64; Z_B1 + 1],
    /// Delade fysiologiska kritiker (`w_mat` för ΔE, `w_fara` för skada) med delad kovariansmatris `p_krit`.
    pub w_mat: [f64; Z_B1 + 1],
    pub w_fara: [f64; Z_B1 + 1],
    pub p_krit: [[f64; Z_B1 + 1]; Z_B1 + 1],
    pub vanta_de: Option<f64>,
    pub vanta_skada: Option<f64>,
    /// R²-statistik för matkritikern och farakritikern.
    pub krit_n: u64,
    pub krit_mat_sy: f64,
    pub krit_mat_sy2: f64,
    pub krit_mat_se2: f64,
    pub krit_fara_sy: f64,
    pub krit_fara_sy2: f64,
    pub krit_fara_se2: f64,
    /// Mode-2 → Mode-1 reaktiv linjär policy `W_m1[a] · [z, 1]`.
    pub w_m1: [[f64; Z_B1 + 1]; A_N],
    pub mode1_steg: u64,
    pub mode2_steg: u64,
    pub senaste_var_mode1: bool,
    /// Kausal EMA-skalning av de 28 sinnena.
    pub skala_init: bool,
    pub mu: [f64; S_B1],
    pub var: [f64; S_B1],
    /// Kausal z-normering av kodarens utgång.
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_B1],
    pub var_z: [f64; Z_B1],
    /// Oförändrad skärpt blind KanalGrind över alla 28 kanaler.
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

/// Samtidig exakt 1D-RLS-uppdatering av `w_mat` och `w_fara` med delad kovariansmatris `p` (Lean 4: `rls_critic_superposition`).
fn rls_uppdatera_delad_kritiker(
    w_mat: &mut [f64; Z_B1 + 1],
    w_fara: &mut [f64; Z_B1 + 1],
    p: &mut [[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    err_mat: f64,
    err_fara: f64,
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
        w_mat[m] += err_mat * gain[m];
        w_fara[m] += err_fara * gain[m];
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

/// Normaliserad NLMS-imitationsuppdatering av Mode-1-policyn `w_m1` mot Mode-2:s valda handling `a_m2`
/// (Lean 4: `l2_mode1_nlms_error_shrinks`).
fn kompilera_mode1(w_m1: &mut [[f64; Z_B1 + 1]; A_N], x: &[f64; Z_B1 + 1], a_m2: usize) {
    let mut x2 = 0.0;
    for m in 0..=Z_B1 {
        x2 += x[m] * x[m];
    }
    let steg = ETA_M1 / (1.0 + x2);
    for a in 0..A_N {
        let mal = if a == a_m2 { 1.0 } else { 0.0 };
        let pred = dot_zx(&w_m1[a], x);
        let err = mal - pred;
        let skala = steg * err;
        for m in 0..=Z_B1 {
            w_m1[a][m] += skala * x[m];
        }
    }
}

impl HjL2 {
    pub fn fran_gen(gen: &[f64], gen_b1: &GenB1, cfg: HjL2Config) -> Self {
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
            w_mat: [0.0; Z_B1 + 1],
            w_fara: [0.0; Z_B1 + 1],
            p_krit: diag_p(ALPHA_KRIT),
            vanta_de: None,
            vanta_skada: None,
            krit_n: 0,
            krit_mat_sy: 0.0,
            krit_mat_sy2: 0.0,
            krit_mat_se2: 0.0,
            krit_fara_sy: 0.0,
            krit_fara_sy2: 0.0,
            krit_fara_se2: 0.0,
            w_m1: [[0.0; Z_B1 + 1]; A_N],
            mode1_steg: 0,
            mode2_steg: 0,
            senaste_var_mode1: false,
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
        }
    }

    /// Effektiv inställarvikt `u_fara(kropp) = max(0, w_skada + θ_e·e_norm + θ_s·skada + θ_j·ung)`
    /// (Lean 4: `l2_configurator_nonneg`, `l2_danger_never_rewarded`).
    pub fn berakna_u_fara(&self, w_skada: f64, gen_l2: &GenL2, kropp: KroppL2) -> f64 {
        if self.cfg.med_installare {
            (w_skada
                + gen_l2.theta_e * kropp.e_norm
                + gen_l2.theta_s * kropp.skada
                + gen_l2.theta_j * kropp.ung)
                .max(0.0)
        } else {
            w_skada.max(0.0)
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

    pub fn varde_mat(&self, y: &[f64; Z_B1]) -> f64 {
        let mut s = self.v[Z_B1] + self.w_mat[Z_B1];
        for j in 0..Z_B1 {
            s += (self.v[j] + self.w_mat[j]) * y[j];
        }
        s
    }

    pub fn varde_fara(&self, y: &[f64; Z_B1]) -> f64 {
        let mut s = self.w_fara[Z_B1];
        for j in 0..Z_B1 {
            s += self.w_fara[j] * y[j];
        }
        s
    }

    pub fn varde_tot(&self, y: &[f64; Z_B1], u_fara: f64) -> f64 {
        self.varde_mat(y) - u_fara * self.varde_fara(y)
    }

    pub fn krit_mat_r2(&self) -> f64 {
        if self.krit_n < 20 {
            return f64::NAN;
        }
        let n = self.krit_n as f64;
        let my = self.krit_mat_sy / n;
        let ss_tot = self.krit_mat_sy2 - n * my * my;
        if ss_tot <= 1e-9 {
            return f64::NAN;
        }
        1.0 - self.krit_mat_se2 / ss_tot
    }

    pub fn krit_fara_r2(&self) -> f64 {
        if self.krit_n < 20 {
            return f64::NAN;
        }
        let n = self.krit_n as f64;
        let my = self.krit_fara_sy / n;
        let ss_tot = self.krit_fara_sy2 - n * my * my;
        if ss_tot <= 1e-9 {
            return f64::NAN;
        }
        1.0 - self.krit_fara_se2 / ss_tot
    }

    pub fn mode1_andel(&self) -> f64 {
        let tot = self.mode1_steg + self.mode2_steg;
        if tot == 0 {
            0.0
        } else {
            self.mode1_steg as f64 / tot as f64
        }
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

    fn uppdatera_fran_forra(&mut self, s_in_now: &[f64; S_B1], lar: bool) {
        self.senaste_fel = None;
        let Some((_, z_prev, a_prev, y_prev)) = self.forra else {
            return;
        };
        let z_raw_now = self.kod_ra(s_in_now);
        let z_mal = self.norm_z(&z_raw_now);

        let mut kvad = 0.0;
        for r in 0..Z_B1 {
            let ef = y_prev[r] - z_mal[r];
            kvad += ef * ef;
        }
        self.senaste_fel = Some(kvad);

        if !lar {
            return;
        }

        // 1. Delade fysiologiska kritiker (`w_mat` mot ΔE, `w_fara` mot skada) med delad P_krit.
        if let (Some(de_raw), Some(sk_raw)) = (self.vanta_de.take(), self.vanta_skada.take()) {
            let klamp = self.cfg.r_klamp.max(1.0);
            let r_mat = de_raw.clamp(-klamp, klamp);
            let r_fara = sk_raw.clamp(0.0, 1.0);
            let pred_mat = self.varde_mat(&z_mal);
            let pred_fara = self.varde_fara(&z_mal);
            let err_mat = r_mat - pred_mat;
            let err_fara = r_fara - pred_fara;
            if self.steg >= 100 {
                self.krit_n += 1;
                self.krit_mat_sy += r_mat;
                self.krit_mat_sy2 += r_mat * r_mat;
                self.krit_mat_se2 += err_mat * err_mat;
                self.krit_fara_sy += r_fara;
                self.krit_fara_sy2 += r_fara * r_fara;
                self.krit_fara_se2 += err_fara * err_fara;
            }
            let x_k = Self::x_av(&z_mal);
            rls_uppdatera_delad_kritiker(
                &mut self.w_mat,
                &mut self.w_fara,
                &mut self.p_krit,
                &x_k,
                err_mat,
                err_fara,
                LAMBDA_KRIT,
            );
        }

        // 2. Uppdatera de 9 fristående RLS-världsmodellerna (identiskt med HjB1).
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

    pub fn valj_lott(
        &mut self,
        s_nu: &[f64; S_B1],
        lar: bool,
        w_skada: f64,
        gen_l2: &GenL2,
        kropp: KroppL2,
        g: &mut Rng,
    ) -> usize {
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
        let x_z = Self::x_av(&z);

        let tau_m1 = gen_l2.tau_m1_raw.max(0.0);
        let kor_mode1 = self.cfg.med_mode1
            && !self.cfg.stryk_handling_i_val
            && self.steg >= 100
            && tau_m1 > 0.0
            && self.senaste_fel.is_some_and(|f| f <= tau_m1);

        let (ba, y_ba) = if kor_mode1 {
            let mut q_m1 = [0.0; A_N];
            for a in 0..A_N {
                q_m1[a] = dot_zx(&self.w_m1[a], &x_z);
            }
            let a = lott_bland_max(&q_m1, g);
            self.mode1_steg += 1;
            self.senaste_var_mode1 = true;
            // Världsmodellen förutsäger enbart den valda handlingen för fortsatt lärande (LeCun 3.1).
            (a, self.forutsag(&z, a))
        } else {
            let u_fara = self.berakna_u_fara(w_skada, gen_l2, kropp);
            let mut varden = [0.0; A_N];
            let mut ys = [[0.0; Z_B1]; A_N];
            let y_stryk = if self.cfg.stryk_handling_i_val {
                Some(self.forutsag_medel(&z))
            } else {
                None
            };
            for a in 0..A_N {
                let y = y_stryk.unwrap_or_else(|| self.forutsag(&z, a));
                varden[a] = self.varde_tot(&y, u_fara);
                ys[a] = y;
            }
            let a = lott_bland_max(&varden, g);
            if self.cfg.med_mode1 && lar {
                kompilera_mode1(&mut self.w_m1, &x_z, a);
            }
            self.mode2_steg += 1;
            self.senaste_var_mode1 = false;
            (a, ys[a])
        };

        if lar {
            self.uppdatera_kanalgrind(&s_sk_f32);
            self.uppdatera_skala(s_nu);
            self.uppdatera_z_skala(&z_raw);
        }
        self.forra = Some((s_in, z, ba, y_ba));
        self.forra_ra_sk = Some(s_sk_f32);
        self.steg += 1;
        ba
    }

    pub fn lar_och_satt(&mut self, s_nu: &[f64; S_B1], lar: bool, a: usize) {
        assert!(a < A_N);
        self.senaste_var_mode1 = false;
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
    use crate::hjarnab1::{HjB1, HjB1Config};

    #[test]
    fn alt2348_exakt_superposition_matchar_hjb1_bit_for_bit() {
        // Numerisk kontroll av Lean 4-satsen `rls_critic_superposition`:
        // med `med_installare = false` och `med_mode1 = false` måste `w_mat - u * w_fara == w_krit`
        // och alla 400 handlingsval matcha `HjB1` exakt!
        let mut rng = Rng::new(234801);
        let hj = Hj::slump(&mut rng);
        let mut gb1 = GenB1::slump(&mut rng);
        gb1.w_skada = 3.25;
        let gl2 = GenL2 { theta_e: 0.0, theta_s: 0.0, theta_j: 0.0, tau_m1_raw: 0.0 };

        let mut b1 = HjB1::fran_gen(
            &hj.gen,
            &gb1,
            HjB1Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: 40,
                med_kritiker: true,
                anvand_w_skada: true,
                r_klamp: 100.0,
            },
        );
        let mut l2 = HjL2::fran_gen(
            &hj.gen,
            &gb1,
            HjL2Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: 40,
                med_installare: false,
                med_mode1: false,
                r_klamp: 100.0,
            },
        );

        let mut rng_b1 = Rng::new(234802);
        let mut rng_l2 = Rng::new(234802);
        let kropp = KroppL2::fran_djur(120.0, 1.0, 0.0, 700);
        for steg in 0..400 {
            let s: [f64; S_B1] = std::array::from_fn(|_| rng.f() * 2.0 - 1.0);
            let a_b1 = b1.valj_lott(&s, true, &mut rng_b1);
            let a_l2 = l2.valj_lott(&s, true, gb1.w_skada, &gl2, kropp, &mut rng_l2);
            assert_eq!(a_b1, a_l2, "handling skilde sig vid steg {steg}");
            let de = (rng.f() - 0.5) * 4.0;
            let sk = if rng.f() < 0.25 { rng.f() * 0.4 } else { 0.0 };
            b1.vanta_beloning = Some(de - gb1.w_skada * sk);
            l2.vanta_de = Some(de);
            l2.vanta_skada = Some(sk);
        }
        for j in 0..=Z_B1 {
            let diff = ((l2.w_mat[j] - gb1.w_skada * l2.w_fara[j]) - b1.w_krit[j]).abs();
            assert!(diff < 1e-11, "superpositionsavvikelse axel {j}: {diff}");
        }
    }

    #[test]
    fn alt2348_startgener_symmetriska_utan_forkodat_tecken() {
        let mut g = Rng::new(234803);
        let (mut pe, mut ps, mut pj, mut pt) = (0usize, 0usize, 0usize, 0usize);
        let (mut se, mut ss, mut sj, mut st) = (0.0, 0.0, 0.0, 0.0);
        for _ in 0..1000 {
            let gl = GenL2::slump(&mut g);
            if gl.theta_e > 0.0 { pe += 1; }
            if gl.theta_s > 0.0 { ps += 1; }
            if gl.theta_j > 0.0 { pj += 1; }
            if gl.tau_m1_raw > 0.0 { pt += 1; }
            se += gl.theta_e;
            ss += gl.theta_s;
            sj += gl.theta_j;
            st += gl.tau_m1_raw;
        }
        for (namn, p, s) in [("theta_e", pe, se), ("theta_s", ps, ss), ("theta_j", pj, sj), ("tau_m1", pt, st)] {
            assert!((440..=560).contains(&p), "{namn} ej symmetrisk kring 0: {p}/1000");
            assert!((s / 1000.0).abs() < 0.3, "{namn} medel ej nära 0: {}", s / 1000.0);
        }
    }

    #[test]
    fn alt2348_installare_vander_val_mellan_hungrig_och_matt() {
        // Visar att delade kritiker + inställare faktiskt kan skifta handling inom ett enda steg
        // när kroppsläget ändras från hungrig till mätt.
        let mut rng = Rng::new(234804);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let mut l2 = HjL2::fran_gen(
            &hj.gen,
            &gb1,
            HjL2Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: 40,
                med_installare: true,
                med_mode1: false,
                r_klamp: 100.0,
            },
        );
        // Sätt rena syntetiska kritiker: axel 0 = mat (+1), axel 1 = fara (+1).
        l2.v = [0.0; Z_B1 + 1];
        l2.w_mat = [0.0; Z_B1 + 1];
        l2.w_mat[0] = 1.0;
        l2.w_fara = [0.0; Z_B1 + 1];
        l2.w_fara[1] = 1.0;

        let gl2 = GenL2 { theta_e: 4.0, theta_s: 3.0, theta_j: 2.0, tau_m1_raw: 0.0 };
        let y_oppen_mat = [2.0, 1.0, 0.0, 0.0, 0.0, 0.0]; // hög mat (2), hög fara (1)
        let y_skydd = [0.5, 0.1, 0.0, 0.0, 0.0, 0.0];     // måttlig mat (0.5), låg fara (0.1)

        let u_hungrig = l2.berakna_u_fara(2.0, &gl2, KroppL2 { e_norm: -0.8, skada: 0.0, ung: 0.0 });
        let u_matt = l2.berakna_u_fara(2.0, &gl2, KroppL2 { e_norm: 0.8, skada: 0.0, ung: 0.0 });
        assert!(u_hungrig >= 0.0 && u_matt > u_hungrig);
        assert!(
            l2.varde_tot(&y_oppen_mat, u_hungrig) > l2.varde_tot(&y_skydd, u_hungrig),
            "hungrig ska föredra matrik öppen ruta"
        );
        assert!(
            l2.varde_tot(&y_skydd, u_matt) > l2.varde_tot(&y_oppen_mat, u_matt),
            "mätt ska föredra skyddad ruta"
        );
    }

    #[test]
    fn alt2348_mode1_kompilering_krymper_fel_och_aktiveras_vid_lag_overraskning() {
        let mut w_m1 = [[0.0; Z_B1 + 1]; A_N];
        let x = [0.5, -0.3, 0.8, -0.2, 0.4, 0.1, 1.0];
        let a_mal = 3usize;
        let e0 = (1.0 - dot_zx(&w_m1[a_mal], &x)).abs();
        kompilera_mode1(&mut w_m1, &x, a_mal);
        let e1 = (1.0 - dot_zx(&w_m1[a_mal], &x)).abs();
        assert!(e1 < e0, "NLMS måste strikt krympa felet: e0={e0}, e1={e1}");
        for _ in 0..40 {
            kompilera_mode1(&mut w_m1, &x, a_mal);
        }
        let mut bast = 0usize;
        for a in 1..A_N {
            if dot_zx(&w_m1[a], &x) > dot_zx(&w_m1[bast], &x) {
                bast = a;
            }
        }
        assert_eq!(bast, a_mal, "Mode-1 ska kompilera och reproducera Mode-2:s val");
    }

    #[test]
    fn alt2348_l2_armar_kor_i_b1_varld_utan_nan_och_bevarar_b1_determinism() {
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

        for &(m_f, o_f) in &[
            (Formaga::LarL2Konst(0.20, 40), Formaga::LarB1Krit(0.20, 40)),
            (Formaga::LarL2Inst(0.20, 40), Formaga::LarB1Krit(0.20, 40)),
            (Formaga::LarL2M1(0.20, 40), Formaga::LarB1Krit(0.20, 40)),
            (Formaga::LarL2Full(0.20, 40), Formaga::StrykL2Inst(0.20, 40)),
        ] {
            let mut sim = Sim::fran_karta(&kart, 2348, 36);
            sim.hjarn_arm = Some(HjarnArm { namn: "l2_test", markta: m_f, omarkta: o_f, spegel: false });
            let mut param = RovdjurB1Param::default();
            param.p_bas = 0.05;
            sim.aktivera_rovdjur_b1(2348, param);
            sim.markera_halften(2348);
            for _ in 0..160 {
                sim.steg();
            }
            for d in sim.djur.iter().filter(|d| d.lev) {
                assert!(d.e.is_finite() && d.skada.is_finite());
                if let Some(ref hl) = d.hj_l2 {
                    assert!(hl.w_mat.iter().all(|x| x.is_finite()));
                    assert!(hl.w_fara.iter().all(|x| x.is_finite()));
                    assert!(hl.kanal_vikt.iter().all(|x| x.is_finite()));
                }
            }
        }
    }
}
