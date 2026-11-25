//! Sju namngivna prov för S4: flodnätet ur frö.

use std::path::PathBuf;

use hckarta::{
    karta_full, kontrollsumma, lagg_flodnat, las_3d, skriv_3d, Kalla, Karta, Lager, Marke, Omrade,
    Period, Tidtabell, Vatten,
};

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../paleokarta/data")
}

fn kalla() -> Kalla {
    Kalla::las(&data()).expect("kunde inte läsa paleokarta/data")
}

fn tidtabell() -> Tidtabell {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tidtabell.tsv"))
        .expect("kunde inte läsa tidtabell.tsv");
    Tidtabell::las(&text).expect("tidtabellen")
}

fn omrade_stort() -> Omrade {
    Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 64,
        h: 48,
        ruta_m: 2000.0,
    }
}

fn temp_mapp(namn: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "hckarta-s4-{}-{}-{}",
        namn,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("temp-mapp");
    p
}

fn idx(b: usize, i: usize, j: usize) -> usize {
    i + j * b
}

/// Samma frö ger samma kontrollsumma.
#[test]
fn samma_fro_samma_flodnat() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade_stort();
    let a = karta_full(&k, &tt, &omr, 500_000.0, 7).unwrap();
    let b = karta_full(&k, &tt, &omr, 500_000.0, 7).unwrap();
    assert_eq!(kontrollsumma(&a), kontrollsumma(&b));
}

/// Annat frö ger annan kontrollsumma.
#[test]
fn annat_fro_annat_flodnat() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade_stort();
    let a = karta_full(&k, &tt, &omr, 500_000.0, 7).unwrap();
    let b = karta_full(&k, &tt, &omr, 500_000.0, 8).unwrap();
    assert_ne!(kontrollsumma(&a), kontrollsumma(&b));
}

/// Varje sammanhängande flodkomponent som rör västra kanten når också östra kanten (8-grannar).
#[test]
fn foror_nar_ostkanten() {
    let kart = karta_full(&kalla(), &tidtabell(), &omrade_stort(), 500_000.0, 7).unwrap();
    let b = kart.omrade.b;
    let h = kart.omrade.h;
    let n = b * h;
    let mut besokt = vec![false; n];
    let grannar: [(isize, isize); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    let mut vaster_komponenter = 0usize;
    for j in 0..h {
        let start = idx(b, 0, j);
        if kart.vattentyp.varden[start] != Vatten::Flodfara || besokt[start] {
            continue;
        }
        vaster_komponenter += 1;
        let mut stack = vec![(0usize, j)];
        besokt[start] = true;
        let mut nard_ost = false;
        while let Some((i, jj)) = stack.pop() {
            if i + 1 == b {
                nard_ost = true;
            }
            for &(di, dj) in &grannar {
                let ni = i as isize + di;
                let nj = jj as isize + dj;
                if ni < 0 || nj < 0 || ni as usize >= b || nj as usize >= h {
                    continue;
                }
                let ni = ni as usize;
                let nj = nj as usize;
                let p = idx(b, ni, nj);
                if besokt[p] || kart.vattentyp.varden[p] != Vatten::Flodfara {
                    continue;
                }
                besokt[p] = true;
                stack.push((ni, nj));
            }
        }
        assert!(
            nard_ost,
            "flodkomponent från västra kanten (j={j}) nådde inte östra kanten"
        );
    }
    assert!(
        vaster_komponenter > 0,
        "väntade minst en flodfåra vid västra kanten"
    );
}

/// Inget steg mellan två 8-grannar i en fåra går västerut (di < 0).
/// Kontrolleras längs varje fårans centrumlinje: från väst går varje steg till en östlig granne.
#[test]
fn foror_gar_osterut() {
    let kart = karta_full(&kalla(), &tidtabell(), &omrade_stort(), 500_000.0, 7).unwrap();
    let b = kart.omrade.b;
    let h = kart.omrade.h;

    // För varje flodruta: alla flodgrannar med lägre i (västerut) är tillåtna som "föregångare",
    // men det får inte finnas en flodkedja som tar ett steg med di < 0 när man följer från väst.
    // Enklare: bland alla par av 8-grannar som båda är Flodfara, om |di|+|dj| steg längs
    // en faktisk vandring österut — vi spårar från västra kanten och accepterar bara steg med di >= 0.
    let mut fran_vaster = vec![false; b * h];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for j in 0..h {
        let p = idx(b, 0, j);
        if kart.vattentyp.varden[p] == Vatten::Flodfara {
            fran_vaster[p] = true;
            stack.push((0, j));
        }
    }
    // Tillåtna steg: nord, syd, och de tre österut (aldrig västerut).
    let tillatna: [(isize, isize); 5] = [(1, -1), (1, 0), (1, 1), (0, -1), (0, 1)];
    while let Some((i, j)) = stack.pop() {
        for &(di, dj) in &tillatna {
            let ni = i as isize + di;
            let nj = j as isize + dj;
            if ni < 0 || nj < 0 || ni as usize >= b || nj as usize >= h {
                continue;
            }
            let ni = ni as usize;
            let nj = nj as usize;
            let p = idx(b, ni, nj);
            if kart.vattentyp.varden[p] != Vatten::Flodfara || fran_vaster[p] {
                continue;
            }
            assert!(di >= 0, "steg västerut från ({i},{j}) till ({ni},{nj})");
            fran_vaster[p] = true;
            stack.push((ni, nj));
        }
    }
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if kart.vattentyp.varden[p] == Vatten::Flodfara {
                assert!(
                    fran_vaster[p],
                    "flodruta ({i},{j}) nås bara via västligt steg eller är isolerad"
                );
            }
        }
    }
}

