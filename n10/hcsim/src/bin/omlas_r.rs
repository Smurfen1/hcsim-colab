//! Omläsning Alt 1245: anpassar G1 på ledprovets logg (141–150) och räknar
//! (a) flyttträff + Spearman, (b) andel där G1 rankar stå kvar högst,
//! (c) intag − medel av G1:s 9 gissningar, per frö ur rangloggen (161–180).
//! Läser bara; inga slumptal ur världens följd.

use std::env;
use std::path::PathBuf;
use std::process;

use hcsim::minstakvadrat::{self, G1Vikter};
use hcsim::rang::{LED_ANP_N, LED_ANP_START, LED_LOGG_VAG};
use hcsim::tur::RANG_LOGG_VAG;

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let led_path = ta_vag(&mut args, "--led", LED_LOGG_VAG);
    let rang_path = ta_vag(&mut args, "--rang", RANG_LOGG_VAG);
    let ut_path = ta_vag(
        &mut args,
        "--ut",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tur1246-omlas.tsv")
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

    let per_rad = match minstakvadrat::omlas_per_rad(&rang, &g1) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let per_fro = match minstakvadrat::omlas_per_fro(&per_rad) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    let mut tsv = String::from(
        "fro\tflytt_traff\tflytt_spearman\tandel_g1_sta_kvar\tintag_minus_g1_medel\tn_djur\tn_flytt_rader\tn_rader\n",
    );
    for f in &per_fro {
        let ft = f
            .flytt_traff
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "saknas".into());
        let fs = f
            .flytt_spearman
            .map(|x| format!("{x}"))
            .unwrap_or_else(|| "saknas".into());
        let rad = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            f.fro,
            ft,
            fs,
            f.andel_g1_sta_kvar,
            f.intag_minus_g1_medel,
            f.n_djur,
            f.n_flytt_rader,
            f.n_rader
        );
        print!("{rad}");
        tsv.push_str(&rad);
    }
    if let Err(e) = std::fs::write(&ut_path, &tsv) {
        eprintln!("kunde inte skriva {}: {e}", ut_path);
        process::exit(1);
    }
    eprintln!("skrev {ut_path}");
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
