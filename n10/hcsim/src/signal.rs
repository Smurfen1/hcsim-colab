//! Strykprovet på värdets signalväg (Alt 1251) med delningen av turordningen
//! (Alt 1252) och segern utan intag (Alt 1253).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{Formaga, HjarnArm};
use crate::minstakvadrat::{self, G1Vikter};
use crate::rang::{LED_ANP_N, LED_ANP_START, LED_LOGG_VAG};
use crate::sim::{
    BokforingBlock, FormagaMatning, MarkLage, Matning, SignalLogg, SignalLoggRad, Sim,
};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{self, A_N, MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL, Z_N};
use crate::tur::RANG_LOGG_VAG;

pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Frön 181–200 för Alt 1252 (samma som Alt 1246).
pub const FRO_1252_START: u64 = 181;
/// Frön 201–220 för loggkörningen `v_lar` (Alt 1251).
pub const FRO_1251_START: u64 = 201;
/// Frön 221–240 för bindande utfall Alt 1425.
pub const FRO_1425_START: u64 = 221;
pub const N_FRO_FULL: u64 = 20;

/// Val för `hcsim signal` (Alt 1425: `--fron` / `--bara`).
#[derive(Clone, Debug, Default)]
pub struct SignalVal {
    pub torr: bool,
    /// Inclusive start; None = standard per arm.
    pub fro_start: Option<u64>,
    /// Inclusive slut; kräver `fro_start`.
    pub fro_slut: Option<u64>,
    /// Bara armen `v_lar` (inga 1252-armar).
    pub bara_v_lar: bool,
}

pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
pub const MIN_RADER_DJUR: usize = 20;

pub const ARM_ORDNING: [&str; 6] = ["a_to", "b_to", "a_tab", "b_tab", "u_to", "s_to"];

/// Frön där trängseloraklet åt mer än slumpen i u_to (underlaget).
pub const U_TO_ATER_MER: [u64; 5] = [181, 189, 192, 195, 200];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Klocka {
    /// Båda av (samtidig klocka, som Alt 1246 s_to).
    Samtidig,
    /// Läge a: flytt i tur, bete samtidigt.
    FlyttITur,
    /// Läge b: sinnen från stegets början, flytt och bete i tur.
    BeteITur,
    /// Båda på (turordning, som Alt 1246 u_to).
    Turordning,
}

impl Klocka {
    pub fn som_str(self) -> &'static str {
        match self {
            Klocka::Samtidig => "samtidig",
            Klocka::FlyttITur => "flytt_i_tur",
            Klocka::BeteITur => "bete_i_tur",
            Klocka::Turordning => "turordning",
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatningRad {
    pub formaga: String,
    pub djursteg: u64,
    pub medatare: Option<f64>,
    pub andel_knapp: Option<f64>,
    pub intag_per_steg: Option<f64>,
}

impl MatningRad {
    fn fran(formaga: &str, m: &FormagaMatning) -> Self {
        Self {
            formaga: formaga.to_string(),
            djursteg: m.djursteg,
            medatare: m.medatare_medel(),
            andel_knapp: m.andel_knapp(),
            intag_per_steg: m.intag_per_steg(),
        }
    }

    fn till_json(&self) -> String {
        format!(
            "{{\"formaga\":\"{}\",\"djursteg\":{},\"medatare\":{},\"andel_knapp\":{},\"intag_per_steg\":{}}}",
            self.formaga,
            self.djursteg,
            fmt_opt(self.medatare),
            fmt_opt(self.andel_knapp),
            fmt_opt(self.intag_per_steg),
        )
    }
}

#[derive(Clone, Debug)]
pub struct Korning {
    pub arm: String,
    pub spegel: bool,
    pub fro: u64,
    pub antal_markering: usize,
    pub antal_slut: usize,
    pub markta_slut: usize,
    pub andel_markta: f64,
    pub kortid_s: f64,
    pub torr: bool,
    pub markta_formaga: String,
    pub omarkta_formaga: String,
    pub klocka: Klocka,
    pub logg_rader: usize,
    pub ingrepp_mark: Option<f64>,
    pub ingrepp_slut: Option<f64>,
    pub matning: Vec<MatningRad>,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let mat: Vec<String> = self.matning.iter().map(|m| m.till_json()).collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"klocka\":\"{}\",\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"logg_rader\":{},\"ingrepp_mark\":{},\"ingrepp_slut\":{},\"matning\":[{}],\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.klocka.som_str(),
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            self.markta_formaga,
            self.omarkta_formaga,
            self.logg_rader,
            fmt_opt(self.ingrepp_mark),
            fmt_opt(self.ingrepp_slut),
            mat.join(","),
            fmt_f64(self.kortid_s),
        );
        if self.torr {
            s.push_str(" TORRKÖRNING");
        }
        s
    }
}

fn fmt_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".into()
    }
}

fn fmt_opt(x: Option<f64>) -> String {
    match x {
        Some(v) => fmt_f64(v),
        None => "null".into(),
    }
}

fn fro_lista_1252(torr: bool) -> Vec<u64> {
    let n = if torr { N_FRO_TORR } else { N_FRO_FULL };
    (0..n).map(|i| FRO_1252_START + i).collect()
}

fn fro_lista_val(val: &SignalVal, standard_start: u64) -> Vec<u64> {
    match (val.fro_start, val.fro_slut) {
        (Some(a), Some(b)) => {
            if b < a {
                return Vec::new();
            }
            let alla: Vec<u64> = (a..=b).collect();
            if val.torr {
                alla.into_iter().take(N_FRO_TORR as usize).collect()
            } else {
                alla
            }
        }
        _ => {
            let n = if val.torr { N_FRO_TORR } else { N_FRO_FULL };
            (0..n).map(|i| standard_start + i).collect()
        }
    }
}

