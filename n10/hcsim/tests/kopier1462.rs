//! Kontroller 1–6 för kopieringsprovet Alt 1462.
//! Underlag: `n10/docs/2026-11-22-lasunderlag-kopier1462.md`.

use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};
use hcsim::formaga::{Formaga, HjarnArm};
use hcsim::hjarna::{Hj, NP, NK, N_GEN};
use hcsim::kopier::{self, Korning, Utfall, VANA_KVOT_MAX, VUNNA_KRAV};
use hcsim::sim::{KopierMatning, MarkLage, Sim};
use hcsim::tal::{self, A_N, MARKT_INKOR_AR, Z_N};

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

/// Sätt `W_a` till identitet och `b_a` till 0 för alla handlingar.
fn satur_wa_identitet_ba_noll(h: &mut Hj) {
    for a in 0..A_N {
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            for j in 0..=Z_N {
                let idx = b + i * (Z_N + 1) + j;
                h.wp[idx] = if j == i { 1.0 } else { 0.0 };
            }
        }
    }
    // Spegla i generna (startvikter) så arv/frusen inte stör provet.
    h.gen[NK..NK + NP].copy_from_slice(&h.wp);
}

/// Sätt `W_a` = 0 (bara bias kvar; bias oförändrad).
fn satur_wa_noll(h: &mut Hj) {
    for a in 0..A_N {
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            for j in 0..Z_N {
                h.wp[b + i * (Z_N + 1) + j] = 0.0;
            }
        }
    }
    h.gen[NK..NK + NP].copy_from_slice(&h.wp);
}

/// Kontroll 1: förutsägare som exakt kopierar → fel = kopieringsfel.
#[test]
fn kontroll1_forutsagare_ar_kopiering() {
    let mut gen = vec![0.0; N_GEN];
    // Kodare: z = tanh(s_0) ungefär; sätt enkla vikter.
    for i in 0..Z_N {
        gen[i * (tal::S_N + 1)] = 1.0; // vikt på s[0]
    }
    let mut h = Hj::fran_gen(gen);
    satur_wa_identitet_ba_noll(&mut h);

    let s1 = vec![0.2; tal::S_N];
    let s2 = vec![0.7; tal::S_N];
    let _ = h.valj(&s1, false, 0);
    let _ = h.valj(&s2, false, 0);

    let f = h.senaste_fel.expect("förutsägelsefel");
    let k = h.senaste_fel_kopiering.expect("kopieringsfel");
    let ba = h.senaste_fel_fast.expect("fast fel");
    assert!(
        (f - k).abs() < 1e-12,
        "förutsägare ska = kopiering när W=I, b=0: f={f} k={k}"
    );
    // b_a = 0 → |0 − z|² = |z|², vanligen ≠ kopiering.
    assert!(ba.is_finite());
}

/// Mutant till kontroll 1: W = 2·I → felet skiljer sig (provet blir rött om man byter).
#[test]
fn kontroll1_rod_om_wa_inte_identitet() {
    let mut gen = vec![0.0; N_GEN];
    for i in 0..Z_N {
        gen[i * (tal::S_N + 1)] = 1.0;
    }
    let mut h = Hj::fran_gen(gen);
    satur_wa_identitet_ba_noll(&mut h);
    // Mutant: skala W till 2.
    for a in 0..A_N {
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            h.wp[b + i * (Z_N + 1) + i] = 2.0;
        }
    }
    let s1 = vec![0.2; tal::S_N];
    let s2 = vec![0.7; tal::S_N];
    let _ = h.valj(&s1, false, 0);
    let _ = h.valj(&s2, false, 0);
    let f = h.senaste_fel.unwrap();
    let k = h.senaste_fel_kopiering.unwrap();
    assert!(
        (f - k).abs() > 1e-9,
        "mutanten W=2I ska ge skilda fel (f={f} k={k}); kontroll 1 blir röd om den byts in"
    );
}

/// Kontroll 2: W_a = 0 → förutsägelsefel = fast-delsfel.
#[test]
fn kontroll2_forutsagare_ar_fast_del() {
    let mut gen = vec![0.0; N_GEN];
    for i in 0..Z_N {
        gen[i * (tal::S_N + 1)] = 1.0;
        // Bias i W_a-starten (kopieras till wp).
        for a in 0..A_N {
            let b = NK + a * Z_N * (Z_N + 1);
            gen[b + i * (Z_N + 1) + Z_N] = 0.3 + 0.1 * i as f64;
        }
    }
    let mut h = Hj::fran_gen(gen);
    satur_wa_noll(&mut h);

    let s1 = vec![0.1; tal::S_N];
    let s2 = vec![0.9; tal::S_N];
    let _ = h.valj(&s1, false, 0);
    let _ = h.valj(&s2, false, 0);

    let f = h.senaste_fel.expect("förutsägelsefel");
    let fa = h.senaste_fel_fast.expect("fast fel");
    assert!(
        (f - fa).abs() < 1e-12,
        "förutsägare ska = fast del när W=0: f={f} fa={fa}"
    );
}

