//! Alt 2326 / Alt 2312 (Steg L1 i LeCun-planen `Alt 2285`, slutritning v2):
//! Korttidsminne med råsinnen och omkodning (Del L1-A), exakt tvåstegsplanering (Del L1-B),
//! osäkerhet ur RLS-kovariansen `P_a` med `q`-räknare före klampning (Del L1-C),
//! samt helt läsande skuggläge för mätning av S1–S5 på oförändrad `LarB1Krit`.
//! Håller `hjarna.rs`, `hjarna2008.rs` och `hjarnab1.rs` helt orörda.

use serde::{Deserialize, Serialize};

use crate::hjarna::{gauss, lott_bland_max, NK, NP};
use crate::hjarnab1::{GenB1, S_B1, Z_B1};
use crate::rng::Rng;
use crate::tal::{A_N, S_N, Z_N};

/// Antal minnesposter i korttidsminnets cirkulära buffert (Hippocampus, Del L1-A).
pub const M_L1: usize = 64;
/// Standard steglängd för dämpad normaliserad kritiker-återuppspelning (låst: `η ≤ 0,5`, rör aldrig `P_krit`).
pub const ETA_REPLAY: f64 = 0.35;
/// Standard diskonteringsfaktor för tvåstegsplanering (`T = 2`, Del L1-B).
pub const GAMMA_L1: f64 = 0.70;

/// Fast salt för `replay_rng`-fröblandning med världens frö och djurets `uid` (ritning v2 avsnitt 3).
pub const REPLAY_SALT: u64 = 0x2326_4E91_9E37_79B9;

/// Bitflaggor för att slå på/av L1:s delmekanismer och kontrollarmar K-a..K-d (ritning v2 avsnitt 4 & 6).
pub const L1_REPLAY: u8 = 1;
pub const L1_PLAN2: u8 = 2;
pub const L1_OSAK: u8 = 4;
pub const L1_FULL: u8 = L1_REPLAY | L1_PLAN2 | L1_OSAK;
/// Kontrollarm K-a: kritikern tränas mot `r_t + γ · r_{t+1}` (fördröjd ett steg, utan utrullning).
pub const L1_K_A_TVASTEGS_MAL: u8 = 8;
/// Kontrollarm K-b: två extra projektionssteg på nutidsposten (formeln i avsnitt 3), utan minne.
pub const L1_K_B_EXTRA_UPPD: u8 = 16;
/// Kontrollarm K-c (ihop med `L1_REPLAY`): två likformiga dragningar ur minnet i stället för topp-av-8.
pub const L1_K_C_LIKFORMIG: u8 = 32;
/// Kontrollarm K-d: egenvärdesklampning av `P_a` i `[ε, 1/α]` med `w_osak` av.
pub const L1_STABIL_PA: u8 = 64;

/// Minsta tillåtna egenvärde i stabiliserad `P_a` (ritning v2 avsnitt 4, eget val).
pub const P_A_EGEN_MIN: f64 = 1e-9;
/// Fast antal cykliska Jacobi-svep på den symmetriska 7×7-matrisen `P_a` (deterministisk ordning).
const JACOBI_SVEP: usize = 6;

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

/// Fast SplitMix64-blandning så att `replay_rng` skiljer sig mellan världar även vid samma `uid`.
pub fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Deterministiskt frö för djurets egna återspelningsström blandat av världens frö, `uid` och fast salt.
pub fn blanda_replay_fro(varld_fro: u64, uid: u64) -> u64 {
    splitmix64(splitmix64(varld_fro ^ REPLAY_SALT) ^ uid.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// Anger om tvåstegsföljden `(a1, a2)` slutar inom djurets nio sinnesrutor (`GRANNAR`, 49 av 81 par).
pub fn tva_steg_inom_grannar(a1: usize, a2: usize) -> bool {
    let (di1, dj1) = crate::sim::GRANNAR[a1];
    let (di2, dj2) = crate::sim::GRANNAR[a2];
    (di1 + di2).abs() <= 1 && (dj1 + dj2).abs() <= 1
}

/// Ärvd L1-tilläggsgen utöver `GenB1`: vikt för hur lite handlingen har prövats i riktningen (`> 0` försiktig, `< 0` nyfiken).
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
    pub eta_replay: f64,
    pub plan_steg: u8,
    pub gamma: f64,
    pub anvand_w_osak: bool,
    pub stabil_p_a: bool,
    pub k_a_tvastegs_mal: bool,
    pub k_b_extra_uppd: bool,
    pub likformig_replay: bool,
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
            eta_replay: ETA_REPLAY,
            plan_steg: if (mask & L1_PLAN2) != 0 { 2 } else { 1 },
            gamma: GAMMA_L1,
            anvand_w_osak: (mask & L1_OSAK) != 0,
            stabil_p_a: (mask & (L1_OSAK | L1_STABIL_PA)) != 0,
            k_a_tvastegs_mal: (mask & L1_K_A_TVASTEGS_MAL) != 0,
            k_b_extra_uppd: (mask & L1_K_B_EXTRA_UPPD) != 0,
            likformig_replay: (mask & L1_K_C_LIKFORMIG) != 0,
        }
    }
}

/// Minnespost i version 2 (124 byte/post, 7 936 byte/djur vid M = 64):
/// sparar råa sinnen vid `t+1` för omkodning i dagens koordinater och redo för L4.
#[derive(Clone, Copy, Debug)]
pub struct MinnePostL1 {
    pub s_ra: [f32; S_B1],
    pub r: f32,
    pub err_abs: f32,
    pub a_prev: u8,
    pub foljd: u8,
}

impl Default for MinnePostL1 {
    fn default() -> Self {
        Self {
            s_ra: [0.0; S_B1],
            r: 0.0,
            err_abs: 0.0,
            a_prev: 0,
            foljd: 0,
        }
    }
}

/// Räknare för `q = xᵀ P_a x` före klampning (avsnitt 2 S4 och avsnitt 4 punkt 1 i ritning v2).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OsakQStat {
    pub n_anrop: u64,
    pub n_neg: u64,
    pub n_over100: u64,
    pub min_q: f64,
}

impl Default for OsakQStat {
    fn default() -> Self {
        Self {
            n_anrop: 0,
            n_neg: 0,
            n_over100: 0,
            min_q: f64::INFINITY,
        }
    }
}

impl OsakQStat {
    pub fn registrera(&mut self, q: f64) {
        self.n_anrop += 1;
        if q < 0.0 {
            self.n_neg += 1;
        }
        if q > 100.0 {
            self.n_over100 += 1;
        }
        if q < self.min_q {
            self.min_q = q;
        }
    }

    pub fn lagg(&mut self, o: &Self) {
        self.n_anrop += o.n_anrop;
        self.n_neg += o.n_neg;
        self.n_over100 += o.n_over100;
        if o.min_q < self.min_q {
            self.min_q = o.min_q;
        }
    }
}

/// Finkornigt histogram för snabb och minnessnål median av S5-driften `D` (stegbredd 0,00005 i [0, 2)).
const DRIFT_BINS: usize = 40_000;
const DRIFT_MAX: f64 = 2.0;

#[derive(Clone, Debug)]
pub struct DriftHist {
    pub bins: Vec<u64>,
    pub overflow: Vec<f64>,
    pub n: u64,
}

impl Default for DriftHist {
    fn default() -> Self {
        Self {
            bins: vec![0; DRIFT_BINS],
            overflow: Vec::new(),
            n: 0,
        }
    }
}

impl DriftHist {
    pub fn lagg(&mut self, d: f64) {
        if !d.is_finite() || d < 0.0 {
            return;
        }
        self.n += 1;
        if d < DRIFT_MAX {
            let idx = ((d / DRIFT_MAX) * DRIFT_BINS as f64) as usize;
            self.bins[idx.min(DRIFT_BINS - 1)] += 1;
        } else {
            self.overflow.push(d);
        }
    }

    pub fn median(&self) -> f64 {
        if self.n == 0 {
            return f64::NAN;
        }
        let target = self.n / 2;
        let mut ack = 0u64;
        let bredd = DRIFT_MAX / DRIFT_BINS as f64;
        for (i, &c) in self.bins.iter().enumerate() {
            ack += c;
            if ack > target {
                return (i as f64 + 0.5) * bredd;
            }
        }
        let mut ov = self.overflow.clone();
        ov.sort_by(|a, b| a.total_cmp(b));
        let k = (target.saturating_sub(self.n.saturating_sub(ov.len() as u64))) as usize;
        ov[k.min(ov.len().saturating_sub(1))]
    }
}

/// Sparad post för S5-skuggan vid steg `t` (jämförs vid `t + 64`).
#[derive(Clone, Copy, Debug, Default)]
pub struct S5Post {
    pub s_ra: [f32; S_B1],
    pub z_raw: [f32; Z_B1],
    pub z_norm: [f32; Z_B1],
}

/// Per-djur läsande skuggtillstånd (allokeras enbart när `Sim::skugga_l1` är på).
#[derive(Clone, Debug)]
pub struct SkuggaDjurL1 {
    /// Hjärnsteget (`hjl.steg`) vid djurets senaste angrepp (`senaste_skada > 0`).
    pub senaste_angrepp_steg: Option<u64>,
    /// Väntande tvåstegsprognos från `t+1` att mäta mot `z_mal` vid `t+2`: `(z_t, y2_pred, inom_grannar, forvantat_steg)`.
    pub vanta_s2: Option<([f64; Z_B1], [f64; Z_B1], bool, u64)>,
    /// Cirkulär buffert om exakt 64 steg för S5 (kodningens drift mellan `t` och `t+64`).
    pub s5_ring: Vec<S5Post>,
    pub s5_pos: usize,
    pub s5_len: usize,
}

