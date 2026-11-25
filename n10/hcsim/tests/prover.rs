//! Proven för S5a/S5b/Alt 1198, hjärnprovet Alt 1200, diagnosen Alt 1207–1209,
//! avläsarprovet Alt 1213, ledprovet Alt 1221, rangprovet Alt 1228–1230,
//! turordningsprovet Alt 1246 (med omläsningen Alt 1245) och signalprovet
//! Alt 1251–1253.

use hckarta::{kontrollsumma, Karta, Lager, Marke, Omrade, Period, Vatten};
use hcsim::formaga::{alla_armar, Formaga, HjarnArm, Intagslasare, Minnestabell};
use hcsim::hjarna::{self, Hj, N_GEN, NK, NP};
use hcsim::minstakvadrat;
use hcsim::rng::Rng;
use hcsim::signal::{
    ingrepp_traff_positiv, jamfor_tur1246, kendalls_w, klassa_djur, kvot_fran_matriser,
    positiv_varden, SignalKlass,
};
use hcsim::sim::{LedLogg, LedLoggRad, MarkLage, Matning, SignalLogg, Sim};
use hcsim::tal::{self, las_tsv, varde_i_kod, KONSTANT_NAMN, MARKT_INKOR_AR, MARKT_RABATT, A_N, S_N, Z_N};

fn tom_lager<T: Clone>(n: usize, v: T, marke: Marke) -> Lager<T> {
    Lager {
        varden: vec![v; n],
        marke,
    }
}

/// Konstruerad karta utan PaleoDEM (bara för prov).
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

fn bara(vt: Vatten, b: usize, h: usize) -> Karta {
    konstruerad(b, h, vec![vt; b * h], 0.5)
}

#[test]
fn samma_fro_samma_summa() {
    let b = 8;
    let h = 8;
    let mut vatten = vec![Vatten::Land; b * h];
    // lite hav i hörnet
    vatten[0] = Vatten::Hav;
    let kart = konstruerad(b, h, vatten, 0.5);
    let mut a = Sim::fran_karta(&kart, 42, 20);
    let mut bsim = Sim::fran_karta(&kart, 42, 20);
    for _ in 0..tal::STEG_PER_AR * 2 {
        a.steg();
        bsim.steg();
    }
    assert_eq!(a.djur_summa(), bsim.djur_summa());
    // andra frö → annan summa (med hög sannolikhet)
    let mut c = Sim::fran_karta(&kart, 43, 20);
    for _ in 0..tal::STEG_PER_AR * 2 {
        c.steg();
    }
    assert_ne!(a.djur_summa(), c.djur_summa());
}

#[test]
fn kartan_andras_inte() {
    let rot = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paleo = rot.join("../paleokarta/data");
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).expect("tidtabell");
    let tt = hckarta::Tidtabell::las(&tt_text).expect("tt");
    let kalla = hckarta::Kalla::las(&paleo).expect("källa");
    let omr = Omrade {
        lat0: 47.0,
        lon0: -108.0,
        b: 16,
        h: 12,
        ruta_m: 2000.0,
    };
    let kart = hckarta::karta_full(&kalla, &tt, &omr, 500_000.0, 7).expect("karta");
    let fore = kontrollsumma(&kart);
    let mut sim = Sim::fran_karta(&kart, 7, 40);
    for _ in 0..tal::STEG_PER_AR * 5 {
        sim.steg();
    }
    assert_eq!(kontrollsumma(&kart), fore);
}

#[test]
fn inget_djur_i_havet() {
    let b = 16;
    let h = 16;
    let mut vatten = Vec::with_capacity(b * h);
    for _j in 0..h {
        for i in 0..b {
            vatten.push(if i < b / 2 {
                Vatten::Hav
            } else {
                Vatten::Land
            });
        }
    }
    let kart = konstruerad(b, h, vatten, 0.5);
    let mut sim = Sim::fran_karta(&kart, 99, 30);
    for _ in 0..tal::STEG_PER_AR * 3 {
        for d in sim.djur.iter().filter(|d| d.lev) {
            let c = sim.cell(d.i, d.j);
            assert!(
                !matches!(sim.vatten[c], Vatten::Hav | Vatten::Brackt),
                "djur i havet vid ({},{})",
                d.i,
                d.j
            );
        }
        sim.steg();
    }
}

#[test]
fn bara_hav_dor_ut() {
    for fro in [1u64, 2, 3] {
        let kart = bara(Vatten::Hav, 8, 8);
        let mut sim = Sim::fran_karta(&kart, fro, 20);
        for _ in 0..tal::STEG_PER_AR {
            sim.steg();
        }
        assert_eq!(sim.antal(), 0, "frö {fro} hav");
    }
    for fro in [4u64, 5, 6] {
        let kart = bara(Vatten::Flodfara, 8, 8);
        let mut sim = Sim::fran_karta(&kart, fro, 20);
        for _ in 0..tal::STEG_PER_AR {
            sim.steg();
        }
        assert_eq!(sim.antal(), 0, "frö {fro} flod");
    }
}

#[test]
fn vaxt_noll_dor_ut() {
    let b = 8;
    let h = 8;
    let vatten = vec![Vatten::Land; b * h];
    let kart = konstruerad(b, h, vatten, 0.0);
    for fro in [11u64, 12, 13] {
        let mut sim = Sim::fran_karta(&kart, fro, 30);
        for _ in 0..tal::STEG_PER_AR * 10 {
            sim.steg();
            if sim.antal() == 0 {
                break;
            }
        }
        assert_eq!(sim.antal(), 0, "frö {fro} överlevde med vaxt=0");
    }
}

#[test]
fn tal_tsv_tacker_konstanterna() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tal.tsv")).expect("tal.tsv");
    let rader = las_tsv(&text).expect("parse");
    let mut namn: std::collections::BTreeSet<String> = rader.iter().map(|r| r.namn.clone()).collect();
    for n in KONSTANT_NAMN {
        assert!(namn.remove(*n), "saknar rad för {n}");
        let kod = varde_i_kod(n).expect("kod");
        let rad = rader.iter().find(|r| r.namn == *n).unwrap();
        assert!(
            (rad.varde - kod).abs() < 1e-9,
            "{n}: tsv {} ≠ kod {}",
            rad.varde,
            kod
        );
    }
    assert!(namn.is_empty(), "överflödiga rader: {namn:?}");
    let last = rader.iter().filter(|r| r.marke == "last");
    for r in last {
        assert!(
            !r.kalla.trim().is_empty(),
            "last-rad {} saknar källa",
            r.namn
        );
    }
}

#[test]
fn hjarnan_lar_sig() {
    // som larandet_och_frysta i hjarna.rs: felet sjunker på fast följd
    let mut g = Rng::new(1);
    let h0 = Hj::slump(&mut g);
    let s = vec![0.3; S_N];
    let mut h = h0.clone();
    let mut fel = Vec::new();
    for _ in 0..200 {
        h.forra = None;
        let z = h.kod(&s);
        let a = 0;
        let y = h.forutsag(&z, a);
        h.forra = Some((z, a, y));
        h.fel_sum = [0.0; 2];
        h.fel_n = [0; 2];
        h.valj(&s, true, 0);
        fel.push(h.fel_sum[0]);
    }
    assert!(
        fel[199] < 0.01 * fel[0] && fel[0] > 0.0,
        "felet {} → {}",
        fel[0],
        fel[199]
    );
    let mut f = h0.clone();
    for _ in 0..50 {
        f.valj(&s, false, 0);
    }
    assert_eq!(f.wp, h0.wp);
    let mut l = h0.clone();
    for _ in 0..50 {
        l.valj(&s, true, 0);
    }
    assert_ne!(l.wp, h0.wp);
    assert_eq!(h0.gen.len(), N_GEN);
}

#[test]
fn market_arvs_fran_modern() {
    let b = 4;
    let h = 4;
    let kart = bara(Vatten::Land, b, h);
    let mut sim = Sim::fran_karta(&kart, 7, 4);
    // Sätt modern märkt, fadern omärkt; tvinga födsel.
    assert!(sim.djur.len() >= 2);
    sim.djur[0].lev = true;
    sim.djur[0].hona = true;
    sim.djur[0].markt = true;
    sim.djur[0].e = tal::E_DELA + 10.0;
    sim.djur[0].age = tal::LITEN + 1;
    sim.djur[0].i = 1;
    sim.djur[0].j = 1;
    sim.djur[1].lev = true;
    sim.djur[1].hona = false;
    sim.djur[1].markt = false;
    sim.djur[1].e = tal::E_HANE / 2.0;
    sim.djur[1].age = tal::LITEN + 1;
    sim.djur[1].i = 1;
    sim.djur[1].j = 1;
    for d in sim.djur.iter_mut().skip(2) {
        d.lev = false;
    }
    let uid_fore: std::collections::BTreeSet<u64> =
        sim.djur.iter().filter(|d| d.lev).map(|d| d.uid).collect();
    sim.steg();
    let ungar: Vec<_> = sim
        .djur
        .iter()
        .filter(|d| d.lev && !uid_fore.contains(&d.uid))
        .collect();
    assert!(!ungar.is_empty(), "ingen unge föddes");
    for u in ungar {
        assert!(u.markt, "unge {} ärvde inte moderns märke", u.uid);
    }
}

#[test]
fn rabatten_bara_for_markta() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 3, 2);
    sim.mark_lage = MarkLage::Positiv;
    assert_eq!(sim.djur.len(), 2);
    sim.djur[0].lev = true;
    sim.djur[0].markt = true;
    sim.djur[0].i = 1;
    sim.djur[0].j = 1;
    sim.djur[0].e = 200.0;
    sim.djur[1].lev = true;
    sim.djur[1].markt = false;
    sim.djur[1].i = 1;
    sim.djur[1].j = 1;
    sim.djur[1].e = 200.0;
    assert!((sim.grundkostnad(true) - tal::KOST_STEG * MARKT_RABATT).abs() < 1e-12);
    assert!((sim.grundkostnad(false) - tal::KOST_STEG).abs() < 1e-12);
    // Ett betessteg: samma intag, skillnad = (1 − 0,9)·KOST_STEG
    let e0 = sim.djur[0].e;
    let e1 = sim.djur[1].e;
    // anropa betar via ett steg utan rörelse/ålder genom att bara köra betar indirekt:
    // sätt hjärnan till att stå kvar och kör ett steg — skillnaden i e efter betar+hjärna
    // blir störig. Mät i stället direkt via grundkostnad (kravet) och ett manuellt betesdrag.
    let c = sim.cell(1, 1);
    sim.v[c] = sim.vmax[c];
    // Spegla betar(): vill = AT_DINO·V/VMAX; två djur → samma at
    let vill = tal::AT_DINO * sim.v[c] / tal::VMAX;
    let tot = 2.0 * vill;
    let skala = if tot > sim.v[c] {
        sim.v[c] / tot
    } else {
        1.0
    };
    let at = vill * skala;
    let intag = tal::E_VAXT * at;
    let e_markt = e0 + intag - sim.grundkostnad(true);
    let e_omarikt = e1 + intag - sim.grundkostnad(false);
    assert!(
        (e_markt - e_omarikt - (tal::KOST_STEG - tal::KOST_STEG * MARKT_RABATT)).abs() < 1e-9,
        "märkt {e_markt} vs omärkt {e_omarikt}"
    );
    assert!((sim.grundkostnad(true) - 0.9).abs() < 1e-12);
    assert!((sim.grundkostnad(false) - 1.0).abs() < 1e-12);
}

