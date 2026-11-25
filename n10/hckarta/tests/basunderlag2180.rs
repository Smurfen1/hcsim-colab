//! Enhällig omröstning förna, alger och plankton som kartlager (Alt 2180, Alt 2183, Alt 2184;
//! Håkans Alt 2189; protokoll `n10/docs/2026-11-24-omrostning-forna.md`).
//! Proven jämför med uppmätta tal ur `piharness/docs/evolution/forna-matning.txt` skrivna som tal,
//! aldrig med programmets egna konstanter.

use std::path::PathBuf;

use hckarta::{
    forna_miljo_av, karta_for_ar_lutning, karta_full, kontrollsumma, kontrollsumma_basunderlag,
    kron_tathet_av, las_3d_med_basunderlag, las_forna_alger_plankton_tsv, rakna_basunderlag,
    skriv_3d, skriv_3d_med, skriv_3d_med_basunderlag, vattenyta_andel_av, Flodesklass, FornaMiljo,
    Kalla, Omrade, Tidtabell,
};

const LAS_TABELL: u64 = 10543265791439906444;

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
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/forna_alger_plankton.tsv"
    ))
    .expect("forna_alger_plankton.tsv")
}

#[test]
fn standard2_fro_1_till_20_matningens_tal_och_mark_orord() {
    let (k, tt, omr) = data_och_tt();
    let mut s_fara_a = 0.0;
    let mut s_trask_a = 0.0;
    let mut s_sjo_a = 0.0;
    let mut s_fara_o = 0.0;
    let mut s_trask_o = 0.0;
    let mut s_sjo_o = 0.0;

    for fro in 1..=20u64 {
        for (t, ar_aldre) in [(979_000.0, true), (200_000.0, false)] {
            let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, fro, 2, 2, 2).unwrap();
            let and = and_opt.expect("standard 2 har andelar");
            let bu = rakna_basunderlag(&kart, Some(&and)).unwrap();
            assert!(bu.har_basunderlag);
            assert_eq!(bu.vattenyta_andel.varden.len(), 3072);
            assert_eq!(bu.kron_tathet.varden.len(), 3072);
            assert_eq!(bu.vattenvaxt_andel.varden.len(), 3072);
            assert_eq!(bu.forna_miljo.varden.len(), 3072);

            let (mut a_fara, mut a_trask, mut a_sjo) = (0.0f64, 0.0f64, 0.0f64);
            let (mut n_l, mut n_r, mut n_t, mut n_s) = (0usize, 0usize, 0usize, 0usize);
            let (mut k_l, mut k_r, mut k_t, mut k_s) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            let (mut vv_r, mut vv_t, mut vv_s) = (0.0f64, 0.0f64, 0.0f64);

            for c in 0..3072 {
                // Alt 2184 / Alt 2207 / Håkans låsta beslut Alt 2215: k.mark ska vara helt orört
                // [0.25; 4] på alla 3 072 rutor så att karta.json-kontrollsummorna aldrig bryts.
                let m = kart.mark.varden[c];
                assert!(
                    (m[0] - 0.25).abs() < 1e-6
                        && (m[1] - 0.25).abs() < 1e-6
                        && (m[2] - 0.25).abs() < 1e-6
                        && (m[3] - 0.25).abs() < 1e-6,
                    "frö {fro} t={t} ruta {c}: k.mark ändrad ({m:?})"
                );

                let va = bu.vattenyta_andel.varden[c];
                let kr = bu.kron_tathet.varden[c];
                let vv = bu.vattenvaxt_andel.varden[c];
                match bu.forna_miljo.varden[c] {
                    FornaMiljo::LandSkog => {
                        assert_eq!(va, 0.0, "LandSkog ska ha vattenyta_andel == 0.0");
                        n_l += 1;
                        k_l += kr as f64;
                    }
                    FornaMiljo::RinnandeStrand => {
                        let fa = and.fara_andel.varden[c];
                        assert!(
                            va > 0.0 && va < 1.0 && (va - fa).abs() < 1e-6,
                            "RinnandeStrand ska bära delrutearea fara_andel ({fa}), fick {va}"
                        );
                        n_r += 1;
                        a_fara += va as f64 * 4.0;
                        k_r += kr as f64;
                        vv_r += vv as f64;
                    }
                    FornaMiljo::TraskBland => {
                        assert_eq!(va, 1.0, "TraskBland ska ha vattenyta_andel == 1.0");
                        n_t += 1;
                        a_trask += va as f64 * 4.0;
                        k_t += kr as f64;
                        vv_t += vv as f64;
                    }
                    FornaMiljo::SjoOppen => {
                        assert_eq!(va, 1.0, "SjoOppen ska ha vattenyta_andel == 1.0");
                        assert_eq!(kr, 0.0, "SjoOppen ska ha kron_tathet == 0.0");
                        n_s += 1;
                        a_sjo += va as f64 * 4.0;
                        k_s += kr as f64;
                        vv_s += vv as f64;
                    }
                }
            }
            assert_eq!(n_l + n_r + n_t + n_s, 3072);
            assert!((k_l / n_l as f64 - 0.51).abs() < 1e-3);
            assert!((k_r / n_r as f64 - 0.51).abs() < 1e-3);
            assert!((k_t / n_t as f64 - 0.25).abs() < 1e-3);
            assert!((k_s / n_s as f64 - 0.0).abs() < 1e-6);
            assert!((vv_r / n_r as f64 - 0.0).abs() < 1e-6);
            assert!((vv_t / n_t as f64 - 0.10).abs() < 1e-3);
            assert!((vv_s / n_s as f64 - 0.30).abs() < 1e-3);

            if ar_aldre {
                s_fara_a += a_fara;
                s_trask_a += a_trask;
                s_sjo_a += a_sjo;
            } else {
                s_fara_o += a_fara;
                s_trask_o += a_trask;
                s_sjo_o += a_sjo;
            }
        }
    }

    let mf_a = s_fara_a / 20.0;
    let mt_a = s_trask_a / 20.0;
    let ms_a = s_sjo_a / 20.0;
    let mf_o = s_fara_o / 20.0;
    let mt_o = s_trask_o / 20.0;
    let ms_o = s_sjo_o / 20.0;

    assert!((mf_a - 125.7).abs() < 1.0, "Äldre fåreyta {mf_a:.1} km², väntade 125.7");
    assert!((mt_a - 1051.2).abs() < 1.0, "Äldre träskyta {mt_a:.1} km², väntade 1051.2");
    assert!((ms_a - 244.0).abs() < 0.5, "Äldre sjöyta {ms_a:.1} km², väntade 244.0");
    assert!((mf_o - 49.9).abs() < 1.0, "Övre fåreyta {mf_o:.1} km², väntade 49.9");
    assert!((mt_o - 406.6).abs() < 1.0, "Övre träskyta {mt_o:.1} km², väntade 406.6");
    assert!((ms_o - 184.0).abs() < 0.5, "Övre sjöyta {ms_o:.1} km², väntade 184.0");
}

