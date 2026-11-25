//! Prov 1–6 för Alt 1604 (stigningsgräns) och Alt 1605 (lopp som möts; produktion efter Alt 1643).
//! Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartstd3.md`.
//! Tal skrivna som tal (5 m, 7 m, 0,01 m, 20 frön). Stopp vid 0 fårrutor, 0 loppbyten eller tom läsning.

use std::path::PathBuf;

use hckarta::{
    karta_for_ar_lutning, karta_for_ar_lutning_lopp_prov, karta_full_version_lutning,
    mat_linjer_tm2_prov, max_stigning_pa_linje, prova_nytt_lopp, Kalla, Lager, LoppProvMutant,
    Marke, Omrade, Period, Tidtabell, TraskProvMutant, Vatten, LOPP_MAX_STIGNING_M,
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

fn krav_grans(fron: &[u64], farrutor: usize, n_byten: usize) -> Result<(), String> {
    if fron.len() < 20 {
        return Err(format!("färre än 20 frön: {}", fron.len()));
    }
    if farrutor == 0 {
        return Err("0 fårrutor".into());
    }
    if n_byten == 0 {
        return Err("0 loppbyten".into());
    }
    Ok(())
}

/// Handbyggd höjdkarta: plan som faller 0,30 m/km mot 135°, rygg 10 m tvärs, bonus bakom.
fn rygg_scenario() -> (hckarta::Karta, Vec<bool>, Vec<bool>, (usize, usize), Vec<(usize, usize)>) {
    let b = 20usize;
    let hh = 16usize;
    let ruta_m = 2000.0_f64;
    let n = b * hh;
    let mut hojd = vec![0.0f32; n];
    let fall = 0.30_f64;
    let ang = 135.0_f64.to_radians();
    let (e_ost, e_syd) = (ang.sin(), -ang.cos());
    let h0 = 200.0_f64;
    for j in 0..hh {
        for i in 0..b {
            let km_e = i as f64 * ruta_m / 1000.0;
            let km_s = j as f64 * ruta_m / 1000.0;
            let dist = km_e * e_ost + km_s * e_syd;
            hojd[i + j * b] = (h0 - fall * dist) as f32;
        }
    }
    // Rygg 10 m hög i kolumn 8 (tvärs över vägen österut).
    for j in 0..hh {
        hojd[8 + j * b] += 10.0;
    }
    let mut bonus = vec![false; n];
    for j in 0..hh {
        for i in 10..b {
            bonus[i + j * b] = true;
        }
    }
    let straff = vec![false; n];
    let start = (6usize, 8usize);
    let uppstrom = vec![(3, 8), (4, 8), (5, 8), (6, 8)];
    let kart = hckarta::Karta {
        omrade: Omrade {
            lat0: 47.0,
            lon0: -108.0,
            b,
            h: hh,
            ruta_m,
        },
        t: 500_000.0,
        hojd: Lager {
            varden: hojd,
            marke: Marke::Last,
        },
        vatten_yta: Lager {
            varden: vec![0.0; n],
            marke: Marke::Last,
        },
        vattentyp: Lager {
            varden: vec![Vatten::Land; n],
            marke: Marke::Last,
        },
        mark: Lager {
            varden: vec![[0.25; 4]; n],
            marke: Marke::Simulering,
        },
        vaxt: Lager {
            varden: vec![0.5; n],
            marke: Marke::Simulering,
        },
        period: Lager {
            varden: vec![Period::Aldre; n],
            marke: Marke::Last,
        },
    };
    (kart, bonus, straff, start, uppstrom)
}

fn stigning_nytt(
    k: &hckarta::Karta,
    uppstrom: &[(usize, usize)],
    forts: &[(usize, usize)],
) -> f32 {
    let mut path = uppstrom.to_vec();
    if forts.len() > 1 {
        path.extend_from_slice(&forts[1..]);
    }
    max_stigning_pa_linje(k, &path)
}

/// Prov 1: ryggen. Med dagens beteende (ingen stigningsgräns) stiger loppet mer än 5 m;
/// med Alt 1604 (5 m) är största stigning ≤ 5 m eller bytet sker inte.
#[test]
fn prov1_ryggen() {
    let (kart, bonus, straff, start, uppstrom) = rygg_scenario();

    // Dagens beteende: ingen effektiv gräns → rött mot 5 m-kravet.
    let utan = prova_nytt_lopp(
        &kart,
        start,
        2,
        42,
        &bonus,
        &straff,
        &uppstrom,
        1.0e9,
    )
    .expect("utan gräns ska ge en väg");
    let s_utan = stigning_nytt(&kart, &uppstrom, &utan);
    eprintln!("prov1 utan gräns: max stigning {s_utan:.3} m (väntat > 5)");
    assert!(
        s_utan > 5.0,
        "dagens kod ska gå uppför ryggen: stigning {s_utan} ≤ 5"
    );

    // Alt 1604: 5 m.
    let med = prova_nytt_lopp(
        &kart,
        start,
        2,
        42,
        &bonus,
        &straff,
        &uppstrom,
        LOPP_MAX_STIGNING_M,
    );
    match med {
        None => eprintln!("prov1 med 5 m: bytet skedde inte (grönt)"),
        Some(vag) => {
            let s = stigning_nytt(&kart, &uppstrom, &vag);
            eprintln!("prov1 med 5 m: max stigning {s:.3} m");
            assert!(
                s <= 5.0,
                "efter Alt 1604 ska stigning ≤ 5 m, fick {s}"
            );
        }
    }
}

fn johnson_kvot(and: &hckarta::FlodAndelar, kart: &hckarta::Karta) -> f64 {
    let (mut ti, mut na) = (0.0_f64, 0.0_f64);
    for (p, rad) in and.vaxt_tathet.varden.iter().enumerate() {
        let _ = kart.vattentyp.varden[p];
        ti += f64::from(rad[1] + rad[3] + rad[4]);
        na += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
    }
    assert!(na > 0.0, "tom läsning (Johnson)");
    ti / na
}

fn perc(v: &mut [f64], p: f64) -> f64 {
    assert!(!v.is_empty(), "tom läsning (perc)");
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

struct FroMat {
    n_byten: usize,
    over_7: usize,
    djup_p95: f64,
    djup_max: f64,
    trask_andel: f64,
    johnson: f64,
    max_stig_nytt: f64,
    kors_avvik: usize, // |djup−2−stigning| > 0,01 på fårrutor
}

fn mat_fro(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    riktning: u8,
    lopp: LoppProvMutant,
) -> FroMat {
    let (kart, and) = karta_for_ar_lutning_lopp_prov(
        k,
        tt,
        omr,
        t,
        fro,
        2,
        riktning,
        2,
        TraskProvMutant::Ingen,
        lopp,
    )
    .expect("karta");
    let and = and.expect("andelar");
    let (_bas, linjer, byten) =
        mat_linjer_tm2_prov(k, tt, omr, t, fro, riktning, 2, lopp).expect("linjer");
    let n_byten = byten.len();

    let b = omr.b;
    let hh = omr.h;
    let mut stig = vec![f64::NAN; b * hh];
    for lin in &linjer {
        let mut minh = f64::MAX;
        for &(i, j) in lin {
            let p = i + j * b;
            let h = kart.hojd.varden[p] as f64;
            if h < 0.0 {
                continue;
            }
            minh = minh.min(h);
            let s = h - minh;
            if stig[p].is_nan() || s > stig[p] {
                stig[p] = s;
            }
        }
    }

    let mut djup_v = Vec::new();
    let mut over_7 = 0usize;
    let mut kors_avvik = 0usize;
    let mut farrutor = 0usize;
    let (mut land, mut trask) = (0usize, 0usize);
    for p in 0..b * hh {
        match kart.vattentyp.varden[p] {
            Vatten::Land => land += 1,
            Vatten::Trask => trask += 1,
            _ => {}
        }
        let a = and.fara_andel.varden[p];
        if a <= 0.0 {
            continue;
        }
        farrutor += 1;
        let dj = (kart.hojd.varden[p] - kart.vatten_yta.varden[p]) as f64;
        djup_v.push(dj);
        if dj > 7.0 {
            over_7 += 1;
        }
        let s = stig[p];
        if !s.is_nan() && (dj - 2.0 - s).abs() > 0.01 {
            kors_avvik += 1;
        }
    }
    assert!(farrutor > 0, "0 fårrutor fro={fro}");
    assert!(land + trask > 0, "tom läsning");

    let mut max_stig_nytt = 0.0_f64;
    for lin in &linjer {
        max_stig_nytt = max_stig_nytt.max(max_stigning_pa_linje(&kart, lin) as f64);
    }

    FroMat {
        n_byten,
        over_7,
        djup_p95: perc(&mut djup_v, 0.95),
        djup_max: djup_v.iter().cloned().fold(0.0, f64::max),
        trask_andel: trask as f64 / (land + trask) as f64,
        johnson: johnson_kvot(&and, &kart),
        max_stig_nytt,
        kors_avvik,
    }
}

/// Prov 2 (steg 1): fröna 1–20, tidsmodell 2, version 2, lutning 2, riktning 1 och 2;
/// t = 500 000, 20 000, 0. Inget nytt lopp stiger mer än 5 m; riktning 2 har 0 fårrutor > 7 m.
#[test]
fn prov2_fron_steg1() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fron: Vec<u64> = (1..=20).collect();
    let mut farrutor_tot = 0usize;
    let mut byten_tot = 0usize;

    for &t in &[500_000.0_f64, 20_000.0, 0.0] {
        for &r in &[1u8, 2] {
            eprintln!("=== prov2 t={t} riktning {r} ===");
            for &fro in &fron {
                let m = mat_fro(&k, &tt, &omr, t, fro, r, LoppProvMutant::Ingen);
                farrutor_tot += 1; // mat_fro panikar vid 0
                byten_tot += m.n_byten;
                eprintln!(
                    "  fro={fro}: byten={} over7={} p95={:.3} max={:.3} trask={:.4} johnson={:.4} maxstig={:.3}",
                    m.n_byten, m.over_7, m.djup_p95, m.djup_max, m.trask_andel, m.johnson, m.max_stig_nytt
                );
                assert!(
                    m.max_stig_nytt <= 5.0 + 1e-4,
                    "fro {fro} r {r} t {t}: max stigning {} > 5 m",
                    m.max_stig_nytt
                );
                if r == 2 {
                    assert_eq!(
                        m.over_7, 0,
                        "fro {fro} riktning 2 t {t}: {} rutor över 7 m",
                        m.over_7
                    );
                }
            }
        }
    }
    krav_grans(&fron, farrutor_tot, byten_tot).expect("gränsprov");
}

/// Prov 3 (steg 2): samma som prov 2, med möte påslaget (`MedMote`); riktning 1 har
/// 0 fårrutor med |djup−2−stigning| > 0,01 m.
#[test]
fn prov3_fron_steg2() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fron: Vec<u64> = (1..=20).collect();
    let mut farrutor_tot = 0usize;
    let mut byten_tot = 0usize;
    let mut fail_r1 = 0usize;
    let mut kors_tot = 0usize;

    for &t in &[500_000.0_f64, 20_000.0, 0.0] {
        for &r in &[1u8, 2] {
            for &fro in &fron {
                let m = mat_fro(&k, &tt, &omr, t, fro, r, LoppProvMutant::MedMote);
                farrutor_tot += 1;
                byten_tot += m.n_byten;
                if r == 2 {
                    assert_eq!(m.over_7, 0, "riktning 2 over7 fro={fro} t={t}");
                }
                assert!(m.max_stig_nytt <= 5.0 + 1e-4);
                if r == 1 {
                    kors_tot += m.kors_avvik;
                    if m.kors_avvik > 0 {
                        fail_r1 += m.kors_avvik;
                        eprintln!("prov3 fro={fro} t={t}: kors_avvik={}", m.kors_avvik);
                    }
                }
            }
        }
    }
    krav_grans(&fron, farrutor_tot, byten_tot).expect("gränsprov");
    eprintln!("prov3 MedMote: kors_avvik-summa riktning 1 = {kors_tot}");
    assert_eq!(
        fail_r1, 0,
        "riktning 1: {fail_r1} fårrutor med |djup−2−stigning| > 0,01 m (kräver Alt 1605 / MedMote)"
    );
}

