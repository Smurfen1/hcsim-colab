//! Anpassar G1 på ledprovets logg (fröna 141–150) och räknar träff + Spearman
//! på rangprovets logg (alla 9 hjärnvärden per rad). Läser bara.

use std::env;
use std::path::PathBuf;
use std::process;

use hcsim::minstakvadrat::{self, G1Vikter};
use hcsim::rang::{LED_ANP_N, LED_ANP_START, LED_LOGG_VAG};

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let led_path = ta_vag(&mut args, "--led", LED_LOGG_VAG);
    let rang_path = ta_vag(
        &mut args,
        "--rang",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("rang1228-logg.tsv")
            .to_string_lossy(),
    );

    let led_text = match std::fs::read_to_string(&led_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kunde inte läsa ledlogg {}: {e}", led_path);
            process::exit(1);
        }
    };
    let led = match minstakvadrat::las_tsv(&led_text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("led-TSV-fel: {e}");
            process::exit(1);
        }
    };
    if led.is_empty() {
        eprintln!("tom ledlogg");
        process::exit(1);
    }

    let rang_text = match std::fs::read_to_string(&rang_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kunde inte läsa ranglogg {}: {e}", rang_path);
            process::exit(1);
        }
    };
    let rang = match minstakvadrat::las_rang_tsv(&rang_text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("rang-TSV-fel: {e}");
            process::exit(1);
        }
    };
    if rang.is_empty() {
        eprintln!("tom ranglogg");
        process::exit(1);
    }

    let anp: Vec<u64> = (LED_ANP_START..LED_ANP_START + LED_ANP_N).collect();
    let g1 = minstakvadrat::anpassa_g1(&led, &anp);
    skriv_g1(&g1);
    match minstakvadrat::rang_per_rad(&rang, &g1) {
        Ok(per_rad) => {
            let per_fro = minstakvadrat::rang_per_fro(&per_rad);
            for f in &per_fro {
                println!(
                    "fro={} traff={:.6} spearman={:.6} n_djur={} n_rader={}",
                    f.fro, f.traff, f.spearman, f.n_djur, f.n_rader
                );
            }
            let n = per_fro.len() as f64;
            let mt = per_fro.iter().map(|f| f.traff).sum::<f64>() / n;
            let ms: Vec<f64> = per_fro
                .iter()
                .map(|f| f.spearman)
                .filter(|x| x.is_finite())
                .collect();
            let ms_medel = if ms.is_empty() {
                f64::NAN
            } else {
                ms.iter().sum::<f64>() / ms.len() as f64
            };
            println!("medel_traff={mt:.6} medel_spearman={ms_medel:.6}");
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn ta_vag(args: &mut Vec<String>, flagga: &str, standard: &str) -> String {
    if let Some(i) = args.iter().position(|a| a == flagga) {
        let p = args.get(i + 1).cloned().unwrap_or_else(|| {
            eprintln!("{flagga} saknar sökväg");
            process::exit(2);
        });
        args.drain(i..=i + 1);
        p
    } else {
        standard.to_string()
    }
}

fn skriv_g1(g: &G1Vikter) {
    for h in 0..hcsim::tal::A_N {
        if g.singulaer[h] {
            println!("G1 h{h} SINGULÄR {}", g.meddelande[h]);
        } else if g.beta[h].is_none() {
            println!("G1 h{h} saknas {}", g.meddelande[h]);
        } else {
            println!("G1 h{h} n_anp={}", g.n_anp[h]);
        }
    }
}
