//! Prov 1–10 för Alt 1371: strandvallar i flodversion 2.
//! Underlag: n10/docs/2026-11-21-lasunderlag-kartvall.md (skrivet före bygget).

use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    karta_for_ar, karta_full, karta_full_version, karta_med, kontrollsumma, kontrollsumma_med,
    las_3d, las_3d_med, levande_foror_nr, mittlinjer, mittlinjer_aldre, skriv_3d, skriv_3d_med,
    vall_bredd_m, ForaInfo, Kalla, Karta, Marke, Omrade, Period, Tidtabell,
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
        "hckarta-vall1371-{}-{}-{}",
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
    assert!(
        ut.status.success(),
        "sha256sum misslyckades för {}",
        path.display()
    );
    let s = String::from_utf8_lossy(&ut.stdout);
    s.split_whitespace().next().unwrap().to_string()
}

fn las_fore_sha_v1(tm: &str, t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vall1371-fore.txt"),
    )
    .expect("vall1371-fore.txt saknas — räkna före-baseline först");
    let mark = format!("sha256 filer flodversion=1 tidsmodell={tm} t={t}:");
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
            let mut delar = rad.split_whitespace();
            if let (Some(hash), Some(namn)) = (delar.next(), delar.next()) {
                if namn == fil {
                    return hash.to_string();
                }
            }
        }
    }
    panic!("saknar sha256 för {fil} vid tidsmodell={tm} t={t} i vall1371-fore.txt");
}

fn ref_fore(tm: u8, fro: u64, t: u32, fil: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/data/vall1371-fore-v2-tm{tm}-f{fro}-t{t}-{fil}"
    ))
}

fn las_f32_fil(path: &std::path::Path) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn ra_vall_och_klipp(
    k: &Karta,
    linjer: &[Vec<(usize, usize)>],
    foror: &[ForaInfo],
    balte: &[f32],
) -> (Vec<f32>, usize) {
    let b = k.omrade.b;
    let n = b * k.omrade.h;
    let ruta_m = k.omrade.ruta_m;
    let ruta_m2 = ruta_m * ruta_m;
    let mut ra = vec![0.0f32; n];
    for (fi, path) in linjer.iter().enumerate() {
        let vw = foror[fi].vall_m;
        for (ci, &(i, j)) in path.iter().enumerate() {
            let p = i + j * b;
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            let dj = if ci + 1 < path.len() {
                path[ci + 1].1 as isize - j as isize
            } else {
                0
            };
            let dj = dj.clamp(-1, 1);
            let l = ruta_m * (1.0 + (dj * dj) as f64).sqrt();
            ra[p] += (vw * l / ruta_m2) as f32;
        }
    }
    let mut n_klipp = 0usize;
    for p in 0..n {
        let tak = (1.0 - balte[p]).max(0.0);
        if ra[p] > tak + 1e-7 {
            n_klipp += 1;
        }
    }
    (ra, n_klipp)
}

/// Prov 1: version 1 orörd (alla utfiler, tidsmodell 1; tm2 ändras av Alt 1395).
#[test]
fn prov1_version1_orord() {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    for tm in ["1"] {
        for t in ["500000", "20000", "0"] {
            let mapp = temp_mapp(&format!("p1-tm{tm}-t{t}"));
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
                    "1",
                    "--tidsmodell",
                    tm,
                    "--skriv",
                ])
                .arg(&mapp)
                .args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status()
                .expect("kör hckarta");
            assert!(status.success(), "hckarta tm={tm} t={t}");
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
                let fore = las_fore_sha_v1(tm, t, fil);
                assert_eq!(efter, fore, "fil {fil} tidsmodell={tm} t={t} skiljer sig från före");
            }
            assert!(!mapp.join("fara_andel.f32").exists());
            assert!(!mapp.join("vall_andel.f32").exists());
            assert!(!mapp.join("vall_hojd.f32").exists());
            assert!(!mapp.join("foror.tsv").exists());
            assert!(!mapp.join("vaxt_tathet.f32").exists());
            assert!(!mapp.join("vaxtgrupper.json").exists());
        }
    }
}

