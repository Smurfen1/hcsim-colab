//! Låst källtabell och hårda spärrar för övriga sjukdomar i Hell Creek (Alt 2216 + Alt 2217;
//! Håkans byggbeslut Alt 2220; protokoll `n10/docs/2026-11-24-omrostning-sjukdomar.md`).
//! Viktigt: noll nya kartlager i `hckarta`; `vattenkvalitet.tsv`, `karta.json` och `k.mark` förblir orörda.

use std::path::PathBuf;

use hckarta::{
    karta_for_ar_lutning, kontrollsumma_med, las_sjukdomar_tsv, las_vattenkvalitet_tsv, Kalla,
    Omrade, Tidtabell,
};

const LAS_SJUKDOMAR: u64 = 8842831950641959822;
const LAS_VATTEN_ORORD: u64 = 11522424303521908281;

fn fnv_str(t: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in t.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn sjukdomstext() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/sjukdomar.tsv"))
        .expect("sjukdomar.tsv")
}

fn vattentext() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/vattenkvalitet.tsv"))
        .expect("vattenkvalitet.tsv")
}

#[test]
fn kalltabell_hel_last_och_vattenkvalitet_orord() {
    let t = sjukdomstext();
    let rader = las_sjukdomar_tsv(&t).unwrap();
    assert_eq!(rader.len(), 25, "antal datarader i sjukdomar.tsv (Alt 2216/2217)");
    assert_eq!(
        fnv_str(&t),
        LAS_SJUKDOMAR,
        "sjukdomar.tsv har ändrats sedan låset (Alt 2220)"
    );

    // Alt 2217 del (5): vattenkvalitet.tsv (inkl. rad 15-19) ska vara helt orörd.
    let vt = vattentext();
    assert_eq!(
        fnv_str(&vt),
        LAS_VATTEN_ORORD,
        "Alt 2217: vattenkvalitet.tsv får inte ändras av sjukdomar.tsv"
    );
    let vrader = las_vattenkvalitet_tsv(&vt).unwrap();
    assert_eq!(vrader.len(), 36);

    // 1. Alla sex Hell Creek-patologier finns med ursprung=hell_creek och markning=Hell Creek.
    for doi in [
        "10.1371/journal.pone.0007288", // Wolff 2009 Trichomonas
        "10.1371/journal.pone.0036227", // Peterson & Vittore 2012
        "10.1038/s41598-020-75731-0",   // Hamm 2020 T. rex osteomyelit
        "10.1371/journal.pone.0004252", // Farke 2009 Triceratops
        "10.7717/peerj.20796",          // Bertozzo 2026 Edmontosaurus
        "10.1007/s00114-003-0473-9",    // Rothschild 2003 tumörer
    ] {
        assert!(
            rader
                .iter()
                .any(|r| r.doi == doi && r.ursprung == "hell_creek" && r.markning == "Hell Creek"),
            "Hell Creek-källa {doi} saknas eller är felmärkt i sjukdomar.tsv"
        );
    }

    // 2. Hamm 2020 nutida procentsatser 32.0|53.9 ligger på egen nutid/lanat-rad.
    let hamm_nutid = rader
        .iter()
        .find(|r| r.matt == "osteomyelit_nutida_faglar_reptiler")
        .expect("Hamm 2020 nutidsrad saknas");
    assert_eq!(hamm_nutid.varde, "32.0|53.9");
    assert_eq!(hamm_nutid.ursprung, "nutid");
    assert_eq!(hamm_nutid.markning, "lanat");

    // 3. Fästingar (Peñalver 2017) och sandmyggor (Kaczmarek 2025) märks aldre/lanat + Varelser.
    for (doi, ord) in [
        ("10.1038/s41467-017-01550-z", "Varelser"),
        ("10.1038/s41598-025-23797-z", "Varelser"),
    ] {
        let r = rader
            .iter()
            .find(|r| r.doi == doi)
            .unwrap_or_else(|| panic!("vektorkälla {doi} saknas"));
        assert_eq!(r.ursprung, "aldre");
        assert_eq!(r.markning, "lanat");
        assert!(r.forbehall.contains(ord), "{doi} saknar Varelser-hänvisning");
    }

    // 4. Foggin 2023 (fulltext från Alt 2206) och Dolfi 2024 (nutid, ej B. anthracis i Hell Creek).
    let foggin = rader
        .iter()
        .find(|r| r.doi == "10.1038/s41467-023-41987-z")
        .expect("Foggin 2023 saknas");
    assert_eq!(foggin.last, "fulltext");
    assert_eq!(foggin.ursprung, "nutid");

    let dolfi = rader
        .iter()
        .find(|r| r.doi == "10.1098/rspb.2023.2568")
        .expect("Dolfi 2024 saknas");
    assert_eq!(dolfi.ursprung, "nutid");
    assert!(dolfi.forbehall.contains("INTE"));

    // 5. Alla 5 obligatoriska spärr- och ägarrader + 2 näringsvävskopplingar har ej_last / ingen_kalla / "-".
    for matt in [
        "fyra_blodsugare_lankar_tsv",
        "fyra_kadavernedbrytare_lankar_tsv",
        "sparr_r0_och_smittrater",
        "sparr_moderna_virus",
        "sparr_vattenburna_dubbelrakning",
        "agargrans_varelser_vektorer",
        "agargrans_brain_hcsim",
    ] {
        let r = rader
            .iter()
            .find(|r| r.matt == matt)
            .unwrap_or_else(|| panic!("obligatorisk rad {matt} saknas"));
        assert_eq!(r.last, "ej_last");
        assert_eq!(r.markning, "ingen_kalla");
        assert_eq!(r.varde, "-");
    }
}

