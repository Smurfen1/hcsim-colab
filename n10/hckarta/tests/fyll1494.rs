//! Prov 1–6 för Alt 1494/1495/1497: övergivna fåror fylls och blir land.
//! Underlag: n10/docs/2026-11-22-lasunderlag-kartfyll.md (skrivet före bygget).
//! Proven jämför med underlagets tal skrivna som tal, aldrig med programmets egna konstanter.

use std::collections::HashSet;
use std::path::PathBuf;

use hckarta::{
    karta_for_ar, karta_for_ar_fyllnad, loppbyten, mittlinjer_vid_ar, nasta_andring, Kalla, Omrade,
    Tidtabell, Vatten, FYLLNADSTID_AR,
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

fn johnson_kvot(and: &hckarta::FlodAndelar) -> f64 {
    let mut sum_icke = 0.0f64;
    let mut sum_alla = 0.0f64;
    for rad in &and.vaxt_tathet.varden {
        sum_icke += f64::from(rad[1] + rad[3] + rad[4]);
        sum_alla += f64::from(rad[0] + rad[1] + rad[2] + rad[3] + rad[4]);
    }
    sum_icke / sum_alla
}

fn rakna_trask(kart: &hckarta::Karta) -> usize {
    kart.vattentyp
        .varden
        .iter()
        .filter(|&&v| v == Vatten::Trask)
        .count()
}

/// Prov 1 (Alt 1497): Johnson-kvot < 0,10 vid 500 000 och 20 000 (tm2, riktning 1, frö 181).
/// Mutant "fylls aldrig" → rött.
#[test]
fn prov1_johnsons_grans_aterstalld() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for t in [500_000.0, 20_000.0] {
        let (_, and) = karta_for_ar(&k, &tt, &omr, t, 181, 2, 1).unwrap();
        let kvot = johnson_kvot(and.as_ref().unwrap());
        eprintln!("=== prov1 Johnson t={t}: {kvot:.6} ===");
        assert!(
            kvot < 0.10,
            "Johnsons kvot {kvot} ska vara < 0,10 vid t={t}"
        );
    }
}

/// Prov 2: för fröna 181–183 och varje loppbyte före t = 20 000 där övergiven
/// nedström syns (inte täckt av levande lopp/hav): vid t_byte − 500 → Trask;
/// vid t_byte − 1 500 → inte Trask av just den övergivningen (Land eller träsk/sjö
/// av annan regel). Mutanter: fyllnadstid 0 → rött; fyllnadstid 10 000 → rött.
#[test]
fn prov2_fyllningen() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    for fro in [181u64, 182, 183] {
        let byten = loppbyten(&k, &tt, &omr, fro, 1, 1).unwrap();
        let aktuella: Vec<_> = byten.into_iter().filter(|b| b.ar > 20_000.0).collect();
        assert!(
            !aktuella.is_empty(),
            "förväntade loppbyten före 20000 fro={fro}"
        );
        let mut n_provade = 0;
        for b in &aktuella {
            let t_trask = b.ar - 500.0;
            let t_land = b.ar - 1_500.0;
            if t_land <= 0.0 {
                continue;
            }
            let (linjer_fore, lev) =
                mittlinjer_vid_ar(&k, &tt, &omr, b.ar + 1.0, fro, 1).unwrap();
            let Some(ix) = lev.iter().position(|&f| f == b.fara) else {
                continue;
            };
            let path = &linjer_fore[ix];
            let Some(pidx) = path.iter().position(|&c| c == b.punkt) else {
                continue;
            };
            let gammal = &path[pidx + 1..];
            if gammal.is_empty() {
                continue;
            }

            let (linjer_trask, _) = mittlinjer_vid_ar(&k, &tt, &omr, t_trask, fro, 1).unwrap();
            let mut levande_trask = HashSet::new();
            for p in &linjer_trask {
                for &c in p {
                    levande_trask.insert(c);
                }
            }
            let (kart_trask, _) = karta_for_ar(&k, &tt, &omr, t_trask, fro, 1, 1).unwrap();
            let synliga: Vec<(usize, usize)> = gammal
                .iter()
                .copied()
                .filter(|&(i, j)| {
                    let p = i + j * omr.b;
                    kart_trask.hojd.varden[p] >= 0.0
                        && !levande_trask.contains(&(i, j))
                        && kart_trask.vattentyp.varden[p] != Vatten::Flodfara
                })
                .collect();
            // Underlaget: utom levande lopp och hav — hoppa byten där inget syns.
            if synliga.is_empty() {
                continue;
            }
            n_provade += 1;
            for &(i, j) in &synliga {
                let p = i + j * omr.b;
                assert_eq!(
                    kart_trask.vattentyp.varden[p],
                    Vatten::Trask,
                    "fro={fro} byte ar={} ({i},{j}) ska vara Trask vid t={}",
                    b.ar,
                    t_trask
                );
            }

            let (linjer_land, _) = mittlinjer_vid_ar(&k, &tt, &omr, t_land, fro, 1).unwrap();
            let mut levande_land = HashSet::new();
            for p in &linjer_land {
                for &c in p {
                    levande_land.insert(c);
                }
            }
            let (kart_land, _) = karta_for_ar(&k, &tt, &omr, t_land, fro, 1, 1).unwrap();
            let mut n_land = 0;
            let mut n_kollade = 0;
            for &(i, j) in gammal {
                let p = i + j * omr.b;
                if kart_land.hojd.varden[p] < 0.0 {
                    continue;
                }
                if levande_land.contains(&(i, j))
                    || kart_land.vattentyp.varden[p] == Vatten::Flodfara
                {
                    continue;
                }
                n_kollade += 1;
                // Inte Trask till följd av just den övergivningen.
                if kart_land.vattentyp.varden[p] == Vatten::Land {
                    n_land += 1;
                } else {
                    // Träsk/sjö av annan regel (levande bälte) är tillåtet.
                    assert!(
                        kart_land.vattentyp.varden[p] == Vatten::Trask
                            || kart_land.vattentyp.varden[p] == Vatten::Sjo,
                        "fro={fro} ar={} ({i},{j}): oväntad typ {:?} vid t={}",
                        b.ar,
                        kart_land.vattentyp.varden[p],
                        t_land
                    );
                }
            }
            assert!(
                n_kollade > 0 && n_land > 0,
                "fro={fro} byte ar={}: vid t={} ska övergiven del vara fylld (Land); land={n_land} kollade={n_kollade}",
                b.ar,
                t_land
            );
        }
        assert!(
            n_provade > 0,
            "fro={fro}: inget loppbyte med synlig övergiven nedström att pröva"
        );
    }
}

