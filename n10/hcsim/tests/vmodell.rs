//! Prov för Fas 1 (Håkan 2026-11-23): prövningen av hjärnan som handlingsvillkorad världsmodell (`hcsim::vmodell`).
//! Gränserna är skrivna som tal ur piharness `docs/evolution/vmodell-fas1-utfallsregel.md`.
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::sim::{KantLage, Sim, GRANNAR};
use hcsim::tal::{A_N, STEG_PER_AR, Z_N};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::vmodell::{dom_k1, dom_k2, dom_k3, grupp, prov_med_persistens, prov_med_skugga, prova_djur, samma_handling_samma_svar, valj_djur, Dom, Matt, Modell, Prov, Skugga, Utfall};
use hckarta::{Kalla, Tidtabell};
use std::path::PathBuf;

fn varld_ar2() -> Sim {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let k = Kalla::las(&rot.join("../paleokarta/data")).unwrap();
    let tt = Tidtabell::las(&std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).unwrap()).unwrap();
    let arm = utforsk_armar().into_iter().find(|x| x.namn == "e_lar" && !x.spegel).expect("armen e_lar");
    let mut sim = bygg_sim_balte(&k, &tt, B_FULL, H_FULL, BALTE_K, 261, &arm).unwrap();
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    satt_rattningar_utdod(&mut sim);
    while sim.t < 2 * STEG_PER_AR {
        sim.steg();
    }
    assert!(sim.antal() > 100, "tom läsning: för få djur vid år 2");
    sim
}

fn hela(sim: &Sim) -> String {
    serde_json::to_string(sim).expect("världen går att skriva")
}

/// Kroken: utan den går världen som förut; med hjärnans eget val som tvingad handling blir världen bit för bit densamma;
/// med en annan handling hamnar djuret i den rutan.
#[test]
fn prov1_kroken_tvingar_ratt_handling_och_ror_inget_annat() {
    let bas = varld_ar2();
    let idx = valj_djur(&bas, 1, 7)[0];
    let (i0, j0) = (bas.djur[idx].i, bas.djur[idx].j);
    let mut fri = bas.clone();
    fri.steg();
    let eget_val = fri.djur[idx].hj.forra.as_ref().unwrap().1;
    let mut tv = bas.clone();
    tv.tvinga = Some((idx, eget_val));
    tv.steg();
    assert_eq!(tv.djur_summa(), fri.djur_summa(), "tvingat eget val ska ge samma värld som det fria steget");
    let mut olika = 0;
    for a in 0..A_N {
        let mut k = bas.clone();
        k.tvinga = Some((idx, a));
        k.steg();
        assert_eq!(k.djur[idx].hj.forra.as_ref().unwrap().1, a, "hjärnan ska ha förutsagt för den tvingade handlingen");
        let (di, dj) = GRANNAR[a];
        let (mi, mj) = k.kant_granne(i0 as i32 + di, j0 as i32 + dj);
        let pa_mal = (k.djur[idx].i as i32, k.djur[idx].j as i32) == (mi, mj);
        let kvar = (k.djur[idx].i, k.djur[idx].j) == (i0, j0);
        assert!(pa_mal || kvar, "handling {a}: djuret står varken i målrutan eller kvar (spärrad ruta)");
        if k.djur_summa() != fri.djur_summa() {
            olika += 1;
        }
    }
    assert!(olika >= 7, "minst sju av nio handlingar ska ge en annan värld än det fria steget; fick {olika}");
}

/// K4: originalet är orört efter ett prov, också i de fält som kontrollsumman inte läser. Rött prov: ett steg i originalet ändrar texten.
#[test]
fn prov2_provet_ror_inte_originalet() {
    let mut bas = varld_ar2();
    let fore = hela(&bas);
    let summa = bas.djur_summa();
    for idx in valj_djur(&bas, 3, 11) {
        let _ = prova_djur(&bas, idx);
    }
    assert_eq!(bas.djur_summa(), summa);
    assert!(hela(&bas) == fore, "originalet ändrades av provet");
    bas.steg();
    assert!(hela(&bas) != fore, "kontrollen kan inte bli röd: ett steg syns inte i texten");
}

