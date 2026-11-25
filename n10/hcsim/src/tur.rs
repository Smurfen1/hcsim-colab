//! Turordningsprovet (Alt 1246) med omläsningen av rangloggen (Alt 1245).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{Formaga, HjarnArm};
use crate::minstakvadrat::{self, G1Vikter, OmlasFroSam};
use crate::rang::{LED_ANP_N, LED_ANP_START, LED_LOGG_VAG};
use crate::sim::{FormagaMatning, MarkLage, Matning, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL};

/// Kartstorlek för hela turprovet (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön 181–200 (underlaget).
pub const FRO_START: u64 = 181;
pub const N_FRO_FULL: u64 = 20;

/// Gränser ur underlaget (skrivna före körningen).
pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
/// Alt 1245: rangkorrelation bland flyttarna under denna i minst 15 av 20 frön.
pub const OMLAS_SPEARMAN_UNDER: f64 = -0.05;

pub const ARM_ORDNING: [&str; 6] = ["u_tab", "u_to", "u_ork", "u_lar", "s_tab", "s_to"];

/// Standardväg till rangprovets logg (omläsning; läses bara).
pub const RANG_LOGG_VAG: &str = "/home/hakan/sandbox/pi/work/hcsim/rang1228-logg.tsv";

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
    pub turordning: bool,
    pub matning: Vec<MatningRad>,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let mat: Vec<String> = self.matning.iter().map(|m| m.till_json()).collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"turordning\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"matning\":[{}],\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            if self.turordning { "true" } else { "false" },
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

/// De sex basarmarna (underlaget) + speglar.
pub fn tur_armar() -> Vec<(HjarnArm, bool)> {
    let bas: [(&str, Formaga, Formaga, bool); 6] = [
        ("u_tab", Formaga::Tabell, Formaga::Slump, true),
        ("u_to", Formaga::TrangselOrakel, Formaga::Slump, true),
        ("u_ork", Formaga::OrakelForutsagare, Formaga::Slump, true),
        ("u_lar", Formaga::Lar, Formaga::Slump, true),
        ("s_tab", Formaga::Tabell, Formaga::Slump, false),
        ("s_to", Formaga::TrangselOrakel, Formaga::Slump, false),
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for &(namn, markta, omarkta, tur) in &bas {
        let arm = HjarnArm {
            namn,
            markta,
            omarkta,
            spegel: false,
        };
        ut.push((arm.clone(), tur));
        ut.push((arm.spegel_av(), tur));
    }
    ut
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    turordning: bool,
    fro: u64,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let fron = fro_lista(torr);
    let mut jobb = Vec::new();
    for (arm, tur) in tur_armar() {
        for &fro in &fron {
            jobb.push(Jobb {
                arm: arm.clone(),
                turordning: tur,
                fro,
            });
        }
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
    turordning: bool,
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
    sim.turordning = turordning;
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
    turordning: bool,
    torr: bool,
) -> Result<Korning, String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm, turordning)?;
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
        turordning,
        matning,
    })
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
                let r = kor_en(&kalla, &tt, b, h, j.fro, &j.arm, j.turordning, torr);
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let mut korningar = Vec::new();
    for r in ut.lock().unwrap().drain(..) {
        korningar.push(r?);
    }
    korningar.sort_by(|a, b| {
        let ka = ARM_ORDNING
            .iter()
            .position(|&n| n == a.arm)
            .map(|i| i * 2 + if a.spegel { 1 } else { 0 })
            .unwrap_or(99);
        let kb = ARM_ORDNING
            .iter()
            .position(|&n| n == b.arm)
            .map(|i| i * 2 + if b.spegel { 1 } else { 0 })
            .unwrap_or(99);
        ka.cmp(&kb).then(a.fro.cmp(&b.fro))
    });
    Ok(korningar)
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
    pub turordning: bool,
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
            turordning: rader[0].turordning,
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

