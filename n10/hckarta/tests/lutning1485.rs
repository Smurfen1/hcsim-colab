//! Prov för Alt 1485/1490/1488 + Alt 1501/1502 + Alt 1533: `--lutning 2`, träsk mot närmaste
//! fåra, låsta band för träsk (frö 41–60).
//! Underlag: n10/docs/2026-11-22-lasunderlag-kartlutning.md,
//! n10/docs/2026-11-22-lasunderlag-kartlut2.md och
//! /home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartlut3.md (skrivna före bygget).
//! Proven jämför med underlagets tal skrivna som tal, aldrig med programmets egna konstanter.

use std::path::PathBuf;
use std::process::Command;

use hckarta::{
    applicera_lutning_plan, karta_for_ar, karta_for_ar_lutning, karta_full_version,
    karta_full_version_lutning, karta_med, lagg_flodnat_version_lutning_prov, mat_plan,
    mat_riktning_statistik, mittlinjer_aldre_riktning, mittlinjer_riktning, Kalla, Omrade,
    Tidtabell, TraskProvMutant, Vatten,
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
        "hckarta-lutning1485-{}-{}-{}",
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

fn las_fore_sha(version: &str, tm: &str, rikning: &str, t: &str, fil: &str) -> String {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lutning1485-fore.txt"),
    )
    .expect("lutning1485-fore.txt saknas — räkna före-baseline först");
    let mark = format!(
        "sha256 filer flodversion={version} tidsmodell={tm} flodriktning={rikning} t={t}:"
    );
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
    panic!(
        "saknar sha256 för {fil} vid flodversion={version} tidsmodell={tm} flodriktning={rikning} t={t}"
    );
}

fn kor_skriv(
    version: &str,
    tm: &str,
    t: &str,
    flodriktning: &str,
    lutning: Option<&str>,
) -> PathBuf {
    let hk = bin_hckarta();
    let tt = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv");
    let mapp = temp_mapp(&format!(
        "v{version}-tm{tm}-r{flodriktning}-t{t}-l{}",
        lutning.unwrap_or("d")
    ));
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
        "--flodriktning",
        flodriktning,
    ]);
    if let Some(l) = lutning {
        cmd.args(["--lutning", l]);
    }
    cmd.arg("--skriv").arg(&mapp);
    let status = cmd.args(["--flodversion", "1", "--tidsmodell", "1", "--flodriktning", "1", "--lutning", "1"]).status().expect("kör hckarta");
    assert!(
        status.success(),
        "hckarta v={version} tm={tm} r={flodriktning} t={t} lutning={lutning:?}"
    );
    mapp
}

fn assert_mappar_lika_fore(
    version: &str,
    tm: &str,
    rikning: &str,
    t: &str,
    mapp: &std::path::Path,
) {
    let filer: Vec<_> = std::fs::read_dir(mapp)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!filer.is_empty());
    for fil in &filer {
        let efter = sha256_fil(&mapp.join(fil));
        let fore = las_fore_sha(version, tm, rikning, t, fil);
        assert_eq!(
            efter, fore,
            "fil {fil} v={version} tm={tm} r={rikning} t={t} skiljer sig från före"
        );
    }
}

fn johnson_kvot(and: &hckarta::FlodAndelar) -> f64 {
    let mut si = 0.0f64;
    let mut sa = 0.0f64;
    for rad in &and.vaxt_tathet.varden {
        si += f64::from(rad[1] + rad[3] + rad[4]);
        sa += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
    }
    si / sa
}

