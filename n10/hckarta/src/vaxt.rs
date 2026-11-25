//! Växtgrupper med höjd och täthet per ruta (Alt 1416 / 1433). Bara flodversion 2.
//! Höjder och tätheter är egna val; Johnson 2002 [A] styr örter vid fåran och <10 %-gränsen.

use crate::{Period, Vatten};

/// Antal växtgrupper (ordning fast i alla filer).
pub const VAXT_N: usize = 6;

/// Gruppnamn i ordningen lovtrad … vattenvaxt.
pub const VAXT_NAMN: [&str; VAXT_N] = [
    "lovtrad",
    "barrtrad",
    "ort",
    "ormbunke",
    "kottepalm_ginkgo",
    "vattenvaxt",
];

/// Höjder i meter (eget val; underlaget 15; 25; 0,5; 1; 2; 0,1).
pub const VAXT_HOJD_M: [f32; VAXT_N] = [15.0, 25.0, 0.5, 1.0, 2.0, 0.1];

/// Index för örter i täthetsvektorn.
pub const VAXT_ORT: usize = 2;

/// Inbäddad artkoppling (Cursor ändrar inte `vaxtgrupper.tsv`).
const VAXTGRUPPER_TSV: &str = include_str!("../vaxtgrupper.tsv");

/// En rad i `vaxtgrupper.tsv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaxtArtRad {
    pub art: String,
    pub grupp: String,
    pub kalla: String,
}

/// Antal arter per känd grupp (sex tal) plus ev. `okand`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaxtgrupperLasning {
    pub rader: Vec<VaxtArtRad>,
    /// Antal arter per grupp i `VAXT_NAMN`-ordning (okand räknas inte hit).
    pub arter_per_grupp: [usize; VAXT_N],
    pub okand: usize,
}

fn grupp_index(namn: &str) -> Option<usize> {
    VAXT_NAMN.iter().position(|&g| g == namn)
}

/// Läser artkopplingen. Fel om gruppen är okänd, eller om källa saknas när gruppen inte är `okand`.
pub fn las_vaxtgrupper_tsv(text: &str) -> Result<VaxtgrupperLasning, String> {
    let mut rader = Vec::new();
    let mut arter_per_grupp = [0usize; VAXT_N];
    let mut okand = 0usize;
    for (nr, rad) in text.lines().enumerate() {
        let rad = rad.trim_end();
        if rad.is_empty() || rad.starts_with('#') {
            continue;
        }
        let falt: Vec<&str> = rad.split('\t').collect();
        if falt.len() < 3 {
            return Err(format!(
                "vaxtgrupper.tsv rad {}: väntade minst 3 fält, fick {}",
                nr + 1,
                falt.len()
            ));
        }
        let art = falt[0].trim().to_string();
        let grupp = falt[1].trim().to_string();
        let kalla = falt[2..].join("\t").trim().to_string();
        if art.is_empty() {
            return Err(format!("vaxtgrupper.tsv rad {}: art saknas", nr + 1));
        }
        if grupp == "okand" {
            okand += 1;
        } else if let Some(i) = grupp_index(&grupp) {
            if kalla.is_empty() {
                return Err(format!(
                    "vaxtgrupper.tsv rad {}: gruppen {grupp} saknar källa",
                    nr + 1
                ));
            }
            arter_per_grupp[i] += 1;
        } else {
            return Err(format!(
                "vaxtgrupper.tsv rad {}: okänd grupp {grupp:?}",
                nr + 1
            ));
        }
        rader.push(VaxtArtRad { art, grupp, kalla });
    }
    Ok(VaxtgrupperLasning {
        rader,
        arter_per_grupp,
        okand,
    })
}

/// Standardfilen inbäddad i binären.
pub fn las_vaxtgrupper_inbyggd() -> Result<VaxtgrupperLasning, String> {
    las_vaxtgrupper_tsv(VAXTGRUPPER_TSV)
}

/// Täthet per grupp för en ruta (andel 0–1). Talen är underlagets egna val.
pub fn vaxt_tathet_ruta(
    vatten: Vatten,
    period: Period,
    balte_andel: f32,
    vall_andel: f32,
) -> [f32; VAXT_N] {
    let mut t = match vatten {
        Vatten::Land => [0.50, 0.0, 0.10, 0.02, 0.01, 0.0],
        Vatten::Trask => [0.0, 0.25, 0.10, 0.05, 0.0, 0.10],
        Vatten::Sjo => [0.0, 0.0, 0.0, 0.0, 0.0, 0.30],
        Vatten::Hav | Vatten::Flodfara | Vatten::Brackt => [0.0; VAXT_N],
    };
    // Johnson 2002 [A]: örter vid stora fåror i mitten av formationen; period Aldre och +0,20 = eget val.
    if vatten == Vatten::Land
        && period == Period::Aldre
        && (balte_andel > 0.0 || vall_andel > 0.0)
    {
        t[VAXT_ORT] = 0.30;
    }
    t
}

