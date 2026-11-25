//! Kartbiblioteket för Hell Creek: `karta(område, t)` ger ett rutnät vid tiden t (år före K-Pg).
//! I S2 styr tidtabellens lästa rader periodlagret via `karta_med`; utan tidtabell är perioden simulering.
//! I S3 skrivs och läses filerna till 3D-bilden (`filer::skriv_3d` / `las_3d`).
//! I S4 läggs flodnätet (`flod::lagg_flodnat` / `karta_full`) ur ett frö.

use std::path::Path;

use paleokarta::{Dem, Rotationer};

mod filer;
pub use filer::{las_3d, las_3d_med, skriv_3d, skriv_3d_med, strandlinje};

mod flod;
pub use flod::{
    andel_nytt_lopp_pa_tidigare, applicera_lutning_plan, fara_bredd_m, flodregler,
    fora_riktning_grader, karta_for_ar, karta_for_ar_fyllnad, karta_for_ar_fyllnad_lutning,
    karta_for_ar_fyllnad_lutning_lopp_prov, karta_for_ar_fyllnad_lutning_prov, karta_for_ar_lutning,
    karta_for_ar_lutning_lopp_prov, karta_for_ar_lutning_prov, karta_full, karta_full_version,
    karta_full_version_lutning, lagg_flodnat, lagg_flodnat_version, lagg_flodnat_version_lutning,
    lagg_flodnat_version_lutning_prov, levande_foror_nr, loppbyten, mat_linjer_tm2,
    mat_linjer_tm2_prov, mat_plan, mat_riktning_statistik, mat_riktning_statistik_straff,
    max_stigning_pa_linje, mittlinjer,
    mittlinjer_aldre, mittlinjer_aldre_riktning, mittlinjer_riktning, mittlinjer_riktning_straff,
    mittlinjer_vid_ar, nasta_andring_med_lopp, prova_nytt_lopp, vall_bredd_m, valj_riktning_straff,
    FlodAndelar, Flodregler, ForaInfo, LoppProvMutant, Loppbyte, TraskProvMutant, BALTE_FAKTOR,
    BALTE_MAX, FARA_BREDD_MAX_M, FARA_BREDD_MIN_M, FLODRIKTNING_K_KANDIDATER, FLODRIKTNING_MAL_MAX,
    FLODRIKTNING_MAL_MIN, FLODRIKTNING_STRAFF_M, FYLLNADSTID_AR, LOPP_BONUS_M, LOPP_MAX_AR,
    LOPP_MAX_STIGNING_M, LOPP_MIN_AR, LOPP_STRAFF_M, LUTNING2_M_PER_KM, LUTNING2_RIKTNING_GRADER,
    VALL_FAKTOR, VALL_HOJD_M, VALL_MAX_M,
};

mod vaxt;
pub use vaxt::{
    bygg_vaxtgrupper_json, las_vaxtgrupper_inbyggd, las_vaxtgrupper_tsv, parsa_vaxtgrupper_json,
    rakna_vaxt_tathet, rakna_vaxt_tathet_med, vaxt_tathet_ruta, vaxt_tathet_ruta_standard, TRASK_TATHET_ALT1694, VaxtArtRad, VaxtgrupperLasning, VAXT_HOJD_M, VAXT_N,
    VAXT_NAMN, VAXT_ORT,
};

mod vattenkvalitet;
pub use vattenkvalitet::{
    flodesklass_av, kontrollsumma_vattenkvalitet, las_3d_med_vattenkvalitet, las_vattenkvalitet,
    las_vattenkvalitet_tsv, rakna_vattenkvalitet, salthalt_klass_av, skriv_3d_med_vattenkvalitet,
    skriv_vattenkvalitet, Flodesklass, SalthaltKlass, VattenKallaRad, VattenKvalitet,
};

mod naturlage;
pub use naturlage::{
    flodzon_av, kontrollsumma_naturlage, las_3d_med_naturlage, las_naturhandelser_tsv,
    las_naturlage, rakna_naturlage, skriv_3d_med_naturlage, skriv_naturlage, FlodZon,
    NaturKallaRad, NaturLage,
};

