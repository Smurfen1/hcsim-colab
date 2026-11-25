//! Provvy: en levande terminalvy i samma stil som `kartvy` och `hjarnvakt` för `prov2008`-körningar.
//! Visar förlopp, världskarta med färgblock per värld, stigande vinstkurva i X/Y-diagram och
//! senaste klara världarna på enkel svenska utan råa decimaltal (`--siffror` visar tekniska tal).
//!
//! Användning: `provvy [--logg FIL | FIL] [--var-s S] [--en-gang] [--rubrik TEXT] [--mal-r2 F] [--siffror]`

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const ARBETSKATALOG: &str = "/home/hakan/sandbox/pi/work/hcsim";

// Dova 24-bitars färger, samma palett som i `hjarnvakt.rs` och `kartvy.rs`.
const GRON: &str = "\x1b[38;2;127;166;106m";
const ROD: &str = "\x1b[38;2;196;106;90m";
const GUL: &str = "\x1b[38;2;201;169;79m";
const BLA: &str = "\x1b[38;2;114;156;190m";
const LILA: &str = "\x1b[38;2;166;138;184m";
const CYAN: &str = "\x1b[38;2;106;172;174m";
const GRA: &str = "\x1b[38;2;122;122;122m";
const TEXT: &str = "\x1b[38;2;216;212;200m";
const FET: &str = "\x1b[1m";
const AV: &str = "\x1b[0m";

const ARM_SYMBOLER: [(char, &str); 6] = [
    ('●', BLA),
    ('◆', GRON),
    ('▲', GUL),
    ('■', ROD),
    ('★', LILA),
    ('✚', CYAN),
];