/// K5: samma handling ger samma svar. Rött prov: två olika handlingar ger olika världar.
#[test]
fn prov3_samma_handling_samma_svar() {
    let bas = varld_ar2();
    let idx = valj_djur(&bas, 1, 13)[0];
    for a in [0, 3, 8] {
        assert!(samma_handling_samma_svar(&bas, idx, a));
    }
    let en = |a: usize| {
        let mut k = bas.clone();
        k.tvinga = Some((idx, a));
        k.steg();
        k.djur_summa()
    };
    assert_ne!(en(0), en(3), "kontrollen kan inte bli röd: två olika handlingar gav samma värld");
}

/// K0: de nio kopiorna utgår från samma inre bild, och nio utfall kommer tillbaka.
#[test]
fn prov4_nio_handlingar_fran_samma_lage() {
    let bas = varld_ar2();
    let mut n = 0;
    for idx in valj_djur(&bas, 5, 17) {
        if let Ok(p) = prova_djur(&bas, idx) {
            assert!(p.samma_z);
            assert_eq!(p.utfall.len(), 9);
            assert!(p.utfall[1..].iter().filter(|u| u.flyttade).count() >= 6, "de flesta förflyttningar ska flytta djuret");
            assert!(!p.utfall[0].flyttade, "handling 0 är att stå kvar");
            n += 1;
        }
    }
    assert!(n >= 3, "för få prov gick att räkna: {n}");
}

/// K6: förutsägelsen läser bara djurets egna sinnen. Växter utanför de nio rutorna ändrar inget; växter i den egna rutan gör det.
#[test]
fn prov5_ingen_dold_kunskap() {
    let bas = varld_ar2();
    let idx = valj_djur(&bas, 1, 19)[0];
    let (i, j) = (bas.djur[idx].i, bas.djur[idx].j);
    let forutsag = |s: &Sim| -> Vec<Vec<f64>> {
        let z = s.djur[idx].hj.kod(&s.sinnen_for(idx));
        (0..A_N).map(|a| s.djur[idx].hj.forutsag(&z, a)).collect()
    };
    let grund = forutsag(&bas);
    // Långt bort: fem rutor åt sidan (utanför de nio), alla växter borta där.
    let mut bort = bas.clone();
    let fi = (i + 5) % bort.b;
    let c = bort.cell(fi, j);
    assert!(bort.v[c] > 0.0 || bort.vmax[c] == 0.0);
    bort.v[c] = 0.0;
    // Också ett annat djurs energi långt bort.
    let annat = (0..bort.djur.len()).find(|&k| bort.djur[k].lev && k != idx && bort.djur[k].i.abs_diff(i) > 3 && bort.djur[k].i.abs_diff(i) < bort.b - 3).expect("ett djur långt bort");
    bort.djur[annat].e *= 0.5;
    assert_eq!(forutsag(&bort), grund, "en ändring utanför djurets sinnen ändrade förutsägelsen");
    // Den egna rutan: växterna halveras.
    let mut nara = bas.clone();
    let c = nara.cell(i, j);
    assert!(nara.v[c] > 0.0, "tom läsning: inga växter i djurets ruta");
    nara.v[c] *= 0.5;
    assert_ne!(forutsag(&nara), grund, "kontrollen kan inte bli röd: en ändring i den egna rutan syntes inte");
}

fn utfall(y: [f64; Z_N], zn: [f64; Z_N]) -> Utfall {
    Utfall { y, zn, v_pred: y[0], v_sann: zn[0], intag: 0.0, flyttade: false }
}

/// Ett påhittat prov: utfallet för handling a är z + a·steg i första talet; `f` ger förutsägelsen ur (z, utfallet, a).
fn pahittat(f: impl Fn([f64; Z_N], [f64; Z_N], usize) -> [f64; Z_N]) -> Prov {
    let z = [0.1, 0.2, 0.3, 0.4];
    let u = (0..A_N)
        .map(|a| {
            let zn = [z[0] + 0.05 * a as f64, z[1] + 0.01, z[2], z[3]];
            utfall(f(z, zn, a), zn)
        })
        .collect();
    Prov { uid: 1, z, utfall: u, hjarnans_val: 0, samma_z: true, ren_diff: 0.0, erfarenhet: None }
}

