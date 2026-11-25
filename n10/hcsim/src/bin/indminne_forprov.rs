//! indminne_forprov: förprovet för platsminnet utan minne i hjärnan (Håkans Alt 1943, 2026-11-23).
//! Mått och gränser (skrivna före körningen): piharness `docs/evolution/indminne-1943-utfallsregel.md`.
//! EXTERN AVLÄSNING: skuggtabellen läser bara simuleringsläget och påverkar aldrig något val, någon hjärna eller någon värld.
//! Användning:
//!   indminne_forprov kor --fron 261-280 --ar 40 --steg 450 --djur 200 --tradar 3 --ut MAPP
//! Rust (AGENTS.md regel 11).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::sim::{KantLage, Sim, GRANNAR};
use hcsim::tal::STEG_PER_AR;
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::Rng;

const KLASSER: [(i64, i64, &str); 5] = [
    (2, 5, "2–5"),
    (6, 20, "6–20"),
    (21, 60, "21–60"),
    (61, 150, "61–150"),
    (151, i64::MAX, "151+"),
];

#[derive(Clone, Default)]
struct Ack {
    n: u64,
    fel_tab_v: f64,
    fel_pers_v: f64,
    fel_mark_v: f64,
    fel_tab_n: f64,
    fel_pers_n: f64,
    fel_mark_n: f64,
}

impl Ack {
    fn lagg(&mut self, tv: f64, pv: f64, mv: f64, tn: f64, pn: f64, mn: f64) {
        self.n += 1;
        self.fel_tab_v += tv;
        self.fel_pers_v += pv;
        self.fel_mark_v += mv;
        self.fel_tab_n += tn;
        self.fel_pers_n += pn;
        self.fel_mark_n += mn;
    }

    fn merge(&mut self, o: &Ack) {
        self.n += o.n;
        self.fel_tab_v += o.fel_tab_v;
        self.fel_pers_v += o.fel_pers_v;
        self.fel_mark_v += o.fel_mark_v;
        self.fel_tab_n += o.fel_tab_n;
        self.fel_pers_n += o.fel_pers_n;
        self.fel_mark_n += o.fel_mark_n;
    }

    fn kvot_v(&self) -> f64 {
        self.fel_tab_v / self.fel_pers_v.max(1e-300)
    }
    fn kvot_n(&self) -> f64 {
        self.fel_tab_n / self.fel_pers_n.max(1e-300)
    }
    fn r2_pers_v(&self) -> f64 {
        1.0 - self.fel_tab_v / self.fel_pers_v.max(1e-300)
    }
    fn r2_mark_v(&self) -> f64 {
        1.0 - self.fel_tab_v / self.fel_mark_v.max(1e-300)
    }
    fn r2_pers_n(&self) -> f64 {
        1.0 - self.fel_tab_n / self.fel_pers_n.max(1e-300)
    }
    fn r2_mark_n(&self) -> f64 {
        1.0 - self.fel_tab_n / self.fel_mark_n.max(1e-300)
    }
}

#[derive(Clone, Debug)]
struct MinnesPost {
    uid: u64,
    fodelsesteg: i64,
    t_sedd: i64,
    v_norm: f64,
    n_norm: f64,
}

struct DjurSkugga {
    slot: usize,
    uid: u64,
    fodelsesteg: i64,
    aktiv: bool,
    forra_pos: (usize, usize),
    forra_egna: (f64, f64),
    forra_fonster: [usize; 9],
    minne: HashMap<usize, MinnesPost>,
    mark_sum: [(f64, f64, u64); 6],
}

impl DjurSkugga {
    fn ny(slot: usize, uid: u64, fodelsesteg: i64, pos: (usize, usize), egna: (f64, f64), fonster: [usize; 9]) -> Self {
        Self {
            slot,
            uid,
            fodelsesteg,
            aktiv: true,
            forra_pos: pos,
            forra_egna: egna,
            forra_fonster: fonster,
            minne: HashMap::new(),
            mark_sum: [(0.0, 0.0, 0); 6],
        }
    }