#[derive(Clone, Debug, PartialEq)]
struct ResultatRad {
    klockslag: String,
    fro: u64,
    arm: String,
    hjr2_600: f64,
    hjr2_1500: f64,
    hjr2_2700: f64,
    fk: Option<f64>,
    g_d: Option<f64>,
    py_in: Option<f64>,
    pz_in: Option<f64>,
    pop: Option<u64>,
    tid_s: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Forlopp {
    klara: usize,
    totalt: usize,
    kort_min: Option<f64>,
    kvar_min: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct LoggData {
    rubrik: Option<String>,
    armar: Vec<String>,
    rader: Vec<ResultatRad>,
    forlopp: Option<Forlopp>,
    inkorningar: usize,
    planerade_varldar: Option<usize>,
}

fn sv_tal(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse::<f64>().ok()
}

fn sv_fmt(v: f64, dec: usize) -> String {
    format!("{v:.dec$}").replace('.', ",")
}

fn sv_signed(v: f64, dec: usize) -> String {
    let sign = if v > 0.0 { "+" } else { "" };
    format!("{sign}{v:.dec$}").replace('.', ",")
}

fn tolka_forlopp(rad: &str) -> Option<Forlopp> {
    let start = rad.find("[klara ")?;
    let slut = rad[start..].find(']')? + start;
    let inre = &rad[start + 1..slut];
    let mut klara = None;
    let mut totalt = None;
    let mut kort_min = None;
    let mut kvar_min = None;

    for del in inre.split(',') {
        let d = del.trim();
        if let Some(rest) = d.strip_prefix("klara ") {
            if let Some((a, b)) = rest.split_once('/') {
                klara = a.trim().parse().ok();
                totalt = b.trim().parse().ok();
            }
        } else if let Some(rest) = d.strip_prefix("kört ") {
            let s = rest.trim_end_matches("min").trim();
            kort_min = sv_tal(s);
        } else if let Some(rest) = d.strip_prefix("kvar ") {
            let s = rest.trim_start_matches('~').trim_end_matches("min").trim();
            kvar_min = sv_tal(s);
        }
    }

    Some(Forlopp {
        klara: klara?,
        totalt: totalt?,
        kort_min,
        kvar_min,
    })
}

fn las_logg(text: &str) -> LoggData {
    let mut data = LoggData::default();
    let mut sedda_armar: HashMap<String, usize> = HashMap::new();

    for rad in text.lines() {
        let r = rad.trim();
        if r.is_empty() {
            continue;
        }
        if data.rubrik.is_none() {
            if let Some(idx) = r.find("Startar ") {
                let rubrik = r[idx + "Startar ".len()..].trim();
                if !rubrik.is_empty() {
                    data.rubrik = Some(rubrik.to_string());
                    if let Some(p_idx) = rubrik.find(" på ") {
                        let efter = &rubrik[p_idx + " på ".len()..];
                        let ord: Vec<&str> = efter.split_whitespace().collect();
                        if ord.len() >= 2 && ord[1].starts_with("värld") {
                            if let Ok(n) = ord[0].parse::<usize>() {
                                if n > 0 {
                                    data.planerade_varldar = Some(n);
                                }
                            }
                        }
                    }
                }
            }
        }

        if r.contains("Inkörning ") && r.contains(" klar") {
            data.inkorningar += 1;
        }
        let Some(fro_pos) = r.find("Frö ") else {
            continue;
        };

        let mut delar = r.split_whitespace();
        let d0 = delar.next().unwrap_or("");
        let d1 = delar.next().unwrap_or("");
        let klockslag = if d0.len() == 10 && d0.contains('-') && d1.contains(':') {
            d1.to_string()
        } else {
            String::new()
        };

        let svans = &r[fro_pos + "Frö ".len()..];
        let ord: Vec<&str> = svans.split_whitespace().collect();
        if ord.len() < 3 {
            continue;
        }
        let Ok(fro) = ord[0].parse::<u64>() else {
            continue;
        };
        let arm = ord[1].trim_end_matches(':').to_string();
        if arm.is_empty() {
            continue;
        }

        let mut hjr2 = None;
        let mut fk = None;
        let mut g_d = None;
        let mut py_in = None;
        let mut pz_in = None;
        let mut pop = None;
        let mut tid_s = None;

        for tok in &ord[2..] {
            if tok.starts_with('[') {
                break;
            }
            if let Some(rest) = tok.strip_prefix("hjR²(600/1500/2700)=") {
                let p: Vec<&str> = rest.split('/').collect();
                if p.len() == 3 {
                    if let (Some(a), Some(b), Some(c)) = (sv_tal(p[0]), sv_tal(p[1]), sv_tal(p[2])) {
                        hjr2 = Some((a, b, c));
                    }
                }
            } else if let Some(rest) = tok.strip_prefix("fk=") {
                fk = sv_tal(rest);
            } else if let Some(rest) = tok.strip_prefix("g_d=") {
                g_d = sv_tal(rest);
            } else if let Some(rest) = tok.strip_prefix("g(v/d/E)=") {
                let p: Vec<&str> = rest.split('/').collect();
                if p.len() == 3 {
                    g_d = sv_tal(p[1]);
                }
            } else if let Some(rest) = tok.strip_prefix("py_in=") {
                py_in = sv_tal(rest);
            } else if let Some(rest) = tok.strip_prefix("inom_py(in/dE)=") {
                let p: Vec<&str> = rest.split('/').collect();
                if !p.is_empty() {
                    py_in = sv_tal(p[0]);
                }
            } else if let Some(rest) = tok.strip_prefix("pz_in=") {
                pz_in = sv_tal(rest);
            } else if let Some(rest) = tok.strip_prefix("inom_pz(in/dE)=") {
                let p: Vec<&str> = rest.split('/').collect();
                if !p.is_empty() {
                    pz_in = sv_tal(p[0]);
                }
            } else if let Some(rest) = tok.strip_prefix("pop=") {
                pop = rest.parse::<u64>().ok();
            } else if tok.starts_with('(') && tok.ends_with("s)") {
                let inre = &tok[1..tok.len() - 2];
                tid_s = sv_tal(inre);
            }
        }

        let Some((h600, h1500, h2700)) = hjr2 else {
            continue;
        };

        if let Some(f) = tolka_forlopp(r) {
            data.forlopp = Some(f);
        }

        if !sedda_armar.contains_key(&arm) {
            sedda_armar.insert(arm.clone(), data.armar.len());
            data.armar.push(arm.clone());
        }

        data.rader.push(ResultatRad {
            klockslag,
            fro,
            arm,
            hjr2_600: h600,
            hjr2_1500: h1500,
            hjr2_2700: h2700,
            fk,
            g_d,
            py_in,
            pz_in,
            pop,
            tid_s,
        });
    }

    data
}

fn median(mut vals: Vec<f64>) -> Option<f64> {
    vals.retain(|x| x.is_finite());
    if vals.is_empty() {
        return None;
    }
    vals.sort_by(|a, b| a.total_cmp(b));
    let n = vals.len();
    if n % 2 == 1 {
        Some(vals[n / 2])
    } else {
        Some(0.5 * (vals[n / 2 - 1] + vals[n / 2]))
    }
}

fn klartext_namn(arm: &str) -> String {
    match arm {
        "D2_Bas" => "Grundmodell".into(),
        "D2_Adaptiv" => "Långsam skalning".into(),
        "D2_RMS" => "Absolut skalfaktor".into(),
        "D2_FrysZ" => "Fryst inre bild".into(),
        "D2_Osak20" => "Osäkerhet (fast)".into(),
        "D2_OsakAvt" => "Osäkerhet (ung→vuxen)".into(),
        "D2_UCB" => "Osäkerhetsbonus".into(),
        "Lar2144_D2_a4" => "Nio fristående modeller".into(),
        "Lar2116_M20" => "Tvåstegsmodell".into(),
        _ => arm.to_string(),
    }
}

fn brusskydd_ok(g_d: Option<f64>) -> bool {
    g_d.map_or(true, |g| g <= 0.10)
}

fn varld_godkand(r: &ResultatRad) -> bool {
    r.hjr2_2700 > 0.0 && brusskydd_ok(r.g_d)
}

fn varld_starkt_godkand(r: &ResultatRad, mal_r2: f64) -> bool {
    r.hjr2_2700 >= mal_r2 && brusskydd_ok(r.g_d)
}

/// Kumulativt antal godkända världar efter varje färdig värld (i slutförandeföljd, som i `hjarnvakt`).
fn bana(rader: &[ResultatRad], arm: &str) -> Vec<usize> {
    let mut w = 0usize;
    let mut ut = Vec::new();
    for r in rader.iter().filter(|r| r.arm == arm) {
        if varld_godkand(r) {
            w += 1;
        }
        ut.push(w);
    }
    ut
}

fn opt_med_str(v: Option<f64>, dec: usize) -> String {
    match v {
        Some(x) => sv_fmt(x, dec),
        None => "  -  ".into(),
    }
}

fn farg_for_arm(klara: usize, med_hj2700: f64, med_gd: Option<f64>, mal_r2: f64) -> &'static str {
    let gd_ok = brusskydd_ok(med_gd);
    if med_hj2700 >= mal_r2 && gd_ok {
        GRON
    } else if med_hj2700 < 0.0 || !gd_ok || (klara >= 10 && med_hj2700 < mal_r2) {
        ROD
    } else {
        GUL
    }
}

fn rita_siffertabell(s: &mut String, data: &LoggData, mal_r2: f64) {
    s.push_str(&format!(
        "{FET}{TEXT}{:<23} {:>5}  {:>7}  {:>5}  {:>5}  {:>5}  {:>6}  {:>6}  {:>6}  {}{AV}\n",
        "Variant (tekniska tal)",
        "Klara",
        "hjR²",
        ">0",
        "fk",
        "g_d",
        "pz_in",
        "py_in",
        "Pop",
        "Kurva (600→1500→2700)"
    ));
    s.push_str(&format!("{GRA}{}{AV}\n", "─".repeat(94)));

    for arm in &data.armar {
        let rader: Vec<&ResultatRad> = data.rader.iter().filter(|r| &r.arm == arm).collect();
        let klara = rader.len();
        let m600 = median(rader.iter().map(|r| r.hjr2_600).collect()).unwrap_or(0.0);
        let m1500 = median(rader.iter().map(|r| r.hjr2_1500).collect()).unwrap_or(0.0);
        let m2700 = median(rader.iter().map(|r| r.hjr2_2700).collect()).unwrap_or(0.0);
        let pos = rader.iter().filter(|r| r.hjr2_2700 > 0.0).count();
        let m_fk = median(rader.iter().filter_map(|r| r.fk).collect());
        let m_gd = median(rader.iter().filter_map(|r| r.g_d).collect());
        let m_pz = median(rader.iter().filter_map(|r| r.pz_in).collect());
        let m_py = median(rader.iter().filter_map(|r| r.py_in).collect());
        let m_pop = median(rader.iter().filter_map(|r| r.pop.map(|p| p as f64)).collect());
        let farg = farg_for_arm(klara, m2700, m_gd, mal_r2);
        let kurva = format!("{} → {} → {}", sv_fmt(m600, 3), sv_fmt(m1500, 3), sv_fmt(m2700, 3));
        let pop_s = match m_pop {
            Some(p) => format!("{:.0}", p.round()),
            None => "-".into(),
        };
        s.push_str(&format!(
            "{farg}{FET}{:<23}{AV} {:>5}  {farg}{:>7}{AV}  {:>2}/{:<2}  {:>5}  {:>5}  {:>6}  {:>6}  {:>6}  {kurva}\n",
            klartext_namn(arm),
            klara,
            sv_signed(m2700, 3),
            pos,
            klara,
            opt_med_str(m_fk, 3),
            opt_med_str(m_gd, 3),
            opt_med_str(m_pz, 3),
            opt_med_str(m_py, 3),
            pop_s,
        ));
    }
    s.push('\n');
}

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Debug, Default)]
struct LiveStatus {
    aktiv: bool,
    karnor: usize,
    tradar: usize,
    omgang_s: u64,
    tick: usize,
}