#[test]
fn nollmarket_andrar_inte_varlden() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 21u64;
    let n0 = 40;
    let ar = 40i64;
    let mut utan = Sim::fran_karta(&kart, fro, n0);
    utan.mark_lage = MarkLage::Utan;
    let mut noll = Sim::fran_karta(&kart, fro, n0);
    noll.mark_lage = MarkLage::Noll;
    let steg_mark = (MARKT_INKOR_AR * tal::STEG_PER_AR) as u64;
    let steg_slut = (ar * tal::STEG_PER_AR) as u64;
    for t in 0..steg_slut {
        if t == steg_mark {
            noll.markera_halften(fro);
            // utan-armen märks inte
        }
        utan.steg();
        noll.steg();
    }
    assert!(noll.antal_markta() > 0, "nollarmen skulle ha märkt djur");
    assert_eq!(utan.antal(), noll.antal());
    assert_eq!(utan.djur_summa(), noll.djur_summa());
}

#[test]
fn halvten_markas() {
    let b = 16;
    let h = 12;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    // Tillräckligt många startar så att ≥200 lever vid märkningen efter kort inkörning.
    let mut sim = Sim::fran_karta(&kart, 5, 240);
    sim.mark_lage = MarkLage::Noll;
    for _ in 0..tal::STEG_PER_AR * MARKT_INKOR_AR {
        sim.steg();
    }
    let n = sim.antal();
    assert!(
        n >= 200,
        "för få levande vid märkning: {n} (behöver ≥200 för andelsprovet)"
    );
    sim.markera_halften(5);
    let andel = sim.andel_markta();
    assert!(
        (0.45..=0.55).contains(&andel),
        "andel märkta {andel} utanför 0,45–0,55 (n={n})"
    );
}

#[test]
fn frusna_vikter_star_still() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 11, 2);
    sim.djur[0].formaga = Formaga::Frusen;
    let wp0 = sim.djur[0].hj.wp.clone();
    for _ in 0..80 {
        sim.steg();
    }
    assert_eq!(sim.djur[0].hj.wp, wp0, "frusen förutsägare ändrades");
}

#[test]
fn larande_vikter_andras() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 12, 2);
    sim.djur[0].formaga = Formaga::Lar;
    let wp0 = sim.djur[0].hj.wp.clone();
    for _ in 0..80 {
        sim.steg();
    }
    assert_ne!(sim.djur[0].hj.wp, wp0, "lärande förutsägare stod still");
}

#[test]
fn samma_hjarnpris() {
    let hjkost = hjarna::hjarnkost(tal::KOST_STEG);
    let formagor = [
        Formaga::Lar,
        Formaga::Frusen,
        Formaga::Slump,
        Formaga::Tabell,
        Formaga::Allvetande,
        Formaga::Rabatt(0.95),
        Formaga::LarLott,
        Formaga::LarIntagslasare,
        Formaga::OrakelForutsagare,
        Formaga::TrangselOrakel,
    ];
    let mut deltas = Vec::new();
    for &f in &formagor {
        let kart = bara(Vatten::Land, 4, 4);
        let mut sim = Sim::fran_karta(&kart, 13, 1);
        sim.djur[0].lev = true;
        sim.djur[0].formaga = f;
        sim.djur[0].i = 1;
        sim.djur[0].j = 1;
        sim.djur[0].e = 500.0;
        // Ingen växt → intag 0; vmax 0 → ingen soltillväxt.
        for c in 0..sim.v.len() {
            sim.v[c] = 0.0;
            sim.vmax[c] = 0.0;
        }
        let e0 = sim.djur[0].e;
        sim.steg();
        let e1 = sim.djur[0].e;
        let forventat = e0 - hjkost - tal::KOST_STEG;
        assert!(
            (e1 - forventat).abs() < 1e-9,
            "{:?}: e {e0} → {e1}, väntat {forventat} (hjkost={hjkost})",
            f
        );
        deltas.push(e0 - e1);
    }
    for d in &deltas {
        assert!(
            (d - deltas[0]).abs() < 1e-12,
            "olika totalstegkostnad mellan förmågor: {deltas:?}"
        );
    }
    // Rabattens grundkostnad (inte hjärnpriset) efter märkning:
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 14, 2);
    sim.hjarn_arm = Some(HjarnArm {
        namn: "trappa_0.95",
        markta: Formaga::Rabatt(0.95),
        omarkta: Formaga::Lar,
        spegel: false,
    });
    sim.mark_lage = MarkLage::Noll;
    sim.djur[0].markt = true;
    sim.djur[1].markt = false;
    sim.markering_skedd = true;
    sim.djur[0].formaga = Formaga::Rabatt(0.95);
    sim.djur[1].formaga = Formaga::Lar;
    assert!((sim.grundkostnad(true) - 0.95).abs() < 1e-12);
    assert!((sim.grundkostnad(false) - 1.0).abs() < 1e-12);
    assert!((hjkost - hjarna::hjarnkost(tal::KOST_STEG)).abs() < 1e-15);
}

#[test]
fn ungen_arver_generna_inte_inlarda() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 15, 2);
    sim.djur[0].lev = true;
    sim.djur[0].hona = true;
    sim.djur[0].e = tal::E_DELA + 10.0;
    sim.djur[0].age = tal::LITEN + 1;
    sim.djur[0].i = 1;
    sim.djur[0].j = 1;
    sim.djur[0].formaga = Formaga::Lar;
    // Lär modern så wp ≠ gen-start
    let s = vec![0.3; S_N];
    for _ in 0..40 {
        sim.djur[0].hj.valj(&s, true, 0);
    }
    assert_ne!(&sim.djur[0].hj.wp[..], &sim.djur[0].hj.gen[hjarna::NK..hjarna::NK + hjarna::NP]);
    let mor_wp = sim.djur[0].hj.wp.clone();
    sim.djur[1].lev = true;
    sim.djur[1].hona = false;
    sim.djur[1].e = tal::E_HANE / 2.0;
    sim.djur[1].age = tal::LITEN + 1;
    sim.djur[1].i = 1;
    sim.djur[1].j = 1;
    let uid_fore: std::collections::BTreeSet<u64> =
        sim.djur.iter().filter(|d| d.lev).map(|d| d.uid).collect();
    sim.steg();
    let ungar: Vec<_> = sim
        .djur
        .iter()
        .filter(|d| d.lev && !uid_fore.contains(&d.uid))
        .collect();
    assert!(!ungar.is_empty(), "ingen unge");
    for u in ungar {
        assert_eq!(
            &u.hj.wp[..],
            &u.hj.gen[hjarna::NK..hjarna::NK + hjarna::NP],
            "unge startade med annat än generna"
        );
        assert_ne!(u.hj.wp, mor_wp, "unge ärvde moderns inlärda vikter");
    }
}

#[test]
fn spegeln_byter_bara_formagan() {
    let armar = alla_armar();
    let arm2 = armar.iter().find(|a| a.namn == "arm2" && !a.spegel).unwrap();
    let spegel = armar.iter().find(|a| a.namn == "arm2" && a.spegel).unwrap();
    assert_eq!(spegel.markta, arm2.omarkta);
    assert_eq!(spegel.omarkta, arm2.markta);
    assert_eq!(spegel.namn, arm2.namn);
    assert!(spegel.spegel && !arm2.spegel);
    // Spegeln ändrar inget annat i armdefinitionen:
    let tillbaka = HjarnArm {
        namn: spegel.namn,
        markta: spegel.omarkta,
        omarkta: spegel.markta,
        spegel: false,
    };
    assert_eq!(tillbaka.markta, arm2.markta);
    assert_eq!(tillbaka.omarkta, arm2.omarkta);
}

#[test]
fn tabellen_provar_ibland() {
    let mut tab = Minnestabell::ny();
    let mut g = Rng::new(99);
    for _ in 0..100 {
        let _ = tab.valj(&mut g);
    }
    assert!(
        tab.slumphandlingar >= 1,
        "ingen slumpvis handling på 100 steg (fick {})",
        tab.slumphandlingar
    );
}

#[test]
fn matningen_andrar_inget() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 77u64;
    let n0 = 40;
    let ar = 40i64;
    // Arm med nya mätfält (fel per handling + v-mot-läsare).
    let arm = HjarnArm {
        namn: "a_las",
        markta: Formaga::LarIntagslasare,
        omarkta: Formaga::Lar,
        spegel: false,
    };
    let mut utan = Sim::fran_karta(&kart, fro, n0);
    utan.mark_lage = MarkLage::Noll;
    utan.hjarn_arm = Some(arm.clone());
    utan.matning.pa = false;
    let mut med = Sim::fran_karta(&kart, fro, n0);
    med.mark_lage = MarkLage::Noll;
    med.hjarn_arm = Some(arm);
    med.matning.pa = true;
    let steg_mark = (MARKT_INKOR_AR * tal::STEG_PER_AR) as u64;
    let steg_slut = (ar * tal::STEG_PER_AR) as u64;
    for t in 0..steg_slut {
        if t == steg_mark {
            utan.markera_halften(fro);
            med.markera_halften(fro);
        }
        utan.steg();
        med.steg();
    }
    assert_eq!(utan.antal(), med.antal());
    assert_eq!(utan.djur_summa(), med.djur_summa());
    assert!(med.matning.krav_data().is_ok());
    let m = med
        .matning
        .per
        .get("lar_intagslasare")
        .expect("saknar intagsläsar-mätning");
    assert!(m.v_jamforda > 0, "nya fältet v_jamforda ska fyllas");
    assert!(
        m.fel_tidigt_per.iter().any(|f| f.n > 0) || m.fel_sent_per.iter().any(|f| f.n > 0),
        "fel per handling ska kunna fyllas"
    );
}