/// Alt 1694 (agenterna, Håkans Alt 1680): träskets täthet i standardvägen (tidsmodell 2 med lutning ≥ 2): lövträd 0,05 och barrträd 0,20
/// i stället för 0 och 0,25; summan 0,50 kvar; eget val. Tidsmodell 1 och lutning 1 behåller `vaxt_tathet_ruta` oförändrad.
pub const TRASK_TATHET_ALT1694: [f32; VAXT_N] = [0.05, 0.20, 0.10, 0.05, 0.0, 0.10];

/// Täthet per ruta i standardvägen (Alt 1694): som `vaxt_tathet_ruta`, men träsket får `TRASK_TATHET_ALT1694`.
pub fn vaxt_tathet_ruta_standard(vatten: Vatten, period: Period, balte_andel: f32, vall_andel: f32) -> [f32; VAXT_N] {
    if vatten == Vatten::Trask {
        TRASK_TATHET_ALT1694
    } else {
        vaxt_tathet_ruta(vatten, period, balte_andel, vall_andel)
    }
}

/// Som `rakna_vaxt_tathet`; med `alt1694` (standardvägen) räknas varje ruta med `vaxt_tathet_ruta_standard`.
pub fn rakna_vaxt_tathet_med(vattentyp: &[Vatten], period: &[Period], balte_andel: &[f32], vall_andel: &[f32], alt1694: bool) -> Vec<[f32; VAXT_N]> {
    let mut ut = rakna_vaxt_tathet(vattentyp, period, balte_andel, vall_andel);
    if alt1694 {
        for (i, rad) in ut.iter_mut().enumerate() {
            *rad = vaxt_tathet_ruta_standard(vattentyp[i], period[i], balte_andel[i], vall_andel[i]);
        }
    }
    ut
}

/// Räknar täthet för alla rutor (ordning ruta för ruta, sex tal per ruta).
pub fn rakna_vaxt_tathet(
    vattentyp: &[Vatten],
    period: &[Period],
    balte_andel: &[f32],
    vall_andel: &[f32],
) -> Vec<[f32; VAXT_N]> {
    let n = vattentyp.len();
    debug_assert_eq!(period.len(), n);
    debug_assert_eq!(balte_andel.len(), n);
    debug_assert_eq!(vall_andel.len(), n);
    let mut ut = Vec::with_capacity(n);
    for i in 0..n {
        ut.push(vaxt_tathet_ruta(
            vattentyp[i],
            period[i],
            balte_andel[i],
            vall_andel[i],
        ));
    }
    ut
}

/// JSON för `vaxtgrupper.json` (namn, höjd, märke, antal arter).
pub fn bygg_vaxtgrupper_json(arter_per_grupp: &[usize; VAXT_N]) -> String {
    let mut s = String::from("{\n  \"grupper\": [\n");
    for i in 0..VAXT_N {
        if i > 0 {
            s.push_str(",\n");
        }
        s.push_str(&format!(
            "    {{\"namn\": \"{}\", \"hojd_m\": {}, \"marke\": \"simulering\", \"arter\": {}}}",
            VAXT_NAMN[i],
            fmt_f32(VAXT_HOJD_M[i]),
            arter_per_grupp[i]
        ));
    }
    s.push_str("\n  ]\n}\n");
    s
}

fn fmt_f32(x: f32) -> String {
    if x == 0.0 {
        "0.0".into()
    } else {
        let s = format!("{x}");
        if s.contains('.') {
            s
        } else {
            format!("{s}.0")
        }
    }
}

/// Parsar `vaxtgrupper.json` till antal arter och höjder (för kontrollsumma / läsning).
pub fn parsa_vaxtgrupper_json(text: &str) -> Result<([usize; VAXT_N], [f32; VAXT_N]), String> {
    let mut arter = [0usize; VAXT_N];
    let mut hojder = [0.0f32; VAXT_N];
    let mut sett = [false; VAXT_N];
    for (i, namn) in VAXT_NAMN.iter().enumerate() {
        let nyckel = format!("\"namn\": \"{namn}\"");
        let pos = text
            .find(&nyckel)
            .ok_or_else(|| format!("vaxtgrupper.json saknar grupp {namn}"))?;
        let block = &text[pos..];
        let hojd = json_tal_efter(block, "\"hojd_m\"")?;
        let n_arter = json_tal_efter(block, "\"arter\"")? as usize;
        arter[i] = n_arter;
        hojder[i] = hojd as f32;
        sett[i] = true;
    }
    if sett.iter().any(|&s| !s) {
        return Err("vaxtgrupper.json saknar någon grupp".into());
    }
    Ok((arter, hojder))
}

fn json_tal_efter(text: &str, nyckel: &str) -> Result<f64, String> {
    let i = text
        .find(nyckel)
        .ok_or_else(|| format!("saknar {nyckel}"))?;
    let rest = text[i + nyckel.len()..].trim_start();
    if !rest.starts_with(':') {
        return Err(format!("saknar ':' efter {nyckel}"));
    }
    let rest = rest[1..].trim_start();
    let slut = rest
        .find(|c: char| c == ',' || c == '}' || c == '\n' || c.is_whitespace())
        .unwrap_or(rest.len());
    rest[..slut]
        .parse::<f64>()
        .map_err(|_| format!("kunde inte läsa tal efter {nyckel}"))
}
