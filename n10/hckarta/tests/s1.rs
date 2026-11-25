//! Fem namngivna prov för S1 (Claude skriver rödproven/mutanterna efteråt).

use std::path::PathBuf;

use hckarta::{karta, kontrollsumma, Kalla, Marke, Omrade};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

/// Två anrop med samma område och t ger samma kontrollsumma.
#[test]
fn samma_karta_samma_summa() {
    let k = kalla();
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 4,
        h: 3,
        ruta_m: 2000.0,
    };
    let a = karta(&k, &omr, 500_000.0).unwrap();
    let b = karta(&k, &omr, 500_000.0).unwrap();
    assert_eq!(kontrollsumma(&a), kontrollsumma(&b));
}

/// Ändras ett enda värde i höjdlagret blir summan en annan.
#[test]
fn andrad_ruta_annan_summa() {
    let k = kalla();
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 4,
        h: 3,
        ruta_m: 2000.0,
    };
    let mut kart = karta(&k, &omr, 500_000.0).unwrap();
    let fore = kontrollsumma(&kart);
    kart.hojd.varden[0] += 1.0;
    assert_ne!(kontrollsumma(&kart), fore);
}

/// 1 × 1 ruta vars mitt ligger på Gilbert Creek ger höjden 194 m ± 1 m.
#[test]
fn gilbert_creek_som_paleokarta() {
    const LAT_MITT: f64 = 47.637699;
    const LON_MITT: f64 = -106.569901;
    const RUTA_M: f64 = 2000.0;
    const M_PER_GRAD: f64 = 111_320.0;

    let lat0 = LAT_MITT - 0.5 * RUTA_M / M_PER_GRAD;
    let lon0 = LON_MITT - 0.5 * RUTA_M / (M_PER_GRAD * LAT_MITT.to_radians().cos());
    let omr = Omrade {
        lat0,
        lon0,
        b: 1,
        h: 1,
        ruta_m: RUTA_M,
    };
    let kart = karta(&kalla(), &omr, 0.0).unwrap();
    let h = kart.hojd.varden[0];
    assert!(
        (h - 194.0).abs() <= 1.0,
        "höjd {h} m, väntade 194 ± 1 (gilbert-creek-66.txt)"
    );
}

/// Höjd, vatten_yta och vattentyp har Last; mark och växt har Simulering.
#[test]
fn markena() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 2,
        h: 2,
        ruta_m: 2000.0,
    };
    let kart = karta(&kalla(), &omr, 1000.0).unwrap();
    assert_eq!(kart.hojd.marke, Marke::Last);
    assert_eq!(kart.vatten_yta.marke, Marke::Last);
    assert_eq!(kart.vattentyp.marke, Marke::Last);
    assert_eq!(kart.mark.marke, Marke::Simulering);
    assert_eq!(kart.vaxt.marke, Marke::Simulering);
}

/// b = 0 eller h = 0 ger Err.
#[test]
fn tomt_omrade_ar_fel() {
    let k = kalla();
    let bas = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 1,
        h: 1,
        ruta_m: 2000.0,
    };
    let mut tom_b = bas.clone();
    tom_b.b = 0;
    assert!(karta(&k, &tom_b, 0.0).is_err());
    let mut tom_h = bas;
    tom_h.h = 0;
    assert!(karta(&k, &tom_h, 0.0).is_err());
}
