//! Tidsskiftets databas (Håkan 2026-11-22: värden i en databas, inte en bild per rad).

use hcsim::tidsskift::{db_vag, lage_json, Fast, Lage, Lasare, Skrivare};

fn fast(b: usize, h: usize) -> Fast {
    Fast { rubrik: "prov \"x\"".into(), b, h, steg_per_ar: 150, vmax: vec![800.0; b * h], vatten: (0..b * h).map(|k| (k % 4) as u8).collect() }
}

fn lage(k: i64, b: usize, h: usize, djur: usize) -> Lage {
    Lage {
        klocka: 1000 + 10 * k as u64,
        t: 30 * k,
        markerad: k > 3,
        v: (0..b * h).map(|c| (c as f32) * 0.25 + k as f32).collect(),
        djur: (0..djur).map(|d| ((d % b) as u8, ((d + k as usize) % h) as u8, d % 2 == 0)).collect(),
    }
}

fn vag(namn: &str) -> std::path::PathBuf {
    db_vag(&std::env::temp_dir().join(format!("tidsskift-prov-{namn}-{}.json", std::process::id())))
}

fn stada(p: &std::path::Path) {
    for sv in ["", "-wal", "-shm"] {
        let mut q = p.as_os_str().to_owned();
        q.push(sv);
        let _ = std::fs::remove_file(std::path::PathBuf::from(q));
    }
}

#[test]
fn vardena_kommer_tillbaka_lika() {
    let p = vag("lika");
    let f = fast(64, 48);
    let mut sk = Skrivare::ny(&p, &f).unwrap();
    let l0 = lage(0, 64, 48, 400);
    let l1 = lage(5, 64, 48, 6000);
    assert!(!sk.lagg_till(&l0).unwrap());
    assert!(!sk.lagg_till(&l1).unwrap());
    let la = Lasare::oppna(&p).unwrap();
    assert_eq!(la.fast().unwrap(), f);
    let lista = la.lista().unwrap();
    assert_eq!(lista.iter().map(|x| (x.1, x.2)).collect::<Vec<_>>(), vec![(1000, 0), (1050, 150)]);
    assert_eq!(la.lage(lista[0].0).unwrap(), l0);
    assert_eq!(la.lage(lista[1].0).unwrap(), l1);
    // Texten till kartvyn: giltig JSON med samma fält som kartbildens fil.
    let j: serde_json::Value = serde_json::from_str(&la.lage_json(lista[1].0).unwrap()).unwrap();
    assert_eq!(j["klocka"], 1050);
    assert_eq!(j["ar"], 1.0);
    assert_eq!(j["markerad"], true);
    assert_eq!(j["rubrik"], "prov \"x\"");
    assert_eq!(j["v"].as_array().unwrap().len(), 64 * 48);
    assert_eq!(j["djur"].as_array().unwrap().len(), 6000);
    assert_eq!(j["djur"][1], serde_json::json!([1, 6, 0]));
    assert_eq!(lage_json(&f, &l1), la.lage_json(lista[1].0).unwrap());
    stada(&p);
}

#[test]
fn ny_korning_borjar_om() {
    let p = vag("om");
    let f = fast(4, 4);
    let mut sk = Skrivare::ny(&p, &f).unwrap();
    sk.lagg_till(&lage(0, 4, 4, 3)).unwrap();
    sk.lagg_till(&lage(1, 4, 4, 3)).unwrap();
    drop(sk);
    let mut sk = Skrivare::ny(&p, &f).unwrap();
    sk.lagg_till(&lage(9, 4, 4, 3)).unwrap();
    assert_eq!(Lasare::oppna(&p).unwrap().lista().unwrap().len(), 1);
    stada(&p);
}

/// Taket: databasen blir aldrig större än taket, det första och det sista läget finns alltid kvar, och lägena ligger
/// jämnt (högst en faktor 2 mellan största och minsta avstånd, utom det sista).
#[test]
fn taket_glesar_ut_och_behaller_start_och_slut() {
    let p = vag("tak");
    let f = fast(4, 4);
    let mut sk = Skrivare::ny_med_tak(&p, &f, 8).unwrap();
    // Som i körningen: efter en glesning dubblas tiden mellan lägena.
    let (mut glesningar, mut k, mut steg, mut sista) = (0, 0i64, 1i64, 0i64);
    for _ in 0..60 {
        sista = k;
        if sk.lagg_till(&lage(k, 4, 4, 3)).unwrap() {
            glesningar += 1;
            steg *= 2;
        }
        k += steg;
        let n = Lasare::oppna(&p).unwrap().lista().unwrap().len();
        assert!(n <= 8, "{n} lägen, taket är 8");
    }
    assert!(glesningar >= 3, "bara {glesningar} glesningar på 60 lägen med taket 8");
    let t: Vec<i64> = Lasare::oppna(&p).unwrap().lista().unwrap().iter().map(|x| x.2).collect();
    assert_eq!(t[0], 0, "det första läget är borta");
    assert_eq!(*t.last().unwrap(), 30 * sista, "det sista läget är borta");
    let d: Vec<i64> = t.windows(2).map(|w| w[1] - w[0]).collect();
    let inre = &d[..d.len() - 1];
    assert!(inre.iter().max().unwrap() <= &(2 * inre.iter().min().unwrap()), "ojämna avstånd: {d:?}");
    stada(&p);
}

/// Utrymmet: ett läge med 6 000 djur på kartan 64 × 48 tar under 30 000 byte i databasen, och mindre än en fjärdedel av
/// samma läge som text (det gamla sättet). Med taket på 600 lägen blir hela databasen då högst 18 MB.
#[test]
fn ett_lage_tar_lite_plats() {
    let p = vag("plats");
    let f = fast(64, 48);
    let mut sk = Skrivare::ny(&p, &f).unwrap();
    for k in 0..40 {
        sk.lagg_till(&lage(k, 64, 48, 6000)).unwrap();
    }
    drop(sk); // stänger databasen, så att allt ligger i huvudfilen
    assert!(!std::path::Path::new(&format!("{}-wal", p.display())).exists(), "loggfilen finns kvar efter stängningen");
    let per_lage = std::fs::metadata(&p).unwrap().len() / 40;
    let text = lage_json(&f, &lage(3, 64, 48, 6000)).len() as u64;
    assert!(per_lage < 30_000, "{per_lage} byte per läge");
    assert_eq!(hcsim::tidsskift::MAX_LAGEN, 600);
    assert!(per_lage * 4 < text, "{per_lage} byte per läge mot {text} byte som text");
    stada(&p);
}
