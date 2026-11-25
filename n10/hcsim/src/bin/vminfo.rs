//! vminfo: informationstestet för kodaren och de kostnadsfria analyserna efter Fas 1 (Håkans Alt 1974, 2026-11-23).
//! Mått och gränser (skrivna före körningen): piharness `docs/evolution/vmodell-info-utfallsregel.md`.
//! EXTERN AVLÄSNING: de yttre modellerna läser bara utfilerna; de påverkar inget djur, ingen hjärna och ingen värld.
//! Användning:
//!   vminfo kor --fron 261-280 --ar 40 --djur 40 --steg 300 --avstand 7 --tradar 3 --ut MAPP
//!   vminfo rapport --in MAPP
//!   vminfo fas1extra --in MAPP_MED_FAS1_RADER
//! Rust (AGENTS.md regel 11).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::utforsk_armar;
use hcsim::kartbild::{satt_rattningar_utdod, BALTE_K};
use hcsim::minstakvadrat::spearman;
use hcsim::sim::{KantLage, Sim, GRANNAR};
use hcsim::tal::{A_N, LIV, STEG_PER_AR, S_N, Z_N};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::{Formaga, Rng};

const BLOCK: i64 = 30;
/// Dämpningarna som prövas; den som ger minst fel på de utelämnade träningsdelarna väljs (Tillägg 1 i reglerna). Provmängden rörs inte.
pub const LAMBDOR: [f64; 7] = [0.01, 0.1, 1.0, 10.0, 100.0, 1000.0, 10000.0];
/// Dämpningen när träningsraderna bara har en del (ingen korsprövning möjlig).
const LAMBDA_ENSAM: f64 = 1.0;
/// Golv för spridningen vid standardiseringen (Tillägg 1): ingen kolumn får skalan över 1/0,05 = 20.
pub const SD_GOLV: f64 = 0.05;

/// Skalan för en kolumn med spridningen `sd`: 0 utan spridning, annars 1/max(sd, golvet).
fn skala(sd: f64) -> f64 {
    if sd > 1e-12 { 1.0 / sd.max(SD_GOLV) } else { 0.0 }
}
const N_DRAG: usize = 100;
const DRAG_FRO: u64 = 1974;
const MIN_STEG: usize = 150;

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

/// Provblock: block 2, 5 och 8 (av block om 30 steg) är provmängd.
pub fn ar_prov(t: i64) -> bool {
    (t / BLOCK) % 3 == 2
}

/// Största avståndet i x eller y på torusen.
fn avstand(sim: &Sim, a: usize, b: usize) -> usize {
    let (da, db) = (&sim.djur[a], &sim.djur[b]);
    let dx = (da.i as i64 - db.i as i64).unsigned_abs() as usize;
    let dy = (da.j as i64 - db.j as i64).unsigned_abs() as usize;
    let dx = dx.min(sim.b - dx);
    let dy = dy.min(sim.h - dy);
    dx.max(dy)
}

/// Djur som får följas: levande, minst 1 år, inte döda av ålder inom `steg + 2` steg, parvis minst `min_avst` rutor isär.
fn valj_spridda(sim: &Sim, antal: usize, min_avst: usize, steg: i64, fro: u64) -> Vec<usize> {
    let liv = sim.liv_steg.unwrap_or(LIV);
    let mut kand: Vec<usize> = (0..sim.djur.len())
        .filter(|&i| {
            let d = &sim.djur[i];
            d.lev && d.age >= STEG_PER_AR && d.age + steg + 2 < liv && d.hj.forra.is_some()
        })
        .collect();
    let mut g = Rng::new(fro ^ 0x1974_1974);
    g.blanda(&mut kand);
    // Avstånd 0 = prov med flit fel (regelns rödprov för I1): det första djuret och de närmaste andra.
    if min_avst == 0 && !kand.is_empty() {
        let f = kand[0];
        let mut nara: Vec<usize> = kand.clone();
        nara.sort_by_key(|&k| (avstand(sim, f, k), k));
        nara.truncate(antal);
        nara.sort_unstable();
        return nara;
    }
    let mut valda: Vec<usize> = Vec::new();
    for k in kand {
        if valda.len() >= antal {
            break;
        }
        if valda.iter().all(|&v| avstand(sim, v, k) >= min_avst) {
            valda.push(k);
        }
    }
    valda.sort_unstable();
    valda
}

/// Delar djuren (platser i `idx`) i grupper där alla par står minst `min_avst` rutor isär i steget (girigt, i ordning).
/// Djuren rör sig, så avståndet prövas i varje steg (Tillägg 2). `min_avst = 0` (rödprovet) ger en enda grupp.
fn grupper(sim: &Sim, idx: &[usize], min_avst: usize) -> Vec<Vec<usize>> {
    if min_avst == 0 {
        return vec![(0..idx.len()).collect()];
    }
    let mut g: Vec<Vec<usize>> = Vec::new();
    for n in 0..idx.len() {
        match g.iter_mut().find(|gr| gr.iter().all(|&m| avstand(sim, idx[m], idx[n]) >= min_avst)) {
            Some(gr) => gr.push(n),
            None => g.push(vec![n]),
        }
    }
    g
}

/// Utfallet för ett djur i en kopia.
#[derive(Clone)]
struct Ut {
    z_kopia: Vec<f64>,
    y: Vec<f64>,
    s2: Vec<f64>,
    zn: Vec<f64>,
    v_pred: f64,
    v_sann: f64,
    intag: f64,
    flyttade: bool,
}

/// Kör kopian där djuren `idx` tar handlingen `a` (en grupp, eller ett djur ensamt). None för ett djur som dog eller byttes.
fn kopia(sim: &Sim, idx: &[usize], uid: &[u64], a: usize, ensam: bool) -> Vec<Option<Ut>> {
    let mut k = sim.clone();
    if ensam {
        assert_eq!(idx.len(), 1);
        k.tvinga = Some((idx[0], a));
    } else {
        k.tvinga_grupp = Some((idx.to_vec(), a));
    }
    let pos: Vec<(usize, usize)> = idx.iter().map(|&x| (sim.djur[x].i, sim.djur[x].j)).collect();
    k.steg();
    k.tvinga = None;
    k.tvinga_grupp = None;
    // Växterna växer en gång (som i början av nästa steg), sedan läses varje djurs sinnen.
    let mut vaxt = false;
    let mut ut = Vec::with_capacity(idx.len());
    for (n, &x) in idx.iter().enumerate() {
        let d = &k.djur[x];
        if !d.lev || d.uid != uid[n] {
            ut.push(None);
            continue;
        }
        let (z_kopia, a2, y) = d.hj.forra.clone().expect("hjärnan har förutsagt i steget");
        assert_eq!(a2, a, "kroken gav en annan handling");
        let intag = d.forra_intag.unwrap_or(0.0);
        let flyttade = (d.i, d.j) != pos[n];
        let s2 = if vaxt { k.sinnen_for(x) } else { vaxt = true; k.sinnen_nasta_steg(x) };
        let hj = &k.djur[x].hj;
        let zn = hj.kod(&s2);
        ut.push(Some(Ut { v_pred: hj.varde(&y), v_sann: hj.varde(&zn), z_kopia, y, s2, zn, intag, flyttade }));
    }
    ut
}

fn bitlika(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.to_bits() == q.to_bits())
}

fn tal(v: &[f64], t: &mut String) {
    for x in v {
        let _ = write!(t, "\t{x:.10e}");
    }
}

fn rubrik() -> String {
    let mut r = String::from("fro\tuid\tt\ta\tprov");
    for (n, k) in [("s", S_N), ("z", Z_N), ("s2", S_N), ("zn", Z_N), ("y", Z_N)] {
        for i in 0..k {
            let _ = write!(r, "\t{n}_{i}");
        }
    }
    r.push_str("\tv_pred\tv_sann\tintag\tflyttade");
    r
}

fn logg(fil: &Path, text: &str) {
    let nu = std::process::Command::new("date").arg("+%F %T").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(fil) {
        let _ = writeln!(f, "{nu} {text}");
    }
    eprintln!("{nu} {text}");
}

fn kor(rot: &Path, fro: u64, ar: i64, n_djur: usize, steg: i64, min_avst: usize, ut: &Path) -> Result<String, String> {
    let lf = ut.join("korning.logg");
    let t0 = std::time::Instant::now();
    let mut sim = bygg_varld(rot, fro)?;
    while sim.t < ar * STEG_PER_AR {
        sim.steg();
        if sim.t % (5 * STEG_PER_AR) == 0 {
            logg(&lf, &format!("frö {fro}: år {} av {ar}, {} djur, {:.0} s", sim.t / STEG_PER_AR, sim.antal(), t0.elapsed().as_secs_f64()));
        }
    }
    assert!(!sim.markering_skedd, "ingen märkning");
    assert!(sim.djur.iter().filter(|d| d.lev).all(|d| d.formaga == Formaga::Lar), "alla djur ska ha förmågan lar");
    let valda = valj_spridda(&sim, n_djur, min_avst, steg, fro);
    let uid: Vec<u64> = valda.iter().map(|&x| sim.djur[x].uid).collect();
    logg(&lf, &format!("frö {fro}: år {ar}, {} djur i världen, {} valda (avstånd minst {min_avst}), {:.0} s", sim.antal(), valda.len(), t0.elapsed().as_secs_f64()));
    let referens = sim.clone();
    let mut aktiv = vec![true; valda.len()];
    let (mut i0_fel, mut i1_ok, mut i1_fel, mut prov, mut bortfall) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut steg_flera, mut forsta_flera, mut max_grupper) = (0u64, None::<i64>, 1usize);
    let mut f = std::io::BufWriter::new(std::fs::File::create(ut.join(format!("rader-{fro}.tsv"))).map_err(|e| e.to_string())?);
    writeln!(f, "{}", rubrik()).map_err(|e| e.to_string())?;
    for t in 0..steg {
        for (n, &x) in valda.iter().enumerate() {
            if aktiv[n] && (!sim.djur[x].lev || sim.djur[x].uid != uid[n]) {
                aktiv[n] = false;
                bortfall += 1;
            }
        }
        let akt: Vec<usize> = (0..valda.len()).filter(|&n| aktiv[n]).collect();
        if akt.is_empty() {
            break;
        }
        let idx: Vec<usize> = akt.iter().map(|&n| valda[n]).collect();
        let uidv: Vec<u64> = akt.iter().map(|&n| uid[n]).collect();
        // Sinnena och den inre bilden före steget (ren räkning på en kopia; växterna växer först, som i steget).
        let mut c = sim.clone();
        let s0: Vec<Vec<f64>> = idx.iter().enumerate().map(|(n, &x)| if n == 0 { c.sinnen_nasta_steg(x) } else { c.sinnen_for(x) }).collect();
        let z0: Vec<Vec<f64>> = idx.iter().zip(&s0).map(|(&x, s)| sim.djur[x].hj.kod(s)).collect();
        let gr = grupper(&sim, &idx, min_avst);
        if gr.len() > 1 {
            steg_flera += 1;
            if forsta_flera.is_none() {
                forsta_flera = Some(t);
            }
        }
        max_grupper = max_grupper.max(gr.len());
        let mut utf: Vec<Vec<Option<Ut>>> = vec![vec![None; idx.len()]; A_N];
        for g in &gr {
            let gi: Vec<usize> = g.iter().map(|&n| idx[n]).collect();
            let gu: Vec<u64> = g.iter().map(|&n| uidv[n]).collect();
            for (a, rad) in utf.iter_mut().enumerate() {
                for (k, u) in kopia(&sim, &gi, &gu, a, false).into_iter().enumerate() {
                    rad[g[k]] = u;
                }
            }
        }
        // I1: ett djur prövas också ensamt i var tionde steg; utfallet ska vara bit för bit lika.
        if t % 10 == 0 {
            let m = ((t / 10) as usize) % idx.len();
            for a in 0..A_N {
                let ensam = kopia(&sim, &idx[m..=m], &uidv[m..=m], a, true);
                match (&ensam[0], &utf[a][m]) {
                    (Some(e), Some(g)) if bitlika(&e.s2, &g.s2) && bitlika(&e.zn, &g.zn) && bitlika(&e.y, &g.y) => i1_ok += 1,
                    (None, None) => {}
                    _ => i1_fel += 1,
                }
            }
        }
        let mut rad = String::new();
        for n in 0..idx.len() {
            if (0..A_N).any(|a| utf[a][n].is_none()) {
                aktiv[akt[n]] = false;
                bortfall += 1;
                continue;
            }
            prov += 1;
            for a in 0..A_N {
                let u = utf[a][n].as_ref().unwrap();
                if !bitlika(&u.z_kopia, &z0[n]) {
                    i0_fel += 1;
                }
                rad.clear();
                let _ = write!(rad, "{fro}\t{}\t{t}\t{a}\t{}", uidv[n], ar_prov(t) as u8);
                tal(&s0[n], &mut rad);
                tal(&z0[n], &mut rad);
                tal(&u.s2, &mut rad);
                tal(&u.zn, &mut rad);
                tal(&u.y, &mut rad);
                let _ = write!(rad, "\t{:.10e}\t{:.10e}\t{:.10e}\t{}", u.v_pred, u.v_sann, u.intag, u.flyttade as u8);
                writeln!(f, "{rad}").map_err(|e| e.to_string())?;
            }
        }
        sim.steg();
        if (t + 1) % 10 == 0 {
            logg(&lf, &format!("frö {fro}: steg {} av {steg}, {} djur kvar, {prov} prov, I1 {i1_ok} lika / {i1_fel} olika, steg med flera grupper {steg_flera}, {:.0} s", t + 1, akt.len(), t0.elapsed().as_secs_f64()));
        }
    }
    f.flush().map_err(|e| e.to_string())?;
    // I2: originalvärlden ska vara lika med en körning utan prov.
    let mut r = referens;
    while r.t < sim.t {
        r.steg();
    }
    let i2 = r.djur_summa() == sim.djur_summa() && r.g.tillstand() == sim.g.tillstand();
    let text = format!(
        "frö {fro}: valda {}, prov {prov}, bortfall {bortfall}, I0 olika inre bild {i0_fel}, I1 {} ({i1_ok} lika, {i1_fel} olika), I2 {}, steg med flera grupper {steg_flera} (första steg {}, högst {max_grupper} grupper), {:.0} s",
        valda.len(),
        if i1_fel == 0 && i1_ok > 0 { "PASS" } else { "FAIL" },
        if i2 { "PASS" } else { "FAIL" },
        forsta_flera.map(|t| t.to_string()).unwrap_or_else(|| "inget".into()),
        t0.elapsed().as_secs_f64()
    );
    std::fs::write(ut.join(format!("kontroll-{fro}.txt")), format!("{text}\n")).map_err(|e| e.to_string())?;
    logg(&lf, &format!("KLAR {text}"));
    Ok(text)
}

// ---------------------------------------------------------------- de yttre modellerna

/// Ridge med standardiserade indata och centrerat mål.
#[derive(Clone)]
pub struct Ridge {
    mx: Vec<f64>,
    sx: Vec<f64>,
    my: Vec<f64>,
    w: Vec<Vec<f64>>, // p × q
}

fn cholesky_los(a: &mut [Vec<f64>], b: &mut [Vec<f64>]) -> Result<(), String> {
    let p = a.len();
    for j in 0..p {
        let mut d = a[j][j];
        for k in 0..j {
            d -= a[j][k] * a[j][k];
        }
        if d <= 0.0 {
            return Err("matrisen är inte positivt definit".into());
        }
        let d = d.sqrt();
        a[j][j] = d;
        for i in j + 1..p {
            let mut s = a[i][j];
            for k in 0..j {
                s -= a[i][k] * a[j][k];
            }
            a[i][j] = s / d;
        }
    }
    let q = b[0].len();
    for c in 0..q {
        for i in 0..p {
            let mut s = b[i][c];
            for k in 0..i {
                s -= a[i][k] * b[k][c];
            }
            b[i][c] = s / a[i][i];
        }
        for i in (0..p).rev() {
            let mut s = b[i][c];
            for k in i + 1..p {
                s -= a[k][i] * b[k][c];
            }
            b[i][c] = s / a[i][i];
        }
    }
    Ok(())
}

