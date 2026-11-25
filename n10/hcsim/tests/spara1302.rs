//! Kontroller 1–8 för sparat läge (Alt 1302). Underlag:
//! `n10/docs/2026-11-21-lasunderlag-spara1302.md`.
//! Fortsättning: prov 2–3 i egen process (+ kill -9), prov 5 med vägsamlare.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use hckarta::{kontrollsumma, Karta, Lager, Marke, Omrade, Period, Vatten};
use hcsim::formaga::Formaga;
use hcsim::rng::Rng;
use hcsim::sim::{BokforingBlock, FormagaMatning, LedLoggRad, MarkLage, SignalLoggRad};
use hcsim::spara::{
    kor_fler_ar, kor_med_spara, ladda, ladda_bytes, spara, spara_utan_g_PROV, KorParametrar,
};
use hcsim::tal;
use hcsim::vagsamlare::{kanonisk, samla_vagar};
use hcsim::Sim;
use serde::Serialize;

/// Världens summa efter 40 år. Byttes 2026-11-22 (Alt 1624, flodrutan blir flodkant med växter; underlaget
/// `n10/docs/2026-11-22-lasunderlag-flodkant1624.md`): förut 4_211_610_881_721_223_074, med landandel 0 i flodrutor.
const DJUR_SUMMA_40: u64 = 12_501_195_664_153_507_673;

fn target_dir() -> PathBuf {
    PathBuf::from(option_env!("CARGO_TARGET_DIR").unwrap_or("target")).join("spara1302-prov")
}

fn hcsim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_hcsim"))
}

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

fn liten_varld(fro: u64, n0: usize) -> (Karta, Sim, KorParametrar) {
    let b = 8;
    let h = 8;
    let mut vatten = vec![Vatten::Land; b * h];
    vatten[0] = Vatten::Hav;
    let kart = konstruerad(b, h, vatten, 0.5);
    let sim = Sim::fran_karta(&kart, fro, n0);
    let parametrar = KorParametrar {
        lat0: 47.0,
        lon0: -108.0,
        b,
        h,
        ruta_m: 2000.0,
        t: 500_000.0,
        fro,
        n0,
        karta_summa: 0,
    };
    (kart, sim, parametrar)
}

fn n10_param() -> (f64, f64, usize, usize, f64, f64, u64, i64, usize) {
    (47.0, -108.0, 64, 48, 2000.0, 500_000.0, 181, 40, 400)
}

fn bygg_n10_karta() -> (Karta, u64) {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let paleo = rot.join("../paleokarta/data");
    let tt_text = fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).expect("tidtabell");
    let tt = hckarta::Tidtabell::las(&tt_text).expect("tt");
    let kalla = hckarta::Kalla::las(&paleo).expect("källa");
    let (lat0, lon0, b, h, ruta_m, t, fro, _, _) = n10_param();
    let omr = Omrade {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
    };
    let kart = hckarta::karta_full(&kalla, &tt, &omr, t, fro).expect("karta");
    let summa = kontrollsumma(&kart);
    (kart, summa)
}

fn n10_kor_args(ar: i64, spara: Option<&Path>, spara_var_ar: Option<i64>) -> Vec<String> {
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let mut a = vec![
        "kor".into(),
        "--lat0".into(),
        lat0.to_string(),
        "--lon0".into(),
        lon0.to_string(),
        "--b".into(),
        b.to_string(),
        "--h".into(),
        h.to_string(),
        "--ruta-m".into(),
        ruta_m.to_string(),
        "--t".into(),
        t.to_string(),
        "--fro".into(),
        fro.to_string(),
        "--ar".into(),
        ar.to_string(),
        "--n0".into(),
        n0.to_string(),
        "--var-ar".into(),
        "10".into(),
    ];
    if let Some(p) = spara {
        a.push("--spara".into());
        a.push(p.to_string_lossy().into());
    }
    if let Some(n) = spara_var_ar {
        a.push("--spara-var-ar".into());
        a.push(n.to_string());
    }
    a
}

fn las_djur_summa(ut: &str) -> u64 {
    for rad in ut.lines().rev() {
        let t = rad.trim();
        if let Some(i) = t.find("\"djur_summa\":") {
            let rest = &t[i + "\"djur_summa\":".len()..];
            let tal: String = rest
                .chars()
                .skip_while(|c| c.is_whitespace() || *c == ':')
                .take_while(|c| c.is_ascii_digit())
                .collect();
            return tal.parse().unwrap_or_else(|_| panic!("kunde inte läsa djur_summa ur: {rad}"));
        }
    }
    panic!("ingen djur_summa i utskrift:\n{ut}");
}

fn kor_hcsim(args: &[String]) -> String {
    let ut = Command::new(hcsim_bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("starta hcsim: {e}"));
    let stdout = String::from_utf8_lossy(&ut.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&ut.stderr).into_owned();
    assert!(
        ut.status.success(),
        "hcsim misslyckades ({:?}):\n{stderr}\n{stdout}",
        ut.status
    );
    stdout
}

/// Diagnos: spara/ladda utan fler år ska ge bit-identiskt läge.
#[test]
fn diagnos_spara_ladda_bitexakt() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("bitexakt.hcsim");
    let (_, mut sim, mut parametrar) = liten_varld(42, 20);
    for _ in 0..(tal::STEG_PER_AR * 5) {
        sim.steg();
    }
    parametrar.karta_summa = 7;
    let summa_fore = sim.djur_summa();
    let g_fore = sim.g.tillstand();
    let t_fore = sim.t;
    let v0_fore = sim.v[1].to_bits();
    spara(&fil, &parametrar, &sim).unwrap();
    let (_, sim2) = ladda(&fil).unwrap();
    assert_eq!(sim2.t, t_fore);
    assert_eq!(sim2.g.tillstand(), g_fore);
    assert_eq!(sim2.v[1].to_bits(), v0_fore, "v[1] bitar");
    assert_eq!(sim.djur.len(), sim2.djur.len());
    for (i, (a, b)) in sim.djur.iter().zip(sim2.djur.iter()).enumerate() {
        assert_eq!(a.uid, b.uid, "uid {i}");
        assert_eq!(a.lev, b.lev, "lev {i}");
        assert_eq!(a.e.to_bits(), b.e.to_bits(), "e {i}");
        assert_eq!(a.hj.gen.len(), b.hj.gen.len(), "gen len {i}");
        for (j, (x, y)) in a.hj.gen.iter().zip(b.hj.gen.iter()).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(), "gen[{i}][{j}] {x} vs {y}");
        }
        for (j, (x, y)) in a.hj.wp.iter().zip(b.hj.wp.iter()).enumerate() {
            assert_eq!(x.to_bits(), y.to_bits(), "wp[{i}][{j}]");
        }
    }
    assert_eq!(sim2.djur_summa(), summa_fore, "djur_summa efter laddning");
    let mut a = sim;
    let mut b = sim2;
    kor_fler_ar(&mut a, 1, 1);
    kor_fler_ar(&mut b, 1, 1);
    assert_eq!(a.djur_summa(), b.djur_summa(), "efter ett år till");
}