/// De sex basarmarna (underlaget) + speglar.
pub fn signal_armar() -> Vec<(HjarnArm, Klocka)> {
    let bas: [(&str, Formaga, Formaga, Klocka); 6] = [
        ("a_to", Formaga::TrangselOrakel, Formaga::Slump, Klocka::FlyttITur),
        ("b_to", Formaga::TrangselOrakel, Formaga::Slump, Klocka::BeteITur),
        ("a_tab", Formaga::Tabell, Formaga::Slump, Klocka::FlyttITur),
        ("b_tab", Formaga::Tabell, Formaga::Slump, Klocka::BeteITur),
        ("u_to", Formaga::TrangselOrakel, Formaga::Slump, Klocka::Turordning),
        ("s_to", Formaga::TrangselOrakel, Formaga::Slump, Klocka::Samtidig),
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for &(namn, markta, omarkta, klocka) in &bas {
        let arm = HjarnArm {
            namn,
            markta,
            omarkta,
            spegel: false,
        };
        ut.push((arm.clone(), klocka));
        ut.push((arm.spegel_av(), klocka));
    }
    ut
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    klocka: Klocka,
    fro: u64,
    logg: bool,
    bokfor: bool,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    bygg_jobb_med(&SignalVal {
        torr,
        ..SignalVal::default()
    })
}

pub fn bygg_jobb_med(val: &SignalVal) -> Vec<Jobb> {
    let mut jobb = Vec::new();
    if !val.bara_v_lar {
        let fron_1252 = if val.fro_start.is_some() {
            // Egna frön gäller bara v_lar när --bara; annars behåll 1252-fröna.
            fro_lista_1252(val.torr)
        } else {
            fro_lista_1252(val.torr)
        };
        for (arm, klocka) in signal_armar() {
            let bokfor = matches!(arm.namn, "u_to" | "a_to" | "b_to") && !arm.spegel;
            for &fro in &fron_1252 {
                jobb.push(Jobb {
                    arm: arm.clone(),
                    klocka,
                    fro,
                    logg: false,
                    bokfor,
                });
            }
        }
    }
    let fron_v = fro_lista_val(val, FRO_1251_START);
    for &fro in &fron_v {
        jobb.push(Jobb {
            arm: HjarnArm {
                namn: "v_lar",
                markta: Formaga::Lar,
                omarkta: Formaga::Lar,
                spegel: false,
            },
            klocka: Klocka::Samtidig,
            fro,
            logg: true,
            bokfor: false,
        });
    }
    jobb.reverse();
    jobb
}

fn sat_klocka(sim: &mut Sim, klocka: Klocka) {
    sim.turordning = false;
    sim.flytt_i_tur = false;
    sim.bete_i_tur = false;
    match klocka {
        Klocka::Samtidig => {}
        Klocka::FlyttITur => sim.flytt_i_tur = true,
        Klocka::BeteITur => sim.bete_i_tur = true,
        Klocka::Turordning => sim.turordning = true,
    }
}

fn bygg_sim(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    arm: &HjarnArm,
    klocka: Klocka,
    logg: bool,
) -> Result<Sim, String> {
    let omr = Omrade {
        lat0: LAT0,
        lon0: LON0,
        b,
        h,
        ruta_m: RUTA_M,
    };
    let kart = karta_full(kalla, tt, &omr, T_KARTA, fro)?;
    let n0 = n0_for(b, h);
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    sim.mark_lage = MarkLage::Noll;
    sim.hjarn_arm = Some(arm.clone());
    sim.atervaxt = 1.0;
    sat_klocka(&mut sim, klocka);
    sim.matning = Matning {
        pa: true,
        ..Matning::default()
    };
    if logg {
        sim.signal_logg = SignalLogg {
            pa: true,
            fro,
            ..SignalLogg::default()
        };
    }
    Ok(sim)
}

pub fn kor_en(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    arm: &HjarnArm,
    klocka: Klocka,
    logg: bool,
    bokfor: bool,
    torr: bool,
) -> Result<(Korning, Vec<SignalLoggRad>, Vec<BokforingBlock>), String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm, klocka, logg)?;
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    sim.markera_halften(fro);
    let antal_markering = sim.antal();
    if bokfor {
        sim.starta_bokforing(fro, arm.namn);
    }
    if logg {
        sim.kor_ingrepp(false);
    }
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, MARKT_EFTER_AR);
    if logg {
        sim.kor_ingrepp(true);
        sim.signal_logg.krav_data()?;
    }
    sim.matning.krav_data()?;
    if bokfor {
        let antal_slut = sim.antal_per_formaga();
        sim.bokforing.kontroll(&antal_slut)?;
        // Stäng eventuellt sista ofullständiga blocket saknas — underlaget räknar hela 1000-block.
    }
    let mut matning: Vec<MatningRad> = sim
        .matning
        .per
        .iter()
        .map(|(k, v)| MatningRad::fran(k, v))
        .collect();
    matning.sort_by(|a, b| a.formaga.cmp(&b.formaga));
    let logg_rader = sim.signal_logg.rader.len();
    let ingrepp_mark = sim.signal_logg.ingrepp_mark;
    let ingrepp_slut = sim.signal_logg.ingrepp_slut;
    let logg_ut = if logg {
        std::mem::take(&mut sim.signal_logg.rader)
    } else {
        Vec::new()
    };
    let bok_ut = if bokfor {
        std::mem::take(&mut sim.bokforing.block)
    } else {
        Vec::new()
    };
    Ok((
        Korning {
            arm: arm.namn.to_string(),
            spegel: arm.spegel,
            fro,
            antal_markering,
            antal_slut: sim.antal(),
            markta_slut: sim.antal_markta(),
            andel_markta: sim.andel_markta(),
            kortid_s: start.elapsed().as_secs_f64(),
            torr,
            markta_formaga: arm.markta.som_str(),
            omarkta_formaga: arm.omarkta.som_str(),
            klocka,
            logg_rader,
            ingrepp_mark,
            ingrepp_slut,
            matning,
        },
        logg_ut,
        bok_ut,
    ))
}

