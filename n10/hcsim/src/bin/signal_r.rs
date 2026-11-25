//! Räknar Alt 1251:s mått 1–5 ur signalloggen (och rangloggen för mått 5).
//! Läser bara; inga slumptal ur världens följd.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process;

use hcsim::minstakvadrat;
use hcsim::rang::{LED_ANP_N, LED_ANP_START, LED_LOGG_VAG};
use hcsim::signal::{
    avgor_1251, avgor_fast_per_djur, blandning_pa_rang, las_logg_tsv, matt_per_fro, skriv_signal_tsv,
};
use hcsim::tur::RANG_LOGG_VAG;

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let logg_path = ta_vag(
        &mut args,
        "--logg",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("v1251-logg.tsv")
            .to_string_lossy(),
    );
    let led_path = ta_vag(&mut args, "--led", LED_LOGG_VAG);
    let rang_path = ta_vag(&mut args, "--rang", RANG_LOGG_VAG);
    let ut_path = ta_vag(
        &mut args,
        "--ut",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("v1251-signal.tsv")
            .to_string_lossy(),
    );
    let korningar_path = ta_vag(
        &mut args,
        "--korningar",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("v1251-korningar.jsonl")
            .to_string_lossy(),
    );

    let logg_text = match std::fs::read_to_string(&logg_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("kunde inte läsa signallogg {}: {e}", logg_path);
            process::exit(1);
        }
    };
    let logg = match las_logg_tsv(&logg_text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("signallogg-TSV-fel: {e}");
            process::exit(1);
        }
    };
    if logg.is_empty() {
        eprintln!("tom signallogg");
        process::exit(1);
    }

    let (ing_mark, ing_slut) = las_ingrepp(&korningar_path);

    let matt = match matt_per_fro(&logg, &ing_mark, &ing_slut) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    let bland = match rakna_blandning(&led_path, &rang_path) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("mått 5 kunde inte räknas: {e}");
            None
        }
    };

    let tsv = skriv_signal_tsv(&matt, bland.as_deref());
    print!("{tsv}");
    if let Err(e) = std::fs::write(&ut_path, &tsv) {
        eprintln!("kunde inte skriva {}: {e}", ut_path);
        process::exit(1);
    }

    let (instrument, alt1251, flock, _, v0) = avgor_1251(&matt);
    println!("# Avgöranden");
    println!("Instrumentet: {instrument}");
    println!("Alt 1251: {alt1251}");
    println!("Flockens drift: {flock}");
    if let Some(ref b) = bland {
        println!("Fast per djur: {}", avgor_fast_per_djur(b));
    }
    println!("{v0}");
    eprintln!("skrev {ut_path}");
}

fn las_ingrepp(path: &str) -> (BTreeMap<u64, f64>, BTreeMap<u64, f64>) {
    let mut mark = BTreeMap::new();
    let mut slut = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return (mark, slut);
    };
    for rad in text.lines() {
        let json = rad.split(" TORRKÖRNING").next().unwrap_or(rad);
        if !json.contains("\"arm\":\"v_lar\"") {
            continue;
        }
        let fro = match json_u64(json, "fro") {
            Some(f) => f,
            None => continue,
        };
        if let Some(v) = json_f64_opt(json, "ingrepp_mark") {
            mark.insert(fro, v);
        }
        if let Some(v) = json_f64_opt(json, "ingrepp_slut") {
            slut.insert(fro, v);
        }
    }
    (mark, slut)
}

fn json_u64(json: &str, nyckel: &str) -> Option<u64> {
    let mark = format!("\"{nyckel}\":");
    let i = json.find(&mark)?;
    let start = i + mark.len();
    let slut = json[start..]
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(json[start..].len());
    json[start..start + slut].parse().ok()
}

fn json_f64_opt(json: &str, nyckel: &str) -> Option<f64> {
    let mark = format!("\"{nyckel}\":");
    let i = json.find(&mark)?;
    let start = i + mark.len();
    if json[start..].starts_with("null") {
        return None;
    }
    let slut = json[start..]
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(json[start..].len());
    json[start..start + slut].parse().ok()
}

fn rakna_blandning(
    led_vag: &str,
    rang_vag: &str,
) -> Result<Vec<hcsim::signal::BlandningFro>, String> {
    let led_text = std::fs::read_to_string(led_vag)
        .map_err(|e| format!("kunde inte läsa ledprovsloggen {led_vag}: {e}"))?;
    let led = minstakvadrat::las_tsv(&led_text)?;
    let rang_text = std::fs::read_to_string(rang_vag)
        .map_err(|e| format!("kunde inte läsa rangloggen {rang_vag}: {e}"))?;
    let rang = minstakvadrat::las_rang_tsv(&rang_text)?;
    let anp: Vec<u64> = (LED_ANP_START..LED_ANP_START + LED_ANP_N).collect();
    let g1 = minstakvadrat::anpassa_g1(&led, &anp);
    blandning_pa_rang(&rang, &g1)
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