/// Prov 1: dagens körning oförändrad (samma djur_summa som N10/mint).
#[test]
fn kontroll1_dagens_korning_oforandrad() {
    let (kart, summa_fore) = bygg_n10_karta();
    let (_, _, _, _, _, _, fro, ar, n0) = n10_param();
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    let _rader = sim.kor_ar(ar, 10);
    assert_eq!(kontrollsumma(&kart), summa_fore);
    assert_eq!(
        sim.djur_summa(),
        DJUR_SUMMA_40,
        "djur_summa ska vara oförändrad utan --spara"
    );
}

/// Prov 2: rundtur 20+20 via **nya processer**, samma summa och slutläge som 40 i ett svep.
#[test]
fn kontroll2_rundtur() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil_del = dir.join("rundtur-del.hcsim");
    let fil_hel = dir.join("rundtur-hel.hcsim");
    let _ = fs::remove_file(&fil_del);
    let _ = fs::remove_file(&fil_hel);

    let ut_hel = kor_hcsim(&n10_kor_args(40, Some(&fil_hel), None));
    assert_eq!(las_djur_summa(&ut_hel), DJUR_SUMMA_40);
    assert!(fil_hel.exists());

    let _ut20 = kor_hcsim(&n10_kor_args(20, Some(&fil_del), None));
    assert!(fil_del.exists());

    let ut_fort = kor_hcsim(&[
        "fortsatt".into(),
        "--fran".into(),
        fil_del.to_string_lossy().into(),
        "--ar".into(),
        "20".into(),
        "--var-ar".into(),
        "10".into(),
    ]);
    assert_eq!(
        las_djur_summa(&ut_fort),
        DJUR_SUMMA_40,
        "rundtur i ny process ska ge samma djur_summa"
    );

    let a = fs::read(&fil_hel).unwrap();
    let b = fs::read(&fil_del).unwrap();
    assert_eq!(a, b, "slutläge efter rundtur ska vara byte för byte lika");
}

/// Prov 3: `kill -9` mitt i en sparperiod; fortsatt till 40 ger samma summa/läge; `.forra` går att ladda.
#[test]
fn kontroll3_avbrott() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("avbrott.hcsim");
    let forra = PathBuf::from(format!("{}.forra", fil.display()));
    let fil_hel = dir.join("avbrott-hel.hcsim");
    let _ = fs::remove_file(&fil);
    let _ = fs::remove_file(&forra);
    let _ = fs::remove_file(&fil_hel);

    // Referens: 40 år i egen process.
    let ut_hel = kor_hcsim(&n10_kor_args(40, Some(&fil_hel), None));
    assert_eq!(las_djur_summa(&ut_hel), DJUR_SUMMA_40);

    let mut barn = Command::new(hcsim_bin())
        .args(n10_kor_args(40, Some(&fil), Some(5)))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("starta hcsim kor med spara-var-ar");

    // Vänta tills filen skrivits minst två gånger (då finns FIL.forra).
    let t0 = Instant::now();
    loop {
        if forra.exists() && fil.exists() {
            // Mitt i nästa period (mellan sparningar var 5:e år).
            thread::sleep(Duration::from_secs(5));
            let _ = barn.kill(); // SIGKILL på Unix
            let _ = barn.wait();
            break;
        }
        match barn.try_wait() {
            Ok(Some(st)) => panic!("hcsim avslutade före två sparningar: {st:?}"),
            Ok(None) => {}
            Err(e) => panic!("try_wait: {e}"),
        }
        assert!(
            t0.elapsed() < Duration::from_secs(600),
            "timeout väntar på två sparningar"
        );
        thread::sleep(Duration::from_millis(200));
    }

    assert!(fil.exists(), "sparfil saknas efter kill");
    let (p, sim_bruten) = ladda(&fil).unwrap();
    let start_ar = sim_bruten.t / tal::STEG_PER_AR;
    assert!(start_ar >= 10, "förväntade minst år 10 efter två sparningar, fick {start_ar}");
    let kvar = 40 - start_ar;
    assert!(kvar > 0);

    // .forra ska gå att ladda.
    let (_p_forra, sim_forra) = ladda(&forra).unwrap();
    assert!(
        sim_forra.t / tal::STEG_PER_AR < start_ar,
        ".forra ska vara äldre generation"
    );

    let ut = kor_hcsim(&[
        "fortsatt".into(),
        "--fran".into(),
        fil.to_string_lossy().into(),
        "--ar".into(),
        kvar.to_string(),
        "--var-ar".into(),
        "10".into(),
        "--spara".into(),
        dir.join("avbrott-slut.hcsim").to_string_lossy().into(),
    ]);
    assert_eq!(las_djur_summa(&ut), DJUR_SUMMA_40);
    assert_eq!(
        fs::read(dir.join("avbrott-slut.hcsim")).unwrap(),
        fs::read(&fil_hel).unwrap(),
        "slutläge efter kill+fortsatt ska vara byte-lika med obruten 40"
    );
    let _ = p; // parametrar laddade ok
}

/// Prov 4: osparad `g` (PROV-flagga) ger annan summa.
#[test]
#[allow(non_snake_case)]
fn kontroll4_osparad_g_PROV() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil_ok = dir.join("med-g.hcsim");
    let fil_utan = dir.join("utan-g.hcsim");

    let (kart, karta_summa) = bygg_n10_karta();
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa,
    };

    let mut sim = Sim::fran_karta(&kart, fro, n0);
    kor_med_spara(&mut sim, 10, 10, Some(&fil_ok), 10, &parametrar).unwrap();
    spara_utan_g_PROV(&fil_utan, &parametrar, &sim).unwrap();

    let (_, mut a) = ladda(&fil_ok).unwrap();
    let (_, mut b) = ladda(&fil_utan).unwrap();
    kor_fler_ar(&mut a, 10, 10);
    kor_fler_ar(&mut b, 10, 10);
    assert_ne!(
        a.djur_summa(),
        b.djur_summa(),
        "utan sparad g ska slutsumman skilja"
    );
}

