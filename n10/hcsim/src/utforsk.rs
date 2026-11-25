//! Ärvd utforskning Alt 1463: armarna e_lar, e_fast, e_tab med speglar (frö 261–280).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{utforsk_armar, Formaga, HjarnArm};
use crate::sim::{AldrigAck, KopierArAck, MarkLage, Matning, Sim, UtforskMatning};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR, SPEGEL_TOL};

/// Alt 1806 (agenternas runda 2 om utdöendet, alla tre): jämförelsen Alt 1463 körs om i den rättade världen. Påslagen (`hcsim utforsk
/// --rattad`) byggs varje värld som visningskörningen: bälte `BALTE_K`, sammanfogade sidor och rättningarna mot utdöendet (Håkans
/// Alt 1825). Resultatfilerna får då namnet `utforsk1806-rattad-*`, så att Alt 1463:s filer står kvar.
pub static RATTAD: AtomicBool = AtomicBool::new(false);

pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön (underlaget).
pub const FRO_START: u64 = 261;
pub const N_FRO_FULL: u64 = 20;

pub const VUNNA_FORDEL: usize = 15;
pub const VUNNA_NACKDEL: usize = 5;
/// Rött om ε-medel vid slut < hälften av startvärdet i minst 15 frön.
pub const EPS_ROD_MAX: f64 = 0.025;

#[derive(Clone, Debug)]
pub struct GruppMat {
    pub formaga: String,
    pub aldrig_valda: Option<f64>,
    pub vanligaste_andel: Option<f64>,
    pub n_aldrig_djur: u64,
    pub medel_forutsag: Option<f64>,
    pub medel_kopiering: Option<f64>,
    pub kvot_kopiering: Option<f64>,
    pub djursteg_kopier: u64,
}

impl GruppMat {
    fn fran(
        formaga: &str,
        aldrig: Option<&AldrigAck>,
        kopier: Option<&KopierArAck>,
    ) -> Self {
        let (medel_forutsag, medel_kopiering, kvot_kopiering, djursteg_kopier) = match kopier {
            Some(k) => {
                let mf = k.medel_forutsag();
                let mk = k.medel_kopiering();
                let kvot = match (mf, mk) {
                    (Some(f), Some(c)) if c > 0.0 => Some(f / c),
                    _ => None,
                };
                (mf, mk, kvot, k.n)
            }
            None => (None, None, None, 0),
        };
        Self {
            formaga: formaga.to_string(),
            aldrig_valda: aldrig.and_then(|a| a.medel_aldrig()),
            vanligaste_andel: aldrig.and_then(|a| a.medel_vanligaste_andel()),
            n_aldrig_djur: aldrig.map(|a| a.n_djur).unwrap_or(0),
            medel_forutsag,
            medel_kopiering,
            kvot_kopiering,
            djursteg_kopier,
        }
    }