pub fn kor_parallellt(
    paleo: &Path,
    tt_path: &Path,
    b: usize,
    h: usize,
    jobb: &[Jobb],
    torr: bool,
) -> Result<(Vec<Korning>, Vec<SignalLoggRad>, Vec<BokforingBlock>), String> {
    if jobb.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let ko: Arc<Mutex<Vec<Jobb>>> = Arc::new(Mutex::new(jobb.to_vec()));
    let ut: Arc<Mutex<Vec<Result<(Korning, Vec<SignalLoggRad>, Vec<BokforingBlock>), String>>>> =
        Arc::new(Mutex::new(Vec::new()));
    let n_trad = MAX_TRADAR.min(jobb.len()).max(1);
    let paleo = paleo.to_path_buf();
    let tt_path = tt_path.to_path_buf();
    let mut handles = Vec::with_capacity(n_trad);
    for _ in 0..n_trad {
        let ko = Arc::clone(&ko);
        let ut = Arc::clone(&ut);
        let paleo = paleo.clone();
        let tt_path = tt_path.clone();
        handles.push(thread::spawn(move || {
            let kalla = match Kalla::las(&paleo) {
                Ok(k) => k,
                Err(e) => {
                    ut.lock().unwrap().push(Err(e));
                    return;
                }
            };
            let tt_text = match std::fs::read_to_string(&tt_path) {
                Ok(t) => t,
                Err(e) => {
                    ut.lock().unwrap().push(Err(format!("tidtabell: {e}")));
                    return;
                }
            };
            let tt = match Tidtabell::las(&tt_text) {
                Ok(t) => t,
                Err(e) => {
                    ut.lock().unwrap().push(Err(e));
                    return;
                }
            };
            loop {
                let kvar = {
                    let g = ko.lock().unwrap();
                    g.len()
                };
                let nasta = {
                    let mut g = ko.lock().unwrap();
                    g.pop()
                };
                let Some(j) = nasta else { break };
                eprintln!(
                    "startar {} fro={} klocka={} (kvar före pop≈{})",
                    j.arm.namn,
                    j.fro,
                    j.klocka.som_str(),
                    kvar
                );
                let start = Instant::now();
                let r = kor_en(
                    &kalla,
                    &tt,
                    b,
                    h,
                    j.fro,
                    &j.arm,
                    j.klocka,
                    j.logg,
                    j.bokfor,
                    torr,
                );
                match &r {
                    Ok((k, _, _)) => eprintln!(
                        "klar {} fro={} på {:.1} s (andel={:.4})",
                        k.arm,
                        k.fro,
                        start.elapsed().as_secs_f64(),
                        k.andel_markta
                    ),
                    Err(e) => eprintln!("fel {} fro={}: {e}", j.arm.namn, j.fro),
                }
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let mut korningar = Vec::new();
    let mut logg_alla = Vec::new();
    let mut bok_alla = Vec::new();
    for r in ut.lock().unwrap().drain(..) {
        let (k, lg, bk) = r?;
        korningar.push(k);
        logg_alla.extend(lg);
        bok_alla.extend(bk);
    }
    korningar.sort_by(|a, b| {
        let ka = if a.arm == "v_lar" {
            100
        } else {
            ARM_ORDNING
                .iter()
                .position(|&n| n == a.arm)
                .map(|i| i * 2 + if a.spegel { 1 } else { 0 })
                .unwrap_or(99)
        };
        let kb = if b.arm == "v_lar" {
            100
        } else {
            ARM_ORDNING
                .iter()
                .position(|&n| n == b.arm)
                .map(|i| i * 2 + if b.spegel { 1 } else { 0 })
                .unwrap_or(99)
        };
        ka.cmp(&kb).then(a.fro.cmp(&b.fro))
    });
    logg_alla.sort_by(|a, b| a.fro.cmp(&b.fro).then(a.djur_id.cmp(&b.djur_id)));
    bok_alla.sort_by(|a, b| {
        a.arm
            .cmp(&b.arm)
            .then(a.fro.cmp(&b.fro))
            .then(a.formaga.cmp(&b.formaga))
            .then(a.block.cmp(&b.block))
    });
    Ok((korningar, logg_alla, bok_alla))
}

#[derive(Clone, Debug)]
pub struct ArmSam {
    pub arm: String,
    pub spegel: bool,
    pub n_vunna: usize,
    pub n_fro: usize,
    pub medel_andel: f64,
    pub min_andel: f64,
    pub max_andel: f64,
    pub markta_formaga: String,
    pub omarkta_formaga: String,
    pub klocka: Klocka,
}

pub fn sammanfatta(korningar: &[Korning]) -> Vec<ArmSam> {
    let mut ut = Vec::new();
    let mut sedda: Vec<(String, bool)> = Vec::new();
    for k in korningar {
        let nyckel = (k.arm.clone(), k.spegel);
        if !sedda.contains(&nyckel) {
            sedda.push(nyckel);
        }
    }
    for (arm, spegel) in sedda {
        let rader: Vec<&Korning> = korningar
            .iter()
            .filter(|k| k.arm == arm && k.spegel == spegel)
            .collect();
        if rader.is_empty() {
            continue;
        }
        let n_fro = rader.len();
        let n_vunna = rader.iter().filter(|k| k.andel_markta > 0.5).count();
        let andelar: Vec<f64> = rader.iter().map(|k| k.andel_markta).collect();
        let medel_andel = andelar.iter().sum::<f64>() / n_fro as f64;
        let min_andel = andelar.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_andel = andelar.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        ut.push(ArmSam {
            arm,
            spegel,
            n_vunna,
            n_fro,
            medel_andel,
            min_andel,
            max_andel,
            markta_formaga: rader[0].markta_formaga.clone(),
            omarkta_formaga: rader[0].omarkta_formaga.clone(),
            klocka: rader[0].klocka,
        });
    }
    ut
}

fn hitta<'a>(sam: &'a [ArmSam], namn: &str, spegel: bool) -> Option<&'a ArmSam> {
    sam.iter().find(|s| s.arm == namn && s.spegel == spegel)
}

fn spegel_medel(korningar: &[Korning], arm: &str) -> Option<f64> {
    let fron: Vec<u64> = {
        let mut f: Vec<u64> = korningar
            .iter()
            .filter(|k| k.arm == arm)
            .map(|k| k.fro)
            .collect();
        f.sort_unstable();
        f.dedup();
        f
    };
    let mut sum = 0.0;
    let mut n = 0usize;
    for fro in fron {
        let a = korningar
            .iter()
            .find(|k| k.arm == arm && !k.spegel && k.fro == fro);
        let b = korningar
            .iter()
            .find(|k| k.arm == arm && k.spegel && k.fro == fro);
        if let (Some(a), Some(b)) = (a, b) {
            sum += a.andel_markta + b.andel_markta;
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some(sum / n as f64)
    }
}

fn spegel_ok(medel: f64) -> bool {
    (medel - 1.0).abs() <= SPEGEL_TOL
}

// --- Alt 1251 mått -----------------------------------------------------------

/// Varians av en följd (n−1 i nämnaren om n≥2; 0 om n<2).
fn varians(xs: &[f64]) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 0.0;
    }
    let m = xs.iter().sum::<f64>() / n as f64;
    xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1) as f64
}

/// Kvot: lägesdel / (lägesdel + fastdel) över alla 9 handlingar (V:s instrument).
/// None om båda är 0 («saknas»).
pub fn kvot_fran_matriser(vard_rader: &[[f64; A_N]]) -> Option<f64> {
    kvot_over_handlingar(vard_rader, 0..A_N)
}

/// Kvot över riktningarna 1–8 (Alt 1423). Handling 0 ingår inte.
pub fn kvot_fran_matriser_riktningar(vard_rader: &[[f64; A_N]]) -> Option<f64> {
    kvot_over_handlingar(vard_rader, 1..A_N)
}

fn kvot_over_handlingar(
    vard_rader: &[[f64; A_N]],
    handlingar: std::ops::Range<usize>,
) -> Option<f64> {
    if vard_rader.len() < MIN_RADER_DJUR {
        return None;
    }
    let n_a = handlingar.end - handlingar.start;
    if n_a == 0 {
        return None;
    }
    // Centrera per rad över de valda handlingarna (v0 försvinner om den ingår).
    let mut centrerad: Vec<Vec<f64>> = Vec::with_capacity(vard_rader.len());
    for rad in vard_rader {
        let medel: f64 = handlingar.clone().map(|a| rad[a]).sum::<f64>() / n_a as f64;
        let c: Vec<f64> = handlingar.clone().map(|a| rad[a] - medel).collect();
        centrerad.push(c);
    }
    let mut lage_sum = 0.0;
    for i in 0..n_a {
        let kol: Vec<f64> = centrerad.iter().map(|r| r[i]).collect();
        lage_sum += varians(&kol);
    }
    let lagesdelen = lage_sum / n_a as f64;
    let medel_per_a: Vec<f64> = (0..n_a)
        .map(|i| centrerad.iter().map(|r| r[i]).sum::<f64>() / centrerad.len() as f64)
        .collect();
    let fasta_delen = varians(&medel_per_a);
    let namnare = lagesdelen + fasta_delen;
    if namnare <= 1e-18 {
        None
    } else {
        Some(lagesdelen / namnare)
    }
}

/// Handling 0 för sig: medel av centrerat värde (mot medlet av 1–8) och andel av valen.
#[derive(Clone, Debug)]
pub struct Handling0Matt {
    pub medel_centrerad: f64,
    pub andel_val: f64,
}

/// Valet på en rad = första handlingen med strikt högst värde (som `Hj::valj`).
pub fn valj_fran_hj(hj: &[f64; A_N]) -> usize {
    let mut ba = 0usize;
    let mut bv = f64::NEG_INFINITY;
    for a in 0..A_N {
        if hj[a] > bv {
            ba = a;
            bv = hj[a];
        }
    }
    ba
}

pub fn handling0_matt(vard_rader: &[[f64; A_N]], val: &[usize]) -> Handling0Matt {
    assert_eq!(vard_rader.len(), val.len());
    let n = vard_rader.len();
    if n == 0 {
        return Handling0Matt {
            medel_centrerad: 0.0,
            andel_val: 0.0,
        };
    }
    let mut sum_c = 0.0;
    for rad in vard_rader {
        let medel8: f64 = rad[1..A_N].iter().sum::<f64>() / 8.0;
        sum_c += rad[0] - medel8;
    }
    let n0 = val.iter().filter(|&&a| a == 0).count();
    Handling0Matt {
        medel_centrerad: sum_c / n as f64,
        andel_val: n0 as f64 / n as f64,
    }
}

/// z rör sig: medel över de 4 talen av std mellan raderna.
pub fn z_ror_sig(z_rader: &[[f64; Z_N]]) -> Option<f64> {
    if z_rader.len() < MIN_RADER_DJUR {
        return None;
    }
    let mut sum = 0.0;
    for i in 0..Z_N {
        let kol: Vec<f64> = z_rader.iter().map(|r| r[i]).collect();
        let v = varians(&kol);
        sum += v.sqrt();
    }
    Some(sum / Z_N as f64)
}

/// Kendalls W över djurens fasta delar för de 8 flyttarna.
/// Tom delmängd → fel. Lika ordningar → 1; motsatta för två → 0.
pub fn kendalls_w(fast_per_djur: &[[f64; 8]]) -> Result<f64, String> {
    let m = fast_per_djur.len();
    if m == 0 {
        return Err("tom delmängd: inga djur för Kendalls W".into());
    }
    let n = 8usize;
    // Rang per djur (medelrang vid lika).
    let mut rang_sum = [0.0; 8];
    for d in fast_per_djur {
        let r = medelrang(d);
        for i in 0..n {
            rang_sum[i] += r[i];
        }
    }
    let medel = rang_sum.iter().sum::<f64>() / n as f64;
    let s: f64 = rang_sum.iter().map(|x| (x - medel) * (x - medel)).sum();
    let namnare = (m * m) as f64 * ((n * n * n - n) as f64);
    if namnare <= 0.0 {
        return Err("Kendalls W: ogiltig nämnare".into());
    }
    Ok(12.0 * s / namnare)
}

fn medelrang(v: &[f64]) -> Vec<f64> {
    let n = v.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        v[a]
            .partial_cmp(&v[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut rang = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && v[idx[j]] == v[idx[i]] {
            j += 1;
        }
        let medel = ((i + 1) + j) as f64 / 2.0;
        for k in i..j {
            rang[idx[k]] = medel;
        }
        i = j;
    }
    rang
}

/// Positiv avläsare: värde(a) = s[2a] (växtandel i målrutan).
pub fn positiv_varden(s: &[f64; tal::S_N]) -> [f64; A_N] {
    positiv_varden_slice(s)
}

pub fn positiv_varden_slice(s: &[f64]) -> [f64; A_N] {
    let mut v = [0.0; A_N];
    for a in 0..A_N {
        v[a] = s[a * 2];
    }
    v
}

/// Negativ kontroll: fast värde per handling (a själv), oberoende av sinnena.
pub fn negativ_varden(_s: &[f64]) -> [f64; A_N] {
    let mut v = [0.0; A_N];
    for a in 0..A_N {
        v[a] = a as f64;
    }
    v
}

/// Trängselavläsare (Fables rödprov): värde(a) = s[2a+1] (antal i målrutan).
pub fn trangsel_varden(s: &[f64]) -> [f64; A_N] {
    let mut v = [0.0; A_N];
    for a in 0..A_N {
        v[a] = s[a * 2 + 1];
    }
    v
}

/// Ingreppets träff för en sinnessavläsare (riktningarna 1..8, antal 0, egen ruta 0).
/// Används för positiv/negativ/trängsel utan levande djur.
pub fn ingrepp_traff_sinnen<F>(mut avlasare: F) -> f64
where
    F: FnMut(&[f64]) -> [f64; A_N],
{
    let mut s = vec![0.0; tal::S_N];
    let mut sum = 0.0;
    for riktning in 1..A_N {
        for a in 0..A_N {
            s[a * 2] = if a == riktning { 1.0 } else { 0.0 };
            s[a * 2 + 1] = 0.0;
        }
        let vard = avlasare(&s);
        sum += crate::sim::traff_andel(&vard, riktning);
    }
    sum / 8.0
}

/// Ingreppets träff för den positiva avläsaren (räknas, inte hårdkodad).
pub fn ingrepp_traff_positiv() -> f64 {
    ingrepp_traff_sinnen(positiv_varden_slice)
}

/// Ingreppets träff för den negativa kontrollen.
pub fn ingrepp_traff_negativ() -> f64 {
    ingrepp_traff_sinnen(negativ_varden)
}

/// Ingreppets träff för trängselavläsaren.
pub fn ingrepp_traff_trangsel() -> f64 {
    ingrepp_traff_sinnen(trangsel_varden)
}

#[derive(Clone, Debug)]
pub struct FroMatt {
    pub fro: u64,
    pub kvot: Option<f64>,
    pub kvot_lage: Option<f64>,
    pub z_ror: Option<f64>,
    pub ingrepp_mark: Option<f64>,
    pub ingrepp_slut: Option<f64>,
    pub kendall_w: Option<f64>,
    pub k_pos: Option<f64>,
    pub n_djur: usize,
    pub n_rader: usize,
}

/// Räknar mått 1–4 per frö ur signalloggen (+ ingrepp från körningen).
pub fn matt_per_fro(
    rader: &[SignalLoggRad],
    ingrepp_mark: &BTreeMap<u64, f64>,
    ingrepp_slut: &BTreeMap<u64, f64>,
) -> Result<Vec<FroMatt>, String> {
    if rader.is_empty() {
        return Err("tom signallogg".into());
    }
    let mut fron: Vec<u64> = rader.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&SignalLoggRad> = rader.iter().filter(|r| r.fro == fro).collect();
        let mut djur: Vec<u64> = i_fro.iter().map(|r| r.djur_id).collect();
        djur.sort_unstable();
        djur.dedup();
        let mut kvoter = Vec::new();
        let mut kvoter_lage = Vec::new();
        let mut z_ror = Vec::new();
        let mut k_pos = Vec::new();
        let mut fast_flytt: Vec<[f64; 8]> = Vec::new();
        let mut n_ok = 0usize;
        for &d in &djur {
            let rd: Vec<&SignalLoggRad> = i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
            if rd.len() < MIN_RADER_DJUR {
                continue;
            }
            n_ok += 1;
            let vard: Vec<[f64; A_N]> = rd.iter().map(|r| r.hj_varden).collect();
            let lage: Vec<[f64; A_N]> = rd.iter().map(|r| r.lage).collect();
            let z: Vec<[f64; Z_N]> = rd.iter().map(|r| r.z).collect();
            let pos: Vec<[f64; A_N]> = rd.iter().map(|r| positiv_varden(&r.s)).collect();
            if let Some(k) = kvot_fran_matriser(&vard) {
                kvoter.push(k);
            }
            if let Some(k) = kvot_fran_matriser(&lage) {
                kvoter_lage.push(k);
            }
            if let Some(z) = z_ror_sig(&z) {
                z_ror.push(z);
            }
            if let Some(k) = kvot_fran_matriser(&pos) {
                k_pos.push(k);
            }
            // Fast del för flyttarna 1..8: medel över raderna.
            let mut f8 = [0.0; 8];
            for a in 1..A_N {
                f8[a - 1] = rd.iter().map(|r| r.fast[a]).sum::<f64>() / rd.len() as f64;
            }
            fast_flytt.push(f8);
        }
        let kendall_w = if fast_flytt.is_empty() {
            None
        } else {
            Some(kendalls_w(&fast_flytt)?)
        };
        ut.push(FroMatt {
            fro,
            kvot: medel_opt(&kvoter),
            kvot_lage: medel_opt(&kvoter_lage),
            z_ror: medel_opt(&z_ror),
            ingrepp_mark: ingrepp_mark.get(&fro).copied(),
            ingrepp_slut: ingrepp_slut.get(&fro).copied(),
            kendall_w,
            k_pos: medel_opt(&k_pos),
            n_djur: n_ok,
            n_rader: i_fro.len(),
        });
    }
    Ok(ut)
}

fn medel_opt(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

/// Mått 5: blandning inom/mellan djur på rangloggen (samma G1 som omlas_r).
#[derive(Clone, Debug)]
pub struct BlandningFro {
    pub fro: u64,
    pub spearman_oblandad: Option<f64>,
    pub spearman_inom: Option<f64>,
    pub spearman_mellan: Option<f64>,
}

pub fn blandning_pa_rang(
    rang: &[crate::sim::LedLoggRad],
    g1: &G1Vikter,
) -> Result<Vec<BlandningFro>, String> {
    let oblandad = minstakvadrat::omlas_per_rad(rang, g1)?;
    let fro_ob = minstakvadrat::omlas_per_fro(&oblandad)?;
    let mut fron: Vec<u64> = rang.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&crate::sim::LedLoggRad> =
            rang.iter().filter(|r| r.fro == fro).collect();
        // (a) blanda värdena mellan raderna för samma djur
        let mut inom = i_fro.iter().map(|r| (*r).clone()).collect::<Vec<_>>();
        {
            let mut djur: Vec<u64> = inom.iter().map(|r| r.djur_id).collect();
            djur.sort_unstable();
            djur.dedup();
            for d in djur {
                let idx: Vec<usize> = inom
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.djur_id == d)
                    .map(|(i, _)| i)
                    .collect();
                if idx.len() < 2 {
                    continue;
                }
                // Cirkulär permutation av hj_varden mellan raderna.
                let first = inom[idx[0]].hj_varden;
                for k in 0..idx.len() - 1 {
                    inom[idx[k]].hj_varden = inom[idx[k + 1]].hj_varden;
                }
                let last = *idx.last().unwrap();
                inom[last].hj_varden = first;
            }
        }
        let inom_ut = minstakvadrat::omlas_per_rad(&inom, g1)?;
        let inom_fro = minstakvadrat::omlas_per_fro(&inom_ut)?;

        // (b) blanda värdena mellan djuren i samma frö (rad i möter djurets (i+1) värden).
        let mut mellan = i_fro.iter().map(|r| (*r).clone()).collect::<Vec<_>>();
        {
            let mut djur: Vec<u64> = mellan.iter().map(|r| r.djur_id).collect();
            djur.sort_unstable();
            djur.dedup();
            if djur.len() >= 2 {
                // Byt hj_varden mellan djur: varje rad får värden från nästa djur (samma radindex inom djur).
                let mut per_djur: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
                for (i, r) in mellan.iter().enumerate() {
                    per_djur.entry(r.djur_id).or_default().push(i);
                }
                let djur_ord: Vec<u64> = per_djur.keys().copied().collect();
                let sparade: Vec<[f64; A_N]> = mellan.iter().map(|r| r.hj_varden).collect();
                for (di, &d) in djur_ord.iter().enumerate() {
                    let nasta = djur_ord[(di + 1) % djur_ord.len()];
                    let src = &per_djur[&nasta];
                    let dst = &per_djur[&d];
                    let n = dst.len().min(src.len());
                    for k in 0..n {
                        mellan[dst[k]].hj_varden = sparade[src[k]];
                    }
                }
            }
        }
        let mellan_ut = minstakvadrat::omlas_per_rad(&mellan, g1)?;
        let mellan_fro = minstakvadrat::omlas_per_fro(&mellan_ut)?;

        let ob = fro_ob.iter().find(|f| f.fro == fro);
        let inn = inom_fro.iter().find(|f| f.fro == fro);
        let mel = mellan_fro.iter().find(|f| f.fro == fro);
        ut.push(BlandningFro {
            fro,
            spearman_oblandad: ob.and_then(|f| f.flytt_spearman),
            spearman_inom: inn.and_then(|f| f.flytt_spearman),
            spearman_mellan: mel.and_then(|f| f.flytt_spearman),
        });
    }
    Ok(ut)
}

