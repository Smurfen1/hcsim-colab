//! Prov för Alt 1643 (träskbälte 2) och Alt 1644 (Johnsons kvot < 0,10 på fröna 1–20).
//! Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-johntrask.md`.
//! Tal skrivna som tal (0,10, 2, 3, 16 av 20, fröna 1–20, t = 500 000 och 20 000).
//! Stopp vid 0 fårrutor, 0 träskrutor eller en tom läsning.

use std::path::PathBuf;

use hckarta::{
    karta_for_ar_lutning, karta_for_ar_lutning_lopp_prov, mat_linjer_tm2_prov, Kalla, LoppProvMutant,
    Omrade, Tidtabell, TraskProvMutant, Vatten,
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

fn johnson(and: &hckarta::FlodAndelar) -> f64 {
    let (mut si, mut sa) = (0.0f64, 0.0f64);
    for rad in &and.vaxt_tathet.varden {
        si += f64::from(rad[1] + rad[3] + rad[4]);
        sa += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
    }
    assert!(sa > 0.0, "tom läsning");
    si / sa
}

fn chebyshev(i0: usize, j0: usize, i1: usize, j1: usize) -> usize {
    i0.abs_diff(i1).max(j0.abs_diff(j1))
}

fn far_rutor(and: &hckarta::FlodAndelar) -> usize {
    and.fara_andel
        .varden
        .iter()
        .filter(|a| **a > 0.0)
        .count()
}

fn rakna_trask_land(kart: &hckarta::Karta) -> (usize, usize) {
    let tr = kart
        .vattentyp
        .varden
        .iter()
        .filter(|v| **v == Vatten::Trask)
        .count();
    let la = kart
        .vattentyp
        .varden
        .iter()
        .filter(|v| **v == Vatten::Land)
        .count();
    assert!(la + tr > 0, "tom karta");
    (tr, la)
}

/// Korsavvik som i uppfor1604: antal fårrutor med |djup−2−stigning| > 0,01 m.
fn kors_avvik(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    riktning: u8,
    lopp: LoppProvMutant,
) -> usize {
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
    let (_bas, linjer, _) =
        mat_linjer_tm2_prov(k, tt, omr, t, fro, riktning, 2, lopp).expect("linjer");
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
    let mut n = 0usize;
    let mut farrutor = 0usize;
    for p in 0..b * hh {
        if and.fara_andel.varden[p] <= 0.0 {
            continue;
        }
        farrutor += 1;
        let dj = (kart.hojd.varden[p] - kart.vatten_yta.varden[p]) as f64;
        let s = stig[p];
        if !s.is_nan() && (dj - 2.0 - s).abs() > 0.01 {
            n += 1;
        }
    }
    assert!(farrutor > 0, "0 fårrutor fro={fro}");
    n
}

/// Steg 0 (Fables mätning): träsk per Chebyshev-avstånd 0–3, med/utan möte, bälte 0–3;
/// räkna också träskrutor med höjd ≥ närmaste mittlinjeyta + 1 m.
#[test]
#[ignore]
fn steg0_matning() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fron: Vec<u64> = (1..=20).collect();
    let b = omr.b;

    println!("avst\tmote\tt\tfro\ttrask\tland\tandel\tolagliga\td0\td1\td2\td3\tover3");
    let mut stopp_olagliga = false;

    for avst in 0usize..=3 {
        for mote in [false, true] {
            let lm = if mote {
                LoppProvMutant::MedMote
            } else {
                LoppProvMutant::MotetAvslaget
            };
            let tm = TraskProvMutant::Avstand(avst);
            for &t in &[500_000.0_f64, 20_000.0] {
                let mut sum_olag = 0usize;
                let mut sum_d = [0usize; 5];
                let mut sum_tr = 0usize;
                let mut sum_la = 0usize;
                for &fro in &fron {
                    let (kart, and) = karta_for_ar_lutning_lopp_prov(
                        &k, &tt, &omr, t, fro, 2, 2, 2, tm, lm,
                    )
                    .expect("karta");
                    let and = and.as_ref().expect("andelar saknas");
                    assert!(far_rutor(and) > 0, "0 fårrutor fro={fro} t={t}");
                    let (tr, la) = rakna_trask_land(&kart);
                    let andel = if la + tr > 0 {
                        tr as f64 / (la + tr) as f64
                    } else {
                        0.0
                    };
                    let (_, linjer, _) =
                        mat_linjer_tm2_prov(&k, &tt, &omr, t, fro, 2, 2, lm).expect("linjer");
                    // Yta på mittlinjerutor efter kartbygget (Land med satt vatten_yta).
                    let mut mitt: Vec<(usize, usize, f32)> = Vec::new();
                    for path in &linjer {
                        for &(i, j) in path {
                            let p = i + j * b;
                            if kart.hojd.varden[p] < 0.0 {
                                continue;
                            }
                            mitt.push((i, j, kart.vatten_yta.varden[p]));
                        }
                    }
                    assert!(!mitt.is_empty(), "0 mittlinjerutor fro={fro} t={t}");
                    let mut olag = 0usize;
                    let mut dbin = [0usize; 5];
                    for j in 0..omr.h {
                        for i in 0..b {
                            let p = i + j * b;
                            if kart.vattentyp.varden[p] != Vatten::Trask {
                                continue;
                            }
                            let mut best: Option<(usize, f32)> = None;
                            for &(mi, mj, my) in &mitt {
                                let d = chebyshev(i, j, mi, mj);
                                best = Some(match best {
                                    None => (d, my),
                                    Some((d0, _)) if d < d0 => (d, my),
                                    Some((d0, y0)) if d == d0 => (d, y0.min(my)),
                                    Some(v) => v,
                                });
                            }
                            let (d, yta) = best.expect("mitt");
                            if d <= 3 {
                                dbin[d] += 1;
                            } else {
                                dbin[4] += 1;
                            }
                            // Bältesträsk har vatten_yta = höjd − 0,5; övergivna har vatten_yta = höjd.
                            // Regeln som förbjuder höjd ≥ yta+1 gäller bältesträsket (Alt 1501/1605).
                            let balte = (kart.vatten_yta.varden[p] - (kart.hojd.varden[p] - 0.5))
                                .abs()
                                < 1e-3;
                            if balte && kart.hojd.varden[p] >= yta + 1.0 {
                                olag += 1;
                            }
                        }
                    }
                    sum_olag += olag;
                    for i in 0..5 {
                        sum_d[i] += dbin[i];
                    }
                    sum_tr += tr;
                    sum_la += la;
                    println!(
                        "{avst}\t{mote}\t{t}\t{fro}\t{tr}\t{la}\t{andel:.4}\t{olag}\t{}\t{}\t{}\t{}\t{}",
                        dbin[0], dbin[1], dbin[2], dbin[3], dbin[4]
                    );
                }
                println!(
                    "summa\tavst={avst}\tmote={mote}\tt={t}\ttrask={sum_tr}\tland={sum_la}\tandel={:.4}\tolagliga={sum_olag}\td0={}\td1={}\td2={}\td3={}\tover3={}",
                    sum_tr as f64 / (sum_tr + sum_la) as f64,
                    sum_d[0],
                    sum_d[1],
                    sum_d[2],
                    sum_d[3],
                    sum_d[4]
                );
                if mote && sum_olag > 0 {
                    eprintln!(
                        "STOPP: {sum_olag} olagliga träskrutor (höjd ≥ yta+1 m) med mötet på, avst={avst} t={t}"
                    );
                    stopp_olagliga = true;
                }
            }
        }
    }
    assert!(
        !stopp_olagliga,
        "kodfel i Alt 1605: träskrutor ovanför närmaste mittlinjeyta + 1 m med mötet på"
    );
}

