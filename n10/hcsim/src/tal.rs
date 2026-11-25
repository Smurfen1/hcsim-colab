//! Konstanter för hcsim. Värden och märken speglas i `tal.tsv` (Alt 1197).
//! Densitetstal per plätt räknas om med `PLATT_KM2` till per km², sedan × rutans landyta.

/// Alla namngivna konstanter i koden (prov `tal_tsv_tacker_konstanterna` kräver en rad per namn).
pub const KONSTANT_NAMN: &[&str] = &[
    "VMAX",
    "R_V",
    "SOL",
    "FRO",
    "AT_DINO",
    "E_VAXT",
    "E_START",
    "E_DELA",
    "E_HANE",
    "LIV",
    "LITEN",
    "STEG_PER_AR",
    "KOST_STEG",
    "HJ_ANDEL",
    "KOST_TRASK",
    "KOST_FLOD",
    "KOST_SJO",
    "PLATT_KM2",
    "BREDD",
    "LUTNING",
    "T_MEDEL",
    "T_AMPL",
    "T_FORSKJ_DYGN",
    "VAT_VINTER",
    "W_TORR",
    "PLATT",
    "DYGN",
    "PUNKTER",
    "Z_N",
    "ETA",
    "SIGMA0",
    "SIGMA_MUT",
    "V0_ANDEL",
    "A_N",
    "S_N",
    "MARKT_RABATT",
    "MARKT_INKOR_AR",
    "MARKT_EFTER_AR",
    "TABELL_VIKT",
    "TABELL_SLUMP_P",
    "SPEGEL_TOL",
    "LASARE_ETA",
];

// --- växter / energi (låset v3.0 punkt 3, flockdal.rs grund()) ---
/// Växttak per plätt i flockdal; omräknas till per km² via PLATT_KM2.
pub const VMAX: f64 = 100.0;
pub const R_V: f64 = 0.04;
pub const SOL: f64 = 0.02;
/// Utan multituberkulater är M = 0, så fr = 1 − FRO. Behålls som i flockdal (last).
pub const FRO: f64 = 0.9;
pub const AT_DINO: f64 = 0.4;
pub const E_VAXT: f64 = 8.0;
pub const E_START: f64 = 120.0;
pub const E_DELA: f64 = 240.0;
/// Tak efter födsel (förprov 6 / dal8 med E_HANE = 960).
pub const E_HANE: f64 = 960.0;
pub const LIV: i64 = 3000;
pub const LITEN: i64 = 600;
/// Ett år = 150 steg (LIV / LIVSAR med LIVSAR = 20).
pub const STEG_PER_AR: i64 = 150;
/// Stegkostnad i bete5: `e += intag − 1`.
pub const KOST_STEG: f64 = 1.0;
/// Räknestegens pris: `pris = HJ_ANDEL · KOST_STEG / OPS` (didel.rs).
pub const HJ_ANDEL: f64 = 0.01;

// --- rörelsekostnader (egna val, märkta simulering) ---
pub const KOST_TRASK: f64 = 0.5;
pub const KOST_FLOD: f64 = 1.0;
pub const KOST_SJO: f64 = 1.0;

/// Plättens yta i km² (saknar läst källa; omräkningsfaktor flockdal → km²).
pub const PLATT_KM2: f64 = 0.25;

// --- klimat (låset punkt 14 / klimat.rs / dal8-standard) ---
pub const BREDD: f64 = 50.0;
pub const LUTNING: f64 = 23.13;
pub const T_MEDEL: f64 = 12.5;
pub const T_AMPL: f64 = 7.0;
pub const T_FORSKJ_DYGN: f64 = 30.0;
pub const VAT_VINTER: f64 = 1.0;
pub const W_TORR: f64 = 0.5;
pub const PLATT: f64 = 0.0;
pub const DYGN: f64 = 365.25;
pub const PUNKTER: usize = 36_525;

// --- hjärna ---
pub const Z_N: usize = 4;
pub const ETA: f64 = 0.05;
pub const SIGMA0: f64 = 0.5;
pub const SIGMA_MUT: f64 = 0.05;
/// Startväxt som andel av rutans tak (V0/VMAX = 0,6 i flockdal).
pub const V0_ANDEL: f64 = 0.6;
pub const A_N: usize = 9;
pub const S_N: usize = 19;

// --- populationssvep Alt 1198 (egna val, simulering) ---
/// Märkta betalar denna andel av grundkostnaden i positivarmen.
pub const MARKT_RABATT: f64 = 0.9;
/// Inkörning utan märke innan hälften märks.
pub const MARKT_INKOR_AR: i64 = 30;
/// År efter märkningen i svepet.
pub const MARKT_EFTER_AR: i64 = 150;

