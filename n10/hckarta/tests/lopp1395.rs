//! Prov 1–10 för Alt 1395: floderna byter lopp över tiden (tidsmodell 2).
//! Underlag: n10/docs/2026-11-22-lasunderlag-kartlopp.md (skrivet före bygget).
//! Proven jämför med underlagets tal skrivna som tal, aldrig med programmets egna konstanter.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use hckarta::{
    andel_nytt_lopp_pa_tidigare, karta_for_ar, karta_full_version, loppbyten, mittlinjer_vid_ar,
    nasta_andring, Kalla, Omrade, Period, Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell() -> Tidtabell {
    let text =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
            .expect("kunde inte läsa tidtabell.tsv");
    Tidtabell::las(&text).expect("tidtabellen")
}

fn omrade() -> Omrade {
    Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 64,
        h: 48,
        ruta_m: 2000.0,
    }
}

fn temp_mapp(namn: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "hckarta-lopp1395-{}-{}-{}",
        namn,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("temp-mapp");
    p
}

fn bin_hckarta() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hckarta"))
}

fn sha256_fil(path: &std::path::Path) -> String {
    let ut = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum");
    assert!(ut.status.success());
    let s = String::from_utf8_lossy(&ut.stdout);
    s.split_whitespace().next().unwrap().to_string()
}

fn las_fore_sha(version: &str, tm: &str, t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lopp1395-fore.txt"),
    )
    .expect("lopp1395-fore.txt saknas — räkna före-baseline först");
    let mark = format!("sha256 filer flodversion={version} tidsmodell={tm} t={t}:");
    let mut efter = false;
    for rad in text.lines() {
        if rad.contains(&mark) {
            efter = true;
            continue;
        }
        if efter {
            if rad.starts_with("===") || rad.starts_with("---") || rad.starts_with("sha256") {
                break;
            }
            if rad.trim().is_empty() {
                break;
            }
            let mut delar = rad.split_whitespace();
            if let (Some(hash), Some(namn)) = (delar.next(), delar.next()) {
                if namn == fil {
                    return hash.to_string();
                }
            }
        }
    }
    panic!("saknar sha256 för {fil} vid flodversion={version} tidsmodell={tm} t={t}");
}

fn kor_skriv(version: &str, tm: &str, t: &str) -> PathBuf {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    let mapp = temp_mapp(&format!("v{version}-tm{tm}-t{t}"));
    // Standardbytet (Alt 1566): provet gäller den gamla vägen; läge 1 läggs sist för de fyra valen, och en flagga som redan står tidigare gäller (programmet läser första förekomsten).
    let status = Command::new(&hk)
        .args([
            "--lat0",
            "47",
            "--lon0",
            "-108",
            "--b",
            "64",
            "--h",
            "48",
            "--ruta-m",
            "2000",
            "--t",
            t,
            "--tidtabell",
        ])
        .arg(&tt)
        .args([
            "--fro",
            "181",
            "--flodversion",
            version,
            "--tidsmodell",
            tm,
            "--skriv",
        ])
        .arg(&mapp)
        .args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status()
        .expect("kör hckarta");
    assert!(status.success(), "hckarta v={version} tm={tm} t={t}");
    mapp
}

/// Prov 1: tidsmodell 1 och karta_full orörda (sha256 = före).
#[test]
fn prov1_gamla_kartor_ororda() {
    for version in ["1", "2"] {
        for t in ["500000", "20000", "0"] {
            let mapp = kor_skriv(version, "1", t);
            let filer: Vec<_> = std::fs::read_dir(&mapp)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert!(!filer.is_empty());
            for fil in &filer {
                let efter = sha256_fil(&mapp.join(fil));
                let fore = las_fore_sha(version, "1", t, fil);
                assert_eq!(
                    efter, fore,
                    "fil {fil} flodversion={version} tidsmodell=1 t={t} skiljer sig från före"
                );
            }
        }
    }
}

/// Prov 2: hcsim slutsummor orörda.
#[test]
fn prov2_hcsim_orort() {
    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    assert!(
        hcsim.exists(),
        "bygg hcsim först: CARGO_TARGET_DIR=…/hcsim/target cargo build --release i n10/hcsim"
    );
    let ut = Command::new(&hcsim)
        .args([
            "kor",
            "--lat0",
            "47",
            "--lon0",
            "-108",
            "--b",
            "64",
            "--h",
            "48",
            "--ruta-m",
            "2000",
            "--t",
            "500000",
            "--fro",
            "181",
            "--ar",
            "5",
            "--n0",
            "400",
        ])
        .output()
        .expect("hcsim");
    assert!(ut.status.success(), "hcsim misslyckades");
    let stdout = String::from_utf8_lossy(&ut.stdout);
    // 5548 sedan hcsim 88c0a5f (BRAIN, Alt 1624: flodkanten får växter); kartans karta_summa oförändrad. Förut 4992.
    assert!(
        stdout.contains("\"slut_antal\":5548"),
        "slut_antal: {stdout}"
    );
    assert!(
        // djur_summa 4225976351498359620 sedan hcsim 88c0a5f (BRAIN, Alt 1624); förut 9527944887746424482.
        stdout.contains("\"djur_summa\":4225976351498359620"),
        "djur_summa: {stdout}"
    );
    assert!(
        stdout.contains("\"karta_summa\":9828389463751256610"),
        "karta_summa: {stdout}"
    );
}

