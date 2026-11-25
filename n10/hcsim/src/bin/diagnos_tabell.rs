//! Läser `diagnos1207-korningar.jsonl` och skriver medel per förmåga över alla körningar (Claude Code, 2026-11-21).
//! Rust enligt regel 11: talen till agenternas frågor räknas här, inte i frågeskriptet.
//! Körs: `cargo run --release --bin diagnos_tabell -- diagnos1207-korningar.jsonl > diagnos1207-formagor.tsv`.
//! Läsaren är enkel och gäller bara filens eget format (en JSON-rad per körning, fälten i `matning`).
use std::collections::BTreeMap;

/// Talet efter `"nyckel":` med början i `s`; `None` för `null`.
fn tal(s: &str, nyckel: &str) -> Option<f64> {
    let m = format!("\"{nyckel}\":");
    let p = s.find(&m).unwrap_or_else(|| panic!("nyckeln {nyckel} saknas")) + m.len();
    let r = &s[p..];
    let slut = r.find(|c: char| c == ',' || c == '}').expect("slut på talet");
    let v = r[..slut].trim();
    if v == "null" {
        None
    } else {
        Some(v.parse().unwrap_or_else(|_| panic!("inte ett tal: {v}")))
    }
}

fn text<'a>(s: &'a str, nyckel: &str) -> &'a str {
    let m = format!("\"{nyckel}\":\"");
    let p = s.find(&m).unwrap_or_else(|| panic!("nyckeln {nyckel} saknas")) + m.len();
    let r = &s[p..];
    &r[..r.find('"').expect("slut på texten")]
}

const FALT: [&str; 7] = ["andel_sta_kvar", "entropi", "intag_per_steg", "andra_medel", "e_vid_dod", "fel_tidigt", "fel_sent"];

fn main() {
    let vag = std::env::args().nth(1).expect("ange jsonl-filen");
    let innehall = std::fs::read_to_string(&vag).expect("läs filen");
    // förmåga -> (summor per fält, antal per fält, svält, ålder, körningar)
    let mut acc: BTreeMap<String, ([f64; 7], [u32; 7], f64, f64, u32)> = BTreeMap::new();
    let mut rader = 0;
    for rad in innehall.lines().filter(|r| !r.trim().is_empty()) {
        assert!(!rad.contains("TORRKÖRNING"), "torrkörningens rad i filen");
        rader += 1;
        let m = &rad[rad.find("\"matning\":[").expect("matning saknas")..];
        for del in m.split("{\"formaga\":").skip(1) {
            let obj = format!("{{\"formaga\":{del}");
            let f = text(&obj, "formaga").to_string();
            let e = acc.entry(f).or_insert(([0.0; 7], [0; 7], 0.0, 0.0, 0));
            for (k, n) in FALT.iter().enumerate() {
                if let Some(v) = tal(&obj, n) {
                    e.0[k] += v;
                    e.1[k] += 1;
                }
            }
            e.2 += tal(&obj, "dod_svalt").expect("dod_svalt");
            e.3 += tal(&obj, "dod_alder").expect("dod_alder");
            e.4 += 1;
        }
    }
    assert!(rader > 0 && !acc.is_empty(), "tom läsning");
    eprintln!("{rader} körningar lästa");
    println!("formaga\tkorningar\t{}\tandel_svalt_av_doda", FALT.join("\t"));
    for (f, (s, n, sv, al, k)) in &acc {
        let mut ut = format!("{f}\t{k}");
        for i in 0..7 {
            if n[i] == 0 {
                ut += "\t-";
            } else {
                ut += &format!("\t{:.4}", s[i] / n[i] as f64);
            }
        }
        ut += &format!("\t{:.4}", sv / (sv + al));
        println!("{ut}");
    }
}

#[cfg(test)]
mod prov {
    use super::*;
    #[test]
    fn laser_en_riktig_rad() {
        let r = r#"{"formaga":"slump","andelar":[0.1],"entropi":2.19,"andel_sta_kvar":0.11,"intag_per_steg":1.28,"e_vid_dod":121.0,"dod_svalt":41711,"dod_alder":15125,"andra_medel":1.36,"fel_tidigt":null,"fel_sent":null,"djursteg":8}"#;
        assert_eq!(text(r, "formaga"), "slump");
        assert_eq!(tal(r, "andra_medel"), Some(1.36));
        assert_eq!(tal(r, "fel_tidigt"), None);
        assert_eq!(tal(r, "dod_svalt"), Some(41711.0));
    }
}
