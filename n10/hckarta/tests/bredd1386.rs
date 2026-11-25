//! Prov 1–10 för Alt 1386: bredd per fåra i flodversion 2.
//! Underlag: n10/docs/2026-11-21-lasunderlag-kartbredd.md (skrivet före bygget).

use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    fara_bredd_m, karta_full, karta_full_version, karta_med, kontrollsumma, kontrollsumma_med,
    las_3d, las_3d_med, mittlinjer, skriv_3d, skriv_3d_med, BALTE_FAKTOR, BALTE_MAX,
    FARA_BREDD_MAX_M, FARA_BREDD_MIN_M, Kalla, Marke, Omrade, Tidtabell, Vatten,
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
        "hckarta-bredd1386-{}-{}-{}",
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
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bredd1386-fore.txt"))
        .expect("bredd1386-fore.txt saknas — räkna före-baseline först");
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
    panic!("saknar sha256 för {fil} vid t={t} i bredd1386-fore.txt");
}

fn ref_data(namn: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(namn)
}

/// Prov 1: version 1:s utfiler byte-identiska med före-baseline.
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

/// Prov 2: hcsim slutsummor oförändrade.
#[test]
fn prov2_hcsim_orord() {
    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    if !hcsim.exists() {
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
    assert!(
        ut.status.success(),
        "hcsim misslyckades: {}",
        String::from_utf8_lossy(&ut.stderr)
    );
    let stdout = String::from_utf8_lossy(&ut.stdout);
    let fore =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bredd1386-fore.txt"))
            .expect("fore");
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

/// Prov 3: mittlinjer, träsk och sjöar oförändrade mot före-baseline (vattentyp + vatten_yta + mittlinjemask).
#[test]
fn prov3_mittlinjer_oforandrade() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in [181u64, 182, 183] {
        for t in [500_000.0, 20_000.0] {
            let t_i = t as i64;
            let (kart, and) = karta_full_version(&k, &tt, &omr, t, fro, 2, 1).unwrap();
            let a = and.unwrap();
            let vt_fore = std::fs::read(ref_data(&format!(
                "bredd1386-fore-v2-f{fro}-t{t_i}-vattentyp.u8"
            )))
            .expect("vattentyp-före");
            let vy_fore = std::fs::read(ref_data(&format!(
                "bredd1386-fore-v2-f{fro}-t{t_i}-vatten_yta.f32"
            )))
            .expect("vatten_yta-före");
            let mitt_fore = std::fs::read(ref_data(&format!(
                "bredd1386-fore-v2-f{fro}-t{t_i}-mittlinje.u8"
            )))
            .expect("mittlinje-före");
            let vt_efter: Vec<u8> = kart.vattentyp.varden.iter().map(|v| *v as u8).collect();
            assert_eq!(vt_efter, vt_fore, "vattentyp fro={fro} t={t}");
            let mut vy_bytes = Vec::with_capacity(kart.vatten_yta.varden.len() * 4);
            for v in &kart.vatten_yta.varden {
                vy_bytes.extend_from_slice(&v.to_le_bytes());
            }
            assert_eq!(vy_bytes, vy_fore, "vatten_yta fro={fro} t={t}");
            let mitt_efter: Vec<u8> = a
                .fara_andel
                .varden
                .iter()
                .map(|&x| if x > 0.0 { 1 } else { 0 })
                .collect();
            assert_eq!(mitt_efter, mitt_fore, "mittlinje-mask fro={fro} t={t}");
            // Pathfinding-mittlinjer lika med version 1:s lista.
            let bas = karta_med(&k, &tt, &omr, t).unwrap();
            let linjer = mittlinjer(&bas, fro);
            assert_eq!(linjer.len(), a.foror.len());
        }
    }
}

/// Prov 4: varje w_f i [100, 320]; samma frö → samma bredder; b_f = 4 · w_f.
#[test]
fn prov4_granser() {
    for fro in [1u64, 181, 999] {
        let (_, and) =
            karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, fro, 2, 1).unwrap();
        let a = and.unwrap();
        assert!(!a.foror.is_empty());
        for (i, f) in a.foror.iter().enumerate() {
            assert!(
                (FARA_BREDD_MIN_M..=FARA_BREDD_MAX_M).contains(&f.bredd_m),
                "bredd {} utanför [100,320]",
                f.bredd_m
            );
            assert!((f.balte_m - BALTE_FAKTOR * f.bredd_m).abs() < 1e-9);
            let w2 = fara_bredd_m(fro, i);
            assert!((w2 - f.bredd_m).abs() < 1e-12, "samma frö andra bredd");
        }
        let (_, and2) =
            karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, fro, 2, 1).unwrap();
        assert_eq!(a.foror, and2.unwrap().foror);
    }
}

