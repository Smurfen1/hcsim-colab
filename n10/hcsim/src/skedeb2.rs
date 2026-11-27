//! Alt 2361 — Skede B, steg B2: Finare 250 m Hell Creek-arena (64×64 = 16×16 km),
//! autonoma *T. rex* med egen JEPA-hjärna (Alt 1877), vinddriven luktadvektion (Alt 1878–1880),
//! lika stark ofarlig kontrollukt (Alt 1881), skymd sikt och bakhåll i tät växtlighet (Alt 1886),
//! latent minnesspår (Alt 1884), linjär prob på dold *T. rex* (Alt 1885b) och ablaterade prov (Alt 1885a/c).
//!
//! Helt isolerad modul: rör INTE `hjarna.rs`, `hjarna2008.rs`, `hjarnab1.rs`, `sim.rs` eller `formaga.rs`.

use crate::hjarna::{gauss, lott_bland_max};
use crate::klimat::Klimat;
use crate::rng::Rng;
use crate::sim::{ar_hav, landandel_av};
use crate::svep::{LAT0, LON0, T_KARTA};
use crate::tal::{
    vmax_ruta, A_N, AT_DINO, E_DELA, E_HANE, E_START, E_VAXT, KOST_FLOD, KOST_SJO, KOST_STEG,
    KOST_TRASK, LITEN, LIV, SIGMA0, SIGMA_MUT, STEG_PER_AR, TABELL_SLUMP_P, TABELL_VIKT, V0_ANDEL,
};
use hckarta::{karta_full_version, Kalla, Karta, Omrade, Tidtabell, Vatten};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Fysik- och arkitekturkonstanter (matchar HcsimJepaB2.lean)
// ---------------------------------------------------------------------------

pub const B2_B: usize = 64;
pub const B2_H: usize = 64;
pub const B2_RUTA_M: f64 = 250.0;

/// Vindadvektion och avklingning (verifierade i HcsimJepaB2.lean: 0 < GAMMA < 1, 0 < BETA <= 1).
pub const GAMMA_LUKT: f64 = 0.80;
pub const BETA_VIND: f64 = 0.65;
pub const VIND_BYT_P: f64 = 0.02;

/// Bytesdjurets 57 blinda sinnen:
/// 0..9 vaxt, 9..18 djur, 18 energi, 19..28 kron, 28..37 sikt_rov,
/// 37..46 lukt_rov, 46..55 lukt_harmlos, 55..57 vind (wx, wy).
pub const S_B2: usize = 57;
pub const Z_B2: usize = 10;
/// I B2_Minne behåller axlar 0..Z_NU knivskarpt nu-tillstånd medan axlar Z_NU..Z_B2 bär exponentiellt minnesspår.
pub const Z_NU: usize = 6;
/// Dubblad frusen kodare för diagnosarm D2_Bred_Z20 (Alt 2370).
pub const Z_B2_20: usize = 20;
pub const Z_NU_20: usize = 12;
/// Alt 2379 (Arm B & D): Gles icke-negativ LpWM-kodare (16 nu-axlar med top-6 gleshet + 8 minnesspår-axlar).
pub const Z_GLES: usize = 24;
pub const Z_GLES_NU: usize = 16;
pub const GLES_TOP_K: usize = 6;
pub const P_TAK_GLES: f64 = 40.0;
const CHOCK_BUFFERT_MAX: usize = 24;

/// *T. rex* 39 blinda sinnen:
/// 0..9 sikt_byte, 9..18 lukt_byte, 18..27 kron, 27..36 sikt_trex, 36..38 vind, 38 egen energi.
pub const S_TREX: usize = 39;
pub const Z_TREX: usize = 6;

pub const BETA_MINNE_AKTIV: f64 = 0.25;

const ETA_NORM: f64 = 0.02;
const ETA_ZNORM: f64 = 0.05;
const GRIND_GAMMA: f32 = 0.995;
const GRIND_MIN_N: f32 = 5.0;
const GRIND_STRAFF: f32 = 3.0;
const GRIND_MATTNAD: f64 = 0.35;
const GRIND_AKTIV_MIN: f64 = 0.20;
const LAMBDA_RLS: f64 = 0.99;
const LAMBDA_KRIT: f64 = 0.99;
const ALPHA_RLS: f64 = 4.0;
const ALPHA_KRIT: f64 = 4.0;
/// Kausal TD(0)-diskontering för livstidskritikern (propagerar smärta/föda bakåt över doftplymen, helt blint).
const GAMMA_TD: f64 = 0.55;
/// Långsam målviktsuppdatering (target network) för stabil RLS-TD(0).
const ETA_KRIT_MAL: f64 = 0.05;

const SIGMA0_W_SKADA: f64 = 5.0;
const SIGMA_MUT_W_SKADA: f64 = 1.5;
const W_SKADA_MAX: f64 = 50.0;
const UTFORSK_P: f64 = 0.20;

/// 9 handlingar: 0 = stå still, 1..=8 = 8 väderstreck (N, NÖ, Ö, SÖ, S, SV, V, NV).
pub const DIR: [(i32, i32); A_N] = [
    (0, 0),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Enhetsvektor för de 8 vindriktningarna (vind_dir ∈ 0..8 motsvarar DIR[vind_dir + 1]).
pub fn vind_vektor(vind_dir: usize) -> (f64, f64) {
    let (dx, dy) = DIR[(vind_dir % 8) + 1];
    let n = ((dx * dx + dy * dy) as f64).sqrt();
    (dx as f64 / n, dy as f64 / n)
}

/// Lean 4-verifierad synbarhet `(1 - k)^2` för `k ∈ [0, 1]`.
#[inline]
pub fn synbarhet(kron: f64) -> f64 {
    let d = (1.0 - kron).clamp(0.0, 1.0);
    d * d
}

/// Lean 4-verifierat luktadvektionssteg `γ * ((1 - β) * l_egen + β * l_uppvind) + utslapp`.
#[inline]
pub fn lukt_steg_scalar(gamma: f64, beta: f64, l_egen: f64, l_uppvind: f64, utslapp: f64) -> f64 {
    gamma * ((1.0 - beta) * l_egen + beta * l_uppvind) + utslapp
}

/// Lean 4-verifierat latent minnesspår `h' = (1 - β) * h + β * z`.
#[inline]
pub fn minnes_spar_scalar(beta: f64, h: f64, z: f64) -> f64 {
    (1.0 - beta) * h + beta * z
}

// ---------------------------------------------------------------------------
// Parametrar för Skede B2-miljön (kalibreras på 261–265, låses före 20 världar)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct B2Param {
    /// Skalfaktor på primärproduktionen i 16×16 km flodslätt (ger ~1 100–1 500 byten).
    pub vaxt_skala: f64,
    /// Startantal *T. rex* (och lika många ofarliga doftkällor).
    pub start_trex: usize,
    /// Minsta golv för *T. rex* (invandring från omgivande Hell Creek om antal faller under golvet).
    pub min_trex: usize,
    /// Högsta tak för *T. rex*-populationen i 16×16 km-arenan.
    pub max_trex: usize,
    /// Grundsannolikhet för fysiskt angrepp när en *T. rex* delar 250 m-ruta med ett byte.
    pub p_angrepp_bas: f64,
    /// Multiplikator på angreppschans vid bakhåll i tät växtlighet (`kron >= kron_dold_grans`).
    pub bakhall_faktor: f64,
    /// Krontäckesgräns där en *T. rex* i grannruta blir 100 % dold för synen (`sikt_rov = 0`).
    pub kron_dold_grans: f64,
    /// Sannolikhet att en träff är direkt dödlig (annars överlever bytet med skada = 1.0).
    pub p_dod_vid_traff: f64,
    /// Energiförlust för bytet vid icke-dödlig träff.
    pub byte_skada_energi: f64,
    /// Energi som *T. rex* får vid dödligt byte.
    pub trex_byte_energi: f64,
    /// Energi som *T. rex* får vid icke-dödligt angrepp.
    pub trex_traff_energi: f64,
    /// *T. rex* baskostnad per steg.
    pub trex_kost_steg: f64,
    /// *T. rex* startenergi och delningsenergi.
    pub trex_e_start: f64,
    pub trex_e_dela: f64,
    /// *T. rex* mättnadsvila (steg) efter ett dödligt byte.
    pub trex_vila_steg: u16,
    /// Om skymd sikt och bakhållsbonus är aktiva (avstängt i kontrollarmen Alt 1886).
    pub skymd_sikt: bool,
    /// Om bytesdjurens hjärna använder villkorlig blind belöningskoppling i kanalgrinden (Alt 2278).
    pub belonings_grind: bool,
    /// Fysisk luktavklingning och vindadvektion (standard 0.80 / 0.65 / 0.60; kan skärpas i Alt 2373).
    #[serde(default = "default_gamma_lukt")]
    pub gamma_lukt: f64,
    #[serde(default = "default_beta_vind")]
    pub beta_vind: f64,
    #[serde(default = "default_kon_mitt")]
    pub kon_mitt: f64,
}

fn default_gamma_lukt() -> f64 { GAMMA_LUKT }
fn default_beta_vind() -> f64 { BETA_VIND }
fn default_kon_mitt() -> f64 { 0.60 }

impl B2Param {
    /// Skärpt spelplansprofil för Alt 2373 (kalibrerad på 261–265 via ärlig Fuskflykt > 60 %, 5/5):
    /// mer lokal, skarp doftplym + biologiskt farliga och lärbara T. rex-bakhåll.
    pub fn v2_skarp() -> Self {
        Self {
            vaxt_skala: 6.5,
            start_trex: 14,
            min_trex: 6,
            max_trex: 22,
            p_angrepp_bas: 0.10,
            bakhall_faktor: 5.0,
            kron_dold_grans: 0.50,
            p_dod_vid_traff: 0.45,
            byte_skada_energi: 48.0,
            trex_byte_energi: 85.0,
            trex_traff_energi: 20.0,
            trex_kost_steg: 0.90,
            trex_e_start: 160.0,
            trex_e_dela: 340.0,
            trex_vila_steg: 18,
            skymd_sikt: true,
            belonings_grind: false,
            gamma_lukt: 0.55,
            beta_vind: 0.72,
            kon_mitt: 0.76,
        }
    }
}

impl Default for B2Param {
    fn default() -> Self {
        Self {
            vaxt_skala: 6.0,
            start_trex: 16,
            min_trex: 6,
            max_trex: 24,
            p_angrepp_bas: 0.06,
            bakhall_faktor: 4.0,
            kron_dold_grans: 0.50,
            p_dod_vid_traff: 0.20,
            byte_skada_energi: 24.0,
            trex_byte_energi: 95.0,
            trex_traff_energi: 22.0,
            trex_kost_steg: 0.90,
            trex_e_start: 160.0,
            trex_e_dela: 340.0,
            trex_vila_steg: 14,
            skymd_sikt: true,
            belonings_grind: false,
            gamma_lukt: GAMMA_LUKT,
            beta_vind: BETA_VIND,
            kon_mitt: 0.60,
        }
    }
}

// ---------------------------------------------------------------------------
// Generisk RLS-hjälpare för valfri latent dimension Z
// ---------------------------------------------------------------------------

fn diag_p<const Z: usize>(alpha: f64) -> [[f64; Z]; Z] {
    let mut p = [[0.0; Z]; Z];
    let inv = 1.0 / alpha;
    for i in 0..Z {
        p[i][i] = inv;
    }
    p
}

#[inline]
fn dot_arr<const N: usize>(a: &[f64; N], b: &[f64; N]) -> f64 {
    let mut s = 0.0;
    for i in 0..N {
        s += a[i] * b[i];
    }
    s
}