/// Med samma frö har Aldre fler fåror som korsar mittlinjen (i = b/2) än Ovre.
#[test]
fn aldre_har_fler_foror() {
    let k = kalla();
    let tt = tidtabell();
    let omr = omrade_stort();
    let fro = 7u64;
    let aldre = karta_full(&k, &tt, &omr, 500_000.0, fro).unwrap();
    let ovre = karta_full(&k, &tt, &omr, 100_000.0, fro).unwrap();
    assert_eq!(aldre.period.varden[0], Period::Aldre);
    assert_eq!(ovre.period.varden[0], Period::Ovre);

    fn korsar_mittlinjen(kart: &Karta) -> usize {
        let b = kart.omrade.b;
        let h = kart.omrade.h;
        let i = b / 2;
        let mut antal = 0usize;
        let mut j = 0usize;
        while j < h {
            if kart.vattentyp.varden[idx(b, i, j)] == Vatten::Flodfara {
                antal += 1;
                // Hoppa över sammanhängande block (en fåra med bredd > 1).
                while j < h && kart.vattentyp.varden[idx(b, i, j)] == Vatten::Flodfara {
                    j += 1;
                }
            } else {
                j += 1;
            }
        }
        antal
    }

    let n_aldre = korsar_mittlinjen(&aldre);
    let n_ovre = korsar_mittlinjen(&ovre);
    assert!(
        n_aldre > n_ovre,
        "Aldre ({n_aldre}) ska ha fler fåror vid mittlinjen än Ovre ({n_ovre})"
    );
}

/// Västra halvan hav (höjd −10): alla havsrutor förblir Hav efter flodnätet.
#[test]
fn hav_rors_inte() {
    let b = 16usize;
    let h = 12usize;
    let n = b * h;
    let mut hojd = vec![10.0f32; n];
    let mut vatten_yta = vec![10.0f32; n];
    let mut vattentyp = vec![Vatten::Land; n];
    for j in 0..h {
        for i in 0..b {
            if i < b / 2 {
                let p = idx(b, i, j);
                hojd[p] = -10.0;
                vatten_yta[p] = 0.0;
                vattentyp[p] = Vatten::Hav;
            }
        }
    }
    let mut kart = Karta {
        omrade: Omrade {
            lat0: 47.0,
            lon0: -108.0,
            b,
            h,
            ruta_m: 2000.0,
        },
        t: 500_000.0,
        hojd: Lager {
            varden: hojd,
            marke: Marke::Last,
        },
        vatten_yta: Lager {
            varden: vatten_yta,
            marke: Marke::Last,
        },
        vattentyp: Lager {
            varden: vattentyp,
            marke: Marke::Last,
        },
        mark: Lager {
            varden: vec![[0.25; 4]; n],
            marke: Marke::Simulering,
        },
        vaxt: Lager {
            varden: vec![0.5; n],
            marke: Marke::Simulering,
        },
        period: Lager {
            varden: vec![Period::Aldre; n],
            marke: Marke::Last,
        },
    };
    lagg_flodnat(&mut kart, 7);
    for j in 0..h {
        for i in 0..b / 2 {
            let p = idx(b, i, j);
            assert_eq!(
                kart.vattentyp.varden[p],
                Vatten::Hav,
                "havsruta ({i},{j}) ändrades"
            );
            assert_eq!(kart.hojd.varden[p], -10.0);
        }
    }
}

/// Med flodnätet: vattentyp och vatten_yta har Simulering; höjden fortfarande Last.
#[test]
fn markena_s4() {
    let kart = karta_full(&kalla(), &tidtabell(), &omrade_stort(), 500_000.0, 7).unwrap();
    assert_eq!(kart.hojd.marke, Marke::Last);
    assert_eq!(kart.vatten_yta.marke, Marke::Simulering);
    assert_eq!(kart.vattentyp.marke, Marke::Simulering);
}

/// skriv_3d och las_3d ger samma kontrollsumma för en karta med flodnät.
#[test]
fn filer_med_flodnat() {
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 16,
        h: 12,
        ruta_m: 2000.0,
    };
    let kart = karta_full(&kalla(), &tidtabell(), &omr, 500_000.0, 7).unwrap();
    assert!(kart
        .vattentyp
        .varden
        .iter()
        .any(|&v| v == Vatten::Flodfara));
    let fore = kontrollsumma(&kart);
    let mapp = temp_mapp("flodnat");
    let skriven = skriv_3d(&kart, &mapp).unwrap();
    assert_eq!(skriven, fore);
    let last = las_3d(&mapp).unwrap();
    assert_eq!(kontrollsumma(&last), fore);
    let _ = std::fs::remove_dir_all(&mapp);
}
