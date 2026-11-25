//! litet program: skriver kartans storlek, högsta och lägsta höjd, andelen hav och kontrollsumman.
//! Med `--tidtabell` sätts periodlagret ur tidtabellen och perioden skrivs ut.
//! Med `--skriv <mapp>` skrivs filerna till 3D-bilden (JSON + binära rutnät + PNG + strand).
//! Med `--fro <tal>` läggs flodnätet (kräver `--tidtabell`); antalet rutor per vattentyp skrivs ut.
//! Med `--flodversion 2` (standard 2 med `--fro`, Alt 1566) skrivs fårans och bältets andelar i stället för hela Flodfara-rutor.
//! Med `--tidsmodell 2` (standard 2 med `--fro`, Alt 1566) används `karta_for_ar` (fåror som objekt); 1 = `karta_full_version`.
//! Med `--flodriktning 2` (standard 2 med `--fro`, Alt 1566) går fårorna mot sydost (öst/sydost/söder); 1 = oförändrat österut.
//! Med `--lutning 2` (standard 2 med `--fro`, Alt 1566) ersätts fönstrets plan med 0,30 m/km mot 135° och fallande vattenyta.
//!
//! `hckarta --lat0 .. --lon0 .. --b .. --h .. --ruta-m .. --t .. [--tidtabell fil] [--fro tal] [--flodversion 1|2] [--tidsmodell 1|2] [--flodriktning 1|2] [--lutning 1|2] [--skriv mapp]`

use std::path::PathBuf;

use hckarta::{
    karta, karta_for_ar_lutning, karta_full_version_lutning, kontrollsumma_med, levande_foror_nr,
    skriv_3d_med, Kalla, Omrade, Period, Tidtabell, Vatten, VAXT_NAMN, VAXT_N, VAXT_ORT,
};

fn varde(a: &[String], namn: &str) -> Result<Option<String>, String> {
    match a.iter().position(|x| x == namn) {
        None => Ok(None),
        Some(i) => a
            .get(i + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{namn} ska följas av ett värde")),
    }
}

fn tal(a: &[String], namn: &str) -> Result<Option<f64>, String> {
    varde(a, namn)?
        .map(|s| s.parse::<f64>().map_err(|_| format!("{namn}: {s} är inget tal")))
        .transpose()
}

fn heltal(a: &[String], namn: &str) -> Result<Option<usize>, String> {
    varde(a, namn)?
        .map(|s| s.parse::<usize>().map_err(|_| format!("{namn}: {s} är inget heltal")))
        .transpose()
}

fn fro_tal(a: &[String]) -> Result<Option<u64>, String> {
    varde(a, "--fro")?
        .map(|s| s.parse::<u64>().map_err(|_| format!("--fro: {s} är inget heltal")))
        .transpose()
}

fn period_namn(p: Period) -> &'static str {
    match p {
        Period::Aldre => "Aldre",
        Period::Ovre => "Ovre",
        Period::Gransen => "Gransen",
    }
}

fn vatten_namn(v: Vatten) -> &'static str {
    match v {
        Vatten::Land => "Land",
        Vatten::Flodfara => "Flodfara",
        Vatten::Trask => "Trask",
        Vatten::Sjo => "Sjo",
        Vatten::Brackt => "Brackt",
        Vatten::Hav => "Hav",
    }
}