fn rls_uppdatera_mat<const Z: usize, const ZP1: usize>(
    w: &mut [[f64; ZP1]; Z],
    p: &mut [[f64; ZP1]; ZP1],
    x: &[f64; ZP1],
    target: &[f64; Z],
    pred: &[f64; Z],
    lambda: f64,
) {
    let mut px = [0.0; ZP1];
    for m in 0..ZP1 {
        let mut s = 0.0;
        for n in 0..ZP1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..ZP1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + xpx).max(1e-9);
    let mut gain = [0.0; ZP1];
    for m in 0..ZP1 {
        gain[m] = px[m] / denom;
    }
    for r in 0..Z {
        let err = target[r] - pred[r];
        for m in 0..ZP1 {
            w[r][m] += err * gain[m];
        }
    }
    let inv_l = 1.0 / lambda;
    for m in 0..ZP1 {
        for n in 0..ZP1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..ZP1 {
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

fn rls_uppdatera_skalar<const ZP1: usize>(
    w: &mut [f64; ZP1],
    p: &mut [[f64; ZP1]; ZP1],
    x: &[f64; ZP1],
    err: f64,
    lambda: f64,
) {
    let mut px = [0.0; ZP1];
    for m in 0..ZP1 {
        let mut s = 0.0;
        for n in 0..ZP1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..ZP1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + xpx).max(1e-9);
    let mut gain = [0.0; ZP1];
    for m in 0..ZP1 {
        gain[m] = px[m] / denom;
        w[m] += err * gain[m];
    }
    let inv_l = 1.0 / lambda;
    for m in 0..ZP1 {
        for n in 0..ZP1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..ZP1 {
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

/// Lean 4-verifierad symmetrisk kongruensskalning (`b2_sym_p_cap_preserves_sym_psd_and_bound`):
/// skalar både rad `i` och kolumn `j` med `d[i] = √(tak / p[i][i])` när `p[i][i] > tak`.
/// Bevarar exakt symmetri och positiv semidefinithet `P ≥ 0` (till skillnad från diagonalklipp).
pub fn sym_kongruens_p_tak<const D: usize>(p: &mut [[f64; D]; D], tak: f64) -> u64 {
    let mut d = [1.0; D];
    let mut slog_till = false;
    for i in 0..D {
        if p[i][i] > tak {
            d[i] = (tak / p[i][i]).sqrt();
            slog_till = true;
        }
    }
    if !slog_till {
        return 0;
    }
    for i in 0..D {
        for j in 0..D {
            p[i][j] *= d[i] * d[j];
        }
    }
    for i in 0..D {
        for j in 0..i {
            let sym = 0.5 * (p[i][j] + p[j][i]);
            p[i][j] = sym;
            p[j][i] = sym;
        }
    }
    1
}

/// Lean 4-verifierad positivt skalad ReLU (`b2_sparse_relu_nonneg_zero_bound`):
/// bevarar exakta nollor (`x <= 0 -> 0`) och ger alltid `>= 0`.
#[inline]
pub fn relu_pos_skala(x: f64, scale: f64) -> f64 {
    if x > 0.0 {
        x * scale
    } else {
        0.0
    }
}

fn rls_uppdatera_mat_symtak<const Z: usize, const ZP1: usize>(
    w: &mut [[f64; ZP1]; Z],
    p: &mut [[f64; ZP1]; ZP1],
    x: &[f64; ZP1],
    target: &[f64; Z],
    pred: &[f64; Z],
    lambda: f64,
    tak: f64,
) -> u64 {
    let mut px = [0.0; ZP1];
    for m in 0..ZP1 {
        let mut s = 0.0;
        for n in 0..ZP1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..ZP1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + xpx).max(1e-9);
    let mut gain = [0.0; ZP1];
    for m in 0..ZP1 {
        gain[m] = px[m] / denom;
    }
    for r in 0..Z {
        let err = target[r] - pred[r];
        for m in 0..ZP1 {
            w[r][m] += err * gain[m];
        }
    }
    let inv_l = 1.0 / lambda;
    for m in 0..ZP1 {
        for n in 0..ZP1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..ZP1 {
        for n in 0..m {
            let sym = 0.5 * (p[m][n] + p[n][m]);
            p[m][n] = sym;
            p[n][m] = sym;
        }
    }
    sym_kongruens_p_tak(p, tak)
}

/// Lean 4-verifierat viktat RLS-steg (`b2_weighted_replay_rls_invariants`, `c > 0`, utan `√c`)
/// kombinerat med valfritt symmetriskt kongruens-P-tak (`b2_sym_p_cap_preserves_sym_psd_and_bound`).
pub fn rls_uppdatera_skalar_viktad<const ZP1: usize>(
    w: &mut [f64; ZP1],
    p: &mut [[f64; ZP1]; ZP1],
    x: &[f64; ZP1],
    err: f64,
    lambda: f64,
    c: f64,
    tak: Option<f64>,
) -> u64 {
    let mut px = [0.0; ZP1];
    for m in 0..ZP1 {
        let mut s = 0.0;
        for n in 0..ZP1 {
            s += p[m][n] * x[n];
        }
        px[m] = s;
    }
    let mut xpx = 0.0;
    for m in 0..ZP1 {
        xpx += x[m] * px[m];
    }
    let denom = (lambda + c * xpx).max(1e-9);
    let mut gain = [0.0; ZP1];
    for m in 0..ZP1 {
        gain[m] = (c * px[m]) / denom;
        w[m] += err * gain[m];
    }
    let inv_l = 1.0 / lambda;
    for m in 0..ZP1 {
        for n in 0..ZP1 {
            p[m][n] = (p[m][n] - gain[m] * px[n]) * inv_l;
        }
    }
    for m in 0..ZP1 {
        for n in 0..m {
            let sym = 0.5 * (p[m][n] + p[n][m]);
            p[m][n] = sym;
            p[n][m] = sym;
        }
    }
    if let Some(t) = tak {
        sym_kongruens_p_tak(p, t)
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Bytesdjurets genom, hjärna (HjB2) och ärlig minnestabell (TabellB2)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GenB2 {
    pub wk: [[f64; S_B2 + 1]; Z_B2],
    pub v: [f64; Z_B2 + 1],
    /// Ärvd smärtvikt utan förkodat tecken (N(0, 5), symmetrisk kring 0).
    pub w_skada: f64,
}

impl GenB2 {
    pub fn slump(g: &mut Rng) -> Self {
        let mut wk = [[0.0; S_B2 + 1]; Z_B2];
        for rad in &mut wk {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut v = [0.0; Z_B2 + 1];
        for x in &mut v {
            *x = SIGMA0 * gauss(g);
        }
        let w_skada = (SIGMA0_W_SKADA * gauss(g)).clamp(-W_SKADA_MAX, W_SKADA_MAX);
        Self { wk, v, w_skada }
    }

    pub fn arv(mor: &GenB2, far: &GenB2, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ 0x2361_B200_5A4B_0001);
        let mut wk = [[0.0; S_B2 + 1]; Z_B2];
        for i in 0..Z_B2 {
            for j in 0..=S_B2 {
                let bas = if g.f() < 0.5 { mor.wk[i][j] } else { far.wk[i][j] };
                wk[i][j] = bas + SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut v = [0.0; Z_B2 + 1];
        for i in 0..=Z_B2 {
            let bas = if g.f() < 0.5 { mor.v[i] } else { far.v[i] };
            v[i] = bas + SIGMA_MUT * gauss(&mut g);
        }
        let bas_w = if g.f() < 0.5 { mor.w_skada } else { far.w_skada };
        let w_skada = (bas_w + SIGMA_MUT_W_SKADA * gauss(&mut g)).clamp(-W_SKADA_MAX, W_SKADA_MAX);
        Self { wk, v, w_skada }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ByteTyp {
    /// Bekräftad B1-kärna uppskalad till B2-sinnena (inget minnesspår: β = 1.0 på alla 10 axlar).
    Bas,
    /// Parallellt latent arbetsminne: axlar 0..6 = knivskarpt nuet z_t, axlar 6..10 = exponentiellt minnesspår (β = 0.25).
    Minne,
    /// Ablation: avstängd världsmodell (alla handlingar får medelförutsägelse).
    StrykWm,
    /// Ablation (Alt 1885a för Bas): rovdjurslukt nollad, utan minnesspår.
    StrykLukt,
    /// Ablation (Alt 1885a för Minne, utan confounder): exakt samma minnesspår som Minne men rovdjurslukt nollad.
    MinneStrykLukt,
    /// Kontroll (Alt 1885c): ärlig 9-handlings minnestabell med samma kroppssignal.
    Tabell,
    // --- Alt 2370 diagnosvarianter ---
    /// Variansbevarande kontroll: rumsförskjuten (i+B/2, j+H/2)-kopia av lukt_rov i kanal 37..46 (samma varians/förutsägbarhet, 0 T.rex-koppling).
    MinneFalskLukt,
    /// D0b (Arena-tak): B2_Minne + ren orakelflykt bort från lokal lukt_rov-gradient.
    TakOrakelFlykt,
    /// D0a (Signal-tak): 100 % blind B2_Minne vars kanal 37..46 innehåller sann närhet 1/(1+d_trex).
    TakSannNarhet,
    /// D2: Dubblad frusen kodare Z=20 (12 nu + 8 minne).
    MinneZ20,
    /// D2 kontroll: Dubblad frusen kodare Z=20 med variansbevarande falsk lukt.
    MinneFalskLuktZ20,
    /// D3: Högre kritiker-diskontering γ_td = 0.80.
    MinneGamma80,
    /// D3 kontroll: Högre γ_td = 0.80 med variansbevarande falsk lukt.
    MinneFalskLuktGamma80,
    /// D4: Ny helt blind sänkande belöningsgrind som dämpar kanaler utan belönings-/smärtsamvariation.
    MinneSankGrind,
    /// Alt 2373: Samma sänkande belöningsgrind men med variansbevarande falsk lukt (ren Alt 1885a utan confounder).
    MinneSankFalskLukt,
    /// Alt 2373: Samma sänkande belöningsgrind men med avstängd världsmodell (spegelkontroll).
    MinneSankStrykWm,
    // --- Alt 2379 diagnosarmar (enhällig agentkonsensus 4/4) ---
    /// Arm A: 100 % kroppsstyrd livstids-chockgrind (utan 0,995-glömska) + livstids omkodad chock-replay.
    ArmAChock,
    ArmAChockFalsk,
    /// Arm B: Gles icke-negativ ReLU/top-k-kodare (Z=24, positiv skalning) + symmetriskt P-tak + sänkande grind.
    ArmBGles,
    ArmBGlesFalsk,
    /// Arm C: Liten online-kodare (L4: kritikergradient + IDM + anti-kollaps) matad ur livstids-chockreserv.
    ArmCOnline,
    ArmCOnlineFalsk,
    /// Arm D: Kombination Arm A + Arm B (livstids-chockgrind/replay + gles ReLU/top-k med symmetriskt P-tak).
    ArmDKomb,
    ArmDKombFalsk,
}

#[derive(Clone, Copy, Debug)]
pub struct HjB2Config {
    pub typ: ByteTyp,
    pub belonings_grind: bool,
    pub sankande_grind: bool,
    pub chock_grind: bool,
    pub chock_replay: bool,
    pub online_kodare: bool,
    pub gamma_td: f64,
    pub r_klamp: f64,
}

impl HjB2Config {
    pub fn ny(typ: ByteTyp, belonings_grind: bool) -> Self {
        let gamma_td = match typ {
            ByteTyp::MinneGamma80 | ByteTyp::MinneFalskLuktGamma80 => 0.80,
            _ => GAMMA_TD,
        };
        let sankande_grind = matches!(
            typ,
            ByteTyp::MinneSankGrind
                | ByteTyp::MinneSankFalskLukt
                | ByteTyp::MinneSankStrykWm
                | ByteTyp::ArmAChock
                | ByteTyp::ArmAChockFalsk
                | ByteTyp::ArmBGles
                | ByteTyp::ArmBGlesFalsk
                | ByteTyp::ArmCOnline
                | ByteTyp::ArmCOnlineFalsk
                | ByteTyp::ArmDKomb
                | ByteTyp::ArmDKombFalsk
        );
        let chock_grind = matches!(
            typ,
            ByteTyp::ArmAChock
                | ByteTyp::ArmAChockFalsk
                | ByteTyp::ArmCOnline
                | ByteTyp::ArmCOnlineFalsk
                | ByteTyp::ArmDKomb
                | ByteTyp::ArmDKombFalsk
        );
        let chock_replay = chock_grind;
        let online_kodare = matches!(typ, ByteTyp::ArmCOnline | ByteTyp::ArmCOnlineFalsk);
        Self {
            typ,
            belonings_grind,
            sankande_grind,
            chock_grind,
            chock_replay,
            online_kodare,
            gamma_td,
            r_klamp: 50.0,
        }
    }

    pub fn har_minne(&self) -> bool {
        matches!(
            self.typ,
            ByteTyp::Minne
                | ByteTyp::MinneStrykLukt
                | ByteTyp::MinneFalskLukt
                | ByteTyp::TakOrakelFlykt
                | ByteTyp::TakSannNarhet
                | ByteTyp::MinneZ20
                | ByteTyp::MinneFalskLuktZ20
                | ByteTyp::MinneGamma80
                | ByteTyp::MinneFalskLuktGamma80
                | ByteTyp::MinneSankGrind
                | ByteTyp::MinneSankFalskLukt
                | ByteTyp::MinneSankStrykWm
                | ByteTyp::ArmAChock
                | ByteTyp::ArmAChockFalsk
                | ByteTyp::ArmBGles
                | ByteTyp::ArmBGlesFalsk
                | ByteTyp::ArmCOnline
                | ByteTyp::ArmCOnlineFalsk
                | ByteTyp::ArmDKomb
                | ByteTyp::ArmDKombFalsk
        )
    }

    pub fn stryk_wm(&self) -> bool {
        matches!(self.typ, ByteTyp::StrykWm | ByteTyp::MinneSankStrykWm)
    }

    pub fn stryk_lukt(&self) -> bool {
        matches!(self.typ, ByteTyp::StrykLukt | ByteTyp::MinneStrykLukt)
    }
}

#[derive(Clone, Debug)]
pub struct HjB2 {
    pub cfg: HjB2Config,
    pub wk: [[f64; S_B2 + 1]; Z_B2],
    pub w_rls_akt: [[[f64; Z_B2 + 1]; Z_B2]; A_N],
    pub p_rls_akt: [[[f64; Z_B2 + 1]; Z_B2 + 1]; A_N],
    pub v: [f64; Z_B2 + 1],
    pub w_krit: [f64; Z_B2 + 1],
    pub w_krit_mal: [f64; Z_B2 + 1],
    pub p_krit: [[f64; Z_B2 + 1]; Z_B2 + 1],
    pub vanta_beloning: Option<f64>,
    pub td_forra: Option<([f64; Z_B2], f64)>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    pub skala_init: bool,
    pub mu: [f64; S_B2],
    pub var: [f64; S_B2],
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_B2],
    pub var_z: [f64; Z_B2],
    /// Latent minnesspår h_t (identisk med z_t när beta_minne == 1.0).
    pub h_init: bool,
    pub h_spar: [f64; Z_B2],
    /// Skärpt blind KanalGrind (f32-ackumulatorer för hastighet och minne).
    pub grind_n: [f32; A_N],
    pub grind_sy: [[f32; S_B2]; A_N],
    pub grind_sy2: [[f32; S_B2]; A_N],
    pub grind_sx: [[f32; S_B2]; A_N],
    pub grind_sx2: [[f32; S_B2]; A_N],
    pub grind_syx: Vec<[[f32; S_B2]; S_B2]>,
    pub grind_adj: [[f32; S_B2]; A_N],
    /// Valfritt helt blint belöningskopplat tillskott per kanal (Alt 2278 / Alt 2370 sänkande grind).
    pub bel_n: f32,
    pub bel_sr: f32,
    pub bel_sr2: f32,
    pub bel_sa: f32,
    pub bel_sa2: f32,
    pub bel_sx: [f32; S_B2],
    pub bel_sx2: [f32; S_B2],
    pub bel_srx: [f32; S_B2],
    pub bel_sax: [f32; S_B2],
    pub kanal_vikt: [f64; S_B2],
    pub forra_ra_sk: Option<[f32; S_B2]>,
    pub forra: Option<([f64; Z_B2], usize, [f64; Z_B2])>,
    pub steg: u64,
    // Alt 2379: Livstids-chockgrind (fryst, utan 0,995-glömska), omkodad chock-replay och online-kodare (Arm A & C)
    pub chock_golv: [f64; S_B2],
    pub vanta_chock: bool,
    pub forra_s_ra: Option<[f32; S_B2]>,
    pub td_s_ra: Option<([f32; S_B2], usize)>,
    pub chock_minne: Vec<([f32; S_B2], f32, [f32; S_B2], u8)>,
    pub lugn_minne: Vec<([f32; S_B2], f32, [f32; S_B2], u8)>,
    pub replay_pekare: usize,
    pub lugn_pekare: usize,
    pub p_tak_traffar: u64,
    pub w_idm: Option<Box<[[f64; 2 * Z_B2 + 1]; A_N]>>,
}

impl HjB2 {
    pub fn fran_gen(gen: &GenB2, cfg: HjB2Config) -> Self {
        let inv_sqrt_s = 1.0 / (S_B2 as f64).sqrt();
        let mut wk = [[0.0; S_B2 + 1]; Z_B2];
        for i in 0..Z_B2 {
            for j in 0..=S_B2 {
                wk[i][j] = gen.wk[i][j] * inv_sqrt_s;
            }
        }
        Self {
            cfg,
            wk,
            w_rls_akt: [[[0.0; Z_B2 + 1]; Z_B2]; A_N],
            p_rls_akt: [diag_p::<{ Z_B2 + 1 }>(ALPHA_RLS); A_N],
            v: gen.v,
            w_krit: [0.0; Z_B2 + 1],
            w_krit_mal: [0.0; Z_B2 + 1],
            p_krit: diag_p::<{ Z_B2 + 1 }>(ALPHA_KRIT),
            vanta_beloning: None,
            td_forra: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_B2],
            var: [1.0; S_B2],
            z_skala_init: false,
            mu_z: [0.0; Z_B2],
            var_z: [1.0; Z_B2],
            h_init: false,
            h_spar: [0.0; Z_B2],
            grind_n: [0.0; A_N],
            grind_sy: [[0.0; S_B2]; A_N],
            grind_sy2: [[0.0; S_B2]; A_N],
            grind_sx: [[0.0; S_B2]; A_N],
            grind_sx2: [[0.0; S_B2]; A_N],
            grind_syx: vec![[[0.0; S_B2]; S_B2]; A_N],
            grind_adj: [[0.0; S_B2]; A_N],
            bel_n: 0.0,
            bel_sr: 0.0,
            bel_sr2: 0.0,
            bel_sa: 0.0,
            bel_sa2: 0.0,
            bel_sx: [0.0; S_B2],
            bel_sx2: [0.0; S_B2],
            bel_srx: [0.0; S_B2],
            bel_sax: [0.0; S_B2],
            kanal_vikt: [1.0; S_B2],
            forra_ra_sk: None,
            forra: None,
            steg: 0,
            chock_golv: [0.0; S_B2],
            vanta_chock: false,
            forra_s_ra: None,
            td_s_ra: None,
            chock_minne: Vec::new(),
            lugn_minne: Vec::new(),
            replay_pekare: 0,
            lugn_pekare: 0,
            p_tak_traffar: 0,
            w_idm: if cfg.online_kodare {
                Some(Box::new([[0.0; 2 * Z_B2 + 1]; A_N]))
            } else {
                None
            },
        }
    }

    pub fn ge_beloning(&mut self, r: f64) {
        self.vanta_beloning = Some(r);
        self.vanta_chock = false;
        if self.cfg.belonings_grind || self.cfg.sankande_grind {
            if let Some(s_prev) = self.forra_ra_sk {
                let rf = r.clamp(-self.cfg.r_klamp, self.cfg.r_klamp) as f32;
                // Kausal statistik FÖRE steget (Fables krav 3.4 i Alt 2377!)
                let mr_prev = if self.bel_n > 1.0 { self.bel_sr / self.bel_n } else { 0.0 };
                let vr_prev = if self.bel_n > 2.0 {
                    (self.bel_sr2 / self.bel_n - mr_prev * mr_prev).max(1e-4)
                } else {
                    1.0
                };
                let af = (rf - mr_prev).abs();
                if self.cfg.chock_grind && self.bel_n >= 40.0 && af > 3.0 * vr_prev.sqrt() {
                    self.vanta_chock = true;
                    // Fryst livstids-chockgolv (Lean 4 `chockGrindFaktor`, 0 % 0,995-glömska):
                    // kanaler som avviker kraftigt (`z^2 > 1.5`) vid kroppens egen chock låses permanent uppåt.
                    for j in 0..S_B2 {
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
                for j in 0..S_B2 {
                    let x = s_prev[j];
                    self.bel_sx[j] = GRIND_GAMMA * self.bel_sx[j] + x;
                    self.bel_sx2[j] = GRIND_GAMMA * self.bel_sx2[j] + x * x;
                    self.bel_srx[j] = GRIND_GAMMA * self.bel_srx[j] + rf * x;
                    self.bel_sax[j] = GRIND_GAMMA * self.bel_sax[j] + af * x;
                }
            }
        }
    }

    fn skala(&self, s: &[f64; S_B2]) -> [f64; S_B2] {
        let mut ut = [0.0; S_B2];
        if !self.skala_init {
            return ut;
        }
        for j in 0..S_B2 {
            ut[j] = ((s[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    fn uppdatera_skala(&mut self, s: &[f64; S_B2]) {
        if !self.skala_init {
            self.mu = *s;
            self.var = [1.0; S_B2];
            self.skala_init = true;
            return;
        }
        for j in 0..S_B2 {
            let d = s[j] - self.mu[j];
            self.mu[j] += ETA_NORM * d;
            self.var[j] = (1.0 - ETA_NORM) * self.var[j] + ETA_NORM * d * d;
        }
    }

    fn berakna_grind_rad(&self, a: usize) -> [f32; S_B2] {
        let mut rad = [0.0f32; S_B2];
        let n = self.grind_n[a];
        if n < GRIND_MIN_N {
            return rad;
        }
        let inv_n = 1.0f32 / n;
        let straff = GRIND_STRAFF * inv_n;
        let mut mx = [0.0f32; S_B2];
        let mut inv_vx = [0.0f32; S_B2];
        for k in 0..S_B2 {
            let m = self.grind_sx[a][k] * inv_n;
            mx[k] = m;
            let vx = self.grind_sx2[a][k] * inv_n - m * m;
            inv_vx[k] = if vx >= 1e-6 { 1.0 / vx } else { 0.0 };
        }
        let syx_a = &self.grind_syx[a];
        for j in 0..S_B2 {
            let my = self.grind_sy[a][j] * inv_n;
            let vy = self.grind_sy2[a][j] * inv_n - my * my;
            if vy < 1e-6 {
                continue;
            }
            let inv_vy = 1.0 / vy;
            let row = &syx_a[j];
            let mut best_r2 = 0.0f32;
            for k in 0..S_B2 {
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

    fn uppdatera_kanalgrind(&mut self, s_sk_nu_f32: &[f32; S_B2]) {
        let (Some(s_prev), Some((_, a_prev, _))) = (self.forra_ra_sk, self.forra) else {
            return;
        };
        self.grind_n[a_prev] = GRIND_GAMMA * self.grind_n[a_prev] + 1.0;
        let sy = &mut self.grind_sy[a_prev];
        let sy2 = &mut self.grind_sy2[a_prev];
        let sx = &mut self.grind_sx[a_prev];
        let sx2 = &mut self.grind_sx2[a_prev];
        let syx = &mut self.grind_syx[a_prev];
        for j in 0..S_B2 {
            let yj = s_sk_nu_f32[j];
            let xj = s_prev[j];
            sy[j] = GRIND_GAMMA * sy[j] + yj;
            sy2[j] = GRIND_GAMMA * sy2[j] + yj * yj;
            sx[j] = GRIND_GAMMA * sx[j] + xj;
            sx2[j] = GRIND_GAMMA * sx2[j] + xj * xj;
            let row = &mut syx[j];
            for k in 0..S_B2 {
                row[k] = GRIND_GAMMA * row[k] + yj * s_prev[k];
            }
        }
        if self.steg < 250 {
            if self.steg % 5 == 0 {
                self.grind_adj[a_prev] = self.berakna_grind_rad(a_prev);
            } else {
                return;
            }
        } else if self.steg % 25 == 0 {
            for a in 0..A_N {
                self.grind_adj[a] = self.berakna_grind_rad(a);
            }
        } else {
            return;
        }
        let mut poang = [0.0f64; S_B2];
        let mut antal = 0usize;
        for a in 0..A_N {
            if self.grind_n[a] >= GRIND_MIN_N {
                antal += 1;
                for j in 0..S_B2 {
                    poang[j] += self.grind_adj[a][j] as f64;
                }
            }
        }
        if antal > 0 {
            let inv_a = 1.0 / antal as f64;
            for j in 0..S_B2 {
                poang[j] *= inv_a;
            }
        }
        // Villkorlig blind belöningskoppling (Alt 2278, bara om påslagen)
        if self.cfg.belonings_grind && self.bel_n >= 15.0 {
            let inv_n = 1.0 / self.bel_n;
            let mr = self.bel_sr * inv_n;
            let vr = self.bel_sr2 * inv_n - mr * mr;
            if vr >= 1e-6 {
                let inv_vr = 1.0 / vr;
                let straff = 2.0 * inv_n;
                for j in 0..S_B2 {
                    let mx = self.bel_sx[j] * inv_n;
                    let vx = self.bel_sx2[j] * inv_n - mx * mx;
                    if vx >= 1e-6 {
                        let cov = self.bel_srx[j] * inv_n - mr * mx;
                        let r2 = (cov * cov * inv_vr / vx - straff).max(0.0) as f64;
                        // En kanal som förklarar kroppens egen belöning slipper stängas av grinden
                        let bel_poang = (r2 * 4.0).min(1.0);
                        if bel_poang > poang[j] {
                            poang[j] = bel_poang;
                        }
                    }
                }
            }
        }
        let max_p = poang.iter().copied().fold(0.0f64, f64::max);
        if max_p > GRIND_AKTIV_MIN {
            for j in 0..S_B2 {
                self.kanal_vikt[j] = (poang[j] / GRIND_MATTNAD).clamp(0.0, 1.0).powi(3);
            }
        } else {
            self.kanal_vikt = [1.0; S_B2];
        }
        // Alt 2370 punkt 4c: NY helt blind sänkande belöningsgrind som dämpar högförutsägbara men
        // belönings-/smärt-irrelevanta kanaler (t.ex. ofarlig lukt) mot golvet 0.25 utan etiketter.
        if self.cfg.sankande_grind && self.bel_n >= 40.0 {
            let inv_n = 1.0 / self.bel_n;
            let mr = self.bel_sr * inv_n;
            let vr = self.bel_sr2 * inv_n - mr * mr;
            let ma = self.bel_sa * inv_n;
            let va = self.bel_sa2 * inv_n - ma * ma;
            let straff = 1.5 * inv_n;
            for j in 0..S_B2 {
                let mx = self.bel_sx[j] * inv_n;
                let vx = self.bel_sx2[j] * inv_n - mx * mx;
                let mut best_r2 = 0.0f64;
                if vx >= 1e-6 {
                    if vr >= 1e-6 {
                        let cov = self.bel_srx[j] * inv_n - mr * mx;
                        let r2 = (cov * cov / (vr * vx) - straff).max(0.0) as f64;
                        if r2 > best_r2 { best_r2 = r2; }
                    }
                    if va >= 1e-6 {
                        let cov_a = self.bel_sax[j] * inv_n - ma * mx;
                        let r2_a = (cov_a * cov_a / (va * vx) - straff).max(0.0) as f64;
                        if r2_a > best_r2 { best_r2 = r2_a; }
                    }
                }
                let eff_r2 = if self.cfg.chock_grind {
                    best_r2.max(self.chock_golv[j])
                } else {
                    best_r2
                };
                let faktor = 0.25 + 0.75 * (eff_r2 / 0.008).clamp(0.0, 1.0);
                self.kanal_vikt[j] *= faktor;
            }
        }
    }

    fn kod_ra(&self, s_in: &[f64; S_B2]) -> [f64; Z_B2] {
        let mut z = [0.0; Z_B2];
        for i in 0..Z_B2 {
            let mut lin = self.wk[i][S_B2];
            for j in 0..S_B2 {
                lin += self.wk[i][j] * s_in[j];
            }
            z[i] = lin.tanh();
        }
        z
    }

    fn norm_z(&self, z_raw: &[f64; Z_B2]) -> [f64; Z_B2] {
        if !self.z_skala_init {
            return *z_raw;
        }
        let mut ut = [0.0; Z_B2];
        for i in 0..Z_B2 {
            ut[i] = ((z_raw[i] - self.mu_z[i]) / (self.var_z[i] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    fn uppdatera_z_skala(&mut self, z_raw: &[f64; Z_B2]) {
        if !self.z_skala_init {
            self.mu_z = *z_raw;
            self.var_z = [0.25; Z_B2];
            self.z_skala_init = true;
            return;
        }
        for i in 0..Z_B2 {
            let d = z_raw[i] - self.mu_z[i];
            self.mu_z[i] += ETA_ZNORM * d;
            self.var_z[i] = (1.0 - ETA_ZNORM) * self.var_z[i] + ETA_ZNORM * d * d;
        }
    }

    fn x_av(h: &[f64; Z_B2]) -> [f64; Z_B2 + 1] {
        let mut x = [1.0; Z_B2 + 1];
        x[..Z_B2].copy_from_slice(h);
        x
    }

    fn forutsag(&self, h: &[f64; Z_B2], a: usize) -> [f64; Z_B2] {
        let x = Self::x_av(h);
        let mut y = [0.0; Z_B2];
        for r in 0..Z_B2 {
            y[r] = h[r] + dot_arr(&self.w_rls_akt[a][r], &x);
        }
        y
    }

    fn forutsag_medel(&self, h: &[f64; Z_B2]) -> [f64; Z_B2] {
        let mut y = [0.0; Z_B2];
        for a in 0..A_N {
            let ya = self.forutsag(h, a);
            for r in 0..Z_B2 {
                y[r] += ya[r] / A_N as f64;
            }
        }
        y
    }

    pub fn varde(&self, y: &[f64; Z_B2]) -> f64 {
        let mut s = self.v[Z_B2] + self.w_krit[Z_B2];
        for j in 0..Z_B2 {
            s += (self.v[j] + self.w_krit[j]) * y[j];
        }
        s
    }

    fn varde_mal(&self, y: &[f64; Z_B2]) -> f64 {
        let mut s = self.v[Z_B2] + self.w_krit_mal[Z_B2];
        for j in 0..Z_B2 {
            s += (self.v[j] + self.w_krit_mal[j]) * y[j];
        }
        s
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

    fn minst_besokta_handling(&self, g: &mut Rng) -> usize {
        let mut neg = [0.0; A_N];
        for a in 0..A_N {
            neg[a] = -(self.grind_n[a] as f64);
        }
        lott_bland_max(&neg, g)
    }

    pub fn g_intervall(&self, start: usize, len: usize) -> f64 {
        self.kanal_vikt[start..start + len].iter().sum::<f64>() / len as f64
    }

    pub fn senaste_skalade(&self) -> [f64; S_B2] {
        match self.forra_ra_sk {
            Some(s) => std::array::from_fn(|j| s[j] as f64),
            None => [0.0; S_B2],
        }
    }

    /// Kör ett komplett kausalt tankesteg och returnerar (vald handling, aktuell inre bild h_t).
    pub fn valj_lott(&mut self, s_nu_ra: &[f64; S_B2], g: &mut Rng) -> (usize, [f64; Z_B2]) {
        self.valj_lott_intern(s_nu_ra, None, g)
    }

    /// Ärligt bantest (Fuskflykt): exakt samma JEPA-hjärna och födoval som motståndaren,
    /// men förbjuder steg in i stark rovdjurslukt (> 0.10) och väljer bästa JEPA-poäng bland säkra steg.
    /// Nolltillgång till rå föda s[0..9] eller terräng — prövar rent om luktundvikning lönar sig på spelplanen.
    pub fn valj_lott_med_fuskflykt(
        &mut self,
        s_nu_ra: &[f64; S_B2],
        lukt_rov_9: &[f64; A_N],
        g: &mut Rng,
    ) -> (usize, [f64; Z_B2]) {
        self.valj_lott_intern(s_nu_ra, Some(lukt_rov_9), g)
    }

    fn valj_lott_intern(
        &mut self,
        s_nu_ra: &[f64; S_B2],
        lukt_veto: Option<&[f64; A_N]>,
        g: &mut Rng,
    ) -> (usize, [f64; Z_B2]) {
        let mut s_nu = *s_nu_ra;
        if self.cfg.stryk_lukt() {
            // Alt 1885a: rovdjurslukten nollad (medan ofarlig lukt och övriga sinnen finns kvar)
            for j in 37..46 {
                s_nu[j] = 0.0;
            }
        }
        if !self.skala_init {
            self.uppdatera_skala(&s_nu);
        }
        let s_sk = self.skala(&s_nu);
        let s_sk_f32: [f32; S_B2] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B2];
        for j in 0..S_B2 {
            s_in[j] = s_sk[j] * self.kanal_vikt[j];
        }

        let z_raw_now = self.kod_ra(&s_in);
        let z_now = self.norm_z(&z_raw_now);
        let har_minne = self.cfg.har_minne();
        let h_now = if !self.h_init || !har_minne {
            z_now
        } else {
            std::array::from_fn(|r| {
                if r < Z_NU {
                    // Axlar 0..Z_NU: skarpt nu-tillstånd
                    z_now[r]
                } else {
                    // Axlar Z_NU..Z_B2: kvardröjande latent arbetsminne (Lean 4 minnesSpar)
                    minnes_spar_scalar(BETA_MINNE_AKTIV, self.h_spar[r], z_now[r])
                }
            })
        };

        // 1. Lär från föregående steg (kausalt före statistikuppdatering)
        if let Some((h_prev, a_prev, _y_prev)) = self.forra {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let klamp = self.cfg.r_klamp.max(1.0);
                let r_nu = r_raw.clamp(-klamp, klamp);
                // Kausal TD(0)-uppdatering av föregående tillstånd (h_td, r_td) med nästa tillstånd h_now:
                // y_td(h_td) = r_td + GAMMA_TD * V_mal(h_now). Propagerar smärta/föda bakåt över doftplymen!
                if let Some((h_td, r_td)) = self.td_forra {
                    let v_nasta = self.varde_mal(&h_now).clamp(-klamp, klamp);
                    let y_td = (r_td + self.cfg.gamma_td * v_nasta).clamp(-klamp, klamp);
                    let pred_k = self.varde(&h_td);
                    let err_k = y_td - pred_k;
                    if self.steg >= 80 {
                        self.krit_n += 1;
                        self.krit_sy += y_td;
                        self.krit_sy2 += y_td * y_td;
                        self.krit_se2 += err_k * err_k;
                    }
                    let x_k = Self::x_av(&h_td);
                    if self.cfg.chock_replay {
                        self.p_tak_traffar += rls_uppdatera_skalar_viktad(
                            &mut self.w_krit,
                            &mut self.p_krit,
                            &x_k,
                            err_k,
                            LAMBDA_KRIT,
                            1.0,
                            Some(P_TAK_GLES),
                        );
                    } else {
                        rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
                    }
                    for m in 0..=Z_B2 {
                        self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
                    }
                }
                // Alt 2379 (Arm A & C): Spara råsinnen vid kroppschock (livstidsreserv, max 24 poster) + omedelbar chockpräglings-burst
                let s_now_f32: [f32; S_B2] = std::array::from_fn(|j| s_nu[j] as f32);
                if let Some(s_prev_f32) = self.forra_s_ra {
                    let post = (s_prev_f32, r_nu as f32, s_now_f32, a_prev as u8);
                    if self.vanta_chock && self.cfg.chock_replay {
                        if self.chock_minne.len() < CHOCK_BUFFERT_MAX {
                            self.chock_minne.push(post);
                        } else {
                            let idx = self.replay_pekare % CHOCK_BUFFERT_MAX;
                            self.chock_minne[idx] = post;
                        }
                        // Omedelbar prägling (3 viktade steg, c=2.0, lam=1.0) så redan ETT överlevt bett sätter spår!
                        for _ in 0..3 {
                            self.kor_chock_replay_post(post, 2.0);
                        }
                    } else if self.cfg.online_kodare {
                        if self.lugn_minne.len() < CHOCK_BUFFERT_MAX {
                            self.lugn_minne.push(post);
                        } else {
                            let idx = self.lugn_pekare % CHOCK_BUFFERT_MAX;
                            self.lugn_minne[idx] = post;
                        }
                        self.lugn_pekare = self.lugn_pekare.wrapping_add(1);
                    }
                }
                self.vanta_chock = false;
                // Periodisk livstids-chockåterspelning (begränsad frekvens: 1 sparat chocksteg per 10 vanliga steg)
                if self.cfg.chock_replay && self.steg % 10 == 0 && !self.chock_minne.is_empty() {
                    let idx = self.replay_pekare % self.chock_minne.len();
                    let post = self.chock_minne[idx];
                    self.replay_pekare = self.replay_pekare.wrapping_add(1);
                    self.kor_chock_replay_post(post, 1.0);
                }
                if self.cfg.online_kodare && self.steg % 10 == 5 {
                    self.kor_online_kodare_steg();
                }
                self.td_forra = Some((h_now, r_nu));
            }
            let x = Self::x_av(&h_prev);
            let delta_sann: [f64; Z_B2] = std::array::from_fn(|r| h_now[r] - h_prev[r]);
            let res_pred: [f64; Z_B2] =
                std::array::from_fn(|r| dot_arr(&self.w_rls_akt[a_prev][r], &x));
            if self.cfg.chock_replay {
                self.p_tak_traffar += rls_uppdatera_mat_symtak(
                    &mut self.w_rls_akt[a_prev],
                    &mut self.p_rls_akt[a_prev],
                    &x,
                    &delta_sann,
                    &res_pred,
                    LAMBDA_RLS,
                    P_TAK_GLES,
                );
            } else {
                rls_uppdatera_mat(
                    &mut self.w_rls_akt[a_prev],
                    &mut self.p_rls_akt[a_prev],
                    &x,
                    &delta_sann,
                    &res_pred,
                    LAMBDA_RLS,
                );
            }
        } else {
            let _ = self.vanta_beloning.take();
        }

        // 2. Uppdatera blind KanalGrind, sinnesskala, z-skala och minnesspår
        self.uppdatera_kanalgrind(&s_sk_f32);
        self.uppdatera_skala(&s_nu);
        self.uppdatera_z_skala(&z_raw_now);
        self.h_spar = h_now;
        self.h_init = true;

        // 3. Planera handling via de 9 världsmodellerna + lärd kritiker
        let mut poang = [0.0; A_N];
        let mut forut = [[0.0; Z_B2]; A_N];
        if self.cfg.stryk_wm() {
            let y_m = self.forutsag_medel(&h_now);
            let vm = self.varde(&y_m);
            for a in 0..A_N {
                forut[a] = y_m;
                poang[a] = vm;
            }
        } else {
            for a in 0..A_N {
                let ya = self.forutsag(&h_now, a);
                forut[a] = ya;
                poang[a] = self.varde(&ya);
            }
        }
        let mut a = if g.f() < UTFORSK_P {
            self.minst_besokta_handling(g)
        } else {
            lott_bland_max(&poang, g)
        };
        if let Some(lukt) = lukt_veto {
            if lukt[a] > 0.10 {
                let mut poang_saker = [f64::NEG_INFINITY; A_N];
                let mut har_saker = false;
                for k in 0..A_N {
                    if lukt[k] <= 0.10 {
                        poang_saker[k] = poang[k];
                        har_saker = true;
                    }
                }
                if har_saker {
                    a = lott_bland_max(&poang_saker, g);
                } else {
                    let mut neg_lukt = [0.0; A_N];
                    for k in 0..A_N {
                        neg_lukt[k] = -lukt[k];
                    }
                    a = lott_bland_max(&neg_lukt, g);
                }
            }
        }
        self.forra_ra_sk = Some(s_sk_f32);
        self.forra_s_ra = Some(std::array::from_fn(|j| s_nu[j] as f32));
        self.forra = Some((h_now, a, forut[a]));
        self.steg += 1;
        (a, h_now)
    }

    /// Omkodar sparade råsinnen genom aktuell skala, kanalgrind, kodare och z-norm (skydd mot S5-drift).
    fn omkoda_ra(&self, s_ra: &[f32; S_B2]) -> ([f64; S_B2], [f64; Z_B2], [f64; Z_B2]) {
        let s_f64: [f64; S_B2] = std::array::from_fn(|j| s_ra[j] as f64);
        let s_sk = self.skala(&s_f64);
        let mut s_in = [0.0; S_B2];
        for j in 0..S_B2 {
            s_in[j] = s_sk[j] * self.kanal_vikt[j];
        }
        let z_raw = self.kod_ra(&s_in);
        let z_now = self.norm_z(&z_raw);
        (s_in, z_raw, z_now)
    }

    /// Alt 2379 (Arm A): Livstids-chockåterspelning med omkodning av råsinnen och Lean 4-verifierat viktat RLS-steg.
    fn kor_chock_replay_post(&mut self, post: ([f32; S_B2], f32, [f32; S_B2], u8), c_vikt: f64) {
        let klamp = self.cfg.r_klamp.max(1.0);
        let r_ch = (post.1 as f64).clamp(-klamp, klamp);
        let (_, _, z_prev) = self.omkoda_ra(&post.0);
        let (_, _, z_now) = self.omkoda_ra(&post.2);
        // 1. Själva träffläget `z_now` får direkt chockmål `r_ch` (lär kritikern att steg in i farlig ruta = smärta)
        let x_now = Self::x_av(&z_now);
        let err_now = (r_ch - self.varde(&z_now)).clamp(-klamp, klamp);
        self.p_tak_traffar += rls_uppdatera_skalar_viktad(
            &mut self.w_krit,
            &mut self.p_krit,
            &x_now,
            err_now,
            1.0,
            c_vikt,
            Some(P_TAK_GLES),
        );
        // 2. Förvarningsläget `z_prev` får diskonterat TD-mål `γ · r_ch`
        let x_prev = Self::x_av(&z_prev);
        let y_prev = (self.cfg.gamma_td * r_ch).clamp(-klamp, klamp);
        let err_prev = (y_prev - self.varde(&z_prev)).clamp(-klamp, klamp);
        self.p_tak_traffar += rls_uppdatera_skalar_viktad(
            &mut self.w_krit,
            &mut self.p_krit,
            &x_prev,
            err_prev,
            1.0,
            c_vikt * 0.5,
            Some(P_TAK_GLES),
        );
        for m in 0..=Z_B2 {
            self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
        }
    }

    /// Alt 2379 (Arm C): Liten online-kodare (kritikergradient + IDM + anti-kollaps) matad ur livstids-chockreserv + lugna steg.
    fn kor_online_kodare_steg(&mut self) {
        if self.lugn_minne.len() < 8 {
            return;
        }
        let klamp = self.cfg.r_klamp.max(1.0);
        let mut batch: Vec<([f32; S_B2], f32, [f32; S_B2], u8)> = Vec::with_capacity(8);
        let n_ch = self.chock_minne.len().min(4);
        for k in 0..n_ch {
            let idx = (self.replay_pekare + k) % self.chock_minne.len();
            batch.push(self.chock_minne[idx]);
        }
        let n_lu = (8 - n_ch).min(self.lugn_minne.len());
        for k in 0..n_lu {
            let idx = (self.lugn_pekare + k) % self.lugn_minne.len();
            batch.push(self.lugn_minne[idx]);
        }
        let bs = batch.len() as f64;
        if bs < 2.0 {
            return;
        }

        let mut grad_wk = [[0.0f64; S_B2 + 1]; Z_B2];
        let mut z_sam = Vec::with_capacity(batch.len());
        let mut s_in_sam = Vec::with_capacity(batch.len());
        let mut z_raw_sam = Vec::with_capacity(batch.len());

        for &(s_prev, r_f32, s_now, a_u8) in &batch {
            let (s_in_p, z_raw_p, z_p) = self.omkoda_ra(&s_prev);
            let (s_in_n, z_raw_n, z_n) = self.omkoda_ra(&s_now);
            z_sam.push(z_n);
            s_in_sam.push(s_in_n);
            z_raw_sam.push(z_raw_n);

            // 1. Kritikergradient in i wk (riktar kodaren mot kroppschocker och matbelöning)
            let r_val = (r_f32 as f64).clamp(-klamp, klamp);
            let y_mal = (r_val + self.cfg.gamma_td * self.varde_mal(&z_n).clamp(-klamp, klamp)).clamp(-klamp, klamp);
            let err_k = ((y_mal - self.varde(&z_n)) / klamp).clamp(-1.0, 1.0);
            for i in 0..Z_B2 {
                let d_tanh = 1.0 - z_raw_n[i] * z_raw_n[i];
                let w_eff = (self.v[i] + self.w_krit[i]).clamp(-10.0, 10.0);
                let delta = (err_k * w_eff * d_tanh).clamp(-0.5, 0.5);
                for j in 0..S_B2 {
                    grad_wk[i][j] += 0.008 * delta * s_in_n[j] / bs;
                }
                grad_wk[i][S_B2] += 0.008 * delta / bs;
            }

            // 2. Invers dynamik (IDM: Toso/LeCun 2026 & AC-MTM 2026): gissa handlingen `a` ur [z_p, z_n - z_p, 1]
            if let Some(ref mut w_idm) = self.w_idm {
                let a_sann = (a_u8 as usize).min(A_N - 1);
                let mut feat = [0.0f64; 2 * Z_B2 + 1];
                for r in 0..Z_B2 {
                    feat[r] = z_p[r];
                    feat[Z_B2 + r] = z_n[r] - z_p[r];
                }
                feat[2 * Z_B2] = 1.0;
                let mut logits = [0.0f64; A_N];
                let mut lmax = f64::NEG_INFINITY;
                for a in 0..A_N {
                    let mut s = 0.0;
                    for m in 0..(2 * Z_B2 + 1) {
                        s += w_idm[a][m] * feat[m];
                    }
                    logits[a] = s;
                    if s > lmax { lmax = s; }
                }
                let mut sum_e = 0.0;
                let mut probs = [0.0f64; A_N];
                for a in 0..A_N {
                    probs[a] = (logits[a] - lmax).exp();
                    sum_e += probs[a];
                }
                for a in 0..A_N {
                    probs[a] /= sum_e.max(1e-12);
                }
                // Gradient mot IDM-huvud + tillbaka in i wk via z_n och z_p
                let mut d_zp = [0.0f64; Z_B2];
                let mut d_zn = [0.0f64; Z_B2];
                for a in 0..A_N {
                    let g_a = (if a == a_sann { 1.0 } else { 0.0 }) - probs[a];
                    for r in 0..Z_B2 {
                        d_zp[r] += g_a * (w_idm[a][r] - w_idm[a][Z_B2 + r]);
                        d_zn[r] += g_a * w_idm[a][Z_B2 + r];
                    }
                    for m in 0..(2 * Z_B2 + 1) {
                        w_idm[a][m] = (w_idm[a][m] + 0.025 * g_a * feat[m] / bs).clamp(-5.0, 5.0);
                    }
                }
                for i in 0..Z_B2 {
                    let dt_p = (1.0 - z_raw_p[i] * z_raw_p[i]) * d_zp[i].clamp(-0.5, 0.5);
                    let dt_n = (1.0 - z_raw_n[i] * z_raw_n[i]) * d_zn[i].clamp(-0.5, 0.5);
                    for j in 0..S_B2 {
                        grad_wk[i][j] += 0.004 * (dt_p * s_in_p[j] + dt_n * s_in_n[j]) / bs;
                    }
                }
            }
        }

        // 3. Lätt varians-/kovarians-kollapsskydd över mini-batchen + normtak på varje rad i wk
        let mut mz = [0.0f64; Z_B2];
        for z in &z_sam {
            for i in 0..Z_B2 { mz[i] += z[i] / bs; }
        }
        let mut vz = [0.0f64; Z_B2];
        for z in &z_sam {
            for i in 0..Z_B2 {
                let d = z[i] - mz[i];
                vz[i] += d * d / bs;
            }
        }
        for (idx_b, z) in z_sam.iter().enumerate() {
            let s_in = &s_in_sam[idx_b];
            let z_raw = &z_raw_sam[idx_b];
            for i in 0..Z_B2 {
                if vz[i] < 0.35 {
                    let push = (z[i] - mz[i]).signum() * (1.0 - z_raw[i] * z_raw[i]);
                    for j in 0..S_B2 {
                        grad_wk[i][j] += 0.002 * push * s_in[j] / bs;
                    }
                }
            }
        }
        for i in 0..Z_B2 {
            let mut n2 = 0.0;
            for j in 0..=S_B2 {
                self.wk[i][j] += grad_wk[i][j].clamp(-0.05, 0.05);
                n2 += self.wk[i][j] * self.wk[i][j];
            }
            let norm = n2.sqrt();
            if norm > 2.0 {
                let sc = 2.0 / norm;
                for j in 0..=S_B2 {
                    self.wk[i][j] *= sc;
                }
            }
        }
    }
}

/// Ärlig 9-handlings minnestabell för B2 (`Alt 1885c`), exakt samma tabellkonstruktion
/// som `Minnestabell` / `TabellB1` i `formaga.rs` (`Alt 1200` & `Alt 2282`), utan handkodat argmax.
#[derive(Clone, Debug)]
pub struct TabellB2 {
    medel: [f64; A_N],
    forra: Option<usize>,
}

impl Default for TabellB2 {
    fn default() -> Self {
        Self::ny()
    }
}

impl TabellB2 {
    pub fn ny() -> Self {
        Self {
            medel: [0.0; A_N],
            forra: None,
        }
    }

    pub fn uppdatera(&mut self, r: f64) {
        if let Some(a) = self.forra {
            self.medel[a] = (1.0 - TABELL_VIKT) * self.medel[a] + TABELL_VIKT * r.clamp(-50.0, 50.0);
        }
    }

    pub fn valj(&mut self, _s: &[f64; S_B2], g: &mut Rng) -> usize {
        let a = if g.f() < TABELL_SLUMP_P {
            g.under(A_N)
        } else {
            lott_bland_max(&self.medel, g)
        };
        self.forra = Some(a);
        a
    }
}

// ---------------------------------------------------------------------------
// Autonom *T. rex* med EGEN blind JEPA-hjärna (Alt 1877) — NOLL handkodad jakt!
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GenTrex {
    pub wk: [[f64; S_TREX + 1]; Z_TREX],
    pub v: [f64; Z_TREX + 1],
}

impl GenTrex {
    pub fn slump(g: &mut Rng) -> Self {
        let mut wk = [[0.0; S_TREX + 1]; Z_TREX];
        for rad in &mut wk {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut v = [0.0; Z_TREX + 1];
        for x in &mut v {
            *x = SIGMA0 * gauss(g);
        }
        Self { wk, v }
    }

    pub fn mutera(&self, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ 0x2361_74E8_0001);
        let mut wk = self.wk;
        for rad in &mut wk {
            for x in rad.iter_mut() {
                *x += SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut v = self.v;
        for x in &mut v {
            *x += SIGMA_MUT * gauss(&mut g);
        }
        Self { wk, v }
    }
}

#[derive(Clone, Debug)]
pub struct HjTrex {
    pub wk: [[f64; S_TREX + 1]; Z_TREX],
    pub w_rls_akt: [[[f64; Z_TREX + 1]; Z_TREX]; A_N],
    pub p_rls_akt: [[[f64; Z_TREX + 1]; Z_TREX + 1]; A_N],
    pub v: [f64; Z_TREX + 1],
    pub w_krit: [f64; Z_TREX + 1],
    pub w_krit_mal: [f64; Z_TREX + 1],
    pub p_krit: [[f64; Z_TREX + 1]; Z_TREX + 1],
    pub vanta_beloning: Option<f64>,
    pub td_forra: Option<([f64; Z_TREX], f64)>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    pub skala_init: bool,
    pub mu: [f64; S_TREX],
    pub var: [f64; S_TREX],
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_TREX],
    pub var_z: [f64; Z_TREX],
    pub besok: [f32; A_N],
    pub forra: Option<([f64; Z_TREX], usize)>,
    pub steg: u64,
}

impl HjTrex {
    pub fn fran_gen(gen: &GenTrex) -> Self {
        let inv_sqrt_s = 1.0 / (S_TREX as f64).sqrt();
        let mut wk = [[0.0; S_TREX + 1]; Z_TREX];
        for i in 0..Z_TREX {
            for j in 0..=S_TREX {
                wk[i][j] = gen.wk[i][j] * inv_sqrt_s;
            }
        }
        Self {
            wk,
            w_rls_akt: [[[0.0; Z_TREX + 1]; Z_TREX]; A_N],
            p_rls_akt: [diag_p::<{ Z_TREX + 1 }>(ALPHA_RLS); A_N],
            v: gen.v,
            w_krit: [0.0; Z_TREX + 1],
            w_krit_mal: [0.0; Z_TREX + 1],
            p_krit: diag_p::<{ Z_TREX + 1 }>(8.0),
            vanta_beloning: None,
            td_forra: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_TREX],
            var: [1.0; S_TREX],
            z_skala_init: false,
            mu_z: [0.0; Z_TREX],
            var_z: [1.0; Z_TREX],
            besok: [0.0; A_N],
            forra: None,
            steg: 0,
        }
    }

    pub fn ge_beloning(&mut self, r: f64) {
        self.vanta_beloning = Some(r);
    }

    fn x_av(z: &[f64; Z_TREX]) -> [f64; Z_TREX + 1] {
        let mut x = [1.0; Z_TREX + 1];
        x[..Z_TREX].copy_from_slice(z);
        x
    }

    fn varde(&self, y: &[f64; Z_TREX]) -> f64 {
        let mut s = self.v[Z_TREX] + self.w_krit[Z_TREX];
        for j in 0..Z_TREX {
            s += (self.v[j] + self.w_krit[j]) * y[j];
        }
        s
    }

    fn varde_mal(&self, y: &[f64; Z_TREX]) -> f64 {
        let mut s = self.v[Z_TREX] + self.w_krit_mal[Z_TREX];
        for j in 0..Z_TREX {
            s += (self.v[j] + self.w_krit_mal[j]) * y[j];
        }
        s
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

    pub fn valj_lott(&mut self, s_nu: &[f64; S_TREX], g: &mut Rng) -> usize {
        if !self.skala_init {
            self.mu = *s_nu;
            self.var = [1.0; S_TREX];
            self.skala_init = true;
        }
        let mut s_sk = [0.0; S_TREX];
        for j in 0..S_TREX {
            s_sk[j] = ((s_nu[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        let mut z_raw = [0.0; Z_TREX];
        for i in 0..Z_TREX {
            let mut lin = self.wk[i][S_TREX];
            for j in 0..S_TREX {
                lin += self.wk[i][j] * s_sk[j];
            }
            z_raw[i] = lin.tanh();
        }
        let z_now = if !self.z_skala_init {
            z_raw
        } else {
            std::array::from_fn(|i| {
                ((z_raw[i] - self.mu_z[i]) / (self.var_z[i] + 1e-4).sqrt()).clamp(-3.0, 3.0)
            })
        };

        if let Some((z_prev, a_prev)) = self.forra {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let r_nu = r_raw.clamp(-15.0, 25.0);
                if let Some((z_td, r_td)) = self.td_forra {
                    let v_nasta = self.varde_mal(&z_now).clamp(-25.0, 25.0);
                    let y_td = (r_td + GAMMA_TD * v_nasta).clamp(-25.0, 25.0);
                    let pred_k = self.varde(&z_td);
                    let err_k = y_td - pred_k;
                    if self.steg >= 60 {
                        self.krit_n += 1;
                        self.krit_sy += y_td;
                        self.krit_sy2 += y_td * y_td;
                        self.krit_se2 += err_k * err_k;
                    }
                    let x_k = Self::x_av(&z_td);
                    rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
                    for m in 0..=Z_TREX {
                        self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
                    }
                }
                self.td_forra = Some((z_now, r_nu));
            }
            let x = Self::x_av(&z_prev);
            let delta: [f64; Z_TREX] = std::array::from_fn(|r| z_now[r] - z_prev[r]);
            let pred: [f64; Z_TREX] =
                std::array::from_fn(|r| dot_arr(&self.w_rls_akt[a_prev][r], &x));
            rls_uppdatera_mat(
                &mut self.w_rls_akt[a_prev],
                &mut self.p_rls_akt[a_prev],
                &x,
                &delta,
                &pred,
                LAMBDA_RLS,
            );
        } else {
            let _ = self.vanta_beloning.take();
        }

        for j in 0..S_TREX {
            let d = s_nu[j] - self.mu[j];
            self.mu[j] += ETA_NORM * d;
            self.var[j] = (1.0 - ETA_NORM) * self.var[j] + ETA_NORM * d * d;
        }
        if !self.z_skala_init {
            self.mu_z = z_raw;
            self.var_z = [0.25; Z_TREX];
            self.z_skala_init = true;
        } else {
            for i in 0..Z_TREX {
                let d = z_raw[i] - self.mu_z[i];
                self.mu_z[i] += ETA_ZNORM * d;
                self.var_z[i] = (1.0 - ETA_ZNORM) * self.var_z[i] + ETA_ZNORM * d * d;
            }
        }

        let x = Self::x_av(&z_now);
        let mut poang = [0.0; A_N];
        for a in 0..A_N {
            let ya: [f64; Z_TREX] =
                std::array::from_fn(|r| z_now[r] + dot_arr(&self.w_rls_akt[a][r], &x));
            poang[a] = self.varde(&ya);
        }
        let a = if g.f() < UTFORSK_P {
            let mut neg = [0.0; A_N];
            for k in 0..A_N {
                neg[k] = -(self.besok[k] as f64);
            }
            lott_bland_max(&neg, g)
        } else {
            lott_bland_max(&poang, g)
        };
        self.besok[a] = GRIND_GAMMA * self.besok[a] + 1.0;
        self.forra = Some((z_now, a));
        self.steg += 1;
        a
    }
}

// ---------------------------------------------------------------------------
// Agenter & B2-simulering
// ---------------------------------------------------------------------------

/// Dubblad frusen kodare (Z=20: 12 nu-axlar + 8 minnesaxlar) för diagnosarm D2_Bred_Z20 (Alt 2370).
#[derive(Clone, Debug)]
pub struct GenB2Z20 {
    pub wk: [[f64; S_B2 + 1]; Z_B2_20],
    pub v: [f64; Z_B2_20 + 1],
}

impl GenB2Z20 {
    pub fn slump(g: &mut Rng) -> Self {
        let mut wk = [[0.0; S_B2 + 1]; Z_B2_20];
        for rad in &mut wk {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut v = [0.0; Z_B2_20 + 1];
        for x in &mut v {
            *x = SIGMA0 * gauss(g);
        }
        Self { wk, v }
    }

    pub fn arv(mor: &GenB2Z20, far: &GenB2Z20, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ 0x2370_2020_5A4B_0001);
        let mut wk = [[0.0; S_B2 + 1]; Z_B2_20];
        for i in 0..Z_B2_20 {
            for j in 0..=S_B2 {
                let bas = if g.f() < 0.5 { mor.wk[i][j] } else { far.wk[i][j] };
                wk[i][j] = bas + SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut v = [0.0; Z_B2_20 + 1];
        for i in 0..=Z_B2_20 {
            let bas = if g.f() < 0.5 { mor.v[i] } else { far.v[i] };
            v[i] = bas + SIGMA_MUT * gauss(&mut g);
        }
        Self { wk, v }
    }
}

#[derive(Clone, Debug)]
pub struct HjB2Z20 {
    pub wk: [[f64; S_B2 + 1]; Z_B2_20],
    pub w_rls_akt: [[[f64; Z_B2_20 + 1]; Z_B2_20]; A_N],
    pub p_rls_akt: [[[f64; Z_B2_20 + 1]; Z_B2_20 + 1]; A_N],
    pub v: [f64; Z_B2_20 + 1],
    pub w_krit: [f64; Z_B2_20 + 1],
    pub w_krit_mal: [f64; Z_B2_20 + 1],
    pub p_krit: [[f64; Z_B2_20 + 1]; Z_B2_20 + 1],
    pub vanta_beloning: Option<f64>,
    pub td_forra: Option<([f64; Z_B2_20], f64)>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    pub skala_init: bool,
    pub mu: [f64; S_B2],
    pub var: [f64; S_B2],
    pub z_skala_init: bool,
    pub mu_z: [f64; Z_B2_20],
    pub var_z: [f64; Z_B2_20],
    pub h_init: bool,
    pub h_spar: [f64; Z_B2_20],
    pub besok: [f32; A_N],
    pub forra_sk: [f64; S_B2],
    pub forra: Option<([f64; Z_B2_20], usize)>,
    pub steg: u64,
}

impl HjB2Z20 {
    pub fn fran_gen(gen: &GenB2Z20) -> Self {
        let inv_sqrt_s = 1.0 / (S_B2 as f64).sqrt();
        let mut wk = [[0.0; S_B2 + 1]; Z_B2_20];
        for i in 0..Z_B2_20 {
            for j in 0..=S_B2 {
                wk[i][j] = gen.wk[i][j] * inv_sqrt_s;
            }
        }
        Self {
            wk,
            w_rls_akt: [[[0.0; Z_B2_20 + 1]; Z_B2_20]; A_N],
            p_rls_akt: [diag_p::<{ Z_B2_20 + 1 }>(ALPHA_RLS); A_N],
            v: gen.v,
            w_krit: [0.0; Z_B2_20 + 1],
            w_krit_mal: [0.0; Z_B2_20 + 1],
            p_krit: diag_p::<{ Z_B2_20 + 1 }>(ALPHA_KRIT),
            vanta_beloning: None,
            td_forra: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_B2],
            var: [1.0; S_B2],
            z_skala_init: false,
            mu_z: [0.0; Z_B2_20],
            var_z: [1.0; Z_B2_20],
            h_init: false,
            h_spar: [0.0; Z_B2_20],
            besok: [0.0; A_N],
            forra_sk: [0.0; S_B2],
            forra: None,
            steg: 0,
        }
    }

    pub fn ge_beloning(&mut self, r: f64) {
        self.vanta_beloning = Some(r);
    }

    fn x_av(h: &[f64; Z_B2_20]) -> [f64; Z_B2_20 + 1] {
        let mut x = [1.0; Z_B2_20 + 1];
        x[..Z_B2_20].copy_from_slice(h);
        x
    }

    fn varde(&self, y: &[f64; Z_B2_20]) -> f64 {
        let mut s = self.v[Z_B2_20] + self.w_krit[Z_B2_20];
        for j in 0..Z_B2_20 {
            s += (self.v[j] + self.w_krit[j]) * y[j];
        }
        s
    }

    fn varde_mal(&self, y: &[f64; Z_B2_20]) -> f64 {
        let mut s = self.v[Z_B2_20] + self.w_krit_mal[Z_B2_20];
        for j in 0..Z_B2_20 {
            s += (self.v[j] + self.w_krit_mal[j]) * y[j];
        }
        s
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

    pub fn valj_lott(&mut self, s_nu: &[f64; S_B2], g: &mut Rng) -> (usize, [f64; Z_B2_20]) {
        if !self.skala_init {
            self.mu = *s_nu;
            self.var = [1.0; S_B2];
            self.skala_init = true;
        }
        let mut s_sk = [0.0; S_B2];
        for j in 0..S_B2 {
            s_sk[j] = ((s_nu[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        let mut z_raw = [0.0; Z_B2_20];
        for i in 0..Z_B2_20 {
            let mut lin = self.wk[i][S_B2];
            for j in 0..S_B2 {
                lin += self.wk[i][j] * s_sk[j];
            }
            z_raw[i] = lin.tanh();
        }
        let z_now = if !self.z_skala_init {
            z_raw
        } else {
            std::array::from_fn(|i| {
                ((z_raw[i] - self.mu_z[i]) / (self.var_z[i] + 1e-4).sqrt()).clamp(-3.0, 3.0)
            })
        };
        let h_now = if !self.h_init {
            z_now
        } else {
            std::array::from_fn(|r| {
                if r < Z_NU_20 {
                    z_now[r]
                } else {
                    minnes_spar_scalar(BETA_MINNE_AKTIV, self.h_spar[r], z_now[r])
                }
            })
        };

        if let Some((h_prev, a_prev)) = self.forra {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let r_nu = r_raw.clamp(-50.0, 50.0);
                if let Some((h_td, r_td)) = self.td_forra {
                    let v_nasta = self.varde_mal(&h_now).clamp(-50.0, 50.0);
                    let y_td = (r_td + GAMMA_TD * v_nasta).clamp(-50.0, 50.0);
                    let pred_k = self.varde(&h_td);
                    let err_k = y_td - pred_k;
                    if self.steg >= 80 {
                        self.krit_n += 1;
                        self.krit_sy += y_td;
                        self.krit_sy2 += y_td * y_td;
                        self.krit_se2 += err_k * err_k;
                    }
                    let x_k = Self::x_av(&h_td);
                    rls_uppdatera_skalar(&mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT);
                    for m in 0..=Z_B2_20 {
                        self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
                    }
                }
                self.td_forra = Some((h_now, r_nu));
            }
            let x = Self::x_av(&h_prev);
            let delta: [f64; Z_B2_20] = std::array::from_fn(|r| h_now[r] - h_prev[r]);
            let pred: [f64; Z_B2_20] =
                std::array::from_fn(|r| dot_arr(&self.w_rls_akt[a_prev][r], &x));
            rls_uppdatera_mat(
                &mut self.w_rls_akt[a_prev],
                &mut self.p_rls_akt[a_prev],
                &x,
                &delta,
                &pred,
                LAMBDA_RLS,
            );
        } else {
            let _ = self.vanta_beloning.take();
        }

        for j in 0..S_B2 {
            let d = s_nu[j] - self.mu[j];
            self.mu[j] += ETA_NORM * d;
            self.var[j] = (1.0 - ETA_NORM) * self.var[j] + ETA_NORM * d * d;
        }
        if !self.z_skala_init {
            self.mu_z = z_raw;
            self.var_z = [0.25; Z_B2_20];
            self.z_skala_init = true;
        } else {
            for i in 0..Z_B2_20 {
                let d = z_raw[i] - self.mu_z[i];
                self.mu_z[i] += ETA_ZNORM * d;
                self.var_z[i] = (1.0 - ETA_ZNORM) * self.var_z[i] + ETA_ZNORM * d * d;
            }
        }
        self.h_spar = h_now;
        self.h_init = true;
        self.forra_sk = s_sk;

        let x = Self::x_av(&h_now);
        let mut poang = [0.0; A_N];
        for a in 0..A_N {
            let ya: [f64; Z_B2_20] =
                std::array::from_fn(|r| h_now[r] + dot_arr(&self.w_rls_akt[a][r], &x));
            poang[a] = self.varde(&ya);
        }
        let a = if g.f() < UTFORSK_P {
            let mut neg = [0.0; A_N];
            for k in 0..A_N {
                neg[k] = -(self.besok[k] as f64);
            }
            lott_bland_max(&neg, g)
        } else {
            lott_bland_max(&poang, g)
        };
        self.besok[a] = GRIND_GAMMA * self.besok[a] + 1.0;
        self.forra = Some((h_now, a));
        self.steg += 1;
        (a, h_now)
    }
}

// ---------------------------------------------------------------------------
// Alt 2379 (Arm B & Arm D): Gles icke-negativ LpWM-kodare (Z_GLES = 24 = 16 nu + 8 minne)
// med positiv RMS-skalning (aldrig medelvärdesavdraget norm_z!), top-6 gleshet,
// Lean 4-verifierat symmetriskt kongruens-P-tak och valfri livstids-chockgrind/replay (Arm D).
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GenB2Gles {
    pub wk: [[f64; S_B2 + 1]; Z_GLES_NU],
    pub v: [f64; Z_GLES + 1],
}

impl GenB2Gles {
    pub fn slump(g: &mut Rng) -> Self {
        let mut wk = [[0.0; S_B2 + 1]; Z_GLES_NU];
        for rad in &mut wk {
            for x in rad.iter_mut() {
                *x = SIGMA0 * gauss(g);
            }
        }
        let mut v = [0.0; Z_GLES + 1];
        for x in &mut v {
            *x = SIGMA0 * gauss(g);
        }
        Self { wk, v }
    }

    pub fn arv(mor: &GenB2Gles, far: &GenB2Gles, uid: u64) -> Self {
        let mut g = Rng::new(uid ^ 0x2379_61E5_0001);
        let mut wk = [[0.0; S_B2 + 1]; Z_GLES_NU];
        for i in 0..Z_GLES_NU {
            for j in 0..=S_B2 {
                let bas = if g.f() < 0.5 { mor.wk[i][j] } else { far.wk[i][j] };
                wk[i][j] = bas + SIGMA_MUT * gauss(&mut g);
            }
        }
        let mut v = [0.0; Z_GLES + 1];
        for i in 0..=Z_GLES {
            let bas = if g.f() < 0.5 { mor.v[i] } else { far.v[i] };
            v[i] = bas + SIGMA_MUT * gauss(&mut g);
        }
        Self { wk, v }
    }
}

#[derive(Clone, Debug)]
pub struct HjB2Gles {
    pub chock_aktiv: bool,
    pub wk: [[f64; S_B2 + 1]; Z_GLES_NU],
    pub w_rls_akt: [[[f64; Z_GLES + 1]; Z_GLES]; A_N],
    pub p_rls_akt: [[[f64; Z_GLES + 1]; Z_GLES + 1]; A_N],
    pub v: [f64; Z_GLES + 1],
    pub w_krit: [f64; Z_GLES + 1],
    pub w_krit_mal: [f64; Z_GLES + 1],
    pub p_krit: [[f64; Z_GLES + 1]; Z_GLES + 1],
    pub vanta_beloning: Option<f64>,
    pub vanta_chock: bool,
    pub td_forra: Option<([f64; Z_GLES], f64)>,
    pub krit_n: u64,
    pub krit_sy: f64,
    pub krit_sy2: f64,
    pub krit_se2: f64,
    pub skala_init: bool,
    pub mu: [f64; S_B2],
    pub var: [f64; S_B2],
    /// Positiv RMS-skala per gles axel (aldrig medelvärdesavdrag -> exakta nollor bevaras!).
    pub ms_z: [f64; Z_GLES_NU],
    pub h_init: bool,
    pub h_spar: [f64; Z_GLES],
    pub grind_n: [f32; A_N],
    pub grind_sy: [[f32; S_B2]; A_N],
    pub grind_sy2: [[f32; S_B2]; A_N],
    pub grind_sx: [[f32; S_B2]; A_N],
    pub grind_sx2: [[f32; S_B2]; A_N],
    pub grind_syx: Vec<[[f32; S_B2]; S_B2]>,
    pub grind_adj: [[f32; S_B2]; A_N],
    pub bel_n: f32,
    pub bel_sr: f32,
    pub bel_sr2: f32,
    pub bel_sa: f32,
    pub bel_sa2: f32,
    pub bel_sx: [f32; S_B2],
    pub bel_sx2: [f32; S_B2],
    pub bel_srx: [f32; S_B2],
    pub bel_sax: [f32; S_B2],
    pub chock_golv: [f64; S_B2],
    pub kanal_vikt: [f64; S_B2],
    pub forra_ra_sk: Option<[f32; S_B2]>,
    pub forra_s_ra: Option<[f32; S_B2]>,
    pub forra_sk: [f64; S_B2],
    pub forra: Option<([f64; Z_GLES], usize)>,
    pub chock_minne: Vec<([f32; S_B2], f32, [f32; S_B2])>,
    pub replay_pekare: usize,
    pub p_tak_traffar: u64,
    pub steg: u64,
}

impl HjB2Gles {
    pub fn fran_gen(gen: &GenB2Gles, chock_aktiv: bool) -> Self {
        let inv_sqrt_s = 1.0 / (S_B2 as f64).sqrt();
        let mut wk = [[0.0; S_B2 + 1]; Z_GLES_NU];
        for i in 0..Z_GLES_NU {
            for j in 0..=S_B2 {
                wk[i][j] = gen.wk[i][j] * inv_sqrt_s;
            }
        }
        Self {
            chock_aktiv,
            wk,
            w_rls_akt: [[[0.0; Z_GLES + 1]; Z_GLES]; A_N],
            p_rls_akt: [diag_p::<{ Z_GLES + 1 }>(ALPHA_RLS); A_N],
            v: gen.v,
            w_krit: [0.0; Z_GLES + 1],
            w_krit_mal: [0.0; Z_GLES + 1],
            p_krit: diag_p::<{ Z_GLES + 1 }>(ALPHA_KRIT),
            vanta_beloning: None,
            vanta_chock: false,
            td_forra: None,
            krit_n: 0,
            krit_sy: 0.0,
            krit_sy2: 0.0,
            krit_se2: 0.0,
            skala_init: false,
            mu: [0.0; S_B2],
            var: [1.0; S_B2],
            ms_z: [1.0; Z_GLES_NU],
            h_init: false,
            h_spar: [0.0; Z_GLES],
            grind_n: [0.0; A_N],
            grind_sy: [[0.0; S_B2]; A_N],
            grind_sy2: [[0.0; S_B2]; A_N],
            grind_sx: [[0.0; S_B2]; A_N],
            grind_sx2: [[0.0; S_B2]; A_N],
            grind_syx: vec![[[0.0; S_B2]; S_B2]; A_N],
            grind_adj: [[0.0; S_B2]; A_N],
            bel_n: 0.0,
            bel_sr: 0.0,
            bel_sr2: 0.0,
            bel_sa: 0.0,
            bel_sa2: 0.0,
            bel_sx: [0.0; S_B2],
            bel_sx2: [0.0; S_B2],
            bel_srx: [0.0; S_B2],
            bel_sax: [0.0; S_B2],
            chock_golv: [0.0; S_B2],
            kanal_vikt: [1.0; S_B2],
            forra_ra_sk: None,
            forra_s_ra: None,
            forra_sk: [0.0; S_B2],
            forra: None,
            chock_minne: Vec::new(),
            replay_pekare: 0,
            p_tak_traffar: 0,
            steg: 0,
        }
    }

    pub fn ge_beloning(&mut self, r: f64) {
        self.vanta_beloning = Some(r);
        self.vanta_chock = false;
        if let Some(s_prev) = self.forra_ra_sk {
            let rf = r.clamp(-50.0, 50.0) as f32;
            let mr_prev = if self.bel_n > 1.0 { self.bel_sr / self.bel_n } else { 0.0 };
            let vr_prev = if self.bel_n > 2.0 {
                (self.bel_sr2 / self.bel_n - mr_prev * mr_prev).max(1e-4)
            } else {
                1.0
            };
            let af = (rf - mr_prev).abs();
            if self.chock_aktiv && self.bel_n >= 40.0 && af > 3.0 * vr_prev.sqrt() {
                self.vanta_chock = true;
                for j in 0..S_B2 {
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
            for j in 0..S_B2 {
                let x = s_prev[j];
                self.bel_sx[j] = GRIND_GAMMA * self.bel_sx[j] + x;
                self.bel_sx2[j] = GRIND_GAMMA * self.bel_sx2[j] + x * x;
                self.bel_srx[j] = GRIND_GAMMA * self.bel_srx[j] + rf * x;
                self.bel_sax[j] = GRIND_GAMMA * self.bel_sax[j] + af * x;
            }
        }
    }

    pub fn g_intervall(&self, start: usize, len: usize) -> f64 {
        self.kanal_vikt[start..start + len].iter().sum::<f64>() / len as f64
    }

    pub fn krit_r2(&self) -> f64 {
        if self.krit_n < 20 { return f64::NAN; }
        let n = self.krit_n as f64;
        let my = self.krit_sy / n;
        let ss_tot = self.krit_sy2 - n * my * my;
        if ss_tot <= 1e-9 { return f64::NAN; }
        1.0 - self.krit_se2 / ss_tot
    }

    fn x_av(h: &[f64; Z_GLES]) -> [f64; Z_GLES + 1] {
        let mut x = [1.0; Z_GLES + 1];
        x[..Z_GLES].copy_from_slice(h);
        x
    }

    fn varde(&self, y: &[f64; Z_GLES]) -> f64 {
        let mut s = self.v[Z_GLES] + self.w_krit[Z_GLES];
        for j in 0..Z_GLES {
            s += (self.v[j] + self.w_krit[j]) * y[j];
        }
        s
    }

    fn varde_mal(&self, y: &[f64; Z_GLES]) -> f64 {
        let mut s = self.v[Z_GLES] + self.w_krit_mal[Z_GLES];
        for j in 0..Z_GLES {
            s += (self.v[j] + self.w_krit_mal[j]) * y[j];
        }
        s
    }

    fn skala(&self, s: &[f64; S_B2]) -> [f64; S_B2] {
        let mut ut = [0.0; S_B2];
        for j in 0..S_B2 {
            ut[j] = ((s[j] - self.mu[j]) / (self.var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
        }
        ut
    }

    /// Gles icke-negativ kodning: ReLU -> top-k (k = 6) -> positiv RMS-skalning (bevarar exakta nollor!).
    pub fn koda_gles_nu(&self, s_in: &[f64; S_B2]) -> ([f64; Z_GLES_NU], [f64; Z_GLES_NU]) {
        let mut raw = [0.0; Z_GLES_NU];
        for i in 0..Z_GLES_NU {
            let mut lin = self.wk[i][S_B2];
            for j in 0..S_B2 {
                lin += self.wk[i][j] * s_in[j];
            }
            raw[i] = lin.max(0.0);
        }
        // Top-k (k = GLES_TOP_K) på nu-axlarna
        let mut ord: [usize; Z_GLES_NU] = std::array::from_fn(|i| i);
        ord.sort_by(|&a, &b| raw[b].partial_cmp(&raw[a]).unwrap_or(std::cmp::Ordering::Equal));
        let mut gles = [0.0; Z_GLES_NU];
        for &idx in ord.iter().take(GLES_TOP_K) {
            gles[idx] = raw[idx];
        }
        let mut skalad = [0.0; Z_GLES_NU];
        for i in 0..Z_GLES_NU {
            let sc = (1.0 / (self.ms_z[i] + 1e-4).sqrt()).min(3.0);
            skalad[i] = relu_pos_skala(gles[i], sc).min(3.0);
        }
        (gles, skalad)
    }

    fn omkoda_h_ra(&self, s_ra: &[f32; S_B2]) -> [f64; Z_GLES] {
        let s_f64: [f64; S_B2] = std::array::from_fn(|j| s_ra[j] as f64);
        let s_sk = self.skala(&s_f64);
        let mut s_in = [0.0; S_B2];
        for j in 0..S_B2 {
            s_in[j] = s_sk[j] * self.kanal_vikt[j];
        }
        let (_, z_nu) = self.koda_gles_nu(&s_in);
        let mut h = [0.0; Z_GLES];
        for r in 0..Z_GLES_NU {
            h[r] = z_nu[r];
        }
        for r in 0..(Z_GLES - Z_GLES_NU) {
            h[Z_GLES_NU + r] = z_nu[r];
        }
        h
    }

    fn kor_chock_replay_post(&mut self, post: ([f32; S_B2], f32, [f32; S_B2]), c_vikt: f64) {
        let r_ch = (post.1 as f64).clamp(-50.0, 50.0);
        let h_prev = self.omkoda_h_ra(&post.0);
        let h_now = self.omkoda_h_ra(&post.2);
        let x_now = Self::x_av(&h_now);
        let err_now = (r_ch - self.varde(&h_now)).clamp(-50.0, 50.0);
        self.p_tak_traffar += rls_uppdatera_skalar_viktad(
            &mut self.w_krit, &mut self.p_krit, &x_now, err_now, 1.0, c_vikt, Some(P_TAK_GLES),
        );
        let x_prev = Self::x_av(&h_prev);
        let y_prev = (GAMMA_TD * r_ch).clamp(-50.0, 50.0);
        let err_prev = (y_prev - self.varde(&h_prev)).clamp(-50.0, 50.0);
        self.p_tak_traffar += rls_uppdatera_skalar_viktad(
            &mut self.w_krit, &mut self.p_krit, &x_prev, err_prev, 1.0, c_vikt * 0.5, Some(P_TAK_GLES),
        );
        for m in 0..=Z_GLES {
            self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
        }
    }

    fn uppdatera_grind(&mut self, s_sk_nu: &[f32; S_B2]) {
        if let (Some((_, a)), Some(s_prev)) = (self.forra, self.forra_ra_sk) {
            self.grind_n[a] = GRIND_GAMMA * self.grind_n[a] + 1.0;
            for j in 0..S_B2 {
                let y = s_sk_nu[j];
                let x = s_prev[j];
                self.grind_sy[a][j] = GRIND_GAMMA * self.grind_sy[a][j] + y;
                self.grind_sy2[a][j] = GRIND_GAMMA * self.grind_sy2[a][j] + y * y;
                self.grind_sx[a][j] = GRIND_GAMMA * self.grind_sx[a][j] + x;
                self.grind_sx2[a][j] = GRIND_GAMMA * self.grind_sx2[a][j] + x * x;
                for k in 0..S_B2 {
                    self.grind_syx[a][j][k] = GRIND_GAMMA * self.grind_syx[a][j][k] + y * s_prev[k];
                }
            }
            let n = self.grind_n[a];
            if n >= GRIND_MIN_N {
                let inv_n = 1.0 / n;
                let straff = GRIND_STRAFF * inv_n;
                let mut mx = [0.0f32; S_B2];
                let mut vx = [0.0f32; S_B2];
                for k in 0..S_B2 {
                    mx[k] = self.grind_sx[a][k] * inv_n;
                    vx[k] = self.grind_sx2[a][k] * inv_n - mx[k] * mx[k];
                }
                for j in 0..S_B2 {
                    let my = self.grind_sy[a][j] * inv_n;
                    let vy = self.grind_sy2[a][j] * inv_n - my * my;
                    if vy < 1e-6 {
                        self.grind_adj[a][j] = 0.0;
                        continue;
                    }
                    let inv_vy = 1.0 / vy;
                    let mut max_r2 = 0.0f32;
                    for k in 0..S_B2 {
                        if vx[k] < 1e-6 { continue; }
                        let cov = self.grind_syx[a][j][k] * inv_n - my * mx[k];
                        let r2 = cov * cov * inv_vy / vx[k];
                        if r2 > max_r2 { max_r2 = r2; }
                    }
                    self.grind_adj[a][j] = (max_r2 - straff).max(0.0);
                }
            }
        }
        let mut poang = [0.0f64; S_B2];
        for j in 0..S_B2 {
            let mut m = 0.0f32;
            for a in 0..A_N {
                if self.grind_n[a] >= GRIND_MIN_N && self.grind_adj[a][j] > m {
                    m = self.grind_adj[a][j];
                }
            }
            poang[j] = m as f64;
        }
        let max_p = poang.iter().copied().fold(0.0f64, f64::max);
        if max_p > GRIND_AKTIV_MIN {
            for j in 0..S_B2 {
                self.kanal_vikt[j] = (poang[j] / GRIND_MATTNAD).clamp(0.0, 1.0).powi(3);
            }
        } else {
            self.kanal_vikt = [1.0; S_B2];
        }
        if self.bel_n >= 40.0 {
            let inv_n = 1.0 / self.bel_n;
            let mr = self.bel_sr * inv_n;
            let vr = self.bel_sr2 * inv_n - mr * mr;
            let ma = self.bel_sa * inv_n;
            let va = self.bel_sa2 * inv_n - ma * ma;
            let straff = 1.5 * inv_n;
            for j in 0..S_B2 {
                let mx = self.bel_sx[j] * inv_n;
                let vx = self.bel_sx2[j] * inv_n - mx * mx;
                let mut best_r2 = 0.0f64;
                if vx >= 1e-6 {
                    if vr >= 1e-6 {
                        let cov = self.bel_srx[j] * inv_n - mr * mx;
                        let r2 = (cov * cov / (vr * vx) - straff).max(0.0) as f64;
                        if r2 > best_r2 { best_r2 = r2; }
                    }
                    if va >= 1e-6 {
                        let cov_a = self.bel_sax[j] * inv_n - ma * mx;
                        let r2_a = (cov_a * cov_a / (va * vx) - straff).max(0.0) as f64;
                        if r2_a > best_r2 { best_r2 = r2_a; }
                    }
                }
                let eff = if self.chock_aktiv { best_r2.max(self.chock_golv[j]) } else { best_r2 };
                let faktor = 0.25 + 0.75 * (eff / 0.008).clamp(0.0, 1.0);
                self.kanal_vikt[j] *= faktor;
            }
        }
    }

    pub fn valj_lott(&mut self, s_nu: &[f64; S_B2], g: &mut Rng) -> (usize, [f64; Z_GLES]) {
        if !self.skala_init {
            self.mu = *s_nu;
            self.var = [1.0; S_B2];
            self.skala_init = true;
        }
        let s_sk = self.skala(s_nu);
        let s_sk_f32: [f32; S_B2] = std::array::from_fn(|j| s_sk[j] as f32);
        let mut s_in = [0.0; S_B2];
        for j in 0..S_B2 {
            s_in[j] = s_sk[j] * self.kanal_vikt[j];
        }
        let (z_gles, z_nu) = self.koda_gles_nu(&s_in);
        let h_now: [f64; Z_GLES] = std::array::from_fn(|r| {
            if r < Z_GLES_NU {
                z_nu[r]
            } else if !self.h_init {
                z_nu[r - Z_GLES_NU]
            } else {
                // Gles-kompatibelt minnesspår (β = 0.20, icke-negativt)
                minnes_spar_scalar(0.20, self.h_spar[r], z_nu[r - Z_GLES_NU])
            }
        });

        if let Some((h_prev, a_prev)) = self.forra {
            if let Some(r_raw) = self.vanta_beloning.take() {
                let r_nu = r_raw.clamp(-50.0, 50.0);
                if let Some((h_td, r_td)) = self.td_forra {
                    let v_nasta = self.varde_mal(&h_now).clamp(-50.0, 50.0);
                    let y_td = (r_td + GAMMA_TD * v_nasta).clamp(-50.0, 50.0);
                    let err_k = y_td - self.varde(&h_td);
                    if self.steg >= 80 {
                        self.krit_n += 1;
                        self.krit_sy += y_td;
                        self.krit_sy2 += y_td * y_td;
                        self.krit_se2 += err_k * err_k;
                    }
                    let x_k = Self::x_av(&h_td);
                    self.p_tak_traffar += rls_uppdatera_skalar_viktad(
                        &mut self.w_krit, &mut self.p_krit, &x_k, err_k, LAMBDA_KRIT, 1.0, Some(P_TAK_GLES),
                    );
                    for m in 0..=Z_GLES {
                        self.w_krit_mal[m] += ETA_KRIT_MAL * (self.w_krit[m] - self.w_krit_mal[m]);
                    }
                }
                if self.chock_aktiv && self.vanta_chock {
                    if let Some(s_prev_f32) = self.forra_s_ra {
                        let s_now_f32: [f32; S_B2] = std::array::from_fn(|j| s_nu[j] as f32);
                        let post = (s_prev_f32, r_nu as f32, s_now_f32);
                        if self.chock_minne.len() < CHOCK_BUFFERT_MAX {
                            self.chock_minne.push(post);
                        } else {
                            let idx = self.replay_pekare % CHOCK_BUFFERT_MAX;
                            self.chock_minne[idx] = post;
                        }
                        for _ in 0..3 {
                            self.kor_chock_replay_post(post, 2.0);
                        }
                    }
                }
                self.vanta_chock = false;
                if self.chock_aktiv && self.steg % 10 == 0 && !self.chock_minne.is_empty() {
                    let idx = self.replay_pekare % self.chock_minne.len();
                    let post = self.chock_minne[idx];
                    self.replay_pekare = self.replay_pekare.wrapping_add(1);
                    self.kor_chock_replay_post(post, 1.0);
                }
                self.td_forra = Some((h_now, r_nu));
            }
            let x = Self::x_av(&h_prev);
            let delta: [f64; Z_GLES] = std::array::from_fn(|r| h_now[r] - h_prev[r]);
            let pred: [f64; Z_GLES] = std::array::from_fn(|r| dot_arr(&self.w_rls_akt[a_prev][r], &x));
            self.p_tak_traffar += rls_uppdatera_mat_symtak(
                &mut self.w_rls_akt[a_prev],
                &mut self.p_rls_akt[a_prev],
                &x,
                &delta,
                &pred,
                LAMBDA_RLS,
                P_TAK_GLES,
            );
        } else {
            let _ = self.vanta_beloning.take();
        }

        self.uppdatera_grind(&s_sk_f32);
        for j in 0..S_B2 {
            let d = s_nu[j] - self.mu[j];
            self.mu[j] += ETA_NORM * d;
            self.var[j] = (1.0 - ETA_NORM) * self.var[j] + ETA_NORM * d * d;
        }
        for i in 0..Z_GLES_NU {
            // Uppdatera endast positiv RMS (aldrig medelvärde!)
            self.ms_z[i] = (1.0 - ETA_ZNORM) * self.ms_z[i] + ETA_ZNORM * z_gles[i] * z_gles[i];
        }
        self.h_spar = h_now;
        self.h_init = true;

        let mut poang = [0.0; A_N];
        let x = Self::x_av(&h_now);
        for a in 0..A_N {
            let y: [f64; Z_GLES] = std::array::from_fn(|r| h_now[r] + dot_arr(&self.w_rls_akt[a][r], &x));
            poang[a] = self.varde(&y);
        }
        let a = if g.f() < UTFORSK_P {
            let mut neg = [0.0; A_N];
            for k in 0..A_N { neg[k] = -(self.grind_n[k] as f64); }
            lott_bland_max(&neg, g)
        } else {
            lott_bland_max(&poang, g)
        };
        self.forra_ra_sk = Some(s_sk_f32);
        self.forra_s_ra = Some(std::array::from_fn(|j| s_nu[j] as f32));
        self.forra_sk = s_sk;
        self.forra = Some((h_now, a));
        self.steg += 1;
        (a, h_now)
    }
}

#[derive(Clone, Debug)]
pub struct ByteB2 {
    pub lev: bool,
    pub i: usize,
    pub j: usize,
    pub e: f64,
    pub age: i64,
    pub hona: bool,
    pub markt: bool,
    pub uid: u64,
    pub gen: GenB2,
    pub gen_z20: Option<Box<GenB2Z20>>,
    pub gen_gles: Option<Box<GenB2Gles>>,
    pub hj: Option<HjB2>,
    pub hj_z20: Option<Box<HjB2Z20>>,
    pub hj_gles: Option<Box<HjB2Gles>>,
    pub tabell: Option<TabellB2>,
    pub egen_rng: Rng,
    pub e_fore_steg: f64,
    pub senaste_skada: f64,
    /// Steg sedan djuret senast kände rovdjurslukt > 0.10 (för Alt 1885b-proben).
    pub steg_sedan_lukt: u16,
    /// Föregående lukt vid djuret (för Alt 1881 undvikningsmätning).
    pub forra_lukt_rov: f64,
    pub forra_lukt_harmlos: f64,
    pub hade_lukt_rov_granne: bool,
    pub hade_lukt_harmlos_granne: bool,
    /// Diagnostik (Alt 2379 förprov, påverkar ej beteende): steg sedan senaste överlevda bett samt antal överlevda bett.
    pub steg_sedan_skada: u32,
    pub antal_overlevda_bett: u32,
}

#[derive(Clone, Debug)]
pub struct TrexB2 {
    pub lev: bool,
    pub i: usize,
    pub j: usize,
    pub e: f64,
    pub age: i64,
    pub uid: u64,
    pub gen: GenTrex,
    pub hj: HjTrex,
    pub egen_rng: Rng,
    pub e_fore_steg: f64,
    pub vila: u16,
}

#[derive(Clone, Debug)]
pub struct HarmlosKalla {
    pub i: usize,
    pub j: usize,
    pub dir: usize,
}

/// Insamlad punkt för den linjära Alt 1885b-proben (per individ, dold nära T. rex efter att lukten upphört).
#[derive(Clone, Debug)]
pub struct ProbPunkt {
    pub uid: u64,
    pub markt: bool,
    pub z: [f64; Z_B2],
    pub s_ra: [f64; S_B2],
    pub s_sk: [f64; S_B2],
    pub label: bool,
    /// True om punkten har nylig lukthistoria (1..=5 steg sedan lukt) — används för matchad K6-sond (Alt 2370).
    pub matchad_lukt: bool,
}

#[derive(Clone, Debug)]
pub struct SimB2 {
    pub b: usize,
    pub h: usize,
    pub param: B2Param,
    pub vatten: Vec<Vatten>,
    pub landandel: Vec<f64>,
    pub vmax: Vec<f64>,
    pub v: Vec<f64>,
    pub p_steg: Vec<f64>,
    pub kron: Vec<f64>,
    pub land_rutor: Vec<(usize, usize)>,
    pub klimat: Klimat,
    pub steg: i64,
    pub vind_dir: usize,
    pub lukt_rov: Vec<f64>,
    pub lukt_harmlos: Vec<f64>,
    pub lukt_byte: Vec<f64>,
    pub byten: Vec<ByteB2>,
    pub trex: Vec<TrexB2>,
    pub harmlos: Vec<HarmlosKalla>,
    pub g: Rng,
    pub nasta_uid: u64,
    pub typ_markt: ByteTyp,
    pub typ_omarkt: ByteTyp,
    pub samla_prob: bool,
    pub prob_data: Vec<ProbPunkt>,
    // Statistik
    pub dod_tot_m: u64,
    pub dod_tot_o: u64,
    pub dod_rov_m: u64,
    pub dod_rov_o: u64,
    pub traff_m: u64,
    pub traff_o: u64,
    pub trex_invandring: u64,
    pub trex_fodslar: u64,
    pub trex_dodar: u64,
    pub trex_byten_dodade: u64,
    pub undvik_rov_sum_m: f64,
    pub undvik_rov_n_m: u64,
    pub undvik_h_sum_m: f64,
    pub undvik_h_n_m: u64,
    pub undvik_rov_sum_o: f64,
    pub undvik_rov_n_o: u64,
    pub undvik_h_sum_o: f64,
    pub undvik_h_n_o: u64,
    pub intag_sum_m: f64,
    pub intag_n_m: u64,
    pub intag_sum_o: f64,
    pub intag_n_o: u64,
    /// Diagnostik (Alt 2379 förprov): levande djursteg efter första överlevda bettet (fokusgrupp).
    pub steg_efter_bett_m: u64,
    pub steg_tot_m: u64,
}

impl SimB2 {
    #[inline]
    fn idx(&self, i: usize, j: usize) -> usize {
        j * self.b + i
    }

    #[inline]
    fn granne(&self, i: usize, j: usize, dir_idx: usize) -> (usize, usize) {
        let (dx, dy) = DIR[dir_idx];
        let ni = (i as i32 + dx).rem_euclid(self.b as i32) as usize;
        let nj = (j as i32 + dy).rem_euclid(self.h as i32) as usize;
        (ni, nj)
    }

    fn ar_land(&self, c: usize) -> bool {
        self.landandel[c] > 0.0 && !ar_hav(self.vatten[c])
    }

    /// Bygger en riktig 64×64 (250 m) Hell Creek-arena från `hckarta::karta_full_version`.
    pub fn fran_hell_creek(fro: u64, param: B2Param) -> Result<Self, String> {
        let rot_buf = if let Ok(p) = std::env::var("HCSIM_ROT") {
            PathBuf::from(p)
        } else if Path::new("../paleokarta/data").exists() {
            PathBuf::from(".")
        } else {
            PathBuf::from("/home/hakan/sandbox/pi-vmodell-b2/n10/hcsim")
        };
        let rot = rot_buf.as_path();
        let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
        let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv"))
            .map_err(|e| format!("tidtabell: {e}"))?;
        let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
        let omr = Omrade {
            lat0: LAT0,
            lon0: LON0,
            b: B2_B,
            h: B2_H,
            ruta_m: B2_RUTA_M,
        };
        let (kart, andelar_opt) = karta_full_version(&kalla, &tt, &omr, T_KARTA, fro, 2, 1)?;
        let andelar = andelar_opt.ok_or("flodversion 2 saknade FlodAndelar")?;
        Self::fran_karta(&kart, &andelar, fro, param)
    }

    pub fn fran_karta(
        kart: &Karta,
        andelar: &hckarta::FlodAndelar,
        fro: u64,
        param: B2Param,
    ) -> Result<Self, String> {
        let b = kart.omrade.b;
        let h = kart.omrade.h;
        let ruta_m = kart.omrade.ruta_m;
        let n = b * h;
        let tathet = hckarta::rakna_vaxt_tathet(
            &kart.vattentyp.varden,
            &kart.period.varden,
            &andelar.balte_andel.varden,
            &andelar.vall_andel.varden,
        );
        let _bas = hckarta::rakna_basunderlag(kart, Some(andelar))?;

        const B_KG_M2: [f64; hckarta::VAXT_N] = [0.200, 0.020, 0.474, 0.345, 0.200, 0.300];
        const P_KG_M2_AR: [f64; hckarta::VAXT_N] = [0.574, 0.062, 0.664, 0.345, 0.574, 0.600];
        const B_REF: f64 = 0.50 * 0.200 + 0.10 * 0.474 + 0.02 * 0.345 + 0.01 * 0.200;
        const P_REF: f64 = 0.50 * 0.574 + 0.10 * 0.664 + 0.02 * 0.345 + 0.01 * 0.574;

        let tak_ref = vmax_ruta(ruta_m, 1.0, 0.50) * param.vaxt_skala;
        let p_steg_ref = 0.0012 * tak_ref;

        let mut vatten = Vec::with_capacity(n);
        let mut landandel = vec![0.0; n];
        let mut vmax = vec![0.0; n];
        let mut v = vec![0.0; n];
        let mut p_steg = vec![0.0; n];
        let mut kron = vec![0.0; n];
        let mut land_rutor = Vec::new();

        let mut g_k = Rng::new(fro ^ 0x2361_C40A_1001);
        let phi1 = g_k.f() * std::f64::consts::TAU;
        let phi2 = g_k.f() * std::f64::consts::TAU;
        let phi3 = g_k.f() * std::f64::consts::TAU;
        let phi4 = g_k.f() * std::f64::consts::TAU;

        for j in 0..h {
            let fy = j as f64 / h.max(1) as f64 * std::f64::consts::TAU;
            for i in 0..b {
                let fx = i as f64 / b.max(1) as f64 * std::f64::consts::TAU;
                let c = j * b + i;
                let vt = kart.vattentyp.varden[c];
                vatten.push(vt);
                let la = landandel_av(vt, ruta_m);
                landandel[c] = la;
                if la > 0.0 && !ar_hav(vt) {
                    let mut b_sum = 0.0;
                    let mut p_sum = 0.0;
                    for gr in 0..hckarta::VAXT_N {
                        let t_g = tathet[c][gr] as f64;
                        b_sum += t_g * B_KG_M2[gr];
                        p_sum += t_g * P_KG_M2_AR[gr];
                    }
                    let tak = (tak_ref * la * (b_sum / B_REF)).max(0.5);
                    let ps = (p_steg_ref * la * (p_sum / P_REF)).max(0.0005);
                    vmax[c] = tak;
                    v[c] = tak * V0_ANDEL;
                    p_steg[c] = ps;
                    if matches!(vt, Vatten::Land | Vatten::Trask) {
                        land_rutor.push((i, j));
                        let mosaik = 0.24 * (5.0 * fx + phi1).sin() * (5.0 * fy + phi2).cos()
                            + 0.18 * (7.0 * fx - 3.0 * fy + phi3).cos() * (3.0 * fx + 7.0 * fy + phi4).sin();
                        let ort = tathet[c][hckarta::VAXT_ORT] as f64;
                        let bas_k = match vt {
                            Vatten::Trask => 0.38,
                            Vatten::Land if ort >= 0.25 => 0.20,
                            _ => 0.56,
                        };
                        let amp = if matches!(vt, Vatten::Land) && ort >= 0.25 { 0.30 } else { 1.0 };
                        kron[c] = (bas_k + amp * mosaik).clamp(0.05, 0.95);
                    }
                }
            }
        }

        if land_rutor.is_empty() {
            return Err("B2-kartan saknade landrutor".to_string());
        }

        let mut g = Rng::new(fro ^ 0x2361_B250_0001);
        let mut nasta_uid = 1u64;
        let n0 = 900usize;
        let mut byten = Vec::with_capacity(n0 * 2);
        for k in 0..n0 {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            let gen = GenB2::slump(&mut g);
            byten.push(ByteB2 {
                lev: true,
                i,
                j,
                e: E_START,
                age: g.under(LIV as usize) as i64,
                hona: k % 2 == 0,
                markt: false,
                uid: nasta_uid,
                hj: Some(HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::Bas, param.belonings_grind))),
                gen_z20: None,
                gen_gles: None,
                hj_z20: None,
                hj_gles: None,
                tabell: None,
                gen,
                egen_rng: Rng::new(nasta_uid),
                e_fore_steg: E_START,
                senaste_skada: 0.0,
                steg_sedan_lukt: 999,
                forra_lukt_rov: 0.0,
                forra_lukt_harmlos: 0.0,
                hade_lukt_rov_granne: false,
                hade_lukt_harmlos_granne: false,
                steg_sedan_skada: u32::MAX,
                antal_overlevda_bett: 0,
            });
            nasta_uid += 1;
        }

        let mut trex = Vec::with_capacity(param.max_trex);
        for _ in 0..param.start_trex {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            let gen = GenTrex::slump(&mut g);
            let hj = HjTrex::fran_gen(&gen);
            trex.push(TrexB2 {
                lev: true,
                i,
                j,
                e: param.trex_e_start,
                age: g.under(LIV as usize) as i64,
                uid: nasta_uid,
                gen,
                hj,
                egen_rng: Rng::new(nasta_uid ^ 0x74E8),
                e_fore_steg: param.trex_e_start,
                vila: 0,
            });
            nasta_uid += 1;
        }

        let mut harmlos = Vec::with_capacity(param.start_trex);
        for _ in 0..param.start_trex {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            harmlos.push(HarmlosKalla {
                i,
                j,
                dir: 1 + g.under(8),
            });
        }

        let vind_dir = g.under(8);
        Ok(Self {
            b,
            h,
            param,
            vatten,
            landandel,
            vmax,
            v,
            p_steg,
            kron,
            land_rutor,
            klimat: Klimat::ny(STEG_PER_AR as f64),
            steg: 0,
            vind_dir,
            lukt_rov: vec![0.0; n],
            lukt_harmlos: vec![0.0; n],
            lukt_byte: vec![0.0; n],
            byten,
            trex,
            harmlos,
            g,
            nasta_uid,
            typ_markt: ByteTyp::Bas,
            typ_omarkt: ByteTyp::Bas,
            samla_prob: false,
            prob_data: Vec::new(),
            dod_tot_m: 0,
            dod_tot_o: 0,
            dod_rov_m: 0,
            dod_rov_o: 0,
            traff_m: 0,
            traff_o: 0,
            trex_invandring: 0,
            trex_fodslar: 0,
            trex_dodar: 0,
            trex_byten_dodade: 0,
            undvik_rov_sum_m: 0.0,
            undvik_rov_n_m: 0,
            undvik_h_sum_m: 0.0,
            undvik_h_n_m: 0,
            undvik_rov_sum_o: 0.0,
            undvik_rov_n_o: 0,
            undvik_h_sum_o: 0.0,
            undvik_h_n_o: 0,
            intag_sum_m: 0.0,
            intag_n_m: 0,
            intag_sum_o: 0.0,
            intag_n_o: 0,
            steg_efter_bett_m: 0,
            steg_tot_m: 0,
        })
    }

    /// Syntetisk testarena (för snabba röd-till-grön enhetsprov utan kartfiler).
    #[cfg(test)]
    pub fn syntetisk(b: usize, h: usize, fro: u64, param: B2Param) -> Self {
        let n = b * h;
        let mut land_rutor = Vec::with_capacity(n);
        for j in 0..h {
            for i in 0..b {
                land_rutor.push((i, j));
            }
        }
        let mut kron = vec![0.25; n];
        for c in 0..n {
            if (c / b + c % b) % 2 == 0 {
                kron[c] = 0.75;
            }
        }
        let mut g = Rng::new(fro);
        let mut nasta_uid = 1u64;
        let mut byten = Vec::new();
        for k in 0..80 {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            let gen = GenB2::slump(&mut g);
            byten.push(ByteB2 {
                lev: true,
                i,
                j,
                e: E_START,
                age: g.under(LIV as usize) as i64,
                hona: k % 2 == 0,
                markt: k % 2 == 0,
                uid: nasta_uid,
                hj: Some(HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::Bas, param.belonings_grind))),
                gen_z20: None,
                gen_gles: None,
                hj_z20: None,
                hj_gles: None,
                tabell: None,
                gen,
                egen_rng: Rng::new(nasta_uid),
                e_fore_steg: E_START,
                senaste_skada: 0.0,
                steg_sedan_lukt: 999,
                forra_lukt_rov: 0.0,
                forra_lukt_harmlos: 0.0,
                hade_lukt_rov_granne: false,
                hade_lukt_harmlos_granne: false,
                steg_sedan_skada: u32::MAX,
                antal_overlevda_bett: 0,
            });
            nasta_uid += 1;
        }
        let mut trex = Vec::new();
        for _ in 0..param.start_trex.min(6) {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            let gen = GenTrex::slump(&mut g);
            let hj = HjTrex::fran_gen(&gen);
            trex.push(TrexB2 {
                lev: true,
                i,
                j,
                e: param.trex_e_start,
                age: 100,
                uid: nasta_uid,
                gen,
                hj,
                egen_rng: Rng::new(nasta_uid ^ 0x74E8),
                e_fore_steg: param.trex_e_start,
                vila: 0,
            });
            nasta_uid += 1;
        }
        let mut harmlos = Vec::new();
        for _ in 0..trex.len() {
            let (i, j) = land_rutor[g.under(land_rutor.len())];
            harmlos.push(HarmlosKalla { i, j, dir: 3 });
        }
        Self {
            b,
            h,
            param,
            vatten: vec![Vatten::Land; n],
            landandel: vec![1.0; n],
            vmax: vec![40.0; n],
            v: vec![24.0; n],
            p_steg: vec![0.12; n],
            kron,
            land_rutor,
            klimat: Klimat::ny(STEG_PER_AR as f64),
            steg: 0,
            vind_dir: 2, // rakt österut (+1, 0)
            lukt_rov: vec![0.0; n],
            lukt_harmlos: vec![0.0; n],
            lukt_byte: vec![0.0; n],
            byten,
            trex,
            harmlos,
            g,
            nasta_uid,
            typ_markt: ByteTyp::Bas,
            typ_omarkt: ByteTyp::Bas,
            samla_prob: false,
            prob_data: Vec::new(),
            dod_tot_m: 0,
            dod_tot_o: 0,
            dod_rov_m: 0,
            dod_rov_o: 0,
            traff_m: 0,
            traff_o: 0,
            trex_invandring: 0,
            trex_fodslar: 0,
            trex_dodar: 0,
            trex_byten_dodade: 0,
            undvik_rov_sum_m: 0.0,
            undvik_rov_n_m: 0,
            undvik_h_sum_m: 0.0,
            undvik_h_n_m: 0,
            undvik_rov_sum_o: 0.0,
            undvik_rov_n_o: 0,
            undvik_h_sum_o: 0.0,
            undvik_h_n_o: 0,
            intag_sum_m: 0.0,
            intag_n_m: 0,
            intag_sum_o: 0.0,
            intag_n_o: 0,
            steg_efter_bett_m: 0,
            steg_tot_m: 0,
        }
    }

    /// Märker exakt hälften av levande byten (slumpvis men deterministiskt från `fro`),
    /// nollställer tävlingsstatistik och kopplar på de två hjärntyperna för parade tävlingen.
    pub fn markera_halften(&mut self, fro: u64, typ_markt: ByteTyp, typ_omarkt: ByteTyp) {
        self.typ_markt = typ_markt;
        self.typ_omarkt = typ_omarkt;
        let mut lev_idx: Vec<usize> = self
            .byten
            .iter()
            .enumerate()
            .filter(|(_, d)| d.lev)
            .map(|(k, _)| k)
            .collect();
        let mut gm = Rng::new(fro ^ 0x2361_A110_0007);
        for k in (1..lev_idx.len()).rev() {
            let r = gm.under(k + 1);
            lev_idx.swap(k, r);
        }
        let halv = lev_idx.len() / 2;
        for (pos, &idx) in lev_idx.iter().enumerate() {
            let m = pos < halv;
            let d = &mut self.byten[idx];
            d.markt = m;
            let typ = if m { typ_markt } else { typ_omarkt };
            if typ == ByteTyp::Tabell {
                d.hj = None;
                d.hj_z20 = None;
                d.gen_z20 = None;
                d.hj_gles = None;
                d.gen_gles = None;
                d.tabell = Some(TabellB2::ny());
            } else if matches!(typ, ByteTyp::MinneZ20 | ByteTyp::MinneFalskLuktZ20) {
                d.hj = None;
                d.tabell = None;
                d.hj_gles = None;
                d.gen_gles = None;
                let mut gz_rng = Rng::new(d.uid ^ 0x2370_2020_0001);
                let gz = GenB2Z20::slump(&mut gz_rng);
                d.hj_z20 = Some(Box::new(HjB2Z20::fran_gen(&gz)));
                d.gen_z20 = Some(Box::new(gz));
            } else if matches!(
                typ,
                ByteTyp::ArmBGles | ByteTyp::ArmBGlesFalsk | ByteTyp::ArmDKomb | ByteTyp::ArmDKombFalsk
            ) {
                d.hj = None;
                d.tabell = None;
                d.hj_z20 = None;
                d.gen_z20 = None;
                let mut gg_rng = Rng::new(d.uid ^ 0x2379_61E5_0007);
                let gg = GenB2Gles::slump(&mut gg_rng);
                let chock = matches!(typ, ByteTyp::ArmDKomb | ByteTyp::ArmDKombFalsk);
                d.hj_gles = Some(Box::new(HjB2Gles::fran_gen(&gg, chock)));
                d.gen_gles = Some(Box::new(gg));
            } else {
                d.tabell = None;
                d.hj_z20 = None;
                d.gen_z20 = None;
                d.hj_gles = None;
                d.gen_gles = None;
                d.hj = Some(HjB2::fran_gen(
                    &d.gen,
                    HjB2Config::ny(typ, self.param.belonings_grind),
                ));
            }
            d.senaste_skada = 0.0;
            d.steg_sedan_lukt = 999;
            d.steg_sedan_skada = u32::MAX;
            d.antal_overlevda_bett = 0;
        }
        self.steg_efter_bett_m = 0;
        self.steg_tot_m = 0;
        self.dod_tot_m = 0;
        self.dod_tot_o = 0;
        self.dod_rov_m = 0;
        self.dod_rov_o = 0;
        self.traff_m = 0;
        self.traff_o = 0;
        self.trex_invandring = 0;
        self.trex_fodslar = 0;
        self.trex_dodar = 0;
        self.trex_byten_dodade = 0;
        self.undvik_rov_sum_m = 0.0;
        self.undvik_rov_n_m = 0;
        self.undvik_h_sum_m = 0.0;
        self.undvik_h_n_m = 0;
        self.undvik_rov_sum_o = 0.0;
        self.undvik_rov_n_o = 0;
        self.undvik_h_sum_o = 0.0;
        self.undvik_h_n_o = 0;
        self.intag_sum_m = 0.0;
        self.intag_n_m = 0;
        self.intag_sum_o = 0.0;
        self.intag_n_o = 0;
        self.prob_data.clear();
    }

    /// Uppdaterar vinden och alla tre luktfälten med exakt Lean 4-verifierad advektionsfysik.
    pub fn uppdatera_vind_och_lukt(&mut self) {
        if self.g.f() < VIND_BYT_P {
            if self.g.f() < 0.5 {
                self.vind_dir = (self.vind_dir + 1) % 8;
            } else {
                self.vind_dir = (self.vind_dir + 7) % 8;
            }
        }
        let (dx_v, dy_v) = DIR[(self.vind_dir % 8) + 1];
        // Sidomedvind (±45° kring uppvindsriktningen) för realistisk doftplymskon (exakt 0 i motvind!)
        let (dx_l, dy_l) = DIR[((self.vind_dir + 7) % 8) + 1];
        let (dx_r, dy_r) = DIR[((self.vind_dir + 1) % 8) + 1];
        let n = self.b * self.h;
        let mut emit_rov = vec![0.0f64; n];
        let mut emit_harmlos = vec![0.0f64; n];
        let mut emit_byte = vec![0.0f64; n];

        for t in &self.trex {
            if t.lev {
                emit_rov[self.idx(t.i, t.j)] += 1.0;
            }
        }
        for hk in &self.harmlos {
            emit_harmlos[self.idx(hk.i, hk.j)] += 1.0;
        }
        for d in &self.byten {
            if d.lev {
                emit_byte[self.idx(d.i, d.j)] += 0.15;
            }
        }

        let mut ny_rov = vec![0.0f64; n];
        let mut ny_harmlos = vec![0.0f64; n];
        let mut ny_byte = vec![0.0f64; n];

        let kon_m = self.param.kon_mitt;
        let kon_s = 0.5 * (1.0 - kon_m);
        let gam_l = self.param.gamma_lukt;
        let bet_v = self.param.beta_vind;
        for j in 0..self.h {
            let uj = (j as i32 - dy_v).rem_euclid(self.h as i32) as usize;
            let ujl = (j as i32 - dy_l).rem_euclid(self.h as i32) as usize;
            let ujr = (j as i32 - dy_r).rem_euclid(self.h as i32) as usize;
            for i in 0..self.b {
                let ui = (i as i32 - dx_v).rem_euclid(self.b as i32) as usize;
                let uil = (i as i32 - dx_l).rem_euclid(self.b as i32) as usize;
                let uir = (i as i32 - dx_r).rem_euclid(self.b as i32) as usize;
                let c = j * self.b + i;
                let cu = uj * self.b + ui;
                let cul = ujl * self.b + uil;
                let cur = ujr * self.b + uir;
                let upp_rov = kon_m * self.lukt_rov[cu] + kon_s * self.lukt_rov[cul] + kon_s * self.lukt_rov[cur];
                let upp_h = kon_m * self.lukt_harmlos[cu] + kon_s * self.lukt_harmlos[cul] + kon_s * self.lukt_harmlos[cur];
                let upp_b = kon_m * self.lukt_byte[cu] + kon_s * self.lukt_byte[cul] + kon_s * self.lukt_byte[cur];
                ny_rov[c] = lukt_steg_scalar(gam_l, bet_v, self.lukt_rov[c], upp_rov, emit_rov[c]);
                ny_harmlos[c] = lukt_steg_scalar(gam_l, bet_v, self.lukt_harmlos[c], upp_h, emit_harmlos[c]);
                ny_byte[c] = lukt_steg_scalar(gam_l, bet_v, self.lukt_byte[c], upp_b, emit_byte[c].min(2.0));
            }
        }
        self.lukt_rov = ny_rov;
        self.lukt_harmlos = ny_harmlos;
        self.lukt_byte = ny_byte;
    }

    pub fn bygg_byte_sinnen(
        &self,
        i: usize,
        j: usize,
        energi: f64,
        antal_byte: &[u16],
        antal_trex: &[u16],
    ) -> [f64; S_B2] {
        let mut s = [0.0; S_B2];
        let (wx, wy) = vind_vektor(self.vind_dir);
        for k in 0..A_N {
            let (ni, nj) = self.granne(i, j, k);
            let c = self.idx(ni, nj);
            let vmax_c = self.vmax[c].max(1.0);
            s[k] = self.v[c] / vmax_c;
            s[9 + k] = (antal_byte[c] as f64).min(12.0) / 12.0;
            s[19 + k] = self.kron[c];
            let tr_c = antal_trex[c] as f64;
            let sikt = if k == 0 || !self.param.skymd_sikt {
                tr_c
            } else if self.kron[c] >= self.param.kron_dold_grans {
                // Helt dold T. rex bakom tät vegetation (LeCun uppgift 1 / Alt 1886)
                0.0
            } else {
                tr_c * synbarhet(self.kron[c])
            };
            s[28 + k] = sikt;
            s[37 + k] = self.lukt_rov[c];
            s[46 + k] = self.lukt_harmlos[c];
        }
        s[18] = (energi / E_DELA).clamp(0.0, 2.0);
        s[55] = wx;
        s[56] = wy;
        s
    }

    fn bygg_trex_sinnen(
        &self,
        i: usize,
        j: usize,
        energi: f64,
        antal_byte: &[u16],
        antal_trex: &[u16],
    ) -> [f64; S_TREX] {
        let mut s = [0.0; S_TREX];
        let (wx, wy) = vind_vektor(self.vind_dir);
        for k in 0..A_N {
            let (ni, nj) = self.granne(i, j, k);
            let c = self.idx(ni, nj);
            let b_c = (antal_byte[c] as f64).min(12.0) / 12.0;
            let sikt_b = if k == 0 || !self.param.skymd_sikt {
                b_c
            } else {
                b_c * synbarhet(self.kron[c])
            };
            s[k] = sikt_b;
            s[9 + k] = self.lukt_byte[c];
            s[18 + k] = self.kron[c];
            s[27 + k] = antal_trex[c] as f64;
        }
        s[36] = wx;
        s[37] = wy;
        s[38] = (energi / self.param.trex_e_dela).clamp(0.0, 2.0);
        s
    }

    fn min_trex_avstand(&self, i: usize, j: usize) -> usize {
        let mut best = usize::MAX;
        for t in &self.trex {
            if !t.lev {
                continue;
            }
            let dx = (i as i64 - t.i as i64).unsigned_abs() as usize;
            let dy = (j as i64 - t.j as i64).unsigned_abs() as usize;
            let d = dx.min(self.b - dx).max(dy.min(self.h - dy));
            if d < best {
                best = d;
            }
        }
        best
    }

    fn har_nara_trex(&self, i: usize, j: usize, max_r: usize) -> bool {
        self.min_trex_avstand(i, j) <= max_r
    }

    /// Kör ett helt tidssteg i Skede B2-arenan.
    pub fn steg(&mut self) {
        let n = self.b * self.h;

        // 1. Växtåterväxt med årstidscykel
        let fas = self.klimat.fas(self.steg);
        let k_f = self.klimat.g(fas).max(0.15);
        for c in 0..n {
            let tak = self.vmax[c];
            if tak > 0.0 {
                let tillv = self.p_steg[c] * k_f * (1.0 - self.v[c] / tak).max(0.0);
                self.v[c] = (self.v[c] + tillv).min(tak);
            }
        }

        // 2. Flytta ofarliga doftkällor (samma antal och rörelsemönster på land som T. rex)
        let antal_lev_trex = self.trex.iter().filter(|t| t.lev).count().max(1);
        while self.harmlos.len() < antal_lev_trex {
            let (i, j) = self.land_rutor[self.g.under(self.land_rutor.len())];
            self.harmlos.push(HarmlosKalla {
                i,
                j,
                dir: 1 + self.g.under(8),
            });
        }
        while self.harmlos.len() > antal_lev_trex {
            self.harmlos.pop();
        }
        for idx_h in 0..self.harmlos.len() {
            if self.g.f() < 0.25 {
                self.harmlos[idx_h].dir = 1 + self.g.under(8);
            }
            let (i, j, dir) = (self.harmlos[idx_h].i, self.harmlos[idx_h].j, self.harmlos[idx_h].dir);
            let (ni, nj) = self.granne(i, j, dir);
            if self.ar_land(self.idx(ni, nj)) {
                self.harmlos[idx_h].i = ni;
                self.harmlos[idx_h].j = nj;
            } else {
                self.harmlos[idx_h].dir = 1 + self.g.under(8);
            }
        }

        // 3. Uppdatera fysisk vind och alla tre luktfält
        self.uppdatera_vind_och_lukt();

        // 4. Räkna agenter per ruta för sinnen
        let mut antal_byte = vec![0u16; n];
        let mut antal_trex = vec![0u16; n];
        for d in &self.byten {
            if d.lev {
                antal_byte[self.idx(d.i, d.j)] = antal_byte[self.idx(d.i, d.j)].saturating_add(1);
            }
        }
        for t in &self.trex {
            if t.lev {
                antal_trex[self.idx(t.i, t.j)] = antal_trex[self.idx(t.i, t.j)].saturating_add(1);
            }
        }

        // 5. Bytesdjur: välj handling, mät ren steg-undvikning (Alt 1881), samla per-individ probdata (Alt 1885b) & rör sig
        let samla_nu = self.samla_prob && self.prob_data.len() < 40_000;
        let nb = self.byten.len();
        for idx_d in 0..nb {
            if !self.byten[idx_d].lev {
                continue;
            }
            let (di, dj, de, d_markt, d_uid) = (
                self.byten[idx_d].i,
                self.byten[idx_d].j,
                self.byten[idx_d].e,
                self.byten[idx_d].markt,
                self.byten[idx_d].uid,
            );
            let s = self.bygg_byte_sinnen(di, dj, de, &antal_byte, &antal_trex);
            let d_typ = if d_markt { self.typ_markt } else { self.typ_omarkt };
            // Alt 2370: modifiera bara kanal 37..46 för variansbevarande falsk lukt eller signal-takkontroll
            let mut s_hj = s;
            if matches!(
                d_typ,
                ByteTyp::MinneFalskLukt
                    | ByteTyp::MinneFalskLuktZ20
                    | ByteTyp::MinneFalskLuktGamma80
                    | ByteTyp::MinneSankFalskLukt
                    | ByteTyp::ArmAChockFalsk
                    | ByteTyp::ArmBGlesFalsk
                    | ByteTyp::ArmCOnlineFalsk
                    | ByteTyp::ArmDKombFalsk
            ) {
                // Rumsförskjuten (i + B/2, j + H/2)-kopia av lukt_rov: exakt samma varians/förutsägbarhet, 0 lokal T.rex-koppling!
                let fi = (di + self.b / 2) % self.b;
                let fj = (dj + self.h / 2) % self.h;
                for k in 0..A_N {
                    let (ni, nj) = self.granne(fi, fj, k);
                    s_hj[37 + k] = self.lukt_rov[self.idx(ni, nj)];
                }
            } else if d_typ == ByteTyp::TakSannNarhet {
                // D0a: sann närhet 1/(1 + d_min_trex) i var och en av de 9 rutorna
                for k in 0..A_N {
                    let (ni, nj) = self.granne(di, dj, k);
                    let dm = self.min_trex_avstand(ni, nj);
                    s_hj[37 + k] = if dm == usize::MAX { 0.0 } else { 1.0 / (1.0 + dm as f64) };
                }
            }

            let (a, z_inre, s_sk_nu) = {
                let d = &mut self.byten[idx_d];
                d.e_fore_steg = d.e;
                let lukt_max_9 = s[37..46].iter().copied().fold(0.0f64, f64::max);
                if lukt_max_9 > 0.08 {
                    d.steg_sedan_lukt = 0;
                } else {
                    d.steg_sedan_lukt = d.steg_sedan_lukt.saturating_add(1);
                }
                if let Some(ref mut hj) = d.hj {
                    let (act, z) = if d_typ == ByteTyp::TakOrakelFlykt {
                        let lukt9: [f64; A_N] = std::array::from_fn(|k| s[37 + k]);
                        hj.valj_lott_med_fuskflykt(&s_hj, &lukt9, &mut d.egen_rng)
                    } else {
                        hj.valj_lott(&s_hj, &mut d.egen_rng)
                    };
                    let sk = hj.senaste_skalade();
                    (act, z, sk)
                } else if let Some(ref mut hz) = d.hj_z20 {
                    let (act, z20) = hz.valj_lott(&s_hj, &mut d.egen_rng);
                    let z10: [f64; Z_B2] = std::array::from_fn(|r| z20[r]);
                    let sk = hz.forra_sk;
                    (act, z10, sk)
                } else if let Some(ref mut hg) = d.hj_gles {
                    let (act, zg) = hg.valj_lott(&s_hj, &mut d.egen_rng);
                    let z10: [f64; Z_B2] = std::array::from_fn(|r| if r < 6 { zg[r] } else { zg[Z_GLES_NU + (r - 6)] });
                    let sk = hg.forra_sk;
                    (act, z10, sk)
                } else if let Some(ref mut tab) = d.tabell {
                    let act = tab.valj(&s_hj, &mut d.egen_rng);
                    (act, [0.0; Z_B2], [0.0; S_B2])
                } else {
                    (d.egen_rng.under(A_N), [0.0; Z_B2], [0.0; S_B2])
                }
            };

            // Alt 1881: ren rörelseoberoende undvikning på NU-fältet (medel över 9 valbara rutor minus vald ruta, normaliserat).
            // Ett slumpmässigt val har väntevärde exakt 0.0; positivt = väljer ruta med mindre lukt än omgivningen.
            let max_r = s[37..46].iter().copied().fold(0.0f64, f64::max);
            let min_r = s[37..46].iter().copied().fold(f64::INFINITY, f64::min);
            if max_r > 0.12 && (max_r - min_r) > 0.04 {
                let med_r = s[37..46].iter().sum::<f64>() / A_N as f64;
                let poang_r = (med_r - s[37 + a]) / max_r;
                if d_markt {
                    self.undvik_rov_sum_m += poang_r;
                    self.undvik_rov_n_m += 1;
                } else {
                    self.undvik_rov_sum_o += poang_r;
                    self.undvik_rov_n_o += 1;
                }
            }
            let max_h = s[46..55].iter().copied().fold(0.0f64, f64::max);
            let min_h = s[46..55].iter().copied().fold(f64::INFINITY, f64::min);
            if max_h > 0.12 && (max_h - min_h) > 0.04 {
                let med_h = s[46..55].iter().sum::<f64>() / A_N as f64;
                let poang_h = (med_h - s[46 + a]) / max_h;
                if d_markt {
                    self.undvik_h_sum_m += poang_h;
                    self.undvik_h_n_m += 1;
                } else {
                    self.undvik_h_sum_o += poang_h;
                    self.undvik_h_n_o += 1;
                }
            }

            // Alt 1885b-prob (per individ): "dold nära T. rex EFTER att lukten upphört".
            // Samlas ENDAST på 100 % tysta, osynliga steg (sikt_rov == 0 och lukt_rov < 0.05 i hela 3x3-fönstret!):
            // - Positiv (label=true): dold T. rex inom <= 4 rutor OCH djuret kände rovdjurslukt 1..=5 steg tidigare (nu avklingad).
            // - Negativ (label=false): ingen T. rex inom <= 6 rutor på ett lika tyst, osynligt steg.
            if samla_nu && (d_uid % 6 == 0) {
                let sikt_max = s[28..37].iter().copied().fold(0.0f64, f64::max);
                let lukt_max = s[37..46].iter().copied().fold(0.0f64, f64::max);
                if sikt_max < 1e-6 && lukt_max < 0.05 {
                    let steg_sedan = self.byten[idx_d].steg_sedan_lukt;
                    let nylig_lukt = (1..=5).contains(&steg_sedan);
                    let pos = nylig_lukt && self.har_nara_trex(di, dj, 4);
                    let neg = !self.har_nara_trex(di, dj, 6);
                    if pos || (neg && (nylig_lukt || self.byten[idx_d].egen_rng.f() < 0.08)) {
                        self.prob_data.push(ProbPunkt {
                            uid: d_uid,
                            markt: d_markt,
                            z: z_inre,
                            s_ra: s,
                            s_sk: s_sk_nu,
                            label: pos,
                            matchad_lukt: nylig_lukt,
                        });
                    }
                }
            }

            // Utför rörelsen (land/träsk tillåtet, aldrig ut i hav)
            let (ni, nj) = self.granne(di, dj, a);
            let nc = self.idx(ni, nj);
            if self.ar_land(nc) {
                self.byten[idx_d].i = ni;
                self.byten[idx_d].j = nj;
                let ror_kost = match self.vatten[nc] {
                    Vatten::Trask => KOST_TRASK,
                    Vatten::Flodfara => KOST_FLOD,
                    Vatten::Sjo => KOST_SJO,
                    _ => 0.0,
                };
                self.byten[idx_d].e -= ror_kost;
            }
        }

        // 6. Autonoma *T. rex* väljer handling med sin EGNA JEPA-hjärna och flyttar
        let nt = self.trex.len();
        for idx_t in 0..nt {
            if !self.trex[idx_t].lev {
                continue;
            }
            let (ti, tj, te) = (self.trex[idx_t].i, self.trex[idx_t].j, self.trex[idx_t].e);
            let s_t = self.bygg_trex_sinnen(ti, tj, te, &antal_byte, &antal_trex);
            let a = {
                let t = &mut self.trex[idx_t];
                t.e_fore_steg = t.e;
                t.hj.valj_lott(&s_t, &mut t.egen_rng)
            };
            let (ni, nj) = self.granne(ti, tj, a);
            let nc = self.idx(ni, nj);
            if self.ar_land(nc) {
                self.trex[idx_t].i = ni;
                self.trex[idx_t].j = nj;
            }
            self.trex[idx_t].e -= self.param.trex_kost_steg;
            self.trex[idx_t].age += 1;
            if self.trex[idx_t].vila > 0 {
                self.trex[idx_t].vila -= 1;
            }
        }

        // 7. Bete (bytesdjur delar växtlighet i sin ruta)
        let mut byten_i_ruta: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (idx_d, d) in self.byten.iter().enumerate() {
            if d.lev {
                byten_i_ruta[self.idx(d.i, d.j)].push(idx_d);
            }
        }
        for c in 0..n {
            let lista = &byten_i_ruta[c];
            if lista.is_empty() {
                continue;
            }
            let behov = AT_DINO * lista.len() as f64;
            let taget = self.v[c].min(behov);
            self.v[c] -= taget;
            let per_djur = (taget / lista.len() as f64) * E_VAXT;
            for &idx_d in lista {
                self.byten[idx_d].e += per_djur - KOST_STEG;
                self.byten[idx_d].age += 1;
                self.byten[idx_d].senaste_skada = 0.0;
                self.byten[idx_d].steg_sedan_skada = self.byten[idx_d].steg_sedan_skada.saturating_add(1);
                if self.byten[idx_d].markt {
                    self.intag_sum_m += per_djur;
                    self.intag_n_m += 1;
                    self.steg_tot_m += 1;
                    if self.byten[idx_d].antal_overlevda_bett > 0 {
                        self.steg_efter_bett_m += 1;
                    }
                } else {
                    self.intag_sum_o += per_djur;
                    self.intag_n_o += 1;
                }
            }
        }

        // 8. Fysiska *T. rex*-möten (bakhållsbonus från tät vegetation enligt Alt 1886!)
        for idx_t in 0..self.trex.len() {
            if !self.trex[idx_t].lev || self.trex[idx_t].vila > 0 {
                continue;
            }
            let c = self.idx(self.trex[idx_t].i, self.trex[idx_t].j);
            let kandidater: Vec<usize> = byten_i_ruta[c]
                .iter()
                .copied()
                .filter(|&id| self.byten[id].lev)
                .collect();
            if kandidater.is_empty() {
                continue;
            }
            let bakhall = self.param.skymd_sikt && self.kron[c] >= self.param.kron_dold_grans;
            let p_traff = (self.param.p_angrepp_bas
                * if bakhall { self.param.bakhall_faktor } else { 1.0 })
            .min(0.95);
            if self.g.f() < p_traff {
                let mal = kandidater[self.g.under(kandidater.len())];
                let markt = self.byten[mal].markt;
                if markt {
                    self.traff_m += 1;
                } else {
                    self.traff_o += 1;
                }
                if self.g.f() < self.param.p_dod_vid_traff {
                    // Dödligt byte!
                    self.byten[mal].lev = false;
                    self.trex[idx_t].e = (self.trex[idx_t].e + self.param.trex_byte_energi)
                        .min(self.param.trex_e_dela * 2.0);
                    self.trex[idx_t].vila = self.param.trex_vila_steg;
                    self.trex_byten_dodade += 1;
                    if markt {
                        self.dod_rov_m += 1;
                        self.dod_tot_m += 1;
                    } else {
                        self.dod_rov_o += 1;
                        self.dod_tot_o += 1;
                    }
                } else {
                    // Icke-dödlig träff: fysisk smärta/skada = 1.0 till bytesdjurets kropp
                    self.byten[mal].senaste_skada = 1.0;
                    self.byten[mal].steg_sedan_skada = 0;
                    self.byten[mal].antal_overlevda_bett = self.byten[mal].antal_overlevda_bett.saturating_add(1);
                    self.byten[mal].e -= self.param.byte_skada_energi;
                    self.trex[idx_t].e = (self.trex[idx_t].e + self.param.trex_traff_energi)
                        .min(self.param.trex_e_dela * 2.0);
                }
            }
        }

        // 9. Kroppssignaler, ålder/svält-död och förökning för bytesdjur
        let mut nya_byten: Vec<ByteB2> = Vec::new();
        for idx_d in 0..self.byten.len() {
            if !self.byten[idx_d].lev {
                continue;
            }
            let d = &mut self.byten[idx_d];
            let de = d.e - d.e_fore_steg;
            let w_eff = d.gen.w_skada.max(0.0);
            let r_t = de - w_eff * d.senaste_skada;
            if let Some(ref mut hj) = d.hj {
                hj.ge_beloning(r_t);
            }
            if let Some(ref mut hz) = d.hj_z20 {
                hz.ge_beloning(r_t);
            }
            if let Some(ref mut hg) = d.hj_gles {
                hg.ge_beloning(r_t);
            }
            if let Some(ref mut tab) = d.tabell {
                tab.uppdatera(r_t);
            }
            if d.e <= 0.0 || d.age >= LIV {
                d.lev = false;
                if d.markt {
                    self.dod_tot_m += 1;
                } else {
                    self.dod_tot_o += 1;
                }
            }
        }

        // Förökning för bytesdjur (hona + hane i samma eller angränsande ruta, annars asexuell fallback om ensam)
        let honor_redo: Vec<usize> = self
            .byten
            .iter()
            .enumerate()
            .filter(|(_, d)| d.lev && d.hona && d.age >= LITEN && d.e >= E_DELA)
            .map(|(k, _)| k)
            .collect();
        for idx_mor in honor_redo {
            let (mi, mj, m_markt) = (
                self.byten[idx_mor].i,
                self.byten[idx_mor].j,
                self.byten[idx_mor].markt,
            );
            // Hitta hane i samma eller någon av de 8 grannrutorna (inom samma tävlingslinje)
            let mut far_opt: Option<usize> = None;
            for k in 0..A_N {
                let (ni, nj) = self.granne(mi, mj, k);
                let c = self.idx(ni, nj);
                for &cand in &byten_i_ruta[c] {
                    let dc = &self.byten[cand];
                    if dc.lev && !dc.hona && dc.age >= LITEN && dc.markt == m_markt {
                        far_opt = Some(cand);
                        break;
                    }
                }
                if far_opt.is_some() {
                    break;
                }
            }
            let idx_far = far_opt.unwrap_or(idx_mor);
            let uid = self.nasta_uid;
            self.nasta_uid += 1;
            let barn_gen = GenB2::arv(&self.byten[idx_mor].gen, &self.byten[idx_far].gen, uid);
            self.byten[idx_mor].e *= 0.5;
            let barn_e = self.byten[idx_mor].e.min(E_HANE);
            let typ = if m_markt { self.typ_markt } else { self.typ_omarkt };
            let (hj, gen_z20, hj_z20, gen_gles, hj_gles, tabell) = if typ == ByteTyp::Tabell {
                (None, None, None, None, None, Some(TabellB2::ny()))
            } else if matches!(typ, ByteTyp::MinneZ20 | ByteTyp::MinneFalskLuktZ20) {
                let gz_barn = match (&self.byten[idx_mor].gen_z20, &self.byten[idx_far].gen_z20) {
                    (Some(gm), Some(gf)) => GenB2Z20::arv(gm, gf, uid),
                    (Some(gm), None) => GenB2Z20::arv(gm, gm, uid),
                    _ => {
                        let mut gr = Rng::new(uid ^ 0x2370_2020_0002);
                        GenB2Z20::slump(&mut gr)
                    }
                };
                let hz = HjB2Z20::fran_gen(&gz_barn);
                (None, Some(Box::new(gz_barn)), Some(Box::new(hz)), None, None, None)
            } else if matches!(
                typ,
                ByteTyp::ArmBGles | ByteTyp::ArmBGlesFalsk | ByteTyp::ArmDKomb | ByteTyp::ArmDKombFalsk
            ) {
                let gg_barn = match (&self.byten[idx_mor].gen_gles, &self.byten[idx_far].gen_gles) {
                    (Some(gm), Some(gf)) => GenB2Gles::arv(gm, gf, uid),
                    (Some(gm), None) => GenB2Gles::arv(gm, gm, uid),
                    _ => {
                        let mut gr = Rng::new(uid ^ 0x2379_61E5_0002);
                        GenB2Gles::slump(&mut gr)
                    }
                };
                let chock = matches!(typ, ByteTyp::ArmDKomb | ByteTyp::ArmDKombFalsk);
                let hg = HjB2Gles::fran_gen(&gg_barn, chock);
                (None, None, None, Some(Box::new(gg_barn)), Some(Box::new(hg)), None)
            } else {
                (
                    Some(HjB2::fran_gen(
                        &barn_gen,
                        HjB2Config::ny(typ, self.param.belonings_grind),
                    )),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };
            nya_byten.push(ByteB2 {
                lev: true,
                i: mi,
                j: mj,
                e: barn_e,
                age: 0,
                hona: (uid % 2) == 0,
                markt: m_markt,
                uid,
                gen: barn_gen,
                gen_z20,
                gen_gles,
                hj,
                hj_z20,
                hj_gles,
                tabell,
                egen_rng: Rng::new(uid),
                e_fore_steg: barn_e,
                senaste_skada: 0.0,
                steg_sedan_lukt: 999,
                forra_lukt_rov: 0.0,
                forra_lukt_harmlos: 0.0,
                hade_lukt_rov_granne: false,
                hade_lukt_harmlos_granne: false,
                steg_sedan_skada: u32::MAX,
                antal_overlevda_bett: 0,
            });
        }
        if !nya_byten.is_empty() {
            self.byten.extend(nya_byten);
        }
        if self.steg % 50 == 0 {
            self.byten.retain(|d| d.lev);
        }

        // 10. Kroppssignal (ren ΔE_trex), död, förökning och golvskydd för *T. rex*
        let mut nya_trex: Vec<TrexB2> = Vec::new();
        let mut lev_trex = 0usize;
        for idx_t in 0..self.trex.len() {
            if !self.trex[idx_t].lev {
                continue;
            }
            let t = &mut self.trex[idx_t];
            let de_t = t.e - t.e_fore_steg;
            t.hj.ge_beloning(de_t);
            if t.e <= 0.0 || t.age >= LIV {
                t.lev = false;
                self.trex_dodar += 1;
            } else {
                lev_trex += 1;
            }
        }
        for idx_t in 0..self.trex.len() {
            if lev_trex + nya_trex.len() >= self.param.max_trex {
                break;
            }
            if self.trex[idx_t].lev
                && self.trex[idx_t].age >= LITEN / 2
                && self.trex[idx_t].e >= self.param.trex_e_dela
            {
                let uid = self.nasta_uid;
                self.nasta_uid += 1;
                let barn_gen = self.trex[idx_t].gen.mutera(uid);
                let barn_hj = HjTrex::fran_gen(&barn_gen);
                self.trex[idx_t].e *= 0.5;
                let barn_e = self.trex[idx_t].e;
                nya_trex.push(TrexB2 {
                    lev: true,
                    i: self.trex[idx_t].i,
                    j: self.trex[idx_t].j,
                    e: barn_e,
                    age: 0,
                    uid,
                    gen: barn_gen,
                    hj: barn_hj,
                    egen_rng: Rng::new(uid ^ 0x74E8),
                    e_fore_steg: barn_e,
                    vila: 0,
                });
                self.trex_fodslar += 1;
            }
        }
        self.trex.extend(nya_trex);
        // Golvskydd: om *T. rex* faller under min_trex invandrar ny *T. rex* från omgivningen (loggas!)
        let aktiva_trex: Vec<usize> = self
            .trex
            .iter()
            .enumerate()
            .filter(|(_, t)| t.lev)
            .map(|(k, _)| k)
            .collect();
        if aktiva_trex.len() < self.param.min_trex {
            let saknas = self.param.min_trex - aktiva_trex.len();
            for _ in 0..saknas {
                let uid = self.nasta_uid;
                self.nasta_uid += 1;
                let (i, j) = self.land_rutor[self.g.under(self.land_rutor.len())];
                let gen = if !aktiva_trex.is_empty() {
                    let k = aktiva_trex[self.g.under(aktiva_trex.len())];
                    self.trex[k].gen.mutera(uid)
                } else {
                    GenTrex::slump(&mut self.g)
                };
                let hj = HjTrex::fran_gen(&gen);
                self.trex.push(TrexB2 {
                    lev: true,
                    i,
                    j,
                    e: self.param.trex_e_start,
                    age: 0,
                    uid,
                    gen,
                    hj,
                    egen_rng: Rng::new(uid ^ 0x74E8),
                    e_fore_steg: self.param.trex_e_start,
                    vila: 0,
                });
                self.trex_invandring += 1;
            }
        }
        if self.steg % 50 == 0 {
            self.trex.retain(|t| t.lev);
        }

        self.steg += 1;
    }

    pub fn antal_levande(&self) -> (usize, usize, usize) {
        let mut m = 0usize;
        let mut o = 0usize;
        for d in &self.byten {
            if d.lev {
                if d.markt {
                    m += 1;
                } else {
                    o += 1;
                }
            }
        }
        let tr = self.trex.iter().filter(|t| t.lev).count();
        (m, o, tr)
    }
}

// ---------------------------------------------------------------------------
// Alt 1885b: Linjär prob med hållna ROC-AUC (inre bild vs råa sinnen vs slump)
// ---------------------------------------------------------------------------

/// Beräknar exakt ROC-AUC (Mann-Whitney U-statistika) givet (poäng, sann_etikett).
pub fn berakna_auc(par: &[(f64, bool)]) -> f64 {
    let mut pos: Vec<f64> = Vec::new();
    let mut neg: Vec<f64> = Vec::new();
    for &(s, y) in par {
        if s.is_finite() {
            if y {
                pos.push(s);
            } else {
                neg.push(s);
            }
        }
    }
    if pos.is_empty() || neg.is_empty() {
        return f64::NAN;
    }
    let mut vinster = 0.0f64;
    for &p in &pos {
        for &n in &neg {
            if p > n {
                vinster += 1.0;
            } else if (p - n).abs() <= 1e-12 {
                vinster += 0.5;
            }
        }
    }
    vinster / (pos.len() as f64 * neg.len() as f64)
}

/// Enkel regulariserad linjär diskriminant (Ridge-regression med sluten Gauss-eliminering)
/// tränad på första halvan och utvärderad med ROC-AUC på andra halvan.
pub fn linjar_prob_auc<const D: usize>(
    data: &[([f64; D], bool)],
    ridge: f64,
    slumpa_etikett: bool,
    fro: u64,
) -> f64 {
    let n = data.len();
    if n < 20 {
        return f64::NAN;
    }
    let split = n / 2;
    let mut etiketter: Vec<bool> = data.iter().map(|(_, y)| *y).collect();
    if slumpa_etikett {
        let mut g = Rng::new(fro ^ 0x1885_510A);
        for k in (1..etiketter.len()).rev() {
            let r = g.under(k + 1);
            etiketter.swap(k, r);
        }
    }
    let dp1 = D + 1;
    let mut a = vec![vec![0.0f64; dp1]; dp1];
    let mut b = vec![0.0f64; dp1];
    for idx in 0..split {
        let x = &data[idx].0;
        let y = if etiketter[idx] { 1.0 } else { -1.0 };
        for r in 0..dp1 {
            let xr = if r < D { x[r] } else { 1.0 };
            b[r] += xr * y;
            for c in 0..dp1 {
                let xc = if c < D { x[c] } else { 1.0 };
                a[r][c] += xr * xc;
            }
        }
    }
    for r in 0..D {
        a[r][r] += ridge;
    }
    a[D][D] += 1e-3;

    // Gauss-eliminering med partiell pivotering
    for col in 0..dp1 {
        let mut piv = col;
        let mut max_v = a[col][col].abs();
        for row in (col + 1)..dp1 {
            if a[row][col].abs() > max_v {
                max_v = a[row][col].abs();
                piv = row;
            }
        }
        if max_v < 1e-12 {
            return f64::NAN;
        }
        if piv != col {
            a.swap(col, piv);
            b.swap(col, piv);
        }
        let diag = a[col][col];
        for row in (col + 1)..dp1 {
            let f = a[row][col] / diag;
            for c in col..dp1 {
                a[row][c] -= f * a[col][c];
            }
            b[row] -= f * b[col];
        }
    }
    let mut w = vec![0.0f64; dp1];
    for r in (0..dp1).rev() {
        let mut s = b[r];
        for c in (r + 1)..dp1 {
            s -= a[r][c] * w[c];
        }
        w[r] = s / a[r][r];
    }

    let mut test_par = Vec::with_capacity(n - split);
    for idx in split..n {
        let x = &data[idx].0;
        let mut score = w[D];
        for r in 0..D {
            score += w[r] * x[r];
        }
        test_par.push((score, etiketter[idx]));
    }
    berakna_auc(&test_par)
}

/// Kör den linjära Alt 1885b-proben SEPARAT per individ (eftersom olika individer har olika slumpkodare wk)
/// och tar medel-AUC över alla individer som har tillräckligt många hållna positiva och negativa punkter.
pub fn per_individ_prob_auc(
    per_individ: &HashMap<u64, Vec<&ProbPunkt>>,
    fro: u64,
) -> (f64, f64, f64) {
    let mut ai_sum = 0.0;
    let mut ar_sum = 0.0;
    let mut as_sum = 0.0;
    let mut n_ok = 0usize;
    let mut nycklar: Vec<u64> = per_individ.keys().copied().collect();
    nycklar.sort_unstable();
    for uid in nycklar {
        let pts = &per_individ[&uid];
        if pts.len() < 20 {
            continue;
        }
        let pos = pts.iter().filter(|p| p.label).count();
        let neg = pts.len() - pos;
        if pos < 5 || neg < 5 {
            continue;
        }
        let z_pts: Vec<([f64; Z_B2], bool)> = pts.iter().map(|p| (p.z, p.label)).collect();
        let s_pts: Vec<([f64; S_B2], bool)> = pts.iter().map(|p| (p.s_ra, p.label)).collect();
        let ai = linjar_prob_auc(&z_pts, 2.0, false, fro ^ uid);
        let ar = linjar_prob_auc(&s_pts, 10.0, false, fro ^ uid);
        let asl = linjar_prob_auc(&z_pts, 2.0, true, fro ^ uid);
        if ai.is_finite() && ar.is_finite() && asl.is_finite() {
            ai_sum += ai;
            ar_sum += ar;
            as_sum += asl;
            n_ok += 1;
        }
    }
    if n_ok > 0 {
        let inv = 1.0 / n_ok as f64;
        (ai_sum * inv, ar_sum * inv, as_sum * inv)
    } else {
        (f64::NAN, f64::NAN, f64::NAN)
    }
}

// ---------------------------------------------------------------------------
// Utfall och tävlingskörning (`kalib_b2`, `sparb2361`, `slaihops2361`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct B2ArmUtfall {
    pub fro: u64,
    pub arm_idx: usize,
    pub arm_namn: String,
    pub fokus_andel: f64,
    pub n_markt_slut: usize,
    pub n_omarkt_slut: usize,
    pub n_trex_slut: usize,
    pub utdod_byte: bool,
    pub utdod_trex: bool,
    pub rov_dod_andel: f64,
    pub trex_byten_dodade: u64,
    pub trex_invandring: u64,
    pub trex_krit_r2: f64,
    pub fokus_krit_r2: f64,
    pub fokus_w_skada: f64,
    /// Alt 1885b: linjär prob ROC-AUC på dold nära *T. rex* efter att lukten avklingat.
    pub prob_n: usize,
    pub prob_pos: usize,
    pub auc_inre: f64,
    pub auc_ra: f64,
    pub auc_slump: f64,
    /// Alt 1881: stegvis undvikning av rovdjurslukt minus lika stark ofarlig lukt.
    pub undvik_rov_fokus: f64,
    pub undvik_harmlos_fokus: f64,
    pub undvik_gap_fokus: f64,
    /// Blind KanalGrind per sinnesgrupp hos fokusgruppen.
    pub g_vaxt: f64,
    pub g_djur: f64,
    pub g_kron: f64,
    pub g_sikt_rov: f64,
    pub g_lukt_rov: f64,
    pub g_lukt_harmlos: f64,
    pub g_vind: f64,
    pub sek: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct B2KorningRa {
    pub param: B2Param,
    pub inkor_ar: i64,
    pub tavling_ar: i64,
    pub utfall: Vec<B2ArmUtfall>,
}

pub const B2_ARM_NAMN: [&str; 8] = [
    "B2_Noll",
    "B2_Bas_StrykWM_A",
    "B2_Bas_StrykWM_Spegel",
    "B2_Stryk_Lukt",
    "B2_Minne_vs_Bas",
    "B2_Minne_vs_StrykLukt",
    "B2_Tabell",
    "B2_Utan_SkymdSikt",
];

fn medel_finita(xs: impl Iterator<Item = f64>) -> f64 {
    let mut s = 0.0;
    let mut n = 0usize;
    for x in xs {
        if x.is_finite() {
            s += x;
            n += 1;
        }
    }
    if n > 0 { s / n as f64 } else { f64::NAN }
}

pub fn kor_en_arm_b2(
    sim_bas: &SimB2,
    fro: u64,
    arm_idx: usize,
    tavling_ar: i64,
) -> B2ArmUtfall {
    let t0 = Instant::now();
    let (typ_m, typ_o, fokus_m, skymd_sikt) = match arm_idx {
        0 => (ByteTyp::Bas, ByteTyp::Bas, true, true),
        1 => (ByteTyp::Bas, ByteTyp::StrykWm, true, true),
        2 => (ByteTyp::StrykWm, ByteTyp::Bas, false, true),
        3 => (ByteTyp::Bas, ByteTyp::StrykLukt, true, true),
        4 => (ByteTyp::Minne, ByteTyp::Bas, true, true),
        5 => (ByteTyp::Minne, ByteTyp::MinneStrykLukt, true, true),
        6 => (ByteTyp::Minne, ByteTyp::Tabell, true, true),
        7 => (ByteTyp::Minne, ByteTyp::MinneStrykLukt, true, false),
        _ => unreachable!(),
    };

    let mut sim = sim_bas.clone();
    sim.param.skymd_sikt = skymd_sikt;
    sim.markera_halften(fro, typ_m, typ_o);

    let total_steg = tavling_ar * STEG_PER_AR;
    let prob_start = (tavling_ar / 2) * STEG_PER_AR;
    let mut andel_sum = 0.0;
    let mut andel_n = 0usize;

    for st in 0..total_steg {
        sim.samla_prob = st >= prob_start;
        sim.steg();
        if (st + 1) % STEG_PER_AR == 0 && (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR {
            let (m, o, _) = sim.antal_levande();
            let tot = m + o;
            if tot > 0 {
                let f = if fokus_m { m } else { o };
                andel_sum += f as f64 / tot as f64;
                andel_n += 1;
            }
        }
    }

    let (m_slut, o_slut, tr_slut) = sim.antal_levande();
    let fokus_andel = if andel_n > 0 {
        andel_sum / andel_n as f64
    } else if m_slut + o_slut > 0 {
        let f = if fokus_m { m_slut } else { o_slut };
        f as f64 / (m_slut + o_slut) as f64
    } else {
        0.0
    };

    let dod_tot = sim.dod_tot_m + sim.dod_tot_o;
    let dod_rov = sim.dod_rov_m + sim.dod_rov_o;
    let rov_dod_andel = if dod_tot > 0 {
        dod_rov as f64 / dod_tot as f64
    } else {
        0.0
    };

    let fokus_byten: Vec<&ByteB2> = sim
        .byten
        .iter()
        .filter(|d| d.lev && d.markt == fokus_m)
        .collect();
    let fokus_krit_r2 = medel_finita(
        fokus_byten
            .iter()
            .filter_map(|d| d.hj.as_ref().map(|h| h.krit_r2())),
    );
    let fokus_w_skada = medel_finita(fokus_byten.iter().map(|d| d.gen.w_skada));
    let trex_krit_r2 = medel_finita(sim.trex.iter().filter(|t| t.lev).map(|t| t.hj.krit_r2()));

    let g_vaxt = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(0, 9))));
    let g_djur = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(9, 9))));
    let g_kron = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(19, 9))));
    let g_sikt_rov = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(28, 9))));
    let g_lukt_rov = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(37, 9))));
    let g_lukt_harmlos = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(46, 9))));
    let g_vind = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(55, 2))));

    // Alt 1885b linjär prob PER INDIVID (eftersom varje individ har en egen slumpkodare wk!)
    let mut per_individ: HashMap<u64, Vec<&ProbPunkt>> = HashMap::new();
    let mut prob_n_tot = 0usize;
    let mut pos_c = 0usize;
    for p in &sim.prob_data {
        if p.markt == fokus_m {
            prob_n_tot += 1;
            if p.label {
                pos_c += 1;
            }
            per_individ.entry(p.uid).or_default().push(p);
        }
    }
    let (auc_inre, auc_ra, auc_slump) = per_individ_prob_auc(&per_individ, fro);

    let (ur_s, ur_n, uh_s, uh_n) = if fokus_m {
        (sim.undvik_rov_sum_m, sim.undvik_rov_n_m, sim.undvik_h_sum_m, sim.undvik_h_n_m)
    } else {
        (sim.undvik_rov_sum_o, sim.undvik_rov_n_o, sim.undvik_h_sum_o, sim.undvik_h_n_o)
    };
    let undvik_rov_fokus = if ur_n > 0 { ur_s / ur_n as f64 } else { 0.0 };
    let undvik_harmlos_fokus = if uh_n > 0 { uh_s / uh_n as f64 } else { 0.0 };

    B2ArmUtfall {
        fro,
        arm_idx,
        arm_namn: B2_ARM_NAMN[arm_idx].to_string(),
        fokus_andel,
        n_markt_slut: m_slut,
        n_omarkt_slut: o_slut,
        n_trex_slut: tr_slut,
        utdod_byte: m_slut + o_slut == 0,
        utdod_trex: tr_slut == 0,
        rov_dod_andel,
        trex_byten_dodade: sim.trex_byten_dodade,
        trex_invandring: sim.trex_invandring,
        trex_krit_r2,
        fokus_krit_r2,
        fokus_w_skada,
        prob_n: prob_n_tot,
        prob_pos: pos_c,
        auc_inre,
        auc_ra,
        auc_slump,
        undvik_rov_fokus,
        undvik_harmlos_fokus,
        undvik_gap_fokus: undvik_rov_fokus - undvik_harmlos_fokus,
        g_vaxt,
        g_djur,
        g_kron,
        g_sikt_rov,
        g_lukt_rov,
        g_lukt_harmlos,
        g_vind,
        sek: t0.elapsed().as_secs_f64(),
    }
}

pub fn kora_sparb2361(
    fron: &[u64],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    param: B2Param,
    ra_ut: Option<&Path>,
) -> Result<String, String> {
    let t_start = Instant::now();
    // 1. Bygg och kör in basvärldarna parallellt
    let jobb_bas: Vec<u64> = fron.to_vec();
    let nt = tradar.max(1).min(jobb_bas.len().max(1));
    let res_bas: Arc<std::sync::Mutex<Result<Vec<(u64, SimB2)>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(Vec::new())));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|sc| {
        for _ in 0..nt {
            let jobb_ref = &jobb_bas;
            let idx_c = Arc::clone(&idx_atom);
            let res_c = Arc::clone(&res_bas);
            sc.spawn(move || loop {
                let k = idx_c.fetch_add(1, Ordering::Relaxed);
                if k >= jobb_ref.len() {
                    break;
                }
                let fro = jobb_ref[k];
                let r = SimB2::fran_hell_creek(fro, param).map(|mut sim| {
                    for _ in 0..(inkor_ar * STEG_PER_AR) {
                        sim.steg();
                    }
                    (fro, sim)
                });
                let mut guard = res_c.lock().unwrap();
                match (&mut *guard, r) {
                    (Ok(vec), Ok(par)) => vec.push(par),
                    (Ok(_), Err(e)) => *guard = Err(e),
                    _ => {}
                }
            });
        }
    });
    let mut bas_varldar = Arc::try_unwrap(res_bas).unwrap().into_inner().unwrap()?;
    bas_varldar.sort_by_key(|(f, _)| *f);

    // 2. Kör alla 8 parade armar över alla världar parallellt
    let mut uppgifter: Vec<(usize, usize)> = Vec::with_capacity(bas_varldar.len() * B2_ARM_NAMN.len());
    for vi in 0..bas_varldar.len() {
        for ai in 0..B2_ARM_NAMN.len() {
            uppgifter.push((vi, ai));
        }
    }
    let totalt = uppgifter.len();
    let nt_arm = tradar.max(1).min(totalt.max(1));
    let klara = Arc::new(AtomicUsize::new(0));
    let u_idx = Arc::new(AtomicUsize::new(0));
    let utfall_arc: Arc<std::sync::Mutex<Vec<B2ArmUtfall>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(totalt)));

    std::thread::scope(|sc| {
        for _ in 0..nt_arm {
            let bas_ref = &bas_varldar;
            let upp_ref = &uppgifter;
            let u_c = Arc::clone(&u_idx);
            let k_c = Arc::clone(&klara);
            let ut_c = Arc::clone(&utfall_arc);
            sc.spawn(move || loop {
                let k = u_c.fetch_add(1, Ordering::Relaxed);
                if k >= upp_ref.len() {
                    break;
                }
                let (vi, ai) = upp_ref[k];
                let (fro, ref sim_bas) = bas_ref[vi];
                let u = kor_en_arm_b2(sim_bas, fro, ai, tavling_ar);
                let done = k_c.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "[Alt 2361] klara {done}/{totalt}: frö {fro} {} -> fokus={:.1}% rov_dod={:.1}% auc_inre={:.3} auc_ra={:.3} ({:.1}s)",
                    u.arm_namn,
                    u.fokus_andel * 100.0,
                    u.rov_dod_andel * 100.0,
                    u.auc_inre,
                    u.auc_ra,
                    u.sek
                );
                ut_c.lock().unwrap().push(u);
            });
        }
    });

    let mut utfall = Arc::try_unwrap(utfall_arc).unwrap().into_inner().unwrap();
    utfall.sort_by_key(|u| (u.arm_idx, u.fro));

    let ra = B2KorningRa {
        param,
        inkor_ar,
        tavling_ar,
        utfall,
    };
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = std::fs::create_dir_all(par);
        }
        let js = serde_json::to_string_pretty(&ra).map_err(|e| format!("json: {e}"))?;
        std::fs::write(p, js).map_err(|e| format!("skriv ra_ut: {e}"))?;
    }

    Ok(formatera_b2_rapport(&ra, t_start.elapsed().as_secs_f64()))
}

pub fn slaihops_2361(filer: &[PathBuf]) -> Result<String, String> {
    if filer.is_empty() {
        return Err("slaihops2361 kräver minst en JSON-fil".to_string());
    }
    let mut param_opt = None;
    let mut inkor_ar = 0;
    let mut tavling_ar = 0;
    let mut alla: Vec<B2ArmUtfall> = Vec::new();
    for p in filer {
        let txt = std::fs::read_to_string(p).map_err(|e| format!("läs {}: {e}", p.display()))?;
        let del: B2KorningRa =
            serde_json::from_str(&txt).map_err(|e| format!("tolka {}: {e}", p.display()))?;
        if param_opt.is_none() {
            param_opt = Some(del.param);
            inkor_ar = del.inkor_ar;
            tavling_ar = del.tavling_ar;
        }
        alla.extend(del.utfall);
    }
    alla.sort_by_key(|u| (u.arm_idx, u.fro));
    let ra = B2KorningRa {
        param: param_opt.unwrap_or_default(),
        inkor_ar,
        tavling_ar,
        utfall: alla,
    };
    Ok(formatera_b2_rapport(&ra, 0.0))
}

pub fn formatera_b2_rapport(ra: &B2KorningRa, total_sek: f64) -> String {
    let mut out = String::new();
    out.push_str("# Alt 2361 — Skede B, steg B2 (250 m arena, autonoma T. rex, vind/lukt & skymd sikt)\n\n");
    out.push_str(&format!(
        "- Inkörning: `{}` år | Tävling: `{}` år | Körtid: `{:.1} s`\n",
        ra.inkor_ar, ra.tavling_ar, total_sek
    ));
    out.push_str(&format!(
        "- Parametrar: `vaxt_skala={:.2}`, `start_trex={}`, `p_angrepp_bas={:.2}`, `bakhall_faktor={:.2}`, `p_dod={:.2}`, `belonings_grind={}`\n\n",
        ra.param.vaxt_skala,
        ra.param.start_trex,
        ra.param.p_angrepp_bas,
        ra.param.bakhall_faktor,
        ra.param.p_dod_vid_traff,
        ra.param.belonings_grind
    ));

    out.push_str("| Arm | n | Fokus % | Vinster (>50%) | Utdöd (B/T) | Rovdöd % | T.rex slut (inv) | AUC inre | AUC rå | ΔAUC | AUC slump | Undvik gap (Alt 1881) | g_lukt_r | g_lukt_h | g_sikt_r | g_kron | w_skada | Krit R² |\n");
    out.push_str("|---|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");

    for (ai, namn) in B2_ARM_NAMN.iter().enumerate() {
        let del: Vec<&B2ArmUtfall> = ra.utfall.iter().filter(|u| u.arm_idx == ai).collect();
        if del.is_empty() {
            continue;
        }
        let n = del.len();
        let med_f = medel_finita(del.iter().map(|u| u.fokus_andel)) * 100.0;
        let vinster = del.iter().filter(|u| u.fokus_andel > 0.5).count();
        let ud_b = del.iter().filter(|u| u.utdod_byte).count();
        let ud_t = del.iter().filter(|u| u.utdod_trex).count();
        let med_rov = medel_finita(del.iter().map(|u| u.rov_dod_andel)) * 100.0;
        let med_tr = medel_finita(del.iter().map(|u| u.n_trex_slut as f64));
        let med_inv = medel_finita(del.iter().map(|u| u.trex_invandring as f64));
        let auc_i = medel_finita(del.iter().map(|u| u.auc_inre));
        let auc_r = medel_finita(del.iter().map(|u| u.auc_ra));
        let auc_s = medel_finita(del.iter().map(|u| u.auc_slump));
        let gap = medel_finita(del.iter().map(|u| u.undvik_gap_fokus));
        let g_lr = medel_finita(del.iter().map(|u| u.g_lukt_rov));
        let g_lh = medel_finita(del.iter().map(|u| u.g_lukt_harmlos));
        let g_sr = medel_finita(del.iter().map(|u| u.g_sikt_rov));
        let g_kr = medel_finita(del.iter().map(|u| u.g_kron));
        let ws = medel_finita(del.iter().map(|u| u.fokus_w_skada));
        let kr2 = medel_finita(del.iter().map(|u| u.fokus_krit_r2));

        out.push_str(&format!(
            "| `{}` | {} | {:.1}% | {}/{} | {}/{} | {:.1}% | {:.1} ({:.1}) | {:.3} | {:.3} | {:+.3} | {:.3} | {:+.4} | {:.3} | {:.3} | {:.3} | {:.3} | {:+.2} | {:.3} |\n",
            namn, n, med_f, vinster, n, ud_b, ud_t, med_rov, med_tr, med_inv,
            auc_i, auc_r, auc_i - auc_r, auc_s, gap, g_lr, g_lh, g_sr, g_kr, ws, kr2
        ));
    }
    out
}

/// Snabb 5-världars förkalibrering (`kalib_b2`) för att kontrollera ekologi, körtid och KanalGrind.
pub fn kora_kalib_b2(
    fron: &[u64],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    param: B2Param,
) -> Result<String, String> {
    kora_sparb2361(fron, inkor_ar, tavling_ar, tradar, param, None)
}

// ---------------------------------------------------------------------------
// Röd-till-grön enhetsprov (fysik, skymd sikt, Alt 1885b-prob, 8 armar, hopslagning)
// ---------------------------------------------------------------------------

/// Alt 2370: utökad per-individ prob som särredovisar oskalade (`auc_ra`), skalade (`auc_skalad`),
/// lukthistoriamatchade (`auc_inre_matchad`, `auc_skalad_matchad`) samt exakt individantal `n_ok`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ProbDiag2370 {
    pub auc_inre: f64,
    pub auc_ra: f64,
    pub auc_skalad: f64,
    pub auc_slump: f64,
    pub n_ok: usize,
    pub auc_inre_matchad: f64,
    pub auc_skalad_matchad: f64,
    pub n_ok_matchad: usize,
}

pub fn per_individ_prob_diag2370(
    per_individ: &HashMap<u64, Vec<&ProbPunkt>>,
    fro: u64,
) -> ProbDiag2370 {
    let mut ai_s = 0.0;
    let mut ar_s = 0.0;
    let mut ask_s = 0.0;
    let mut asl_s = 0.0;
    let mut n_ok = 0usize;

    let mut mai_s = 0.0;
    let mut mask_s = 0.0;
    let mut n_ok_m = 0usize;

    let mut nycklar: Vec<u64> = per_individ.keys().copied().collect();
    nycklar.sort_unstable();
    for uid in nycklar {
        let pts = &per_individ[&uid];
        if pts.len() >= 20 {
            let pos = pts.iter().filter(|p| p.label).count();
            let neg = pts.len() - pos;
            if pos >= 5 && neg >= 5 {
                let z_pts: Vec<([f64; Z_B2], bool)> = pts.iter().map(|p| (p.z, p.label)).collect();
                let s_pts: Vec<([f64; S_B2], bool)> = pts.iter().map(|p| (p.s_ra, p.label)).collect();
                let sk_pts: Vec<([f64; S_B2], bool)> = pts.iter().map(|p| (p.s_sk, p.label)).collect();
                let ai = linjar_prob_auc(&z_pts, 2.0, false, fro ^ uid);
                let ar = linjar_prob_auc(&s_pts, 10.0, false, fro ^ uid);
                let ask = linjar_prob_auc(&sk_pts, 10.0, false, fro ^ uid);
                let asl = linjar_prob_auc(&z_pts, 2.0, true, fro ^ uid);
                if ai.is_finite() && ar.is_finite() && ask.is_finite() && asl.is_finite() {
                    ai_s += ai;
                    ar_s += ar;
                    ask_s += ask;
                    asl_s += asl;
                    n_ok += 1;
                }
            }
        }
        // Matchad lukthistoria: både pos och neg har nylig lukt 1..=5 steg tidigare
        let m_pts: Vec<&&ProbPunkt> = pts.iter().filter(|p| p.matchad_lukt).collect();
        if m_pts.len() >= 20 {
            let pos = m_pts.iter().filter(|p| p.label).count();
            let neg = m_pts.len() - pos;
            if pos >= 5 && neg >= 5 {
                let z_pts: Vec<([f64; Z_B2], bool)> = m_pts.iter().map(|p| (p.z, p.label)).collect();
                let sk_pts: Vec<([f64; S_B2], bool)> = m_pts.iter().map(|p| (p.s_sk, p.label)).collect();
                let mai = linjar_prob_auc(&z_pts, 2.0, false, fro ^ uid ^ 0x55);
                let mask = linjar_prob_auc(&sk_pts, 10.0, false, fro ^ uid ^ 0x55);
                if mai.is_finite() && mask.is_finite() {
                    mai_s += mai;
                    mask_s += mask;
                    n_ok_m += 1;
                }
            }
        }
    }
    ProbDiag2370 {
        auc_inre: if n_ok > 0 { ai_s / n_ok as f64 } else { f64::NAN },
        auc_ra: if n_ok > 0 { ar_s / n_ok as f64 } else { f64::NAN },
        auc_skalad: if n_ok > 0 { ask_s / n_ok as f64 } else { f64::NAN },
        auc_slump: if n_ok > 0 { asl_s / n_ok as f64 } else { f64::NAN },
        n_ok,
        auc_inre_matchad: if n_ok_m > 0 { mai_s / n_ok_m as f64 } else { f64::NAN },
        auc_skalad_matchad: if n_ok_m > 0 { mask_s / n_ok_m as f64 } else { f64::NAN },
        n_ok_matchad: n_ok_m,
    }
}

pub const DIAG2370_ARM_NAMN: [&str; 6] = [
    "D0b_Tak_OrakelFlykt",
    "D0a_Tak_SannNarhet_JEPA",
    "D1_Rattvis_StrykLukt",
    "D2_Bred_Z20",
    "D3_Hogre_Gamma80",
    "D4_Sankande_Grind",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diag2370Utfall {
    pub fro: u64,
    pub arm_idx: usize,
    pub arm_namn: String,
    pub fokus_andel: f64,
    pub n_fokus_slut: usize,
    pub n_mot_slut: usize,
    pub n_trex_slut: usize,
    pub rov_dod_fokus_pct: f64,
    pub rov_dod_mot_pct: f64,
    pub traff_fokus: u64,
    pub traff_mot: u64,
    #[serde(default)]
    pub traff_per_1000ar_fokus: f64,
    #[serde(default)]
    pub traff_per_1000ar_mot: f64,
    #[serde(default)]
    pub trex_invandring: u64,
    #[serde(default)]
    pub p_tak_traffar: u64,
    pub intag_fokus: f64,
    pub intag_mot: f64,
    pub undvik_gap_fokus: f64,
    pub undvik_gap_mot: f64,
    pub g_lukt_rov: f64,
    pub g_lukt_harmlos: f64,
    pub g_kron: f64,
    pub fokus_krit_r2: f64,
    pub prob: ProbDiag2370,
    pub sek: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diag2370Ra {
    pub param: B2Param,
    pub inkor_ar: i64,
    pub tavling_ar: i64,
    pub utfall: Vec<Diag2370Utfall>,
}

pub fn kor_en_arm_diag2370(
    sim_bas: &SimB2,
    fro: u64,
    arm_idx: usize,
    tavling_ar: i64,
) -> Diag2370Utfall {
    let t0 = Instant::now();
    let (typ_m, typ_o) = match arm_idx {
        0 => (ByteTyp::TakOrakelFlykt, ByteTyp::MinneFalskLukt),
        1 => (ByteTyp::TakSannNarhet, ByteTyp::MinneFalskLukt),
        2 => (ByteTyp::Minne, ByteTyp::MinneFalskLukt),
        3 => (ByteTyp::MinneZ20, ByteTyp::MinneFalskLuktZ20),
        4 => (ByteTyp::MinneGamma80, ByteTyp::MinneFalskLuktGamma80),
        5 => (ByteTyp::MinneSankGrind, ByteTyp::MinneFalskLukt),
        _ => unreachable!(),
    };
    let mut sim = sim_bas.clone();
    sim.param.skymd_sikt = true;
    sim.markera_halften(fro, typ_m, typ_o);

    let total_steg = tavling_ar * STEG_PER_AR;
    let prob_start = (tavling_ar / 2) * STEG_PER_AR;
    let mut andel_sum = 0.0;
    let mut andel_n = 0usize;

    for st in 0..total_steg {
        sim.samla_prob = st >= prob_start;
        sim.steg();
        if (st + 1) % STEG_PER_AR == 0 && (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR {
            let (m, o, _) = sim.antal_levande();
            let tot = m + o;
            if tot > 0 {
                andel_sum += m as f64 / tot as f64;
                andel_n += 1;
            }
        }
    }

    let (m_slut, o_slut, tr_slut) = sim.antal_levande();
    let fokus_andel = if andel_n > 0 {
        andel_sum / andel_n as f64
    } else if m_slut + o_slut > 0 {
        m_slut as f64 / (m_slut + o_slut) as f64
    } else {
        0.0
    };

    let rov_dod_fokus_pct = if sim.dod_tot_m > 0 {
        100.0 * sim.dod_rov_m as f64 / sim.dod_tot_m as f64
    } else {
        0.0
    };
    let rov_dod_mot_pct = if sim.dod_tot_o > 0 {
        100.0 * sim.dod_rov_o as f64 / sim.dod_tot_o as f64
    } else {
        0.0
    };
    let intag_fokus = if sim.intag_n_m > 0 { sim.intag_sum_m / sim.intag_n_m as f64 } else { 0.0 };
    let intag_mot = if sim.intag_n_o > 0 { sim.intag_sum_o / sim.intag_n_o as f64 } else { 0.0 };

    let ur_m = if sim.undvik_rov_n_m > 0 { sim.undvik_rov_sum_m / sim.undvik_rov_n_m as f64 } else { 0.0 };
    let uh_m = if sim.undvik_h_n_m > 0 { sim.undvik_h_sum_m / sim.undvik_h_n_m as f64 } else { 0.0 };
    let ur_o = if sim.undvik_rov_n_o > 0 { sim.undvik_rov_sum_o / sim.undvik_rov_n_o as f64 } else { 0.0 };
    let uh_o = if sim.undvik_h_n_o > 0 { sim.undvik_h_sum_o / sim.undvik_h_n_o as f64 } else { 0.0 };

    let fokus_byten: Vec<&ByteB2> = sim.byten.iter().filter(|d| d.lev && d.markt).collect();
    let fokus_krit_r2 = medel_finita(fokus_byten.iter().filter_map(|d| {
        if let Some(ref h) = d.hj {
            Some(h.krit_r2())
        } else {
            d.hj_z20.as_ref().map(|hz| hz.krit_r2())
        }
    }));
    let g_lukt_rov = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(37, 9))));
    let g_lukt_harmlos = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(46, 9))));
    let g_kron = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(19, 9))));

    let mut per_individ: HashMap<u64, Vec<&ProbPunkt>> = HashMap::new();
    for p in &sim.prob_data {
        if p.markt {
            per_individ.entry(p.uid).or_default().push(p);
        }
    }
    let prob = per_individ_prob_diag2370(&per_individ, fro);

    Diag2370Utfall {
        fro,
        arm_idx,
        arm_namn: DIAG2370_ARM_NAMN[arm_idx].to_string(),
        fokus_andel,
        n_fokus_slut: m_slut,
        n_mot_slut: o_slut,
        n_trex_slut: tr_slut,
        rov_dod_fokus_pct,
        rov_dod_mot_pct,
        traff_fokus: sim.traff_m,
        traff_mot: sim.traff_o,
        traff_per_1000ar_fokus: if sim.intag_n_m > 0 { (sim.traff_m as f64 / sim.intag_n_m as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        traff_per_1000ar_mot: if sim.intag_n_o > 0 { (sim.traff_o as f64 / sim.intag_n_o as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        trex_invandring: sim.trex_invandring,
        p_tak_traffar: 0,
        intag_fokus,
        intag_mot,
        undvik_gap_fokus: ur_m - uh_m,
        undvik_gap_mot: ur_o - uh_o,
        g_lukt_rov,
        g_lukt_harmlos,
        g_kron,
        fokus_krit_r2,
        prob,
        sek: t0.elapsed().as_secs_f64(),
    }
}

pub fn kora_diag2370(
    fron: &[u64],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    param: B2Param,
    ra_ut: Option<&Path>,
) -> Result<String, String> {
    let t_start = Instant::now();
    let jobb_bas: Vec<u64> = fron.to_vec();
    let nt = tradar.max(1).min(jobb_bas.len().max(1));
    let res_bas: Arc<std::sync::Mutex<Result<Vec<(u64, SimB2)>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(Vec::new())));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|sc| {
        for _ in 0..nt {
            let jobb_ref = &jobb_bas;
            let idx_c = Arc::clone(&idx_atom);
            let res_c = Arc::clone(&res_bas);
            sc.spawn(move || loop {
                let k = idx_c.fetch_add(1, Ordering::Relaxed);
                if k >= jobb_ref.len() {
                    break;
                }
                let fro = jobb_ref[k];
                let r = SimB2::fran_hell_creek(fro, param).map(|mut sim| {
                    for _ in 0..(inkor_ar * STEG_PER_AR) {
                        sim.steg();
                    }
                    (fro, sim)
                });
                let mut guard = res_c.lock().unwrap();
                match (&mut *guard, r) {
                    (Ok(vec), Ok(par)) => vec.push(par),
                    (Ok(_), Err(e)) => *guard = Err(e),
                    _ => {}
                }
            });
        }
    });
    let mut bas_varldar = Arc::try_unwrap(res_bas).unwrap().into_inner().unwrap()?;
    bas_varldar.sort_by_key(|(f, _)| *f);

    let mut uppgifter: Vec<(usize, usize)> = Vec::with_capacity(bas_varldar.len() * DIAG2370_ARM_NAMN.len());
    for vi in 0..bas_varldar.len() {
        for ai in 0..DIAG2370_ARM_NAMN.len() {
            uppgifter.push((vi, ai));
        }
    }
    let totalt = uppgifter.len();
    let nt_arm = tradar.max(1).min(totalt.max(1));
    let klara = Arc::new(AtomicUsize::new(0));
    let u_idx = Arc::new(AtomicUsize::new(0));
    let utfall_arc: Arc<std::sync::Mutex<Vec<Diag2370Utfall>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(totalt)));

    std::thread::scope(|sc| {
        for _ in 0..nt_arm {
            let bas_ref = &bas_varldar;
            let upp_ref = &uppgifter;
            let u_c = Arc::clone(&u_idx);
            let k_c = Arc::clone(&klara);
            let ut_c = Arc::clone(&utfall_arc);
            sc.spawn(move || loop {
                let k = u_c.fetch_add(1, Ordering::Relaxed);
                if k >= upp_ref.len() {
                    break;
                }
                let (vi, ai) = upp_ref[k];
                let (fro, ref sim_bas) = bas_ref[vi];
                let u = kor_en_arm_diag2370(sim_bas, fro, ai, tavling_ar);
                let done = k_c.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "[Alt 2370] klara {done}/{totalt}: frö {fro} {} -> fokus={:.1}% rov(F/M)={:.1}%/{:.1}% intag(F/M)={:.3}/{:.3} g(R/H)={:.3}/{:.3} auc(in/sk/match)={:.3}/{:.3}/{:.3} (n={}/{}) ({:.1}s)",
                    u.arm_namn,
                    u.fokus_andel * 100.0,
                    u.rov_dod_fokus_pct,
                    u.rov_dod_mot_pct,
                    u.intag_fokus,
                    u.intag_mot,
                    u.g_lukt_rov,
                    u.g_lukt_harmlos,
                    u.prob.auc_inre,
                    u.prob.auc_skalad,
                    u.prob.auc_inre_matchad,
                    u.prob.n_ok,
                    u.prob.n_ok_matchad,
                    u.sek
                );
                ut_c.lock().unwrap().push(u);
            });
        }
    });

    let mut utfall = Arc::try_unwrap(utfall_arc).unwrap().into_inner().unwrap();
    utfall.sort_by_key(|u| (u.arm_idx, u.fro));
    let ra = Diag2370Ra {
        param,
        inkor_ar,
        tavling_ar,
        utfall,
    };
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = std::fs::create_dir_all(par);
        }
        let js = serde_json::to_string_pretty(&ra).map_err(|e| format!("json: {e}"))?;
        std::fs::write(p, js).map_err(|e| format!("skriv ra_ut: {e}"))?;
    }
    Ok(formatera_diag2370_rapport(&ra, t_start.elapsed().as_secs_f64()))
}