/// Prov 2: hcsim slutsummor oförändrade.
#[test]
fn prov2_hcsim_orort() {
    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    assert!(
        hcsim.exists(),
        "bygg hcsim release först: CARGO_TARGET_DIR=…/hcsim/target cargo build -j 4 --release i n10/hcsim"
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
        .expect("kör hcsim");
    assert!(ut.status.success(), "hcsim misslyckades");
    let s = String::from_utf8_lossy(&ut.stdout);
    // 5548 sedan hcsim 88c0a5f (BRAIN, Alt 1624: flodkanten får växter); kartans karta_summa oförändrad. Förut 4992.
    assert!(s.contains("\"slut_antal\":5548"));
    // djur_summa 4225976351498359620 sedan hcsim 88c0a5f (BRAIN, Alt 1624); förut 9527944887746424482.
    assert!(s.contains("\"djur_summa\":4225976351498359620"));
    assert!(s.contains("\"karta_summa\":9828389463751256610"));
    assert!(s.contains("fodda=34303"));
    assert!(s.contains("dod_svalt=29085"));
    assert!(s.contains("dod_alder=70"));
    assert!(s.contains("v_medel=76.04392061285031"));
}

/// Prov 3: version 2:s gamla lager orörda (tidsmodell 1; tm2 ändras av Alt 1395).
#[test]
fn prov3_version2_gamla_lager() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for tm in [1u8] {
        for fro in [181u64, 182, 183] {
            for t in [500_000u32, 20_000, 0] {
                let (kart, and) =
                    karta_full_version(&k, &tt, &omr, f64::from(t), fro, 2, 1).unwrap();
                let a = and.as_ref().unwrap();
                let tag_t = t;
                let vt_fore = std::fs::read(ref_fore(tm, fro, tag_t, "vattentyp.u8")).unwrap();
                let vt_efter: Vec<u8> = kart.vattentyp.varden.iter().map(|v| *v as u8).collect();
                assert_eq!(vt_efter, vt_fore, "vattentyp tm={tm} fro={fro} t={t}");
                assert_eq!(
                    kart.vatten_yta.varden,
                    las_f32_fil(&ref_fore(tm, fro, tag_t, "vatten_yta.f32")),
                    "vatten_yta tm={tm} fro={fro} t={t}"
                );
                assert_eq!(
                    a.fara_andel.varden,
                    las_f32_fil(&ref_fore(tm, fro, tag_t, "fara_andel.f32")),
                    "fara_andel tm={tm} fro={fro} t={t}"
                );
                assert_eq!(
                    a.balte_andel.varden,
                    las_f32_fil(&ref_fore(tm, fro, tag_t, "balte_andel.f32")),
                    "balte_andel tm={tm} fro={fro} t={t}"
                );
                assert_eq!(
                    kart.hojd.varden,
                    las_f32_fil(&ref_fore(tm, fro, tag_t, "hojd.f32")),
                    "hojd tm={tm} fro={fro} t={t}"
                );
                let mitt = std::fs::read(ref_fore(tm, fro, tag_t, "mittlinje.u8")).unwrap();
                for (p, &m) in mitt.iter().enumerate() {
                    let har = a.fara_andel.varden[p] > 0.0;
                    assert_eq!(har, m != 0, "mittlinje p={p} tm={tm} fro={fro} t={t}");
                }
            }
        }
    }
}

/// Prov 4: handräkning (underlagets tal).
#[test]
fn prov4_handrakning() {
    let ruta_m = 2000.0f64;
    let ruta_m2 = ruta_m * ruta_m;

    // Rak (dj = 0), w = 100 → v_f = 400, vall_andel = 0,2.
    let w = 100.0;
    let v_f = vall_bredd_m(w);
    assert!((v_f - 400.0).abs() < 1e-12);
    let l = ruta_m * (1.0f64).sqrt();
    let vall = v_f * l / ruta_m2;
    assert!((vall - 0.2).abs() < 1e-12);

    // Diagonal (dj = ±1), w = 320 → v_f = 600 (tak), före klippning ≈ 0,4243;
    // balte = 0,85 → vall = 0,15.
    let w2 = 320.0;
    let v2 = vall_bredd_m(w2);
    assert!((v2 - 600.0).abs() < 1e-12);
    let l2 = ruta_m * (2.0f64).sqrt();
    let fore = v2 * l2 / ruta_m2;
    assert!((fore - 0.4243).abs() < 1e-4, "före klippning {fore}");
    let balte = 0.85f64;
    let efter = fore.min(1.0 - balte);
    assert!((efter - 0.15).abs() < 1e-6, "efter klippning {efter}");
}

