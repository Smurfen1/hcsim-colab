//! Geometriskt basunderlag och låst källtabell för förna, alger och plankton i Hell Creek-kartan
//! (enhällig omröstning `forna` 2026-11-24: Alt 2180 + Alt 2183 + Alt 2184; Håkans Alt 2189).
//!
//! Kartan ger **ren geometri** per ruta utan proxyfält, utan biomassa i kg/m² och utan djurplankton:
//! - `vattenyta_andel`: `0.0` på `Land`, `fara_andel` på `Rinnande` (faktisk delrutearea: 125,7 km² i
//!   Äldre och 49,9 km² i Övre över fröna 1–20), `1.0` på `Träsk` och `Sjö`.
//! - `kron_tathet`: `lovtrad + barrtrad + kottepalm_ginkgo` ur `vaxt_tathet` (`0.51` på Land/Rinnande,
//!   `0.25` i Träsk, `0.00` i Sjö). Ingen påhittad `1 - kron_tathet`-skugga i flodfårans mitt och ingen
//!   `fornatillforsel_proxy` (varning i Tonin m.fl. 2018; Neons fråga 3-invändning).
//! - `vattenvaxt_andel`: vattenväxternas täthet (`vaxt_tathet[5]`: `0.00` Rinnande, `0.10` Träsk, `0.30` Sjö).
//! - `forna_miljo`: fyra rena flodslättsmiljöer ur `Flodesklass` (`LandSkog`, `RinnandeStrand`,
//!   `TraskBland`, `SjoOppen`).
//! - Dynamik (stående förnapool, nedbrytningskonstant `k`, algblomning, betning och allt `Djurplankton`)
//!   hör strikt till `n10/hcsim` (sessionen BRAIN, Alt 1968) efter fulltextkontroll (Alt 2184).
//! - Fil-I/O är frivillig (`har_basunderlag: bool`) och rör aldrig `k.mark`, läge 1 eller `karta.json`.

use std::fs;
use std::path::Path;

use crate::{flodesklass_av, FlodAndelar, Flodesklass, Karta, Lager, Marke, VAXT_N};

/// Ren geometrisk miljöklass per ruta för förna- och algunderlaget (Alt 2180).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FornaMiljo {
    LandSkog = 0,
    RinnandeStrand = 1,
    TraskBland = 2,
    SjoOppen = 3,
}

/// Kartans geometriska basunderlag per ruta för förna, alger och växtplankton (Alt 2180 + Alt 2184).
/// När `har_basunderlag == false` (läge 1 eller äldre kartmapp utan `basunderlag.json`) är lagren tomma.
#[derive(Clone, Debug, PartialEq)]
pub struct BasUnderlag {
    pub har_basunderlag: bool,
    pub vattenyta_andel: Lager<f32>,
    pub kron_tathet: Lager<f32>,
    pub vattenvaxt_andel: Lager<f32>,
    pub forna_miljo: Lager<FornaMiljo>,
}

impl BasUnderlag {
    /// Tomt basunderlag (läge 1 eller äldre kartmapp utan `basunderlag.json`).
    pub fn tom() -> Self {
        Self {
            har_basunderlag: false,
            vattenyta_andel: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            kron_tathet: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            vattenvaxt_andel: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            forna_miljo: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
        }
    }
}

/// En källrad ur `forna_alger_plankton.tsv` (13 kolumner, Alt 2183).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasKallaRad {
    pub resurs: String,
    pub miljo: String,
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

/// Mappar `Flodesklass` till ren miljöklass `FornaMiljo`.
pub fn forna_miljo_av(fk: Flodesklass) -> FornaMiljo {
    match fk {
        Flodesklass::Land => FornaMiljo::LandSkog,
        Flodesklass::Rinnande => FornaMiljo::RinnandeStrand,
        Flodesklass::Trask => FornaMiljo::TraskBland,
        Flodesklass::Sjo => FornaMiljo::SjoOppen,
    }
}

/// Faktisk vattenyta som andel av rutan (`0.0..=1.0`):
/// `0.0` på `Land`, `fara_andel` på `Rinnande` (delrutearea, aldrig `1.0` för smala fåror),
/// och `1.0` på `Träsk` och `Sjö`.
pub fn vattenyta_andel_av(fk: Flodesklass, fara_andel: f32) -> f32 {
    match fk {
        Flodesklass::Land => 0.0,
        Flodesklass::Rinnande => {
            if fara_andel > 0.0 {
                fara_andel.clamp(0.0, 1.0)
            } else {
                // Läge 1 utan fara_andel (hel Flodfara-ruta).
                1.0
            }
        }
        Flodesklass::Trask | Flodesklass::Sjo => 1.0,
    }
}