pub fn formatera_diag2370_rapport(ra: &Diag2370Ra, total_sek: f64) -> String {
    let mut out = String::new();
    out.push_str("# Alt 2370 — 5-världars mekanismdiagnos för Skede B, steg B2 (`261–265`)\n\n");
    out.push_str(&format!(
        "- Inkörning: `{}` år | Tävling: `{}` år | Körtid: `{:.1} s`\n\n",
        ra.inkor_ar, ra.tavling_ar, total_sek
    ));
    out.push_str("| Arm | n | Fokus % | Vinster (>50%) | Rovdöd Fokus/Mot % | Träff F/M | Intag F/M | Undvik gap F / M | g_lukt_r / g_lukt_h | Krit R² | AUC inre / rå / skalad (n) | Matchad inre / skalad (n) | AUC slump |\n");
    out.push_str("|---|---:|---:|---:|---|---:|---|---|---|---:|---|---|---:|\n");

    for (ai, namn) in DIAG2370_ARM_NAMN.iter().enumerate() {
        let del: Vec<&Diag2370Utfall> = ra.utfall.iter().filter(|u| u.arm_idx == ai).collect();
        if del.is_empty() {
            continue;
        }
        let n = del.len();
        let mf = medel_finita(del.iter().map(|u| u.fokus_andel)) * 100.0;
        let w = del.iter().filter(|u| u.fokus_andel > 0.5).count();
        let rf = medel_finita(del.iter().map(|u| u.rov_dod_fokus_pct));
        let rm = medel_finita(del.iter().map(|u| u.rov_dod_mot_pct));
        let tf = medel_finita(del.iter().map(|u| u.traff_fokus as f64));
        let tm = medel_finita(del.iter().map(|u| u.traff_mot as f64));
        let inf = medel_finita(del.iter().map(|u| u.intag_fokus));
        let inm = medel_finita(del.iter().map(|u| u.intag_mot));
        let gf = medel_finita(del.iter().map(|u| u.undvik_gap_fokus));
        let gm = medel_finita(del.iter().map(|u| u.undvik_gap_mot));
        let glr = medel_finita(del.iter().map(|u| u.g_lukt_rov));
        let glh = medel_finita(del.iter().map(|u| u.g_lukt_harmlos));
        let kr2 = medel_finita(del.iter().map(|u| u.fokus_krit_r2));
        let ai_m = medel_finita(del.iter().map(|u| u.prob.auc_inre));
        let ar_m = medel_finita(del.iter().map(|u| u.prob.auc_ra));
        let ask_m = medel_finita(del.iter().map(|u| u.prob.auc_skalad));
        let asl_m = medel_finita(del.iter().map(|u| u.prob.auc_slump));
        let nok = medel_finita(del.iter().map(|u| u.prob.n_ok as f64));
        let mai = medel_finita(del.iter().map(|u| u.prob.auc_inre_matchad));
        let mask = medel_finita(del.iter().map(|u| u.prob.auc_skalad_matchad));
        let nokm = medel_finita(del.iter().map(|u| u.prob.n_ok_matchad as f64));

        out.push_str(&format!(
            "| `{namn}` | {n} | {mf:.1}% | {w}/{n} | {rf:.1}% / {rm:.1}% | {tf:.0}/{tm:.0} | {inf:.3}/{inm:.3} | {gf:+.4} / {gm:+.4} | {glr:.3} / {glh:.3} | {kr2:+.3} | {ai_m:.3} / {ar_m:.3} / {ask_m:.3} ({nok:.0}) | {mai:.3} / {mask:.3} ({nokm:.0}) | {asl_m:.3} |\n"
        ));
    }
    out
}

