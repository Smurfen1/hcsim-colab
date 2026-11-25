//! Ledprovet (Alt 1221 med kontrollen K Alt 1220 och känslighetsprovet Alt 1224).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{Formaga, HjarnArm};
use crate::minstakvadrat::{self, R2Samling};
use crate::sim::{LedLogg, MarkLage, Matning, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL};

/// Kartstorlek för hela ledprovet (underlaget).
pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön 141–160 (underlaget).
pub const FRO_START: u64 = 141;
pub const N_FRO_FULL: u64 = 20;

/// Gränser ur underlaget (skrivna före körningen).
pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
pub const R2_G1_SAKNAR: f64 = 0.2;
pub const R2_G1_BAR: f64 = 0.5;
pub const R2_G2_LAGE_DIFF: f64 = 0.2;

pub const ARM_ORDNING: [&str; 9] = [
    "o_slump", "o_tab", "o_tak", "k_las", "v1_tab", "v05_tab", "v05_ork", "v2_tab", "v2_ork",
];

#[derive(Clone, Debug)]
pub struct LedArm {
    pub namn: &'static str,
    pub markta: Formaga,
    pub omarkta: Formaga,
    pub atervaxt: f64,
    pub spegel: bool,
}

impl LedArm {
    fn till_hjarn(&self) -> HjarnArm {
        HjarnArm {
            namn: self.namn,
            markta: self.markta,
            omarkta: self.omarkta,
            spegel: self.spegel,
        }
    }

    fn spegel_av(&self) -> LedArm {
        LedArm {
            namn: self.namn,
            markta: self.omarkta,
            omarkta: self.markta,
            atervaxt: self.atervaxt,
            spegel: true,
        }
    }
}