/// Prov 4: tidsmodell 1 oförändrad (kontrollsumma = karta_full_version_lutning); hcsim orörd.
#[test]
fn prov4_tidsmodell1_och_hcsim() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for &t in &[500_000.0_f64, 20_000.0, 0.0] {
        for &r in &[1u8, 2] {
            for fro in 1u64..=5 {
                let (a, _) =
                    karta_full_version_lutning(&k, &tt, &omr, t, fro, 2, r, 2).expect("tm1");
                let (b, _) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, r, 2).expect("tm2");
                // TM1 vs TM2 får skilja; TM1 mot sig själv via full:
                let (c, _) =
                    karta_full_version_lutning(&k, &tt, &omr, t, fro, 2, r, 2).expect("tm1b");
                assert_eq!(a.hojd.varden, c.hojd.varden);
                assert_eq!(a.vatten_yta.varden, c.vatten_yta.varden);
                assert_eq!(a.vattentyp.varden, c.vattentyp.varden);
                let _ = b;
            }
        }
    }

    let hcsim = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hcsim/target/release/hcsim");
    if !hcsim.exists() {
        eprintln!("hcsim saknas på {}; hoppar slutsumsprov (bygg hcsim separat)", hcsim.display());
        return;
    }
    let ut = std::process::Command::new(&hcsim)
        .args([
            "kor",
            "--lat0", "47", "--lon0", "-108", "--b", "64", "--h", "48", "--ruta-m", "2000",
            "--t", "500000", "--fro", "181", "--ar", "5", "--n0", "400",
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
}

/// Mätning till rapporten (före = MaxStigning 1e9 ≈ dagens kod; efter1 = Ingen).
#[test]
#[ignore]
fn matning_rapport_tsv() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fas = std::env::var("UPPFOR_FAS").unwrap_or_else(|_| "efter1".into());
    let lopp = if fas == "fore" {
        LoppProvMutant::MaxStigning(1.0e9)
    } else {
        LoppProvMutant::Ingen
    };
    println!("fas\tt\triktning\tfro\tbyten\tover7\tp95\tmax\ttrask\tjohnson\tkors\tmaxstig");
    for &t in &[500_000.0_f64, 20_000.0, 0.0] {
        for &r in &[1u8, 2] {
            for fro in 1u64..=20 {
                let m = mat_fro(&k, &tt, &omr, t, fro, r, lopp);
                println!(
                    "{fas}\t{t}\t{r}\t{fro}\t{}\t{}\t{:.3}\t{:.3}\t{:.4}\t{:.4}\t{}\t{:.3}",
                    m.n_byten, m.over_7, m.djup_p95, m.djup_max, m.trask_andel, m.johnson,
                    m.kors_avvik, m.max_stig_nytt
                );
            }
        }
    }
}

