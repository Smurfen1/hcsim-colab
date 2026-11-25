//! Fysisk vattenkvalitet för Hell Creek-kartan (enhällig omröstning Alt 2098, Alt 2100, Alt 2103;
//! Håkans Alt 2076, Alt 2083 och Alt 2109; protokoll `n10/docs/2026-11-24-omrostning-vatten.md`).
//!
//! Ägargräns (regel 15): `n10/hckarta` (SIMULERINGEN) levererar enbart fysiska egenskaper per ruta
//! (flödesklass härledd ur `fara_andel > 0` och `Vatten`, samt salthaltsklass och ett valfritt
//! `salthalt_psu`-fält som är `None` i dagens Montana-fönster) och den låsta källtabellen
//! `vattenkvalitet.tsv`. Ingen statisk basrisk i kartan (avvisad i Alt 2098 mot Alt 2099);
//! temperatur, torka, kadaver, törst, förgiftning och smitta per individ hör till `n10/hcsim` (BRAIN).

use std::fs;
use std::path::Path;

use crate::{FlodAndelar, Karta, Lager, Marke, Vatten};

/// Fysisk flödesklass per ruta i kartan (Alt 2098).
/// Varje ruta får exakt en klass härledd ur `fara_andel > 0` och `Vatten`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Flodesklass {
    Land = 0,
    Rinnande = 1,
    Sjo = 2,
    Trask = 3,
}

/// Fysisk salthaltsklass per ruta (Alt 2098; Fables krav i runda 2 punkt 3.1).
/// I dagens Montana-fönster är alla rutor `Sott` med `salthalt_psu == None` (märkt förenkling).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SalthaltKlass {
    Sott = 0,
    Brackt = 1,
    Hav = 2,
}

/// Frivilligt vattenkvalitetslager bredvid `FlodAndelar` (Alt 2098 + Alt 2103).
/// När `har_vattenkvalitet == false` (läge 1 eller äldre kartmapp utan vattenkvalitetsfiler)
/// är lagren tomma och läge 1 / äldre kontrollsummor påverkas inte.
#[derive(Clone, Debug, PartialEq)]
pub struct VattenKvalitet {
    pub har_vattenkvalitet: bool,
    pub flodesklass: Lager<Flodesklass>,
    pub salthalt_klass: Lager<SalthaltKlass>,
    /// Praktisk salthalt i psu; `None` tills ett kustfönster och lästa fulltexter motiverar tal.
    pub salthalt_psu: Lager<Option<f32>>,
}

/// En rad ur den låsta källtabellen `vattenkvalitet.tsv` (Alt 2100).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VattenKallaRad {
    pub drivare: String,
    pub flodesklass: String,
    pub matt: String,
    pub varde: String,
    pub enhet: String,
    pub kalla: String,
    pub doi: String,
    pub citat: String,
    pub last: String,
    pub plats_tid: String,
    pub markning: String,
    pub forbehall: String,
}

impl VattenKvalitet {
    /// Tom struktur för kartmappar som saknar vattenkvalitetsfiler (`har_vattenkvalitet == false`).
    pub fn tom() -> Self {
        VattenKvalitet {
            har_vattenkvalitet: false,
            flodesklass: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            salthalt_klass: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
            salthalt_psu: Lager {
                varden: Vec::new(),
                marke: Marke::Simulering,
            },
        }
    }
}

/// Härleder flödesklass för en enskild ruta ur `Vatten` och `fara_andel` (Alt 2098).
/// I standard 2 är `Vatten::Flodfara` alltid 0 och rinnande flodfåror bärs av `fara_andel > 0.0`.
pub fn flodesklass_av(vt: Vatten, fara_andel: f32) -> Flodesklass {
    if fara_andel > 0.0 || vt == Vatten::Flodfara {
        Flodesklass::Rinnande
    } else {
        match vt {
            Vatten::Sjo => Flodesklass::Sjo,
            Vatten::Trask => Flodesklass::Trask,
            Vatten::Land | Vatten::Brackt | Vatten::Hav | Vatten::Flodfara => Flodesklass::Land,
        }
    }
}