/// Prov 3: bara en fåra ändras per byte (övriga mittlinjer lika; uppström oförändrad).
#[test]
fn prov3_bara_en_fara_andras() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for flodriktning in [1u8, 2] {
        let byten = loppbyten(&k, &tt, &omr, 181, 1, flodriktning).unwrap();
        assert!(!byten.is_empty(), "förväntade loppbyten fro=181 r={flodriktning}");
        // Hitta ett byte där loppet faktiskt ändras (riktning 2 kan ge samma väg längs kanten).
        let mut hittat = false;
        for b0 in &byten {
            let t_fore = b0.ar + 1.0;
            let t_efter = b0.ar - 1.0;
            let (linjer_fore, lev_fore) =
                mittlinjer_vid_ar(&k, &tt, &omr, t_fore, 181, flodriktning).unwrap();
            let (linjer_efter, lev_efter) =
                mittlinjer_vid_ar(&k, &tt, &omr, t_efter, 181, flodriktning).unwrap();
            assert_eq!(lev_fore, lev_efter);
            let Some(ix) = lev_fore.iter().position(|&f| f == b0.fara) else {
                continue;
            };
            if linjer_fore[ix] == linjer_efter[ix] {
                continue;
            }
            for (i, (a, b)) in linjer_fore.iter().zip(linjer_efter.iter()).enumerate() {
                if i == ix {
                    let pidx = a
                        .iter()
                        .position(|&c| c == b0.punkt)
                        .expect("punkten ska finnas i fåran före bytet");
                    assert_eq!(&a[..=pidx], &b[..=pidx], "uppström ändrad");
                } else {
                    assert_eq!(a, b, "annan fåra {i} ändrades vid byte av {}", b0.fara);
                }
            }
            hittat = true;
            break;
        }
        assert!(
            hittat,
            "hittade inget byte som faktiskt ändrade loppet r={flodriktning}"
        );
    }
}

/// Prov 4: punkten ligger mellan 20 % och 80 % av fårans längd före bytet.
#[test]
fn prov4_punkten() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in 1u64..=50 {
        for flodriktning in [1u8, 2] {
            let byten = loppbyten(&k, &tt, &omr, fro, 1, flodriktning).unwrap();
            for b in &byten {
                let t_fore = b.ar + 1.0;
                let (linjer, lev) =
                    mittlinjer_vid_ar(&k, &tt, &omr, t_fore, fro, flodriktning).unwrap();
                let Some(ix) = lev.iter().position(|&f| f == b.fara) else {
                    continue;
                };
                let path = &linjer[ix];
                let len = path.len();
                assert!(len >= 2);
                let pidx = path
                    .iter()
                    .position(|&c| c == b.punkt)
                    .expect("punkten i path");
                let andel = pidx as f64 / (len as f64);
                // Underlagets tal 20 % och 80 % skrivna som tal.
                assert!(
                    (0.20..=0.80).contains(&andel),
                    "punktandel {andel} utanför 20–80 % fro={fro} ar={} fara={} pidx={pidx} len={len}",
                    b.ar,
                    b.fara
                );
            }
        }
    }
}

/// Prov 2 (takten): mellanrum i [82 000, 152 000] för fröna 1–50.
#[test]
fn prov2_takten() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in 1u64..=50 {
        let byten = loppbyten(&k, &tt, &omr, fro, 1, 1).unwrap();
        let mut per_fara: Vec<Vec<f64>> = vec![Vec::new(); 5];
        for b in &byten {
            if b.fara < per_fara.len() {
                per_fara[b.fara].push(b.ar);
            }
        }
        for ar_lista in &per_fara {
            // byten är i tidsordning äldst→yngst (ar fallande).
            for w in ar_lista.windows(2) {
                let mellanrum = w[0] - w[1];
                assert!(
                    mellanrum >= 82_000.0 - 1e-6 && mellanrum <= 152_000.0 + 1e-6,
                    "mellanrum {mellanrum} utanför [82000, 152000] fro={fro}"
                );
            }
        }
    }
}

