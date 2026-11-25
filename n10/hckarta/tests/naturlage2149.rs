//! Enhällig omröstning naturhändelser (Alt 2149, Alt 2151, Alt 2153, Alt 2154; Håkans Alt 2157;
//! protokoll `n10/docs/2026-11-24-omrostning-naturhandelser.md`).
//! Proven jämför med uppmätta tal ur `piharness/docs/evolution/natur-matning.txt` och Fables
//! förmätning skrivna som tal, aldrig med programmets egna konstanter.

use std::path::PathBuf;

use hckarta::{
    bytesar, flodzon_av, karta_for_ar_lutning, karta_full, kontrollsumma, kontrollsumma_naturlage,
    las_3d_med_naturlage, las_naturhandelser_tsv, rakna_naturlage, skriv_3d, skriv_3d_med,
    skriv_3d_med_naturlage, FlodZon, Kalla, Karta, Lager, Marke, Omrade, Period, Tidtabell, Vatten,
};

const LAS_TABELL: u64 = 1299298922029289004;

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
    let tt = Tidtabell::las(
        &std::fs::read_to_string(rot.join("tidtabell.tsv")).expect("tidtabell"),
    )
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

fn tabelltext() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/naturhandelser.tsv"))
        .expect("naturhandelser.tsv")
}