impl Ridge {
    /// Anpassar med dämpningen vald genom korsprövning över delarna `del` (en per rad; bara träningsrader). Ger modellen och vald dämpning.
    pub fn anpassa(x: &[Vec<f64>], y: &[Vec<f64>], del: &[usize]) -> Result<(Ridge, f64), String> {
        let n = x.len();
        if n < 2 {
            return Err("för få rader".into());
        }
        assert_eq!(del.len(), n);
        let (p, q) = (x[0].len(), y[0].len());
        let mut mx = vec![0.0; p];
        let mut my = vec![0.0; q];
        for (xr, yr) in x.iter().zip(y) {
            for j in 0..p {
                mx[j] += xr[j];
            }
            for c in 0..q {
                my[c] += yr[c];
            }
        }
        mx.iter_mut().for_each(|v| *v /= n as f64);
        my.iter_mut().for_each(|v| *v /= n as f64);
        let mut sx = vec![0.0; p];
        for xr in x {
            for j in 0..p {
                sx[j] += (xr[j] - mx[j]).powi(2);
            }
        }
        let sx: Vec<f64> = sx.iter().map(|v| skala((v / n as f64).sqrt())).collect();
        let k = del.iter().max().unwrap() + 1;
        let mut ak = vec![vec![vec![0.0; p]; p]; k];
        let mut bk = vec![vec![vec![0.0; q]; p]; k];
        let xs: Vec<Vec<f64>> = x.iter().map(|xr| (0..p).map(|j| (xr[j] - mx[j]) * sx[j]).collect()).collect();
        let yc: Vec<Vec<f64>> = y.iter().map(|yr| (0..q).map(|c| yr[c] - my[c]).collect()).collect();
        for r in 0..n {
            let (a, b) = (&mut ak[del[r]], &mut bk[del[r]]);
            for j in 0..p {
                for l in 0..=j {
                    a[j][l] += xs[r][j] * xs[r][l];
                }
                for c in 0..q {
                    b[j][c] += xs[r][j] * yc[r][c];
                }
            }
        }
        let mut at = vec![vec![0.0; p]; p];
        let mut bt = vec![vec![0.0; q]; p];
        for d in 0..k {
            for j in 0..p {
                for l in 0..=j {
                    at[j][l] += ak[d][j][l];
                }
                for c in 0..q {
                    bt[j][c] += bk[d][j][c];
                }
            }
        }
        let los = |a0: &[Vec<f64>], b0: &[Vec<f64>], lam: f64| -> Result<Vec<Vec<f64>>, String> {
            let mut a: Vec<Vec<f64>> = a0.to_vec();
            for j in 0..p {
                for l in 0..j {
                    a[l][j] = a[j][l];
                }
                a[j][j] += lam;
            }
            let mut b = b0.to_vec();
            cholesky_los(&mut a, &mut b)?;
            Ok(b)
        };
        let delar_med_rader: Vec<usize> = (0..k).filter(|&d| del.contains(&d)).collect();
        let lam = if delar_med_rader.len() < 2 {
            LAMBDA_ENSAM
        } else {
            let mut bast = (f64::INFINITY, LAMBDA_ENSAM);
            for &lam in &LAMBDOR {
                let mut fel = 0.0;
                for &d in &delar_med_rader {
                    let a: Vec<Vec<f64>> = (0..p).map(|j| (0..p).map(|l| if l <= j { at[j][l] - ak[d][j][l] } else { 0.0 }).collect()).collect();
                    let b: Vec<Vec<f64>> = (0..p).map(|j| (0..q).map(|c| bt[j][c] - bk[d][j][c]).collect()).collect();
                    let w = los(&a, &b, lam)?;
                    for r in (0..n).filter(|&r| del[r] == d) {
                        for c in 0..q {
                            let g: f64 = (0..p).map(|j| xs[r][j] * w[j][c]).sum();
                            fel += (g - yc[r][c]).powi(2);
                        }
                    }
                }
                if fel < bast.0 {
                    bast = (fel, lam);
                }
            }
            bast.1
        };
        let w = los(&at, &bt, lam)?;
        Ok((Ridge { mx, sx, my, w }, lam))
    }

    pub fn gissa(&self, x: &[f64]) -> Vec<f64> {
        let mut ut = self.my.clone();
        for j in 0..self.mx.len() {
            let v = (x[j] - self.mx[j]) * self.sx[j];
            if v != 0.0 {
                for (c, u) in ut.iter_mut().enumerate() {
                    *u += v * self.w[j][c];
                }
            }
        }
        ut
    }
}

/// Fasta slumpade tanh-drag (frö 1974) av standardiserade indata.
pub struct Drag {
    mx: Vec<f64>,
    sx: Vec<f64>,
    r: Vec<Vec<f64>>,
    b: Vec<f64>,
}

impl Drag {
    pub fn ny(x: &[Vec<f64>]) -> Drag {
        let (n, p) = (x.len(), x[0].len());
        let mut mx = vec![0.0; p];
        for xr in x {
            for j in 0..p {
                mx[j] += xr[j] / n as f64;
            }
        }
        let mut sx = vec![0.0; p];
        for xr in x {
            for j in 0..p {
                sx[j] += (xr[j] - mx[j]).powi(2) / n as f64;
            }
        }
        let sx = sx.iter().map(|v: &f64| skala(v.sqrt())).collect();
        let mut g = Rng::new(DRAG_FRO ^ p as u64);
        let mut gauss = || {
            let (u1, u2) = (g.f().max(1e-300), g.f());
            (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
        };
        let skala = 1.0 / (p as f64).sqrt();
        let r = (0..N_DRAG).map(|_| (0..p).map(|_| gauss() * skala).collect()).collect();
        let b = (0..N_DRAG).map(|_| gauss() * 0.5).collect();
        Drag { mx, sx, r, b }
    }

    pub fn av(&self, x: &[f64]) -> Vec<f64> {
        self.r
            .iter()
            .zip(&self.b)
            .map(|(rr, b)| (b + rr.iter().enumerate().map(|(j, w)| w * (x[j] - self.mx[j]) * self.sx[j]).sum::<f64>()).tanh())
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    Ridge,
    Drag,
}

/// En anpassad modell: en per handling, eller en för alla (utan handling).
pub struct Modell {
    drag: Option<Drag>,
    delar: Vec<Ridge>,
    /// De valda dämpningarna (en per del).
    pub lambdor: Vec<f64>,
}

impl Modell {
    /// `x[a]` och `y[a]`: träningsraderna för handling a. `med_handling = false`: alla handlingars rader i en modell.
    /// `del[a]`: korsprövningens del för varje träningsrad (tidsblock eller individ).
    pub fn anpassa(form: Form, x: &[Vec<Vec<f64>>], y: &[Vec<Vec<f64>>], del: &[Vec<usize>], med_handling: bool) -> Result<Modell, String> {
        let alla_x: Vec<Vec<f64>> = x.iter().flatten().cloned().collect();
        let drag = if form == Form::Drag { Some(Drag::ny(&alla_x)) } else { None };
        let f = |v: &Vec<Vec<f64>>| -> Vec<Vec<f64>> { match &drag { Some(d) => v.iter().map(|r| d.av(r)).collect(), None => v.clone() } };
        let par: Vec<(Ridge, f64)> = if med_handling {
            (0..A_N).map(|a| Ridge::anpassa(&f(&x[a]), &y[a], &del[a])).collect::<Result<Vec<_>, _>>()?
        } else {
            let ay: Vec<Vec<f64>> = y.iter().flatten().cloned().collect();
            let ad: Vec<usize> = del.iter().flatten().copied().collect();
            vec![Ridge::anpassa(&f(&alla_x), &ay, &ad)?]
        };
        let lambdor = par.iter().map(|p| p.1).collect();
        Ok(Modell { drag, delar: par.into_iter().map(|p| p.0).collect(), lambdor })
    }

    pub fn gissa(&self, x: &[f64], a: usize) -> Vec<f64> {
        let xx = match &self.drag { Some(d) => d.av(x), None => x.to_vec() };
        let del = if self.delar.len() == 1 { &self.delar[0] } else { &self.delar[a] };
        del.gissa(&xx)
    }
}

// ---------------------------------------------------------------- läsningen av raderna

#[derive(Clone)]
struct Prov {
    uid: u64,
    t: i64,
    prov: bool,
    s: Vec<f64>,
    z: Vec<f64>,
    s2: Vec<Vec<f64>>,
    zn: Vec<Vec<f64>>,
    y: Vec<Vec<f64>>,
    v_pred: Vec<f64>,
    v_sann: Vec<f64>,
    intag: Vec<f64>,
}

fn las_rader(fil: &Path) -> Result<(u64, Vec<Prov>), String> {
    let text = std::fs::read_to_string(fil).map_err(|e| format!("{}: {e}", fil.display()))?;
    let mut rader = text.lines();
    let r = rader.next().ok_or("tom fil")?;
    if r != rubrik() {
        return Err(format!("{}: fel rubrik", fil.display()));
    }
    let mut fro = 0;
    let mut karta: BTreeMap<(u64, i64), Prov> = BTreeMap::new();
    for (nr, l) in rader.enumerate() {
        let k: Vec<&str> = l.split('\t').collect();
        let bredd = 5 + 2 * S_N + 3 * Z_N + 4;
        if k.len() != bredd {
            return Err(format!("{} rad {}: {} kolumner, väntat {bredd}", fil.display(), nr + 2, k.len()));
        }
        let p = |i: usize| -> Result<f64, String> { k[i].parse::<f64>().map_err(|e| format!("rad {}: {e}", nr + 2)) };
        fro = k[0].parse().map_err(|e| format!("{e}"))?;
        let uid: u64 = k[1].parse().map_err(|e| format!("{e}"))?;
        let t: i64 = k[2].parse().map_err(|e| format!("{e}"))?;
        let a: usize = k[3].parse().map_err(|e| format!("{e}"))?;
        let vek = |fran: usize, n: usize| -> Result<Vec<f64>, String> { (fran..fran + n).map(p).collect() };
        let s = vek(5, S_N)?;
        let z = vek(5 + S_N, Z_N)?;
        let s2 = vek(5 + S_N + Z_N, S_N)?;
        let zn = vek(5 + 2 * S_N + Z_N, Z_N)?;
        let y = vek(5 + 2 * S_N + 2 * Z_N, Z_N)?;
        let (vp, vs, intag_v) = (p(5 + 2 * S_N + 3 * Z_N)?, p(6 + 2 * S_N + 3 * Z_N)?, p(7 + 2 * S_N + 3 * Z_N)?);
        let e = karta.entry((uid, t)).or_insert_with(|| Prov { uid, t, prov: k[4] == "1", s: s.clone(), z: z.clone(), s2: vec![], zn: vec![], y: vec![], v_pred: vec![], v_sann: vec![], intag: vec![] });
        if e.s2.len() != a {
            return Err(format!("rad {}: handling {a} i fel ordning", nr + 2));
        }
        e.s2.push(s2);
        e.zn.push(zn);
        e.y.push(y);
        e.v_pred.push(vp);
        e.v_sann.push(vs);
        e.intag.push(intag_v);
    }
    let prov: Vec<Prov> = karta.into_values().collect();
    if let Some(p) = prov.iter().find(|p| p.s2.len() != A_N) {
        return Err(format!("djur {} har ett prov med {} handlingar", p.uid, p.s2.len()));
    }
    Ok((fro, prov))
}

// ---------------------------------------------------------------- måtten

/// Summor för handlingens verkan och felet.
#[derive(Clone, Default, Debug)]
pub struct Summa {
    pub rest: f64,
    pub verklig: f64,
    pub fel: f64,
    pub fel_pers: f64,
    pub rest_d: Vec<f64>,
    pub verklig_d: Vec<f64>,
    pub prov: u64,
}

impl Summa {
    pub fn r2(&self) -> f64 {
        1.0 - self.rest / self.verklig.max(1e-300)
    }
    pub fn kvot(&self) -> f64 {
        self.fel / self.fel_pers.max(1e-300)
    }
    pub fn r2_d(&self, d: usize) -> f64 {
        1.0 - self.rest_d[d] / self.verklig_d[d].max(1e-300)
    }
    fn lagg(&mut self, o: &Summa) {
        self.rest += o.rest;
        self.verklig += o.verklig;
        self.fel += o.fel;
        self.fel_pers += o.fel_pers;
        self.prov += o.prov;
        if self.rest_d.is_empty() {
            self.rest_d = vec![0.0; o.rest_d.len()];
            self.verklig_d = vec![0.0; o.verklig_d.len()];
        }
        for d in 0..o.rest_d.len() {
            self.rest_d[d] += o.rest_d[d];
            self.verklig_d[d] += o.verklig_d[d];
        }
    }
}

/// Ett provs bidrag: `gissn[a]` och `utf[a]` för de nio handlingarna, `fore` = läget före (persistensen).
pub fn bidrag(gissn: &[Vec<f64>], utf: &[Vec<f64>], fore: &[f64]) -> Summa {
    let q = fore.len();
    let mut mg = vec![0.0; q];
    let mut mu = vec![0.0; q];
    for a in 0..A_N {
        for d in 0..q {
            mg[d] += gissn[a][d] / A_N as f64;
            mu[d] += utf[a][d] / A_N as f64;
        }
    }
    let mut s = Summa { rest_d: vec![0.0; q], verklig_d: vec![0.0; q], prov: 1, ..Default::default() };
    for a in 0..A_N {
        for d in 0..q {
            let fd = gissn[a][d] - mg[d];
            let vd = utf[a][d] - mu[d];
            s.rest_d[d] += (fd - vd).powi(2);
            s.verklig_d[d] += vd * vd;
            s.fel += (gissn[a][d] - utf[a][d]).powi(2);
            s.fel_pers += (fore[d] - utf[a][d]).powi(2);
        }
    }
    s.rest = s.rest_d.iter().sum();
    s.verklig = s.verklig_d.iter().sum();
    s
}

const M_NAMN: [&str; 6] = ["M1 sinnen + handling → inre bild", "M2 inre bild + handling → inre bild", "M3 inre bild utan handling → inre bild", "M4 sinnen utan handling → inre bild", "Hjärnan själv", "Persistensen"];

/// Resultat för en värld.
struct VarldSvar {
    fro: u64,
    individer: usize,
    /// [modell][form] för M1–M4; plats 4 hjärnan och 5 persistensen (form 0).
    m: Vec<Vec<Summa>>,
    /// Median över individer av R² (handlingens verkan) per modell och form.
    m_median: Vec<Vec<f64>>,
    /// T1, T2 per form.
    t: Vec<Vec<Summa>>,
    t_pers: Summa,
    /// Median av de valda dämpningarna per modell och form.
    lam_med: Vec<Vec<f64>>,
    t_lam_med: Vec<Vec<f64>>,
}

type Trn = (Vec<Vec<Vec<f64>>>, Vec<Vec<Vec<f64>>>, Vec<Vec<usize>>);

fn traning(p: &[&Prov], indata: impl Fn(&Prov) -> Vec<f64>, mal: impl Fn(&Prov, usize) -> Vec<f64>, del: impl Fn(&Prov) -> usize) -> Trn {
    let mut x = vec![Vec::new(); A_N];
    let mut y = vec![Vec::new(); A_N];
    let mut d = vec![Vec::new(); A_N];
    for pr in p {
        for a in 0..A_N {
            x[a].push(indata(pr));
            y[a].push(mal(pr, a));
            d[a].push(del(pr));
        }
    }
    (x, y, d)
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn rakna_varld(fro: u64, prov: &[Prov]) -> Result<VarldSvar, String> {
    let mut per: BTreeMap<u64, Vec<&Prov>> = BTreeMap::new();
    for p in prov {
        per.entry(p.uid).or_default().push(p);
    }
    let former = [Form::Ridge, Form::Drag];
    let mut m = vec![vec![Summa::default(); 2]; 6];
    let mut med: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 2]; 6];
    let mut lam: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 2]; 6];
    let mut individer = 0;
    let s_in = |p: &Prov| p.s.clone();
    let z_in = |p: &Prov| p.z.clone();
    let zn_ut = |p: &Prov, a: usize| p.zn[a].clone();
    for (_, pv) in &per {
        if pv.len() < MIN_STEG {
            continue;
        }
        let tr: Vec<&Prov> = pv.iter().filter(|p| !p.prov).copied().collect();
        let te: Vec<&Prov> = pv.iter().filter(|p| p.prov).copied().collect();
        if tr.len() < 20 || te.is_empty() {
            continue;
        }
        individer += 1;
        for (fi, &form) in former.iter().enumerate() {
            // Korsprövningens delar: träningsblocken (block om 30 steg).
            let blk = |p: &Prov| (p.t / BLOCK) as usize;
            let (x1, y1, d1) = traning(&tr, s_in, zn_ut, blk);
            let (x2, y2, d2) = traning(&tr, z_in, zn_ut, blk);
            let mods = [
                Modell::anpassa(form, &x1, &y1, &d1, true)?,
                Modell::anpassa(form, &x2, &y2, &d2, true)?,
                Modell::anpassa(form, &x2, &y2, &d2, false)?,
                Modell::anpassa(form, &x1, &y1, &d1, false)?,
            ];
            for (mi, md) in mods.iter().enumerate() {
                lam[mi][fi].extend(md.lambdor.iter().copied());
            }
            for (mi, md) in mods.iter().enumerate() {
                let mut ind = Summa::default();
                for p in &te {
                    let inn = if mi == 0 || mi == 3 { &p.s } else { &p.z };
                    let g: Vec<Vec<f64>> = (0..A_N).map(|a| md.gissa(inn, a)).collect();
                    ind.lagg(&bidrag(&g, &p.zn, &p.z));
                }
                med[mi][fi].push(ind.r2());
                m[mi][fi].lagg(&ind);
            }
        }
        let mut hj = Summa::default();
        let mut pe = Summa::default();
        for p in &te {
            hj.lagg(&bidrag(&p.y, &p.zn, &p.z));
            let pz: Vec<Vec<f64>> = (0..A_N).map(|_| p.z.clone()).collect();
            pe.lagg(&bidrag(&pz, &p.zn, &p.z));
        }
        med[4][0].push(hj.r2());
        med[5][0].push(pe.r2());
        m[4][0].lagg(&hj);
        m[5][0].lagg(&pe);
    }
    // T1 och T2: gemensam modell för världen; hälften av individerna (jämnt uid-tal efter blandning) tränar, den andra prövar.
    let mut uids: Vec<u64> = per.keys().copied().collect();
    let mut g = Rng::new(fro ^ 0x7171);
    g.blanda(&mut uids);
    let (tr_u, te_u) = uids.split_at(uids.len() / 2);
    let tr: Vec<&Prov> = tr_u.iter().flat_map(|u| per[u].iter().copied()).collect();
    let te: Vec<&Prov> = te_u.iter().flat_map(|u| per[u].iter().copied()).collect();
    let mut t = vec![vec![Summa::default(); 2]; 2];
    let mut t_pers = Summa::default();
    let mut t_lam: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); 2]; 2];
    if !tr.is_empty() && !te.is_empty() {
        let s2_ut = |p: &Prov, a: usize| p.s2[a].clone();
        // Korsprövningens delar: fem grupper av träningsindividerna.
        let grupp: std::collections::HashMap<u64, usize> = tr_u.iter().enumerate().map(|(i, u)| (*u, i % 5)).collect();
        let (x, y, d) = traning(&tr, s_in, s2_ut, |p: &Prov| grupp[&p.uid]);
        for (fi, &form) in former.iter().enumerate() {
            let mods = [Modell::anpassa(form, &x, &y, &d, true)?, Modell::anpassa(form, &x, &y, &d, false)?];
            for (ti, md) in mods.iter().enumerate() {
                t_lam[ti][fi].extend(md.lambdor.iter().copied());
            }
            for (ti, md) in mods.iter().enumerate() {
                for p in &te {
                    let gs: Vec<Vec<f64>> = (0..A_N).map(|a| md.gissa(&p.s, a)).collect();
                    t[ti][fi].lagg(&bidrag(&gs, &p.s2, &p.s));
                }
            }
        }
        for p in &te {
            let ps: Vec<Vec<f64>> = (0..A_N).map(|_| p.s.clone()).collect();
            t_pers.lagg(&bidrag(&ps, &p.s2, &p.s));
        }
    }
    let m_median = med.into_iter().map(|v| v.into_iter().map(median).collect()).collect();
    let lam_med = lam.into_iter().map(|v| v.into_iter().map(median).collect()).collect();
    let t_lam_med = t_lam.into_iter().map(|v| v.into_iter().map(median).collect()).collect();
    Ok(VarldSvar { fro, individer, m, m_median, t, t_pers, lam_med, t_lam_med })
}