/// Mutant: plan mot 315° i stället för 135° (underlagets mutant).
fn mutant_plan_mot_315(h: &[f32], b: usize, hh: usize, ruta_km: f64) -> (f64, f64, f64) {
    // Applicera fel riktning på en tillfällig karta via samma formler som programmet.
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b,
        h: hh,
        ruta_m: ruta_km * 1000.0,
    };
    let mut kart = hckarta::Karta {
        omrade: omr,
        t: 0.0,
        hojd: hckarta::Lager {
            varden: h.to_vec(),
            marke: hckarta::Marke::Last,
        },
        vatten_yta: hckarta::Lager {
            varden: h.to_vec(),
            marke: hckarta::Marke::Last,
        },
        vattentyp: hckarta::Lager {
            varden: vec![Vatten::Land; b * hh],
            marke: hckarta::Marke::Last,
        },
        mark: hckarta::Lager {
            varden: vec![[0.25; 4]; b * hh],
            marke: hckarta::Marke::Simulering,
        },
        vaxt: hckarta::Lager {
            varden: vec![0.5; b * hh],
            marke: hckarta::Marke::Simulering,
        },
        period: hckarta::Lager {
            varden: vec![hckarta::Period::Aldre; b * hh],
            marke: hckarta::Marke::Last,
        },
    };
    applicera_lutning_plan(&mut kart, 0.30, 315.0).expect("315° ska inte ge land under 0");
    mat_plan(&kart.hojd.varden, b, hh, ruta_km)
}

/// Mutant: vattenyta = höjd − 2 utan min-regeln (underlagets mutant).
fn mutant_yta_utan_min(path: &[(usize, usize)], hojd: &[f32], b: usize) -> bool {
    // Returnerar true om ytan stiger någonstans (ska ge rött mot prov 3).
    let mut prev: Option<f32> = None;
    let mut stiger = false;
    for &(i, j) in path {
        let y = hojd[i + j * b] - 2.0;
        if let Some(py) = prev {
            if y > py + 1e-4 {
                stiger = true;
            }
        }
        prev = Some(y);
    }
    stiger
}

/// Prov 1: utan `--lutning 2` orört (version 1 och 2, båda tidsmodellerna, riktning 1 och 2).
#[test]
fn prov1_utan_lutning2_orort() {
    for version in ["1", "2"] {
        for tm in ["1", "2"] {
            for rikning in ["1", "2"] {
                for t in ["500000", "20000", "0"] {
                    for lut in [None, Some("1")] {
                        let mapp = kor_skriv(version, tm, t, rikning, lut);
                        assert_mappar_lika_fore(version, tm, rikning, t, &mapp);
                    }
                }
            }
        }
    }
}

/// Prov 1b (underlaget prov 1): hcsim orört (karta_full = lutning 1, riktning 1).
#[test]
fn prov1b_hcsim_orort() {
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
    // 5548 sedan hcsim 88c0a5f (BRAIN, Alt 1624: flodkanten får växter); kartans karta_summa oförändrad. Förut 4992.
    assert!(
        s.contains("slut_antal\":5548") || s.contains("antal=5548"),
        "slut_antal: {s}"
    );
    // djur_summa 4225976351498359620 sedan hcsim 88c0a5f (BRAIN, Alt 1624); förut 9527944887746424482.
    assert!(s.contains("4225976351498359620"), "djur_summa saknas: {s}");
    assert!(s.contains("9828389463751256610"), "karta_summa saknas: {s}");
    assert!(s.contains("34303"), "fodda: {s}");
    assert!(s.contains("29085"), "dod_svalt: {s}");
    assert!(s.contains("76.04392061285031"), "v_medel: {s}");
}