fn kor(a: &[String]) -> Result<(), String> {
    let lat0 = tal(a, "--lat0")?.ok_or("--lat0 saknas")?;
    let lon0 = tal(a, "--lon0")?.ok_or("--lon0 saknas")?;
    let b = heltal(a, "--b")?.ok_or("--b saknas")?;
    let h = heltal(a, "--h")?.ok_or("--h saknas")?;
    let ruta_m = tal(a, "--ruta-m")?.ok_or("--ruta-m saknas")?;
    let t = tal(a, "--t")?.ok_or("--t saknas")?;
    let tidtabell_fil = varde(a, "--tidtabell")?;
    let skriv_mapp = varde(a, "--skriv")?;
    let fro = fro_tal(a)?;
    // Standardbytet (Håkans Alt 1566; agenternas Alt 1665, 1694; grinden på fröna 81–100 grön, piharness docs/evolution/kartstd5-prova-81-100.txt):
    // med --fro är standarden 2 för alla fyra valen; utan --fro (ingen flod) är den 1. `karta_full` (hcsim) ändras inte.
    let standard: usize = if fro.is_some() { 2 } else { 1 };
    let flodversion = heltal(a, "--flodversion")?.unwrap_or(standard) as u8;
    if flodversion != 1 && flodversion != 2 {
        return Err(format!("--flodversion: {flodversion} (väntade 1 eller 2)"));
    }
    let tidsmodell = heltal(a, "--tidsmodell")?.unwrap_or(standard) as u8;
    if tidsmodell != 1 && tidsmodell != 2 {
        return Err(format!("--tidsmodell: {tidsmodell} (väntade 1 eller 2)"));
    }
    let flodriktning = heltal(a, "--flodriktning")?.unwrap_or(standard) as u8;
    if flodriktning != 1 && flodriktning != 2 {
        return Err(format!("--flodriktning: {flodriktning} (väntade 1 eller 2)"));
    }
    let lutning = heltal(a, "--lutning")?.unwrap_or(standard) as u8;
    if lutning != 1 && lutning != 2 {
        return Err(format!("--lutning: {lutning} (väntade 1 eller 2)"));
    }

    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data");
    let kalla = Kalla::las(&data)?;
    let omr = Omrade {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
    };
    let (k, andelar) = if let Some(fro) = fro {
        let path = tidtabell_fil
            .as_ref()
            .ok_or("--fro kräver --tidtabell")?;
        let text = std::fs::read_to_string(path).map_err(|e| format!("tidtabell {path}: {e}"))?;
        let tt = Tidtabell::las(&text)?;
        if tidsmodell == 2 {
            karta_for_ar_lutning(&kalla, &tt, &omr, t, fro, flodversion, flodriktning, lutning)?
        } else {
            karta_full_version_lutning(
                &kalla, &tt, &omr, t, fro, flodversion, flodriktning, lutning,
            )?
        }
    } else if let Some(path) = &tidtabell_fil {
        if flodversion != 1 {
            return Err("--flodversion kräver --fro".into());
        }
        if tidsmodell != 1 {
            return Err("--tidsmodell kräver --fro".into());
        }
        if lutning != 1 {
            return Err("--lutning kräver --fro".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("tidtabell {path}: {e}"))?;
        let tt = Tidtabell::las(&text)?;
        (hckarta::karta_med(&kalla, &tt, &omr, t)?, None)
    } else {
        if flodversion != 1 {
            return Err("--flodversion kräver --fro".into());
        }
        if tidsmodell != 1 {
            return Err("--tidsmodell kräver --fro".into());
        }
        if lutning != 1 {
            return Err("--lutning kräver --fro".into());
        }
        (karta(&kalla, &omr, t)?, None)
    };

    let n = b * h;
    let (hogst, lagst) = k
        .hojd
        .varden
        .iter()
        .fold((f32::NEG_INFINITY, f32::INFINITY), |(hi, lo), &v| (hi.max(v), lo.min(v)));
    let hav = k.vattentyp.varden.iter().filter(|&&v| v == Vatten::Hav).count();
    let andel_hav = if n == 0 { 0.0 } else { hav as f64 / n as f64 };
    let summa = kontrollsumma_med(&k, andelar.as_ref());

    println!("storlek: {b} × {h} = {n} rutor");
    println!("höjd: lägst {lagst:.1} m, högst {hogst:.1} m");
    println!("andel hav: {andel_hav:.4} ({hav}/{n})");
    if tidtabell_fil.is_some() {
        let p = k.period.varden[0];
        println!("period: {} ({:?})", period_namn(p), k.period.marke);
    }
    if fro.is_some() {
        println!("flodversion: {flodversion}");
        println!("tidsmodell: {tidsmodell}");
        println!("flodriktning: {flodriktning}");
        println!("lutning: {lutning}");
        if tidsmodell == 2 {
            let p = k.period.varden[0];
            let lev = levande_foror_nr(fro.unwrap(), p);
            print!("levande fåror:");
            for n in &lev {
                print!(" {n}");
            }
            println!();
        }
        for typ in [
            Vatten::Land,
            Vatten::Flodfara,
            Vatten::Trask,
            Vatten::Sjo,
            Vatten::Brackt,
            Vatten::Hav,
        ] {
            let antal = k.vattentyp.varden.iter().filter(|&&v| v == typ).count();
            println!("vattentyp {}: {antal}", vatten_namn(typ));
        }
        if let Some(a) = &andelar {
            let n_f = n.max(1) as f64;
            let sum_f: f64 = a.fara_andel.varden.iter().map(|&x| f64::from(x)).sum();
            let sum_b: f64 = a.balte_andel.varden.iter().map(|&x| f64::from(x)).sum();
            println!("medel fara_andel: {:.6}", sum_f / n_f);
            println!("medel balte_andel: {:.6}", sum_b / n_f);
            if a.har_vall {
                let sum_v: f64 = a.vall_andel.varden.iter().map(|&x| f64::from(x)).sum();
                let n_vall = a.vall_andel.varden.iter().filter(|&&x| x > 0.0).count();
                println!("medel vall_andel: {:.6}", sum_v / n_f);
                println!("rutor med vall: {n_vall}");
            }
            if a.har_vaxtgrupper {
                for g in 0..VAXT_N {
                    let sum_g: f64 = a
                        .vaxt_tathet
                        .varden
                        .iter()
                        .map(|rad| f64::from(rad[g]))
                        .sum();
                    println!(
                        "medel vaxt_tathet {}: {:.6} (arter={})",
                        VAXT_NAMN[g],
                        sum_g / n_f,
                        a.vaxt_arter[g]
                    );
                }
                let n_ort30 = a
                    .vaxt_tathet
                    .varden
                    .iter()
                    .filter(|rad| (rad[VAXT_ORT] - 0.30).abs() < 1e-6)
                    .count();
                println!("Land-rutor med örttäthet 0,30: {n_ort30}");
            }
            let nummer: Vec<usize> = if tidsmodell == 2 {
                levande_foror_nr(fro.unwrap(), k.period.varden[0])
            } else {
                (0..a.foror.len()).collect()
            };
            for (i, f) in a.foror.iter().enumerate() {
                let nr = nummer.get(i).copied().unwrap_or(i);
                if a.har_vall {
                    println!(
                        "fåra {nr}: bredd_m={:.6} balte_m={:.6} vall_m={:.6} rutor={}",
                        f.bredd_m, f.balte_m, f.vall_m, f.rutor
                    );
                } else {
                    println!(
                        "fåra {nr}: bredd_m={:.6} balte_m={:.6} rutor={}",
                        f.bredd_m, f.balte_m, f.rutor
                    );
                }
            }
        }
    }
    if let Some(mapp) = &skriv_mapp {
        let skriven = skriv_3d_med(&k, andelar.as_ref(), PathBuf::from(mapp).as_path())?;
        println!("skrev 3D-filer till {mapp} (kontrollsumma {skriven})");
    }
    println!("kontrollsumma: {summa:#018x} ({summa})");
    Ok(())
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if let Err(e) = kor(&a) {
        eprintln!("hckarta: {e}");
        std::process::exit(2);
    }
}