#[test]
fn standard2_fro_1_till_20_matningens_tal_och_nollprov() {
    let (k, tt, omr) = data_och_tt();
    let mut sum_zon_aldre = 0usize;
    let mut sum_zon_ovre = 0usize;
    let mut sum_torr6_aldre = 0usize;
    let mut sum_torr6_ovre = 0usize;
    let mut sum_torr10_aldre = 0usize;
    let mut sum_torr10_ovre = 0usize;
    let mut sum_av_aldre = 0.0f64;
    let mut sum_av_ovre = 0.0f64;
    let mut sum_af_aldre = 0.0f64;
    let mut sum_af_ovre = 0.0f64;

    for fro in 1..=20u64 {
        let bt = bytesar(&tt, fro, 979_000.0, 0.0);
        assert!(
            (69..=78).contains(&bt.len()),
            "frö {fro}: bytesar ska vara oförändrat i 69..=78, fick {}",
            bt.len()
        );

        for (t, ar_aldre, zon_min, zon_max, forv_sjo) in [
            (979_000.0, true, 461usize, 578usize, 61usize),
            (200_000.0, false, 181usize, 266usize, 46usize),
        ] {
            let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).unwrap();
            let and = and_opt.expect("standard 2 har andelar");
            let nl = rakna_naturlage(&kart, Some(&and)).unwrap();
            assert!(nl.har_naturlage);
            assert_eq!(nl.flodzon.varden.len(), 3072);
            assert_eq!(nl.avstand_vatten_m.varden.len(), 3072);
            assert_eq!(nl.avstand_fara_m.varden.len(), 3072);

            let mut n_zon = 0usize;
            let mut n_sjo = 0usize;
            let mut n_torr6 = 0usize;
            let mut n_torr10 = 0usize;
            let mut av_sum = 0.0f64;
            let mut af_sum = 0.0f64;

            for c in 0..3072 {
                let z = nl.flodzon.varden[c];
                let av = nl.avstand_vatten_m.varden[c];
                let af = nl.avstand_fara_m.varden[c];
                let fa = and.fara_andel.varden[c];
                let vt = kart.vattentyp.varden[c];

                // Neons nollprov: varje vattenruta ger avstand_vatten_m == 0.0; icke-vatten >= 2000.0.
                let ar_fara = fa > 0.0 || vt == Vatten::Flodfara;
                let ar_vatten = ar_fara
                    || matches!(
                        vt,
                        Vatten::Trask | Vatten::Sjo | Vatten::Brackt | Vatten::Hav
                    );
                if ar_vatten {
                    assert_eq!(
                        av, 0.0,
                        "frö {fro} t={t} ruta {c}: vattenruta måste ge avstand_vatten_m == 0.0"
                    );
                } else {
                    assert!(
                        av >= 2000.0 && (av % 2000.0).abs() < 1e-3,
                        "frö {fro} t={t} ruta {c}: landruta ska ha positiv multipel av 2000 m, fick {av}"
                    );
                }
                if ar_fara {
                    assert_eq!(
                        af, 0.0,
                        "frö {fro} t={t} ruta {c}: fåreruta måste ge avstand_fara_m == 0.0"
                    );
                    assert_eq!(z, FlodZon::Fara);
                } else {
                    assert!(
                        af >= 2000.0 && (af % 2000.0).abs() < 1e-3,
                        "frö {fro} t={t} ruta {c}: icke-fåra ska ha positiv multipel av 2000 m, fick {af}"
                    );
                }
                assert!(
                    av <= af,
                    "frö {fro} t={t} ruta {c}: avstand_vatten_m ({av}) får aldrig överstiga avstand_fara_m ({af})"
                );

                if matches!(
                    z,
                    FlodZon::Fara | FlodZon::Balte | FlodZon::Vall | FlodZon::Trask
                ) {
                    n_zon += 1;
                }
                if z == FlodZon::Sjo {
                    n_sjo += 1;
                }
                if !ar_vatten && av >= 6000.0 {
                    n_torr6 += 1;
                }
                if !ar_vatten && av >= 10000.0 {
                    n_torr10 += 1;
                }
                av_sum += av as f64;
                af_sum += af as f64;
            }

            assert!(
                (zon_min..=zon_max).contains(&n_zon),
                "frö {fro} t={t}: flodnära zon {n_zon} utanför uppmätt intervall {zon_min}..={zon_max}"
            );
            assert_eq!(n_sjo, forv_sjo, "frö {fro} t={t}: antal sjörutor");

            if ar_aldre {
                sum_zon_aldre += n_zon;
                sum_torr6_aldre += n_torr6;
                sum_torr10_aldre += n_torr10;
                sum_av_aldre += av_sum / 3072.0;
                sum_af_aldre += af_sum / 3072.0;
            } else {
                sum_zon_ovre += n_zon;
                sum_torr6_ovre += n_torr6;
                sum_torr10_ovre += n_torr10;
                sum_av_ovre += av_sum / 3072.0;
                sum_af_ovre += af_sum / 3072.0;
            }
        }
    }

    let m_zon_a = sum_zon_aldre as f64 / 20.0;
    let m_zon_o = sum_zon_ovre as f64 / 20.0;
    assert!((m_zon_a - 524.1).abs() < 1.0, "Äldre zonmedel {m_zon_a:.1}, väntade 524.1");
    assert!((m_zon_o - 216.5).abs() < 1.0, "Övre zonmedel {m_zon_o:.1}, väntade 216.5");

    let m_t6_a = sum_torr6_aldre as f64 / 20.0;
    let m_t6_o = sum_torr6_ovre as f64 / 20.0;
    assert!((m_t6_a - 1203.5).abs() < 1.0, "Äldre >=6 km {m_t6_a:.1}, väntade 1203.5");
    assert!((m_t6_o - 2156.2).abs() < 1.0, "Övre >=6 km {m_t6_o:.1}, väntade 2156.2");

    let m_t10_a = sum_torr10_aldre as f64 / 20.0;
    let m_t10_o = sum_torr10_ovre as f64 / 20.0;
    assert!((m_t10_a - 426.1).abs() < 1.0, "Äldre >=10 km {m_t10_a:.1}, väntade 426.1");
    assert!((m_t10_o - 1705.8).abs() < 1.0, "Övre >=10 km {m_t10_o:.1}, väntade 1705.8");

    let m_av_a = sum_av_aldre / 20.0;
    let m_av_o = sum_av_ovre / 20.0;
    let m_af_a = sum_af_aldre / 20.0;
    let m_af_o = sum_af_ovre / 20.0;
    assert!((m_av_a - 4502.1).abs() < 5.0, "Äldre medel avstand_vatten_m {m_av_a:.1}, väntade 4502.1");
    assert!((m_av_o - 14233.5).abs() < 5.0, "Övre medel avstand_vatten_m {m_av_o:.1}, väntade 14233.5");
    assert!((m_af_a - 6011.4).abs() < 5.0, "Äldre medel avstand_fara_m {m_af_a:.1}, väntade 6011.4");
    assert!((m_af_o - 16013.1).abs() < 5.0, "Övre medel avstand_fara_m {m_af_o:.1}, väntade 16013.1");
}

