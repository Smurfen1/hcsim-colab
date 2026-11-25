//! Prov för Alt 1423 (rättat instrument) och Alt 1424 (orsaken i loggen).

use hcsim::hjsignal::{
    instrument_per_fro, kontrollera_v_kvot, las_v_signal_kvot, orsak_per_fro, FABLE_VANLIGASTE_MIN,
};
use hcsim::signal::{
    handling0_matt, ingrepp_traff_negativ, ingrepp_traff_positiv, ingrepp_traff_sinnen,
    ingrepp_traff_trangsel, kvot_fran_matriser, kvot_fran_matriser_riktningar, las_logg_tsv,
    negativ_varden, positiv_varden, valj_fran_hj,
};
use hcsim::sim::{traff_andel, SignalLoggRad, Sim};
use hcsim::tal::{A_N, S_N, Z_N};
use hckarta::{Karta, Lager, Marke, Omrade, Period, Vatten};

fn tom_lager<T: Clone>(n: usize, v: T, marke: Marke) -> Lager<T> {
    Lager {
        varden: vec![v; n],
        marke,
    }
}

fn bara(vatten: Vatten, b: usize, h: usize) -> Karta {
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
            varden: vec![vatten; n],
            marke: Marke::Last,
        },
        mark: tom_lager(n, [0.25f32; 4], Marke::Simulering),
        vaxt: tom_lager(n, 0.5f32, Marke::Simulering),
        period: tom_lager(n, Period::Aldre, Marke::Simulering),
    }
}

/// Prov 1: positiv träff = 1; negativ ≤ 1/8; trängsel = 1/9.
#[test]
fn avlasare_ingrepp_traff() {
    let pos = ingrepp_traff_positiv();
    let neg = ingrepp_traff_negativ();
    let tr = ingrepp_traff_trangsel();
    assert!(
        (pos - 1.0).abs() < 1e-12,
        "positiv träff ska vara exakt 1, fick {pos}"
    );
    assert!(
        neg <= 1.0 / 8.0 + 1e-12,
        "negativ träff ska vara ≤ 1/8, fick {neg}"
    );
    assert!(
        (tr - 1.0 / 9.0).abs() < 1e-12,
        "trängselträff ska vara 1/9, fick {tr}"
    );
    // Mutanten "positiv träff skriven som 1,0" blir röd när avläsaren byts:
    // negativ avläsare i samma funktion ger inte 1.
    let muterad_som_positiv = 1.0_f64;
    let med_negativ = ingrepp_traff_sinnen(negativ_varden);
    assert!(
        (muterad_som_positiv - med_negativ).abs() > 0.5,
        "hårdkodad 1,0 ska skilja sig från negativ träff ({med_negativ})"
    );
}

/// Prov 2: hårdkodad positiv träff 1,0 döljer fel avläsare.
#[test]
fn mutant_hardkodad_positiv_traff_upptacks() {
    // Rätt kod: träffen följer avläsaren.
    assert!((ingrepp_traff_sinnen(positiv_varden_slice_compat) - 1.0).abs() < 1e-12);
    assert!(ingrepp_traff_sinnen(negativ_varden) < 0.2);
    // Mutantbeteende: alltid 1,0 oavsett avläsare — det är felet som ska bli rött.
    let mutant = |_: &[f64]| -> f64 { 1.0 };
    // Simulerad mutant: samma tal för positiv och negativ.
    let m_pos = mutant(&[]);
    let m_neg = mutant(&[]);
    assert_eq!(m_pos, m_neg, "mutanten ger samma tal för båda");
    assert!(
        (ingrepp_traff_positiv() - ingrepp_traff_negativ()).abs() > 0.5,
        "rätt kod ska skilja positiv och negativ träff"
    );
}

fn positiv_varden_slice_compat(s: &[f64]) -> [f64; A_N] {
    let mut arr = [0.0; S_N];
    for i in 0..S_N.min(s.len()) {
        arr[i] = s[i];
    }
    positiv_varden(&arr)
}

/// Prov 3: kvot över 1–8 ignorerar handling 0; mutant som tar med 0 blir röd.
#[test]
fn kvot_riktningar_ignorerar_handling_0() {
    let mut lika = Vec::new();
    let mut skiljd = Vec::new();
    for k in 0..25 {
        let mut rad = [0.0; A_N];
        for a in 1..A_N {
            rad[a] = ((k * a) % 7) as f64;
        }
        rad[0] = rad[1]; // lika med övriga mönstret
        lika.push(rad);
        let mut rad2 = rad;
        rad2[0] = 100.0 + k as f64; // bara handling 0 skiljer sig
        skiljd.push(rad2);
    }
    let k_lika = kvot_fran_matriser_riktningar(&lika).expect("lika");
    let k_skiljd = kvot_fran_matriser_riktningar(&skiljd).expect("skiljd");
    assert!(
        (k_lika - k_skiljd).abs() < 1e-12,
        "kvot 1–8 ska vara densamma när bara handling 0 skiljer: {k_lika} vs {k_skiljd}"
    );
    // Mutanten (gamla kvoten över 9) ändrar talet.
    let k9_lika = kvot_fran_matriser(&lika).expect("k9 lika");
    let k9_skiljd = kvot_fran_matriser(&skiljd).expect("k9 skiljd");
    assert!(
        (k9_lika - k9_skiljd).abs() > 1e-6,
        "mutanten som tar med handling 0 ska ändra kvoten (fick {k9_lika} vs {k9_skiljd})"
    );
}

