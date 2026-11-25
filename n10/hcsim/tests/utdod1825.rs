//! Rättningarna mot utdöendet i visningskörningen (Håkans Alt 1825; agenternas Alt 1797, 1799, 1800, 1802). Talen i proven är skrivna som
//! tal ur mätningarna (sandbox/pi work/hcsim/utdod/): utan rättningarna dör världen med frö 261 år 21; med dem lever den.
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::sim::{KantLage, Sim};
use hcsim::tal::STEG_PER_AR;
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hckarta::{Kalla, Tidtabell};
use std::path::PathBuf;

fn varld(rattad: bool) -> Sim {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = Kalla::las(&rot.join("../paleokarta/data")).unwrap();
    let tt = Tidtabell::las(&std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).unwrap()).unwrap();
    let arm = utforsk_armar().into_iter().find(|x| x.namn == "e_lar" && !x.spegel).expect("armen e_lar");
    let mut sim = bygg_sim_balte(&k, &tt, B_FULL, H_FULL, BALTE_K, 261, &arm).unwrap();
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    if rattad {
        satt_rattningar_utdod(&mut sim);
    }
    sim
}

fn kor_till_ar(sim: &mut Sim, ar: i64) {
    while sim.t < ar * STEG_PER_AR && sim.antal() > 0 {
        sim.steg();
    }
}

#[test]
fn prov1_vaxlarna_och_startenergin() {
    let mut sim = varld(false);
    assert!(sim.antal() > 0, "tom läsning: inga djur vid start");
    let fore: Vec<f64> = sim.djur.iter().map(|d| d.e).collect();
    satt_rattningar_utdod(&mut sim);
    assert!(sim.intag_per_fyllnad && sim.hona_vuxen && sim.fodsel_arlig);
    assert_eq!(sim.mogen_steg, Some(450));
    assert_eq!(sim.fodsel_troskel, Some(120.0));
    assert!((sim.e_skala - 0.15625).abs() < 1e-12);
    for (d, f) in sim.djur.iter().zip(&fore) {
        assert!((d.e - f * 0.15625).abs() < 1e-9, "startenergin ska skalas");
        assert!(d.e <= 150.0 + 1e-9, "ingen startenergi över taket 150");
    }
}

/// Rött prov och grönt prov i samma körning: utan rättningarna ska världen vara död före år 22 (mätt: år 21 i 20 av 20 frön);
/// med dem ska den leva år 25 med minst 20 % honor (gränsen i Alt 1802).
#[test]
fn prov2_varlden_dor_utan_och_lever_med() {
    let mut utan = varld(false);
    kor_till_ar(&mut utan, 22);
    assert_eq!(utan.antal(), 0, "utan rättningarna ska världen vara död före år 22");
    let mut med = varld(true);
    kor_till_ar(&mut med, 25);
    let lev = med.djur.iter().filter(|d| d.lev).count();
    let honor = med.djur.iter().filter(|d| d.lev && d.hona).count();
    assert!(lev > 0, "med rättningarna ska världen leva år 25");
    assert!(honor * 5 >= lev, "minst 20 % honor år 25: {honor} av {lev}");
}