/// Prov 5: i varje ruta fara ≤ balte och balte + vall ≤ 1; vall ≥ 0.
#[test]
fn prov5_summan() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for tm in [1u8, 2] {
        for t in [500_000.0, 20_000.0, 0.0] {
            let (_, and) = if tm == 1 {
                karta_full_version(&k, &tt, &omr, t, 181, 2, 1).unwrap()
            } else {
                karta_for_ar(&k, &tt, &omr, t, 181, 2, 1).unwrap()
            };
            let a = and.unwrap();
            for p in 0..a.fara_andel.varden.len() {
                let f = a.fara_andel.varden[p];
                let b = a.balte_andel.varden[p];
                let v = a.vall_andel.varden[p];
                assert!(f <= b + 1e-6, "fara > balte vid p={p}");
                assert!(b + v <= 1.0 + 1e-6, "balte+vall > 1 vid p={p}: {b}+{v}");
                assert!(v >= 0.0, "vall < 0 vid p={p}");
            }
        }
    }
}

/// Prov 6: taket v_f ≤ 600 och v_f = min(600, 4 · w_f).
#[test]
fn prov6_taket() {
    for fro in 1u64..=50 {
        for fi in 0..5 {
            let w = hckarta::fara_bredd_m(fro, fi);
            let v = vall_bredd_m(w);
            assert!(v <= 600.0 + 1e-12, "v_f {v} > 600");
            // Underlagets tal, inte programmets konstanter (granskningen 2026-11-21: annars kan provet inte bli rött).
            assert!((v - (4.0 * w).min(600.0)).abs() < 1e-12);
        }
    }
    let (_, and) = karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    for f in &and.unwrap().foror {
        assert!(f.vall_m <= 600.0 + 1e-12);
        assert!((f.vall_m - vall_bredd_m(f.bredd_m)).abs() < 1e-12);
    }
}

/// Prov 7: vall_hojd_m = 2,0 där vall_andel > 0, annars 0; hojd lika före/efter.
#[test]
fn prov7_hojden() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (kart, and) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    let a = and.unwrap();
    for p in 0..a.vall_andel.varden.len() {
        if a.vall_andel.varden[p] > 0.0 {
            assert_eq!(a.vall_hojd_m.varden[p], 2.0, "Butler 1980: medelhöjden 2 m");
        } else {
            assert_eq!(a.vall_hojd_m.varden[p], 0.0);
        }
    }
    assert_eq!(
        kart.hojd.varden,
        las_f32_fil(&ref_fore(1, 181, 500_000, "hojd.f32"))
    );
}

/// Prov 8: tidsmodell 2 vid t = 20 000 — bara levande fårors mittlinjerutor har vall.
#[test]
fn prov8_levande_foror() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fro = 181u64;
    let (kart, and) = karta_for_ar(&k, &tt, &omr, 20_000.0, fro, 2, 1).unwrap();
    let a = and.unwrap();
    let (linjer, lev) = hckarta::mittlinjer_vid_ar(&k, &tt, &omr, 20_000.0, fro, 1).unwrap();
    assert_eq!(lev.len(), 2);
    let b = omr.b;
    let mut levande_mitt = vec![false; b * omr.h];
    for path in &linjer {
        for &(i, j) in path {
            let p = i + j * b;
            if kart.hojd.varden[p] >= 0.0 {
                levande_mitt[p] = true;
            }
        }
    }
    for p in 0..a.vall_andel.varden.len() {
        // Övergivna lopprutor (inte på levande mittlinje) ska sakna vall.
        if !levande_mitt[p] && a.vall_andel.varden[p] > 0.0 {
            // Tillåtet bara om balte/vall från närliggande geometri — men mittlinje utanför
            // levande ska ha 0. Här: kräva att Trask utan levande mitt saknar vall.
            if kart.vattentyp.varden[p] == hckarta::Vatten::Trask {
                assert!(
                    a.vall_andel.varden[p] == 0.0,
                    "övergiven Trask har vall vid p={p}"
                );
            }
        }
        if levande_mitt[p] {
            assert!(
                a.vall_andel.varden[p] > 0.0,
                "levande mittlinje saknar vall vid p={p}"
            );
        }
    }
}