#[test]
fn noll_nya_kartlager_och_oforandrad_karta_och_mark() {
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

    let (kart, and_opt) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    let and = and_opt.expect("standard 2 har andelar");
    // Deterministisk kontrollsumma och orörd k.mark = [0.25; 4] på alla 3 072 rutor.
    let s1 = kontrollsumma_med(&kart, Some(&and));
    let (kart2, and2) = karta_for_ar_lutning(&k, &tt, &omr, 979_000.0, 261, 2, 2, 2).unwrap();
    assert_eq!(s1, kontrollsumma_med(&kart2, and2.as_ref()));
    assert!(kart
        .mark
        .varden
        .iter()
        .all(|m| m.iter().all(|&x| (x - 0.25).abs() < 1e-6)));
}

#[test]
fn roda_prov_med_flit_sparrar() {
    let t = sjukdomstext();

    // 1. Tom fil och enbart kommentar stoppas.
    assert!(las_sjukdomar_tsv("").is_err());
    assert!(las_sjukdomar_tsv("# bara kommentar\n").is_err());

    // 2. Påhittat R0-tal på spärrrad stoppas (Alt 2217 / regel 16).
    let med_r0 = t.replacen(
        "sparr_r0_och_smittrater\t-\t-",
        "sparr_r0_och_smittrater\t2.4\tR0",
        1,
    );
    assert!(las_sjukdomar_tsv(&med_r0)
        .unwrap_err()
        .contains("utan läst källa"));

    // 3. Borttagen DOI på läst rad stoppas.
    let utan_doi = t.replacen("10.1371/journal.pone.0007288", "-", 1);
    assert!(las_sjukdomar_tsv(&utan_doi).unwrap_err().contains("DOI"));

    // 4. Vattenburen dubbelräkning (cyanotoxin/botulism som ny rad) stoppas.
    let med_cyano = format!(
        "{t}cyanotoxin\tdricksvatten\tcyanotoxin_ny\t10\tug/L\tBengis 2016\t10.4102/jsava.v87i1.1391\tcitat\tfulltext\tnutid\tnutid\tlanat\tförbehåll\n"
    );
    assert!(las_sjukdomar_tsv(&med_cyano)
        .unwrap_err()
        .contains("dubbelräkning"));

    // 5. Modernt virus utan kritabelägg stoppas.
    let med_virus = format!(
        "{t}influensa\tluft\tinfluensa_fagel\t0.1\tandel\tTest 2020\t10.1000/test\tcitat\tsammandrag\tnutid\tnutid\tlanat\tförbehåll\n"
    );
    assert!(las_sjukdomar_tsv(&med_virus)
        .unwrap_err()
        .contains("modernt virus"));

    // 6. Myanmar-fästingar (Peñalver 2017) felmärkta som hell_creek stoppas.
    let myanmar_hc = t.replacen(
        "99 Ma bärnsten, Myanmar (PMC5727220, äldre än Hell Creek)\taldre\tlanat",
        "99 Ma bärnsten, Myanmar (PMC5727220, äldre än Hell Creek)\thell_creek\tHell Creek",
        1,
    );
    assert!(las_sjukdomar_tsv(&myanmar_hc)
        .unwrap_err()
        .contains("hell_creek"));

    // 7. Bacillus anthracis (Dolfi 2024) felmärkt som hell_creek stoppas.
    let anthrax_hc = t.replacen(
        "Etosha National Park, Namibia, nutid (PMC10846954)\tnutid\tlanat",
        "Etosha National Park, Namibia, nutid (PMC10846954)\thell_creek\tHell Creek",
        1,
    );
    assert!(las_sjukdomar_tsv(&anthrax_hc)
        .unwrap_err()
        .contains("hell_creek"));

    // 8. Nutida jämförelsetal 32.0|53.9 (Hamm 2020) felmärkta som hell_creek stoppas.
    let hamm_hc = t.replacen(
        "Nutida fåglar (32,0 %) och icke-fågel-kräldjur (53,9 %)\tnutid\tlanat",
        "Hell Creek-dinosaurier\thell_creek\tHell Creek",
        1,
    );
    assert!(las_sjukdomar_tsv(&hamm_hc)
        .unwrap_err()
        .contains("32.0|53.9"));

    // 9. Låset ser en enda teckenändring.
    assert_ne!(fnv_str(&t), fnv_str(&t.replacen("32.0|53.9", "32.1|53.9", 1)));
}
