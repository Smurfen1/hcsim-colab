//! Enhällig omröstning vattenkvalitet (Alt 2098, Alt 2100, Alt 2103; Håkans Alt 2076, 2083, 2109;
//! protokoll `n10/docs/2026-11-24-omrostning-vatten.md`).
//! Proven jämför med mätningens tal skrivna som tal (från `piharness docs/evolution/vatten-matning.txt`),
//! aldrig med programmets egna konstanter.

use std::path::PathBuf;

use hckarta::{
    flodesklass_av, karta_for_ar_lutning, karta_full, kontrollsumma, kontrollsumma_vattenkvalitet,
    las_3d_med_vattenkvalitet, las_vattenkvalitet_tsv, rakna_vattenkvalitet, salthalt_klass_av,
    skriv_3d, skriv_3d_med, skriv_3d_med_vattenkvalitet, Flodesklass, Kalla, Omrade,
    SalthaltKlass, Tidtabell, Vatten,
};

const LAS_TABELL: u64 = 11522424303521908281;

fn fnv_str(t: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in t.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn data_och_tt() -> (Kalla, Tidtabell, Omrade) {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = Kalla::las(&rot.join("../paleokarta/data")).expect("paleokarta data");
    let tt = Tidtabell::las(&std::fs::read_to_string(rot.join("tidtabell.tsv")).expect("tidtabell"))
        .expect("parsa tidtabell");
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 64,
        h: 48,
        ruta_m: 2000.0,
    };
    (k, tt, omr)
}

fn medelavstand_km(b: usize, h: usize, flode: &[Flodesklass]) -> f64 {
    let mut vatten = Vec::new();
    for j in 0..h {
        for i in 0..b {
            if flode[j * b + i] != Flodesklass::Land {
                vatten.push((i as isize, j as isize));
            }
        }
    }
    assert!(!vatten.is_empty(), "kartan har inga vattenrutor");
    let mut sum = 0usize;
    for j in 0..h {
        for i in 0..b {
            if flode[j * b + i] != Flodesklass::Land {
                continue;
            }
            let mut best = usize::MAX;
            for &(vi, vj) in &vatten {
                let d = (i as isize - vi)
                    .unsigned_abs()
                    .max((j as isize - vj).unsigned_abs());
                if d < best {
                    best = d;
                    if best == 1 {
                        break;
                    }
                }
            }
            sum += best;
        }
    }
    (sum as f64 / (b * h) as f64) * 2.0
}

fn tabelltext() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/vattenkvalitet.tsv"))
        .expect("vattenkvalitet.tsv")
}

#[test]
fn standard2_fro_1_till_20_matningens_tal() {
    let (k, tt, omr) = data_och_tt();
    let mut avst_aldre = 0.0;
    let mut avst_ovre = 0.0;

    for fro in 1..=20u64 {
        for (t, forv_sjo, ar_aldre) in [(979_000.0, 61usize, true), (200_000.0, 46usize, false)] {
            let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).unwrap();
            let and = and_opt.expect("standard 2 har andelar");
            let vk = rakna_vattenkvalitet(&kart, Some(&and)).unwrap();
            assert!(vk.har_vattenkvalitet);
            assert_eq!(vk.flodesklass.varden.len(), 3072);
            assert_eq!(vk.salthalt_klass.varden.len(), 3072);
            assert_eq!(vk.salthalt_psu.varden.len(), 3072);

            let (mut n_land, mut n_rinn, mut n_sjo, mut n_trask) = (0, 0, 0, 0);
            let mut n_overlapp = 0;
            for c in 0..3072 {
                let vt = kart.vattentyp.varden[c];
                let fa = and.fara_andel.varden[c];
                if vt == Vatten::Trask && fa > 0.0 {
                    n_overlapp += 1;
                }
                assert_ne!(vt, Vatten::Brackt, "frö {fro} t={t}: bräckt ska vara 0");
                assert_ne!(vt, Vatten::Hav, "frö {fro} t={t}: hav ska vara 0");
                assert_eq!(
                    vk.salthalt_klass.varden[c],
                    SalthaltKlass::Sott,
                    "frö {fro} t={t}: Montana-ruta ska ha klassen Sott"
                );
                assert_eq!(
                    vk.salthalt_psu.varden[c], None,
                    "frö {fro} t={t}: Montana-ruta får aldrig ge Some(0.0) före kustfönster"
                );
                match vk.flodesklass.varden[c] {
                    Flodesklass::Land => n_land += 1,
                    Flodesklass::Rinnande => n_rinn += 1,
                    Flodesklass::Sjo => n_sjo += 1,
                    Flodesklass::Trask => n_trask += 1,
                }
            }
            assert_eq!(n_overlapp, 0, "frö {fro} t={t}: överlapp fåra och träsk ska vara 0");
            assert_eq!(n_sjo, forv_sjo, "frö {fro} t={t}: antal sjöar");
            assert!(n_rinn > 0, "frö {fro} t={t}: rinnande rutor ska vara > 0");
            assert_eq!(n_land + n_rinn + n_sjo + n_trask, 3072);

            let avst = medelavstand_km(omr.b, omr.h, &vk.flodesklass.varden);
            if ar_aldre {
                avst_aldre += avst;
            } else {
                avst_ovre += avst;
            }
        }
    }
    let m_aldre = avst_aldre / 20.0;
    let m_ovre = avst_ovre / 20.0;
    assert!(
        m_aldre > 3.5 && m_aldre < 6.0,
        "Äldre medelavstånd {m_aldre:.2} km ska ligga nära uppmätta 4,50 km"
    );
    assert!(
        m_ovre > 10.0 && m_ovre < 18.0,
        "Övre medelavstånd {m_ovre:.2} km ska ligga nära uppmätta 14,23 km"
    );
    assert!(
        m_ovre > m_aldre * 2.0,
        "Övre ({m_ovre:.2} km) ska ha mer än dubbelt så långt vattenavstånd som Äldre ({m_aldre:.2} km)"
    );
}

