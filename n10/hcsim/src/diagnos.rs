//! Diagnosen av hjärnan (Alt 1207 med 1208 och 1209): 11 armar + speglar, mätfält, H1–H3.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{diagnos_armar, Formaga, HjarnArm};
use crate::sim::{FormagaMatning, MarkLage, Matning, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL, A_N};

/// Kartstorlek för hela diagnosen (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön (inte hjärnprovets 1–20).
pub const FRO_START: u64 = 101;
pub const N_FRO_FULL: u64 = 20;

/// Gränser ur underlaget (skrivna före körningen).
pub const H1_FAKTOR: f64 = 1.5;
pub const H2_STA_KVAR: f64 = 0.5;
pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;

#[derive(Clone, Debug)]
pub struct MatningRad {
    pub formaga: String,
    pub andelar: [f64; A_N],
    pub entropi: f64,
    pub andel_sta_kvar: f64,
    pub intag_per_steg: Option<f64>,
    pub e_vid_dod: Option<f64>,
    pub dod_svalt: u64,
    pub dod_alder: u64,
    pub andra_medel: Option<f64>,
    pub fel_tidigt: Option<f64>,
    pub fel_sent: Option<f64>,
    pub djursteg: u64,
}

impl MatningRad {
    fn fran(formaga: &str, m: &FormagaMatning) -> Self {
        let mut andelar = [0.0; A_N];
        for a in 0..A_N {
            andelar[a] = m.andel_handling(a);
        }
        Self {
            formaga: formaga.to_string(),
            andelar,
            entropi: m.entropi(),
            andel_sta_kvar: m.andel_sta_kvar(),
            intag_per_steg: m.intag_per_steg(),
            e_vid_dod: m.e_vid_dod_medel(),
            dod_svalt: m.dod_svalt,
            dod_alder: m.dod_alder,
            andra_medel: m.andra_medel(),
            fel_tidigt: m.fel_tidigt.medel(),
            fel_sent: m.fel_sent.medel(),
            djursteg: m.djursteg,
        }
    }

    fn till_json(&self) -> String {
        let andelar: Vec<String> = self.andelar.iter().map(|x| fmt_f64(*x)).collect();
        format!(
            "{{\"formaga\":\"{}\",\"andelar\":[{}],\"entropi\":{},\"andel_sta_kvar\":{},\"intag_per_steg\":{},\"e_vid_dod\":{},\"dod_svalt\":{},\"dod_alder\":{},\"andra_medel\":{},\"fel_tidigt\":{},\"fel_sent\":{},\"djursteg\":{}}}",
            self.formaga,
            andelar.join(","),
            fmt_f64(self.entropi),
            fmt_f64(self.andel_sta_kvar),
            fmt_opt(self.intag_per_steg),
            fmt_opt(self.e_vid_dod),
            self.dod_svalt,
            self.dod_alder,
            fmt_opt(self.andra_medel),
            fmt_opt(self.fel_tidigt),
            fmt_opt(self.fel_sent),
            self.djursteg,
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
    pub matning: Vec<MatningRad>,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let mat: Vec<String> = self.matning.iter().map(|m| m.till_json()).collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"matning\":[{}],\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            self.markta_formaga,
            self.omarkta_formaga,
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
        format!("{}", x)
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

fn fro_lista(torr: bool) -> Vec<u64> {
    let n = if torr { N_FRO_TORR } else { N_FRO_FULL };
    (0..n).map(|i| FRO_START + i).collect()
}

fn bygg_sim(kalla: &Kalla, tt: &Tidtabell, b: usize, h: usize, fro: u64, arm: &HjarnArm) -> Result<Sim, String> {
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
    sim.matning = Matning {
        pa: true,
        ..Matning::default()
    };
    Ok(sim)
}

pub fn kor_en(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    arm: &HjarnArm,
    torr: bool,
) -> Result<Korning, String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm)?;
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    sim.markera_halften(fro);
    let antal_markering = sim.antal();
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, MARKT_EFTER_AR);
    sim.matning.krav_data()?;
    let mut matning: Vec<MatningRad> = sim
        .matning
        .per
        .iter()
        .map(|(k, v)| MatningRad::fran(k, v))
        .collect();
    matning.sort_by(|a, b| a.formaga.cmp(&b.formaga));
    Ok(Korning {
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
        matning,
    })
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    fro: u64,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let fron = fro_lista(torr);
    let mut jobb = Vec::new();
    for arm in diagnos_armar() {
        for &fro in &fron {
            jobb.push(Jobb {
                arm: arm.clone(),
                fro,
            });
        }
    }
    jobb.reverse();
    jobb
}