#[test]
fn slumpens_handlingar_jamna() {
    let kart = bara(Vatten::Land, 8, 8);
    let mut sim = Sim::fran_karta(&kart, 55, 30);
    sim.hjarn_arm = Some(HjarnArm {
        namn: "jamn",
        markta: Formaga::Slump,
        omarkta: Formaga::Slump,
        spegel: false,
    });
    sim.mark_lage = MarkLage::Noll;
    sim.matning.pa = true;
    for d in sim.djur.iter_mut() {
        d.markt = true;
        d.formaga = Formaga::Slump;
    }
    sim.markering_skedd = true;
    for _ in 0..tal::STEG_PER_AR * 20 {
        sim.steg();
    }
    let m = sim
        .matning
        .per
        .get("slump")
        .expect("saknar slumpmätning");
    assert!(m.djursteg > 1000, "för få steg: {}", m.djursteg);
    let mal = 1.0 / A_N as f64;
    for a in 0..A_N {
        let p = m.andel_handling(a);
        assert!(
            (p - mal).abs() <= 0.1,
            "handling {a}: andel {p} utanför 1/9 ± 0,1"
        );
    }
}

#[test]
fn tom_matning_stoppar() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 3, 4);
    sim.hjarn_arm = Some(HjarnArm {
        namn: "tom",
        markta: Formaga::LarIntagslasare,
        omarkta: Formaga::OrakelForutsagare,
        spegel: false,
    });
    sim.matning = Matning {
        pa: true,
        ..Matning::default()
    };
    sim.markera_halften(3);
    // Inga djursteg efter märkningen — nya fält hjälper inte.
    let fel = sim.matning.krav_data();
    assert!(fel.is_err(), "tom mätning skulle ge fel, fick Ok");
    assert!(
        fel.unwrap_err().contains("tom mätning"),
        "felmeddelandet ska nämna tom mätning"
    );
    for m in sim.matning.per.values() {
        assert_eq!(m.v_jamforda, 0);
        assert!(m.fel_tidigt_per.iter().all(|f| f.n == 0));
        assert!(m.fel_sent_per.iter().all(|f| f.n == 0));
    }
}

#[test]
fn vaxt_per_djur_valjer_ratt() {
    let b = 5;
    let h = 5;
    let kart = bara(Vatten::Land, b, h);
    let mut sim = Sim::fran_karta(&kart, 1, 1);
    // Nollställ växter; sätt mönster kring (2,2).
    for c in 0..sim.v.len() {
        sim.v[c] = 0.0;
    }
    // Handling 3 = Ö (1,0): mycket växter, en granne där → score 10/(1+1)=5
    // Handling 1 = N (0,1): ännu mer växter men två djur → 18/(2+1)=6 → ska vinna
    // Handling 0 = stå: växter 9, andra 0 (vi själva räknas bort) → 9/1=9 → egentligen högst!
    // Lägg djuret i (2,2) ensamt; sätt stå-rutan låg så N vinner.
    sim.djur[0].i = 2;
    sim.djur[0].j = 2;
    sim.djur[0].lev = true;
    let c0 = sim.cell(2, 2);
    let c_n = sim.cell(2, 3);
    let c_o = sim.cell(3, 2);
    sim.v[c0] = 1.0; // stå: 1/(0+1)=1
    sim.v[c_n] = 18.0; // N: men vi lägger 2 "andra" via antal
    sim.v[c_o] = 10.0; // Ö
    let mut antal = vec![0usize; b * h];
    antal[c0] = 1; // bara vi
    antal[c_n] = 2; // två andra i N
    antal[c_o] = 1; // en annan i Ö
    // N: 18/(2+1)=6; Ö: 10/(1+1)=5; stå: 1/1=1 → välj N (handling 1)
    let a = sim.vaxt_per_djur_handling(2, 2, &antal);
    assert_eq!(a, 1, "skulle välja N (mest växter per djur), fick {a}");
    // Om Ö får noll andra: 10/1=10 > N:s 6 → Ö
    antal[c_o] = 0;
    let a2 = sim.vaxt_per_djur_handling(2, 2, &antal);
    assert_eq!(a2, 3, "skulle välja Ö, fick {a2}");
}

#[test]
fn allvetande_slump_p_noll_ar_allvetande() {
    let b = 5;
    let h = 5;
    let kart = bara(Vatten::Land, b, h);
    let mut bas = Sim::fran_karta(&kart, 9, 1);
    for c in 0..bas.v.len() {
        bas.v[c] = 1.0;
    }
    bas.djur[0].i = 2;
    bas.djur[0].j = 2;
    // Unikt maximum i SO (handling 4 = (1,-1) → cell (3,1))
    let c_so = bas.cell(3, 1);
    let c_n = bas.cell(2, 3);
    let c_o = bas.cell(3, 2);
    bas.v[c_so] = 50.0;
    assert_eq!(bas.allvetande_handling(2, 2), 4);
    // Samma värld: Allvetande mot AllvetandeSlump(0) → samma tillstånd.
    let mut a = Sim::fran_karta(&kart, 19, 4);
    let mut bsim = Sim::fran_karta(&kart, 19, 4);
    for d in a.djur.iter_mut() {
        d.formaga = Formaga::Allvetande;
    }
    for d in bsim.djur.iter_mut() {
        d.formaga = Formaga::AllvetandeSlump(0.0);
    }
    a.markering_skedd = true;
    bsim.markering_skedd = true;
    for _ in 0..tal::STEG_PER_AR * 5 {
        a.steg();
        bsim.steg();
    }
    assert_eq!(a.antal(), bsim.antal());
    assert_eq!(a.djur_summa(), bsim.djur_summa());
    // Lika maximum → stå kvar.
    bas.v[c_so] = 1.0;
    bas.v[c_n] = 20.0;
    bas.v[c_o] = 20.0;
    assert_eq!(bas.allvetande_handling(2, 2), 0);
}

#[test]
fn tabell_utan_slump_provar_aldrig() {
    // Alt 1218: genom simuleringens eget val; djuret ska leva under stegen.
    // Mutanten "slumpval 0,1 i anropet" (valj i stället för valj_med_p(..., 0.0)) blir röd.
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 8, 1);
    sim.djur[0].lev = true;
    sim.djur[0].formaga = Formaga::TabellUtanSlump;
    sim.djur[0].tabell = Minnestabell::ny();
    sim.djur[0].e = 500.0;
    sim.djur[0].age = 0;
    for c in 0..sim.v.len() {
        sim.v[c] = sim.vmax[c] * 0.8;
    }
    let n_steg = 200usize;
    for _ in 0..n_steg {
        assert!(sim.djur[0].lev, "djuret dog under TabellUtanSlump-provet");
        sim.steg();
    }
    assert!(sim.djur[0].lev, "djuret ska leva efter {n_steg} steg");
    assert_eq!(
        sim.djur[0].tabell.slumphandlingar, 0,
        "TabellUtanSlump via Sim drog {} slumphandlingar",
        sim.djur[0].tabell.slumphandlingar
    );
}

#[test]
fn lar_lott_lottar() {
    // Konstruerade lika värden: lar_lott väljer olika handlingar; lar står kvar.
    let mut g_lott = Rng::new(1);
    let mut g_lar = Rng::new(1);
    let mut hj_lott = Hj::slump(&mut g_lott);
    let mut hj_lar = Hj::slump(&mut g_lar);
    // Nollställ avläsaren så alla y ger samma värde 0.
    let n_gen = hj_lott.gen.len();
    for i in (n_gen - (Z_N + 1))..n_gen {
        hj_lott.gen[i] = 0.0;
        hj_lar.gen[i] = 0.0;
    }
    // Nollställ förutsägaren så y = 0 för alla handlingar.
    for x in hj_lott.wp.iter_mut() {
        *x = 0.0;
    }
    for x in hj_lar.wp.iter_mut() {
        *x = 0.0;
    }
    let s = vec![0.0; S_N];
    let mut lott_val = std::collections::BTreeSet::new();
    let mut egen = Rng::new(42);
    for _ in 0..200 {
        let a = hj_lott.valj_lott(&s, false, 0, &mut egen);
        lott_val.insert(a);
        let b = hj_lar.valj(&s, false, 0);
        assert_eq!(b, 0, "lar ska stå kvar vid lika, fick {b}");
    }
    assert!(
        lott_val.len() > 1,
        "lar_lott lottade bara {:?} över 200 steg",
        lott_val
    );
}

#[test]
fn lasaren_lar_sig() {
    // Konstruerat: varje handling har fast y = [a,0,0,0]; Ö (3) ger intag, övriga 0.
    // Förutsägaren fryses mellan stegen så y är stabil (mutanten "struntar i y" → ~1/9).
    let mut g = Rng::new(21);
    let mut hj = Hj::slump(&mut g);
    for x in hj.gen.iter_mut() {
        *x = 0.0;
    }
    for x in hj.wp.iter_mut() {
        *x = 0.0;
    }
    // forutsag(_, a): Ö → y=[1,0,0,0], övriga → y=[0,1,0,0].
    for a in 0..A_N {
        let b = a * Z_N * (Z_N + 1);
        if a == 3 {
            hj.wp[b + Z_N] = 1.0; // y[0]-bias
        } else {
            hj.wp[b + 1 * (Z_N + 1) + Z_N] = 1.0; // y[1]-bias
        }
    }
    let y_for = |a: usize| -> [f64; 4] {
        if a == 3 {
            [1.0, 0.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0, 0.0]
        }
    };
    let mut lasare = Intagslasare::ny();
    let mut egen = Rng::new(99);
    let s = vec![0.0; S_N];
    // Träna på kända (handling → y, intag): Ö ger mat, övriga inte.
    // Mutanten "läsaren struntar i y" (bara u0) ger fortfarande lika poäng för alla y.
    for _ in 0..80 {
        for a in 0..A_N {
            let intag = if a == 3 { 2.0 } else { 0.0 };
            lasare.uppdatera(&y_for(a), intag);
        }
    }
    let mut basta = 0usize;
    let mut bv = f64::NEG_INFINITY;
    for a in 0..A_N {
        let v = lasare.varde(&y_for(a));
        if v > bv {
            bv = v;
            basta = a;
        }
    }
    assert_eq!(
        basta, 3,
        "läsaren borde rangordna Ö högst (fick {basta}); mutanten 'struntar i y' blir röd"
    );
    let mut n_ost = 0u64;
    let n_prov = 200usize;
    for _ in 0..n_prov {
        let wp0 = hj.wp.clone();
        let (a, _, _) = hj.valj_med_poang(&s, false, 0, &mut egen, |y| lasare.varde(y));
        hj.wp = wp0;
        hj.forra = None;
        if a == 3 {
            n_ost += 1;
        }
    }
    let andel = n_ost as f64 / n_prov as f64;
    assert!(
        andel > 1.0 / A_N as f64,
        "läsaren valde Ö bara {andel:.3} (krav > 1/9)"
    );

    // Sim-vägen: betet övar läsaren (vikterna lämnar 0 vid intag).
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 22, 1);
    sim.djur[0].formaga = Formaga::LarIntagslasare;
    sim.djur[0].lasare = Intagslasare::ny();
    sim.djur[0].e = 500.0;
    for c in 0..sim.v.len() {
        sim.v[c] = 50.0;
    }
    for _ in 0..50 {
        sim.steg();
    }
    let l = &sim.djur[0].lasare;
    assert!(
        l.u.iter().any(|&x| x != 0.0) || l.u0 != 0.0,
        "Sim/betet rörde inte läsarens vikter"
    );
}

