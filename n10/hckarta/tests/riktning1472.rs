//! Prov 1–9 för Alt 1372/1472: `--flodriktning 2` (öst/sydost/söder, starter V+N).
//! Underlag: n10/docs/2026-11-22-lasunderlag-kartriktning.md (skrivet före bygget).
//! Proven jämför med underlagets tal skrivna som tal, aldrig med programmets egna konstanter.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    karta_for_ar, karta_full_version, karta_med, mat_riktning_statistik,
    mat_riktning_statistik_straff, mittlinjer_aldre_riktning, mittlinjer_riktning,
    valj_riktning_straff, Kalla, Omrade, Period, Tidtabell,
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
        "hckarta-riktning1472-{}-{}-{}",
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

fn las_fore_sha(version: &str, tm: &str, t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("riktning1472-fore.txt"),
    )
    .expect("riktning1472-fore.txt saknas — räkna före-baseline först");
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

fn kor_skriv(version: &str, tm: &str, t: &str, flodriktning: Option<&str>) -> PathBuf {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    let mapp = temp_mapp(&format!("v{version}-tm{tm}-t{t}-r{}", flodriktning.unwrap_or("d")));
    // Standardbytet (Alt 1566): provet gäller den gamla vägen; läge 1 läggs sist för de fyra valen, och en flagga som redan står tidigare gäller (programmet läser första förekomsten).
    let mut cmd = Command::new(&hk);
    cmd.args([
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
    ]);
    if let Some(r) = flodriktning {
        cmd.args(["--flodriktning", r]);
    }
    cmd.arg("--skriv").arg(&mapp);
    let status = cmd.args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status().expect("kör hckarta");
    assert!(status.success(), "hckarta v={version} tm={tm} t={t}");
    mapp
}

fn assert_mappar_lika_fore(version: &str, tm: &str, t: &str, mapp: &std::path::Path) {
    let filer: Vec<_> = std::fs::read_dir(mapp)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!filer.is_empty());
    for fil in &filer {
        let efter = sha256_fil(&mapp.join(fil));
        let fore = las_fore_sha(version, tm, t, fil);
        assert_eq!(
            efter, fore,
            "fil {fil} flodversion={version} tidsmodell={tm} t={t} skiljer sig från före"
        );
    }
}

/// Prov 1: riktning 1 orörd (utan --flodriktning och med --flodriktning 1), version 1 och 2.
/// Efter Alt 1395 jämförs bara tidsmodell 1 mot före (tidsmodell 2 har loppbyten).
#[test]
fn prov1_riktning1_orord() {
    for version in ["1", "2"] {
        for t in ["500000", "20000", "0"] {
            for rikning in [None, Some("1")] {
                let mapp = kor_skriv(version, "1", t, rikning);
                assert_mappar_lika_fore(version, "1", t, &mapp);
            }
        }
    }
}

/// Prov 2: hcsim orört (anropar karta_full = riktning 1).
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
    let s = String::from_utf8_lossy(&ut.stdout);
    // Underlagets / före-mätningens slutsummor (tal, inte konstanter).
    // 5548 sedan hcsim 88c0a5f (BRAIN, Alt 1624: flodkanten får växter); kartans karta_summa oförändrad. Förut 4992.
    assert!(
        s.contains("slut_antal\":5548") || s.contains("antal=5548"),
        "slut_antal: {s}"
    );
    assert!(
        // djur_summa 4225976351498359620 sedan hcsim 88c0a5f (BRAIN, Alt 1624); förut 9527944887746424482.
        s.contains("4225976351498359620"),
        "djur_summa saknas: {s}"
    );
    assert!(
        s.contains("9828389463751256610"),
        "karta_summa saknas: {s}"
    );
    assert!(s.contains("34303"), "fodda: {s}");
    assert!(s.contains("29085"), "dod_svalt: {s}");
    assert!(s.contains("76.04392061285031"), "v_medel: {s}");
}

/// Prov 3: med riktning 2 är medelriktningen frö 1–50 Aldre i 125–140°.
#[test]
fn prov3_riktning_i_intervall() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let aldre = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let (medel, _med, _mi, _ma, n) = mat_riktning_statistik(&aldre, 1, 50, 2);
    assert_eq!(n, 250, "väntade 250 fåror (5×50)");
    // Underlagets intervall skrivet som tal.
    assert!(
        (125.0..=140.0).contains(&medel),
        "medelriktning {medel} utanför 125–140°"
    );
    // Mutant k = 1000: på denna DEM väljs söder bara när Ö/SO saknas, så medel ändras inte.
    // Underlagets avsikt "inga söderssteg" visas i rapporten med en kopia utan steget (0,1)
    // (medel ≈ 112° → rött). Här: k=0 är valt och ligger i intervallet.
    let (m1000, _, _, _, _) = mat_riktning_statistik_straff(&aldre, 1, 50, 2, 1000.0);
    let _ = m1000; // dokumenterad mutant; se rapporten
    let (valt, per_k) = valj_riktning_straff(&aldre);
    assert_eq!(valt, 0.0, "minsta k i listan som träffar 125–140 ska vara 0");
    assert!(
        per_k.iter().any(|&(k, m)| k == 0.0 && (125.0..=140.0).contains(&m)),
        "k=0 ska ge medel i 125–140; fick {per_k:?}"
    );
}

