//! vmodell: Fas 1 (Håkan 2026-11-23), prövar om Edmontosaurus hjärna är en handlingsvillkorad världsmodell.
//! Mått och gränser: piharness `docs/evolution/vmodell-fas1-utfallsregel.md`. Räkningen: `hcsim::vmodell`.
//! Världen är visningens (paket E: bälte 8, sammanfogade sidor, rättningarna av utdöendet), utan märkning: alla djur har `lar`.
//! Användning: vmodell --fron 261-280 --ar 40,47,54 --djur 100 --tradar 10 --ut MAPP
//! Rust (AGENTS.md regel 11).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::sim::{KantLage, Sim};
use hcsim::tal::{A_N, STEG_PER_AR, Z_N};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::vmodell::{
    dom_k1, dom_k2, dom_k3, erfarenhet_vid_prov, median, prov_med_persistens, prov_med_skugga, prova_djur, samma_handling_samma_svar, valj_djur, Dom, Matt, Modell,
    Skugga, GRUPPER, GRUPP_NAMN, N_GEM_F, N_GEM_R, N_NIO,
};
use hcsim::Formaga;

/// Modellerna i jämförelsen: hjärnan själv, tre skuggor (lär sig bara av djurets egna steg) och persistensen.
const MODELLER: usize = 5;
const MODELL_NAMN: [&str; MODELLER] = [
    "Hjärnan i dag (nio skilda, ärvd start)",
    "Nio skilda utan ärvd start (skugga)",
    "Gemensam med förskjutning per handling (skugga)",
    "Gemensam med handlingens riktning som indata (skugga)",
    "Persistensen (inget ändras)",
];

struct FroSvar {
    fro: u64,
    matt: Matt,
    /// Samma prov räknade med varje modell (plats 0 = hjärnan, lika med `matt` men med erfarenheten ifylld).
    modeller: Vec<Matt>,
    skuggsteg: u64,
    /// K4 och K5 per tillfälle.
    orord: Vec<bool>,
    samma_svar: Vec<bool>,
    djur_vid_prov: Vec<usize>,
    sekunder: f64,
}

pub fn bygg_varld(rot: &Path, fro: u64) -> Result<Sim, String> {
    let arm = utforsk_armar().into_iter().find(|a| a.namn == "e_lar" && !a.spegel).ok_or("armen e_lar saknas")?;
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    let mut sim = bygg_sim_balte(&kalla, &tt, B_FULL, H_FULL, BALTE_K, fro, &arm)?;
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    satt_rattningar_utdod(&mut sim);
    Ok(sim)
}

fn kor(rot: &Path, fro: u64, ar: &[i64], n_djur: usize) -> Result<FroSvar, String> {
    let t0 = std::time::Instant::now();
    let mut sim = bygg_varld(rot, fro)?;
    let mut svar = FroSvar { fro, matt: Matt::default(), modeller: vec![Matt::default(); MODELLER], skuggsteg: 0, orord: vec![], samma_svar: vec![], djur_vid_prov: vec![], sekunder: 0.0 };
    // Skuggorna följer varje djur från världens början och läser bara vad djuret självt såg och valde i varje steg.
    let mut skuggor: HashMap<u64, Skugga> = HashMap::new();
    for &a in ar {
        while sim.t < a * STEG_PER_AR {
            sim.steg();
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some((z, val, _)) = &d.hj.forra {
                    let mut zz = [0.0; Z_N];
                    zz.copy_from_slice(&z[..Z_N]);
                    skuggor.entry(d.uid).or_default().steg(&zz, *val);
                    svar.skuggsteg += 1;
                }
            }
            if sim.t % STEG_PER_AR == 0 {
                let lever: HashSet<u64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.uid).collect();
                skuggor.retain(|uid, _| lever.contains(uid));
            }
        }
        assert!(!sim.markering_skedd, "ingen märkning i valideringen");
        assert!(sim.djur.iter().filter(|d| d.lev).all(|d| d.formaga == Formaga::Lar), "alla djur ska ha förmågan lar");
        svar.djur_vid_prov.push(sim.antal());
        let fore = (sim.djur_summa(), sim.g.tillstand());
        let valda = valj_djur(&sim, n_djur, fro.wrapping_mul(1000).wrapping_add(a as u64));
        if let Some(&forsta) = valda.first() {
            svar.samma_svar.push(samma_handling_samma_svar(&sim, forsta, 3));
        }
        for idx in valda {
            match prova_djur(&sim, idx) {
                Ok(p) => {
                    svar.matt.lagg(&p);
                    let sk = skuggor.get(&p.uid).expect("djuret har en skugga");
                    assert_eq!(sk.forra.as_ref().map(|f| f.1), sim.djur[idx].hj.forra.as_ref().map(|f| f.1), "skuggan och hjärnan har olika förra handling");
                    let mut ph = p.clone();
                    ph.erfarenhet = Some(erfarenhet_vid_prov(sk));
                    svar.modeller[0].lagg(&ph);
                    svar.modeller[1].lagg(&prov_med_skugga(&p, sk, Modell::Nio));
                    svar.modeller[2].lagg(&prov_med_skugga(&p, sk, Modell::GemForskjutning));
                    svar.modeller[3].lagg(&prov_med_skugga(&p, sk, Modell::GemRiktning));
                    svar.modeller[4].lagg(&prov_med_persistens(&p));
                }
                Err(_) => svar.matt.bortfall += 1,
            }
        }
        svar.orord.push(fore == (sim.djur_summa(), sim.g.tillstand()));
        eprintln!("KLAR frö {fro} år {a}: {} prov, {} bortfall, {:.0} s", svar.matt.prov, svar.matt.bortfall, t0.elapsed().as_secs_f64());
    }
    svar.sekunder = t0.elapsed().as_secs_f64();
    Ok(svar)
}