pub const B2V2_ARM_NAMN: [&str; 8] = [
    "B2v2_Noll",
    "Bantest_Fuskflykt",
    "B2v2_SankGrind_vs_Bas",
    "B2v2_SankGrind_StrykWM_A",
    "B2v2_SankGrind_StrykWM_Spegel",
    "B2v2_Rattvis_Lukt_Sank",
    "B2v2_Rattvis_Lukt_Std",
    "B2v2_Tabell",
];

pub fn kor_en_arm_b2v2(
    sim_bas: &SimB2,
    fro: u64,
    arm_idx: usize,
    tavling_ar: i64,
) -> Diag2370Utfall {
    let t0 = Instant::now();
    let (typ_m, typ_o, fokus_m) = match arm_idx {
        0 => (ByteTyp::MinneSankGrind, ByteTyp::MinneSankGrind, true),
        1 => (ByteTyp::TakOrakelFlykt, ByteTyp::MinneFalskLukt, true),
        2 => (ByteTyp::MinneSankGrind, ByteTyp::Minne, true),
        3 => (ByteTyp::MinneSankGrind, ByteTyp::MinneSankStrykWm, true),
        4 => (ByteTyp::MinneSankStrykWm, ByteTyp::MinneSankGrind, false),
        5 => (ByteTyp::MinneSankGrind, ByteTyp::MinneSankFalskLukt, true),
        6 => (ByteTyp::Minne, ByteTyp::MinneFalskLukt, true),
        7 => (ByteTyp::MinneSankGrind, ByteTyp::Tabell, true),
        _ => unreachable!(),
    };
    let mut sim = sim_bas.clone();
    sim.param.skymd_sikt = true;
    sim.markera_halften(fro, typ_m, typ_o);

    let total_steg = tavling_ar * STEG_PER_AR;
    let prob_start = (tavling_ar / 2) * STEG_PER_AR;
    let mut andel_sum = 0.0;
    let mut andel_n = 0usize;

    for st in 0..total_steg {
        sim.samla_prob = st >= prob_start;
        sim.steg();
        if (st + 1) % STEG_PER_AR == 0 && (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR {
            let (m, o, _) = sim.antal_levande();
            let tot = m + o;
            if tot > 0 {
                let f = if fokus_m { m } else { o };
                andel_sum += f as f64 / tot as f64;
                andel_n += 1;
            }
        }
    }

    let (m_slut, o_slut, tr_slut) = sim.antal_levande();
    let (n_f, n_mot) = if fokus_m { (m_slut, o_slut) } else { (o_slut, m_slut) };
    let fokus_andel = if andel_n > 0 {
        andel_sum / andel_n as f64
    } else if m_slut + o_slut > 0 {
        n_f as f64 / (m_slut + o_slut) as f64
    } else {
        0.0
    };

    let (dt_f, dr_f, tr_f, is_f, in_f, ur_sf, ur_nf, uh_sf, uh_nf) = if fokus_m {
        (sim.dod_tot_m, sim.dod_rov_m, sim.traff_m, sim.intag_sum_m, sim.intag_n_m,
         sim.undvik_rov_sum_m, sim.undvik_rov_n_m, sim.undvik_h_sum_m, sim.undvik_h_n_m)
    } else {
        (sim.dod_tot_o, sim.dod_rov_o, sim.traff_o, sim.intag_sum_o, sim.intag_n_o,
         sim.undvik_rov_sum_o, sim.undvik_rov_n_o, sim.undvik_h_sum_o, sim.undvik_h_n_o)
    };
    let (dt_o, dr_o, tr_o, is_o, in_o, ur_so, ur_no, uh_so, uh_no) = if fokus_m {
        (sim.dod_tot_o, sim.dod_rov_o, sim.traff_o, sim.intag_sum_o, sim.intag_n_o,
         sim.undvik_rov_sum_o, sim.undvik_rov_n_o, sim.undvik_h_sum_o, sim.undvik_h_n_o)
    } else {
        (sim.dod_tot_m, sim.dod_rov_m, sim.traff_m, sim.intag_sum_m, sim.intag_n_m,
         sim.undvik_rov_sum_m, sim.undvik_rov_n_m, sim.undvik_h_sum_m, sim.undvik_h_n_m)
    };

    let rov_dod_fokus_pct = if dt_f > 0 { 100.0 * dr_f as f64 / dt_f as f64 } else { 0.0 };
    let rov_dod_mot_pct = if dt_o > 0 { 100.0 * dr_o as f64 / dt_o as f64 } else { 0.0 };
    let intag_fokus = if in_f > 0 { is_f / in_f as f64 } else { 0.0 };
    let intag_mot = if in_o > 0 { is_o / in_o as f64 } else { 0.0 };
    let gap_f = (if ur_nf > 0 { ur_sf / ur_nf as f64 } else { 0.0 })
        - (if uh_nf > 0 { uh_sf / uh_nf as f64 } else { 0.0 });
    let gap_o = (if ur_no > 0 { ur_so / ur_no as f64 } else { 0.0 })
        - (if uh_no > 0 { uh_so / uh_no as f64 } else { 0.0 });

    let fokus_byten: Vec<&ByteB2> = sim.byten.iter().filter(|d| d.lev && d.markt == fokus_m).collect();
    let fokus_krit_r2 = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.krit_r2())));
    let g_lukt_rov = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(37, 9))));
    let g_lukt_harmlos = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(46, 9))));
    let g_kron = medel_finita(fokus_byten.iter().filter_map(|d| d.hj.as_ref().map(|h| h.g_intervall(19, 9))));

    let mut per_individ: HashMap<u64, Vec<&ProbPunkt>> = HashMap::new();
    for p in &sim.prob_data {
        if p.markt == fokus_m {
            per_individ.entry(p.uid).or_default().push(p);
        }
    }
    let prob = per_individ_prob_diag2370(&per_individ, fro);

    Diag2370Utfall {
        fro,
        arm_idx,
        arm_namn: B2V2_ARM_NAMN[arm_idx].to_string(),
        fokus_andel,
        n_fokus_slut: n_f,
        n_mot_slut: n_mot,
        n_trex_slut: tr_slut,
        rov_dod_fokus_pct,
        rov_dod_mot_pct,
        traff_fokus: tr_f,
        traff_mot: tr_o,
        traff_per_1000ar_fokus: if in_f > 0 { (tr_f as f64 / in_f as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        traff_per_1000ar_mot: if in_o > 0 { (tr_o as f64 / in_o as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        trex_invandring: sim.trex_invandring,
        p_tak_traffar: 0,
        intag_fokus,
        intag_mot,
        undvik_gap_fokus: gap_f,
        undvik_gap_mot: gap_o,
        g_lukt_rov,
        g_lukt_harmlos,
        g_kron,
        fokus_krit_r2,
        prob,
        sek: t0.elapsed().as_secs_f64(),
    }
}

pub fn kora_sparb2373(
    fron: &[u64],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    param: B2Param,
    bara_kalib: bool,
    ra_ut: Option<&Path>,
) -> Result<String, String> {
    let t_start = Instant::now();
    let jobb_bas: Vec<u64> = fron.to_vec();
    let nt = tradar.max(1).min(jobb_bas.len().max(1));
    let res_bas: Arc<std::sync::Mutex<Result<Vec<(u64, SimB2)>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(Vec::new())));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|sc| {
        for _ in 0..nt {
            let jobb_ref = &jobb_bas;
            let idx_c = Arc::clone(&idx_atom);
            let res_c = Arc::clone(&res_bas);
            sc.spawn(move || loop {
                let k = idx_c.fetch_add(1, Ordering::Relaxed);
                if k >= jobb_ref.len() {
                    break;
                }
                let fro = jobb_ref[k];
                let r = SimB2::fran_hell_creek(fro, param).map(|mut sim| {
                    for _ in 0..(inkor_ar * STEG_PER_AR) {
                        sim.steg();
                    }
                    (fro, sim)
                });
                let mut guard = res_c.lock().unwrap();
                match (&mut *guard, r) {
                    (Ok(vec), Ok(par)) => vec.push(par),
                    (Ok(_), Err(e)) => *guard = Err(e),
                    _ => {}
                }
            });
        }
    });
    let mut bas_varldar = Arc::try_unwrap(res_bas).unwrap().into_inner().unwrap()?;
    bas_varldar.sort_by_key(|(f, _)| *f);

    let armar: Vec<usize> = if bara_kalib {
        vec![1, 2, 5, 6]
    } else {
        (0..B2V2_ARM_NAMN.len()).collect()
    };
    let mut uppgifter: Vec<(usize, usize)> = Vec::with_capacity(bas_varldar.len() * armar.len());
    for vi in 0..bas_varldar.len() {
        for &ai in &armar {
            uppgifter.push((vi, ai));
        }
    }
    let totalt = uppgifter.len();
    let nt_arm = tradar.max(1).min(totalt.max(1));
    let klara = Arc::new(AtomicUsize::new(0));
    let u_idx = Arc::new(AtomicUsize::new(0));
    let utfall_arc: Arc<std::sync::Mutex<Vec<Diag2370Utfall>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(totalt)));

    std::thread::scope(|sc| {
        for _ in 0..nt_arm {
            let bas_ref = &bas_varldar;
            let upp_ref = &uppgifter;
            let u_c = Arc::clone(&u_idx);
            let k_c = Arc::clone(&klara);
            let ut_c = Arc::clone(&utfall_arc);
            sc.spawn(move || loop {
                let k = u_c.fetch_add(1, Ordering::Relaxed);
                if k >= upp_ref.len() {
                    break;
                }
                let (vi, ai) = upp_ref[k];
                let (fro, ref sim_bas) = bas_ref[vi];
                let u = kor_en_arm_b2v2(sim_bas, fro, ai, tavling_ar);
                let done = k_c.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "[Alt 2373] klara {done}/{totalt}: frö {fro} {} -> fokus={:.1}% rov(F/M)={:.1}%/{:.1}% intag(F/M)={:.3}/{:.3} g(R/H)={:.3}/{:.3} ({:.1}s)",
                    u.arm_namn,
                    u.fokus_andel * 100.0,
                    u.rov_dod_fokus_pct,
                    u.rov_dod_mot_pct,
                    u.intag_fokus,
                    u.intag_mot,
                    u.g_lukt_rov,
                    u.g_lukt_harmlos,
                    u.sek
                );
                ut_c.lock().unwrap().push(u);
            });
        }
    });

    let mut utfall = Arc::try_unwrap(utfall_arc).unwrap().into_inner().unwrap();
    utfall.sort_by_key(|u| (u.arm_idx, u.fro));
    let ra = Diag2370Ra {
        param,
        inkor_ar,
        tavling_ar,
        utfall,
    };
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = std::fs::create_dir_all(par);
        }
        let js = serde_json::to_string_pretty(&ra).map_err(|e| format!("json: {e}"))?;
        std::fs::write(p, js).map_err(|e| format!("skriv ra_ut: {e}"))?;
    }
    Ok(formatera_b2v2_rapport(&ra, t_start.elapsed().as_secs_f64()))
}

