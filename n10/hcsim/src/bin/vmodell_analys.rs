//! vmodell_analys: kompletteringsanalysen efter Fas 1 (Håkan 2026-11-23): träningsfördelning, urvalsbias och överföring mellan handlingar.
//! Mått och bedömningsregler: piharness `docs/evolution/vmodell-fas1-utfallsregel.md` (tillägg 2). Ändrar ingenting i hjärnan eller världen.
//! `kor` kör samma världar, djur och provtillfällen som referensen (grenen vmodell, 2ec5188) och skriver en rad per prov och handling;
//! `rapport` läser raderna och skriver tabellerna. Raderna gör att nya frågor kan besvaras utan en ny körning.
//! Användning: vmodell_analys kor --fron 261-280 --ar 40,47,54 --djur 100 --tradar 10 --ut MAPP
//!             vmodell_analys rapport --in MAPP
//! Rust (AGENTS.md regel 11).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::utforsk_armar;
use hcsim::hjarna::NK;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::sim::{KantLage, Sim};
use hcsim::tal::{A_N, STEG_PER_AR, Z_N};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::vmodell::{
    fel_och_rest, grupp, kvad, median, prov_med_skugga, prova_djur, valj_djur, verklig_del, Historik, Matt, Modell, Skugga, GRUPPER, GRUPP_NAMN,
};
use hcsim::Formaga;

const HANDLING: [&str; A_N] = ["stå kvar", "(0,+1)", "(+1,+1)", "(+1,0)", "(+1,−1)", "(0,−1)", "(−1,−1)", "(−1,0)", "(−1,+1)"];
const MODELLER: usize = 4;
const MODELL_NAMN: [&str; MODELLER] = ["Hjärnan i dag", "Nio skilda utan ärvd start", "Gemensam med förskjutning", "Gemensam med riktning"];
const RUBRIK: &str = "fro\tar\tuid\talder\ta\tvald\tforra\tutf365\tutfliv\tsedan\tororda\tfel_p\tfel_h\trest_h\tfel_n9\trest_n9\tfel_gf\trest_gf\tfel_gr\trest_gr\tverklig\tv_pred\tv_sann\tintag\tflyttade";