fn f3(x: f64) -> String {
    format!("{x:.3}").replace('.', ",")
}

fn rapport(mapp: &Path) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() {
        return Err("inga radfiler".into());
    }
    let mut svar = Vec::new();
    for fil in &filer {
        let (fro, prov) = las_rader(fil)?;
        if prov.is_empty() {
            return Err(format!("{}: inga prov", fil.display()));
        }
        eprintln!("räknar frö {fro}: {} prov", prov.len());
        svar.push(rakna_varld(fro, &prov)?);
    }
    let n = svar.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Informationstestet för kodaren: EXTERN AVLÄSNING (tal räknade av `vminfo rapport`)\n");
    let _ = writeln!(t, "Regler skrivna före körningen: piharness `docs/evolution/vmodell-info-utfallsregel.md`. {n} världar; kravet \"minst 15 av 20\" räknas som minst {krav} av {n}.");
    let _ = writeln!(t, "Modellerna är yttre: de läser bara raderna och påverkar inget djur. R² = handlingens verkan (medlet över de nio handlingarna borttaget per prov); 0 = bryr sig inte om handlingen. Felkvot = felet delat med persistensens.\n");
    // Kontrollerna
    let _ = writeln!(t, "## Kontrollerna\n");
    for fil in &filer {
        let k = fil.with_file_name(fil.file_name().unwrap().to_str().unwrap().replace("rader-", "kontroll-").replace(".tsv", ".txt"));
        let _ = writeln!(t, "- {}", std::fs::read_to_string(&k).map(|s| s.trim().to_string()).unwrap_or_else(|_| format!("{} saknas", k.display())));
    }
    let _ = writeln!(t, "\n## Per värld (R² för handlingens verkan; ridge / slumpdrag)\n");
    let _ = writeln!(t, "| Frö | Individer | M1 | M2 | Hjärnan | T1 sinnen→sinnen | M1 − M2 (bästa formen) |\n|---|---|---|---|---|---|---|");
    let bast = |s: &VarldSvar, mi: usize| s.m[mi][0].r2().max(s.m[mi][1].r2());
    let bast_t = |s: &VarldSvar| s.t[0][0].r2().max(s.t[0][1].r2());
    for s in &svar {
        let _ = writeln!(
            t,
            "| {} | {} | {} / {} | {} / {} | {} | {} / {} | {} |",
            s.fro,
            s.individer,
            f3(s.m[0][0].r2()),
            f3(s.m[0][1].r2()),
            f3(s.m[1][0].r2()),
            f3(s.m[1][1].r2()),
            f3(s.m[4][0].r2()),
            f3(s.t[0][0].r2()),
            f3(s.t[0][1].r2()),
            f3(bast(s, 0) - bast(s, 1))
        );
    }
    let _ = writeln!(t, "\n## Sammanfattat över världarna\n");
    let _ = writeln!(t, "| Modell | Form | R² median | lägst | högst | Världar med R² över 0 | Median över individer (median av världar) | Felkvot median | Vald dämpning (median) |\n|---|---|---|---|---|---|---|---|---|");
    for mi in 0..6 {
        let nf = if mi < 4 { 2 } else { 1 };
        for fi in 0..nf {
            let r: Vec<f64> = svar.iter().map(|s| s.m[mi][fi].r2()).collect();
            let fk: Vec<f64> = svar.iter().map(|s| s.m[mi][fi].kvot()).collect();
            let im: Vec<f64> = svar.iter().map(|s| s.m_median[mi][fi]).collect();
            let lo = r.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = r.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let form = if mi >= 4 { "–" } else if fi == 0 { "ridge" } else { "slumpdrag" };
            let lm = if mi < 4 { f3(median(svar.iter().map(|s| s.lam_med[mi][fi]).collect())) } else { "–".into() };
            let _ = writeln!(t, "| {} | {form} | {} | {} | {} | {} av {n} | {} | {} | {lm} |", M_NAMN[mi], f3(median(r.clone())), f3(lo), f3(hi), r.iter().filter(|&&x| x > 0.0).count(), f3(median(im)), f3(median(fk)));
        }
    }
    for (ti, namn) in ["T1 sinnen + handling → sinnen", "T2 sinnen utan handling → sinnen"].iter().enumerate() {
        for fi in 0..2 {
            let r: Vec<f64> = svar.iter().map(|s| s.t[ti][fi].r2()).collect();
            let fk: Vec<f64> = svar.iter().map(|s| s.t[ti][fi].kvot()).collect();
            let lm = f3(median(svar.iter().map(|s| s.t_lam_med[ti][fi]).collect()));
            let _ = writeln!(t, "| {namn} | {} | {} | – | – | {} av {n} | – | {} | {lm} |", if fi == 0 { "ridge" } else { "slumpdrag" }, f3(median(r.clone())), r.iter().filter(|&&x| x > 0.0).count(), f3(median(fk)));
        }
    }
    let fkp: Vec<f64> = svar.iter().map(|s| s.t_pers.kvot()).collect();
    let _ = writeln!(t, "| Persistensen för sinnena | – | 0 | – | – | – | – | {} | – |", f3(median(fkp)));
    let _ = writeln!(t, "\n## R² per inre tal (median över världar; ridge / slumpdrag)\n\n| Modell | z0 | z1 | z2 | z3 |\n|---|---|---|---|---|");
    for mi in [0usize, 1, 4] {
        let mut rad = format!("| {} |", M_NAMN[mi]);
        for d in 0..Z_N {
            let a = median(svar.iter().map(|s| s.m[mi][0].r2_d(d)).collect());
            if mi == 4 {
                let _ = write!(rad, " {} |", f3(a));
            } else {
                let b = median(svar.iter().map(|s| s.m[mi][1].r2_d(d)).collect());
                let _ = write!(rad, " {} / {} |", f3(a), f3(b));
            }
        }
        let _ = writeln!(t, "{rad}");
    }
    // Domarna enligt reglerna
    let r1: Vec<f64> = svar.iter().map(|s| bast(s, 0)).collect();
    let r2: Vec<f64> = svar.iter().map(|s| bast(s, 1)).collect();
    let diff: Vec<f64> = r1.iter().zip(&r2).map(|(a, b)| a - b).collect();
    let tak: Vec<f64> = svar.iter().map(bast_t).collect();
    let c = |v: &[f64], f: &dyn Fn(f64) -> bool| v.iter().filter(|&&x| f(x)).count();
    let stod = c(&diff, &|x| x >= 0.10) >= krav && c(&r1, &|x| x > 0.0) >= krav;
    let info_i_z = c(&r2, &|x| x > 0.0) >= krav && c(&diff, &|x| x < 0.10) >= krav;
    let ingen = c(&r1, &|x| x <= 0.0) >= krav;
    let _ = writeln!(t, "\n## Domarna (regeln skriven före körningen; den bästa av de två formerna per värld)\n");
    let _ = writeln!(t, "- Taket (T1, handlingens verkan på sinnena): median {}; under 0,10 i {} av {n} världar.", f3(median(tak.clone())), c(&tak, &|x| x < 0.10));
    let _ = writeln!(t, "- M1 − M2 minst 0,10: {} av {n}; M1 över 0: {} av {n}; M2 över 0: {} av {n}; M1 − M2 under 0,10: {} av {n}; M1 högst 0: {} av {n}.", c(&diff, &|x| x >= 0.10), c(&r1, &|x| x > 0.0), c(&r2, &|x| x > 0.0), c(&diff, &|x| x < 0.10), c(&r1, &|x| x <= 0.0));
    let dom = if stod {
        "STÖD FÖR MISSTANKEN OM KODAREN: sinnena bär information om handlingens verkan som de fyra inre talen saknar."
    } else if info_i_z {
        "INFORMATIONEN FINNS I DE INRE TALEN: felet sitter i förutsägaren eller i inlärningen, inte i kodaren."
    } else if ingen {
        "INGEN AV DEM: handlingens verkan på den inre bilden går inte att läsa ur sinnena med de här modellerna."
    } else {
        "OAVGJORT."
    };
    let _ = writeln!(t, "\n**Dom: {dom}** (EXTERN AVLÄSNING: säger vad som finns i talen, inte vad djuret använder.)");
    Ok(t)
}

// ---------------------------------------------------------------- de kostnadsfria analyserna ur Fas 1:s rader

fn fas1extra(mapp: &Path) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() {
        return Err("inga radfiler".into());
    }
    struct P {
        alder: i64,
        utfliv: f64,
        v_pred: Vec<f64>,
        v_sann: Vec<f64>,
        intag: Vec<f64>,
        rest: f64,
        verklig: f64,
    }
    let mut per_fro: Vec<(u64, Vec<P>)> = Vec::new();
    let mut rader_totalt = 0usize;
    for fil in &filer {
        let text = std::fs::read_to_string(fil).map_err(|e| e.to_string())?;
        let mut l = text.lines();
        let rub: Vec<&str> = l.next().ok_or("tom fil")?.split('\t').collect();
        let k = |n: &str| rub.iter().position(|x| *x == n).ok_or(format!("kolumnen {n} saknas"));
        let (kf, kar, ku, kal, ka, kul, kr, kv, kvp, kvs, ki) = (k("fro")?, k("ar")?, k("uid")?, k("alder")?, k("a")?, k("utfliv")?, k("rest_h")?, k("verklig")?, k("v_pred")?, k("v_sann")?, k("intag")?);
        let mut karta: BTreeMap<(i64, u64), P> = BTreeMap::new();
        let mut fro = 0u64;
        for (nr, rad) in l.enumerate() {
            let c: Vec<&str> = rad.split('\t').collect();
            if c.len() != rub.len() {
                return Err(format!("{} rad {}: {} kolumner, väntat {}", fil.display(), nr + 2, c.len(), rub.len()));
            }
            let f = |i: usize| c[i].parse::<f64>().map_err(|e| format!("{} rad {}: {e}", fil.display(), nr + 2));
            fro = c[kf].parse().map_err(|e| format!("{e}"))?;
            let a: usize = c[ka].parse().map_err(|e| format!("{e}"))?;
            let e = karta.entry((c[kar].parse().map_err(|e| format!("{e}"))?, c[ku].parse().map_err(|e| format!("{e}"))?)).or_insert(P {
                alder: c[kal].parse().map_err(|e| format!("{e}"))?,
                utfliv: 0.0,
                v_pred: vec![],
                v_sann: vec![],
                intag: vec![],
                rest: 0.0,
                verklig: 0.0,
            });
            if e.v_pred.len() != a {
                return Err(format!("{} rad {}: handling {a} i fel ordning", fil.display(), nr + 2));
            }
            e.utfliv += f(kul)?;
            e.v_pred.push(f(kvp)?);
            e.v_sann.push(f(kvs)?);
            e.intag.push(f(ki)?);
            e.rest += f(kr)?;
            e.verklig += f(kv)?;
            rader_totalt += 1;
        }
        let v: Vec<P> = karta.into_values().collect();
        if v.iter().any(|p| p.v_pred.len() != A_N) {
            return Err(format!("{}: ett prov saknar handlingar", fil.display()));
        }
        per_fro.push((fro, v));
    }
    let argmax = |v: &[f64]| { let mut b = 0; for i in 1..v.len() { if v[i] > v[b] { b = i; } } b };
    let kvartiler = |mut v: Vec<f64>| -> [f64; 3] {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
        [q(0.25), q(0.5), q(0.75)]
    };
    let mut t = String::new();
    let prov_totalt: usize = per_fro.iter().map(|(_, v)| v.len()).sum();
    let _ = writeln!(t, "# De kostnadsfria analyserna ur Fas 1:s rader (tal räknade av `vminfo fas1extra`)\n");
    let _ = writeln!(t, "Underlag: {rader_totalt} rader = {prov_totalt} prov × 9 handlingar, {} världar. Regler: piharness `docs/evolution/vmodell-info-utfallsregel.md` (sista avsnittet).\n", per_fro.len());
    let _ = writeln!(t, "## Värdet mot intaget\n\nRangkorrelation (Spearman) över de nio handlingarna i varje prov; prov där intaget är lika för alla nio räknas inte (antalet står i tabellen). Träff = hjärnans bästa handling efter v_pred ger också störst intag (slumpen 0,111).\n");
    let _ = writeln!(t, "| Frö | Prov | Utan spridning i intaget | v_pred mot intag, median | v_sann mot intag, median | Träff |\n|---|---|---|---|---|---|");
    let (mut alla_p, mut alla_s, mut traff, mut n_traff, mut utan) = (Vec::new(), Vec::new(), 0usize, 0usize, 0usize);
    for (fro, v) in &per_fro {
        let (mut rp, mut rs, mut tr, mut ut, mut nt) = (Vec::new(), Vec::new(), 0, 0, 0);
        for p in v {
            let spridd = p.intag.iter().any(|x| (x - p.intag[0]).abs() > 1e-12);
            if !spridd {
                ut += 1;
                continue;
            }
            rp.push(spearman(&p.v_pred, &p.intag));
            rs.push(spearman(&p.v_sann, &p.intag));
            let imax = p.intag.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            nt += 1;
            if (p.intag[argmax(&p.v_pred)] - imax).abs() <= 1e-12 {
                tr += 1;
            }
        }
        let _ = writeln!(t, "| {fro} | {} | {ut} | {} | {} | {} |", v.len(), f3(median(rp.clone())), f3(median(rs.clone())), f3(tr as f64 / nt.max(1) as f64));
        alla_p.extend(rp);
        alla_s.extend(rs);
        traff += tr;
        n_traff += nt;
        utan += ut;
    }
    let _ = writeln!(t, "| Alla | {prov_totalt} | {utan} | {} | {} | {} |", f3(median(alla_p.clone())), f3(median(alla_s)), f3(traff as f64 / n_traff.max(1) as f64));
    let kp = kvartiler(alla_p);
    let _ = writeln!(t, "\nKvartilerna för v_pred mot intaget: {} / {} / {}.", f3(kp[0]), f3(kp[1]), f3(kp[2]));
    let _ = writeln!(t, "\n## R² per prov (handlingens verkan, ur rest_h och verklig)\n\nProv där den verkliga delen är 0 räknas inte.\n");
    let _ = writeln!(t, "| Frö | Prov | Nedre kvartil | Median | Övre kvartil | Andel över 0 |\n|---|---|---|---|---|---|");
    let mut alla = Vec::new();
    let mut alla_pr: Vec<(i64, f64, f64)> = Vec::new();
    for (fro, v) in &per_fro {
        let r: Vec<f64> = v.iter().filter(|p| p.verklig > 1e-12).map(|p| 1.0 - p.rest / p.verklig).collect();
        let q = kvartiler(r.clone());
        let _ = writeln!(t, "| {fro} | {} | {} | {} | {} | {} |", r.len(), f3(q[0]), f3(q[1]), f3(q[2]), f3(r.iter().filter(|&&x| x > 0.0).count() as f64 / r.len() as f64));
        alla.extend(r);
        alla_pr.extend(v.iter().filter(|p| p.verklig > 1e-12).map(|p| (p.alder, p.utfliv, 1.0 - p.rest / p.verklig)));
    }
    let q = kvartiler(alla.clone());
    let _ = writeln!(t, "| Alla | {} | {} | {} | {} | {} |", alla.len(), f3(q[0]), f3(q[1]), f3(q[2]), f3(alla.iter().filter(|&&x| x > 0.0).count() as f64 / alla.len() as f64));
    let _ = writeln!(t, "\n### Efter ålder (år)\n\n| Ålder | Prov | Median | Andel över 0 |\n|---|---|---|---|");
    for (lo, hi, namn) in [(1, 2, "1–2"), (2, 4, "2–4"), (4, 8, "4–8"), (8, 1000, "8 eller mer")] {
        let r: Vec<f64> = alla_pr.iter().filter(|p| p.0 >= lo * STEG_PER_AR && p.0 < hi * STEG_PER_AR).map(|p| p.2).collect();
        if r.is_empty() {
            continue;
        }
        let _ = writeln!(t, "| {namn} | {} | {} | {} |", r.len(), f3(median(r.clone())), f3(r.iter().filter(|&&x| x > 0.0).count() as f64 / r.len() as f64));
    }
    let mut ul: Vec<f64> = alla_pr.iter().map(|p| p.1).collect();
    ul.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gr = [ul[ul.len() / 4], ul[ul.len() / 2], ul[3 * ul.len() / 4]];
    let _ = writeln!(t, "\n### Efter träning (summan av utföranden under livet över de nio handlingarna; kvartiler)\n\n| Träning | Prov | Median | Andel över 0 |\n|---|---|---|---|");
    let gr_namn = [format!("under {}", gr[0]), format!("{}–{}", gr[0], gr[1]), format!("{}–{}", gr[1], gr[2]), format!("{} eller mer", gr[2])];
    for g in 0..4 {
        let r: Vec<f64> = alla_pr
            .iter()
            .filter(|p| {
                let k = if p.1 < gr[0] { 0 } else if p.1 < gr[1] { 1 } else if p.1 < gr[2] { 2 } else { 3 };
                k == g
            })
            .map(|p| p.2)
            .collect();
        if r.is_empty() {
            continue;
        }
        let _ = writeln!(t, "| {} | {} | {} | {} |", gr_namn[g], r.len(), f3(median(r.clone())), f3(r.iter().filter(|&&x| x > 0.0).count() as f64 / r.len() as f64));
    }
    Ok(t)
}

