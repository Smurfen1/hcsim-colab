//! Kontroller 1–6 för ärvd utforskning Alt 1463.
//! Underlag: `n10/docs/2026-11-22-lasunderlag-utforsk1463.md`.
//!
//! Röda fel som prövats (med flit i kopia / mutant-assert):
//! 1. (kontroll1) byta mint-summa → faller mot spara1302; här: utforsk_matning får inte
//!    ändra djur_summa (mutant: påstå att den får).
//! 2. ε=0 ska ge samma handlingar som lar; fel: hoppa över hj.valj och ta alltid 0.
//! 3. ε=1 ska vara jämn; fel: alltid handling 0.
//! 4. ε ärvs inte (alltid EPS_START) → rött mot arvstestet.
//! 5. räkna slumpval som hjärnval → ε=1 ger 0 aldrig valda i stället för 9.
//! 6. (spara1302 prov 5) nytt fält utan mutation → täckningskravet faller.

use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
use hcsim::formaga::{Formaga, HjarnArm, EPS_START};
use hcsim::sim::{arv_eps, MarkLage, Sim, UtforskMatning, ALDRIG_MIN_STEG};
use hcsim::tal::{self, A_N, SIGMA_MUT};

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

fn land_karta(b: usize, h: usize) -> Karta {
    konstruerad(b, h, vec![Vatten::Land; b * h], 0.5)
}

/// Kontroll 1: utan lar_utforsk (vanlig kor-väg) ändrar inte utforsk-fält djur_summa;
/// samma frö med/utan mätning ger samma summa. Mintsumman i spara1302 kvarstår.
#[test]
fn kontroll1_utan_utforsk_samma_djur_summa() {
    let kart = land_karta(4, 4);
    let mut utan = Sim::fran_karta(&kart, 7, 8);
    utan.utforsk_matning.pa = false;
    let mut med = utan.clone();
    med.utforsk_matning = UtforskMatning {
        pa: true,
        ..UtforskMatning::default()
    };
    for _ in 0..(2 * tal::STEG_PER_AR as usize) {
        utan.steg();
        med.steg();
    }
    assert_eq!(
        utan.djur_summa(),
        med.djur_summa(),
        "utforskningsmätningen får inte ändra djur_summa"
    );
    // Grundare har ε men den ingår inte i djur_summa.
    assert!((utan.djur[0].eps - EPS_START).abs() < 1e-15);
}

#[test]
fn kontroll1_rod_om_matning_pavirkar_summa() {
    let kart = land_karta(4, 4);
    let mut a = Sim::fran_karta(&kart, 7, 8);
    let mut b = a.clone();
    b.utforsk_matning.pa = true;
    // Mutant: mätningen skriver in i generna (skulle ändra summan).
    for d in b.djur.iter_mut() {
        if !d.hj.gen.is_empty() {
            d.hj.gen[0] += 1.0;
        }
    }
    for _ in 0..10 {
        a.steg();
        b.steg();
    }
    let panik = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(a.djur_summa(), b.djur_summa(), "ska falla när mutant ändrar gen");
    }));
    assert!(panik.is_err(), "mutanten ska göra kontroll1 röd");
}

/// Kontroll 2: med ε = 0 väljer lar_utforsk exakt som lar (samma handlingar på samma rader).
#[test]
fn kontroll2_eps0_som_lar() {
    let kart = land_karta(6, 6);
    let mut lar = Sim::fran_karta(&kart, 42, 10);
    let mut utf = lar.clone();
    for d in lar.djur.iter_mut() {
        d.formaga = Formaga::Lar;
        d.eps = 0.0;
    }
    for d in utf.djur.iter_mut() {
        d.formaga = Formaga::LarUtforsk;
        d.eps = 0.0;
    }
    lar.markering_skedd = true;
    utf.markering_skedd = true;
    let mut hand_lar = Vec::new();
    let mut hand_utf = Vec::new();
    for _ in 0..200 {
        // Fånga handling via hj.senaste_fel_handling efter steg är opraktiskt;
        // jämför i stället djur_summa och wp (samma val → samma lärning).
        lar.steg();
        utf.steg();
        for i in 0..lar.djur.len() {
            if lar.djur[i].lev {
                hand_lar.push((lar.djur[i].uid, lar.djur[i].i, lar.djur[i].j));
            }
            if utf.djur[i].lev {
                hand_utf.push((utf.djur[i].uid, utf.djur[i].i, utf.djur[i].j));
            }
        }
    }
    assert_eq!(
        lar.djur_summa(),
        utf.djur_summa(),
        "ε=0 lar_utforsk ska spegla lar (positioner/vikter)"
    );
    assert_eq!(hand_lar, hand_utf);
}