/// Prov 5: över fröna 1–200 finns bredder under 150 och över 250; median i 160–220.
#[test]
fn prov5_spridning() {
    let mut alla = Vec::new();
    for fro in 1u64..=200 {
        for t in [500_000.0, 20_000.0] {
            let (_, and) =
                karta_full_version(&kalla(), &tidtabell(), &omrade(), t, fro, 2, 1).unwrap();
            for f in and.unwrap().foror {
                alla.push(f.bredd_m);
            }
        }
    }
    assert!(alla.iter().any(|&w| w < 150.0), "saknar bredd < 150");
    assert!(alla.iter().any(|&w| w > 250.0), "saknar bredd > 250");
    alla.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = alla[alla.len() / 2];
    assert!(
        (160.0..=220.0).contains(&med),
        "median {med} utanför 160–220"
    );
}

/// Prov 6: handräkning — rak w=100 → 0,05/0,2; diagonal w=320 → 0,2263 / 0,85 efter tak.
#[test]
fn prov6_handrakning() {
    let ruta_m = 2000.0;
    let ruta_m2 = ruta_m * ruta_m;
    // Rak (dj = 0), w = 100.
    let l0 = ruta_m * (1.0_f64).sqrt();
    let fa0 = (100.0 * l0 / ruta_m2).min(1.0);
    let ba0 = (400.0 * l0 / ruta_m2).min(BALTE_MAX as f64);
    assert!((fa0 - 0.05).abs() < 1e-12);
    assert!((ba0 - 0.2).abs() < 1e-12);
    // Diagonal (dj = ±1), w = 320, b = 1280.
    let l1 = ruta_m * (2.0_f64).sqrt();
    let fa1 = 320.0 * l1 / ruta_m2;
    let ba1_fore = 1280.0 * l1 / ruta_m2;
    let ba1 = ba1_fore.min(f64::from(BALTE_MAX));
    assert!((fa1 - 0.2263).abs() < 1e-4, "fa1={fa1}");
    assert!((ba1_fore - 0.9051).abs() < 1e-4, "ba1_fore={ba1_fore}");
    // Taket är `BALTE_MAX` (f32 0,85); efter klipp = exakt den.
    assert_eq!(ba1, f64::from(BALTE_MAX));
    assert!((0.85 - f64::from(BALTE_MAX)).abs() < 1e-7);
}

/// Prov 7: i varje ruta 0 ≤ fara ≤ balte ≤ 0,85 och fara ≤ 1; flera fåror klipps.
#[test]
fn prov7_tak_och_ordning() {
    for fro in [181u64, 182, 183] {
        for t in [500_000.0, 20_000.0] {
            let (_, and) =
                karta_full_version(&kalla(), &tidtabell(), &omrade(), t, fro, 2, 1).unwrap();
            let a = and.unwrap();
            for p in 0..a.fara_andel.varden.len() {
                let f = a.fara_andel.varden[p];
                let b = a.balte_andel.varden[p];
                assert!(f >= 0.0 && f <= 1.0 + 1e-6);
                assert!(b >= f - 1e-6);
                assert!(b <= BALTE_MAX + 1e-6);
            }
        }
    }
}

/// Prov 8: foror.tsv en rad per fåra; talen lika med kartans; version 1 har ingen fil.
#[test]
fn prov8_filen() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let m1 = temp_mapp("p8v1");
    let m2 = temp_mapp("p8v2");
    let kart1 = karta_full(&k, &tt, &omr, 500_000.0, 181).unwrap();
    skriv_3d(&kart1, &m1).unwrap();
    assert!(!m1.join("foror.tsv").exists());

    let (kart2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    let a = and2.as_ref().unwrap();
    skriv_3d_med(&kart2, and2.as_ref(), &m2).unwrap();
    assert!(m2.join("foror.tsv").exists());
    let text = std::fs::read_to_string(m2.join("foror.tsv")).unwrap();
    let rader: Vec<&str> = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("fara"))
        .collect();
    assert_eq!(rader.len(), a.foror.len());
    assert!(!rader.is_empty());
    for (i, rad) in rader.iter().enumerate() {
        let d: Vec<&str> = rad.split('\t').collect();
        // Alt 1371: foror.tsv har även vall_m (5 kolumner).
        assert_eq!(d.len(), 5);
        let bredd: f64 = d[1].parse().unwrap();
        let balte: f64 = d[2].parse().unwrap();
        let rutor: usize = d[3].parse().unwrap();
        let vall: f64 = d[4].parse().unwrap();
        assert!((bredd - a.foror[i].bredd_m).abs() < 1e-9);
        assert!((balte - a.foror[i].balte_m).abs() < 1e-9);
        assert_eq!(rutor, a.foror[i].rutor);
        assert!((vall - a.foror[i].vall_m).abs() < 1e-9);
    }
    let json = std::fs::read_to_string(m2.join("karta.json")).unwrap();
    assert!(json.contains("foror.tsv"));
}