// ---------------------------------------------------------------- diagnoserna A och B (tillägg 3 i reglerna)

/// Medel över världar med 95 %-intervall ur 2 000 omdragningar av världarna (frö 1984).
pub fn intervall(v: &[f64]) -> (f64, f64, f64) {
    let n = v.len();
    let medel = v.iter().sum::<f64>() / n as f64;
    let mut g = Rng::new(1984);
    let mut m: Vec<f64> = (0..2000).map(|_| (0..n).map(|_| v[g.under(n)]).sum::<f64>() / n as f64).collect();
    m.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (medel, m[49], m[1949])
}

fn idm_drag(p: &Prov, a: usize, sinnen: bool) -> Vec<f64> {
    let (fore, efter) = if sinnen { (&p.s, &p.s2[a]) } else { (&p.z, &p.zn[a]) };
    let mut x = fore.clone();
    x.extend(efter.iter());
    x.extend(fore.iter().zip(efter).map(|(f, e)| e - f));
    x
}

fn argmax(v: &[f64]) -> usize {
    let mut b = 0;
    for i in 1..v.len() {
        if v[i] > v[b] {
            b = i;
        }
    }
    b
}

/// A för en individ: förväxlingsmatrisen [verklig][gissad] på provmängden.
fn idm_individ(tr: &[&Prov], te: &[&Prov], form: Form, sinnen: bool) -> Result<[[u64; A_N]; A_N], String> {
    let mut x = Vec::new();
    let mut y = Vec::new();
    let mut d = Vec::new();
    for p in tr {
        for a in 0..A_N {
            x.push(idm_drag(p, a, sinnen));
            let mut e = vec![0.0; A_N];
            e[a] = 1.0;
            y.push(e);
            d.push((p.t / BLOCK) as usize);
        }
    }
    let m = Modell::anpassa(form, &[x], &[y], &[d], false)?;
    let mut fm = [[0u64; A_N]; A_N];
    for p in te {
        for a in 0..A_N {
            fm[a][argmax(&m.gissa(&idm_drag(p, a, sinnen), 0))] += 1;
        }
    }
    Ok(fm)
}

fn diagnos(mapp: &Path) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() {
        return Err("inga radfiler".into());
    }
    // [indata z/sinnen][form] → per värld träffsäkerhet och balanserad; matris över alla världar.
    let mut a_traff = vec![vec![Vec::new(); 2]; 2];
    let mut a_bal = vec![vec![Vec::new(); 2]; 2];
    let mut a_kvar = vec![vec![Vec::new(); 2]; 2];
    let mut a_mat = vec![vec![[[0u64; A_N]; A_N]; 2]; 2];
    let (mut b_rho, mut b_rho_s, mut b_traff, mut b_tak) = (Vec::new(), Vec::new(), Vec::new(), vec![Vec::new(); 2]);
    let (mut b_ogiltiga, mut b_prov, mut individer_tot) = (0u64, 0u64, 0usize);
    let mut rader = String::new();
    for fil in &filer {
        let (fro, prov) = las_rader(fil)?;
        eprintln!("diagnos frö {fro}: {} prov", prov.len());
        let mut per: BTreeMap<u64, Vec<&Prov>> = BTreeMap::new();
        for p in &prov {
            per.entry(p.uid).or_default().push(p);
        }
        let mut mat = vec![vec![[[0u64; A_N]; A_N]; 2]; 2];
        let mut tak = vec![Summa::default(); 2];
        let mut individer = 0;
        for pv in per.values() {
            if pv.len() < MIN_STEG {
                continue;
            }
            let tr: Vec<&Prov> = pv.iter().filter(|p| !p.prov).copied().collect();
            let te: Vec<&Prov> = pv.iter().filter(|p| p.prov).copied().collect();
            if tr.len() < 20 || te.is_empty() {
                continue;
            }
            individer += 1;
            for (ii, sinnen) in [false, true].into_iter().enumerate() {
                for (fi, form) in [Form::Ridge, Form::Drag].into_iter().enumerate() {
                    let fm = idm_individ(&tr, &te, form, sinnen)?;
                    for v in 0..A_N {
                        for g in 0..A_N {
                            mat[ii][fi][v][g] += fm[v][g];
                        }
                    }
                }
            }
            // B taket: z + a → ΔE per individ.
            let de = |p: &Prov, a: usize| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]];
            let (x, y, d) = traning(&tr, |p: &Prov| p.z.clone(), de, |p: &Prov| (p.t / BLOCK) as usize);
            for (fi, form) in [Form::Ridge, Form::Drag].into_iter().enumerate() {
                let m = Modell::anpassa(form, &x, &y, &d, true)?;
                for p in &te {
                    let g: Vec<Vec<f64>> = (0..A_N).map(|a| m.gissa(&p.z, a)).collect();
                    let u: Vec<Vec<f64>> = (0..A_N).map(|a| de(p, a)).collect();
                    tak[fi].lagg(&bidrag(&g, &u, &[0.0]));
                }
            }
        }
        individer_tot += individer;
        for ii in 0..2 {
            for fi in 0..2 {
                let m = &mat[ii][fi];
                let tot: u64 = m.iter().flatten().sum();
                let ratt: u64 = (0..A_N).map(|k| m[k][k]).sum();
                let bal = (0..A_N).map(|k| m[k][k] as f64 / m[k].iter().sum::<u64>().max(1) as f64).sum::<f64>() / A_N as f64;
                // Stå kvar (0) mot flytt (1–8): rätt om båda är 0 eller båda är flyttar.
                let kvar_ratt: u64 = (0..A_N).flat_map(|v| (0..A_N).map(move |g| (v, g))).filter(|&(v, g)| (v == 0) == (g == 0)).map(|(v, g)| m[v][g]).sum();
                a_traff[ii][fi].push(ratt as f64 / tot.max(1) as f64);
                a_bal[ii][fi].push(bal);
                a_kvar[ii][fi].push(kvar_ratt as f64 / tot.max(1) as f64);
                for v in 0..A_N {
                    for g in 0..A_N {
                        a_mat[ii][fi][v][g] += m[v][g];
                    }
                }
            }
        }
        // B per prov (alla prov i världen, också individer med få steg; måttet är inom samma läge).
        let (mut rho, mut rho_s, mut traff, mut n_tr) = (Vec::new(), Vec::new(), 0u64, 0u64);
        for p in &prov {
            b_prov += 1;
            let de: Vec<f64> = (0..A_N).map(|a| p.s2[a][S_N - 1] - p.s[S_N - 1]).collect();
            let (lo, hi) = de.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
            if hi - lo < 1e-12 {
                b_ogiltiga += 1;
                continue;
            }
            rho.push(spearman(&p.v_pred, &de));
            rho_s.push(spearman(&p.v_sann, &de));
            n_tr += 1;
            if de[argmax(&p.v_pred)] >= hi - 1e-12 {
                traff += 1;
            }
        }
        let medel = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        b_rho.push(medel(&rho));
        b_rho_s.push(medel(&rho_s));
        b_traff.push(traff as f64 / n_tr.max(1) as f64);
        for fi in 0..2 {
            b_tak[fi].push(tak[fi].r2());
        }
        let _ = writeln!(
            rader,
            "| {fro} | {individer} | {} | {} | {} | {} | {} | {} | {} |",
            f3(a_traff[0][0].last().copied().unwrap()),
            f3(a_traff[0][1].last().copied().unwrap()),
            f3(a_traff[1][0].last().copied().unwrap()),
            f3(a_traff[1][1].last().copied().unwrap()),
            f3(b_rho.last().copied().unwrap()),
            f3(b_traff.last().copied().unwrap()),
            f3(tak[0].r2().max(tak[1].r2()))
        );
    }
    let n = filer.len();
    let iv = |v: &[f64]| {
        let (m, lo, hi) = intervall(v);
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |f: f64| s[((s.len() - 1) as f64 * f).round() as usize];
        format!("{} [{}; {}] | {} | {} / {} | {} / {}", f3(m), f3(lo), f3(hi), f3(median(v.to_vec())), f3(q(0.25)), f3(q(0.75)), f3(s[0]), f3(s[s.len() - 1]))
    };
    let mut t = String::new();
    let _ = writeln!(t, "# Diagnoserna A och B ur informationstestets rader: EXTERN AVLÄSNING (tal räknade av `vminfo diagnos`)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/vmodell-info-utfallsregel.md`, tillägg 3 (skrivet före resultatet). {n} världar, {individer_tot} individer med minst {MIN_STEG} steg. Inga gränser för PASS eller FAIL; intervallen är 95 % ur 2 000 omdragningar av världarna.\n");
    let _ = writeln!(t, "## A. Känna igen handlingen ur bilden före och efter steget (slumpen 0,111)\n");
    let _ = writeln!(t, "| Indata | Form | Medel över världar [95 %-intervall] | Median | Kvartiler | Lägst / högst |\n|---|---|---|---|---|---|");
    for (ii, namn) in ["inre bilden z, z′ (4 + 4 tal)", "sinnena s, s′ (19 + 19 tal)"].iter().enumerate() {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let _ = writeln!(t, "| {namn} | {form} | {} |", iv(&a_traff[ii][fi]));
        }
    }
    let _ = writeln!(t, "\nBalanserad träffsäkerhet (kontroll; ska vara lika eftersom handlingarna är lika vanliga):\n");
    for (ii, namn) in ["inre bilden", "sinnena"].iter().enumerate() {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let _ = writeln!(t, "- {namn}, {form}: medel {} (träffsäkerheten {})", f3(a_bal[ii][fi].iter().sum::<f64>() / n as f64), f3(a_traff[ii][fi].iter().sum::<f64>() / n as f64));
        }
    }
    let _ = writeln!(t, "\nStå kvar mot flytt (två klasser; att alltid gissa flytt ger 8/9 = 0,889):\n");
    for (ii, namn) in ["inre bilden", "sinnena"].iter().enumerate() {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let _ = writeln!(t, "- {namn}, {form}: {}", iv(&a_kvar[ii][fi]));
        }
    }
    for (ii, namn) in ["inre bilden", "sinnena"].iter().enumerate() {
        let fi = if a_traff[ii][1].iter().sum::<f64>() > a_traff[ii][0].iter().sum::<f64>() { 1 } else { 0 };
        let _ = writeln!(t, "\nFörväxlingsmatris, {namn}, {} (rad = verklig handling, kolumn = gissad; alla världar):\n", if fi == 0 { "ridge" } else { "slumpdrag" });
        let _ = writeln!(t, "| verklig \\ gissad | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n|---|---|---|---|---|---|---|---|---|---|");
        for v in 0..A_N {
            let _ = writeln!(t, "| {v} | {} |", a_mat[ii][fi][v].iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" | "));
        }
    }
    let _ = writeln!(t, "\n## B. Värderingen mot energins ändring inom samma läge\n");
    let _ = writeln!(t, "Prov: {b_prov}; utan spridning i energins ändring (räknas inte): {b_ogiltiga}.\n");
    let _ = writeln!(t, "| Mått | Medel över världar [95 %-intervall] | Median | Kvartiler | Lägst / högst |\n|---|---|---|---|---|");
    let _ = writeln!(t, "| Rangkorrelation v_pred mot ΔE (primärt) | {} |", iv(&b_rho));
    let _ = writeln!(t, "| Rangkorrelation v_sann mot ΔE | {} |", iv(&b_rho_s));
    let _ = writeln!(t, "| Högst v_pred ger störst ΔE (slumpen 0,111) | {} |", iv(&b_traff));
    let _ = writeln!(t, "| Taket: R² z + a → ΔE, ridge | {} |", iv(&b_tak[0]));
    let _ = writeln!(t, "| Taket: R² z + a → ΔE, slumpdrag | {} |", iv(&b_tak[1]));
    let _ = writeln!(t, "\nDet tidigare måttet (värderingen mot födointaget, Fas 1:s rader): median −0,017 (piharness `docs/evolution/vmodell-info/1-kostnadsfria-resultat.md`); det ersätts inte av B.");
    let _ = writeln!(t, "\n## Per värld\n\n| Frö | Individer | A z ridge | A z slumpdrag | A sinnen ridge | A sinnen slumpdrag | B rangkorr. | B träff | B taket (bästa formen) |\n|---|---|---|---|---|---|---|---|---|");
    t.push_str(&rader);
    Ok(t)
}

// ---------------------------------------------------------------- tillägg 4: uppdelat T1, s+a→ΔE/intag, förskjuten invers (Alt 2000)

#[derive(Clone, Copy)]
enum SinnesGrupp {
    Alla,
    VaxtStannar,
    VaxtNy,
    VaxtAlla,
    AntalStannar,
    AntalNy,
    AntalAlla,
    Energi,
}

const GRUPPER: [(SinnesGrupp, &str, f64); 8] = [
    (SinnesGrupp::Alla, "Alla 19 sinnen (kontroll)", 19.0),
    (SinnesGrupp::VaxtStannar, "Växt i rutor som stannar", 49.0 / 9.0),
    (SinnesGrupp::VaxtNy, "Växt i nya rutor", 32.0 / 9.0),
    (SinnesGrupp::VaxtAlla, "Växt alla 9", 9.0),
    (SinnesGrupp::AntalStannar, "Antal djur i rutor som stannar", 49.0 / 9.0),
    (SinnesGrupp::AntalNy, "Antal djur i nya rutor", 32.0 / 9.0),
    (SinnesGrupp::AntalAlla, "Antal djur alla 9", 9.0),
    (SinnesGrupp::Energi, "Energi (s18)", 1.0),
];

