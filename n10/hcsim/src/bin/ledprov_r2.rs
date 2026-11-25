//! Räknar R² för G1/G2/G3 ur ledprovets logg (Alt 1221).
//! Anpassning på fröna 141–150, prövning på 151–160.

use std::env;
use std::path::PathBuf;
use std::process;

use hcsim::ledprov::{FRO_START, N_FRO_FULL};
use hcsim::minstakvadrat::{self, R2Samling};

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let logg_path = if let Some(i) = args.iter().position(|a| a == "--logg") {
        let p = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| {
                eprintln!("--logg saknar sökväg");
                process::exit(2);
            });
        args.drain(i..=i + 1);
        PathBuf::from(p)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ledprov1221-logg.tsv")
    };

    let text = match std::fs::read_to_string(&logg_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kunde inte läsa {}: {e}", logg_path.display());
            process::exit(1);
        }
    };
    let rader = match minstakvadrat::las_tsv(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("TSV-fel: {e}");
            process::exit(1);
        }
    };
    if rader.is_empty() {
        eprintln!("tom logg: inga rader");
        process::exit(1);
    }

    let anp: Vec<u64> = (FRO_START..FRO_START + 10).collect();
    let prov: Vec<u64> = (FRO_START + 10..FRO_START + N_FRO_FULL).collect();
    match minstakvadrat::rakna_alla(&rader, &anp, &prov) {
        Ok((g1, g2, g3)) => {
            skriv(&g1);
            skriv(&g2);
            skriv(&g3);
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

fn skriv(g: &R2Samling) {
    println!(
        "{} medel_flytt={:?} sta_kvar={:?}",
        g.namn, g.medel_flytt, g.sta_kvar
    );
    for r in &g.per_handling {
        if r.singulaer {
            println!("  h{} SINGULÄR {}", r.handling, r.meddelande);
        } else {
            println!(
                "  h{} R2={:?} n_anp={} n_prov={}",
                r.handling, r.r2, r.n_anp, r.n_prov
            );
        }
    }
}