pub fn kor_parallellt(
    paleo: &Path,
    tt_path: &Path,
    b: usize,
    h: usize,
    jobb: &[Jobb],
    torr: bool,
) -> Result<Vec<Korning>, String> {
    if jobb.is_empty() {
        return Ok(Vec::new());
    }
    let ko: Arc<Mutex<Vec<Jobb>>> = Arc::new(Mutex::new(jobb.to_vec()));
    let ut: Arc<Mutex<Vec<Result<Korning, String>>>> = Arc::new(Mutex::new(Vec::new()));
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
                let nasta = {
                    let mut g = ko.lock().unwrap();
                    g.pop()
                };
                let Some(j) = nasta else { break };
                let r = kor_en(&kalla, &tt, b, h, j.fro, &j.arm, torr);
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let mut resultat = Vec::new();
    for r in ut.lock().unwrap().drain(..) {
        resultat.push(r?);
    }
    let ordning = [
        ("d_arm2", false),
        ("d_arm1", false),
        ("d_arm3", false),
        ("d_tak", false),
        ("tak_a", false),
        ("tak_b10", false),
        ("tak_b30", false),
        ("tak_b50", false),
        ("u1", false),
        ("u2", false),
        ("u3", false),
        ("d_arm2", true),
        ("d_arm1", true),
        ("d_arm3", true),
        ("d_tak", true),
        ("tak_a", true),
        ("tak_b10", true),
        ("tak_b30", true),
        ("tak_b50", true),
        ("u1", true),
        ("u2", true),
        ("u3", true),
    ];
    resultat.sort_by(|a, b| {
        let ka = ordning
            .iter()
            .position(|&(n, s)| n == a.arm && s == a.spegel)
            .unwrap_or(99);
        let kb = ordning
            .iter()
            .position(|&(n, s)| n == b.arm && s == b.spegel)
            .unwrap_or(99);
        ka.cmp(&kb).then(a.fro.cmp(&b.fro))
    });
    Ok(resultat)
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
        });
    }
    ut
}

fn tolka_vunna(n: usize) -> &'static str {
    if n >= VUNNA_FORDEL {
        "synlig fördel"
    } else if n <= VUNNA_NACKDEL {
        "synlig nackdel"
    } else {
        "ingen visad skillnad"
    }
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