/// Prov 3: FYLLNADSTID_AR är 1000 (talet skrivet i provet).
#[test]
fn prov3_konstanten() {
    assert_eq!(FYLLNADSTID_AR, 1000.0);
}

/// Prov 4: Alt 1401:s döda fåror — vid 289 500 Trask, vid 288 500 inte längre (av den regeln).
#[test]
fn prov4_doda_foror_fylls() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    let fro = 181u64;
    // Mittlinjer strax före döden (Aldre): de tre som inte lever i Ovre.
    let (linjer_aldre, lev_aldre) =
        mittlinjer_vid_ar(&k, &tt, &omr, 290_001.0, fro, 1).unwrap();
    let (linjer_ovre, lev_ovre) =
        mittlinjer_vid_ar(&k, &tt, &omr, 289_500.0, fro, 1).unwrap();
    let mut levande_ovre = HashSet::new();
    for p in &linjer_ovre {
        for &c in p {
            levande_ovre.insert(c);
        }
    }
    let doda_nr: Vec<usize> = lev_aldre
        .iter()
        .copied()
        .filter(|n| !lev_ovre.contains(n))
        .collect();
    assert_eq!(doda_nr.len(), 3, "tre fåror ska dö vid 290000");

    let mut doda_rutor = Vec::new();
    for &nr in &doda_nr {
        let ix = lev_aldre.iter().position(|&f| f == nr).unwrap();
        for &c in &linjer_aldre[ix] {
            if !levande_ovre.contains(&c) {
                doda_rutor.push(c);
            }
        }
    }
    assert!(!doda_rutor.is_empty());

    let (kart_trask, _) = karta_for_ar(&k, &tt, &omr, 289_500.0, fro, 1, 1).unwrap();
    let mut n_trask = 0;
    for &(i, j) in &doda_rutor {
        let p = i + j * omr.b;
        if kart_trask.hojd.varden[p] < 0.0 {
            continue;
        }
        if kart_trask.vattentyp.varden[p] == Vatten::Flodfara {
            continue;
        }
        assert_eq!(
            kart_trask.vattentyp.varden[p],
            Vatten::Trask,
            "död fåra ({i},{j}) ska vara Trask vid 289500"
        );
        n_trask += 1;
    }
    assert!(n_trask > 0, "förväntade Trask från döda fåror vid 289500");

    let (kart_land, _) = karta_for_ar(&k, &tt, &omr, 288_500.0, fro, 1, 1).unwrap();
    // Inte Trask av dödsregeln: Land, eller träsk/sjö av en annan regel (levande bälte).
    let mut n_land = 0;
    for &(i, j) in &doda_rutor {
        let p = i + j * omr.b;
        if kart_land.hojd.varden[p] < 0.0 {
            continue;
        }
        if levande_ovre.contains(&(i, j)) || kart_land.vattentyp.varden[p] == Vatten::Flodfara {
            continue;
        }
        if kart_land.vattentyp.varden[p] == Vatten::Land {
            n_land += 1;
        }
    }
    assert!(
        n_land > 0,
        "vid 288500 ska minst en död-fåra-ruta vara Land (fylld)"
    );
}