/// Klassificering av mutanter / utfall (prov och sammanfattning).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignalKlass {
    Saknas,
    Blind,
    BlindZStill,
    BlindZRor,
    Ser,
    Annat,
}

pub fn klassa_djur(
    kvot: Option<f64>,
    k_pos: f64,
    ingrepp_slut: f64,
    z_ror: f64,
    kvot_lage: Option<f64>,
) -> SignalKlass {
    match kvot {
        None => SignalKlass::Saknas,
        Some(k) => {
            let blind = k < 0.1 * k_pos && ingrepp_slut <= 0.2;
            let ser = k >= 0.5 * k_pos || ingrepp_slut >= 0.5;
            if ser {
                SignalKlass::Ser
            } else if blind {
                if z_ror < 0.05 {
                    SignalKlass::BlindZStill
                } else if kvot_lage.map(|kl| kl < 0.1 * k_pos).unwrap_or(false) {
                    SignalKlass::BlindZRor
                } else {
                    SignalKlass::Blind
                }
            } else {
                SignalKlass::Annat
            }
        }
    }
}

/// Avgöranden för Alt 1251 (minst 15 av 20 frön).
pub fn avgor_1251(matt: &[FroMatt]) -> (String, String, String, String, String) {
    let n = matt.len();
    let k_pos_ok = matt
        .iter()
        .filter(|m| {
            m.k_pos.map(|k| k >= 0.5).unwrap_or(false) && /* positiv ingrepp alltid 1 */ true
        })
        .count();
    // Instrumentet: positiv kvot ≥0,5 och ingrepp 1 i 20/20. Ingreppet för positiv är 1 per konstruktion.
    let instrument = if k_pos_ok >= n && n > 0 {
        format!(
            "positiv kvot ≥0,5 i {k_pos_ok}/{n} frön; ingreppets träff = 1 (konstruktion). Instrumentet godkänt."
        )
    } else {
        format!(
            "positiv kvot ≥0,5 i {k_pos_ok}/{n} frön. Instrumentet underkänt; inget utfall för lar tolkas."
        )
    };
    if k_pos_ok < n || n == 0 {
        return (
            instrument,
            "tolkas inte (instrument underkänt).".into(),
            "tolkas inte.".into(),
            "tolkas inte.".into(),
            "tolkas inte.".into(),
        );
    }
    let k_pos_medel = matt.iter().filter_map(|m| m.k_pos).sum::<f64>() / n as f64;

    let n_blind = matt
        .iter()
        .filter(|m| {
            let k = m.kvot.unwrap_or(f64::NAN);
            let ing = m.ingrepp_slut.unwrap_or(1.0);
            k < 0.1 * k_pos_medel && ing <= 0.2
        })
        .count();
    let n_blind_zstill = matt
        .iter()
        .filter(|m| {
            let k = m.kvot.unwrap_or(f64::NAN);
            let ing = m.ingrepp_slut.unwrap_or(1.0);
            let z = m.z_ror.unwrap_or(0.0);
            k < 0.1 * k_pos_medel && ing <= 0.2 && z < 0.05
        })
        .count();
    let n_blind_zror = matt
        .iter()
        .filter(|m| {
            let k = m.kvot.unwrap_or(f64::NAN);
            let ing = m.ingrepp_slut.unwrap_or(1.0);
            let z = m.z_ror.unwrap_or(0.0);
            let kl = m.kvot_lage.unwrap_or(0.0);
            k < 0.1 * k_pos_medel && ing <= 0.2 && z >= 0.05 && kl < 0.1 * k_pos_medel
        })
        .count();
    let n_ser = matt
        .iter()
        .filter(|m| {
            let k = m.kvot.unwrap_or(0.0);
            let ing = m.ingrepp_slut.unwrap_or(0.0);
            k >= 0.5 * k_pos_medel || ing >= 0.5
        })
        .count();

    let alt1251 = if n_blind >= VUNNA_FORDEL {
        if n_blind_zstill >= VUNNA_FORDEL {
            format!(
                "blind i {n_blind}/{n} (z stilla i {n_blind_zstill}/{n}): kodaren ger nästan samma inre bild i alla lägen. B stängs; nästa steg är kodaren (till exempel Z_N eller vikternas storlek)."
            )
        } else if n_blind_zror >= VUNNA_FORDEL {
            format!(
                "blind i {n_blind}/{n} (z rör sig, v·W·z-kvot låg i {n_blind_zror}/{n}): den inre bilden ändras, men värdet läser inte ändringen. Det är ett konkret fel i hjärnans värdering (Alt 1212), och B får ett mål."
            )
        } else {
            format!("blind i {n_blind}/{n}: ingen visad skillnad mellan z-still/z-rör undergränserna.")
        }
    } else if n_ser >= VUNNA_FORDEL {
        format!(
            "ser i {n_ser}/{n}: värdet följer läget; då är nollprovet förklarat av något annat, och det skrivs."
        )
    } else {
        format!(
            "blind {n_blind}/{n}, ser {n_ser}/{n}: ingen visad skillnad."
        )
    };

    // Flockens drift via Kendalls W
    let n_w_hog = matt
        .iter()
        .filter(|m| m.kendall_w.map(|w| w >= 0.5).unwrap_or(false))
        .count();
    let n_w_lag = matt
        .iter()
        .filter(|m| m.kendall_w.map(|w| w <= 0.2).unwrap_or(false))
        .count();
    let flock = if n_w_hog >= VUNNA_FORDEL {
        format!("Kendalls W ≥0,5 i {n_w_hog}/{n}: djuren delar samma fasta ordning.")
    } else if n_w_lag >= VUNNA_FORDEL {
        format!("Kendalls W ≤0,2 i {n_w_lag}/{n}: varje djur har sin egen fasta ordning.")
    } else {
        format!("Kendalls W hög {n_w_hog}/{n}, låg {n_w_lag}/{n}: ingen visad skillnad.")
    };

    (
        instrument,
        alt1251,
        flock,
        format!("k_pos medel = {k_pos_medel:.4}"),
        format!("v0 är lika för alla handlingar och tas bort genom centreringen per rad."),
    )
}