    fn dod(&mut self) {
        self.aktiv = false;
        self.minne.clear();
        self.mark_sum = [(0.0, 0.0, 0); 6];
    }

    fn mark_medel(&self, mark_id: usize, fallback: (f64, f64)) -> (f64, f64) {
        let (sv, sn, cnt) = self.mark_sum[mark_id];
        if cnt == 0 {
            fallback
        } else {
            (sv / cnt as f64, sn / cnt as f64)
        }
    }
}

fn djur_antal_per_cell(sim: &Sim) -> Vec<usize> {
    let mut antal = vec![0usize; sim.b * sim.h];
    for d in sim.djur.iter().filter(|d| d.lev) {
        antal[sim.cell(d.i, d.j)] += 1;
    }
    antal
}

fn fonster_celler(sim: &Sim, i: usize, j: usize) -> [usize; 9] {
    let mut ut = [0usize; 9];
    for (k, &(di, dj)) in GRANNAR.iter().enumerate() {
        let (ni, nj) = sim.kant_granne(i as i32 + di, j as i32 + dj);
        ut[k] = sim.cell(ni as usize, nj as usize);
    }
    ut
}

fn cell_varden(sim: &Sim, antal: &[usize], c: usize) -> (f64, f64, usize) {
    let tak = sim.vmax[c].max(1e-12);
    let v_norm = sim.v[c] / tak;
    let n_norm = antal[c] as f64 / 4.0;
    let mark = (sim.vatten[c] as usize).min(5);
    (v_norm, n_norm, mark)
}

fn uppdatera_skugga(sk: &mut DjurSkugga, sim: &Sim, antal: &[usize], fonster: [usize; 9], t: i64) {
    for &c in &fonster {
        let (vn, nn, mark) = cell_varden(sim, antal, c);
        sk.minne.insert(
            c,
            MinnesPost {
                uid: sk.uid,
                fodelsesteg: sk.fodelsesteg,
                t_sedd: t,
                v_norm: vn,
                n_norm: nn,
            },
        );
        let entry = &mut sk.mark_sum[mark];
        entry.0 += vn;
        entry.1 += nn;
        entry.2 += 1;
    }
}

fn bygg_varld(rot: &Path, fro: u64) -> Result<Sim, String> {
    let arm = utforsk_armar()
        .into_iter()
        .find(|a| a.namn == "e_lar" && !a.spegel)
        .ok_or("armen e_lar saknas")?;
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv")).map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    let mut sim = bygg_sim_balte(&kalla, &tt, B_FULL, H_FULL, BALTE_K, fro, &arm)?;
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    satt_rattningar_utdod(&mut sim);
    Ok(sim)
}

struct VarldUtfall {
    fro: u64,
    valda: usize,
    kvar: usize,
    tot: Ack,
    klasser: Vec<Ack>,
    djur_summa_ok: bool,
    sek: f64,
}