#[test]
fn fro_261_exakta_tal() {
    let (k, tt, omr) = data_och_tt();
    assert_eq!(bytesar(&tt, 261, 979_000.0, 0.0).len(), 69);

    for (t, forv_fara, forv_trask, forv_sjo, forv_zon, forv_torr6, forv_av, forv_af) in [
        (979_000.0, 268usize, 241usize, 61usize, 509usize, 1251usize, 4699.2f64, 6242.2f64),
        (200_000.0, 131usize, 144usize, 46usize, 275usize, 2034usize, 11041.0f64, 12999.3f64),
    ] {
        let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, 261, 2, 2, 2).unwrap();
        let and = and_opt.expect("andelar");
        let nl = rakna_naturlage(&kart, Some(&and)).unwrap();

        let n_fara = nl.flodzon.varden.iter().filter(|&&z| z == FlodZon::Fara).count();
        let n_trask = nl.flodzon.varden.iter().filter(|&&z| z == FlodZon::Trask).count();
        let n_sjo = nl.flodzon.varden.iter().filter(|&&z| z == FlodZon::Sjo).count();
        let n_zon = nl
            .flodzon
            .varden
            .iter()
            .filter(|&&z| {
                matches!(
                    z,
                    FlodZon::Fara | FlodZon::Balte | FlodZon::Vall | FlodZon::Trask
                )
            })
            .count();
        let n_torr6 = nl
            .avstand_vatten_m
            .varden
            .iter()
            .filter(|&&d| d >= 6000.0)
            .count();
        let med_av = nl.avstand_vatten_m.varden.iter().map(|&v| v as f64).sum::<f64>() / 3072.0;
        let med_af = nl.avstand_fara_m.varden.iter().map(|&v| v as f64).sum::<f64>() / 3072.0;

        assert_eq!(n_fara, forv_fara, "frö 261 t={t}: fårerutor");
        assert_eq!(n_trask, forv_trask, "frö 261 t={t}: träskrutor");
        assert_eq!(n_sjo, forv_sjo, "frö 261 t={t}: sjörutor");
        assert_eq!(n_zon, forv_zon, "frö 261 t={t}: flodnära zon");
        assert_eq!(n_torr6, forv_torr6, "frö 261 t={t}: torra >=6 km");
        assert!((med_av - forv_av).abs() < 1.0, "frö 261 t={t}: avstand_vatten_m {med_av:.1}");
        assert!((med_af - forv_af).abs() < 1.0, "frö 261 t={t}: avstand_fara_m {med_af:.1}");
    }
}