pub fn formatera_b2v2_rapport(ra: &Diag2370Ra, total_sek: f64) -> String {
    let mut out = String::new();
    out.push_str("# Alt 2373 — Skede B, steg B2 v2 (skärpt spelplan + Fuskflykt + blind sänkande belöningsgrind)\n\n");
    out.push_str(&format!(
        "- Inkörning: `{}` år | Tävling: `{}` år | Körtid: `{:.1} s`\n",
        ra.inkor_ar, ra.tavling_ar, total_sek
    ));
    out.push_str(&format!(
        "- Spelplan: `vaxt_skala={:.2}`, `p_angrepp={:.2}`, `bakhall={:.2}`, `p_dod={:.2}`, `gamma_lukt={:.2}`, `beta_vind={:.2}`, `kon_mitt={:.2}`\n\n",
        ra.param.vaxt_skala, ra.param.p_angrepp_bas, ra.param.bakhall_faktor, ra.param.p_dod_vid_traff,
        ra.param.gamma_lukt, ra.param.beta_vind, ra.param.kon_mitt
    ));
    out.push_str("| Arm | n | Fokus % | Vinster (>50%) | Rovdöd F / M % | Träff/1000år F / M | Intag F / M | Undvik gap F / M | g_lukt_r / g_lukt_h | Krit R² | AUC inre / skalad | Matchad inre (n) | Slump |\n");
    out.push_str("|---|---:|---:|---:|---|---|---|---|---|---:|---|---|---:|\n");

    for (ai, namn) in B2V2_ARM_NAMN.iter().enumerate() {
        let del: Vec<&Diag2370Utfall> = ra.utfall.iter().filter(|u| u.arm_idx == ai).collect();
        if del.is_empty() {
            continue;
        }
        let n = del.len();
        let mf = medel_finita(del.iter().map(|u| u.fokus_andel)) * 100.0;
        let w = del.iter().filter(|u| u.fokus_andel > 0.5).count();
        let rf = medel_finita(del.iter().map(|u| u.rov_dod_fokus_pct));
        let rm = medel_finita(del.iter().map(|u| u.rov_dod_mot_pct));
        let tfk = medel_finita(del.iter().map(|u| u.traff_per_1000ar_fokus));
        let tmk = medel_finita(del.iter().map(|u| u.traff_per_1000ar_mot));
        let inf = medel_finita(del.iter().map(|u| u.intag_fokus));
        let inm = medel_finita(del.iter().map(|u| u.intag_mot));
        let gf = medel_finita(del.iter().map(|u| u.undvik_gap_fokus));
        let gm = medel_finita(del.iter().map(|u| u.undvik_gap_mot));
        let glr = medel_finita(del.iter().map(|u| u.g_lukt_rov));
        let glh = medel_finita(del.iter().map(|u| u.g_lukt_harmlos));
        let kr2 = medel_finita(del.iter().map(|u| u.fokus_krit_r2));
        let ai_m = medel_finita(del.iter().map(|u| u.prob.auc_inre));
        let ask_m = medel_finita(del.iter().map(|u| u.prob.auc_skalad));
        let mai = medel_finita(del.iter().map(|u| u.prob.auc_inre_matchad));
        let nokm = medel_finita(del.iter().map(|u| u.prob.n_ok_matchad as f64));
        let asl_m = medel_finita(del.iter().map(|u| u.prob.auc_slump));

        out.push_str(&format!(
            "| `{namn}` | {n} | {mf:.1}% | {w}/{n} | {rf:.1}% / {rm:.1}% | {tfk:.1} / {tmk:.1} | {inf:.3}/{inm:.3} | {gf:+.4} / {gm:+.4} | {glr:.3} / {glh:.3} | {kr2:+.3} | {ai_m:.3} / {ask_m:.3} | {mai:.3} ({nokm:.0}) | {asl_m:.3} |\n"
        ));
    }
    out
}