fn avgor_klockan(sam: &[ArmSam], korningar: &[Korning]) -> String {
    let u_tab = hitta(sam, "u_tab", false);
    let u_to = hitta(sam, "u_to", false);
    let sp_tab = spegel_medel(korningar, "u_tab")
        .map(spegel_ok)
        .unwrap_or(false);
    let sp_to = spegel_medel(korningar, "u_to")
        .map(spegel_ok)
        .unwrap_or(false);
    let (vt, nt) = match u_tab {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let (vo, no) = match u_to {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let tab_ok = sp_tab && vt >= VUNNA_FORDEL;
    let to_ok = sp_to && vo >= VUNNA_FORDEL;
    let tab_nack = sp_tab && vt <= VUNNA_NACKDEL;
    let to_nack = sp_to && vo <= VUNNA_NACKDEL;
    let text = if tab_ok || to_ok {
        "u_tab eller u_to minst 15 av 20 (spegel inom gränsen): klumpningen är en artefakt av samtidiga flyttar. Om turordning blir världens regel beslutas i en ny omgång."
    } else if tab_nack && to_nack {
        "u_tab och u_to båda högst 5 av 20: spridningen måste komma ur djuret (Alt 1244 eller H)."
    } else {
        "ingen visad skillnad."
    };
    format!(
        "u_tab {vt}/{nt} (spegel {}), u_to {vo}/{no} (spegel {}). {text}",
        if sp_tab { "ok" } else { "utanför" },
        if sp_to { "ok" } else { "utanför" },
    )
}

fn avgor_kontroll(sam: &[ArmSam], korningar: &[Korning]) -> String {
    let s_tab = hitta(sam, "s_tab", false);
    let s_to = hitta(sam, "s_to", false);
    let sp_tab = spegel_medel(korningar, "s_tab")
        .map(spegel_ok)
        .unwrap_or(false);
    let sp_to = spegel_medel(korningar, "s_to")
        .map(spegel_ok)
        .unwrap_or(false);
    let (vt, nt) = match s_tab {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let (vo, no) = match s_to {
        Some(a) => (a.n_vunna, a.n_fro),
        None => (0, 0),
    };
    let upprepar = sp_tab && sp_to && vt == 0 && vo == 0 && nt > 0 && no > 0;
    let text = if upprepar {
        "s_tab och s_to upprepar rangprovets 0 av 20; u-armarna tolkas."
    } else {
        "s_tab och s_to upprepar inte rangprovets 0 av 20; u-armarna tolkas inte."
    };
    format!(
        "s_tab {vt}/{nt} (spegel {}), s_to {vo}/{no} (spegel {}). {text}",
        if sp_tab { "ok" } else { "utanför" },
        if sp_to { "ok" } else { "utanför" },
    )
}

fn avgor_omlas(per_fro: &[OmlasFroSam]) -> String {
    let n = per_fro.len();
    let n_under = per_fro
        .iter()
        .filter(|f| f.flytt_spearman.is_some_and(|s| s < OMLAS_SPEARMAN_UNDER))
        .count();
    let text = if n_under >= VUNNA_FORDEL {
        "rangkorrelationen bland flyttarna under −0,05 i minst 15 av 20 frön: hjärnan värderar flyttarna fel, och B får ett mål."
    } else {
        "rangkorrelationen bland flyttarna under −0,05 i färre än 15 av 20 frön: B har inget mål ännu."
    };
    format!(
        "frön med flytt-Spearman < {OMLAS_SPEARMAN_UNDER}: {n_under}/{n}. {text}"
    )
}

pub fn skriv_sammanfattning(
    korningar: &[Korning],
    torr: bool,
    total_s: f64,
    omlas: Option<&[OmlasFroSam]>,
    omlas_fel: Option<&str>,
    g1: Option<&G1Vikter>,
) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Turordningsprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str(
            "# Turordningsprovet (Alt 1246, med omläsningen Alt 1245)\n\n",
        );
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-tur1246.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str(
        "| Arm | Spegel | Tur | Märkt | Omärkt | Vunna (andel>0,5) | Medel | Min | Max |\n",
    );
    md.push_str(
        "|-----|--------|-----|-------|--------|-------------------|-------|-----|-----|\n",
    );
    for s in &sam {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {}/{} | {:.4} | {:.4} | {:.4} |\n",
            s.arm,
            if s.spegel { "ja" } else { "nej" },
            if s.turordning { "ja" } else { "nej" },
            s.markta_formaga,
            s.omarkta_formaga,
            s.n_vunna,
            s.n_fro,
            s.medel_andel,
            s.min_andel,
            s.max_andel,
        ));
    }

    md.push_str("\n## Trängselmätning (utan spegel)\n\n");
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

    md.push_str(&format!("- **Klockan:** {}\n", avgor_klockan(&sam, korningar)));
    md.push_str(&format!("- **Kontrollen:** {}\n", avgor_kontroll(&sam, korningar)));

    // u_ork och u_lar redovisas (ingen hypotes)
    for namn in ["u_ork", "u_lar"] {
        if let Some(a) = hitta(&sam, namn, false) {
            md.push_str(&format!(
                "- **{namn} (redovisning):** {}/{} vunna, medel andel {:.4}.\n",
                a.n_vunna, a.n_fro, a.medel_andel
            ));
        }
    }

    md.push_str("- **Alt 1245:** ");
    if let Some(e) = omlas_fel {
        md.push_str(&format!("kunde inte räknas: {e}\n"));
    } else if let Some(pf) = omlas {
        md.push_str(&format!("{}\n", avgor_omlas(pf)));
        if let Some(g1) = g1 {
            for h in 0..crate::tal::A_N {
                if g1.singulaer[h] {
                    md.push_str(&format!(
                        "  - G1 handling {h}: SINGULÄR — {}\n",
                        g1.meddelande[h]
                    ));
                } else {
                    md.push_str(&format!("  - G1 handling {h}: n_anp={}\n", g1.n_anp[h]));
                }
            }
        }
        for f in pf {
            let ft = match f.flytt_traff {
                Some(t) => format!("{t:.4}"),
                None => "saknas".into(),
            };
            let fs = match f.flytt_spearman {
                Some(s) => format!("{s:.4}"),
                None => "saknas".into(),
            };
            md.push_str(&format!(
                "  - frö {}: flyttträff={}, flytt-Spearman={}, andel_g1_sta={:.4}, intag_minus_g1medel={:.4} (n_djur={}, n_flytt={}, n_rader={})\n",
                f.fro,
                ft,
                fs,
                f.andel_g1_sta_kvar,
                f.intag_minus_g1_medel,
                f.n_djur,
                f.n_flytt_rader,
                f.n_rader
            ));
        }
    } else {
        md.push_str("saknas.\n");
    }

    md
}