fn format_min_sek(sek: u64) -> String {
    let m = sek / 60;
    let s = sek % 60;
    if m > 0 {
        format!("{m} min {s:02} s")
    } else {
        format!("{s} s")
    }
}

fn las_proc_cpu_ticks(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let slut = text.rfind(')')?;
    let falt: Vec<&str> = text[slut + 2..].split_whitespace().collect();
    let utime: u64 = falt.get(11)?.parse().ok()?;
    let stime: u64 = falt.get(12)?.parse().ok()?;
    Some(utime + stime)
}

fn las_proc_tradar(pid: u32) -> usize {
    let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
        return 0;
    };
    for rad in text.lines() {
        if let Some(rest) = rad.strip_prefix("Threads:") {
            if let Ok(n) = rest.trim().parse::<usize>() {
                return n;
            }
        }
    }
    0
}

fn logg_sparb_prefix(logg_vag: Option<&std::path::Path>) -> Option<String> {
    let p = logg_vag?;
    for del in p.iter() {
        if let Some(s) = del.to_str() {
            if let Some(nummer) = s.strip_prefix("alt") {
                if !nummer.is_empty() && nummer.chars().all(|c| c.is_ascii_digit()) {
                    return Some(format!("sparb{nummer}"));
                }
            }
        }
    }
    None
}

fn hitta_aktiv_prov2008(logg_vag: Option<&std::path::Path>) -> Option<(u32, usize)> {
    let forvantad_sub = logg_sparb_prefix(logg_vag);
    let poster = std::fs::read_dir("/proc").ok()?;
    for post in poster.flatten() {
        let namn = post.file_name();
        let Some(pid_str) = namn.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let Ok(raw) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let args: Vec<&str> = raw
            .split(|&b| b == 0)
            .filter(|s| !s.is_empty())
            .filter_map(|s| std::str::from_utf8(s).ok())
            .collect();
        if args.len() < 2 {
            continue;
        }
        let prog_ok = args[0].ends_with("/prov2008") || args[0] == "prov2008";
        let sub_ok = args[1].starts_with("sparb");
        if !prog_ok || !sub_ok {
            continue;
        }
        if let Some(ref forv) = forvantad_sub {
            if args[1] != forv {
                continue;
            }
        }
        let tradar = las_proc_tradar(pid);
        return Some((pid, tradar));
    }
    None
}

