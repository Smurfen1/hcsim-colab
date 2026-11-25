//! Prov för flodkanten (Alt 1624, 1625; underlaget `n10/docs/2026-11-22-lasunderlag-flodkant1624.md`).
//! Fel med flit som prövats: se slutet av filen.

use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
use hcsim::sim::{landandel_av, Sim};
use hcsim::tal;

fn tom_lager<T: Clone>(n: usize, v: T, marke: Marke) -> Lager<T> {
    Lager {
        varden: vec![v; n],
        marke,
    }
}

fn konstruerad(b: usize, h: usize, vatten: Vec<Vatten>, vaxt: f32) -> Karta {
    assert_eq!(vatten.len(), b * h);
    let n = b * h;
    Karta {
        omrade: Omrade {
            lat0: 47.0,
            lon0: -108.0,
            b,
            h,
            ruta_m: 2000.0,
        },
        t: 500_000.0,
        hojd: tom_lager(n, 100.0f32, Marke::Last),
        vatten_yta: tom_lager(n, 100.0f32, Marke::Last),
        vattentyp: Lager {
            varden: vatten,
            marke: Marke::Last,
        },
        mark: tom_lager(n, [0.25f32; 4], Marke::Simulering),
        vaxt: tom_lager(n, vaxt, Marke::Simulering),
        period: tom_lager(n, Period::Aldre, Marke::Simulering),
    }
}

/// Prov 1: talen ur källorna skrivna som tal, inte programmets konstanter.
#[test]
fn prov1_landandel_ur_farans_bredd() {
    let w = (100.0f64 * 320.0).sqrt();
    let vantat = 1.0 - w / 2000.0;
    assert!((vantat - 0.910557).abs() < 1e-6, "{vantat}");
    assert!((landandel_av(Vatten::Flodfara, 2000.0) - vantat).abs() < 1e-9);
    for vt in [Vatten::Sjo, Vatten::Hav, Vatten::Brackt] {
        assert_eq!(landandel_av(vt, 2000.0), 0.0, "{vt:?}");
    }
    for vt in [Vatten::Land, Vatten::Trask] {
        assert_eq!(landandel_av(vt, 2000.0), 1.0, "{vt:?}");
    }
}

fn varld() -> (Sim, usize, usize) {
    // 4 × 4 land; ruta 5 är flodfåra, ruta 10 är sjö.
    let mut vatten = vec![Vatten::Land; 16];
    vatten[5] = Vatten::Flodfara;
    vatten[10] = Vatten::Sjo;
    let kart = konstruerad(4, 4, vatten, 0.5);
    let mut sim = Sim::fran_karta(&kart, 11, 0);
    for _ in 0..(10 * tal::STEG_PER_AR as usize) {
        sim.steg();
    }
    (sim, 5, 10)
}

/// Prov 2: flodrutan har växter efter 10 år (rött med koden före Alt 1624, där landandelen var 0).
#[test]
fn prov2_flodrutan_har_vaxter() {
    let (sim, flod, _) = varld();
    assert!(sim.vmax[flod] > 0.0, "taket i flodrutan är {}", sim.vmax[flod]);
    assert!(sim.v[flod] > 0.0, "växtmassan i flodrutan är {}", sim.v[flod]);
}

/// Prov 3: sjörutan har fortfarande inga växter.
#[test]
fn prov3_sjon_har_inga_vaxter() {
    let (sim, _, sjo) = varld();
    assert_eq!(sim.vmax[sjo], 0.0);
    assert_eq!(sim.v[sjo], 0.0);
}

// Fel med flit som prövats (2026-11-22, Claude): Flodfåra => 0.0 (som före Alt 1624) gav rött i prov 1 och 2;
// fårans bredd 115 m i stället för √(100 · 320) gav rött i prov 1.