/// Mutant till kontroll 2: lämna W ≠ 0 → felet skiljer sig.
#[test]
fn kontroll2_rod_om_wa_inte_noll() {
    let mut gen = vec![0.0; N_GEN];
    for i in 0..Z_N {
        gen[i * (tal::S_N + 1)] = 1.0;
        for a in 0..A_N {
            let b = NK + a * Z_N * (Z_N + 1);
            gen[b + i * (Z_N + 1) + Z_N] = 0.3;
            gen[b + i * (Z_N + 1) + i] = 0.5; // W ≠ 0
        }
    }
    let mut h = Hj::fran_gen(gen);
    let s1 = vec![0.1; tal::S_N];
    let s2 = vec![0.9; tal::S_N];
    let _ = h.valj(&s1, false, 0);
    let _ = h.valj(&s2, false, 0);
    let f = h.senaste_fel.unwrap();
    let fa = h.senaste_fel_fast.unwrap();
    assert!(
        (f - fa).abs() > 1e-9,
        "mutanten med W≠0 ska ge skilda fel; kontroll 2 blir röd om W=0-kravet tappas"
    );
}

/// Kontroll 3: mätningen ändrar inte djur_summa.
#[test]
fn kontroll3_matningen_andrar_inte_djur_summa() {
    let b = 8;
    let h = 8;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 91u64;
    let n0 = 30;
    let ar = 40i64;
    let arm = HjarnArm {
        namn: "v_lar",
        markta: Formaga::Lar,
        omarkta: Formaga::Lar,
        spegel: false,
    };
    let mut utan = Sim::fran_karta(&kart, fro, n0);
    utan.mark_lage = MarkLage::Noll;
    utan.hjarn_arm = Some(arm.clone());
    utan.kopier_matning.pa = false;
    let mut med = Sim::fran_karta(&kart, fro, n0);
    med.mark_lage = MarkLage::Noll;
    med.hjarn_arm = Some(arm);
    med.kopier_matning = KopierMatning {
        pa: true,
        ..KopierMatning::default()
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
    assert_eq!(
        utan.djur_summa(),
        med.djur_summa(),
        "kopieringsmätningen får inte ändra djur_summa"
    );
    assert!(med.kopier_matning.krav_data().is_ok());
    assert!(med.kopier_matning.tot.n > 0);
}

/// Mutant till kontroll 3: om mätningen rörde en vikt skulle summan skilja sig.
#[test]
fn kontroll3_rod_om_matning_ror_vikt() {
    let b = 4;
    let h = 4;
    let kart = konstruerad(b, h, vec![Vatten::Land; b * h], 0.5);
    let fro = 3u64;
    let mut a = Sim::fran_karta(&kart, fro, 8);
    let mut bsim = Sim::fran_karta(&kart, fro, 8);
    for _ in 0..20 {
        a.steg();
        bsim.steg();
    }
    // Mutant: "mätningen" skriver i wp.
    bsim.djur[0].hj.wp[0] += 1.0;
    assert_ne!(
        a.djur_summa(),
        bsim.djur_summa(),
        "mutanten som rör wp ska ändra djur_summa (kontroll 3 blir röd om mätningen skriver)"
    );
}

/// Kontroll 4: mutant som räknar felet efter uppdateringen ger annat tal.
#[test]
fn kontroll4_mutant_efter_uppdatering_skiljer() {
    let gen = vec![0.1; N_GEN];
    let mut ratt = Hj::fran_gen(gen.clone());
    let mut mutant = Hj::fran_gen(gen);

    let s1 = vec![0.4; tal::S_N];
    let s2 = vec![0.6; tal::S_N];
    // Bygg forra.
    let _ = ratt.valj(&s1, true, 0);
    let _ = mutant.valj(&s1, true, 0);
    assert_eq!(ratt.wp, mutant.wp);

    // Nästa steg: rätt väg vs mutant.
    let z = ratt.kod(&s2);
    // Klona tillstånd före lärning.
    let mut ratt2 = ratt.clone();
    let mut mut2 = mutant.clone();
    // Rätt: via valj (mäter före).
    let _ = ratt2.valj(&s2, true, 0);
    let f_ratt = ratt2.senaste_fel.unwrap();
    let fa_ratt = ratt2.senaste_fel_fast.unwrap();

    // Mutant: mät efter uppdatering på samma forra.
    let z2 = mut2.kod(&s2);
    mut2.lar_fran_forra_mutant_efter_uppdatering(&z2, true, 0);
    let f_mut = mut2.senaste_fel.unwrap();
    let fa_mut = mut2.senaste_fel_fast.unwrap();

    assert!(
        (f_ratt - f_mut).abs() > 1e-15 || (fa_ratt - fa_mut).abs() > 1e-15,
        "mutanten 'efter uppdateringen' ska ge annat tal (f {f_ratt} vs {f_mut}, fast {fa_ratt} vs {fa_mut})"
    );
    // Kopieringen |zf−z| beror inte av vikter — den kan vara lika; det räcker att något skiljer.
    let _ = z;
}

/// Kontroll 4 positiv: rätt ordning (före) ger samma tal vid omkörning utan lärning.
#[test]
fn kontroll4_fore_uppdatering_stabil_utan_lar() {
    let mut h = Hj::fran_gen(vec![0.05; N_GEN]);
    let s1 = vec![0.3; tal::S_N];
    let s2 = vec![0.8; tal::S_N];
    let _ = h.valj(&s1, false, 0);
    let _ = h.valj(&s2, false, 0);
    let f1 = h.senaste_fel.unwrap();
    // Utan lärning är yf oförändrad; omkörning med samma forra-återställning.
    let mut h2 = Hj::fran_gen(vec![0.05; N_GEN]);
    let _ = h2.valj(&s1, false, 0);
    let _ = h2.valj(&s2, false, 0);
    assert_eq!(f1, h2.senaste_fel.unwrap());
}

/// Kontroll 5: nya fält täcks av prov 5 i spara1302 (fältvägen ur serialiseringen).
/// Här: forra_ba syns i vägsamlaren på en levande Hj.
#[test]
fn kontroll5_forra_ba_syns_i_vagar() {
    use hcsim::vagsamlare::samla_vagar;
    let mut h = Hj::fran_gen(vec![0.0; N_GEN]);
    let s = vec![0.2; tal::S_N];
    let _ = h.valj(&s, false, 0);
    assert!(h.forra_ba.is_some());
    let vagar = samla_vagar(&h).expect("vägar");
    assert!(
        vagar.iter().any(|v| v == "forra_ba" || v.starts_with("forra_ba")),
        "forra_ba ska synas i serialiseringen: {vagar:?}"
    );
    assert!(
        vagar.iter().any(|v| v.contains("senaste_fel_kopiering")),
        "senaste_fel_kopiering ska synas: {vagar:?}"
    );
    assert!(
        vagar.iter().any(|v| v.contains("senaste_fel_fast")),
        "senaste_fel_fast ska synas: {vagar:?}"
    );
}

/// Utfallsregeln: konstruerade körningar.
#[test]
fn utfallsregel_rod_nar_fa_slag() {
    let mut korningar = Vec::new();
    for i in 0..20u64 {
        // Förutsägare sämre än kopiering (högre fel) i alla.
        korningar.push(Korning {
            fro: 241 + i,
            antal_markering: 10,
            antal_slut: 10,
            kortid_s: 1.0,
            torr: false,
            djursteg: 100,
            medel_forutsag: 2.0,
            medel_kopiering: 1.0,
            medel_fast: 3.0,
            kvot_kopiering: 2.0,
            kvot_fast: 2.0 / 3.0,
            per_ar: vec![],
        });
    }
    let rak = kopier::rakna_utfall(&korningar);
    assert_eq!(rak.utfall, Utfall::RodtFable3);
    assert!(rak.n_slag_kopiering < VUNNA_KRAV);
}

#[test]
fn utfallsregel_vana_per_handling() {
    let mut korningar = Vec::new();
    for i in 0..20u64 {
        // Slår kopiering, men nära fast del (kvot ≤ 1.05).
        korningar.push(Korning {
            fro: 241 + i,
            antal_markering: 10,
            antal_slut: 10,
            kortid_s: 1.0,
            torr: false,
            djursteg: 100,
            medel_forutsag: 1.0,
            medel_kopiering: 2.0,
            medel_fast: 1.0 / VANA_KVOT_MAX, // kvot = 1.05
            kvot_kopiering: 0.5,
            kvot_fast: VANA_KVOT_MAX,
            per_ar: vec![],
        });
    }
    let rak = kopier::rakna_utfall(&korningar);
    assert_eq!(rak.utfall, Utfall::VanaPerHandling);
}

/// Mutant: fel gräns (kräv 20/20) skulle klassa om — dokumenterar att gränsen är 15.
#[test]
fn utfallsregel_rod_om_grans_andras() {
    let mut korningar = Vec::new();
    for i in 0..20u64 {
        let slag = i < 15; // precis 15 slår
        korningar.push(Korning {
            fro: 241 + i,
            antal_markering: 10,
            antal_slut: 10,
            kortid_s: 1.0,
            torr: false,
            djursteg: 100,
            medel_forutsag: if slag { 0.5 } else { 2.0 },
            medel_kopiering: 1.0,
            medel_fast: 3.0,
            kvot_kopiering: if slag { 0.5 } else { 2.0 },
            kvot_fast: 0.5,
            per_ar: vec![],
        });
    }
    let rak = kopier::rakna_utfall(&korningar);
    assert_eq!(rak.n_slag_kopiering, 15);
    // Med gräns 15: inte rött (går vidare till andra regler).
    assert_ne!(rak.utfall, Utfall::RodtFable3);
    // Mutant-gräns 16 skulle ge rött:
    assert!(
        rak.n_slag_kopiering < 16,
        "mutanten 'kräv 16' skulle fälla detta fall; gränsen är {VUNNA_KRAV}"
    );
}

/// Formaga::Lar används i v_lar (båda grupper).
#[test]
fn v_lar_anvander_lar() {
    assert_eq!(Formaga::Lar.som_str(), "lar");
    assert!(Formaga::Lar.lar_sig());
}
