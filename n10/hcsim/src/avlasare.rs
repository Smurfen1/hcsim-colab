//! Avläsarprovet (Alt 1213 med 1218, 1215 och 1214): 10 armar + speglar, H4/H4b/H6/H5.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{avlasare_armar, Formaga, HjarnArm};
use crate::sim::{FormagaMatning, MarkLage, Matning, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL, A_N};

/// Kartstorlek för hela avläsarprovet (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön 121–140 (underlaget).
pub const FRO_START: u64 = 121;
pub const N_FRO_FULL: u64 = 20;

/// Gränser ur underlaget (skrivna före körningen).
pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
pub const H5_FLYTT_MIN: f64 = 0.8;
pub const H5_STA_MAX: f64 = 0.5;

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
    pub fel_tidigt_per: [Option<f64>; A_N],
    pub fel_sent_per: [Option<f64>; A_N],
    pub andel_v_annorlunda: Option<f64>,
    pub djursteg: u64,
}

impl MatningRad {
    fn fran(formaga: &str, m: &FormagaMatning) -> Self {
        let mut andelar = [0.0; A_N];
        let mut fel_tidigt_per = [None; A_N];
        let mut fel_sent_per = [None; A_N];
        for a in 0..A_N {
            andelar[a] = m.andel_handling(a);
            fel_tidigt_per[a] = m.fel_tidigt_handling(a);
            fel_sent_per[a] = m.fel_sent_handling(a);
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
            fel_tidigt_per,
            fel_sent_per,
            andel_v_annorlunda: m.andel_v_annorlunda(),
            djursteg: m.djursteg,
        }
    }