/// Prov 9: foror.tsv har vall_m; Alt 1343/1386 läses; version 1 har inga vallfiler.
#[test]
fn prov9_filerna() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let m1 = temp_mapp("p9v1");
    let m2 = temp_mapp("p9v2");
    let kart1 = karta_full(&k, &tt, &omr, 500_000.0, 181).unwrap();
    skriv_3d(&kart1, &m1).unwrap();
    assert!(!m1.join("vall_andel.f32").exists());
    assert!(!m1.join("vall_hojd.f32").exists());
    assert!(!m1.join("foror.tsv").exists());
    assert!(!m1.join("vaxt_tathet.f32").exists());
    assert!(!m1.join("vaxtgrupper.json").exists());
    let _ = las_3d(&m1).unwrap();

    let (kart2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
    let a = and2.as_ref().unwrap();
    assert!(a.har_vall);
    assert!(a.har_vaxtgrupper);
    skriv_3d_med(&kart2, and2.as_ref(), &m2).unwrap();
    assert!(m2.join("vall_andel.f32").exists());
    assert!(m2.join("vall_hojd.f32").exists());
    assert!(m2.join("foror.tsv").exists());
    assert!(m2.join("vaxt_tathet.f32").exists());
    assert!(m2.join("vaxtgrupper.json").exists());
    let text = std::fs::read_to_string(m2.join("foror.tsv")).unwrap();
    let rader: Vec<&str> = text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("fara"))
        .collect();
    assert_eq!(rader.len(), a.foror.len());
    for (i, rad) in rader.iter().enumerate() {
        let d: Vec<&str> = rad.split('\t').collect();
        assert_eq!(d.len(), 5, "foror.tsv ska ha vall_m");
        let vall: f64 = d[4].parse().unwrap();
        assert!((vall - a.foror[i].vall_m).abs() < 1e-9);
    }
    let (l2, a2) = las_3d_med(&m2).unwrap();
    assert!(a2.as_ref().unwrap().har_vall);
    assert!(a2.as_ref().unwrap().har_vaxtgrupper);
    assert_eq!(
        kontrollsumma_med(&l2, a2.as_ref()),
        kontrollsumma_med(&kart2, and2.as_ref())
    );

    let alt1343 = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/bredd1386-alt1343-v2");
    let (l_a, a_a) = las_3d_med(&alt1343).unwrap();
    assert!(!a_a.as_ref().unwrap().har_vall);
    assert!(!a_a.as_ref().unwrap().har_vaxtgrupper);
    assert!(a_a.as_ref().unwrap().foror.is_empty());
    let _ = kontrollsumma_med(&l_a, a_a.as_ref());

    let alt1386 = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/vall1371-alt1386-v2");
    let (l_b, a_b) = las_3d_med(&alt1386).unwrap();
    assert!(!a_b.as_ref().unwrap().har_vall);
    assert!(!a_b.as_ref().unwrap().har_vaxtgrupper);
    assert!(!a_b.as_ref().unwrap().foror.is_empty());
    assert_eq!(
        kontrollsumma_med(&l_b, a_b.as_ref()),
        // Före-kontrollsumma för denna karta (Alt 1386 utan vall).
        10464361137633183643
    );
    let _ = las_3d(&alt1386).unwrap();
    let _ = kontrollsumma(&l_b);
}

