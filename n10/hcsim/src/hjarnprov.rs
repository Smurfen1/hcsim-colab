//! Hjärnprovet Alt 1200 (+ 1201 trappa, 1202 tak): tio armar på 64×48.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{alla_armar, Formaga, HjarnArm};
use crate::sim::{MarkLage, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_FULL, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL};

/// Kartstorlek för hela provet (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
/// Torrkörning.
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

#[derive(Clone, Debug)]
pub struct Korning {
    pub arm: String,
    pub spegel: bool,
    pub fro: u64,
    pub antal_markering: usize,
    pub antal_slut: usize,
    pub markta_slut: usize,
    pub andel_markta: f64,
    pub fel_tidigt_markt: Option<f64>,
    pub fel_tidigt_omarkt: Option<f64>,
    pub fel_sent_markt: Option<f64>,
    pub fel_sent_omarkt: Option<f64>,
    pub kortid_s: f64,
    pub torr: bool,
    pub markta_formaga: String,
    pub omarkta_formaga: String,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"fel_tidigt_markt\":{},\"fel_tidigt_omarkt\":{},\"fel_sent_markt\":{},\"fel_sent_omarkt\":{},\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            self.markta_formaga,
            self.omarkta_formaga,
            fmt_opt(self.fel_tidigt_markt),
            fmt_opt(self.fel_tidigt_omarkt),
            fmt_opt(self.fel_sent_markt),
            fmt_opt(self.fel_sent_omarkt),
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
    Ok(Korning {
        arm: arm.namn.to_string(),
        spegel: arm.spegel,
        fro,
        antal_markering,
        antal_slut: sim.antal(),
        markta_slut: sim.antal_markta(),
        andel_markta: sim.andel_markta(),
        fel_tidigt_markt: sim.fel_tidigt_markt.medel(),
        fel_tidigt_omarkt: sim.fel_tidigt_omarkt.medel(),
        fel_sent_markt: sim.fel_sent_markt.medel(),
        fel_sent_omarkt: sim.fel_sent_omarkt.medel(),
        kortid_s: start.elapsed().as_secs_f64(),
        torr,
        markta_formaga: arm.markta.som_str(),
        omarkta_formaga: arm.omarkta.som_str(),
    })
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    fro: u64,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let n_fro = if torr { N_FRO_TORR } else { N_FRO_FULL };
    let mut jobb = Vec::new();
    for arm in alla_armar() {
        for fro in 1..=n_fro {
            jobb.push(Jobb {
                arm: arm.clone(),
                fro,
            });
        }
    }
    // pop tar från slutet — vänd så arm2/frö1 börjar först
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
    resultat.sort_by(|a, b| {
        (a.arm.as_str(), a.spegel, a.fro).cmp(&(b.arm.as_str(), b.spegel, b.fro))
    });
    // Körordning i underlaget: arm2, arm1, arm3, speglar, trappa, tak
    let ordning = [
        ("arm2", false),
        ("arm1", false),
        ("arm3", false),
        ("arm2", true),
        ("arm1", true),
        ("arm3", true),
        ("trappa_0.99", false),
        ("trappa_0.97", false),
        ("trappa_0.95", false),
        ("tak", false),
    ];
    resultat.sort_by_key(|k| {
        ordning
            .iter()
            .position(|&(n, s)| n == k.arm && s == k.spegel)
            .unwrap_or(99)
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
    if n >= 15 {
        "synlig fördel"
    } else if n <= 5 {
        "synlig nackdel"
    } else {
        "ingen visad skillnad"
    }
}

fn hitta<'a>(sam: &'a [ArmSam], namn: &str, spegel: bool) -> Option<&'a ArmSam> {
    sam.iter().find(|s| s.arm == namn && s.spegel == spegel)
}

