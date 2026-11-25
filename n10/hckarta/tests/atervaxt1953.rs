//! Agenternas Alt 1953 (protokollet n10/docs/2026-11-23-omrostning-atervaxt.md): tabellen över växternas återväxt och tak per växtgrupp
//! (`atervaxt.tsv`) har källa, citat och läsning på varje rad med tal, talen är rätt omräknade ur källans egna tal, alla kartans växtgrupper
//! finns med, och tabellen är låst: en ändring gör provet `tabellen_ar_last` rött (då ska ändringen skälas i commit-meddelandet, aldrig
//! göras efter en grindkörning).

const LAS: u64 = 9171912442114829311; // kontrollsumman ur provets egen utskrift när tabellen låstes
const GRUPPER: [&str; 6] = ["lovtrad", "barrtrad", "ort", "ormbunke", "kottepalm_ginkgo", "vattenvaxt"];
const ANDRA: [&str; 3] = ["alla", "skog", "okand"];

fn fnv(t: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in t.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn tal(s: &str) -> Option<f64> {
    if s == "-" { None } else { Some(s.parse::<f64>().unwrap_or(f64::NAN)) }
}

fn kontrollera(text: &str) -> Result<usize, String> {
    let mut rader = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let hu: Vec<&str> = rader.next().ok_or("tom tabell")?.split('\t').collect();
    let ix = |n: &str| hu.iter().position(|x| *x == n).ok_or(format!("kolumnen {n} saknas"));
    let (cg, cl, cm, ch, cu, cf, ck, cd, cc, cs) =
        (ix("grupp")?, ix("lag")?, ix("medel")?, ix("hog")?, ix("ur_kallan")?, ix("faktor")?, ix("kalla")?, ix("doi")?, ix("citat")?, ix("last")?);
    let mut antal = 0;
    let mut sedda = std::collections::BTreeSet::new();
    for (nr, r) in rader.enumerate() {
        let c: Vec<&str> = r.split('\t').collect();
        if c.len() != hu.len() { return Err(format!("rad {nr}: {} kolumner, ska vara {}", c.len(), hu.len())); }
        let g = c[cg];
        if !GRUPPER.contains(&g) && !ANDRA.contains(&g) { return Err(format!("rad {nr}: okänd grupp {g}")); }
        sedda.insert(g.to_string());
        let varden = [tal(c[cl]), tal(c[cm]), tal(c[ch])];
        if c[ck] == "ingen källa" {
            if varden.iter().any(|v| v.is_some()) { return Err(format!("rad {nr}: tal utan källa")); }
            antal += 1;
            continue;
        }
        if c[cd] == "-" || c[cd].is_empty() || !c[cd].starts_with("10.") { return Err(format!("rad {nr}: DOI saknas")); }
        if c[cc] == "-" || c[cc].len() < 10 { return Err(format!("rad {nr}: citat saknas")); }
        if c[cs] != "fulltext" && c[cs] != "sammandrag" { return Err(format!("rad {nr}: läsningen ska vara fulltext eller sammandrag")); }
        let f: f64 = c[cf].parse().map_err(|_| format!("rad {nr}: faktorn"))?;
        let ur_del = c[cu].split(' ').next().unwrap_or("");
        let ur: Vec<&str> = if ur_del == "-" { vec![] } else { ur_del.split('|').collect() };
        if varden.iter().any(|v| v.is_some()) {
            if ur.len() != 3 { return Err(format!("rad {nr}: ur_kallan ska ha tre delar")); }
            for k in 0..3 {
                match (varden[k], tal(ur[k])) {
                    (None, None) => {}
                    (Some(v), Some(u)) => if !((v - u * f).abs() <= 1e-9 * (1.0 + v.abs())) {
                        return Err(format!("rad {nr}: {v} är inte {u} · {f}"));
                    },
                    _ => return Err(format!("rad {nr}: tal och källans tal stämmer inte överens i del {k}")),
                }
            }
        }
        antal += 1;
    }
    if antal == 0 { return Err("inga rader".into()); }
    for g in GRUPPER { if !sedda.contains(g) { return Err(format!("växtgruppen {g} saknas")); } }
    Ok(antal)
}

fn tabellen() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/atervaxt.tsv")).expect("atervaxt.tsv")
}

#[test]
fn tabellen_ar_hel() {
    let n = kontrollera(&tabellen()).unwrap();
    assert_eq!(n, 27, "antal rader");
}

#[test]
fn tabellen_ar_last() {
    assert_eq!(fnv(&tabellen()), LAS, "atervaxt.tsv har ändrats sedan låset (Alt 1953)");
}

#[test]
fn rott_utan_doi() {
    let t = tabellen().replacen("10.1002/ecs2.3209", "-", 1);
    assert!(kontrollera(&t).is_err());
}

#[test]
fn rott_vid_fel_omrakning() {
    let t = tabellen().replacen("\t0.474\t", "\t0.475\t", 1);
    assert!(kontrollera(&t).unwrap_err().contains("är inte"));
}

#[test]
fn rott_vid_tal_utan_kalla() {
    let t = format!("{}vattenvaxt\tB\t-\t0.5\t-\tkg/m2\t-\t1\tingen källa\t-\t-\tingen\t-\t-\t-\t-\t-\n", tabellen());
    assert!(kontrollera(&t).unwrap_err().contains("utan källa"));
}

#[test]
fn rott_nar_en_grupp_saknas() {
    let t: String = tabellen().lines().filter(|l| !l.starts_with("kottepalm_ginkgo")).map(|l| format!("{l}\n")).collect();
    assert!(kontrollera(&t).unwrap_err().contains("kottepalm_ginkgo"));
}

#[test]
fn stopp_vid_tom_fil() {
    assert!(kontrollera("").is_err());
    assert!(kontrollera("# bara rubrik\n").is_err());
}

#[test]
fn laset_ser_en_andring() {
    let t = tabellen();
    assert_ne!(fnv(&t), fnv(&t.replacen("0.474", "0.475", 1)));
}