#[test]
fn fro_261_rinnande_rutor_som_tal() {
    let (k, tt, omr) = data_och_tt();
    for (t, forv_rinn, forv_trask, forv_sjo) in
        [(979_000.0, 268usize, 241usize, 61usize), (200_000.0, 131usize, 144usize, 46usize)]
    {
        let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, 261, 2, 2, 2).unwrap();
        let and = and_opt.expect("andelar");
        let vk = rakna_vattenkvalitet(&kart, Some(&and)).unwrap();
        let rinn = vk
            .flodesklass
            .varden
            .iter()
            .filter(|&&x| x == Flodesklass::Rinnande)
            .count();
        let trask = vk
            .flodesklass
            .varden
            .iter()
            .filter(|&&x| x == Flodesklass::Trask)
            .count();
        let sjo = vk
            .flodesklass
            .varden
            .iter()
            .filter(|&&x| x == Flodesklass::Sjo)
            .count();
        assert_eq!(rinn, forv_rinn, "frö 261 t={t}: rinnande rutor");
        assert_eq!(trask, forv_trask, "frö 261 t={t}: träskrutor");
        assert_eq!(sjo, forv_sjo, "frö 261 t={t}: sjörutor");

        // Rött med flit: om fara_andel stryks (None) försvinner alla rinnande rutor i standard 2.
        let vk_utan_fara = rakna_vattenkvalitet(&kart, None).unwrap();
        let rinn_utan = vk_utan_fara
            .flodesklass
            .varden
            .iter()
            .filter(|&&x| x == Flodesklass::Rinnande)
            .count();
        assert_eq!(rinn_utan, 0);
        assert_ne!(rinn_utan, forv_rinn);
    }
}