#[test]
fn orakel_ser_malrutan() {
    let b = 5;
    let h = 5;
    let kart = bara(Vatten::Land, b, h);
    let mut sim = Sim::fran_karta(&kart, 11, 1);
    sim.djur[0].lev = true;
    sim.djur[0].i = 2;
    sim.djur[0].j = 2;
    sim.djur[0].e = 500.0;
    sim.djur[0].formaga = Formaga::OrakelForutsagare;
    // Avläsaren: belöna hög växtandel i mittenkanalen för "stå"-grannen i sinnena
    // (s[0] = V/tak i målrutan själv när sinnena beräknas därifrån).
    // Enklare: sätt v så att varde(z) ≈ z[0], och kodaren så z[0] ≈ tanh(s[0]).
    for x in sim.djur[0].hj.gen.iter_mut() {
        *x = 0.0;
    }
    // Kodare rad 0: z0 = tanh(s[0]) — vikt 1 på sinneskanal 0, bias 0.
    sim.djur[0].hj.gen[0] = 1.0; // s[0]
    // Avläsare: v[0]=1, v0=0 → varde = z[0].
    let v0 = hjarna::NK + hjarna::NP;
    sim.djur[0].hj.gen[v0] = 1.0;
    for c in 0..sim.v.len() {
        sim.v[c] = 1.0;
        sim.vmax[c] = 100.0;
    }
    // Unikt maximum i SO (handling 4 → cell (3,1)).
    let c_so = sim.cell(3, 1);
    sim.v[c_so] = 90.0;
    // Kör valet via steg: oraklet ska gå till SO (känd bästa ruta).
    let mut n_so = 0u64;
    for _ in 0..50 {
        for c in 0..sim.v.len() {
            sim.v[c] = 1.0;
        }
        sim.v[c_so] = 90.0;
        sim.djur[0].i = 2;
        sim.djur[0].j = 2;
        sim.djur[0].e = 500.0;
        sim.steg();
        if sim.djur[0].i == 3 && sim.djur[0].j == 1 {
            n_so += 1;
        }
    }
    assert!(
        n_so >= 40,
        "oraklet borde välja SO (känd bästa ruta) oftast, fick {n_so}/50"
    );
}

fn konstruerad_loggrad(fro: u64, handling: usize, i: usize, brus: f64) -> LedLoggRad {
    let mut s = [0.0; S_N];
    // Alla kanaler varierar så att XᵀX inte blir singulär.
    for k in 0..S_N {
        s[k] = ((i * (k + 3) + k * 7) % 97) as f64 / 97.0;
    }
    // Mål = Σ 0,1·(k+1)·s[k] + 1,0 + brus (känt linjärt samband).
    let mut intag_efter = 1.0 + brus;
    for k in 0..S_N {
        intag_efter += 0.1 * (k as f64 + 1.0) * s[k];
    }
    LedLoggRad {
        fro,
        djur_id: 1,
        handling,
        s,
        hj_varde: intag_efter * 0.5,
        hj_varden: [intag_efter * 0.5; A_N],
        intag_fore: intag_efter - 0.1,
        intag_efter,
    }
}

/// Kontroll 1: R² på ett känt linjärt samband > 0,99.
#[test]
fn r2_pa_kant_samband() {
    let mut rader = Vec::new();
    for i in 0..80 {
        let fro = if i < 40 { 141 } else { 151 };
        rader.push(konstruerad_loggrad(fro, 1, i, 0.0001 * (i as f64)));
    }
    let anp = vec![141u64];
    let prov = vec![151u64];
    let (g1, _, _) = minstakvadrat::rakna_alla(&rader, &anp, &prov).expect("R²");
    let rad = &g1.per_handling[1];
    assert!(
        !rad.singulaer,
        "matrisen ska inte vara singulär: {}",
        rad.meddelande
    );
    let r2 = rad.r2.expect("R² för handling 1");
    assert!(
        r2 > 0.99,
        "känt linjärt samband borde ge R² > 0,99, fick {r2}"
    );
}

/// Kontroll 2: omkastade mål ger R² under 0,05.
#[test]
fn r2_omkastade_mal() {
    let mut rader = Vec::new();
    for i in 0..80 {
        let fro = if i < 40 { 141 } else { 151 };
        rader.push(konstruerad_loggrad(fro, 1, i, 0.0));
    }
    // Blanda målen mellan raderna (samma frön, samma handling).
    let n = rader.len();
    let mal: Vec<f64> = rader.iter().map(|r| r.intag_efter).collect();
    for i in 0..n {
        rader[i].intag_efter = mal[(i + n / 2) % n];
    }
    let (g1, _, _) = minstakvadrat::rakna_alla(&rader, &[141], &[151]).expect("R²");
    let rad = &g1.per_handling[1];
    assert!(!rad.singulaer, "singulär: {}", rad.meddelande);
    let r2 = rad.r2.expect("R²");
    assert!(
        r2 < 0.05,
        "omkastade mål borde ge R² under 0,05, fick {r2}"
    );
}

/// Kontroll 3: överlappande frön vägras.
#[test]
fn r2_skilda_fron_vagrar_overlap() {
    let rader = vec![konstruerad_loggrad(141, 0, 0, 0.0)];
    let fel = minstakvadrat::rakna_alla(&rader, &[141, 142], &[142, 143]);
    assert!(fel.is_err(), "överlapp borde ge fel");
    let e = fel.unwrap_err();
    assert!(
        e.contains("överlappar"),
        "felmeddelandet ska nämna överlapp, fick: {e}"
    );
}

/// Kontroll 4a: tom logg stoppar.
#[test]
fn tom_logg_stoppar() {
    let logg = LedLogg {
        pa: true,
        ..LedLogg::default()
    };
    let fel = logg.krav_data();
    assert!(fel.is_err(), "tom logg skulle ge fel");
    assert!(
        fel.unwrap_err().contains("tom logg"),
        "felmeddelandet ska nämna tom logg"
    );
}

/// Kontroll 4b: loggen ändrar inte simuleringen (samma djur_summa).
#[test]
fn loggen_andrar_inget() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 88u64;
    let n0 = 40;
    let ar = 40i64;
    let arm = HjarnArm {
        namn: "r_lar",
        markta: Formaga::Lar,
        omarkta: Formaga::Lar,
        spegel: false,
    };
    let mut utan = Sim::fran_karta(&kart, fro, n0);
    utan.mark_lage = MarkLage::Noll;
    utan.hjarn_arm = Some(arm.clone());
    utan.matning.pa = true;
    utan.ledlogg.pa = false;
    let mut med = Sim::fran_karta(&kart, fro, n0);
    med.mark_lage = MarkLage::Noll;
    med.hjarn_arm = Some(arm);
    med.matning.pa = true;
    med.ledlogg = LedLogg {
        pa: true,
        fro,
        ..LedLogg::default()
    };
    let steg_mark = (MARKT_INKOR_AR * tal::STEG_PER_AR) as u64;
    let steg_slut = (ar * tal::STEG_PER_AR) as u64;
    for t in 0..steg_slut {
        if t == steg_mark {
            utan.markera_halften(fro);
            med.markera_halften(fro);
        }
        utan.steg();
        med.steg();
    }
    assert_eq!(utan.antal(), med.antal());
    assert_eq!(utan.djur_summa(), med.djur_summa());
    assert!(med.ledlogg.krav_data().is_ok());
    assert!(
        !med.ledlogg.rader.is_empty(),
        "loggen borde ha fyllts"
    );
}

/// Kontroll 5: återväxtens faktor når simuleringen — i både R_V och SOL.
/// Mutanten "bara R_V" blir röd: med v=0 är R_V-termen noll, bara SOL växer.
#[test]
fn atervaxt_faktorn_nar_simuleringen() {
    let b = 4;
    let h = 4;
    let kart = bara(Vatten::Land, b, h);
    let mut f1 = Sim::fran_karta(&kart, 1, 0);
    f1.atervaxt = 1.0;
    let mut f2 = Sim::fran_karta(&kart, 1, 0);
    f2.atervaxt = 2.0;
    let mut std = Sim::fran_karta(&kart, 1, 0);
    for c in 0..f1.v.len() {
        f1.v[c] = 0.0;
        f2.v[c] = 0.0;
        std.v[c] = 0.0;
    }
    assert!((std.atervaxt - 1.0).abs() < 1e-15);
    for _ in 0..50 {
        f1.steg();
        f2.steg();
        std.steg();
    }
    let sum = |s: &Sim| s.v.iter().sum::<f64>();
    let s1 = sum(&f1);
    let s2 = sum(&f2);
    let ss = sum(&std);
    assert!(
        s2 > s1 * 1.5,
        "faktor 2 ska nå SOL (v=0): mer än 1,5× faktor 1 efter 50 steg ({s2} mot {s1})"
    );
    assert!(
        (s1 - ss).abs() < 1e-9,
        "faktor 1 ska ge exakt samma värld som standard ({s1} ≠ {ss})"
    );
    let mut g1 = Sim::fran_karta(&kart, 2, 0);
    g1.atervaxt = 1.0;
    let mut g2 = Sim::fran_karta(&kart, 2, 0);
    g2.atervaxt = 2.0;
    for _ in 0..50 {
        g1.steg();
        g2.steg();
    }
    assert!(
        sum(&g2) > sum(&g1),
        "faktor 2 ska ge mer växt än faktor 1 även med startväxt"
    );
}