fn bygg_varld(rot: &Path, fro: u64) -> Result<Sim, String> {
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

struct FroSvar {
    fro: u64,
    matt: Matt,
    djur_vid_prov: Vec<usize>,
    rader: Vec<String>,
}

fn kor(rot: &Path, fro: u64, ar: &[i64], n_djur: usize) -> Result<FroSvar, String> {
    let t0 = std::time::Instant::now();
    let mut sim = bygg_varld(rot, fro)?;
    let mut svar = FroSvar { fro, matt: Matt::default(), djur_vid_prov: vec![], rader: vec![] };
    let mut skuggor: HashMap<u64, (Skugga, Historik)> = HashMap::new();
    for &a in ar {
        while sim.t < a * STEG_PER_AR {
            sim.steg();
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some((z, val, _)) = &d.hj.forra {
                    let mut zz = [0.0; Z_N];
                    zz.copy_from_slice(&z[..Z_N]);
                    let e = skuggor.entry(d.uid).or_default();
                    e.0.steg(&zz, *val);
                    e.1.utfor(*val);
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
        for idx in valda {
            let p = match prova_djur(&sim, idx) {
                Ok(p) => p,
                Err(_) => {
                    svar.matt.bortfall += 1;
                    continue;
                }
            };
            svar.matt.lagg(&p);
            let (sk, hist) = skuggor.get(&p.uid).expect("djuret har en skugga");
            let d = &sim.djur[idx];
            assert_eq!(hist.forra(), d.hj.forra.as_ref().map(|f| f.1), "historiken och hjärnan har olika förra handling");
            let (ar365, verk) = (hist.senaste_aret(), verklig_del(&p));
            let fh = fel_och_rest(&p);
            let f9 = fel_och_rest(&prov_med_skugga(&p, sk, Modell::Nio));
            let ff = fel_och_rest(&prov_med_skugga(&p, sk, Modell::GemForskjutning));
            let fr = fel_och_rest(&prov_med_skugga(&p, sk, Modell::GemRiktning));
            for h in 0..A_N {
                // Orörd: förutsägarens vikter för handlingen är bit för bit de ärvda (före provsteget).
                let b = h * Z_N * (Z_N + 1);
                let ororda = (0..Z_N * (Z_N + 1)).all(|k| d.hj.wp[b + k].to_bits() == d.hj.gen[NK + b + k].to_bits());
                let u = &p.utfall[h];
                svar.rader.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{:e}\t{}",
                    fro, a, p.uid, d.age, h, (p.hjarnans_val == h) as u8, (hist.forra() == Some(h)) as u8, ar365[h], hist.liv[h],
                    hist.sedan(h).unwrap_or(-1), ororda as u8, kvad(&p.z, &u.zn), fh[h].0, fh[h].1, f9[h].0, f9[h].1, ff[h].0, ff[h].1, fr[h].0, fr[h].1,
                    verk[h], u.v_pred, u.v_sann, u.intag, u.flyttade as u8
                ));
            }
        }
        assert!(fore == (sim.djur_summa(), sim.g.tillstand()), "originalvärlden ändrades av proven");
        eprintln!("KLAR frö {fro} år {a}: {} prov, {} bortfall, {:.0} s", svar.matt.prov, svar.matt.bortfall, t0.elapsed().as_secs_f64());
    }
    Ok(svar)
}

/// De två tabellerna ur referensrapporten, med samma format tecken för tecken (utan kolumnen Sekunder), för jämförelse med `diff`.
fn kontrolltabeller(svar: &[FroSvar]) -> String {
    let per: Vec<Matt> = svar.iter().map(|s| s.matt.clone()).collect();
    let mut alla = Matt::default();
    for m in &per {
        alla.sla_ihop(m);
    }
    let mut t = String::new();
    let _ = writeln!(t, "## Per handling (alla frön)\n");
    for a in 0..A_N {
        let p = alla.prov.max(1) as f64;
        let _ = writeln!(t, "| {} {} | {:.5} | {:.5} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |", a, HANDLING[a], alla.fel_pred[a] / p, alla.fel_kop[a] / p, alla.k2(a),
            alla.fel_pred_vald[a] / alla.fel_kop_vald[a].max(1e-300), alla.fel_pred_ovald[a] / alla.fel_kop_ovald[a].max(1e-300), alla.val[a] as f64 / p, alla.flyttade[a] as f64 / p,
            median(per.iter().map(|m| m.k2(a)).collect()));
    }
    let _ = writeln!(t, "\n## Per frö\n");
    for s in svar {
        let m = &s.matt;
        let _ = writeln!(t, "| {} | {} | {} | {:?} | {:.3} | {:.3} | {:.3} | {} |", s.fro, m.prov, m.bortfall, s.djur_vid_prov, m.k1(), m.k3(),
            m.fel_pred.iter().sum::<f64>() / m.fel_kop.iter().sum::<f64>().max(1e-300), (0..A_N).filter(|&a| m.k2(a) >= 1.0).count());
    }
    t
}

// ------------------------------------------------------------------------------------------------------------------------------ rapporten

#[derive(Clone, Debug)]
pub struct Rad {
    fro: u64,
    ar: i64,
    uid: u64,
    alder: i64,
    a: usize,
    vald: bool,
    forra: bool,
    utf365: u32,
    utfliv: u32,
    sedan: i64,
    ororda: bool,
    fel_p: f64,
    fel: [f64; MODELLER],
    rest: [f64; MODELLER],
    verklig: f64,
    v_pred: f64,
    intag: f64,
}

pub fn las_rad(l: &str) -> Rad {
    let c: Vec<&str> = l.split('\t').collect();
    assert_eq!(c.len(), 25, "raden har {} fält, väntade 25: {l}", c.len());
    let f = |i: usize| -> f64 { c[i].parse().unwrap_or_else(|_| panic!("fält {i} är inget tal: {}", c[i])) };
    Rad {
        fro: c[0].parse().unwrap(),
        ar: c[1].parse().unwrap(),
        uid: c[2].parse().unwrap(),
        alder: c[3].parse().unwrap(),
        a: c[4].parse().unwrap(),
        vald: c[5] == "1",
        forra: c[6] == "1",
        utf365: c[7].parse().unwrap(),
        utfliv: c[8].parse().unwrap(),
        sedan: c[9].parse().unwrap(),
        ororda: c[10] == "1",
        fel_p: f(11),
        fel: [f(12), f(14), f(16), f(18)],
        rest: [f(13), f(15), f(17), f(19)],
        verklig: f(20),
        v_pred: f(21),
        intag: f(23),
    }
}

/// Summor för en grupp rader: antal, modellens fel, persistensens fel, handlingsdelens rest och verkliga storlek.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sum {
    pub n: u64,
    pub fel: f64,
    pub fel_p: f64,
    pub rest: f64,
    pub verklig: f64,
}

impl Sum {
    pub fn lagg(&mut self, r: &Rad, m: usize) {
        self.n += 1;
        self.fel += r.fel[m];
        self.fel_p += r.fel_p;
        self.rest += r.rest[m];
        self.verklig += r.verklig;
    }
    pub fn kvot(&self) -> f64 {
        self.fel / self.fel_p.max(1e-300)
    }
    pub fn r2(&self) -> f64 {
        1.0 - self.rest / self.verklig.max(1e-300)
    }
    fn cell(&self) -> String {
        if self.n == 0 { "–".into() } else { format!("{:.3} ({})", self.kvot(), self.n) }
    }
}