/// Alt 2379 (1-minuts förprov enligt Fable + 4/4 konsensus):
/// Mäter i dagens oförändrade Alt 2373-hjärna (`MinneSankGrind` mot `MinneSankFalskLukt`) på `261–265`:
/// 1. `g_lukt_rov` och `g_lukt_harmlos` uppdelat på: (a) överlevt bett <= 200 steg, (b) > 200 steg, (c) aldrig biten.
/// 2. Andelen levande djursteg efter första överlevda bettet (det fysikaliska taket för Arm A/C!).
pub fn kora_forprov2379(fron: &[u64], inkor_ar: i64, tavling_ar: i64, tradar: usize) -> Result<String, String> {
    let t0 = Instant::now();
    let param = B2Param::v2_skarp();
    let nt = tradar.max(1).min(fron.len().max(1));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    #[derive(Clone, Default, Debug)]
    struct Rad {
        fro: u64,
        and_efter_bett: f64,
        n_nylig: usize,
        g_rov_nylig: f64,
        g_h_nylig: f64,
        n_gammal: usize,
        g_rov_gammal: f64,
        g_h_gammal: f64,
        n_aldrig: usize,
        g_rov_aldrig: f64,
        g_h_aldrig: f64,
        medel_bett: f64,
    }
    let res: Arc<std::sync::Mutex<Result<Vec<Rad>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(Vec::new())));
    std::thread::scope(|sc| {
        for _ in 0..nt {
            let ic = Arc::clone(&idx_atom);
            let rc = Arc::clone(&res);
            sc.spawn(move || loop {
                let k = ic.fetch_add(1, Ordering::Relaxed);
                if k >= fron.len() { break; }
                let fro = fron[k];
                let r = SimB2::fran_hell_creek(fro, param).map(|mut sim| {
                    for _ in 0..(inkor_ar * STEG_PER_AR) { sim.steg(); }
                    sim.markera_halften(fro, ByteTyp::MinneSankGrind, ByteTyp::MinneSankFalskLukt);
                    let tot_steg = tavling_ar * STEG_PER_AR;
                    // Mät även löpande under sista 10 åren (var 150:e steg) så stickprovet på nyligt bitna blir stort!
                    let (mut sn_r, mut sn_h, mut cn) = (0.0, 0.0, 0usize);
                    let (mut sg_r, mut sg_h, mut cg) = (0.0, 0.0, 0usize);
                    let (mut sa_r, mut sa_h, mut ca) = (0.0, 0.0, 0usize);
                    let (mut b_sum, mut b_n) = (0u64, 0u64);
                    for st in 0..tot_steg {
                        sim.steg();
                        if (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR && (st + 1) % 30 == 0 {
                            for d in &sim.byten {
                                if !d.lev || !d.markt { continue; }
                                if let Some(ref hj) = d.hj {
                                    let gr = hj.g_intervall(37, 9);
                                    let gh = hj.g_intervall(46, 9);
                                    b_sum += d.antal_overlevda_bett as u64;
                                    b_n += 1;
                                    if d.antal_overlevda_bett == 0 {
                                        sa_r += gr; sa_h += gh; ca += 1;
                                    } else if d.steg_sedan_skada <= 200 {
                                        sn_r += gr; sn_h += gh; cn += 1;
                                    } else {
                                        sg_r += gr; sg_h += gh; cg += 1;
                                    }
                                }
                            }
                        }
                    }
                    Rad {
                        fro,
                        and_efter_bett: if sim.steg_tot_m > 0 { 100.0 * sim.steg_efter_bett_m as f64 / sim.steg_tot_m as f64 } else { 0.0 },
                        n_nylig: cn,
                        g_rov_nylig: if cn > 0 { sn_r / cn as f64 } else { f64::NAN },
                        g_h_nylig: if cn > 0 { sn_h / cn as f64 } else { f64::NAN },
                        n_gammal: cg,
                        g_rov_gammal: if cg > 0 { sg_r / cg as f64 } else { f64::NAN },
                        g_h_gammal: if cg > 0 { sg_h / cg as f64 } else { f64::NAN },
                        n_aldrig: ca,
                        g_rov_aldrig: if ca > 0 { sa_r / ca as f64 } else { f64::NAN },
                        g_h_aldrig: if ca > 0 { sa_h / ca as f64 } else { f64::NAN },
                        medel_bett: if b_n > 0 { b_sum as f64 / b_n as f64 } else { 0.0 },
                    }
                });
                let mut g = rc.lock().unwrap();
                match (&mut *g, r) {
                    (Ok(v), Ok(rad)) => v.push(rad),
                    (Ok(_), Err(e)) => *g = Err(e),
                    _ => {}
                }
            });
        }
    });
    let mut rader = Arc::try_unwrap(res).unwrap().into_inner().unwrap()?;
    rader.sort_by_key(|r| r.fro);
    let mut out = String::new();
    out.push_str(&format!("# Alt 2379 — 1-minuts förprov på dagens Alt 2373-hjärna ({:.1} s)\n\n", t0.elapsed().as_secs_f64()));
    out.push_str("| Frö | Steg efter 1:a bett % | Medel bett/levande | Nyligt bett <=200 (n): g_rov / g_h | Gammalt bett >200 (n): g_rov / g_h | Aldrig biten (n): g_rov / g_h |\n");
    out.push_str("|---:|---:|---:|---|---|---|\n");
    for r in &rader {
        out.push_str(&format!(
            "| {} | {:.1}% | {:.2} | ({}) {:.3} / {:.3} | ({}) {:.3} / {:.3} | ({}) {:.3} / {:.3} |\n",
            r.fro, r.and_efter_bett, r.medel_bett,
            r.n_nylig, r.g_rov_nylig, r.g_h_nylig,
            r.n_gammal, r.g_rov_gammal, r.g_h_gammal,
            r.n_aldrig, r.g_rov_aldrig, r.g_h_aldrig
        ));
    }
    let m_efter = medel_finita(rader.iter().map(|r| r.and_efter_bett));
    let m_bett = medel_finita(rader.iter().map(|r| r.medel_bett));
    let m_rn = medel_finita(rader.iter().map(|r| r.g_rov_nylig));
    let m_hn = medel_finita(rader.iter().map(|r| r.g_h_nylig));
    let m_rg = medel_finita(rader.iter().map(|r| r.g_rov_gammal));
    let m_hg = medel_finita(rader.iter().map(|r| r.g_h_gammal));
    let m_ra = medel_finita(rader.iter().map(|r| r.g_rov_aldrig));
    let m_ha = medel_finita(rader.iter().map(|r| r.g_h_aldrig));
    out.push_str(&format!(
        "| **Medel** | **{:.1}%** | **{:.2}** | **{:.3} / {:.3}** | **{:.3} / {:.3}** | **{:.3} / {:.3}** |\n",
        m_efter, m_bett, m_rn, m_hn, m_rg, m_hg, m_ra, m_ha
    ));
    Ok(out)
}