#[test]
fn konstruerad_kustruta_brackt_och_hav() {
    let (k, tt, omr) = data_och_tt();
    let (mut kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and = and_opt.expect("andelar");
    // Konstruera en bräckt ruta och en havsruta (Fables punkt 3.1).
    kart.vattentyp.varden[0] = Vatten::Brackt;
    kart.vattentyp.varden[1] = Vatten::Hav;
    let vk = rakna_vattenkvalitet(&kart, Some(&and)).unwrap();

    assert_eq!(vk.salthalt_klass.varden[0], SalthaltKlass::Brackt);
    assert_ne!(vk.salthalt_klass.varden[0], SalthaltKlass::Sott);
    assert_eq!(vk.salthalt_psu.varden[0], None);

    assert_eq!(vk.salthalt_klass.varden[1], SalthaltKlass::Hav);
    assert_ne!(vk.salthalt_klass.varden[1], SalthaltKlass::Sott);
    assert_eq!(vk.salthalt_psu.varden[1], None);
}

#[test]
fn kartan_lacker_inte_temperatur() {
    let (k, tt, omr) = data_och_tt();
    let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and = and_opt.expect("andelar");
    // Cursors temperaturprov: oavsett temperatur i omvärlden är kartans fysiska lager oförändrade.
    let mut summor = Vec::new();
    for _temp_c in [5.0f64, 15.0, 25.0, 35.0] {
        let vk = rakna_vattenkvalitet(&kart, Some(&and)).unwrap();
        summor.push(kontrollsumma_vattenkvalitet(&vk));
    }
    assert!(summor.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn frivillig_io_och_oforandrat_lage1() {
    let (k, tt, omr) = data_och_tt();
    let tmp = std::env::temp_dir().join(format!("hckarta-vk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    // 1. Läge 1 utan vattenkvalitetsfiler: har_vattenkvalitet == false och oförändrad kontrollsumma.
    let m1 = tmp.join("lage1");
    let k1 = karta_full(&k, &tt, &omr, 979_000.0, 261).unwrap();
    let s1_fore = kontrollsumma(&k1);
    let s1_skriv = skriv_3d(&k1, &m1).unwrap();
    assert_eq!(s1_fore, s1_skriv);
    assert!(!m1.join("vattenkvalitet.json").exists());
    let (k1_last, and1_last, vk1_last) = las_3d_med_vattenkvalitet(&m1).unwrap();
    assert!(and1_last.is_none());
    assert!(!vk1_last.har_vattenkvalitet);
    assert_eq!(kontrollsumma(&k1_last), s1_fore);

    // 2. Standard 2 utan vattenkvalitetsfiler (äldre mapp): har_vattenkvalitet == false, ingen krasch.
    let m2_gammal = tmp.join("std2_utan_vk");
    let (k2, and2_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and2 = and2_opt.expect("andelar");
    let s2_gammal = skriv_3d_med(&k2, Some(&and2), &m2_gammal).unwrap();
    let (_k2g, a2g, vk2g) = las_3d_med_vattenkvalitet(&m2_gammal).unwrap();
    assert!(a2g.is_some());
    assert!(!vk2g.har_vattenkvalitet);

    // 3. Standard 2 MED vattenkvalitet: roundtrip med samma karta-kontrollsumma och exakt lika lager.
    let m2_ny = tmp.join("std2_med_vk");
    let vk2 = rakna_vattenkvalitet(&k2, Some(&and2)).unwrap();
    let s2_ny = skriv_3d_med_vattenkvalitet(&k2, Some(&and2), Some(&vk2), &m2_ny).unwrap();
    assert_eq!(
        s2_ny, s2_gammal,
        "vattenkvalitet får inte ändra befintliga karta.json-kontrollsummor"
    );
    let (_k2n, a2n, vk2_last) = las_3d_med_vattenkvalitet(&m2_ny).unwrap();
    assert!(a2n.is_some());
    assert!(vk2_last.har_vattenkvalitet);
    assert_eq!(vk2_last, vk2);
    assert_eq!(
        kontrollsumma_vattenkvalitet(&vk2_last),
        kontrollsumma_vattenkvalitet(&vk2)
    );

    // 4. Rött prov: korrupt byte i flodesklass.u8 måste stoppas av kontrollsumman.
    let mut raw = std::fs::read(m2_ny.join("flodesklass.u8")).unwrap();
    raw[0] = if raw[0] == 0 { 1 } else { 0 };
    std::fs::write(m2_ny.join("flodesklass.u8"), &raw).unwrap();
    let err = las_3d_med_vattenkvalitet(&m2_ny).unwrap_err();
    assert!(
        err.contains("kontrollsumma"),
        "väntade kontrollsummefel, fick: {err}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn kalltabell_hel_last_och_tobin2014() {
    let t = tabelltext();
    let rader = las_vattenkvalitet_tsv(&t).unwrap();
    assert_eq!(rader.len(), 36, "antal datarader i vattenkvalitet.tsv (Alt 2214)");
    assert_eq!(
        fnv_str(&t),
        LAS_TABELL,
        "vattenkvalitet.tsv har ändrats sedan låset (Alt 2100/Alt 2214)"
    );

    // Alt 2206/Alt 2214: de fyra PMC-fulltexterna ska vara märkta fulltext med verifierade DOIs.
    for doi in [
        "10.4102/jsava.v87i1.1391",    // Bengis 2016 (PMC6138192)
        "10.1016/j.xinn.2021.100092",  // Wang 2021 (PMC8454612)
        "10.3390/ani12182423",         // Ash & Patterson 2022 (PMC9494982)
        "10.1038/s41467-023-41987-z",  // Foggin 2023 (PMC10600241)
    ] {
        let r = rader
            .iter()
            .find(|r| r.doi == doi)
            .unwrap_or_else(|| panic!("fulltextrad {doi} saknas i vattenkvalitet.tsv"));
        assert_eq!(r.last, "fulltext", "{doi} ska vara märkt fulltext efter Alt 2214");
    }

    // Fables punkt 3.3: Tobin 2014 ska finnas med DOI, sammandrag, Hell Creek och förbehåll.
    let tobin = rader
        .iter()
        .find(|r| r.doi == "10.1130/g35262.1")
        .expect("Tobin m.fl. 2014 saknas i vattenkvalitet.tsv");
    assert_eq!(tobin.last, "sammandrag");
    assert_eq!(tobin.markning, "Hell Creek");
    assert!(
        tobin.forbehall.contains("annan miljöpåverkan"),
        "Tobin-raden saknar förbehållet om annan miljöpåverkan"
    );

    // Hartman & Kirkland 2002 ska vara märkt ej_last / ingen_kalla och utan psu-tal.
    let hartman = rader
        .iter()
        .find(|r| r.doi == "10.1130/0-8137-2361-2.271")
        .expect("Hartman & Kirkland 2002 saknas");
    assert_eq!(hartman.last, "ej_last");
    assert_eq!(hartman.markning, "ingen_kalla");
    assert_eq!(hartman.varde, "-");
}

#[test]
fn roda_prov_med_flit_tabell() {
    let t = tabelltext();
    // 1. Tom fil och enbart kommentar stoppas.
    assert!(las_vattenkvalitet_tsv("").is_err());
    assert!(las_vattenkvalitet_tsv("# bara kommentar\n").is_err());

    // 2. Borttagen DOI stoppas.
    let utan_doi = t.replacen("10.1130/g35262.1", "-", 1);
    assert!(las_vattenkvalitet_tsv(&utan_doi).unwrap_err().contains("DOI"));

    // 3. Ogiltig märkning stoppas.
    let fel_mark = t.replacen("\tHell Creek\t", "\tgissning\t", 1);
    assert!(las_vattenkvalitet_tsv(&fel_mark)
        .unwrap_err()
        .contains("märkning"));

    // 4. Tal på en ej_last-rad (Hartman & Kirkland) stoppas.
    let tal_utan_kalla = t.replacen(
        "havstunga_brackt\tbrackt\tbrackt_mollusker\t-\t-",
        "havstunga_brackt\tbrackt\tbrackt_mollusker\t15\tpsu",
        1,
    );
    assert!(las_vattenkvalitet_tsv(&tal_utan_kalla)
        .unwrap_err()
        .contains("utan läst källa"));

    // 5. Saknat förbehåll stoppas.
    let utan_forbehall = t.replacen(
        "förbehåll i källan: kan bero på absolut temperatursänkning eller annan miljöpåverkan på organismerna; fulltext stängd; BRAIN avgör för klimat.rs",
        "-",
        1,
    );
    assert!(las_vattenkvalitet_tsv(&utan_forbehall)
        .unwrap_err()
        .contains("förbehåll"));

    // 6. Låset ser en enda teckenändring.
    assert_ne!(fnv_str(&t), fnv_str(&t.replacen("508.0", "508.1", 1)));
}

#[test]
fn roda_prov_enkel_klassning() {
    assert_eq!(flodesklass_av(Vatten::Land, 0.0), Flodesklass::Land);
    assert_eq!(flodesklass_av(Vatten::Land, 0.05), Flodesklass::Rinnande);
    assert_eq!(flodesklass_av(Vatten::Sjo, 0.0), Flodesklass::Sjo);
    assert_eq!(flodesklass_av(Vatten::Trask, 0.0), Flodesklass::Trask);
    assert_eq!(salthalt_klass_av(Vatten::Land), SalthaltKlass::Sott);
    assert_eq!(salthalt_klass_av(Vatten::Brackt), SalthaltKlass::Brackt);
    assert_eq!(salthalt_klass_av(Vatten::Hav), SalthaltKlass::Hav);
}