fn summa<'a>(rader: impl Iterator<Item = &'a Rad>, m: usize) -> Sum {
    let mut s = Sum::default();
    for r in rader {
        s.lagg(r, m);
    }
    s
}

fn kvantil(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q).round() as usize]
}

const SEDAN_NAMN: [&str; 5] = ["1 (förra steget)", "2–9", "10–149", "150 eller fler", "aldrig"];
fn sedan_grupp(s: i64) -> usize {
    match s {
        -1 => 4,
        1 => 0,
        2..=9 => 1,
        10..=149 => 2,
        _ => 3,
    }
}
const ANDRA_NAMN: [&str; 4] = ["under 150", "150–599", "600–1499", "1500 eller fler"];
fn andra_grupp(n: u32) -> usize {
    match n {
        0..=149 => 0,
        150..=599 => 1,
        600..=1499 => 2,
        _ => 3,
    }
}

/// Raderna samlade per prov (ett djur vid ett tillfälle), i ordningen handling 0–8.
pub fn per_prov(rader: &[Rad]) -> Vec<&[Rad]> {
    assert!(rader.len() % A_N == 0, "antalet rader är inte en multipel av 9");
    let ut: Vec<&[Rad]> = rader.chunks(A_N).collect();
    for p in &ut {
        for (i, r) in p.iter().enumerate() {
            assert!(r.a == i && r.uid == p[0].uid && r.ar == p[0].ar && r.fro == p[0].fro, "raderna ligger inte prov för prov");
        }
        assert_eq!(p.iter().filter(|r| r.vald).count(), 1, "ett prov ska ha exakt en vald handling");
    }
    ut
}