pub fn kor_tur(ut_dir: &Path, torr: bool) -> Result<(), String> {
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

    let jsonl_path = ut_dir.join("tur1246-korningar.jsonl");
    let omlas_path = ut_dir.join("tur1246-omlas.tsv");
    let md_path = ut_dir.join("tur1246-sammanfattning.md");

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let led_vag = std::env::var("HCSIM_LED_LOGG").unwrap_or_else(|_| LED_LOGG_VAG.to_string());
    let rang_vag = std::env::var("HCSIM_RANG_LOGG").unwrap_or_else(|_| RANG_LOGG_VAG.to_string());
    let anp: Vec<u64> = (LED_ANP_START..LED_ANP_START + LED_ANP_N).collect();
    let (omlas, omlas_fel, g1) = match rakna_omlas_fran_filer(&led_vag, &rang_vag, &anp) {
        Ok((pf, g1, tsv)) => {
            std::fs::write(&omlas_path, &tsv)
                .map_err(|e| format!("skriv {}: {e}", omlas_path.display()))?;
            (Some(pf), None, Some(g1))
        }
        Err(e) => (None, Some(e), None),
    };

    let md = skriv_sammanfattning(
        &korningar,
        torr,
        total_s,
        omlas.as_deref(),
        omlas_fel.as_deref(),
        g1.as_ref(),
    );
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

fn rakna_omlas_fran_filer(
    led_vag: &str,
    rang_vag: &str,
    anp: &[u64],
) -> Result<(Vec<OmlasFroSam>, G1Vikter, String), String> {
    let led_text = std::fs::read_to_string(led_vag)
        .map_err(|e| format!("kunde inte läsa ledprovsloggen {led_vag}: {e}"))?;
    let led = minstakvadrat::las_tsv(&led_text)?;
    if led.is_empty() {
        return Err("tom ledprovslogg".into());
    }
    let rang_text = std::fs::read_to_string(rang_vag)
        .map_err(|e| format!("kunde inte läsa rangloggen {rang_vag}: {e}"))?;
    let rang = minstakvadrat::las_rang_tsv(&rang_text)?;
    if rang.is_empty() {
        return Err("tom ranglogg".into());
    }
    let g1 = minstakvadrat::anpassa_g1(&led, anp);
    let per_rad = minstakvadrat::omlas_per_rad(&rang, &g1)?;
    let per_fro = minstakvadrat::omlas_per_fro(&per_rad)?;
    let mut tsv = String::from(
        "fro\tflytt_traff\tflytt_spearman\tandel_g1_sta_kvar\tintag_minus_g1_medel\tn_djur\tn_flytt_rader\tn_rader\n",
    );
    for f in &per_fro {
        let ft = f
            .flytt_traff
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "saknas".into());
        let fs = f
            .flytt_spearman
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "saknas".into());
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            f.fro,
            ft,
            fs,
            f.andel_g1_sta_kvar,
            f.intag_minus_g1_medel,
            f.n_djur,
            f.n_flytt_rader,
            f.n_rader
        ));
    }
    Ok((per_fro, g1, tsv))
}
