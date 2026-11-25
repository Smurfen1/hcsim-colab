//! Populationssvepet Alt 1198: positiv- och nollkontroll på tre kartstorlekar.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::sim::{MarkLage, Sim};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR};

pub const LAT0: f64 = 47.0;
pub const LON0: f64 = -108.0;
pub const RUTA_M: f64 = 2000.0;
pub const T_KARTA: f64 = 500_000.0;
pub const MAX_TRADAR: usize = 12;
pub const N_FRO_FULL: u64 = 20;
pub const N_FRO_TORR: u64 = 2;

pub const STORLEKAR: [(usize, usize); 3] = [(16, 12), (32, 24), (64, 48)];

#[derive(Clone, Debug)]
pub struct Korning {
    pub b: usize,
    pub h: usize,
    pub arm: MarkLage,
    pub fro: u64,
    pub antal_markering: usize,
    pub antal_slut: usize,
    pub markta_slut: usize,
    pub andel_markta: f64,
    pub kortid_s: f64,
    pub torr: bool,
}

impl Korning {
    pub fn storlek_str(&self) -> String {
        format!("{}x{}", self.b, self.h)
    }

    pub fn till_json(&self) -> String {
        let mut s = format!(
            "{{\"storlek\":\"{}\",\"arm\":\"{}\",\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"kortid_s\":{}}}",
            self.storlek_str(),
            self.arm.som_str(),
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            fmt_f64(self.kortid_s),
        );
        if self.torr {
            s.push_str(" TORRKÖRNING");
        }
        s
    }
}

pub fn n0_for(b: usize, h: usize) -> usize {
    let x = 400.0 * (b * h) as f64 / (64.0 * 48.0);
    (x.round() as usize).max(20)
}

fn fmt_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{}", x)
    } else {
        "null".into()
    }
}

/// Sänker processens prioritet (nice 19) om det går.
pub fn sat_nice() {
    let _ = std::process::Command::new("renice")
        .args(["-n", "19", "-p", &std::process::id().to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn bygg_sim(kalla: &Kalla, tt: &Tidtabell, b: usize, h: usize, fro: u64, arm: MarkLage) -> Result<Sim, String> {
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
    sim.mark_lage = arm;
    Ok(sim)
}

/// En svepkörning: inkörning → märkning → efterår.
pub fn kor_en(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    arm: MarkLage,
    torr: bool,
) -> Result<Korning, String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro, arm)?;
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    if arm != MarkLage::Utan {
        sim.markera_halften(fro);
    }
    let antal_markering = sim.antal();
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, MARKT_EFTER_AR);
    let antal_slut = sim.antal();
    let markta_slut = sim.antal_markta();
    let andel_markta = sim.andel_markta();
    Ok(Korning {
        b,
        h,
        arm,
        fro,
        antal_markering,
        antal_slut,
        markta_slut,
        andel_markta,
        kortid_s: start.elapsed().as_secs_f64(),
        torr,
    })
}

#[derive(Clone, Copy)]
pub struct Jobb {
    pub b: usize,
    pub h: usize,
    pub arm: MarkLage,
    pub fro: u64,
}