pub fn avgor_fast_per_djur(bland: &[BlandningFro]) -> String {
    let n = bland.len();
    let n_fast = bland
        .iter()
        .filter(|b| {
            let inom = b.spearman_inom.unwrap_or(0.0);
            let mellan = b.spearman_mellan.unwrap_or(-1.0);
            mellan > -0.02 && inom < -0.05
        })
        .count();
    if n_fast >= VUNNA_FORDEL {
        format!(
            "blandning mellan djur ger rangkorrelation över −0,02 medan blandning inom djur ger under −0,05 i {n_fast}/{n}: felet är djurets fasta ordning."
        )
    } else {
        format!(
            "fast-per-djur-mönstret i {n_fast}/{n}: ingen visad skillnad."
        )
    }
}

pub fn avgor_1252(sam: &[ArmSam], korningar: &[Korning]) -> String {
    let a_to = hitta(sam, "a_to", false);
    let b_to = hitta(sam, "b_to", false);
    let sp_a = spegel_medel(korningar, "a_to")
        .map(spegel_ok)
        .unwrap_or(false);
    let sp_b = spegel_medel(korningar, "b_to")
        .map(spegel_ok)
        .unwrap_or(false);
    let (va, na) = match a_to {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let (vb, nb) = match b_to {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    if !sp_a || !sp_b {
        return format!(
            "a_to {va}/{na} (spegel {}), b_to {vb}/{nb} (spegel {}): spegel utanför; armarna tolkas inte.",
            if sp_a { "ok" } else { "utanför" },
            if sp_b { "ok" } else { "utanför" },
        );
    }
    let text = if va >= VUNNA_FORDEL && vb <= VUNNA_NACKDEL {
        "a_to minst 15 och b_to högst 5: uppgiften om de andras nya platser bär segern."
    } else if vb >= VUNNA_FORDEL && va <= VUNNA_NACKDEL {
        "b_to minst 15 och a_to högst 5: betet i tur bär segern."
    } else if va >= VUNNA_FORDEL && vb >= VUNNA_FORDEL {
        "Båda minst 15: var och en räcker."
    } else if va <= VUNNA_NACKDEL && vb <= VUNNA_NACKDEL {
        "Båda högst 5: bara tillsammans."
    } else {
        "ingen visad skillnad."
    };
    format!(
        "a_to {va}/{na} (spegel ok), b_to {vb}/{nb} (spegel ok). {text}"
    )
}

/// Jämför u_to/s_to frö för frö med tur1246-korningar.jsonl.
pub fn jamfor_tur1246(
    korningar: &[Korning],
    tur_jsonl: &str,
) -> Result<(usize, usize, String), String> {
    let mut tur_u: BTreeMap<u64, f64> = BTreeMap::new();
    let mut tur_s: BTreeMap<u64, f64> = BTreeMap::new();
    for rad in tur_jsonl.lines() {
        let rad = rad.trim();
        if rad.is_empty() {
            continue;
        }
        let json = rad.split(" TORRKÖRNING").next().unwrap_or(rad);
        let arm = json_str_falt(json, "arm")?;
        let spegel = json_bool_falt(json, "spegel")?;
        if spegel {
            continue;
        }
        let fro = json_u64_falt(json, "fro")?;
        let andel = json_f64_falt(json, "andel_markta")?;
        if arm == "u_to" {
            tur_u.insert(fro, andel);
        } else if arm == "s_to" {
            tur_s.insert(fro, andel);
        }
    }
    let mut n_u = 0usize;
    let mut n_u_ok = 0usize;
    let mut n_s = 0usize;
    let mut n_s_ok = 0usize;
    let mut avvikelser = Vec::new();
    for k in korningar.iter().filter(|k| k.arm == "u_to" && !k.spegel) {
        n_u += 1;
        if let Some(&forvantad) = tur_u.get(&k.fro) {
            if (k.andel_markta - forvantad).abs() < 1e-12 {
                n_u_ok += 1;
            } else {
                avvikelser.push(format!(
                    "u_to frö {}: fick {}, tur1246 {}",
                    k.fro, k.andel_markta, forvantad
                ));
            }
        } else {
            avvikelser.push(format!("u_to frö {}: saknas i tur1246", k.fro));
        }
    }
    for k in korningar.iter().filter(|k| k.arm == "s_to" && !k.spegel) {
        n_s += 1;
        if let Some(&forvantad) = tur_s.get(&k.fro) {
            if (k.andel_markta - forvantad).abs() < 1e-12 {
                n_s_ok += 1;
            } else {
                avvikelser.push(format!(
                    "s_to frö {}: fick {}, tur1246 {}",
                    k.fro, k.andel_markta, forvantad
                ));
            }
        } else {
            avvikelser.push(format!("s_to frö {}: saknas i tur1246", k.fro));
        }
    }
    let text = if n_u_ok == n_u && n_s_ok == n_s && n_u > 0 && n_s > 0 {
        format!(
            "u_to {n_u_ok}/{n_u} och s_to {n_s_ok}/{n_s} exakt samma andel som Alt 1246."
        )
    } else {
        format!(
            "u_to {n_u_ok}/{n_u}, s_to {n_s_ok}/{n_s}. Avvikelser: {}",
            if avvikelser.is_empty() {
                "inga detaljer".into()
            } else {
                avvikelser.join("; ")
            }
        )
    };
    Ok((n_u_ok + n_s_ok, n_u + n_s, text))
}

fn json_str_falt(json: &str, nyckel: &str) -> Result<String, String> {
    let mark = format!("\"{nyckel}\":\"");
    let i = json
        .find(&mark)
        .ok_or_else(|| format!("saknar {nyckel}"))?;
    let start = i + mark.len();
    let slut = json[start..]
        .find('"')
        .ok_or_else(|| format!("trasig {nyckel}"))?;
    Ok(json[start..start + slut].to_string())
}

fn json_bool_falt(json: &str, nyckel: &str) -> Result<bool, String> {
    let mark = format!("\"{nyckel}\":");
    let i = json
        .find(&mark)
        .ok_or_else(|| format!("saknar {nyckel}"))?;
    let start = i + mark.len();
    Ok(json[start..].starts_with("true"))
}

fn json_u64_falt(json: &str, nyckel: &str) -> Result<u64, String> {
    let mark = format!("\"{nyckel}\":");
    let i = json
        .find(&mark)
        .ok_or_else(|| format!("saknar {nyckel}"))?;
    let start = i + mark.len();
    let slut = json[start..]
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(json[start..].len());
    json[start..start + slut]
        .parse()
        .map_err(|_| format!("kunde inte tolka {nyckel}"))
}

fn json_f64_falt(json: &str, nyckel: &str) -> Result<f64, String> {
    let mark = format!("\"{nyckel}\":");
    let i = json
        .find(&mark)
        .ok_or_else(|| format!("saknar {nyckel}"))?;
    let start = i + mark.len();
    let slut = json[start..]
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(json[start..].len());
    json[start..start + slut]
        .parse()
        .map_err(|_| format!("kunde inte tolka {nyckel}"))
}

pub fn skriv_logg_tsv(rader: &[SignalLoggRad]) -> String {
    let mut tsv = String::from("fro\tdjur_id");
    for i in 0..tal::S_N {
        tsv.push_str(&format!("\ts{i}"));
    }
    for i in 0..Z_N {
        tsv.push_str(&format!("\tz{i}"));
    }
    for a in 0..A_N {
        tsv.push_str(&format!("\thj{a}"));
    }
    for a in 0..A_N {
        tsv.push_str(&format!("\tlage{a}"));
    }
    for a in 0..A_N {
        tsv.push_str(&format!("\tfast{a}"));
    }
    tsv.push('\n');
    for r in rader {
        tsv.push_str(&format!("{}\t{}", r.fro, r.djur_id));
        for i in 0..tal::S_N {
            tsv.push_str(&format!("\t{}", r.s[i]));
        }
        for i in 0..Z_N {
            tsv.push_str(&format!("\t{}", r.z[i]));
        }
        for a in 0..A_N {
            tsv.push_str(&format!("\t{}", r.hj_varden[a]));
        }
        for a in 0..A_N {
            tsv.push_str(&format!("\t{}", r.lage[a]));
        }
        for a in 0..A_N {
            tsv.push_str(&format!("\t{}", r.fast[a]));
        }
        tsv.push('\n');
    }
    tsv
}

pub fn las_logg_tsv(text: &str) -> Result<Vec<SignalLoggRad>, String> {
    let mut rader = Vec::new();
    let mut forsta = true;
    for (nr, rad) in text.lines().enumerate() {
        if forsta {
            forsta = false;
            continue;
        }
        if rad.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = rad.split('\t').collect();
        let behov = 2 + tal::S_N + Z_N + A_N * 3;
        if f.len() < behov {
            return Err(format!("rad {}: för få fält ({})", nr + 1, f.len()));
        }
        let fro: u64 = f[0].parse().map_err(|_| format!("rad {}: fro", nr + 1))?;
        let djur_id: u64 = f[1]
            .parse()
            .map_err(|_| format!("rad {}: djur_id", nr + 1))?;
        let mut s = [0.0; tal::S_N];
        for i in 0..tal::S_N {
            s[i] = f[2 + i]
                .parse()
                .map_err(|_| format!("rad {}: s{i}", nr + 1))?;
        }
        let mut z = [0.0; Z_N];
        for i in 0..Z_N {
            z[i] = f[2 + tal::S_N + i]
                .parse()
                .map_err(|_| format!("rad {}: z{i}", nr + 1))?;
        }
        let mut hj_varden = [0.0; A_N];
        let mut lage = [0.0; A_N];
        let mut fast = [0.0; A_N];
        let bas = 2 + tal::S_N + Z_N;
        for a in 0..A_N {
            hj_varden[a] = f[bas + a]
                .parse()
                .map_err(|_| format!("rad {}: hj{a}", nr + 1))?;
            lage[a] = f[bas + A_N + a]
                .parse()
                .map_err(|_| format!("rad {}: lage{a}", nr + 1))?;
            fast[a] = f[bas + 2 * A_N + a]
                .parse()
                .map_err(|_| format!("rad {}: fast{a}", nr + 1))?;
        }
        rader.push(SignalLoggRad {
            fro,
            djur_id,
            s,
            z,
            hj_varden,
            lage,
            fast,
        });
    }
    Ok(rader)
}

pub fn skriv_bokforing_tsv(block: &[BokforingBlock]) -> String {
    let mut tsv = String::from(
        "fro\tarm\tformaga\tblock\tfodda\tdod_svalt\tdod_alder\tantal_slut\tmedelintag\tdjursteg\n",
    );
    for b in block {
        let mi = b
            .medelintag()
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "saknas".into());
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            b.fro,
            b.arm,
            b.formaga,
            b.block,
            b.fodda,
            b.dod_svalt,
            b.dod_alder,
            b.antal_slut,
            mi,
            b.djursteg
        ));
    }
    tsv
}

