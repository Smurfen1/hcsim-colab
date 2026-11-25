//! Rangprovet (Alt 1228 med trängselmätningen Alt 1229 och trängseloraklet Alt 1230).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{Formaga, HjarnArm};
use crate::minstakvadrat::{self, G1Vikter, RangFroSam};
use crate::sim::{FormagaMatning, LedLogg, MarkLage, Matning, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL};

/// Kartstorlek för hela rangprovet (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön 161–180 (underlaget).
pub const FRO_START: u64 = 161;
pub const N_FRO_FULL: u64 = 20;

/// Ledprovets anpassningsfrön (G1; bara läsas).
pub const LED_ANP_START: u64 = 141;
pub const LED_ANP_N: u64 = 10;

/// Gränser ur underlaget (skrivna före körningen).
pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
pub const R_TRAFF_SAKNAR: f64 = 0.2;
pub const R_SPEARMAN_SAKNAR: f64 = 0.1;
pub const R_TRAFF_BAR: f64 = 0.5;
pub const KLUMP_MEDATARE_FAKTOR: f64 = 1.2;

pub const ARM_ORDNING: [&str; 3] = ["t_tab", "t_ork", "t_to"];

/// Standardväg till ledprovets logg (anpassning; läses bara).
pub const LED_LOGG_VAG: &str = "/home/hakan/sandbox/pi/work/hcsim/ledprov1221-logg.tsv";

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
    pub logg_rader: usize,
    pub matning: Vec<MatningRad>,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let mat: Vec<String> = self.matning.iter().map(|m| m.till_json()).collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"logg_rader\":{},\"matning\":[{}],\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            self.markta_formaga,
            self.omarkta_formaga,
            self.logg_rader,
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

fn fro_lista(torr: bool) -> Vec<u64> {
    let n = if torr { N_FRO_TORR } else { N_FRO_FULL };
    (0..n).map(|i| FRO_START + i).collect()
}

/// De tre basarmarna (underlaget) + speglar.
pub fn rang_armar() -> Vec<HjarnArm> {
    let bas = [
        HjarnArm {
            namn: "t_tab",
            markta: Formaga::Tabell,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "t_ork",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "t_to",
            markta: Formaga::TrangselOrakel,
            omarkta: Formaga::Slump,
            spegel: false,
        },
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for a in &bas {
        ut.push(a.clone());
    }
    for a in &bas {
        ut.push(a.clone().spegel_av());
    }
    ut
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    logg: bool,
    fro: u64,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let fron = fro_lista(torr);
    let mut jobb = Vec::new();
    for arm in rang_armar() {
        for &fro in &fron {
            jobb.push(Jobb {
                arm: arm.clone(),
                logg: false,
                fro,
            });
        }
    }
    for &fro in &fron {
        jobb.push(Jobb {
            arm: HjarnArm {
                namn: "r_lar",
                markta: Formaga::Lar,
                omarkta: Formaga::Lar,
                spegel: false,
            },
            logg: true,
            fro,
        });
    }
    jobb.reverse();
    jobb
}

fn bygg_sim(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    arm: &HjarnArm,
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
    sim.matning = Matning {
        pa: true,
        ..Matning::default()
    };
    if logg {
        sim.ledlogg = LedLogg {
            pa: true,
            fro,
            ..LedLogg::default()
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
    logg: bool,
    torr: bool,
) -> Result<(Korning, Vec<crate::sim::LedLoggRad>), String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm, logg)?;
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    sim.markera_halften(fro);
    let antal_markering = sim.antal();
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, MARKT_EFTER_AR);
    sim.matning.krav_data()?;
    if logg {
        sim.ledlogg.krav_data()?;
    }
    let mut matning: Vec<MatningRad> = sim
        .matning
        .per
        .iter()
        .map(|(k, v)| MatningRad::fran(k, v))
        .collect();
    matning.sort_by(|a, b| a.formaga.cmp(&b.formaga));
    let logg_rader = sim.ledlogg.rader.len();
    let logg_ut = if logg {
        std::mem::take(&mut sim.ledlogg.rader)
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
            logg_rader,
            matning,
        },
        logg_ut,
    ))
}