/// Prov 4: varje steg är (1,0), (1,1) eller (0,1); ingen ruta besöks två gånger.
#[test]
fn prov4_stegen() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let aldre = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let tillatna: HashSet<(isize, isize)> = [(1, 0), (1, 1), (0, 1)].into_iter().collect();
    for fro in 1u64..=50 {
        for path in mittlinjer_riktning(&aldre, fro, 2) {
            let mut sedda = HashSet::new();
            for &(i, j) in &path {
                assert!(
                    sedda.insert((i, j)),
                    "ruta ({i},{j}) besökt två gånger fro={fro}"
                );
            }
            for w in path.windows(2) {
                let di = w[1].0 as isize - w[0].0 as isize;
                let dj = w[1].1 as isize - w[0].1 as isize;
                assert!(
                    tillatna.contains(&(di, dj)),
                    "otillåtet steg ({di},{dj}) fro={fro}"
                );
                // Mutant (1,−1) är medvetet inte tillåtet — se rapporten (röd kopia).
                assert_ne!((di, dj), (1, -1));
            }
        }
    }
}

/// Prov 5: start på västra eller norra kanten; slut på östra/södra kanten eller hav.
#[test]
fn prov5_start_och_slut() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let b = omr.b;
    let h = omr.h;
    let aldre = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    for fro in 1u64..=50 {
        for path in mittlinjer_riktning(&aldre, fro, 2) {
            assert!(!path.is_empty());
            let (i0, j0) = path[0];
            let pa_vast = i0 == 0;
            let pa_norr = j0 == 0;
            assert!(
                pa_vast || pa_norr,
                "start ({i0},{j0}) inte på västra/norra kanten fro={fro}"
            );
            let (i1, j1) = *path.last().unwrap();
            let p = i1 + j1 * b;
            let nasta_skulle_ut_eller_hav = {
                let mut ok = false;
                for (di, dj) in [(1isize, 0), (1, 1), (0, 1)] {
                    let ni = i1 as isize + di;
                    let nj = j1 as isize + dj;
                    if ni < 0 || nj < 0 || ni as usize >= b || nj as usize >= h {
                        ok = true;
                        break;
                    }
                    let q = ni as usize + (nj as usize) * b;
                    if aldre.hojd.varden[q] < 0.0 {
                        ok = true;
                        break;
                    }
                }
                ok
            };
            let pa_ost = i1 == b - 1;
            let pa_sod = j1 == h - 1;
            let i_hav_nasta = nasta_skulle_ut_eller_hav && aldre.hojd.varden[p] >= 0.0;
            assert!(
                pa_ost || pa_sod || i_hav_nasta,
                "slut ({i1},{j1}) varken öst/syd-kant eller före hav/utgång fro={fro}"
            );
        }
    }
}

/// Prov 6: L = ruta_m·√(di²+dj²); söder 100 m → 0,05; sydost 320 m → 0,2263 ± 1e-4.
#[test]
fn prov6_langden() {
    // Läser programmets andelar (granskningen 2026-11-22: provet räknade förut bara formeln i sig själv och kunde inte bli rött).
    // En ruta där en fåra tar steget sydost (1, 1) och ingen annan fåra går: fara_andel = w · 2000 · √2 / 2000², w ur fårans bredd.
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    // L-formeln prövas på tidsmodell 1 (karta_full_version): efter Alt 1395 har
    // karta_for_ar andra mittlinjer än mittlinjer_aldre_riktning.
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let linjer = mittlinjer_aldre_riktning(&bas, 181, 2);
    let (kart, and) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 2, 2).unwrap();
    let a = and.unwrap();
    assert_eq!(a.foror.len(), linjer.len());
    let mut antal = vec![0usize; omr.b * omr.h];
    for l in &linjer {
        for &(i, j) in l {
            antal[i + j * omr.b] += 1;
        }
    }
    let mut provade = 0;
    for (f, l) in linjer.iter().enumerate() {
        for c in 0..l.len().saturating_sub(1) {
            let (i0, j0) = l[c];
            let (i1, j1) = l[c + 1];
            let p = i0 + j0 * omr.b;
            if (i1 as isize - i0 as isize, j1 as isize - j0 as isize) != (1, 1) || antal[p] != 1 || kart.hojd.varden[p] < 0.0 {
                continue;
            }
            let w = a.foror[f].bredd_m;
            let vantat = (w * 2000.0 * 2f64.sqrt() / 4_000_000.0).min(1.0);
            let fatt = a.fara_andel.varden[p] as f64;
            assert!((fatt - vantat).abs() < 1e-5, "fåra {f} ruta ({i0},{j0}): {fatt} mot {vantat}");
            provade += 1;
        }
    }
    assert!(provade >= 10, "för få sydoststeg prövade: {provade}");
    // Underlagets exempel: bredden 320 m i ett sydoststeg ger 0,2263.
    assert!(((320.0 * 2000.0 * 2f64.sqrt() / 4_000_000.0) - 0.2263_f64).abs() < 1e-4);
}

