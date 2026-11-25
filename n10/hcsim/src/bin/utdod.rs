//! utdod: mäter varför djuren dör ut i visningskörningen (Håkan 2026-11-22: "undersöker först varför djuren dog ut").
//! Kör samma värld som `hcsim kartbild` (armen e_lar, kartan 64 · 48 med eller utan bälte) och skriver en rad per år:
//! antal djur, vuxna, växternas fyllnad, djurens energi, födslar och dödsfall. Läser bara; ändrar inga regler.
//! Användning: utdod --kant vagg|torus --k 0|8 --fron 261-280 --ar 60 [--tradar 6] [--intag-fyllnad] [--hona-vuxen] [--fodsel-arlig] [--e-tak 300] [--e-skala 0.15625] [--mogen-steg 450] [--fodsel-troskel 120]
//! Rust (AGENTS.md regel 11).

use std::path::Path;

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::utforsk_armar;
use hcsim::sim::{KantLage, Sim};
use hcsim::tal::{E_DELA, LITEN, MARKT_INKOR_AR, STEG_PER_AR};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};

/// Årets mått ur världen.
pub fn ar_rad(sim: &Sim) -> String {
    let lev: Vec<_> = sim.djur.iter().filter(|d| d.lev).collect();
    let n = lev.len();
    let vuxna = lev.iter().filter(|d| d.age >= LITEN).count();
    let honor = lev.iter().filter(|d| d.hona).count();
    let redo = lev.iter().filter(|d| d.hona && d.e >= sim.fodsel_troskel.unwrap_or(E_DELA * sim.e_skala)).count();
    let e_medel = if n == 0 { 0.0 } else { lev.iter().map(|d| d.e).sum::<f64>() / n as f64 };
    let (mut v, mut tak, mut land, mut tomma) = (0.0, 0.0, 0usize, 0usize);
    for c in 0..sim.v.len() {
        if sim.vmax[c] > 0.0 {
            v += sim.v[c];
            tak += sim.vmax[c];
            land += 1;
            if sim.v[c] < 0.05 * sim.vmax[c] {
                tomma += 1;
            }
        }
    }
    // Trängsel: andelen av djuren som står i de 1 % av rutorna som har flest djur, och antalet rutor med djur.
    let mut per_ruta = vec![0usize; sim.v.len()];
    for d in &lev {
        per_ruta[sim.cell(d.i, d.j)] += 1;
    }
    let med_djur = per_ruta.iter().filter(|&&x| x > 0).count();
    per_ruta.sort_unstable_by(|a, b| b.cmp(a));
    let topp: usize = per_ruta.iter().take((sim.v.len() / 100).max(1)).sum();
    let trangsel = if n == 0 { 0.0 } else { topp as f64 / n as f64 };
    format!(
        "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.1}\t{}\t{}\t{}\t{:.2}\t{:.3}\t{}",
        n,
        vuxna,
        honor,
        redo,
        v / tak.max(1e-12),
        tomma as f64 / land.max(1) as f64,
        e_medel,
        sim.rak.fodda_ar,
        sim.rak.dod_svalt_ar,
        sim.rak.dod_alder_ar,
        lev.iter().filter_map(|d| d.forra_intag).fold(0.0, f64::max),
        trangsel,
        med_djur
    )
}

fn kor(rot: &Path, kant: KantLage, k: usize, fro: u64, ar: i64, vaxlar: (bool, bool, bool), e_tak: Option<f64>, e_skala: f64, mogen: Option<i64>, troskel: Option<f64>, liv: Option<i64>, atervaxt: f64) -> Result<Vec<String>, String> {
    let arm = utforsk_armar().into_iter().find(|a| a.namn == "e_lar" && !a.spegel).ok_or("armen e_lar saknas")?;
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    let mut sim = bygg_sim_balte(&kalla, &tt, B_FULL, H_FULL, k, fro, &arm)?;
    sim.kant = kant;
    sim.fonster_k = k;
    sim.intag_per_fyllnad = vaxlar.0;
    sim.hona_vuxen = vaxlar.1;
    sim.fodsel_arlig = vaxlar.2;
    sim.e_tak = e_tak;
    sim.e_skala = e_skala;
    sim.mogen_steg = mogen;
    sim.fodsel_troskel = troskel;
    sim.liv_steg = liv;
    // Agenternas Alt 1863: växternas återväxt gånger ett tal (1 = oförändrad värld).
    sim.atervaxt = atervaxt;
    for d in sim.djur.iter_mut() {
        d.e *= e_skala;
        // Startdjurens åldrar är dragna jämnt i 0..LIV; de skalas till den valda livslängden, så att ingen årskull dör i första steget.
        if let Some(l) = liv {
            d.age = d.age * l / hcsim::tal::LIV;
        }
    }
    let mut ut = vec![format!("{fro}\t0\t{}", ar_rad(&sim))];
    let inkor = MARKT_INKOR_AR * STEG_PER_AR;
    for a in 1..=ar {
        while sim.t < a * STEG_PER_AR {
            if sim.t == inkor && !sim.markering_skedd {
                sim.markera_halften(fro);
            }
            sim.steg();
        }
        ut.push(format!("{fro}\t{a}\t{}", ar_rad(&sim)));
        sim.rak.fodda_ar = 0;
        sim.rak.dod_svalt_ar = 0;
        sim.rak.dod_alder_ar = 0;
        if sim.antal() == 0 {
            break;
        }
    }
    Ok(ut)
}