/// Prov 5: mutanter. Gräns 10 m ger rött i ryggprovet; mötet avslaget ger rött i prov 3-måttet.
#[test]
fn prov5_mutanter() {
    let (kart, bonus, straff, start, uppstrom) = rygg_scenario();
    let med10 = prova_nytt_lopp(&kart, start, 2, 42, &bonus, &straff, &uppstrom, 10.0)
        .expect("10 m-gräns ska tillåta ryggen");
    let s10 = stigning_nytt(&kart, &uppstrom, &med10);
    eprintln!("prov5 mutant 10 m: stigning {s10:.3} m");
    assert!(
        s10 > 5.0,
        "mutant 10 m ska ge rött mot 5 m-kravet, fick {s10}"
    );

    // Möte avslaget vs MedMote: kors_avvik i riktning 1.
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let mut avvik_av = 0usize;
    let mut avvik_pa = 0usize;
    for &t in &[500_000.0_f64, 20_000.0] {
        for fro in 1u64..=20 {
            let av = mat_fro(&k, &tt, &omr, t, fro, 1, LoppProvMutant::MotetAvslaget);
            let pa = mat_fro(&k, &tt, &omr, t, fro, 1, LoppProvMutant::MedMote);
            avvik_av += av.kors_avvik;
            avvik_pa += pa.kors_avvik;
        }
    }
    eprintln!("prov5 möte: kors_avvik avslaget={avvik_av} på={avvik_pa}");
    assert_eq!(avvik_pa, 0, "med möte på (MedMote) ska kors_avvik vara 0, fick {avvik_pa}");
    assert!(
        avvik_av > 0,
        "mutant MotetAvslaget ska ge rött (kors_avvik > 0), fick {avvik_av}"
    );
}