pub fn kor_parallellt(
    paleo: &Path,
    tt_path: &Path,
    b: usize,
    h: usize,
    jobb: &[Jobb],
    torr: bool,
) -> Result<(Vec<Korning>, Vec<crate::sim::LedLoggRad>), String> {
    if jobb.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let ko: Arc<Mutex<Vec<Jobb>>> = Arc::new(Mutex::new(jobb.to_vec()));
    let ut: Arc<Mutex<Vec<Result<(Korning, Vec<crate::sim::LedLoggRad>), String>>>> =
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
                let nasta = {
                    let mut g = ko.lock().unwrap();
                    g.pop()
                };
                let Some(j) = nasta else { break };
                let r = kor_en(&kalla, &tt, b, h, j.fro, &j.arm, j.logg, torr);
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let mut korningar = Vec::new();
    let mut alla_logg = Vec::new();
    for r in ut.lock().unwrap().drain(..) {
        let (k, logg) = r?;
        alla_logg.extend(logg);
        korningar.push(k);
    }
    korningar.sort_by(|a, b| {
        let ka = if a.arm == "r_lar" {
            100
        } else {
            ARM_ORDNING
                .iter()
                .position(|&n| n == a.arm)
                .map(|i| i * 2 + if a.spegel { 1 } else { 0 })
                .unwrap_or(99)
        };
        let kb = if b.arm == "r_lar" {
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
    Ok((korningar, alla_logg))
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
        if k.arm == "r_lar" {
            continue;
        }
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

fn mat_for<'a>(k: &'a Korning, formaga: &str) -> Option<&'a MatningRad> {
    k.matning.iter().find(|m| m.formaga == formaga)
}

/// Klumpning per frö: märkt har ≥20 % fler medätare och lägre intag än slump.
fn klump_fro(k: &Korning) -> Option<bool> {
    let mark = mat_for(k, &k.markta_formaga)?;
    let slump = mat_for(k, "slump")?;
    let (mm, ms) = (mark.medatare?, slump.medatare?);
    let (im, is_) = (mark.intag_per_steg?, slump.intag_per_steg?);
    Some(mm >= KLUMP_MEDATARE_FAKTOR * ms && im < is_)
}

fn rakna_klump(korningar: &[Korning], arm: &str) -> (usize, usize) {
    let rader: Vec<&Korning> = korningar
        .iter()
        .filter(|k| k.arm == arm && !k.spegel)
        .collect();
    let mut n_ok = 0usize;
    for k in &rader {
        if klump_fro(k) == Some(true) {
            n_ok += 1;
        }
    }
    (n_ok, rader.len())
}

fn avgor_r(per_fro: &[RangFroSam]) -> (usize, usize, usize, &'static str) {
    let n = per_fro.len();
    let n_saknar = per_fro
        .iter()
        .filter(|f| f.traff < R_TRAFF_SAKNAR && f.spearman < R_SPEARMAN_SAKNAR)
        .count();
    let n_bar = per_fro
        .iter()
        .filter(|f| f.traff >= R_TRAFF_BAR)
        .count();
    let text = if n_saknar >= VUNNA_FORDEL {
        "träff under 0,2 och rangkorrelation under 0,1 i minst 15 av 20 frön: hjärnan rangordnar inte maten. Konkret fel i hjärnans värdering (Alt 1212); B får öppnas."
    } else if n_bar >= VUNNA_FORDEL {
        "träff minst 0,5 i minst 15 av 20 frön: hjärnan rangordnar maten rätt; B förblir stängd, och felet ligger i vad som rangordnas (till exempel trängseln)."
    } else {
        "ingen visad skillnad."
    };
    (n_saknar, n_bar, n, text)
}

pub fn skriv_sammanfattning(
    korningar: &[Korning],
    torr: bool,
    total_s: f64,
    rang_fro: Option<&[RangFroSam]>,
    rang_fel: Option<&str>,
    g1: Option<&G1Vikter>,
) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Rangprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str(
            "# Rangprovet (Alt 1228, med trängselmätningen Alt 1229 och trängseloraklet Alt 1230)\n\n",
        );
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-rang1228.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str(
        "| Arm | Spegel | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |\n",
    );
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
    let n_rlar = korningar.iter().filter(|k| k.arm == "r_lar").count();
    let logg_tot: usize = korningar
        .iter()
        .filter(|k| k.arm == "r_lar")
        .map(|k| k.logg_rader)
        .sum();
    md.push_str(&format!(
        "\nLoggkörning `r_lar`: {n_rlar} frön, {logg_tot} loggrader totalt.\n\n"
    ));

    // Trängselmätning per arm (utan spegel)
    md.push_str("## Trängselmätning (Alt 1229, utan spegel)\n\n");
    md.push_str("| Arm | Frö | Märkt medätare | Slump medätare | Märkt intag | Slump intag | Märkt knapp | Slump knapp |\n");
    md.push_str("|-----|-----|----------------|----------------|-------------|-------------|-------------|-------------|\n");
    for arm in ARM_ORDNING {
        for k in korningar.iter().filter(|k| k.arm == arm && !k.spegel) {
            let mark = mat_for(k, &k.markta_formaga);
            let slump = mat_for(k, "slump");
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                arm,
                k.fro,
                fmt_opt(mark.and_then(|m| m.medatare)),
                fmt_opt(slump.and_then(|m| m.medatare)),
                fmt_opt(mark.and_then(|m| m.intag_per_steg)),
                fmt_opt(slump.and_then(|m| m.intag_per_steg)),
                fmt_opt(mark.and_then(|m| m.andel_knapp)),
                fmt_opt(slump.and_then(|m| m.andel_knapp)),
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

    // R
    md.push_str("- **R (Alt 1228):** ");
    if let Some(e) = rang_fel {
        md.push_str(&format!("kunde inte räknas: {e}\n"));
    } else if let Some(pf) = rang_fro {
        let (n_saknar, n_bar, n, text) = avgor_r(pf);
        let medel_t = pf.iter().map(|f| f.traff).sum::<f64>() / n.max(1) as f64;
        let medel_s = pf.iter().map(|f| f.spearman).filter(|x| x.is_finite()).sum::<f64>()
            / pf.iter().filter(|f| f.spearman.is_finite()).count().max(1) as f64;
        md.push_str(&format!(
            "medel träff = {:.4}, medel rangkorrelation = {:.4}; frön med träff<0,2 och Spearman<0,1: {n_saknar}/{n}; frön med träff≥0,5: {n_bar}/{n}. {text}\n",
            medel_t, medel_s
        ));
        if let Some(g1) = g1 {
            for h in 0..crate::tal::A_N {
                if g1.singulaer[h] {
                    md.push_str(&format!(
                        "  - G1 handling {h}: SINGULÄR — {}\n",
                        g1.meddelande[h]
                    ));
                } else {
                    md.push_str(&format!(
                        "  - G1 handling {h}: n_anp={}\n",
                        g1.n_anp[h]
                    ));
                }
            }
        }
        for f in pf {
            md.push_str(&format!(
                "  - frö {}: träff={:.4}, Spearman={:.4} (n_djur={}, n_rader={})\n",
                f.fro, f.traff, f.spearman, f.n_djur, f.n_rader
            ));
        }
    } else {
        md.push_str("saknas.\n");
    }

    // Klumpningen
    let (kt, nt) = rakna_klump(korningar, "t_tab");
    let (ko, no) = rakna_klump(korningar, "t_ork");
    let klump_star = kt >= VUNNA_FORDEL && ko >= VUNNA_FORDEL && nt > 0 && no > 0;
    let klump_text = if klump_star {
        "står (giriga gruppen har minst 20 % fler medätare och lägre intag i minst 15 av 20 frön i båda armarna)"
    } else {
        "stryks, och orsaken till slumpens seger är okänd"
    };
    md.push_str(&format!(
        "- **Klumpningen (Alt 1229):** t_tab {kt}/{nt}, t_ork {ko}/{no} (≥{VUNNA_FORDEL} i båda). {klump_text}.\n"
    ));

    // Trängseloraklet
    let t_to = hitta(&sam, "t_to", false);
    let sp_to = spegel_medel(korningar, "t_to")
        .map(spegel_ok)
        .unwrap_or(false);
    let (v, n) = match t_to {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let to_text = if !sp_to {
        "Tolkas inte (spegel utanför)"
    } else if v >= VUNNA_FORDEL {
        "t_to minst 15 av 20: sinnena räcker när trängseln räknas in, och `v` väger inte antalet."
    } else if v <= VUNNA_NACKDEL {
        "högst 5 av 20: uppgiften om samtidiga flyttar saknas i sinnena; då hjälper ingen kostnad, och frågan blir blandade val eller världens bete"
    } else {
        "6–14: ingen visad skillnad."
    };
    md.push_str(&format!(
        "- **Trängseloraklet (Alt 1230):** t_to {v}/{n}. Spegel {}. {}.\n",
        if sp_to { "ok" } else { "utanför" },
        to_text
    ));

    md
}

pub fn kor_rang(ut_dir: &Path, torr: bool) -> Result<(), String> {
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
    let (korningar, logg) = kor_parallellt(&paleo, &tt_path, b, h, &jobb, torr)?;
    let total_s = start.elapsed().as_secs_f64();

    let jsonl_path = ut_dir.join("rang1228-korningar.jsonl");
    let logg_path = ut_dir.join("rang1228-logg.tsv");
    let md_path = ut_dir.join("rang1228-sammanfattning.md");

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let mut tsv = minstakvadrat::rang_tsv_huvud();
    for r in &logg {
        tsv.push_str(&minstakvadrat::rang_tsv_rad(r));
    }
    std::fs::write(&logg_path, &tsv)
        .map_err(|e| format!("skriv {}: {e}", logg_path.display()))?;

    // G1 från ledprovets logg (bara läsas); rang från vår logg.
    let led_vag = std::env::var("HCSIM_LED_LOGG").unwrap_or_else(|_| LED_LOGG_VAG.to_string());
    let anp: Vec<u64> = (LED_ANP_START..LED_ANP_START + LED_ANP_N).collect();
    let (rang_fro, rang_fel, g1) = match rakna_rang_fran_filer(&led_vag, &logg, &anp) {
        Ok((pf, g1)) => (Some(pf), None, Some(g1)),
        Err(e) => (None, Some(e), None),
    };

    let md = skriv_sammanfattning(
        &korningar,
        torr,
        total_s,
        rang_fro.as_deref(),
        rang_fel.as_deref(),
        g1.as_ref(),
    );
    print!("{md}");
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!(
        "skrev {} ({} körningar), {} ({} loggrader) och {} på {:.1} s",
        jsonl_path.display(),
        korningar.len(),
        logg_path.display(),
        logg.len(),
        md_path.display(),
        total_s
    );
    Ok(())
}

fn rakna_rang_fran_filer(
    led_vag: &str,
    rang_logg: &[crate::sim::LedLoggRad],
    anp: &[u64],
) -> Result<(Vec<RangFroSam>, G1Vikter), String> {
    let text = std::fs::read_to_string(led_vag)
        .map_err(|e| format!("kunde inte läsa ledprovsloggen {led_vag}: {e}"))?;
    let led = minstakvadrat::las_tsv(&text)?;
    if led.is_empty() {
        return Err("tom ledprovslogg".into());
    }
    let g1 = minstakvadrat::anpassa_g1(&led, anp);
    let per_rad = minstakvadrat::rang_per_rad(rang_logg, &g1)?;
    Ok((minstakvadrat::rang_per_fro(&per_rad), g1))
}
