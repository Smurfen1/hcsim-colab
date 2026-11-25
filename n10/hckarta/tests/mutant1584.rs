//! Prov för Alt 1584: träskmutanter A/B syns i tidsmodell 2.
//! Underlag: `/home/hakan/sandbox/pi/n10/docs/2026-11-22-lasunderlag-kartstd2.md` (Alt 1584).
//! Mätning som visade felet: `kartstd-matning.txt` (A och B lika med utan mutant i TM2).

use std::path::PathBuf;

use hckarta::{
    karta_for_ar_lutning, karta_for_ar_lutning_prov, Kalla, Omrade, Tidtabell, TraskProvMutant,
    Vatten,
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

fn rakna_trask(kart: &hckarta::Karta) -> usize {
    kart.vattentyp
        .varden
        .iter()
        .filter(|&&v| v == Vatten::Trask)
        .count()
}

fn rakna_farrutor(andelar: &Option<hckarta::FlodAndelar>) -> usize {
    andelar
        .as_ref()
        .map(|a| a.fara_andel.varden.iter().filter(|&&x| x > 0.0).count())
        .unwrap_or(0)
}

/// Gränsprov: minst 20 frön och minst en fårruta. Stoppar med fel annars.
fn krav_gransprov(fron: &[u64], farrutor: usize) -> Result<(), String> {
    if fron.len() < 20 {
        return Err(format!(
            "gränsprov kräver minst 20 frön, fick {}",
            fron.len()
        ));
    }
    if farrutor == 0 {
        return Err("gränsprov stoppar vid 0 fårrutor".into());
    }
    Ok(())
}

fn trask_per_fro(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    mutant: TraskProvMutant,
) -> (usize, usize) {
    let (kart, and) =
        karta_for_ar_lutning_prov(k, tt, omr, t, fro, 2, 2, 2, mutant).expect("karta");
    (rakna_trask(&kart), rakna_farrutor(&and))
}

/// Prov 1: mutanterna A och B skiljer antalet Trask-rutor från utan mutant i minst 18 av 20 frön
/// för varje av t = 500 000, 20 000 och 0 (version 2, riktning 2, lutning 2).
#[test]
fn prov1_mutanter_syns_i_tidsmodell2() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fron: Vec<u64> = (1..=20).collect();
    let tider = [500_000.0_f64, 20_000.0, 0.0];

    for &t in &tider {
        let mut lika_a = 0usize;
        let mut lika_b = 0usize;
        let mut farrutor_tot = 0usize;
        eprintln!("=== prov1 t={t}: Trask per frö (utan / A / B) ===");
        for &fro in &fron {
            let (utan, far) = trask_per_fro(&k, &tt, &omr, t, fro, TraskProvMutant::Ingen);
            let (a, _) = trask_per_fro(
                &k,
                &tt,
                &omr,
                t,
                fro,
                TraskProvMutant::NarmasteSomLagsta,
            );
            let (b, _) = trask_per_fro(&k, &tt, &omr, t, fro, TraskProvMutant::DubbeltAvstand);
            farrutor_tot += far;
            if a == utan {
                lika_a += 1;
            }
            if b == utan {
                lika_b += 1;
            }
            eprintln!("  fro={fro}: utan={utan} A={a} B={b}");
        }
        krav_gransprov(&fron, farrutor_tot).expect("gränsprov");
        let skiljer_a = 20 - lika_a;
        let skiljer_b = 20 - lika_b;
        eprintln!(
            "=== prov1 t={t}: A skiljer i {skiljer_a}/20; B skiljer i {skiljer_b}/20 ==="
        );
        assert!(
            skiljer_a >= 18,
            "mutant A skiljer bara i {skiljer_a} av 20 frön vid t={t} (mål ≥ 18)"
        );
        assert!(
            skiljer_b >= 18,
            "mutant B skiljer bara i {skiljer_b} av 20 frön vid t={t} (mål ≥ 18)"
        );
    }
}

/// Prov 2: utan mutant oförändrat — `karta_for_ar_lutning` och prov med `Ingen` ger samma
/// vattentyp, vatten_yta och fara_andel ruta för ruta.
#[test]
fn prov2_utan_mutant_oforandrat() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let tider = [500_000.0_f64, 20_000.0, 0.0];
    for &t in &tider {
        for fro in 1u64..=20 {
            let (prod, and_prod) =
                karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).expect("prod");
            let (prov, and_prov) = karta_for_ar_lutning_prov(
                &k,
                &tt,
                &omr,
                t,
                fro,
                2,
                2,
                2,
                TraskProvMutant::Ingen,
            )
            .expect("prov");
            assert_eq!(
                prod.vattentyp.varden, prov.vattentyp.varden,
                "vattentyp skiljer fro={fro} t={t}"
            );
            assert_eq!(
                prod.vatten_yta.varden, prov.vatten_yta.varden,
                "vatten_yta skiljer fro={fro} t={t}"
            );
            let ap = and_prod.as_ref().expect("andelar prod");
            let aq = and_prov.as_ref().expect("andelar prov");
            assert_eq!(
                ap.fara_andel.varden, aq.fara_andel.varden,
                "fara_andel skiljer fro={fro} t={t}"
            );
        }
    }
}

/// Prov 3: hjälpfunktionen vägrar färre än 20 frön och stoppar vid 0 fårrutor.
#[test]
fn prov3_gransprov_minst_20_fron_och_farrutor() {
    let ett = [1u64];
    assert!(
        krav_gransprov(&ett, 100).is_err(),
        "1 frö ska ge fel"
    );
    let tjugo: Vec<u64> = (1..=20).collect();
    assert!(
        krav_gransprov(&tjugo, 0).is_err(),
        "0 fårrutor ska ge fel"
    );
    assert!(
        krav_gransprov(&tjugo, 1).is_ok(),
        "20 frön och ≥1 fårruta ska godkännas"
    );
}