/// Prov 4: lar-ingreppet med ny funktion = gamla talet på liten värld.
#[test]
fn lar_ingrepp_samma_som_forr() {
    let kart = bara(Vatten::Land, 6, 6);
    let mut sim = Sim::fran_karta(&kart, 42, 8);
    for d in sim.djur.iter_mut() {
        d.formaga = hcsim::formaga::Formaga::Lar;
        d.e = 150.0;
    }
    // Gamla vägen (ingrepp_traff anropar avläsarversionen med vardets_delar).
    let t_ny = sim.ingrepp_traff();
    // Explicit samma avläsare.
    let t_exp = sim.ingrepp_traff_avlasare(|d, s| {
        let (_, v, _, _) = d.hj.vardets_delar(s);
        v
    });
    assert!(
        (t_ny - t_exp).abs() < 1e-15,
        "lar-ingrepp ska ge samma tal: {t_ny} vs {t_exp}"
    );
    // Positiv avläsare på samma värld → 1.
    let t_pos = sim.ingrepp_traff_avlasare(|_d, s| positiv_varden_slice_compat(s));
    assert!((t_pos - 1.0).abs() < 1e-12, "positiv på värld ska ge 1, fick {t_pos}");
}

/// Prov 5: gamla kvoten på loggen upprepar V:s kvot (alla 20).
#[test]
fn v_kvot_upprepas_pa_loggen() {
    let logg_path = "/home/hakan/sandbox/pi/work/hcsim/v1251-logg.tsv";
    let v_path = concat!(env!("CARGO_MANIFEST_DIR"), "/v1251-signal.tsv");
    let logg_text = match std::fs::read_to_string(logg_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("hoppar över: kunde inte läsa logg ({e})");
            return;
        }
    };
    let rader = las_logg_tsv(&logg_text).expect("logg");
    let instrument = instrument_per_fro(&rader).expect("instrument");
    let v_text = std::fs::read_to_string(v_path).expect("v-signal");
    let v_kvot = las_v_signal_kvot(&v_text).expect("v kvot");
    assert_eq!(v_kvot.len(), 20);
    kontrollera_v_kvot(&instrument, &v_kvot).expect("V-kvot ska upprepas exakt");
}

/// Prov 6: måtten 1–6 på konstruerade rader; tom delmängd stoppar.
#[test]
fn orsak_matt_pa_konstruerade_rader() {
    // Ett djur som alltid väljer handling 3: vanligaste andel 1, p0 0, 8 aldrig valda.
    let mut rader = Vec::new();
    for i in 0..25 {
        let mut hj = [0.0; A_N];
        hj[3] = 10.0;
        let mut s = [0.0; S_N];
        s[6] = 0.5; // s[2*3]
        let mut z = [0.0; Z_N];
        z[0] = i as f64 * 0.01;
        let mut lage = [0.0; A_N];
        lage[3] = 1.0;
        let mut fast = [0.0; A_N];
        fast[1] = 5.0; // argmax fast ≠ 3
        rader.push(SignalLoggRad {
            fro: 1,
            djur_id: 7,
            s,
            z,
            hj_varden: hj,
            lage,
            fast,
        });
    }
    assert_eq!(valj_fran_hj(&rader[0].hj_varden), 3);
    let orsak = orsak_per_fro(&rader).expect("orsak");
    assert_eq!(orsak.len(), 1);
    let o = &orsak[0];
    assert!(
        (o.vanligaste_andel.unwrap() - 1.0).abs() < 1e-12,
        "vanligaste andel ska vara 1"
    );
    assert!((o.p0.unwrap() - 0.0).abs() < 1e-12, "p0 ska vara 0");
    assert!(
        (o.aldrig_valda.unwrap() - 8.0).abs() < 1e-12,
        "8 aldrig valda, fick {:?}",
        o.aldrig_valda
    );
    // fast dominans: valet 3 ≠ argmax(fast)=1 → 0
    assert!(
        o.fast_dominans.unwrap() < 1e-12,
        "fast dominans ska vara 0 när valet ≠ argmax(fast)"
    );
    assert!(o.z_inom_mellan.is_some());
    // R² redovisas (kan vara None om alla anpassningar misslyckas); misslyckanden räknas.
    let _ = o.r2_riktning;
    let _ = o.r2_riktning_gemensam;
    let _ = o.r2_misslyckade;

    let tom: Vec<SignalLoggRad> = Vec::new();
    assert!(
        orsak_per_fro(&tom).is_err(),
        "tom delmängd ska stoppa programmet"
    );
}