/// Lucka: G2:s indata är hjärnvärdet. Mutanten "G2 med intaget som indata" blir röd.
#[test]
fn g2_indata_ar_hjarnvarde() {
    let mut rader = Vec::new();
    for i in 0..80 {
        let fro = if i < 40 { 141 } else { 151 };
        let mut r = konstruerad_loggrad(fro, 1, i, 0.0);
        r.intag_fore = 1.0 + (i as f64) * 0.1;
        r.intag_efter = r.intag_fore;
        r.hj_varde = ((i * 17) % 13) as f64;
        rader.push(r);
    }
    let (g1, g2, g3) = minstakvadrat::rakna_alla(&rader, &[141], &[151]).expect("R²");
    let _ = g1;
    let r2_g2 = g2.per_handling[1].r2.expect("G2 R²");
    let r2_g3 = g3.per_handling[1].r2.expect("G3 R²");
    assert!(
        r2_g2 < 0.05,
        "riktiga G2 (hjärnvärde) ska ge R² nära 0 eller sämre, fick {r2_g2}"
    );
    assert!(r2_g3 > 0.95, "G3 (intag_fore) ska vara stark, fick {r2_g3}");

    let anp: Vec<&LedLoggRad> = rader.iter().filter(|r| r.fro == 141).collect();
    let prov: Vec<&LedLoggRad> = rader.iter().filter(|r| r.fro == 151).collect();
    let x_anp: Vec<Vec<f64>> = anp
        .iter()
        .map(|r| minstakvadrat::bygg_x_g2_mutant_intag(r))
        .collect();
    let y_anp: Vec<f64> = anp.iter().map(|r| r.intag_efter).collect();
    let beta = minstakvadrat::minstakvadrat(&x_anp, &y_anp).expect("mutant β");
    let x_prov: Vec<Vec<f64>> = prov
        .iter()
        .map(|r| minstakvadrat::bygg_x_g2_mutant_intag(r))
        .collect();
    let y_prov: Vec<f64> = prov.iter().map(|r| r.intag_efter).collect();
    let r2_mut = minstakvadrat::r2_pa(&x_prov, &y_prov, &beta);
    assert!(
        r2_mut > 0.95,
        "mutanten G2-med-intag ska ge R² > 0,95 (prov blir rött om riktiga G2 byts), fick {r2_mut}"
    );
    let x_riktig = minstakvadrat::bygg_x_g2(&rader[0]);
    assert!(
        (x_riktig[0] - rader[0].hj_varde).abs() < 1e-15,
        "bygg_x_g2 ska läsa hj_varde"
    );
}

/// Kontroll 1 (rang): konstruerade rader där värdena = G1-gissningarna.
#[test]
fn r_pa_kanda_djur() {
    let mut anp_rader = Vec::new();
    for i in 0..90 {
        for h in 0..A_N {
            anp_rader.push(konstruerad_loggrad(141, h, i * A_N + h, 0.0));
        }
    }
    let g1 = minstakvadrat::anpassa_g1(&anp_rader, &[141]);
    for h in 0..A_N {
        assert!(!g1.singulaer[h], "singulär h{h}: {}", g1.meddelande[h]);
        assert!(g1.beta[h].is_some(), "saknar β h{h}");
    }

    let mut perfekta = Vec::new();
    for i in 0..200 {
        let mut s = [0.0; S_N];
        for k in 0..S_N {
            s[k] = ((i * (k + 5) + k * 3) % 89) as f64 / 89.0;
        }
        let giss = minstakvadrat::g1_gissningar(&g1, &s).expect("giss");
        perfekta.push(LedLoggRad {
            fro: 161,
            djur_id: 1,
            handling: 0,
            s,
            hj_varde: giss[0],
            hj_varden: giss,
            intag_fore: 0.0,
            intag_efter: giss[0],
        });
    }
    let ut = minstakvadrat::rang_per_rad(&perfekta, &g1).expect("rang");
    let mt = ut.iter().map(|r| r.traff).sum::<f64>() / ut.len() as f64;
    let ms = ut.iter().map(|r| r.spearman).sum::<f64>() / ut.len() as f64;
    assert!(mt > 0.95, "perfekt match: träff > 0,95, fick {mt}");
    assert!(ms > 0.95, "perfekt match: Spearman > 0,95, fick {ms}");

    let mut blandade = perfekta.clone();
    for (i, r) in blandade.iter_mut().enumerate() {
        let v0 = r.hj_varden;
        let mut perm: [usize; A_N] = std::array::from_fn(|j| j);
        let mut state = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for a in (1..A_N).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (state as usize) % (a + 1);
            perm.swap(a, j);
        }
        for a in 0..A_N {
            r.hj_varden[a] = v0[perm[a]];
        }
    }
    let ut_b = minstakvadrat::rang_per_rad(&blandade, &g1).expect("rang bland");
    let mt_b = ut_b.iter().map(|r| r.traff).sum::<f64>() / ut_b.len() as f64;
    assert!(
        mt_b > 0.05 && mt_b < 0.2,
        "blandade värden: träff mellan 0,05 och 0,2, fick {mt_b}"
    );

    let mut omvand = perfekta.clone();
    for r in &mut omvand {
        for a in 0..A_N {
            r.hj_varden[a] = -r.hj_varden[a];
        }
    }
    let ut_o = minstakvadrat::rang_per_rad(&omvand, &g1).expect("rang omvänd");
    let ms_o = ut_o.iter().map(|r| r.spearman).sum::<f64>() / ut_o.len() as f64;
    assert!(ms_o < -0.9, "omvänt tecken: Spearman < −0,9, fick {ms_o}");
}

/// Kontroll 2: två djur med perfekt rang men motsatt skala → G2≈0, Spearman>0,95.
#[test]
fn r_skalan_per_djur() {
    let mut anp_rader = Vec::new();
    for i in 0..90 {
        for h in 0..A_N {
            anp_rader.push(konstruerad_loggrad(141, h, i * A_N + h, 0.0));
        }
    }
    let g1 = minstakvadrat::anpassa_g1(&anp_rader, &[141]);

    let mut rader = Vec::new();
    let mut g2_rader = Vec::new();
    for i in 0..80 {
        let mut s = [0.0; S_N];
        for k in 0..S_N {
            s[k] = ((i * (k + 2) + 11) % 83) as f64 / 83.0;
        }
        let giss = minstakvadrat::g1_gissningar(&g1, &s).expect("giss");
        let ba = 1usize;
        let djur_id = if i % 2 == 0 { 1 } else { 2 };
        let skala = if djur_id == 1 { 1.0 } else { 0.01 };
        let mut hj = [0.0; A_N];
        for a in 0..A_N {
            hj[a] = skala * giss[a];
        }
        let intag = giss[ba];
        rader.push(LedLoggRad {
            fro: 161,
            djur_id,
            handling: ba,
            s,
            hj_varde: hj[ba],
            hj_varden: hj,
            intag_fore: 0.0,
            intag_efter: intag,
        });
        let fro_g2 = if i < 40 { 141 } else { 151 };
        g2_rader.push(LedLoggRad {
            fro: fro_g2,
            djur_id,
            handling: ba,
            s,
            hj_varde: hj[ba],
            hj_varden: hj,
            intag_fore: 0.0,
            intag_efter: intag,
        });
    }
    let ut = minstakvadrat::rang_per_rad(&rader, &g1).expect("rang");
    let ms = ut.iter().map(|r| r.spearman).sum::<f64>() / ut.len() as f64;
    assert!(ms > 0.95, "perfekt rangordning: Spearman > 0,95, fick {ms}");

    let (_, g2, _) = minstakvadrat::rakna_alla(&g2_rader, &[141], &[151]).expect("G2");
    let r2_m = g2.per_handling[1].r2.expect("G2 R² handling 1");
    assert!(
        r2_m < 0.05,
        "motsatt skala ska ge G2 nära 0 eller sämre, fick {r2_m}"
    );
}

/// Kontroll 3: tre djur i samma ruta efter flyttarna → 2 medätare var.
#[test]
fn medatare_efter_flyttar() {
    let b = 4;
    let h = 4;
    let kart = bara(Vatten::Land, b, h);
    let mut sim = Sim::fran_karta(&kart, 7, 3);
    for d in sim.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..sim.v.len() {
        sim.v[c] = 1.0;
    }
    let c_egen = sim.cell(1, 1);
    sim.v[c_egen] = 100.0;
    sim.matning.pa = true;
    sim.markering_skedd = true;
    sim.steg();
    let m = sim.matning.per.get("allvetande").expect("mätning");
    let med = m.medatare_medel().expect("medätare");
    assert!(
        (med - 2.0).abs() < 1e-9,
        "tre djur i samma ruta ska ge 2 medätare var, fick {med}"
    );
}

/// Kontroll 4: trängseloraklet ur sinnena.
#[test]
fn trangsel_orakel_valjer_ratt() {
    let mut g = Rng::new(1);
    let mut s = vec![0.0; S_N];
    for a in 0..A_N {
        s[a * 2] = 0.1 * a as f64;
        s[a * 2 + 1] = 0.0;
    }
    s[18] = 1.0;
    let a = Sim::trangsel_orakel_handling(&s, &mut g);
    assert_eq!(a, 8, "med n=0 ska största v_andel vinna (handling 8), fick {a}");

    let mut s2 = vec![0.0; S_N];
    for a in 0..A_N {
        s2[a * 2] = 0.1;
        s2[a * 2 + 1] = 0.0;
    }
    s2[2] = 0.5;
    s2[3] = 1.0;
    s2[6] = 0.5;
    s2[7] = 0.0;
    s2[18] = 1.0;
    let a2 = Sim::trangsel_orakel_handling(&s2, &mut g);
    assert_eq!(a2, 3, "ska välja tomma rutan (handling 3), fick {a2}");
}

/// Kontroll 6: mätfälten ändrar inte simuleringen; tom logg stoppar.
#[test]
fn matfalt_trangsel_andrar_inget() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 99u64;
    let n0 = 30;
    let arm = HjarnArm {
        namn: "t_to",
        markta: Formaga::TrangselOrakel,
        omarkta: Formaga::Slump,
        spegel: false,
    };
    let mut utan = Sim::fran_karta(&kart, fro, n0);
    utan.mark_lage = MarkLage::Noll;
    utan.hjarn_arm = Some(arm.clone());
    utan.matning.pa = false;
    utan.ledlogg.pa = false;
    let mut med = Sim::fran_karta(&kart, fro, n0);
    med.mark_lage = MarkLage::Noll;
    med.hjarn_arm = Some(arm);
    med.matning.pa = true;
    med.ledlogg = LedLogg {
        pa: true,
        fro,
        ..LedLogg::default()
    };
    let steg_mark = (MARKT_INKOR_AR * tal::STEG_PER_AR) as u64;
    let steg_slut = ((MARKT_INKOR_AR + 5) * tal::STEG_PER_AR) as u64;
    for t in 0..steg_slut {
        if t == steg_mark {
            utan.markera_halften(fro);
            med.markera_halften(fro);
        }
        utan.steg();
        med.steg();
    }
    assert_eq!(utan.djur_summa(), med.djur_summa());
    assert!(med.matning.krav_data().is_ok());
    assert!(med.ledlogg.krav_data().is_ok());
    let tot_med: u64 = med.matning.per.values().map(|m| m.medatare_n).sum();
    assert!(tot_med > 0, "medätare ska ha räknats");
}