/// Prov 2b (underlagets planmätning): med `--lutning 2` ger planmätningen 0,300±0,005 m/km mot 135,0±0,5°;
/// medelhöjd lika (±0,01 m). Mutant 315° → rött.
#[test]
fn prov2_planet() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let (l0, r0, m0) = mat_plan(&bas.hojd.varden, omr.b, omr.h, 2.0);
    eprintln!("=== plan före: {l0:.3} m/km mot {r0:.1}°; medel {m0:.3} m ===");
    // Underlagets mätta PaleoDEM-tal.
    assert!((l0 - 0.510).abs() < 0.005, "före-lutning {l0}");
    assert!((r0 - 270.8).abs() < 0.5, "före-riktning {r0}");

    let mut efter = bas.clone();
    applicera_lutning_plan(&mut efter, 0.30, 135.0).unwrap();
    let (l1, r1, m1) = mat_plan(&efter.hojd.varden, omr.b, omr.h, 2.0);
    eprintln!("=== plan efter lutning 2: {l1:.3} m/km mot {r1:.1}°; medel {m1:.3} m ===");
    assert!((l1 - 0.300).abs() <= 0.005, "lutning {l1} utanför 0,300±0,005");
    assert!((r1 - 135.0).abs() <= 0.5, "riktning {r1} utanför 135±0,5");
    assert!((m1 - m0).abs() <= 0.01, "medel {m1} vs {m0}");

    // Mutant: planet mot 315° → inte 135°.
    let (lm, rm, _) = mutant_plan_mot_315(&bas.hojd.varden, omr.b, omr.h, 2.0);
    eprintln!("=== mutant 315°: {lm:.3} m/km mot {rm:.1}° ===");
    assert!(
        (rm - 135.0).abs() > 0.5,
        "mutant 315° borde ge riktning långt från 135°, fick {rm}"
    );
    // Programmets utdata med --lutning 2 (läser CLI).
    let mapp = kor_skriv("1", "1", "500000", "1", Some("2"));
    let hojd = std::fs::read(mapp.join("hojd.f32")).unwrap();
    let mut varden = Vec::with_capacity(omr.b * omr.h);
    for chunk in hojd.chunks_exact(4) {
        varden.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    let (lcli, rcli, _) = mat_plan(&varden, omr.b, omr.h, 2.0);
    assert!((lcli - 0.300).abs() <= 0.005, "CLI-lutning {lcli}");
    assert!((rcli - 135.0).abs() <= 0.5, "CLI-riktning {rcli}");
}

/// Prov 3: vattenytan stiger aldrig med lutning 2; visas rött utan lutning 2 först.
#[test]
fn prov3_vattenytan_stiger_aldrig() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let b = omr.b;

    // Rött på dagens kod (lutning 1): minst en fåra stiger.
    let mut n_stiger_utan = 0usize;
    let mut n_foror = 0usize;
    for fro in 1u64..=20 {
        for r in [1u8, 2] {
            let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
            let linjer = mittlinjer_aldre_riktning(&bas, fro, r);
            let (kart, _) = karta_full_version(&k, &tt, &omr, 500_000.0, fro, 2, r).unwrap();
            for path in &linjer {
                n_foror += 1;
                for w in path.windows(2) {
                    let y0 = kart.vatten_yta.varden[w[0].0 + w[0].1 * b];
                    let y1 = kart.vatten_yta.varden[w[1].0 + w[1].1 * b];
                    if y1 > y0 + 1e-4 {
                        n_stiger_utan += 1;
                        break;
                    }
                }
            }
        }
    }
    eprintln!("=== utan lutning 2: {n_stiger_utan}/{n_foror} fåror har stigande vattenyta ===");
    assert!(
        n_stiger_utan > 0,
        "provet ska vara rött på dagens kod (utan lutning 2)"
    );

    // Mutant: utan min-regeln (höjd−2 per ruta) stiger ytan på denna DEM.
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let mut bas2 = bas.clone();
    applicera_lutning_plan(&mut bas2, 0.30, 135.0).unwrap();
    let linjer = mittlinjer_aldre_riktning(&bas2, 1, 2);
    let mutant_stiger = linjer
        .iter()
        .any(|p| mutant_yta_utan_min(p, &bas2.hojd.varden, b));
    assert!(
        mutant_stiger,
        "mutant utan min-regeln ska ge stigande yta (rött)"
    );

    // Med lutning 2: aldrig stigande (Aldre+Ovre, båda riktningarna, frö 1–20).
    // Mittlinjer ur samma period som kartan (inte tvingad Aldre) så loppen matchar.
    for t in [500_000.0, 20_000.0] {
        for fro in 1u64..=20 {
            for r in [1u8, 2] {
                let mut bas_t = karta_med(&k, &tt, &omr, t).unwrap();
                applicera_lutning_plan(&mut bas_t, 0.30, 135.0).unwrap();
                let linjer = mittlinjer_riktning(&bas_t, fro, r);
                let (kart, _) =
                    karta_full_version_lutning(&k, &tt, &omr, t, fro, 2, r, 2).unwrap();
                for path in &linjer {
                    for w in path.windows(2) {
                        let y0 = kart.vatten_yta.varden[w[0].0 + w[0].1 * b];
                        let y1 = kart.vatten_yta.varden[w[1].0 + w[1].1 * b];
                        assert!(
                            y1 <= y0 + 1e-4,
                            "vattenyta stiger fro={fro} r={r} t={t}: {y0} -> {y1}"
                        );
                    }
                }
            }
        }
    }
}

