//! Alt 1679 och 1673 (Håkans beslut 2026-11-22): hjärnan med valfri storlek räknar som `Hj` vid standardstorleken,
//! och skalan från antal nervceller ger de tal som beslutet säger (skrivna som tal här, inte ur programmets konstanter).

use hcsim::hjarna::Hj;
use hcsim::hjarnstorlek::{z_ur_nervceller, HjS, Storlek};
use hcsim::Rng;

fn sinnen(g: &mut Rng, n: usize) -> Vec<f64> {
    (0..n).map(|_| 2.0 * g.f() - 1.0).collect()
}

/// Kör `steg` steg med samma gener och samma sinnen; ger antalet steg där valen skiljer sig och om vikterna är lika.
fn jamfor(lar_hj: bool, lar_hjs: bool, steg: usize) -> (usize, bool) {
    let mut g = Rng::new(1679);
    let mut hj = Hj::slump(&mut g);
    let mut hjs = HjS::fran_gen(Storlek::STANDARD, hj.gen.clone());
    let mut olika = 0;
    for _ in 0..steg {
        let s = sinnen(&mut g, 19);
        if hj.valj(&s, lar_hj, 0) != hjs.valj(&s, lar_hjs) {
            olika += 1;
        }
    }
    let lika_vikter = hj.wp.len() == hjs.wp.len() && hj.wp.iter().zip(&hjs.wp).all(|(a, b)| a.to_bits() == b.to_bits());
    (olika, lika_vikter)
}

#[test]
fn standardstorleken_ar_dagens() {
    assert_eq!(Storlek::STANDARD, Storlek { s: 19, z: 4, a: 9 });
    assert_eq!(Storlek::STANDARD.n_gen(), 4 * 20 + 9 * 4 * 5 + 5);
    assert_eq!(Storlek::STANDARD.n_gen(), hcsim::hjarna::N_GEN);
    assert_eq!(Storlek::STANDARD.ops(), hcsim::hjarna::OPS);
}

#[test]
fn samma_val_och_vikter_som_hj() {
    let (olika, lika_vikter) = jamfor(true, true, 5000);
    assert_eq!(olika, 0, "valen skiljer sig från Hj");
    assert!(lika_vikter, "förutsägarens vikter skiljer sig från Hj");
}

/// Rödprov: jämförelsen ska slå larm när den ena hjärnan inte lär sig.
#[test]
fn jamforelsen_ser_en_skillnad() {
    let (_, lika_vikter) = jamfor(true, false, 5000);
    assert!(!lika_vikter, "jämförelsen såg inte att den ena hjärnan inte lärde sig");
}

#[test]
fn skalan_alt_1673() {
    assert_eq!(z_ur_nervceller(199_380.0), 2); // bananflugan (Raji och Potter 2021)
    assert_eq!(z_ur_nervceller(225_911.0), 2); // Culex quinquefasciatus (samma källa)
    assert_eq!(z_ur_nervceller(1.0e6), 3);
    assert_eq!(z_ur_nervceller(1.0e7), 4);
    assert_eq!(z_ur_nervceller(1.0e8), 5);
    assert_eq!(z_ur_nervceller(1.0e11), 8);
    assert_eq!(z_ur_nervceller(1.0e15), 8); // taket
    assert_eq!(z_ur_nervceller(10.0), 2); // golvet
    assert!(z_ur_nervceller(1.0e9) > z_ur_nervceller(199_380.0), "en stor hjärna och en fluga får samma Z");
}

#[test]
fn liten_hjarna_valjer_inom_sina_handlingar() {
    let st = Storlek { s: 8, z: 2, a: 5 };
    assert_eq!(st.n_gen(), 2 * 9 + 5 * 2 * 3 + 3);
    let mut g = Rng::new(7);
    let mut h = HjS::slump(st, &mut g);
    for _ in 0..2000 {
        let s = sinnen(&mut g, 8);
        assert!(h.valj(&s, true) < 5);
    }
    assert!(h.wp.iter().all(|x| x.is_finite()));
    assert_eq!(h.steg, 2000);
}
