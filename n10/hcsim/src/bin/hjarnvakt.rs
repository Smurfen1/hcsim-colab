//! Hjärnvakten: en levande XY-bild i terminalen som visar om ett hjärnförsök fortfarande kan nå sitt mål,
//! så att det kan avbrytas innan tid slösas (Håkan 2026-11-22: "kunna se om försöket inte kommer att nå sitt mål
//! och kunna avbryta det innan tid slösats i onödan", "begripligt visualiserad för en vanlig", "prickar i ett x y diagram").
//!
//! Läser försökets logg: en rad per färdig körning, `KLAR {json}` (eller bara `{json}`), med fälten `arm`, `spegel`,
//! `fro`, `andel_markta` och `kortid_s` (som `hcsim utforsk` skriver). Ett frö räknas som vunnet när de märkta har mer
//! än hälften av djuren vid slutet (samma regel som försöken).
//!
//! Målfilen (TSV, en rad per mål): `namn<TAB>arm<TAB>spegel (ja/nej)<TAB>K<TAB>N`: målet är minst K vunna av N frön.
//!
//! Användning: `hjarnvakt --logg FIL --mal FIL [--totalt M] [--tradar T] [--var-s S] [--en-gang]`
//! Bilden ritas om när loggen ändras, och annars var 50:e minut.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const OM_RITA_S: u64 = 50 * 60;

#[derive(Clone, Debug)]
struct Mal {
    namn: String,
    arm: String,
    spegel: bool,
    k: usize,
    n: usize,
}

#[derive(Clone, Debug)]
struct Korning {
    arm: String,
    spegel: bool,
    vann: bool,
    kortid_s: f64,
}

#[derive(Debug, PartialEq)]
enum Lage {
    Natt,
    Omojligt,
    Mojligt { behovs: usize, kvar: usize },
}

fn las_mal(text: &str) -> Result<Vec<Mal>, String> {
    let mut ut = Vec::new();
    for (i, rad) in text.lines().enumerate() {
        let rad = rad.trim_end();
        if rad.is_empty() || rad.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = rad.split('\t').collect();
        if f.len() != 5 {
            return Err(format!("målfilen rad {}: väntade 5 fält med tab emellan, fick {}", i + 1, f.len()));
        }
        let spegel = match f[2] {
            "ja" => true,
            "nej" => false,
            x => return Err(format!("målfilen rad {}: spegel ska vara ja eller nej, fick {x}", i + 1)),
        };
        let k: usize = f[3].parse().map_err(|e| format!("målfilen rad {}: K: {e}", i + 1))?;
        let n: usize = f[4].parse().map_err(|e| format!("målfilen rad {}: N: {e}", i + 1))?;
        if k == 0 || k > n {
            return Err(format!("målfilen rad {}: K ska vara 1..=N", i + 1));
        }
        ut.push(Mal { namn: f[0].into(), arm: f[1].into(), spegel, k, n });
    }
    if ut.is_empty() {
        return Err("målfilen har inga mål".into());
    }
    Ok(ut)
}

fn las_logg(text: &str) -> Result<Vec<Korning>, String> {
    let mut ut = Vec::new();
    for (i, rad) in text.lines().enumerate() {
        let r = rad.trim();
        let json = if let Some(x) = r.strip_prefix("KLAR ") { x } else if r.starts_with('{') { r } else { continue };
        if json.contains("TORRKÖRNING") {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("loggen rad {}: {e}", i + 1))?;
        let arm = v.get("arm").and_then(|x| x.as_str()).ok_or(format!("loggen rad {}: arm saknas", i + 1))?;
        let spegel = v.get("spegel").and_then(|x| x.as_bool()).ok_or(format!("loggen rad {}: spegel saknas", i + 1))?;
        let andel = v.get("andel_markta").and_then(|x| x.as_f64()).ok_or(format!("loggen rad {}: andel_markta saknas", i + 1))?;
        let kortid_s = v.get("kortid_s").and_then(|x| x.as_f64()).unwrap_or(0.0);
        ut.push(Korning { arm: arm.into(), spegel, vann: andel > 0.5, kortid_s });
    }
    Ok(ut)
}

