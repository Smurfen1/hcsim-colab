//! Naturläge och källtabell för naturhändelser i Hell Creek-kartan (enhällig omröstning
//! `naturhandelser` 2026-11-24: Alt 2149 + Alt 2151 + Alt 2153; Håkans Alt 2157).
//!
//! Kartan ger **ren geometri** (`flodzon`, `avstand_vatten_m`, `avstand_fara_m`) utan
//! `hojd_over_fara_m` eller tolkade klasser (`Lagslatt`/`Hogmark`):
//! - I lutning 2 (`flod.rs`) är markhöjden nytt 0,30 m/km-plan plus kvarstående 1° PaleoDEM-avvikelse
//!   (uppmätt std 11,19 m, min −24,44 m, max +30,29 m), medan strandvallen (2 m) ligger i eget lager
//!   (`vall_hojd_m`). Förmätningen över fröna 1–20 och 261 visade att 28,6 % (Äldre) och 23,0 % (Övre)
//!   av icke-fårerutorna får negativ höjdskillnad mot närmaste fåras vattenyta (ned till −31,94 m)
//!   på grund av 1° PaleoDEM-avvikelsen. Därför hör ingen `hojd_over_fara_m` hemma som översvämningsklass.
//! - Dynamik (torka, brand, översvämning, asknedfall) byggs inte här; den väntar på sessionen BRAIN
//!   i `n10/hcsim` (Alt 1968) och på fulltextrundan i Alt 2154.
//! - Fil-I/O är frivillig (`har_naturlage: bool`) och ändrar aldrig läge 1, `karta.json` eller äldre
//!   kontrollsummor.

use std::collections::VecDeque;
use std::fs;
use std::path::Path;

use crate::{FlodAndelar, Karta, Lager, Marke, Vatten};

/// Flodslättszon per ruta (ren geometri ur kartans befintliga lager; Alt 2149).
/// Prioritet vid överlapp: `Fara > Sjo > Trask > Balte > Vall > Land`.
/// I standard 2 (2 km-rutor) sammanfaller `balte_andel > 0` och `vall_andel > 0` med `fara_andel > 0`
/// på samma 2 km-ruta (exakta delruteandelar finns kvar i `FlodAndelar`), medan finare rutnät eller
/// konstruerade rutor utan fåra klassas som `Balte` eller `Vall`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FlodZon {
    Land = 0,
    Fara = 1,
    Balte = 2,
    Vall = 3,
    Trask = 4,
    Sjo = 5,
}

/// Kartans naturläge per ruta (Alt 2149 + Alt 2153).
/// När `har_naturlage == false` (läge 1 eller äldre kartmapp utan `naturlage.json`) är lagren tomma.
#[derive(Clone, Debug, PartialEq)]
pub struct NaturLage {
    pub har_naturlage: bool,
    pub flodzon: Lager<FlodZon>,
    pub avstand_vatten_m: Lager<f32>,
    pub avstand_fara_m: Lager<f32>,
}

impl NaturLage {
    /// Tomt naturläge (läge 1 eller äldre kartmapp utan `naturlage.json`).
    pub fn tom() -> Self {
        Self {
            har_naturlage: false,
            flodzon: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            avstand_vatten_m: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            avstand_fara_m: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
        }
    }
}

/// En källrad ur `naturhandelser.tsv` (13 kolumner, Alt 2151).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturKallaRad {
    pub handelse: String,
    pub flodzon: String,
    pub matt: String,
    pub varde: String,
    pub enhet: String,
    pub kalla: String,
    pub doi: String,
    pub citat: String,
    pub last: String,
    pub plats_tid: String,
    pub ursprung: String,
    pub markning: String,
    pub forbehall: String,
}

/// Klassar en ruta i `FlodZon` ur vattentyp och delruteandelar (fåra, bälte, vall).
pub fn flodzon_av(vt: Vatten, fara_andel: f32, balte_andel: f32, vall_andel: f32) -> FlodZon {
    if fara_andel > 0.0 || vt == Vatten::Flodfara {
        FlodZon::Fara
    } else if vt == Vatten::Sjo {
        FlodZon::Sjo
    } else if vt == Vatten::Trask {
        FlodZon::Trask
    } else if balte_andel > 0.0 {
        FlodZon::Balte
    } else if vall_andel > 0.0 {
        FlodZon::Vall
    } else {
        FlodZon::Land
    }
}