    fn till_json(&self) -> String {
        format!(
            "{{\"formaga\":\"{}\",\"aldrig_valda\":{},\"vanligaste_andel\":{},\"n_aldrig_djur\":{},\"medel_forutsag\":{},\"medel_kopiering\":{},\"kvot_kopiering\":{},\"djursteg_kopier\":{}}}",
            self.formaga,
            fmt_opt(self.aldrig_valda),
            fmt_opt(self.vanligaste_andel),
            self.n_aldrig_djur,
            fmt_opt(self.medel_forutsag),
            fmt_opt(self.medel_kopiering),
            fmt_opt(self.kvot_kopiering),
            self.djursteg_kopier,
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
    pub markta: GruppMat,
    pub omarkta: GruppMat,
    pub eps_medel: Option<f64>,
    pub eps_median: Option<f64>,
    pub eps_andel_under_001: Option<f64>,
    pub eps_per_10ar: Vec<(i64, f64)>,
    /// ε vid slutet i den omärkta gruppen (neutral: ärvd men oanvänd; Claudes tillägg).
    pub eps_omarkt_medel: Option<f64>,
    pub eps_omarkt_median: Option<f64>,
}

impl Korning {
    pub fn till_json(&self) -> String {
        let eps10: Vec<String> = self
            .eps_per_10ar
            .iter()
            .map(|(ar, m)| format!("{{\"ar\":{},\"eps_medel\":{}}}", ar, fmt_f64(*m)))
            .collect();
        let mut s = format!(
            "{{\"arm\":\"{}\",\"spegel\":{},\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"markta_vid_slut\":{},\"andel_markta\":{},\"markta_formaga\":\"{}\",\"omarkta_formaga\":\"{}\",\"markta\":{},\"omarkta\":{},\"eps_medel\":{},\"eps_median\":{},\"eps_andel_under_001\":{},\"eps_per_10ar\":[{}],\"eps_omarkt_medel\":{},\"eps_omarkt_median\":{},\"kortid_s\":{}}}",
            self.arm,
            if self.spegel { "true" } else { "false" },
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.markta_slut,
            fmt_f64(self.andel_markta),
            self.markta_formaga,
            self.omarkta_formaga,
            self.markta.till_json(),
            self.omarkta.till_json(),
            fmt_opt(self.eps_medel),
            fmt_opt(self.eps_median),
            fmt_opt(self.eps_andel_under_001),
            eps10.join(","),
            fmt_opt(self.eps_omarkt_medel),
            fmt_opt(self.eps_omarkt_median),
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

pub fn bygg_sim(kalla: &Kalla, tt: &Tidtabell, b: usize, h: usize, fro: u64, arm: &HjarnArm) -> Result<Sim, String> {
    bygg_sim_balte(kalla, tt, b, h, 0, fro, arm)
}

/// Som `bygg_sim`, med `k` rutor land runt fönstret b · h: kartan blir (b + 2k) · (h + 2k) och dess hörn flyttas k rutor åt sydväst,
/// så att fönstret ligger kvar på samma plats (visningskörningen, Håkans Alt 1725). `k` = 0 ger samma karta som `bygg_sim`.
pub fn bygg_sim_balte(kalla: &Kalla, tt: &Tidtabell, b: usize, h: usize, k: usize, fro: u64, arm: &HjarnArm) -> Result<Sim, String> {
    let lat0 = LAT0 - (k as f64 * RUTA_M) / 111_320.0;
    let lon0 = LON0 - (k as f64 * RUTA_M) / (111_320.0 * lat0.to_radians().cos());
    let (b, h) = (b + 2 * k, h + 2 * k);
    let omr = Omrade {
        lat0,
        lon0,
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
    sim.utforsk_matning = UtforskMatning {
        pa: true,
        ..UtforskMatning::default()
    };
    sim.turordning = false;
    sim.flytt_i_tur = false;
    sim.bete_i_tur = false;
    Ok(sim)
}

fn grupp_av(sim: &Sim, formaga: Formaga) -> GruppMat {
    let nyckel = formaga.som_str();
    GruppMat::fran(
        &nyckel,
        sim.utforsk_matning.aldrig_per.get(&nyckel),
        sim.utforsk_matning.kopier_per.get(&nyckel),
    )
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
    let mut sim = if RATTAD.load(Ordering::Relaxed) {
        let k = crate::kartbild::BALTE_K;
        let mut s = bygg_sim_balte(kalla, tt, b, h, k, fro, arm)?;
        s.kant = crate::sim::KantLage::Torus;
        s.fonster_k = k;
        crate::kartbild::satt_rattningar_utdod(&mut s);
        s
    } else {
        bygg_sim(kalla, tt, b, h, fro, arm)?
    };
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    sim.markera_halften(fro);
    let antal_markering = sim.antal();
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, 10);
    sim.bokfor_aldrig_alla();

    let (eps_medel, eps_median, eps_andel) = match sim.eps_slut_markta() {
        Some((a, b, c)) => (Some(a), Some(b), Some(c)),
        None => (None, None, None),
    };
    let (eps_omarkt_medel, eps_omarkt_median) = match sim.eps_slut(false) {
        Some((a, b, _)) => (Some(a), Some(b)),
        None => (None, None),
    };
    let mut eps_per_10ar: Vec<(i64, f64)> = sim
        .utforsk_matning
        .eps_per_ar
        .iter()
        .filter_map(|(&ar, ack)| ack.medel().map(|m| (ar, m)))
        .collect();
    eps_per_10ar.sort_by_key(|(ar, _)| *ar);

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
        markta: grupp_av(&sim, arm.markta),
        omarkta: grupp_av(&sim, arm.omarkta),
        eps_medel,
        eps_median,
        eps_andel_under_001: eps_andel,
        eps_per_10ar,
        eps_omarkt_medel,
        eps_omarkt_median,
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
    for arm in utforsk_armar() {
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
                    ut.lock().unwrap().push(Err(format!("karta: {e}")));
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
                    ut.lock().unwrap().push(Err(format!("tidtabell: {e}")));
                    return;
                }
            };
            loop {
                let jobb = {
                    let mut q = ko.lock().unwrap();
                    q.pop()
                };
                let Some(jobb) = jobb else { break };
                let r = kor_en(&kalla, &tt, b, h, jobb.fro, &jobb.arm, torr);
                // Claudes tillägg: varje färdig körning skrivs direkt, så att inget går förlorat vid ett stopp.
                if let Ok(k) = &r {
                    eprintln!("KLAR {}", k.till_json());
                }
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let raw = Arc::try_unwrap(ut).unwrap().into_inner().unwrap();
    let mut ut = Vec::with_capacity(raw.len());
    for r in raw {
        ut.push(r?);
    }
    ut.sort_by(|a, b| {
        a.arm
            .cmp(&b.arm)
            .then(a.spegel.cmp(&b.spegel))
            .then(a.fro.cmp(&b.fro))
    });
    Ok(ut)
}

#[derive(Clone, Debug)]
pub struct ArmSam {
    pub arm: String,
    pub spegel: bool,
    pub markta_formaga: String,
    pub omarkta_formaga: String,
    pub n_fro: usize,
    pub n_vunna: usize,
    pub medel_andel: f64,
    pub min_andel: f64,
    pub max_andel: f64,
}

fn sammanfatta(korningar: &[Korning]) -> Vec<ArmSam> {
    let mut nycklar: Vec<(String, bool)> = Vec::new();
    for k in korningar {
        let ny = (k.arm.clone(), k.spegel);
        if !nycklar.contains(&ny) {
            nycklar.push(ny);
        }
    }
    let mut ut = Vec::new();
    for (arm, spegel) in nycklar {
        let grup: Vec<&Korning> = korningar
            .iter()
            .filter(|k| k.arm == arm && k.spegel == spegel)
            .collect();
        if grup.is_empty() {
            continue;
        }
        let andelar: Vec<f64> = grup.iter().map(|k| k.andel_markta).collect();
        let n_vunna = andelar.iter().filter(|&&a| a > 0.5).count();
        let medel = andelar.iter().sum::<f64>() / andelar.len() as f64;
        let min = andelar.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = andelar.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        ut.push(ArmSam {
            arm,
            spegel,
            markta_formaga: grup[0].markta_formaga.clone(),
            omarkta_formaga: grup[0].omarkta_formaga.clone(),
            n_fro: grup.len(),
            n_vunna,
            medel_andel: medel,
            min_andel: min,
            max_andel: max,
        });
    }
    ut
}

fn hitta<'a>(sam: &'a [ArmSam], namn: &str, spegel: bool) -> Option<&'a ArmSam> {
    sam.iter().find(|s| s.arm == namn && s.spegel == spegel)
}

fn spegel_medel(korningar: &[Korning], namn: &str) -> Option<f64> {
    let mut par: Vec<(f64, f64)> = Vec::new();
    let bas: Vec<&Korning> = korningar
        .iter()
        .filter(|k| k.arm == namn && !k.spegel)
        .collect();
    for b in bas {
        if let Some(s) = korningar
            .iter()
            .find(|k| k.arm == namn && k.spegel && k.fro == b.fro)
        {
            par.push((b.andel_markta, s.andel_markta));
        }
    }
    if par.is_empty() {
        None
    } else {
        let sum: f64 = par.iter().map(|(a, b)| a + b).sum();
        Some(sum / par.len() as f64)
    }
}

fn spegel_ok(m: f64) -> bool {
    (m - 1.0).abs() <= SPEGEL_TOL
}

/// Utfallsregler ur underlaget (skrivna före).
#[derive(Clone, Debug, Default)]
pub struct UtfallsRakning {
    pub e_lar_vunna: usize,
    pub e_fast_vunna: usize,
    pub e_tab_vunna: usize,
    pub e_lar_n: usize,
    pub e_fast_n: usize,
    pub e_tab_n: usize,
    pub e_lar_aldrig_lagre: usize,
    pub e_lar_eps_under: usize,
    pub e_lar_kopier_under_1: usize,
    pub e_fast_lar_vunna: usize,
    pub e_fast_lar_n: usize,
    pub spegel_e_fast_lar: Option<f64>,
    pub spegel_e_fast_lar_ok: bool,
    /// Frön där de märktas ε vid slutet är lägre än de omärktas (neutral) ε.
    pub e_lar_eps_under_neutral: usize,
    pub e_lar_eps_par: usize,
    pub spegel_e_lar_ok: bool,
    pub spegel_e_fast_ok: bool,
    pub spegel_e_tab_ok: bool,
    pub spegel_e_lar: Option<f64>,
    pub spegel_e_fast: Option<f64>,
    pub spegel_e_tab: Option<f64>,
    pub rader: Vec<String>,
}

pub fn rakna_utfall(korningar: &[Korning]) -> UtfallsRakning {
    let sam = sammanfatta(korningar);
    let mut u = UtfallsRakning::default();

    if let Some(a) = hitta(&sam, "e_lar", false) {
        u.e_lar_vunna = a.n_vunna;
        u.e_lar_n = a.n_fro;
    }
    if let Some(a) = hitta(&sam, "e_fast", false) {
        u.e_fast_vunna = a.n_vunna;
        u.e_fast_n = a.n_fro;
    }
    if let Some(a) = hitta(&sam, "e_fast_lar", false) {
        u.e_fast_lar_vunna = a.n_vunna;
        u.e_fast_lar_n = a.n_fro;
    }
    if let Some(a) = hitta(&sam, "e_tab", false) {
        u.e_tab_vunna = a.n_vunna;
        u.e_tab_n = a.n_fro;
    }

    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        match (k.markta.aldrig_valda, k.omarkta.aldrig_valda) {
            (Some(m), Some(o)) if m < o => u.e_lar_aldrig_lagre += 1,
            _ => {}
        }
        if let Some(e) = k.eps_medel {
            if e < EPS_ROD_MAX {
                u.e_lar_eps_under += 1;
            }
        }
        if let (Some(m), Some(o)) = (k.eps_medel, k.eps_omarkt_medel) {
            u.e_lar_eps_par += 1;
            if m < o {
                u.e_lar_eps_under_neutral += 1;
            }
        }
        if let Some(kv) = k.markta.kvot_kopiering {
            if kv < 1.0 {
                u.e_lar_kopier_under_1 += 1;
            }
        }
    }

    u.spegel_e_lar = spegel_medel(korningar, "e_lar");
    u.spegel_e_fast = spegel_medel(korningar, "e_fast");
    u.spegel_e_tab = spegel_medel(korningar, "e_tab");
    u.spegel_e_fast_lar = spegel_medel(korningar, "e_fast_lar");
    u.spegel_e_fast_lar_ok = u.spegel_e_fast_lar.map(spegel_ok).unwrap_or(false);
    u.spegel_e_lar_ok = u.spegel_e_lar.map(spegel_ok).unwrap_or(false);
    u.spegel_e_fast_ok = u.spegel_e_fast.map(spegel_ok).unwrap_or(false);
    u.spegel_e_tab_ok = u.spegel_e_tab.map(spegel_ok).unwrap_or(false);

    // Utfallsregler (minst 15 av 20; spegel inom 1 ± 0,116).
    let lar_hjalper = u.e_lar_vunna >= VUNNA_FORDEL
        && u.e_lar_aldrig_lagre >= VUNNA_FORDEL
        && u.spegel_e_lar_ok;
    let rod_aldrig = u.e_lar_aldrig_lagre < VUNNA_FORDEL;
    let rod_overlevnad = u.e_lar_vunna <= VUNNA_NACKDEL;
    let rod_eps = u.e_lar_eps_under >= VUNNA_FORDEL;
    let arv_spelar_roll =
        u.e_lar_vunna >= VUNNA_FORDEL && u.e_fast_vunna <= VUNNA_NACKDEL && u.spegel_e_lar_ok;
    let bara_slump = u.e_lar_vunna >= VUNNA_FORDEL
        && u.e_fast_vunna >= VUNNA_FORDEL
        && u.spegel_e_lar_ok
        && u.spegel_e_fast_ok;
    let ribba_slar = u.e_tab_vunna >= VUNNA_FORDEL && u.spegel_e_tab_ok;
    let ribba_som_diagnos = u.e_tab_vunna <= VUNNA_NACKDEL;

    if lar_hjalper {
        u.rader.push(format!(
            "- **Lagningen hjälper:** e_lar {}/{} och märktas aldrig valda lägre i {}/{} frön (spegel ok).",
            u.e_lar_vunna, u.e_lar_n, u.e_lar_aldrig_lagre, u.e_lar_n
        ));
    }
    if rod_aldrig || rod_overlevnad || rod_eps {
        u.rader.push(format!(
            "- **Rött (Fable-2):** aldrig-lägre {}/{}; e_lar vunna {}/{} (≤{} = sämre överlevnad); ε-medel < {} i {}/{} frön.",
            u.e_lar_aldrig_lagre,
            u.e_lar_n,
            u.e_lar_vunna,
            u.e_lar_n,
            VUNNA_NACKDEL,
            EPS_ROD_MAX,
            u.e_lar_eps_under,
            u.e_lar_n
        ));
    }
    if arv_spelar_roll {
        u.rader.push(format!(
            "- **Arvet spelar roll:** e_lar {}/{} och e_fast {}/{} (≤{}).",
            u.e_lar_vunna, u.e_lar_n, u.e_fast_vunna, u.e_fast_n, VUNNA_NACKDEL
        ));
    }
    if bara_slump {
        u.rader.push(format!(
            "- **Bara slumpen spelar roll:** e_lar {}/{} och e_fast {}/{}.",
            u.e_lar_vunna, u.e_lar_n, u.e_fast_vunna, u.e_fast_n
        ));
    }
    if ribba_slar {
        u.rader.push(format!(
            "- **Ribban:** e_tab {}/{} — den lagade hjärnan slår minnestabellen.",
            u.e_tab_vunna, u.e_tab_n
        ));
    } else if ribba_som_diagnos {
        u.rader.push(format!(
            "- **Ribban:** e_tab {}/{} — som i diagnosen (högst {}).",
            u.e_tab_vunna, u.e_tab_n, VUNNA_NACKDEL
        ));
    } else {
        u.rader.push(format!(
            "- **Ribban:** e_tab {}/{} — ingen visad skillnad mot tabellen.",
            u.e_tab_vunna, u.e_tab_n
        ));
    }
    // Claudes tillägg före körningen (underlagets tillägg): den rena fasta armen och den neutrala ε.
    if u.e_fast_lar_n > 0 {
        let txt = if u.e_lar_vunna >= VUNNA_FORDEL && u.e_fast_lar_vunna <= VUNNA_NACKDEL && u.spegel_e_lar_ok {
            "arvet spelar roll"
        } else if u.e_lar_vunna >= VUNNA_FORDEL && u.e_fast_lar_vunna >= VUNNA_FORDEL && u.spegel_e_lar_ok && u.spegel_e_fast_lar_ok {
            "bara slumpen spelar roll"
        } else {
            "ingen av de två"
        };
        u.rader.push(format!(
            "- **Tillägg, ren fast arm:** e_lar {}/{} och e_fast_lar {}/{} (lar_utforsk med fast ε, lär av slumpvalet): {}.",
            u.e_lar_vunna, u.e_lar_n, u.e_fast_lar_vunna, u.e_fast_lar_n, txt
        ));
    }
    u.rader.push(format!(
        "- **Tillägg, neutral ε:** de märktas ε vid slutet lägre än de omärktas (ärvd men oanvänd) i {}/{} frön där båda grupperna lever ({} = urvalet driver ε nedåt, högst {} = uppåt).",
        u.e_lar_eps_under_neutral, u.e_lar_eps_par, VUNNA_FORDEL, VUNNA_NACKDEL
    ));
    u.rader.push(format!(
        "- **Kopieringsprovet** (redovisning, inget krav): lar_utforsk kvot < 1 i {}/{} frön (e_lar).",
        u.e_lar_kopier_under_1, u.e_lar_n
    ));
    if u.rader.len() <= 2 && !lar_hjalper && !rod_aldrig && !rod_overlevnad && !rod_eps {
        u.rader.insert(
            0,
            "- **Ingen visad skillnad** enligt utfallsreglernas huvudgrenar.".into(),
        );
    }
    u
}

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let sam = sammanfatta(korningar);
    let utfall = rakna_utfall(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Utforsk 1463 — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Ärvd utforskning (Alt 1463)\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-22-lasunderlag-utforsk1463.md` (skrivna före körningen).\n\n",
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

    md.push_str("## Speglar\n\n");
    for (namn, medel, ok) in [
        ("e_lar", utfall.spegel_e_lar, utfall.spegel_e_lar_ok),
        ("e_fast", utfall.spegel_e_fast, utfall.spegel_e_fast_ok),
        ("e_tab", utfall.spegel_e_tab, utfall.spegel_e_tab_ok),
        ("e_fast_lar", utfall.spegel_e_fast_lar, utfall.spegel_e_fast_lar_ok),
    ] {
        match medel {
            Some(m) => md.push_str(&format!(
                "- **Spegel {namn}:** medel av (andel + spegelandel) = {:.4} (krav 1 ± {:.3}) — {}.\n",
                m,
                SPEGEL_TOL,
                if ok {
                    "inom gränsen"
                } else {
                    "utanför: märket har en bieffekt; armen tolkas inte"
                }
            )),
            None => md.push_str(&format!("- **Spegel {namn}:** saknas.\n")),
        }
    }

    md.push_str("\n## Aldrig valda av hjärnan (e_lar, ej spegel)\n\n");
    md.push_str("| Frö | Märkt aldrig | Omärkt aldrig | Märkt lägre | Märkt vanligaste | Omärkt vanligaste |\n");
    md.push_str("|-----|--------------|---------------|-------------|------------------|-------------------|\n");
    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        let ml = match (k.markta.aldrig_valda, k.omarkta.aldrig_valda) {
            (Some(m), Some(o)) if m < o => "ja",
            _ => "nej",
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            k.fro,
            fmt_opt(k.markta.aldrig_valda),
            fmt_opt(k.omarkta.aldrig_valda),
            ml,
            fmt_opt(k.markta.vanligaste_andel),
            fmt_opt(k.omarkta.vanligaste_andel),
        ));
    }

    md.push_str("\n## Kopieringskvot per grupp (e_lar, ej spegel)\n\n");
    md.push_str("| Frö | Märkt kvot | Omärkt kvot |\n");
    md.push_str("|-----|------------|-------------|\n");
    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            k.fro,
            fmt_opt(k.markta.kvot_kopiering),
            fmt_opt(k.omarkta.kvot_kopiering),
        ));
    }

    md.push_str("\n## ε vid slutet (märkt grupp, e_lar ej spegel)\n\n");
    md.push_str("| Frö | Medel | Median | Andel < 0,01 | Omärkt medel (neutral) | Omärkt median |\n");
    md.push_str("|-----|-------|--------|--------------|------------------------|---------------|\n");
    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            k.fro,
            fmt_opt(k.eps_medel),
            fmt_opt(k.eps_median),
            fmt_opt(k.eps_andel_under_001),
            fmt_opt(k.eps_omarkt_medel),
            fmt_opt(k.eps_omarkt_median),
        ));
    }

    md.push_str("\n## ε-medel per 10 år (medel över e_lar-frön, ej spegel)\n\n");
    md.push_str("| År | ε-medel |\n");
    md.push_str("|----|--------|\n");
    let mut ar_sum: std::collections::BTreeMap<i64, (f64, usize)> =
        std::collections::BTreeMap::new();
    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        for &(ar, m) in &k.eps_per_10ar {
            let e = ar_sum.entry(ar).or_insert((0.0, 0));
            e.0 += m;
            e.1 += 1;
        }
    }
    for (ar, (sum, n)) in ar_sum {
        if n > 0 {
            md.push_str(&format!("| {} | {:.6} |\n", ar, sum / n as f64));
        }
    }

    md.push_str(&neutral_eps_avsnitt(korningar));

    md.push_str("\n## Utfallsregler (gäller; skrivna före)\n\n");
    for rad in &utfall.rader {
        md.push_str(rad);
        md.push('\n');
    }
    md.push_str("\nRäknat: e_lar aldrig-lägre ");
    md.push_str(&format!(
        "{}/{}; e_lar ε<{} {}/{}; e_lar kopier<1 {}/{}.\n",
        utfall.e_lar_aldrig_lagre,
        utfall.e_lar_n,
        EPS_ROD_MAX,
        utfall.e_lar_eps_under,
        utfall.e_lar_n,
        utfall.e_lar_kopier_under_1,
        utfall.e_lar_n
    ));

    md
}

pub fn kor_utforsk(ut_dir: &Path, torr: bool) -> Result<(), String> {
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

    let rattad = RATTAD.load(Ordering::Relaxed);
    let prefix = match (rattad, torr) {
        (true, true) => "utforsk1806-rattad-torr",
        (true, false) => "utforsk1806-rattad",
        (false, _) => "utforsk1463",
    };
    let jsonl_path = ut_dir.join(format!("{prefix}-korningar.jsonl"));
    let md_path = ut_dir.join(format!("{prefix}-sammanfattning.md"));
    let tsv_path = ut_dir.join(format!("{prefix}-tabell.tsv"));

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let mut tsv = String::from(
        "arm\tspegel\tfro\tandel_markta\taldrig_markta\taldrig_omarkta\tkvot_markta\tkvot_omarkta\teps_medel\teps_median\teps_andel_under_001\n",
    );
    for k in &korningar {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            k.arm,
            k.spegel,
            k.fro,
            fmt_f64(k.andel_markta),
            fmt_opt(k.markta.aldrig_valda),
            fmt_opt(k.omarkta.aldrig_valda),
            fmt_opt(k.markta.kvot_kopiering),
            fmt_opt(k.omarkta.kvot_kopiering),
            fmt_opt(k.eps_medel),
            fmt_opt(k.eps_median),
            fmt_opt(k.eps_andel_under_001),
        ));
    }
    std::fs::write(&tsv_path, &tsv).map_err(|e| format!("skriv {}: {e}", tsv_path.display()))?;

    let md = skriv_sammanfattning(&korningar, torr, total_s);
    print!("{md}");
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!(
        "skrev {} ({} körningar), {} och {} på {:.1} s",
        jsonl_path.display(),
        korningar.len(),
        tsv_path.display(),
        md_path.display(),
        total_s
    );
    Ok(())
}

/// Claudes tillägg efter körningen: ε vid slutet per arm och grupp. Bara `lar_utforsk` använder sin ε; i alla andra
/// grupper är ε ärvd men oanvänd (neutral drift med gränsen vid 0). Jämförelsen per frö är räknad efter körningen
/// och är ingen regel skriven före.
pub fn neutral_eps_avsnitt(korningar: &[Korning]) -> String {
    let mut md = String::from("\n## ε vid slutet per arm och grupp (Claudes tillägg efter körningen)\n\n");
    md.push_str("Bara gruppen `lar_utforsk` använder sin ε; i övriga grupper är ε ärvd men oanvänd (neutral).\n\n");
    md.push_str("| Arm | Spegel | Märkt grupp | Märktas ε (frön) | Omärkt grupp | Omärktas ε (frön) |\n");
    md.push_str("|-----|--------|-------------|------------------|--------------|-------------------|\n");
    let mut nycklar: Vec<(String, bool)> = Vec::new();
    for k in korningar {
        let ny = (k.arm.clone(), k.spegel);
        if !nycklar.contains(&ny) {
            nycklar.push(ny);
        }
    }
    let medel = |v: Vec<f64>| -> String {
        if v.is_empty() {
            "–".into()
        } else {
            format!("{:.4} ({})", v.iter().sum::<f64>() / v.len() as f64, v.len())
        }
    };
    for (arm, spegel) in &nycklar {
        let g: Vec<&Korning> = korningar.iter().filter(|k| &k.arm == arm && k.spegel == *spegel).collect();
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            arm,
            if *spegel { "ja" } else { "nej" },
            g[0].markta_formaga,
            medel(g.iter().filter_map(|k| k.eps_medel).collect()),
            g[0].omarkta_formaga,
            medel(g.iter().filter_map(|k| k.eps_omarkt_medel).collect()),
        ));
    }
    // Per frö: e_lar:s märkta (använd ε) mot e_fast_lar:s båda grupper (oanvänd ε) i samma frö.
    let mut hogre = 0usize;
    let mut par = 0usize;
    for k in korningar.iter().filter(|k| k.arm == "e_lar" && !k.spegel) {
        let Some(e) = k.eps_medel else { continue };
        let n: Vec<f64> = korningar
            .iter()
            .filter(|x| x.arm == "e_fast_lar" && !x.spegel && x.fro == k.fro)
            .flat_map(|x| [x.eps_medel, x.eps_omarkt_medel])
            .flatten()
            .collect();
        if n.is_empty() {
            continue;
        }
        par += 1;
        if e > n.iter().sum::<f64>() / n.len() as f64 {
            hogre += 1;
        }
    }
    md.push_str(&format!(
        "\nPer frö: e_lar:s märktas ε högre än den neutrala ε i e_fast_lar (båda grupperna, samma frö) i {hogre}/{par} frön.\n"
    ));
    md
}

fn gm_fran(v: &serde_json::Value) -> Result<GruppMat, String> {
    let f = |n: &str| v.get(n).and_then(|x| x.as_f64());
    Ok(GruppMat {
        formaga: v.get("formaga").and_then(|x| x.as_str()).ok_or("formaga saknas")?.to_string(),
        aldrig_valda: f("aldrig_valda"),
        vanligaste_andel: f("vanligaste_andel"),
        n_aldrig_djur: v.get("n_aldrig_djur").and_then(|x| x.as_u64()).ok_or("n_aldrig_djur saknas")?,
        medel_forutsag: f("medel_forutsag"),
        medel_kopiering: f("medel_kopiering"),
        kvot_kopiering: f("kvot_kopiering"),
        djursteg_kopier: v.get("djursteg_kopier").and_then(|x| x.as_u64()).ok_or("djursteg_kopier saknas")?,
    })
}

/// Läser en rad ur `utforsk1463-korningar.jsonl` tillbaka till en körning (för `--omrakna`).
pub fn korning_fran_json(rad: &str) -> Result<Korning, String> {
    assert!(!rad.contains("TORRKÖRNING"), "torrkörningens rader räknas inte om");
    let v: serde_json::Value = serde_json::from_str(rad).map_err(|e| format!("json: {e}"))?;
    let f = |n: &str| v.get(n).and_then(|x| x.as_f64());
    let u = |n: &str| v.get(n).and_then(|x| x.as_u64()).ok_or(format!("{n} saknas"));
    let st = |n: &str| v.get(n).and_then(|x| x.as_str()).map(|s| s.to_string()).ok_or(format!("{n} saknas"));
    let eps10 = v
        .get("eps_per_10ar")
        .and_then(|x| x.as_array())
        .ok_or("eps_per_10ar saknas")?
        .iter()
        .map(|e| Ok((e.get("ar").and_then(|x| x.as_i64()).ok_or("ar")?, e.get("eps_medel").and_then(|x| x.as_f64()).ok_or("eps_medel")?)))
        .collect::<Result<Vec<(i64, f64)>, &str>>()?;
    Ok(Korning {
        arm: st("arm")?,
        spegel: v.get("spegel").and_then(|x| x.as_bool()).ok_or("spegel saknas")?,
        fro: u("fro")?,
        antal_markering: u("antal_levande_vid_markning")? as usize,
        antal_slut: u("antal_levande_vid_slut")? as usize,
        markta_slut: u("markta_vid_slut")? as usize,
        andel_markta: f("andel_markta").ok_or("andel_markta saknas")?,
        kortid_s: f("kortid_s").ok_or("kortid_s saknas")?,
        torr: false,
        markta_formaga: st("markta_formaga")?,
        omarkta_formaga: st("omarkta_formaga")?,
        markta: gm_fran(v.get("markta").ok_or("markta saknas")?)?,
        omarkta: gm_fran(v.get("omarkta").ok_or("omarkta saknas")?)?,
        eps_medel: f("eps_medel"),
        eps_median: f("eps_median"),
        eps_andel_under_001: f("eps_andel_under_001"),
        eps_per_10ar: eps10,
        eps_omarkt_medel: f("eps_omarkt_medel"),
        eps_omarkt_median: f("eps_omarkt_median"),
    })
}

/// `hcsim utforsk --omrakna`: skriver sammanfattningen igen ur körfilen (samma körningar, nya avsnitt).
pub fn omrakna_utforsk(ut_dir: &Path) -> Result<(), String> {
    let jsonl_path = ut_dir.join("utforsk1463-korningar.jsonl");
    let md_path = ut_dir.join("utforsk1463-sammanfattning.md");
    let text = std::fs::read_to_string(&jsonl_path).map_err(|e| format!("läs {}: {e}", jsonl_path.display()))?;
    let mut korningar = Vec::new();
    for rad in text.lines().filter(|r| !r.trim().is_empty()) {
        korningar.push(korning_fran_json(rad)?);
    }
    let gammal = std::fs::read_to_string(&md_path).map_err(|e| format!("läs {}: {e}", md_path.display()))?;
    let total_s: f64 = gammal
        .lines()
        .find_map(|r| r.strip_prefix("Total körtid: ").and_then(|x| x.strip_suffix(" s")))
        .ok_or("Total körtid saknas i sammanfattningen")?
        .parse()
        .map_err(|e| format!("körtid: {e}"))?;
    let n_jobb = bygg_jobb(false).len();
    if korningar.len() != n_jobb {
        return Err(format!("körfilen har {} körningar, armarna ger {}", korningar.len(), n_jobb));
    }
    korningar.sort_by(|a, b| a.arm.cmp(&b.arm).then(a.spegel.cmp(&b.spegel)).then(a.fro.cmp(&b.fro)));
    let md = skriv_sammanfattning(&korningar, false, total_s);
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!("omräknad {} ur {} körningar", md_path.display(), korningar.len());
    Ok(())
}