/// Stickprov: första, mitten, sista index (+ unika).
fn stickprov_idx(n: usize) -> Vec<usize> {
    if n == 0 {
        return vec![];
    }
    let mut s = BTreeSet::new();
    s.insert(0);
    s.insert(n / 2);
    s.insert(n - 1);
    s.into_iter().collect()
}

fn djur_stickprov(bas: &Sim) -> Vec<usize> {
    let mut idx = BTreeSet::new();
    let levande: Vec<usize> = bas
        .djur
        .iter()
        .enumerate()
        .filter(|(_, d)| d.lev)
        .map(|(i, _)| i)
        .collect();
    for i in stickprov_idx(levande.len()) {
        idx.insert(levande[i]);
    }
    if let Some(di) = bas.djur.iter().position(|d| !d.lev) {
        idx.insert(di);
    }
    // Om bara ett djur: ta det.
    if idx.is_empty() && !bas.djur.is_empty() {
        idx.insert(0);
    }
    idx.into_iter().collect()
}

/// Bygger mutationer som täcker serialiseringens vägar (stickprov för listor).
fn bygg_mutationer(bas: &Sim) -> Vec<(String, Box<dyn Fn(&mut Sim)>)> {
    let mut m: Vec<(String, Box<dyn Fn(&mut Sim)>)> = Vec::new();
    let push = |m: &mut Vec<(String, Box<dyn Fn(&mut Sim)>)>,
                namn: String,
                f: Box<dyn Fn(&mut Sim)>| { m.push((namn, f)); };

    push(&mut m, "t".into(), Box::new(|s| s.t += 1));
    push(&mut m, "g.s[]".into(), Box::new(|s| s.g = Rng::new(0)));
    push(
        &mut m,
        "tur_rng.s[]".into(),
        Box::new(|s| s.tur_rng = Rng::new(1)),
    );
    push(&mut m, "nasta_uid".into(), Box::new(|s| s.nasta_uid += 1));
    push(&mut m, "atervaxt".into(), Box::new(|s| s.atervaxt += 0.5));
    push(
        &mut m,
        "hav_stangt".into(),
        Box::new(|s| s.hav_stangt = !s.hav_stangt),
    );
    push(
        &mut m,
        "turordning".into(),
        Box::new(|s| s.turordning = !s.turordning),
    );
    push(
        &mut m,
        "markering_skedd".into(),
        Box::new(|s| s.markering_skedd = !s.markering_skedd),
    );
    push(&mut m, "ruta_m".into(), Box::new(|s| s.ruta_m += 1.0));
    push(
        &mut m,
        "b".into(),
        Box::new(|s| {
            s.b += 1;
            // Padda rutor så efterföljande steg inte indexerar utanför (bara fältprov).
            let n = s.b * s.h;
            while s.v.len() < n {
                s.v.push(0.0);
                s.vmax.push(0.0);
                s.landandel.push(0.0);
                s.vatten.push(Vatten::Land);
            }
        }),
    );
    push(
        &mut m,
        "h".into(),
        Box::new(|s| {
            s.h += 1;
            let n = s.b * s.h;
            while s.v.len() < n {
                s.v.push(0.0);
                s.vmax.push(0.0);
                s.landandel.push(0.0);
                s.vatten.push(Vatten::Land);
            }
        }),
    );
    push(&mut m, "rak.fodda".into(), Box::new(|s| s.rak.fodda += 1));
    push(
        &mut m,
        "rak.fodda_ar".into(),
        Box::new(|s| s.rak.fodda_ar += 1),
    );
    push(
        &mut m,
        "rak.dod_svalt".into(),
        Box::new(|s| s.rak.dod_svalt += 1),
    );
    push(
        &mut m,
        "rak.dod_svalt_ar".into(),
        Box::new(|s| s.rak.dod_svalt_ar += 1),
    );
    push(
        &mut m,
        "rak.dod_alder".into(),
        Box::new(|s| s.rak.dod_alder += 1),
    );
    push(
        &mut m,
        "rak.dod_alder_ar".into(),
        Box::new(|s| s.rak.dod_alder_ar += 1),
    );
    push(&mut m, "klimat.m".into(), Box::new(|s| s.klimat.m += 1.0));
    push(&mut m, "klimat.ar".into(), Box::new(|s| s.klimat.ar += 1.0));
    push(&mut m, "klimat.phi".into(), Box::new(|s| s.klimat.phi += 0.1));
    push(&mut m, "klimat.eps".into(), Box::new(|s| s.klimat.eps += 0.1));
    push(&mut m, "klimat.a".into(), Box::new(|s| s.klimat.a += 0.1));
    push(
        &mut m,
        "klimat.fas_tmax".into(),
        Box::new(|s| s.klimat.fas_tmax += 1.0),
    );
    push(
        &mut m,
        "klimat.vat_vinter".into(),
        Box::new(|s| s.klimat.vat_vinter = !s.klimat.vat_vinter),
    );
    push(
        &mut m,
        "klimat.w_torr".into(),
        Box::new(|s| s.klimat.w_torr += 0.1),
    );
    push(
        &mut m,
        "klimat.platt".into(),
        Box::new(|s| s.klimat.platt = !s.klimat.platt),
    );
    push(
        &mut m,
        "klimat.g_medel".into(),
        Box::new(|s| s.klimat.g_medel += 0.1),
    );
    push(
        &mut m,
        "matning.pa".into(),
        Box::new(|s| s.matning.pa = !s.matning.pa),
    );
    push(
        &mut m,
        "matning.per".into(),
        Box::new(|s| {
            s.matning
                .per
                .insert("lar".into(), FormagaMatning::default());
        }),
    );
    push(&mut m, "ledlogg.pa".into(), Box::new(|s| s.ledlogg.pa = !s.ledlogg.pa));
    push(&mut m, "ledlogg.fro".into(), Box::new(|s| s.ledlogg.fro ^= 1));
    push(
        &mut m,
        "ledlogg.var_n".into(),
        Box::new(|s| s.ledlogg.var_n += 1),
    );
    push(
        &mut m,
        "ledlogg.max_rader".into(),
        Box::new(|s| s.ledlogg.max_rader += 1),
    );
    push(
        &mut m,
        "ledlogg.djursteg".into(),
        Box::new(|s| s.ledlogg.djursteg += 1),
    );
    push(
        &mut m,
        "ledlogg.rader".into(),
        Box::new(|s| {
            s.ledlogg.rader.push(LedLoggRad {
                fro: 1,
                djur_id: 1,
                handling: 0,
                s: [0.0; tal::S_N],
                hj_varde: 0.0,
                hj_varden: [0.0; tal::A_N],
                intag_fore: 0.0,
                intag_efter: 0.0,
            });
        }),
    );
    push(
        &mut m,
        "ledlogg.pending".into(),
        Box::new(|s| {
            s.ledlogg.pending.push((
                0,
                LedLoggRad {
                    fro: 2,
                    djur_id: 2,
                    handling: 1,
                    s: [0.0; tal::S_N],
                    hj_varde: 0.0,
                    hj_varden: [0.0; tal::A_N],
                    intag_fore: 0.0,
                    intag_efter: 0.0,
                },
            ));
        }),
    );
    // Strykprovet V (Alt 1251–1253): lägena, signalloggen och bokföringen (tillagda vid ihopslagningen med main, Claude).
    push(&mut m, "flytt_i_tur".into(), Box::new(|s| s.flytt_i_tur = !s.flytt_i_tur));
    push(&mut m, "bete_i_tur".into(), Box::new(|s| s.bete_i_tur = !s.bete_i_tur));
    push(&mut m, "signal_logg.pa".into(), Box::new(|s| s.signal_logg.pa = !s.signal_logg.pa));
    push(&mut m, "signal_logg.fro".into(), Box::new(|s| s.signal_logg.fro ^= 1));
    push(&mut m, "signal_logg.var_n".into(), Box::new(|s| s.signal_logg.var_n += 1));
    push(&mut m, "signal_logg.max_rader".into(), Box::new(|s| s.signal_logg.max_rader += 1));
    push(&mut m, "signal_logg.djursteg".into(), Box::new(|s| s.signal_logg.djursteg += 1));
    push(&mut m, "signal_logg.ingrepp_mark".into(), Box::new(|s| s.signal_logg.ingrepp_mark = Some(0.5)));
    push(&mut m, "signal_logg.ingrepp_slut".into(), Box::new(|s| s.signal_logg.ingrepp_slut = Some(0.5)));
    push(
        &mut m,
        "signal_logg.rader".into(),
        Box::new(|s| {
            s.signal_logg.rader.push(SignalLoggRad {
                fro: 1,
                djur_id: 1,
                s: [0.0; tal::S_N],
                z: [0.0; tal::Z_N],
                hj_varden: [0.0; tal::A_N],
                lage: [0.0; tal::A_N],
                fast: [0.0; tal::A_N],
            });
        }),
    );
    push(&mut m, "bokforing.pa".into(), Box::new(|s| s.bokforing.pa = !s.bokforing.pa));
    push(&mut m, "bokforing.fro".into(), Box::new(|s| s.bokforing.fro ^= 1));
    push(&mut m, "bokforing.arm".into(), Box::new(|s| s.bokforing.arm.push('x')));
    push(&mut m, "bokforing.t0".into(), Box::new(|s| s.bokforing.t0 += 1));
    push(&mut m, "bokforing.block_langd".into(), Box::new(|s| s.bokforing.block_langd += 1));
    push(&mut m, "bokforing.nasta_block".into(), Box::new(|s| s.bokforing.nasta_block += 1));
    // Alt 1462: kopieringsmätning (läser bara).
    push(
        &mut m,
        "kopier_matning.pa".into(),
        Box::new(|s| s.kopier_matning.pa = !s.kopier_matning.pa),
    );
    push(
        &mut m,
        "kopier_matning.tot.forutsag_sum".into(),
        Box::new(|s| s.kopier_matning.tot.forutsag_sum += 1.0),
    );
    push(
        &mut m,
        "kopier_matning.tot.kopiering_sum".into(),
        Box::new(|s| s.kopier_matning.tot.kopiering_sum += 1.0),
    );
    push(
        &mut m,
        "kopier_matning.tot.fast_sum".into(),
        Box::new(|s| s.kopier_matning.tot.fast_sum += 1.0),
    );
    push(
        &mut m,
        "kopier_matning.tot.n".into(),
        Box::new(|s| s.kopier_matning.tot.n += 1),
    );
    push(
        &mut m,
        "kopier_matning.per_ar".into(),
        Box::new(|s| {
            s.kopier_matning
                .per_ar
                .insert(0, hcsim::KopierArAck::default());
        }),
    );
    // Alt 1463: utforskningsmätning (läser bara).
    push(
        &mut m,
        "utforsk_matning.pa".into(),
        Box::new(|s| s.utforsk_matning.pa = !s.utforsk_matning.pa),
    );
    push(
        &mut m,
        "utforsk_matning.kopier_per".into(),
        Box::new(|s| {
            s.utforsk_matning
                .kopier_per
                .insert("lar".into(), hcsim::KopierArAck::default());
        }),
    );
    push(
        &mut m,
        "utforsk_matning.aldrig_per".into(),
        Box::new(|s| {
            s.utforsk_matning
                .aldrig_per
                .insert("lar".into(), hcsim::AldrigAck::default());
        }),
    );
    push(
        &mut m,
        "utforsk_matning.eps_per_ar".into(),
        Box::new(|s| {
            s.utforsk_matning
                .eps_per_ar
                .insert(40, hcsim::EpsArAck::default());
        }),
    );
    push(
        &mut m,
        "utforsk_matning.bokforda_uid".into(),
        Box::new(|s| s.utforsk_matning.bokforda_uid.push(1)),
    );
    push(
        &mut m,
        "bokforing.antal_markering".into(),
        Box::new(|s| {
            s.bokforing.antal_markering.insert("lar".into(), 1);
        }),
    );
    push(
        &mut m,
        "bokforing.fodda_tot".into(),
        Box::new(|s| {
            s.bokforing.fodda_tot.insert("lar".into(), 1);
        }),
    );
    push(
        &mut m,
        "bokforing.dod_tot".into(),
        Box::new(|s| {
            s.bokforing.dod_tot.insert("lar".into(), 1);
        }),
    );
    // `ack` är privat; den nås bara genom `lagg_intag`, som kräver `pa` (mutationen ändrar därför också `pa`).
    push(
        &mut m,
        "bokforing.ack".into(),
        Box::new(|s| {
            s.bokforing.pa = true;
            s.bokforing.lagg_intag(Formaga::Lar, 1.0);
        }),
    );
    push(
        &mut m,
        "bokforing.block".into(),
        Box::new(|s| {
            s.bokforing.block.push(BokforingBlock {
                fro: 1,
                arm: "x".into(),
                formaga: "lar".into(),
                block: 1,
                fodda: 0,
                dod_svalt: 0,
                dod_alder: 0,
                antal_slut: 0,
                intag_sum: 0.0,
                djursteg: 0,
            });
        }),
    );
    push(
        &mut m,
        "fel_tidigt_markt.sum".into(),
        Box::new(|s| s.fel_tidigt_markt.sum += 1.0),
    );
    push(
        &mut m,
        "fel_tidigt_markt.n".into(),
        Box::new(|s| s.fel_tidigt_markt.n += 1),
    );
    push(
        &mut m,
        "fel_tidigt_omarkt.sum".into(),
        Box::new(|s| s.fel_tidigt_omarkt.sum += 1.0),
    );
    push(
        &mut m,
        "fel_tidigt_omarkt.n".into(),
        Box::new(|s| s.fel_tidigt_omarkt.n += 1),
    );
    push(
        &mut m,
        "fel_sent_markt.sum".into(),
        Box::new(|s| s.fel_sent_markt.sum += 1.0),
    );
    push(
        &mut m,
        "fel_sent_markt.n".into(),
        Box::new(|s| s.fel_sent_markt.n += 1),
    );
    push(
        &mut m,
        "fel_sent_omarkt.sum".into(),
        Box::new(|s| s.fel_sent_omarkt.sum += 1.0),
    );
    push(
        &mut m,
        "fel_sent_omarkt.n".into(),
        Box::new(|s| s.fel_sent_omarkt.n += 1),
    );
    push(
        &mut m,
        "mark_lage".into(),
        Box::new(|s| {
            s.mark_lage = match s.mark_lage {
                MarkLage::Utan => MarkLage::Noll,
                MarkLage::Noll => MarkLage::Positiv,
                MarkLage::Positiv => MarkLage::Utan,
            };
        }),
    );
    push(
        &mut m,
        "hjarn_arm".into(),
        Box::new(|s| {
            s.hjarn_arm = Some(hcsim::HjarnArm {
                namn: "arm1",
                markta: Formaga::Lar,
                omarkta: Formaga::Frusen,
                spegel: false,
            });
        }),
    );
    push(
        &mut m,
        "senaste_turordning".into(),
        Box::new(|s| s.senaste_turordning.push(42)),
    );

    for i in stickprov_idx(bas.v.len()) {
        push(
            &mut m,
            format!("v[{i}]"),
            Box::new(move |s| s.v[i] += 1.0),
        );
    }
    for i in stickprov_idx(bas.vmax.len()) {
        push(
            &mut m,
            format!("vmax[{i}]"),
            Box::new(move |s| s.vmax[i] += 1.0),
        );
    }
    for i in stickprov_idx(bas.landandel.len()) {
        push(
            &mut m,
            format!("landandel[{i}]"),
            Box::new(move |s| s.landandel[i] += 0.1),
        );
    }
    for i in stickprov_idx(bas.vatten.len()) {
        push(
            &mut m,
            format!("vatten[{i}]"),
            Box::new(move |s| {
                s.vatten[i] = if s.vatten[i] == Vatten::Land {
                    Vatten::Hav
                } else {
                    Vatten::Land
                };
            }),
        );
    }

    for i in djur_stickprov(bas) {
        push(
            &mut m,
            format!("djur[{i}].e"),
            Box::new(move |s| s.djur[i].e += 1.0),
        );
        push(
            &mut m,
            format!("djur[{i}].age"),
            Box::new(move |s| s.djur[i].age += 1),
        );
        push(
            &mut m,
            format!("djur[{i}].i"),
            Box::new(move |s| s.djur[i].i ^= 1),
        );
        push(
            &mut m,
            format!("djur[{i}].j"),
            Box::new(move |s| s.djur[i].j ^= 1),
        );
        push(
            &mut m,
            format!("djur[{i}].hona"),
            Box::new(move |s| s.djur[i].hona = !s.djur[i].hona),
        );
        push(
            &mut m,
            format!("djur[{i}].markt"),
            Box::new(move |s| s.djur[i].markt = !s.djur[i].markt),
        );
        push(
            &mut m,
            format!("djur[{i}].lev"),
            Box::new(move |s| s.djur[i].lev = !s.djur[i].lev),
        );
        push(
            &mut m,
            format!("djur[{i}].uid"),
            Box::new(move |s| s.djur[i].uid ^= 0xffff),
        );
        push(
            &mut m,
            format!("djur[{i}].formaga"),
            Box::new(move |s| {
                s.djur[i].formaga = match s.djur[i].formaga {
                    Formaga::Lar => Formaga::Frusen,
                    _ => Formaga::Lar,
                };
            }),
        );
        push(
            &mut m,
            format!("djur[{i}].egen_rng.s[]"),
            Box::new(move |s| s.djur[i].egen_rng = Rng::new(999)),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.steg"),
            Box::new(move |s| s.djur[i].hj.steg += 1),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.senaste_fel"),
            Box::new(move |s| s.djur[i].hj.senaste_fel = Some(1.0)),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.senaste_fel_handling"),
            Box::new(move |s| s.djur[i].hj.senaste_fel_handling = Some(3)),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.forra_ba"),
            Box::new(move |s| s.djur[i].hj.forra_ba = Some(vec![0.3; tal::Z_N])),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.forra_ba[]"),
            Box::new(move |s| {
                if let Some(ba) = &mut s.djur[i].hj.forra_ba {
                    if !ba.is_empty() {
                        ba[0] += 1.0;
                    }
                } else {
                    s.djur[i].hj.forra_ba = Some(vec![1.0; tal::Z_N]);
                }
            }),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.senaste_fel_kopiering"),
            Box::new(move |s| s.djur[i].hj.senaste_fel_kopiering = Some(2.0)),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.senaste_fel_fast"),
            Box::new(move |s| s.djur[i].hj.senaste_fel_fast = Some(3.0)),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.fel_sum[]"),
            Box::new(move |s| s.djur[i].hj.fel_sum[0] += 1.0),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.fel_n[]"),
            Box::new(move |s| s.djur[i].hj.fel_n[0] += 1),
        );
        // forra: None-löv och Some-innehåll (forra[]) — båda namnen för täckning.
        push(
            &mut m,
            format!("djur[{i}].hj.forra"),
            Box::new(move |s| {
                s.djur[i].hj.forra = Some((vec![0.1; tal::Z_N], 0, vec![0.2; tal::Z_N]));
            }),
        );
        push(
            &mut m,
            format!("djur[{i}].hj.forra[]"),
            Box::new(move |s| {
                if let Some((zf, a, yf)) = &mut s.djur[i].hj.forra {
                    if !zf.is_empty() {
                        zf[0] += 1.0;
                    }
                    *a += 1;
                    if !yf.is_empty() {
                        yf[0] += 1.0;
                    }
                } else {
                    s.djur[i].hj.forra = Some((vec![1.0; tal::Z_N], 1, vec![1.0; tal::Z_N]));
                }
            }),
        );
        if !bas.djur[i].hj.gen.is_empty() {
            for gi in stickprov_idx(bas.djur[i].hj.gen.len()) {
                push(
                    &mut m,
                    format!("djur[{i}].hj.gen[{gi}]"),
                    Box::new(move |s| s.djur[i].hj.gen[gi] += 1.0),
                );
            }
        }
        if !bas.djur[i].hj.wp.is_empty() {
            for wi in stickprov_idx(bas.djur[i].hj.wp.len()) {
                push(
                    &mut m,
                    format!("djur[{i}].hj.wp[{wi}]"),
                    Box::new(move |s| s.djur[i].hj.wp[wi] += 1.0),
                );
            }
        }
        push(
            &mut m,
            format!("djur[{i}].forra_intag"),
            Box::new(move |s| s.djur[i].forra_intag = Some(1.0)),
        );
        // Alt 1463: ε och aldrig-valda-räknare.
        push(
            &mut m,
            format!("djur[{i}].eps"),
            Box::new(move |s| s.djur[i].eps = (s.djur[i].eps + 0.1).clamp(0.0, 1.0)),
        );
        push(
            &mut m,
            format!("djur[{i}].steg_efter_mark"),
            Box::new(move |s| s.djur[i].steg_efter_mark += 1),
        );
        for hi in stickprov_idx(bas.djur[i].hj_handlingar.len()) {
            push(
                &mut m,
                format!("djur[{i}].hj_handlingar[{hi}]"),
                Box::new(move |s| s.djur[i].hj_handlingar[hi] += 1),
            );
        }
        for ti in stickprov_idx(bas.djur[i].tabell.medel.len()) {
            push(
                &mut m,
                format!("djur[{i}].tabell.medel[{ti}]"),
                Box::new(move |s| s.djur[i].tabell.medel[ti] += 1.0),
            );
        }
        push(
            &mut m,
            format!("djur[{i}].tabell.forra_handling"),
            Box::new(move |s| s.djur[i].tabell.forra_handling = Some(2)),
        );
        push(
            &mut m,
            format!("djur[{i}].tabell.slumphandlingar"),
            Box::new(move |s| s.djur[i].tabell.slumphandlingar += 1),
        );
        push(
            &mut m,
            format!("djur[{i}].lasare.u0"),
            Box::new(move |s| s.djur[i].lasare.u0 += 1.0),
        );
        for ui in stickprov_idx(bas.djur[i].lasare.u.len()) {
            push(
                &mut m,
                format!("djur[{i}].lasare.u[{ui}]"),
                Box::new(move |s| s.djur[i].lasare.u[ui] += 1.0),
            );
        }
        push(
            &mut m,
            format!("djur[{i}].lasare.forra_y"),
            Box::new(move |s| s.djur[i].lasare.forra_y = Some(vec![0.5; tal::Z_N])),
        );
    }
    m
}