fn stannar_i_fonster(a: usize, k2: usize) -> bool {
    let (di, dj) = GRANNAR[a];
    let (gi, gj) = GRANNAR[k2];
    (di + gi).abs() <= 1 && (dj + gj).abs() <= 1
}

fn grupp_har(g: SinnesGrupp, a: usize, d: usize) -> bool {
    match g {
        SinnesGrupp::Alla => true,
        SinnesGrupp::VaxtStannar => d < 18 && d % 2 == 0 && stannar_i_fonster(a, d / 2),
        SinnesGrupp::VaxtNy => d < 18 && d % 2 == 0 && !stannar_i_fonster(a, d / 2),
        SinnesGrupp::VaxtAlla => d < 18 && d % 2 == 0,
        SinnesGrupp::AntalStannar => d < 18 && d % 2 == 1 && stannar_i_fonster(a, d / 2),
        SinnesGrupp::AntalNy => d < 18 && d % 2 == 1 && !stannar_i_fonster(a, d / 2),
        SinnesGrupp::AntalAlla => d < 18 && d % 2 == 1,
        SinnesGrupp::Energi => d == 18,
    }
}

fn bidrag_grupp(gissn: &[Vec<f64>], utf: &[Vec<f64>], fore: &[f64], g: SinnesGrupp) -> Summa {
    let q = fore.len();
    let mut mg = vec![0.0; q];
    let mut mu = vec![0.0; q];
    for a in 0..A_N {
        for d in 0..q {
            mg[d] += gissn[a][d] / A_N as f64;
            mu[d] += utf[a][d] / A_N as f64;
        }
    }
    let mut s = Summa { prov: 1, ..Default::default() };
    for a in 0..A_N {
        for d in 0..q {
            if !grupp_har(g, a, d) {
                continue;
            }
            let fd = gissn[a][d] - mg[d];
            let vd = utf[a][d] - mu[d];
            s.rest += (fd - vd).powi(2);
            s.verklig += vd * vd;
            s.fel += (gissn[a][d] - utf[a][d]).powi(2);
            s.fel_pers += (fore[d] - utf[a][d]).powi(2);
        }
    }
    s
}

fn overlapp_par(h: usize) -> Vec<(usize, usize)> {
    let (di, dj) = GRANNAR[h];
    let mut ut = Vec::new();
    for k2 in 0..9 {
        let (gi, gj) = GRANNAR[k2];
        let target = (di + gi, dj + gj);
        if let Some(k1) = GRANNAR.iter().position(|&p| p == target) {
            ut.push((k1, k2));
        }
    }
    ut
}

fn forskjutna_avstand(s_fore: &[f64], s_efter: &[f64]) -> ([f64; A_N], [f64; A_N]) {
    let mut dv = [0.0; A_N];
    let mut da = [0.0; A_N];
    for h in 0..A_N {
        let par = overlapp_par(h);
        let n = par.len() as f64;
        let (mut sv, mut sa) = (0.0, 0.0);
        for (k1, k2) in par {
            sv += (s_efter[k2 * 2] - s_fore[k1 * 2]).powi(2);
            sa += (s_efter[k2 * 2 + 1] - s_fore[k1 * 2 + 1]).powi(2);
        }
        dv[h] = sv / n;
        da[h] = sa / n;
    }
    (dv, da)
}

fn forutsag_forskjuten_drag(s_fore: &[f64], s_efter: &[f64]) -> Vec<f64> {
    let (dv, da) = forskjutna_avstand(s_fore, s_efter);
    let mut x = Vec::with_capacity(2 * A_N + 1);
    x.extend_from_slice(&dv);
    x.extend_from_slice(&da);
    x.push(s_efter[S_N - 1] - s_fore[S_N - 1]);
    x
}

fn argmin(v: &[f64]) -> usize {
    let mut b = 0;
    for i in 1..v.len() {
        if v[i] < v[b] {
            b = i;
        }
    }
    b
}