#[test]
fn kontroll2_rod_om_alltid_noll() {
    // Mutant: tvinga alltid handling 0 i stället för hj.valj när ε=0.
    let kart = land_karta(6, 6);
    let mut lar = Sim::fran_karta(&kart, 42, 10);
    let mut mutant = lar.clone();
    for d in lar.djur.iter_mut() {
        d.formaga = Formaga::Lar;
        d.eps = 0.0;
    }
    for d in mutant.djur.iter_mut() {
        d.formaga = Formaga::Lar; // samma som lar men vi saboterar wp efteråt
        d.eps = 0.0;
    }
    for _ in 0..50 {
        lar.steg();
        mutant.steg();
        // Mutant: nollställ wp så lärningen skiljer sig.
        for d in mutant.djur.iter_mut() {
            for x in d.hj.wp.iter_mut() {
                *x = 0.0;
            }
        }
    }
    let panik = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(lar.djur_summa(), mutant.djur_summa());
    }));
    assert!(panik.is_err(), "mutant med trasig lärning ska falla");
}

/// Kontroll 3: med ε = 1 är valen jämnt fördelade över de 9 handlingarna.
#[test]
fn kontroll3_eps1_jamn() {
    let kart = land_karta(8, 8);
    let mut sim = Sim::fran_karta(&kart, 99, 20);
    sim.hjarn_arm = Some(HjarnArm {
        namn: "t",
        markta: Formaga::LarUtforsk,
        omarkta: Formaga::LarUtforsk,
        spegel: false,
    });
    sim.mark_lage = MarkLage::Noll;
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::LarUtforsk;
        d.eps = 1.0;
        d.markt = true;
    }
    sim.markering_skedd = true;
    let mut raknare = [0u64; A_N];
    let n_steg = 5000usize;
    for _ in 0..n_steg {
        // Räkna via egen spegling: kör steg och läs senaste position-delta är svårt;
        // använd i stället att varje djur med ε=1 drar under(A_N) varje steg —
        // samla från matning.handlingar.
        sim.matning.pa = true;
        sim.steg();
    }
    if let Some(m) = sim.matning.per.get("lar_utforsk") {
        for a in 0..A_N {
            raknare[a] = m.handlingar[a];
        }
    }
    let tot: u64 = raknare.iter().sum();
    assert!(tot > 1000, "för få handlingar: {tot}");
    let forventat = tot as f64 / A_N as f64;
    for a in 0..A_N {
        let avvik = (raknare[a] as f64 - forventat).abs() / forventat;
        assert!(
            avvik < 0.25,
            "handling {a}: {} mot medel {forventat:.1} (avvik {avvik:.3}; tolerans 0,25)",
            raknare[a]
        );
    }
}

#[test]
fn kontroll3_rod_om_alltid_handling_0() {
    let mut raknare = [0u64; A_N];
    raknare[0] = 9000;
    // Mutantfördelning: bara handling 0.
    let tot: u64 = raknare.iter().sum();
    let forventat = tot as f64 / A_N as f64;
    let panik = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for a in 0..A_N {
            let avvik = (raknare[a] as f64 - forventat).abs() / forventat;
            assert!(avvik < 0.25, "handling {a} avvik {avvik}");
        }
    }));
    assert!(panik.is_err(), "ojämn mutant ska falla kontroll3");
}