#[test]
fn syntetisk_flodzon_balte_vall_och_chebyshev_meter() {
    // Pröva alla 6 FlodZon-varianter direkt.
    assert_eq!(flodzon_av(Vatten::Land, 0.0, 0.0, 0.0), FlodZon::Land);
    assert_eq!(flodzon_av(Vatten::Land, 0.1, 0.3, 0.2), FlodZon::Fara);
    assert_eq!(flodzon_av(Vatten::Flodfara, 0.0, 0.0, 0.0), FlodZon::Fara);
    assert_eq!(flodzon_av(Vatten::Sjo, 0.0, 0.0, 0.0), FlodZon::Sjo);
    assert_eq!(flodzon_av(Vatten::Trask, 0.0, 0.0, 0.0), FlodZon::Trask);
    assert_eq!(flodzon_av(Vatten::Land, 0.0, 0.25, 0.0), FlodZon::Balte);
    assert_eq!(flodzon_av(Vatten::Land, 0.0, 0.0, 0.15), FlodZon::Vall);

    // Litet 5x5 rutnät (2000 m/ruta) med en enda fåreruta i mitten (2,2) och bräckt/hav i hörn.
    let n = 25;
    let mut vt = vec![Vatten::Land; n];
    vt[2 * 5 + 2] = Vatten::Flodfara;
    vt[0] = Vatten::Brackt;
    vt[4] = Vatten::Hav;
    let synt = Karta {
        omrade: Omrade {
            lat0: 47.0,
            lon0: -108.0,
            b: 5,
            h: 5,
            ruta_m: 2000.0,
        },
        t: 979_000.0,
        hojd: Lager {
            varden: vec![100.0; n],
            marke: Marke::Simulering,
        },
        vatten_yta: Lager {
            varden: vec![100.0; n],
            marke: Marke::Simulering,
        },
        vattentyp: Lager {
            varden: vt,
            marke: Marke::Simulering,
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
            marke: Marke::Simulering,
        },
    };
    let nl = rakna_naturlage(&synt, None).unwrap();
    // Centrum (2,2): fåra -> 0 m till både vatten och fåra.
    assert_eq!(nl.flodzon.varden[12], FlodZon::Fara);
    assert_eq!(nl.avstand_vatten_m.varden[12], 0.0);
    assert_eq!(nl.avstand_fara_m.varden[12], 0.0);
    // Bräckt (0,0) och Hav (4,0): vattenavstånd == 0.0, fåreavstånd == 4000.0 m (2 Chebyshev-steg * 2000 m).
    assert_eq!(nl.avstand_vatten_m.varden[0], 0.0);
    assert_eq!(nl.avstand_vatten_m.varden[4], 0.0);
    assert_eq!(nl.avstand_fara_m.varden[0], 4000.0);
    assert_eq!(nl.avstand_fara_m.varden[4], 4000.0);
    // Hörn (4,4): landruta 2 steg från mitten -> 4000.0 m (aldrig 2.0 rutor!).
    assert_eq!(nl.avstand_vatten_m.varden[24], 4000.0);
    assert_eq!(nl.avstand_fara_m.varden[24], 4000.0);
    // Granne (3,2): 1 steg -> 2000.0 m.
    assert_eq!(nl.avstand_vatten_m.varden[13], 2000.0);
    assert_eq!(nl.avstand_fara_m.varden[13], 2000.0);

    // Syntetisk standard-2-karta där en ruta enbart har vall_andel > 0 och en ruta enbart balte_andel > 0.
    let (k, tt, omr) = data_och_tt();
    let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let mut and = and_opt.expect("andelar");
    let land_idx: Vec<usize> = (0..3072)
        .filter(|&c| {
            kart.vattentyp.varden[c] == Vatten::Land
                && and.fara_andel.varden[c] == 0.0
                && and.balte_andel.varden[c] == 0.0
                && and.vall_andel.varden[c] == 0.0
        })
        .take(2)
        .collect();
    assert_eq!(land_idx.len(), 2);
    and.balte_andel.varden[land_idx[0]] = 0.2;
    and.vall_andel.varden[land_idx[1]] = 0.15;
    let nl2 = rakna_naturlage(&kart, Some(&and)).unwrap();
    assert_eq!(nl2.flodzon.varden[land_idx[0]], FlodZon::Balte);
    assert_eq!(nl2.flodzon.varden[land_idx[1]], FlodZon::Vall);
}

