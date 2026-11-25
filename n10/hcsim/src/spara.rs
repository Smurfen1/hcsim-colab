//! Sparat och laddat läge (Alt 1302): serde på hela `Sim`, versionsnummer, kontrollsumma, säker skrivning.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::sim::{ArRad, Sim};
use crate::tal;

/// Versionsnummer för sparfilens format. Laddning vägrar annat nummer.
pub const SPAR_VERSION: u32 = 1;

const MAGIC: &[u8; 4] = b"HCSM";

/// Körparametrar som sparfilen bär (för att bygga om kartan och kräva samma summa).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct KorParametrar {
    pub lat0: f64,
    pub lon0: f64,
    pub b: usize,
    pub h: usize,
    pub ruta_m: f64,
    pub t: f64,
    pub fro: u64,
    pub n0: usize,
    pub karta_summa: u64,
}

#[derive(Serialize, Deserialize)]
struct SparInnehall {
    parametrar: KorParametrar,
    sim: Sim,
}

/// FNV-1a över bytes (samma stil som djur_summa).
pub fn kontrollsumma_bytes(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut h = FNV_OFFSET;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn bygg_payload(parametrar: &KorParametrar, sim: &Sim) -> Result<Vec<u8>, String> {
    let innehall = SparInnehall {
        parametrar: parametrar.clone(),
        sim: sim.clone(),
    };
    // Binärt format med f64::to_bits (JSON tappar ULP och bryter bit-exakt rundtur).
    crate::bitbin::till_bytes(&innehall).map_err(|e| format!("serialisering: {e}"))
}

fn skriv_kuvert(payload: &[u8]) -> Vec<u8> {
    let summa = kontrollsumma_bytes(payload);
    let mut ut = Vec::with_capacity(4 + 4 + 8 + 8 + payload.len());
    ut.extend_from_slice(MAGIC);
    ut.extend_from_slice(&SPAR_VERSION.to_le_bytes());
    ut.extend_from_slice(&summa.to_le_bytes());
    ut.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    ut.extend_from_slice(payload);
    ut
}

/// Säker skrivning: tillfällig fil, fsync, behåll `FIL.forra`, byt namn.
pub fn spara(path: &Path, parametrar: &KorParametrar, sim: &Sim) -> Result<(), String> {
    let payload = bygg_payload(parametrar, sim)?;
    let bytes = skriv_kuvert(&payload);
    skriv_sakert(path, &bytes)
}

/// Som `spara`, men nollställer `g` före serialisering (bara för prov 4).
#[allow(non_snake_case)]
pub fn spara_utan_g_PROV(path: &Path, parametrar: &KorParametrar, sim: &Sim) -> Result<(), String> {
    let mut kopia = sim.clone();
    kopia.g = Rng::new(0);
    spara(path, parametrar, &kopia)
}

fn skriv_sakert(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = tmp_path(path);
    {
        let mut f = File::create(&tmp).map_err(|e| format!("skapa {:?}: {e}", tmp))?;
        f.write_all(bytes)
            .map_err(|e| format!("skriv {:?}: {e}", tmp))?;
        f.sync_all()
            .map_err(|e| format!("fsync {:?}: {e}", tmp))?;
    }
    if path.exists() {
        let forra = forra_path(path);
        fs::rename(path, &forra).map_err(|e| format!("behåll förra {:?}: {e}", forra))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("byt namn till {:?}: {e}", path))?;
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

fn forra_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".forra");
    PathBuf::from(s)
}

/// Läs sparfil: vägrar fel magic, version eller kontrollsumma (aldrig tyst grundvärde).
pub fn ladda(path: &Path) -> Result<(KorParametrar, Sim), String> {
    let mut f = File::open(path).map_err(|e| format!("öppna {:?}: {e}", path))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| format!("läs {:?}: {e}", path))?;
    ladda_bytes(&buf)
}

pub fn ladda_bytes(buf: &[u8]) -> Result<(KorParametrar, Sim), String> {
    if buf.len() < 24 {
        return Err("sparfil för kort".into());
    }
    if &buf[0..4] != MAGIC {
        return Err(format!(
            "fel magiskt tal i sparfil (förväntade HCSM, fick {:?})",
            &buf[0..4]
        ));
    }
    let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    if version != SPAR_VERSION {
        return Err(format!(
            "fel versionsnummer i sparfil: {version} (denna kod kräver {SPAR_VERSION})"
        ));
    }
    let sparad_summa = u64::from_le_bytes(buf[8..16].try_into().unwrap());
    let len = u64::from_le_bytes(buf[16..24].try_into().unwrap()) as usize;
    if buf.len() != 24 + len {
        return Err(format!(
            "sparfil fel längd: kuvert säger {len} bytes, filen har {}",
            buf.len().saturating_sub(24)
        ));
    }
    let payload = &buf[24..];
    let beraknad = kontrollsumma_bytes(payload);
    if beraknad != sparad_summa {
        return Err(format!(
            "fel kontrollsumma i sparfil: sparad {sparad_summa}, beräknad {beraknad}"
        ));
    }
    let innehall: SparInnehall =
        crate::bitbin::fran_bytes(payload).map_err(|e| format!("deserialisering: {e}"))?;
    Ok((innehall.parametrar, innehall.sim))
}