/// Vinster efter varje färdigt frö, i den ordning fröna blev klara.
fn bana(mal: &Mal, k: &[Korning]) -> Vec<usize> {
    let mut w = 0;
    let mut ut = Vec::new();
    for x in k.iter().filter(|x| x.arm == mal.arm && x.spegel == mal.spegel) {
        if x.vann {
            w += 1;
        }
        ut.push(w);
    }
    ut
}

fn lage(mal: &Mal, klara: usize, vunna: usize) -> Lage {
    let kvar = mal.n.saturating_sub(klara);
    if vunna >= mal.k {
        Lage::Natt
    } else if vunna + kvar < mal.k {
        Lage::Omojligt
    } else {
        Lage::Mojligt { behovs: mal.k - vunna, kvar }
    }
}

/// Lägsta antal vinster efter x klara frön för att målet ännu ska kunna nås.
fn grans(mal: &Mal, x: usize) -> usize {
    mal.k.saturating_sub(mal.n.saturating_sub(x))
}

// Dova färger (Håkan 2026-11-22: "Lite dovare färger. Så att det är bra för ögonen"): grönt godkänt, rött inte godkänt,
// gult pågår.
const GRON: &str = "\x1b[38;2;127;166;106m";
const ROD: &str = "\x1b[38;2;196;106;90m";
const GUL: &str = "\x1b[38;2;201;169;79m";
const GRA: &str = "\x1b[38;2;122;122;122m";
const FET: &str = "\x1b[1m";
const AV: &str = "\x1b[0m";

fn rita_mal(mal: &Mal, b: &[usize]) -> String {
    let klara = b.len();
    let vunna = b.last().copied().unwrap_or(0);
    let l = lage(mal, klara, vunna);
    let farg = match l {
        Lage::Natt => GRON,
        Lage::Omojligt => ROD,
        Lage::Mojligt { .. } => GUL,
    };
    let mut s = String::new();
    s.push_str(&format!("{farg}{FET}{}{AV}  (målet: minst {} vunna av {} frön)\n", mal.namn, mal.k, mal.n));
    let kol = 3usize;
    for y in (0..=mal.n).rev() {
        let etikett = if y == mal.k { format!("{y:>2} mål") } else if y % 5 == 0 { format!("{y:>2}    ") } else { "      ".into() };
        s.push_str(&etikett);
        s.push_str(if y == 0 { "└" } else { "│" });
        for x in 0..=mal.n {
            let tecken = if x >= 1 && x <= klara && b[x - 1] == y {
                format!("{farg}●{AV}")
            } else if x >= 1 && grans(mal, x) == y && y > 0 {
                format!("{GRA}·{AV}")
            } else if y == mal.k {
                format!("{GRA}─{AV}")
            } else if y == 0 {
                "─".into()
            } else {
                " ".into()
            };
            s.push_str(&tecken);
            let fyll = if y == 0 { "─" } else if y == mal.k { "─" } else { " " };
            for _ in 1..kol {
                if y == mal.k && y != 0 {
                    s.push_str(&format!("{GRA}{fyll}{AV}"));
                } else {
                    s.push_str(fyll);
                }
            }
        }
        s.push('\n');
    }
    s.push_str("       ");
    for x in 0..=mal.n {
        if x % 5 == 0 {
            s.push_str(&format!("{x:<width$}", width = kol * 5));
        }
    }
    s.push_str("\n       klara frön →        ● = vunna hittills   ");
    s.push_str(&format!("{GRA}·{AV} = färre än så: målet kan inte nås\n"));
    s.push_str(&format!("       Färger: {GRON}grönt = godkänt{AV}, {ROD}rött = inte godkänt{AV}, {GUL}gult = pågår{AV}\n"));
    let text = match l {
        Lage::Natt => format!("{GRON}{FET}MÅLET ÄR NÅTT{AV}: {vunna} vunna av {klara} klara."),
        Lage::Omojligt => format!(
            "{ROD}{FET}MÅLET KAN INTE NÅS LÄNGRE — AVBRYT{AV}: {vunna} vunna av {klara} klara; även om alla {} som är kvar vinner blir det {} (målet {}).",
            mal.n - klara,
            vunna + mal.n - klara,
            mal.k
        ),
        Lage::Mojligt { behovs, kvar } => format!(
            "{GUL}{FET}KAN FORTFARANDE NÅS{AV}: {vunna} vunna av {klara} klara; {behovs} vinster behövs av de {kvar} som är kvar."
        ),
    };
    s.push_str(&text);
    s.push_str("\n\n");
    s
}