fn matt_av(p: &Prov) -> Matt {
    let mut m = Matt::default();
    m.lagg(p);
    m
}

/// Måtten på kända fall, med gränserna 0,90, 1 och 0,10 skrivna som tal.
#[test]
fn prov6_matten_pa_kanda_fall() {
    // Fullträff: R² = 1, felet 0, alla par särskiljda.
    let m = matt_av(&pahittat(|_, zn, _| zn));
    assert!((m.k3() - 1.0).abs() < 1e-12 && m.k2(3) == 0.0 && (m.k1() - 1.0).abs() < 1e-12);
    assert_eq!((dom_k1(&[m.clone()]), dom_k2(&[m.clone()]), dom_k3(&[m.clone()])), (Dom::Pass, Dom::Pass, Dom::Pass));
    // Persistensen (gissar nuläget för alla handlingar): R² = 0, kvoten 1, alla 36 par identiska.
    let m = matt_av(&pahittat(|z, _, _| z));
    assert!(m.k3().abs() < 1e-12 && (m.k2(5) - 1.0).abs() < 1e-12 && m.k1() == 0.0);
    assert_eq!(m.par_identiska, 36);
    assert_eq!((dom_k1(&[m.clone()]), dom_k2(&[m.clone()]), dom_k3(&[m.clone()])), (Dom::Fail, Dom::Fail, Dom::Fail));
    // Blind för handlingen men rätt i medel: bättre än persistensen i felet (K2 kan bli grönt), men R² = 0 (K3 rött).
    let medel = [0.1 + 0.05 * 4.0, 0.21, 0.3, 0.4];
    let m = matt_av(&pahittat(|_, _, _| medel));
    assert!(m.k3().abs() < 1e-12, "en förutsägare som inte bryr sig om handlingen ska få R² = 0, fick {}", m.k3());
    assert_eq!(dom_k3(&[m.clone()]), Dom::Fail);
    assert!(m.k2(8) < 1.0, "den blinda förutsägaren slår persistensen för handling 8");
    // Fel tecken på handlingens verkan: R² = −3.
    let m = matt_av(&pahittat(|_, zn, _| [2.0 * medel[0] - zn[0], zn[1], zn[2], zn[3]]));
    assert!((m.k3() + 3.0).abs() < 1e-9, "spegelvänd verkan ska ge R² = −3, fick {}", m.k3());
    assert!((m.cosinus() + 1.0).abs() < 1e-9);
    // Rätt riktning men hälften så stor: R² = 0,75, storleken 0,5; PASS vid gränsen 0,10.
    let m = matt_av(&pahittat(|_, zn, _| [medel[0] + 0.5 * (zn[0] - medel[0]), zn[1], zn[2], zn[3]]));
    assert!((m.k3() - 0.75).abs() < 1e-9 && (m.storlek() - 0.5).abs() < 1e-9);
    assert_eq!(dom_k3(&[m]), Dom::Pass);
}