/// Uttryckliga undantag: kanonisk väg → skäl (måste finnas i kommentar här).
fn undantag_vagar() -> Vec<(&'static str, &'static str)> {
    vec![
        // Privata fält i `Klimat` (synliga i serialiseringen men inte från integrationstest).
        (
            "klimat.q_medel",
            "privat fält i klimat-modulen; ingen pub-mutator tillåten utanför derive",
        ),
        (
            "klimat.g_norm",
            "privat fält i klimat-modulen; ingen pub-mutator tillåten utanför derive",
        ),
    ]
}

fn assert_vag_tackning(vagar: &[String], mutationer: &[(String, Box<dyn Fn(&mut Sim)>)]) {
    let tackta: HashSet<String> = mutationer.iter().map(|(n, _)| kanonisk(n)).collect();
    let und: HashSet<&str> = undantag_vagar().iter().map(|(v, _)| *v).collect();
    // Dokumentera skälen så de inte är tomma etiketter.
    for (v, skäl) in undantag_vagar() {
        assert!(!skäl.is_empty(), "undantag {v} saknar skäl");
    }
    let mut saknade = Vec::new();
    for vag in vagar {
        if !tackta.contains(vag) && !und.contains(vag.as_str()) {
            saknade.push(vag.clone());
        }
    }
    assert!(
        saknade.is_empty(),
        "fältvägar ur serialiseringen saknas i mutationslistan (och undantag): {saknade:?}"
    );
}