/// Medel över fröna av (andel + spegelandel); spegelpar matchas på frö.
fn spegel_medel(korningar: &[Korning], arm: &str) -> Option<f64> {
    let mut sum = 0.0;
    let mut n = 0usize;
    for fro in 1..=N_FRO_FULL {
        let a = korningar
            .iter()
            .find(|k| k.arm == arm && !k.spegel && k.fro == fro && !k.torr);
        let b = korningar
            .iter()
            .find(|k| k.arm == arm && k.spegel && k.fro == fro && !k.torr);
        if let (Some(a), Some(b)) = (a, b) {
            sum += a.andel_markta + b.andel_markta;
            n += 1;
        }
    }
    if n == 0 {
        // torr: tillåt också torrkörning
        for fro in 1..=N_FRO_TORR {
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

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Hjärnprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Hjärnprovet (Alt 1200 med 1201 och 1202)\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-hjarnprov.md` (skrivna före körningen).\n\n",
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

    let arm2 = hitta(&sam, "arm2", false);
    let arm1 = hitta(&sam, "arm1", false);
    let arm3 = hitta(&sam, "arm3", false);
    let tak = hitta(&sam, "tak", false);

    // Arm 2
    if let Some(a) = arm2 {
        let tolk = tolka_vunna(a.n_vunna);
        let rott = (6..=14).contains(&a.n_vunna);
        md.push_str(&format!(
            "- **Arm 2 (frusen mot slump):** {} av {} frön — {}.{}\n",
            a.n_vunna,
            a.n_fro,
            tolk,
            if rott {
                " **Rött:** hjärnans beteende styr inget i världen; arm 1 och 3 tolkas inte."
            } else {
                ""
            }
        ));
    } else {
        md.push_str("- **Arm 2:** saknas.\n");
    }

    // Speglar
    for namn in ["arm2", "arm1", "arm3"] {
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

    let arm2_gron = arm2.map(|a| !(6..=14).contains(&a.n_vunna)).unwrap_or(false);
    let spegel1_ok = spegel_medel(korningar, "arm1")
        .map(spegel_ok)
        .unwrap_or(false);
    let spegel2_ok = spegel_medel(korningar, "arm2")
        .map(spegel_ok)
        .unwrap_or(false);
    let spegel3_ok = spegel_medel(korningar, "arm3")
        .map(spegel_ok)
        .unwrap_or(false);

    // Arm 1
    if let Some(a) = arm1 {
        let tolk = tolka_vunna(a.n_vunna);
        let tolkas = arm2_gron && spegel1_ok && spegel2_ok;
        md.push_str(&format!(
            "- **Arm 1 (lärande mot frusen):** {} av {} frön — {}. {}\n",
            a.n_vunna,
            a.n_fro,
            tolk,
            if tolkas {
                "Tolkas."
            } else {
                "Tolkas inte (arm 2 röd eller spegel utanför)."
            }
        ));
    }

    // Arm 3
    if let Some(a) = arm3 {
        let tolk = tolka_vunna(a.n_vunna);
        let tolkas = arm2_gron && spegel3_ok && spegel2_ok;
        md.push_str(&format!(
            "- **Arm 3 (lärande mot tabell):** {} av {} frön — {}. {}\n",
            a.n_vunna,
            a.n_fro,
            tolk,
            if tolkas {
                "Tolkas."
            } else {
                "Tolkas inte (arm 2 röd eller spegel utanför)."
            }
        ));
    }

    // Trappa
    let t99 = hitta(&sam, "trappa_0.99", false);
    let t97 = hitta(&sam, "trappa_0.97", false);
    let t95 = hitta(&sam, "trappa_0.95", false);
    if let (Some(a), Some(b), Some(c)) = (t99, t97, t95) {
        let v = [a.n_vunna, b.n_vunna, c.n_vunna];
        let kostnader = [0.99_f64, 0.97, 0.95];
        let monoton = v[0] <= v[1] && v[1] <= v[2];
        let grans = kostnader
            .iter()
            .zip(v.iter())
            .find(|(_, &n)| n >= 15)
            .map(|(k, _)| format!("{k}"))
            .unwrap_or_else(|| "ingen kostnadsfördel gav minst 15".into());
        md.push_str(&format!(
            "- **Trappa (0,99 / 0,97 / 0,95):** vunna {} / {} / {}. Instrumentets gräns: {}. {}\n",
            v[0],
            v[1],
            v[2],
            grans,
            if monoton {
                "Följden är monoton."
            } else {
                "**Rött:** följden är inte monoton (större fördel gav färre vunna)."
            }
        ));
    } else {
        md.push_str("- **Trappa:** saknas.\n");
    }

    // Tak
    if let Some(a) = tak {
        let ok = a.n_vunna >= 15;
        md.push_str(&format!(
            "- **Tak (allvetande mot slump):** {} av {} frön. {}\n",
            a.n_vunna,
            a.n_fro,
            if ok {
                "Världen belönar kunskap om växterna (≥15)."
            } else {
                "Allvetande ger inte minst 15: världen belönar inte kunskap om växterna; nej i arm 1 och 3 kan bero på världen."
            }
        ));
    }

    // Världsmodell (regel 14)
    let arm1_fordel = arm1.map(|a| a.n_vunna >= 15).unwrap_or(false);
    let arm3_fordel = arm3.map(|a| a.n_vunna >= 15).unwrap_or(false);
    let varldsmodell = arm2_gron
        && arm1_fordel
        && arm3_fordel
        && spegel1_ok
        && spegel2_ok
        && spegel3_ok;
    md.push('\n');
    if varldsmodell {
        md.push_str(
            "**Ord:** världsmodell — arm 1 och arm 3 ger synlig fördel för den lärande hjärnan och arm 2 är grön (regel 14).\n",
        );
    } else {
        md.push_str("**Ord:** inte visat — villkoren för ordet världsmodell är inte uppfyllda (regel 14).\n");
    }

    // tyst referens så Formaga-importen används om vi utökar
    let _ = Formaga::Lar;
    md
}

pub fn kor_hjarnprov(ut_dir: &Path, torr: bool) -> Result<(), String> {
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

    let jsonl_path = ut_dir.join("hjarnprov-korningar.jsonl");
    let md_path = ut_dir.join("hjarnprov-sammanfattning.md");

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