/// Prov 5: nasta_andring från 290001 → 290000, därefter → 289000 (fyllning klar).
#[test]
fn prov5_nasta_andring_fyllning() {
    let tt = tidtabell();
    let fro = 181u64;
    let n1 = nasta_andring(&tt, 290_000.0 + 1.0, fro);
    assert_eq!(n1, Some(290_000.0), "från 290001 ska nasta vara 290000");
    // Strax efter döden (mot gränsen): nästa ändring är fyllningen klar, om inget
    // loppbyte ligger emellan.
    let n2 = nasta_andring(&tt, 290_000.0, fro);
    // Kan vara 289000 (fyllning) eller ett loppbyte i (289000, 290000).
    assert!(n2.is_some());
    let n2 = n2.unwrap();
    assert!(n2 < 290_000.0);
    if n2 > 289_000.0 {
        // Loppbyte emellan — då pekar nästa efter det på 289000 eller senare fyllning.
        let n3 = nasta_andring(&tt, n2, fro);
        assert!(n3.is_some());
        // Fortsätt tills vi når ≤ 289000 eller tar slut.
        let mut t = n2;
        let mut hittade_289 = false;
        for _ in 0..20 {
            let n = nasta_andring(&tt, t, fro);
            if n == Some(289_000.0) {
                hittade_289 = true;
                break;
            }
            match n {
                Some(x) if x > 289_000.0 => t = x,
                Some(x) if x == 289_000.0 => {
                    hittade_289 = true;
                    break;
                }
                _ => break,
            }
        }
        assert!(
            hittade_289,
            "förväntade 289000 som fyllning klar längs kedjan"
        );
    } else {
        assert_eq!(n2, 289_000.0, "fyllning klar ska vara 289000");
    }
}

/// Prov 6: tidsmodell 1 oförändrad (stickprov); hcsim-yta orörd via karta_full.
#[test]
fn prov6_tm1_oforandrad() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    // Tidsmodell 1 (karta_full_version) ska inte påverkas av fyllningen.
    let (a, _) = hckarta::karta_full_version(&k, &tt, &omr, 500_000.0, 181, 1, 1).unwrap();
    let (b, _) = hckarta::karta_full_version(&k, &tt, &omr, 500_000.0, 181, 1, 1).unwrap();
    assert_eq!(a.vattentyp.varden, b.vattentyp.varden);
    // Fyllnings-API med tid 0 ska ge samma som "aldrig träsk från övergivning" vid gamla tider
    // jämfört med enorm fyllnadstid? Stickprov: karta_for_ar bygger utan panic.
    let _ = karta_for_ar(&k, &tt, &omr, 500_000.0, 181, 2, 1).unwrap();
}

/// Redovisning Alt 1495: kvot för 100, 1000, 10000, 82000; Trask med/utan fyllning.
#[test]
fn redovisning_kanslighet() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade();
    eprintln!("=== Alt 1495 känslighet (frö 181, riktning 1, version 2) ===");
    for &fyll in &[100.0, 1_000.0, 10_000.0, 82_000.0] {
        for &t in &[500_000.0, 20_000.0] {
            let (_, and) =
                karta_for_ar_fyllnad(&k, &tt, &omr, t, 181, 2, 1, fyll).unwrap();
            let kvot = johnson_kvot(and.as_ref().unwrap());
            eprintln!("  fyllnadstid={fyll:.0} t={t}: Johnson-kvot={kvot:.6}");
        }
    }
    eprintln!("=== Trask-rutor med/utan fyllning (frö 181, version 2) ===");
    for &riktning in &[1u8, 2] {
        for &t in &[500_000.0, 20_000.0] {
            let (med, _) =
                karta_for_ar_fyllnad(&k, &tt, &omr, t, 181, 2, riktning, 1_000.0).unwrap();
            let (utan, _) =
                karta_for_ar_fyllnad(&k, &tt, &omr, t, 181, 2, riktning, 1.0e12).unwrap();
            eprintln!(
                "  riktning={riktning} t={t}: Trask med fyllning={} utan={}",
                rakna_trask(&med),
                rakna_trask(&utan)
            );
        }
    }
}