/// Härleder salthaltsklass ur rutans `Vatten`-typ (Alt 2098; Fables punkt 3.1).
pub fn salthalt_klass_av(vt: Vatten) -> SalthaltKlass {
    match vt {
        Vatten::Hav => SalthaltKlass::Hav,
        Vatten::Brackt => SalthaltKlass::Brackt,
        Vatten::Land | Vatten::Flodfara | Vatten::Trask | Vatten::Sjo => SalthaltKlass::Sott,
    }
}

/// Räknar ut `VattenKvalitet` för en karta (standard 2 med `Some(andelar)` eller läge 1 med `None`).
/// `salthalt_psu` lämnas alltid `None` (inga ogrundade psu-tal före fulltext och kustfönster).
pub fn rakna_vattenkvalitet(
    k: &Karta,
    andelar: Option<&FlodAndelar>,
) -> Result<VattenKvalitet, String> {
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
    }
    let mut flode = Vec::with_capacity(n);
    let mut salt_kl = Vec::with_capacity(n);
    let mut salt_psu = Vec::with_capacity(n);
    for c in 0..n {
        let vt = k.vattentyp.varden[c];
        let fa = andelar.map(|a| a.fara_andel.varden[c]).unwrap_or(0.0);
        flode.push(flodesklass_av(vt, fa));
        salt_kl.push(salthalt_klass_av(vt));
        salt_psu.push(None);
    }
    Ok(VattenKvalitet {
        har_vattenkvalitet: true,
        flodesklass: Lager {
            varden: flode,
            marke: Marke::Simulering,
        },
        salthalt_klass: Lager {
            varden: salt_kl,
            marke: Marke::Simulering,
        },
        salthalt_psu: Lager {
            varden: salt_psu,
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

/// FNV-1a 64 över `VattenKvalitet` (flödesklass, salthaltsklass, valfri salthalt_psu och märken).
pub fn kontrollsumma_vattenkvalitet(vk: &VattenKvalitet) -> u64 {
    let mut h = FNV_OFFSET;
    fnv_byte(&mut h, u8::from(vk.har_vattenkvalitet));
    if !vk.har_vattenkvalitet {
        return h;
    }
    for &f in &vk.flodesklass.varden {
        fnv_byte(&mut h, f as u8);
    }
    fnv_byte(&mut h, marke_kod(vk.flodesklass.marke));
    for &s in &vk.salthalt_klass.varden {
        fnv_byte(&mut h, s as u8);
    }
    fnv_byte(&mut h, marke_kod(vk.salthalt_klass.marke));
    for &p in &vk.salthalt_psu.varden {
        match p {
            None => fnv_byte(&mut h, 0),
            Some(v) => {
                fnv_byte(&mut h, 1);
                fnv_bytes(&mut h, &v.to_le_bytes());
            }
        }
    }
    fnv_byte(&mut h, marke_kod(vk.salthalt_psu.marke));
    h
}

/// Skriver frivilliga vattenkvalitetsfiler (`flodesklass.u8`, `salthalt_klass.u8`, `salthalt_psu.f32`,
/// `vattenkvalitet.json`) till `mapp` utan att röra `karta.json` eller äldre kontrollsummor (Alt 2103).
/// I `salthalt_psu.f32` kodas `None` som `f32::NAN` (nodata = NaN).
pub fn skriv_vattenkvalitet(vk: &VattenKvalitet, mapp: &Path) -> Result<u64, String> {
    if !vk.har_vattenkvalitet {
        return Err("vägrar skriva vattenkvalitet när har_vattenkvalitet är false".into());
    }
    let n = vk.flodesklass.varden.len();
    if vk.salthalt_klass.varden.len() != n || vk.salthalt_psu.varden.len() != n {
        return Err("vattenkvalitetslagrens längd stämmer inte överens".into());
    }
    fs::create_dir_all(mapp).map_err(|e| format!("kunde inte skapa mapp {}: {e}", mapp.display()))?;

    let flode_bytes: Vec<u8> = vk.flodesklass.varden.iter().map(|&v| v as u8).collect();
    fs::write(mapp.join("flodesklass.u8"), &flode_bytes)
        .map_err(|e| format!("flodesklass.u8: {e}"))?;

    let salt_bytes: Vec<u8> = vk.salthalt_klass.varden.iter().map(|&v| v as u8).collect();
    fs::write(mapp.join("salthalt_klass.u8"), &salt_bytes)
        .map_err(|e| format!("salthalt_klass.u8: {e}"))?;

    let mut psu_bytes = Vec::with_capacity(n * 4);
    for &p in &vk.salthalt_psu.varden {
        let v = p.unwrap_or(f32::NAN);
        psu_bytes.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(mapp.join("salthalt_psu.f32"), &psu_bytes)
        .map_err(|e| format!("salthalt_psu.f32: {e}"))?;

    let summa = kontrollsumma_vattenkvalitet(vk);
    let m_str = |m: Marke| match m {
        Marke::Last => "last",
        Marke::Simulering => "simulering",
    };
    let json = format!(
        "{{\n  \"version\": 1,\n  \"antal_rutor\": {n},\n  \"kontrollsumma\": \"{summa}\",\n  \"nodata_psu\": \"NaN\",\n  \"marke_flodesklass\": \"{}\",\n  \"marke_salthalt_klass\": \"{}\",\n  \"marke_salthalt_psu\": \"{}\",\n  \"flodesklass_koder\": {{\"0\": \"Land\", \"1\": \"Rinnande\", \"2\": \"Sjo\", \"3\": \"Trask\"}},\n  \"salthalt_klass_koder\": {{\"0\": \"Sott\", \"1\": \"Brackt\", \"2\": \"Hav\"}}\n}}\n",
        m_str(vk.flodesklass.marke),
        m_str(vk.salthalt_klass.marke),
        m_str(vk.salthalt_psu.marke),
    );
    fs::write(mapp.join("vattenkvalitet.json"), json.as_bytes())
        .map_err(|e| format!("vattenkvalitet.json: {e}"))?;
    Ok(summa)
}

fn las_json_str(text: &str, nyckel: &str) -> Result<String, String> {
    let m = format!("\"{nyckel}\": \"");
    let start = text
        .find(&m)
        .ok_or_else(|| format!("vattenkvalitet.json saknar {nyckel}"))?
        + m.len();
    let slut = text[start..]
        .find('"')
        .ok_or_else(|| format!("vattenkvalitet.json ogiltig sträng för {nyckel}"))?
        + start;
    Ok(text[start..slut].to_string())
}

fn parsa_marke(s: &str) -> Result<Marke, String> {
    match s {
        "last" => Ok(Marke::Last),
        "simulering" => Ok(Marke::Simulering),
        annat => Err(format!("okänt märke i vattenkvalitet.json: {annat:?}")),
    }
}

/// Läser frivilliga vattenkvalitetsfiler ur `mapp` (Alt 2103).
/// Saknas `vattenkvalitet.json` returneras `VattenKvalitet::tom()` med `har_vattenkvalitet == false`
/// (kraschar aldrig på äldre mappar eller läge 1). Finns den, prövas alla filer och kontrollsumman.
pub fn las_vattenkvalitet(mapp: &Path, b: usize, h: usize) -> Result<VattenKvalitet, String> {
    let json_path = mapp.join("vattenkvalitet.json");
    if !json_path.exists() {
        return Ok(VattenKvalitet::tom());
    }
    let n = b * h;
    let text = fs::read_to_string(&json_path)
        .map_err(|e| format!("vattenkvalitet.json kunde inte läsas: {e}"))?;
    let summa_fil: u64 = las_json_str(&text, "kontrollsumma")?
        .parse()
        .map_err(|_| "ogiltig kontrollsumma i vattenkvalitet.json".to_string())?;
    let marke_flode = parsa_marke(&las_json_str(&text, "marke_flodesklass")?)?;
    let marke_salt_kl = parsa_marke(&las_json_str(&text, "marke_salthalt_klass")?)?;
    let marke_salt_psu = parsa_marke(&las_json_str(&text, "marke_salthalt_psu")?)?;

    let flode_raw =
        fs::read(mapp.join("flodesklass.u8")).map_err(|e| format!("flodesklass.u8: {e}"))?;
    if flode_raw.len() != n {
        return Err(format!(
            "flodesklass.u8 har längd {}, väntade {}",
            flode_raw.len(),
            n
        ));
    }
    let mut flode = Vec::with_capacity(n);
    for (i, &kod) in flode_raw.iter().enumerate() {
        flode.push(match kod {
            0 => Flodesklass::Land,
            1 => Flodesklass::Rinnande,
            2 => Flodesklass::Sjo,
            3 => Flodesklass::Trask,
            annat => return Err(format!("flodesklass.u8 ruta {i}: okänd kod {annat}")),
        });
    }

    let salt_raw = fs::read(mapp.join("salthalt_klass.u8"))
        .map_err(|e| format!("salthalt_klass.u8: {e}"))?;
    if salt_raw.len() != n {
        return Err(format!(
            "salthalt_klass.u8 har längd {}, väntade {}",
            salt_raw.len(),
            n
        ));
    }
    let mut salt_kl = Vec::with_capacity(n);
    for (i, &kod) in salt_raw.iter().enumerate() {
        salt_kl.push(match kod {
            0 => SalthaltKlass::Sott,
            1 => SalthaltKlass::Brackt,
            2 => SalthaltKlass::Hav,
            annat => return Err(format!("salthalt_klass.u8 ruta {i}: okänd kod {annat}")),
        });
    }

    let psu_raw =
        fs::read(mapp.join("salthalt_psu.f32")).map_err(|e| format!("salthalt_psu.f32: {e}"))?;
    if psu_raw.len() != n * 4 {
        return Err(format!(
            "salthalt_psu.f32 har längd {} byte, väntade {}",
            psu_raw.len(),
            n * 4
        ));
    }
    let mut salt_psu = Vec::with_capacity(n);
    for chunk in psu_raw.chunks_exact(4) {
        let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        salt_psu.push(if v.is_nan() { None } else { Some(v) });
    }

    let vk = VattenKvalitet {
        har_vattenkvalitet: true,
        flodesklass: Lager {
            varden: flode,
            marke: marke_flode,
        },
        salthalt_klass: Lager {
            varden: salt_kl,
            marke: marke_salt_kl,
        },
        salthalt_psu: Lager {
            varden: salt_psu,
            marke: marke_salt_psu,
        },
    };
    let beraknad = kontrollsumma_vattenkvalitet(&vk);
    if beraknad != summa_fil {
        return Err(format!(
            "vattenkvalitetens kontrollsumma stämmer inte: filen har {summa_fil}, läst data ger {beraknad}"
        ));
    }
    Ok(vk)
}

/// Skriver en 3D-mapp med befintlig `skriv_3d_med` (oförändrad kontrollsumma för läge 1 och 2)
/// och skriver frivilligt `VattenKvalitet` bredvid om `Some(vk)` ges (Alt 2103).
pub fn skriv_3d_med_vattenkvalitet(
    k: &Karta,
    andelar: Option<&FlodAndelar>,
    vk: Option<&VattenKvalitet>,
    mapp: &Path,
) -> Result<u64, String> {
    let summa = crate::skriv_3d_med(k, andelar, mapp)?;
    if let Some(v) = vk {
        if v.har_vattenkvalitet {
            skriv_vattenkvalitet(v, mapp)?;
        }
    }
    Ok(summa)
}

/// Läser en 3D-mapp med befintlig `las_3d_med` samt frivillig `VattenKvalitet` (Alt 2103).
/// För en äldre mapp eller läge 1 blir `vk.har_vattenkvalitet == false`.
pub fn las_3d_med_vattenkvalitet(
    mapp: &Path,
) -> Result<(Karta, Option<FlodAndelar>, VattenKvalitet), String> {
    let (k, andelar) = crate::las_3d_med(mapp)?;
    let vk = las_vattenkvalitet(mapp, k.omrade.b, k.omrade.h)?;
    Ok((k, andelar, vk))
}

const GILTIG_LAST: [&str; 4] = ["fulltext", "sammandrag", "fulltext_del", "ej_last"];
const GILTIG_MARKNING: [&str; 4] = ["Hell Creek", "lanat", "eget val", "ingen_kalla"];

/// Läser och validerar källtabellen `vattenkvalitet.tsv` (Alt 2100).
/// Varje rad ska ha exakt 12 kolumner, giltig `last` och `markning`, och (för källbelagda rader)
/// DOI som börjar på `10.` samt citat på minst 10 tecken (ej för `ej_last` / `ingen_kalla`).
pub fn las_vattenkvalitet_tsv(text: &str) -> Result<Vec<VattenKallaRad>, String> {
    let mut rader = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let rubrik: Vec<&str> = rader
        .next()
        .ok_or_else(|| "vattenkvalitet.tsv är tom".to_string())?
        .split('\t')
        .collect();
    let forvantad = [
        "drivare",
        "flodesklass",
        "matt",
        "varde",
        "enhet",
        "kalla",
        "doi",
        "citat",
        "last",
        "plats_tid",
        "markning",
        "forbehall",
    ];
    if rubrik != forvantad {
        return Err(format!(
            "ogiltig rubrikrad i vattenkvalitet.tsv: fick {rubrik:?}"
        ));
    }
    let mut ut = Vec::new();
    for (idx, rad) in rader.enumerate() {
        let c: Vec<&str> = rad.split('\t').collect();
        if c.len() != 12 {
            return Err(format!("rad {}: {} kolumner, ska vara 12", idx + 1, c.len()));
        }
        let last = c[8];
        let markning = c[10];
        if !GILTIG_LAST.contains(&last) {
            return Err(format!("rad {}: ogiltig läsning {last:?}", idx + 1));
        }
        if !GILTIG_MARKNING.contains(&markning) {
            return Err(format!("rad {}: ogiltig märkning {markning:?}", idx + 1));
        }
        if markning == "ingen_kalla" || last == "ej_last" {
            if c[3] != "-" {
                return Err(format!(
                    "rad {}: tal {:?} får inte bäras utan läst källa",
                    idx + 1,
                    c[3]
                ));
            }
        } else {
            if !c[6].starts_with("10.") {
                return Err(format!("rad {}: ogiltig eller saknad DOI {:?}", idx + 1, c[6]));
            }
            if c[7] == "-" || c[7].len() < 10 {
                return Err(format!("rad {}: saknat eller för kort citat", idx + 1));
            }
        }
        if c[11].trim().is_empty() || c[11] == "-" {
            return Err(format!("rad {}: förbehåll saknas", idx + 1));
        }
        ut.push(VattenKallaRad {
            drivare: c[0].to_string(),
            flodesklass: c[1].to_string(),
            matt: c[2].to_string(),
            varde: c[3].to_string(),
            enhet: c[4].to_string(),
            kalla: c[5].to_string(),
            doi: c[6].to_string(),
            citat: c[7].to_string(),
            last: last.to_string(),
            plats_tid: c[9].to_string(),
            markning: markning.to_string(),
            forbehall: c[11].to_string(),
        });
    }
    if ut.is_empty() {
        return Err("vattenkvalitet.tsv har inga datarader".into());
    }
    Ok(ut)
}