/// Bygg ArRad som `Sim::ar_rad` (privat där; samma fält).
fn ar_rad(sim: &Sim, ar: i64) -> ArRad {
    ArRad {
        ar,
        antal: sim.antal(),
        fodda: sim.rak.fodda_ar,
        dod_svalt: sim.rak.dod_svalt_ar,
        dod_alder: sim.rak.dod_alder_ar,
        v_medel: sim.v_medel(),
    }
}

/// Kör till absolut slutår med samma rapportschema som `Sim::kor_ar`.
/// Antar att `sim.t` redan kan vara mitt i en körning (fortsättning).
pub fn kor_till_ar(sim: &mut Sim, slut_ar: i64, var_ar: i64) -> Vec<ArRad> {
    let mut rader = Vec::new();
    let steg_tot = slut_ar * tal::STEG_PER_AR;
    let start_ar = sim.t / tal::STEG_PER_AR;
    let mut nasta_rapport = if start_ar >= slut_ar {
        slut_ar
    } else {
        let mut n = var_ar;
        while n <= start_ar {
            n += var_ar;
        }
        n
    };
    while sim.t < steg_tot {
        let ar_fore = sim.t / tal::STEG_PER_AR;
        sim.steg();
        let ar_nu = sim.t / tal::STEG_PER_AR;
        if ar_nu > ar_fore {
            if ar_nu == nasta_rapport || ar_nu == slut_ar {
                rader.push(ar_rad(sim, ar_nu));
                sim.rak.fodda_ar = 0;
                sim.rak.dod_svalt_ar = 0;
                sim.rak.dod_alder_ar = 0;
                if ar_nu == nasta_rapport {
                    nasta_rapport += var_ar;
                }
            }
        }
    }
    if rader.last().map(|r| r.ar) != Some(slut_ar) {
        rader.push(ar_rad(sim, slut_ar));
    }
    rader
}

/// Kör `fler_ar` år till från nuvarande läge.
pub fn kor_fler_ar(sim: &mut Sim, fler_ar: i64, var_ar: i64) -> Vec<ArRad> {
    let start_ar = sim.t / tal::STEG_PER_AR;
    kor_till_ar(sim, start_ar + fler_ar, var_ar)
}

/// Kör från år 0 till `ar` och sparar var `spara_var_ar`:e år (och vid slutet) om sökväg anges.
pub fn kor_med_spara(
    sim: &mut Sim,
    ar: i64,
    var_ar: i64,
    spara_fil: Option<&Path>,
    spara_var_ar: i64,
    parametrar: &KorParametrar,
) -> Result<Vec<ArRad>, String> {
    let mut rader = Vec::new();
    let steg_tot = ar * tal::STEG_PER_AR;
    let mut nasta_rapport = var_ar;
    let mut nasta_spara = if spara_fil.is_some() && spara_var_ar > 0 {
        spara_var_ar
    } else {
        i64::MAX
    };
    while sim.t < steg_tot {
        let ar_fore = sim.t / tal::STEG_PER_AR;
        sim.steg();
        let ar_nu = sim.t / tal::STEG_PER_AR;
        if ar_nu > ar_fore {
            if ar_nu == nasta_rapport || ar_nu == ar {
                rader.push(ar_rad(sim, ar_nu));
                sim.rak.fodda_ar = 0;
                sim.rak.dod_svalt_ar = 0;
                sim.rak.dod_alder_ar = 0;
                if ar_nu == nasta_rapport {
                    nasta_rapport += var_ar;
                }
            }
            if let Some(path) = spara_fil {
                if ar_nu == nasta_spara || ar_nu == ar {
                    spara(path, parametrar, sim)?;
                    if ar_nu == nasta_spara {
                        nasta_spara += spara_var_ar;
                    }
                }
            }
        }
    }
    if rader.last().map(|r| r.ar) != Some(ar) {
        rader.push(ar_rad(sim, ar));
    }
    if let Some(path) = spara_fil {
        // Säkerställ slutläge även om sista året redan sparats (idempotent innehåll).
        spara(path, parametrar, sim)?;
    }
    Ok(rader)
}