impl SkuggaDjurL1 {
    pub fn ny() -> Self {
        Self {
            senaste_angrepp_steg: None,
            vanta_s2: None,
            s5_ring: vec![S5Post::default(); M_L1],
            s5_pos: 0,
            s5_len: 0,
        }
    }
}

/// Samlade skuggmått S1–S5 per värld (läser bara, ändrar aldrig djurens tillstånd eller slumpströmmar).
#[derive(Clone, Debug)]
pub struct SkuggaL1Ack {
    pub mat_aktiv: bool,
    /// Endast för rödprovet i avsnitt 2 (simulerar en trasig skugga som drar ett tal ur `egen_rng`). Alltid `false` i drift.
    pub trasig_rng_lacka_for_prov: bool,
    // S1: glömska vid angrepp (g <= 16, 16 < g <= 64, g > 64)
    pub s1_kort_sum: f64,
    pub s1_kort_n: u64,
    pub s1_mellan_sum: f64,
    pub s1_mellan_n: u64,
    pub s1_lang_sum: f64,
    pub s1_lang_n: u64,
    // S2: tvåstegsprognos (totalt, inom 49 grannpar, utanför 32 par)
    pub s2_tot_fel: f64,
    pub s2_tot_kop: f64,
    pub s2_tot_n: u64,
    pub s2_inom_fel: f64,
    pub s2_inom_kop: f64,
    pub s2_inom_n: u64,
    pub s2_utan_fel: f64,
    pub s2_utan_kop: f64,
    pub s2_utan_n: u64,
    // S3: annat val mellan T=1 och T=2 (ingen gemensam bästa handling)
    pub s3_annat: u64,
    pub s3_totalt: u64,
    // S4: q = xᵀ P_a x före klampning + andel val där mest prövad handling har större u än minst prövad
    pub s4_q: OsakQStat,
    pub s4_mest_storre_u: u64,
    pub s4_val_totalt: u64,
    // S5: kodningens drift mellan t och t+64 (full omkodning och bara ny norm_z)
    pub s5_full: DriftHist,
    pub s5_bara_norm: DriftHist,
}

impl SkuggaL1Ack {
    pub fn ny(mat_aktiv: bool) -> Self {
        Self {
            mat_aktiv,
            trasig_rng_lacka_for_prov: false,
            s1_kort_sum: 0.0,
            s1_kort_n: 0,
            s1_mellan_sum: 0.0,
            s1_mellan_n: 0,
            s1_lang_sum: 0.0,
            s1_lang_n: 0,
            s2_tot_fel: 0.0,
            s2_tot_kop: 0.0,
            s2_tot_n: 0,
            s2_inom_fel: 0.0,
            s2_inom_kop: 0.0,
            s2_inom_n: 0,
            s2_utan_fel: 0.0,
            s2_utan_kop: 0.0,
            s2_utan_n: 0,
            s3_annat: 0,
            s3_totalt: 0,
            s4_q: OsakQStat::default(),
            s4_mest_storre_u: 0,
            s4_val_totalt: 0,
            s5_full: DriftHist::default(),
            s5_bara_norm: DriftHist::default(),
        }
    }

    pub fn s1_kvot(&self) -> Option<f64> {
        if self.s1_kort_n < 200 || self.s1_mellan_n < 200 {
            return None;
        }
        let mk = self.s1_kort_sum / self.s1_kort_n as f64;
        if mk <= 1e-18 {
            return None;
        }
        let mm = self.s1_mellan_sum / self.s1_mellan_n as f64;
        Some(mm / mk)
    }

    pub fn s1_lang_kvot(&self) -> Option<f64> {
        if self.s1_kort_n < 200 || self.s1_lang_n < 200 {
            return None;
        }
        let mk = self.s1_kort_sum / self.s1_kort_n as f64;
        if mk <= 1e-18 {
            return None;
        }
        let ml = self.s1_lang_sum / self.s1_lang_n as f64;
        Some(ml / mk)
    }

    pub fn s2_tot(&self) -> f64 {
        if self.s2_tot_kop <= 1e-18 {
            f64::NAN
        } else {
            1.0 - self.s2_tot_fel / self.s2_tot_kop
        }
    }

    pub fn s2_inom(&self) -> f64 {
        if self.s2_inom_kop <= 1e-18 {
            f64::NAN
        } else {
            1.0 - self.s2_inom_fel / self.s2_inom_kop
        }
    }

    pub fn s2_utan(&self) -> f64 {
        if self.s2_utan_kop <= 1e-18 {
            f64::NAN
        } else {
            1.0 - self.s2_utan_fel / self.s2_utan_kop
        }
    }

    pub fn s3_andel(&self) -> f64 {
        if self.s3_totalt == 0 {
            f64::NAN
        } else {
            self.s3_annat as f64 / self.s3_totalt as f64
        }
    }