fn bild(rubrik: &str, mal: &[Mal], k: &[Korning], totalt: Option<usize>, tradar: usize, logg_andrad: Option<SystemTime>) -> String {
    let mut s = String::new();
    let nu = klockslag(SystemTime::now());
    s.push_str(&format!("{FET}HJÄRNVAKTEN{AV}  {rubrik}   ritad {nu}"));
    if let Some(t) = logg_andrad {
        s.push_str(&format!(", senaste körningen klar {}", klockslag(t)));
    }
    s.push_str("\n\n");
    for m in mal {
        s.push_str(&rita_mal(m, &bana(m, k)));
    }
    let klara = k.len();
    match totalt {
        Some(t) if klara < t => {
            let medel = if klara > 0 { k.iter().map(|x| x.kortid_s).sum::<f64>() / klara as f64 } else { 0.0 };
            let kvar_s = medel * (t - klara) as f64 / tradar.max(1) as f64;
            s.push_str(&format!(
                "Körningar: {klara} av {t} klara. Kvar: cirka {} minuter (medel {:.0} s per körning, {} samtidigt).\n",
                (kvar_s / 60.0).round() as u64,
                medel,
                tradar
            ));
        }
        Some(t) => s.push_str(&format!("Körningar: alla {t} klara.\n")),
        None => s.push_str(&format!("Körningar: {klara} klara.\n")),
    }
    let alla_omojliga = mal.iter().all(|m| {
        let b = bana(m, k);
        lage(m, b.len(), b.last().copied().unwrap_or(0)) == Lage::Omojligt
    });
    if alla_omojliga {
        s.push_str(&format!("{ROD}{FET}Inget av målen kan nås längre. Försöket kan stoppas nu.{AV}\n"));
    }
    s
}

/// Svensk tid (Europe/Stockholm) ur `date`, så att sommartiden blir rätt.
fn klockslag(t: SystemTime) -> String {
    let sek = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::process::Command::new("date")
        .env("TZ", "Europe/Stockholm")
        .args(["-d", &format!("@{sek}"), "+%H:%M"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|x| x.trim().to_string())
        .unwrap_or_else(|| "?".into())
}

/// Ritar en bild utan flimmer (Håkan 2026-11-22: "Det ska inte flimra"): skärmen töms inte; markören går hem, varje rad
/// skrivs över och tömmer bara resten av raden, det som blir över under bilden tas bort, och allt skickas i en skrivning
/// (inom terminalens "synkroniserad uppdatering" där den finns).
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(logg), Some(malfil)) = (arg(&args, "--logg"), arg(&args, "--mal")) else {
        eprintln!("användning: hjarnvakt --logg FIL --mal FIL [--totalt M] [--tradar T] [--var-s S] [--en-gang] [--rubrik TEXT]");
        std::process::exit(2);
    };
    let logg = PathBuf::from(logg);
    let mal = match std::fs::read_to_string(&malfil).map_err(|e| format!("läs {malfil}: {e}")).and_then(|t| las_mal(&t)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let totalt: Option<usize> = arg(&args, "--totalt").and_then(|x| x.parse().ok());
    let tradar: usize = arg(&args, "--tradar").and_then(|x| x.parse().ok()).unwrap_or(12);
    let var_s: u64 = arg(&args, "--var-s").and_then(|x| x.parse().ok()).unwrap_or(10);
    let en_gang = args.iter().any(|a| a == "--en-gang");
    let rubrik = arg(&args, "--rubrik").unwrap_or_else(|| logg.display().to_string());
    let mut senast: Option<SystemTime> = None;
    let mut ritad = Instant::now() - Duration::from_secs(OM_RITA_S + 1);
    loop {
        let andrad = std::fs::metadata(&logg).and_then(|m| m.modified()).ok();
        if andrad != senast || ritad.elapsed().as_secs() >= OM_RITA_S {
            let text = std::fs::read_to_string(&logg).unwrap_or_default();
            let s = match las_logg(&text) {
                Ok(k) => bild(&rubrik, &mal, &k, totalt, tradar, andrad),
                Err(e) => format!("{ROD}Loggen går inte att läsa: {e}{AV}\n"),
            };
            if en_gang {
                print!("{s}");
                return;
            }
            skriv_en_gang(&utan_flimmer(&s));
            senast = andrad;
            ritad = Instant::now();
        }
        std::thread::sleep(Duration::from_secs(var_s));
    }
}