/// Percentil som i `kartlut-matning1500.rs`: sortera och ta index round((n−1)·p).
fn perc(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() {
        return f64::NAN;
    }
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Mätning som `kartlut-matning1500.rs` för frö `fro_lo..=fro_hi`, riktning `r`, lutning `lut`.
/// Returnerar (träsk/fårruta alla, öst/väst-kvot, p95, max).
fn mat_kartlut(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    fro_lo: u64,
    fro_hi: u64,
    r: u8,
    lut: u8,
) -> (f64, f64, f64, f64) {
    mat_kartlut_mutant(k, tt, omr, fro_lo, fro_hi, r, lut, TraskProvMutant::Ingen)
}

/// Som `mat_kartlut`, men med träskmutant via kartans egen `lagg_flodnat_version_lutning_prov`.
fn mat_kartlut_mutant(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    fro_lo: u64,
    fro_hi: u64,
    r: u8,
    lut: u8,
    mutant: TraskProvMutant,
) -> (f64, f64, f64, f64) {
    let bas0 = karta_med(k, tt, omr, 500_000.0).unwrap();
    let mut bas = bas0;
    if lut >= 2 {
        applicera_lutning_plan(&mut bas, 0.30, 135.0).unwrap();
    }
    let (b, hh) = (omr.b, omr.h);
    let mut djup = Vec::new();
    let (mut mitt_v, mut mitt_o, mut trask_v, mut trask_o) = (0.0, 0.0, 0.0, 0.0);
    for fro in fro_lo..=fro_hi {
        let linjer = mittlinjer_riktning(&bas, fro, r);
        let mut kart = bas.clone();
        let _ = lagg_flodnat_version_lutning_prov(&mut kart, fro, 2, r, lut, mutant);
        let mut mitt = vec![false; b * hh];
        for l in &linjer {
            for &(i, j) in l {
                mitt[i + j * b] = true;
            }
        }
        for p in 0..b * hh {
            let ost = (p % b) >= b / 2;
            if mitt[p] {
                let d = (kart.hojd.varden[p] - kart.vatten_yta.varden[p]) as f64;
                djup.push(d);
                if ost {
                    mitt_o += 1.0;
                } else {
                    mitt_v += 1.0;
                }
            }
            if kart.vattentyp.varden[p] == Vatten::Trask {
                if ost {
                    trask_o += 1.0;
                } else {
                    trask_v += 1.0;
                }
            }
        }
    }
    let tv = trask_v / mitt_v;
    let to = trask_o / mitt_o;
    let talla = (trask_v + trask_o) / (mitt_v + mitt_o);
    let maxd = djup.iter().cloned().fold(f64::MIN, f64::max);
    let p95 = perc(&mut djup, 0.95);
    (talla, to / tv, p95, maxd)
}

/// Prov 4 (Alt 1502/1533): p95 ≤ 4,0 m och max ≤ 7,0 m av höjd − yta i fårrutorna
/// (frö 41–60, riktning 2, lutning 2; mätsätt som kartlut-matning1500.rs).
#[test]
fn prov4_djupet() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (_, _, p95, maxd) = mat_kartlut(&k, &tt, &omr, 41, 60, 2, 2);
    eprintln!("=== prov4 djup frö 41–60 r2+l2: p95={p95:.3} m (≤4,0); max={maxd:.3} m (≤7,0) ===");
    assert!(
        p95 <= 4.0 + 1e-9,
        "p95 djup {p95} m > 4,0 m (underlagets tal; ändra inte)"
    );
    assert!(
        maxd <= 7.0 + 1e-9,
        "max djup {maxd} m > 7,0 m (underlagets tal; ändra inte)"
    );
}

