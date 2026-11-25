//! Kartvyn (Alt 1595, Håkan 2026-11-22: "Det måste gå att se vars på kartan djuren trängs"): ritar kartbilden som
//! `hcsim kartbild` skriver, som en XY-karta i terminalen. Bakgrunden visar växterna (brunt = uppätet, grönt = mycket)
//! och vattnet (blått); prickarna i rutan är djuren där, en prick per djur (punktskriftstecken, högst 16 per ruta; Håkan
//! 2026-11-22: "Kan inte göra terminalnumren till prickar istället? ... Så det blir mer som en karta"). `--siffror` visar
//! antalet som tal i stället. Trängda rutor (minst `--trangsel` djur) har en mjuk sandfärg.
//! Ritas om när filen ändras, högst en gång per `--var-s` sekunder (10 om inget anges), utan att skärmen töms.
//!
//! Användning: `kartvy --fil FIL [--skala S] [--trangsel N] [--var-s S] [--absolut] [--siffror] [--dator NAMN|ingen] [--en-gang]`
//! `kartvy --fil FIL --val` skriver rekommendationen för kartvyns öppna val (Alt 1639–1642, 1649–1650) och slutar.
//! Var 60:e sekund spelas tidsskiftet upp från start till nu (`--auto-s`): en kvarts sekund per läge, högst 30 sekunder
//! (`--spela-s`) och högst 120 lägen (`--bilder`); 0 stänger av. Håkan 2026-11-22 kväll: "Varje minut ska den visa ända från
//! början till var den är. Och max 30 sekunder". Lägena läses ur databasen `FIL.tidsskift.db` (värden, inga bilder).
//! Färgskalan är relativ (grönast = mest växter på kartan just nu); `--absolut` mäter mot taket.
//! `--skala 2` slår ihop 2 × 2 rutor till en (för en mindre terminal).

use std::path::PathBuf;
#[path = "kartvy/dator.rs"]
mod dator;
#[path = "kartvy/val.rs"]
mod val;
use std::time::{Duration, SystemTime};

const AV: &str = "\x1b[0m";
const FET: &str = "\x1b[1m";
/// Växtandel 0..1 → bakgrundsfärg (helfärg, dova toner; Håkan 2026-11-22: "Lite dovare färger. Så att det är bra för
/// ögonen"; "Grön färg ska vara för växter ... när den är uppäten, så ska den ju mer vara markfärg"): dov jordbrun mark
/// (uppätet) som blir grönare ju mer växter rutan har. Grönare toner (Håkan 2026-11-22: "Jag vill ha lite grönare färg ... Så att
/// den är lättare att se"; de förra tonerna, upp till (64, 116, 76), såg gråa ut som oliv). Sedan mindre grönt (Håkan samma dag:
/// "Allting blir jättegrönt. Mindre grönt, mer kontrast mellan vatten och land"; tonen (40, 142, 60) var för stark).
const VAXT_FARG: [(u8, u8, u8); 7] = [(104, 84, 62), (98, 94, 62), (90, 102, 62), (80, 110, 62), (70, 116, 62), (62, 122, 62), (54, 128, 62)];
/// Vattnet behåller sin blå färg (Håkan: "Blått kan vara kvar för vattnet").
const VATTEN: (u8, u8, u8) = (0, 95, 175);
/// Tonerna för flodkant och träsk (blandas in med 50 % i växternas färg; Håkan: "Nu ser jag inte flodkanterna mer" vid 25 %).
const FLODKANT: (u8, u8, u8) = (40, 100, 170);
const TRASK: (u8, u8, u8) = (40, 120, 120);
const BLAND: f64 = 0.5;

fn blanda(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f64 * (1.0 - t) + y as f64 * t).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}
/// Talens färger: benvitt (mest omärkta), dovt rosa (mest märkta); trängsel: mjuk sand med mörk text.
/// Talens färg efter trängseln (Håkan 2026-11-22: "Skapa annan färg för högre trängsel också på siffrorna"), dova toner:
/// under en fjärdedel av trängselgränsen benvitt, under hälften dovt gult, under gränsen dovt orange, från gränsen mörkrött
/// på sand. (Understrykningen för märkta djur togs bort, Håkans Alt 1622.)
const TAL_NIVA: [(u8, u8, u8); 3] = [(216, 212, 200), (206, 180, 104), (204, 134, 84)];
const TRANGSEL_BG: (u8, u8, u8) = (196, 182, 142);
const TRANGSEL_FG: (u8, u8, u8) = (140, 44, 34);

/// Färgen på ett tal med `n` djur när trängselgränsen är `t` (n < t).
fn tal_farg(n: usize, t: usize) -> (u8, u8, u8) {
    if n * 4 < t {
        TAL_NIVA[0]
    } else if n * 2 < t {
        TAL_NIVA[1]
    } else {
        TAL_NIVA[2]
    }
}

fn bg(c: (u8, u8, u8)) -> String {
    format!("\x1b[48;2;{};{};{}m", c.0, c.1, c.2)
}
fn fg(c: (u8, u8, u8)) -> String {
    format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2)
}

#[derive(Debug, Clone)]
struct Bild {
    rubrik: String,
    ar: f64,
    markerad: bool,
    b: usize,
    h: usize,
    v: Vec<f64>,
    vmax: Vec<f64>,
    vatten: Vec<u8>,
    djur: Vec<(usize, usize, bool)>,
}

fn las(text: &str) -> Result<Bild, String> {
    let j: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("json: {e}"))?;
    let f = |n: &str| j.get(n).ok_or(format!("{n} saknas"));
    let tal = |n: &str| -> Result<Vec<f64>, String> {
        f(n)?.as_array().ok_or(format!("{n} är ingen lista"))?.iter().map(|x| x.as_f64().ok_or(format!("{n}: inget tal"))).collect()
    };
    let b = f("b")?.as_u64().ok_or("b")? as usize;
    let h = f("h")?.as_u64().ok_or("h")? as usize;
    let v = tal("v")?;
    let vmax = tal("vmax")?;
    let vatten: Vec<u8> = tal("vatten")?.into_iter().map(|x| x as u8).collect();
    if v.len() != b * h || vmax.len() != b * h || vatten.len() != b * h {
        return Err(format!("listorna har fel längd för {b} × {h}"));
    }
    let mut djur = Vec::new();
    for d in f("djur")?.as_array().ok_or("djur")? {
        let a = d.as_array().ok_or("djur: ingen lista")?;
        let (i, jj, m) = (a[0].as_u64().ok_or("i")? as usize, a[1].as_u64().ok_or("j")? as usize, a[2].as_u64().ok_or("m")? == 1);
        if i >= b || jj >= h {
            return Err(format!("djur utanför kartan: {i}, {jj}"));
        }
        djur.push((i, jj, m));
    }
    Ok(Bild {
        rubrik: f("rubrik")?.as_str().unwrap_or("").to_string(),
        ar: f("ar")?.as_f64().unwrap_or(0.0),
        markerad: f("markerad")?.as_bool().unwrap_or(false),
        b,
        h,
        v,
        vmax,
        vatten,
        djur,
    })
}