const HANDLING: [&str; A_N] = ["stå kvar", "(0,+1)", "(+1,+1)", "(+1,0)", "(+1,−1)", "(0,−1)", "(−1,−1)", "(−1,0)", "(−1,+1)"];

fn rapport(svar: &[FroSvar], ar: &[i64], n_djur: usize) -> String {
    let per: Vec<Matt> = svar.iter().map(|s| s.matt.clone()).collect();
    let mut alla = Matt::default();
    for m in &per {
        alla.sla_ihop(m);
    }
    let n = per.len();
    let k0 = if alla.ej_samma_z == 0 && alla.prov > 0 { Dom::Pass } else { Dom::Fail };
    let (k1, k2, k3) = (dom_k1(&per), dom_k2(&per), dom_k3(&per));
    let k4 = if svar.iter().all(|s| s.orord.iter().all(|&x| x) && !s.orord.is_empty()) { Dom::Pass } else { Dom::Fail };
    let k5 = if svar.iter().all(|s| s.samma_svar.iter().all(|&x| x) && !s.samma_svar.is_empty()) { Dom::Pass } else { Dom::Fail };
    let mut t = String::new();
    let _ = writeln!(t, "# Fas 1: är Edmontosaurus hjärna en handlingsvillkorad världsmodell? (resultat)\n");
    let _ = writeln!(t, "Skrivet av programmet `vmodell` (sandbox/pi-vmodell `n10/hcsim/src/bin/vmodell.rs`). Mått och gränser: piharness `docs/evolution/vmodell-fas1-utfallsregel.md`.");
    let _ = writeln!(t, "Världen: visningens (paket E), frö {}–{}, prov år {:?}, {} djur per tillfälle. {} prov räknade, {} bortfall (djuret dog i någon kopia). Varje prov = 9 kopior av världen.\n",
        svar.first().map(|s| s.fro).unwrap_or(0), svar.last().map(|s| s.fro).unwrap_or(0), ar, n_djur, alla.prov, alla.bortfall);
    let _ = writeln!(t, "## Domarna\n\n| Mått | Dom | Tal |\n|---|---|---|");
    let _ = writeln!(t, "| K0 samma utgångsläge | {} | {} av {} prov med olika z; största skillnad mellan ren förutsägelse och hjärnans egen i steget {:.3e} |", k0.text(), alla.ej_samma_z, alla.prov, alla.ren_diff_max);
    let k1v: Vec<f64> = per.iter().map(|m| m.k1()).collect();
    let _ = writeln!(t, "| K1 särskiljbara förutsägelser | {} | andel särskiljda par: median {:.3}, lägst {:.3}, högst {:.3}; frön med minst 0,90: {} av {}; identiska par: {} av {} |",
        k1.text(), median(k1v.clone()), k1v.iter().cloned().fold(f64::INFINITY, f64::min), k1v.iter().cloned().fold(0.0, f64::max), k1v.iter().filter(|&&x| x >= 0.90).count(), n, alla.par_identiska, alla.prov * 36);
    let under = per.iter().filter(|m| (0..A_N).all(|a| m.k2(a) < 1.0)).count();
    let _ = writeln!(t, "| K2 felet mot persistensen | {} | frön där alla nio handlingar ligger under 1: {} av {}; kvot över alla handlingar {:.3} |",
        k2.text(), under, n, alla.fel_pred.iter().sum::<f64>() / alla.fel_kop.iter().sum::<f64>());
    let k3v: Vec<f64> = per.iter().map(|m| m.k3()).collect();
    let _ = writeln!(t, "| K3 handlingens verkan förutsagd | {} | R²: median {:.3}, lägst {:.3}, högst {:.3}; frön över 0: {} av {}; cosinus {:.3}; storlek (förutsagd/verklig) {:.3} |",
        k3.text(), median(k3v.clone()), k3v.iter().cloned().fold(f64::INFINITY, f64::min), k3v.iter().cloned().fold(f64::NEG_INFINITY, f64::max), k3v.iter().filter(|&&x| x > 0.0).count(), n, alla.cosinus(), alla.storlek());
    let _ = writeln!(t, "| K4 ingen sidoeffekt | {} | originalets kontrollsumma och slumpföljd lika före och efter vid {} av {} tillfällen |", k4.text(),
        svar.iter().map(|s| s.orord.iter().filter(|&&x| x).count()).sum::<usize>(), svar.iter().map(|s| s.orord.len()).sum::<usize>());
    let _ = writeln!(t, "| K5 samma handling, samma svar | {} | {} av {} |", k5.text(),
        svar.iter().map(|s| s.samma_svar.iter().filter(|&&x| x).count()).sum::<usize>(), svar.iter().map(|s| s.samma_svar.len()).sum::<usize>());
    let _ = writeln!(t, "\nK6 (ingen dold kunskap) prövas i `tests/vmodell.rs`, inte här.\n");
    let _ = writeln!(t, "## Per handling (alla frön)\n\n| Handling | Fel förutsägelse | Fel persistens | Kvot | Kvot när hjärnan valde den | Kvot när den inte valde den | Hjärnans val (andel) | Flyttade (andel) | Median av kvoten över frön |\n|---|---|---|---|---|---|---|---|---|");
    for a in 0..A_N {
        let p = alla.prov.max(1) as f64;
        let _ = writeln!(t, "| {} {} | {:.5} | {:.5} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |", a, HANDLING[a], alla.fel_pred[a] / p, alla.fel_kop[a] / p, alla.k2(a),
            alla.fel_pred_vald[a] / alla.fel_kop_vald[a].max(1e-300), alla.fel_pred_ovald[a] / alla.fel_kop_ovald[a].max(1e-300), alla.val[a] as f64 / p, alla.flyttade[a] as f64 / p,
            median(per.iter().map(|m| m.k2(a)).collect()));
    }
    let _ = writeln!(t, "\n## Handlingens del av förändringen\n");
    let _ = writeln!(t, "- Andelen av den verkliga förändringen (mot nuläget, summerad över handlingarna) som beror på handlingen: {:.3}.", alla.handlingens_andel());
    let _ = writeln!(t, "- Förutsägelsernas spridning mellan handlingarna är {:.3} av utfallens (längd); riktningen stämmer med cosinus {:.3}.", alla.storlek(), alla.cosinus());
    let _ = writeln!(t, "\n## Par av handlingar: förutsagt avstånd delat med verkligt (kvadrat, alla frön); identiska förutsägelser inom parentes\n");
    let mut rad = String::from("| |");
    for b in 0..A_N { let _ = write!(rad, " {b} |"); }
    let _ = writeln!(t, "{rad}\n|---|{}", "---|".repeat(A_N));
    for a in 0..A_N {
        let mut rad = format!("| {a} |");
        for b in 0..A_N {
            if b <= a { rad.push_str(" |"); } else { let _ = write!(rad, " {:.3} ({}) |", alla.par_pred[a][b] / alla.par_verklig[a][b].max(1e-300), alla.par_ident[a][b]); }
        }
        let _ = writeln!(t, "{rad}");
    }
    let p = alla.prov.max(1) as f64;
    let _ = writeln!(t, "\n## Valet (redovisas utan gräns)\n");
    let _ = writeln!(t, "- Hjärnans bästa handling efter förutsägelserna är också bäst efter utfallen (hjärnans eget värde på den verkliga nästa bilden) i {:.3} av proven (slumpen ger {:.3}).", alla.topp_lika as f64 / p, 1.0 / A_N as f64);
    let _ = writeln!(t, "- Intaget i steget efter hjärnans val: {:.4}; medel över de nio handlingarna: {:.4}; det bästa av de nio: {:.4}.", alla.intag_vald / p, alla.intag_medel / p, alla.intag_bast / p);
    let _ = writeln!(t, "\n## Per frö\n\n| Frö | Prov | Bortfall | Djur vid proven | K1 | K3 R² | Kvot alla handlingar | Handlingar med kvot minst 1 | Sekunder |\n|---|---|---|---|---|---|---|---|---|");
    for s in svar {
        let m = &s.matt;
        let _ = writeln!(t, "| {} | {} | {} | {:?} | {:.3} | {:.3} | {:.3} | {} | {:.0} |", s.fro, m.prov, m.bortfall, s.djur_vid_prov, m.k1(), m.k3(),
            m.fel_pred.iter().sum::<f64>() / m.fel_kop.iter().sum::<f64>().max(1e-300), (0..A_N).filter(|&a| m.k2(a) >= 1.0).count(), s.sekunder);
    }
    jamforelse(&mut t, svar);
    let alla_pass = [k0, k1, k2, k3, k4, k5].iter().all(|&d| d == Dom::Pass);
    let _ = writeln!(t, "\n## Slutbedömning enligt regeln\n\n{}", if alla_pass {
        "K0–K5 är PASS. Med K6 grönt i proven uppfyller hjärnan kraven i utfallsregeln."
    } else {
        "Minst ett av K0–K5 är inte PASS. Hjärnan uppfyller INTE kraven på en handlingsvillkorad världsmodell enligt utfallsregeln."
    });
    t
}