/// Prov 5: nytt lopp söker gamla bälten — andel med bonus 8 m > andel med bonus 0.
#[test]
fn prov5_bonus_soker_gamla_balten() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let med_bonus = andel_nytt_lopp_pa_tidigare(&k, &tt, &omr, 1, 50, 1, 8.0).unwrap();
    let utan_bonus = andel_nytt_lopp_pa_tidigare(&k, &tt, &omr, 1, 50, 1, 0.0).unwrap();
    assert!(
        med_bonus > utan_bonus,
        "bonus 8 m ska ge högre andel på tidigare lopp: med={med_bonus} utan={utan_bonus}"
    );
}

/// Prov 6: gammal nedström → Trask (utom hav och levande lopp).
#[test]
fn prov6_overgivna_blir_trask() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let byten = loppbyten(&k, &tt, &omr, 181, 1, 1).unwrap();
    assert!(!byten.is_empty());
    let b0 = &byten[0];
    let t_fore = b0.ar + 1.0;
    let t_efter = b0.ar - 1.0;
    let (linjer_fore, lev) = mittlinjer_vid_ar(&k, &tt, &omr, t_fore, 181, 1).unwrap();
    let ix = lev.iter().position(|&f| f == b0.fara).unwrap();
    let path = &linjer_fore[ix];
    let pidx = path.iter().position(|&c| c == b0.punkt).unwrap();
    let gammal_nedstrom = &path[pidx + 1..];
    let (linjer_efter, _) = mittlinjer_vid_ar(&k, &tt, &omr, t_efter, 181, 1).unwrap();
    let mut levande = HashSet::new();
    for p in &linjer_efter {
        for &c in p {
            levande.insert(c);
        }
    }
    let (kart, _) = karta_for_ar(&k, &tt, &omr, t_efter, 181, 1, 1).unwrap();
    let mut n_trask = 0;
    for &(i, j) in gammal_nedstrom {
        let p = i + j * omr.b;
        if kart.hojd.varden[p] < 0.0 {
            continue;
        }
        // Underlaget: Trask utom hav och rutor där ett levande lopp går
        // (mittlinje eller bredd-carving → Flodfara).
        if levande.contains(&(i, j)) || kart.vattentyp.varden[p] == Vatten::Flodfara {
            continue;
        }
        assert_eq!(
            kart.vattentyp.varden[p],
            Vatten::Trask,
            "gammal nedström ({i},{j}) ska vara Trask"
        );
        n_trask += 1;
    }
    assert!(n_trask > 0, "förväntade minst en Trask-ruta i gammal nedström");
}

/// Prov 7: ingen ändring utan händelse; nasta_andring pekar på nästa byte.
#[test]
fn prov7_ingen_andring_utan_handelse() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fro = 181u64;
    let byten = loppbyten(&k, &tt, &omr, fro, 1, 1).unwrap();
    assert!(!byten.is_empty());
    let b0 = &byten[0];
    // Strax före bytet: nasta_andring ska peka på bytet (eller närmare periodbyte).
    let t_fore = b0.ar + 10_000.0;
    let nasta = nasta_andring(&tt, t_fore, fro).unwrap();
    assert!(nasta < t_fore);
    assert!(nasta >= b0.ar - 1e-6 || nasta == 290_000.0 || nasta == 0.0);
    // Mellan två t utan händelse: kartorna lika.
    let t_hi = nasta + 5_000.0;
    if t_hi < t_fore {
        let (a, _) = karta_for_ar(&k, &tt, &omr, t_hi, fro, 1, 1).unwrap();
        let (b, _) = karta_for_ar(&k, &tt, &omr, nasta + 1.0, fro, 1, 1).unwrap();
        assert_eq!(a.vattentyp.varden, b.vattentyp.varden);
    }
}

/// Prov 8: samma frö → samma historik; 181 ≠ 182.
#[test]
fn prov8_samma_fro() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let a1 = loppbyten(&k, &tt, &omr, 181, 1, 1).unwrap();
    let a2 = loppbyten(&k, &tt, &omr, 181, 1, 1).unwrap();
    assert_eq!(a1, a2);
    let b = loppbyten(&k, &tt, &omr, 182, 1, 1).unwrap();
    assert_ne!(a1, b);
    let (k1, _) = karta_for_ar(&k, &tt, &omr, 500_000.0, 181, 1, 1).unwrap();
    let (k1b, _) = karta_for_ar(&k, &tt, &omr, 500_000.0, 181, 1, 1).unwrap();
    assert_eq!(k1.vattentyp.varden, k1b.vattentyp.varden);
    let (k2, _) = karta_for_ar(&k, &tt, &omr, 500_000.0, 182, 1, 1).unwrap();
    assert_ne!(k1.vattentyp.varden, k2.vattentyp.varden);
}