fn tillagg4(mapp: &Path) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() {
        return Err("inga radfiler".into());
    }
    let former = [Form::Ridge, Form::Drag];
    let mut t1_grp = vec![vec![Vec::new(); 2]; GRUPPER.len()]; // [grupp][form] -> Vec<Summa> per värld
    let mut de_sum = vec![Vec::new(); 2];
    let mut intag_sum = vec![Vec::new(); 2];
    let mut inv_vaxt = Vec::new();
    let mut inv_alla = Vec::new();
    let mut inv_ridge = Vec::new();
    let mut inv_drag = Vec::new();

    for fil in &filer {
        let (fro, prov) = las_rader(fil)?;
        eprintln!("tillagg4 frö {fro}: {} prov", prov.len());
        let mut per: BTreeMap<u64, Vec<&Prov>> = BTreeMap::new();
        for p in &prov {
            per.entry(p.uid).or_default().push(p);
        }

        // 1 & 2: Samma 50/50-individdelning per värld som T1.
        let mut uids: Vec<u64> = per.keys().copied().collect();
        let mut g = Rng::new(fro ^ 0x7171);
        g.blanda(&mut uids);
        let (tr_u, te_u) = uids.split_at(uids.len() / 2);
        let tr: Vec<&Prov> = tr_u.iter().flat_map(|u| per[u].iter().copied()).collect();
        let te: Vec<&Prov> = te_u.iter().flat_map(|u| per[u].iter().copied()).collect();
        let grupp: std::collections::HashMap<u64, usize> = tr_u.iter().enumerate().map(|(i, u)| (*u, i % 5)).collect();

        let s_in = |p: &Prov| p.s.clone();
        let s2_ut = |p: &Prov, a: usize| p.s2[a].clone();
        let de_ut = |p: &Prov, a: usize| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]];
        let intag_ut = |p: &Prov, a: usize| vec![p.intag[a]];

        let (xt, yt, dt) = traning(&tr, s_in, s2_ut, |p: &Prov| grupp[&p.uid]);
        let (xde, yde, dde) = traning(&tr, s_in, de_ut, |p: &Prov| grupp[&p.uid]);
        let (xin, yin, din) = traning(&tr, s_in, intag_ut, |p: &Prov| grupp[&p.uid]);

        for (fi, &form) in former.iter().enumerate() {
            let mt = Modell::anpassa(form, &xt, &yt, &dt, true)?;
            let mde = Modell::anpassa(form, &xde, &yde, &dde, true)?;
            let min = Modell::anpassa(form, &xin, &yin, &din, true)?;

            let mut gs_gr = vec![Summa::default(); GRUPPER.len()];
            let mut sd = Summa::default();
            let mut si = Summa::default();
            for p in &te {
                let g_s2: Vec<Vec<f64>> = (0..A_N).map(|a| mt.gissa(&p.s, a)).collect();
                for (gi, &(gr, _, _)) in GRUPPER.iter().enumerate() {
                    gs_gr[gi].lagg(&bidrag_grupp(&g_s2, &p.s2, &p.s, gr));
                }
                let g_de: Vec<Vec<f64>> = (0..A_N).map(|a| mde.gissa(&p.s, a)).collect();
                let u_de: Vec<Vec<f64>> = (0..A_N).map(|a| de_ut(p, a)).collect();
                sd.lagg(&bidrag(&g_de, &u_de, &[0.0]));

                let g_in: Vec<Vec<f64>> = (0..A_N).map(|a| min.gissa(&p.s, a)).collect();
                let u_in: Vec<Vec<f64>> = (0..A_N).map(|a| intag_ut(p, a)).collect();
                si.lagg(&bidrag(&g_in, &u_in, &[0.0]));
            }
            // Kontroll: de fem disjunkta delgrupperna (1, 2, 4, 5, 7) summerar till total (0).
            let del_sum: f64 = [1usize, 2, 4, 5, 7].iter().map(|&idx| gs_gr[idx].verklig).sum();
            assert!((del_sum - gs_gr[0].verklig).abs() < 1e-8, "delgruppernas verklig {} != total {}", del_sum, gs_gr[0].verklig);

            for gi in 0..GRUPPER.len() {
                t1_grp[gi][fi].push(gs_gr[gi].clone());
            }
            de_sum[fi].push(sd);
            intag_sum[fi].push(si);
        }

        // 3. Förskjuten invers modell (individer med minst 150 steg, samma provblock som Diagnos A).
        let (mut cv, mut ca, mut cr, mut cd, mut tot_inv) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for pv in per.values() {
            if pv.len() < MIN_STEG {
                continue;
            }
            let tr_i: Vec<&Prov> = pv.iter().filter(|p| !p.prov).copied().collect();
            let te_i: Vec<&Prov> = pv.iter().filter(|p| p.prov).copied().collect();
            if tr_i.len() < 20 || te_i.is_empty() {
                continue;
            }
            let mut x = Vec::new();
            let mut y = Vec::new();
            let mut d = Vec::new();
            for p in &tr_i {
                for a in 0..A_N {
                    x.push(forutsag_forskjuten_drag(&p.s, &p.s2[a]));
                    let mut e = vec![0.0; A_N];
                    e[a] = 1.0;
                    y.push(e);
                    d.push((p.t / BLOCK) as usize);
                }
            }
            let mr = Modell::anpassa(Form::Ridge, &[x.clone()], &[y.clone()], &[d.clone()], false)?;
            let md = Modell::anpassa(Form::Drag, &[x], &[y], &[d], false)?;
            for p in &te_i {
                for a in 0..A_N {
                    let (dv, da) = forskjutna_avstand(&p.s, &p.s2[a]);
                    if argmin(&dv) == a {
                        cv += 1;
                    }
                    let comb: Vec<f64> = (0..A_N).map(|h| dv[h] + da[h]).collect();
                    if argmin(&comb) == a {
                        ca += 1;
                    }
                    let drag = forutsag_forskjuten_drag(&p.s, &p.s2[a]);
                    if argmax(&mr.gissa(&drag, 0)) == a {
                        cr += 1;
                    }
                    if argmax(&md.gissa(&drag, 0)) == a {
                        cd += 1;
                    }
                    tot_inv += 1;
                }
            }
        }
        inv_vaxt.push(cv as f64 / tot_inv.max(1) as f64);
        inv_alla.push(ca as f64 / tot_inv.max(1) as f64);
        inv_ridge.push(cr as f64 / tot_inv.max(1) as f64);
        inv_drag.push(cd as f64 / tot_inv.max(1) as f64);
    }

    let n = filer.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Tillägg 4 (Alt 2000): uppdelat T1, s+a→ΔE/intag och förskjuten invers: EXTERN AVLÄSNING\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/vmodell-info-utfallsregel.md` tillägg 4. {n} världar; gräns tydlig signal: R² ≥ 0,10 i minst {krav} av {n} världar.\n");

    let _ = writeln!(t, "## 1. Uppdelat T1 per sinnesgrupp\n");
    let _ = writeln!(t, "| Sinnesgrupp | Form | R² median | Lägst / högst | Världar ≥ 0,10 | Andel av total verklig (median) | Bedömbar (varians/kanal ≥ 1e-6) |\n|---|---|---|---|---|---|---|");
    for (gi, &(_, namn, kanaler)) in GRUPPER.iter().enumerate() {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let r2s: Vec<f64> = t1_grp[gi][fi].iter().map(|s| s.r2()).collect();
            let andel: Vec<f64> = (0..n).map(|w| 100.0 * t1_grp[gi][fi][w].verklig / t1_grp[0][fi][w].verklig.max(1e-300)).collect();
            let vpk: Vec<f64> = t1_grp[gi][fi].iter().map(|s| s.verklig / (s.prov as f64 * kanaler)).collect();
            let bedombar = if median(vpk) >= 1e-6 { "ja" } else { "nej (inte bedömbar)" };
            let lo = r2s.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = r2s.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let over = r2s.iter().filter(|&&x| x >= 0.10).count();
            let _ = writeln!(t, "| {namn} | {form} | {} | {} / {} | {over} av {n} | {:.1} % | {bedombar} |", f3(median(r2s)), f3(lo), f3(hi), median(andel));
        }
    }

    let _ = writeln!(t, "\n## 2. s + a → ΔE och s + a → intag\n");
    let _ = writeln!(t, "| Mål | Form | R² median | Lägst / högst | Världar ≥ 0,10 | Felkvot median |\n|---|---|---|---|---|---|");
    for (namn, lst) in [("ΔE (s'_18 − s_18, klippt 0–2)", &de_sum), ("Rått intag", &intag_sum)] {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let r2s: Vec<f64> = lst[fi].iter().map(|s| s.r2()).collect();
            let fks: Vec<f64> = lst[fi].iter().map(|s| s.kvot()).collect();
            let lo = r2s.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = r2s.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let over = r2s.iter().filter(|&&x| x >= 0.10).count();
            let _ = writeln!(t, "| {namn} | {form} | {} | {} / {} | {over} av {n} | {} |", f3(median(r2s)), f3(lo), f3(hi), f3(median(fks)));
        }
    }

    let _ = writeln!(t, "\n## 3. Förskjuten invers modell för Diagnos A (slumpen 0,111)\n");
    let _ = writeln!(t, "| Metod | Medel [95 %-intervall] | Median | Lägst / högst |\n|---|---|---|---|");
    for (namn, v) in [
        ("Ren parameterfri argmin d_vaxt", &inv_vaxt),
        ("Ren parameterfri argmin (d_vaxt + d_antal)", &inv_alla),
        ("Tränad ridge på 18 avstånd + ΔE", &inv_ridge),
        ("Tränad slumpdrag på 18 avstånd + ΔE", &inv_drag),
    ] {
        let (m, lo, hi) = intervall(v);
        let vmin = v.iter().cloned().fold(f64::INFINITY, f64::min);
        let vmax = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let _ = writeln!(t, "| {namn} | {} [{}; {}] | {} | {} / {} |", f3(m), f3(lo), f3(hi), f3(median(v.clone())), f3(vmin), f3(vmax));
    }
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2005 Steg 0: skuggmätning på befintliga radfiler

/// Återvinner en individs fasta kodarmatris `W_k` (`Z_N × (S_N + 1)`) ur omättade `(s, z)`-par via `atanh(z)`.
/// Returnerar `(W_k, max_abs_fel)` över individens samtliga rader.
fn atervinn_kod(pv: &[&Prov]) -> Result<([[f64; S_N + 1]; Z_N], f64), String> {
    const P: usize = S_N + 1;
    let mut par: Vec<(&[f64], &[f64])> = Vec::with_capacity(pv.len() * (1 + A_N));
    for p in pv {
        par.push((&p.s, &p.z));
        for a in 0..A_N {
            par.push((&p.s2[a], &p.zn[a]));
        }
    }
    let mut w = [[0.0; P]; Z_N];
    for i in 0..Z_N {
        let mut ata = vec![vec![0.0; P]; P];
        let mut atb = vec![vec![0.0; 1]; P];
        let mut n_omattade = 0usize;
        for &(s, z) in &par {
            if z[i].abs() >= 0.9999 {
                continue;
            }
            n_omattade += 1;
            let t = z[i].atanh();
            let mut x = [1.0; P];
            x[..S_N].copy_from_slice(s);
            for r in 0..P {
                for c in 0..=r {
                    ata[r][c] += x[r] * x[c];
                }
                atb[r][0] += x[r] * t;
            }
        }
        let lam = if n_omattade >= P + 5 { 1e-12 } else { 1e-6 };
        for r in 0..P {
            for c in 0..r {
                ata[c][r] = ata[r][c];
            }
            ata[r][r] += lam;
        }
        cholesky_los(&mut ata, &mut atb)?;
        for r in 0..P {
            w[i][r] = atb[r][0];
        }
    }
    let mut max_fel = 0.0f64;
    for &(s, z) in &par {
        for i in 0..Z_N {
            let lin = w[i][S_N] + (0..S_N).map(|j| w[i][j] * s[j]).sum::<f64>();
            let pred = lin.tanh();
            max_fel = max_fel.max((pred - z[i]).abs());
        }
    }
    Ok((w, max_fel))
}

fn koda_med(w: &[[f64; S_N + 1]; Z_N], s: &[f64]) -> Vec<f64> {
    (0..Z_N)
        .map(|i| {
            let lin = w[i][S_N] + (0..S_N).map(|j| w[i][j] * s[j]).sum::<f64>();
            lin.tanh()
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkuggForm {
    Kontroll,
    Ema,
    Welford,
    StruknaRa,
    StruknaEma,
}

const SKUGG_NAMN: [(SkuggForm, &str); 5] = [
    (SkuggForm::Kontroll, "Kontroll (rå s via återvunnen W_k)"),
    (SkuggForm::Ema, "Löpande z-poäng (EMA η=0,02, [-3,3])"),
    (SkuggForm::Welford, "Löpande z-poäng (Welford, [-3,3])"),
    (SkuggForm::StruknaRa, "Strukna djurantal (rå växt+energi; diagnos)"),
    (SkuggForm::StruknaEma, "Strukna djurantal + EMA z-poäng (diagnos)"),
];

/// Skapar skuggkopior av en individs prov kausalt i tidsordning utan att ändra `s2[18]`/`s[18]` i `ΔE` eller `intag`.
fn skugga_individ(pv: &[&Prov], w: &[[f64; S_N + 1]; Z_N], form: SkuggForm) -> Vec<Prov> {
    let mut mu = pv[0].s.clone();
    let mut var = vec![1.0_f64; S_N];
    let mut m2 = vec![0.0_f64; S_N];
    let mut n_wel = 1.0f64;
    let eta = 0.02f64;
    let mut ut = Vec::with_capacity(pv.len());

    for (idx, &p) in pv.iter().enumerate() {
        let skala_vec = |sv: &[f64]| -> Vec<f64> {
            let mut res = vec![0.0; S_N];
            for j in 0..S_N {
                let ar_antal = j < 18 && j % 2 == 1;
                res[j] = match form {
                    SkuggForm::Kontroll => sv[j],
                    SkuggForm::StruknaRa => if ar_antal { 0.0 } else { sv[j] },
                    SkuggForm::Ema | SkuggForm::Welford => ((sv[j] - mu[j]) / (var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0),
                    SkuggForm::StruknaEma => {
                        if ar_antal {
                            0.0
                        } else {
                            ((sv[j] - mu[j]) / (var[j] + 1e-4).sqrt()).clamp(-3.0, 3.0)
                        }
                    }
                };
            }
            res
        };
        let s_sk = skala_vec(&p.s);
        let z_sk = koda_med(w, &s_sk);
        let s2_sk: Vec<Vec<f64>> = (0..A_N).map(|a| skala_vec(&p.s2[a])).collect();
        let zn_sk: Vec<Vec<f64>> = s2_sk.iter().map(|sv| koda_med(w, sv)).collect();
        ut.push(Prov {
            uid: p.uid,
            t: p.t,
            prov: p.prov,
            s: s_sk,
            z: z_sk,
            s2: s2_sk,
            zn: zn_sk,
            y: p.y.clone(),
            v_pred: p.v_pred.clone(),
            v_sann: p.v_sann.clone(),
            intag: p.intag.clone(),
        });
        if idx > 0 {
            match form {
                SkuggForm::Ema | SkuggForm::StruknaEma => {
                    for j in 0..S_N {
                        let d = p.s[j] - mu[j];
                        mu[j] += eta * d;
                        var[j] = (1.0 - eta) * var[j] + eta * d * d;
                    }
                }
                SkuggForm::Welford => {
                    n_wel += 1.0;
                    for j in 0..S_N {
                        let d = p.s[j] - mu[j];
                        mu[j] += d / n_wel;
                        let d2 = p.s[j] - mu[j];
                        m2[j] += d * d2;
                        var[j] = (m2[j] / (n_wel - 1.0)).max(0.0);
                    }
                }
                SkuggForm::Kontroll | SkuggForm::StruknaRa => {}
            }
        }
    }
    ut
}

/// Deterministisk Jacobi-egenvärdeslösare för symmetrisk d×d-kovariansmatris; returnerar (topp-4 egenvektorer, förklarad variansandel).
struct Pca4 {
    dims: Vec<usize>,
    mu: Vec<f64>,
    sx: Vec<f64>,
    vekt: Vec<Vec<f64>>, // 4 × dims.len()
    andel_var: f64,
}

impl Pca4 {
    fn anpassa(tr: &[&Prov], dims: Vec<usize>, standardisera: bool) -> Pca4 {
        let p = dims.len();
        let n = tr.len() as f64;
        let mut mu = vec![0.0; p];
        for pr in tr {
            for (k, &d) in dims.iter().enumerate() {
                mu[k] += pr.s[d] / n;
            }
        }
        let mut sx = vec![1.0; p];
        if standardisera {
            let mut var = vec![0.0; p];
            for pr in tr {
                for (k, &d) in dims.iter().enumerate() {
                    var[k] += (pr.s[d] - mu[k]).powi(2) / n;
                }
            }
            for k in 0..p {
                sx[k] = skala(var[k].sqrt());
            }
        }
        let mut cov = vec![vec![0.0; p]; p];
        for pr in tr {
            let u: Vec<f64> = dims.iter().enumerate().map(|(k, &d)| (pr.s[d] - mu[k]) * sx[k]).collect();
            for i in 0..p {
                for j in 0..=i {
                    cov[i][j] += u[i] * u[j] / n;
                }
            }
        }
        for i in 0..p {
            for j in 0..i {
                cov[j][i] = cov[i][j];
            }
        }
        let mut v = vec![vec![0.0; p]; p];
        for i in 0..p {
            v[i][i] = 1.0;
        }
        for _ in 0..100 * p * p {
            let (mut bp, mut bq, mut bmax) = (0usize, 1usize, 0.0f64);
            for i in 0..p {
                for j in (i + 1)..p {
                    let av = cov[i][j].abs();
                    if av > bmax {
                        bmax = av;
                        bp = i;
                        bq = j;
                    }
                }
            }
            if bmax < 1e-12 {
                break;
            }
            let diff = cov[bq][bq] - cov[bp][bp];
            let tau = diff / (2.0 * cov[bp][bq]);
            let t = if tau >= 0.0 {
                1.0 / (tau + (1.0 + tau * tau).sqrt())
            } else {
                -1.0 / (-tau + (1.0 + tau * tau).sqrt())
            };
            let c = 1.0 / (1.0 + t * t).sqrt();
            let s = t * c;
            let app = cov[bp][bp];
            let aqq = cov[bq][bq];
            let apq = cov[bp][bq];
            cov[bp][bp] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
            cov[bq][bq] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
            cov[bp][bq] = 0.0;
            cov[bq][bp] = 0.0;
            for r in 0..p {
                if r != bp && r != bq {
                    let arp = cov[r][bp];
                    let arq = cov[r][bq];
                    cov[r][bp] = c * arp - s * arq;
                    cov[bp][r] = cov[r][bp];
                    cov[r][bq] = s * arp + c * arq;
                    cov[bq][r] = cov[r][bq];
                }
            }
            for r in 0..p {
                let vrp = v[r][bp];
                let vrq = v[r][bq];
                v[r][bp] = c * vrp - s * vrq;
                v[r][bq] = s * vrp + c * vrq;
            }
        }
        let mut ord: Vec<usize> = (0..p).collect();
        ord.sort_by(|&a, &b| cov[b][b].partial_cmp(&cov[a][a]).unwrap());
        let tot_var: f64 = (0..p).map(|i| cov[i][i].max(0.0)).sum();
        let top4_var: f64 = ord.iter().take(Z_N).map(|&i| cov[i][i].max(0.0)).sum();
        let vekt = ord
            .iter()
            .take(Z_N)
            .map(|&col| (0..p).map(|r| v[r][col]).collect())
            .collect();
        Pca4 {
            dims,
            mu,
            sx,
            vekt,
            andel_var: top4_var / tot_var.max(1e-300),
        }
    }

    fn proj(&self, s: &[f64]) -> Vec<f64> {
        let u: Vec<f64> = self.dims.iter().enumerate().map(|(k, &d)| (s[d] - self.mu[k]) * self.sx[k]).collect();
        self.vekt
            .iter()
            .map(|ev| ev.iter().zip(&u).map(|(a, b)| a * b).sum())
            .collect()
    }
}

#[derive(Default, Clone)]
struct BastavalSum {
    ratt_de: u64,
    tot_de: u64,
    ratt_intag: u64,
    tot_intag: u64,
}

impl BastavalSum {
    fn lagg(&mut self, o: &BastavalSum) {
        self.ratt_de += o.ratt_de;
        self.tot_de += o.tot_de;
        self.ratt_intag += o.ratt_intag;
        self.tot_intag += o.tot_intag;
    }
    fn de_kvot(&self) -> f64 { self.ratt_de as f64 / self.tot_de.max(1) as f64 }
    fn intag_kvot(&self) -> f64 { self.ratt_intag as f64 / self.tot_intag.max(1) as f64 }
}

const PROBE_GRUPPER: [(SinnesGrupp, &str); 4] = [
    (SinnesGrupp::VaxtStannar, "Växt stannar"),
    (SinnesGrupp::VaxtAlla, "Växt alla 9"),
    (SinnesGrupp::AntalAlla, "Antal djur alla 9"),
    (SinnesGrupp::Energi, "Energi (s18)"),
];

struct Steg0Varld {
    fro: u64,
    individer: usize,
    aterv_max_fel: f64,
    aterv_median_fel: f64,
    /// [skuggform 0..5][form ridge/drag]
    m1: Vec<Vec<Summa>>,
    m2: Vec<Vec<Summa>>,
    de: Vec<Vec<Summa>>,
    intag: Vec<Vec<Summa>>,
    /// [skuggform][probe_grupp 0..4]: förutsagd z' (av M2 ridge) respektive verklig z'
    probe_m2: Vec<Vec<Summa>>,
    probe_verklig: Vec<Vec<Summa>>,
    /// [pca_variant 0..3][form ridge/drag]
    pca_m2: Vec<Vec<Summa>>,
    pca_de: Vec<Vec<Summa>>,
    pca_intag: Vec<Vec<Summa>>,
    pca_val: Vec<Vec<BastavalSum>>,
    pca_var: Vec<Vec<f64>>,
}

fn steg0_varld(fil: &Path) -> Result<Steg0Varld, String> {
    let (fro, prov) = las_rader(fil)?;
    let mut per: BTreeMap<u64, Vec<&Prov>> = BTreeMap::new();
    for p in &prov {
        per.entry(p.uid).or_default().push(p);
    }
    let former = [Form::Ridge, Form::Drag];
    let mut v = Steg0Varld {
        fro,
        individer: 0,
        aterv_max_fel: 0.0,
        aterv_median_fel: 0.0,
        m1: vec![vec![Summa::default(); 2]; SKUGG_NAMN.len()],
        m2: vec![vec![Summa::default(); 2]; SKUGG_NAMN.len()],
        de: vec![vec![Summa::default(); 2]; SKUGG_NAMN.len()],
        intag: vec![vec![Summa::default(); 2]; SKUGG_NAMN.len()],
        probe_m2: vec![vec![Summa::default(); PROBE_GRUPPER.len()]; SKUGG_NAMN.len()],
        probe_verklig: vec![vec![Summa::default(); PROBE_GRUPPER.len()]; SKUGG_NAMN.len()],
        pca_m2: vec![vec![Summa::default(); 2]; 3],
        pca_de: vec![vec![Summa::default(); 2]; 3],
        pca_intag: vec![vec![Summa::default(); 2]; 3],
        pca_val: vec![vec![BastavalSum::default(); 2]; 3],
        pca_var: vec![Vec::new(); 3],
    };
    let mut aterv_fel = Vec::new();

    for pv in per.values() {
        if pv.len() < MIN_STEG {
            continue;
        }
        let mut pvs = pv.clone();
        pvs.sort_by_key(|p| p.t);
        let tr_orig: Vec<&Prov> = pvs.iter().filter(|p| !p.prov).copied().collect();
        let te_orig: Vec<&Prov> = pvs.iter().filter(|p| p.prov).copied().collect();
        if tr_orig.len() < 20 || te_orig.is_empty() {
            continue;
        }
        v.individer += 1;
        let (w_k, f_ind) = atervinn_kod(&pvs)?;
        v.aterv_max_fel = v.aterv_max_fel.max(f_ind);
        aterv_fel.push(f_ind);

        // 1. De 5 skuggformerna genom samma fasta kodare W_k.
        for (si, &(sform, _)) in SKUGG_NAMN.iter().enumerate() {
            let sk = skugga_individ(&pvs, &w_k, sform);
            let tr_s: Vec<&Prov> = sk.iter().filter(|p| !p.prov).collect();
            let te_s: Vec<&Prov> = sk.iter().filter(|p| p.prov).collect();
            let blk = |p: &Prov| (p.t / BLOCK) as usize;
            let (xs, yz, ds) = traning(&tr_s, |p| p.s.clone(), |p, a| p.zn[a].clone(), blk);
            let (xz, _, dz) = traning(&tr_s, |p| p.z.clone(), |p, a| p.zn[a].clone(), blk);
            // ΔE och intag tas alltid från de riktiga okomprimerade loggarna (`orig`).
            let de_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|a| tr_orig.iter().map(|p| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]]).collect())
                .collect();
            let in_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|a| tr_orig.iter().map(|p| vec![p.intag[a]]).collect())
                .collect();

            let mut m2_ridge_opt = None;
            for (fi, &form) in former.iter().enumerate() {
                let mod_m1 = Modell::anpassa(form, &xs, &yz, &ds, true)?;
                let mod_m2 = Modell::anpassa(form, &xz, &yz, &dz, true)?;
                let mod_de = Modell::anpassa(form, &xz, &de_tr, &dz, true)?;
                let mod_in = Modell::anpassa(form, &xz, &in_tr, &dz, true)?;
                for (pt_sk, &pt_orig) in te_s.iter().zip(&te_orig) {
                    let g1: Vec<Vec<f64>> = (0..A_N).map(|a| mod_m1.gissa(&pt_sk.s, a)).collect();
                    let g2: Vec<Vec<f64>> = (0..A_N).map(|a| mod_m2.gissa(&pt_sk.z, a)).collect();
                    v.m1[si][fi].lagg(&bidrag(&g1, &pt_sk.zn, &pt_sk.z));
                    v.m2[si][fi].lagg(&bidrag(&g2, &pt_sk.zn, &pt_sk.z));
                    let gde: Vec<Vec<f64>> = (0..A_N).map(|a| mod_de.gissa(&pt_sk.z, a)).collect();
                    let ude: Vec<Vec<f64>> = (0..A_N).map(|a| vec![pt_orig.s2[a][S_N - 1] - pt_orig.s[S_N - 1]]).collect();
                    v.de[si][fi].lagg(&bidrag(&gde, &ude, &[0.0]));
                    let gin: Vec<Vec<f64>> = (0..A_N).map(|a| mod_in.gissa(&pt_sk.z, a)).collect();
                    let uin: Vec<Vec<f64>> = (0..A_N).map(|a| vec![pt_orig.intag[a]]).collect();
                    v.intag[si][fi].lagg(&bidrag(&gin, &uin, &[0.0]));
                }
                if fi == 0 {
                    m2_ridge_opt = Some(mod_m2);
                }
            }
            // Externa linjära prober från z'_skugg till verkliga råa sinnen.
            let mut px = Vec::with_capacity(tr_s.len() * A_N);
            let mut py = Vec::with_capacity(tr_s.len() * A_N);
            let mut pd = Vec::with_capacity(tr_s.len() * A_N);
            for (pt_sk, &pt_orig) in tr_s.iter().zip(&tr_orig) {
                for a in 0..A_N {
                    px.push(pt_sk.zn[a].clone());
                    py.push(pt_orig.s2[a].clone());
                    pd.push((pt_sk.t / BLOCK) as usize);
                }
            }
            let probe = Modell::anpassa(Form::Ridge, &[px], &[py], &[pd], false)?;
            let mod_m2 = m2_ridge_opt.unwrap();
            for (pt_sk, &pt_orig) in te_s.iter().zip(&te_orig) {
                let g_m2_s: Vec<Vec<f64>> = (0..A_N).map(|a| probe.gissa(&mod_m2.gissa(&pt_sk.z, a), 0)).collect();
                let g_vz_s: Vec<Vec<f64>> = (0..A_N).map(|a| probe.gissa(&pt_sk.zn[a], 0)).collect();
                for (gi, &(grp, _)) in PROBE_GRUPPER.iter().enumerate() {
                    v.probe_m2[si][gi].lagg(&bidrag_grupp(&g_m2_s, &pt_orig.s2, &pt_orig.s, grp));
                    v.probe_verklig[si][gi].lagg(&bidrag_grupp(&g_vz_s, &pt_orig.s2, &pt_orig.s, grp));
                }
            }
        }

        // 2. 4-talstaket (PCA-4 på träningsblocken av rå s).
        let alla_dims: Vec<usize> = (0..S_N).collect();
        let vaxt_energi_dims: Vec<usize> = (0..9).map(|k| k * 2).chain(std::iter::once(S_N - 1)).collect();
        let pcas = [
            Pca4::anpassa(&tr_orig, alla_dims.clone(), true),
            Pca4::anpassa(&tr_orig, alla_dims, false),
            Pca4::anpassa(&tr_orig, vaxt_energi_dims, true),
        ];
        for (pi, pca) in pcas.iter().enumerate() {
            v.pca_var[pi].push(pca.andel_var);
            let xp_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|_| tr_orig.iter().map(|p| pca.proj(&p.s)).collect())
                .collect();
            let yp_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|a| tr_orig.iter().map(|p| pca.proj(&p.s2[a])).collect())
                .collect();
            let de_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|a| tr_orig.iter().map(|p| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]]).collect())
                .collect();
            let in_tr: Vec<Vec<Vec<f64>>> = (0..A_N)
                .map(|a| tr_orig.iter().map(|p| vec![p.intag[a]]).collect())
                .collect();
            let d_tr: Vec<Vec<usize>> = (0..A_N)
                .map(|_| tr_orig.iter().map(|p| (p.t / BLOCK) as usize).collect())
                .collect();
            for (fi, &form) in former.iter().enumerate() {
                let mp = Modell::anpassa(form, &xp_tr, &yp_tr, &d_tr, true)?;
                let mde = Modell::anpassa(form, &xp_tr, &de_tr, &d_tr, true)?;
                let min = Modell::anpassa(form, &xp_tr, &in_tr, &d_tr, true)?;
                for pt in &te_orig {
                    let p0 = pca.proj(&pt.s);
                    let p2: Vec<Vec<f64>> = (0..A_N).map(|a| pca.proj(&pt.s2[a])).collect();
                    let gp: Vec<Vec<f64>> = (0..A_N).map(|a| mp.gissa(&p0, a)).collect();
                    v.pca_m2[pi][fi].lagg(&bidrag(&gp, &p2, &p0));

                    let gde: Vec<f64> = (0..A_N).map(|a| mde.gissa(&p0, a)[0]).collect();
                    let ude_v: Vec<f64> = (0..A_N).map(|a| pt.s2[a][S_N - 1] - pt.s[S_N - 1]).collect();
                    let gde_v: Vec<Vec<f64>> = gde.iter().map(|&x| vec![x]).collect();
                    let ude_vv: Vec<Vec<f64>> = ude_v.iter().map(|&x| vec![x]).collect();
                    v.pca_de[pi][fi].lagg(&bidrag(&gde_v, &ude_vv, &[0.0]));
                    let (lo_d, hi_d) = ude_v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
                    if hi_d - lo_d >= 1e-12 {
                        v.pca_val[pi][fi].tot_de += 1;
                        if ude_v[argmax(&gde)] >= hi_d - 1e-12 {
                            v.pca_val[pi][fi].ratt_de += 1;
                        }
                    }

                    let gin: Vec<f64> = (0..A_N).map(|a| min.gissa(&p0, a)[0]).collect();
                    let gin_v: Vec<Vec<f64>> = gin.iter().map(|&x| vec![x]).collect();
                    let uin_vv: Vec<Vec<f64>> = pt.intag.iter().map(|&x| vec![x]).collect();
                    v.pca_intag[pi][fi].lagg(&bidrag(&gin_v, &uin_vv, &[0.0]));
                    let (lo_i, hi_i) = pt.intag.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
                    if hi_i - lo_i >= 1e-12 {
                        v.pca_val[pi][fi].tot_intag += 1;
                        if pt.intag[argmax(&gin)] >= hi_i - 1e-12 {
                            v.pca_val[pi][fi].ratt_intag += 1;
                        }
                    }
                }
            }
        }
    }
    v.aterv_median_fel = median(aterv_fel);
    Ok(v)
}

fn steg0(mapp: &Path) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() {
        return Err("inga radfiler".into());
    }
    let mut svar = Vec::with_capacity(filer.len());
    for del in filer.chunks(4) {
        let res: Vec<Result<Steg0Varld, String>> = std::thread::scope(|s| {
            let h: Vec<_> = del.iter().map(|f| s.spawn(move || {
                let v = steg0_varld(f)?;
                eprintln!("steg0 frö {}: {} individer, kodarfel max {:.2e}", v.fro, v.individer, v.aterv_max_fel);
                Ok(v)
            })).collect();
            h.into_iter().map(|x| x.join().unwrap_or_else(|_| Err("tråd föll".into()))).collect()
        });
        for r in res {
            svar.push(r?);
        }
    }

    let n = svar.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2005 Steg 0 (skuggmätning utan ändring i `hjarna.rs`): EXTERN AVLÄSNING\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/hjarna-utvardering-2010.md`. {n} världar; arbetsgräns grönt: `M1 ≥ 0,10`, `M2 ≥ 0,10` och `z+a→ΔE > 0` i minst {krav} av {n} världar.\n");

    let med_rec = median(svar.iter().map(|v| v.aterv_median_fel).collect());
    let max_rec = svar.iter().map(|v| v.aterv_max_fel).fold(0.0f64, f64::max);
    let _ = writeln!(t, "## 1. Kodaråtervinning (`W_k` ur loggade rader)\n");
    let _ = writeln!(t, "- Återvinningsfel `|tanh(W_k · [s, 1]) − z|`: median **{med_rec:.3e}**, högst över alla individer och världar **{max_rec:.3e}**.\n");

    let _ = writeln!(t, "## 2. Skuggingångar genom samma fasta kodare `W_k` (median R² över {n} världar; ridge / slumpdrag)\n");
    let _ = writeln!(t, "| Skuggingång | Form | M1 R² | M2 R² | z+a→ΔE R² | z+a→intag R² | M2 ≥ 0,10 & ΔE > 0 |\n|---|---|---|---|---|---|---|");
    for (si, &(_, namn)) in SKUGG_NAMN.iter().enumerate() {
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let m1r: Vec<f64> = svar.iter().map(|v| v.m1[si][fi].r2()).collect();
            let m2r: Vec<f64> = svar.iter().map(|v| v.m2[si][fi].r2()).collect();
            let der: Vec<f64> = svar.iter().map(|v| v.de[si][fi].r2()).collect();
            let inr: Vec<f64> = svar.iter().map(|v| v.intag[si][fi].r2()).collect();
            let pass = (0..n).filter(|&w| m1r[w] >= 0.10 && m2r[w] >= 0.10 && der[w] > 0.0).count();
            let _ = writeln!(t, "| {namn} | {form} | {} | {} | {} | {} | {pass} av {n} |", f3(median(m1r)), f3(median(m2r)), f3(median(der)), f3(median(inr)));
        }
    }

    let _ = writeln!(t, "\n## 3. Externa linjära prober från förutsagd `z'` (M2) respektive verklig `z'` till råa sinnen (median R²)\n");
    let _ = writeln!(t, "| Skuggingång | Källa till z' | Växt stannar | Växt alla 9 | Antal djur alla 9 | Energi (s18) |\n|---|---|---|---|---|---|");
    for (si, &(_, namn)) in SKUGG_NAMN.iter().enumerate() {
        for (knamn, kalla) in [("M2-förutsagd z'", &svar.iter().map(|v| &v.probe_m2[si]).collect::<Vec<_>>()), ("Verklig z'", &svar.iter().map(|v| &v.probe_verklig[si]).collect::<Vec<_>>())] {
            let vals: Vec<String> = (0..PROBE_GRUPPER.len())
                .map(|gi| f3(median(kalla.iter().map(|p| p[gi].r2()).collect())))
                .collect();
            let _ = writeln!(t, "| {namn} | {knamn} | {} |", vals.join(" | "));
        }
    }

    let _ = writeln!(t, "\n## 4. 4-talstaket (`PCA-4` på träningsblocken av `s`; slumpen bästa val = 0,111)\n");
    let _ = writeln!(t, "| PCA-variant (4 tal) | Förkl. varians | Form | M2 R² | p+a→ΔE R² | p+a→intag R² | Bästa val ΔE | Bästa val intag |\n|---|---|---|---|---|---|---|---|");
    let pca_namn = ["(a) Standardiserad s (19 sinnen)", "(b) Oskalad s (19 sinnen)", "(c) Standardiserad växt+energi (10 sinnen)"];
    for (pi, pnamn) in pca_namn.iter().enumerate() {
        let var_med = median(svar.iter().flat_map(|v| v.pca_var[pi].iter().copied()).collect());
        for (fi, form) in ["ridge", "slumpdrag"].iter().enumerate() {
            let m2r = median(svar.iter().map(|v| v.pca_m2[pi][fi].r2()).collect());
            let der = median(svar.iter().map(|v| v.pca_de[pi][fi].r2()).collect());
            let inr = median(svar.iter().map(|v| v.pca_intag[pi][fi].r2()).collect());
            let bv_de = median(svar.iter().map(|v| v.pca_val[pi][fi].de_kvot()).collect());
            let bv_in = median(svar.iter().map(|v| v.pca_val[pi][fi].intag_kvot()).collect());
            let _ = writeln!(t, "| {pnamn} | {:.1} % | {form} | {} | {} | {} | {} | {} |", 100.0 * var_med, f3(m2r), f3(der), f3(inr), f3(bv_de), f3(bv_in));
        }
    }

    let pass_arm = |si: usize| -> usize {
        (0..n)
            .filter(|&w| {
                let m1b = svar[w].m1[si][0].r2().max(svar[w].m1[si][1].r2());
                let m2b = svar[w].m2[si][0].r2().max(svar[w].m2[si][1].r2());
                let deb = svar[w].de[si][0].r2().max(svar[w].de[si][1].r2());
                m1b >= 0.10 && m2b >= 0.10 && deb > 0.0
            })
            .count()
    };
    let ema_pass = pass_arm(1);
    let wel_pass = pass_arm(2);
    let str_ra_pass = pass_arm(3);
    let str_ema_pass = pass_arm(4);
    let dom = if ema_pass >= krav || wel_pass >= krav {
        "GRÖNT FÖR Z-SKALNING MED FRYST KODARE: löpande z-poäng når gränsen; nästa steg kan pröva skalning + Alt 2007 utan att träna kodaren."
    } else if str_ra_pass >= krav || str_ema_pass >= krav {
        "PEKAR MOT ALT 2008 (TRÄNAD KODARE): z-skalning med fryst kodare räcker inte när 9 djurantal ligger kvar, medan strukna antal når grönt."
    } else {
        "RÖTT I BÅDA: varken z-skalning eller strukna djurantal når gränsen genom fryst kodare; ny analys krävs före kodändring i hjärnan."
    };
    let _ = writeln!(t, "\n## 5. Dom enligt `hjarna-utvardering-2010.md`\n");
    let _ = writeln!(t, "- Antal världar som klarar `M1 ≥ 0,10`, `M2 ≥ 0,10`, `z+a→ΔE > 0` (krav {krav} av {n}): EMA **{ema_pass}**, Welford **{wel_pass}**, Strukna antal rå **{str_ra_pass}**, Strukna antal + EMA **{str_ema_pass}**.");
    let _ = writeln!(t, "- **Dom:** {dom} (EXTERN AVLÄSNING; `intag` `R²` är formelbundet och ingår inte i gränsen.)");
    Ok(t)
}