/// Prov 5: varje fältväg ur serialiseringen prövas; döda fält redovisas.
#[test]
fn kontroll5_varje_falt() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("faltprov.hcsim");

    let (_, mut sim, mut parametrar) = liten_varld(99, 12);
    for _ in 0..(tal::STEG_PER_AR * 3) {
        sim.steg();
    }
    parametrar.karta_summa = 1;
    spara(&fil, &parametrar, &sim).unwrap();
    let (_, bas) = ladda(&fil).unwrap();

    let vagar = samla_vagar(&bas).expect("vägsamlare");
    assert!(!vagar.is_empty(), "serialiseringen ska ge minst en väg");

    let (_, mut ref_sim) = ladda(&fil).unwrap();
    kor_fler_ar(&mut ref_sim, 2, 1);
    let ref_summa = ref_sim.djur_summa();

    let mutationer = bygg_mutationer(&bas);
    assert_vag_tackning(&vagar, &mutationer);

    let mut doda = Vec::new();
    let mut levande_n = 0usize;
    for (namn, mutera) in &mutationer {
        let mut s = bas.clone();
        mutera(&mut s);
        let tmp = dir.join("falt-mut.hcsim");
        spara(&tmp, &parametrar, &s).unwrap();
        let (_, mut s2) = ladda(&tmp).unwrap();
        let ut = panic::catch_unwind(AssertUnwindSafe(|| {
            kor_fler_ar(&mut s2, 2, 1);
            s2.djur_summa()
        }));
        match ut {
            Ok(summa) if summa == ref_summa => {
                let tmp2 = dir.join("falt-mut-slut.hcsim");
                let tmp_ref = dir.join("falt-ref-slut.hcsim");
                let lika = panic::catch_unwind(AssertUnwindSafe(|| {
                    spara(&tmp2, &parametrar, &s2).unwrap();
                    spara(&tmp_ref, &parametrar, &ref_sim).unwrap();
                    fs::read(&tmp2).unwrap() == fs::read(&tmp_ref).unwrap()
                }));
                if matches!(lika, Ok(true)) {
                    doda.push(namn.clone());
                } else {
                    levande_n += 1;
                }
            }
            _ => levande_n += 1,
        }
    }

    assert!(
        levande_n > 0,
        "minst något fält ska påverka slutsumman (annars är provet blint)"
    );
    let rapport = dir.join("doda-falt.txt");
    fs::write(
        &rapport,
        format!(
            "serialiserade_vagar: {}\nfältprovade: {}\nlevande: {}\ndöda:\n{}\n",
            vagar.len(),
            mutationer.len(),
            levande_n,
            doda.join("\n")
        ),
    )
    .unwrap();
    eprintln!(
        "kontroll5: {} vägar, {} mutationer, {} levande, {} döda — se {:?}",
        vagar.len(),
        mutationer.len(),
        levande_n,
        doda.len(),
        rapport
    );
}