/// Prov 9: efter 290 000 år byter bara de två levande.
#[test]
fn prov9_doda_foror_byter_inte() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let lev = hckarta::levande_foror_nr(181, Period::Ovre);
    assert_eq!(lev.len(), 2);
    let byten = loppbyten(&k, &tt, &omr, 181, 1, 1).unwrap();
    for b in &byten {
        if b.ar <= 290_000.0 {
            assert!(
                lev.contains(&b.fara),
                "död fåra {} bytte vid ar={} (levande={:?})",
                b.fara,
                b.ar,
                lev
            );
        }
    }
    assert!(
        byten.iter().any(|b| b.ar <= 290_000.0),
        "förväntade minst ett byte efter 290000 för levande fåror"
    );
}

/// Prov 10: LASMIG märker Butler/Yamamura [A], takten 82–152 ka, 20–80 %, bonus 8 m, basen.
#[test]
fn prov10_markningen_lasmig() {
    let text =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"))
            .expect("LASMIG.md");
    assert!(text.contains("Butler"));
    assert!(text.contains("Yamamura") || text.contains("[A]"));
    assert!(text.contains("82 000") || text.contains("82000") || text.contains("82\u{00a0}000"));
    assert!(text.contains("152 000") || text.contains("152000"));
    assert!(text.contains("20") && text.contains("80"));
    assert!(
        text.contains("8 m") || text.contains("8\u{00a0}m") || text.contains("bonus"),
        "LASMIG ska nämna bonus 8 m"
    );
    assert!(text.contains("basen") || text.contains("1 081 000") || text.contains("1081000"));
    assert!(text.contains("eget val") || text.contains("**eget val**"));
}

/// Redovisning: tal för frö 181 (riktning 1 och 2) — skrivs till stdout vid testkörning.
#[test]
fn redovisning_fro181() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for rikning in [1u8, 2] {
        let byten = loppbyten(&k, &tt, &omr, 181, 1, rikning).unwrap();
        let mut per_fara_basen = [0u32; 5];
        let mut per_fara_980 = [0u32; 5];
        for b in &byten {
            if b.fara < 5 {
                per_fara_basen[b.fara] += 1;
                if b.ar <= 980_000.0 {
                    per_fara_980[b.fara] += 1;
                }
            }
        }
        println!("=== redovisning fro=181 flodriktning={rikning} ===");
        println!("byten per fåra basen→gränsen: {:?}", per_fara_basen);
        println!("byten per fåra 980000→gränsen: {:?}", per_fara_980);
        for b in &byten {
            println!("  byte ar={} fara={} punkt={:?}", b.ar, b.fara, b.punkt);
        }
        let andel8 = andel_nytt_lopp_pa_tidigare(&k, &tt, &omr, 1, 50, rikning, 8.0).unwrap();
        let andel0 = andel_nytt_lopp_pa_tidigare(&k, &tt, &omr, 1, 50, rikning, 0.0).unwrap();
        println!("andel nytt lopp på tidigare (bonus 8): {andel8:.6}");
        println!("andel nytt lopp på tidigare (bonus 0): {andel0:.6}");
        for t in [500_000.0, 20_000.0] {
            let (kart, _) = karta_for_ar(&k, &tt, &omr, t, 181, 1, rikning).unwrap();
            let mut rakna = [0usize; 6];
            for &v in &kart.vattentyp.varden {
                rakna[v as usize] += 1;
            }
            println!(
                "t={t}: Land={} Flodfara={} Trask={} Sjo={} Hav={}",
                rakna[Vatten::Land as usize],
                rakna[Vatten::Flodfara as usize],
                rakna[Vatten::Trask as usize],
                rakna[Vatten::Sjo as usize],
                rakna[Vatten::Hav as usize]
            );
        }
        let start = Instant::now();
        let _ = karta_for_ar(&k, &tt, &omr, 0.0, 181, 1, rikning).unwrap();
        println!("karta_for_ar(0) tid: {:?}", start.elapsed());
    }
    // Mutant-dokumentation (röda proven prövade manuellt i kopia — se rapporten):
    // 1) mellanrum 10 000 → prov2_takten rött
    // 2) bonus 0 → prov5 rött
    // 3) annan fåra tillåts ändras → prov3 rött
}

/// Hjälp: karta_full_version (tm1) oförändrad mot kontroll vid t=500000.
#[test]
fn smoke_karta_full_version_orord() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (a, _) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 1, 1).unwrap();
    assert_eq!(
        a.vattentyp
            .varden
            .iter()
            .filter(|&&v| v == Vatten::Flodfara)
            .count(),
        640
    );
}