#[test]
fn fro_261_exakta_tal() {
    let (k, tt, omr) = data_och_tt();
    for (t, forv_fara, forv_trask, forv_sjo, forv_tot) in [
        (979_000.0, 97.7f64, 964.0f64, 244.0f64, 1305.7f64),
        (200_000.0, 48.0f64, 576.0f64, 184.0f64, 808.0f64),
    ] {
        let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, t, 261, 2, 2, 2).unwrap();
        let and = and_opt.expect("andelar");
        let bu = rakna_basunderlag(&kart, Some(&and)).unwrap();

        let mut a_fara = 0.0f64;
        let mut a_trask = 0.0f64;
        let mut a_sjo = 0.0f64;
        for c in 0..3072 {
            let km2 = bu.vattenyta_andel.varden[c] as f64 * 4.0;
            match bu.forna_miljo.varden[c] {
                FornaMiljo::RinnandeStrand => a_fara += km2,
                FornaMiljo::TraskBland => a_trask += km2,
                FornaMiljo::SjoOppen => a_sjo += km2,
                FornaMiljo::LandSkog => assert_eq!(km2, 0.0),
            }
        }
        assert!((a_fara - forv_fara).abs() < 0.2, "frö 261 t={t}: fåreyta {a_fara:.1} km²");
        assert!((a_trask - forv_trask).abs() < 0.2, "frö 261 t={t}: träskyta {a_trask:.1} km²");
        assert!((a_sjo - forv_sjo).abs() < 0.2, "frö 261 t={t}: sjöyta {a_sjo:.1} km²");
        assert!(((a_fara + a_trask + a_sjo) - forv_tot).abs() < 0.3);
    }
}