/// Prov 5a (Alt 1533): träsk per fårruta med lutning 2 — riktning 2: 1,35–2,10; riktning 1: 0,16–0,40
/// (frö 41–60; mätsätt som kartlut-matning1500.rs). Banden är underlagets låsta tal.
#[test]
fn prov5a_trasken() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (t2, _, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 2, 2);
    let (t1, _, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 1, 2);
    eprintln!("=== prov5a träsk/fårruta frö 41–60 r2+l2: {t2:.3} (mål 1,35–2,10) ===");
    eprintln!("=== prov5a träsk/fårruta frö 41–60 r1+l2: {t1:.3} (mål 0,16–0,40) ===");
    assert!(
        (1.35..=2.10).contains(&t2),
        "träsk per fårruta r2 {t2} utanför 1,35–2,10 (underlagets tal; ändra inte)"
    );
    assert!(
        (0.16..=0.40).contains(&t1),
        "träsk per fårruta r1 {t1} utanför 0,16–0,40 (underlagets tal; ändra inte)"
    );
}

/// Prov 5a-mutanter (Alt 1533): båda mutanterna i kartan på fröna 1–20 ska vara röda i riktning 2
/// mot bandet 1,35–2,10. Mutant A = närmaste := lägsta; mutant B = dubblat `trask_avstand`.
#[test]
fn prov5a_mutanter_roda_riktning2() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (ta, _, _, _) = mat_kartlut_mutant(
        &k,
        &tt,
        &omr,
        1,
        20,
        2,
        2,
        TraskProvMutant::NarmasteSomLagsta,
    );
    let (tb, _, _, _) = mat_kartlut_mutant(
        &k,
        &tt,
        &omr,
        1,
        20,
        2,
        2,
        TraskProvMutant::DubbeltAvstand,
    );
    let (ta1, _, _, _) = mat_kartlut_mutant(
        &k,
        &tt,
        &omr,
        1,
        20,
        1,
        2,
        TraskProvMutant::NarmasteSomLagsta,
    );
    let (tb1, _, _, _) = mat_kartlut_mutant(
        &k,
        &tt,
        &omr,
        1,
        20,
        1,
        2,
        TraskProvMutant::DubbeltAvstand,
    );
    eprintln!("=== mutant A (närmaste:=lägsta) frö 1–20 r2+l2: {ta:.3}; r1+l2: {ta1:.3} ===");
    eprintln!("=== mutant B (dubbelt avstånd) frö 1–20 r2+l2: {tb:.3}; r1+l2: {tb1:.3} ===");
    assert!(
        !(1.35..=2.10).contains(&ta),
        "mutant A ska vara röd i riktning 2 (träsk {ta} inom 1,35–2,10) — stoppa före frö 41–60"
    );
    assert!(
        !(1.35..=2.10).contains(&tb),
        "mutant B ska vara röd i riktning 2 (träsk {tb} inom 1,35–2,10) — stoppa före frö 41–60"
    );
}