    fn till_json(&self) -> String {
        let andelar: Vec<String> = self.andelar.iter().map(|x| fmt_f64(*x)).collect();
        let ftp: Vec<String> = self.fel_tidigt_per.iter().map(|x| fmt_opt(*x)).collect();
        let fsp: Vec<String> = self.fel_sent_per.iter().map(|x| fmt_opt(*x)).collect();
        format!(
            "{{\"formaga\":\"{}\",\"andelar\":[{}],\"entropi\":{},\"andel_sta_kvar\":{},\"intag_per_steg\":{},\"e_vid_dod\":{},\"dod_svalt\":{},\"dod_alder\":{},\"andra_medel\":{},\"fel_tidigt\":{},\"fel_sent\":{},\"fel_tidigt_per\":[{}],\"fel_sent_per\":[{}],\"andel_v_annorlunda\":{},\"djursteg\":{}}}",
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
            ftp.join(","),
            fsp.join(","),
            fmt_opt(self.andel_v_annorlunda),
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
    for arm in avlasare_armar() {
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

pub const ARM_ORDNING: [&str; 10] = [
    "a_lott",
    "a_las",
    "a_las_lott",
    "a_las_tab",
    "a_las_ls",
    "a_orakel",
    "s_lar",
    "s_tab",
    "s_tab_tak",
    "s_lar_tak",
];

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

fn avgor_arm(sam: &[ArmSam], korningar: &[Korning], namn: &str) -> (usize, usize, bool, &'static str) {
    let a = hitta(sam, namn, false);
    let spegel = spegel_medel(korningar, namn)
        .map(spegel_ok)
        .unwrap_or(false);
    match a {
        Some(a) => (a.n_vunna, a.n_fro, spegel, tolka_vunna(a.n_vunna)),
        None => (0, 0, false, "saknas"),
    }
}

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Avläsarprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Avläsarprovet (Alt 1213 med 1218, 1215 och 1214)\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-avlasare1213.md` (skrivna före körningen).\n\n",
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

    // H4
    let (las_v, las_n, las_sp, _) = avgor_arm(&sam, korningar, "a_las");
    let (las_lott_v, las_lott_n, las_lott_sp, _) = avgor_arm(&sam, korningar, "a_las_lott");
    let h4_spegel = las_sp && las_lott_sp;
    let h4_stammer = las_v >= VUNNA_FORDEL && las_lott_v >= VUNNA_FORDEL && h4_spegel;
    md.push_str(&format!(
        "- **H4 (avläsaren är felet):** a_las (lar_intagslasare mot lar) {}/{} (≥{}); a_las_lott (mot lar_lott) {}/{}. Speglar {}. {}.\n",
        las_v,
        las_n,
        VUNNA_FORDEL,
        las_lott_v,
        las_lott_n,
        if h4_spegel { "ok" } else { "utanför" },
        if h4_stammer {
            "Stämmer"
        } else if !h4_spegel {
            "Tolkas inte (spegel utanför)"
        } else {
            "Stämmer inte"
        }
    ));

    // H4b
    let (lott_v, lott_n, lott_sp, _) = avgor_arm(&sam, korningar, "a_lott");
    let h4b_stammer = lott_v >= VUNNA_FORDEL && lott_sp;
    md.push_str(&format!(
        "- **H4b (regeln för lika värden):** a_lott (lar_lott mot lar) {}/{} (≥{}). Spegel {}. {}.\n",
        lott_v,
        lott_n,
        VUNNA_FORDEL,
        if lott_sp { "ok" } else { "utanför" },
        if h4b_stammer {
            "Stämmer"
        } else if !lott_sp {
            "Tolkas inte (spegel utanför)"
        } else {
            "Stämmer inte"
        }
    ));

    // H6
    let (ora_v, ora_n, ora_sp, _) = avgor_arm(&sam, korningar, "a_orakel");
    let h6_stammer = ora_v >= VUNNA_FORDEL && ora_sp;
    md.push_str(&format!(
        "- **H6 (förutsägaren):** a_orakel (orakel_forutsagare mot lar) {}/{} (≥{}). Spegel {}. {}.\n",
        ora_v,
        ora_n,
        VUNNA_FORDEL,
        if ora_sp { "ok" } else { "utanför" },
        if h6_stammer {
            "Stämmer"
        } else if !ora_sp {
            "Tolkas inte (spegel utanför)"
        } else if ora_v <= VUNNA_NACKDEL {
            "Stämmer inte (oraklet förlorar; förutsägaren är inte flaskhalsen med dagens avläsare)"
        } else {
            "Stämmer inte (ingen visad skillnad; förutsägaren är inte flaskhalsen med dagens avläsare)"
        }
    ));

    // Regel 14
    let (tab_v, tab_n, tab_sp, _) = avgor_arm(&sam, korningar, "a_las_tab");
    let regel14 = tab_v < VUNNA_FORDEL;
    md.push_str(&format!(
        "- **Regel 14:** a_las_tab (lar_intagslasare mot tabell) {}/{} (under 15 betyder att förutsägaren inte bär mer än en tabell). Spegel {}. {}.\n",
        tab_v,
        tab_n,
        if tab_sp { "ok" } else { "utanför" },
        if !tab_sp {
            "Tolkas inte (spegel utanför)"
        } else if regel14 {
            "Förutsägaren bär inte mer än en tabell; ordet världsmodell används inte"
        } else {
            "lar_intagslasare når ≥15 av 20 mot tabell"
        }
    ));

    // H5
    let mut kvoter_flytt = Vec::new();
    let mut kvoter_sta = Vec::new();
    for k in korningar.iter().filter(|k| k.arm == "s_lar" && !k.spegel) {
        if let Some(m) = k.matning.iter().find(|m| m.formaga == "lar") {
            if let (Some(ft0), Some(fs0)) = (m.fel_tidigt_per[0], m.fel_sent_per[0]) {
                if ft0 > 0.0 {
                    kvoter_sta.push(fs0 / ft0);
                }
            }
            let mut flytt_tidigt = 0.0;
            let mut flytt_sent = 0.0;
            let mut n_f = 0usize;
            for a in 1..A_N {
                if let (Some(ft), Some(fs)) = (m.fel_tidigt_per[a], m.fel_sent_per[a]) {
                    flytt_tidigt += ft;
                    flytt_sent += fs;
                    n_f += 1;
                }
            }
            if n_f == 8 && flytt_tidigt > 0.0 {
                kvoter_flytt.push((flytt_sent / n_f as f64) / (flytt_tidigt / n_f as f64));
            }
        }
    }
    let spegel_slar_ok = spegel_medel(korningar, "s_lar")
        .map(spegel_ok)
        .unwrap_or(false);
    match (
        if kvoter_flytt.is_empty() {
            None
        } else {
            Some(kvoter_flytt.iter().sum::<f64>() / kvoter_flytt.len() as f64)
        },
        if kvoter_sta.is_empty() {
            None
        } else {
            Some(kvoter_sta.iter().sum::<f64>() / kvoter_sta.len() as f64)
        },
    ) {
        (Some(kf), Some(ks)) => {
            let stammer = kf >= H5_FLYTT_MIN && ks <= H5_STA_MAX && spegel_slar_ok;
            md.push_str(&format!(
                "- **H5 (felet per handling):** s_lar / lar: kvot sent/tidigt flyttmedel = {:.4} (krav ≥ {:.1}), stå kvar = {:.4} (krav ≤ {:.1}). Spegel s_lar {}. {}.\n",
                kf,
                H5_FLYTT_MIN,
                ks,
                H5_STA_MAX,
                if spegel_slar_ok { "ok" } else { "utanför" },
                if stammer {
                    "Stämmer"
                } else if !spegel_slar_ok {
                    "Tolkas inte (spegel utanför)"
                } else {
                    "Stämmer inte"
                }
            ));
        }
        _ => md.push_str("- **H5 (felet per handling):** saknas mätvärden.\n"),
    }

    // Stegen (Alt 1215)
    md.push_str("- **Stegen (Alt 1215):** ");
    let mut steg_delar = Vec::new();
    for namn in ["s_lar", "s_tab", "s_tab_tak", "s_lar_tak"] {
        if let Some(a) = hitta(&sam, namn, false) {
            let spegel = spegel_medel(korningar, namn)
                .map(spegel_ok)
                .unwrap_or(false);
            steg_delar.push(format!(
                "{} {}/{} ({}; spegel {})",
                namn,
                a.n_vunna,
                a.n_fro,
                tolka_vunna(a.n_vunna),
                if spegel { "ok" } else { "utanför" }
            ));
        }
    }
    md.push_str(&steg_delar.join("; "));
    let tab_tak = hitta(&sam, "s_tab_tak", false);
    let tab_tak_sp = spegel_medel(korningar, "s_tab_tak")
        .map(spegel_ok)
        .unwrap_or(false);
    if let Some(a) = tab_tak {
        if a.n_vunna >= VUNNA_FORDEL && tab_tak_sp {
            md.push_str(
                ". Tabellen vinner över allvetande_slump(0,1) ≥15 av 20: det allvetande djuret med 10 % slumpval är inget tak.\n",
            );
        } else {
            md.push_str(".\n");
        }
    } else {
        md.push_str(".\n");
    }

    // Andel v-annorlunda (redovisning)
    if let Some(v) = medel_matning_falt(korningar, "a_las", "lar_intagslasare", |m| {
        m.andel_v_annorlunda
    }) {
        md.push_str(&format!(
            "- **v mot läsaren (a_las):** medel andel steg där v valt annat = {:.4}.\n",
            v
        ));
    }

    let _ = Formaga::Lar;
    md
}

pub fn kor_avlasare(ut_dir: &Path, torr: bool) -> Result<(), String> {
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

    let jsonl_path = ut_dir.join("avlasare1213-korningar.jsonl");
    let md_path = ut_dir.join("avlasare1213-sammanfattning.md");

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