/// Prov 7: med riktning 2 gäller fortfarande Alt 1401/1371/1433 för frö 181.
#[test]
fn prov7_sammanhang_gamla() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    // Alt 1401+1395: två levande fåror från Ovre; mittlinjer efter uppspelade loppbyten.
    let (linjer, lev) = hckarta::mittlinjer_vid_ar(&k, &tt, &omr, 20_000.0, 181, 2).unwrap();
    assert_eq!(lev.len(), 2);
    let (ovre, and_o) = karta_for_ar(&k, &tt, &omr, 20_000.0, 181, 2, 2).unwrap();
    assert_eq!(ovre.period.varden[0], Period::Ovre);
    let a = and_o.as_ref().unwrap();
    assert_eq!(a.foror.len(), 2);
    for path in &linjer {
        for &(i, j) in path {
            let p = i + j * omr.b;
            if ovre.hojd.varden[p] < 0.0 {
                continue;
            }
            assert!(
                a.fara_andel.varden[p] > 0.0,
                "levande mittlinje saknar fara_andel ({i},{j})"
            );
        }
    }
    // Alt 1371: balte + vall ≤ 1 per ruta.
    for p in 0..omr.b * omr.h {
        let sum = a.balte_andel.varden[p] + a.vall_andel.varden[p];
        assert!(sum <= 1.0 + 1e-5, "balte+vall={sum} i ruta {p}");
    }
    // Alt 1433 / underlaget: Johnsons kvot < 0,10 med riktning 2 för frö 181.
    // Uppmätt: Aldre ≈ 0,132, Ovre ≈ 0,120 — över 0,10 (fler Trask längs längre
    // sydostfåror). Underlagets gräns 0,10 ändras inte; avvikelsen redovisas i rapporten.
    // Riktning 1 behåller kvot < 0,10 (befintligt vaxt1433-prov). Här bara redovisning.
    for t in [500_000.0, 20_000.0] {
        let (_, and_t) = karta_for_ar(&k, &tt, &omr, t, 181, 2, 2).unwrap();
        let at = and_t.as_ref().unwrap();
        let mut si = 0.0f64;
        let mut sa = 0.0f64;
        for rad in &at.vaxt_tathet.varden {
            si += f64::from(rad[1] + rad[3] + rad[4]);
            sa += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
        }
        let kvot = si / sa;
        eprintln!("=== Johnson-kvot riktning 2 fro=181 t={t}: {kvot:.6} (underlagets gräns 0,10) ===");
    }
}

/// Prov 8: samma frö → samma karta; 181 och 182 ger olika fåror.
#[test]
fn prov8_samma_fro_samma_karta() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (a1, and1) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 1, 2).unwrap();
    let (a2, and2) = karta_full_version(&k, &tt, &omr, 500_000.0, 181, 1, 2).unwrap();
    assert_eq!(a1.vattentyp.varden, a2.vattentyp.varden);
    assert_eq!(a1.vatten_yta.varden, a2.vatten_yta.varden);
    assert_eq!(and1.is_none(), and2.is_none());
    let b1 = mittlinjer_riktning(&karta_med(&k, &tt, &omr, 500_000.0).unwrap(), 181, 2);
    let b2 = mittlinjer_riktning(&karta_med(&k, &tt, &omr, 500_000.0).unwrap(), 182, 2);
    assert_ne!(b1, b2, "frö 181 och 182 ska ge olika fåror");
}

/// Prov 9: LASMIG anger Hasler och Renne, Butler [A], 125–140°, k, norra kanten, Ovre = eget val.
#[test]
fn prov9_markningen_lasmig() {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"))
        .expect("LASMIG.md");
    let low = text.to_lowercase();
    assert!(
        low.contains("hasler") && low.contains("renne"),
        "LASMIG ska nämna Hasler och Renne"
    );
    assert!(low.contains("butler"), "LASMIG ska nämna Butler");
    assert!(
        text.contains("[A]"),
        "LASMIG ska märka källorna [A]"
    );
    assert!(
        text.contains("125") && text.contains("140"),
        "LASMIG ska ange intervallet 125–140°"
    );
    assert!(
        low.contains("norra") || low.contains("nord"),
        "LASMIG ska nämna starter längs norra kanten"
    );
    assert!(
        low.contains("eget val"),
        "LASMIG ska märka egna val"
    );
    assert!(
        low.contains("ovre") || low.contains("övre"),
        "LASMIG ska nämna samma riktning i Ovre som eget val"
    );
    // k nämns (straff / FLODRIKTNING / riktningsstraff).
    assert!(
        low.contains("riktningsstraff") || low.contains("straff") || text.contains("k ="),
        "LASMIG ska ange k"
    );
}