mod basunderlag;
pub use basunderlag::{
    forna_miljo_av, kontrollsumma_basunderlag, kron_tathet_av, las_3d_med_basunderlag,
    las_basunderlag, las_forna_alger_plankton_tsv, rakna_basunderlag, skriv_3d_med_basunderlag,
    skriv_basunderlag, vattenyta_andel_av, BasKallaRad, BasUnderlag, FornaMiljo,
};

mod sjukdomar;
pub use sjukdomar::{las_sjukdomar_tsv, SjukdomsRad};

/// Märket på ett lager: Last = ur en läst källa, Simulering = programmets val (regel 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marke {
    Last,
    Simulering,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lager<T> {
    pub varden: Vec<T>,
    pub marke: Marke,
}

/// Perioden i Hell Creek-formationen (styrd av tidtabellen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Period {
    Aldre = 0,
    Ovre = 1,
    Gransen = 2,
}

/// Vattentyp per ruta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Vatten {
    Land = 0,
    Flodfara = 1,
    Trask = 2,
    Sjo = 3,
    Brackt = 4,
    Hav = 5,
}

/// Området i dagens koordinater: sydvästra hörnet (lat, lon i grader), antal rutor i öst (b) och nord (h), rutans sida i meter.
#[derive(Clone, Debug)]
pub struct Omrade {
    pub lat0: f64,
    pub lon0: f64,
    pub b: usize,
    pub h: usize,
    pub ruta_m: f64,
}

#[derive(Clone, Debug)]
pub struct Handelse {
    pub ar_fore_gransen: Option<f64>,
    pub handelse: String,
    pub varde: String,
    pub kalla: String,
    pub marke: Marke,
}

#[derive(Clone, Debug)]
pub struct Tidtabell {
    pub rader: Vec<Handelse>,
}

impl Tidtabell {
    /// Läser TSV: rader som börjar med '#' hoppas över; fem fält skilda med tab; tom tid = None;
    /// märket "läst" → Marke::Last, "simulering" → Marke::Simulering, annat → Err.
    /// Fel om antalet fält inte är fem. FEL om ingen rad är läst med en känd tid.
    pub fn las(text: &str) -> Result<Tidtabell, String> {
        let mut rader = Vec::new();
        for (nr, rad) in text.lines().enumerate() {
            let rad = rad.trim_end();
            if rad.is_empty() || rad.starts_with('#') {
                continue;
            }
            let falt: Vec<&str> = rad.split('\t').collect();
            if falt.len() != 5 {
                return Err(format!(
                    "rad {}: väntade 5 fält, fick {}",
                    nr + 1,
                    falt.len()
                ));
            }
            let ar_fore_gransen = if falt[0].is_empty() {
                None
            } else {
                Some(
                    falt[0]
                        .parse::<f64>()
                        .map_err(|_| format!("rad {}: tiden {:?} är inget tal", nr + 1, falt[0]))?,
                )
            };
            let marke = match falt[4] {
                "läst" => Marke::Last,
                "simulering" => Marke::Simulering,
                annat => {
                    return Err(format!(
                        "rad {}: okänt märke {:?} (väntade läst eller simulering)",
                        nr + 1,
                        annat
                    ));
                }
            };
            rader.push(Handelse {
                ar_fore_gransen,
                handelse: falt[1].to_string(),
                varde: falt[2].to_string(),
                kalla: falt[3].to_string(),
                marke,
            });
        }
        let har_last_med_tid = rader
            .iter()
            .any(|r| r.marke == Marke::Last && r.ar_fore_gransen.is_some());
        if !har_last_med_tid {
            return Err(
                "tidtabellen saknar läst rad med känd tid (tom läsning godkänns aldrig)".into(),
            );
        }
        Ok(Tidtabell { rader })
    }

    /// Perioden vid t: Gransen om t ≤ tiden för den lästa raden "gransen";
    /// Ovre om t ≤ tiden för den lästa raden "magnetvandning"; annars Aldre.
    /// Saknas någon av de två lästa raderna (med tid): Err.
    pub fn period(&self, t: f64) -> Result<Period, String> {
        let gransen = self
            .last_tid("gransen")
            .ok_or_else(|| "saknar läst rad \"gransen\" med känd tid".to_string())?;
        let magnet = self
            .last_tid("magnetvandning")
            .ok_or_else(|| "saknar läst rad \"magnetvandning\" med känd tid".to_string())?;
        if t <= gransen {
            Ok(Period::Gransen)
        } else if t <= magnet {
            Ok(Period::Ovre)
        } else {
            Ok(Period::Aldre)
        }
    }

pub fn last_tid(&self, namn: &str) -> Option<f64> {
        self.rader
            .iter()
            .find(|r| r.marke == Marke::Last && r.handelse == namn)
            .and_then(|r| r.ar_fore_gransen)
    }
}