fn rita_bild(
    rubrik: &str,
    data: &LoggData,
    mal_r2: f64,
    logg_andrad: Option<SystemTime>,
    live: &LiveStatus,
    siffror: bool,
) -> String {
    let mut s = String::new();
    let nu = klockslag_hms(SystemTime::now());
    let visad_rubrik = data.rubrik.as_deref().unwrap_or(rubrik);

    s.push_str(&format!("{FET}{TEXT}PROVVY{AV}  {visad_rubrik}"));
    if let Some(t) = logg_andrad {
        s.push_str(&format!("   {GRA}logg ändrad {}{AV}", klockslag_hms(t)));
    }
    s.push('\n');

    // Levande sekund-för-sekund-indikator längst upp
    if live.aktiv {
        let sp = SPINNER[live.tick % SPINNER.len()];
        let karn_vis = if live.karnor > 0 { live.karnor } else { live.tradar.min(16).max(1) };
        let cykel_s = median(data.rader.iter().filter_map(|r| r.tid_s).collect())
            .map(|t| (t.round() as u64).max(60))
            .unwrap_or(480);
        let bredd = 16usize;
        let fyllda = ((live.omgang_s.min(cykel_s) * bredd as u64) / cykel_s.max(1)) as usize;
        let f_str = "█".repeat(fyllda);
        let t_str = "░".repeat(bredd.saturating_sub(fyllda));
        s.push_str(&format!(
            "{GUL}{FET}{sp} ARBETAR NU{AV}  {TEXT}kl. {nu}{AV} · {karn_vis} kärnor arbetar · pågående omgång: {}  [{GUL}{f_str}{AV}{GRA}{t_str}{AV}]\n",
            format_min_sek(live.omgang_s),
        ));
    } else {
        s.push_str(&format!(
            "{GRON}● AVSLUTAD{AV}  {TEXT}kl. {nu}{AV} · {GRA}ingen aktiv körning för denna logg{AV}\n"
        ));
    }

    // Panel 1: Förloppslist i klartext (inga decimalkomman)
    match &data.forlopp {
        Some(f) if f.totalt > 0 => {
            let bredd = 40usize;
            let andel = (f.klara as f64 / f.totalt as f64).clamp(0.0, 1.0);
            let fyllda = (andel * bredd as f64).round() as usize;
            let farg = if f.klara >= f.totalt { GRON } else { GUL };
            let proc = (andel * 100.0).round() as usize;
            let fylld_str: String = "█".repeat(fyllda);
            let tom_str: String = "░".repeat(bredd.saturating_sub(fyllda));
            s.push_str(&format!(
                "Förlopp: [{farg}{fylld_str}{AV}{GRA}{tom_str}{AV}] {FET}{} av {} världar klara{AV} ({proc} %)",
                f.klara, f.totalt
            ));
            if f.klara < f.totalt {
                if let Some(kv) = f.kvar_min {
                    let min_avrund = (kv.round() as u64).max(1);
                    s.push_str(&format!(" · kvar cirka {min_avrund} min"));
                }
            } else if let Some(k) = f.kort_min {
                let min_avrund = k.round() as u64;
                s.push_str(&format!(" · {GRON}körning klar på {min_avrund} min{AV}"));
            } else {
                s.push_str(&format!(" · {GRON}körning klar{AV}"));
            }
            s.push('\n');
        }
        _ => {
            if data.inkorningar > 0 {
                s.push_str(&format!(
                    "Förlopp: {GUL}inkörning klar ({} världar) · första omgången kör nu (första resultaten dröjer cirka 8–9 min){AV}\n",
                    data.inkorningar
                ));
            } else {
                s.push_str(&format!("Förlopp: {GUL}väntar på första raden i loggen{AV}\n"));
            }
        }
    }
    s.push('\n');

    if data.rader.is_empty() {
        if data.inkorningar > 0 {
            s.push_str(&format!(
                "{GRA}Arbetar med första omgången världar — grafen fylls i automatiskt så snart första världen blir klar.{AV}\n"
            ));
        } else {
            s.push_str(&format!("{GRA}Väntar på att testet ska skriva första raden...{AV}\n"));
        }
        return s;
    }

    let max_klara = data
        .armar
        .iter()
        .map(|a| data.rader.iter().filter(|r| &r.arm == a).count())
        .max()
        .unwrap_or(1)
        .max(1);
    let n_per_arm = data
        .planerade_varldar
        .or_else(|| (data.inkorningar > 0).then_some(data.inkorningar))
        .map(|v| v.max(max_klara))
        .unwrap_or_else(|| match &data.forlopp {
            Some(f) if f.totalt > 0 && !data.armar.is_empty() && f.totalt % data.armar.len() == 0 => {
                (f.totalt / data.armar.len()).max(max_klara)
            }
            _ => max_klara.max(20),
        });
    let mal_antal = (n_per_arm * 3).div_ceil(4).max(1); // 15 av 20
    let par_krav = (n_per_arm * 3).div_ceil(5).max(1);  // 12 av 20

    let alla_fron: Vec<u64> = data
        .rader
        .iter()
        .map(|r| r.fro)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    // Panel 2: Världskarta per variant (kartvy-stil med färgade rutor per värld)
    s.push_str(&format!(
        "{FET}{TEXT}Världskarta per variant{AV}  {GRA}(mål: minst {mal_antal} godkända av {n_per_arm} världar){AV}\n"
    ));
    let bas_namn = data.armar.first().cloned().unwrap_or_default();
    let mut bas_per_fro: HashMap<u64, &ResultatRad> = HashMap::new();
    for r in data.rader.iter().filter(|r| r.arm == bas_namn) {
        bas_per_fro.insert(r.fro, r);
    }

    for (ai, arm) in data.armar.iter().enumerate() {
        let mut arm_per_fro: HashMap<u64, &ResultatRad> = HashMap::new();
        let mut rader_arm: Vec<&ResultatRad> = Vec::new();
        for r in data.rader.iter().filter(|r| &r.arm == arm) {
            arm_per_fro.insert(r.fro, r);
            rader_arm.push(r);
        }

        let klara = rader_arm.len();
        let kvar = n_per_arm.saturating_sub(klara);
        let godkanda = rader_arm.iter().filter(|r| varld_godkand(r)).count();
        let m2700 = median(rader_arm.iter().map(|r| r.hjr2_2700).collect()).unwrap_or(0.0);
        let m_gd = median(rader_arm.iter().filter_map(|r| r.g_d).collect());
        let brus_ok = brusskydd_ok(m_gd);

        let (dom_txt, dom_farg) = if godkanda >= mal_antal && m2700 >= mal_r2 && brus_ok {
            ("GODKÄND", GRON)
        } else if godkanda + kvar < mal_antal || !brus_ok || (klara >= n_per_arm && m2700 < mal_r2) {
            ("KAN INTE NÅ MÅL", ROD)
        } else {
            ("PÅGÅR", GUL)
        };

        let mut rutor = String::new();
        for idx in 0..n_per_arm {
            if let Some(&fro) = alla_fron.get(idx) {
                if let Some(r) = arm_per_fro.get(&fro) {
                    if varld_starkt_godkand(r, mal_r2) {
                        rutor.push_str(&format!("{GRON}█{AV}"));
                    } else if varld_godkand(r) {
                        rutor.push_str(&format!("{GUL}▓{AV}"));
                    } else {
                        rutor.push_str(&format!("{ROD}░{AV}"));
                    }
                    continue;
                }
            }
            rutor.push_str(&format!("{GRA}·{AV}"));
        }

        let par_str = if ai == 0 {
            format!("{GRA}(jämförelsegrund){AV}")
        } else {
            let mut par_klara = 0usize;
            let mut par_vunna = 0usize;
            for (&fro, r) in &arm_per_fro {
                if let Some(br) = bas_per_fro.get(&fro) {
                    par_klara += 1;
                    if r.hjr2_2700 > br.hjr2_2700 {
                        par_vunna += 1;
                    }
                }
            }
            if par_klara == 0 {
                format!("{GRA}väntar på jämförelse{AV}")
            } else {
                let par_kvar = n_per_arm.saturating_sub(par_klara);
                let pf = if par_vunna >= par_krav {
                    GRON
                } else if par_vunna + par_kvar < par_krav {
                    ROD
                } else {
                    GUL
                };
                format!("{pf}bättre än grund i {par_vunna} av {par_klara}{AV}")
            }
        };

        let brus_str = if brus_ok {
            format!("{GRON}brusskydd: bra{AV}")
        } else {
            format!("{ROD}brusskydd: svagt{AV}")
        };

        let etikett = klartext_namn(arm);
        s.push_str(&format!(
            "  {dom_farg}{FET}{etikett:<22}{AV} [{rutor}]  {FET}{godkanda:>2} av {klara:<2}{AV} godkända  {dom_farg}{FET}{dom_txt:<16}{AV}  {par_str:<32}  {brus_str}\n"
        ));
    }
    s.push_str(&format!(
        "  {GRA}Rutor: {GRON}█{GRA} starkt godkänd · {GUL}▓{GRA} godkänd · {ROD}░{GRA} under krav · · väntar{AV}\n\n"
    ));

    // Panel 3: Begriplig X/Y-graf i hjärnvaktsstil (stigande vinstkurva, inga decimaltal)
    s.push_str(&format!(
        "{FET}{TEXT}Antal godkända världar allt eftersom testet går{AV}  {GRA}(mål: nå upp till {mal_antal}-linjen){AV}\n"
    ));
    let banor: Vec<Vec<usize>> = data.armar.iter().map(|a| bana(&data.rader, a)).collect();
    let kol = 3usize;
    for y in (0..=n_per_arm).rev() {
        let etikett = if y == mal_antal {
            format!("{y:>2} MÅL ")
        } else if y % 5 == 0 {
            format!("{y:>2}     ")
        } else {
            "       ".into()
        };
        s.push_str(&format!("{GRA}{etikett}{AV}"));
        s.push_str(if y == 0 { "└" } else { "│" });
        for x in 1..=n_per_arm {
            let mut traff: Vec<(char, &'static str)> = Vec::new();
            for (ai, b) in banor.iter().enumerate() {
                if b.get(x - 1).copied() == Some(y) {
                    traff.push(ARM_SYMBOLER[ai % ARM_SYMBOLER.len()]);
                }
            }
            let punkt = if traff.len() > 1 {
                format!("{FET}{TEXT}+{AV}")
            } else if traff.len() == 1 {
                let (ch, farg) = traff[0];
                format!("{farg}{ch}{AV}")
            } else if y == mal_antal && y != 0 {
                format!("{GRA}─{AV}")
            } else if y == 0 {
                "─".into()
            } else {
                " ".into()
            };
            s.push_str(&punkt);
            let fyll = if y == 0 {
                "─".into()
            } else if y == mal_antal {
                format!("{GRA}─{AV}")
            } else {
                " ".into()
            };
            for _ in 1..kol {
                s.push_str(&fyll);
            }
        }
        s.push('\n');
    }
    s.push_str("        ");
    for x in 1..=n_per_arm {
        if x == 1 || x % 5 == 0 || x == n_per_arm {
            s.push_str(&format!("{x:<kol$}"));
        } else {
            s.push_str(&" ".repeat(kol));
        }
    }
    s.push_str(&format!("\n        {GRA}klara världar →{AV}   "));
    for (ai, arm) in data.armar.iter().enumerate() {
        let (sym, farg) = ARM_SYMBOLER[ai % ARM_SYMBOLER.len()];
        s.push_str(&format!("{farg}{sym}{AV} {}   ", klartext_namn(arm)));
    }
    s.push_str(&format!("{GRA}(+ = flera kurvor på samma höjd){AV}\n\n"));

    // Panel 4: Senaste klara världarna i klartext
    s.push_str(&format!("{FET}{TEXT}Senaste färdiga världarna:{AV}\n"));
    let start = data.rader.len().saturating_sub(5);
    for r in data.rader[start..].iter().rev() {
        let brus_ok = brusskydd_ok(r.g_d);
        let (dom, df) = if varld_starkt_godkand(r, mal_r2) {
            ("Starkt godkänd", GRON)
        } else if varld_godkand(r) {
            ("Godkänd", GUL)
        } else {
            ("Under krav", ROD)
        };
        let brus = if brus_ok {
            format!("{GRON}bra brusskydd{AV}")
        } else {
            format!("{ROD}svagt brusskydd{AV}")
        };
        let tid = r.klockslag.get(..5).unwrap_or(if r.klockslag.is_empty() { "--:--" } else { &r.klockslag });
        s.push_str(&format!(
            "  {GRA}{tid}{AV}  Värld {:<4}  {:<22}  {df}{dom:<15}{AV} · {brus}\n",
            r.fro,
            klartext_namn(&r.arm),
        ));
    }

    if siffror {
        s.push('\n');
        rita_siffertabell(&mut s, data, mal_r2);
    }
    s
}

/// Svensk tid med sekunder (Europe/Stockholm) ur `date`, så att sommartiden blir rätt.
fn klockslag_hms(t: SystemTime) -> String {
    let sek = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::process::Command::new("date")
        .env("TZ", "Europe/Stockholm")
        .args(["-d", &format!("@{sek}"), "+%H:%M:%S"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .unwrap_or_else(|| "?".into())
}

/// Ritar en bild utan flimmer (samma teknik som `hjarnvakt.rs`).
pub fn utan_flimmer(bild: &str) -> String {
    let mut ut = String::with_capacity(bild.len() + 64);
    ut.push_str("\x1b[?2026h\x1b[H");
    for rad in bild.split_inclusive('\n') {
        match rad.strip_suffix('\n') {
            Some(r) => {
                ut.push_str(r);
                ut.push_str("\x1b[K\n");
            }
            None => ut.push_str(rad),
        }
    }
    ut.push_str("\x1b[J\x1b[?2026l");
    ut
}

fn skriv_en_gang(s: &str) {
    use std::io::Write;
    let mut ut = std::io::stdout().lock();
    let _ = ut.write_all(s.as_bytes());
    let _ = ut.flush();
}

fn arg(args: &[String], namn: &str) -> Option<String> {
    args.iter().position(|a| a == namn).and_then(|i| args.get(i + 1)).cloned()
}

fn forsta_positionella(args: &[String]) -> Option<String> {
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--logg" || a == "--var-s" || a == "--rubrik" || a == "--mal-r2" {
            i += 2;
            continue;
        }
        if a.starts_with('-') {
            i += 1;
            continue;
        }
        return Some(a.clone());
    }
    None
}

/// Letar upp den senast ändrade `alt*/korning.logg` under `rot`.
fn senaste_logg_i(rot: &std::path::Path) -> Option<PathBuf> {
    let poster = std::fs::read_dir(rot).ok()?;
    let mut bast: Option<(SystemTime, PathBuf)> = None;
    for post in poster.flatten() {
        let mapp = post.path();
        if !mapp.is_dir() {
            continue;
        }
        let namn = mapp.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !namn.starts_with("alt") {
            continue;
        }
        let kandidat = mapp.join("korning.logg");
        let Ok(meta) = std::fs::metadata(&kandidat) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let tid = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let byt = match &bast {
            None => true,
            Some((bt, bp)) => tid > *bt || (tid == *bt && kandidat > *bp),
        };
        if byt {
            bast = Some((tid, kandidat));
        }
    }
    bast.map(|x| x.1)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--hjalp" || a == "-h") {
        println!("Användning: provvy [--logg FIL | FIL] [--var-s S] [--en-gang] [--rubrik TEXT] [--mal-r2 F] [--siffror]");
        println!("Visar världskarta och stigande vinstkurva för prov2008-loggar utan råa siffror (--siffror visar tekniska tal).");
        println!("Utan filnamn väljs automatiskt senaste alt*/korning.logg under {ARBETSKATALOG}.");
        return;
    }

    let vald_logg = arg(&args, "--logg").or_else(|| forsta_positionella(&args)).map(PathBuf::from);
    let auto_logg = vald_logg.is_none();
    let mut aktiv_logg = vald_logg;
    let var_s: u64 = arg(&args, "--var-s").and_then(|x| x.parse().ok()).unwrap_or(1).max(1);
    let en_gang = args.iter().any(|a| a == "--en-gang");
    let siffror = args.iter().any(|a| a == "--siffror");
    let egen_rubrik = arg(&args, "--rubrik");
    let mal_r2 = arg(&args, "--mal-r2").and_then(|x| sv_tal(&x)).unwrap_or(0.080);

    let mut senast_fil: Option<PathBuf> = None;
    let mut senast_tid: Option<SystemTime> = None;
    let mut cachad_data: Option<(String, LoggData)> = None;
    let mut forra_cpu: Option<(u32, u64, Instant)> = None;
    let mut tick: usize = 0;

    loop {
        if auto_logg {
            if let Some(ny) = senaste_logg_i(std::path::Path::new(ARBETSKATALOG)) {
                aktiv_logg = Some(ny);
            }
        }
        let andrad = aktiv_logg
            .as_ref()
            .and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        let ny_fil = aktiv_logg != senast_fil;
        if ny_fil || andrad != senast_tid || cachad_data.is_none() {
            cachad_data = aktiv_logg.as_ref().and_then(|logg| {
                let standard_namn = logg
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| logg.display().to_string());
                std::fs::read_to_string(logg).ok().map(|text| {
                    let mut data = las_logg(&text);
                    if let Some(ref r) = egen_rubrik {
                        data.rubrik = Some(r.clone());
                    }
                    (standard_namn, data)
                })
            });
            senast_fil = aktiv_logg.clone();
            senast_tid = andrad;
        }

        let live = if let Some((pid, tradar)) = hitta_aktiv_prov2008(aktiv_logg.as_deref()) {
            let nu_inst = Instant::now();
            let nu_ticks = las_proc_cpu_ticks(pid);
            let mut karnor = 0usize;
            if let Some(nt) = nu_ticks {
                if let Some((fp, ft, fi)) = forra_cpu {
                    if fp == pid {
                        let dt_s = nu_inst.duration_since(fi).as_secs_f64();
                        if dt_s >= 0.05 && nt >= ft {
                            karnor = ((nt - ft) as f64 / (dt_s * 100.0)).round() as usize;
                        }
                    }
                } else if en_gang {
                    if let Some(t0) = Some(nt) {
                        std::thread::sleep(Duration::from_millis(120));
                        if let Some(t1) = las_proc_cpu_ticks(pid) {
                            karnor = ((t1.saturating_sub(t0)) as f64 / (0.12 * 100.0)).round() as usize;
                        }
                    }
                }
                forra_cpu = Some((pid, nt, nu_inst));
            }
            let omgang_s = andrad
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            LiveStatus {
                aktiv: true,
                karnor,
                tradar,
                omgang_s,
                tick,
            }
        } else {
            forra_cpu = None;
            LiveStatus::default()
        };

        let s = match (&aktiv_logg, &cachad_data) {
            (Some(_), Some((standard_namn, data))) => rita_bild(standard_namn, data, mal_r2, andrad, &live, siffror),
            (Some(logg), None) => format!("{ROD}Kan inte läsa loggfilen {}{AV}\n", logg.display()),
            (None, _) => format!("{GUL}Väntar på första loggfilen (alt*/korning.logg) under {ARBETSKATALOG}...{AV}\n"),
        };
        if en_gang {
            print!("{s}");
            return;
        }
        skriv_en_gang(&utan_flimmer(&s));
        tick = tick.wrapping_add(1);
        std::thread::sleep(Duration::from_secs(var_s));
    }
}