/// Exakt deterministiskt Chebyshev-avstånd (8-granne-BFS) i meter från en uppsättning källrutor.
/// Källrutor får avståndet `0.0` m; saknas källrutor helt blir avståndet `f32::INFINITY`.
fn chebyshev_avstand_m(b: usize, h: usize, ruta_m: f32, kallor: &[bool]) -> Vec<f32> {
    let n = b * h;
    let mut steg = vec![u32::MAX; n];
    let mut ko = VecDeque::new();
    for (c, &ar_kalla) in kallor.iter().enumerate() {
        if ar_kalla {
            steg[c] = 0;
            ko.push_back(c);
        }
    }
    const DR: [(isize, isize); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    while let Some(c) = ko.pop_front() {
        let i = (c % b) as isize;
        let j = (c / b) as isize;
        let ns = steg[c] + 1;
        for (di, dj) in DR {
            let ni = i + di;
            let nj = j + dj;
            if ni < 0 || nj < 0 || ni >= b as isize || nj >= h as isize {
                continue;
            }
            let nc = nj as usize * b + ni as usize;
            if steg[nc] == u32::MAX {
                steg[nc] = ns;
                ko.push_back(nc);
            }
        }
    }
    steg.into_iter()
        .map(|s| {
            if s == u32::MAX {
                f32::INFINITY
            } else {
                s as f32 * ruta_m
            }
        })
        .collect()
}

/// Härleder `NaturLage` ur en befintlig `Karta` och frivillig `FlodAndelar` (standard 2).
/// Alla vattenrutor (`Fara`, `Trask`, `Sjo`, `Brackt`, `Hav`) får `avstand_vatten_m == 0.0` (Neons nollprov);
/// alla fårerutor (`fara_andel > 0` eller `Vatten::Flodfara`) får `avstand_fara_m == 0.0`.
pub fn rakna_naturlage(k: &Karta, andelar: Option<&FlodAndelar>) -> Result<NaturLage, String> {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let n = b * h;
    if k.omrade.ruta_m <= 0.0 || !k.omrade.ruta_m.is_finite() {
        return Err(format!("ogiltig ruta_m: {}", k.omrade.ruta_m));
    }
    if k.vattentyp.varden.len() != n {
        return Err(format!(
            "vattentyp har längd {}, väntade {}",
            k.vattentyp.varden.len(),
            n
        ));
    }
    if let Some(a) = andelar {
        if a.fara_andel.varden.len() != n
            || a.balte_andel.varden.len() != n
            || a.vall_andel.varden.len() != n
        {
            return Err(format!(
                "FlodAndelar har fel längd (fara {}, bälte {}, vall {}), väntade {}",
                a.fara_andel.varden.len(),
                a.balte_andel.varden.len(),
                a.vall_andel.varden.len(),
                n
            ));
        }
    }

    let mut flodzon = Vec::with_capacity(n);
    let mut ar_vatten = vec![false; n];
    let mut ar_fara = vec![false; n];
    for c in 0..n {
        let vt = k.vattentyp.varden[c];
        let fa = andelar.map(|a| a.fara_andel.varden[c]).unwrap_or(0.0);
        let ba = andelar.map(|a| a.balte_andel.varden[c]).unwrap_or(0.0);
        let va = andelar.map(|a| a.vall_andel.varden[c]).unwrap_or(0.0);
        let zon = flodzon_av(vt, fa, ba, va);
        flodzon.push(zon);

        let is_fara = fa > 0.0 || vt == Vatten::Flodfara;
        let is_vatten = is_fara
            || matches!(
                vt,
                Vatten::Trask | Vatten::Sjo | Vatten::Brackt | Vatten::Hav
            );
        ar_fara[c] = is_fara;
        ar_vatten[c] = is_vatten;
    }

    let ruta_m = k.omrade.ruta_m as f32;
    let avstand_vatten_m = chebyshev_avstand_m(b, h, ruta_m, &ar_vatten);
    let avstand_fara_m = chebyshev_avstand_m(b, h, ruta_m, &ar_fara);

    Ok(NaturLage {
        har_naturlage: true,
        flodzon: Lager {
            varden: flodzon,
            marke: Marke::Simulering,
        },
        avstand_vatten_m: Lager {
            varden: avstand_vatten_m,
            marke: Marke::Simulering,
        },
        avstand_fara_m: Lager {
            varden: avstand_fara_m,
            marke: Marke::Simulering,
        },
    })
}

const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

fn fnv_byte(h: &mut u64, b: u8) {
    *h ^= u64::from(b);
    *h = h.wrapping_mul(FNV_PRIME);
}

fn fnv_bytes(h: &mut u64, bs: &[u8]) {
    for &b in bs {
        fnv_byte(h, b);
    }
}

fn marke_kod(m: Marke) -> u8 {
    match m {
        Marke::Last => 0,
        Marke::Simulering => 1,
    }
}

/// FNV-1a 64 över `NaturLage` (`flodzon`, `avstand_vatten_m`, `avstand_fara_m` och märken).
pub fn kontrollsumma_naturlage(nl: &NaturLage) -> u64 {
    let mut h = FNV_OFFSET;
    fnv_byte(&mut h, u8::from(nl.har_naturlage));
    if !nl.har_naturlage {
        return h;
    }
    for &z in &nl.flodzon.varden {
        fnv_byte(&mut h, z as u8);
    }
    fnv_byte(&mut h, marke_kod(nl.flodzon.marke));
    for &v in &nl.avstand_vatten_m.varden {
        fnv_bytes(&mut h, &v.to_le_bytes());
    }
    fnv_byte(&mut h, marke_kod(nl.avstand_vatten_m.marke));
    for &v in &nl.avstand_fara_m.varden {
        fnv_bytes(&mut h, &v.to_le_bytes());
    }
    fnv_byte(&mut h, marke_kod(nl.avstand_fara_m.marke));
    h
}

/// Skriver frivilliga naturlägesfiler (`flodzon.u8`, `avstand_vatten_m.f32`, `avstand_fara_m.f32`,
/// `naturlage.json`) till `mapp` utan att röra `karta.json` eller äldre kontrollsummor (Alt 2153).
pub fn skriv_naturlage(nl: &NaturLage, mapp: &Path) -> Result<u64, String> {
    if !nl.har_naturlage {
        return Err("vägrar skriva naturläge när har_naturlage är false".into());
    }
    let n = nl.flodzon.varden.len();
    if nl.avstand_vatten_m.varden.len() != n || nl.avstand_fara_m.varden.len() != n {
        return Err("naturlägets lager har olika längd".into());
    }
    fs::create_dir_all(mapp)
        .map_err(|e| format!("kunde inte skapa mapp {}: {e}", mapp.display()))?;

    let zon_bytes: Vec<u8> = nl.flodzon.varden.iter().map(|&z| z as u8).collect();
    fs::write(mapp.join("flodzon.u8"), &zon_bytes).map_err(|e| format!("flodzon.u8: {e}"))?;

    let mut vatten_bytes = Vec::with_capacity(n * 4);
    for &v in &nl.avstand_vatten_m.varden {
        vatten_bytes.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(mapp.join("avstand_vatten_m.f32"), &vatten_bytes)
        .map_err(|e| format!("avstand_vatten_m.f32: {e}"))?;

    let mut fara_bytes = Vec::with_capacity(n * 4);
    for &v in &nl.avstand_fara_m.varden {
        fara_bytes.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(mapp.join("avstand_fara_m.f32"), &fara_bytes)
        .map_err(|e| format!("avstand_fara_m.f32: {e}"))?;

    let summa = kontrollsumma_naturlage(nl);
    let m_str = |m: Marke| match m {
        Marke::Last => "last",
        Marke::Simulering => "simulering",
    };
    let json = format!(
        "{{\n  \"version\": 1,\n  \"antal_rutor\": {n},\n  \"kontrollsumma\": \"{summa}\",\n  \"avstand_metod\": \"chebyshev_m\",\n  \"marke_flodzon\": \"{}\",\n  \"marke_avstand_vatten_m\": \"{}\",\n  \"marke_avstand_fara_m\": \"{}\",\n  \"flodzon_koder\": {{\"0\": \"Land\", \"1\": \"Fara\", \"2\": \"Balte\", \"3\": \"Vall\", \"4\": \"Trask\", \"5\": \"Sjo\"}}\n}}\n",
        m_str(nl.flodzon.marke),
        m_str(nl.avstand_vatten_m.marke),
        m_str(nl.avstand_fara_m.marke),
    );
    fs::write(mapp.join("naturlage.json"), json.as_bytes())
        .map_err(|e| format!("naturlage.json: {e}"))?;
    Ok(summa)
}

fn las_json_str(text: &str, nyckel: &str) -> Result<String, String> {
    let m = format!("\"{nyckel}\": \"");
    let start = text
        .find(&m)
        .ok_or_else(|| format!("naturlage.json saknar {nyckel}"))?
        + m.len();
    let slut = text[start..]
        .find('"')
        .ok_or_else(|| format!("naturlage.json ogiltig sträng för {nyckel}"))?
        + start;
    Ok(text[start..slut].to_string())
}

fn parsa_marke(s: &str) -> Result<Marke, String> {
    match s {
        "last" => Ok(Marke::Last),
        "simulering" => Ok(Marke::Simulering),
        annat => Err(format!("okänt märke i naturlage.json: {annat:?}")),
    }
}

/// Läser frivilliga naturlägesfiler ur `mapp` (Alt 2153).
/// Saknas `naturlage.json` returneras `NaturLage::tom()` med `har_naturlage == false`
/// (kraschar aldrig på äldre mappar eller läge 1). Finns den, prövas alla filer och kontrollsumman.
pub fn las_naturlage(mapp: &Path, b: usize, h: usize) -> Result<NaturLage, String> {
    let json_path = mapp.join("naturlage.json");
    if !json_path.exists() {
        return Ok(NaturLage::tom());
    }
    let n = b * h;
    let text = fs::read_to_string(&json_path)
        .map_err(|e| format!("naturlage.json kunde inte läsas: {e}"))?;
    let summa_fil: u64 = las_json_str(&text, "kontrollsumma")?
        .parse()
        .map_err(|_| "ogiltig kontrollsumma i naturlage.json".to_string())?;
    let marke_zon = parsa_marke(&las_json_str(&text, "marke_flodzon")?)?;
    let marke_vatten = parsa_marke(&las_json_str(&text, "marke_avstand_vatten_m")?)?;
    let marke_fara = parsa_marke(&las_json_str(&text, "marke_avstand_fara_m")?)?;

    let zon_raw = fs::read(mapp.join("flodzon.u8")).map_err(|e| format!("flodzon.u8: {e}"))?;
    if zon_raw.len() != n {
        return Err(format!(
            "flodzon.u8 har längd {}, väntade {}",
            zon_raw.len(),
            n
        ));
    }
    let mut flodzon = Vec::with_capacity(n);
    for (i, &kod) in zon_raw.iter().enumerate() {
        flodzon.push(match kod {
            0 => FlodZon::Land,
            1 => FlodZon::Fara,
            2 => FlodZon::Balte,
            3 => FlodZon::Vall,
            4 => FlodZon::Trask,
            5 => FlodZon::Sjo,
            annat => return Err(format!("flodzon.u8 ruta {i}: okänd kod {annat}")),
        });
    }

    let vatten_raw = fs::read(mapp.join("avstand_vatten_m.f32"))
        .map_err(|e| format!("avstand_vatten_m.f32: {e}"))?;
    if vatten_raw.len() != n * 4 {
        return Err(format!(
            "avstand_vatten_m.f32 har längd {} byte, väntade {}",
            vatten_raw.len(),
            n * 4
        ));
    }
    let mut avstand_vatten_m = Vec::with_capacity(n);
    for chunk in vatten_raw.chunks_exact(4) {
        avstand_vatten_m.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    let fara_raw = fs::read(mapp.join("avstand_fara_m.f32"))
        .map_err(|e| format!("avstand_fara_m.f32: {e}"))?;
    if fara_raw.len() != n * 4 {
        return Err(format!(
            "avstand_fara_m.f32 har längd {} byte, väntade {}",
            fara_raw.len(),
            n * 4
        ));
    }
    let mut avstand_fara_m = Vec::with_capacity(n);
    for chunk in fara_raw.chunks_exact(4) {
        avstand_fara_m.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    let nl = NaturLage {
        har_naturlage: true,
        flodzon: Lager {
            varden: flodzon,
            marke: marke_zon,
        },
        avstand_vatten_m: Lager {
            varden: avstand_vatten_m,
            marke: marke_vatten,
        },
        avstand_fara_m: Lager {
            varden: avstand_fara_m,
            marke: marke_fara,
        },
    };
    let beraknad = kontrollsumma_naturlage(&nl);
    if beraknad != summa_fil {
        return Err(format!(
            "naturlägets kontrollsumma stämmer inte: filen har {summa_fil}, läst data ger {beraknad}"
        ));
    }
    Ok(nl)
}

/// Skriver en 3D-mapp med befintlig `skriv_3d_med` (oförändrad kontrollsumma för läge 1 och 2)
/// och skriver frivilligt `NaturLage` bredvid om `Some(nl)` ges (Alt 2153).
pub fn skriv_3d_med_naturlage(
    k: &Karta,
    andelar: Option<&FlodAndelar>,
    nl: Option<&NaturLage>,
    mapp: &Path,
) -> Result<u64, String> {
    let summa = crate::skriv_3d_med(k, andelar, mapp)?;
    if let Some(n) = nl {
        if n.har_naturlage {
            skriv_naturlage(n, mapp)?;
        }
    }
    Ok(summa)
}

/// Läser en 3D-mapp med befintlig `las_3d_med` samt frivilligt `NaturLage` (Alt 2153).
/// För en äldre mapp eller läge 1 blir `nl.har_naturlage == false`.
pub fn las_3d_med_naturlage(
    mapp: &Path,
) -> Result<(Karta, Option<FlodAndelar>, NaturLage), String> {
    let (k, andelar) = crate::las_3d_med(mapp)?;
    let nl = las_naturlage(mapp, k.omrade.b, k.omrade.h)?;
    Ok((k, andelar, nl))
}

const GILTIG_LAST: [&str; 4] = ["fulltext", "sammandrag", "fulltext_del", "ej_last"];
const GILTIG_URSPRUNG: [&str; 5] = ["hell_creek", "grannformation", "aldre", "globalt", "nutid"];
const GILTIG_MARKNING: [&str; 4] = ["Hell Creek", "lanat", "eget val", "ingen_kalla"];
const EJ_HELL_CREEK_PLATS: [&str; 11] = [
    "lance",
    "frenchman",
    "two medicine",
    "scollard",
    "battle",
    "seymour",
    "alberta",
    "saskatchewan",
    "wyoming",
    "indien",
    "sydatlanten",
];

/// Läser och validerar källtabellen `naturhandelser.tsv` (Alt 2151).
/// - Exakt 13 kolumner inklusive `ursprung`.
/// - Inga tal får bäras på rader med `last == "ej_last"` eller `markning == "ingen_kalla"`.
/// - Källbelagda rader kräver DOI som börjar på `10.` och ordagrant citat på minst 10 tecken.
/// - En rad med `ursprung == "hell_creek"` får inte hänvisa till en grannformation, äldre formation
///   eller annan kontinent i `plats_tid`; en rad märkt `Hell Creek` måste ha `ursprung == "hell_creek"`.
/// - `lokal_vulkan` måste vara `nej`, och varje bentonitrad måste bära `"bevarat lager, inte frekvens"`.
pub fn las_naturhandelser_tsv(text: &str) -> Result<Vec<NaturKallaRad>, String> {
    let mut rader = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let rubrik: Vec<&str> = rader
        .next()
        .ok_or_else(|| "naturhandelser.tsv är tom".to_string())?
        .split('\t')
        .collect();
    let forvantad = [
        "handelse",
        "flodzon",
        "matt",
        "varde",
        "enhet",
        "kalla",
        "doi",
        "citat",
        "last",
        "plats_tid",
        "ursprung",
        "markning",
        "forbehall",
    ];
    if rubrik != forvantad {
        return Err(format!(
            "ogiltig rubrikrad i naturhandelser.tsv: fick {rubrik:?}"
        ));
    }
    let mut ut = Vec::new();
    for (idx, rad) in rader.enumerate() {
        let c: Vec<&str> = rad.split('\t').collect();
        if c.len() != 13 {
            return Err(format!("rad {}: {} kolumner, ska vara 13", idx + 1, c.len()));
        }
        let handelse = c[0];
        let matt = c[2];
        let varde = c[3];
        let doi = c[6];
        let citat = c[7];
        let last = c[8];
        let plats_tid = c[9];
        let ursprung = c[10];
        let markning = c[11];
        let forbehall = c[12];

        if !GILTIG_LAST.contains(&last) {
            return Err(format!("rad {}: ogiltig läsning {last:?}", idx + 1));
        }
        if !GILTIG_URSPRUNG.contains(&ursprung) {
            return Err(format!("rad {}: ogiltigt ursprung {ursprung:?}", idx + 1));
        }
        if !GILTIG_MARKNING.contains(&markning) {
            return Err(format!("rad {}: ogiltig märkning {markning:?}", idx + 1));
        }
        if markning == "ingen_kalla" || last == "ej_last" {
            if varde != "-" {
                return Err(format!(
                    "rad {}: tal {varde:?} får inte bäras utan läst källa",
                    idx + 1
                ));
            }
        } else {
            if !doi.starts_with("10.") {
                return Err(format!("rad {}: ogiltig eller saknad DOI {doi:?}", idx + 1));
            }
            if citat == "-" || citat.len() < 10 {
                return Err(format!("rad {}: saknat eller för kort citat", idx + 1));
            }
        }
        if markning == "Hell Creek" && ursprung != "hell_creek" {
            return Err(format!(
                "rad {}: märkning Hell Creek kräver ursprung hell_creek, fick {ursprung:?}",
                idx + 1
            ));
        }
        if ursprung == "hell_creek" {
            let p_low = plats_tid.to_lowercase();
            for forbjuden in EJ_HELL_CREEK_PLATS {
                if p_low.contains(forbjuden) {
                    return Err(format!(
                        "rad {}: ursprung hell_creek får inte innehålla {forbjuden:?} i plats_tid ({plats_tid:?})",
                        idx + 1
                    ));
                }
            }
        }
        if handelse == "vulkan" && matt == "lokal_vulkan" && varde != "nej" {
            return Err(format!(
                "rad {}: lokal_vulkan måste vara \"nej\" i Hell Creek, fick {varde:?}",
                idx + 1
            ));
        }
        if handelse == "vulkan"
            && (matt.starts_with("bentonit_") || matt.starts_with("tefra_"))
            && !forbehall.contains("bevarat lager, inte frekvens")
        {
            return Err(format!(
                "rad {}: bentonitrad saknar förbehållet \"bevarat lager, inte frekvens\"",
                idx + 1
            ));
        }
        if forbehall.trim().is_empty() || forbehall == "-" {
            return Err(format!("rad {}: förbehåll saknas", idx + 1));
        }
        ut.push(NaturKallaRad {
            handelse: handelse.to_string(),
            flodzon: c[1].to_string(),
            matt: matt.to_string(),
            varde: varde.to_string(),
            enhet: c[4].to_string(),
            kalla: c[5].to_string(),
            doi: doi.to_string(),
            citat: citat.to_string(),
            last: last.to_string(),
            plats_tid: plats_tid.to_string(),
            ursprung: ursprung.to_string(),
            markning: markning.to_string(),
            forbehall: forbehall.to_string(),
        });
    }
    if ut.is_empty() {
        return Err("naturhandelser.tsv har inga datarader".into());
    }
    Ok(ut)
}