/// Prov 9: las_3d läser v1, ny v2 och Alt 1343:s v2 (utan foror.tsv).
#[test]
fn prov9_3d_format() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let m1 = temp_mapp("p9v1");
    let m2 = temp_mapp("p9v2");
    let kart1 = karta_full(&k, &tt, &omr, 500_000.0, 181).unwrap();
    skriv_3d(&kart1, &m1).unwrap();
    let _ = las_3d(&m1).unwrap();

    let (kart2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    skriv_3d_med(&kart2, and2.as_ref(), &m2).unwrap();
    let (l2, a2) = las_3d_med(&m2).unwrap();
    assert!(!a2.as_ref().unwrap().foror.is_empty());
    assert_eq!(
        kontrollsumma_med(&l2, a2.as_ref()),
        kontrollsumma_med(&kart2, and2.as_ref())
    );

    let alt = ref_data("bredd1386-alt1343-v2");
    assert!(!alt.join("foror.tsv").exists());
    let (l_alt, a_alt) = las_3d_med(&alt).unwrap();
    assert!(a_alt.as_ref().unwrap().foror.is_empty());
    let _ = kontrollsumma_med(&l_alt, a_alt.as_ref());
    let _ = las_3d(&alt).unwrap();
    let _ = kontrollsumma(&l_alt);
}

/// Prov 10: märkning Simulering; LASMIG anger 100/320 med [A], faktor 4 och fördelning som eget val;
/// ingen kvarvarande Butler-115-rad.
#[test]
fn prov10_markning() {
    let (_, and) =
        karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    let a = and.unwrap();
    assert_eq!(a.fara_andel.marke, Marke::Simulering);
    assert_eq!(a.balte_andel.marke, Marke::Simulering);
    assert!(a.har_vall);
    assert_eq!(a.vall_andel.marke, Marke::Simulering);
    assert!(a.har_vaxtgrupper);
    assert_eq!(a.vaxt_tathet.marke, Marke::Simulering);

    let lasmig = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"))
        .expect("LASMIG");
    assert!(
        !lasmig.contains("115 m"),
        "Butler 115 m-raden ska vara borttagen"
    );
    assert!(lasmig.contains("100 m"), "saknar 100 m");
    assert!(lasmig.contains("320 m"), "saknar 320 m");
    assert!(lasmig.contains("Butler 1980 [A]"));
    assert!(lasmig.contains("Hasler och Renne 2025 [A]"));
    assert!(
        !lasmig.to_lowercase().contains("320 m")
            || lasmig.lines().any(|r| {
                r.contains("320")
                    && (r.contains("Simulering") || r.contains("eget val") || r.contains("[A]"))
            }),
        "320 m ska vara märkt Simulering/[A]/eget val"
    );
    assert!(
        lasmig.lines().any(|r| {
            r.contains("BALTE_FAKTOR") && (r.contains("eget val") || r.contains("Simulering"))
        }),
        "faktorn 4 ska vara eget val / Simulering"
    );
    assert!(!lasmig.to_lowercase().contains("uppmätt"));
    assert!(lasmig.contains("eget val"));
    assert!(lasmig.contains("jämn i logaritmen") || lasmig.contains("jamn i logaritmen"));
}

/// Redovisning: bredder, medel-andelar, spridning över frö 1–200.
#[test]
fn redovisning() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let n = (omr.b * omr.h) as f64;
    for (t, namn) in [(500_000.0, "Aldre"), (20_000.0, "Ovre")] {
        let (v2, and2) = karta_full_version(&k, &tt, &omr, t, 181, 2, 1).unwrap();
        let a = and2.unwrap();
        let rakna = |vt: Vatten| v2.vattentyp.varden.iter().filter(|&&v| v == vt).count();
        eprintln!("=== redovisning {namn} t={t} fro=181 ===");
        for (i, f) in a.foror.iter().enumerate() {
            eprintln!(
                "  fåra {i}: bredd_m={:.6} balte_m={:.6} rutor={}",
                f.bredd_m, f.balte_m, f.rutor
            );
        }
        eprintln!(
            "  Land={} Flodfara={} Trask={} Sjo={} | medel fara={:.6} balte={:.6} | summa={}",
            rakna(Vatten::Land),
            rakna(Vatten::Flodfara),
            rakna(Vatten::Trask),
            rakna(Vatten::Sjo),
            a.fara_andel.varden.iter().map(|&x| f64::from(x)).sum::<f64>() / n,
            a.balte_andel.varden.iter().map(|&x| f64::from(x)).sum::<f64>() / n,
            kontrollsumma_med(&v2, Some(&a)),
        );
    }
    let mut alla = Vec::new();
    for fro in 1u64..=200 {
        for t in [500_000.0, 20_000.0] {
            let (_, and) = karta_full_version(&k, &tt, &omr, t, fro, 2, 1).unwrap();
            for f in and.unwrap().foror {
                alla.push(f.bredd_m);
            }
        }
    }
    alla.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = *alla.first().unwrap();
    let max = *alla.last().unwrap();
    let med = alla[alla.len() / 2];
    eprintln!("=== spridning frö 1–200 (båda perioder): min={min:.6} median={med:.6} max={max:.6} n={} ===", alla.len());
}