    pub fn s4_mest_storre_andel(&self) -> f64 {
        if self.s4_val_totalt == 0 {
            f64::NAN
        } else {
            self.s4_mest_storre_u as f64 / self.s4_val_totalt as f64
        }
    }
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
    /// Del L1-A: cirkulär buffert om 64 poster med råsinnen (124 B/post = 7 936 B/djur).
    pub buffert: Vec<MinnePostL1>,
    pub buf_pos: usize,
    pub buf_len: usize,
    pub senaste_post_steg: Option<u64>,
    pub replay_rng: Rng,
    /// Räknare för `q = xᵀ P_a x` före klampning vid osäkerhetsanrop i drift.
    pub q_stat: OsakQStat,
    /// Kontrollarm K-a: väntande `(z_mal, r_t)` för ett stegs fördröjd träning mot `r_t + γ · r_{t+1}`.
    pub vanta_ka: Option<([f64; Z_B1], f64)>,
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

/// Rå kvadratisk form `q = xᵀ P x` före klampning.
pub fn xpx_rls(p: &[[f64; Z_B1 + 1]; Z_B1 + 1], x: &[f64; Z_B1 + 1]) -> f64 {
    let mut xpx = 0.0;
    for m in 0..=Z_B1 {
        let mut s = 0.0;
        for n in 0..=Z_B1 {
            s += p[m][n] * x[n];
        }
        xpx += x[m] * s;
    }
    xpx
}

/// Mått på hur lite handlingen har prövats i den här riktningen: `sqrt((xᵀ P x).clamp(0, 100))`.
pub fn osakerhet_rls(p: &[[f64; Z_B1 + 1]; Z_B1 + 1], x: &[f64; Z_B1 + 1]) -> f64 {
    xpx_rls(p, x).clamp(0.0, 100.0).sqrt()
}

/// Klampning av egenvärdena i den symmetriska 7×7-matrisen `P` till `[P_A_EGEN_MIN, max_egen]`
/// med deterministisk cyklisk Jacobi-metod (fast ordning, fast antal svep; ritning v2 avsnitt 4).
pub fn klampa_egenvarden_p(p: &mut [[f64; Z_B1 + 1]; Z_B1 + 1], max_egen: f64) {
    const N: usize = Z_B1 + 1;
    let mut a = [[0.0f64; N]; N];
    for i in 0..N {
        a[i][i] = p[i][i];
        for j in (i + 1)..N {
            let sym = 0.5 * (p[i][j] + p[j][i]);
            a[i][j] = sym;
            a[j][i] = sym;
        }
    }
    let mut v = [[0.0f64; N]; N];
    for i in 0..N {
        v[i][i] = 1.0;
    }

    for _ in 0..JACOBI_SVEP {
        for ip in 0..N {
            for iq in (ip + 1)..N {
                let apq = a[ip][iq];
                if apq.abs() <= 1e-15 {
                    continue;
                }
                let diff = a[iq][iq] - a[ip][ip];
                let t = if apq.abs() < 1e-100 * diff.abs() {
                    apq / diff
                } else {
                    let tau = diff / (2.0 * apq);
                    let sign = if tau >= 0.0 { 1.0 } else { -1.0 };
                    sign / (tau.abs() + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                let h = t * apq;
                a[ip][ip] -= h;
                a[iq][iq] += h;
                a[ip][iq] = 0.0;
                a[iq][ip] = 0.0;
                for r in 0..N {
                    if r != ip && r != iq {
                        let arp = a[r][ip];
                        let arq = a[r][iq];
                        let np = c * arp - s * arq;
                        let nq = s * arp + c * arq;
                        a[r][ip] = np;
                        a[ip][r] = np;
                        a[r][iq] = nq;
                        a[iq][r] = nq;
                    }
                    let vrp = v[r][ip];
                    let vrq = v[r][iq];
                    v[r][ip] = c * vrp - s * vrq;
                    v[r][iq] = s * vrp + c * vrq;
                }
            }
        }
    }

    let mut d = [0.0f64; N];
    for k in 0..N {
        d[k] = a[k][k].clamp(P_A_EGEN_MIN, max_egen);
    }
    for i in 0..N {
        for j in i..N {
            let mut sum = 0.0;
            for k in 0..N {
                sum += v[i][k] * d[k] * v[j][k];
            }
            p[i][j] = sum;
            p[j][i] = sum;
        }
    }
}

fn rls_uppdatera(
    w: &mut [[f64; Z_B1 + 1]; Z_B1],
    p: &mut [[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    target: &[f64; Z_B1],
    pred: &[f64; Z_B1],
    lambda: f64,
    stabil_max_egen: Option<f64>,
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
    if let Some(max_egen) = stabil_max_egen {
        klampa_egenvarden_p(p, max_egen);
    } else {
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

/// Dämpad normaliserad projektionsuppdatering för återspelning (`η ≤ 0,5`, lämnar `P_krit` orörd).
fn replay_uppdatera_skalar(
    w: &mut [f64; Z_B1 + 1],
    p: &[[f64; Z_B1 + 1]; Z_B1 + 1],
    x: &[f64; Z_B1 + 1],
    err: f64,
    eta: f64,
) {
    assert!((0.0..=0.5).contains(&eta), "eta_replay måste vara i [0, 0.5], fick {eta}");
    if eta == 0.0 {
        return;
    }
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

/// Helt läsande skugghook före `uppdatera_fran_forra`: mäter S1, S2-utfall och S5 utan att ändra `HjL1`.
pub fn skugga_fore_uppdatering(
    hjl: &HjL1,
    s_nu: &[f64; S_B1],
    r_raw: Option<f64>,
    var_angrepp: bool,
    djur: &mut SkuggaDjurL1,
    ack: &mut SkuggaL1Ack,
) {
    if !hjl.skala_init || !hjl.z_skala_init {
        return;
    }
    let s_sk = hjl.skala(s_nu);
    let mut s_in = [0.0; S_B1];
    for j in 0..S_B1 {
        s_in[j] = hjl.kanal_vikt[j] * s_sk[j];
    }
    let z_raw_now = hjl.kod_ra(&s_in);
    let z_mal = hjl.norm_z(&z_raw_now);

    // S1: kritikerns fel i kvadrat före uppdateringen vid angrepp (`senaste_skada > 0`), uppdelat på avstånd g.
    if var_angrepp && hjl.forra.is_some() {
        if let Some(rv) = r_raw {
            let klamp = hjl.cfg.r_klamp.max(1.0);
            let r = rv.clamp(-klamp, klamp);
            let pred_k = hjl.varde(&z_mal);
            let err_k = r - pred_k;
            if let Some(prev_steg) = djur.senaste_angrepp_steg {
                if ack.mat_aktiv && hjl.steg >= 100 {
                    let g = hjl.steg.saturating_sub(prev_steg);
                    let e2 = err_k * err_k;
                    if g <= 16 {
                        ack.s1_kort_sum += e2;
                        ack.s1_kort_n += 1;
                    } else if g <= 64 {
                        ack.s1_mellan_sum += e2;
                        ack.s1_mellan_n += 1;
                    } else {
                        ack.s1_lang_sum += e2;
                        ack.s1_lang_n += 1;
                    }
                }
            }
            djur.senaste_angrepp_steg = Some(hjl.steg);
        }
    }

    // S2: jämför tvåstegsprognosen `ŷ₂` från steg t+1 med `z_mal` vid t+2 och kopiering `z_t`.
    if let Some((z_t, y2_pred, inom, forvantat_steg)) = djur.vanta_s2.take() {
        if forvantat_steg == hjl.steg && ack.mat_aktiv && hjl.steg >= 100 {
            let mut kvad_y2 = 0.0;
            let mut kvad_kop = 0.0;
            for r in 0..Z_B1 {
                let ef = y2_pred[r] - z_mal[r];
                kvad_y2 += ef * ef;
                let ek = z_t[r] - z_mal[r];
                kvad_kop += ek * ek;
            }
            ack.s2_tot_fel += kvad_y2;
            ack.s2_tot_kop += kvad_kop;
            ack.s2_tot_n += 1;
            if inom {
                ack.s2_inom_fel += kvad_y2;
                ack.s2_inom_kop += kvad_kop;
                ack.s2_inom_n += 1;
            } else {
                ack.s2_utan_fel += kvad_y2;
                ack.s2_utan_kop += kvad_kop;
                ack.s2_utan_n += 1;
            }
        }
    }

    // S5: kodningens drift mellan steg t-64 och t (full omkodning samt bara ny norm_z).
    if djur.s5_len == M_L1 && ack.mat_aktiv && hjl.steg >= 100 {
        let old = djur.s5_ring[djur.s5_pos];
        let z_ny = hjl.koda_om_ra(&old.s_ra);
        let z_raw_old: [f64; Z_B1] = std::array::from_fn(|i| old.z_raw[i] as f64);
        let z_ny_bara_norm = hjl.norm_z(&z_raw_old);
        let mut norm2_g = 0.0;
        let mut diff2_full = 0.0;
        let mut diff2_norm = 0.0;
        for i in 0..Z_B1 {
            let zg = old.z_norm[i] as f64;
            norm2_g += zg * zg;
            let df = z_ny[i] - zg;
            diff2_full += df * df;
            let dn = z_ny_bara_norm[i] - zg;
            diff2_norm += dn * dn;
        }
        if norm2_g > 1e-12 {
            let norm_g = norm2_g.sqrt();
            ack.s5_full.lagg(diff2_full.sqrt() / norm_g);
            ack.s5_bara_norm.lagg(diff2_norm.sqrt() / norm_g);
        }
    }
    djur.s5_ring[djur.s5_pos] = S5Post {
        s_ra: std::array::from_fn(|j| s_nu[j] as f32),
        z_raw: std::array::from_fn(|i| z_raw_now[i] as f32),
        z_norm: std::array::from_fn(|i| z_mal[i] as f32),
    };
    djur.s5_pos = (djur.s5_pos + 1) % M_L1;
    if djur.s5_len < M_L1 {
        djur.s5_len += 1;
    }
}

/// Helt läsande skugghook efter `uppdatera_fran_forra` och val (före `uppdatera_skala`):
/// förbereder S2 för t+2 samt mäter S3 och S4 vid val utan utforskning. Drar aldrig slumptal.
pub fn skugga_vid_val(
    hjl: &HjL1,
    z: &[f64; Z_B1],
    ys: &[[f64; Z_B1]; A_N],
    a_vald: usize,
    var_girig: bool,
    forra_for_s2: Option<([f64; Z_B1], usize, [f64; Z_B1])>,
    djur: &mut SkuggaDjurL1,
    ack: &mut SkuggaL1Ack,
) {
    // Förbered S2-prognosen `ŷ₂ = forutsag(ŷ₁, a_{t+1})` för nästa steg t+2.
    if let Some((z_t, a_t, y1_t)) = forra_for_s2 {
        let y2 = hjl.forutsag(&y1_t, a_vald);
        let inom = tva_steg_inom_grannar(a_t, a_vald);
        djur.vanta_s2 = Some((z_t, y2, inom, hjl.steg + 1));
    }

    if !var_girig || !ack.mat_aktiv || hjl.steg < 100 {
        return;
    }

    // S3: jämför bästa handlingsmängder för T = 1 och T = 2 (γ = 0,70, hela sökningen), utan lottning.
    let v1: [f64; A_N] = std::array::from_fn(|a| hjl.varde(&ys[a]));
    let v2 = hjl.planera_varden_t2_fran_v1(&v1, ys, GAMMA_L1);
    let max1 = v1.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let max2 = v2.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let har_gemensam = (0..A_N).any(|a| v1[a] == max1 && v2[a] == max2);
    ack.s3_totalt += 1;
    if !har_gemensam {
        ack.s3_annat += 1;
    }

    // S4: q = xᵀ P_a x före klampning för alla 9 handlingar + mest prövad mot minst prövad handling.
    let x0 = HjL1::x_av(z);
    let mut u = [0.0f64; A_N];
    for a in 0..A_N {
        let q = xpx_rls(&hjl.p_rls_akt[a], &x0);
        ack.s4_q.registrera(q);
        u[a] = q.clamp(0.0, 100.0).sqrt();
    }
    let mut a_mest = 0usize;
    let mut a_minst = 0usize;
    for a in 1..A_N {
        if hjl.grind_n[a] > hjl.grind_n[a_mest] {
            a_mest = a;
        }
        if hjl.grind_n[a] < hjl.grind_n[a_minst] {
            a_minst = a;
        }
    }
    ack.s4_val_totalt += 1;
    if u[a_mest] > u[a_minst] {
        ack.s4_mest_storre_u += 1;
    }
}

impl HjL1 {
    pub fn fran_gen(
        gen: &[f64],
        gen_b1: &GenB1,
        gen_l1: &GenL1,
        varld_fro: u64,
        uid: u64,
        cfg: HjL1Config,
    ) -> Self {
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
            senaste_post_steg: None,
            replay_rng: Rng::new(blanda_replay_fro(varld_fro, uid)),
            q_stat: OsakQStat::default(),
            vanta_ka: None,
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

    /// Ren omkodning av sparade råsinnen i dagens tillstånd (`norm_z(kod_ra(kanal_vikt ⊙ skala(s_ra)))`).
    /// Uppdaterar aldrig något tillstånd.
    pub fn koda_om_ra(&self, s_ra: &[f32; S_B1]) -> [f64; Z_B1] {
        let s_f64: [f64; S_B1] = std::array::from_fn(|j| s_ra[j] as f64);
        let s_sk = self.skala(&s_f64);
        let mut s_in = [0.0; S_B1];
        for j in 0..S_B1 {
            s_in[j] = self.kanal_vikt[j] * s_sk[j];
        }
        let z_raw = self.kod_ra(&s_in);
        self.norm_z(&z_raw)
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

    /// Exakt algebraisk komprimering för steg 2 över alla 81 följder (strikt `>` från lägsta index, inga slumptal).
    pub fn planera_varden_t2_fran_v1(
        &self,
        v1: &[f64; A_N],
        ys: &[[f64; Z_B1]; A_N],
        gamma: f64,
    ) -> [f64; A_N] {
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
            v_tot[a1] = v1[a1] + gamma * max_v2;
        }
        v_tot
    }

    /// Exakt handlingsvärdering (1 steg eller exakt 2 steg över alla 81 följder + valfri osäkerhetsterm).
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
        let v_tot = self.planera_varden_t2_fran_v1(&v1, &ys, self.cfg.gamma);
        (v_tot, ys)
    }

    /// Som `planera_varden`, men registrerar också `q = xᵀ P_a x` före klampning i `self.q_stat` när osäkerhet används.
    fn planera_varden_med_q_stat(&mut self, z: &[f64; Z_B1]) -> ([f64; A_N], [[f64; Z_B1]; A_N]) {
        if self.cfg.anvand_w_osak && !self.cfg.stryk_handling_i_val {
            let x0 = Self::x_av(z);
            for a in 0..A_N {
                let q = xpx_rls(&self.p_rls_akt[a], &x0);
                self.q_stat.registrera(q);
            }
        }
        self.planera_varden(z)
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

    fn spara_och_spela_replay(
        &mut self,
        s_nu_ra: &[f64; S_B1],
        a_prev: usize,
        r: f64,
        err_abs: f64,
    ) {
        if !self.cfg.med_replay || self.buffert.is_empty() {
            return;
        }
        let foljd = if self.senaste_post_steg == Some(self.steg.wrapping_sub(1)) {
            1
        } else {
            0
        };
        self.senaste_post_steg = Some(self.steg);
        let post = MinnePostL1 {
            s_ra: std::array::from_fn(|j| s_nu_ra[j] as f32),
            r: r as f32,
            err_abs: err_abs as f32,
            a_prev: a_prev as u8,
            foljd,
        };
        self.buffert[self.buf_pos] = post;
        self.buf_pos = (self.buf_pos + 1) % M_L1;
        if self.buf_len < M_L1 {
            self.buf_len += 1;
        }
        if self.buf_len < 4 {
            return;
        }
        // Replay-steg 1: likformig dragning (K-c) eller topp-överraskningsminne bland 8 likformiga dragningar (strikt `>`).
        let idx1 = if self.cfg.likformig_replay {
            self.replay_rng.under(self.buf_len)
        } else {
            let mut best_idx = self.replay_rng.under(self.buf_len);
            let mut best_e = self.buffert[best_idx].err_abs;
            for _ in 1..8 {
                let k = self.replay_rng.under(self.buf_len);
                if self.buffert[k].err_abs > best_e {
                    best_e = self.buffert[k].err_abs;
                    best_idx = k;
                }
            }
            best_idx
        };
        self.replay_en_post(idx1);
        // Replay-steg 2: likformig dragning ur bufferten.
        let uni_idx = self.replay_rng.under(self.buf_len);
        self.replay_en_post(uni_idx);
    }

    fn replay_en_post(&mut self, idx: usize) {
        let post = self.buffert[idx];
        let z_f64 = self.koda_om_ra(&post.s_ra);
        let pred = self.varde(&z_f64);
        let err = (post.r as f64) - pred;
        let x = Self::x_av(&z_f64);
        replay_uppdatera_skalar(&mut self.w_krit, &self.p_krit, &x, err, self.cfg.eta_replay);
        self.buffert[idx].err_abs = err.abs() as f32;
    }

    fn uppdatera_fran_forra(&mut self, s_nu_ra: &[f64; S_B1], s_in_now: &[f64; S_B1], lar: bool) {
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

        // 1. Lärd kritiker på nutidssteget + valfri kontrollarm K-a/K-b eller prioriterad/likformig återspelning (Del L1-A).
        if self.cfg.med_kritiker {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let klamp = self.cfg.r_klamp.max(1.0);
                let r = r_raw.clamp(-klamp, klamp);
                if self.cfg.k_a_tvastegs_mal {
                    // Kontrollarm K-a (avsnitt 6): kritikern tränas mot r_t + γ · r_{t+1} (fördröjd ett steg).
                    if let Some((z_prev_ka, r_prev_ka)) = self.vanta_ka.replace((z_mal, r)) {
                        let r2 = r_prev_ka + self.cfg.gamma * r;
                        let pred_k = self.varde(&z_prev_ka);
                        let err_k = r2 - pred_k;
                        if self.steg >= 100 {
                            self.krit_n += 1;
                            self.krit_sy += r2;
                            self.krit_sy2 += r2 * r2;
                            self.krit_se2 += err_k * err_k;
                        }
                        let x_k = Self::x_av(&z_prev_ka);
                        rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
                    }
                } else {
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
                    if self.cfg.k_b_extra_uppd {
                        // Kontrollarm K-b (avsnitt 6): två extra steg med formeln i avsnitt 3 på nutidsposten, utan minne.
                        for _ in 0..2 {
                            let pred_e = self.varde(&z_mal);
                            let err_e = r - pred_e;
                            replay_uppdatera_skalar(
                                &mut self.w_krit,
                                &self.p_krit,
                                &x_k,
                                err_e,
                                self.cfg.eta_replay,
                            );
                        }
                    }
                    self.spara_och_spela_replay(s_nu_ra, a_prev, r, err_k.abs());
                }
            }
        } else {
            let _ = self.vanta_beloning.take();
        }

        // 2. Uppdatera de 9 fristående RLS-världsmodellerna (med egenvärdesklampning när L1-C eller K-d är på).
        let alpha_akt = if self.cfg.rls_alpha_tio > 0 {
            self.cfg.rls_alpha_tio as f64 / 10.0
        } else {
            4.0
        };
        let stabil_max = if self.cfg.stabil_p_a {
            Some(1.0 / alpha_akt)
        } else {
            None
        };
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
            stabil_max,
        );
    }

    pub fn valj_lott(&mut self, s_nu: &[f64; S_B1], lar: bool, g: &mut Rng) -> usize {
        self.valj_lott_med_skugga(s_nu, lar, g, None)
    }

    pub fn valj_lott_med_skugga(
        &mut self,
        s_nu: &[f64; S_B1],
        lar: bool,
        g: &mut Rng,
        mut skugga: Option<(&mut SkuggaDjurL1, &mut SkuggaL1Ack, bool)>,
    ) -> usize {
        if let Some((djur, ack, var_angrepp)) = skugga.as_mut() {
            skugga_fore_uppdatering(self, s_nu, self.vanta_beloning, *var_angrepp, djur, ack);
        }
        let forra_for_s2 = self.forra.map(|(_, z_p, a_p, y_p)| (z_p, a_p, y_p));
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let s_sk_f32: [f32; S_B1] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B1];
        for j in 0..S_B1 {
            s_in[j] = self.kanal_vikt[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(s_nu, &s_in, lar);
        let z_raw = self.kod_ra(&s_in);
        if !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);

        let (varden, ys) = self.planera_varden_med_q_stat(&z);
        for (a, &v) in varden.iter().enumerate() {
            assert!(
                v.is_finite(),
                "icke-ändligt handlingsvärde före slutval för handling {a}: {v}"
            );
        }
        let ba = lott_bland_max(&varden, g);
        if let Some((djur, ack, _)) = skugga {
            skugga_vid_val(self, &z, &ys, ba, true, forra_for_s2, djur, ack);
        }
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
        self.lar_och_satt_med_skugga(s_nu, lar, a, None);
    }

    pub fn lar_och_satt_med_skugga(
        &mut self,
        s_nu: &[f64; S_B1],
        lar: bool,
        a: usize,
        mut skugga: Option<(&mut SkuggaDjurL1, &mut SkuggaL1Ack, bool)>,
    ) {
        assert!(a < A_N);
        if let Some((djur, ack, var_angrepp)) = skugga.as_mut() {
            skugga_fore_uppdatering(self, s_nu, self.vanta_beloning, *var_angrepp, djur, ack);
        }
        let forra_for_s2 = self.forra.map(|(_, z_p, a_p, y_p)| (z_p, a_p, y_p));
        if !self.skala_init {
            self.uppdatera_skala(s_nu);
        }
        let s_sk = self.skala(s_nu);
        let s_sk_f32: [f32; S_B1] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B1];
        for j in 0..S_B1 {
            s_in[j] = self.kanal_vikt[j] * s_sk[j];
        }
        self.uppdatera_fran_forra(s_nu, &s_in, lar);
        let z_raw = self.kod_ra(&s_in);
        if !self.z_skala_init {
            self.uppdatera_z_skala(&z_raw);
        }
        let z = self.norm_z(&z_raw);
        let ya = self.forutsag(&z, a);
        if let Some((djur, ack, _)) = skugga {
            skugga_vid_val(
                self,
                &z,
                &[[0.0; Z_B1]; A_N],
                a,
                false,
                forra_for_s2,
                djur,
                ack,
            );
        }
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
    fn l1_minnepost_storlek_124_och_replay_fro_och_grannar() {
        assert_eq!(std::mem::size_of::<MinnePostL1>(), 124);
        assert_ne!(blanda_replay_fro(341, 1), blanda_replay_fro(342, 1));
        assert_ne!(blanda_replay_fro(341, 1), blanda_replay_fro(341, 2));
        let mut inom = 0usize;
        for a1 in 0..A_N {
            for a2 in 0..A_N {
                if tva_steg_inom_grannar(a1, a2) {
                    inom += 1;
                }
            }
        }
        assert_eq!(inom, 49, "exakt 49 av 81 tvåstegsföljder ska sluta inom GRANNAR");
    }

    #[test]
    fn l1_avstangd_matchar_b1_exakt_aven_med_skuggan_pa() {
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
        let mut l1 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 42, cfg_l1);
        let mut skugga_djur = SkuggaDjurL1::ny();
        let mut skugga_ack = SkuggaL1Ack::ny(true);
        let mut g_b1 = Rng::new(999);
        let mut g_l1 = Rng::new(999);
        for t in 0..300 {
            let mut s = [0.0; S_B1];
            for j in 0..S_B1 {
                s[j] = ((t * 7 + j * 13) as f64).sin() * 0.5 + 0.5;
            }
            let r = (t as f64 * 0.13).cos() * 2.0;
            let var_angrepp = t % 23 == 0;
            b1.vanta_beloning = Some(r);
            l1.vanta_beloning = Some(r);
            let a_b1 = b1.valj_lott(&s, true, &mut g_b1);
            let a_l1 = l1.valj_lott_med_skugga(
                &s,
                true,
                &mut g_l1,
                Some((&mut skugga_djur, &mut skugga_ack, var_angrepp)),
            );
            assert_eq!(a_b1, a_l1, "avstängd L1 med skugga avvek från B1 vid steg {t}");
        }
        // Bit-för-bit-likhet efter att h_t tagits bort och med skuggan på.
        for j in 0..=Z_B1 {
            assert_eq!(b1.w_krit[j].to_bits(), l1.w_krit[j].to_bits());
        }
        for a in 0..A_N {
            for r in 0..Z_B1 {
                for m in 0..=Z_B1 {
                    assert_eq!(b1.w_rls_akt[a][r][m].to_bits(), l1.w_rls_akt[a][r][m].to_bits());
                }
            }
        }
        assert!(skugga_ack.s2_tot_n > 150, "skuggan borde ha mätt S2");
        assert!(skugga_ack.s3_totalt > 150, "skuggan borde ha mätt S3");
        assert!(skugga_ack.s4_q.n_anrop > 150 * A_N as u64, "skuggan borde ha mätt S4");
        assert!(skugga_ack.s5_full.n > 150, "skuggan borde ha mätt S5");
    }

    #[test]
    fn l1_tva_stegs_planering_tar_omvag_och_fjarde_basta_v1_samt_matchar_direkt_rullning() {
        // Del 1: enkel 2-stegsfälla (a=0 ger +0.5 nu men fast; a=1 ger 0 nu men +3.0 via a=2 i steg 2).
        let mut rng = Rng::new(232302);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let bygg = |plan_steg: u8| {
            let mut h = HjL1::fran_gen(
                &hj.gen,
                &gb1,
                &gl1,
                341,
                1,
                HjL1Config {
                    stryk_handling_i_val: false,
                    rls_alpha_tio: 40,
                    med_kritiker: false,
                    anvand_w_skada: false,
                    r_klamp: 20.0,
                    med_replay: false,
                    eta_replay: ETA_REPLAY,
                    plan_steg,
                    gamma: GAMMA_L1,
                    anvand_w_osak: false,
                    stabil_p_a: false,
                    k_a_tvastegs_mal: false,
                    k_b_extra_uppd: false,
                    likformig_replay: false,
                },
            );
            h.v = [0.0; Z_B1 + 1];
            h.v[1] = 1.0;
            h.w_rls_akt = [[[0.0; Z_B1 + 1]; Z_B1]; A_N];
            h.w_rls_akt[0][1][Z_B1] = 0.5;
            h.w_rls_akt[0][1][1] = -1.0;
            h.w_rls_akt[1][0][Z_B1] = 1.0;
            h.w_rls_akt[1][0][0] = -1.0;
            h.w_rls_akt[2][1][0] = 3.0;
            h
        };

        let z0 = [0.0; Z_B1];
        let h1 = bygg(1);
        let (v1, _) = h1.planera_varden(&z0);
        assert_eq!(lott_bland_max(&v1, &mut Rng::new(7)), 0);

        let h2 = bygg(2);
        let (v2, ys) = h2.planera_varden(&z0);
        assert_eq!(lott_bland_max(&v2, &mut Rng::new(7)), 1);

        for a1 in 0..A_N {
            let mut max_direkt = f64::NEG_INFINITY;
            for a2 in 0..A_N {
                let y2 = h2.forutsag(&ys[a1], a2);
                max_direkt = max_direkt.max(h2.varde(&y2));
            }
            let exakt = h2.varde(&ys[a1]) + GAMMA_L1 * max_direkt;
            assert!((v2[a1] - exakt).abs() < 1e-12);
        }

        // Del 2 (ritning v2 avsnitt 5): rätt förstasteg (a = 3) har bara det FJÄRDE bästa V₁!
        // a = 0 (+0.9), a = 1 (+0.7), a = 2 (+0.5) ligger alla före a = 3 (+0.1), men fastnar på samma värde.
        // a = 3 sätter z0 = +1.0 (V₁ = +0.1), där a = 4 sedan ger +4.0 i steg 2 (summa 0.1 + 0.7*4.1 = 2.97).
        let bygg4 = |plan_steg: u8| {
            let mut h = bygg(plan_steg);
            h.w_rls_akt = [[[0.0; Z_B1 + 1]; Z_B1]; A_N];
            // a = 0, 1, 2: höga lokala V₁ men sätter z1 till 0.9/0.7/0.5 och z0 = 0
            for (a, val) in [(0usize, 0.9), (1, 0.7), (2, 0.5)] {
                h.w_rls_akt[a][1][Z_B1] = val;
                h.w_rls_akt[a][1][1] = -1.0;
            }
            // a = 3: fjärde bästa V₁ = +0.1, men öppnar z0 = +1.0
            h.w_rls_akt[3][1][Z_B1] = 0.1;
            h.w_rls_akt[3][1][1] = -1.0;
            h.w_rls_akt[3][0][Z_B1] = 1.0;
            h.w_rls_akt[3][0][0] = -1.0;
            // a = 4: ger +4.0 på axel 1 när z0 = +1.0
            h.w_rls_akt[4][1][0] = 4.0;
            h
        };
        let (v1_4, _) = bygg4(1).planera_varden(&z0);
        // Kontrollera att a = 3 verkligen är fjärde bäst i steg 1 (3 handlingar strikt över a=3).
        let battre_an_3 = (0..A_N).filter(|&a| v1_4[a] > v1_4[3]).count();
        assert_eq!(battre_an_3, 3, "a=3 ska ha exakt det fjärde bästa V₁, fick {battre_an_3} före");
        assert_eq!(lott_bland_max(&v1_4, &mut Rng::new(11)), 0);

        let h2_4 = bygg4(2);
        let (v2_4, ys4) = h2_4.planera_varden(&z0);
        assert_eq!(
            lott_bland_max(&v2_4, &mut Rng::new(11)),
            3,
            "hel sökning över 81 följder måste hitta omvägen a=3 med fjärde bästa V₁"
        );
        for a1 in 0..A_N {
            let mut max_direkt = f64::NEG_INFINITY;
            for a2 in 0..A_N {
                let y2 = h2_4.forutsag(&ys4[a1], a2);
                max_direkt = max_direkt.max(h2_4.varde(&y2));
            }
            let exakt = h2_4.varde(&ys4[a1]) + GAMMA_L1 * max_direkt;
            assert!((v2_4[a1] - exakt).abs() < 1e-12);
        }
    }

    #[test]
    fn l1_planering_likhetsprov_nar_y2_lika_y1() {
        // Avsnitt 5: med ŷ₂ = ŷ₁ (w_rls_akt = 0 i steg 2) blir summan (1 + γ) · V₁ och samma val som T = 1.
        let mut rng = Rng::new(232601);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };
        let mut h1 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 7, HjL1Config::fran_mask(0, 40, 20.0));
        let mut h2 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 7, HjL1Config::fran_mask(L1_PLAN2, 40, 20.0));
        // Låt steg 1 skilja sig per handling genom att bara ge ett konstant v1-skift på ys, men nollställ steg 2:
        // med w_rls_akt == 0 ger forutsag(y1, a2) == y1 (identiteten).
        h1.w_rls_akt = [[[0.0; Z_B1 + 1]; Z_B1]; A_N];
        h2.w_rls_akt = [[[0.0; Z_B1 + 1]; Z_B1]; A_N];
        let z = [0.4, -0.2, 0.7, -0.5, 0.3, 0.1];
        let ys_konst: [[f64; Z_B1]; A_N] =
            std::array::from_fn(|a| std::array::from_fn(|r| z[r] + 0.1 * (a as f64 - 4.0) * (r as f64 + 1.0) * 0.1));
        let v1: [f64; A_N] = std::array::from_fn(|a| h1.varde(&ys_konst[a]));
        let v2 = h2.planera_varden_t2_fran_v1(&v1, &ys_konst, GAMMA_L1);
        for a in 0..A_N {
            let forvantat = (1.0 + GAMMA_L1) * v1[a];
            assert!(
                (v2[a] - forvantat).abs() < 1e-12,
                "med ŷ₂ = ŷ₁ ska V₂ = (1+γ)·V₁: fick {} mot {}",
                v2[a],
                forvantat
            );
        }
        let a1 = lott_bland_max(&v1, &mut Rng::new(19));
        let a2 = lott_bland_max(&v2, &mut Rng::new(19));
        assert_eq!(a1, a2, "med ŷ₂ = ŷ₁ måste T=2 ge exakt samma val som T=1");

        // Röd kontroll: när steg 2 ger ett positivt lyft (inte identiteten) avviker V₂ från (1+γ)·V₁.
        h2.w_rls_akt[0][0][Z_B1] = if h2.v_eff()[0] >= 0.0 { 2.0 } else { -2.0 };
        let v2_rod = h2.planera_varden_t2_fran_v1(&v1, &ys_konst, GAMMA_L1);
        let max_avv = (0..A_N)
            .map(|a| (v2_rod[a] - (1.0 + GAMMA_L1) * v1[a]).abs())
            .fold(0.0f64, f64::max);
        assert!(max_avv > 1e-3, "röd kontroll: icke-noll steg-2-modell måste avvika från (1+γ)·V₁");
    }

    #[test]
    fn l1_replay_eta_noll_matchar_utan_replay_och_eta_pos_skiljer() {
        let mut rng = Rng::new(232602);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let mut utan = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 10, HjL1Config::fran_mask(0, 40, 20.0));
        let mut cfg_eta0 = HjL1Config::fran_mask(L1_REPLAY, 40, 20.0);
        cfg_eta0.eta_replay = 0.0;
        let mut rep0 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 10, cfg_eta0);
        let mut rep35 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 10, HjL1Config::fran_mask(L1_REPLAY, 40, 20.0));

        let (mut g1, mut g2, mut g3) = (Rng::new(77), Rng::new(77), Rng::new(77));
        for t in 0..80 {
            let mut s = [0.0; S_B1];
            for j in 0..S_B1 {
                s[j] = ((t * 5 + j * 11) as f64).sin() * 0.4 + 0.5;
            }
            let r = if t % 17 == 0 { -6.0 } else { 0.3 };
            utan.vanta_beloning = Some(r);
            rep0.vanta_beloning = Some(r);
            rep35.vanta_beloning = Some(r);
            let a_u = utan.valj_lott(&s, true, &mut g1);
            let a_0 = rep0.valj_lott(&s, true, &mut g2);
            let _ = rep35.valj_lott(&s, true, &mut g3);
            assert_eq!(a_u, a_0);
        }
        // Grön: η = 0 ger exakt samma kritiker som utan återspelning.
        for j in 0..=Z_B1 {
            assert_eq!(utan.w_krit[j].to_bits(), rep0.w_krit[j].to_bits());
            for k in 0..=Z_B1 {
                assert_eq!(utan.p_krit[j][k].to_bits(), rep0.p_krit[j][k].to_bits());
            }
        }
        // Röd: η = 0,35 ändrar faktiskt w_krit (medan P_krit är orörd).
        let diff_w: f64 = (0..=Z_B1).map(|j| (utan.w_krit[j] - rep35.w_krit[j]).abs()).sum();
        assert!(diff_w > 1e-3, "replay med η=0,35 ska uppdatera w_krit, diff={diff_w}");
    }