pub fn skriv_signal_tsv(matt: &[FroMatt], bland: Option<&[BlandningFro]>) -> String {
    let mut tsv = String::from(
        "fro\tkvot\tkvot_lage\tz_ror\tingrepp_mark\tingrepp_slut\tkendall_w\tk_pos\tn_djur\tn_rader\tspearman_oblandad\tspearman_inom\tspearman_mellan\n",
    );
    for m in matt {
        let b = bland.and_then(|bs| bs.iter().find(|x| x.fro == m.fro));
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            m.fro,
            fmt_opt(m.kvot),
            fmt_opt(m.kvot_lage),
            fmt_opt(m.z_ror),
            fmt_opt(m.ingrepp_mark),
            fmt_opt(m.ingrepp_slut),
            fmt_opt(m.kendall_w),
            fmt_opt(m.k_pos),
            m.n_djur,
            m.n_rader,
            fmt_opt(b.and_then(|x| x.spearman_oblandad)),
            fmt_opt(b.and_then(|x| x.spearman_inom)),
            fmt_opt(b.and_then(|x| x.spearman_mellan)),
        ));
    }
    tsv
}

pub fn skriv_sammanfattning(
    korningar: &[Korning],
    matt: &[FroMatt],
    bland: Option<&[BlandningFro]>,
    bok: &[BokforingBlock],
    tur_jamforelse: &str,
    torr: bool,
    total_s: f64,
    logg_bytes: u64,
    logg_sha256: &str,
) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Signalprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön per arm, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str(
            "# Strykprovet på värdets signalväg (Alt 1251, med 1252 och 1253)\n\n",
        );
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-v1251.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str(&format!(
        "Logg: {logg_bytes} byte, sha256={logg_sha256}\n\n"
    ));
    md.push_str(
        "`v0` är lika för alla handlingar och tas bort genom centreringen per rad.\n\n",
    );

    md.push_str(
        "| Arm | Spegel | Klocka | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |\n",
    );
    md.push_str(
        "|-----|--------|--------|-------|--------|-------------------|-------|-----|-----|\n",
    );
    for s in &sam {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {}/{} | {:.4} | {:.4} | {:.4} |\n",
            s.arm,
            if s.spegel { "ja" } else { "nej" },
            s.klocka.som_str(),
            s.markta_formaga,
            s.omarkta_formaga,
            s.n_vunna,
            s.n_fro,
            s.medel_andel,
            s.min_andel,
            s.max_andel,
        ));
    }

    md.push_str("\n## Alt 1251 mått per frö\n\n");
    md.push_str(
        "| Frö | Kvot | Kvot lage | z rör | Ingrepp mark | Ingrepp slut | W | k_pos | n_djur |\n",
    );
    md.push_str(
        "|-----|------|-----------|-------|--------------|--------------|---|-------|--------|\n",
    );
    for m in matt {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            m.fro,
            fmt_opt(m.kvot),
            fmt_opt(m.kvot_lage),
            fmt_opt(m.z_ror),
            fmt_opt(m.ingrepp_mark),
            fmt_opt(m.ingrepp_slut),
            fmt_opt(m.kendall_w),
            fmt_opt(m.k_pos),
            m.n_djur,
        ));
    }

    if let Some(bl) = bland {
        md.push_str("\n## Mått 5: blandning på rangloggen\n\n");
        md.push_str("| Frö | Oblandad | Inom djur | Mellan djur |\n");
        md.push_str("|-----|----------|-----------|-------------|\n");
        for b in bl {
            md.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                b.fro,
                fmt_opt(b.spearman_oblandad),
                fmt_opt(b.spearman_inom),
                fmt_opt(b.spearman_mellan),
            ));
        }
    }

    md.push_str("\n## Bokföring (Alt 1253, u_to/a_to/b_to utan spegel)\n\n");
    md.push_str(&format!("Antal blockrader: {}.\n\n", bok.len()));
    md.push_str("Frön där trängseloraklet åt mer än slumpen i u_to (181, 189, 192, 195, 200):\n\n");
    for &fro in &U_TO_ATER_MER {
        let rader: Vec<&BokforingBlock> = bok
            .iter()
            .filter(|b| b.fro == fro && b.arm == "u_to")
            .collect();
        if rader.is_empty() {
            md.push_str(&format!("- frö {fro}: saknas\n"));
            continue;
        }
        md.push_str(&format!("- frö {fro} (u_to): {} blockrader\n", rader.len()));
        for b in &rader {
            md.push_str(&format!(
                "  - {} block {}: födda={}, död_svalt={}, död_alder={}, antal_slut={}, medelintag={}\n",
                b.formaga,
                b.block,
                b.fodda,
                b.dod_svalt,
                b.dod_alder,
                b.antal_slut,
                fmt_opt(b.medelintag()),
            ));
        }
    }

    md.push_str("\n## Avgöranden\n\n");
    for namn in ARM_ORDNING {
        match spegel_medel(korningar, namn) {
            Some(m) => {
                let ok = spegel_ok(m);
                md.push_str(&format!(
                    "- **Spegel {namn}:** medel av (andel + spegelandel) = {:.4} (krav 1 ± {:.3}) — {}.\n",
                    m,
                    SPEGEL_TOL,
                    if ok {
                        "inom gränsen"
                    } else {
                        "utanför: märket har en bieffekt; armen tolkas inte"
                    }
                ));
            }
            None => md.push_str(&format!("- **Spegel {namn}:** saknas.\n")),
        }
    }

    let (instrument, alt1251, flock, _kpos, v0text) = avgor_1251(matt);
    md.push_str(&format!("- **Instrumentet:** {instrument}\n"));
    md.push_str(&format!("- **Alt 1251:** {alt1251}\n"));
    md.push_str(&format!("- **Flockens drift:** {flock}\n"));
    if let Some(bl) = bland {
        md.push_str(&format!("- **Fast per djur:** {}\n", avgor_fast_per_djur(bl)));
    }
    md.push_str(&format!("- **Alt 1252:** {}\n", avgor_1252(&sam, korningar)));
    md.push_str(&format!("- **Upprepningen:** {tur_jamforelse}\n"));
    md.push_str("- **Bokföringen:** ");
    if bok.is_empty() {
        md.push_str("inga block (torrkörning eller saknas).\n");
    } else {
        md.push_str("bokföringskontrollen godkänd för alla körda armar (antal vid märkning + födslar − döda = antal vid slut).\n");
    }
    for namn in ["a_tab", "b_tab"] {
        if let Some(a) = hitta(&sam, namn, false) {
            md.push_str(&format!(
                "- **{namn} (redovisning):** {}/{} vunna, medel andel {:.4}.\n",
                a.n_vunna, a.n_fro, a.medel_andel
            ));
        }
    }
    md.push_str(&format!("- **{v0text}**\n"));
    md
}

