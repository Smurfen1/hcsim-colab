//! Alt 1694 (agenternas omröstning kartstd5, Håkans Alt 1680): lövträd 0,05 och barrträd 0,20 i träsk, summan 0,50 kvar.
//! Talen är skrivna som tal (inte programmets konstanter). Larmgränsen 0,085 (eget val) prövas på fröna 1–20 vid fem tider i standardkombinationen
//! (flodversion 2, tidsmodell 2, riktning 2, lutning 2). Johnsons kvot = (barrträd + ormbunke + kottepalm) / (alla grupper utom vattenväxt).
use hckarta::{karta_for_ar_lutning, vaxt_tathet_ruta, vaxt_tathet_ruta_standard, Kalla, Omrade, Period, Tidtabell, Vatten};
use std::path::PathBuf;

#[test]
fn prov1_traskrutans_tathet() {
    let t = vaxt_tathet_ruta_standard(Vatten::Trask, Period::Aldre, 0.0, 0.0);
    let vant = [0.05f32, 0.20, 0.10, 0.05, 0.0, 0.10];
    for (i, (a, b)) in t.iter().zip(vant.iter()).enumerate() {
        assert!((a - b).abs() < 1e-6, "grupp {i}: {a} mot {b}");
    }
    let summa: f32 = t.iter().sum();
    assert!((summa - 0.50).abs() < 1e-6, "träskrutans summa {summa} ska vara 0,50");
    let kvot = f64::from(t[1] + t[3] + t[4]) / f64::from(t[0] + t[1] + t[2] + t[3] + t[4]);
    assert!((kvot - 0.625).abs() < 1e-6, "en ren träskruta ska ge 0,25/0,40 = 0,625, fick {kvot}");
}

#[test]
fn prov1b_gamla_vagen_orord() {
    // Tidsmodell 1 och lutning 1 behåller den gamla träskraden (lövträd 0, barrträd 0,25).
    let t = vaxt_tathet_ruta(Vatten::Trask, Period::Aldre, 0.0, 0.0);
    assert!((t[0] - 0.0).abs() < 1e-6 && (t[1] - 0.25).abs() < 1e-6, "gamla träskraden ändrad: {t:?}");
}

#[test]
fn prov2_larmgransen_froen_1_20() {
    let m = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = Kalla::las(&m.join("../paleokarta/data")).unwrap();
    let tt = Tidtabell::las(&std::fs::read_to_string(m.join("tidtabell.tsv")).unwrap()).unwrap();
    let omr = Omrade { lat0: 47.0, lon0: -108.0, b: 64, h: 48, ruta_m: 2000.0 };
    let mut n = 0;
    for t in [500_000.0, 20_000.0, 0.0, 289_500.0, 100_000.0] {
        for fro in 1..=20u64 {
            let (_, and) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).unwrap();
            let and = and.expect("version 2 ska ge andelar");
            let (mut si, mut sa) = (0f64, 0f64);
            for rad in &and.vaxt_tathet.varden {
                si += f64::from(rad[1] + rad[3] + rad[4]);
                sa += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
            }
            assert!(sa > 0.0, "tom läsning: frö {fro} t={t}");
            let j = si / sa;
            assert!(j < 0.085, "Johnsons kvot {j} på frö {fro} t={t} ska vara under larmgränsen 0,085");
            n += 1;
        }
    }
    assert_eq!(n, 100, "fem tider gånger 20 frön");
}