    #[test]
    fn l1_felvriden_post_skala2_forbattrar_inte_krit_r2_medan_omkodning_gor() {
        // Ritning v2 avsnitt 9 steg 3:
        // När en sparad post är felvriden (gammal skala × 2 utan omkodning) får den inte förbättra kritikerns R²;
        // med råsinnen och omkodning i dagens koordinater ska kritikerns R² förbättras tydligt.
        let mut rng = Rng::new(232603);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let setup = |med_rep: bool| {
            let mut h = HjL1::fran_gen(
                &hj.gen,
                &gb1,
                &gl1,
                341,
                20,
                HjL1Config::fran_mask(if med_rep { L1_REPLAY } else { 0 }, 40, 20.0),
            );
            h.v = [0.0; Z_B1 + 1];
            h.skala_init = true;
            h.z_skala_init = true;
            h.mu = [0.0; S_B1];
            h.var = [1.0; S_B1];
            h.kanal_vikt = [1.0; S_B1];
            h.mu_z = [0.0; Z_B1];
            h.var_z = [0.25; Z_B1];
            // Ren, kontrollerbar kodare: kanal 0 -> z[0] = tanh(s[0]), kanal 1 -> z[1] = tanh(s[1])
            h.wk = [[0.0; S_B1 + 1]; Z_B1];
            h.wk[0][0] = 1.0;
            h.wk[1][1] = 1.0;
            h
        };

        let mut s_fara = [0.0; S_B1];
        s_fara[0] = 0.8;
        let mut s_lugn = [0.0; S_B1];
        s_lugn[0] = -0.8;

        let mut utan = setup(false);
        let mut omkod = setup(true);
        let mut felvriden = setup(false); // spelar manuellt upp gammal z skalad × 2 (som v1 vid skalskifte)

        // 1. Först 4 sällsynta farosteg i gamla koordinater (var_z = 0.25 -> std = 0.5).
        let z_fara_gammal = omkod.koda_om_ra(&s_fara.map(|x| x as f32));
        for k in 0..4 {
            for h in [&mut utan, &mut omkod, &mut felvriden] {
                h.steg = k;
                let z = h.koda_om_ra(&s_fara.map(|x| x as f32));
                let err = -8.0 - h.varde(&z);
                let x = HjL1::x_av(&z);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
            }
            omkod.spara_och_spela_replay(&s_fara, 0, -8.0, 8.0);
        }

        // 2. Nu skiftar skalan kraftigt (skalan × 2 och centreringen vrids), så gamla z ligger × 2 fel
        //    mot dagens koordinatsystem: z_gammal ≈ -2 * z_ny eller kraftigt skiftat i tecken/skala.
        for h in [&mut utan, &mut omkod, &mut felvriden] {
            h.mu[0] = 1.6; // skiftar s_fara så z_ny byter sida och har annan skala
            h.var[0] = 4.0; // skalan × 2 (std 2,0 mot 1,0)
            h.mu_z[0] = 0.0;
            h.var_z[0] = 1.0; // z-skalan × 2 (std 1,0 mot 0,5)
        }

        // 3. Följt av 40 lugna steg i nya koordinater, med ett nutids-farosteg för att förankra skalan.
        for k in 4..44 {
            let (s_steg, r_steg) = if k == 4 { (s_fara, -8.0) } else { (s_lugn, 0.2) };
            for h in [&mut utan, &mut omkod, &mut felvriden] {
                h.steg = k;
                let z = h.koda_om_ra(&s_steg.map(|x| x as f32));
                let err = r_steg - h.varde(&z);
                let x = HjL1::x_av(&z);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
            }
            omkod.spara_och_spela_replay(&s_steg, 0, r_steg, 1.0);
            // Felvriden (v1 utan omkodning): spelar upp den gamla sparade z_fara_gammal (skalad × 2 i gamla koordinater).
            let z_vrid: [f64; Z_B1] = std::array::from_fn(|i| (z_fara_gammal[i] * 2.0).clamp(-3.0, 3.0));
            let err_v = -8.0 - felvriden.varde(&z_vrid);
            let x_v = HjL1::x_av(&z_vrid);
            replay_uppdatera_skalar(&mut felvriden.w_krit, &felvriden.p_krit, &x_v, err_v, ETA_REPLAY);
        }

        // 4. Utvärdera kritikerns R² i DAGENS koordinater över både fara och lugn.
        // Utvärdera R² i dagens koordinater över både fara och lugn.
        let r2_i_dag = |h: &HjL1| -> f64 {
            let prov = [
                (h.koda_om_ra(&s_fara.map(|x| x as f32)), -8.0),
                (h.koda_om_ra(&s_lugn.map(|x| x as f32)), 0.2),
            ];
            let my = prov.iter().map(|(_, r)| *r).sum::<f64>() / prov.len() as f64;
            let ss_tot: f64 = prov.iter().map(|(_, r)| (*r - my).powi(2)).sum();
            let ss_res: f64 = prov.iter().map(|(z, r)| (*r - h.varde(z)).powi(2)).sum();
            1.0 - ss_res / ss_tot
        };
        let r2_utan = r2_i_dag(&utan);
        let r2_fel = r2_i_dag(&felvriden);
        let r2_omkod = r2_i_dag(&omkod);

        assert!(
            r2_fel <= r2_utan,
            "röd kontroll: felvriden sparad post (skala × 2) får inte förbättra kritikerns R² (fel={r2_fel:.4}, utan={r2_utan:.4})"
        );
        assert!(
            r2_omkod > r2_utan + 0.05 && r2_omkod > r2_fel + 0.05,
            "grön kontroll: omkodning ur råsinnen ska förbättra kritikerns R² (omkod={r2_omkod:.4}, utan={r2_utan:.4}, fel={r2_fel:.4})"
        );
    }