// --- hjärnprovet Alt 1200 (egna val, simulering) ---
/// Glidande medel-vikt i minnestabellen.
pub const TABELL_VIKT: f64 = 0.1;
/// Sannolikhet för slumpvis handling i tabellen.
pub const TABELL_SLUMP_P: f64 = 0.1;
/// Spegelns tolerans: medel av (andel + spegelandel) inom 1 ± SPEGEL_TOL.
pub const SPEGEL_TOL: f64 = 0.116;
/// Steglängd för intagsläsarens vikter (Alt 1213; samma som ETA, eget val).
pub const LASARE_ETA: f64 = 0.05;

/// VMAX per km² = VMAX_plätt / PLATT_KM2.
pub fn vmax_per_km2() -> f64 {
    VMAX / PLATT_KM2
}

/// Rutans landyta i km².
pub fn landyta_km2(ruta_m: f64, landandel: f64) -> f64 {
    let ruta_km2 = (ruta_m / 1000.0) * (ruta_m / 1000.0);
    ruta_km2 * landandel
}

/// Växttak i en ruta: VMAX/PLATT_KM2 · landyta · vaxt (0..1).
pub fn vmax_ruta(ruta_m: f64, landandel: f64, vaxt: f64) -> f64 {
    vmax_per_km2() * landyta_km2(ruta_m, landandel) * vaxt
}

/// Läser `tal.tsv` och returnerar (namn → värde).
pub fn las_tsv(text: &str) -> Result<Vec<TalRad>, String> {
    let mut ut = Vec::new();
    for (nr, rad) in text.lines().enumerate() {
        let rad = rad.trim_end();
        if rad.is_empty() || rad.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = rad.split('\t').collect();
        if f.len() < 5 {
            return Err(format!("rad {}: väntade minst 5 fält", nr + 1));
        }
        let varde: f64 = f[1]
            .parse()
            .map_err(|_| format!("rad {}: värde {:?}", nr + 1, f[1]))?;
        let marke = f[3].to_string();
        if marke != "last" && marke != "simulering" {
            return Err(format!("rad {}: okänt märke {:?}", nr + 1, marke));
        }
        ut.push(TalRad {
            namn: f[0].to_string(),
            varde,
            enhet: f[2].to_string(),
            marke,
            kalla: f[4].to_string(),
            anm: f.get(5).unwrap_or(&"").to_string(),
        });
    }
    Ok(ut)
}

#[derive(Clone, Debug)]
pub struct TalRad {
    pub namn: String,
    pub varde: f64,
    pub enhet: String,
    pub marke: String,
    pub kalla: String,
    pub anm: String,
}

/// Värde i koden för ett konstantnamn (för proven).
pub fn varde_i_kod(namn: &str) -> Option<f64> {
    Some(match namn {
        "VMAX" => VMAX,
        "R_V" => R_V,
        "SOL" => SOL,
        "FRO" => FRO,
        "AT_DINO" => AT_DINO,
        "E_VAXT" => E_VAXT,
        "E_START" => E_START,
        "E_DELA" => E_DELA,
        "E_HANE" => E_HANE,
        "LIV" => LIV as f64,
        "LITEN" => LITEN as f64,
        "STEG_PER_AR" => STEG_PER_AR as f64,
        "KOST_STEG" => KOST_STEG,
        "HJ_ANDEL" => HJ_ANDEL,
        "KOST_TRASK" => KOST_TRASK,
        "KOST_FLOD" => KOST_FLOD,
        "KOST_SJO" => KOST_SJO,
        "PLATT_KM2" => PLATT_KM2,
        "BREDD" => BREDD,
        "LUTNING" => LUTNING,
        "T_MEDEL" => T_MEDEL,
        "T_AMPL" => T_AMPL,
        "T_FORSKJ_DYGN" => T_FORSKJ_DYGN,
        "VAT_VINTER" => VAT_VINTER,
        "W_TORR" => W_TORR,
        "PLATT" => PLATT,
        "DYGN" => DYGN,
        "PUNKTER" => PUNKTER as f64,
        "Z_N" => Z_N as f64,
        "ETA" => ETA,
        "SIGMA0" => SIGMA0,
        "SIGMA_MUT" => SIGMA_MUT,
        "V0_ANDEL" => V0_ANDEL,
        "A_N" => A_N as f64,
        "S_N" => S_N as f64,
        "MARKT_RABATT" => MARKT_RABATT,
        "MARKT_INKOR_AR" => MARKT_INKOR_AR as f64,
        "MARKT_EFTER_AR" => MARKT_EFTER_AR as f64,
        "TABELL_VIKT" => TABELL_VIKT,
        "TABELL_SLUMP_P" => TABELL_SLUMP_P,
        "SPEGEL_TOL" => SPEGEL_TOL,
        "LASARE_ETA" => LASARE_ETA,
        _ => return None,
    })
}