/// De nio basarmarna (underlaget) + speglar.
pub fn ledprov_armar() -> Vec<LedArm> {
    let bas = [
        LedArm {
            namn: "o_slump",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Slump,
            atervaxt: 1.0,
            spegel: false,
        },
        LedArm {
            namn: "o_tab",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Tabell,
            atervaxt: 1.0,
            spegel: false,
        },
        LedArm {
            namn: "o_tak",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::AllvetandeSlump(0.1),
            atervaxt: 1.0,
            spegel: false,
        },
        LedArm {
            namn: "k_las",
            markta: Formaga::LarIntagslasare,
            omarkta: Formaga::Slump,
            atervaxt: 1.0,
            spegel: false,
        },
        LedArm {
            namn: "v1_tab",
            markta: Formaga::Tabell,
            omarkta: Formaga::Slump,
            atervaxt: 1.0,
            spegel: false,
        },
        LedArm {
            namn: "v05_tab",
            markta: Formaga::Tabell,
            omarkta: Formaga::Slump,
            atervaxt: 0.5,
            spegel: false,
        },
        LedArm {
            namn: "v05_ork",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Slump,
            atervaxt: 0.5,
            spegel: false,
        },
        LedArm {
            namn: "v2_tab",
            markta: Formaga::Tabell,
            omarkta: Formaga::Slump,
            atervaxt: 2.0,
            spegel: false,
        },
        LedArm {
            namn: "v2_ork",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Slump,
            atervaxt: 2.0,
            spegel: false,
        },
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for a in &bas {
        ut.push(a.clone());
    }
    for a in &bas {
        ut.push(a.spegel_av());
    }
    ut
}

#[derive(Clone, Debug)]
pub struct MatningRad {
    pub formaga: String,
    pub djursteg: u64,
}

#[derive(Clone, Debug)]
pub struct Korning {
    pub arm: String,
    pub spegel: bool,
    pub fro: u64,
    pub atervaxt: f64,
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
        let mat: Vec<String> = self
            .matning
            .iter()
            .map(|m| {
                format!(
                    "{{\"formaga\":\"{}\",\"djursteg\":{}}}",
                    m.formaga, m.djursteg
                )
            })
            .collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"atervaxt\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"logg_rader\":{},\"matning\":[{}],\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.atervaxt,
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

fn fro_lista(torr: bool) -> Vec<u64> {
    let n = if torr { N_FRO_TORR } else { N_FRO_FULL };
    (0..n).map(|i| FRO_START + i).collect()
}

fn anp_fron(torr: bool) -> Vec<u64> {
    if torr {
        vec![FRO_START]
    } else {
        (FRO_START..FRO_START + 10).collect()
    }
}

fn prov_fron(torr: bool) -> Vec<u64> {
    if torr {
        vec![FRO_START + 1]
    } else {
        (FRO_START + 10..FRO_START + 20).collect()
    }
}

#[derive(Clone)]
pub struct Jobb {
    arm: HjarnArm,
    atervaxt: f64,
    logg: bool,
    fro: u64,
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let fron = fro_lista(torr);
    let mut jobb = Vec::new();
    for arm in ledprov_armar() {
        for &fro in &fron {
            jobb.push(Jobb {
                arm: arm.till_hjarn(),
                atervaxt: arm.atervaxt,
                logg: false,
                fro,
            });
        }
    }
    // Loggkörningen r_lar: båda lar, atervaxt 1, spegel saknas.
    for &fro in &fron {
        jobb.push(Jobb {
            arm: HjarnArm {
                namn: "r_lar",
                markta: Formaga::Lar,
                omarkta: Formaga::Lar,
                spegel: false,
            },
            atervaxt: 1.0,
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
    atervaxt: f64,
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
    sim.atervaxt = atervaxt;
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
    atervaxt: f64,
    logg: bool,
    torr: bool,
) -> Result<(Korning, Vec<crate::sim::LedLoggRad>), String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm, atervaxt, logg)?;
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
        .map(|(k, v)| MatningRad {
            formaga: k.clone(),
            djursteg: v.djursteg,
        })
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
            atervaxt,
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
                let r = kor_en(
                    &kalla,
                    &tt,
                    b,
                    h,
                    j.fro,
                    &j.arm,
                    j.atervaxt,
                    j.logg,
                    torr,
                );
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
    pub atervaxt: f64,
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
            atervaxt: rader[0].atervaxt,
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

fn fmt_opt_r2(x: Option<f64>) -> String {
    match x {
        Some(v) => format!("{v:.4}"),
        None => "saknas".into(),
    }
}

fn skriv_r2_block(md: &mut String, g: &R2Samling) {
    md.push_str(&format!(
        "- **{}:** medel flytt = {}, stå kvar = {}.\n",
        g.namn,
        fmt_opt_r2(g.medel_flytt),
        fmt_opt_r2(g.sta_kvar)
    ));
    for r in &g.per_handling {
        if r.singulaer {
            md.push_str(&format!(
                "  - handling {}: SINGULÄR — {}\n",
                r.handling, r.meddelande
            ));
        } else {
            md.push_str(&format!(
                "  - handling {}: R² = {} (n_anp={}, n_prov={})\n",
                r.handling,
                fmt_opt_r2(r.r2),
                r.n_anp,
                r.n_prov
            ));
        }
    }
}

fn kanslighet_byter(
    n_lag: usize,
    n_mellan: usize,
    n_hog: usize,
) -> bool {
    let klass = |n: usize| -> i8 {
        if n >= VUNNA_FORDEL {
            1
        } else if n <= VUNNA_NACKDEL {
            -1
        } else {
            0
        }
    };
    let a = klass(n_lag);
    let b = klass(n_mellan);
    let c = klass(n_hog);
    // Byter sida: från högst 5 till minst 15 eller omvänt mellan nivåerna.
    (a == -1 && (b == 1 || c == 1))
        || (b == -1 && c == 1)
        || (a == 1 && (b == -1 || c == -1))
        || (b == 1 && c == -1)
}

pub fn skriv_sammanfattning(
    korningar: &[Korning],
    torr: bool,
    total_s: f64,
    g1: Option<&R2Samling>,
    g2: Option<&R2Samling>,
    g3: Option<&R2Samling>,
    r2_fel: Option<&str>,
) -> String {
    let sam = sammanfatta(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Ledprovet — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Ledprovet (Alt 1221, med K Alt 1220 och känslighetsprovet Alt 1224)\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-ledprov1221.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str(
        "| Arm | Spegel | Märkt | Omärkt | Återväxt | Vunna (andel>0,5) | Medel | Min | Max |\n",
    );
    md.push_str(
        "|-----|--------|-------|--------|----------|-------------------|-------|-----|-----|\n",
    );
    for s in &sam {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {}/{} | {:.4} | {:.4} | {:.4} |\n",
            s.arm,
            if s.spegel { "ja" } else { "nej" },
            s.markta_formaga,
            s.omarkta_formaga,
            s.atervaxt,
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

    // K / H4
    let (klas_v, klas_n, klas_sp, _) = avgor_arm(&sam, korningar, "k_las");
    let k_stammer = klas_v >= VUNNA_FORDEL && klas_sp;
    let k_text = if !klas_sp {
        "Tolkas inte (spegel utanför)"
    } else if k_stammer {
        "H4 står (k_las minst 15 av 20)"
    } else {
        "H4 stryks (läsarens seger i Alt 1213 var slump); B förblir stängd"
    };
    md.push_str(&format!(
        "- **K (H4 står eller stryks):** k_las (lar_intagslasare mot slump) {}/{} (≥{}). Spegel {}. {}.\n",
        klas_v,
        klas_n,
        VUNNA_FORDEL,
        if klas_sp { "ok" } else { "utanför" },
        k_text
    ));

    // Oraklet mot slumpen
    let (os_v, os_n, os_sp, _) = avgor_arm(&sam, korningar, "o_slump");
    let (ot_v, ot_n, ot_sp, _) = avgor_arm(&sam, korningar, "o_tab");
    let (ok_v, ok_n, ok_sp, _) = avgor_arm(&sam, korningar, "o_tak");
    let ora_text = if !os_sp {
        "Tolkas inte (spegel utanför)"
    } else if os_v < VUNNA_FORDEL {
        "inte ens sann information slår spridningen; då går G (världens trängsel och bete, med källa) före alla hjärnprov"
    } else {
        "oraklet når synlig fördel mot slumpen"
    };
    md.push_str(&format!(
        "- **Oraklet mot slumpen:** o_slump {}/{} (under 15: G före hjärnprov). Spegel {}. {}. o_tab {}/{} (spegel {}); o_tak {}/{} (spegel {}).\n",
        os_v,
        os_n,
        if os_sp { "ok" } else { "utanför" },
        ora_text,
        ot_v,
        ot_n,
        if ot_sp { "ok" } else { "utanför" },
        ok_v,
        ok_n,
        if ok_sp { "ok" } else { "utanför" },
    ));

    // R²
    md.push_str("- **R²-reglerna (medel över de 8 flyttarna):**\n");
    if let Some(e) = r2_fel {
        md.push_str(&format!("  R² kunde inte räknas: {e}\n"));
    } else if let (Some(g1), Some(g2), Some(g3)) = (g1, g2, g3) {
        skriv_r2_block(&mut md, g1);
        skriv_r2_block(&mut md, g2);
        skriv_r2_block(&mut md, g3);
        let g1m = g1.medel_flytt;
        let g2m = g2.medel_flytt;
        let r2_avg = match (g1m, g2m) {
            (Some(a), _) if a < R2_G1_SAKNAR => {
                "G1 under 0,2: sinnena saknar uppgiften. B stängd; nästa steg är vad Edmontosaurus kunde känna på avstånd (källa, regel 10)."
            }
            (Some(a), Some(b)) if a >= R2_G1_BAR && (a - b) >= R2_G2_LAGE_DIFF => {
                "G1 minst 0,5 och G2 minst 0,2 lägre än G1: konkret fel i hjärnans värdering (Alt 1212); B får öppnas."
            }
            _ => {
                "ingen visad skillnad. Ett lågt linjärt R² utesluter inte ett olinjärt samband."
            }
        };
        md.push_str(&format!("  Avgörande: {r2_avg}\n"));
    } else {
        md.push_str("  R² saknas.\n");
    }

    // Känslighetsprovet
    let (v05t, _, sp05t, _) = avgor_arm(&sam, korningar, "v05_tab");
    let (v1t, _, sp1t, _) = avgor_arm(&sam, korningar, "v1_tab");
    let (v2t, _, sp2t, _) = avgor_arm(&sam, korningar, "v2_tab");
    let (v05o, _, sp05o, _) = avgor_arm(&sam, korningar, "v05_ork");
    let (v1o, _, sp1o, _) = avgor_arm(&sam, korningar, "o_slump"); // v1 för oraklet
    let (v2o, _, sp2o, _) = avgor_arm(&sam, korningar, "v2_ork");
    let tab_byter = kanslighet_byter(v05t, v1t, v2t);
    let ork_byter = kanslighet_byter(v05o, v1o, v2o);
    let speglar_kans =
        sp05t && sp1t && sp2t && sp05o && sp1o && sp2o;
    let kans_text = if !speglar_kans {
        "Tolkas inte (minst en spegel utanför)"
    } else if tab_byter || ork_byter {
        "rangordningen byter sida mellan återväxt 0,5/1/2; beror på ett eget val, och G:s källa läses innan något hjärnprov tolkas"
    } else {
        "ingen sidbyte mellan återväxt 0,5/1/2 (tabell–slump och orakel–slump)"
    };
    md.push_str(&format!(
        "- **Känslighetsprovet (Alt 1224):** v05_tab {}/20, v1_tab {}/20, v2_tab {}/20; v05_ork {}/20, o_slump {}/20, v2_ork {}/20. Speglar {}. {}.\n",
        v05t, v1t, v2t, v05o, v1o, v2o,
        if speglar_kans { "ok" } else { "utanför" },
        kans_text
    ));

    md
}

pub fn kor_ledprov(ut_dir: &Path, torr: bool) -> Result<(), String> {
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

    let jsonl_path = ut_dir.join("ledprov1221-korningar.jsonl");
    let logg_path = ut_dir.join("ledprov1221-logg.tsv");
    let md_path = ut_dir.join("ledprov1221-sammanfattning.md");

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let mut tsv = minstakvadrat::tsv_huvud();
    for r in &logg {
        tsv.push_str(&minstakvadrat::tsv_rad(r));
    }
    std::fs::write(&logg_path, &tsv)
        .map_err(|e| format!("skriv {}: {e}", logg_path.display()))?;

    let anp = anp_fron(torr);
    let prov = prov_fron(torr);
    let (g1, g2, g3, r2_fel) = match minstakvadrat::rakna_alla(&logg, &anp, &prov) {
        Ok((a, b, c)) => (Some(a), Some(b), Some(c), None),
        Err(e) => (None, None, None, Some(e)),
    };

    let md = skriv_sammanfattning(
        &korningar,
        torr,
        total_s,
        g1.as_ref(),
        g2.as_ref(),
        g3.as_ref(),
        r2_fel.as_deref(),
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