    #[test]
    fn l1_felvriden_post_uppmatt_drift_1_46_forbattrar_inte_krit_r2_medan_omkodning_gor() {
        // Ritning v2 avsnitt 2 punkt 5: eftersom median S5 = 1,4610 >= 0,05 på 341-360,
        // prövas rödprovet med felvriden post också med exakt den uppmätta driften D ≈ 1,46
        // orsakad av att EMA-medelvärdet `mu` glider när djuret byter miljö.
        let mut rng = Rng::new(231205);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let setup = |med_rep: bool| {
            let mut h = HjL1::fran_gen(
                &hj.gen,
                &gb1,
                &gl1,
                341,
                21,
                HjL1Config::fran_mask(if med_rep { L1_REPLAY } else { 0 }, 40, 20.0),
            );
            h.v = [0.0; Z_B1 + 1];
            h.skala_init = true;
            h.z_skala_init = true;
            h.mu = [0.0; S_B1];
            h.var = [1.0; S_B1];
            h.kanal_vikt = [1.0; S_B1];
            h.mu_z = [0.0; Z_B1];
            h.var_z = [0.25; Z_B1];
            h.wk = [[0.0; S_B1 + 1]; Z_B1];
            h.wk[0][0] = 1.0;
            h.wk[1][1] = 1.0;
            h
        };

        let mut s_fara = [0.0; S_B1];
        s_fara[0] = 0.8;
        let mut s_lugn = [0.0; S_B1];
        s_lugn[0] = 1.65;
        s_lugn[1] = -0.80;

        let mut utan = setup(false);
        let mut omkod = setup(true);
        let mut felvriden = setup(false);

        let z_fara_gammal = omkod.koda_om_ra(&s_fara.map(|x| x as f32));
        for k in 0..4 {
            for h in [&mut utan, &mut omkod, &mut felvriden] {
                h.steg = k;
                let z = h.koda_om_ra(&s_fara.map(|x| x as f32));
                let err = -8.0 - h.varde(&z);
                let x = HjL1::x_av(&z);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
            }
            omkod.spara_och_spela_replay(&s_fara, 0, -8.0, 8.0);
        }

        // Skifta enbart EMA-centreringen `mu` så relativa driften D = ||z_ny - z_gammal|| / ||z_gammal||
        // blir exakt den uppmätta medianen S5 ≈ 1,46 (gamla farokoordinaten överlappar nu lugna lägen).
        for h in [&mut utan, &mut omkod, &mut felvriden] {
            h.mu[0] = 0.85;
            h.mu[1] = -0.80;
        }
        let z_fara_ny = omkod.koda_om_ra(&s_fara.map(|x| x as f32));
        let norm_g: f64 = z_fara_gammal.iter().map(|x| x * x).sum::<f64>().sqrt();
        let diff_n: f64 = z_fara_ny
            .iter()
            .zip(z_fara_gammal.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();
        let drift = diff_n / norm_g;
        assert!(
            (1.40..=1.52).contains(&drift),
            "testets drift ska matcha uppmätt S5 ≈ 1,46, fick {drift:.4}"
        );

        for k in 4..44 {
            let (s_steg, r_steg) = if k == 4 { (s_fara, -8.0) } else { (s_lugn, 0.2) };
            for h in [&mut utan, &mut omkod, &mut felvriden] {
                h.steg = k;
                let z = h.koda_om_ra(&s_steg.map(|x| x as f32));
                let err = r_steg - h.varde(&z);
                let x = HjL1::x_av(&z);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
            }
            omkod.spara_och_spela_replay(&s_steg, 0, r_steg, 1.0);
            // Felvriden (v1 utan omkodning): spelar upp den gamla sparade z_fara_gammal (drift D ≈ 1,46).
            let err_v = -8.0 - felvriden.varde(&z_fara_gammal);
            let x_v = HjL1::x_av(&z_fara_gammal);
            replay_uppdatera_skalar(&mut felvriden.w_krit, &felvriden.p_krit, &x_v, err_v, ETA_REPLAY);
        }

        let r2_i_dag = |h: &HjL1| -> f64 {
            let prov = [
                (h.koda_om_ra(&s_fara.map(|x| x as f32)), -8.0),
                (h.koda_om_ra(&s_lugn.map(|x| x as f32)), 0.2),
            ];
            let my = prov.iter().map(|(_, r)| *r).sum::<f64>() / prov.len() as f64;
            let ss_tot: f64 = prov.iter().map(|(_, r)| (*r - my).powi(2)).sum();
            let ss_res: f64 = prov.iter().map(|(z, r)| (*r - h.varde(z)).powi(2)).sum();
            1.0 - ss_res / ss_tot
        };
        let r2_utan = r2_i_dag(&utan);
        let r2_fel = r2_i_dag(&felvriden);
        let r2_omkod = r2_i_dag(&omkod);

        assert!(
            r2_fel <= r2_utan,
            "röd kontroll (uppmätt drift D={drift:.3}): gammal post utan omkodning får inte förbättra R² (fel={r2_fel:.4}, utan={r2_utan:.4})"
        );
        assert!(
            r2_omkod > r2_utan + 0.05 && r2_omkod > r2_fel + 0.05,
            "grön kontroll (uppmätt drift D={drift:.3}): omkodning ur råsinnen ska förbättra R² (omkod={r2_omkod:.4}, utan={r2_utan:.4}, fel={r2_fel:.4})"
        );
    }

    #[test]
    fn l1_replay_haller_pkrit_begransad_och_minns_sallsynta_smarthaendelser() {
        let mut rng = Rng::new(232303);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };

        let setup = |mask: u8| {
            let mut h = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 10, HjL1Config::fran_mask(mask, 40, 20.0));
            h.v = [0.0; Z_B1 + 1];
            h.skala_init = true;
            h.z_skala_init = true;
            h.mu = [0.0; S_B1];
            h.var = [1.0; S_B1];
            h.kanal_vikt = [1.0; S_B1];
            h.mu_z = [0.0; Z_B1];
            h.var_z = [0.25; Z_B1];
            h
        };
        let mut med_rep = setup(L1_REPLAY);
        let mut utan_rep = setup(0);