/// Prov 6: fel version och fel kontrollsumma vägras.
#[test]
fn kontroll6_version_och_summa() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("version.hcsim");
    let (_, sim, parametrar) = liten_varld(7, 5);
    spara(&fil, &parametrar, &sim).unwrap();
    let mut bytes = fs::read(&fil).unwrap();

    bytes[4..8].copy_from_slice(&999u32.to_le_bytes());
    let err = ladda_bytes(&bytes).unwrap_err();
    assert!(
        err.contains("versionsnummer") || err.contains("version"),
        "fel version: {err}"
    );

    let mut bytes = fs::read(&fil).unwrap();
    if bytes.len() > 30 {
        bytes[30] ^= 0xff;
    }
    let err = ladda_bytes(&bytes).unwrap_err();
    assert!(
        err.contains("kontrollsumma"),
        "fel kontrollsumma: {err}"
    );
}

/// Prov 7: diff i hcsim mot main stannar vid spara/ladda (Fables krav 6).
/// Gäller bara grenen `hcsim-spara1302`; andra grenar hoppar över (Alt 1423-undantag).
#[test]
fn kontroll7_andringar_stannar_vid_spara() {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gren_ut = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&rot)
        .output()
        .expect("git rev-parse");
    let gren = String::from_utf8_lossy(&gren_ut.stdout).trim().to_string();
    if gren != "hcsim-spara1302" {
        eprintln!(
            "hoppar över kontroll7: grenen är «{gren}», inte hcsim-spara1302 (gäller bara sparat läge)"
        );
        return;
    }
    let ut = Command::new("git")
        .args(["diff", "--stat", "main", "--", "n10/hcsim"])
        .current_dir(&rot)
        .output()
        .expect("git");
    let text = String::from_utf8_lossy(&ut.stdout);
    for rad in text.lines() {
        let rad = rad.trim();
        if rad.is_empty() || rad.contains("changed") {
            continue;
        }
        let path = rad.split('|').next().unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        let ok = path.starts_with("n10/hcsim/")
            && (path.contains("spara")
                || path.contains("serdej")
                || path.contains("bitbin")
                || path.contains("vagsamlare")
                || path.ends_with("main.rs")
                || path.ends_with("lib.rs")
                || path.ends_with("Cargo.toml")
                || path.ends_with("Cargo.lock")
                || path.ends_with("LASMIG.md")
                || path.contains("tests/")
                || path.ends_with("sammanfattning.md")
                || path.ends_with("rng.rs")
                || path.ends_with("klimat.rs")
                || path.ends_with("hjarna.rs")
                || path.ends_with("formaga.rs")
                || path.ends_with("sim.rs"));
        assert!(ok, "otillåten diff-rad i hcsim: {rad}");
    }
}