#[cfg(test)]
mod prov {
    use super::*;
    fn m() -> Mal {
        Mal { namn: "t".into(), arm: "a".into(), spegel: false, k: 15, n: 20 }
    }
    fn kor(vann: &[bool]) -> Vec<Korning> {
        vann.iter().map(|&v| Korning { arm: "a".into(), spegel: false, vann: v, kortid_s: 1.0 }).collect()
    }
    /// Målet 15 av 20: efter 6 förluster kan det inte nås (14 + 0 < 15 fel; 0 vunna + 14 kvar = 14 < 15).
    #[test]
    fn sex_forluster_gor_malet_omojligt() {
        let b = bana(&m(), &kor(&[false; 6]));
        assert_eq!(lage(&m(), b.len(), *b.last().unwrap()), Lage::Omojligt);
        let b5 = bana(&m(), &kor(&[false; 5]));
        assert_eq!(lage(&m(), b5.len(), *b5.last().unwrap()), Lage::Mojligt { behovs: 15, kvar: 15 });
    }
    #[test]
    fn femton_vinster_ar_natt() {
        let b = bana(&m(), &kor(&[true; 15]));
        assert_eq!(lage(&m(), b.len(), *b.last().unwrap()), Lage::Natt);
    }
    #[test]
    fn grans_foljer_kvarvarande() {
        assert_eq!(grans(&m(), 0), 0);
        assert_eq!(grans(&m(), 5), 0);
        assert_eq!(grans(&m(), 6), 1);
        assert_eq!(grans(&m(), 20), 15);
    }
    #[test]
    fn spegel_och_annan_arm_raknas_inte() {
        let mut k = kor(&[true; 3]);
        k.push(Korning { arm: "a".into(), spegel: true, vann: true, kortid_s: 1.0 });
        k.push(Korning { arm: "b".into(), spegel: false, vann: true, kortid_s: 1.0 });
        assert_eq!(bana(&m(), &k), vec![1, 2, 3]);
    }
    #[test]
    fn logg_med_klar_och_torr() {
        let t = "skrap\nKLAR {\"arm\":\"a\",\"spegel\":false,\"fro\":1,\"andel_markta\":0.6,\"kortid_s\":2}\n{\"arm\":\"a\",\"spegel\":false,\"fro\":2,\"andel_markta\":0.4} TORRKÖRNING\n{\"arm\":\"a\",\"spegel\":false,\"fro\":3,\"andel_markta\":0.5,\"kortid_s\":2}\n";
        let k = las_logg(t).unwrap();
        assert_eq!(k.len(), 2);
        assert!(k[0].vann);
        assert!(!k[1].vann, "exakt hälften räknas inte som vunnet");
    }
    #[test]
    fn felaktig_malfil_vagras() {
        assert!(las_mal("a\tb\tja\t15\n").is_err());
        assert!(las_mal("a\tb\tkanske\t15\t20\n").is_err());
        assert!(las_mal("a\tb\tnej\t21\t20\n").is_err());
        assert!(las_mal("# bara kommentar\n").is_err());
    }
}
