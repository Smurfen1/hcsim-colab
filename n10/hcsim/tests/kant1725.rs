//! Kartans kant (Håkans Alt 1712 och 1725; agenternas Alt 1717–1719). Strykprovet (piharness docs/evolution/kantvagg-strykprov.txt) visade
//! att varje stängd kant samlar djuren och att sammanfogade sidor ger jämn fördelning. Talen i proven är skrivna som tal.
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::bild_json;
use hcsim::sim::{KantLage, Sim};
use hcsim::tal::STEG_PER_AR;
use hcsim::utforsk::{bygg_sim, bygg_sim_balte};
use hckarta::{Kalla, Tidtabell};
use std::path::PathBuf;

fn kalla_tt() -> (Kalla, Tidtabell) {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = Kalla::las(&rot.join("../paleokarta/data")).unwrap();
    let tt = Tidtabell::las(&std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).unwrap()).unwrap();
    (k, tt)
}

fn arm() -> hcsim::formaga::HjarnArm {
    utforsk_armar().into_iter().find(|x| x.namn == "e_lar" && !x.spegel).expect("armen e_lar")
}

#[test]
fn prov1_granne_vid_kanten() {
    let (k, tt) = kalla_tt();
    let mut sim = bygg_sim(&k, &tt, 64, 48, 1, &arm()).unwrap();
    assert_eq!(sim.kant, KantLage::Vagg, "standard ska vara vägg");
    assert_eq!(sim.kant_granne(-1, 0), (-1, 0));
    assert_eq!(sim.kant_granne(64, 47), (64, 47));
    sim.kant = KantLage::Torus;
    assert_eq!(sim.kant_granne(-1, 0), (63, 0));
    assert_eq!(sim.kant_granne(64, 48), (0, 0));
    assert_eq!(sim.kant_granne(5, -1), (5, 47));
}

#[test]
fn prov2_fonstret_i_kartbilden() {
    let (k, tt) = kalla_tt();
    let mut sim = bygg_sim_balte(&k, &tt, 64, 48, 8, 1, &arm()).unwrap();
    assert_eq!((sim.b, sim.h), (80, 64), "kartan med bältet");
    sim.fonster_k = 8;
    let j: serde_json::Value = serde_json::from_str(&bild_json(&sim, "prov")).unwrap();
    assert_eq!((j["b"].as_u64(), j["h"].as_u64()), (Some(64), Some(48)));
    assert_eq!(j["v"].as_array().unwrap().len(), 64 * 48);
    let djur = j["djur"].as_array().unwrap();
    assert!(!djur.is_empty(), "tom läsning: inga djur i fönstret");
    for d in djur {
        assert!(d[0].as_u64().unwrap() < 64 && d[1].as_u64().unwrap() < 48, "djur utanför fönstret: {d}");
    }
    // Utan bälte är fönstret hela kartan (som förut).
    let sim0 = bygg_sim(&k, &tt, 64, 48, 1, &arm()).unwrap();
    let j0: serde_json::Value = serde_json::from_str(&bild_json(&sim0, "prov")).unwrap();
    assert_eq!((j0["b"].as_u64(), j0["h"].as_u64()), (Some(64), Some(48)));
}

/// Djur per ruta på avstånd 0 från kanten delat med avstånd `inne`, i rutorna `x0..x0+b`, `y0..y0+h` av kartan.
fn kvot(sim: &Sim, x0: usize, y0: usize, b: usize, h: usize, inne: usize) -> f64 {
    let (mut n, mut r) = ([0usize; 2], [0usize; 2]);
    for j in 0..h {
        for i in 0..b {
            let d = i.min(j).min(b - 1 - i).min(h - 1 - j);
            if d == 0 { r[0] += 1 } else if d == inne { r[1] += 1 }
        }
    }
    for dj in sim.djur.iter().filter(|d| d.lev) {
        if dj.i < x0 || dj.j < y0 || dj.i >= x0 + b || dj.j >= y0 + h { continue }
        let (i, j) = (dj.i - x0, dj.j - y0);
        let d = i.min(j).min(b - 1 - i).min(h - 1 - j);
        if d == 0 { n[0] += 1 } else if d == inne { n[1] += 1 }
    }
    assert!(n[0] + n[1] > 0, "tom läsning");
    (n[0] as f64 / r[0] as f64) / (n[1] as f64 / r[1] as f64).max(1e-9)
}

fn kor_till_ar5(kant: KantLage) -> Sim {
    let (k, tt) = kalla_tt();
    let mut sim = bygg_sim_balte(&k, &tt, 64, 48, 8, 1, &arm()).unwrap();
    sim.kant = kant;
    sim.fonster_k = 8;
    while sim.t < 5 * STEG_PER_AR {
        sim.steg();
    }
    sim
}

#[test]
fn prov3_jamn_fordelning_med_sammanfogade_sidor() {
    let sim = kor_till_ar5(KantLage::Torus);
    let fonster = kvot(&sim, 8, 8, 64, 48, 8);
    let ytter = kvot(&sim, 0, 0, 80, 64, 7);
    eprintln!("sammanfogade sidor, år 5, frö 1: fönstrets kvot {fonster:.3}, bältets ytterkant {ytter:.3}");
    assert!((0.5..=2.0).contains(&fonster), "fönstrets kant mot inåt {fonster} utanför 0,5–2");
    assert!((0.5..=2.0).contains(&ytter), "bältets ytterkant mot inåt {ytter} utanför 0,5–2");
}

#[test]
fn prov4_vagg_samlar_djuren_rott_med_flit() {
    let sim = kor_till_ar5(KantLage::Vagg);
    let ytter = kvot(&sim, 0, 0, 80, 64, 7);
    eprintln!("vägg, år 5, frö 1: bältets ytterkant {ytter:.3}");
    assert!(ytter > 2.0, "med vägg ska djuren samlas vid ytterkanten (kvot över 2), fick {ytter}");
}