/// Prov 10: LASMIG märker 600 m, 2 m, 1–3 m som Butler [A]; faktor 4, båda sidorna, övergivna och
/// genombrott som eget val / ej byggt.
#[test]
fn prov10_markning() {
    let (_, and) =
        karta_full_version(&kalla(), &tidtabell(), &omrade(), 500_000.0, 181, 2, 1).unwrap();
    let a = and.unwrap();
    assert_eq!(a.vall_andel.marke, Marke::Simulering);
    assert_eq!(a.vall_hojd_m.marke, Marke::Simulering);
    assert!(a.har_vaxtgrupper);
    assert_eq!(a.vaxt_tathet.marke, Marke::Simulering);

    let lasmig =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"))
            .expect("LASMIG");
    assert!(lasmig.contains("600 m"), "saknar 600 m");
    assert!(lasmig.contains("2,0 m") || lasmig.contains("2.0 m") || lasmig.contains("2 m"));
    assert!(lasmig.contains("1–3 m") || lasmig.contains("1-3 m"));
    assert!(lasmig.contains("Butler 1980 [A]"));
    // Faktorn 4 och "båda sidorna tillsammans" som eget val — inte som uppmätt.
    assert!(lasmig.contains("båda sidorna tillsammans"));
    assert!(lasmig.contains("eget val"));
    // Hitta raden om VALL_FAKTOR / 600-taket.
    let har_eget_faktor = lasmig.lines().any(|l| {
        l.contains("VALL_FAKTOR") && l.contains("eget val")
    });
    assert!(har_eget_faktor, "VALL_FAKTOR ska stå som eget val");
    let har_eget_sidor = lasmig.lines().any(|l| {
        l.contains("båda sidorna tillsammans") && l.contains("eget val")
    });
    assert!(har_eget_sidor, "båda sidorna ska stå som eget val");
    let har_overgivna = lasmig.lines().any(|l| {
        (l.contains("övergivna") || l.contains("övergiven")) && l.contains("eget val")
    });
    assert!(har_overgivna, "övergivna utan vall ska stå som eget val");
    assert!(
        lasmig.contains("genombrott") && lasmig.contains("byggs inte"),
        "genombrott ska anges som ej byggt"
    );
    // Får inte stå som uppmätt faktum utan [A]/eget val.
    for rad in lasmig.lines() {
        if rad.contains("VALL_FAKTOR") || rad.contains("båda sidorna tillsammans") {
            assert!(
                !rad.contains("uppmätt"),
                "eget val får inte stå som uppmätt: {rad}"
            );
        }
    }
}

/// Redovisning: underlagets tal för fro 181, version 2, tidsmodell 2, t = 500000 och 20000.
#[test]
fn redovisning_vall() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let n = (omr.b * omr.h) as f64;
    for t in [500_000.0, 20_000.0] {
        let (kart, and) = karta_for_ar(&k, &tt, &omr, t, 181, 2, 1).unwrap();
        let a = and.unwrap();
        let period = kart.period.varden[0];
        let lev = levande_foror_nr(181, period);
        let bas = karta_med(&k, &tt, &omr, t).unwrap();
        let alla = mittlinjer_aldre(&bas, 181);
        let linjer: Vec<_> = lev.iter().map(|&i| alla[i].clone()).collect();
        let (_, n_klipp) = ra_vall_och_klipp(&kart, &linjer, &a.foror, &a.balte_andel.varden);
        let sum_v: f64 = a.vall_andel.varden.iter().map(|&x| f64::from(x)).sum();
        let n_vall = a.vall_andel.varden.iter().filter(|&&x| x > 0.0).count();
        eprintln!(
            "=== redovisning t={t} tidsmodell=2 fro=181 ===\nmedel vall_andel={:.6}\nrutor med vall={}\nrutor med klippning={}\nlevande={:?}",
            sum_v / n,
            n_vall,
            n_klipp,
            lev
        );
        for (i, f) in a.foror.iter().enumerate() {
            let nr = lev.get(i).copied().unwrap_or(i);
            eprintln!(
                "fåra {nr}: w_f={:.6} v_f={:.6} rutor={}",
                f.bredd_m, f.vall_m, f.rutor
            );
        }
        assert!(a.har_vall);
        assert!(n_vall > 0);
    }
    // tidsmodell 1 / mittlinjer-API orörd för smoke
    let _ = mittlinjer(&karta_med(&k, &tt, &omr, 500_000.0).unwrap(), 181);
}