/// En ruta i vyn (S × S kartrutor).
#[derive(Debug, Clone, Default, PartialEq)]
struct Ruta {
    djur: usize,
    markta: usize,
    vatten: bool,
    vaxt_andel: f64,
    /// Antal kartrutor med flodkant (Flodfåra) och träsk i rutan; båda har växter (Alt 1624) och ritas som land med en ton.
    flod: usize,
    trask: usize,
}

/// Flodkant och träsk har växter (hcsim `landandel_av`); bara sjö, bräckt vatten och hav ritas som vatten.
fn har_vaxter(kod: u8) -> bool {
    kod <= 2
}

fn rutor(bild: &Bild, skala: usize) -> (usize, usize, Vec<Ruta>) {
    let s = skala.max(1);
    let (bb, hh) = (bild.b.div_ceil(s), bild.h.div_ceil(s));
    let mut ut = vec![Ruta::default(); bb * hh];
    let mut land = vec![0usize; bb * hh];
    let mut sum_v = vec![0.0; bb * hh];
    let mut sum_tak = vec![0.0; bb * hh];
    for j in 0..bild.h {
        for i in 0..bild.b {
            let c = j * bild.b + i;
            let r = (j / s) * bb + i / s;
            match bild.vatten[c] {
                1 => ut[r].flod += 1,
                2 => ut[r].trask += 1,
                _ => {}
            }
            if har_vaxter(bild.vatten[c]) {
                land[r] += 1;
                sum_v[r] += bild.v[c];
                sum_tak[r] += bild.vmax[c];
            }
        }
    }
    for (r, x) in ut.iter_mut().enumerate() {
        let alla = {
            let (rj, ri) = (r / bb, r % bb);
            let bredd = (bild.b - ri * s).min(s);
            let hojd = (bild.h - rj * s).min(s);
            bredd * hojd
        };
        x.vatten = land[r] * 2 < alla;
        x.vaxt_andel = if sum_tak[r] > 0.0 { (sum_v[r] / sum_tak[r]).clamp(0.0, 1.0) } else { 0.0 };
    }
    for &(i, j, m) in &bild.djur {
        let r = (j / s) * bb + i / s;
        ut[r].djur += 1;
        if m {
            ut[r].markta += 1;
        }
    }
    (bb, hh, ut)
}