/// Närmaste t' &lt; t (mot gränsen) där perioden byts, ett lopp byts **eller** en fyllning
/// blir klar (Alt 1395/1494). `fro` styr loppbytets tider; utan frö (tidsmodell 1) finns
/// bara periodbyten — skicka valfritt frö (t.ex. 0) om bara periodgränserna ska räknas,
/// eller använd `nasta_andring_period` för enbart period.
pub fn nasta_andring(tt: &Tidtabell, t: f64, fro: u64) -> Option<f64> {
    crate::flod::nasta_andring_med_lopp(tt, t, fro)
}

/// Alt 1828 (agenternas omröstning karttid; Alt 1843): alla tider t (år före gränsen) där kartan kan ändras, från `t_fran` mot gränsen
/// till och med `t_till`, i fallande ordning. Listan följer `nasta_andring` (periodbyten, loppbyten och fyllningar). Mellan två tider i
/// listan är kartan densamma: kartan är densamma inom varje öppet intervall mellan två tider i listan. Vid själva tiden c kan kartan
/// höra till intervallet ovanför eller under (mätt: ett loppbyte vid 976 810 gäller först för t &lt; c, en fyllning vid 975 810 redan vid c).
/// Simuleringen räknar därför om kartan både när t når en tid i listan och när t går under den. Prov: tests/bytesar1828.rs.
pub fn bytesar(tt: &Tidtabell, fro: u64, t_fran: f64, t_till: f64) -> Vec<f64> {
    let mut ut = Vec::new();
    let mut t = t_fran;
    while let Some(n) = nasta_andring(tt, t, fro) {
        if n < t_till {
            break;
        }
        ut.push(n);
        t = n;
    }
    ut
}