fn main() {
    let arg: Vec<String> = std::env::args().skip(1).collect();
    let v = |n: &str| arg.iter().position(|a| a == n).and_then(|i| arg.get(i + 1)).cloned();
    match arg.first().map(|s| s.as_str()) {
        Some("kor") => {
            let fron = v("--fron").expect("--fron A-B");
            let (a, b) = fron.split_once('-').expect("--fron A-B");
            let (a, b): (u64, u64) = (a.parse().expect("frö"), b.parse().expect("frö"));
            let ar: i64 = v("--ar").expect("--ar").parse().expect("--ar");
            let n_djur: usize = v("--djur").expect("--djur").parse().expect("--djur");
            let steg: i64 = v("--steg").expect("--steg").parse().expect("--steg");
            let avst: usize = v("--avstand").expect("--avstand").parse().expect("--avstand");
            let tradar: usize = v("--tradar").map(|x| x.parse().expect("--tradar")).unwrap_or(1);
            let ut = PathBuf::from(v("--ut").expect("--ut MAPP"));
            std::fs::create_dir_all(&ut).expect("skapa mappen");
            let rot = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let fron: Vec<u64> = (a..=b).collect();
            let mut fel = false;
            for del in fron.chunks(tradar.max(1)) {
                let r: Vec<Result<String, String>> = std::thread::scope(|s| {
                    let h: Vec<_> = del.iter().map(|&f| { let (rot, ut) = (&rot, &ut); s.spawn(move || kor(rot, f, ar, n_djur, steg, avst, ut)) }).collect();
                    h.into_iter().map(|x| x.join().unwrap_or_else(|_| Err("tråden föll".into()))).collect()
                });
                for x in r {
                    match x {
                        Ok(t) => println!("{t}"),
                        Err(e) => {
                            eprintln!("FEL: {e}");
                            fel = true;
                        }
                    }
                }
            }
            logg(&ut.join("korning.logg"), if fel { "SLUT MED FEL" } else { "SLUT" });
            std::process::exit(if fel { 1 } else { 0 });
        }
        Some("rapport") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match rapport(&inn) {
                Ok(t) => {
                    std::fs::write(inn.join("vminfo-resultat.md"), &t).expect("skriv");
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("diagnos") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match diagnos(&inn) {
                Ok(t) => {
                    std::fs::write(inn.join("diagnos-resultat.md"), &t).expect("skriv");
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("tillagg4") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match tillagg4(&inn) {
                Ok(t) => {
                    std::fs::write(inn.join("tillagg4-resultat.md"), &t).expect("skriv");
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("steg0") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match steg0(&inn) {
                Ok(t) => {
                    std::fs::write(inn.join("steg0-resultat.md"), &t).expect("skriv");
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("fas1extra") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match fas1extra(&inn) {
                Ok(t) => println!("{t}"),
                Err(e) => {
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("användning: vminfo kor|rapport|fas1extra (se början av filen)");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod prov {
    use super::*;

    /// Ridge återfinner en känd linjär funktion (λ litet mot n).
    #[test]
    fn ridge_aterfinner_linjar() {
        let mut g = Rng::new(5);
        let x: Vec<Vec<f64>> = (0..500).map(|_| (0..3).map(|_| g.f() * 2.0 - 1.0).collect()).collect();
        let y: Vec<Vec<f64>> = x.iter().map(|r| vec![1.0 + 2.0 * r[0] - 3.0 * r[2], r[1]]).collect();
        let del: Vec<usize> = (0..500).map(|i| i % 5).collect();
        let (m, lam) = Ridge::anpassa(&x, &y, &del).unwrap();
        assert_eq!(lam, LAMBDOR[0], "utan brus ska den minsta dämpningen väljas");
        let gi = m.gissa(&[0.5, -0.2, 0.1]);
        assert!((gi[0] - (1.0 + 1.0 - 0.3)).abs() < 1e-3, "{gi:?}");
        assert!((gi[1] + 0.2).abs() < 1e-3);
    }

    /// Handlingens verkan: rätt gissning ger R² 1, samma gissning för alla handlingar ger 0, motsatt ger under 0.
    #[test]
    fn r2_for_handlingens_verkan() {
        let utf: Vec<Vec<f64>> = (0..A_N).map(|a| vec![a as f64, 1.0]).collect();
        let fore = vec![0.0, 0.0];
        assert!((bidrag(&utf, &utf, &fore).r2() - 1.0).abs() < 1e-12);
        let lika: Vec<Vec<f64>> = (0..A_N).map(|_| vec![3.0, 1.0]).collect();
        assert!(bidrag(&lika, &utf, &fore).r2().abs() < 1e-12);
        let mot: Vec<Vec<f64>> = (0..A_N).map(|a| vec![-(a as f64), 1.0]).collect();
        assert!(bidrag(&mot, &utf, &fore).r2() < 0.0);
    }

    fn prov_med(z: Vec<f64>, zn: Vec<Vec<f64>>, t: i64) -> Prov {
        let s = vec![0.0; S_N];
        Prov { uid: 1, t, prov: ar_prov(t), s: s.clone(), z, s2: vec![s; A_N], zn, y: vec![vec![0.0; Z_N]; A_N], v_pred: vec![0.0; A_N], v_sann: vec![0.0; A_N], intag: vec![0.0; A_N] }
    }

    /// A: när varje handling flyttar den inre bilden åt sitt håll känns den igen; när efterbilden är brus ligger träffen nära 1/9 (rött prov).
    #[test]
    fn invers_modell_ser_handlingen_och_brus_ger_slump() {
        let mut g = Rng::new(3);
        let mut ratt = Vec::new();
        let mut brus = Vec::new();
        for t in 0..300 {
            let z: Vec<f64> = (0..Z_N).map(|_| g.f() - 0.5).collect();
            let zn: Vec<Vec<f64>> = (0..A_N).map(|a| (0..Z_N).map(|i| z[i] + 0.3 * [(a % 3) as f64, (a / 3) as f64, 0.0, 0.0][i] + 0.01 * g.f()).collect()).collect();
            let zb: Vec<Vec<f64>> = (0..A_N).map(|_| (0..Z_N).map(|_| g.f()).collect()).collect();
            ratt.push(prov_med(z.clone(), zn, t));
            brus.push(prov_med(z, zb, t));
        }
        let traff = |v: &Vec<Prov>| {
            let tr: Vec<&Prov> = v.iter().filter(|p| !p.prov).collect();
            let te: Vec<&Prov> = v.iter().filter(|p| p.prov).collect();
            let m = idm_individ(&tr, &te, Form::Drag, false).unwrap();
            (0..A_N).map(|k| m[k][k]).sum::<u64>() as f64 / m.iter().flatten().sum::<u64>() as f64
        };
        let (r, b) = (traff(&ratt), traff(&brus));
        assert!(r > 0.9, "med information: {r}");
        assert!(b < 0.2, "brus: {b}");
    }

    /// Intervallet: lika värden ger ett intervall utan bredd; spridda värden ger ett intervall som innehåller medlet.
    #[test]
    fn intervall_over_varldar() {
        let (m, lo, hi) = intervall(&[0.5; 20]);
        assert!((m - 0.5).abs() < 1e-12 && (lo - 0.5).abs() < 1e-12 && (hi - 0.5).abs() < 1e-12);
        let v: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let (m, lo, hi) = intervall(&v);
        assert!(lo < m && m < hi && lo > 5.0 && hi < 14.0, "{lo} {m} {hi}");
    }

    /// Rött prov: en modell som får informationen (sinnena) ska slå en som bara får brus, på påhittade data där utfallet beror på handling × sinne.
    #[test]
    fn modellen_ser_information_och_brus_far_ingen() {
        let mut g = Rng::new(9);
        let n = 300;
        // Medel 0: handlingens verkan bärs bara av sinnena (ingen fast skillnad mellan handlingarna att gissa utan information).
        let s: Vec<Vec<f64>> = (0..n).map(|_| (0..4).map(|_| g.f() - 0.5).collect()).collect();
        let brus: Vec<Vec<f64>> = (0..n).map(|_| (0..4).map(|_| g.f()).collect()).collect();
        let mal = |r: &Vec<f64>, a: usize| vec![r[a % 4] * (a as f64 - 4.0)];
        let mk = |inn: &Vec<Vec<f64>>| -> (Vec<Vec<Vec<f64>>>, Vec<Vec<Vec<f64>>>) {
            let x = (0..A_N).map(|_| inn[..200].to_vec()).collect();
            let y = (0..A_N).map(|a| s[..200].iter().map(|r| mal(r, a)).collect()).collect();
            (x, y)
        };
        let r2 = |inn: &Vec<Vec<f64>>, form: Form| {
            let (x, y) = mk(inn);
            let d: Vec<Vec<usize>> = (0..A_N).map(|_| (0..200).map(|i| i / 40).collect()).collect();
            let m = Modell::anpassa(form, &x, &y, &d, true).unwrap();
            let mut su = Summa::default();
            for i in 200..n {
                let gs: Vec<Vec<f64>> = (0..A_N).map(|a| m.gissa(&inn[i], a)).collect();
                let ut: Vec<Vec<f64>> = (0..A_N).map(|a| mal(&s[i], a)).collect();
                su.lagg(&bidrag(&gs, &ut, &[0.0]));
            }
            su.r2()
        };
        for form in [Form::Ridge, Form::Drag] {
            let med = r2(&s, form);
            let utan = r2(&brus, form);
            assert!(med > 0.5, "{form:?} med information: {med}");
            assert!(utan < 0.1, "{form:?} med brus: {utan}");
        }
    }

    #[test]
    fn tillagg4_blandade_handlingar_ger_noll() {
        let mut g = Rng::new(41);
        let n = 400;
        let mut tr = Vec::new();
        let mut te = Vec::new();
        for t in 0..n {
            let s: Vec<f64> = (0..S_N).map(|_| g.f()).collect();
            let mut perm: Vec<usize> = (0..A_N).collect();
            g.blanda(&mut perm);
            let mut s2 = vec![vec![0.0; S_N]; A_N];
            for a in 0..A_N {
                // Handlingens signal förstörs genom slumpmässig permutation per prov.
                let pa = perm[a];
                s2[a] = s.clone();
                s2[a][0] = (pa as f64) * 0.1;
            }
            let p = Prov {
                uid: if t < n / 2 { 1 } else { 2 },
                t: t as i64,
                prov: t % 3 == 2,
                s,
                z: vec![0.0; Z_N],
                s2,
                zn: vec![vec![0.0; Z_N]; A_N],
                y: vec![vec![0.0; Z_N]; A_N],
                v_pred: vec![0.0; A_N],
                v_sann: vec![0.0; A_N],
                intag: vec![0.0; A_N],
            };
            if t < n / 2 { tr.push(p); } else { te.push(p); }
        }
        let tr_r: Vec<&Prov> = tr.iter().collect();
        let d_tr: Vec<usize> = tr.iter().map(|p| (p.t as usize / 20) % 5).collect();
        let x: Vec<Vec<Vec<f64>>> = (0..A_N).map(|_| tr_r.iter().map(|p| p.s.clone()).collect()).collect();
        let y: Vec<Vec<Vec<f64>>> = (0..A_N).map(|a| tr_r.iter().map(|p| p.s2[a].clone()).collect()).collect();
        let del: Vec<Vec<usize>> = (0..A_N).map(|_| d_tr.clone()).collect();
        let m = Modell::anpassa(Form::Ridge, &x, &y, &del, true).unwrap();
        let mut sum = Summa::default();
        for p in &te {
            let gs: Vec<Vec<f64>> = (0..A_N).map(|a| m.gissa(&p.s, a)).collect();
            sum.lagg(&bidrag_grupp(&gs, &p.s2, &p.s, SinnesGrupp::Alla));
        }
        assert!(sum.r2() <= 0.01, "blandade handlingar gav oväntat hög R²: {}", sum.r2());
    }

    #[test]
    fn tillagg4_planterad_forskjutning_ger_hog_traff() {
        let mut g = Rng::new(73);
        let mut ratt = 0usize;
        let mut brus_ratt = 0usize;
        let tot = 200 * A_N;
        // Skapa 5×5 världslapp för att garantera att varje 3×3-fönster och flytt får äkta skiftade värden.
        for _ in 0..200 {
            let mut karta_v = [[0.0; 5]; 5];
            for i in 0..5 {
                for j in 0..5 {
                    karta_v[i][j] = g.f();
                }
            }
            let mut s = vec![0.0; S_N];
            for (k, &(di, dj)) in GRANNAR.iter().enumerate() {
                s[k * 2] = karta_v[(2 + di) as usize][(2 + dj) as usize];
            }
            for a in 0..A_N {
                let (mi, mj) = GRANNAR[a];
                let mut s2 = vec![0.0; S_N];
                for (k, &(gi, gj)) in GRANNAR.iter().enumerate() {
                    s2[k * 2] = karta_v[(2 + mi + gi) as usize][(2 + mj + gj) as usize];
                }
                let (dv, _) = forskjutna_avstand(&s, &s2);
                if argmin(&dv) == a {
                    ratt += 1;
                }
                let brus: Vec<f64> = (0..S_N).map(|_| g.f()).collect();
                let (dvb, _) = forskjutna_avstand(&s, &brus);
                if argmin(&dvb) == a {
                    brus_ratt += 1;
                }
            }
        }
        let traff = ratt as f64 / tot as f64;
        let brus_traff = brus_ratt as f64 / tot as f64;
        assert!(traff >= 0.90, "planterad förskjutning gav för låg träff: {traff}");
        assert!(brus_traff < 0.20, "ren slumpbild gav för hög träff: {brus_traff}");
    }

    #[test]
    fn tillagg4_delgrupper_summerar_till_total() {
        let mut g = Rng::new(19);
        let fore: Vec<f64> = (0..S_N).map(|_| g.f()).collect();
        let utf: Vec<Vec<f64>> = (0..A_N).map(|_| (0..S_N).map(|_| g.f()).collect()).collect();
        let gissn: Vec<Vec<f64>> = (0..A_N).map(|_| (0..S_N).map(|_| g.f()).collect()).collect();
        let tot = bidrag_grupp(&gissn, &utf, &fore, SinnesGrupp::Alla);
        let mut del_verklig = 0.0;
        let mut del_rest = 0.0;
        for grp in [SinnesGrupp::VaxtStannar, SinnesGrupp::VaxtNy, SinnesGrupp::AntalStannar, SinnesGrupp::AntalNy, SinnesGrupp::Energi] {
            let s = bidrag_grupp(&gissn, &utf, &fore, grp);
            del_verklig += s.verklig;
            del_rest += s.rest;
        }
        assert!((del_verklig - tot.verklig).abs() < 1e-9, "{del_verklig} != {}", tot.verklig);
        assert!((del_rest - tot.rest).abs() < 1e-9, "{del_rest} != {}", tot.rest);
    }

    #[test]
    fn steg0_atervinner_kodare_exakt_och_blandade_ger_fel() {
        let mut g = Rng::new(101);
        let mut w_sann = [[0.0; S_N + 1]; Z_N];
        for i in 0..Z_N {
            for j in 0..=S_N {
                w_sann[i][j] = (g.f() - 0.5) * 0.4;
            }
        }
        let mut prov = Vec::new();
        for t in 0..200i64 {
            let s: Vec<f64> = (0..S_N).map(|_| g.f() * 2.0 - 1.0).collect();
            let z = koda_med(&w_sann, &s);
            let s2: Vec<Vec<f64>> = (0..A_N).map(|_| (0..S_N).map(|_| g.f() * 2.0 - 1.0).collect()).collect();
            let zn: Vec<Vec<f64>> = s2.iter().map(|sv| koda_med(&w_sann, sv)).collect();
            prov.push(Prov {
                uid: 1, t, prov: ar_prov(t), s, z, s2, zn,
                y: vec![vec![0.0; Z_N]; A_N], v_pred: vec![0.0; A_N], v_sann: vec![0.0; A_N], intag: vec![0.0; A_N],
            });
        }
        let pv: Vec<&Prov> = prov.iter().collect();
        let (_, max_fel) = atervinn_kod(&pv).unwrap();
        assert!(max_fel < 1e-9, "kodaråtervinningen gav för stort fel: {max_fel}");

        // Rödprov: blanda z mellan tidssteg -> återvinningen måste ge stort fel.
        for t in 0..200 {
            let n = (t + 77) % 200;
            prov[t].z = koda_med(&w_sann, &prov[n].s);
        }
        let pvb: Vec<&Prov> = prov.iter().collect();
        let (_, fel_bl) = atervinn_kod(&pvb).unwrap();
        assert!(fel_bl > 0.1, "omkastade z gav oväntat litet återvinningsfel: {fel_bl}");
    }

    #[test]
    fn steg0_omkastade_handlingar_ger_noll_och_slumpval() {
        let mut g = Rng::new(202);
        let mut w = [[0.0; S_N + 1]; Z_N];
        for i in 0..Z_N { w[i][i] = 0.8; }
        let mut prov = Vec::new();
        for t in 0..300i64 {
            let s: Vec<f64> = (0..S_N).map(|_| g.f()).collect();
            let z = koda_med(&w, &s);
            let mut perm: Vec<usize> = (0..A_N).collect();
            g.blanda(&mut perm);
            let mut s2 = Vec::with_capacity(A_N);
            let mut intag = vec![0.0; A_N];
            for a in 0..A_N {
                let pa = perm[a];
                let mut sv = s.clone();
                sv[S_N - 1] = s[S_N - 1] + (pa as f64) * 0.05;
                intag[a] = pa as f64;
                s2.push(sv);
            }
            let zn: Vec<Vec<f64>> = s2.iter().map(|sv| koda_med(&w, sv)).collect();
            prov.push(Prov {
                uid: 1, t, prov: ar_prov(t), s, z, s2, zn,
                y: vec![vec![0.0; Z_N]; A_N], v_pred: vec![0.0; A_N], v_sann: vec![0.0; A_N], intag,
            });
        }
        let tr: Vec<&Prov> = prov.iter().filter(|p| !p.prov).collect();
        let te: Vec<&Prov> = prov.iter().filter(|p| p.prov).collect();
        let pca = Pca4::anpassa(&tr, (0..S_N).collect(), true);
        let xp: Vec<Vec<Vec<f64>>> = (0..A_N).map(|_| tr.iter().map(|p| pca.proj(&p.s)).collect()).collect();
        let yp: Vec<Vec<Vec<f64>>> = (0..A_N).map(|a| tr.iter().map(|p| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]]).collect()).collect();
        let dp: Vec<Vec<usize>> = (0..A_N).map(|_| tr.iter().map(|p| (p.t / BLOCK) as usize).collect()).collect();
        let mde = Modell::anpassa(Form::Ridge, &xp, &yp, &dp, true).unwrap();
        let (mut sum, mut bv) = (Summa::default(), BastavalSum::default());
        for p in &te {
            let p0 = pca.proj(&p.s);
            let g_v: Vec<f64> = (0..A_N).map(|a| mde.gissa(&p0, a)[0]).collect();
            let u_v: Vec<f64> = (0..A_N).map(|a| p.s2[a][S_N - 1] - p.s[S_N - 1]).collect();
            sum.lagg(&bidrag(&g_v.iter().map(|&x| vec![x]).collect::<Vec<_>>(), &u_v.iter().map(|&x| vec![x]).collect::<Vec<_>>(), &[0.0]));
            let hi = u_v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            bv.tot_de += 1;
            if u_v[argmax(&g_v)] >= hi - 1e-12 { bv.ratt_de += 1; }
        }
        assert!(sum.r2() <= 0.01, "omkastade handlingar gav R² {}", sum.r2());
        assert!(bv.de_kvot() < 0.25, "omkastade handlingar gav bästa val {}", bv.de_kvot());
    }

    #[test]
    fn steg0_planterad_4d_signal_i_pca4() {
        let mut g = Rng::new(303);
        let mut prov = Vec::new();
        for t in 0..360i64 {
            // Hela variansen ligger i 4 latenta faktorer upprepade över 19 kanaler + pyttelite brus.
            let lat: Vec<f64> = (0..4).map(|_| g.f() * 2.0 - 1.0).collect();
            let mk_s = |lf: &[f64], gg: &mut Rng| -> Vec<f64> {
                (0..S_N).map(|j| lf[j % 4] + 0.001 * (gg.f() - 0.5)).collect()
            };
            let s = mk_s(&lat, &mut g);
            let mut s2 = Vec::with_capacity(A_N);
            for a in 0..A_N {
                let mut nl = lat.clone();
                // Energiändringen s2[18] - s[18] styrs exakt av handling × faktor 2 (kanal 18 har 18 % 4 = 2).
                nl[2] += ((a as f64) - 4.0) * lat[0] * 0.3;
                s2.push(mk_s(&nl, &mut g));
            }
            prov.push(Prov {
                uid: 1, t, prov: ar_prov(t), s, z: vec![0.0; Z_N], s2, zn: vec![vec![0.0; Z_N]; A_N],
                y: vec![vec![0.0; Z_N]; A_N], v_pred: vec![0.0; A_N], v_sann: vec![0.0; A_N], intag: vec![0.0; A_N],
            });
        }
        let tr: Vec<&Prov> = prov.iter().filter(|p| !p.prov).collect();
        let te: Vec<&Prov> = prov.iter().filter(|p| p.prov).collect();
        let pca = Pca4::anpassa(&tr, (0..S_N).collect(), true);
        let xp: Vec<Vec<Vec<f64>>> = (0..A_N).map(|_| tr.iter().map(|p| pca.proj(&p.s)).collect()).collect();
        let yp: Vec<Vec<Vec<f64>>> = (0..A_N).map(|a| tr.iter().map(|p| vec![p.s2[a][S_N - 1] - p.s[S_N - 1]]).collect()).collect();
        let dp: Vec<Vec<usize>> = (0..A_N).map(|_| tr.iter().map(|p| (p.t / BLOCK) as usize).collect()).collect();
        let mde = Modell::anpassa(Form::Ridge, &xp, &yp, &dp, true).unwrap();
        let (mut sum, mut bv) = (Summa::default(), BastavalSum::default());
        for p in &te {
            let p0 = pca.proj(&p.s);
            let g_v: Vec<f64> = (0..A_N).map(|a| mde.gissa(&p0, a)[0]).collect();
            let u_v: Vec<f64> = (0..A_N).map(|a| p.s2[a][S_N - 1] - p.s[S_N - 1]).collect();
            sum.lagg(&bidrag(&g_v.iter().map(|&x| vec![x]).collect::<Vec<_>>(), &u_v.iter().map(|&x| vec![x]).collect::<Vec<_>>(), &[0.0]));
            let hi = u_v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            bv.tot_de += 1;
            if u_v[argmax(&g_v)] >= hi - 1e-12 { bv.ratt_de += 1; }
        }
        assert!(sum.r2() >= 0.80, "planterad 4D-signal gav bara R² {}", sum.r2());
        assert!(bv.de_kvot() >= 0.80, "planterad 4D-signal gav bästa val {}", bv.de_kvot());
    }
}