/// Prov 5b (Alt 1502/1533): öst/väst med lutning 2 inom 0,5–2,0 × samma kvot med riktning 1 och lutning 1
/// (frö 41–60; mätsätt som kartlut-matning1500.rs).
#[test]
fn prov5b_ost_vast() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (_, ov_l2, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 2, 2);
    let (_, ov_std, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 1, 1);
    let kvot = ov_l2 / ov_std;
    eprintln!(
        "=== prov5b öst/väst frö 41–60: l2={ov_l2:.3}; r1+l1={ov_std:.3}; kvot={kvot:.3} (mål 0,5–2,0) ==="
    );
    assert!(
        (0.5..=2.0).contains(&kvot),
        "öst/väst-kvot mot standard {kvot} utanför 0,5–2,0 (underlagets tal; ändra inte)"
    );
}

/// Prov 6: Johnsons kvot för frö 181 med riktning 2 och lutning 2 < 0,10 i Aldre och Ovre.
#[test]
fn prov6_johnsons_kvot() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for t in [500_000.0, 20_000.0] {
        let (_, and) = karta_for_ar_lutning(&k, &tt, &omr, t, 181, 2, 2, 2).unwrap();
        let kvot = johnson_kvot(and.as_ref().unwrap());
        eprintln!("=== Johnson r2+l2 fro=181 t={t}: {kvot:.6} ===");
        assert!(
            kvot < 0.10,
            "Johnsons kvot {kvot} ska vara < 0,10 (underlagets gräns; ändra inte)"
        );
    }
}

/// Prov 7: medelriktning frö 1–50 Aldre med riktning 2 + lutning 2 i 125–140°; spann ≥ 10°.
#[test]
fn prov7_riktningen() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let mut bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    applicera_lutning_plan(&mut bas, 0.30, 135.0).unwrap();
    let (medel, _, mi, ma, n) = mat_riktning_statistik(&bas, 1, 50, 2);
    eprintln!("=== medel={medel:.1}° min={mi:.1} max={ma:.1} n={n} spann={:.1} ===", ma - mi);
    assert_eq!(n, 250);
    assert!(
        (125.0..=140.0).contains(&medel),
        "medelriktning {medel} utanför 125–140°"
    );
    assert!(ma - mi >= 10.0, "spann {} < 10°", ma - mi);
}

/// Prov 8: ingen landruta under 0 med lutning 2; 30 m/km stoppar med fel.
#[test]
fn prov8_ingen_landruta_under_noll() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let (kart, _) =
        karta_full_version_lutning(&k, &tt, &omr, 500_000.0, 181, 2, 2, 2).unwrap();
    for (p, &h) in kart.hojd.varden.iter().enumerate() {
        if kart.vattentyp.varden[p] != Vatten::Hav {
            // Land/övrigt som hade höjd ≥ 0 före: fortfarande ≥ 0.
            assert!(h >= 0.0, "landruta {p} under 0: {h}");
        }
    }
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let mut brat = bas.clone();
    let fel = applicera_lutning_plan(&mut brat, 30.0, 135.0);
    assert!(
        fel.is_err(),
        "30 m/km ska stoppa bygget med fel (underlagets mutant)"
    );
    eprintln!("=== 30 m/km fel: {} ===", fel.unwrap_err());

    // CLI ska också fela om vi kunde sätta 30 — här via biblioteket ovan.
}