/// Närmaste t' &lt; t där perioden byts (Alt 1401); loppbyten räknas inte.
pub fn nasta_andring_period(tt: &Tidtabell, t: f64) -> Option<f64> {
    let mut kandidater = Vec::new();
    for namn in ["magnetvandning", "gransen"] {
        if let Some(tg) = tt.last_tid(namn) {
            if tg < t {
                kandidater.push(tg);
            }
        }
    }
    kandidater
        .into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

#[derive(Clone, Debug)]
pub struct Karta {
    pub omrade: Omrade,
    pub t: f64,
    pub hojd: Lager<f32>,         // m över havsytan; Last (PaleoDEM, 1°, bilinjärt)
    pub vatten_yta: Lager<f32>,   // m; i S1: 0 där höjden < 0, annars lika med höjden; Last
    pub vattentyp: Lager<Vatten>, // i S1: Hav där höjden < 0, annars Land; Last
    pub mark: Lager<[f32; 4]>,    // andelar sand, silt, lera, torv; i S1 [0.25; 4]; Simulering
    pub vaxt: Lager<f32>,         // växttäthet 0..1; i S1 0.5; Simulering
    pub period: Lager<Period>,    // i S2: ur tidtabellen (Last) eller Aldre (Simulering) utan den
}

/// Rotationerna och PaleoDEM, inlästa en gång.
pub struct Kalla {
    rotationer: Rotationer,
    dem: Dem,
}

const M_PER_GRAD_LAT: f64 = 111_320.0;
const PLATTA_NORDAMERIKA: u32 = 101;
const TID_MA: f64 = 66.0;

impl Kalla {
    /// Läser rotationerna (`PALEOMAP_PlateModel.rot`) och PaleoDEM (`Map16_065Ma.csv`) ur `paleokarta_data`.
    pub fn las(paleokarta_data: &Path) -> Result<Kalla, String> {
        let rot_text = std::fs::read_to_string(paleokarta_data.join("PALEOMAP_PlateModel.rot"))
            .map_err(|e| format!("rotationer: {e}"))?;
        let dem_text = std::fs::read_to_string(paleokarta_data.join("Map16_065Ma.csv"))
            .map_err(|e| format!("PaleoDEM: {e}"))?;
        Ok(Kalla {
            rotationer: Rotationer::las(&rot_text)?,
            dem: Dem::las(&dem_text)?,
        })
    }
}

/// Mittpunkten för ruta `(i, j)` i dagens koordinater (plant närmevärde; se LASMIG.md).
fn mittpunkt(omr: &Omrade, i: usize, j: usize) -> (f64, f64) {
    let lat = omr.lat0 + (j as f64 + 0.5) * omr.ruta_m / M_PER_GRAD_LAT;
    let lon = omr.lon0 + (i as f64 + 0.5) * omr.ruta_m / (M_PER_GRAD_LAT * lat.to_radians().cos());
    (lat, lon)
}

/// Rutnät över `omr` vid tiden `t` (år före K-Pg). I S1/S2 används alltid 66 Ma och Map16; `t` lagras.
/// Periodlagret blir `Aldre` med märket Simulering (utan tidtabell).
pub fn karta(k: &Kalla, omr: &Omrade, t: f64) -> Result<Karta, String> {
    if omr.b == 0 || omr.h == 0 {
        return Err("området är tomt: b och h måste vara minst 1".into());
    }
    let n = omr.b * omr.h;
    let mut hojd = Vec::with_capacity(n);
    let mut vatten_yta = Vec::with_capacity(n);
    let mut vattentyp = Vec::with_capacity(n);
    let mut mark = Vec::with_capacity(n);
    let mut vaxt = Vec::with_capacity(n);

    for j in 0..omr.h {
        for i in 0..omr.b {
            let (lat, lon) = mittpunkt(omr, i, j);
            let (pla, plo) = k
                .rotationer
                .flytta(PLATTA_NORDAMERIKA, lat, lon, TID_MA)
                .map_err(|e| format!("ruta ({i},{j}) vid ({lat}, {lon}): {e}"))?;
            let h = k.dem.hojd(pla, plo) as f32;
            hojd.push(h);
            if h < 0.0 {
                vatten_yta.push(0.0);
                vattentyp.push(Vatten::Hav);
            } else {
                vatten_yta.push(h);
                vattentyp.push(Vatten::Land);
            }
            // Alt 2207 / Håkans beslut Alt 2215 (2026-11-24): k.mark hålls låst och orörd som [0.25; 4]
            // så att inga befintliga karta.json-kontrollsummor bryts och inga ogrundade procenttal förs in.
            // Geologiska zoner ges av FlodZon (naturlage.rs); markfysik ägs av UNREAL (Alt 1755/1756, regel 15).
            mark.push([0.25, 0.25, 0.25, 0.25]);
            vaxt.push(0.5);
        }
    }

    Ok(Karta {
        omrade: omr.clone(),
        t,
        hojd: Lager {
            varden: hojd,
            marke: Marke::Last,
        },
        vatten_yta: Lager {
            varden: vatten_yta,
            marke: Marke::Last,
        },
        vattentyp: Lager {
            varden: vattentyp,
            marke: Marke::Last,
        },
        mark: Lager {
            varden: mark,
            marke: Marke::Simulering,
        },
        vaxt: Lager {
            varden: vaxt,
            marke: Marke::Simulering,
        },
        period: Lager {
            varden: vec![Period::Aldre; n],
            marke: Marke::Simulering,
        },
    })
}

/// Som `karta`, men med periodlagret satt ur tidtabellen (alla rutor samma period i S2; märket Last).
pub fn karta_med(k: &Kalla, tt: &Tidtabell, omr: &Omrade, t: f64) -> Result<Karta, String> {
    let p = tt.period(t)?;
    let mut kart = karta(k, omr, t)?;
    let n = omr.b * omr.h;
    kart.period = Lager {
        varden: vec![p; n],
        marke: Marke::Last,
    };
    Ok(kart)
}

const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

fn fnv1a_byte(h: &mut u64, b: u8) {
    *h ^= u64::from(b);
    *h = h.wrapping_mul(FNV_PRIME);
}

fn fnv1a_bytes(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        fnv1a_byte(h, b);
    }
}

fn marke_u8(m: Marke) -> u8 {
    match m {
        Marke::Last => 0,
        Marke::Simulering => 1,
    }
}

/// FNV-1a 64 över alla lagrens värden (little endian) och märken, i fast ordning
/// (höjd, vatten_yta, vattentyp, mark, växt, period). Version 1: samma som före Alt 1343.
pub fn kontrollsumma(k: &Karta) -> u64 {
    kontrollsumma_med(k, None)
}