        let mut s_fara = [0.0; S_B1];
        s_fara[0] = 1.0;
        let mut s_lugn = [0.0; S_B1];
        s_lugn[0] = -1.0;

        for k in 0..43 {
            let (s_steg, r) = if k < 3 { (s_fara, -8.0) } else { (s_lugn, 0.2) };
            for h in [&mut med_rep, &mut utan_rep] {
                h.steg = k;
                let z_mal = h.koda_om_ra(&s_steg.map(|x| x as f32));
                let pred = h.varde(&z_mal);
                let err = r - pred;
                let x = HjL1::x_av(&z_mal);
                rls_uppdatera_skalar(&mut h.w_krit, &mut h.p_krit, &x, err, LAMBDA_KRIT);
                h.spara_och_spela_replay(&s_steg, 0, r, err.abs());
            }
        }
        for m in 0..=Z_B1 {
            for n in 0..=Z_B1 {
                assert!(med_rep.p_krit[m][n].is_finite());
                assert!((med_rep.p_krit[m][n] - utan_rep.p_krit[m][n]).abs() < 1e-12);
            }
        }
        let z_fara = med_rep.koda_om_ra(&s_fara.map(|x| x as f32));
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
        let mut h = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 5, HjL1Config::fran_mask(L1_OSAK, 40, 20.0));
        let z = [0.5, -0.3, 0.2, 0.1, -0.4, 0.6];
        let u_fore = h.osakerhet(&z, 3);
        let u_annan = h.osakerhet(&z, 4);
        assert!((u_fore - u_annan).abs() < 1e-12 && u_fore > 0.0);
        let x = HjL1::x_av(&z);
        for _ in 0..10 {
            let delta = [0.1; Z_B1];
            let pred: [f64; Z_B1] = std::array::from_fn(|r| dot_zx(&h.w_rls_akt[3][r], &x));
            rls_uppdatera(
                &mut h.w_rls_akt[3],
                &mut h.p_rls_akt[3],
                &x,
                &delta,
                &pred,
                LAMBDA_RLS,
                Some(0.25),
            );
        }
        let u_efter = h.osakerhet(&z, 3);
        assert!(u_efter < u_fore * 0.6);
        assert!(u_efter < h.osakerhet(&z, 4));

        // Kontrollera q-räknaren före klampning.
        let mut qs = OsakQStat::default();
        qs.registrera(-0.25);
        qs.registrera(150.0);
        qs.registrera(0.5);
        assert_eq!(qs.n_anrop, 3);
        assert_eq!(qs.n_neg, 1);
        assert_eq!(qs.n_over100, 1);
        assert!((qs.min_q - (-0.25)).abs() < 1e-12);

        let mut g2 = Rng::new(232305);
        let mut pos = 0usize;
        let mut sum = 0.0;
        for _ in 0..1000 {
            let w = GenL1::slump(&mut g2).w_osak;
            if w > 0.0 { pos += 1; }
            sum += w;
        }
        assert!((440..=560).contains(&pos));
        assert!((sum / 1000.0).abs() < 0.1);
    }

    #[test]
    fn l1_stabil_pa_rodprov_900_uppdateringar_ortogonal_axel() {
        // Ritning v2 avsnitt 4: uppdatera handling 0 900 gånger längs en axel.
        // Mät u på en ortogonal axel för handling 0 och för en oprövad handling 1:
        // - RÖD (utan egenvärdesklampning, som i 7de2aa3): u0 > u1!
        // - GRÖN (med egenvärdesklampning till [1e-9, 1/α]): u0 <= u1 och q >= 0 i alla anrop!
        let mut rng = Rng::new(234101);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 1.0 };

        let mut gammal = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 1, HjL1Config::fran_mask(0, 40, 20.0));
        let mut stabil = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 1, HjL1Config::fran_mask(L1_OSAK, 40, 20.0));
        assert!(!gammal.cfg.stabil_p_a);
        assert!(stabil.cfg.stabil_p_a);

        let z_axel0 = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let z_ort = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let x0 = HjL1::x_av(&z_axel0);
        let x_ort = HjL1::x_av(&z_ort);
        let delta = [0.05; Z_B1];

        let mut min_q_stabil = f64::INFINITY;
        let mut u0_gammal_500 = 0.0;
        for steg in 0..900 {
            let pred_g: [f64; Z_B1] = std::array::from_fn(|r| dot_zx(&gammal.w_rls_akt[0][r], &x0));
            rls_uppdatera(
                &mut gammal.w_rls_akt[0],
                &mut gammal.p_rls_akt[0],
                &x0,
                &delta,
                &pred_g,
                LAMBDA_RLS,
                None,
            );
            if steg == 499 {
                u0_gammal_500 = gammal.osakerhet(&z_ort, 0);
            }
            let pred_s: [f64; Z_B1] = std::array::from_fn(|r| dot_zx(&stabil.w_rls_akt[0][r], &x0));
            rls_uppdatera(
                &mut stabil.w_rls_akt[0],
                &mut stabil.p_rls_akt[0],
                &x0,
                &delta,
                &pred_s,
                LAMBDA_RLS,
                Some(0.25),
            );
            let q_ax0 = xpx_rls(&stabil.p_rls_akt[0], &x0);
            let q_ort = xpx_rls(&stabil.p_rls_akt[0], &x_ort);
            min_q_stabil = min_q_stabil.min(q_ax0).min(q_ort);
        }

        // RÖD kontroll (gammal 7de2aa3-RLS utan egenvärdesklampning):
        // Redan vid steg 500 har den oexciterade ortogonala axeln växt långt över oprövad handling 1 (u0 > u1),
        // och vid steg 900 (efter diagonaltaket 1e3 vid steg 826) har den antingen u0 > u1 eller spårat ur i NaN!
        let u0_gammal = gammal.osakerhet(&z_ort, 0);
        let u1_gammal = gammal.osakerhet(&z_ort, 1);
        assert!(
            u0_gammal_500 > u1_gammal * 2.0,
            "röd kontroll (steg 500): gammal RLS ska ge u0 > u1 på ortogonal axel (u0={u0_gammal_500:.4}, u1={u1_gammal:.4})"
        );
        assert!(
            !u0_gammal.is_finite() || u0_gammal > u1_gammal * 2.0,
            "röd kontroll (steg 900): gammal RLS ska bryta u0 <= u1 (u0={u0_gammal:?}, u1={u1_gammal:.4})"
        );

        // GRÖN kontroll (egenvärdesklampning till [1e-9, 1/α]): u0 <= u1 och q >= 0 i alla 900 anrop!
        let u0_stabil = stabil.osakerhet(&z_ort, 0);
        let u1_stabil = stabil.osakerhet(&z_ort, 1);
        assert!(u0_stabil.is_finite(), "stabil P_a får aldrig ge NaN/Inf");
        assert!(
            u0_stabil <= u1_stabil + 1e-12,
            "grön kontroll: stabil P_a ska ge u0 <= u1 (u0={u0_stabil:.6}, u1={u1_stabil:.6})"
        );
        assert!(
            min_q_stabil >= 0.0,
            "grön kontroll: stabil P_a ska ge q >= 0 i alla anrop (min_q={min_q_stabil})"
        );

        // Kontrollera också att Jacobi bevarar en redan godkänd symmetrisk P-matris till < 1e-12.
        let p_fore = stabil.p_rls_akt[0];
        let mut p_efter = p_fore;
        klampa_egenvarden_p(&mut p_efter, 0.25);
        for i in 0..=Z_B1 {
            for j in 0..=Z_B1 {
                assert!(
                    (p_fore[i][j] - p_efter[i][j]).abs() < 1e-11,
                    "Jacobi borde bevara redan klampad P-matris vid ({i},{j}): {} vs {}",
                    p_fore[i][j],
                    p_efter[i][j]
                );
            }
        }
    }

    #[test]
    fn l1_kontrollarmar_ka_kb_kc_kd_fungerar() {
        let mut rng = Rng::new(234102);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 1.5 };

        // 1. K-a: kritikern tränas mot r_t + γ · r_{t+1} (fördröjd 1 steg) -> lär sig fördröjd belöning.
        let mut h_b1 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 1, HjL1Config::fran_mask(0, 40, 20.0));
        let mut h_ka = HjL1::fran_gen(
            &hj.gen,
            &gb1,
            &gl1,
            261,
            1,
            HjL1Config::fran_mask(L1_K_A_TVASTEGS_MAL, 40, 20.0),
        );
        for h in [&mut h_b1, &mut h_ka] {
            h.v = [0.0; Z_B1 + 1];
            h.skala_init = true;
            h.z_skala_init = true;
            h.mu = [0.0; S_B1];
            h.var = [1.0; S_B1];
            h.kanal_vikt = [1.0; S_B1];
            h.mu_z = [0.0; Z_B1];
            h.var_z = [0.25; Z_B1];
            h.wk = [[0.0; S_B1 + 1]; Z_B1];
            h.wk[0][0] = 1.0;
        }
        let mut s_a = [0.0; S_B1];
        s_a[0] = 0.8;
        let mut s_b = [0.0; S_B1];
        s_b[0] = -0.8;
        // Mönster: varannat steg når s_a (belöning r_t = 0.0) som ALLTID följs av s_b (belöning r_{t+1} = +4.0).
        // B1:s 1-stegskritiker lär sig V(s_a) ≈ 0, medan K-a ska lära sig V(s_a) ≈ 0 + 0.7 * 4.0 = +2.8!
        for t in 0..80 {
            let (s_nu, r_forra) = if t % 2 == 0 { (s_a, 0.0) } else { (s_b, 4.0) };
            for h in [&mut h_b1, &mut h_ka] {
                if t > 0 {
                    h.vanta_beloning = Some(r_forra);
                }
                h.lar_och_satt(&s_nu, true, 0);
            }
        }
        let za = h_ka.koda_om_ra(&s_a.map(|x| x as f32));
        let v_b1_a = h_b1.varde(&za);
        let v_ka_a = h_ka.varde(&za);
        assert!(
            v_ka_a > v_b1_a + 1.5,
            "K-a ska lära sig 2-stegsmålet r_t + γ·r_(t+1) på s_a: v_ka={v_ka_a:.3}, v_b1={v_b1_a:.3}"
        );

        // 2. K-b: två extra uppdateringar på nutidsposten utan minne; med η=0 identisk med B1, med η=0,35 rör w_krit men ej P_krit.
        let mut cfg_kb0 = HjL1Config::fran_mask(L1_K_B_EXTRA_UPPD, 40, 20.0);
        cfg_kb0.eta_replay = 0.0;
        let mut kb_bas = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 2, HjL1Config::fran_mask(0, 40, 20.0));
        let mut kb_0 = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 2, cfg_kb0);
        let mut kb_35 = HjL1::fran_gen(
            &hj.gen,
            &gb1,
            &gl1,
            261,
            2,
            HjL1Config::fran_mask(L1_K_B_EXTRA_UPPD, 40, 20.0),
        );
        for t in 0..40 {
            let s = [((t as f64) * 0.2).sin() * 0.5 + 0.5; S_B1];
            let r = if t % 5 == 0 { -5.0 } else { 0.4 };
            for h in [&mut kb_bas, &mut kb_0, &mut kb_35] {
                h.vanta_beloning = Some(r);
                h.lar_och_satt(&s, true, t % A_N);
            }
        }
        for j in 0..=Z_B1 {
            assert_eq!(kb_bas.w_krit[j].to_bits(), kb_0.w_krit[j].to_bits());
            for k in 0..=Z_B1 {
                assert_eq!(kb_bas.p_krit[j][k].to_bits(), kb_35.p_krit[j][k].to_bits());
            }
        }
        let diff_kb: f64 = (0..=Z_B1).map(|j| (kb_bas.w_krit[j] - kb_35.w_krit[j]).abs()).sum();
        assert!(diff_kb > 1e-3, "K-b med η=0,35 ska uppdatera w_krit extra på nutidsposten");

        // 3. K-c: likformig replay har tom topp-av-8-prioritering och skiljer sig från prioriterad L1-A.
        let mut prio = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 3, HjL1Config::fran_mask(L1_REPLAY, 40, 20.0));
        let mut likf = HjL1::fran_gen(
            &hj.gen,
            &gb1,
            &gl1,
            261,
            3,
            HjL1Config::fran_mask(L1_REPLAY | L1_K_C_LIKFORMIG, 40, 20.0),
        );
        assert!(likf.cfg.med_replay && likf.cfg.likformig_replay && !prio.cfg.likformig_replay);
        for t in 0..60 {
            let mut s = [0.2; S_B1];
            s[0] = if t < 3 { 0.9 } else { -0.5 };
            let r = if t < 3 { -8.0 } else { 0.2 };
            for h in [&mut prio, &mut likf] {
                h.vanta_beloning = Some(r);
                h.lar_och_satt(&s, true, t % A_N);
            }
        }
        let diff_kc: f64 = (0..=Z_B1).map(|j| (prio.w_krit[j] - likf.w_krit[j]).abs()).sum();
        assert!(diff_kc > 1e-4, "prioriterad L1-A och likformig K-c ska ge olika w_krit");

        // 4. K-d: stabil_p_a på men anvand_w_osak av (w_osak påverkar inte valet).
        let mut kd = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 4, HjL1Config::fran_mask(L1_STABIL_PA, 40, 20.0));
        let mut osak = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 261, 4, HjL1Config::fran_mask(L1_OSAK, 40, 20.0));
        assert!(kd.cfg.stabil_p_a && !kd.cfg.anvand_w_osak);
        assert!(osak.cfg.stabil_p_a && osak.cfg.anvand_w_osak);
        let z = [0.3, -0.2, 0.5, 0.1, -0.4, 0.2];
        let (v_kd, _) = kd.planera_varden(&z);
        let (v_os, _) = osak.planera_varden(&z);
        assert!((v_kd[0] - v_os[0]).abs() > 0.1, "K-d ska inte inkludera w_osak-termen i handlingsvärdet");
        let _ = (kd.valj_lott(&[0.5; S_B1], true, &mut Rng::new(1)), osak.valj_lott(&[0.5; S_B1], true, &mut Rng::new(1)));
        assert_eq!(kd.q_stat.n_anrop, 0);
        assert_eq!(osak.q_stat.n_anrop, A_N as u64);
    }

    #[test]
    #[should_panic(expected = "icke-ändligt handlingsvärde")]
    fn l1_icke_andligt_varde_stoppar_med_assert() {
        let mut rng = Rng::new(232604);
        let hj = Hj::slump(&mut rng);
        let gb1 = GenB1::slump(&mut rng);
        let gl1 = GenL1 { w_osak: 0.0 };
        let mut h = HjL1::fran_gen(&hj.gen, &gb1, &gl1, 341, 9, HjL1Config::fran_mask(0, 40, 20.0));
        h.v[0] = f64::NAN;
        let s = [0.5; S_B1];
        let _ = h.valj_lott(&s, true, &mut Rng::new(1));
    }
}