pub fn rapport(rader: &[Rad]) -> String {
    let prov = per_prov(rader);
    let fron: Vec<u64> = { let mut f: Vec<u64> = rader.iter().map(|r| r.fro).collect(); f.sort(); f.dedup(); f };
    let mut t = String::new();
    let _ = writeln!(t, "# Kompletteringsanalys efter Fas 1: träningsfördelning, urvalsbias och överföring (tal räknade av `vmodell_analys rapport`)\n");
    let _ = writeln!(t, "Underlag: {} rader = {} prov (ett djur vid ett tillfälle) × 9 handlingar, {} frön ({}–{}), en art (Edmontosaurus). Felkvot = modellens fel delat med persistensens (\"inget ändras\"); under 1 är bättre än persistensen. R² gäller den del av utfallet som beror på handlingen; 0 = bryr sig inte om handlingen, 1 = fullträff.\n",
        rader.len(), prov.len(), fron.len(), fron.first().unwrap(), fron.last().unwrap());

    // ---------------------------------------------------------------------------------------------------------------- 1
    let _ = writeln!(t, "## 1. Träningsfördelningen (utföranden under de föregående 365 dagarna = {} steg, bara före provet)\n", STEG_PER_AR);
    let _ = writeln!(t, "### Per handling\n\n| Handling | Medel utföranden senaste året | Prov med 0 | 1–9 | 10–49 | 50+ | Medel utföranden under livet |\n|---|---|---|---|---|---|---|");
    for a in 0..A_N {
        let v: Vec<&Rad> = rader.iter().filter(|r| r.a == a).collect();
        let n = v.len() as f64;
        let mut g = [0u64; GRUPPER];
        for r in &v { g[grupp(r.utf365)] += 1; }
        let _ = writeln!(t, "| {} {} | {:.1} | {} | {} | {} | {} | {:.1} |", a, HANDLING[a], v.iter().map(|r| r.utf365 as f64).sum::<f64>() / n, g[0], g[1], g[2], g[3], v.iter().map(|r| r.utfliv as f64).sum::<f64>() / n);
    }
    let _ = writeln!(t, "\n### Per djur ({} prov)\n", prov.len());
    let mut hist = [0u64; A_N + 1];
    let (mut topp, mut alder, mut olika_liv): (Vec<f64>, Vec<f64>, Vec<f64>) = (vec![], vec![], vec![]);
    for p in &prov {
        hist[p.iter().filter(|r| r.utf365 == 0).count()] += 1;
        let tot: u32 = p.iter().map(|r| r.utf365).sum();
        topp.push(p.iter().map(|r| r.utf365).max().unwrap() as f64 / tot.max(1) as f64);
        alder.push(p[0].alder as f64 / STEG_PER_AR as f64);
        olika_liv.push(p.iter().filter(|r| r.utfliv == 0).count() as f64);
    }
    let mut rad = String::from("| Handlingar som djuret inte utfört alls senaste året |");
    for k in 0..=A_N { let _ = write!(rad, " {k} |"); }
    let _ = writeln!(t, "{rad}\n|---|{}", "---|".repeat(A_N + 1));
    let mut rad = String::from("| Antal djur |");
    for k in 0..=A_N { let _ = write!(rad, " {} |", hist[k]); }
    let _ = writeln!(t, "{rad}\n");
    let _ = writeln!(t, "- Den mest utförda handlingens andel av djurets steg senaste året: 10:e percentilen {:.3}, median {:.3}, 90:e percentilen {:.3}.", kvantil(&mut topp.clone(), 0.1), kvantil(&mut topp.clone(), 0.5), kvantil(&mut topp, 0.9));
    let _ = writeln!(t, "- Djurens ålder i år: 10:e percentilen {:.1}, median {:.1}, 90:e percentilen {:.1}.", kvantil(&mut alder.clone(), 0.1), kvantil(&mut alder.clone(), 0.5), kvantil(&mut alder, 0.9));
    let _ = writeln!(t, "- Handlingar som djuret aldrig utfört under livet: medel {:.2} av 9.", olika_liv.iter().sum::<f64>() / olika_liv.len() as f64);
    let _ = writeln!(t, "\n### Per art\n\nEn art finns i simuleringen: Edmontosaurus ({} prov). Alla tal i rapporten gäller den arten; en uppdelning mellan arter går inte att göra förrän fler arter har en hjärna.\n", prov.len());

    // ---------------------------------------------------------------------------------------------------------------- 2
    let _ = writeln!(t, "## 2. Prediktionsfel per erfarenhetsgrupp (hjärnan i dag)\n");
    for (namn, liv) in [("utföranden senaste året", false), ("utföranden under hela livet (= antalet träningar av förutsägaren)", true)] {
        let _ = writeln!(t, "Grupperad efter {namn}:\n\n| Grupp | Antal | Medelfel förutsägelse | Medelfel persistens | Felkvot | R² |\n|---|---|---|---|---|---|");
        for g in 0..GRUPPER {
            let s = summa(rader.iter().filter(|r| grupp(if liv { r.utfliv } else { r.utf365 }) == g), 0);
            let _ = writeln!(t, "| {} | {} | {:.4} | {:.4} | {:.3} | {:.3} |", GRUPP_NAMN[g], s.n, s.fel / s.n.max(1) as f64, s.fel_p / s.n.max(1) as f64, s.kvot(), s.r2());
        }
        let _ = writeln!(t);
    }
    let _ = writeln!(t, "Felkvot per handling och grupp (senaste året; antal inom parentes):\n");
    let mut rad = String::from("| Grupp |");
    for a in 0..A_N { let _ = write!(rad, " {a} |"); }
    let _ = writeln!(t, "{rad}\n|---|{}", "---|".repeat(A_N));
    for g in 0..GRUPPER {
        let mut rad = format!("| {} |", GRUPP_NAMN[g]);
        for a in 0..A_N { let _ = write!(rad, " {} |", summa(rader.iter().filter(|r| r.a == a && grupp(r.utf365) == g), 0).cell()); }
        let _ = writeln!(t, "{rad}");
    }

    // ---------------------------------------------------------------------------------------------------------------- 3
    let _ = writeln!(t, "\n## 3. Urvalsbias (hjärnan i dag)\n");
    let _ = writeln!(t, "\"Handlingen som modellen förutsäger som bäst\" är den valda handlingen: hjärnan väljer alltid den handling vars förutsagda bild har högst värde (ingen slump i valet).\n");
    let _ = writeln!(t, "### 3a. Fem slag av handlingar\n\n| Slag | Antal | Felkvot | R² | Medel utföranden senaste året |\n|---|---|---|---|---|");
    let mest = |p: &[Rad], minst: bool| -> usize {
        let mut b = 0;
        for a in 1..A_N {
            let (x, y) = ((p[a].utf365, p[a].utfliv), (p[b].utf365, p[b].utfliv));
            if (!minst && x > y) || (minst && x < y) { b = a; }
        }
        b
    };
    let mut slag: Vec<(&str, Vec<&Rad>)> = vec![("Vald handling (= modellens bästa)", vec![]), ("Icke vald handling", vec![]), ("Flest erfarenheter senaste året", vec![]), ("Minst erfarenheter senaste året", vec![]), ("Förra stegets handling", vec![])];
    for p in &prov {
        for r in p.iter() {
            if r.vald { slag[0].1.push(r); } else { slag[1].1.push(r); }
            if r.forra { slag[4].1.push(r); }
        }
        slag[2].1.push(&p[mest(p, false)]);
        slag[3].1.push(&p[mest(p, true)]);
    }
    for (namn, v) in &slag {
        let s = summa(v.iter().copied(), 0);
        let _ = writeln!(t, "| {} | {} | {:.3} | {:.3} | {:.1} |", namn, s.n, s.kvot(), s.r2(), v.iter().map(|r| r.utf365 as f64).sum::<f64>() / v.len().max(1) as f64);
    }
    let _ = writeln!(t, "\n### 3b. Sannolikheten att en handling väljs\n\n| Erfarenhetsgrupp (senaste året) | Antal | Andel valda |\n|---|---|---|");
    for g in 0..GRUPPER {
        let v: Vec<&Rad> = rader.iter().filter(|r| grupp(r.utf365) == g).collect();
        let _ = writeln!(t, "| {} | {} | {:.3} |", GRUPP_NAMN[g], v.len(), v.iter().filter(|r| r.vald).count() as f64 / v.len().max(1) as f64);
    }
    let _ = writeln!(t, "\nSlumpen ger 0,111. Rang 1 = handlingen med lägst verkligt fel bland djurets nio i provet:\n\n| Felets rang | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |\n|---|---|---|---|---|---|---|---|---|---|");
    let mut rang = [0u64; A_N];
    for p in &prov {
        let v = p.iter().find(|r| r.vald).unwrap();
        rang[p.iter().filter(|r| r.fel[0] < v.fel[0]).count()] += 1;
    }
    let mut rad = String::from("| Andel av de valda |");
    for k in 0..A_N { let _ = write!(rad, " {:.3} |", rang[k] as f64 / prov.len() as f64); }
    let _ = writeln!(t, "{rad}");
    let _ = writeln!(t, "\n### 3c. Vald mot icke vald inom samma erfarenhetsgrupp (felkvot; antal inom parentes)\n\n| Grupp (senaste året) | Vald | Icke vald | Vald, inte förra stegets | Icke vald, inte förra stegets |\n|---|---|---|---|---|");
    for g in 0..GRUPPER {
        let f = |vald: bool, utan_forra: bool| summa(rader.iter().filter(|r| grupp(r.utf365) == g && r.vald == vald && !(utan_forra && r.forra)), 0).cell();
        let _ = writeln!(t, "| {} | {} | {} | {} | {} |", GRUPP_NAMN[g], f(true, false), f(false, false), f(true, true), f(false, true));
    }
    let _ = writeln!(t, "\n### 3d. Förra stegets handling och valet (felkvot; antal inom parentes)\n\n| | Vald nu | Inte vald nu |\n|---|---|---|");
    for (namn, fo) in [("Var förra stegets handling", true), ("Var inte förra stegets handling", false)] {
        let f = |vald: bool| summa(rader.iter().filter(|r| r.forra == fo && r.vald == vald), 0).cell();
        let _ = writeln!(t, "| {} | {} | {} |", namn, f(true), f(false));
    }
    let _ = writeln!(t, "\nFörra stegets handling är den enda vars förutsägare tränas i själva provsteget, på bilden djuret just ser, innan den förutsäger.\n");
    let _ = writeln!(t, "### 3e. Hur länge sedan handlingen utfördes (bara handlingar som INTE är valda nu; felkvot)\n\n| Steg sedan senast | Antal | Felkvot | R² |\n|---|---|---|---|");
    for g in 0..5 {
        let s = summa(rader.iter().filter(|r| !r.vald && sedan_grupp(r.sedan) == g), 0);
        let _ = writeln!(t, "| {} | {} | {:.3} | {:.3} |", SEDAN_NAMN[g], s.n, s.kvot(), s.r2());
    }
    // 3f: parat inom samma djur, bland handlingar som varken är valda nu eller förra stegets
    let _ = writeln!(t, "\n### 3f. Parat inom samma djur: den mest mot den minst tränade handlingen (bland handlingar som varken är valda nu eller var förra stegets)\n");
    let (mut par, mut lagre, mut s_mest, mut s_minst) = (0u64, 0u64, Sum::default(), Sum::default());
    let mut per_fro: HashMap<u64, (u64, u64)> = HashMap::new();
    for p in &prov {
        let k: Vec<&Rad> = p.iter().filter(|r| !r.vald && !r.forra).collect();
        let (mx, mn) = (k.iter().max_by_key(|r| r.utfliv).unwrap(), k.iter().min_by_key(|r| r.utfliv).unwrap());
        if mx.utfliv == mn.utfliv { continue; }
        par += 1;
        s_mest.lagg(mx, 0);
        s_minst.lagg(mn, 0);
        let e = per_fro.entry(p[0].fro).or_default();
        e.0 += 1;
        if mx.fel[0] < mn.fel[0] { lagre += 1; e.1 += 1; }
    }
    let andelar: Vec<f64> = fron.iter().map(|f| per_fro.get(f).map(|e| e.1 as f64 / e.0.max(1) as f64).unwrap_or(0.0)).collect();
    let _ = writeln!(t, "- {} par. Den mest tränade handlingen har lägre fel i {:.3} av paren (0,5 = ingen skillnad). Per frö: lägst {:.3}, median {:.3}, högst {:.3}; frön över 0,5: {} av {}.", par, lagre as f64 / par.max(1) as f64,
        andelar.iter().cloned().fold(f64::INFINITY, f64::min), median(andelar.clone()), andelar.iter().cloned().fold(0.0, f64::max), andelar.iter().filter(|&&x| x > 0.5).count(), fron.len());
    let _ = writeln!(t, "- Felkvot för den mest tränade: {:.3} (medel {:.0} träningar); för den minst tränade: {:.3} (medel {:.0} träningar).", s_mest.kvot(), 0.0 + prov.iter().filter_map(|p| { let k: Vec<&Rad> = p.iter().filter(|r| !r.vald && !r.forra).collect(); let (a, b) = (k.iter().map(|r| r.utfliv).max()?, k.iter().map(|r| r.utfliv).min()?); if a == b { None } else { Some(a as f64) } }).sum::<f64>() / par.max(1) as f64,
        s_minst.kvot(), prov.iter().filter_map(|p| { let k: Vec<&Rad> = p.iter().filter(|r| !r.vald && !r.forra).collect(); let (a, b) = (k.iter().map(|r| r.utfliv).max()?, k.iter().map(|r| r.utfliv).min()?); if a == b { None } else { Some(b as f64) } }).sum::<f64>() / par.max(1) as f64);
    let _ = writeln!(t, "\n### 3g. Mekanismen i koden, kontrollerad i data\n");
    let (noll, noll_or) = (rader.iter().filter(|r| r.utfliv == 0).count(), rader.iter().filter(|r| r.utfliv == 0 && r.ororda).count());
    let (flera, flera_or) = (rader.iter().filter(|r| r.utfliv > 0).count(), rader.iter().filter(|r| r.utfliv > 0 && r.ororda).count());
    let _ = writeln!(t, "- Handlingar som djuret aldrig utfört: {} rader; förutsägarens vikter är bit för bit de ärvda i {} av dem.", noll, noll_or);
    let _ = writeln!(t, "- Handlingar som djuret utfört minst en gång: {} rader; vikterna är fortfarande de ärvda i {} av dem.", flera, flera_or);

    // ---------------------------------------------------------------------------------------------------------------- 4
    let _ = writeln!(t, "\n## 4. Överföring mellan handlingar\n");
    let _ = writeln!(t, "Frågan: blir förutsägelsen för en handling som djuret sällan eller aldrig utfört bättre när djuret har mer erfarenhet av ANDRA handlingar? Ingen modell är tränad på utfallen i kopiorna.\n");
    for (rubrik, urval) in [("Handlingar som djuret aldrig utfört under livet", 0u32), ("Handlingar som djuret utfört 1–9 gånger under livet", 9u32)] {
        let _ = writeln!(t, "### {rubrik}: felkvot efter djurets erfarenhet av andra handlingar (steg under livet; antal inom parentes)\n\n| Modell |{} Alla | R² alla |\n|---|{}---|---|", ANDRA_NAMN.iter().map(|x| format!(" {x} |")).collect::<String>(), "---|".repeat(4));
        for m in 0..MODELLER {
            let mut rad = format!("| {} |", MODELL_NAMN[m]);
            let mut alla = Sum::default();
            for g in 0..4 {
                let mut s = Sum::default();
                for p in &prov {
                    let tot: u32 = p.iter().map(|r| r.utfliv).sum();
                    for r in p.iter().filter(|r| if urval == 0 { r.utfliv == 0 } else { (1..=9).contains(&r.utfliv) }) {
                        if andra_grupp(tot - r.utfliv) == g { s.lagg(r, m); alla.lagg(r, m); }
                    }
                }
                let _ = write!(rad, " {} |", s.cell());
            }
            let _ = writeln!(t, "{rad} {} | {} |", alla.cell(), if alla.n == 0 { "–".into() } else { format!("{:.3}", alla.r2()) });
        }
        let _ = writeln!(t);
    }
    let _ = writeln!(t, "### Alla modeller per erfarenhetsgrupp (utföranden under livet; felkvot, R² och antal)\n\n| Modell | Mått |{}\n|---|---|{}", GRUPP_NAMN.iter().map(|g| format!(" {g} |")).collect::<String>(), "---|".repeat(GRUPPER));
    for m in 0..MODELLER {
        let s: Vec<Sum> = (0..GRUPPER).map(|g| summa(rader.iter().filter(|r| grupp(r.utfliv) == g), m)).collect();
        let _ = writeln!(t, "| {} | felkvot (antal) |{}", MODELL_NAMN[m], s.iter().map(|x| format!(" {} |", x.cell())).collect::<String>());
        let _ = writeln!(t, "| | R² |{}", s.iter().map(|x| if x.n == 0 { " – |".to_string() } else { format!(" {:.3} |", x.r2()) }).collect::<String>());
    }
    let _ = writeln!(t, "\n### Modellerna över alla rader och per frö\n\n| Modell | Felkvot | R² | R² per frö: lägst | median | högst | Frön med R² över 0 |\n|---|---|---|---|---|---|---|");
    let mut r2_per_modell: Vec<Vec<f64>> = vec![];
    for m in 0..MODELLER {
        let s = summa(rader.iter(), m);
        let pf: Vec<f64> = fron.iter().map(|&f| summa(rader.iter().filter(|r| r.fro == f), m).r2()).collect();
        let _ = writeln!(t, "| {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} av {} |", MODELL_NAMN[m], s.kvot(), s.r2(), pf.iter().cloned().fold(f64::INFINITY, f64::min), median(pf.clone()), pf.iter().cloned().fold(f64::NEG_INFINITY, f64::max), pf.iter().filter(|&&x| x > 0.0).count(), fron.len());
        r2_per_modell.push(pf);
    }
    for m in 2..MODELLER {
        let b = (0..fron.len()).filter(|&i| r2_per_modell[m][i] > r2_per_modell[1][i]).count();
        let _ = writeln!(t, "\n{} har högre R² än \"nio skilda utan ärvd start\" i {} av {} frön.", MODELL_NAMN[m], b, fron.len());
    }

    // ---------------------------------------------------------------------------------------------------------------- 5
    let _ = writeln!(t, "\n## 5. Nyckeltal för de tre hypoteserna (reglerna skrevs före körningen)\n");
    let ej: Vec<Sum> = (0..GRUPPER).map(|g| summa(rader.iter().filter(|r| !r.vald && !r.forra && grupp(r.utfliv) == g), 0)).collect();
    let fall = (1..GRUPPER).all(|g| ej[g].n == 0 || ej[g - 1].n == 0 || ej[g].kvot() < ej[g - 1].kvot());
    let fall_fron = fron.iter().filter(|&&f| {
        let e: Vec<Sum> = (0..GRUPPER).map(|g| summa(rader.iter().filter(|r| r.fro == f && !r.vald && !r.forra && grupp(r.utfliv) == g), 0)).collect();
        (1..GRUPPER).all(|g| e[g].n == 0 || e[g - 1].n == 0 || e[g].kvot() < e[g - 1].kvot())
    }).count();
    let _ = writeln!(t, "- **A (för lite träning).** Felkvoten bland handlingar som varken är valda nu eller förra stegets, per träningsgrupp under livet (0, 1–9, 10–49, 50+): {}. Faller den i varje steg: {} (i {} av {} frön). Parat inom djur har den mest tränade lägre fel i {:.3} av paren (regeln: minst 0,60). Felkvoten i gruppen 50+ är {:.3} (regeln: under 1 för att träning ska räcka).",
        ej.iter().map(|s| s.cell()).collect::<Vec<_>>().join(", "), if fall { "ja" } else { "nej" }, fall_fron, fron.len(), lagre as f64 / par.max(1) as f64, ej[3].kvot());
    let mut bgr = vec![];
    for g in 0..GRUPPER {
        let v = summa(rader.iter().filter(|r| grupp(r.utf365) == g && r.vald && !r.forra), 0);
        let o = summa(rader.iter().filter(|r| grupp(r.utf365) == g && !r.vald && !r.forra), 0);
        if v.n >= 30 && o.n >= 30 { bgr.push(format!("{}: vald {:.3}, icke vald {:.3}, vald/icke vald {:.2}", GRUPP_NAMN[g], v.kvot(), o.kvot(), v.kvot() / o.kvot())); }
    }
    let _ = writeln!(t, "- **B (urvalsbias).** Vald mot icke vald inom samma erfarenhetsgrupp (senaste året), förra stegets handling borttagen, grupper med minst 30 rader av varje slag: {}. Regeln: stöd om den valda har minst 20 % lägre felkvot (kvot högst 0,80) också här.", if bgr.is_empty() { "inga grupper med nog rader".into() } else { bgr.join("; ") });
    let _ = writeln!(t, "- **C (ingen gemensam representation).** Se tabellerna i avsnitt 4: regeln är stöd om en gemensam form har högre R² än nio skilda utan ärvd start i minst 15 av 20 frön, eller R² över 0 för handlingar som djuret aldrig utfört.");
    let iv = summa(rader.iter().filter(|r| r.vald), 0);
    let _ = writeln!(t, "\nValet och födan: intaget efter den valda handlingen är {:.4} i medel; över alla nio {:.4}. Medel av hjärnans förutsagda värde för den valda: {:.4}.", rader.iter().filter(|r| r.vald).map(|r| r.intag).sum::<f64>() / iv.n.max(1) as f64, rader.iter().map(|r| r.intag).sum::<f64>() / rader.len() as f64, rader.iter().filter(|r| r.vald).map(|r| r.v_pred).sum::<f64>() / iv.n.max(1) as f64);
    t
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    let v = |n: &str| arg.iter().position(|a| a == n).and_then(|i| arg.get(i + 1)).cloned();
    match arg.first().map(|x| x.as_str()) {
        Some("kor") => {
            let fron = v("--fron").expect("--fron A-B");
            let (a, b) = fron.split_once('-').expect("--fron A-B");
            let (a, b): (u64, u64) = (a.parse().expect("frö"), b.parse().expect("frö"));
            let ar: Vec<i64> = v("--ar").expect("--ar 40,47,54").split(',').map(|x| x.parse().expect("år")).collect();
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
                    let r = r.expect("körningen föll");
                    std::fs::write(ut.join(format!("rader-{}.tsv", r.fro)), format!("{RUBRIK}\n{}\n", r.rader.join("\n"))).expect("skriv raderna");
                    svar.push(r);
                }
            }
            std::fs::write(ut.join("kontrolltabeller.md"), kontrolltabeller(&svar)).expect("skriv kontrollen");
            eprintln!("KLAR alla frön");
        }
        Some("rapport") => {
            let mapp = std::path::PathBuf::from(v("--in").expect("--in MAPP"));
            let mut filer: Vec<_> = std::fs::read_dir(&mapp).expect("läs mappen").filter_map(|e| e.ok()).map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("rader-") && n.ends_with(".tsv")).unwrap_or(false)).collect();
            filer.sort();
            assert!(!filer.is_empty(), "tom läsning: inga radfiler i {}", mapp.display());
            let mut rader = Vec::new();
            for f in &filer {
                let text = std::fs::read_to_string(f).expect("läs radfilen");
                let mut l = text.lines();
                assert_eq!(l.next(), Some(RUBRIK), "fel rubrik i {}", f.display());
                rader.extend(l.filter(|x| !x.is_empty()).map(las_rad));
            }
            assert!(!rader.is_empty(), "tom läsning: inga rader");
            let text = rapport(&rader);
            std::fs::write(mapp.join("vmodell-analys-resultat.md"), &text).expect("skriv rapporten");
            println!("{text}");
        }
        _ => panic!("ange kor eller rapport"),
    }
}