/// Prov 8: tidigare prov (tal_tsv) fortfarande gröna.
#[test]
fn kontroll8_tal_tsv_oforandrad() {
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(rot.join("tal.tsv")).expect("tal.tsv");
    let rader = tal::las_tsv(&text).expect("parse tal.tsv");
    for namn in tal::KONSTANT_NAMN {
        let kod = tal::varde_i_kod(namn).expect("kod");
        let fil = rader
            .iter()
            .find(|r| r.namn == *namn)
            .unwrap_or_else(|| panic!("saknas i tsv: {namn}"));
        assert_eq!(fil.varde, kod, "tal {namn}");
    }
}

/// Mätning: storlek och tid för spara/ladda på N10-världen efter 5 år.
#[test]
fn matning_spara_ladda() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("matning.hcsim");
    let (kart, karta_summa) = bygg_n10_karta();
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa,
    };
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    sim.kor_ar(5, 5);
    let t0 = Instant::now();
    spara(&fil, &parametrar, &sim).unwrap();
    let tid_spara = t0.elapsed();
    let storlek = fs::metadata(&fil).unwrap().len();
    let t1 = Instant::now();
    let _ = ladda(&fil).unwrap();
    let tid_ladda = t1.elapsed();
    fs::write(
        dir.join("matning.txt"),
        format!(
            "storlek_bytes={storlek}\ntid_spara_ms={}\ntid_ladda_ms={}\n",
            tid_spara.as_millis(),
            tid_ladda.as_millis()
        ),
    )
    .unwrap();
}