/// Svängningens period i år för en följd av årsvärden: första toppen i autokorrelationen (fördröjning 3–39 år, värde över 0,2).
/// `None` = ingen topp. Samma metod för alla körningar (agenternas Alt 1858).
fn period(x: &[f64]) -> Option<usize> {
    if x.len() < 45 {
        return None;
    }
    let m = x.iter().sum::<f64>() / x.len() as f64;
    let d: Vec<f64> = x.iter().map(|a| a - m).collect();
    let var: f64 = d.iter().map(|a| a * a).sum();
    if var <= 0.0 {
        return None;
    }
    let ac = |l: usize| (0..d.len() - l).map(|i| d[i] * d[i + l]).sum::<f64>() / var;
    (3..40).find(|&l| ac(l) > ac(l - 1) && ac(l) >= ac(l + 1) && ac(l) > 0.2)
}

/// `utdod --period FIL [FIL ...]`: perioden per frö ur kolumnen `antal`, från år 20, och medianen per fil.
fn skriv_perioder(filer: &[String]) {
    for f in filer {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let mut per_fro: std::collections::BTreeMap<u64, Vec<f64>> = std::collections::BTreeMap::new();
        for rad in text.lines().skip(1) {
            let d: Vec<&str> = rad.split('\t').collect();
            let (fro, ar, antal): (u64, i64, f64) = (d[0].parse().expect("fro"), d[1].parse().expect("ar"), d[2].parse().expect("antal"));
            if ar >= 20 {
                per_fro.entry(fro).or_default().push(antal);
            }
        }
        assert!(!per_fro.is_empty(), "{f}: tom läsning");
        let p: Vec<Option<usize>> = per_fro.values().map(|x| period(x)).collect();
        let mut med: Vec<usize> = p.iter().flatten().copied().collect();
        med.sort();
        let utan = p.iter().filter(|x| x.is_none()).count();
        let median = if med.is_empty() { "–".to_string() } else { format!("{}", med[med.len() / 2]) };
        let lagst = med.first().map_or("–".to_string(), |x| x.to_string());
        let hogst = med.last().map_or("–".to_string(), |x| x.to_string());
        println!("{f}\tfrön {}\tperiod median {median} år\tlägst {lagst}\thögst {hogst}\tutan topp {utan}", p.len());
    }
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    if arg.first().map(|x| x.as_str()) == Some("--period") {
        skriv_perioder(&arg[1..]);
        return;
    }
    let v = |n: &str| arg.iter().position(|a| a == n).and_then(|i| arg.get(i + 1)).cloned();
    let kant = match v("--kant").expect("--kant vagg|torus").as_str() {
        "vagg" => KantLage::Vagg,
        "torus" => KantLage::Torus,
        x => panic!("okänd kant {x}"),
    };
    let k: usize = v("--k").expect("--k N").parse().expect("--k");
    let fron = v("--fron").expect("--fron A-B");
    let (a, b) = fron.split_once('-').expect("--fron A-B");
    let (a, b): (u64, u64) = (a.parse().expect("frö"), b.parse().expect("frö"));
    assert!(b >= a);
    let ar: i64 = v("--ar").expect("--ar N").parse().expect("--ar");
    let tradar: usize = v("--tradar").map(|x| x.parse().expect("--tradar")).unwrap_or(6);
    let har = |n: &str| arg.iter().any(|a| a == n);
    let vaxlar = (har("--intag-fyllnad"), har("--hona-vuxen"), har("--fodsel-arlig"));
    let e_tak: Option<f64> = v("--e-tak").map(|x| x.parse().expect("--e-tak"));
    let e_skala: f64 = v("--e-skala").map(|x| x.parse().expect("--e-skala")).unwrap_or(1.0);
    let mogen: Option<i64> = v("--mogen-steg").map(|x| x.parse().expect("--mogen-steg"));
    let troskel: Option<f64> = v("--fodsel-troskel").map(|x| x.parse().expect("--fodsel-troskel"));
    let atervaxt: f64 = v("--atervaxt").map(|x| x.parse().expect("--atervaxt TAL")).unwrap_or(1.0);
    let liv: Option<i64> = v("--liv-ar").map(|x| x.parse::<i64>().expect("--liv-ar N (år)") * hcsim::tal::STEG_PER_AR);
    let rot = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    println!("fro\tar\tantal\tvuxna\thonor\thonor_redo\tvaxt_fyllnad\tandel_rutor_under_5pct\te_medel\tfodda\tdod_svalt\tdod_alder\tstorsta_intag\tandel_djur_i_tatast_1pct_rutor\trutor_med_djur");
    let fron: Vec<u64> = (a..=b).collect();
    for del in fron.chunks(tradar.max(1)) {
        let svar: Vec<_> = std::thread::scope(|s| {
            let h: Vec<_> = del.iter().map(|&f| { let rot = &rot; s.spawn(move || kor(rot, kant, k, f, ar, vaxlar, e_tak, e_skala, mogen, troskel, liv, atervaxt)) }).collect();
            h.into_iter().map(|x| x.join().expect("tråden föll")).collect()
        });
        for r in svar {
            for rad in r.expect("körningen föll") {
                println!("{rad}");
            }
        }
    }
}

#[cfg(test)]
mod prov {
    use super::period;
    #[test]
    fn perioden_i_en_kand_vag() {
        let x: Vec<f64> = (0..81).map(|i| (2.0 * std::f64::consts::PI * i as f64 / 15.0).sin()).collect();
        assert_eq!(period(&x), Some(15));
        let y: Vec<f64> = (0..81).map(|i| (2.0 * std::f64::consts::PI * i as f64 / 19.0).sin()).collect();
        assert_eq!(period(&y), Some(19));
        assert_eq!(period(&vec![5.0; 81]), None, "en platt följd har ingen period");
        assert_eq!(period(&[1.0, 2.0]), None, "för kort följd");
    }
}