/// Alt 1644: Johnsons kvot under 0,10 i varje frö 1–20 (Aldre t=500000, Ovre t=20000).
#[test]
fn prov1_johnsons_kvot_fro_1_20() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for &t in &[500_000.0_f64, 20_000.0] {
        for fro in 1u64..=20 {
            let (kart, and) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).expect("karta");
            let and = and.as_ref().expect("andelar saknas");
            assert!(far_rutor(and) > 0, "0 fårrutor fro={fro} t={t}");
            let (tr, la) = rakna_trask_land(&kart);
            assert!(tr > 0, "0 träskrutor fro={fro} t={t}");
            let j = johnson(and);
            let andel = tr as f64 / (la + tr) as f64;
            eprintln!("prov1 t={t} fro={fro} johnson={j:.6} trask={tr} andel={andel:.4}");
            assert!(
                j < 0.10,
                "Johnsons kvot {j} ska vara < 0,10 (fro={fro} t={t})"
            );
        }
    }
}

/// Mutant: avstånd 4 ger minst 16 av 20 frön med Johnson ≥ 0,10 i Aldre. Förut avstånd 3; efter Alt 1694 (träskets täthet lövträd 0,05,
/// barrträd 0,20) gav avstånd 3 bara 9 av 20, så felet med flit gjordes grövre i stället för att kravet 16 sänktes (Claude, 2026-11-22).
#[test]
fn prov2_mutant_avstand_4_rod_i_aldre() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let t = 500_000.0_f64;
    let mut over = 0usize;
    for fro in 1u64..=20 {
        let (kart, and) = karta_for_ar_lutning_lopp_prov(
            &k,
            &tt,
            &omr,
            t,
            fro,
            2,
            2,
            2,
            TraskProvMutant::Avstand(4),
            LoppProvMutant::Ingen,
        )
        .expect("karta");
        let and = and.as_ref().expect("andelar");
        assert!(far_rutor(and) > 0, "0 fårrutor");
        let (tr, _) = rakna_trask_land(&kart);
        assert!(tr > 0, "0 träskrutor");
        let j = johnson(and);
        if j >= 0.10 {
            over += 1;
        }
        eprintln!("prov2 mutant avstånd 4 fro={fro} johnson={j:.6}");
    }
    eprintln!("prov2: {over} av 20 frön ≥ 0,10 (kräver minst 16)");
    assert!(
        over >= 16,
        "mutant avstånd 4 ska ge minst 16 av 20 röda i Aldre, fick {over}"
    );
}