/// Mätning: djurlistan växer (döda tas inte bort). Bara läsning — ingen retain/remove.
#[test]
fn matning_djurlistan_vaxer() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let (kart, karta_summa) = bygg_n10_karta();
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa,
    };
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    let milstolpar = [5i64, 10, 20, 40];
    let mut forra_ar = 0i64;
    let mut rader = Vec::new();
    for &ar in &milstolpar {
        let steg = (ar - forra_ar) * tal::STEG_PER_AR;
        let t0 = Instant::now();
        for _ in 0..steg {
            sim.steg();
        }
        let sek = t0.elapsed().as_secs_f64();
        let period_ar = (ar - forra_ar) as f64;
        let sek_per_ar = sek / period_ar;
        let fil = dir.join(format!("djurlista-{ar}.hcsim"));
        spara(&fil, &parametrar, &sim).unwrap();
        let storlek = fs::metadata(&fil).unwrap().len();
        let levande = sim.djur.iter().filter(|d| d.lev).count();
        rader.push(format!(
            "{ar}\t{}\t{levande}\t{storlek}\t{sek_per_ar:.4}",
            sim.djur.len()
        ));
        forra_ar = ar;
    }
    let text = format!(
        "ar\tdjur_len\tlevande\tstorlek_bytes\tsek_per_ar\n{}\n",
        rader.join("\n")
    );
    fs::write(dir.join("djurlistan-vaxer.tsv"), &text).unwrap();
    eprintln!("djurlistan växer:\n{text}");
}

/// Rött (med flit): fortsatt utan laddad slumptalskälla `g` ger annan summa.
#[test]
fn kontroll_rod_fortsatt_utan_g_visar_skillnad() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("rod-g.hcsim");
    let (kart, karta_summa) = bygg_n10_karta();
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa,
    };
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    kor_med_spara(&mut sim, 10, 10, Some(&fil), 10, &parametrar).unwrap();
    let (_, mut trasig) = ladda(&fil).unwrap();
    // Simulerar "fortsatt läser inte g ur filen".
    trasig.g = Rng::new(0);
    kor_fler_ar(&mut trasig, 30, 10);
    assert_ne!(
        trasig.djur_summa(),
        DJUR_SUMMA_40,
        "utan laddad g ska slutsumman skilja (detta är det röda felet)"
    );
}

/// Med flit röd: assert_eq mot summan efter trasig g — kör manuellt.
#[test]
#[ignore = "med flit röd: fortsatt utan g ska inte ge rätt summa"]
fn kontroll_rod_fortsatt_utan_g_assert_eq() {
    let dir = target_dir();
    fs::create_dir_all(&dir).unwrap();
    let fil = dir.join("rod-g2.hcsim");
    let (kart, karta_summa) = bygg_n10_karta();
    let (lat0, lon0, b, h, ruta_m, t, fro, _, n0) = n10_param();
    let parametrar = KorParametrar {
        lat0,
        lon0,
        b,
        h,
        ruta_m,
        t,
        fro,
        n0,
        karta_summa,
    };
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    kor_med_spara(&mut sim, 10, 10, Some(&fil), 10, &parametrar).unwrap();
    let (_, mut trasig) = ladda(&fil).unwrap();
    trasig.g = Rng::new(0);
    kor_fler_ar(&mut trasig, 30, 10);
    assert_eq!(trasig.djur_summa(), DJUR_SUMMA_40);
}

/// Rött: kopia av `Hj` med nytt serde-fält — vägsamlaren ser det, täckningskravet faller.
#[test]
fn kontroll5_rod_nytt_falt_i_kopia() {
    /// Kopia av `Hj` plus ett fält som signalens lagning (Alt 1301) skulle kunna lägga till.
    #[derive(Serialize)]
    struct HjMedExtra {
        gen: Vec<f64>,
        wp: Vec<f64>,
        forra: Option<(Vec<f64>, usize, Vec<f64>)>,
        forra_ba: Option<Vec<f64>>,
        fel_sum: [f64; 2],
        fel_n: [u64; 2],
        steg: u64,
        senaste_fel: Option<f64>,
        senaste_fel_handling: Option<usize>,
        senaste_fel_kopiering: Option<f64>,
        senaste_fel_fast: Option<f64>,
        signal_tmp: f64,
    }

    let (_, mut sim, _) = liten_varld(3, 4);
    for _ in 0..(tal::STEG_PER_AR) {
        sim.steg();
    }
    let h = &sim.djur[0].hj;
    let kopia = HjMedExtra {
        gen: h.gen.clone(),
        wp: h.wp.clone(),
        forra: h.forra.clone(),
        forra_ba: h.forra_ba.clone(),
        fel_sum: h.fel_sum,
        fel_n: h.fel_n,
        steg: h.steg,
        senaste_fel: h.senaste_fel,
        senaste_fel_handling: h.senaste_fel_handling,
        senaste_fel_kopiering: h.senaste_fel_kopiering,
        senaste_fel_fast: h.senaste_fel_fast,
        signal_tmp: 0.0,
    };
    let vagar = samla_vagar(&kopia).expect("vägar");
    assert!(
        vagar.iter().any(|v| v == "signal_tmp"),
        "nytt fält ska synas i vägarna: {vagar:?}"
    );

    // Mutationsnamn som speglar dagens Hj (utan signal_tmp), samma stil som djur[].hj.*.
    let mut_namn: Vec<String> = [
        "gen[]",
        "wp[]",
        "forra",
        "forra[]",
        "forra_ba",
        "forra_ba[]",
        "fel_sum[]",
        "fel_n[]",
        "steg",
        "senaste_fel",
        "senaste_fel_handling",
        "senaste_fel_kopiering",
        "senaste_fel_fast",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let tackta: HashSet<String> = mut_namn.iter().map(|n| kanonisk(n)).collect();
    let saknade: Vec<_> = vagar
        .iter()
        .filter(|v| !tackta.contains(*v))
        .cloned()
        .collect();
    assert!(
        saknade.iter().any(|v| v == "signal_tmp"),
        "väntade att signal_tmp saknas, fick {saknade:?}"
    );

    let panik = panic::catch_unwind(AssertUnwindSafe(|| {
        assert!(
            saknade.is_empty(),
            "fältvägar ur serialiseringen saknas i mutationslistan (och undantag): {saknade:?}"
        );
    }));
    let err = panik.expect_err("täckningskravet ska falla när nytt fält saknas");
    let msg = if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = err.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        format!("{err:?}")
    };
    assert!(
        msg.contains("signal_tmp"),
        "felmeddelandet ska nämna fältets väg, fick: {msg}"
    );
}

/// Med flit röda kontroller (körs inte i normal suite).
#[test]
#[ignore = "med flit röd: kör manuellt för att verifiera att kontrollerna fångar fel"]
fn med_flit_roda_mutationer() {
    assert_eq!(1u64, DJUR_SUMMA_40);
}