#[cfg(test)]
mod prov {
    use super::*;

    #[test]
    fn tolkar_alt2172_rad() {
        let rad = "2026-11-24 19:44:57 Frö 278 D2_Adaptiv: hjR²(600/1500/2700)=-0,130/-0,157/-0,167 fk=0,814 corr_E=0,581 py_in=-0,000 pz_in=0,310 (n=17) g_d=0,209 pop=8726 (520.2s) [klara 40/80, kört 32.8 min, kvar ~32.8 min]";
        let d = las_logg(rad);
        assert_eq!(d.rader.len(), 1);
        let r = &d.rader[0];
        assert_eq!(r.klockslag, "19:44:57");
        assert_eq!(r.fro, 278);
        assert_eq!(r.arm, "D2_Adaptiv");
        assert!((r.hjr2_600 + 0.130).abs() < 1e-6);
        assert!((r.hjr2_1500 + 0.157).abs() < 1e-6);
        assert!((r.hjr2_2700 + 0.167).abs() < 1e-6);
        assert!((r.fk.unwrap() - 0.814).abs() < 1e-6);
        assert!((r.g_d.unwrap() - 0.209).abs() < 1e-6);
        assert!((r.pz_in.unwrap() - 0.310).abs() < 1e-6);
        assert_eq!(r.pop, Some(8726));
        assert!((r.tid_s.unwrap() - 520.2).abs() < 1e-6);
        let f = d.forlopp.unwrap();
        assert_eq!(f.klara, 40);
        assert_eq!(f.totalt, 80);
        assert!((f.kort_min.unwrap() - 32.8).abs() < 1e-6);
        assert!((f.kvar_min.unwrap() - 32.8).abs() < 1e-6);
    }