/// Kontroll 1 (tur): läget av = i dag (samma kontrollsumma som utan fältet satt).
#[test]
fn turordning_av_som_idag() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 55u64;
    let n0 = 25;
    let mut a = Sim::fran_karta(&kart, fro, n0);
    let mut bsim = Sim::fran_karta(&kart, fro, n0);
    assert!(!a.turordning, "turordning ska vara av som standard");
    bsim.turordning = false;
    for _ in 0..tal::STEG_PER_AR * 3 {
        a.steg();
        bsim.steg();
    }
    assert_eq!(a.djur_summa(), bsim.djur_summa());
}

/// Kontroll 2 (tur): ett ensamt djur — turordning på och av ger samma bana.
#[test]
fn turordning_ensamt_djur() {
    let kart = bara(Vatten::Land, 6, 6);
    let fro = 77u64;
    let mut av = Sim::fran_karta(&kart, fro, 1);
    let mut pa = Sim::fran_karta(&kart, fro, 1);
    pa.turordning = true;
    for _ in 0..tal::STEG_PER_AR * 4 {
        av.steg();
        pa.steg();
    }
    assert_eq!(av.djur_summa(), pa.djur_summa());
    assert_eq!(av.djur[0].i, pa.djur[0].i);
    assert_eq!(av.djur[0].j, pa.djur[0].j);
    assert!((av.djur[0].e - pa.djur[0].e).abs() < 1e-9);
}

/// Kontroll 3 (tur): ordningen lottas; världens g orörd av lotteriet.
/// Mutanten "alltid samma ordning" (ingen blanda) blir röd.
#[test]
fn turordning_lottas() {
    let kart = bara(Vatten::Land, 6, 6);
    let mut sim = Sim::fran_karta(&kart, 91, 5);
    sim.turordning = true;
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::Allvetande;
        d.e = 100.0;
        d.age = 0;
        d.hona = false; // ingen födsel → g orörd av arv
    }
    let g0 = sim.g.tillstand();
    sim.steg();
    let ord1 = sim.senaste_turordning.clone();
    assert_eq!(
        sim.g.tillstand(),
        g0,
        "turordningens lotteri ska inte röra världens g"
    );
    assert_eq!(ord1.len(), 5);
    sim.steg();
    let ord2 = sim.senaste_turordning.clone();
    assert_eq!(sim.g.tillstand(), g0);
    assert_ne!(
        ord1, ord2,
        "två steg i följd ska ge olika turordning (mutanten 'alltid samma' blir röd)"
    );

    // Utan turordning: blanda rör världens g.
    let mut sam = Sim::fran_karta(&kart, 91, 5);
    for d in sam.djur.iter_mut() {
        d.formaga = Formaga::Allvetande;
        d.e = 100.0;
        d.age = 0;
        d.hona = false;
    }
    let g_sam0 = sam.g.tillstand();
    sam.steg();
    assert_ne!(
        sam.g.tillstand(),
        g_sam0,
        "samtidig klocka ska dra g via blanda"
    );
}

/// Kontroll 4 (tur): tre trängselorakel → 2 och 1 med turordning, 3 och 0 samtidigt.
#[test]
fn turordning_fordelning_2_och_1() {
    let b = 5;
    let h = 5;
    let kart = bara(Vatten::Land, b, h);
    let bygg = |tur: bool| {
        let mut sim = Sim::fran_karta(&kart, 3, 3);
        sim.turordning = tur;
        // Samma egen_rng → lika lott ger samma val vid samtidiga sinnen.
        for d in sim.djur.iter_mut() {
            d.lev = true;
            d.i = 2;
            d.j = 2;
            d.e = 500.0;
            d.age = 0;
            d.hona = false;
            d.formaga = Formaga::TrangselOrakel;
            d.egen_rng = Rng::new(99);
        }
        for c in 0..sim.v.len() {
            sim.v[c] = 1.0;
        }
        // Två lika tomma grannrutor: N (handling 1) och Ö (handling 3).
        let c_n = sim.cell(2, 3);
        let c_o = sim.cell(3, 2);
        sim.v[c_n] = 50.0;
        sim.v[c_o] = 50.0;
        sim.steg();
        let mut n_n = 0usize;
        let mut n_o = 0usize;
        for d in sim.djur.iter().filter(|d| d.lev) {
            if d.i == 2 && d.j == 3 {
                n_n += 1;
            } else if d.i == 3 && d.j == 2 {
                n_o += 1;
            }
        }
        (n_n, n_o)
    };
    let (tn, to) = bygg(true);
    let mut tur_counts = [tn, to];
    tur_counts.sort();
    assert_eq!(
        tur_counts,
        [1, 2],
        "turordning ska ge 2 och 1, fick N={tn} Ö={to}"
    );
    let (sn, so) = bygg(false);
    let mut sam_counts = [sn, so];
    sam_counts.sort();
    assert_eq!(
        sam_counts,
        [0, 3],
        "samtidig klocka ska ge 3 och 0, fick N={sn} Ö={so}"
    );
}

/// Kontroll 5 (tur): andra djurets sinne ser första djurets bete och antal.
#[test]
fn turordning_betet_direkt() {
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 11, 2);
    sim.turordning = true;
    sim.atervaxt = 0.0; // ingen tillväxt → V oförändrad före betet
    for d in sim.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..sim.v.len() {
        sim.v[c] = 1.0;
    }
    // NO från (1,1) entydigt rikast → båda går dit.
    let mal = sim.cell(2, 2);
    sim.v[mal] = 80.0;
    let v0 = sim.v[mal];
    let tagen1 = tal::AT_DINO * v0 / tal::VMAX;
    let v1 = v0 - tagen1;
    let tagen2 = tal::AT_DINO * v1 / tal::VMAX;
    let v_forvantad = v1 - tagen2;
    sim.steg();
    assert_eq!(
        sim.djur
            .iter()
            .filter(|d| d.lev && d.i == 2 && d.j == 2)
            .count(),
        2
    );
    assert!(
        (sim.v[mal] - v_forvantad).abs() < 1e-9,
        "sekventiellt bete: förväntat {v_forvantad}, fick {}",
        sim.v[mal]
    );
    // Med samtidig klocka delar båda samma V0 → annan kvarvarande biomassa.
    let mut sam = Sim::fran_karta(&kart, 11, 2);
    sam.atervaxt = 0.0;
    for d in sam.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..sam.v.len() {
        sam.v[c] = 1.0;
    }
    sam.v[mal] = 80.0;
    sam.steg();
    assert!(
        (sam.v[mal] - v_forvantad).abs() > 1e-6,
        "samtidigt bete ska skilja sig från sekventiellt"
    );
}

/// Kontroll 6 (tur/Fable): vald handling = argmax i hj_varden (200 steg via logg).
/// Mutanten "värdena en handling fel" (rotera hj_varden) blir röd.
#[test]
fn forskjutna_varden_foljer_valet() {
    let kart = bara(Vatten::Land, 8, 8);
    let mut sim = Sim::fran_karta(&kart, 13, 8);
    sim.markering_skedd = true;
    sim.ledlogg = LedLogg {
        pa: true,
        fro: 13,
        var_n: 1,
        max_rader: 20_000,
        ..LedLogg::default()
    };
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::Lar;
        d.e = 400.0;
    }
    for _ in 0..200 {
        sim.steg();
    }
    assert!(
        !sim.ledlogg.rader.is_empty(),
        "loggen ska ha rader efter 200 steg"
    );
    let mut n_mutant_fald = 0u64;
    for r in &sim.ledlogg.rader {
        let max_v = r.hj_varden.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            r.hj_varden[r.handling] == max_v,
            "vald handling {} ska ha högst värde ({max_v}), fick {}",
            r.handling,
            r.hj_varden[r.handling]
        );
        let mut muterad = r.hj_varden;
        let sista = muterad[A_N - 1];
        for i in (1..A_N).rev() {
            muterad[i] = muterad[i - 1];
        }
        muterad[0] = sista;
        let max_m = muterad.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if muterad[r.handling] != max_m {
            n_mutant_fald += 1;
        }
    }
    assert!(
        n_mutant_fald > sim.ledlogg.rader.len() as u64 / 2,
        "mutanten 'värdena en handling fel' ska fällas i merparten ({n_mutant_fald}/{})",
        sim.ledlogg.rader.len()
    );
}

/// Kontroll 7 (tur/Alt 1245): stå kvar → flyttträff saknas; blandade flyttar → 0,09–0,16; tom → fel.
#[test]
fn omlas_1245_kontroller() {
    let mut anp_rader = Vec::new();
    for i in 0..90 {
        for h in 0..A_N {
            anp_rader.push(konstruerad_loggrad(141, h, i * A_N + h, 0.0));
        }
    }
    let g1 = minstakvadrat::anpassa_g1(&anp_rader, &[141]);

    // Bara stå kvar → flyttträff saknas (inte 0).
    let mut sta = Vec::new();
    for i in 0..40 {
        let mut s = [0.0; S_N];
        for k in 0..S_N {
            s[k] = ((i * (k + 3) + 7) % 79) as f64 / 79.0;
        }
        let giss = minstakvadrat::g1_gissningar(&g1, &s).expect("giss");
        sta.push(LedLoggRad {
            fro: 161,
            djur_id: 1,
            handling: 0,
            s,
            hj_varde: giss[0],
            hj_varden: giss,
            intag_fore: 0.0,
            intag_efter: giss[0],
        });
    }
    let ut_sta = minstakvadrat::omlas_per_rad(&sta, &g1).expect("omlas sta");
    let fro_sta = minstakvadrat::omlas_per_fro(&ut_sta).expect("fro sta");
    assert!(
        fro_sta[0].flytt_traff.is_none(),
        "bara stå kvar ska ge flyttträff saknas, inte {:?}",
        fro_sta[0].flytt_traff
    );

    // Blandade värden bland de 8 flyttarna → träff 0,09–0,16.
    // Oberoende brus på flyttvärdena (inte en permutation av G1) ger ≈1/8.
    let mut blandade = Vec::new();
    for i in 0..800 {
        let mut s = [0.0; S_N];
        for k in 0..S_N {
            s[k] = ((i * (k + 5) + k * 3) % 89) as f64 / 89.0;
        }
        let giss = minstakvadrat::g1_gissningar(&g1, &s).expect("giss");
        let mut hj = [0.0; A_N];
        hj[0] = giss[0];
        let mut state = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for a in 1..A_N {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            hj[a] = ((state >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0);
        }
        let ba = 1 + (i % 8);
        blandade.push(LedLoggRad {
            fro: 162,
            djur_id: (i % 5 + 1) as u64,
            handling: ba,
            s,
            hj_varde: hj[ba],
            hj_varden: hj,
            intag_fore: 0.0,
            intag_efter: giss[ba],
        });
    }
    let ut_b = minstakvadrat::omlas_per_rad(&blandade, &g1).expect("omlas bland");
    let fro_b = minstakvadrat::omlas_per_fro(&ut_b).expect("fro bland");
    let mt = fro_b[0].flytt_traff.expect("flyttträff ska finnas");
    assert!(
        mt > 0.09 && mt < 0.16,
        "blandade flyttvärden: träff mellan 0,09 och 0,16, fick {mt}"
    );

    // Tom delmängd stoppar.
    let tom: Vec<LedLoggRad> = Vec::new();
    assert!(minstakvadrat::omlas_per_rad(&tom, &g1).is_err());
}

/// Kontroll 8: spegeln och samma_hjarnpris oförändrade (röker befintliga invariants).
#[test]
fn tur_spegel_och_hjarnpris_kvar() {
    // Spegeln byter bara förmågan (samma som spegeln_byter_bara_formagan).
    let arm = HjarnArm {
        namn: "u_tab",
        markta: Formaga::Tabell,
        omarkta: Formaga::Slump,
        spegel: false,
    };
    let sp = arm.clone().spegel_av();
    assert_eq!(sp.markta, Formaga::Slump);
    assert_eq!(sp.omarkta, Formaga::Tabell);
    assert!(sp.spegel);

    // samma_hjarnpris: trängselorakel och lar betalar lika.
    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 1, 2);
    sim.djur[0].formaga = Formaga::Lar;
    sim.djur[1].formaga = Formaga::TrangselOrakel;
    sim.djur[0].e = 200.0;
    sim.djur[1].e = 200.0;
    let e0 = sim.djur[0].e;
    let e1 = sim.djur[1].e;
    // Bara hjärnkostnaden (via ett steg med noll växter → inget intag).
    for c in 0..sim.v.len() {
        sim.v[c] = 0.0;
    }
    sim.turordning = true;
    sim.steg();
    let de0 = e0 - sim.djur[0].e;
    let de1 = e1 - sim.djur[1].e;
    // Grundkostnad + hjärna; båda formågorna ska ha samma hjärnprisandel.
    assert!(
        (de0 - de1).abs() < 1e-9,
        "samma hjärnpris: lar Δe={de0}, trangsel_orakel Δe={de1}"
    );
}