#[test]
fn enkel_klassning_och_hjalpfunktioner() {
    assert_eq!(forna_miljo_av(Flodesklass::Land), FornaMiljo::LandSkog);
    assert_eq!(forna_miljo_av(Flodesklass::Rinnande), FornaMiljo::RinnandeStrand);
    assert_eq!(forna_miljo_av(Flodesklass::Trask), FornaMiljo::TraskBland);
    assert_eq!(forna_miljo_av(Flodesklass::Sjo), FornaMiljo::SjoOppen);

    assert_eq!(vattenyta_andel_av(Flodesklass::Land, 0.0), 0.0);
    assert!((vattenyta_andel_av(Flodesklass::Rinnande, 0.12) - 0.12).abs() < 1e-6);
    assert_eq!(vattenyta_andel_av(Flodesklass::Trask, 0.0), 1.0);
    assert_eq!(vattenyta_andel_av(Flodesklass::Sjo, 0.0), 1.0);
    assert!((kron_tathet_av([0.45, 0.05, 0.10, 0.02, 0.01, 0.0]) - 0.51).abs() < 1e-6);
}

#[test]
fn frivillig_io_och_oforandrat_lage1() {
    let (k, tt, omr) = data_och_tt();
    let tmp = std::env::temp_dir().join(format!("hckarta-bu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    // 1. Läge 1 utan basunderlagsfiler: har_basunderlag == false och oförändrad kontrollsumma.
    let m1 = tmp.join("lage1");
    let k1 = karta_full(&k, &tt, &omr, 979_000.0, 261).unwrap();
    let s1_fore = kontrollsumma(&k1);
    let s1_skriv = skriv_3d(&k1, &m1).unwrap();
    assert_eq!(s1_fore, s1_skriv);
    assert!(!m1.join("basunderlag.json").exists());
    let (k1_last, and1_last, bu1_last) = las_3d_med_basunderlag(&m1).unwrap();
    assert!(and1_last.is_none());
    assert!(!bu1_last.har_basunderlag);
    assert_eq!(kontrollsumma(&k1_last), s1_fore);

    // 2. Standard 2 utan basunderlagsfiler (äldre mapp): har_basunderlag == false, ingen krasch.
    let m2_gammal = tmp.join("std2_utan_bu");
    let (k2, and2_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and2 = and2_opt.expect("andelar");
    let s2_gammal = skriv_3d_med(&k2, Some(&and2), &m2_gammal).unwrap();
    let karta_json_fore = std::fs::read(m2_gammal.join("karta.json")).unwrap();
    let (_k2g, a2g, bu2g) = las_3d_med_basunderlag(&m2_gammal).unwrap();
    assert!(a2g.is_some());
    assert!(!bu2g.har_basunderlag);

    // 3. Standard 2 MED basunderlag: karta.json byte-för-byte oförändrad + exakt roundtrip.
    let m2_ny = tmp.join("std2_med_bu");
    let bu2 = rakna_basunderlag(&k2, Some(&and2)).unwrap();
    let s2_ny = skriv_3d_med_basunderlag(&k2, Some(&and2), Some(&bu2), &m2_ny).unwrap();
    assert_eq!(s2_ny, s2_gammal);
    let karta_json_efter = std::fs::read(m2_ny.join("karta.json")).unwrap();
    assert_eq!(
        karta_json_fore, karta_json_efter,
        "basunderlag får inte ändra en enda byte i karta.json"
    );
    let (_k2n, a2n, bu2_last) = las_3d_med_basunderlag(&m2_ny).unwrap();
    assert!(a2n.is_some());
    assert!(bu2_last.har_basunderlag);
    assert_eq!(bu2_last, bu2);
    assert_eq!(
        kontrollsumma_basunderlag(&bu2_last),
        kontrollsumma_basunderlag(&bu2)
    );

    // 4. Rött prov: korrupt byte i forna_miljo.u8 stoppas av kontrollsumman.
    let mut raw = std::fs::read(m2_ny.join("forna_miljo.u8")).unwrap();
    raw[0] = if raw[0] == 0 { 1 } else { 0 };
    std::fs::write(m2_ny.join("forna_miljo.u8"), &raw).unwrap();
    let err = las_3d_med_basunderlag(&m2_ny).unwrap_err();
    assert!(
        err.contains("kontrollsumma"),
        "väntade kontrollsummefel, fick: {err}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn kalltabell_hel_last_och_realismkrav() {
    let t = tabelltext();
    let rader = las_forna_alger_plankton_tsv(&t).unwrap();
    assert_eq!(rader.len(), 36, "antal datarader i forna_alger_plankton.tsv (Alt 2214)");
    assert_eq!(
        fnv_str(&t),
        LAS_TABELL,
        "forna_alger_plankton.tsv har ändrats sedan låset (Alt 2183/Alt 2214)"
    );

    // 1. Lewis 2011 förindustriell bakgrund = 160 märkt nutid/lanat.
    let lewis = rader
        .iter()
        .find(|r| r.matt == "sjo_npp_bakgrund")
        .expect("Lewis 2011 saknas");
    assert_eq!(lewis.varde, "160");
    assert_eq!(lewis.ursprung, "nutid");
    assert_eq!(lewis.markning, "lanat");
    assert_eq!(lewis.doi, "10.5268/iw-1.1.384");

    // 2. Mesozoisk algdominans (grönalger, ej kiselalger) och Hell Creek-alger (Roberts 2026).
    assert!(rader
        .iter()
        .any(|r| r.matt == "mesozoisk_algdominans" && r.varde == "gronalger_chlorococcales"));
    assert!(rader
        .iter()
        .any(|r| r.matt == "vattenalger_bakvatten" && r.ursprung == "hell_creek" && r.last == "fulltext"));
    assert!(rader
        .iter()
        .any(|r| r.matt == "tidiga_bentiska_kiselalger" && r.ursprung == "aldre"));

    // 3. Verifierade runda 1-tillägg (Fastovsky & McSweeney 1987, Retallack 1994, Tonin 2018).
    assert!(rader
        .iter()
        .any(|r| r.matt == "hoppackade_vaxtrester_sankor" && r.ursprung == "hell_creek"));
    assert!(rader
        .iter()
        .any(|r| r.matt == "klimatram_arsnederbord" && r.varde == "900-1200" && r.ursprung == "hell_creek"));

    // 4. Kvarvarande overifierade runda 1-citat har ej_last och "-"; nyverifierade Alt 2214-rader låsta.
    for kalla_namn in ["Lyson m.fl. 2011", "Van Damme & Kotov 2016"] {
        let r = rader
            .iter()
            .find(|r| r.kalla == kalla_namn)
            .unwrap_or_else(|| panic!("rad för {kalla_namn} saknas"));
        assert_eq!(r.last, "ej_last");
        assert_eq!(r.varde, "-");
    }
    let bowes = rader.iter().find(|r| r.kalla == "Bowes m.fl. 2012").unwrap();
    assert_eq!(bowes.doi, "10.1016/j.scitotenv.2011.09.082");
    assert_eq!(bowes.last, "sammandrag");
    assert_eq!(bowes.varde, "-50");
    assert_eq!(bowes.markning, "lanat");

    let khanna = rader.iter().find(|r| r.doi == "10.1002/eap.70146").unwrap();
    assert_eq!(khanna.last, "fulltext");

    let wu_barr = rader
        .iter()
        .find(|r| r.matt == "nedbrytning_k_barr_kontra_lov")
        .expect("Wu 2025 barrträd/lövträd saknas");
    assert_eq!(wu_barr.last, "fulltext");
    assert_eq!(wu_barr.varde, "0.38|1.77");

    // 5. Saknade Hell Creek-tal är ingen_kalla och "-"; Alt 2184-villkor finns.
    for matt in [
        "staende_forna_hell_creek",
        "algbiomassa_hell_creek",
        "djurplankton_biomassa_hell_creek",
    ] {
        let r = rader.iter().find(|r| r.matt == matt).unwrap();
        assert_eq!(r.markning, "ingen_kalla");
        assert_eq!(r.varde, "-");
    }
    assert!(rader
        .iter()
        .any(|r| r.matt == "alt2184_fulltext_fore_hcsim" && r.forbehall.contains("Alt 2184")));
}

#[test]
fn roda_prov_med_flit_tabell() {
    let t = tabelltext();

    // 1. Tom fil och kommentar stoppas.
    assert!(las_forna_alger_plankton_tsv("").is_err());
    assert!(las_forna_alger_plankton_tsv("# kommentar\n").is_err());

    // 2. Borttagen DOI stoppas.
    let utan_doi = t.replacen("10.5268/iw-1.1.384", "-", 1);
    assert!(las_forna_alger_plankton_tsv(&utan_doi)
        .unwrap_err()
        .contains("DOI"));

    // 3. Övergödda 260 g C/m²/år stoppas (Neons/Cursors röda prov).
    let med_260 = t.replacen(
        "sjo_npp_bakgrund\t160\tgC/m2/ar",
        "sjo_npp_bakgrund\t260\tgC/m2/ar",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&med_260)
        .unwrap_err()
        .contains("260"));

    // 4. Kiselalgsdominans märkt som hell_creek stoppas (Neons/Cursors röda prov).
    let kisel_hc = t.replacen(
        "vattenalger_bakvatten\tgronalger_och_ormbunkar\tklass",
        "vattenalger_bakvatten\tkiselalger_dominerande\tklass",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&kisel_hc)
        .unwrap_err()
        .contains("kiselalg"));

    // 5. Nutida Louisiana-rad felmärkt som hell_creek stoppas.
    let nutid_som_hc = t.replacen(
        "Sötvattenssumpskog, Louisiana (nutid; korsreferens atervaxt.tsv)\tnutid\tlanat",
        "Sötvattenssumpskog, Louisiana (nutid; korsreferens atervaxt.tsv)\thell_creek\tHell Creek",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&nutid_som_hc)
        .unwrap_err()
        .contains("hell_creek"));

    // 6. Tal på kvarvarande overifierad ej_last-rad (Lyson 2011) stoppas.
    let lyson_tal = t.replacen(
        "pediastrum_dammavlagring\t-\t-",
        "pediastrum_dammavlagring\t50\t%",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&lyson_tal)
        .unwrap_err()
        .contains("utan läst källa"));

    // 7. Felaktig Bowes-DOI (...033) stoppas.
    let fel_bowes = t.replacen(
        "10.1016/j.scitotenv.2011.09.082",
        "10.1016/j.scitotenv.2011.09.033",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&fel_bowes)
        .unwrap_err()
        .contains("fel-DOI"));

    // 8. Djurplanktonbiomassa i Hell Creek stoppas.
    let djurpl_tal = t.replacen(
        "djurplankton_biomassa_hell_creek\t-\t-",
        "djurplankton_biomassa_hell_creek\t0.5\tg/m3",
        1,
    );
    assert!(las_forna_alger_plankton_tsv(&djurpl_tal)
        .unwrap_err()
        .contains("utan läst källa"));

    // 9. Låset ser en enda teckenändring.
    assert_ne!(fnv_str(&t), fnv_str(&t.replacen("0.574", "0.575", 1)));
}