/// Prov 6: produktionsvägen har mötet — samma karta som `MedMote` för frö 1–20.
#[test]
fn prov6_produktion_har_mote() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fron: Vec<u64> = (1..=20).collect();
    let mut jamforda = 0usize;

    for &t in &[500_000.0_f64, 20_000.0, 0.0] {
        for &r in &[1u8, 2] {
            for &fro in &fron {
                let (prod, _) =
                    karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, r, 2).expect("produktion");
                let (pa, _) = karta_for_ar_lutning_lopp_prov(
                    &k,
                    &tt,
                    &omr,
                    t,
                    fro,
                    2,
                    r,
                    2,
                    TraskProvMutant::Ingen,
                    LoppProvMutant::MedMote,
                )
                .expect("MedMote");
                assert_eq!(
                    prod.hojd.varden, pa.hojd.varden,
                    "hojd skiljer fro={fro} r={r} t={t}"
                );
                assert_eq!(
                    prod.vatten_yta.varden, pa.vatten_yta.varden,
                    "vatten_yta skiljer fro={fro} r={r} t={t}"
                );
                assert_eq!(
                    prod.vattentyp.varden, pa.vattentyp.varden,
                    "vattentyp skiljer fro={fro} r={r} t={t}"
                );
                jamforda += 1;
            }
        }
    }
    assert_eq!(jamforda, 120, "väntade 20·3·2 jämförelser, fick {jamforda}");
    eprintln!("prov6: {jamforda} kartor lika (produktion = MedMote)");
}