fn medel_matning_falt(
    korningar: &[Korning],
    arm: &str,
    formaga: &str,
    falt: impl Fn(&MatningRad) -> Option<f64>,
) -> Option<f64> {
    let mut sum = 0.0;
    let mut n = 0usize;
    for k in korningar.iter().filter(|k| k.arm == arm && !k.spegel) {
        if let Some(m) = k.matning.iter().find(|m| m.formaga == formaga) {
            if let Some(v) = falt(m) {
                sum += v;
                n += 1;
            }
        }
    }
    if n == 0 {
        None
    } else {
        Some(sum / n as f64)
    }
}

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Diagnosen — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Diagnosen av hjärnan (Alt 1207 med 1208 och 1209)\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-diagnos1207.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str("| Arm | Spegel | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |\n");
    md.push_str("|-----|--------|-------|--------|-------------------|-------|-----|-----|\n");
    for s in &sam {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {}/{} | {:.4} | {:.4} | {:.4} |\n",
            s.arm,
            if s.spegel { "ja" } else { "nej" },
            s.markta_formaga,
            s.omarkta_formaga,
            s.n_vunna,
            s.n_fro,
            s.medel_andel,
            s.min_andel,
            s.max_andel,
        ));
    }
    md.push('\n');
    md.push_str("## Avgöranden\n\n");

    // Speglar
    let arm_namn = [
        "d_arm2", "d_arm1", "d_arm3", "d_tak", "tak_a", "tak_b10", "tak_b30", "tak_b50", "u1",
        "u2", "u3",
    ];
    for namn in arm_namn {
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

    // Taket (Alt 1208)
    let tak_armar = ["tak_a", "tak_b10", "tak_b30", "tak_b50"];
    let mut tak_vinner = false;
    let mut tak_rader = Vec::new();
    for namn in tak_armar {
        if let Some(a) = hitta(&sam, namn, false) {
            let spegel_tolkas = spegel_medel(korningar, namn)
                .map(spegel_ok)
                .unwrap_or(false);
            if a.n_vunna >= VUNNA_FORDEL && spegel_tolkas {
                tak_vinner = true;
            }
            tak_rader.push(format!(
                "{} {}/{} ({})",
                namn,
                a.n_vunna,
                a.n_fro,
                tolka_vunna(a.n_vunna)
            ));
        }
    }
    md.push_str(&format!(
        "- **Taket (Alt 1208):** {}. {}\n",
        if tak_rader.is_empty() {
            "saknas".into()
        } else {
            tak_rader.join("; ")
        },
        if tak_vinner {
            "Taket vinner: minst en arm når ≥15 av 20 (världen belönar kunskap om växterna)."
        } else {
            "Taket vinner inte: ingen arm i Alt 1208 når minst 15 av 20; världen belönar ingen kunskap om växterna (Alt 1212)."
        }
    ));

    // H1 trängsel (d_tak)
    let andra_allv = medel_matning_falt(korningar, "d_tak", "allvetande", |m| m.andra_medel);
    let andra_slump = medel_matning_falt(korningar, "d_tak", "slump", |m| m.andra_medel);
    let spegel_dtak_ok = spegel_medel(korningar, "d_tak")
        .map(spegel_ok)
        .unwrap_or(false);
    match (andra_allv, andra_slump) {
        (Some(av), Some(sl)) if sl > 0.0 => {
            let kvot = av / sl;
            let stammer = kvot >= H1_FAKTOR && spegel_dtak_ok;
            md.push_str(&format!(
                "- **H1 (trängsel):** allvetande andra-djur-medel = {:.4}, slump = {:.4}, kvot = {:.4} (krav ≥ {:.1}). Spegel d_tak {}. {}.\n",
                av,
                sl,
                kvot,
                H1_FAKTOR,
                if spegel_dtak_ok { "ok" } else { "utanför" },
                if stammer {
                    "Stämmer"
                } else if !spegel_dtak_ok {
                    "Tolkas inte (spegel utanför)"
                } else {
                    "Stämmer inte"
                }
            ));
        }
        _ => md.push_str("- **H1 (trängsel):** saknas mätvärden.\n"),
    }

    // H2 frusen stilla/upprepar (d_arm2)
    let sta = medel_matning_falt(korningar, "d_arm2", "frusen", |m| Some(m.andel_sta_kvar));
    let ent = medel_matning_falt(korningar, "d_arm2", "frusen", |m| Some(m.entropi));
    let spegel_darm2_ok = spegel_medel(korningar, "d_arm2")
        .map(spegel_ok)
        .unwrap_or(false);
    let ln9 = (A_N as f64).ln();
    match (sta, ent) {
        (Some(sta_kvar), Some(entropi)) => {
            let stammer =
                (sta_kvar >= H2_STA_KVAR || entropi < 0.5 * ln9) && spegel_darm2_ok;
            md.push_str(&format!(
                "- **H2 (frusen stilla/upprepar):** stå kvar = {:.4} (krav ≥ {:.1}), entropi = {:.4} (krav < {:.4} = ½·ln9). Spegel d_arm2 {}. {}.\n",
                sta_kvar,
                H2_STA_KVAR,
                entropi,
                0.5 * ln9,
                if spegel_darm2_ok { "ok" } else { "utanför" },
                if stammer {
                    "Stämmer"
                } else if !spegel_darm2_ok {
                    "Tolkas inte (spegel utanför)"
                } else {
                    "Stämmer inte"
                }
            ));
        }
        _ => md.push_str("- **H2 (frusen stilla/upprepar):** saknas mätvärden.\n"),
    }

    // H3 utforskning (u2, u1)
    let u2 = hitta(&sam, "u2", false);
    let u1 = hitta(&sam, "u1", false);
    let spegel_u1_ok = spegel_medel(korningar, "u1")
        .map(spegel_ok)
        .unwrap_or(false);
    let spegel_u2_ok = spegel_medel(korningar, "u2")
        .map(spegel_ok)
        .unwrap_or(false);
    match (u2, u1) {
        (Some(u2a), Some(u1a)) => {
            let utan_forlorar = u2a.n_vunna <= VUNNA_NACKDEL;
            let lar_slump_ok = u1a.n_vunna >= 6;
            let speglar_ok = spegel_u1_ok && spegel_u2_ok;
            let stammer = utan_forlorar && lar_slump_ok && speglar_ok;
            md.push_str(&format!(
                "- **H3 (utforskning):** u2 (tabell_utan_slump mot tabell) {}/{} (krav ≤ {}); u1 (lar_slump(0.1) mot tabell) {}/{} (krav ≥ 6). Speglar {}. {}.\n",
                u2a.n_vunna,
                u2a.n_fro,
                VUNNA_NACKDEL,
                u1a.n_vunna,
                u1a.n_fro,
                if speglar_ok { "ok" } else { "utanför" },
                if stammer {
                    "Stämmer"
                } else if !speglar_ok {
                    "Tolkas inte (spegel utanför)"
                } else {
                    "Stämmer inte"
                }
            ));
        }
        _ => md.push_str("- **H3 (utforskning):** saknas.\n"),
    }

    // Diagnosarmar kort
    for namn in ["d_arm2", "d_arm1", "d_arm3", "d_tak"] {
        if let Some(a) = hitta(&sam, namn, false) {
            let spegel = spegel_medel(korningar, namn)
                .map(spegel_ok)
                .unwrap_or(false);
            md.push_str(&format!(
                "- **{}:** {} av {} frön — {}. Spegel {}.\n",
                namn,
                a.n_vunna,
                a.n_fro,
                tolka_vunna(a.n_vunna),
                if spegel { "ok" } else { "utanför" }
            ));
        }
    }

    let _ = Formaga::Lar;
    md
}

pub fn kor_diagnos(ut_dir: &Path, torr: bool) -> Result<(), String> {
    sat_nice();
    let start = Instant::now();
    let paleo = ut_dir.join("../paleokarta/data");
    let tt_path = ut_dir.join("../hckarta/tidtabell.tsv");
    let (b, h) = if torr {
        (B_TORR, H_TORR)
    } else {
        (B_FULL, H_FULL)
    };
    let jobb = bygg_jobb(torr);
    let korningar = kor_parallellt(&paleo, &tt_path, b, h, &jobb, torr)?;
    let total_s = start.elapsed().as_secs_f64();

    let jsonl_path = ut_dir.join("diagnos1207-korningar.jsonl");
    let md_path = ut_dir.join("diagnos1207-sammanfattning.md");

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let md = skriv_sammanfattning(&korningar, torr, total_s);
    print!("{md}");
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!(
        "skrev {} ({} körningar) och {} på {:.1} s",
        jsonl_path.display(),
        korningar.len(),
        md_path.display(),
        total_s
    );
    Ok(())
}