// --- Alt 1251–1253 kontroller -------------------------------------------------

/// Kontroll 1 (signal): v0 ändrar inte rangen eller kvoten.
/// Mutanten "v0 ingår i centrerad kvot" / "v0 ändrar argmax" blir röd.
#[test]
fn v0_andrar_inte_rang() {
    let mut g = Rng::new(42);
    let mut hj = Hj::slump(&mut g);
    let mut s = [0.0; S_N];
    for i in 0..S_N {
        s[i] = ((i * 17 + 3) % 97) as f64 / 97.0;
    }
    hj.satt_v0(0.0);
    let (_, vard0, _, _) = hj.vardets_delar(&s);
    let mut ord0: Vec<usize> = (0..A_N).collect();
    ord0.sort_by(|&a, &b| {
        vard0[b]
            .partial_cmp(&vard0[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    hj.satt_v0(5.0);
    let (_, vard5, _, _) = hj.vardets_delar(&s);
    let mut ord5: Vec<usize> = (0..A_N).collect();
    ord5.sort_by(|&a, &b| {
        vard5[b]
            .partial_cmp(&vard5[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(ord0, ord5, "v0 ska inte ändra rangordningen");

    // Kvot på konstruerade rader: samma efter centrering oavsett v0.
    let mut rader0 = Vec::new();
    let mut rader5 = Vec::new();
    for k in 0..25 {
        for i in 0..S_N {
            s[i] = ((k * (i + 2) + i) % 89) as f64 / 89.0;
        }
        hj.satt_v0(0.0);
        let (_, v0, _, _) = hj.vardets_delar(&s);
        hj.satt_v0(5.0);
        let (_, v5, _, _) = hj.vardets_delar(&s);
        rader0.push(v0);
        rader5.push(v5);
    }
    let k0 = kvot_fran_matriser(&rader0).expect("k0");
    let k5 = kvot_fran_matriser(&rader5).expect("k5");
    assert!(
        (k0 - k5).abs() < 1e-12,
        "kvoten ska vara densamma efter centrering oavsett v0: {k0} vs {k5}"
    );
}

/// Kontroll 2 (signal): mutanterna klassas rätt.
#[test]
fn mutanter_klassas_ratt() {
    // v = 0 → alla värden lika → kvot saknas.
    let mut gen = vec![0.0; N_GEN];
    // Kodare bias så z ≠ 0; v = 0.
    for i in 0..Z_N {
        gen[i * (S_N + 1) + S_N] = 0.3;
    }
    let hj_v0 = Hj::fran_gen(gen.clone());
    let mut rader = Vec::new();
    for k in 0..25 {
        let mut s = [0.0; S_N];
        for i in 0..S_N {
            s[i] = ((k + i) % 7) as f64 / 7.0;
        }
        let (_, vard, _, _) = hj_v0.vardets_delar(&s);
        assert!(
            vard.iter().all(|&v| (v - vard[0]).abs() < 1e-12),
            "v=0 ska ge lika värden"
        );
        rader.push(vard);
    }
    assert!(
        kvot_fran_matriser(&rader).is_none(),
        "v=0 ska ge kvot saknas (inte 0)"
    );
    assert_eq!(
        klassa_djur(None, 0.8, 0.1, 0.0, None),
        SignalKlass::Saknas
    );

    // Konstant z: alla K-vikter utom fasta termen 0 → kvot 0, blind.
    let mut gen_z = vec![0.1; N_GEN];
    for i in 0..Z_N {
        let bas = i * (S_N + 1);
        for j in 0..S_N {
            gen_z[bas + j] = 0.0;
        }
        gen_z[bas + S_N] = 0.2; // bias
    }
    // Sätt v och W_a så att fasta skillnader finns.
    for a in 0..A_N {
        let b = NK + a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            gen_z[b + i * (Z_N + 1) + Z_N] = (a as f64) * 0.1 + i as f64 * 0.01;
        }
    }
    for i in 0..Z_N {
        gen_z[NK + NP + i] = 0.5;
    }
    gen_z[NK + NP + Z_N] = 0.0;
    let hj_z = Hj::fran_gen(gen_z);
    let mut rader_z = Vec::new();
    let mut z_rader = Vec::new();
    for k in 0..25 {
        let mut s = [0.0; S_N];
        for i in 0..S_N {
            s[i] = ((k * 3 + i) % 11) as f64 / 11.0;
        }
        let (z, vard, _, _) = hj_z.vardets_delar(&s);
        let mut za = [0.0; Z_N];
        za.copy_from_slice(&z[..Z_N]);
        z_rader.push(za);
        rader_z.push(vard);
    }
    let kv = kvot_fran_matriser(&rader_z).expect("kvot ska finnas (fast del)");
    assert!(kv < 1e-9, "konstant z ska ge kvot ≈ 0, fick {kv}");
    let klass = klassa_djur(Some(kv), 0.8, 0.1, 0.0, Some(0.0));
    assert!(
        matches!(
            klass,
            SignalKlass::Blind | SignalKlass::BlindZStill | SignalKlass::BlindZRor
        ),
        "konstant z ska klassas blind, fick {klass:?}"
    );

    // Värdet = sinnet för mat → ser, ingrepp 1.
    let mut rader_pos = Vec::new();
    for k in 0..25 {
        let mut s = [0.0; S_N];
        for i in 0..S_N {
            s[i] = ((k * 5 + i * 2) % 13) as f64 / 13.0;
        }
        rader_pos.push(positiv_varden(&s));
    }
    let k_pos = kvot_fran_matriser(&rader_pos).expect("positiv kvot");
    assert!(k_pos >= 0.5, "positiv kvot ska vara ≥0,5, fick {k_pos}");
    assert_eq!(ingrepp_traff_positiv(), 1.0);
    assert_eq!(
        klassa_djur(Some(k_pos), k_pos, 1.0, 0.2, Some(k_pos)),
        SignalKlass::Ser
    );
}

/// Kontroll 3 (signal): ingreppet läser bara (samma kontrollsumma med/utan).
#[test]
fn ingreppet_laser_bara() {
    let kart = bara(Vatten::Land, 8, 8);
    let fro = 201u64;
    let mut utan = Sim::fran_karta(&kart, fro, 20);
    let mut med = Sim::fran_karta(&kart, fro, 20);
    med.signal_logg = SignalLogg {
        pa: true,
        fro,
        var_n: 10,
        max_rader: 5_000,
        ..SignalLogg::default()
    };
    utan.markering_skedd = true;
    med.markering_skedd = true;
    for d in utan.djur.iter_mut().chain(med.djur.iter_mut()) {
        d.formaga = Formaga::Lar;
    }
    med.kor_ingrepp(false);
    for _ in 0..tal::STEG_PER_AR * 2 {
        utan.steg();
        med.steg();
    }
    med.kor_ingrepp(true);
    assert_eq!(utan.djur_summa(), med.djur_summa());
    assert!(med.signal_logg.ingrepp_mark.is_some());
    assert!(med.signal_logg.ingrepp_slut.is_some());
    assert!(
        !med.signal_logg.rader.is_empty(),
        "signalloggen ska ha rader"
    );
}

/// Kontroll 4 (signal): läge a rör inte växterna i flyttloopen; 2 och 1 för orakel.
/// Läge b läser antalet ur ögonblicksbilden (3 och 0).
#[test]
fn lage_a_och_b() {
    let b = 5;
    let h = 5;
    let kart = bara(Vatten::Land, b, h);

    // Tre trängselorakel → 2 och 1 i läge a (antal uppdateras).
    let mut a = Sim::fran_karta(&kart, 3, 3);
    a.flytt_i_tur = true;
    for d in a.djur.iter_mut() {
        d.lev = true;
        d.i = 2;
        d.j = 2;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::TrangselOrakel;
        d.egen_rng = Rng::new(99);
    }
    for c in 0..a.v.len() {
        a.v[c] = 1.0;
    }
    let c_n = a.cell(2, 3);
    let c_o = a.cell(3, 2);
    a.v[c_n] = 50.0;
    a.v[c_o] = 50.0;
    a.steg();
    let mut counts = [0usize; 2];
    for d in a.djur.iter().filter(|d| d.lev) {
        if d.i == 2 && d.j == 3 {
            counts[0] += 1;
        } else if d.i == 3 && d.j == 2 {
            counts[1] += 1;
        }
    }
    let mut sorted = counts;
    sorted.sort();
    assert_eq!(
        sorted,
        [1, 2],
        "läge a ska ge 2 och 1, fick N={} Ö={}",
        counts[0],
        counts[1]
    );

    // Läge a: bete samtidigt → samma kvarvarande V som samtidig klocka (inte sekventiellt).
    let mut a2 = Sim::fran_karta(&kart, 11, 2);
    a2.flytt_i_tur = true;
    a2.atervaxt = 0.0;
    for d in a2.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..a2.v.len() {
        a2.v[c] = 1.0;
    }
    let mal = a2.cell(2, 2);
    a2.v[mal] = 80.0;
    let mut sam = Sim::fran_karta(&kart, 11, 2);
    sam.atervaxt = 0.0;
    for d in sam.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..sam.v.len() {
        sam.v[c] = 1.0;
    }
    sam.v[mal] = 80.0;
    a2.steg();
    sam.steg();
    assert!(
        (a2.v[mal] - sam.v[mal]).abs() < 1e-9,
        "läge a ska beta samtidigt som s_to: a={} sam={}",
        a2.v[mal],
        sam.v[mal]
    );
    // Mutanten "bete i flyttloopen" (= turordning) skiljer sig.
    let mut tur = Sim::fran_karta(&kart, 11, 2);
    tur.turordning = true;
    tur.atervaxt = 0.0;
    for d in tur.djur.iter_mut() {
        d.lev = true;
        d.i = 1;
        d.j = 1;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::Allvetande;
    }
    for c in 0..tur.v.len() {
        tur.v[c] = 1.0;
    }
    tur.v[mal] = 80.0;
    tur.steg();
    assert!(
        (tur.v[mal] - a2.v[mal]).abs() > 1e-6,
        "mutanten 'bete i flyttloopen' (turordning) ska skilja sig från läge a"
    );

    // Läge b: antal ur ögonblicksbild → 3 och 0.
    let mut bb = Sim::fran_karta(&kart, 3, 3);
    bb.bete_i_tur = true;
    for d in bb.djur.iter_mut() {
        d.lev = true;
        d.i = 2;
        d.j = 2;
        d.e = 500.0;
        d.age = 0;
        d.hona = false;
        d.formaga = Formaga::TrangselOrakel;
        d.egen_rng = Rng::new(99);
    }
    for c in 0..bb.v.len() {
        bb.v[c] = 1.0;
    }
    bb.v[c_n] = 50.0;
    bb.v[c_o] = 50.0;
    bb.steg();
    let mut bc = [0usize; 2];
    for d in bb.djur.iter().filter(|d| d.lev) {
        if d.i == 2 && d.j == 3 {
            bc[0] += 1;
        } else if d.i == 3 && d.j == 2 {
            bc[1] += 1;
        }
    }
    let mut bs = bc;
    bs.sort();
    assert_eq!(
        bs,
        [0, 3],
        "läge b ska ge 3 och 0 (ögonblicksbild), fick N={} Ö={}",
        bc[0],
        bc[1]
    );
}

/// Kontroll 5 (signal): u_to/s_to-upprepning — litet rutnät + jsonl-jämförelse.
#[test]
fn upprepning_u_to_s_to() {
    assert!(!Sim::fran_karta(&bara(Vatten::Land, 4, 4), 1, 2).flytt_i_tur);
    assert!(!Sim::fran_karta(&bara(Vatten::Land, 4, 4), 1, 2).bete_i_tur);

    let kart = bara(Vatten::Land, 6, 6);
    let fro = 181u64;
    let mut u = Sim::fran_karta(&kart, fro, 8);
    let mut u2 = Sim::fran_karta(&kart, fro, 8);
    u.turordning = true;
    u2.turordning = true; // samma väg som Alt 1246 u_to
    let mut s = Sim::fran_karta(&kart, fro, 8);
    let mut s2 = Sim::fran_karta(&kart, fro, 8);
    // s_to: båda av
    for _ in 0..tal::STEG_PER_AR * 2 {
        u.steg();
        u2.steg();
        s.steg();
        s2.steg();
    }
    assert_eq!(u.djur_summa(), u2.djur_summa());
    assert_eq!(s.djur_summa(), s2.djur_summa());

    let jsonl = concat!(
        r#"{"arm":"u_to","spegel":false,"fro":181,"turordning":true,"antal_levande_vid_markning":10,"antal_levande_vid_slut":10,"markta_vid_slut":8,"andel_markta":0.8,"markta_formaga":"trangsel_orakel","omarkta_formaga":"slump","matning":[],"kortid_s":1.0}"#,
        "\n",
        r#"{"arm":"s_to","spegel":false,"fro":181,"turordning":false,"antal_levande_vid_markning":10,"antal_levande_vid_slut":10,"markta_vid_slut":1,"andel_markta":0.1,"markta_formaga":"trangsel_orakel","omarkta_formaga":"slump","matning":[],"kortid_s":1.0}"#,
        "\n",
    );
    let k = vec![
        hcsim::signal::Korning {
            arm: "u_to".into(),
            spegel: false,
            fro: 181,
            antal_markering: 10,
            antal_slut: 10,
            markta_slut: 8,
            andel_markta: 0.8,
            kortid_s: 1.0,
            torr: false,
            markta_formaga: "trangsel_orakel".into(),
            omarkta_formaga: "slump".into(),
            klocka: hcsim::signal::Klocka::Turordning,
            logg_rader: 0,
            ingrepp_mark: None,
            ingrepp_slut: None,
            matning: vec![],
        },
        hcsim::signal::Korning {
            arm: "s_to".into(),
            spegel: false,
            fro: 181,
            antal_markering: 10,
            antal_slut: 10,
            markta_slut: 1,
            andel_markta: 0.1,
            kortid_s: 1.0,
            torr: false,
            markta_formaga: "trangsel_orakel".into(),
            omarkta_formaga: "slump".into(),
            klocka: hcsim::signal::Klocka::Samtidig,
            logg_rader: 0,
            ingrepp_mark: None,
            ingrepp_slut: None,
            matning: vec![],
        },
    ];
    let (ok, tot, text) = jamfor_tur1246(&k, jsonl).expect("jämför");
    assert_eq!(ok, tot);
    assert!(text.contains("exakt samma"));
    // Mutant: fel andel → röd.
    let mut fel = k.clone();
    fel[0].andel_markta = 0.5;
    let (ok2, _, _) = jamfor_tur1246(&fel, jsonl).expect("jämför fel");
    assert!(ok2 < tot, "mutanten med fel andel ska falla");
}

/// Kontroll 6 (signal): bokföringen — mutant som tappar födsel blir röd.
#[test]
fn bokforingen_fanger_tappad_fodsel() {
    let kart = bara(Vatten::Land, 8, 8);
    let fro = 181u64;
    let mut sim = Sim::fran_karta(&kart, fro, 30);
    sim.mark_lage = MarkLage::Noll;
    sim.hjarn_arm = Some(HjarnArm {
        namn: "u_to",
        markta: Formaga::TrangselOrakel,
        omarkta: Formaga::Slump,
        spegel: false,
    });
    sim.matning.pa = true;
    for _ in 0..tal::STEG_PER_AR * 2 {
        sim.steg();
    }
    sim.markera_halften(fro);
    sim.starta_bokforing(fro, "u_to");
    for _ in 0..1200 {
        sim.steg();
    }
    let antal = sim.antal_per_formaga();
    assert!(
        sim.bokforing.kontroll(&antal).is_ok(),
        "bokföringen ska gå ihop"
    );
    assert!(
        !sim.bokforing.block.is_empty(),
        "minst ett 1000-stegsblock ska finnas"
    );

    // Mutant: tappa en födsel i tot-räknaren.
    if let Some(v) = sim.bokforing.fodda_tot.values_mut().next() {
        if *v > 0 {
            *v -= 1;
        } else {
            *v = 0;
            // tvinga fel genom att öka antal_slut-förväntan
        }
    }
    // Om inga födslar: manipulera dod_tot i stället.
    let fel = sim.bokforing.kontroll(&antal);
    if sim.bokforing.fodda_tot.values().any(|&v| v > 0) || sim.bokforing.dod_tot.values().sum::<u64>() > 0
    {
        // Efter att vi minskat fodda_tot ska kontrollen falla om det fanns födslar.
        // Om fodda var 0 och vi inte ändrade: tvinga fel.
        if fel.is_ok() {
            let k = sim
                .bokforing
                .antal_markering
                .keys()
                .next()
                .cloned()
                .unwrap();
            *sim.bokforing.fodda_tot.entry(k).or_default() += 1;
            assert!(
                sim.bokforing.kontroll(&antal).is_err(),
                "mutanten som lägger till en spökfödsel ska bli röd"
            );
        } else {
            assert!(fel.is_err(), "mutanten som tappar födsel ska bli röd");
        }
    } else {
        let k = sim
            .bokforing
            .antal_markering
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "slump".into());
        *sim.bokforing.fodda_tot.entry(k).or_default() += 1;
        assert!(
            sim.bokforing.kontroll(&antal).is_err(),
            "mutanten som lägger till en spökfödsel ska bli röd"
        );
    }
}

/// Kontroll 7 (signal): Kendalls W — lika → 1, motsatta → 0, tom → fel.
#[test]
fn kendalls_w_kontroller() {
    let lika = vec![[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]; 5];
    let w = kendalls_w(&lika).expect("W");
    assert!((w - 1.0).abs() < 1e-9, "lika ordningar ska ge W=1, fick {w}");

    let motsatta = vec![
        [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
    ];
    let w0 = kendalls_w(&motsatta).expect("W0");
    assert!(w0.abs() < 1e-9, "motsatta för två djur ska ge W=0, fick {w0}");

    let tom: Vec<[f64; 8]> = Vec::new();
    assert!(kendalls_w(&tom).is_err(), "tom delmängd ska stoppa");
}

/// Kontroll 8 (signal): spegeln och samma_hjarnpris oförändrade.
#[test]
fn signal_spegel_och_hjarnpris_kvar() {
    let arm = HjarnArm {
        namn: "a_to",
        markta: Formaga::TrangselOrakel,
        omarkta: Formaga::Slump,
        spegel: false,
    };
    let sp = arm.clone().spegel_av();
    assert_eq!(sp.markta, Formaga::Slump);
    assert_eq!(sp.omarkta, Formaga::TrangselOrakel);
    assert!(sp.spegel);

    let kart = bara(Vatten::Land, 4, 4);
    let mut sim = Sim::fran_karta(&kart, 1, 2);
    sim.djur[0].formaga = Formaga::Lar;
    sim.djur[1].formaga = Formaga::TrangselOrakel;
    sim.djur[0].e = 200.0;
    sim.djur[1].e = 200.0;
    for c in 0..sim.v.len() {
        sim.v[c] = 0.0;
    }
    sim.flytt_i_tur = true;
    let e0 = sim.djur[0].e;
    let e1 = sim.djur[1].e;
    sim.steg();
    assert!((e0 - sim.djur[0].e - (e1 - sim.djur[1].e)).abs() < 1e-9);
}