pub const DIAG2379_ARM_NAMN: [&str; 5] = [
    "Arm0_Fuskflykt",
    "ArmA_LivstidsChock_Replay",
    "ArmB_Gles_LpWM_PTak",
    "ArmC_OnlineKodare_ChockReserv",
    "ArmD_Komb_A_plus_B",
];

pub fn kor_en_arm_diag2379(
    sim_bas: &SimB2,
    fro: u64,
    arm_idx: usize,
    tavling_ar: i64,
) -> Diag2370Utfall {
    let t0 = Instant::now();
    let (typ_m, typ_o) = match arm_idx {
        0 => (ByteTyp::TakOrakelFlykt, ByteTyp::MinneFalskLukt),
        1 => (ByteTyp::ArmAChock, ByteTyp::ArmAChockFalsk),
        2 => (ByteTyp::ArmBGles, ByteTyp::ArmBGlesFalsk),
        3 => (ByteTyp::ArmCOnline, ByteTyp::ArmCOnlineFalsk),
        4 => (ByteTyp::ArmDKomb, ByteTyp::ArmDKombFalsk),
        _ => unreachable!(),
    };
    let mut sim = sim_bas.clone();
    sim.param.skymd_sikt = true;
    sim.markera_halften(fro, typ_m, typ_o);

    let total_steg = tavling_ar * STEG_PER_AR;
    let prob_start = (tavling_ar / 2) * STEG_PER_AR;
    let mut andel_sum = 0.0;
    let mut andel_n = 0usize;

    for st in 0..total_steg {
        sim.samla_prob = st >= prob_start;
        sim.steg();
        if (st + 1) % STEG_PER_AR == 0 && (st + 1) >= (tavling_ar - 10).max(1) * STEG_PER_AR {
            let (m, o, _) = sim.antal_levande();
            let tot = m + o;
            if tot > 0 {
                andel_sum += m as f64 / tot as f64;
                andel_n += 1;
            }
        }
    }

    let (m_slut, o_slut, tr_slut) = sim.antal_levande();
    let fokus_andel = if andel_n > 0 {
        andel_sum / andel_n as f64
    } else if m_slut + o_slut > 0 {
        m_slut as f64 / (m_slut + o_slut) as f64
    } else {
        0.0
    };

    let rov_dod_fokus_pct = if sim.dod_tot_m > 0 { 100.0 * sim.dod_rov_m as f64 / sim.dod_tot_m as f64 } else { 0.0 };
    let rov_dod_mot_pct = if sim.dod_tot_o > 0 { 100.0 * sim.dod_rov_o as f64 / sim.dod_tot_o as f64 } else { 0.0 };
    let intag_fokus = if sim.intag_n_m > 0 { sim.intag_sum_m / sim.intag_n_m as f64 } else { 0.0 };
    let intag_mot = if sim.intag_n_o > 0 { sim.intag_sum_o / sim.intag_n_o as f64 } else { 0.0 };
    let ur_m = if sim.undvik_rov_n_m > 0 { sim.undvik_rov_sum_m / sim.undvik_rov_n_m as f64 } else { 0.0 };
    let uh_m = if sim.undvik_h_n_m > 0 { sim.undvik_h_sum_m / sim.undvik_h_n_m as f64 } else { 0.0 };
    let ur_o = if sim.undvik_rov_n_o > 0 { sim.undvik_rov_sum_o / sim.undvik_rov_n_o as f64 } else { 0.0 };
    let uh_o = if sim.undvik_h_n_o > 0 { sim.undvik_h_sum_o / sim.undvik_h_n_o as f64 } else { 0.0 };

    let fokus_byten: Vec<&ByteB2> = sim.byten.iter().filter(|d| d.lev && d.markt).collect();
    let fokus_krit_r2 = medel_finita(fokus_byten.iter().filter_map(|d| {
        if let Some(ref h) = d.hj { Some(h.krit_r2()) }
        else if let Some(ref hg) = d.hj_gles { Some(hg.krit_r2()) }
        else { d.hj_z20.as_ref().map(|hz| hz.krit_r2()) }
    }));
    let g_lukt_rov = medel_finita(fokus_byten.iter().filter_map(|d| {
        if let Some(ref h) = d.hj { Some(h.g_intervall(37, 9)) }
        else { d.hj_gles.as_ref().map(|hg| hg.g_intervall(37, 9)) }
    }));
    let g_lukt_harmlos = medel_finita(fokus_byten.iter().filter_map(|d| {
        if let Some(ref h) = d.hj { Some(h.g_intervall(46, 9)) }
        else { d.hj_gles.as_ref().map(|hg| hg.g_intervall(46, 9)) }
    }));
    let g_kron = medel_finita(fokus_byten.iter().filter_map(|d| {
        if let Some(ref h) = d.hj { Some(h.g_intervall(19, 9)) }
        else { d.hj_gles.as_ref().map(|hg| hg.g_intervall(19, 9)) }
    }));
    let p_tak_traffar: u64 = fokus_byten.iter().map(|d| {
        d.hj.as_ref().map(|h| h.p_tak_traffar).unwrap_or(0)
            + d.hj_gles.as_ref().map(|hg| hg.p_tak_traffar).unwrap_or(0)
    }).sum();

    let mut per_individ: HashMap<u64, Vec<&ProbPunkt>> = HashMap::new();
    for p in &sim.prob_data {
        if p.markt {
            per_individ.entry(p.uid).or_default().push(p);
        }
    }
    let prob = per_individ_prob_diag2370(&per_individ, fro);

    Diag2370Utfall {
        fro,
        arm_idx,
        arm_namn: DIAG2379_ARM_NAMN[arm_idx].to_string(),
        fokus_andel,
        n_fokus_slut: m_slut,
        n_mot_slut: o_slut,
        n_trex_slut: tr_slut,
        rov_dod_fokus_pct,
        rov_dod_mot_pct,
        traff_fokus: sim.traff_m,
        traff_mot: sim.traff_o,
        traff_per_1000ar_fokus: if sim.intag_n_m > 0 { (sim.traff_m as f64 / sim.intag_n_m as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        traff_per_1000ar_mot: if sim.intag_n_o > 0 { (sim.traff_o as f64 / sim.intag_n_o as f64) * STEG_PER_AR as f64 * 1000.0 } else { 0.0 },
        trex_invandring: sim.trex_invandring,
        p_tak_traffar,
        intag_fokus,
        intag_mot,
        undvik_gap_fokus: ur_m - uh_m,
        undvik_gap_mot: ur_o - uh_o,
        g_lukt_rov,
        g_lukt_harmlos,
        g_kron,
        fokus_krit_r2,
        prob,
        sek: t0.elapsed().as_secs_f64(),
    }
}

pub fn kora_diag2379(
    fron: &[u64],
    inkor_ar: i64,
    tavling_ar: i64,
    tradar: usize,
    param: B2Param,
    ra_ut: Option<&Path>,
) -> Result<String, String> {
    let t_start = Instant::now();
    let jobb_bas: Vec<u64> = fron.to_vec();
    let nt = tradar.max(1).min(jobb_bas.len().max(1));
    let res_bas: Arc<std::sync::Mutex<Result<Vec<(u64, SimB2)>, String>>> =
        Arc::new(std::sync::Mutex::new(Ok(Vec::new())));
    let idx_atom = Arc::new(AtomicUsize::new(0));
    std::thread::scope(|sc| {
        for _ in 0..nt {
            let jobb_ref = &jobb_bas;
            let idx_c = Arc::clone(&idx_atom);
            let res_c = Arc::clone(&res_bas);
            sc.spawn(move || loop {
                let k = idx_c.fetch_add(1, Ordering::Relaxed);
                if k >= jobb_ref.len() { break; }
                let fro = jobb_ref[k];
                let r = SimB2::fran_hell_creek(fro, param).map(|mut sim| {
                    for _ in 0..(inkor_ar * STEG_PER_AR) { sim.steg(); }
                    (fro, sim)
                });
                let mut guard = res_c.lock().unwrap();
                match (&mut *guard, r) {
                    (Ok(vec), Ok(par)) => vec.push(par),
                    (Ok(_), Err(e)) => *guard = Err(e),
                    _ => {}
                }
            });
        }
    });
    let mut bas_varldar = Arc::try_unwrap(res_bas).unwrap().into_inner().unwrap()?;
    bas_varldar.sort_by_key(|(f, _)| *f);

    let mut uppgifter: Vec<(usize, usize)> = Vec::with_capacity(bas_varldar.len() * DIAG2379_ARM_NAMN.len());
    for vi in 0..bas_varldar.len() {
        for ai in 0..DIAG2379_ARM_NAMN.len() {
            uppgifter.push((vi, ai));
        }
    }
    let totalt = uppgifter.len();
    let nt_arm = tradar.max(1).min(totalt.max(1));
    let klara = Arc::new(AtomicUsize::new(0));
    let u_idx = Arc::new(AtomicUsize::new(0));
    let utfall_arc: Arc<std::sync::Mutex<Vec<Diag2370Utfall>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(totalt)));
    std::thread::scope(|sc| {
        for _ in 0..nt_arm {
            let bas_ref = &bas_varldar;
            let upp_ref = &uppgifter;
            let u_c = Arc::clone(&u_idx);
            let k_c = Arc::clone(&klara);
            let ut_c = Arc::clone(&utfall_arc);
            sc.spawn(move || loop {
                let k = u_c.fetch_add(1, Ordering::Relaxed);
                if k >= upp_ref.len() { break; }
                let (vi, ai) = upp_ref[k];
                let (fro, ref sim_bas) = bas_ref[vi];
                let u = kor_en_arm_diag2379(sim_bas, fro, ai, tavling_ar);
                let done = k_c.fetch_add(1, Ordering::Relaxed) + 1;
                eprintln!(
                    "[Alt 2379] klara {done}/{totalt}: frö {fro} {} -> fokus={:.1}% tr1k={:.1}/{:.1} g(R/H)={:.3}/{:.3} p_tak={} ({:.1}s)",
                    u.arm_namn, u.fokus_andel * 100.0, u.traff_per_1000ar_fokus, u.traff_per_1000ar_mot,
                    u.g_lukt_rov, u.g_lukt_harmlos, u.p_tak_traffar, u.sek
                );
                ut_c.lock().unwrap().push(u);
            });
        }
    });
    let mut utfall = Arc::try_unwrap(utfall_arc).unwrap().into_inner().unwrap();
    utfall.sort_by_key(|u| (u.arm_idx, u.fro));
    let ra = Diag2370Ra { param, inkor_ar, tavling_ar, utfall };
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() { let _ = std::fs::create_dir_all(par); }
        let js = serde_json::to_string_pretty(&ra).map_err(|e| format!("json: {e}"))?;
        std::fs::write(p, js).map_err(|e| format!("skriv ra_ut: {e}"))?;
    }
    Ok(formatera_diag2379_rapport(&ra, t_start.elapsed().as_secs_f64()))
}