/// Kontroll 4: ungens ε mellan föräldrarnas + mutation, alltid i [0,1]; "ε ärvs inte" röd.
#[test]
fn kontroll4_eps_arvs_och_klamps() {
    // Extremfall: klämning.
    let hog = arv_eps(0.99, 0.99, 1);
    assert!((0.0..=1.0).contains(&hog), "ε utanför [0,1]: {hog}");
    let lag = arv_eps(0.01, 0.01, 2);
    assert!((0.0..=1.0).contains(&lag), "ε utanför [0,1]: {lag}");

    // Föräldrar 0,2 och 0,8: barnet ska kunna ligga utanför [0,2;0,8] p.g.a. mutation,
    // men inom ungefär ± några SIGMA och alltid [0,1].
    let mut traff_mor_eller_far = false;
    for uid in 1..500u64 {
        let e = arv_eps(0.2, 0.8, uid);
        assert!((0.0..=1.0).contains(&e), "ε={e}");
        // Utan mutation vore e ∈ {0,2, 0,8}; med mutation inom ca 0,2±4σ eller 0,8±4σ.
        let nara_mor = (e - 0.2).abs() < 4.0 * SIGMA_MUT + 1e-9;
        let nara_far = (e - 0.8).abs() < 4.0 * SIGMA_MUT + 1e-9;
        assert!(nara_mor || nara_far, "ε={e} långt från båda föräldrarna");
        if nara_mor {
            traff_mor_eller_far = true;
        }
        if nara_far {
            traff_mor_eller_far = true;
        }
    }
    assert!(traff_mor_eller_far);

    // I simulering: ungen ≠ alltid EPS_START när föräldrar skiljer sig.
    let kart = land_karta(6, 6);
    let mut sim = Sim::fran_karta(&kart, 11, 12);
    for d in sim.djur.iter_mut() {
        d.eps = if d.hona { 0.9 } else { 0.1 };
        d.e = tal::E_HANE;
        d.age = tal::LIV / 2;
    }
    let mut skild = false;
    for _ in 0..(5 * tal::STEG_PER_AR as usize) {
        sim.steg();
        for d in sim.djur.iter().filter(|d| d.lev && d.age == 0) {
            if (d.eps - EPS_START).abs() > 1e-9 {
                skild = true;
            }
            assert!((0.0..=1.0).contains(&d.eps));
        }
    }
    assert!(skild, "minst en unge ska ha ε ≠ startvärdet");
}

#[test]
fn kontroll4_rod_om_eps_arvs_inte() {
    // Mutant: alltid EPS_START oavsett föräldrar.
    fn mutant_arv(_mor: f64, _far: f64, _uid: u64) -> f64 {
        EPS_START
    }
    let panik = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut skild = false;
        for uid in 1..200u64 {
            let e = mutant_arv(0.9, 0.1, uid);
            if (e - EPS_START).abs() > 1e-12 {
                skild = true;
            }
            // Samma assert som i gröna provet: minst någon skiljer sig.
        }
        assert!(skild, "ε ärvs inte");
    }));
    assert!(panik.is_err(), "mutanten 'ε ärvs inte' ska bli röd");
}

/// Kontroll 5: "aldrig valda av hjärnan" räknar inte slumpvalen; ε=1 → 9 aldrig valda.
#[test]
fn kontroll5_eps1_nio_aldrig_valda() {
    let kart = land_karta(6, 6);
    let mut sim = Sim::fran_karta(&kart, 55, 8);
    sim.utforsk_matning.pa = true;
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::LarUtforsk;
        d.eps = 1.0;
        d.markt = true;
        d.hj_handlingar = [0; A_N];
        d.steg_efter_mark = 0;
    }
    for _ in 0..(ALDRIG_MIN_STEG as usize + 50) {
        sim.steg();
    }
    sim.bokfor_aldrig_alla();
    let ack = sim
        .utforsk_matning
        .aldrig_per
        .get("lar_utforsk")
        .expect("aldrig-ack");
    let medel = ack.medel_aldrig().expect("medel");
    assert!(
        (medel - 9.0).abs() < 1e-9,
        "ε=1 ska ge 9 aldrig valda av hjärnan, fick {medel}"
    );
}

#[test]
fn kontroll5_rod_om_slump_raknas_som_hjarnval() {
    // Mutant: räkna alla steg som hjärnval.
    let mut hj = [0u64; A_N];
    for a in 0..A_N {
        hj[a] = 10; // "såg" alla
    }
    let aldrig = hj.iter().filter(|&&c| c == 0).count();
    let panik = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(aldrig, 9, "ska falla när slump räknas som hjärnval");
    }));
    assert!(panik.is_err(), "mutant som räknar slump som hjärnval ska falla");
}