    #[test]
    fn tolkar_alt2159_rad() {
        let rad = "2026-11-24 18:34:54 Frö 307 Lar2116_M20: hjR²(600/1500/2700)=0,067/0,048/0,027 fk=0,809 inom_py(in/dE)=0,137/-0,000 inom_pz(in/dE)=0,478/0,123 (n=19) g(v/d/E)=1,000/0,040/1,000 pop=11857 (504.2s) [klara 1/40, kört 8.4 min, kvar ~327.7 min]";
        let d = las_logg(rad);
        assert_eq!(d.rader.len(), 1);
        let r = &d.rader[0];
        assert_eq!(r.fro, 307);
        assert_eq!(r.arm, "Lar2116_M20");
        assert!((r.hjr2_2700 - 0.027).abs() < 1e-6);
        assert!((r.g_d.unwrap() - 0.040).abs() < 1e-6);
        assert!((r.py_in.unwrap() - 0.137).abs() < 1e-6);
        assert!((r.pz_in.unwrap() - 0.478).abs() < 1e-6);
        assert_eq!(r.pop, Some(11857));
    }

    #[test]
    fn tolkar_alt2144_rad() {
        let rad = "2026-11-24 17:12:55 Frö 267 Lar2116_M20: hjR²(600/1500/2700)=0,077/0,099/-0,001 fk=0,837 py_in=-0,004 pz_in=-0,015 g(v/d/E)=1,000/0,040/1,000 pop=10462 (458.3s) [klara 1/80, kört 7.6 min, kvar ~603.5 min]";
        let d = las_logg(rad);
        assert_eq!(d.rader.len(), 1);
        let r = &d.rader[0];
        assert!((r.hjr2_2700 + 0.001).abs() < 1e-6);
        assert!((r.g_d.unwrap() - 0.040).abs() < 1e-6);
        assert!((r.py_in.unwrap() + 0.004).abs() < 1e-6);
        assert!((r.pz_in.unwrap() + 0.015).abs() < 1e-6);
    }