/// Samlad vedartad krontäthet ur kartans sex växtgrupper (`lovtrad + barrtrad + kottepalm_ginkgo`).
pub fn kron_tathet_av(vaxt: [f32; VAXT_N]) -> f32 {
    vaxt[0] + vaxt[1] + vaxt[4]
}

/// Härleder `BasUnderlag` deterministiskt ur en befintlig `Karta` och frivillig `FlodAndelar`.
/// Lämnar `k.mark` och `karta.json` helt orörda.
pub fn rakna_basunderlag(k: &Karta, andelar: Option<&FlodAndelar>) -> Result<BasUnderlag, String> {
    let n = k.omrade.b * k.omrade.h;
    if k.vattentyp.varden.len() != n {
        return Err(format!(
            "vattentyp har längd {}, väntade {}",
            k.vattentyp.varden.len(),
            n
        ));
    }
    if let Some(a) = andelar {
        if a.fara_andel.varden.len() != n {
            return Err(format!(
                "fara_andel har längd {}, väntade {}",
                a.fara_andel.varden.len(),
                n
            ));
        }
        if a.har_vaxtgrupper && a.vaxt_tathet.varden.len() != n {
            return Err(format!(
                "vaxt_tathet har längd {}, väntade {}",
                a.vaxt_tathet.varden.len(),
                n
            ));
        }
    }

    let mut vattenyta_andel = Vec::with_capacity(n);
    let mut kron_tathet = Vec::with_capacity(n);
    let mut vattenvaxt_andel = Vec::with_capacity(n);
    let mut forna_miljo = Vec::with_capacity(n);

    for c in 0..n {
        let vt = k.vattentyp.varden[c];
        let fa = andelar.map(|a| a.fara_andel.varden[c]).unwrap_or(0.0);
        let fk = flodesklass_av(vt, fa);
        vattenyta_andel.push(vattenyta_andel_av(fk, fa));
        forna_miljo.push(forna_miljo_av(fk));

        let (kron, vv) = match andelar {
            Some(a) if a.har_vaxtgrupper => {
                let vx = a.vaxt_tathet.varden[c];
                (kron_tathet_av(vx), vx[5])
            }
            _ => (0.0, 0.0),
        };
        kron_tathet.push(kron);
        vattenvaxt_andel.push(vv);
    }

    Ok(BasUnderlag {
        har_basunderlag: true,
        vattenyta_andel: Lager {
            varden: vattenyta_andel,
            marke: Marke::Simulering,
        },
        kron_tathet: Lager {
            varden: kron_tathet,
            marke: Marke::Simulering,
        },
        vattenvaxt_andel: Lager {
            varden: vattenvaxt_andel,
            marke: Marke::Simulering,
        },
        forna_miljo: Lager {
            varden: forna_miljo,
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

/// FNV-1a 64 över `BasUnderlag` (alla fyra lager och deras märken).
pub fn kontrollsumma_basunderlag(bu: &BasUnderlag) -> u64 {
    let mut h = FNV_OFFSET;
    fnv_byte(&mut h, u8::from(bu.har_basunderlag));
    if !bu.har_basunderlag {
        return h;
    }
    for &v in &bu.vattenyta_andel.varden {
        fnv_bytes(&mut h, &v.to_le_bytes());
    }
    fnv_byte(&mut h, marke_kod(bu.vattenyta_andel.marke));
    for &v in &bu.kron_tathet.varden {
        fnv_bytes(&mut h, &v.to_le_bytes());
    }
    fnv_byte(&mut h, marke_kod(bu.kron_tathet.marke));
    for &v in &bu.vattenvaxt_andel.varden {
        fnv_bytes(&mut h, &v.to_le_bytes());
    }
    fnv_byte(&mut h, marke_kod(bu.vattenvaxt_andel.marke));
    for &m in &bu.forna_miljo.varden {
        fnv_byte(&mut h, m as u8);
    }
    fnv_byte(&mut h, marke_kod(bu.forna_miljo.marke));
    h
}

/// Skriver frivilliga basunderlagsfiler (`vattenyta_andel.f32`, `kron_tathet.f32`,
/// `vattenvaxt_andel.f32`, `forna_miljo.u8`, `basunderlag.json`) utan att röra `karta.json` (Alt 2184).
pub fn skriv_basunderlag(bu: &BasUnderlag, mapp: &Path) -> Result<u64, String> {
    if !bu.har_basunderlag {
        return Err("vägrar skriva basunderlag när har_basunderlag är false".into());
    }
    let n = bu.vattenyta_andel.varden.len();
    if bu.kron_tathet.varden.len() != n
        || bu.vattenvaxt_andel.varden.len() != n
        || bu.forna_miljo.varden.len() != n
    {
        return Err("basunderlagets lager har olika längd".into());
    }
    fs::create_dir_all(mapp)
        .map_err(|e| format!("kunde inte skapa mapp {}: {e}", mapp.display()))?;

    let skriv_f32 = |namn: &str, data: &[f32]| -> Result<(), String> {
        let mut bs = Vec::with_capacity(data.len() * 4);
        for &v in data {
            bs.extend_from_slice(&v.to_le_bytes());
        }
        fs::write(mapp.join(namn), &bs).map_err(|e| format!("{namn}: {e}"))
    };
    skriv_f32("vattenyta_andel.f32", &bu.vattenyta_andel.varden)?;
    skriv_f32("kron_tathet.f32", &bu.kron_tathet.varden)?;
    skriv_f32("vattenvaxt_andel.f32", &bu.vattenvaxt_andel.varden)?;

    let miljo_bytes: Vec<u8> = bu.forna_miljo.varden.iter().map(|&m| m as u8).collect();
    fs::write(mapp.join("forna_miljo.u8"), &miljo_bytes)
        .map_err(|e| format!("forna_miljo.u8: {e}"))?;

    let summa = kontrollsumma_basunderlag(bu);
    let m_str = |m: Marke| match m {
        Marke::Last => "last",
        Marke::Simulering => "simulering",
    };
    let json = format!(
        "{{\n  \"version\": 1,\n  \"antal_rutor\": {n},\n  \"kontrollsumma\": \"{summa}\",\n  \"marke_vattenyta_andel\": \"{}\",\n  \"marke_kron_tathet\": \"{}\",\n  \"marke_vattenvaxt_andel\": \"{}\",\n  \"marke_forna_miljo\": \"{}\",\n  \"forna_miljo_koder\": {{\"0\": \"LandSkog\", \"1\": \"RinnandeStrand\", \"2\": \"TraskBland\", \"3\": \"SjoOppen\"}}\n}}\n",
        m_str(bu.vattenyta_andel.marke),
        m_str(bu.kron_tathet.marke),
        m_str(bu.vattenvaxt_andel.marke),
        m_str(bu.forna_miljo.marke),
    );
    fs::write(mapp.join("basunderlag.json"), json.as_bytes())
        .map_err(|e| format!("basunderlag.json: {e}"))?;
    Ok(summa)
}

fn las_json_str(text: &str, nyckel: &str) -> Result<String, String> {
    let m = format!("\"{nyckel}\": \"");
    let start = text
        .find(&m)
        .ok_or_else(|| format!("basunderlag.json saknar {nyckel}"))?
        + m.len();
    let slut = text[start..]
        .find('"')
        .ok_or_else(|| format!("basunderlag.json ogiltig sträng för {nyckel}"))?
        + start;
    Ok(text[start..slut].to_string())
}

fn parsa_marke(s: &str) -> Result<Marke, String> {
    match s {
        "last" => Ok(Marke::Last),
        "simulering" => Ok(Marke::Simulering),
        annat => Err(format!("okänt märke i basunderlag.json: {annat:?}")),
    }
}

fn las_f32_fil(mapp: &Path, namn: &str, n: usize) -> Result<Vec<f32>, String> {
    let raw = fs::read(mapp.join(namn)).map_err(|e| format!("{namn}: {e}"))?;
    if raw.len() != n * 4 {
        return Err(format!(
            "{namn} har längd {} byte, väntade {}",
            raw.len(),
            n * 4
        ));
    }
    let mut ut = Vec::with_capacity(n);
    for ch in raw.chunks_exact(4) {
        ut.push(f32::from_le_bytes([ch[0], ch[1], ch[2], ch[3]]));
    }
    Ok(ut)
}

/// Läser frivilliga basunderlagsfiler ur `mapp` (Alt 2184).
/// Saknas `basunderlag.json` returneras `BasUnderlag::tom()` med `har_basunderlag == false`
/// (kraschar aldrig på äldre mappar eller läge 1).
pub fn las_basunderlag(mapp: &Path, b: usize, h: usize) -> Result<BasUnderlag, String> {
    let json_path = mapp.join("basunderlag.json");
    if !json_path.exists() {
        return Ok(BasUnderlag::tom());
    }
    let n = b * h;
    let text = fs::read_to_string(&json_path)
        .map_err(|e| format!("basunderlag.json kunde inte läsas: {e}"))?;
    let summa_fil: u64 = las_json_str(&text, "kontrollsumma")?
        .parse()
        .map_err(|_| "ogiltig kontrollsumma i basunderlag.json".to_string())?;
    let m_va = parsa_marke(&las_json_str(&text, "marke_vattenyta_andel")?)?;
    let m_kr = parsa_marke(&las_json_str(&text, "marke_kron_tathet")?)?;
    let m_vv = parsa_marke(&las_json_str(&text, "marke_vattenvaxt_andel")?)?;
    let m_fm = parsa_marke(&las_json_str(&text, "marke_forna_miljo")?)?;

    let vattenyta_andel = las_f32_fil(mapp, "vattenyta_andel.f32", n)?;
    let kron_tathet = las_f32_fil(mapp, "kron_tathet.f32", n)?;
    let vattenvaxt_andel = las_f32_fil(mapp, "vattenvaxt_andel.f32", n)?;

    let fm_raw = fs::read(mapp.join("forna_miljo.u8")).map_err(|e| format!("forna_miljo.u8: {e}"))?;
    if fm_raw.len() != n {
        return Err(format!(
            "forna_miljo.u8 har längd {}, väntade {}",
            fm_raw.len(),
            n
        ));
    }
    let mut forna_miljo = Vec::with_capacity(n);
    for (i, &kod) in fm_raw.iter().enumerate() {
        forna_miljo.push(match kod {
            0 => FornaMiljo::LandSkog,
            1 => FornaMiljo::RinnandeStrand,
            2 => FornaMiljo::TraskBland,
            3 => FornaMiljo::SjoOppen,
            annat => return Err(format!("forna_miljo.u8 ruta {i}: okänd kod {annat}")),
        });
    }

    let bu = BasUnderlag {
        har_basunderlag: true,
        vattenyta_andel: Lager {
            varden: vattenyta_andel,
            marke: m_va,
        },
        kron_tathet: Lager {
            varden: kron_tathet,
            marke: m_kr,
        },
        vattenvaxt_andel: Lager {
            varden: vattenvaxt_andel,
            marke: m_vv,
        },
        forna_miljo: Lager {
            varden: forna_miljo,
            marke: m_fm,
        },
    };
    let beraknad = kontrollsumma_basunderlag(&bu);
    if beraknad != summa_fil {
        return Err(format!(
            "basunderlagets kontrollsumma stämmer inte: filen har {summa_fil}, läst data ger {beraknad}"
        ));
    }
    Ok(bu)
}

/// Skriver en 3D-mapp med befintlig `skriv_3d_med` (oförändrad kontrollsumma för läge 1 och 2)
/// och skriver frivilligt `BasUnderlag` bredvid om `Some(bu)` ges (Alt 2184).
pub fn skriv_3d_med_basunderlag(
    k: &Karta,
    andelar: Option<&FlodAndelar>,
    bu: Option<&BasUnderlag>,
    mapp: &Path,
) -> Result<u64, String> {
    let summa = crate::skriv_3d_med(k, andelar, mapp)?;
    if let Some(b) = bu {
        if b.har_basunderlag {
            skriv_basunderlag(b, mapp)?;
        }
    }
    Ok(summa)
}

/// Läser en 3D-mapp med befintlig `las_3d_med` samt frivilligt `BasUnderlag` (Alt 2184).
/// För en äldre mapp eller läge 1 blir `bu.har_basunderlag == false`.
pub fn las_3d_med_basunderlag(
    mapp: &Path,
) -> Result<(Karta, Option<FlodAndelar>, BasUnderlag), String> {
    let (k, andelar) = crate::las_3d_med(mapp)?;
    let bu = las_basunderlag(mapp, k.omrade.b, k.omrade.h)?;
    Ok((k, andelar, bu))
}

const GILTIG_LAST: [&str; 4] = ["fulltext", "sammandrag", "fulltext_del", "ej_last"];
const GILTIG_URSPRUNG: [&str; 5] = ["hell_creek", "grannformation", "aldre", "globalt", "nutid"];
const GILTIG_MARKNING: [&str; 4] = ["Hell Creek", "lanat", "eget val", "ingen_kalla"];
const EJ_HELL_CREEK_PLATS: [&str; 15] = [
    "louisiana",
    "fujian",
    "kina",
    "kaiparowits",
    "utah",
    "patagonien",
    "argentina",
    "mexiko",
    "kalifornien",
    "nya zeeland",
    "themsen",
    "libanon",
    "lance",
    "globalt",
    "nutid",
];
const FEL_BOWES_DOI: &str = "10.1016/j.scitotenv.2011.09.033";

/// Läser och validerar källtabellen `forna_alger_plankton.tsv` (Alt 2183).
/// Spärrar:
/// - Exakt 13 kolumner inklusive `ursprung`.
/// - Inga tal på `ej_last`- eller `ingen_kalla`-rader; lästa rader kräver `10.`-DOI och citat >= 10 tecken.
/// - `ursprung == "hell_creek"` får aldrig hänvisa till nutida/äldre/globala platser i `plats_tid`, och
///   `markning == "Hell Creek"` kräver `ursprung == "hell_creek"`.
/// - Spärrar nutida övergödda `260` g C/m²/år på sjöproduktionsrader (Lewis 2011 bakgrund = 160).
/// - Spärrar kiselalgsdominans på `hell_creek`-rader (sen krita dominerades av grönalger/kransalger).
/// - Spärrar djurplanktonbiomassa på `hell_creek`-rader (levande konsument i `hcsim`, `ingen_kalla`).
/// - Spärrar den tidigare felaktiga Bowes-DOI:n (`10.1016/j.scitotenv.2011.09.033`).
pub fn las_forna_alger_plankton_tsv(text: &str) -> Result<Vec<BasKallaRad>, String> {
    let mut rader = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let rubrik: Vec<&str> = rader
        .next()
        .ok_or_else(|| "forna_alger_plankton.tsv är tom".to_string())?
        .split('\t')
        .collect();
    let forvantad = [
        "resurs",
        "miljo",
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
            "ogiltig rubrikrad i forna_alger_plankton.tsv: fick {rubrik:?}"
        ));
    }
    let mut ut = Vec::new();
    for (idx, rad) in rader.enumerate() {
        let c: Vec<&str> = rad.split('\t').collect();
        if c.len() != 13 {
            return Err(format!("rad {}: {} kolumner, ska vara 13", idx + 1, c.len()));
        }
        let resurs = c[0];
        let matt = c[2];
        let varde = c[3];
        let doi = c[6];
        let citat = c[7];
        let last = c[8];
        let plats_tid = c[9];
        let ursprung = c[10];
        let markning = c[11];
        let forbehall = c[12];

        if doi.eq_ignore_ascii_case(FEL_BOWES_DOI) {
            return Err(format!(
                "rad {}: förbjuden fel-DOI {doi:?} (ska vara 10.1016/j.scitotenv.2011.09.082)",
                idx + 1
            ));
        }
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
            let v_low = varde.to_lowercase();
            if v_low.contains("kiselalg") || v_low.contains("diatom") {
                return Err(format!(
                    "rad {}: kiselalgsdominans får inte märkas som hell_creek ({varde:?})",
                    idx + 1
                ));
            }
            if resurs == "djurplankton" && (markning != "ingen_kalla" || varde != "-") {
                return Err(format!(
                    "rad {}: djurplankton i Hell Creek måste vara ingen_kalla/- (ägs av hcsim)",
                    idx + 1
                ));
            }
        }
        if (matt.contains("sjo_npp") || matt.contains("sjo_produktion")) && varde == "260" {
            return Err(format!(
                "rad {}: övergödda 260 g C/m²/år är spärrat som sjöbaslinje (använd förindustriell bakgrund 160)",
                idx + 1
            ));
        }
        if forbehall.trim().is_empty() || forbehall == "-" {
            return Err(format!("rad {}: förbehåll saknas", idx + 1));
        }
        ut.push(BasKallaRad {
            resurs: resurs.to_string(),
            miljo: c[1].to_string(),
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
        return Err("forna_alger_plankton.tsv har inga datarader".into());
    }
    Ok(ut)
}