/// Prickarnas ordning i rutans 4 × 4 punkter (två punktskriftstecken, vart och ett 2 × 4): en ordnad spridning (Bayers
/// matris), så att några få djur sprids över rutan och inte klumpar sig i ett hörn. `BAYER[rad][kolumn]`.
const BAYER: [[usize; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// Punkternas bitar i ett punktskriftstecken (U+2800 + bitar): `BIT[rad][kolumn inom tecknet]`.
const BIT: [[u32; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
pub const PRICKAR_MAX: usize = 16;

/// Två punktskriftstecken med `n` prickar (högst 16), spridda enligt `BAYER`.
fn prickar(n: usize) -> String {
    let mut b = [0u32; 2];
    for (rad, br) in BAYER.iter().enumerate() {
        for (kol, &ordning) in br.iter().enumerate() {
            if ordning < n.min(PRICKAR_MAX) {
                b[kol / 2] |= BIT[rad][kol % 2];
            }
        }
    }
    b.iter().map(|x| char::from_u32(0x2800 + x).unwrap_or(' ')).collect()
}

fn rita(bild: &Bild, skala: usize, trangsel: usize, andrad: Option<SystemTime>, absolut: bool, siffror: bool) -> String {
    let (bb, hh, r) = rutor(bild, skala);
    let topp = skal_topp(&r, absolut);
    let mut s = String::new();
    s.push_str(&format!(
        "{FET}KARTVYN{AV}  {}   år {:.1}{}   ritad {}\n",
        bild.rubrik,
        bild.ar,
        if bild.markerad { " (efter märkningen)" } else { " (inkörning, ingen märkt än)" },
        klockslag(andrad.unwrap_or_else(SystemTime::now))
    ));
    // x-axelns siffror var 8:e kolumn
    s.push_str("      x→");
    for x in 0..bb {
        if x % 8 == 0 {
            s.push_str(&format!("{:<16}", x * skala));
        }
    }
    s.push('\n');
    for y in 0..hh {
        if y % 4 == 0 {
            s.push_str(&format!("{:>5} │", y * skala));
        } else {
            s.push_str("      │");
        }
        for x in 0..bb {
            let q = &r[y * bb + x];
            let bgc = if q.vatten {
                VATTEN
            } else if q.flod > 0 {
                blanda(vaxt_farg(q.vaxt_andel, topp), FLODKANT, BLAND)
            } else if q.trask > 0 {
                blanda(vaxt_farg(q.vaxt_andel, topp), TRASK, BLAND)
            } else {
                vaxt_farg(q.vaxt_andel, topp)
            };
            if q.djur == 0 {
                s.push_str(&format!("{}  {AV}", bg(bgc)));
            } else {
                let tal = if !siffror {
                    prickar(q.djur)
                } else if q.djur > 99 {
                    "++".to_string()
                } else {
                    format!("{:>2}", q.djur)
                };
                if q.djur >= trangsel {
                    s.push_str(&format!("{}{}{FET}{tal}{AV}", bg(TRANGSEL_BG), fg(TRANGSEL_FG)));
                } else {
                    s.push_str(&format!("{}{}{tal}{AV}", bg(bgc), fg(tal_farg(q.djur, trangsel))));
                }
            }
        }
        s.push('\n');
    }
    s.push_str("      y↓\n");
    s.push_str("Färger: ");
    for (k, c) in VAXT_FARG.iter().enumerate() {
        s.push_str(&format!("{}  {AV}", bg(*c)));
        if k == 0 {
            s.push_str(" bar mark (0 %) ");
        }
    }
    s.push_str(&format!(
        " {} {:.0} % av taket{}   {}  {AV} flodkant  {}  {AV} träsk  {}  {AV} sjö och hav   ",
        if absolut { "fullt:" } else { "mest på kartan nu:" },
        100.0 * topp,
        if absolut { "" } else { " eller mer" },
        bg(blanda(VAXT_FARG[3], FLODKANT, BLAND)),
        bg(blanda(VAXT_FARG[3], TRASK, BLAND)),
        bg(VATTEN)
    ));
    s.push('\n');
    let (t1, t2) = (trangsel.div_ceil(4), trangsel.div_ceil(2));
    s.push_str(&format!(
        "{} {}1–{}{AV}  {}{}–{}{AV}  {}{}–{}{AV}  {}{FET}{}+ trängsel{AV}. En ruta = {} × {} kartrutor.\n\n",
        if siffror { "Talet = antal djur i rutan:".to_string() } else { format!("En prick = ett djur (högst {PRICKAR_MAX} prickar per ruta); prickens färg:") },
        fg(TAL_NIVA[0]), t1.saturating_sub(1).max(1),
        fg(TAL_NIVA[1]), t1, t2.saturating_sub(1),
        fg(TAL_NIVA[2]), t2, trangsel.saturating_sub(1),
        bg(TRANGSEL_BG), trangsel,
        skala, skala
    ));
    // Siffror om trängseln
    let tot: usize = r.iter().map(|q| q.djur).sum();
    let markta: usize = r.iter().map(|q| q.markta).sum();
    let i_trangsel: usize = r.iter().filter(|q| q.djur >= trangsel).map(|q| q.djur).sum();
    let med_djur = r.iter().filter(|q| q.djur > 0).count();
    let land = r.iter().filter(|q| !q.vatten).count();
    s.push_str(&format!(
        "Djur: {tot} (märkta {markta}). Rutor med djur: {med_djur} av {} (varav {land} mest land). I trängda rutor: {i_trangsel} djur ({:.0} %).\n",
        r.len(),
        if tot > 0 { 100.0 * i_trangsel as f64 / tot as f64 } else { 0.0 }
    ));
    let medel = |f: &dyn Fn(&Ruta) -> bool| {
        let v: Vec<f64> = r.iter().filter(|q| !q.vatten && f(q)).map(|q| q.vaxt_andel).collect();
        if v.is_empty() { None } else { Some(100.0 * v.iter().sum::<f64>() / v.len() as f64) }
    };
    let fmt = |x: Option<f64>| x.map(|v| format!("{v:.0} %")).unwrap_or("–".into());
    s.push_str(&format!(
        "Växter kvar (andel av taket): i rutor med djur {}, i landrutor utan djur {}.\n",
        fmt(medel(&|q| q.djur > 0)),
        fmt(medel(&|q| q.djur == 0))
    ));
    s.push_str(&kant_rad(bild));
    s.push_str(&vatten_rad(bild));
    s.push_str(&kust_rad(bild.ar, &las_kust(&kust_vag())));
    let mut topp: Vec<(usize, usize, &Ruta)> = (0..r.len()).map(|k| (k % bb, k / bb, &r[k])).filter(|(_, _, q)| q.djur > 0).collect();
    topp.sort_by(|a, b| b.2.djur.cmp(&a.2.djur));
    s.push_str("Mest trängda rutor (x, y: djur, växter kvar): ");
    let delar: Vec<String> = topp
        .iter()
        .take(5)
        .map(|(x, y, q)| format!("({}, {}): {}, {:.0} %", x * skala, y * skala, q.djur, 100.0 * q.vaxt_andel))
        .collect();
    s.push_str(&delar.join("; "));
    s.push('\n');
    s
}

/// Djur per vattenslag i den enskilda kartrutan (inte i den hopslagna rutan). Simuleringen låter djuren gå i träsk,
/// flodfåror och sjöar mot en extra energikostnad (sim.rs `ror_sig`, tal.rs KOST_TRASK, KOST_FLOD, KOST_SJO); hav och
/// bräckt vatten är stängda (`ar_hav`).
fn vatten_rad(bild: &Bild) -> String {
    const NAMN: [&str; 6] = ["land", "flodfåra", "träsk", "sjö", "bräckt", "hav"];
    let mut n = [0usize; 6];
    for &(i, j, _) in &bild.djur {
        let k = bild.vatten[j * bild.b + i] as usize;
        n[k.min(5)] += 1;
    }
    let tot: usize = n.iter().sum();
    let delar: Vec<String> = (0..6)
        .map(|k| format!("{} {} ({:.1} %)", NAMN[k], n[k], if tot > 0 { 100.0 * n[k] as f64 / tot as f64 } else { 0.0 }))
        .collect();
    format!("Djur per markslag i sin egen kartruta: {}.\n", delar.join(", "))
}

/// Simuleringens år 0 i år före K-Pg-gränsen (överlämningen `n10/docs/OVERLAMNING-2026-11-21-hjarnan.md`: år 0 = 67,0 Ma, slutet vid
/// gränsen 66,021 Ma = år 979 000). SIMULERINGEN2 (kustens ägare) räknar kustens poster i år före gränsen.
const AR0_FORE_GRANSEN: f64 = 979_000.0;

/// Kustens fil (SIMULERINGEN2, omröstningen kust, Alt 1618 och 1619): `n10/hckarta/kust.json` med
/// {"poster":[{"t_ar_fore_gransen":..., "riktning_grader":..., "avstand_km":..., "kalla":"...", "markning":"..."}]};
/// riktningen från fönstrets mitt, 0° mot norr, 90° mot öster.
fn kust_vag() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../hckarta/kust.json"))
}

#[derive(Debug, Clone, PartialEq)]
struct KustPost {
    t: f64,
    riktning: Option<f64>,
    avstand_km: Option<f64>,
    kalla: String,
    markning: String,
}

fn las_kust(vag: &std::path::Path) -> Vec<KustPost> {
    let Ok(t) = std::fs::read_to_string(vag) else { return Vec::new() };
    let Ok(j) = serde_json::from_str::<serde_json::Value>(&t) else { return Vec::new() };
    j.get("poster")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some(KustPost {
                        t: p.get("t_ar_fore_gransen")?.as_f64()?,
                        riktning: p.get("riktning_grader").and_then(|x| x.as_f64()),
                        avstand_km: p.get("avstand_km").and_then(|x| x.as_f64()),
                        kalla: p.get("kalla").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                        markning: p.get("markning").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Kustens rad under kartan för bildens år: posten närmast i tid. Utan post eller utan avstånd ritas inget hav
/// (SIMULERINGEN2: "en post med avstand_km null och markning "ingen källa" ska ge den texten, inte en pil").
fn kust_rad(ar: f64, poster: &[KustPost]) -> String {
    let t = AR0_FORE_GRANSEN - ar;
    let narmast = poster.iter().min_by(|a, b| (a.t - t).abs().partial_cmp(&(b.t - t).abs()).unwrap_or(std::cmp::Ordering::Equal));
    match narmast {
        Some(p) if p.avstand_km.is_some() && p.riktning.is_some() => {
            let r = p.riktning.unwrap().rem_euclid(360.0);
            const PIL: [&str; 8] = ["↑ norr", "↗ nordost", "→ öster", "↘ sydost", "↓ söder", "↙ sydväst", "← väster", "↖ nordväst"];
            let k = (((r + 22.5) / 45.0).floor() as usize) % 8;
            format!(
                "Kusten: {} {:.0} km från kartans mitt ({:.0}°), {:.0} år före gränsen; {} ({}).\n",
                PIL[k],
                p.avstand_km.unwrap(),
                r,
                p.t,
                p.kalla,
                p.markning
            )
        }
        _ => "Kusten: ingen läst källa för simuleringens tid; inget hav ritas (kustens ägare: SIMULERINGEN2, `n10/hckarta/kust.json`).\n".to_string(),
    }
}

/// Andel djur på kartans yttersta rutor (kanten), per grupp, mot kantens andel av kartan. Ett djur som väljer att gå
/// utanför kartan blir stående (sim.rs `ror_sig`), så kanten kan samla djur.
fn kant_rad(bild: &Bild) -> String {
    let kant = |i: usize, j: usize| i == 0 || j == 0 || i + 1 == bild.b || j + 1 == bild.h;
    let n_kant = (0..bild.h).flat_map(|j| (0..bild.b).map(move |i| (i, j))).filter(|&(i, j)| kant(i, j)).count();
    let andel = |m: bool| {
        let alla: Vec<&(usize, usize, bool)> = bild.djur.iter().filter(|d| d.2 == m).collect();
        if alla.is_empty() {
            "–".to_string()
        } else {
            let k = alla.iter().filter(|d| kant(d.0, d.1)).count();
            format!("{:.1} %", 100.0 * k as f64 / alla.len() as f64)
        }
    };
    format!(
        "Vid kanten (yttersta rutorna, {:.1} % av kartan): {} av de märkta, {} av de omärkta.\n",
        100.0 * n_kant as f64 / (bild.b * bild.h) as f64,
        andel(true),
        andel(false)
    )
}

/// Färgskalans övre ände. Relativ skala (standard; Håkan 2026-11-22: "Jag ser knappt något grönt alls", när bara 6–7 %
/// av växterna fanns kvar): den 95:e percentilen av landrutornas växtandel, så att de rutor som har mest växter just nu blir
/// gröna. `--absolut`: taket (100 %).
fn skal_topp(r: &[Ruta], absolut: bool) -> f64 {
    if absolut {
        return 1.0;
    }
    let mut v: Vec<f64> = r.iter().filter(|q| !q.vatten).map(|q| q.vaxt_andel).collect();
    if v.is_empty() {
        return 1.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95 = v[((v.len() - 1) as f64 * 0.95).round() as usize];
    p95.clamp(0.01, 1.0)
}

fn vaxt_farg(andel: f64, topp: f64) -> (u8, u8, u8) {
    let x = (andel / topp).clamp(0.0, 1.0);
    VAXT_FARG[(x * (VAXT_FARG.len() - 1) as f64).round() as usize]
}

fn klockslag(t: SystemTime) -> String {
    let sek = t.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::process::Command::new("date")
        .env("TZ", "Europe/Stockholm")
        .args(["-d", &format!("@{sek}"), "+%H:%M:%S"])
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

fn arg(args: &[String], n: &str) -> Option<String> {
    args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned()
}

/// Tidsskiftet (Håkan 2026-11-22: "en timeshift på en tidsskala från noll till hur långt det har gått. En timeshift som
/// visar live"; "Timeshiften körs varje minut"): `hcsim kartbild` sparar en bild per minut i `FIL.historik.jsonl`.
/// Den här strukturen håller var varje rad börjar, så att en gammal bild läses först när den visas.
struct Historik {
    vag: PathBuf,
    /// Tidsskiftets databas (`FIL.tidsskift.db`, Håkan 2026-11-22: värden i en databas, inte en textrad per bild).
    /// Finns den läses den; annars den gamla textfilen `FIL.historik.jsonl`.
    db: Option<PathBuf>,
    db_ids: Vec<i64>,
    rader: Vec<(u64, usize)>,
    last: u64,
    klockor: Vec<u64>,
    ar: Vec<f64>,
}

impl Historik {
    fn ny(vag: PathBuf) -> Self {
        Historik { vag, db: None, db_ids: Vec::new(), rader: Vec::new(), last: 0, klockor: Vec::new(), ar: Vec::new() }
    }

    /// Läser de rader som har tillkommit; true om något nytt kom. En fil som krympt (ny körning) läses om från början.
    fn med_db(mut self, db: PathBuf) -> Self {
        self.db = Some(db);
        self
    }

    /// Läser listan över lägen ur databasen; `None` om databasen saknas (då gäller textfilen).
    fn uppdatera_db(&mut self) -> Option<bool> {
        let l = hcsim::tidsskift::Lasare::oppna(self.db.as_ref()?)?;
        let fast = l.fast().ok()?;
        let lista = l.lista().ok()?;
        let ids: Vec<i64> = lista.iter().map(|x| x.0).collect();
        if ids == self.db_ids {
            return Some(false);
        }
        self.rader = ids.iter().map(|&id| (id as u64, 0)).collect();
        self.klockor = lista.iter().map(|x| x.1).collect();
        self.ar = lista.iter().map(|x| x.2 as f64 / fast.steg_per_ar as f64).collect();
        self.db_ids = ids;
        Some(true)
    }

    fn uppdatera(&mut self) -> bool {
        use std::io::{Read, Seek, SeekFrom};
        if let Some(ny) = self.uppdatera_db() {
            return ny;
        }
        let Ok(mut f) = std::fs::File::open(&self.vag) else { return false };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.last {
            *self = Historik::ny(self.vag.clone());
        }
        if len == self.last {
            return false;
        }
        if f.seek(SeekFrom::Start(self.last)).is_err() {
            return false;
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return false;
        }
        let mut start = 0usize;
        let mut ny = false;
        for (k, &c) in buf.iter().enumerate() {
            if c == b'\n' {
                let rad = &buf[start..k];
                let txt = String::from_utf8_lossy(rad);
                let v: Option<serde_json::Value> = serde_json::from_str(&txt).ok();
                let klocka = v.as_ref().and_then(|v| v.get("klocka")).and_then(|x| x.as_u64()).unwrap_or(0);
                let ar = v.as_ref().and_then(|v| v.get("ar")).and_then(|x| x.as_f64()).unwrap_or(0.0);
                self.rader.push((self.last + start as u64, k - start));
                self.klockor.push(klocka);
                self.ar.push(ar);
                start = k + 1;
                ny = true;
            }
        }
        self.last += start as u64;
        ny
    }

    fn rad(&self, k: usize) -> Option<String> {
        use std::io::{Read, Seek, SeekFrom};
        if !self.db_ids.is_empty() {
            let l = hcsim::tidsskift::Lasare::oppna(self.db.as_ref()?)?;
            return l.lage_json(*self.db_ids.get(k)?).ok();
        }
        let &(off, n) = self.rader.get(k)?;
        let mut f = std::fs::File::open(&self.vag).ok()?;
        f.seek(SeekFrom::Start(off)).ok()?;
        let mut buf = vec![0u8; n];
        f.read_exact(&mut buf).ok()?;
        String::from_utf8(buf).ok()
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Tangent {
    Bakat(usize),
    Framat(usize),
    Borjan,
    Live,
    Avsluta,
}

/// Läser en tangent ur början av `buf` (tar bort de bytes som använts). Ofullständiga pilkoder väntar på resten.
fn tangent(buf: &mut Vec<u8>) -> Option<Tangent> {
    loop {
        let &c = buf.first()?;
        if c != 27 {
            buf.remove(0);
            match c {
                b'q' | b'Q' | 3 => return Some(Tangent::Avsluta),
                b'l' | b'L' => return Some(Tangent::Live),
                _ => continue,
            }
        }
        if buf.len() < 3 {
            return None;
        }
        if buf[1] != b'[' {
            buf.remove(0);
            continue;
        }
        let (t, n) = match buf[2] {
            b'D' => (Some(Tangent::Bakat(1)), 3),
            b'C' => (Some(Tangent::Framat(1)), 3),
            b'A' => (Some(Tangent::Framat(10)), 3),
            b'B' => (Some(Tangent::Bakat(10)), 3),
            b'H' => (Some(Tangent::Borjan), 3),
            b'F' => (Some(Tangent::Live), 3),
            b'1' | b'4' => {
                if buf.len() < 4 {
                    return None;
                }
                (if buf[2] == b'1' { Some(Tangent::Borjan) } else { Some(Tangent::Live) }, 4)
            }
            _ => (None, 3),
        };
        buf.drain(..n.min(buf.len()));
        if t.is_some() {
            return t;
        }
    }
}

/// Nytt läge efter en tangent: None = live (senaste bilden), Some(k) = bild k i tidsskiftet. Når man slutet blir det live.
fn flytta(pos: Option<usize>, t: Tangent, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let sist = n - 1;
    match t {
        Tangent::Live | Tangent::Avsluta => None,
        Tangent::Borjan => Some(0),
        Tangent::Bakat(k) => Some(pos.unwrap_or(sist).saturating_sub(k)),
        Tangent::Framat(k) => match pos {
            None => None,
            Some(p) if p + k >= sist => None,
            Some(p) => Some(p + k),
        },
    }
}

/// Sekunder per läge i uppspelningen (eget val).
const S_PER_LAGE: f64 = 0.25;

/// Uppspelningens längd i sekunder (Håkan 2026-11-22: "max 30 sekunder ska ... den timeshift-visningen vara"):
/// en kvarts sekund per läge, minst 1 sekund och högst `max_s`.
fn speltid(antal: usize, max_s: f64) -> f64 {
    (antal as f64 * S_PER_LAGE).clamp(1.0, max_s.max(1.0))
}

/// Bilderna i en uppspelning: högst `max` stycken, jämnt fördelade från första till sista sparade bilden.
fn uppspelning_bilder(n: usize, max: usize) -> Vec<usize> {
    if n == 0 || max == 0 {
        return Vec::new();
    }
    if n <= max {
        return (0..n).collect();
    }
    (0..max).map(|k| (k * (n - 1) + (max - 1) / 2) / (max - 1)).collect()
}

/// Tidslinjen under kartan: 0 till så långt körningen har gått, med markören där bilden som visas är.
fn tidslinje(h: &Historik, pos: Option<usize>, ar_live: f64, bredd: usize, spelar: bool) -> String {
    let n = h.rader.len();
    let mut s = String::new();
    if n == 0 {
        s.push_str("Tidsskift: inga sparade bilder än (en bild sparas varje minut).\n");
        return s;
    }
    let t0 = h.klockor[0];
    let tn = *h.klockor.last().unwrap_or(&t0);
    let min_tot = (tn.saturating_sub(t0)) as f64 / 60.0;
    let k = pos.unwrap_or(n - 1);
    let x = if n > 1 { (k * (bredd - 1) + (n - 1) / 2) / (n - 1) } else { bredd - 1 };
    s.push_str("0 min ");
    for i in 0..bredd {
        if i == x {
            s.push_str(if pos.is_none() { "\x1b[38;2;127;166;106m●\x1b[0m" } else { "\x1b[38;2;201;169;79m●\x1b[0m" });
        } else if i < x {
            s.push('━');
        } else {
            s.push('─');
        }
    }
    s.push_str(&format!(" {:.0} min\n", min_tot));
    match pos {
        None => s.push_str(&format!(
            "\x1b[38;2;127;166;106mLIVE\x1b[0m: år {:.1}. Tidsskiftet har {} sparade lägen. ← → en minut, ↑ ↓ tio minuter, Home början, End eller L live, Q avsluta.\n",
            ar_live, n
        )),
        Some(p) => s.push_str(&format!(
            "\x1b[38;2;201;169;79m{}\x1b[0m: bild {} av {}, minut {:.0}, klockan {}, år {:.1}. {}\n",
            if spelar { "UPPSPELNING från start till nu" } else { "TIDSSKIFT" },
            p + 1,
            n,
            (h.klockor[p].saturating_sub(t0)) as f64 / 60.0,
            klockslag(SystemTime::UNIX_EPOCH + Duration::from_secs(h.klockor[p])),
            h.ar[p],
            if spelar { "Valfri tangent stoppar." } else { "End eller L går tillbaka till live." }
        )),
    }
    s
}

/// Terminalens läge för tangenterna: inga radbuffertar, inget eko, Ctrl+C som tangent (så att läget alltid återställs).
fn terminal_lage(pa: bool, sparat: &Option<String>) -> Option<String> {
    let tty = || std::fs::File::open("/dev/tty").ok();
    if pa {
        let g = std::process::Command::new("stty").arg("-g").stdin(tty()?).output().ok()?;
        let sparat = String::from_utf8(g.stdout).ok()?.trim().to_string();
        std::process::Command::new("stty").args(["-icanon", "-echo", "-isig", "min", "1", "time", "0"]).stdin(tty()?).status().ok()?;
        Some(sparat)
    } else {
        if let (Some(s), Some(t)) = (sparat, tty()) {
            let _ = std::process::Command::new("stty").arg(s).stdin(t).status();
        }
        None
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(fil) = arg(&args, "--fil") else {
        eprintln!("användning: kartvy --fil FIL [--skala S] [--trangsel N] [--var-s S] [--absolut] [--siffror] [--en-gang]");
        std::process::exit(2);
    };
    let fil = PathBuf::from(fil);
    let skala: usize = arg(&args, "--skala").and_then(|x| x.parse().ok()).unwrap_or(1);
    let trangsel: usize = arg(&args, "--trangsel").and_then(|x| x.parse().ok()).unwrap_or(5 * skala * skala);
    let var_ms: u64 = arg(&args, "--var-s").and_then(|x| x.parse::<f64>().ok()).map(|x| (x * 1000.0) as u64).unwrap_or(10_000);
    let en_gang = args.iter().any(|a| a == "--en-gang");
    let absolut = args.iter().any(|a| a == "--absolut");
    // Automatisk uppspelning (Håkan 2026-11-22: "Varje minut ska en automatiskt timeshift köras. Som man ser vars djuren
    // flyttar sig ... kanske 15 sekunder"): var `--auto-s` sekund spelas start till nu upp på `--spela-s` sekunder.
    let auto_s: u64 = arg(&args, "--auto-s").and_then(|x| x.parse().ok()).unwrap_or(60);
    let spela_s: f64 = arg(&args, "--spela-s").and_then(|x| x.parse().ok()).unwrap_or(30.0);
    let max_bilder: usize = arg(&args, "--bilder").and_then(|x| x.parse().ok()).unwrap_or(120);
    let mut hist = Historik::ny({
        let mut s = fil.as_os_str().to_owned();
        s.push(".historik.jsonl");
        PathBuf::from(s)
    })
    .med_db(hcsim::tidsskift::db_vag(&fil));
    let bredd = (fil_bredd(skala)).max(20);
    let siffror = args.iter().any(|a| a == "--siffror");
    let visa = |b: &Bild, andrad: Option<SystemTime>| rita(b, skala, trangsel, andrad, absolut, siffror);
    if args.iter().any(|a| a == "--val") {
        match std::fs::read_to_string(&fil).map_err(|e| e.to_string()).and_then(|t| las(&t)) {
            Ok(b) => print!("{}", val::rita_val(&b)),
            Err(e) => {
                eprintln!("Kartbilden går inte att läsa: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if en_gang {
        hist.uppdatera();
        match std::fs::read_to_string(&fil).map_err(|e| e.to_string()).and_then(|t| las(&t)) {
            Ok(b) => {
                let andrad = std::fs::metadata(&fil).and_then(|m| m.modified()).ok();
                print!("{}{}", visa(&b, andrad), tidslinje(&hist, None, b.ar, bredd, false));
            }
            Err(e) => {
                eprintln!("Kartbilden går inte att läsa: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let sparat = terminal_lage(true, &None);
    let (tx, rx) = std::sync::mpsc::channel::<u8>();
    std::thread::spawn(move || {
        use std::io::Read;
        let mut b = [0u8; 16];
        let mut inn = std::io::stdin();
        while let Ok(n) = inn.read(&mut b) {
            if n == 0 {
                break;
            }
            for &c in &b[..n] {
                if tx.send(c).is_err() {
                    return;
                }
            }
        }
    });
    let mut pos: Option<usize> = None;
    let mut buf: Vec<u8> = Vec::new();
    let mut senast_fil: Option<SystemTime> = None;
    let mut ritad = std::time::Instant::now() - Duration::from_millis(var_ms + 1);
    // Raden om en annan dator (Håkans Alt 1714): `--dator NAMN` (standard n10), `--dator ingen` stänger av.
    let dator_namn = arg(&args, "--dator").unwrap_or_else(|| "n10".into());
    let dator = if dator_namn == "ingen" { None } else { Some(dator::starta(dator_namn.clone(), 10)) };
    let dator_rad = |d: &Option<dator::Delat>| -> String {
        match d {
            None => String::new(),
            Some(d) => {
                let l = d.lock().ok().and_then(|l| l.clone());
                dator::rad(&dator_namn, l.as_ref().map(|(x, t)| (x, t.elapsed())), Duration::from_secs(60))
            }
        }
    };
    let mut visad_dator = String::new();
    let mut behov = true;
    let mut spelning: Option<(std::time::Instant, Vec<usize>)> = None;
    let mut senast_auto = std::time::Instant::now();
    let mut visad_bild: Option<usize> = None;
    loop {
        while let Ok(c) = rx.try_recv() {
            buf.push(c);
        }
        while let Some(t) = tangent(&mut buf) {
            if t == Tangent::Avsluta {
                terminal_lage(false, &sparat);
                println!();
                return;
            }
            if spelning.is_some() {
                // En tangent stoppar uppspelningen och går tillbaka till live.
                spelning = None;
                pos = None;
                senast_auto = std::time::Instant::now();
            } else {
                pos = flytta(pos, t, hist.rader.len());
            }
            behov = true;
        }
        // Starta uppspelningen var auto_s sekund, bara i liveläget.
        if auto_s > 0 && spelning.is_none() && pos.is_none() && senast_auto.elapsed().as_secs() >= auto_s {
            hist.uppdatera();
            let b = uppspelning_bilder(hist.rader.len(), max_bilder);
            if b.len() >= 2 {
                spelning = Some((std::time::Instant::now(), b));
                visad_bild = None;
            }
            senast_auto = std::time::Instant::now();
        }
        if let Some((start, bilder)) = &spelning {
            let x = start.elapsed().as_secs_f64() / speltid(bilder.len(), spela_s);
            if x >= 1.0 {
                spelning = None;
                pos = None;
                senast_auto = std::time::Instant::now();
                behov = true;
            } else {
                let k = bilder[((x * bilder.len() as f64) as usize).min(bilder.len() - 1)];
                if visad_bild != Some(k) {
                    pos = Some(k);
                    visad_bild = Some(k);
                    behov = true;
                }
            }
        }
        let tid_ok = spelning.is_none() && ritad.elapsed().as_millis() as u64 >= var_ms;
        if tid_ok {
            if hist.uppdatera() {
                behov = true;
            }
            let andrad = std::fs::metadata(&fil).and_then(|m| m.modified()).ok();
            if pos.is_none() && andrad.is_some() && andrad != senast_fil {
                behov = true;
            }
        }
        // Raden om den andra datorn ritas om så fort ett nytt svar har kommit (svaren kommer var 10:e sekund).
        if dator_rad(&dator) != visad_dator {
            behov = true;
        }
        if behov {
            let (text, ar) = match pos {
                None => {
                    let andrad = std::fs::metadata(&fil).and_then(|m| m.modified()).ok();
                    senast_fil = andrad;
                    match std::fs::read_to_string(&fil).map_err(|e| e.to_string()).and_then(|t| las(&t)) {
                        Ok(b) => (visa(&b, andrad), b.ar),
                        Err(e) => (format!("Kartbilden går inte att läsa: {e}\n"), 0.0),
                    }
                }
                Some(p) => match hist.rad(p).ok_or("raden saknas".to_string()).and_then(|t| las(&t)) {
                    Ok(b) => {
                        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(hist.klockor[p]);
                        (visa(&b, Some(t)), b.ar)
                    }
                    Err(e) => (format!("Tidsskiftets bild går inte att läsa: {e}\n"), 0.0),
                },
            };
            visad_dator = dator_rad(&dator);
            let s = format!("{}{}{}", text, tidslinje(&hist, pos, ar, bredd, spelning.is_some()), visad_dator);
            skriv_en_gang(&utan_flimmer(&s));
            ritad = std::time::Instant::now();
            behov = false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Tidslinjens längd: ungefär kartans bredd i tecken (2 tecken per ruta) minus etiketterna.
fn fil_bredd(skala: usize) -> usize {
    let kol = 64usize.div_ceil(skala.max(1));
    (kol * 2).saturating_sub(6)
}

#[cfg(test)]
mod prov {
    use super::*;
    fn bild() -> Bild {
        // 4 × 2: rad 0 land, rad 1: två vatten, två land.
        Bild {
            rubrik: "p".into(),
            ar: 1.0,
            markerad: true,
            b: 4,
            h: 2,
            v: vec![1.0, 0.0, 0.5, 0.5, 9.0, 9.0, 1.0, 1.0],
            vmax: vec![1.0; 8],
            vatten: vec![0, 0, 0, 0, 3, 5, 0, 0],
            djur: vec![(0, 0, true), (0, 0, true), (0, 0, false), (3, 1, false)],
        }
    }
    /// Håkan 2026-11-22: "lite grönare färg ... lättare att se". Rutor med växter ska vara tydligt gröna: grönt minus rött
    /// växer för varje steg och är minst 65 i den översta tonen (gränsen eget val; de första tonerna gav 52 och ska bli röda;
    /// 102 var för grönt, Håkan: "Mindre grönt"). Flodkanten ska synas mot landet: minst 40 mer blått; träsket minst 25 (grönblått).
    #[test]
    fn vaxterna_ar_tydligt_grona() {
        let gr = |c: (u8, u8, u8)| c.1 as i32 - c.0 as i32;
        for k in 1..VAXT_FARG.len() {
            assert!(gr(VAXT_FARG[k]) > gr(VAXT_FARG[k - 1]), "steg {k} blir inte grönare");
        }
        assert!(gr(VAXT_FARG[0]) < 0, "bar mark ska vara brun");
        let topp = VAXT_FARG[VAXT_FARG.len() - 1];
        assert!((65..=80).contains(&gr(topp)) && topp.1 as i32 - topp.2 as i32 >= 60, "översta tonen är inte lagom grön: {topp:?}");
        for c in VAXT_FARG {
            for (namn, ton, krav) in [("flodkant", FLODKANT, 40), ("träsk", TRASK, 25)] {
                let b = blanda(c, ton, BLAND);
                assert!(b.2 as i32 - c.2 as i32 >= krav, "{namn} syns inte mot landet {c:?}: {b:?}");
            }
        }
    }

    /// En prick per djur (Håkan 2026-11-22): n djur ger n prickar upp till 16, och redan 2 djur hamnar i var sitt tecken.
    #[test]
    fn en_prick_per_djur() {
        let antal = |t: &str| t.chars().map(|c| (c as u32 - 0x2800).count_ones() as usize).sum::<usize>();
        for n in 0..=20 {
            let t = prickar(n);
            assert_eq!(t.chars().count(), 2, "rutan är två tecken bred");
            assert_eq!(antal(&t), n.min(PRICKAR_MAX), "n = {n}: {t}");
        }
        let t: Vec<char> = prickar(2).chars().collect();
        assert!(t[0] != '\u{2800}' && t[1] != '\u{2800}', "två djur ska spridas: {t:?}");
        let s = rita(&bild(), 1, 20, None, true, false);
        let rader: Vec<&str> = s.lines().filter(|l| l.contains(" │")).collect();
        assert_eq!(rader.len(), 2, "kartans två rader");
        let k = rader.join("\n");
        let ren = |x: &str| x.split('\x1b').map(|d| d.split_once('m').map(|p| p.1).unwrap_or(d)).collect::<String>();
        let rutor: String = rader.iter().map(|l| ren(l.split_once(" │").unwrap().1)).collect();
        assert!(k.contains(&prickar(3)) && k.contains(&prickar(1)), "rutornas prickar saknas: {k}");
        assert!(!rutor.chars().any(|c| c.is_ascii_digit()), "kartan ritar prickar, inte tal: {rutor}");
        let s2 = rita(&bild(), 1, 20, None, true, true);
        let k2: String = s2.lines().filter(|l| l.contains(" │")).map(|l| ren(l.split_once(" │").unwrap().1)).collect();
        assert!(k2.contains(" 3"), "--siffror ger tal: {k2}");
    }

    #[test]
    fn djuren_raknas_i_ratt_ruta() {
        let (bb, hh, r) = rutor(&bild(), 1);
        assert_eq!((bb, hh), (4, 2));
        assert_eq!(r[0].djur, 3);
        assert_eq!(r[0].markta, 2);
        assert_eq!(r[7].djur, 1);
        assert_eq!(r.iter().map(|q| q.djur).sum::<usize>(), 4);
        assert!(r[4].vatten && r[5].vatten && !r[6].vatten);
    }
    #[test]
    fn skala_slar_ihop_och_vatten_raknas_inte_som_vaxter() {
        let (bb, hh, r) = rutor(&bild(), 2);
        assert_eq!((bb, hh), (2, 1));
        assert_eq!(r[0].djur, 3);
        // vänstra 2 × 2: land 1,0 och vatten 9,9 → växtandel ur landet: (1 + 0) / 2
        assert!((r[0].vaxt_andel - 0.5).abs() < 1e-12, "{}", r[0].vaxt_andel);
        assert!(!r[0].vatten, "2 av 4 rutor är land: inte majoritet vatten");
    }
    #[test]
    fn trangda_rutor_syns_i_texten() {
        let s = rita(&bild(), 1, 3, None, true, true);
        assert!(s.contains("I trängda rutor: 3 djur (75 %)"), "{s}");
        assert!(s.contains("(0, 0): 3, 100 %"), "{s}");
        // 4 × 2-kartan är bara kant: alla djur står vid kanten.
        assert!(s.contains("Vid kanten (yttersta rutorna, 100.0 % av kartan): 100.0 % av de märkta, 100.0 % av de omärkta."), "{s}");
    }
    /// Rött om x och y byter plats: ett djur på kolumn 1, rad 0 ska hamna i ruta 1, inte i ruta 4.
    #[test]
    fn kolumn_och_rad_byter_inte_plats() {
        let mut b = bild();
        b.djur = vec![(1, 0, false), (2, 1, true)];
        let (_, _, r) = rutor(&b, 1);
        assert_eq!(r[1].djur, 1);
        assert_eq!(r[4 + 2].djur, 1);
        assert_eq!(r[6].markta, 1);
        assert_eq!(r[4].djur, 0);
    }
    /// Utan flimmer: skärmen töms aldrig (ingen ESC[2J), varje rad tömmer bara sin rest.
    #[test]
    fn ingen_tomning_av_skarmen() {
        let s = utan_flimmer("a\nb\n");
        assert!(!s.contains("\x1b[2J"));
        assert_eq!(s, "\x1b[?2026h\x1b[Ha\x1b[K\nb\x1b[K\n\x1b[J\x1b[?2026l");
    }
    /// Relativ skala: på en karta där inget har mer än 8 % av taket blir den rikaste rutan ändå grönast.
    #[test]
    fn relativ_skala_gor_rikaste_rutan_gron() {
        // 15 fattiga och 5 rika rutor: den 95:e percentilen hamnar bland de rika.
        let mut r = vec![Ruta { vaxt_andel: 0.01, ..Ruta::default() }; 15];
        r.extend(vec![Ruta { vaxt_andel: 0.08, ..Ruta::default() }; 5]);
        let topp = skal_topp(&r, false);
        assert!((topp - 0.08).abs() < 1e-12, "{topp}");
        assert_eq!(vaxt_farg(0.08, topp), VAXT_FARG[VAXT_FARG.len() - 1]);
        assert_eq!(vaxt_farg(0.08, skal_topp(&r, true)), VAXT_FARG[0], "absolut skala: 8 % är bar mark");
        assert_eq!(vaxt_farg(0.0, topp), VAXT_FARG[0]);
    }
    /// Talens färg följer trängseln: gränsen 20 ger 1–4, 5–9, 10–19 och 20+.
    #[test]
    fn talens_farg_foljer_trangseln() {
        assert_eq!(tal_farg(4, 20), TAL_NIVA[0]);
        assert_eq!(tal_farg(5, 20), TAL_NIVA[1]);
        assert_eq!(tal_farg(9, 20), TAL_NIVA[1]);
        assert_eq!(tal_farg(10, 20), TAL_NIVA[2]);
        assert_eq!(tal_farg(19, 20), TAL_NIVA[2]);
        let s = rita(&bild(), 1, 20, None, true, true);
        assert!(s.contains("1–4") && s.contains("5–9") && s.contains("10–19") && s.contains("20+ trängsel"), "{s}");
    }
    /// Djuren räknas efter sin egen kartrutas vatten: i provets karta står 3 på land (0, 0) och 1 på land (3, 1).
    #[test]
    fn djur_per_markslag() {
        let mut b = bild();
        b.djur.push((0, 1, false)); // rad 1 kolumn 0 är sjö
        let s = vatten_rad(&b);
        assert!(s.contains("land 4 (80.0 %)") && s.contains("sjö 1 (20.0 %)") && s.contains("hav 0 (0.0 %)"), "{s}");
    }
    #[test]
    fn tangenterna_lases_ur_koderna() {
        let mut b = vec![27, b'[', b'D', 27, b'[', b'C', 27, b'[', b'A', 27, b'[', b'B', 27, b'[', b'1', b'~', 27, b'[', b'4', b'~', b'l', b'x', 3];
        let mut t = Vec::new();
        while let Some(x) = tangent(&mut b) {
            t.push(x);
        }
        use Tangent::*;
        assert_eq!(t, vec![Bakat(1), Framat(1), Framat(10), Bakat(10), Borjan, Live, Live, Avsluta]);
        let mut halv = vec![27, b'['];
        assert_eq!(tangent(&mut halv), None, "en halv pilkod väntar");
        assert_eq!(halv.len(), 2);
    }
    #[test]
    fn tidsskiftet_flyttar_och_gar_live_vid_slutet() {
        use Tangent::*;
        assert_eq!(flytta(None, Bakat(1), 5), Some(3));
        assert_eq!(flytta(Some(3), Framat(1), 5), None, "sista bilden är live");
        assert_eq!(flytta(Some(1), Framat(1), 5), Some(2));
        assert_eq!(flytta(Some(1), Bakat(10), 5), Some(0));
        assert_eq!(flytta(Some(2), Borjan, 5), Some(0));
        assert_eq!(flytta(Some(2), Live, 5), None);
        assert_eq!(flytta(None, Framat(1), 5), None);
        assert_eq!(flytta(None, Bakat(1), 0), None);
    }
    #[test]
    fn historiken_las_rad_for_rad_och_borjar_om_vid_ny_korning() {
        let d = std::env::temp_dir().join(format!("kartvy_prov_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        let p = d.join("h.jsonl");
        std::fs::write(&p, "{\"klocka\":100,\"ar\":1.0}\n{\"klocka\":160,\"ar\":2.5}\n{\"klocka\":2").unwrap();
        let mut h = Historik::ny(p.clone());
        assert!(h.uppdatera());
        assert_eq!(h.rader.len(), 2, "den halva raden räknas inte");
        assert_eq!(h.klockor, vec![100, 160]);
        assert_eq!(h.rad(1).unwrap(), "{\"klocka\":160,\"ar\":2.5}");
        std::fs::write(&p, "{\"klocka\":5,\"ar\":0}\n").unwrap();
        h.uppdatera();
        assert_eq!(h.klockor, vec![5], "kortare fil = ny körning");
        let s = tidslinje(&h, None, 0.0, 20, false);
        assert!(s.contains("LIVE") && s.contains("1 sparade lägen"), "{s}");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn uppspelningen_tar_hogst_30_sekunder() {
        assert_eq!(speltid(120, 30.0), 30.0);
        assert_eq!(speltid(600, 30.0), 30.0);
        assert_eq!(speltid(40, 30.0), 10.0);
        assert_eq!(speltid(2, 30.0), 1.0);
        assert_eq!(speltid(120, 10.0), 10.0);
    }

    #[test]
    fn historiken_las_ur_databasen_nar_den_finns() {
        use hcsim::tidsskift::{db_vag, Fast, Lage, Skrivare};
        let fil = std::env::temp_dir().join(format!("kartvy-db-prov-{}.json", std::process::id()));
        let fast = Fast { rubrik: "prov".into(), b: 2, h: 2, steg_per_ar: 150, vmax: vec![800.0; 4], vatten: vec![0, 0, 3, 0] };
        let mut sk = Skrivare::ny(&db_vag(&fil), &fast).unwrap();
        for k in 0..3i64 {
            let l = Lage { klocka: 100 + 10 * k as u64, t: 150 * k, markerad: k == 2, v: vec![1.5, 2.5, 0.0, 4.0], djur: vec![(1, 0, false), (0, 1, true)] };
            sk.lagg_till(&l).unwrap();
        }
        let mut h = Historik::ny(PathBuf::from("/finns/inte.jsonl")).med_db(db_vag(&fil));
        assert!(h.uppdatera());
        assert!(!h.uppdatera(), "inget nytt läge, men historiken sa att något ändrats");
        assert_eq!(h.klockor, vec![100, 110, 120]);
        assert_eq!(h.ar, vec![0.0, 1.0, 2.0]);
        let b = las(&h.rad(2).unwrap()).unwrap();
        assert_eq!((b.b, b.h), (2, 2));
        assert!(h.rad(3).is_none());
        for sv in ["", "-wal", "-shm"] {
            let mut p = db_vag(&fil).into_os_string();
            p.push(sv);
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn uppspelningen_valjer_jamnt_fran_start_till_nu() {
        assert_eq!(uppspelning_bilder(5, 60), vec![0, 1, 2, 3, 4]);
        let b = uppspelning_bilder(181, 60);
        assert_eq!(b.len(), 60);
        assert_eq!((b[0], b[59]), (0, 180), "börjar vid start och slutar vid nu");
        assert!(b.windows(2).all(|w| w[1] > w[0]), "framåt i tiden");
        assert!(uppspelning_bilder(0, 60).is_empty());
    }
    /// Flodkant och träsk har växter (Alt 1624) och ritas som land; sjö och hav som vatten.
    #[test]
    fn flodkant_och_trask_ar_land() {
        let mut b = bild();
        b.vatten = vec![1, 2, 0, 0, 3, 5, 0, 0];
        let (_, _, r) = rutor(&b, 1);
        assert!(!r[0].vatten && r[0].flod == 1, "{:?}", r[0]);
        assert!(!r[1].vatten && r[1].trask == 1, "{:?}", r[1]);
        assert!(r[4].vatten && r[5].vatten);
        assert!((r[0].vaxt_andel - 1.0).abs() < 1e-12, "flodkantens växter räknas");
    }
    #[test]
    fn kustens_rad() {
        let ingen = "Kusten: ingen läst källa för simuleringens tid";
        assert!(kust_rad(180.0, &[]).starts_with(ingen));
        let tom = KustPost { t: 979_000.0, riktning: None, avstand_km: None, kalla: "".into(), markning: "ingen källa".into() };
        assert!(kust_rad(180.0, &[tom.clone()]).starts_with(ingen), "avstand null ger texten, inte en pil");
        let ost = KustPost { t: 978_000.0, riktning: Some(95.0), avstand_km: Some(40.0), kalla: "Fowler 2020".into(), markning: "källa".into() };
        let r = kust_rad(180.0, &[tom.clone(), ost.clone()]);
        assert!(r.starts_with(ingen), "posten närmast år 180 (t = 978 820) är den tomma: {r}");
        let r = kust_rad(1000.0, &[tom, ost]);
        assert!(r.contains("→ öster 40 km") && r.contains("Fowler 2020 (källa)"), "{r}");
        let d = std::env::temp_dir().join(format!("kust_prov_{}.json", std::process::id()));
        std::fs::write(&d, r#"{"poster":[{"t_ar_fore_gransen":5,"riktning_grader":180,"avstand_km":3,"kalla":"k","markning":"eget val"}]}"#).unwrap();
        let p = las_kust(&d);
        let _ = std::fs::remove_file(&d);
        assert_eq!(p.len(), 1);
        assert!(kust_rad(0.0, &p).contains("↓ söder 3 km"));
        assert!(las_kust(std::path::Path::new("/finns/inte.json")).is_empty());
    }
    #[test]
    fn djur_utanfor_kartan_vagras() {
        let t = r#"{"rubrik":"x","t":0,"ar":0,"markerad":false,"b":1,"h":1,"v":[0],"vmax":[1],"vatten":[0],"djur":[[1,0,0]]}"#;
        assert!(las(t).is_err());
        let ok = r#"{"rubrik":"x","t":0,"ar":0,"markerad":false,"b":1,"h":1,"v":[0],"vmax":[1],"vatten":[0],"djur":[[0,0,1]]}"#;
        assert_eq!(las(ok).unwrap().djur, vec![(0, 0, true)]);
    }
}