/// Som `kontrollsumma`, och med `fara_andel` + `balte_andel` (värden och märke) om de finns (flodversion 2).
/// Om `foror` inte är tom (ny version 2 med `foror.tsv`) följer varje fåras `bredd_m` och `balte_m`
/// som f64 LE samt `rutor` som u64 LE. Tom `foror` (Alt 1343 utan fil) ger samma summa som före Alt 1386.
/// Vid `har_vall` (Alt 1371) följer `vall_andel` och `vall_hojd_m` efter bältet, och varje fåras
/// `vall_m` (f64 LE) efter `rutor`. Utan vallfiler (Alt 1343/1386) samma summa som före Alt 1371.
/// Vid `har_vaxtgrupper` (Alt 1433) följer `vaxt_tathet` (sex f32 per ruta) och märket, sedan varje
/// grupps `hojd_m` (f32 LE) och `arter` (u64 LE). Utan växtfilsmappar samma summa som före Alt 1433.
pub fn kontrollsumma_med(k: &Karta, andelar: Option<&FlodAndelar>) -> u64 {
    let mut h = FNV_OFFSET;
    for v in &k.hojd.varden {
        fnv1a_bytes(&mut h, &v.to_le_bytes());
    }
    fnv1a_byte(&mut h, marke_u8(k.hojd.marke));
    for v in &k.vatten_yta.varden {
        fnv1a_bytes(&mut h, &v.to_le_bytes());
    }
    fnv1a_byte(&mut h, marke_u8(k.vatten_yta.marke));
    for v in &k.vattentyp.varden {
        fnv1a_byte(&mut h, *v as u8);
    }
    fnv1a_byte(&mut h, marke_u8(k.vattentyp.marke));
    for v in &k.mark.varden {
        for x in v {
            fnv1a_bytes(&mut h, &x.to_le_bytes());
        }
    }
    fnv1a_byte(&mut h, marke_u8(k.mark.marke));
    for v in &k.vaxt.varden {
        fnv1a_bytes(&mut h, &v.to_le_bytes());
    }
    fnv1a_byte(&mut h, marke_u8(k.vaxt.marke));
    for v in &k.period.varden {
        fnv1a_byte(&mut h, *v as u8);
    }
    fnv1a_byte(&mut h, marke_u8(k.period.marke));
    if let Some(a) = andelar {
        for v in &a.fara_andel.varden {
            fnv1a_bytes(&mut h, &v.to_le_bytes());
        }
        fnv1a_byte(&mut h, marke_u8(a.fara_andel.marke));
        for v in &a.balte_andel.varden {
            fnv1a_bytes(&mut h, &v.to_le_bytes());
        }
        fnv1a_byte(&mut h, marke_u8(a.balte_andel.marke));
        if a.har_vall {
            for v in &a.vall_andel.varden {
                fnv1a_bytes(&mut h, &v.to_le_bytes());
            }
            fnv1a_byte(&mut h, marke_u8(a.vall_andel.marke));
            for v in &a.vall_hojd_m.varden {
                fnv1a_bytes(&mut h, &v.to_le_bytes());
            }
            fnv1a_byte(&mut h, marke_u8(a.vall_hojd_m.marke));
        }
        for f in &a.foror {
            fnv1a_bytes(&mut h, &f.bredd_m.to_le_bytes());
            fnv1a_bytes(&mut h, &f.balte_m.to_le_bytes());
            fnv1a_bytes(&mut h, &(f.rutor as u64).to_le_bytes());
            if a.har_vall {
                fnv1a_bytes(&mut h, &f.vall_m.to_le_bytes());
            }
        }
        if a.har_vaxtgrupper {
            for v in &a.vaxt_tathet.varden {
                for x in v {
                    fnv1a_bytes(&mut h, &x.to_le_bytes());
                }
            }
            fnv1a_byte(&mut h, marke_u8(a.vaxt_tathet.marke));
            for i in 0..VAXT_N {
                fnv1a_bytes(&mut h, &a.vaxt_hojd[i].to_le_bytes());
                fnv1a_bytes(&mut h, &(a.vaxt_arter[i] as u64).to_le_bytes());
            }
        }
    }
    h
}