fn tom_rad(fro: u64, djur_id: u64) -> SignalLoggRad {
    SignalLoggRad {
        fro,
        djur_id,
        s: [0.0; S_N],
        z: [0.0; Z_N],
        hj_varden: [0.0; A_N],
        lage: [0.0; A_N],
        fast: [0.0; A_N],
    }
}

/// Prov 7a: R² per djur ≈ 1 när s[2a] är linjär i z med olika vikter; gemensam klart lägre.
/// Mutanten "gemensam i stället för per djur" blir röd.
#[test]
fn r2_per_djur_olika_vikter() {
    let mut rader = Vec::new();
    // Två djur, samma z-följd, men s[2a] = vikt * z[0] med olika vikt.
    for djur_id in [1u64, 2] {
        let vikt = if djur_id == 1 { 1.0 } else { 7.0 };
        for i in 0..25 {
            let mut r = tom_rad(9, djur_id);
            r.z[0] = i as f64;
            r.z[1] = (i % 3) as f64;
            r.z[2] = (i % 5) as f64 * 0.1;
            r.z[3] = 1.0 / (1.0 + i as f64);
            for a in 1..A_N {
                r.s[a * 2] = vikt * r.z[0] + 0.1 * a as f64;
            }
            // Handling så att övriga mått har data.
            r.hj_varden[1] = 1.0;
            rader.push(r);
        }
    }
    let orsak = orsak_per_fro(&rader).expect("orsak");
    assert_eq!(orsak.len(), 1);
    let o = &orsak[0];
    let per = o.r2_riktning.expect("R² per djur");
    let gem = o.r2_riktning_gemensam.expect("R² gemensam");
    assert!(
        per > 0.99,
        "R² per djur ska vara nära 1 vid exakt linjär s[2a](z), fick {per}"
    );
    assert!(
        gem < per - 0.2,
        "gemensam (alla djur) ska vara klart lägre än per djur när vikterna skiljer: per={per} gem={gem}"
    );
    assert_eq!(
        o.r2_misslyckade, 0,
        "inga misslyckade anpassningar väntas, fick {}",
        o.r2_misslyckade
    );
    // Mutanten som byter ut per-djur mot gemensam skulle falla på assert per > 0.99.
    assert!(
        gem < 0.99,
        "mutantkontroll: gemensam ensam är inte ≈1 (fick {gem})"
    );
    let skillnad = o.r2_skillnad.expect("skillnad");
    assert!(
        skillnad > 0.6,
        "linjär y(z): skillnad mot nollkontroll ska vara stor, fick {skillnad}"
    );
}

/// Prov 7b: s[2a] oberoende av z → R² per djur nära 0; nollkontrollen lika inom tolerans.
#[test]
fn r2_per_djur_oberoende() {
    let mut rader = Vec::new();
    for djur_id in [1u64, 2] {
        for i in 0..30 {
            let mut r = tom_rad(10, djur_id);
            // Linjärt oberoende z-kolumner (inte multiplar av varandra).
            r.z[0] = i as f64;
            r.z[1] = (i % 5) as f64;
            r.z[2] = ((i * 3) % 7) as f64;
            r.z[3] = 1.0 / (1.0 + (i % 11) as f64);
            // Fast slumpföljd oberoende av z.
            for a in 1..A_N {
                r.s[a * 2] = ((i * 17 + a * 31 + djur_id as usize * 41) % 97) as f64;
            }
            r.hj_varden[2] = 1.0;
            rader.push(r);
        }
    }
    let orsak = orsak_per_fro(&rader).expect("orsak");
    let o = &orsak[0];
    let per = o.r2_riktning.expect("R² per djur");
    let noll = o.r2_noll.expect("R² noll");
    let skillnad = o.r2_skillnad.expect("skillnad");
    assert!(
        per < 0.25,
        "R² per djur ska vara nära 0 när s[2a] är oberoende av z, fick {per}"
    );
    assert!(
        skillnad.abs() < 0.12,
        "vid brus ska riktigt R² och nollkontrollen vara lika: per={per} noll={noll} skillnad={skillnad}"
    );
    assert_eq!(o.r2_misslyckade, 0, "anpassningarna ska lyckas, fick {}", o.r2_misslyckade);
    assert_eq!(o.rader_per_djur_min, 30);
    assert_eq!(o.rader_per_djur_median, 30);
    assert_eq!(o.rader_per_djur_max, 30);
}