fn kor_varld(rot: &Path, fro: u64, ar: i64, n_djur: usize, steg: i64, kontrollera_summa: bool) -> Result<VarldUtfall, String> {
    let t0 = std::time::Instant::now();
    let mut sim = bygg_varld(rot, fro)?;
    while sim.t < ar * STEG_PER_AR {
        sim.steg();
    }
    let mut referens = if kontrollera_summa { Some(sim.clone()) } else { None };

    let mut kand: Vec<usize> = (0..sim.djur.len())
        .filter(|&idx| {
            let d = &sim.djur[idx];
            d.lev && d.age >= STEG_PER_AR
        })
        .collect();
    let mut g = Rng::new(fro ^ 0x1943_1943);
    g.blanda(&mut kand);
    kand.truncate(n_djur);

    let antal0 = djur_antal_per_cell(&sim);
    let mut skuggor: Vec<DjurSkugga> = Vec::with_capacity(kand.len());
    for idx in kand {
        let d = &sim.djur[idx];
        let fodelsesteg = sim.t - d.age;
        let fonster = fonster_celler(&sim, d.i, d.j);
        let c0 = sim.cell(d.i, d.j);
        let (v0, n0, _) = cell_varden(&sim, &antal0, c0);
        let mut sk = DjurSkugga::ny(idx, d.uid, fodelsesteg, (d.i, d.j), (v0, n0), fonster);
        uppdatera_skugga(&mut sk, &sim, &antal0, fonster, sim.t);
        skuggor.push(sk);
    }

    let mut tot = Ack::default();
    let mut klasser = vec![Ack::default(); KLASSER.len()];

    for _ in 0..steg {
        sim.steg();
        let antal = djur_antal_per_cell(&sim);
        let nu_t = sim.t;
        for sk in &mut skuggor {
            if !sk.aktiv {
                continue;
            }
            let d = &sim.djur[sk.slot];
            let f_nu = nu_t - d.age;
            if !d.lev || d.uid != sk.uid || f_nu != sk.fodelsesteg {
                sk.dod();
                continue;
            }
            let fonster = fonster_celler(&sim, d.i, d.j);
            let pos = (d.i, d.j);
            if pos != sk.forra_pos {
                let (pv, pn) = sk.forra_egna;
                for &c in &fonster {
                    if sk.forra_fonster.contains(&c) {
                        continue;
                    }
                    if let Some(post) = sk.minne.get(&c) {
                        assert_eq!(post.uid, sk.uid, "L3 korsläcka");
                        assert_eq!(post.fodelsesteg, sk.fodelsesteg, "L3 korsläcka");
                        assert!(post.t_sedd < nu_t, "L2 framtidsläcka");
                        let dt = nu_t - post.t_sedd;
                        if dt >= 2 {
                            let (tv, tn, mark) = cell_varden(&sim, &antal, c);
                            let (mv, mn) = sk.mark_medel(mark, (pv, pn));
                            let ev_t = (post.v_norm - tv).powi(2);
                            let ev_p = (pv - tv).powi(2);
                            let ev_m = (mv - tv).powi(2);
                            let en_t = (post.n_norm - tn).powi(2);
                            let en_p = (pn - tn).powi(2);
                            let en_m = (mn - tn).powi(2);
                            tot.lagg(ev_t, ev_p, ev_m, en_t, en_p, en_m);
                            for (ki, &(lo, hi, _)) in KLASSER.iter().enumerate() {
                                if dt >= lo && dt <= hi {
                                    klasser[ki].lagg(ev_t, ev_p, ev_m, en_t, en_p, en_m);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            let c0 = sim.cell(d.i, d.j);
            let (v0, n0, _) = cell_varden(&sim, &antal, c0);
            sk.forra_pos = pos;
            sk.forra_egna = (v0, n0);
            sk.forra_fonster = fonster;
            uppdatera_skugga(sk, &sim, &antal, fonster, nu_t);
        }
    }

    let djur_summa_ok = if let Some(ref mut ref_sim) = referens {
        while ref_sim.t < sim.t {
            ref_sim.steg();
        }
        ref_sim.djur_summa() == sim.djur_summa() && ref_sim.g.tillstand() == sim.g.tillstand()
    } else {
        true
    };

    let valda = skuggor.len();
    let kvar = skuggor.iter().filter(|s| s.aktiv).count();
    Ok(VarldUtfall {
        fro,
        valda,
        kvar,
        tot,
        klasser,
        djur_summa_ok,
        sek: t0.elapsed().as_secs_f64(),
    })
}

fn intervall(v: &[f64]) -> (f64, f64, f64) {
    let n = v.len();
    let medel = v.iter().sum::<f64>() / n as f64;
    let mut g = Rng::new(1943);
    let mut m: Vec<f64> = (0..2000).map(|_| (0..n).map(|_| v[g.under(n)]).sum::<f64>() / n as f64).collect();
    m.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (medel, m[49], m[1949])
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn f3(x: f64) -> String {
    format!("{x:.3}").replace('.', ",")
}

fn parse_fron(s: &str) -> Result<Vec<u64>, String> {
    if let Some((a, b)) = s.split_once('-') {
        let lo: u64 = a.parse().map_err(|_| format!("fel frö: {s}"))?;
        let hi: u64 = b.parse().map_err(|_| format!("fel frö: {s}"))?;
        Ok((lo..=hi).collect())
    } else {
        s.split(',').map(|x| x.parse::<u64>().map_err(|_| format!("fel frö: {x}"))).collect()
    }
}

fn rapport(utfall: &[VarldUtfall]) -> String {
    let n = utfall.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Förprovet för platsminnet utan minne i hjärnan (Alt 1943): EXTERN AVLÄSNING\n");
    let _ = writeln!(t, "Regler skrivna före körningen: piharness `docs/evolution/indminne-1943-utfallsregel.md`. {n} världar; kravet är minst {krav} av {n}.\n");

    let mut summa_obs = 0u64;
    let mut summa_ok_count = 0usize;
    for u in utfall {
        summa_obs += u.tot.n;
        if u.djur_summa_ok {
            summa_ok_count += 1;
        }
    }
    let _ = writeln!(t, "## Kontroll\n- Kontrollsumma (L4 djur_summa lika som utan skugga): {summa_ok_count} av {n} verifierade.\n- Totalt antal återbesöksobservationer (dt ≥ 2): {summa_obs}.\n");

    let _ = writeln!(t, "## Per värld (alla frånvarotider ≥ 2 steg)\n");
    let _ = writeln!(t, "| Frö | Valda / kvar | Återbesök | Växt kvot (tab/pers) | Växt R² (pers) | Växt R² (mark) | Antal kvot (tab/pers) | Antal R² (pers) | Antal R² (mark) | Sek |\n|---|---|---|---|---|---|---|---|---|---|");
    for u in utfall {
        let _ = writeln!(
            t,
            "| {} | {} / {} | {} | {} | {} | {} | {} | {} | {} | {:.0} |",
            u.fro,
            u.valda,
            u.kvar,
            u.tot.n,
            f3(u.tot.kvot_v()),
            f3(u.tot.r2_pers_v()),
            f3(u.tot.r2_mark_v()),
            f3(u.tot.kvot_n()),
            f3(u.tot.r2_pers_n()),
            f3(u.tot.r2_mark_n()),
            u.sek
        );
    }

    let kv: Vec<f64> = utfall.iter().map(|u| u.tot.kvot_v()).collect();
    let kn: Vec<f64> = utfall.iter().map(|u| u.tot.kvot_n()).collect();
    let rpv: Vec<f64> = utfall.iter().map(|u| u.tot.r2_pers_v()).collect();
    let rmv: Vec<f64> = utfall.iter().map(|u| u.tot.r2_mark_v()).collect();
    let rpn: Vec<f64> = utfall.iter().map(|u| u.tot.r2_pers_n()).collect();
    let rmn: Vec<f64> = utfall.iter().map(|u| u.tot.r2_mark_n()).collect();

    let (m_kv, lo_kv, hi_kv) = intervall(&kv);
    let (m_kn, lo_kn, hi_kn) = intervall(&kn);
    let (m_rpv, lo_rpv, hi_rpv) = intervall(&rpv);
    let (m_rmv, lo_rmv, hi_rmv) = intervall(&rmv);
    let (m_rpn, lo_rpn, hi_rpn) = intervall(&rpn);
    let (m_rmn, lo_rmn, hi_rmn) = intervall(&rmn);

    let pass_v = kv.iter().filter(|&&x| x <= 0.95).count();
    let pass_n = kn.iter().filter(|&&x| x <= 0.95).count();

    let _ = writeln!(t, "\n## Sammanfattat över världarna (95 %-intervall ur 2 000 omdragningar)\n");
    let _ = writeln!(t, "| Sinnesgrupp | Mått | Medel [95 %-intervall] | Median | Lägst / högst | Världar ≤ 0,95 (≥ 5 % vinst) |\n|---|---|---|---|---|---|");
    let _ = writeln!(t, "| Växt | Felkvot mot persistens | {} [{}; {}] | {} | {} / {} | {} av {n} |", f3(m_kv), f3(lo_kv), f3(hi_kv), f3(median(kv.clone())), f3(kv.iter().cloned().fold(f64::INFINITY, f64::min)), f3(kv.iter().cloned().fold(f64::NEG_INFINITY, f64::max)), pass_v);
    let _ = writeln!(t, "| Växt | R² mot persistens | {} [{}; {}] | {} | – | – |", f3(m_rpv), f3(lo_rpv), f3(hi_rpv), f3(median(rpv)));
    let _ = writeln!(t, "| Växt | R² mot markslagsmedel | {} [{}; {}] | {} | – | – |", f3(m_rmv), f3(lo_rmv), f3(hi_rmv), f3(median(rmv)));
    let _ = writeln!(t, "| Antal djur | Felkvot mot persistens | {} [{}; {}] | {} | {} / {} | {} av {n} |", f3(m_kn), f3(lo_kn), f3(hi_kn), f3(median(kn.clone())), f3(kn.iter().cloned().fold(f64::INFINITY, f64::min)), f3(kn.iter().cloned().fold(f64::NEG_INFINITY, f64::max)), pass_n);
    let _ = writeln!(t, "| Antal djur | R² mot persistens | {} [{}; {}] | {} | – | – |", f3(m_rpn), f3(lo_rpn), f3(hi_rpn), f3(median(rpn)));
    let _ = writeln!(t, "| Antal djur | R² mot markslagsmedel | {} [{}; {}] | {} | – | – |", f3(m_rmn), f3(lo_rmn), f3(hi_rmn), f3(median(rmn)));

    let _ = writeln!(t, "\n## Per frånvarotid (sammanslaget över alla {n} världar)\n");
    let _ = writeln!(t, "| Frånvarotid (steg) | Återbesök | Växt kvot | Växt R² (pers) | Växt R² (mark) | Antal kvot | Antal R² (pers) | Antal R² (mark) |\n|---|---|---|---|---|---|---|---|");
    for (ki, &(_, _, namn)) in KLASSER.iter().enumerate() {
        let mut a = Ack::default();
        for u in utfall {
            a.merge(&u.klasser[ki]);
        }
        let _ = writeln!(
            t,
            "| {namn} | {} | {} | {} | {} | {} | {} | {} |",
            a.n,
            f3(a.kvot_v()),
            f3(a.r2_pers_v()),
            f3(a.r2_mark_v()),
            f3(a.kvot_n()),
            f3(a.r2_pers_n()),
            f3(a.r2_mark_n())
        );
    }

    let dom_v = if pass_v >= krav { "PASS (slår persistensen med minst 5 %)" } else { "RÖTT (slår inte persistensen med 5 %)" };
    let dom_n = if pass_n >= krav { "PASS (slår persistensen med minst 5 %)" } else { "RÖTT (slår inte persistensen med 5 %)" };
    let _ = writeln!(t, "\n## Utfall mot Alt 1943:s rödprov\n- Växt: **{dom_v}** ({pass_v} av {n}, krav {krav}).\n- Antal djur: **{dom_n}** ({pass_n} av {n}, krav {krav}).");
    t
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("kor") {
        eprintln!("användning: indminne_forprov kor --fron 261-280 --ar 40 --steg 450 --djur 200 --tradar 3 --ut MAPP");
        std::process::exit(2);
    }
    let v = |namn: &str| args.iter().position(|a| a == namn).and_then(|i| args.get(i + 1)).cloned();
    let fron = parse_fron(&v("--fron").unwrap_or_else(|| "261-280".into())).expect("fel --fron");
    let ar: i64 = v("--ar").and_then(|s| s.parse().ok()).unwrap_or(40);
    let steg: i64 = v("--steg").and_then(|s| s.parse().ok()).unwrap_or(450);
    let n_djur: usize = v("--djur").and_then(|s| s.parse().ok()).unwrap_or(200);
    let tradar: usize = v("--tradar").and_then(|s| s.parse().ok()).unwrap_or(3).max(1);
    let ut = PathBuf::from(v("--ut").expect("--ut MAPP"));
    std::fs::create_dir_all(&ut).expect("skapa ut-mapp");
    let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let forsta_fro = fron[0];
    let mut utfall: Vec<VarldUtfall> = Vec::with_capacity(fron.len());
    for chunk in fron.chunks(tradar) {
        let mut res: Vec<Result<VarldUtfall, String>> = std::thread::scope(|s| {
            let mut h = Vec::new();
            for &fro in chunk {
                let r = &rot;
                h.push(s.spawn(move || kor_varld(r, fro, ar, n_djur, steg, fro == forsta_fro)));
            }
            h.into_iter().map(|x| x.join().unwrap_or_else(|_| Err("tråd föll".into()))).collect()
        });
        for x in res.drain(..) {
            match x {
                Ok(u) => {
                    eprintln!("klar frö {}: {} återbesök, växt kvot={:.3}, antal kvot={:.3} ({:.0} s)", u.fro, u.tot.n, u.tot.kvot_v(), u.tot.kvot_n(), u.sek);
                    utfall.push(u);
                }
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
    let text = rapport(&utfall);
    std::fs::write(ut.join("resultat.md"), &text).expect("skriv resultat.md");
    println!("{text}");
}

#[cfg(test)]
mod prov {
    use super::*;

    #[test]
    fn l1_radering_vid_dod_och_rodprov() {
        let mut sk = DjurSkugga::ny(0, 10, 100, (1, 1), (0.5, 0.25), [0; 9]);
        sk.minne.insert(5, MinnesPost { uid: 10, fodelsesteg: 100, t_sedd: 150, v_norm: 0.7, n_norm: 0.5 });
        assert_eq!(sk.minne.len(), 1);
        sk.dod();
        assert_eq!(sk.minne.len(), 0, "efter död ska platsen ha 0 poster");
        assert!(!sk.aktiv);

        // Rödprov: en mutant som inte rensar vid död behåller sin post.
        let mut mutant = DjurSkugga::ny(0, 11, 100, (1, 1), (0.5, 0.25), [0; 9]);
        mutant.minne.insert(5, MinnesPost { uid: 11, fodelsesteg: 100, t_sedd: 150, v_norm: 0.7, n_norm: 0.5 });
        mutant.aktiv = false; // glömde minne.clear()
        assert!(!mutant.minne.is_empty(), "mutanten ska upptäckas ha kvar poster");
    }

    #[test]
    fn l2_ingen_framtidslacka_och_l3_ingen_korslacka() {
        let mut sk1 = DjurSkugga::ny(0, 1, 0, (1, 1), (0.5, 0.0), [0; 9]);
        let mut sk2 = DjurSkugga::ny(1, 2, 5, (1, 1), (0.5, 0.0), [0; 9]);
        sk1.minne.insert(42, MinnesPost { uid: 1, fodelsesteg: 0, t_sedd: 10, v_norm: 0.9, n_norm: 0.0 });
        sk2.minne.insert(42, MinnesPost { uid: 2, fodelsesteg: 5, t_sedd: 10, v_norm: 0.1, n_norm: 0.5 });
        let nu_t = 15;
        let p1 = &sk1.minne[&42];
        let p2 = &sk2.minne[&42];
        assert!(p1.t_sedd < nu_t && p2.t_sedd < nu_t);
        assert_ne!(p1.uid, p2.uid);
        assert!((p1.v_norm - 0.9).abs() < 1e-12);
        assert!((p2.v_norm - 0.1).abs() < 1e-12);
    }

    #[test]
    fn l4_identisk_djur_summa_i_kort_korning() {
        let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let u = kor_varld(&rot, 261, 1, 10, 10, true).unwrap();
        assert!(u.djur_summa_ok, "L4: skuggtabellen får aldrig ändra djur_summa eller rng");
    }

    #[test]
    fn rodprov_slumpvarld_ger_ingen_vinst() {
        let mut g = Rng::new(77);
        let mut ack = Ack::default();
        for _ in 0..5000 {
            let mem = g.f();
            let pers = g.f();
            let sann = g.f();
            ack.lagg((mem - sann).powi(2), (pers - sann).powi(2), (0.5 - sann).powi(2), 0.0, 0.0, 0.0);
        }
        assert!(ack.kvot_v() > 0.95, "i en slumpvärld ska minnet inte slå persistensen med 5 %: {}", ack.kvot_v());
    }
}