pub fn formatera_diag2379_rapport(ra: &Diag2370Ra, total_sek: f64) -> String {
    let mut out = String::new();
    out.push_str("# Alt 2379 — 5-världars isolerad mekanismdiagnos (Livstids-chock, Gles LpWM + P-tak, Online-kodare)\n\n");
    out.push_str(&format!("- Inkörning: `{}` år | Tävling: `{}` år | Körtid: `{:.1} s`\n\n", ra.inkor_ar, ra.tavling_ar, total_sek));
    out.push_str("| Arm | n | Fokus % | Vinster (>50%) | Träff/1000år F / M (F<M) | Rovdöd F / M % | Intag F / M | Undvik gap F / M | g_rov / g_h | Krit R² | P-tak träff | Matchad AUC (n) / Skalad / Slump |\n");
    out.push_str("|---|---:|---:|---:|---|---|---|---|---|---:|---:|---|\n");
    for (ai, namn) in DIAG2379_ARM_NAMN.iter().enumerate() {
        let del: Vec<&Diag2370Utfall> = ra.utfall.iter().filter(|u| u.arm_idx == ai).collect();
        if del.is_empty() { continue; }
        let n = del.len();
        let mf = medel_finita(del.iter().map(|u| u.fokus_andel)) * 100.0;
        let w = del.iter().filter(|u| u.fokus_andel > 0.5).count();
        let tfk = medel_finita(del.iter().map(|u| u.traff_per_1000ar_fokus));
        let tmk = medel_finita(del.iter().map(|u| u.traff_per_1000ar_mot));
        let tr_l = del.iter().filter(|u| u.traff_per_1000ar_fokus < u.traff_per_1000ar_mot).count();
        let rf = medel_finita(del.iter().map(|u| u.rov_dod_fokus_pct));
        let rm = medel_finita(del.iter().map(|u| u.rov_dod_mot_pct));
        let inf = medel_finita(del.iter().map(|u| u.intag_fokus));
        let inm = medel_finita(del.iter().map(|u| u.intag_mot));
        let gf = medel_finita(del.iter().map(|u| u.undvik_gap_fokus));
        let gm = medel_finita(del.iter().map(|u| u.undvik_gap_mot));
        let glr = medel_finita(del.iter().map(|u| u.g_lukt_rov));
        let glh = medel_finita(del.iter().map(|u| u.g_lukt_harmlos));
        let kr2 = medel_finita(del.iter().map(|u| u.fokus_krit_r2));
        let pt = medel_finita(del.iter().map(|u| u.p_tak_traffar as f64));
        let mai = medel_finita(del.iter().map(|u| u.prob.auc_inre_matchad));
        let nok = medel_finita(del.iter().map(|u| u.prob.n_ok_matchad as f64));
        let ask = medel_finita(del.iter().map(|u| u.prob.auc_skalad));
        let asl = medel_finita(del.iter().map(|u| u.prob.auc_slump));
        out.push_str(&format!(
            "| `{namn}` | {n} | {mf:.1}% | {w}/{n} | {tfk:.1} / {tmk:.1} ({tr_l}/{n}) | {rf:.1}% / {rm:.1}% | {inf:.3}/{inm:.3} | {gf:+.4}/{gm:+.4} | {glr:.3}/{glh:.3} | {kr2:+.3} | {pt:.0} | {mai:.3} ({nok:.0}) / {ask:.3} / {asl:.3} |\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vind_asymmetri_medvind_mot_motvind() {
        let mut param = B2Param::default();
        param.start_trex = 0;
        let mut sim = SimB2::syntetisk(12, 12, 42, param);
        sim.trex.clear();
        sim.harmlos.clear();
        sim.lukt_rov.fill(0.0);
        sim.vind_dir = 2; // DIR[3] = (+1, 0), rakt österut
        // Placera en ensam T. rex i (5, 5)
        let gen = GenTrex::slump(&mut Rng::new(7));
        sim.trex.push(TrexB2 {
            lev: true,
            i: 5,
            j: 5,
            e: 150.0,
            age: 10,
            uid: 999,
            hj: HjTrex::fran_gen(&gen),
            gen,
            egen_rng: Rng::new(999),
            e_fore_steg: 150.0,
            vila: 0,
        });
        // Steg 1: T. rex utsöndrar 1.0 i (5,5)
        let c55 = sim.idx(5, 5);
        sim.lukt_rov[c55] = 1.0;
        // Manuell ren advektion utan vindskifte för att pröva exakt fysik
        let (dx, dy) = DIR[sim.vind_dir + 1];
        assert_eq!((dx, dy), (1, 0));
        let medvind = lukt_steg_scalar(GAMMA_LUKT, BETA_VIND, 0.0, 1.0, 0.0);
        let motvind = lukt_steg_scalar(GAMMA_LUKT, BETA_VIND, 0.0, 0.0, 0.0);
        assert!((medvind - GAMMA_LUKT * BETA_VIND).abs() < 1e-12);
        assert!(medvind > 0.35);
        assert_eq!(motvind, 0.0);
    }

    #[test]
    fn dold_trex_bakom_tat_vegetation_ger_noll_sikt_men_positiv_lukt() {
        let param = B2Param::default();
        let mut sim = SimB2::syntetisk(10, 10, 11, param);
        let c_granne = sim.idx(6, 5);
        let c_egen = sim.idx(5, 5);
        sim.kron[c_granne] = 0.80; // tät skog >= 0.50
        sim.lukt_rov[c_egen] = 0.45;
        let antal_byte = vec![0u16; 100];
        let mut antal_trex = vec![0u16; 100];
        antal_trex[c_granne] = 1;

        let s_skymd = sim.bygg_byte_sinnen(5, 5, 120.0, &antal_byte, &antal_trex);
        // Granne 3 = (+1, 0) = (6, 5)
        assert_eq!(s_skymd[28 + 3], 0.0, "T. rex bakom tät vegetation måste ha 0 sikt");
        assert!(s_skymd[37] > 0.4, "lukten måste ändå nå bytet");

        // Röd-till-grön: stäng av skymd sikt -> nu syns T. rex direkt!
        sim.param.skymd_sikt = false;
        let s_fri = sim.bygg_byte_sinnen(5, 5, 120.0, &antal_byte, &antal_trex);
        assert_eq!(s_fri[28 + 3], 1.0);
    }

    #[test]
    fn latent_minnesspar_bevarar_signal_efter_lukt_upphort() {
        let mut h = 0.0;
        h = minnes_spar_scalar(BETA_MINNE_AKTIV, h, 2.0);
        assert!(h > 0.45);
        // Efter att lukten fallit till 0 i 3 steg är minnesspåret fortfarande strikt positivt
        for _ in 0..3 {
            h = minnes_spar_scalar(BETA_MINNE_AKTIV, h, 0.0);
        }
        assert!(h > 0.20, "minnesspåret ska bära kvar signalen efter 3 steg utan lukt");
    }

    #[test]
    fn alt1885b_linjar_prob_gron_pa_signal_och_rod_pa_slump() {
        let mut g = Rng::new(1885);
        let mut data: Vec<([f64; Z_B2], bool)> = Vec::new();
        for k in 0..400 {
            let label = k % 2 == 0;
            let mut z = [0.0; Z_B2];
            for r in 0..Z_B2 {
                z[r] = 0.3 * gauss(&mut g);
            }
            if label {
                z[0] += 1.2;
                z[1] -= 0.8;
            }
            data.push((z, label));
        }
        let auc_ok = linjar_prob_auc(&data, 1.0, false, 7);
        let auc_slump = linjar_prob_auc(&data, 1.0, true, 7);
        assert!(auc_ok > 0.90, "proben måste hitta klar signal, fick {auc_ok}");
        assert!(
            (auc_slump - 0.5).abs() < 0.12,
            "slumpetikett måste ge ~0.50 (röd-till-grön), fick {auc_slump}"
        );
    }

    #[test]
    fn per_individ_prob_hittar_signal_aven_med_motsatta_slumpkodare() {
        let mut g = Rng::new(2361);
        let mut punkter = Vec::new();
        for uid in [1u64, 2u64] {
            let tecken = if uid == 1 { 1.0 } else { -1.0 };
            for k in 0..80 {
                let label = k % 2 == 0;
                let mut z = [0.0; Z_B2];
                for r in 0..Z_B2 {
                    z[r] = 0.25 * gauss(&mut g);
                }
                if label {
                    z[0] += tecken * 1.4;
                }
                let mut s_ra = [0.0; S_B2];
                for j in 0..S_B2 {
                    s_ra[j] = 0.25 * gauss(&mut g);
                }
                punkter.push(ProbPunkt {
                    uid,
                    markt: true,
                    z,
                    s_ra,
                    s_sk: s_ra,
                    label,
                    matchad_lukt: true,
                });
            }
        }
        // Röd-till-grön: hopslagen prob över båda individerna faller till ~0.50 (motsatta tecken)
        let hopslagen: Vec<([f64; Z_B2], bool)> = punkter.iter().map(|p| (p.z, p.label)).collect();
        let auc_hop = linjar_prob_auc(&hopslagen, 2.0, false, 9);
        assert!(auc_hop < 0.65, "hopslagen prob ska missa motsatta kodare, fick {auc_hop}");

        // Per-individ prob hittar stark signal (>0.90) och slår råa sinnen (~0.50) samt slump (~0.50)
        let mut per_id: HashMap<u64, Vec<&ProbPunkt>> = HashMap::new();
        for p in &punkter {
            per_id.entry(p.uid).or_default().push(p);
        }
        let (ai, ar, asl) = per_individ_prob_auc(&per_id, 9);
        assert!(ai > 0.90, "per-individ inre AUC måste vara hög, fick {ai}");
        assert!((ar - 0.5).abs() < 0.15, "råa brussinnen ska ge ~0.50, fick {ar}");
        assert!((asl - 0.5).abs() < 0.15, "slumpetikett ska ge ~0.50, fick {asl}");
    }

    #[test]
    fn td0_kritiker_propagerar_smarta_bakat_till_forvarning() {
        let mut g = Rng::new(23610);
        let gen = GenB2::slump(&mut g);
        let mut hj = HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::Bas, false));
        // Enkel cyklisk kedja: lugn (r=0) -> forvarning (r=0) -> smarta (r=-20)
        let mut s_lugn = [0.0; S_B2];
        let mut s_varn = [0.0; S_B2];
        let mut s_smarta = [0.0; S_B2];
        s_lugn[0] = 1.0;
        s_varn[37] = 1.0;
        s_smarta[28] = 1.0;
        let mut h_lugn = [0.0; Z_B2];
        let mut h_varn = [0.0; Z_B2];
        let mut h_smarta = [0.0; Z_B2];
        for _ in 0..80 {
            // Kom in i lugn ruta (r = 0)
            hj.ge_beloning(0.0);
            let (_, hl) = hj.valj_lott(&s_lugn, &mut g);
            h_lugn = hl;
            // Kom in i förvarningsruta (r = 0, ingen smärta ännu!)
            hj.ge_beloning(0.0);
            let (_, hv) = hj.valj_lott(&s_varn, &mut g);
            h_varn = hv;
            // Kom in i smärtruta (r = -20)
            hj.ge_beloning(-20.0);
            let (_, hs) = hj.valj_lott(&s_smarta, &mut g);
            h_smarta = hs;
        }
        let v_lugn = hj.varde(&h_lugn);
        let v_varn = hj.varde(&h_varn);
        let v_smarta = hj.varde(&h_smarta);
        assert!(
            v_smarta < v_varn && v_varn < v_lugn - 1.5,
            "TD(0)-kritikern måste ge v_smarta < v_varn < v_lugn: smarta={v_smarta:.2}, varn={v_varn:.2}, lugn={v_lugn:.2}"
        );
    }

    #[test]
    fn alla_8_armar_och_hopslagning_utan_nan_eller_panik() {
        let param = B2Param::default();
        let sim_bas = SimB2::syntetisk(16, 16, 261, param);
        let mut utfall = Vec::new();
        for ai in 0..B2_ARM_NAMN.len() {
            let u = kor_en_arm_b2(&sim_bas, 261, ai, 1);
            assert!(u.fokus_andel.is_finite());
            assert!(u.fokus_andel >= 0.0 && u.fokus_andel <= 1.0);
            assert!(u.g_lukt_rov.is_finite());
            utfall.push(u);
        }
        let ra = B2KorningRa {
            param,
            inkor_ar: 0,
            tavling_ar: 1,
            utfall,
        };
        let md = formatera_b2_rapport(&ra, 1.0);
        assert!(md.contains("B2_Noll"));
        assert!(md.contains("B2_Utan_SkymdSikt"));
    }

    #[test]
    fn alt2370_sankande_grind_dampar_ofarlig_kanal_men_behaller_beloningskanal() {
        let mut g = Rng::new(23701);
        let gen = GenB2::slump(&mut g);
        let mut hj = HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::MinneSankGrind, false));
        // Två lika förutsägbara (långsamt oscillerande) kanaler: kanal 37 styr belöningen, kanal 46 är helt ofarlig
        for t in 0..240 {
            let mut s = [0.0; S_B2];
            let sig_farlig = ((t as f64) * 0.15).sin();
            let sig_ofarlig = ((t as f64) * 0.23 + 1.1).cos();
            s[0] = ((t as f64) * 0.11).sin();
            s[37] = sig_farlig;
            s[46] = sig_ofarlig;
            let _ = hj.valj_lott(&s, &mut g);
            // Belöningen samvarierar starkt med kanal 37, men har 0 samvariation med kanal 46
            hj.ge_beloning(10.0 * sig_farlig);
        }
        assert!(
            hj.kanal_vikt[37] > hj.kanal_vikt[46] + 0.30,
            "sänkande belöningsgrind måste behålla belöningskanal ({:.3}) högre än ofarlig kanal ({:.3})",
            hj.kanal_vikt[37],
            hj.kanal_vikt[46]
        );
        assert!(
            hj.kanal_vikt[46] < 0.45,
            "ofarlig kanal måste dämpas tydligt nedåt mot golvet 0.25, fick {:.3}",
            hj.kanal_vikt[46]
        );
    }

    #[test]
    fn alt2370_alla_6_diagnosarmar_och_per_grupp_matt_utan_nan() {
        let param = B2Param::default();
        let sim_bas = SimB2::syntetisk(16, 16, 261, param);
        let mut utfall = Vec::new();
        for ai in 0..DIAG2370_ARM_NAMN.len() {
            let u = kor_en_arm_diag2370(&sim_bas, 261, ai, 1);
            assert!(u.fokus_andel.is_finite());
            assert!(u.rov_dod_fokus_pct.is_finite() && u.rov_dod_mot_pct.is_finite());
            assert!(u.intag_fokus.is_finite() && u.intag_mot.is_finite());
            utfall.push(u);
        }
        let ra = Diag2370Ra {
            param,
            inkor_ar: 0,
            tavling_ar: 1,
            utfall,
        };
        let md = formatera_diag2370_rapport(&ra, 1.0);
        assert!(md.contains("D0b_Tak_OrakelFlykt"));
        assert!(md.contains("D4_Sankande_Grind"));
    }

    #[test]
    fn alt2373_arlig_fuskflykt_och_alla_8_b2v2_armar() {
        // 1. Röd-till-grön: valj_lott_med_fuskflykt får ALDRIG välja lukt > 0.10 om säkert steg finns
        let mut g = Rng::new(2373);
        let gen = GenB2::slump(&mut g);
        let mut hj = HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::TakOrakelFlykt, false));
        let s = [0.5; S_B2];
        let lukt9 = [0.80, 0.80, 0.80, 0.02, 0.80, 0.80, 0.80, 0.80, 0.80]; // endast steg 3 är säkert
        for _ in 0..20 {
            let (a, _) = hj.valj_lott_med_fuskflykt(&s, &lukt9, &mut g);
            assert_eq!(a, 3, "ärlig Fuskflykt måste välja enda säkra steget (3), fick {a}");
        }
        // 2. Alla 8 B2v2-armar kör utan NaN/panik och rapporterar träff/1000år
        let param = B2Param::v2_skarp();
        let sim_bas = SimB2::syntetisk(16, 16, 261, param);
        let mut utfall = Vec::new();
        for ai in 0..B2V2_ARM_NAMN.len() {
            let u = kor_en_arm_b2v2(&sim_bas, 261, ai, 1);
            assert!(u.fokus_andel.is_finite());
            assert!(u.traff_per_1000ar_fokus.is_finite() && u.traff_per_1000ar_mot.is_finite());
            utfall.push(u);
        }
        let ra = Diag2370Ra { param, inkor_ar: 0, tavling_ar: 1, utfall };
        let md = formatera_b2v2_rapport(&ra, 1.0);
        assert!(md.contains("Bantest_Fuskflykt"));
        assert!(md.contains("Träff/1000år"));
    }

    #[test]
    fn alt2379_sym_p_tak_bevarar_psd_dar_diagonalklipp_bryter() {
        // Fables exakta motexempel ur Alt 2377: [[2000, 1500], [1500, 2000]] (PSD, min egenvärde = +500).
        // 1. Röd kontroll: rent diagonalklipp till 1000 ger [[1000, 1500], [1500, 1000]] -> v=[1,-1] ger vᵀPv = -1000 < 0!
        let p_diag_klippt = [[1000.0, 1500.0], [1500.0, 1000.0]];
        let q_rod = p_diag_klippt[0][0] - p_diag_klippt[0][1] - p_diag_klippt[1][0] + p_diag_klippt[1][1];
        assert!(q_rod < 0.0, "diagonalklipp måste bryta PSD, fick q={q_rod}");

        // 2. Grön kontroll: Lean 4-bevisat sym_kongruens_p_tak ger symmetrisk PSD med diagonal <= 1000!
        let mut p_sym = [[2000.0, 1500.0], [1500.0, 2000.0]];
        let traff = sym_kongruens_p_tak(&mut p_sym, 1000.0);
        assert_eq!(traff, 1);
        assert!(p_sym[0][0] <= 1000.0 + 1e-9 && p_sym[1][1] <= 1000.0 + 1e-9);
        assert!((p_sym[0][1] - p_sym[1][0]).abs() < 1e-12);
        let q_gron = p_sym[0][0] - p_sym[0][1] - p_sym[1][0] + p_sym[1][1];
        assert!(q_gron > 0.0, "symmetrisk kongruensskalning måste bevara PSD, fick q={q_gron}");
    }

    #[test]
    fn alt2379_gles_relu_bevarar_exakta_nollor_och_top_k() {
        let mut g = Rng::new(2379);
        let gen = GenB2Gles::slump(&mut g);
        let hj = HjB2Gles::fran_gen(&gen, false);
        let s_in: [f64; S_B2] = std::array::from_fn(|j| ((j as f64) * 0.37).sin());
        let (_, z_nu) = hj.koda_gles_nu(&s_in);
        let antal_pos = z_nu.iter().filter(|&&x| x > 0.0).count();
        assert!(z_nu.iter().all(|&x| x >= 0.0), "alla glesa axlar måste vara >= 0");
        assert!(antal_pos <= GLES_TOP_K, "högst {GLES_TOP_K} nollskilda nu-axlar, fick {antal_pos}");
        assert!(z_nu.iter().any(|&x| x == 0.0), "exakta nollor måste bevaras efter positiv RMS-skalning");
    }

    #[test]
    fn alt2379_livstidschock_minns_efter_500_lugna_steg() {
        let mut g = Rng::new(23791);
        let gen = GenB2::slump(&mut g);
        let mut hj = HjB2::fran_gen(&gen, HjB2Config::ny(ByteTyp::ArmAChock, false));
        // 80 lugna steg med varierad matsignal (kanal 0) men noll på kanal 37
        for st in 0..80 {
            let mut s = [0.0; S_B2];
            s[0] = if st % 2 == 0 { 1.0 } else { -1.0 };
            s[37] = 0.0;
            let _ = hj.valj_lott(&s, &mut g);
            hj.ge_beloning(s[0] * 0.5);
        }
        // ETT enda kraftigt överlevt bett föregånget av spik i kanal 37!
        let mut s_chock = [0.0; S_B2];
        s_chock[37] = 4.0;
        let _ = hj.valj_lott(&s_chock, &mut g);
        hj.ge_beloning(-48.0);
        assert!(hj.chock_golv[37] > 0.004, "ett enda överlevt bett måste lyfta chock_golv[37], fick {}", hj.chock_golv[37]);
        let golv_efter_chock = hj.chock_golv[37];
        // 500 helt lugna steg efter chocken: fryst livstids-chockgolv får ALDRIG blekna!
        for st in 0..500 {
            let mut s = [0.0; S_B2];
            s[0] = if st % 2 == 0 { 1.0 } else { -1.0 };
            let _ = hj.valj_lott(&s, &mut g);
            hj.ge_beloning(s[0] * 0.5);
        }
        assert!(hj.chock_golv[37] >= golv_efter_chock - 1e-12, "fryst chockgolv får aldrig sjunka efter 500 lugna steg");
        assert!(!hj.chock_minne.is_empty(), "överlevt bett måste sparas i livstids-chockminnet");
    }

    #[test]
    fn alt2379_alla_5_diagnosarmar_utan_nan() {
        let param = B2Param::v2_skarp();
        let sim_bas = SimB2::syntetisk(16, 16, 261, param);
        let mut utfall = Vec::new();
        for ai in 0..DIAG2379_ARM_NAMN.len() {
            let u = kor_en_arm_diag2379(&sim_bas, 261, ai, 1);
            assert!(u.fokus_andel.is_finite());
            assert!(u.traff_per_1000ar_fokus.is_finite() && u.traff_per_1000ar_mot.is_finite());
            utfall.push(u);
        }
        let ra = Diag2370Ra { param, inkor_ar: 0, tavling_ar: 1, utfall };
        let md = formatera_diag2379_rapport(&ra, 1.0);
        assert!(md.contains("ArmA_LivstidsChock_Replay"));
        assert!(md.contains("ArmB_Gles_LpWM_PTak"));
        assert!(md.contains("ArmC_OnlineKodare_ChockReserv"));
        assert!(md.contains("ArmD_Komb_A_plus_B"));
    }
}
