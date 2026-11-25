//! CLI: `hcsim kor …`, `hcsim fortsatt …`, `hcsim svep|…|signal [--torr] [--fron A-B] [--bara v_lar]`,
//! `hcsim hjsignal --omrakna --logg …`.

use std::env;
use std::path::PathBuf;

use hckarta::{karta_full, kontrollsumma, Kalla, Omrade, Tidtabell};
use hcsim::avlasare;
use hcsim::diagnos;
use hcsim::hjarnprov;
use hcsim::hjsignal;
use hcsim::kopier;
use hcsim::ledprov;
use hcsim::rang;
use hcsim::signal::{self, SignalVal};
use hcsim::spara::{self, KorParametrar};
use hcsim::svep;
use hcsim::tur;
use hcsim::utforsk;
use hcsim::{sammanfattning_json, Sim};

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "användning:\n  hcsim kor --lat0 … --lon0 … --b … --h … --ruta-m … --t … --fro … --ar … --n0 … [--spara FIL --spara-var-ar N]\n  hcsim fortsatt --fran FIL --ar M [--var-ar N]\n  hcsim svep [--torr]\n  hcsim hjarnprov [--torr]\n  hcsim diagnos [--torr]\n  hcsim avlasare [--torr]\n  hcsim ledprov [--torr]\n  hcsim rang [--torr]\n  hcsim tur [--torr]\n  hcsim signal [--torr] [--fron A-B] [--bara v_lar]\n  hcsim hjsignal --omrakna --logg FIL [--v-signal FIL]\n  hcsim kopier [--torr]\n  hcsim utforsk [--torr] [--omrakna]\n  hcsim kartbild [--arm e_lar] [--spegel] [--fro 261] [--var-steg 30] [--historik-s 10] [--ar N] [--ut FIL]"
        );
        std::process::exit(2);
    }
    let kommando = args.remove(0);
    match kommando.as_str() {
        "kor" => {
            if let Err(e) = kor(&args) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "fortsatt" => {
            if let Err(e) = fortsatt(&args) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "svep" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = svep::kor_svep(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "hjarnprov" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = hjarnprov::kor_hjarnprov(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "diagnos" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = diagnos::kor_diagnos(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "avlasare" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = avlasare::kor_avlasare(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "ledprov" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = ledprov::kor_ledprov(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "rang" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = rang::kor_rang(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "tur" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = tur::kor_tur(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "signal" => {
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            match las_signal_val(&args) {
                Ok(val) => {
                    if let Err(e) = signal::kor_signal_med(&rot, &val) {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            }
        }
        "hjsignal" => {
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = kor_hjsignal(&rot, &args) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "kopier" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if let Err(e) = kopier::kor_kopier(&rot, torr) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "kartbild" => {
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let v = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
            let arm = v("--arm").unwrap_or_else(|| "e_lar".into());
            let spegel = args.iter().any(|a| a == "--spegel");
            let fro: u64 = v("--fro").and_then(|x| x.parse().ok()).unwrap_or(261);
            let var_steg: i64 = v("--var-steg").and_then(|x| x.parse().ok()).unwrap_or(30);
            let historik_s: u64 = v("--historik-s").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut = PathBuf::from(v("--ut").unwrap_or_else(|| "kartbild.json".into()));
            let slut_ar: Option<i64> = v("--ar").map(|x| x.parse().expect("--ar N (heltal)"));
            assert!(slut_ar.map_or(true, |n| n > 0), "--ar ska vara större än 0");
            if let Err(e) = hcsim::kartbild::kor_kartbild(&rot, &arm, spegel, fro, var_steg, historik_s, &ut, slut_ar) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        "utforsk" => {
            let torr = args.iter().any(|a| a == "--torr");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            if args.iter().any(|a| a == "--rattad") {
                utforsk::RATTAD.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            let r = if args.iter().any(|a| a == "--omrakna") {
                utforsk::omrakna_utforsk(&rot)
            } else {
                utforsk::kor_utforsk(&rot, torr)
            };
            if let Err(e) = r {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("okänt kommando {kommando}");
            std::process::exit(2);
        }
    }
}

fn las_signal_val(args: &[String]) -> Result<SignalVal, String> {
    let torr = args.iter().any(|a| a == "--torr");
    let bara_v_lar = args.windows(2).any(|w| w[0] == "--bara" && w[1] == "v_lar")
        || args.iter().any(|a| a == "--bara=v_lar");
    let mut fro_start = None;
    let mut fro_slut = None;
    if let Some(i) = args.iter().position(|a| a == "--fron") {
        let s = args
            .get(i + 1)
            .ok_or("--fron kräver A-B")?;
        let (a, b) = tolka_fro_intervall(s)?;
        fro_start = Some(a);
        fro_slut = Some(b);
    } else if let Some(a) = args.iter().find(|a| a.starts_with("--fron=")) {
        let s = &a["--fron=".len()..];
        let (a, b) = tolka_fro_intervall(s)?;
        fro_start = Some(a);
        fro_slut = Some(b);
    }
    Ok(SignalVal {
        torr,
        fro_start,
        fro_slut,
        bara_v_lar,
    })
}

fn tolka_fro_intervall(s: &str) -> Result<(u64, u64), String> {
    let delar: Vec<&str> = s.split('-').collect();
    if delar.len() != 2 {
        return Err(format!("--fron förväntar A-B, fick {s}"));
    }
    let a: u64 = delar[0]
        .parse()
        .map_err(|_| format!("ogiltigt startfrö {}", delar[0]))?;
    let b: u64 = delar[1]
        .parse()
        .map_err(|_| format!("ogiltigt slutfrö {}", delar[1]))?;
    if b < a {
        return Err(format!("--fron: slut {b} < start {a}"));
    }
    Ok((a, b))
}

fn kor_hjsignal(rot: &PathBuf, args: &[String]) -> Result<(), String> {
    let omrakna = args.iter().any(|a| a == "--omrakna");
    if !omrakna {
        return Err("hcsim hjsignal kräver --omrakna".into());
    }
    let logg = text(args, "--logg")?.unwrap_or_else(|| {
        "/home/hakan/sandbox/pi/work/hcsim/v1251-logg.tsv".into()
    });
    let v_signal = text(args, "--v-signal")?.unwrap_or_else(|| {
        rot.join("v1251-signal.tsv").to_string_lossy().into_owned()
    });
    hjsignal::kor_omrakna(rot, PathBuf::from(logg).as_path(), PathBuf::from(v_signal).as_path())
}

fn kor(args: &[String]) -> Result<(), String> {
    let lat0 = tal(args, "--lat0")?.ok_or("--lat0 saknas")?;
    let lon0 = tal(args, "--lon0")?.ok_or("--lon0 saknas")?;
    let b = tal(args, "--b")?.ok_or("--b saknas")? as usize;
    let h = tal(args, "--h")?.ok_or("--h saknas")? as usize;
    let ruta_m = tal(args, "--ruta-m")?.ok_or("--ruta-m saknas")?;
    let t = tal(args, "--t")?.ok_or("--t saknas")?;
    let fro = tal(args, "--fro")?.ok_or("--fro saknas")? as u64;
    let ar = tal(args, "--ar")?.ok_or("--ar saknas")? as i64;
    let n0 = tal(args, "--n0")?.ok_or("--n0 saknas")? as usize;
    let var_ar = tal(args, "--var-ar")?.unwrap_or(10.0) as i64;
    let spara_fil = text(args, "--spara")?;
    let spara_var_ar = tal(args, "--spara-var-ar")?.unwrap_or(0.0) as i64;

    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paleo = rot.join("../paleokarta/data");
    let tt_path = rot.join("../hckarta/tidtabell.tsv");
    let kalla = Kalla::las(&paleo)?;
    let tt_text = std::fs::read_to_string(&tt_path).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text)?;
    let omr = Omrade {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
    };
    let kart = karta_full(&kalla, &tt, &omr, t, fro)?;
    let summa_fore = kontrollsumma(&kart);
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa: summa_fore,
    };

    let rader = if let Some(ref fil) = spara_fil {
        let path = PathBuf::from(fil);
        let intervall = if spara_var_ar > 0 {
            spara_var_ar
        } else {
            ar
        };
        spara::kor_med_spara(&mut sim, ar, var_ar, Some(&path), intervall, &parametrar)?
    } else {
        // Utan --spara: exakt samma väg som före Alt 1302.
        sim.kor_ar(ar, var_ar)
    };

    for r in &rader {
        println!(
            "ar={} antal={} fodda={} dod_svalt={} dod_alder={} v_medel={}",
            r.ar, r.antal, r.fodda, r.dod_svalt, r.dod_alder, r.v_medel
        );
    }
    let summa_efter = kontrollsumma(&kart);
    if summa_fore != summa_efter {
        return Err(format!(
            "kartan ändrades: {summa_fore} → {summa_efter} (Alt 1199)"
        ));
    }
    let json = sammanfattning_json(
        fro,
        ar,
        n0,
        &rader,
        sim.antal(),
        sim.djur_summa(),
        summa_fore,
    );
    println!("{json}");
    Ok(())
}

fn fortsatt(args: &[String]) -> Result<(), String> {
    let fran = text(args, "--fran")?.ok_or("--fran saknas")?;
    let fler_ar = tal(args, "--ar")?.ok_or("--ar saknas")? as i64;
    let var_ar = tal(args, "--var-ar")?.unwrap_or(10.0) as i64;
    let spara_fil = text(args, "--spara")?;

    let path = PathBuf::from(&fran);
    let (parametrar, mut sim) = spara::ladda(&path)?;

    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paleo = rot.join("../paleokarta/data");
    let tt_path = rot.join("../hckarta/tidtabell.tsv");
    let kalla = Kalla::las(&paleo)?;
    let tt_text = std::fs::read_to_string(&tt_path).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text)?;
    let omr = Omrade {
        lat0: parametrar.lat0,
        lon0: parametrar.lon0,
        b: parametrar.b,
        h: parametrar.h,
        ruta_m: parametrar.ruta_m,
    };
    let kart = karta_full(&kalla, &tt, &omr, parametrar.t, parametrar.fro)?;
    let summa = kontrollsumma(&kart);
    if summa != parametrar.karta_summa {
        return Err(format!(
            "kartans kontrollsumma stämmer inte: fil {} beräknad {summa}",
            parametrar.karta_summa
        ));
    }

    let start_ar = sim.t / hcsim::tal::STEG_PER_AR;
    let slut_ar = start_ar + fler_ar;
    let rader = spara::kor_till_ar(&mut sim, slut_ar, var_ar);

    if let Some(ref fil) = spara_fil {
        spara::spara(std::path::Path::new(fil), &parametrar, &sim)?;
    } else {
        // Skriv tillbaka till samma fil som slutläge (så rundturen kan jämföras).
        spara::spara(&path, &parametrar, &sim)?;
    }

    for r in &rader {
        println!(
            "ar={} antal={} fodda={} dod_svalt={} dod_alder={} v_medel={}",
            r.ar, r.antal, r.fodda, r.dod_svalt, r.dod_alder, r.v_medel
        );
    }
    let json = sammanfattning_json(
        parametrar.fro,
        slut_ar,
        parametrar.n0,
        &rader,
        sim.antal(),
        sim.djur_summa(),
        parametrar.karta_summa,
    );
    println!("{json}");
    Ok(())
}

fn tal(args: &[String], namn: &str) -> Result<Option<f64>, String> {
    for i in 0..args.len() {
        if args[i] == namn {
            let v = args
                .get(i + 1)
                .ok_or_else(|| format!("{namn} saknar värde"))?;
            return Ok(Some(
                v.parse::<f64>()
                    .map_err(|_| format!("{namn}: inget tal ({v})"))?,
            ));
        }
    }
    Ok(None)
}

fn text(args: &[String], namn: &str) -> Result<Option<String>, String> {
    for i in 0..args.len() {
        if args[i] == namn {
            let v = args
                .get(i + 1)
                .ok_or_else(|| format!("{namn} saknar värde"))?;
            return Ok(Some(v.clone()));
        }
    }
    Ok(None)
}
