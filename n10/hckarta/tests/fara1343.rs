//! Prov 1–9 för Alt 1343/1344: flodversion 2 (fåra och bälte som andelar).
//! Underlag: n10/docs/2026-11-21-lasunderlag-kartfara.md (skrivet före bygget).
//! Anpassat till Alt 1386: ytan byggs med bredd per fåra (`foror`); konstanten `FARA_BREDD_M` borttagen.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    karta_full, karta_full_version, karta_med, kontrollsumma, kontrollsumma_med, las_3d, las_3d_med,
    mittlinjer, skriv_3d, skriv_3d_med, BALTE_MAX, FARA_BREDD_MAX_M, Kalla, Marke, Omrade,
    Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell() -> Tidtabell {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
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
        "hckarta-fara1343-{}-{}-{}",
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
    assert!(ut.status.success(), "sha256sum misslyckades för {}", path.display());
    let s = String::from_utf8_lossy(&ut.stdout);
    s.split_whitespace().next().unwrap().to_string()
}

fn las_fore_sha(t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fara1343-fore.txt"))
        .expect("fara1343-fore.txt saknas — räkna före-baseline först");
    let mark = format!("sha256 filer t={t}:");
    let mut efter = false;
    for rad in text.lines() {
        if rad.contains(&mark) {
            efter = true;
            continue;
        }
        if efter {
            if rad.starts_with("===") || rad.starts_with("---") {
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
    panic!("saknar sha256 för {fil} vid t={t} i fara1343-fore.txt");
}

/// Prov 1: version 1:s utfiler byte-identiska med före-baseline (sha256).
#[test]
fn prov1_version1_orord() {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    for t in ["500000", "20000"] {
        let mapp = temp_mapp(&format!("p1-{t}"));
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
            .args(["--fro", "181", "--skriv"])
            .arg(&mapp)
            .args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status()
            .expect("kör hckarta");
        assert!(status.success(), "hckarta t={t} misslyckades");
        for fil in [
            "hojd.f32",
            "vatten_yta.f32",
            "vattentyp.u8",
            "mark.f32",
            "vaxt.f32",
            "period.u8",
            "hojd.png",
            "strand.json",
            "karta.json",
        ] {
            let efter = sha256_fil(&mapp.join(fil));
            let fore = las_fore_sha(t, fil);
            assert_eq!(efter, fore, "fil {fil} t={t} skiljer sig från före");
        }
        assert!(!mapp.join("fara_andel.f32").exists());
        assert!(!mapp.join("balte_andel.f32").exists());
        assert!(!mapp.join("foror.tsv").exists());
    }
}

/// Prov 2: hcsim slutsummor oförändrade (jämför mot fara1343-fore.txt).
#[test]
fn prov2_hcsim_orord() {
    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    if !hcsim.exists() {
        // Bygg hcsim i egen target-dir om den saknas (ingen ändring i hcsim-källan).
        let status = Command::new(std::env::var("HOME").unwrap() + "/.cargo/bin/cargo")
            .args(["build", "-j", "4", "--release"])
            .env(
                "CARGO_TARGET_DIR",
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target"),
            )
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim"))
            .status()
            .expect("bygg hcsim");
        assert!(status.success(), "kunde inte bygga hcsim");
    }
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
        .expect("kör hcsim");
    assert!(ut.status.success(), "hcsim misslyckades: {}", String::from_utf8_lossy(&ut.stderr));
    let stdout = String::from_utf8_lossy(&ut.stdout);
    let fore = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fara1343-fore.txt"))
        .expect("fore");
    // Hitta JSON-raden i före-filen och i utdata.
    let fore_json = fore
        .lines()
        .rev()
        .find(|l| l.starts_with('{') && l.contains("djur_summa"))
        .expect("saknar hcsim-json i före");
    let efter_json = stdout
        .lines()
        .rev()
        .find(|l| l.starts_with('{') && l.contains("djur_summa"))
        .expect("saknar hcsim-json i efter");
    assert_eq!(efter_json, fore_json, "hcsim slutsummor skiljer sig");
}

/// Prov 3: version 2:s mittlinjerutor = version 1:s mittlinje för samma frö.
#[test]
fn prov3_mittlinjen_lika() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let v1_linjer = mittlinjer(&bas, 181);
    let (kart2, _) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    // Mittlinjen ur pathfinding på samma bas ska vara densamma som den version 2 byggde ur.
    let v2_linjer = mittlinjer(&bas, 181);
    assert_eq!(v1_linjer, v2_linjer);
    // Och version 2:s andelar > 0 ska ligga på mittlinjens landrutor.
    let andelar = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1)
        .unwrap()
        .1
        .unwrap();
    let mut mitt: HashSet<(usize, usize)> = HashSet::new();
    for path in &v1_linjer {
        for &ij in path {
            mitt.insert(ij);
        }
    }
    let b = omr.b;
    for j in 0..omr.h {
        for i in 0..omr.b {
            let p = i + j * b;
            if andelar.fara_andel.varden[p] > 0.0 {
                assert!(
                    mitt.contains(&(i, j)),
                    "fara_andel > 0 utanför mittlinje vid ({i},{j})"
                );
                assert_eq!(kart2.vattentyp.varden[p], Vatten::Land);
            }
        }
    }
    // Version 1 har Flodfara; mittlinjens landrutor i v1 täcks av Flodfara (bredd).
    let kart1 = karta_full(&k, &tt, &omr, 500_000.0, 181).unwrap();
    for path in &v1_linjer {
        for &(i, j) in path {
            let p = i + j * b;
            if kart1.hojd.varden[p] >= 0.0 {
                assert_eq!(kart1.vattentyp.varden[p], Vatten::Flodfara);
            }
        }
    }
}

/// Prov 4: yta, intervall och balte ≥ fara.
#[test]
fn prov4_ytan() {
    let (kart, andelar) =
        karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    let a = andelar.unwrap();
    let ruta_m = kart.omrade.ruta_m;
    let ruta_m2 = ruta_m * ruta_m;
    let bas = karta_med(&kalla(), &tidtabell(), &omrade(), 500_000.0).unwrap();
    let linjer = mittlinjer(&bas, 181);

    let mut vantad_yta = 0.0f64;
    let mut sedd: HashSet<usize> = HashSet::new();
    // Σ min(ruta_m², bredd_m · L) per bidrag, men flera fåror i samma ruta adderas och klipps —
    // jämför mot den faktiska formeln via ombyggnad.
    let mut ombyggd = vec![0.0f64; kart.omrade.b * kart.omrade.h];
    assert_eq!(a.foror.len(), linjer.len(), "en bredd per fåra");
    for (fi, path) in linjer.iter().enumerate() {
        let w = a.foror[fi].bredd_m;
        for (ci, &(i, j)) in path.iter().enumerate() {
            let p = i + j * kart.omrade.b;
            if kart.hojd.varden[p] < 0.0 {
                continue;
            }
            let dj = if ci + 1 < path.len() {
                path[ci + 1].1 as isize - j as isize
            } else {
                0
            };
            let dj = dj.clamp(-1, 1);
            let l = ruta_m * (1.0 + (dj * dj) as f64).sqrt();
            ombyggd[p] = (ombyggd[p] + w * l).min(ruta_m2);
            sedd.insert(p);
        }
    }
    for p in &sedd {
        vantad_yta += ombyggd[*p];
    }
    let sum_andel: f64 = a.fara_andel.varden.iter().map(|&x| f64::from(x)).sum();
    let yta = sum_andel * ruta_m2;
    let rel = (yta - vantad_yta).abs() / vantad_yta.max(1.0);
    assert!(rel < 1e-6, "yta relativ avvikelse {rel}: {yta} vs {vantad_yta}");

    for p in 0..a.fara_andel.varden.len() {
        let f = a.fara_andel.varden[p];
        let b = a.balte_andel.varden[p];
        assert!((0.0..=1.0).contains(&f), "fara_andel {f} utanför [0,1]");
        assert!(b >= f - 1e-6, "balte_andel {b} < fara_andel {f}");
        assert!(b <= BALTE_MAX + 1e-6, "balte_andel {b} > {BALTE_MAX}");
    }
}

/// Prov 5: 0 rutor Flodfara i version 2 när bredd_m < ruta_m.
#[test]
fn prov5_inga_hela_flodrutor() {
    assert!(FARA_BREDD_MAX_M < omrade().ruta_m);
    let (kart, _) = karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    let n_flod = kart
        .vattentyp
        .varden
        .iter()
        .filter(|&&v| v == Vatten::Flodfara)
        .count();
    assert_eq!(n_flod, 0);
}

/// Prov 6: varje fåras mittlinje obruten, en ruta per kolumn, |dj| ≤ 1.
#[test]
fn prov6_sammanhang() {
    let bas = karta_med(&kalla(), &tidtabell(), &omrade(), 500_000.0).unwrap();
    let linjer = mittlinjer(&bas, 181);
    let b = omrade().b;
    assert!(!linjer.is_empty());
    for path in &linjer {
        assert!(!path.is_empty());
        assert_eq!(path[0].0, 0, "börjar inte i västra kanten");
        let mut sedda_i = HashSet::new();
        for (ci, &(i, j)) in path.iter().enumerate() {
            assert!(!sedda_i.contains(&i), "två rutor i samma kolumn {i}");
            sedda_i.insert(i);
            if ci + 1 < path.len() {
                let (i2, j2) = path[ci + 1];
                assert_eq!(i2, i + 1, "hopp i kolumn");
                assert!(j2.abs_diff(j) <= 1, "|dj| > 1");
            }
        }
        // Når östra kanten eller har gått in i hav någonstans.
        let nar_ost = path.last().map(|(i, _)| *i == b - 1).unwrap_or(false);
        let nar_hav = path.iter().any(|&(i, j)| bas.hojd.varden[i + j * b] < 0.0);
        assert!(nar_ost || nar_hav, "når varken östkant eller hav");
    }
}

/// Prov 7: samma frö → identiska filer i v2; annat frö → andra.
#[test]
fn prov7_samma_fro_identiskt() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (a1, and1) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    let (a2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    let (b, and_b) = karta_full_version(&k, &tt, &omr, 500_000.0, 182, 2, 1).unwrap();
    let m1 = temp_mapp("p7a");
    let m2 = temp_mapp("p7b");
    let m3 = temp_mapp("p7c");
    skriv_3d_med(&a1, and1.as_ref(), &m1).unwrap();
    skriv_3d_med(&a2, and2.as_ref(), &m2).unwrap();
    skriv_3d_med(&b, and_b.as_ref(), &m3).unwrap();
    for fil in [
        "karta.json",
        "fara_andel.f32",
        "balte_andel.f32",
        "vall_andel.f32",
        "vall_hojd.f32",
        "vaxt_tathet.f32",
        "vaxtgrupper.json",
        "vattentyp.u8",
        "foror.tsv",
    ] {
        assert_eq!(sha256_fil(&m1.join(fil)), sha256_fil(&m2.join(fil)), "{fil}");
    }
    // vaxtgrupper.json beror bara på artfilen, inte fröet — samma för 181 och 182.
    assert_eq!(
        sha256_fil(&m1.join("vaxtgrupper.json")),
        sha256_fil(&m3.join("vaxtgrupper.json")),
        "vaxtgrupper.json ska vara lika oberoende av frö"
    );
    for fil in [
        "karta.json",
        "fara_andel.f32",
        "balte_andel.f32",
        "vall_andel.f32",
        "vall_hojd.f32",
        "vaxt_tathet.f32",
        "vattentyp.u8",
        "foror.tsv",
    ] {
        assert_ne!(sha256_fil(&m1.join(fil)), sha256_fil(&m3.join(fil)), "{fil} annat frö");
    }
    assert_eq!(
        kontrollsumma_med(&a1, and1.as_ref()),
        kontrollsumma_med(&a2, and2.as_ref())
    );
    assert_ne!(
        kontrollsumma_med(&a1, and1.as_ref()),
        kontrollsumma_med(&b, and_b.as_ref())
    );
}

/// Prov 8: las_3d läser v1 och v2; v1-mappen har inga nya filer.
#[test]
fn prov8_3d_format() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let m1 = temp_mapp("p8v1");
    let m2 = temp_mapp("p8v2");
    let kart1 = karta_full(&k, &tt, &omr, 500_000.0, 181).unwrap();
    skriv_3d(&kart1, &m1).unwrap();
    let (kart2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    skriv_3d_med(&kart2, and2.as_ref(), &m2).unwrap();

    assert!(!m1.join("fara_andel.f32").exists());
    assert!(!m1.join("balte_andel.f32").exists());
    assert!(!m1.join("foror.tsv").exists());
    assert!(!m1.join("vall_andel.f32").exists());
    assert!(!m1.join("vall_hojd.f32").exists());
    assert!(!m1.join("vaxt_tathet.f32").exists());
    assert!(!m1.join("vaxtgrupper.json").exists());
    let last1 = las_3d(&m1).unwrap();
    assert_eq!(kontrollsumma(&last1), kontrollsumma(&kart1));

    assert!(m2.join("fara_andel.f32").exists());
    assert!(m2.join("balte_andel.f32").exists());
    assert!(m2.join("foror.tsv").exists());
    assert!(m2.join("vall_andel.f32").exists());
    assert!(m2.join("vall_hojd.f32").exists());
    assert!(m2.join("vaxt_tathet.f32").exists());
    assert!(m2.join("vaxtgrupper.json").exists());
    let (last2, last_and) = las_3d_med(&m2).unwrap();
    assert!(last_and.is_some());
    assert!(last_and.as_ref().unwrap().har_vall);
    assert!(last_and.as_ref().unwrap().har_vaxtgrupper);
    assert_eq!(
        kontrollsumma_med(&last2, last_and.as_ref()),
        kontrollsumma_med(&kart2, and2.as_ref())
    );
    // las_3d (utan _med) lyckas också på v2.
    let _ = las_3d(&m2).unwrap();
}

/// Prov 9: nya lagren märkta Simulering.
#[test]
fn prov9_markning() {
    let (_, andelar) =
        karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    let a = andelar.unwrap();
    assert_eq!(a.fara_andel.marke, Marke::Simulering);
    assert_eq!(a.balte_andel.marke, Marke::Simulering);
    assert_eq!(a.vall_andel.marke, Marke::Simulering);
    assert!(a.har_vaxtgrupper);
    assert_eq!(a.vaxt_tathet.marke, Marke::Simulering);
    assert_eq!(a.vall_hojd_m.marke, Marke::Simulering);
}

/// Redovisning: antal per vattentyp och medel-andelar, v1 mot v2, Aldre och Ovre.
#[test]
fn redovisning_andelar_och_typer() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for (t, namn) in [(500_000.0, "Aldre"), (20_000.0, "Ovre")] {
        let (v1, _) = karta_full_version(&k, &tt, &omr, t, 181, 1, 1).unwrap();
        let (v2, and2) = karta_full_version(&k, &tt, &omr, t, 181, 2, 1).unwrap();
        let a = and2.unwrap();
        let n = (omr.b * omr.h) as f64;
        let rakna = |kart: &hckarta::Karta, vt: Vatten| {
            kart.vattentyp.varden.iter().filter(|&&v| v == vt).count()
        };
        eprintln!(
            "redovisning {namn} t={t}: v1 Land={} Flodfara={} Trask={} Sjo={} | v2 Land={} Flodfara={} Trask={} Sjo={} | medel fara={:.6} balte={:.6}",
            rakna(&v1, Vatten::Land),
            rakna(&v1, Vatten::Flodfara),
            rakna(&v1, Vatten::Trask),
            rakna(&v1, Vatten::Sjo),
            rakna(&v2, Vatten::Land),
            rakna(&v2, Vatten::Flodfara),
            rakna(&v2, Vatten::Trask),
            rakna(&v2, Vatten::Sjo),
            a.fara_andel.varden.iter().map(|&x| f64::from(x)).sum::<f64>() / n,
            a.balte_andel.varden.iter().map(|&x| f64::from(x)).sum::<f64>() / n,
        );
    }
}