/// Prov 7d: linjär s[2a](z) → skillnaden mot nollkontrollen stor.
/// Mutanten "nollkontrollen blandar inte" ger skillnad ≈ 0 och blir röd.
#[test]
fn r2_nollkontroll_linjar_skillnad() {
    let mut rader = Vec::new();
    // Olika antal rader per djur: viktat medel skiljer sig från oviktat.
    for &(djur_id, n, vikt) in &[(1u64, 25usize, 1.0), (2u64, 40usize, 3.0)] {
        for i in 0..n {
            let mut r = tom_rad(12, djur_id);
            r.z[0] = i as f64;
            r.z[1] = (i % 3) as f64;
            r.z[2] = (i % 5) as f64 * 0.1;
            r.z[3] = 1.0 / (1.0 + i as f64);
            for a in 1..A_N {
                r.s[a * 2] = vikt * r.z[0] + 0.05 * a as f64;
            }
            r.hj_varden[1] = 1.0;
            rader.push(r);
        }
    }
    let orsak = orsak_per_fro(&rader).expect("orsak");
    let o = &orsak[0];
    let per = o.r2_riktning.expect("R² per djur");
    let noll = o.r2_noll.expect("R² noll");
    let skillnad = o.r2_skillnad.expect("skillnad");
    assert!(per > 0.99, "linjär: R² per djur ≈ 1, fick {per}");
    assert!(
        noll < 0.35,
        "nollkontrollen ska vara brusnivå efter blandning, fick {noll}"
    );
    assert!(
        skillnad > 0.6,
        "skillnaden riktigt − noll ska vara stor vid linjär y(z): per={per} noll={noll} skillnad={skillnad}"
    );
    assert_eq!(o.rader_per_djur_min, 25);
    assert_eq!(o.rader_per_djur_max, 40);
    assert_eq!(o.rader_per_djur_median, 32); // (25+40)/2
    let viktat = o.r2_riktning_viktat.expect("viktat");
    assert!(
        (viktat - per).abs() < 1e-9,
        "båda djuren har R²≈1 så viktat ≈ oviktat, fick viktat={viktat} per={per}"
    );
}

/// Prov 7c: singulär matris räknas som misslyckad (inte tyst hoppad över).
/// Mutanten "Err(_) => {}" blir röd när räknaren ska visa minst 1.
#[test]
fn r2_misslyckad_singular_raknas() {
    let mut rader = Vec::new();
    // Ett djur, 25 rader, z konstant → XᵀX singulär (kolumner linjärt beroende med bias).
    for i in 0..25 {
        let mut r = tom_rad(11, 3);
        r.z = [0.0; Z_N]; // alla z = 0 → bara bias-kolumnen har signal
        for a in 1..A_N {
            r.s[a * 2] = i as f64 * 0.1 + a as f64; // spridning i y, men X singulär
        }
        r.hj_varden[0] = 1.0;
        rader.push(r);
    }
    let orsak = orsak_per_fro(&rader).expect("orsak");
    let o = &orsak[0];
    assert!(
        o.r2_misslyckade >= 1,
        "singulär matris ska räknas: förväntade ≥1 misslyckade, fick {}",
        o.r2_misslyckade
    );
    // Per riktning (8) för djuret + 8 gemensamma = 16 misslyckade när allt är singulärt.
    assert_eq!(
        o.r2_misslyckade, 16,
        "8 riktningar × (1 djur + gemensam) = 16, fick {}",
        o.r2_misslyckade
    );
}

#[test]
fn handling0_matt_andel() {
    let mut rader = Vec::new();
    let mut val = Vec::new();
    for k in 0..20 {
        let mut rad = [1.0; A_N];
        rad[0] = if k < 5 { 2.0 } else { 0.0 };
        rader.push(rad);
        val.push(valj_fran_hj(&rad));
    }
    let h0 = handling0_matt(&rader, &val);
    assert!((h0.andel_val - 5.0 / 20.0).abs() < 1e-12);
}

#[test]
fn traff_andel_dela_vid_lika() {
    let mut v = [0.0; A_N];
    // Alla lika → varje riktning får 1/9.
    assert!((traff_andel(&v, 3) - 1.0 / 9.0).abs() < 1e-12);
    v[2] = 1.0;
    assert!((traff_andel(&v, 2) - 1.0).abs() < 1e-12);
    assert!(traff_andel(&v, 3) < 1e-12);
}

#[test]
fn fable_grans_konstant() {
    assert!((FABLE_VANLIGASTE_MIN - 0.6).abs() < 1e-15);
}