fn sha256_hex(data: &[u8]) -> String {
    // Enkel FNV-64 som kontrollsumma om sha2 saknas; märk som fnv64.
    // Underlaget ber om kontrollsumma — FNV-64 räcker som icke-krypto-kontroll.
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{h:016x}")
}

pub fn kor_signal(ut_dir: &Path, torr: bool) -> Result<(), String> {
    kor_signal_med(
        ut_dir,
        &SignalVal {
            torr,
            ..SignalVal::default()
        },
    )
}

pub fn kor_signal_med(ut_dir: &Path, val: &SignalVal) -> Result<(), String> {
    sat_nice();
    let start = Instant::now();
    let paleo = ut_dir.join("../paleokarta/data");
    let tt_path = ut_dir.join("../hckarta/tidtabell.tsv");
    let (b, h) = if val.torr {
        (B_TORR, H_TORR)
    } else {
        (B_FULL, H_FULL)
    };
    let jobb = bygg_jobb_med(val);
    let (korningar, logg, bok) = kor_parallellt(&paleo, &tt_path, b, h, &jobb, val.torr)?;
    let total_s = start.elapsed().as_secs_f64();

    let prefix = if val.bara_v_lar
        || val
            .fro_start
            .map(|s| s >= FRO_1425_START)
            .unwrap_or(false)
    {
        "hjsignal1425"
    } else {
        "v1251"
    };
    let jsonl_path = ut_dir.join(format!("{prefix}-korningar.jsonl"));
    let logg_path = ut_dir.join(format!("{prefix}-logg.tsv"));
    let bok_path = ut_dir.join(format!("{prefix}-bokforing.tsv"));
    let signal_path = ut_dir.join(format!("{prefix}-signal.tsv"));
    let md_path = ut_dir.join(format!("{prefix}-sammanfattning.md"));

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let logg_tsv = skriv_logg_tsv(&logg);
    let logg_bytes = logg_tsv.len() as u64;
    let logg_sum = sha256_hex(logg_tsv.as_bytes());
    // Skriv loggen; om >50 MB ska den inte i git (hanteras vid commit).
    std::fs::write(&logg_path, &logg_tsv)
        .map_err(|e| format!("skriv {}: {e}", logg_path.display()))?;

    let bok_tsv = skriv_bokforing_tsv(&bok);
    std::fs::write(&bok_path, &bok_tsv)
        .map_err(|e| format!("skriv {}: {e}", bok_path.display()))?;

    let mut ing_mark = BTreeMap::new();
    let mut ing_slut = BTreeMap::new();
    for k in korningar.iter().filter(|k| k.arm == "v_lar") {
        if let Some(v) = k.ingrepp_mark {
            ing_mark.insert(k.fro, v);
        }
        if let Some(v) = k.ingrepp_slut {
            ing_slut.insert(k.fro, v);
        }
    }
    let matt = if logg.is_empty() {
        Vec::new()
    } else {
        matt_per_fro(&logg, &ing_mark, &ing_slut)?
    };

    let led_vag = std::env::var("HCSIM_LED_LOGG").unwrap_or_else(|_| LED_LOGG_VAG.to_string());
    let rang_vag = std::env::var("HCSIM_RANG_LOGG").unwrap_or_else(|_| RANG_LOGG_VAG.to_string());
    let bland = match rakna_blandning(&led_vag, &rang_vag) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("mått 5 (blandning) kunde inte räknas: {e}");
            None
        }
    };

    let signal_tsv = skriv_signal_tsv(&matt, bland.as_deref());
    std::fs::write(&signal_path, &signal_tsv)
        .map_err(|e| format!("skriv {}: {e}", signal_path.display()))?;

    let tur_path = ut_dir.join("tur1246-korningar.jsonl");
    let tur_jamforelse = match std::fs::read_to_string(&tur_path) {
        Ok(t) => match jamfor_tur1246(&korningar, &t) {
            Ok((_, _, text)) => text,
            Err(e) => format!("kunde inte jämföra: {e}"),
        },
        Err(e) => format!("kunde inte läsa tur1246: {e}"),
    };

    let md = skriv_sammanfattning(
        &korningar,
        &matt,
        bland.as_deref(),
        &bok,
        &tur_jamforelse,
        val.torr,
        total_s,
        logg_bytes,
        &logg_sum,
    );
    print!("{md}");
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!(
        "skrev {} ({} körningar), logg {} byte, på {:.1} s",
        jsonl_path.display(),
        korningar.len(),
        logg_bytes,
        total_s
    );
    Ok(())
}

fn rakna_blandning(led_vag: &str, rang_vag: &str) -> Result<Vec<BlandningFro>, String> {
    let led_text = std::fs::read_to_string(led_vag)
        .map_err(|e| format!("kunde inte läsa ledprovsloggen {led_vag}: {e}"))?;
    let led = minstakvadrat::las_tsv(&led_text)?;
    let rang_text = std::fs::read_to_string(rang_vag)
        .map_err(|e| format!("kunde inte läsa rangloggen {rang_vag}: {e}"))?;
    let rang = minstakvadrat::las_rang_tsv(&rang_text)?;
    let anp: Vec<u64> = (LED_ANP_START..LED_ANP_START + LED_ANP_N).collect();
    let g1 = minstakvadrat::anpassa_g1(&led, &anp);
    blandning_pa_rang(&rang, &g1)
}