/// Tiden för nio förutsägelser (alla handlingar från en bild), mätt på en miljon varv. Bara skuggornas former; hjärnans egen har samma form som "nio skilda".
fn tid_per_nio(m: Modell) -> f64 {
    let s = Skugga::ny();
    let mut z = [0.1, -0.2, 0.3, 0.05];
    let varv = 1_000_000;
    let t0 = std::time::Instant::now();
    let mut summa = 0.0;
    for i in 0..varv {
        z[0] = (i % 97) as f64 * 0.001;
        for a in 0..A_N {
            summa += std::hint::black_box(s.forutsag(m, std::hint::black_box(&z), a))[0];
        }
    }
    std::hint::black_box(summa);
    t0.elapsed().as_secs_f64() * 1e9 / varv as f64
}

/// Jämförelsen (Håkan 2026-11-23): samma världar, djur, erfarenheter och provtillfällen för alla modeller. Skuggorna har bara lärt sig av
/// djurets egna steg; ingen har tränats på utfallen i kopiorna.
fn jamforelse(t: &mut String, svar: &[FroSvar]) {
    let n = svar.len();
    let per = |m: usize| -> Vec<Matt> { svar.iter().map(|s| s.modeller[m].clone()).collect() };
    let alla = |m: usize| -> Matt {
        let mut a = Matt::default();
        for s in svar {
            a.sla_ihop(&s.modeller[m]);
        }
        a
    };
    let _ = writeln!(t, "\n## Jämförelsen mellan modellerna (samma {} prov; en art: Edmontosaurus, den enda i simuleringen)\n", alla(0).prov);
    let _ = writeln!(t, "Skuggorna styr ingenting: djurens val kommer hela tiden från hjärnan i dag. De har ingen ärvd start (börjar som persistensen) och lär sig med samma regel och takt som hjärnan, bara av det djuret självt upplevde. Skuggsteg totalt: {}.\n", svar.iter().map(|s| s.skuggsteg).sum::<u64>());
    let _ = writeln!(t, "| Modell | K1 dom | K1 median | K2 dom | Felkvot mot persistensen (alla handlingar) | Sämsta handlingens kvot | K3 dom | K3 R² median | lägst | högst | Frön med R² över 0 | Cosinus | Storlek |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for m in 0..MODELLER {
        let (p, a) = (per(m), alla(m));
        let k3v: Vec<f64> = p.iter().map(|x| x.k3()).collect();
        let _ = writeln!(t, "| {} | {} | {:.3} | {} | {:.3} | {:.3} | {} | {:.3} | {:.3} | {:.3} | {} av {} | {:.3} | {:.3} |", MODELL_NAMN[m], dom_k1(&p).text(), median(p.iter().map(|x| x.k1()).collect()), dom_k2(&p).text(),
            a.fel_pred.iter().sum::<f64>() / a.fel_kop.iter().sum::<f64>().max(1e-300), (0..A_N).map(|h| a.k2(h)).fold(0.0, f64::max), dom_k3(&p).text(), median(k3v.clone()),
            k3v.iter().cloned().fold(f64::INFINITY, f64::min), k3v.iter().cloned().fold(f64::NEG_INFINITY, f64::max), k3v.iter().filter(|&&x| x > 0.0).count(), n, a.cosinus(), a.storlek());
    }
    let _ = writeln!(t, "\nParvis per frö mot \"nio skilda utan ärvd start\" (samma start och samma erfarenhet, bara formen skiljer):\n");
    for m in 2..MODELLER - 1 {
        let (p1, pm) = (per(1), per(m));
        let r2 = (0..n).filter(|&i| pm[i].k3() > p1[i].k3()).count();
        let fk = |x: &Matt| x.fel_pred.iter().sum::<f64>() / x.fel_kop.iter().sum::<f64>().max(1e-300);
        let fel = (0..n).filter(|&i| fk(&pm[i]) < fk(&p1[i])).count();
        let _ = writeln!(t, "- {}: högre R² i {} av {} frön; lägre felkvot i {} av {} frön.", MODELL_NAMN[m], r2, n, fel, n);
    }
    let _ = writeln!(t, "\n### Felkvot mot persistensen per handling och modell (alla frön)\n");
    let mut rad = String::from("| Modell |");
    for h in 0..A_N { let _ = write!(rad, " {} {} |", h, HANDLING[h]); }
    let _ = writeln!(t, "{rad}\n|---|{}", "---|".repeat(A_N));
    for m in 0..MODELLER - 1 {
        let a = alla(m);
        let mut rad = format!("| {} |", MODELL_NAMN[m]);
        for h in 0..A_N { let _ = write!(rad, " {:.3} |", a.k2(h)); }
        let _ = writeln!(t, "{rad}");
    }
    let h0 = alla(0);
    let _ = writeln!(t, "\n### Träningsfördelningen (hjärnans egna förutsägare; träningar räknade FÖRE förutsägelsen i provsteget)\n");
    let dj = h0.erf_djur.max(1) as f64;
    let _ = writeln!(t, "- Per djur: {:.2} av 9 förutsägare har aldrig tränats; den mest tränade handlingen har {:.3} av djurets alla träningar.", h0.erf_aldrig as f64 / dj, h0.erf_topp_andel / dj);
    let tot: u64 = h0.erf_sum.iter().sum();
    let mut rad = String::from("- Andel av alla träningar per handling:");
    for h in 0..A_N { let _ = write!(rad, " {}: {:.3};", h, h0.erf_sum[h] as f64 / tot.max(1) as f64); }
    let _ = writeln!(t, "{rad}\n");
    let _ = writeln!(t, "Antal prov per handling och erfarenhetsgrupp (ett prov = ett djur vid ett tillfälle):\n");
    let mut rad = String::from("| Träningar |");
    for h in 0..A_N { let _ = write!(rad, " {h} |"); }
    let _ = writeln!(t, "{rad} Alla |\n|---|{}---|", "---|".repeat(A_N));
    for g in 0..GRUPPER {
        let mut rad = format!("| {} |", GRUPP_NAMN[g]);
        for h in 0..A_N { let _ = write!(rad, " {} |", h0.grp_n[h][g]); }
        let _ = writeln!(t, "{rad} {} |", (0..A_N).map(|h| h0.grp_n[h][g]).sum::<u64>());
    }
    let _ = writeln!(t, "\n### Felkvot mot persistensen och R² för handlingens del, per erfarenhetsgrupp och modell (alla handlingar och frön)\n");
    let _ = writeln!(t, "Gruppen är hur många gånger DJURET har utfört handlingen före provet. Gruppen 0 är generaliseringen: handlingar som djuret aldrig har utfört.\n");
    let _ = writeln!(t, "| Modell | Mått |{} \n|---|---|{}", GRUPP_NAMN.iter().map(|g| format!(" {g} |")).collect::<String>(), "---|".repeat(GRUPPER));
    for m in 0..MODELLER - 1 {
        let a = alla(m);
        let mut r1 = format!("| {} | antal prov |", MODELL_NAMN[m]);
        let mut r2 = String::from("| | felkvot |");
        let mut r3 = String::from("| | R² |");
        for g in 0..GRUPPER {
            let nn: u64 = (0..A_N).map(|h| a.grp_n[h][g]).sum();
            let fp: f64 = (0..A_N).map(|h| a.grp_fel_pred[h][g]).sum();
            let fk: f64 = (0..A_N).map(|h| a.grp_fel_kop[h][g]).sum();
            let re: f64 = (0..A_N).map(|h| a.grp_rest[h][g]).sum();
            let ve: f64 = (0..A_N).map(|h| a.grp_verklig[h][g]).sum();
            let _ = write!(r1, " {nn} |");
            if nn == 0 { r2.push_str(" – |"); r3.push_str(" – |"); } else { let _ = write!(r2, " {:.3} |", fp / fk.max(1e-300)); let _ = write!(r3, " {:.3} |", 1.0 - re / ve.max(1e-300)); }
        }
        let _ = writeln!(t, "{r1}\n{r2}\n{r3}");
    }
    let _ = writeln!(t, "\n### Hjärnan i dag: felkvot per handling och erfarenhetsgrupp (antal prov inom parentes)\n");
    let mut rad = String::from("| Träningar |");
    for h in 0..A_N { let _ = write!(rad, " {h} |"); }
    let _ = writeln!(t, "{rad}\n|---|{}", "---|".repeat(A_N));
    for g in 0..GRUPPER {
        let mut rad = format!("| {} |", GRUPP_NAMN[g]);
        for h in 0..A_N {
            if h0.grp_n[h][g] == 0 { rad.push_str(" – |"); } else { let _ = write!(rad, " {:.3} ({}) |", h0.grp_fel_pred[h][g] / h0.grp_fel_kop[h][g].max(1e-300), h0.grp_n[h][g]); }
        }
        let _ = writeln!(t, "{rad}");
    }
    let _ = writeln!(t, "\n### Kostnad (förutsägaren; kodaren och värderingen är lika för alla)\n");
    let _ = writeln!(t, "| Form | Vikter per djur | Minne per djur (byte) | Tid för nio förutsägelser (ns, mätt på den här datorn under last) |\n|---|---|---|---|");
    for (namn, vikter, m) in [("Nio skilda (hjärnan i dag)", N_NIO, Modell::Nio), ("Gemensam med förskjutning", N_GEM_F, Modell::GemForskjutning), ("Gemensam med riktning", N_GEM_R, Modell::GemRiktning)] {
        let _ = writeln!(t, "| {} | {} | {} | {:.0} |", namn, vikter, vikter * 8, tid_per_nio(m));
    }
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    let v = |n: &str| arg.iter().position(|a| a == n).and_then(|i| arg.get(i + 1)).cloned();
    let fron = v("--fron").expect("--fron A-B");
    let (a, b) = fron.split_once('-').expect("--fron A-B");
    let (a, b): (u64, u64) = (a.parse().expect("frö"), b.parse().expect("frö"));
    assert!(b >= a);
    let ar: Vec<i64> = v("--ar").expect("--ar 40,47,54").split(',').map(|x| x.parse().expect("år")).collect();
    assert!(ar.windows(2).all(|w| w[0] < w[1]), "åren ska stiga");
    let n_djur: usize = v("--djur").expect("--djur N").parse().expect("--djur");
    let tradar: usize = v("--tradar").map(|x| x.parse().expect("--tradar")).unwrap_or(4);
    let ut = std::path::PathBuf::from(v("--ut").expect("--ut MAPP"));
    std::fs::create_dir_all(&ut).expect("skapa mappen");
    let rot = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fron: Vec<u64> = (a..=b).collect();
    let mut svar: Vec<FroSvar> = Vec::new();
    for del in fron.chunks(tradar.max(1)) {
        let s: Vec<_> = std::thread::scope(|s| {
            let h: Vec<_> = del.iter().map(|&f| { let (rot, ar) = (&rot, &ar); s.spawn(move || kor(rot, f, ar, n_djur)) }).collect();
            h.into_iter().map(|x| x.join().expect("tråden föll")).collect()
        });
        for r in s {
            svar.push(r.expect("körningen föll"));
        }
    }
    let text = rapport(&svar, &ar, n_djur);
    std::fs::write(ut.join("vmodell-fas1-resultat.md"), &text).expect("skriv rapporten");
    println!("{text}");
}