/// Prov 9: LASMIG har "bilinjär", "eget val" på raden för 135°, och Butler för 0,30 m/km.
#[test]
fn prov9_markningen() {
    let lasmig = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("LASMIG.md"),
    )
    .expect("LASMIG.md");
    assert!(
        lasmig.to_lowercase().contains("bilinjär") || lasmig.contains("bilinjar"),
        "LASMIG saknar bilinjär"
    );
    let mut hittade_135 = false;
    for rad in lasmig.lines() {
        if rad.contains("135") {
            if rad.contains("eget val") {
                hittade_135 = true;
            }
        }
    }
    assert!(
        hittade_135,
        "LASMIG saknar 'eget val' på raden för 135°"
    );
    assert!(
        lasmig.contains("0,30") || lasmig.contains("0.30"),
        "LASMIG saknar 0,30 m/km"
    );
    assert!(
        lasmig.contains("Butler"),
        "LASMIG saknar Butler som källa för 0,30 m/km"
    );
    assert!(
        lasmig.contains("närmaste") || lasmig.contains("narmaste"),
        "LASMIG saknar Alt 1501 (närmaste mittlinjeruta)"
    );
    assert!(
        lasmig.contains("1,35") || lasmig.contains("1.35"),
        "LASMIG saknar band 1,35–2,10"
    );
    assert!(
        lasmig.contains("0,16") || lasmig.contains("0.16"),
        "LASMIG saknar band 0,16–0,40"
    );
    let mut band_eget = false;
    for rad in lasmig.lines() {
        if (rad.contains("1,35") || rad.contains("1.35")) && rad.contains("eget val") {
            band_eget = true;
        }
    }
    assert!(
        band_eget,
        "LASMIG ska märka träskbanden som eget val utan källa"
    );
}

/// Redovisning Alt 1533: mutanter frö 1–20 + talen frö 41–60 (mätsätt kartlut-matning1500) + Johnson.
#[test]
fn redovisning_1533() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for (namn, mutant) in [
        ("utan mutant", TraskProvMutant::Ingen),
        ("mutant A närmaste:=lägsta", TraskProvMutant::NarmasteSomLagsta),
        ("mutant B dubbelt avstånd", TraskProvMutant::DubbeltAvstand),
    ] {
        for r in [1u8, 2] {
            let (talla, _, _, _) =
                mat_kartlut_mutant(&k, &tt, &omr, 1, 20, r, 2, mutant);
            eprintln!("=== {namn} frö 1–20 r{r}+l2: träsk/fårruta={talla:.3} ===");
        }
    }
    for (r, lut) in [(1u8, 1u8), (2, 1), (1, 2), (2, 2)] {
        let (talla, ov, p95, maxd) = mat_kartlut(&k, &tt, &omr, 41, 60, r, lut);
        eprintln!(
            "=== frö 41–60 r{r} l{lut}: träsk/fårruta={talla:.3} öst/väst={ov:.3} djup p95={p95:.3} max={maxd:.3} ==="
        );
    }
    let (_, ov_l2, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 2, 2);
    let (_, ov_std, _, _) = mat_kartlut(&k, &tt, &omr, 41, 60, 1, 1);
    eprintln!(
        "=== prov5b-kvot (l2 / r1l1) frö 41–60 = {:.3} ===",
        ov_l2 / ov_std
    );
    for t in [500_000.0, 20_000.0] {
        let (_, and) = karta_for_ar_lutning(&k, &tt, &omr, t, 181, 2, 2, 2).unwrap();
        let kvot = johnson_kvot(and.as_ref().unwrap());
        eprintln!("=== Johnson r2+l2 fro=181 t={t}: {kvot:.6} ===");
    }
}

/// Redovisning Alt 1501/1502: talen frö 21–40 (mätsätt kartlut-matning1500) + Johnson.
#[test]
fn redovisning_1501() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for (r, lut) in [(1u8, 1u8), (2, 1), (1, 2), (2, 2)] {
        let (talla, ov, p95, maxd) = mat_kartlut(&k, &tt, &omr, 21, 40, r, lut);
        eprintln!(
            "=== MED1501 r{r} l{lut} frö 21–40: träsk/fårruta={talla:.3} öst/väst={ov:.3} djup p95={p95:.3} max={maxd:.3} ==="
        );
    }
    let (_, ov_l2, _, _) = mat_kartlut(&k, &tt, &omr, 21, 40, 2, 2);
    let (_, ov_std, _, _) = mat_kartlut(&k, &tt, &omr, 21, 40, 1, 1);
    eprintln!(
        "=== prov5b-kvot (l2 / r1l1) = {:.3} ===",
        ov_l2 / ov_std
    );
    for t in [500_000.0, 20_000.0] {
        let (_, and) = karta_for_ar_lutning(&k, &tt, &omr, t, 181, 2, 2, 2).unwrap();
        let kvot = johnson_kvot(and.as_ref().unwrap());
        eprintln!("=== Johnson r2+l2 fro=181 t={t}: {kvot:.6} ===");
    }
}

