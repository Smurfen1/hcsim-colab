//! Alt 1828 (agenternas omröstning karttid; Alt 1843): listan över kartans bytestider. Talen är skrivna som tal (mätningen på N10,
//! piharness docs/evolution/karttid-matning.txt: 86 rutor byter vattentyp mellan år 17 000 och 18 000 för frö 261).
//! Standardkartan: flodversion 2, riktning 2, lutning 2, tidsmodell 2 (`karta_for_ar_lutning`).
use hckarta::{bytesar, karta_for_ar_lutning, kontrollsumma_med, Kalla, Omrade, Tidtabell};
use std::path::PathBuf;

fn kalla_tt() -> (Kalla, Tidtabell) {
    let m = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    (
        Kalla::las(&m.join("../paleokarta/data")).unwrap(),
        Tidtabell::las(&std::fs::read_to_string(m.join("tidtabell.tsv")).unwrap()).unwrap(),
    )
}
fn omr() -> Omrade { Omrade { lat0: 47.0, lon0: -108.0, b: 64, h: 48, ruta_m: 2000.0 } }
const AR0: f64 = 979_000.0;

fn summa(k: &Kalla, tt: &Tidtabell, t: f64, fro: u64) -> u64 {
    let (kart, and) = karta_for_ar_lutning(k, tt, &omr(), t, fro, 2, 2, 2).unwrap();
    kontrollsumma_med(&kart, and.as_ref())
}

/// Mittpunkten i det öppna intervallet mellan två tider i listan som innehåller t (listan fallande; gränserna år 0 och t = 0).
fn mitt_i_intervallet(lista: &[f64], t: f64) -> f64 {
    let over = lista.iter().cloned().filter(|&c| c > t).fold(AR0, f64::min);
    let under = lista.iter().cloned().filter(|&c| c < t).fold(0.0, f64::max);
    (over + under) / 2.0
}

#[test]
fn prov1_listan_och_de_86_rutorna() {
    let (k, tt) = kalla_tt();
    let lista = bytesar(&tt, 261, AR0, 0.0);
    assert!(!lista.is_empty(), "tom lista");
    for w in lista.windows(2) {
        assert!(w[0] > w[1], "listan ska vara fallande: {} {}", w[0], w[1]);
    }
    assert!(lista.iter().all(|&t| t < AR0 && t >= 0.0));
    // År 17 000–18 000 (t 962 000–961 000): 86 rutor byter vattentyp, och listan har en tid i intervallet.
    let (a, _) = karta_for_ar_lutning(&k, &tt, &omr(), 962_000.0, 261, 2, 2, 2).unwrap();
    let (b, _) = karta_for_ar_lutning(&k, &tt, &omr(), 961_000.0, 261, 2, 2, 2).unwrap();
    let byte = a.vattentyp.varden.iter().zip(b.vattentyp.varden.iter()).filter(|(x, y)| x != y).count();
    assert_eq!(byte, 86, "mätningen sade 86 rutor");
    let i = lista.iter().filter(|&&t| t <= 962_000.0 && t > 961_000.0).count();
    assert!(i >= 1, "ingen bytestid mellan år 17 000 och 18 000");
    eprintln!("frö 261: {} bytestider från år 0 till gränsen", lista.len());
}

/// En enkel slumpföljd (SplitMix64) så att proven är lika varje gång.
fn slump(x: &mut u64) -> f64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn prov2_kartan_enligt_listan_ar_lika_med_direkt_karta_20_fron() {
    let (k, tt) = kalla_tt();
    let mut x = 1828u64;
    let mut n = 0;
    for fro in 1..=20u64 {
        let lista = bytesar(&tt, fro, AR0, 0.0);
        assert!(!lista.is_empty(), "tom lista för frö {fro}");
        for _ in 0..50 {
            // Halva år, så att t aldrig är en tid i listan (listans tider är hela år).
            let t = (slump(&mut x) * AR0).floor() + 0.5;
            assert!(!lista.contains(&t));
            let c = mitt_i_intervallet(&lista, t);
            assert_eq!(summa(&k, &tt, t, fro), summa(&k, &tt, c, fro), "frö {fro}, t {t}: kartan skiljer sig från kartan mitt i samma intervall ({c})");
            n += 1;
        }
    }
    assert_eq!(n, 1000, "20 frön gånger 50 tider");
}

#[test]
fn prov3_en_struken_tid_upptacks_rott_med_flit() {
    let (k, tt) = kalla_tt();
    let lista = bytesar(&tt, 261, AR0, 0.0);
    let c = *lista.iter().find(|&&t| t <= 962_000.0 && t > 961_000.0).expect("tiden mellan år 17 000 och 18 000");
    let utan: Vec<f64> = lista.iter().cloned().filter(|&t| t != c).collect();
    // Strax under bytestiden: utan tiden i listan hamnar t i ett för stort intervall, vars mitt har en annan karta.
    let t = c - 0.5;
    assert_ne!(summa(&k, &tt, t, 261), summa(&k, &tt, mitt_i_intervallet(&utan, t), 261), "en struken bytestid ska ge fel karta");
}