#[test]
fn frivillig_io_och_oforandrat_lage1() {
    let (k, tt, omr) = data_och_tt();
    let tmp = std::env::temp_dir().join(format!("hckarta-nl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    // 1. Läge 1 utan naturlägesfiler: har_naturlage == false och oförändrad kontrollsumma.
    let m1 = tmp.join("lage1");
    let k1 = karta_full(&k, &tt, &omr, 979_000.0, 261).unwrap();
    let s1_fore = kontrollsumma(&k1);
    let s1_skriv = skriv_3d(&k1, &m1).unwrap();
    assert_eq!(s1_fore, s1_skriv);
    assert!(!m1.join("naturlage.json").exists());
    let (k1_last, and1_last, nl1_last) = las_3d_med_naturlage(&m1).unwrap();
    assert!(and1_last.is_none());
    assert!(!nl1_last.har_naturlage);
    assert_eq!(kontrollsumma(&k1_last), s1_fore);

    // 2. Standard 2 utan naturlägesfiler (äldre mapp): har_naturlage == false, ingen krasch.
    let m2_gammal = tmp.join("std2_utan_nl");
    let (k2, and2_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and2 = and2_opt.expect("andelar");
    let s2_gammal = skriv_3d_med(&k2, Some(&and2), &m2_gammal).unwrap();
    let (_k2g, a2g, nl2g) = las_3d_med_naturlage(&m2_gammal).unwrap();
    assert!(a2g.is_some());
    assert!(!nl2g.har_naturlage);

    // 3. Standard 2 MED naturläge: roundtrip med samma karta-kontrollsumma och exakt lika lager.
    let m2_ny = tmp.join("std2_med_nl");
    let nl2 = rakna_naturlage(&k2, Some(&and2)).unwrap();
    let s2_ny = skriv_3d_med_naturlage(&k2, Some(&and2), Some(&nl2), &m2_ny).unwrap();
    assert_eq!(
        s2_ny, s2_gammal,
        "naturläge får inte ändra befintliga karta.json-kontrollsummor"
    );
    let (_k2n, a2n, nl2_last) = las_3d_med_naturlage(&m2_ny).unwrap();
    assert!(a2n.is_some());
    assert!(nl2_last.har_naturlage);
    assert_eq!(nl2_last, nl2);
    assert_eq!(
        kontrollsumma_naturlage(&nl2_last),
        kontrollsumma_naturlage(&nl2)
    );

    // 4. Rött prov: korrupt byte i flodzon.u8 måste stoppas av kontrollsumman.
    let mut raw = std::fs::read(m2_ny.join("flodzon.u8")).unwrap();
    raw[0] = if raw[0] == 0 { 1 } else { 0 };
    std::fs::write(m2_ny.join("flodzon.u8"), &raw).unwrap();
    let err = las_3d_med_naturlage(&m2_ny).unwrap_err();
    assert!(
        err.contains("kontrollsumma"),
        "väntade kontrollsummefel, fick: {err}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn kalltabell_hel_last_och_krav_ur_omrostningen() {
    let t = tabelltext();
    let rader = las_naturhandelser_tsv(&t).unwrap();
    assert_eq!(rader.len(), 41, "antal datarader i naturhandelser.tsv");
    assert_eq!(
        fnv_str(&t),
        LAS_TABELL,
        "naturhandelser.tsv har ändrats sedan låset (Alt 2151)"
    );

    // 1. Inga lokala vulkaner + okänd askdödlighet.
    let lok_vulk = rader
        .iter()
        .find(|r| r.handelse == "vulkan" && r.matt == "lokal_vulkan")
        .expect("raden lokal_vulkan saknas");
    assert_eq!(lok_vulk.varde, "nej");

    let ask_dod = rader
        .iter()
        .find(|r| r.handelse == "vulkan" && r.matt == "ask_ekologisk_dodlighet")
        .expect("raden ask_ekologisk_dodlighet saknas");
    assert_eq!(ask_dod.markning, "ingen_kalla");
    assert_eq!(ask_dod.varde, "-");

    // 2. Alla 5 bentonitrader märks "bevarat lager, inte frekvens"; Dueling bentonite = 20 cm fulltext.
    let bentoniter: Vec<_> = rader
        .iter()
        .filter(|r| r.handelse == "vulkan" && r.matt.starts_with("bentonit_"))
        .collect();
    assert_eq!(bentoniter.len(), 5);
    for b in &bentoniter {
        assert!(b.forbehall.contains("bevarat lager, inte frekvens"));
    }
    let dueling = rader
        .iter()
        .find(|r| r.matt == "bentonit_tjocklek_dueling")
        .expect("Dueling bentonite saknas");
    assert_eq!(dueling.varde, "20");
    assert_eq!(dueling.enhet, "cm");
    assert_eq!(dueling.last, "fulltext");
    assert_eq!(dueling.doi, "10.1371/journal.pone.0328861");

    // 3. Alla 5 overifierade källor ur runda 1 har last=ej_last och varde="-".
    for kalla_namn in [
        "Arens & Allen 2014",
        "Brown m.fl. 2012",
        "Lyson & Longrich 2011",
        "DePalma 2010",
        "Sprain eScholarship-PDF",
    ] {
        let r = rader
            .iter()
            .find(|r| r.kalla == kalla_namn)
            .unwrap_or_else(|| panic!("overifierad källa {kalla_namn} saknas"));
        assert_eq!(r.last, "ej_last", "{kalla_namn} måste vara ej_last");
        assert_eq!(r.varde, "-", "{kalla_namn} får inte bära ett tal");
    }

    // 4. Alla tre klimatståndpunkter och alla tre Deccan-ståndpunkter sida vid sida.
    for doi in [
        "10.1130/g35262.1",       // Tobin 2014
        "10.1073/pnas.0234701100", // Wilf 2003
        "10.1130/G39771.1",       // Barnet 2018
        "10.1126/science.aau2422", // Schoene 2019
        "10.1126/science.aav1446", // Sprain 2019
        "10.1126/science.aay5055", // Hull 2020
    ] {
        assert!(
            rader.iter().any(|r| r.doi == doi),
            "DOI {doi} saknas i naturhandelser.tsv"
        );
    }

    // 5. Ursprungskolumnen skiljer grannformationer och äldre formationer från Hell Creek.
    let jerrett = rader
        .iter()
        .find(|r| r.doi == "10.1130/b31166.1")
        .expect("Jerrett 2015 saknas");
    assert_eq!(jerrett.ursprung, "grannformation");
    assert_eq!(jerrett.flodzon, "trask");

    let sereno = rader
        .iter()
        .find(|r| r.doi == "10.1126/science.adw3536")
        .expect("Sereno 2025 saknas");
    assert_eq!(sereno.ursprung, "grannformation");

    let rogers = rader
        .iter()
        .find(|r| r.doi == "10.2307/3514834")
        .expect("Rogers 1990 saknas");
    assert_eq!(rogers.ursprung, "aldre");

    // 6. Alt 2154 villkorsrad finns.
    assert!(rader
        .iter()
        .any(|r| r.matt == "alt2154_fulltext_fore_hcsim" && r.forbehall.contains("Alt 2154")));
}

#[test]
fn roda_prov_med_flit_tabell() {
    let t = tabelltext();

    // 1. Tom fil och kommentar stoppas.
    assert!(las_naturhandelser_tsv("").is_err());
    assert!(las_naturhandelser_tsv("# kommentar\n").is_err());

    // 2. Borttagen DOI stoppas.
    let utan_doi = t.replacen("10.1130/g35262.1", "-", 1);
    assert!(las_naturhandelser_tsv(&utan_doi).unwrap_err().contains("DOI"));

    // 3. Ogiltigt ursprung stoppas.
    let fel_ursprung = t.replacen("\tgrannformation\t", "\tokand_plats\t", 1);
    assert!(las_naturhandelser_tsv(&fel_ursprung)
        .unwrap_err()
        .contains("ursprung"));

    // 4. Grannformation (Lance/Wyoming) felmärkt som hell_creek stoppas (Fables/Neons röda prov).
    let lance_som_hc = t.replacen(
        "Lance, östra Wyoming (samtida grannformation söder om Hell Creek)\tgrannformation\tlanat",
        "Lance, östra Wyoming (samtida grannformation söder om Hell Creek)\thell_creek\tHell Creek",
        1,
    );
    assert!(las_naturhandelser_tsv(&lance_som_hc)
        .unwrap_err()
        .contains("hell_creek"));

    // 5. Tal på overifierad ej_last-rad (Arens & Allen 2014) stoppas.
    let arens_tal = t.replacen(
        "nederbord_basala_hell_creek\t-\t-",
        "nederbord_basala_hell_creek\t197\tcm",
        1,
    );
    assert!(las_naturhandelser_tsv(&arens_tal)
        .unwrap_err()
        .contains("utan läst källa"));

    // 6. Lokal vulkan satt till "ja" stoppas (Neons röda prov).
    let lokal_vulk_ja = t.replacen("lokal_vulkan\tnej\tbool", "lokal_vulkan\tja\tbool", 1);
    assert!(las_naturhandelser_tsv(&lokal_vulk_ja)
        .unwrap_err()
        .contains("lokal_vulkan"));

    // 7. Askdödlighet > 0 utan källa stoppas (Cursors röda prov).
    let ask_dod_tal = t.replacen(
        "ask_ekologisk_dodlighet\t-\t-",
        "ask_ekologisk_dodlighet\t0.25\tandel",
        1,
    );
    assert!(las_naturhandelser_tsv(&ask_dod_tal)
        .unwrap_err()
        .contains("utan läst källa"));

    // 8. Bentonitrad utan "bevarat lager, inte frekvens" stoppas.
    let bent_utan_varning = t.replacen("bevarat lager, inte frekvens;", "frekvens 1 per 1000 ar;", 1);
    assert!(las_naturhandelser_tsv(&bent_utan_varning)
        .unwrap_err()
        .contains("bevarat lager, inte frekvens"));

    // 9. Låset ser en enda teckenändring.
    assert_ne!(fnv_str(&t), fnv_str(&t.replacen("66.936", "66.937", 1)));
}