#[cfg(test)]
mod prov {
    use super::*;

    fn rad(a: usize, vald: bool, forra: bool, utf: u32, fel: f64) -> String {
        format!("261\t40\t7\t900\t{a}\t{}\t{}\t{utf}\t{utf}\t{}\t{}\t1e0\t{fel:e}\t5e-1\t1e0\t1e0\t1e0\t1e0\t1e0\t1e0\t1e0\t2e-1\t1e-1\t1.5e0\t{}",
            vald as u8, forra as u8, if utf == 0 { -1 } else { 3 }, (utf == 0) as u8, (a != 0) as u8)
    }

    /// Läsaren prövad på rader i programmets eget format, och måtten på ett känt litet fall.
    #[test]
    fn lasaren_och_summorna() {
        let rader: Vec<Rad> = (0..A_N).map(|a| las_rad(&rad(a, a == 2, a == 4, [0, 5, 20, 60, 0, 0, 3, 100, 12][a], if a == 2 { 0.5 } else { 4.0 }))).collect();
        assert_eq!(rader[2].a, 2);
        assert!(rader[2].vald && !rader[3].vald && rader[4].forra);
        assert_eq!((rader[1].utf365, rader[1].utfliv, rader[0].sedan, rader[1].sedan), (5, 5, -1, 3));
        assert!(rader[0].ororda && !rader[1].ororda);
        assert_eq!(rader[2].fel, [0.5, 1.0, 1.0, 1.0]);
        let p = per_prov(&rader);
        assert_eq!(p.len(), 1);
        let v = summa(rader.iter().filter(|r| r.vald), 0);
        let o = summa(rader.iter().filter(|r| !r.vald), 0);
        assert_eq!((v.n, o.n), (1, 8));
        assert!((v.kvot() - 0.5).abs() < 1e-12 && (o.kvot() - 4.0).abs() < 1e-12);
        assert!((v.r2() - 0.5).abs() < 1e-12, "R² = 1 − rest/verklig = 1 − 0,5/1");
        assert_eq!([grupp(rader[0].utf365), grupp(rader[1].utf365), grupp(rader[2].utf365), grupp(rader[3].utf365)], [0, 1, 2, 3]);
        assert_eq!([sedan_grupp(-1), sedan_grupp(1), sedan_grupp(9), sedan_grupp(10), sedan_grupp(150)], [4, 0, 1, 2, 3]);
        assert_eq!([andra_grupp(149), andra_grupp(150), andra_grupp(600), andra_grupp(1500)], [0, 1, 2, 3]);
        let text = rapport(&rader);
        assert!(text.contains("## 5. Nyckeltal"));
    }

    /// Ett prov med två valda handlingar, eller rader i fel ordning, ska stoppa rapporten.
    #[test]
    #[should_panic(expected = "exakt en vald")]
    fn tva_valda_stoppar() {
        let rader: Vec<Rad> = (0..A_N).map(|a| las_rad(&rad(a, a == 2 || a == 3, false, 1, 1.0))).collect();
        per_prov(&rader);
    }
}