/// Kör alla jobb parallellt (högst MAX_TRADAR trådar). Varje tråd laddar egen karta-källa.
pub fn kor_parallellt(paleo: &Path, tt_path: &Path, jobb: &[Jobb], torr: bool) -> Result<Vec<Korning>, String> {
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
                let r = kor_en(&kalla, &tt, j.b, j.h, j.fro, j.arm, torr);
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for h in handles {
        h.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let mut resultat = Vec::new();
    for r in ut.lock().unwrap().drain(..) {
        resultat.push(r?);
    }
    resultat.sort_by(|a, b| {
        (a.b, a.h, a.arm.som_str(), a.fro).cmp(&(b.b, b.h, b.arm.som_str(), b.fro))
    });
    Ok(resultat)
}

pub fn bygg_jobb(torr: bool) -> Vec<Jobb> {
    let n_fro = if torr { N_FRO_TORR } else { N_FRO_FULL };
    let storlekar: &[(usize, usize)] = if torr {
        &STORLEKAR[..1]
    } else {
        &STORLEKAR
    };
    let armar = [MarkLage::Positiv, MarkLage::Noll];
    let mut jobb = Vec::new();
    for &(b, h) in storlekar {
        for &arm in &armar {
            for fro in 1..=n_fro {
                jobb.push(Jobb { b, h, arm, fro });
            }
        }
    }
    // pop tar från slutet — vänd så 16×12 börjar först
    jobb.reverse();
    jobb
}

#[derive(Clone, Debug)]
pub struct ArmSammanfattning {
    pub b: usize,
    pub h: usize,
    pub arm: MarkLage,
    pub n_vunna: usize,
    pub n_fro: usize,
    pub medel_andel: f64,
    pub min_andel: f64,
    pub max_andel: f64,
    pub medel_antal: f64,
    pub min_antal: usize,
    pub max_antal: usize,
}

impl ArmSammanfattning {
    pub fn positiv_syns(&self) -> bool {
        self.arm == MarkLage::Positiv && self.n_vunna >= 15
    }

    pub fn noll_haller(&self) -> bool {
        self.arm == MarkLage::Noll && (6..=14).contains(&self.n_vunna)
    }
}

pub fn sammanfatta_armar(korningar: &[Korning]) -> Vec<ArmSammanfattning> {
    let mut ut = Vec::new();
    for &(b, h) in &STORLEKAR {
        for &arm in &[MarkLage::Positiv, MarkLage::Noll] {
            let rader: Vec<&Korning> = korningar
                .iter()
                .filter(|k| k.b == b && k.h == h && k.arm == arm)
                .collect();
            if rader.is_empty() {
                continue;
            }
            let n_fro = rader.len();
            let n_vunna = rader.iter().filter(|k| k.andel_markta > 0.5).count();
            let andelar: Vec<f64> = rader.iter().map(|k| k.andel_markta).collect();
            let antal: Vec<usize> = rader.iter().map(|k| k.antal_slut).collect();
            let medel_andel = andelar.iter().sum::<f64>() / n_fro as f64;
            let min_andel = andelar.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_andel = andelar.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let medel_antal = antal.iter().sum::<usize>() as f64 / n_fro as f64;
            let min_antal = *antal.iter().min().unwrap();
            let max_antal = *antal.iter().max().unwrap();
            ut.push(ArmSammanfattning {
                b,
                h,
                arm,
                n_vunna,
                n_fro,
                medel_andel,
                min_andel,
                max_andel,
                medel_antal,
                min_antal,
                max_antal,
            });
        }
    }
    ut
}

/// Minsta storlek där positiv syns och noll håller; annars "ingen storlek".
pub fn avgor_svar(sam: &[ArmSammanfattning]) -> String {
    for &(b, h) in &STORLEKAR {
        let pos = sam
            .iter()
            .find(|s| s.b == b && s.h == h && s.arm == MarkLage::Positiv);
        let noll = sam
            .iter()
            .find(|s| s.b == b && s.h == h && s.arm == MarkLage::Noll);
        match (pos, noll) {
            (Some(p), Some(n)) if p.positiv_syns() && n.noll_haller() => {
                return format!("{}x{}", b, h);
            }
            _ => {}
        }
    }
    "ingen storlek".into()
}

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let sam = sammanfatta_armar(korningar);
    let svar = avgor_svar(&sam);
    let mut md = String::new();
    if torr {
        md.push_str("# Populationssvep Alt 1198 — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, bara 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Populationssvep Alt 1198\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-21-lasunderlag-svep1198.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str("| Storlek | Arm | Vunna (andel>0,5) | Medel andel | Min | Max | Medel antal | Min antal | Max antal |\n");
    md.push_str("|---------|-----|-------------------|-------------|-----|-----|-------------|-----------|----------|\n");
    for s in &sam {
        md.push_str(&format!(
            "| {}x{} | {} | {}/{} | {:.4} | {:.4} | {:.4} | {:.1} | {} | {} |\n",
            s.b,
            s.h,
            s.arm.som_str(),
            s.n_vunna,
            s.n_fro,
            s.medel_andel,
            s.min_andel,
            s.max_andel,
            s.medel_antal,
            s.min_antal,
            s.max_antal,
        ));
    }
    md.push('\n');
    md.push_str("## Avgöranden\n\n");
    for &(b, h) in &STORLEKAR {
        let pos = sam
            .iter()
            .find(|s| s.b == b && s.h == h && s.arm == MarkLage::Positiv);
        let noll = sam
            .iter()
            .find(|s| s.b == b && s.h == h && s.arm == MarkLage::Noll);
        if pos.is_none() && noll.is_none() {
            continue;
        }
        let pos_txt = match pos {
            Some(p) if p.positiv_syns() => {
                format!("positivkontrollen syns ({} av {} frön)", p.n_vunna, p.n_fro)
            }
            Some(p) => format!(
                "positivkontrollen syns inte ({} av {} frön; krävs minst 15)",
                p.n_vunna, p.n_fro
            ),
            None => "positivkontrollen saknas".into(),
        };
        let noll_txt = match noll {
            Some(n) if n.noll_haller() => {
                format!("nollkontrollen håller ({} av {} frön)", n.n_vunna, n.n_fro)
            }
            Some(n) => format!(
                "nollkontrollen håller inte ({} av {} frön; krävs 6–14)",
                n.n_vunna, n.n_fro
            ),
            None => "nollkontrollen saknas".into(),
        };
        md.push_str(&format!("- **{}x{}:** {}; {}.\n", b, h, pos_txt, noll_txt));
    }
    md.push('\n');
    md.push_str(&format!(
        "**Svar:** minsta storlek där positivkontrollen syns och nollkontrollen håller: **{}**.\n",
        svar
    ));
    md
}

pub fn kor_svep(ut_dir: &Path, torr: bool) -> Result<(), String> {
    sat_nice();
    let start = Instant::now();
    let paleo = ut_dir.join("../paleokarta/data");
    let tt_path = ut_dir.join("../hckarta/tidtabell.tsv");
    let jobb = bygg_jobb(torr);
    let korningar = kor_parallellt(&paleo, &tt_path, &jobb, torr)?;
    let total_s = start.elapsed().as_secs_f64();

    let jsonl_path = ut_dir.join("svep1198-korningar.jsonl");
    let md_path = ut_dir.join("svep1198-sammanfattning.md");

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