/// Redovisning: plan, fall, träsk/mitt, Johnson, vattentyper (underlagets lista).
#[test]
fn redovisning() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let bas = karta_med(&k, &tt, &omr, 500_000.0).unwrap();
    let (l0, r0, m0) = mat_plan(&bas.hojd.varden, omr.b, omr.h, 2.0);
    let mut efter = bas.clone();
    applicera_lutning_plan(&mut efter, 0.30, 135.0).unwrap();
    let (l1, r1, m1) = mat_plan(&efter.hojd.varden, omr.b, omr.h, 2.0);
    eprintln!("=== REDOVISNING plan före: {l0:.3} m/km mot {r0:.1}°; medel {m0:.3} m ===");
    eprintln!("=== REDOVISNING plan efter: {l1:.3} m/km mot {r1:.1}°; medel {m1:.3} m ===");

    for (lut, r, label) in [
        (1u8, 1u8, "lutning1 riktning1"),
        (1, 2, "lutning1 riktning2"),
        (2, 1, "lutning2 riktning1"),
        (2, 2, "lutning2 riktning2"),
    ] {
        let (mut mitt, mut trask, mut fall, mut nfall) = (0usize, 0usize, 0.0f64, 0usize);
        for fro in 1u64..=20 {
            let mut bmap = bas.clone();
            if lut >= 2 {
                applicera_lutning_plan(&mut bmap, 0.30, 135.0).unwrap();
            }
            let linjer = mittlinjer_aldre_riktning(&bmap, fro, r);
            let mut sett = vec![false; omr.b * omr.h];
            for l in &linjer {
                let p0 = l[0].0 + l[0].1 * omr.b;
                let p1 = l[l.len() - 1].0 + l[l.len() - 1].1 * omr.b;
                fall += (bmap.hojd.varden[p0] - bmap.hojd.varden[p1]) as f64;
                nfall += 1;
                for &(i, j) in l {
                    let p = i + j * omr.b;
                    if !sett[p] {
                        sett[p] = true;
                        mitt += 1;
                    }
                }
            }
            let (kart, _) = if lut >= 2 {
                karta_for_ar_lutning(&k, &tt, &omr, 500_000.0, fro, 2, r, 2).unwrap()
            } else {
                karta_for_ar(&k, &tt, &omr, 500_000.0, fro, 2, r).unwrap()
            };
            trask += kart
                .vattentyp
                .varden
                .iter()
                .filter(|&&v| v == Vatten::Trask)
                .count();
        }
        eprintln!(
            "=== {label}: fall start→slut {:.1} m; träsk/mitt {:.3} ===",
            fall / nfall as f64,
            trask as f64 / mitt as f64
        );
    }

    for t in [500_000.0, 20_000.0] {
        let (kart, and) =
            karta_for_ar_lutning(&k, &tt, &omr, t, 181, 2, 2, 2).unwrap();
        let kvot = johnson_kvot(and.as_ref().unwrap());
        let mut land = 0usize;
        let mut flod = 0usize;
        let mut trask = 0usize;
        let mut sjo = 0usize;
        let mut hav = 0usize;
        for &v in &kart.vattentyp.varden {
            match v {
                Vatten::Land => land += 1,
                Vatten::Flodfara => flod += 1,
                Vatten::Trask => trask += 1,
                Vatten::Sjo => sjo += 1,
                Vatten::Hav => hav += 1,
                _ => {}
            }
        }
        eprintln!(
            "=== fro=181 t={t} r2+l2: Johnson={kvot:.6}; Land={land} Flodfara={flod} Trask={trask} Sjo={sjo} Hav={hav} ==="
        );
    }
}