/// Rapportmätning: Johnson + träskandel per frö (1–20 och 181).
/// `TRASK1643_FAS=fore` → Avstand(3)+MotetAvslaget (dagens kod före steg 1).
/// `TRASK1643_FAS=efter1` → Ingen+MotetAvslaget (träskbälte 2, möte av).
/// `TRASK1643_FAS=efter2` → Ingen+MedMote (träskbälte 2, möte på; = produktion efter steg 2).
#[test]
#[ignore]
fn matning_johnson_rapport() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fas = std::env::var("TRASK1643_FAS").unwrap_or_else(|_| "efter2".into());
    let (tm, lm) = match fas.as_str() {
        "fore" => (TraskProvMutant::Avstand(3), LoppProvMutant::MotetAvslaget),
        "efter1" => (TraskProvMutant::Ingen, LoppProvMutant::MotetAvslaget),
        _ => (TraskProvMutant::Ingen, LoppProvMutant::MedMote),
    };
    let mut fron: Vec<u64> = (1..=20).collect();
    fron.push(181);
    println!("fas\tt\tfro\tjohnson\ttrask\tland\tandel");
    for &t in &[500_000.0_f64, 20_000.0] {
        for &fro in &fron {
            let (kart, and) = karta_for_ar_lutning_lopp_prov(
                &k, &tt, &omr, t, fro, 2, 2, 2, tm, lm,
            )
            .expect("karta");
            let and = and.as_ref().expect("andelar");
            assert!(far_rutor(and) > 0, "0 fårrutor");
            let (tr, la) = rakna_trask_land(&kart);
            let j = johnson(and);
            let andel = tr as f64 / (la + tr) as f64;
            println!("{fas}\t{t}\t{fro}\t{j:.6}\t{tr}\t{la}\t{andel:.4}");
        }
    }
}

/// Mötet avslaget ger korsavvik > 0 (samma mått som uppfor1604 prov 3).
#[test]
fn prov3_motet_avslaget_rod_i_korsning() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let mut avvik = 0usize;
    for &t in &[500_000.0_f64, 20_000.0] {
        for fro in 1u64..=20 {
            avvik += kors_avvik(&k, &tt, &omr, t, fro, 1, LoppProvMutant::MotetAvslaget);
        }
    }
    eprintln!("prov3 MotetAvslaget: kors_avvik-summa={avvik}");
    assert!(
        avvik > 0,
        "mötet avslaget ska ge rött i korsningarna (kors_avvik > 0)"
    );
}