/// Skuggorna på en påhittad värld där handlingens verkan följer riktningen: nästa bild = bilden + 0,1 · (dx, dy, 0, 0).
/// Djuret har bara upplevt handlingarna 0, 1, 3 och 5. Den gemensamma riktningsmodellen ska då förutsäga handling 7 och 2 (aldrig utförda)
/// rätt; de nio skilda och förskjutningsmodellen kan inte veta något om dem och stannar vid persistensen.
/// (Med bara 0, 1 och 3 går "rör sig" inte att skilja från riktningen, och då för inte heller riktningsmodellen över rätt: felet blev 0,067.)
#[test]
fn prov7_skuggorna_lar_bara_av_egen_erfarenhet() {
    let verkan = |z: &[f64; Z_N], a: usize| {
        let (dx, dy) = GRANNAR[a];
        [z[0] + 0.1 * dx as f64, z[1] + 0.1 * dy as f64, z[2], z[3]]
    };
    let mut s = Skugga::ny();
    let mut g = hcsim::Rng::new(5);
    for _ in 0..6000 {
        let z = [g.f() - 0.5, g.f() - 0.5, g.f() - 0.5, g.f() - 0.5];
        let a = [0, 1, 3, 5][g.under(4)];
        s.lar(&z, a, &verkan(&z, a));
    }
    assert_eq!(s.erfarenhet[7] + s.erfarenhet[2], 0);
    assert_eq!(s.erfarenhet.iter().sum::<u32>(), 6000);
    let z = [0.2, -0.1, 0.3, 0.0];
    let fel = |m: Modell, a: usize| {
        let (y, t) = (s.forutsag(m, &z, a), verkan(&z, a));
        (0..Z_N).map(|i| (y[i] - t[i]).abs()).fold(0.0, f64::max)
    };
    // Upplevda handlingar: alla tre modellerna har lärt sig dem.
    for m in [Modell::Nio, Modell::GemForskjutning, Modell::GemRiktning] {
        assert!(fel(m, 1) < 0.01 && fel(m, 3) < 0.01, "{m:?} lärde sig inte en upplevd handling: {} {}", fel(m, 1), fel(m, 3));
    }
    // Aldrig utförda handlingar 7 = (−1,0) och 2 = (+1,+1): riktningsmodellen för över kunskapen, de andra inte.
    assert!(fel(Modell::GemRiktning, 7) < 0.01 && fel(Modell::GemRiktning, 2) < 0.01, "riktningsmodellen förde inte över: {} {}", fel(Modell::GemRiktning, 7), fel(Modell::GemRiktning, 2));
    assert!(fel(Modell::Nio, 7) > 0.09, "de nio skilda kan inte veta något om en handling som aldrig utförts: {}", fel(Modell::Nio, 7));
    assert!(fel(Modell::GemForskjutning, 7) > 0.05, "förskjutningsmodellen har ingen förskjutning för en handling som aldrig utförts: {}", fel(Modell::GemForskjutning, 7));
    // En ny skugga är persistensen.
    let ny = Skugga::ny();
    for m in [Modell::Nio, Modell::GemForskjutning, Modell::GemRiktning] {
        assert_eq!(ny.forutsag(m, &z, 4), z);
    }
}

/// Erfarenhetsgrupperna med gränserna 0, 1–9, 10–49 och 50 skrivna som tal, och att skuggans prov lär sig av förra steget före förutsägelsen
/// men aldrig ser provets utfall.
#[test]
fn prov8_erfarenhetsgrupper_och_ingen_lacka() {
    assert_eq!([grupp(0), grupp(1), grupp(9), grupp(10), grupp(49), grupp(50), grupp(4000)], [0, 1, 1, 2, 2, 3, 3]);
    let p = pahittat(|_, zn, _| zn);
    let mut s = Skugga::ny();
    s.forra = Some(([0.0, 0.0, 0.0, 0.0], 2));
    let q = prov_med_skugga(&p, &s, Modell::Nio);
    assert_eq!(q.erfarenhet.unwrap()[2], 1, "förra stegets erfarenhet ska räknas före förutsägelsen");
    assert_eq!(s.erfarenhet[2], 0, "skuggan själv får inte ändras av provet");
    // Skuggan har aldrig sett utfallen: för handling 8 (aldrig tränad) är förutsägelsen bilden själv, inte utfallet.
    assert_eq!(q.utfall[8].y, p.z);
    assert_ne!(q.utfall[8].y, p.utfall[8].zn);
    // Utfallen är orörda, och persistensen ger R² = 0.
    for a in 0..A_N {
        assert_eq!(q.utfall[a].zn, p.utfall[a].zn);
    }
    let mut m = Matt::default();
    m.lagg(&prov_med_persistens(&p));
    assert!(m.k3().abs() < 1e-12);
    let mut m = Matt::default();
    m.lagg(&q);
    assert_eq!(m.grp_n[2][1], 1);
    assert_eq!(m.grp_n[8][0], 1);
    assert_eq!(m.erf_aldrig, 8);
}