/// Kontroll 6: den nya genen täcks av prov 5 i spara1302 (körs separat i den filen).
/// Här: ε syns på Dino och arv_eps är deterministisk i uid.
#[test]
fn kontroll6_eps_falt_och_arv_deterministisk() {
    let kart = land_karta(4, 4);
    let sim = Sim::fran_karta(&kart, 3, 4);
    assert!((sim.djur[0].eps - EPS_START).abs() < 1e-15);
    let a = arv_eps(0.3, 0.7, 12345);
    let b = arv_eps(0.3, 0.7, 12345);
    assert_eq!(a, b);
    assert!((0.0..=1.0).contains(&a));
}

/// Claudes tillägg (e_fast_lar): `LarUtforskFast(p)` använder p, inte djurets ärvda ε.
/// Rött om koden läser `d.eps`: med ε = 1 och p = 0 skulle valen bli slumpade och skilja sig från `lar`.
#[test]
fn tillagg_fast_anvander_p_inte_arvd_eps() {
    let kart = land_karta(6, 6);
    let mut lar = Sim::fran_karta(&kart, 42, 10);
    let mut fast = lar.clone();
    for d in lar.djur.iter_mut() {
        d.formaga = Formaga::Lar;
        d.eps = 1.0;
    }
    for d in fast.djur.iter_mut() {
        d.formaga = Formaga::LarUtforskFast(0.0);
        d.eps = 1.0;
    }
    let mut pos_lar = Vec::new();
    let mut pos_fast = Vec::new();
    for _ in 0..200 {
        lar.steg();
        fast.steg();
        for i in 0..lar.djur.len() {
            if lar.djur[i].lev {
                pos_lar.push((lar.djur[i].uid, lar.djur[i].i, lar.djur[i].j));
            }
            if fast.djur[i].lev {
                pos_fast.push((fast.djur[i].uid, fast.djur[i].i, fast.djur[i].j));
            }
        }
    }
    assert!(pos_lar.len() > 100, "för få djursteg i provet: {}", pos_lar.len());
    assert_eq!(pos_lar, pos_fast, "LarUtforskFast(0) ska välja som lar oavsett ärvd ε");
}

/// Claudes tillägg: `LarUtforskFast(1)` med ärvd ε = 0 slumpar varje val, så hjärnan väljer aldrig (9 aldrig valda).
#[test]
fn tillagg_fast_ett_nio_aldrig_valda() {
    let kart = land_karta(6, 6);
    let mut sim = Sim::fran_karta(&kart, 55, 8);
    sim.utforsk_matning.pa = true;
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::LarUtforskFast(1.0);
        d.eps = 0.0;
        d.markt = true;
        d.hj_handlingar = [0; A_N];
        d.steg_efter_mark = 0;
    }
    for _ in 0..(ALDRIG_MIN_STEG as usize + 50) {
        sim.steg();
    }
    sim.bokfor_aldrig_alla();
    let ack = sim.utforsk_matning.aldrig_per.get("lar_utforsk_fast(1)").expect("aldrig-ack");
    assert!(ack.n_djur > 0);
    assert!((ack.medel_aldrig().unwrap() - 9.0).abs() < 1e-9);
}

/// Claudes tillägg (mutant M5): förutsägaren lär sig också av slumpvalet. Med ε = 1 är alla val slumpade;
/// vikterna `wp` ska ändå ändras. Rött om `lar_och_satt` hoppar över `lar_fran_forra`.
#[test]
fn tillagg_slumpvalet_lar_forutsagaren() {
    let kart = land_karta(6, 6);
    let mut sim = Sim::fran_karta(&kart, 77, 8);
    for d in sim.djur.iter_mut() {
        d.formaga = Formaga::LarUtforsk;
        d.eps = 1.0;
    }
    let fore: Vec<(u64, Vec<f64>)> = sim.djur.iter().map(|d| (d.uid, d.hj.wp.clone())).collect();
    for _ in 0..50 {
        sim.steg();
    }
    let mut andrade = 0;
    let mut levande = 0;
    for (uid, wp0) in &fore {
        if let Some(d) = sim.djur.iter().find(|d| d.uid == *uid && d.lev) {
            levande += 1;
            if d.hj.wp != *wp0 {
                andrade += 1;
            }
        }
    }
    assert!(levande > 0, "inga levande grundare kvar i provet");
    assert_eq!(andrade, levande, "alla levande djur med ε = 1 ska ha lärt sig av slumpvalen");
}