    #[test]
    fn ignorerar_skrap_och_inkorning() {
        let logg = "\
2026-11-24 19:12:03 Startar Alt 2172 (Test 2)
2026-11-24 19:12:05 Inkörning frö 269 klar (1995 djur, 1.9s)
skräprad utan innehåll
2026-11-24 19:12:06 Frö inte_ett_tal D2_Bas: hjR²(600/1500/2700)=0,1/0,2/0,3
2026-11-24 19:12:07 Frö 261 D2_Bas: hjR²(600/1500/2700)=trasig/0,2/0,3
";
        let d = las_logg(logg);
        assert_eq!(d.rubrik.as_deref(), Some("Alt 2172 (Test 2)"));
        assert_eq!(d.inkorningar, 1);
        assert!(d.rader.is_empty(), "trasiga och ofullständiga rader ska ignoreras");
    }

    #[test]
    fn median_och_bild_med_extremvarde() {
        assert_eq!(median(vec![0.3, 0.1, 0.2]), Some(0.2));
        assert_eq!(median(vec![0.1, 0.3, 0.2, 0.4]), Some(0.25));
        assert_eq!(median(vec![f64::NAN]), None);

        let logg = "\
2026-11-24 19:12:03 Startar Alt 2172 extremtest
2026-11-24 19:40:00 Frö 261 D2_Bas: hjR²(600/1500/2700)=0,050/0,100/0,140 fk=0,610 py_in=0,040 pz_in=0,420 (n=18) g_d=0,034 pop=10500 (500.0s) [klara 1/2, kört 8.0 min, kvar ~8.0 min]
2026-11-24 19:44:45 Frö 274 D2_Adaptiv: hjR²(600/1500/2700)=-0,163/-0,075/-0,564 fk=1,180 py_in=-0,003 pz_in=0,318 (n=15) g_d=0,214 pop=8560 (516.4s) [klara 2/2, kört 16.0 min, kvar ~0.0 min]
";
        let d = las_logg(logg);
        let live_aktiv = LiveStatus {
            aktiv: true,
            karnor: 10,
            tradar: 10,
            omgang_s: 125,
            tick: 2,
        };
        let bild = rita_bild("test.logg", &d, 0.080, None, &live_aktiv, false);
        assert!(bild.contains("Grundmodell"));
        assert!(bild.contains("Långsam skalning"));
        assert!(bild.contains("MÅL"));
        assert!(bild.contains("godkända"));
        assert!(bild.contains("ARBETAR NU"));
        assert!(bild.contains("10 kärnor arbetar"));
        assert!(bild.contains("2 min 05 s"));
        assert!(!bild.contains("hjR²"), "standardvyn ska inte visa råa mätkoder");
        assert!(!bild.contains("g_d="), "standardvyn ska inte visa råa mätkoder");

        let med_siffror = rita_bild("test.logg", &d, 0.080, None, &LiveStatus::default(), true);
        assert!(med_siffror.contains("AVSLUTAD"));
        assert!(med_siffror.contains("hjR²"));
        assert!(med_siffror.contains("g_d"));
        assert_eq!(format_min_sek(45), "45 s");
        assert_eq!(format_min_sek(125), "2 min 05 s");
    }

    #[test]
    fn hittar_senaste_logg_i_katalog() {
        let bas = std::env::temp_dir().join("provvy_test_senaste");
        let _ = std::fs::remove_dir_all(&bas);
        std::fs::create_dir_all(bas.join("alt1000")).unwrap();
        std::fs::create_dir_all(bas.join("alt2000")).unwrap();
        std::fs::create_dir_all(bas.join("annan")).unwrap();
        let f1 = bas.join("alt1000/korning.logg");
        let f2 = bas.join("alt2000/korning.logg");
        let f3 = bas.join("annan/korning.logg");
        std::fs::write(&f1, "ett").unwrap();
        std::fs::write(&f3, "tre").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        std::fs::write(&f2, "två").unwrap();
        let funnen = senaste_logg_i(&bas).unwrap();
        assert_eq!(funnen, f2);
        let _ = std::fs::remove_dir_all(&bas);
    }
}
