//! prov2008: Spår A (online längs sparade banor i rader-*.tsv) och Spår B (levande simulering) för Alt 2008 under Alt 2010.
//! Loggar i realtid med `date`-klockslag till stderr, `work/hcsim/alt2008/korning.logg` och `[Alt 2008]` i `vminfo-0c11afa/korning.logg`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use hckarta::{Kalla, Tidtabell};
use hcsim::formaga::{utforsk_armar, Formaga, HjarnArm, Minnestabell};
use hcsim::hjarna::{gauss, N_GEN};
use hcsim::hjarna2008::{Hj2008, Hj2008Config};
use hcsim::kartbild::{fast_ur_sim, historik_vag, lage_ur_sim, satt_rattningar_utdod, skriv_bild, BALTE_K};
use hcsim::sim::{KantLage, Sim, GRANNAR};
use hcsim::tal::{A_N, SIGMA0, STEG_PER_AR, S_N, Z_N};
use hcsim::tidsskift::{db_vag, Skrivare};
use hcsim::utforsk::{bygg_sim_balte, B_FULL, H_FULL};
use hcsim::Rng;
use serde::{Deserialize, Serialize};

mod opt_nan {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_some(v)
        } else {
            s.serialize_none()
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
    }
}

const BLOCK: i64 = 30;
const MIN_STEG: usize = 150;
const LAMBDOR: [f64; 7] = [0.01, 0.1, 1.0, 10.0, 100.0, 1000.0, 10000.0];
const LAMBDA_ENSAM: f64 = 1.0;
const SD_GOLV: f64 = 0.05;
const N_DRAG: usize = 100;
const DRAG_FRO: u64 = 1974;

const EGEN_LOGG_2008: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2008/korning.logg";
const EGEN_LOGG_2053: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2053/korning.logg";
const EGEN_LOGG_2056: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2056/korning.logg";
const EGEN_LOGG_2059: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2059/korning.logg";
const EGEN_LOGG_2073: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2073/korning.logg";
const EGEN_LOGG_2116: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2116/korning.logg";
const EGEN_LOGG_2144: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2144/korning.logg";
const EGEN_LOGG_2159: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2159/korning.logg";
const EGEN_LOGG_2172: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2172/korning.logg";
const EGEN_LOGG_2199: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2199/korning.logg";
const EGEN_LOGG_2223: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2223/korning.logg";
const EGEN_LOGG_2238: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2238/korning.logg";
const EGEN_LOGG_2249: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2249/korning.logg";
const EGEN_LOGG_2260: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2260/korning.logg";
const EGEN_LOGG_2267: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2267/korning.logg";
const EGEN_LOGG_2270: &str = "/home/hakan/sandbox/pi/work/hcsim/alt2270/korning.logg";
const SPEGEL_LOGG: &str = "/home/hakan/sandbox/pi/work/hcsim/vminfo-0c11afa/korning.logg";

static LOGG_LAS: Mutex<()> = Mutex::new(());

fn live_logg_med(egen_logg: &str, tagg: &str, rad: &str) {
    let nu = std::process::Command::new("date")
        .arg("+%F %T")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let _g = LOGG_LAS.lock().unwrap();
    if let Some(p) = Path::new(egen_logg).parent() {
        let _ = create_dir_all(p);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(egen_logg) {
        let _ = writeln!(f, "{nu} {rad}");
        let _ = f.flush();
    }
    if Path::new(SPEGEL_LOGG).parent().is_some_and(|p| p.exists()) {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(SPEGEL_LOGG) {
            let _ = writeln!(f, "{nu} [{tagg}] {rad}");
            let _ = f.flush();
        }
    }
    eprintln!("{nu} [{tagg}] {rad}");
}

fn skala_kol(sd: f64) -> f64 {
    if sd > 1e-12 { 1.0 / sd.max(SD_GOLV) } else { 0.0 }
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() { return f64::NAN; }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

fn f3(x: f64) -> String {
    format!("{x:.3}").replace('.', ",")
}

fn argmax(v: &[f64]) -> usize {
    let mut b = 0;
    for i in 1..v.len() {
        if v[i] > v[b] { b = i; }
    }
    b
}

// ---------------------------------------------------------------- läsning och kodaråtervinning

struct RadProv {
    uid: u64,
    t: i64,
    prov: bool,
    s: Vec<f64>,
    z: Vec<f64>,
    s2: Vec<Vec<f64>>,
    zn: Vec<Vec<f64>>,
    intag: Vec<f64>,
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

fn las_rader(fil: &Path) -> Result<(u64, Vec<RadProv>), String> {
    let text = std::fs::read_to_string(fil).map_err(|e| format!("{}: {e}", fil.display()))?;
    let mut rader = text.lines();
    let r = rader.next().ok_or("tom fil")?;
    if r != rubrik() {
        return Err(format!("{}: fel rubrik", fil.display()));
    }
    let mut fro = 0;
    let mut karta: BTreeMap<(u64, i64), RadProv> = BTreeMap::new();
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
        let intag_v = p(7 + 2 * S_N + 3 * Z_N)?;
        let e = karta.entry((uid, t)).or_insert_with(|| RadProv {
            uid, t, prov: k[4] == "1", s: s.clone(), z: z.clone(), s2: vec![], zn: vec![], intag: vec![],
        });
        if e.s2.len() != a {
            return Err(format!("rad {}: handling {a} i fel ordning", nr + 2));
        }
        e.s2.push(s2);
        e.zn.push(zn);
        e.intag.push(intag_v);
    }
    let prov: Vec<RadProv> = karta.into_values().collect();
    if let Some(p) = prov.iter().find(|p| p.s2.len() != A_N) {
        return Err(format!("djur {} har ett prov med {} handlingar", p.uid, p.s2.len()));
    }
    Ok((fro, prov))
}

fn cholesky_los(a: &mut [Vec<f64>], b: &mut [Vec<f64>]) -> Result<(), String> {
    let p = a.len();
    for j in 0..p {
        let mut d = a[j][j];
        for k in 0..j { d -= a[j][k] * a[j][k]; }
        if d <= 0.0 { return Err("matrisen är inte positivt definit".into()); }
        let d = d.sqrt();
        a[j][j] = d;
        for i in j + 1..p {
            let mut s = a[i][j];
            for k in 0..j { s -= a[i][k] * a[j][k]; }
            a[i][j] = s / d;
        }
    }
    let q = b[0].len();
    for c in 0..q {
        for i in 0..p {
            let mut s = b[i][c];
            for k in 0..i { s -= a[i][k] * b[k][c]; }
            b[i][c] = s / a[i][i];
        }
        for i in (0..p).rev() {
            let mut s = b[i][c];
            for k in i + 1..p { s -= a[k][i] * b[k][c]; }
            b[i][c] = s / a[i][i];
        }
    }
    Ok(())
}

fn atervinn_gen(pv: &[&RadProv], uid: u64) -> Result<Vec<f64>, String> {
    const P: usize = S_N + 1;
    let mut par: Vec<(&[f64], &[f64])> = Vec::with_capacity(pv.len() * (1 + A_N));
    for p in pv {
        par.push((&p.s, &p.z));
        for a in 0..A_N {
            par.push((&p.s2[a], &p.zn[a]));
        }
    }
    let mut gen = vec![0.0; N_GEN];
    for i in 0..Z_N {
        let mut ata = vec![vec![0.0; P]; P];
        let mut atb = vec![vec![0.0; 1]; P];
        let mut n_om = 0usize;
        for &(s, z) in &par {
            if z[i].abs() >= 0.9999 { continue; }
            n_om += 1;
            let t = z[i].atanh();
            let mut x = [1.0; P];
            x[..S_N].copy_from_slice(s);
            for r in 0..P {
                for c in 0..=r { ata[r][c] += x[r] * x[c]; }
                atb[r][0] += x[r] * t;
            }
        }
        let lam = if n_om >= P + 5 { 1e-12 } else { 1e-6 };
        for r in 0..P {
            for c in 0..r { ata[c][r] = ata[r][c]; }
            ata[r][r] += lam;
        }
        cholesky_los(&mut ata, &mut atb)?;
        for r in 0..P {
            gen[i * P + r] = atb[r][0];
        }
    }
    let mut rng = Rng::new(uid ^ 0x2008);
    for g in &mut gen[Z_N * P..] {
        *g = SIGMA0 * gauss(&mut rng);
    }
    Ok(gen)
}

// ---------------------------------------------------------------- externa avläsningsmodeller (som vminfo.rs)

#[derive(Clone, Default)]
struct Summa {
    rest: f64,
    verklig: f64,
    fel: f64,
    fel_pers: f64,
    prov: u64,
}

impl Summa {
    fn r2(&self) -> f64 { 1.0 - self.rest / self.verklig.max(1e-300) }
    fn kvot(&self) -> f64 { self.fel / self.fel_pers.max(1e-300) }
    fn lagg(&mut self, o: &Summa) {
        self.rest += o.rest;
        self.verklig += o.verklig;
        self.fel += o.fel;
        self.fel_pers += o.fel_pers;
        self.prov += o.prov;
    }
}

fn bidrag(gissn: &[Vec<f64>], utf: &[Vec<f64>], fore: &[f64]) -> Summa {
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

#[derive(Clone, Copy)]
enum SinnesGrupp { VaxtStannar, VaxtAlla, AntalAlla, Energi }

fn stannar_i_fonster(a: usize, k2: usize) -> bool {
    let (di, dj) = GRANNAR[a];
    let (gi, gj) = GRANNAR[k2];
    (di + gi).abs() <= 1 && (dj + gj).abs() <= 1
}

fn grupp_har(g: SinnesGrupp, a: usize, d: usize) -> bool {
    match g {
        SinnesGrupp::VaxtStannar => d < 18 && d % 2 == 0 && stannar_i_fonster(a, d / 2),
        SinnesGrupp::VaxtAlla => d < 18 && d % 2 == 0,
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
            if !grupp_har(g, a, d) { continue; }
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

#[derive(Clone)]
struct Ridge {
    mx: Vec<f64>,
    sx: Vec<f64>,
    my: Vec<f64>,
    w: Vec<Vec<f64>>,
}

impl Ridge {
    fn anpassa(x: &[Vec<f64>], y: &[Vec<f64>], del: &[usize]) -> Result<Ridge, String> {
        let n = x.len();
        if n < 2 { return Err("för få rader".into()); }
        let (p, q) = (x[0].len(), y[0].len());
        let mut mx = vec![0.0; p];
        let mut my = vec![0.0; q];
        for (xr, yr) in x.iter().zip(y) {
            for j in 0..p { mx[j] += xr[j] / n as f64; }
            for c in 0..q { my[c] += yr[c] / n as f64; }
        }
        let mut sx = vec![0.0; p];
        for xr in x {
            for j in 0..p { sx[j] += (xr[j] - mx[j]).powi(2) / n as f64; }
        }
        let sx: Vec<f64> = sx.iter().map(|v| skala_kol(v.sqrt())).collect();
        let k = del.iter().max().unwrap() + 1;
        let mut ak = vec![vec![vec![0.0; p]; p]; k];
        let mut bk = vec![vec![vec![0.0; q]; p]; k];
        let xs: Vec<Vec<f64>> = x.iter().map(|xr| (0..p).map(|j| (xr[j] - mx[j]) * sx[j]).collect()).collect();
        let yc: Vec<Vec<f64>> = y.iter().map(|yr| (0..q).map(|c| yr[c] - my[c]).collect()).collect();
        for r in 0..n {
            let (a, b) = (&mut ak[del[r]], &mut bk[del[r]]);
            for j in 0..p {
                for l in 0..=j { a[j][l] += xs[r][j] * xs[r][l]; }
                for c in 0..q { b[j][c] += xs[r][j] * yc[r][c]; }
            }
        }
        let mut at = vec![vec![0.0; p]; p];
        let mut bt = vec![vec![0.0; q]; p];
        for d in 0..k {
            for j in 0..p {
                for l in 0..=j { at[j][l] += ak[d][j][l]; }
                for c in 0..q { bt[j][c] += bk[d][j][c]; }
            }
        }
        let los = |a0: &[Vec<f64>], b0: &[Vec<f64>], lam: f64| -> Result<Vec<Vec<f64>>, String> {
            let mut a = a0.to_vec();
            for j in 0..p {
                for l in 0..j { a[l][j] = a[j][l]; }
                a[j][j] += lam;
            }
            let mut b = b0.to_vec();
            cholesky_los(&mut a, &mut b)?;
            Ok(b)
        };
        let delar: Vec<usize> = (0..k).filter(|&d| del.contains(&d)).collect();
        let lam = if delar.len() < 2 {
            LAMBDA_ENSAM
        } else {
            let mut bast = (f64::INFINITY, LAMBDA_ENSAM);
            for &lam in &LAMBDOR {
                let mut fel = 0.0;
                for &d in &delar {
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
                if fel < bast.0 { bast = (fel, lam); }
            }
            bast.1
        };
        Ok(Ridge { mx, sx, my, w: los(&at, &bt, lam)? })
    }

    fn gissa(&self, x: &[f64]) -> Vec<f64> {
        let mut ut = self.my.clone();
        for j in 0..self.mx.len() {
            let v = (x[j] - self.mx[j]) * self.sx[j];
            if v != 0.0 {
                for (c, u) in ut.iter_mut().enumerate() { *u += v * self.w[j][c]; }
            }
        }
        ut
    }
}

struct Drag { mx: Vec<f64>, sx: Vec<f64>, r: Vec<Vec<f64>>, b: Vec<f64> }

impl Drag {
    fn ny(x: &[Vec<f64>]) -> Drag {
        let (n, p) = (x.len(), x[0].len());
        let mut mx = vec![0.0; p];
        for xr in x { for j in 0..p { mx[j] += xr[j] / n as f64; } }
        let mut sx = vec![0.0; p];
        for xr in x { for j in 0..p { sx[j] += (xr[j] - mx[j]).powi(2) / n as f64; } }
        let sx = sx.iter().map(|v: &f64| skala_kol(v.sqrt())).collect();
        let mut g = Rng::new(DRAG_FRO ^ p as u64);
        let mut gfn = || { let (u1, u2) = (g.f().max(1e-300), g.f()); (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos() };
        let sk = 1.0 / (p as f64).sqrt();
        let r = (0..N_DRAG).map(|_| (0..p).map(|_| gfn() * sk).collect()).collect();
        let b = (0..N_DRAG).map(|_| gfn() * 0.5).collect();
        Drag { mx, sx, r, b }
    }

    fn av(&self, x: &[f64]) -> Vec<f64> {
        self.r.iter().zip(&self.b).map(|(rr, b)| {
            (b + rr.iter().enumerate().map(|(j, w)| w * (x[j] - self.mx[j]) * self.sx[j]).sum::<f64>()).tanh()
        }).collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Form { Ridge, Drag }

struct Modell { drag: Option<Drag>, delar: Vec<Ridge> }

impl Modell {
    fn anpassa(form: Form, x: &[Vec<Vec<f64>>], y: &[Vec<Vec<f64>>], del: &[Vec<usize>], med_h: bool) -> Result<Modell, String> {
        let alla_x: Vec<Vec<f64>> = x.iter().flatten().cloned().collect();
        let drag = if form == Form::Drag { Some(Drag::ny(&alla_x)) } else { None };
        let f = |v: &Vec<Vec<f64>>| -> Vec<Vec<f64>> { match &drag { Some(d) => v.iter().map(|r| d.av(r)).collect(), None => v.clone() } };
        let delar = if med_h {
            (0..A_N).map(|a| Ridge::anpassa(&f(&x[a]), &y[a], &del[a])).collect::<Result<Vec<_>, _>>()?
        } else {
            let ay: Vec<Vec<f64>> = y.iter().flatten().cloned().collect();
            let ad: Vec<usize> = del.iter().flatten().copied().collect();
            vec![Ridge::anpassa(&f(&alla_x), &ay, &ad)?]
        };
        Ok(Modell { drag, delar })
    }

    fn gissa(&self, x: &[f64], a: usize) -> Vec<f64> {
        let xx = match &self.drag { Some(d) => d.av(x), None => x.to_vec() };
        let del = if self.delar.len() == 1 { &self.delar[0] } else { &self.delar[a] };
        del.gissa(&xx)
    }
}

// ---------------------------------------------------------------- Spår A: online längs faktiska banan

struct ArmSpec {
    namn: &'static str,
    cfg: Hj2008Config,
}

const ARMAR: [ArmSpec; 4] = [
    ArmSpec { namn: "Alt 2008 (full: online-kodare + IDM + U+ΔW_a)", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Alt 2008 additiv (56 vikter, U+d_a)", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: true, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 1: fryst kodare (skalning + U+ΔW_a)", cfg: Hj2008Config { lar_kodare: false, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 2: online-kodare utan IDM", cfg: Hj2008Config { lar_kodare: true, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
];

const ARMAR_2053: [ArmSpec; 5] = [
    ArmSpec { namn: "Alt 2053 (blind kanalgrind + online-kodare + IDM)", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 1: blind kanalgrind + fryst kodare", cfg: Hj2008Config { lar_kodare: false, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 2: blind kanalgrind + online utan IDM", cfg: Hj2008Config { lar_kodare: true, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Diagnos A (tak): strukna djurantal + online-kodare", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: true, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Diagnos B (tak): strukna djurantal + fryst kodare", cfg: Hj2008Config { lar_kodare: false, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: true, skarp_kanalgrind: false, med_z_norm: false, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
];

const ARMAR_2056: [ArmSpec; 5] = [
    ArmSpec { namn: "Alt 2056 (skärpt grind + z-norm + RLS + online + IDM)", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: true, med_z_norm: true, med_rls: true, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 1: skärpt grind + z-norm + RLS + fryst kodare", cfg: Hj2008Config { lar_kodare: false, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: true, med_z_norm: true, med_rls: true, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 2: skärpt grind + z-norm + RLS + online utan IDM", cfg: Hj2008Config { lar_kodare: true, med_idm: false, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: true, med_z_norm: true, med_rls: true, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Strykprov 3: skärpt grind + z-norm + NLMS (utan RLS)", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: true, stryk_djurantal_diagnos: false, skarp_kanalgrind: true, med_z_norm: true, med_rls: false, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
    ArmSpec { namn: "Diagnos (tak): strukna djurantal + z-norm + RLS + online", cfg: Hj2008Config { lar_kodare: true, med_idm: true, additiv_endast: false, stryk_handling_i_val: false, med_kanalgrind: false, stryk_djurantal_diagnos: true, skarp_kanalgrind: false, med_z_norm: true, med_rls: true, rls_fristaende: false, rls_alpha_tio: 0, norm_lage: 0, med_kritiker: false } },
];

struct EvalSteg {
    t: i64,
    prov: bool,
    s_sk: Vec<f64>,
    z: Vec<f64>,
    y: Vec<Vec<f64>>,
    zn_mal: Vec<Vec<f64>>,
    zn_onl: Vec<Vec<f64>>,
    idm_giss: Vec<usize>,
    s2_orig: Vec<Vec<f64>>,
    s0_orig: Vec<f64>,
    de: Vec<f64>,
    intag: Vec<f64>,
}

struct ArmVarldRes {
    hjarna: Summa,
    m1: [Summa; 2],
    m2: [Summa; 2],
    de: [Summa; 2],
    intag: [Summa; 2],
    diag_a: [f64; 2], // ridge på z, online hj.idm
    diag_a_tot: u64,
    diag_a_idm_ratt: u64,
    probe_m2: [Summa; 4],
    probe_vk: [Summa; 4],
    min_axel_var_pct: Vec<f64>,
    min_axel_var_abs: Vec<f64>,
    grind_vaxt: Vec<f64>,
    grind_djur: Vec<f64>,
    grind_energi: Vec<f64>,
}

impl Default for ArmVarldRes {
    fn default() -> Self {
        Self {
            hjarna: Summa::default(),
            m1: Default::default(),
            m2: Default::default(),
            de: Default::default(),
            intag: Default::default(),
            diag_a: [0.0; 2],
            diag_a_tot: 0,
            diag_a_idm_ratt: 0,
            probe_m2: Default::default(),
            probe_vk: Default::default(),
            min_axel_var_pct: Vec::new(),
            min_axel_var_abs: Vec::new(),
            grind_vaxt: Vec::new(),
            grind_djur: Vec::new(),
            grind_energi: Vec::new(),
        }
    }
}

fn hitta_faktisk_handling(nu: &RadProv, nasta: &RadProv) -> Result<usize, String> {
    if nasta.t != nu.t + 1 {
        return Err(format!("lucka i steg vid uid={} t={} -> {}", nu.uid, nu.t, nasta.t));
    }
    // Jämför primärt på växt+energi (j%2 == 0 || j == S_N - 1), eftersom `vminfo::kopia` med `tvinga_grupp`
    // kan förskjuta en grannes djurantalskanal med 0.25 i 50/213 194 steg.
    let mut kand = Vec::with_capacity(A_N);
    for a in 0..A_N {
        let diff_na = (0..S_N)
            .filter(|&j| j % 2 == 0 || j == S_N - 1)
            .map(|j| (nu.s2[a][j] - nasta.s[j]).abs())
            .fold(0.0f64, f64::max);
        let diff_all = nu.s2[a]
            .iter()
            .zip(&nasta.s)
            .map(|(p, q)| (p - q).abs())
            .fold(0.0f64, f64::max);
        kand.push((diff_na, diff_all, a));
    }
    kand.sort_by(|p, q| p.0.total_cmp(&q.0).then_with(|| p.1.total_cmp(&q.1)));
    let basta = kand[0];
    let nast_basta = kand[1];
    if basta.0 > 1e-6 || nast_basta.0 <= 1e-6 {
        return Err(format!(
            "uid={} t={}: tvetydig eller felaktig matchning mot s(t+1): basta=({:.3e}, {:.3e}, a={}), nast=({:.3e}, {:.3e}, a={})",
            nu.uid, nu.t, basta.0, basta.1, basta.2, nast_basta.0, nast_basta.1, nast_basta.2
        ));
    }
    Ok(basta.2)
}

fn var_axel(v: &[f64]) -> f64 {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n.max(1.0);
    v.iter().map(|&x| (x - m).powi(2)).sum::<f64>() / n.max(1.0)
}

fn idm_ridge_feat(z: &[f64], zn: &[f64]) -> Vec<f64> {
    let mut feat = Vec::with_capacity(Z_N * Z_N + Z_N);
    for i in 0..Z_N {
        let dz = zn[i] - z[i];
        for j in 0..Z_N {
            feat.push(dz * z[j]);
        }
    }
    for i in 0..Z_N {
        let dz = zn[i] - z[i];
        feat.push(dz * dz);
    }
    feat
}

fn spara_varld(fil: &Path, armar: &[ArmSpec], egen_logg: &str, tagg: &str) -> Result<(u64, usize, Vec<ArmVarldRes>), String> {
    let t0 = std::time::Instant::now();
    let (fro, prov) = las_rader(fil)?;
    let mut per: BTreeMap<u64, Vec<&RadProv>> = BTreeMap::new();
    for p in &prov {
        per.entry(p.uid).or_default().push(p);
    }
    let mut resultat: Vec<ArmVarldRes> = (0..armar.len()).map(|_| ArmVarldRes::default()).collect();
    let mut individer = 0usize;
    let mut diag_a_ridge_sum = vec![(0u64, 0u64); armar.len()];

    for (&uid, pv_raw) in &per {
        if pv_raw.len() < MIN_STEG { continue; }
        let mut pv = pv_raw.clone();
        pv.sort_by_key(|p| p.t);
        if pv.iter().filter(|p| !p.prov).count() < 20 || pv.iter().filter(|p| p.prov).count() == 0 {
            continue;
        }
        individer += 1;
        let gen = atervinn_gen(&pv, uid)?;

        let mut fact_a = Vec::with_capacity(pv.len());
        for idx in 0..pv.len() - 1 {
            fact_a.push(hitta_faktisk_handling(pv[idx], pv[idx + 1])?);
        }
        fact_a.push(0); // sista stegets nästa steg används aldrig för träning.

        for (ai, arm) in armar.iter().enumerate() {
            let mut hj = Hj2008::fran_gen(&gen, arm.cfg);
            let mut steg_data = Vec::with_capacity(pv.len());
            let mut z_tidig: [Vec<f64>; Z_N] = Default::default();
            let mut z_prov: [Vec<f64>; Z_N] = Default::default();

            for (idx, p) in pv.iter().enumerate() {
                if !hj.skala_init { hj.uppdatera_skala(&p.s); }
                let mu_t = hj.mu;
                let var_t = hj.var;
                let g_t = hj.aktuell_kanal_vikt();
                if hj.cfg.med_z_norm && !hj.z_skala_init {
                    let s_sk0 = hj.skala(&p.s);
                    let mut s_in0 = [0.0; S_N];
                    for j in 0..S_N { s_in0[j] = g_t[j] * s_sk0[j]; }
                    let z_raw0 = hj.kod_online_ra(&s_in0);
                    hj.uppdatera_z_skala(&z_raw0);
                }
                let z_init_t = hj.z_skala_init;
                let mu_z_t = hj.mu_z;
                let var_z_t = hj.var_z;
                let skala_par = |sv: &[f64]| -> [f64; S_N] {
                    let mut ut = [0.0; S_N];
                    for j in 0..S_N {
                        ut[j] = g_t[j] * ((sv[j] - mu_t[j]) / (var_t[j] + 1e-4).sqrt()).clamp(-3.0, 3.0);
                    }
                    ut
                };
                let med_zn_t = hj.cfg.med_z_norm;
                let norm_par = |raw: [f64; Z_N]| -> [f64; Z_N] {
                    if !med_zn_t || !z_init_t { return raw; }
                    let mut ut = [0.0; Z_N];
                    for i in 0..Z_N {
                        ut[i] = ((raw[i] - mu_z_t[i]) / (var_z_t[i] + 1e-4).sqrt()).clamp(-3.0, 3.0);
                    }
                    ut
                };
                // Kör stegets online-uppdatering ur förra stegets faktiska utfall:
                hj.lar_och_satt(&p.s, true, fact_a[idx]);
                let s_sk_nu = hj.forra.unwrap().0;
                let z_nu = hj.forra.unwrap().1;
                for d in 0..Z_N {
                    if idx < 60 { z_tidig[d].push(z_nu[d]); }
                    if p.prov { z_prov[d].push(z_nu[d]); }
                }
                let y_alla: Vec<Vec<f64>> = (0..A_N).map(|a| hj.forutsag(&z_nu, a).to_vec()).collect();
                let mut zn_mal_alla = Vec::with_capacity(A_N);
                let mut zn_onl_alla = Vec::with_capacity(A_N);
                let mut idm_g = Vec::with_capacity(A_N);
                for a in 0..A_N {
                    let s2_sk = skala_par(&p.s2[a]);
                    let zm = norm_par(hj.kod_mal_ra(&s2_sk));
                    let zo = norm_par(hj.kod_online_ra(&s2_sk));
                    let p_idm = hj.idm_sannolikheter(&z_nu, &zo);
                    let mut ba = 0;
                    for k in 1..A_N { if p_idm[k] > p_idm[ba] { ba = k; } }
                    idm_g.push(ba);
                    zn_mal_alla.push(zm.to_vec());
                    zn_onl_alla.push(zo.to_vec());
                }
                let de: Vec<f64> = (0..A_N).map(|a| p.s2[a][S_N - 1] - p.s[S_N - 1]).collect();
                steg_data.push(EvalSteg {
                    t: p.t,
                    prov: p.prov,
                    s_sk: s_sk_nu.to_vec(),
                    z: z_nu.to_vec(),
                    y: y_alla,
                    zn_mal: zn_mal_alla,
                    zn_onl: zn_onl_alla,
                    idm_giss: idm_g,
                    s2_orig: p.s2.clone(),
                    s0_orig: p.s.clone(),
                    de,
                    intag: p.intag.clone(),
                });
            }

            let min_abs = (0..Z_N).map(|d| var_axel(&z_prov[d])).fold(f64::INFINITY, f64::min);
            let min_pct = (0..Z_N)
                .map(|d| 100.0 * var_axel(&z_prov[d]) / var_axel(&z_tidig[d]).max(1e-12))
                .fold(f64::INFINITY, f64::min);
            resultat[ai].min_axel_var_abs.push(min_abs);
            resultat[ai].min_axel_var_pct.push(min_pct);
            let gv = hj.aktuell_kanal_vikt();
            let vaxt_m = (0..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0;
            let djur_m = (1..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0;
            resultat[ai].grind_vaxt.push(vaxt_m);
            resultat[ai].grind_djur.push(djur_m);
            resultat[ai].grind_energi.push(gv[S_N - 1]);

            let tr: Vec<&EvalSteg> = steg_data.iter().filter(|e| !e.prov).collect();
            let te: Vec<&EvalSteg> = steg_data.iter().filter(|e| e.prov).collect();

            // Externa modeller på samma online-producerade z och s_sk.
            let mut xs = vec![Vec::new(); A_N];
            let mut xz = vec![Vec::new(); A_N];
            let mut yz = vec![Vec::new(); A_N];
            let mut yde = vec![Vec::new(); A_N];
            let mut yin = vec![Vec::new(); A_N];
            let mut del = vec![Vec::new(); A_N];
            let mut x_idm = Vec::new();
            let mut y_idm = Vec::new();
            let mut d_idm = Vec::new();
            let mut px = Vec::new();
            let mut py = Vec::new();
            let mut pd = Vec::new();

            for e in &tr {
                let blk = (e.t / BLOCK) as usize;
                for a in 0..A_N {
                    xs[a].push(e.s_sk.clone());
                    xz[a].push(e.z.clone());
                    yz[a].push(e.zn_mal[a].clone());
                    yde[a].push(vec![e.de[a]]);
                    yin[a].push(vec![e.intag[a]]);
                    del[a].push(blk);

                    x_idm.push(idm_ridge_feat(&e.z, &e.zn_onl[a]));
                    let mut one = vec![0.0; A_N];
                    one[a] = 1.0;
                    y_idm.push(one);
                    d_idm.push(blk);

                    px.push(e.zn_mal[a].clone());
                    py.push(e.s2_orig[a].clone());
                    pd.push(blk);
                }
            }

            let probe = Modell::anpassa(Form::Ridge, &[px], &[py], &[pd], false)?;
            let mod_idm = Modell::anpassa(Form::Ridge, &[x_idm], &[y_idm], &[d_idm], false)?;

            let mut m2_ridge_opt = None;
            for (fi, &form) in [Form::Ridge, Form::Drag].iter().enumerate() {
                let m1 = Modell::anpassa(form, &xs, &yz, &del, true)?;
                let m2 = Modell::anpassa(form, &xz, &yz, &del, true)?;
                let mde = Modell::anpassa(form, &xz, &yde, &del, true)?;
                let min = Modell::anpassa(form, &xz, &yin, &del, true)?;
                for e in &te {
                    let g1: Vec<Vec<f64>> = (0..A_N).map(|a| m1.gissa(&e.s_sk, a)).collect();
                    let g2: Vec<Vec<f64>> = (0..A_N).map(|a| m2.gissa(&e.z, a)).collect();
                    resultat[ai].m1[fi].lagg(&bidrag(&g1, &e.zn_mal, &e.z));
                    resultat[ai].m2[fi].lagg(&bidrag(&g2, &e.zn_mal, &e.z));
                    let gde: Vec<Vec<f64>> = (0..A_N).map(|a| mde.gissa(&e.z, a)).collect();
                    let ude: Vec<Vec<f64>> = (0..A_N).map(|a| vec![e.de[a]]).collect();
                    resultat[ai].de[fi].lagg(&bidrag(&gde, &ude, &[0.0]));
                    let gin: Vec<Vec<f64>> = (0..A_N).map(|a| min.gissa(&e.z, a)).collect();
                    let uin: Vec<Vec<f64>> = (0..A_N).map(|a| vec![e.intag[a]]).collect();
                    resultat[ai].intag[fi].lagg(&bidrag(&gin, &uin, &[0.0]));
                }
                if fi == 0 { m2_ridge_opt = Some(m2); }
            }
            let m2r = m2_ridge_opt.unwrap();

            for e in &te {
                resultat[ai].hjarna.lagg(&bidrag(&e.y, &e.zn_mal, &e.z));
                let g_m2_s: Vec<Vec<f64>> = (0..A_N).map(|a| probe.gissa(&m2r.gissa(&e.z, a), 0)).collect();
                let g_vk_s: Vec<Vec<f64>> = (0..A_N).map(|a| probe.gissa(&e.zn_mal[a], 0)).collect();
                for (gi, grp) in [SinnesGrupp::VaxtStannar, SinnesGrupp::VaxtAlla, SinnesGrupp::AntalAlla, SinnesGrupp::Energi].iter().enumerate() {
                    resultat[ai].probe_m2[gi].lagg(&bidrag_grupp(&g_m2_s, &e.s2_orig, &e.s0_orig, *grp));
                    resultat[ai].probe_vk[gi].lagg(&bidrag_grupp(&g_vk_s, &e.s2_orig, &e.s0_orig, *grp));
                }
                for a in 0..A_N {
                    let feat = idm_ridge_feat(&e.z, &e.zn_onl[a]);
                    if argmax(&mod_idm.gissa(&feat, 0)) == a { diag_a_ridge_sum[ai].0 += 1; }
                    diag_a_ridge_sum[ai].1 += 1;
                    if e.idm_giss[a] == a { resultat[ai].diag_a_idm_ratt += 1; }
                    resultat[ai].diag_a_tot += 1;
                }
            }
        }
    }

    for ai in 0..armar.len() {
        resultat[ai].diag_a[0] = diag_a_ridge_sum[ai].0 as f64 / diag_a_ridge_sum[ai].1.max(1) as f64;
        resultat[ai].diag_a[1] = resultat[ai].diag_a_idm_ratt as f64 / resultat[ai].diag_a_tot.max(1) as f64;
    }

    if tagg == "Alt 2056" {
        live_logg_med(
            egen_logg,
            tagg,
            &format!(
                "Spår A frö {fro} ({individer} ind, {:.1}s): hjR²={} fk={} M2={} ΔE={} IDM={}/{} g(v/d/E)={}/{}/{} | fryst {}/{} | NLMS {}/{} | diag {}/{}",
                t0.elapsed().as_secs_f64(),
                f3(resultat[0].hjarna.r2()),
                f3(resultat[0].hjarna.kvot()),
                f3(resultat[0].m2[0].r2().max(resultat[0].m2[1].r2())),
                f3(resultat[0].de[0].r2().max(resultat[0].de[1].r2())),
                f3(resultat[0].diag_a[0]),
                f3(resultat[0].diag_a[1]),
                f3(median(resultat[0].grind_vaxt.clone())),
                f3(median(resultat[0].grind_djur.clone())),
                f3(median(resultat[0].grind_energi.clone())),
                f3(resultat[1].hjarna.r2()),
                f3(resultat[1].m2[0].r2().max(resultat[1].m2[1].r2())),
                f3(resultat[3].hjarna.r2()),
                f3(resultat[3].m2[0].r2().max(resultat[3].m2[1].r2())),
                f3(resultat[4].hjarna.r2()),
                f3(resultat[4].m2[0].r2().max(resultat[4].m2[1].r2())),
            ),
        );
    } else if armar.len() == 5 {
        live_logg_med(
            egen_logg,
            tagg,
            &format!(
                "Spår A frö {fro} ({individer} ind, {:.1}s): blindM2={} ΔE={} IDM={}/{} g(v/d/E)={}/{}/{} | frystM2={} | diagnosM2={} ΔE={}",
                t0.elapsed().as_secs_f64(),
                f3(resultat[0].m2[0].r2().max(resultat[0].m2[1].r2())),
                f3(resultat[0].de[0].r2().max(resultat[0].de[1].r2())),
                f3(resultat[0].diag_a[0]),
                f3(resultat[0].diag_a[1]),
                f3(median(resultat[0].grind_vaxt.clone())),
                f3(median(resultat[0].grind_djur.clone())),
                f3(median(resultat[0].grind_energi.clone())),
                f3(resultat[1].m2[0].r2().max(resultat[1].m2[1].r2())),
                f3(resultat[3].m2[0].r2().max(resultat[3].m2[1].r2())),
                f3(resultat[3].de[0].r2().max(resultat[3].de[1].r2())),
            ),
        );
    } else {
        live_logg_med(
            egen_logg,
            tagg,
            &format!(
                "Spår A frö {fro} ({individer} ind, {:.1}s): hjR²={} fk={} M2={} ΔE={} IDM={}/{}, minVar={:.1}% | fryst M2={} | utanIDM M2={}",
                t0.elapsed().as_secs_f64(),
                f3(resultat[0].hjarna.r2()),
                f3(resultat[0].hjarna.kvot()),
                f3(resultat[0].m2[0].r2().max(resultat[0].m2[1].r2())),
                f3(resultat[0].de[0].r2().max(resultat[0].de[1].r2())),
                f3(resultat[0].diag_a[0]),
                f3(resultat[0].diag_a[1]),
                median(resultat[0].min_axel_var_pct.clone()),
                f3(resultat[2].m2[0].r2().max(resultat[2].m2[1].r2())),
                f3(resultat[3].m2[0].r2().max(resultat[3].m2[1].r2())),
            ),
        );
    }
    Ok((fro, individer, resultat))
}

fn kora_spara_med(
    mapp: &Path,
    armar: &[ArmSpec],
    rubrik: &str,
    regel_fil: &str,
    egen_logg: &str,
    tagg: &str,
    chunks: usize,
) -> Result<String, String> {
    let mut filer: Vec<PathBuf> = std::fs::read_dir(mapp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("rader-") && n.ends_with(".tsv")))
        .collect();
    filer.sort();
    if filer.is_empty() { return Err("inga radfiler".into()); }
    live_logg_med(egen_logg, tagg, &format!("Startar {tagg} Spår A på {} världar ({})", filer.len(), mapp.display()));

    let mut svar = Vec::with_capacity(filer.len());
    for del in filer.chunks(chunks) {
        let res: Vec<Result<_, String>> = std::thread::scope(|s| {
            let h: Vec<_> = del.iter().map(|f| s.spawn(move || spara_varld(f, armar, egen_logg, tagg))).collect();
            h.into_iter().map(|x| x.join().unwrap_or_else(|_| Err("tråd föll".into()))).collect()
        });
        for r in res { svar.push(r?); }
    }
    svar.sort_by_key(|x| x.0);

    let n = svar.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# {rubrik}\n");
    let _ = writeln!(t, "Regler: piharness `{regel_fil}`. {n} världar; krav grönt: minst {krav} av {n}.\n");

    let _ = writeln!(t, "## 1. Sammanfattning över {n} världar (median; ridge / slumpdrag)\n");
    let _ = writeln!(t, "| Arm | Hjärnans R² | Felkvot | M1 R² | M2 R² | z+a→ΔE R² | z+a→intag R² | Diagnos A (ridge / IDM) | Min axelvar % |\n|---|---|---|---|---|---|---|---|---|");
    for (ai, arm) in armar.iter().enumerate() {
        let hjr = median(svar.iter().map(|v| v.2[ai].hjarna.r2()).collect());
        let hjk = median(svar.iter().map(|v| v.2[ai].hjarna.kvot()).collect());
        let m1r = median(svar.iter().map(|v| v.2[ai].m1[0].r2()).collect());
        let m1d = median(svar.iter().map(|v| v.2[ai].m1[1].r2()).collect());
        let m2r = median(svar.iter().map(|v| v.2[ai].m2[0].r2()).collect());
        let m2d = median(svar.iter().map(|v| v.2[ai].m2[1].r2()).collect());
        let der = median(svar.iter().map(|v| v.2[ai].de[0].r2()).collect());
        let ded = median(svar.iter().map(|v| v.2[ai].de[1].r2()).collect());
        let inr = median(svar.iter().map(|v| v.2[ai].intag[0].r2()).collect());
        let ind = median(svar.iter().map(|v| v.2[ai].intag[1].r2()).collect());
        let da0 = median(svar.iter().map(|v| v.2[ai].diag_a[0]).collect());
        let da1 = median(svar.iter().map(|v| v.2[ai].diag_a[1]).collect());
        let mvar = median(svar.iter().flat_map(|v| v.2[ai].min_axel_var_pct.iter().copied()).collect());
        let _ = writeln!(
            t,
            "| {} | {} | {} | {} / {} | {} / {} | {} / {} | {} / {} | {} / {} | {:.1} % |",
            arm.namn, f3(hjr), f3(hjk), f3(m1r), f3(m1d), f3(m2r), f3(m2d), f3(der), f3(ded), f3(inr), f3(ind), f3(da0), f3(da1), mvar
        );
    }

    let _ = writeln!(t, "\n## 2. Externa prober från verklig `z'` och `M2`-förutsagd `z'` (median R²)\n");
    let _ = writeln!(t, "| Arm | Källa till z' | Växt stannar | Växt alla 9 | Antal djur alla 9 | Energi (s18) |\n|---|---|---|---|---|---|");
    for (ai, arm) in armar.iter().enumerate() {
        for (kn, pvec) in [("Verklig z'", svar.iter().map(|v| &v.2[ai].probe_vk).collect::<Vec<_>>()), ("M2-förutsagd z'", svar.iter().map(|v| &v.2[ai].probe_m2).collect::<Vec<_>>())] {
            let vals: Vec<String> = (0..4).map(|gi| f3(median(pvec.iter().map(|p| p[gi].r2()).collect()))).collect();
            let _ = writeln!(t, "| {} | {kn} | {} |", arm.namn, vals.join(" | "));
        }
    }

    if armar.len() == 5 {
        let _ = writeln!(t, "\n## 3. Lärda kanalvikter (blind kanalgrind) och absolut axelspridning\n");
        let _ = writeln!(t, "| Arm | Växt (9 kanaler) | Djurantal (9 kanaler) | Energi (s18) | Min axelvar (abs) |\n|---|---|---|---|---|");
        for (ai, arm) in armar.iter().enumerate() {
            let gv = median(svar.iter().flat_map(|v| v.2[ai].grind_vaxt.iter().copied()).collect());
            let gd = median(svar.iter().flat_map(|v| v.2[ai].grind_djur.iter().copied()).collect());
            let ge = median(svar.iter().flat_map(|v| v.2[ai].grind_energi.iter().copied()).collect());
            let vabs = median(svar.iter().flat_map(|v| v.2[ai].min_axel_var_abs.iter().copied()).collect());
            let _ = writeln!(t, "| {} | {} | {} | {} | {} |", arm.namn, f3(gv), f3(gd), f3(ge), f3(vabs));
        }
    }

    let (idx_fryst, idx_uidm) = if armar.len() == 5 { (1, 2) } else { (2, 3) };
    let hj_over0 = svar.iter().filter(|v| v.2[0].hjarna.r2() > 0.0).count();
    let fk_med = median(svar.iter().map(|v| v.2[0].hjarna.kvot()).collect());
    let m2_over010 = svar.iter().filter(|v| v.2[0].m2[0].r2().max(v.2[0].m2[1].r2()) >= 0.10).count();
    let de_over0 = svar.iter().filter(|v| v.2[0].de[0].r2().max(v.2[0].de[1].r2()) > 0.0).count();
    let de_med = median(svar.iter().map(|v| v.2[0].de[0].r2().max(v.2[0].de[1].r2())).collect());
    let da_mean = svar.iter().map(|v| v.2[0].diag_a[0].max(v.2[0].diag_a[1])).sum::<f64>() / n as f64;
    let mvar_med = median(svar.iter().flat_map(|v| v.2[0].min_axel_var_pct.iter().copied()).collect());

    let m2_full_med = median(svar.iter().map(|v| v.2[0].m2[0].r2().max(v.2[0].m2[1].r2())).collect());
    let m2_fryst_med = median(svar.iter().map(|v| v.2[idx_fryst].m2[0].r2().max(v.2[idx_fryst].m2[1].r2())).collect());
    let m2_uidm_med = median(svar.iter().map(|v| v.2[idx_uidm].m2[0].r2().max(v.2[idx_uidm].m2[1].r2())).collect());

    let mvar_abs_med = median(svar.iter().flat_map(|v| v.2[0].min_axel_var_abs.iter().copied()).collect());

    let gron = hj_over0 >= krav
        && fk_med < 1.0
        && m2_over010 >= krav
        && da_mean >= 0.25
        && de_over0 >= krav
        && de_med >= 0.05
        && (if tagg == "Alt 2056" { mvar_abs_med >= 0.01 } else { mvar_med >= 10.0 })
        && m2_full_med > m2_fryst_med
        && m2_full_med > m2_uidm_med;

    let rubrik_nr = if armar.len() == 5 { "4" } else { "3" };
    let _ = writeln!(t, "\n## {rubrik_nr}. Grindkontroll enligt `{regel_fil}`\n");
    let _ = writeln!(t, "- Hjärnans R² > 0 (Arm 1): **{hj_over0} av {n}** (krav ≥ {krav}); felkvot median: **{}** (krav < 1,0)", f3(fk_med));
    let _ = writeln!(t, "- Externa M2 ≥ 0,10 (Arm 1): **{m2_over010} av {n}** (krav ≥ {krav}; median {})", f3(m2_full_med));
    let _ = writeln!(t, "- Diagnos A (bästa av ridge/IDM) medel: **{}** (krav ≥ 0,25)", f3(da_mean));
    let _ = writeln!(t, "- z+a→ΔE > 0: **{de_over0} av {n}** (krav ≥ {krav}), median **{}** (krav ≥ 0,05)", f3(de_med));
    let _ = writeln!(t, "- Minsta z-axelvarians: **{mvar_med:.1} %** av start (abs median **{}**)", f3(mvar_abs_med));
    let _ = writeln!(t, "- Strykprov M2: fryst kodare **{}**, utan IDM **{}** mot full **{}**", f3(m2_fryst_med), f3(m2_uidm_med), f3(m2_full_med));
    if armar.len() == 5 {
        let _ = writeln!(t, "- Antal världar `hjR² > 0` / `M2 ≥ 0,10` per arm:");
        for (ai, arm) in armar.iter().enumerate() {
            let hc = svar.iter().filter(|v| v.2[ai].hjarna.r2() > 0.0).count();
            let mc = svar.iter().filter(|v| v.2[ai].m2[0].r2().max(v.2[ai].m2[1].r2()) >= 0.10).count();
            let _ = writeln!(t, "  - {}: **{hc} / {mc}** av {n}", arm.namn);
        }
    }
    let _ = writeln!(t, "- **Dom Spår A:** **{}**", if gron { "GRÖNT (alla grindar passerade)" } else { "RÖTT (en eller flera huvudgrindar föll)" });
    live_logg_med(egen_logg, tagg, &format!("KLAR Spår A: dom = {}", if gron { "GRÖNT" } else { "RÖTT" }));
    Ok(t)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let v = |n: &str| args.iter().position(|a| a == n).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(|s| s.as_str()) {
        Some("spara") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match kora_spara_med(
                &inn,
                &ARMAR,
                "Alt 2008 Spår A (online längs sparade banor i `rader-261..280`): EXTERN AVLÄSNING + HJÄRNANS EGNA MÅTT",
                "docs/evolution/alt2008-utfallsregel.md",
                EGEN_LOGG_2008,
                "Alt 2008",
                8,
            ) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2008");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join("spara-resultat.md"), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2008, "Alt 2008", &format!("FEL i spara: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("spara2053") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match kora_spara_med(
                &inn,
                &ARMAR_2053,
                "Alt 2053 Spår A (blind kausal kanalgrind + diagnos med strukna djurantal): EXTERN AVLÄSNING + HJÄRNANS EGNA MÅTT",
                "docs/evolution/alt2053-utfallsregel.md",
                EGEN_LOGG_2053,
                "Alt 2053",
                10,
            ) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2053");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join("spara-resultat.md"), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2053, "Alt 2053", &format!("FEL i spara2053: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("spara2056") => {
            let inn = PathBuf::from(v("--in").expect("--in MAPP"));
            match kora_spara_med(
                &inn,
                &ARMAR_2056,
                "Alt 2056 Spår A (skärpt blind kanalgrind + z-norm + buffertfri RLS): EXTERN AVLÄSNING + HJÄRNANS EGNA MÅTT",
                "docs/evolution/alt2056-utfallsregel.md",
                EGEN_LOGG_2056,
                "Alt 2056",
                10,
            ) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2056");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join("spara-resultat.md"), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2056, "Alt 2056", &format!("FEL i spara2056: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2059");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join("sparb-resultat.md"), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2059, "Alt 2059", &format!("FEL i sparb: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("diag2059") => {
            let fro: u64 = v("--fro").and_then(|x| x.parse().ok()).unwrap_or(261);
            match kora_diag2059(fro) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2059");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join(format!("diag2059-fro{fro}.md")), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2059, "Alt 2059", &format!("FEL i diag2059: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2073") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let kartbild = v("--kartbild").map(PathBuf::from);
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2073(&fron, inkor_ar, mat_ar, tradar, kartbild.as_deref()) {
                Ok(t) => {
                    let ut_dir = Path::new("/home/hakan/sandbox/pi/work/hcsim/alt2073");
                    let _ = create_dir_all(ut_dir);
                    let _ = std::fs::write(ut_dir.join("sparb2073-resultat.md"), &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2073, "Alt 2073", &format!("FEL i sparb2073: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("kartbild2073") => {
            let arm_str = v("--arm").unwrap_or_else(|| "lar2073_05".to_string());
            let fro: u64 = v("--fro").and_then(|x| x.parse().ok()).unwrap_or(261);
            let var_steg: i64 = v("--var-steg").and_then(|x| x.parse().ok()).unwrap_or(15);
            let historik_s: u64 = v("--historik-s").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ar: i64 = v("--ar").and_then(|x| x.parse().ok()).unwrap_or(979_000);
            let ut = PathBuf::from(v("--ut").unwrap_or_else(|| "/home/hakan/sandbox/pi/work/hcsim/kartbild-live.json".to_string()));
            if let Err(e) = kora_kartbild2073(&arm_str, fro, var_steg, historik_s, ar, &ut) {
                live_logg_med(EGEN_LOGG_2073, "Alt 2073", &format!("FEL i kartbild2073: {e}"));
                eprintln!("FEL: {e}");
                std::process::exit(1);
            }
        }
        Some("sparb2116") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2116/sparb2116-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2116(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2116, "Alt 2116", &format!("FEL i sparb2116: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2144") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2144/sparb2144-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2144(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2144, "Alt 2144", &format!("FEL i sparb2144: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2159") => {
            let fron_str = v("--fron").unwrap_or_else(|| "301-320".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2159/sparb2159-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2159(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2159, "Alt 2159", &format!("FEL i sparb2159: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2172") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2172/sparb2172-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2172(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2172, "Alt 2172", &format!("FEL i sparb2172: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2199") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let mat_ar: i64 = v("--mat").and_then(|x| x.parse().ok()).unwrap_or(20);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2199/sparb2199-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2199(&fron, inkor_ar, mat_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2199, "Alt 2199", &format!("FEL i sparb2199: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2223") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let evo_ar: i64 = v("--evo-ar").and_then(|x| x.parse().ok()).unwrap_or(100);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2223/sparb2223-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2223(&fron, inkor_ar, evo_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2223, "Alt 2223", &format!("FEL i sparb2223: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2238") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let evo_ar: i64 = v("--evo-ar").and_then(|x| x.parse().ok()).unwrap_or(100);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2238/sparb2238-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2238(&fron, inkor_ar, evo_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2238, "Alt 2238", &format!("FEL i sparb2238: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2249") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let tavling_ar: i64 = v("--ar").and_then(|x| x.parse().ok()).unwrap_or(50);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2249/sparb2249-resultat.md"));
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2249(&fron, inkor_ar, tavling_ar, tradar) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2249, "Alt 2249", &format!("FEL i sparb2249: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2260") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let tavling_ar: i64 = v("--ar").and_then(|x| x.parse().ok()).unwrap_or(100);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2260/sparb2260-resultat.md"));
            let ra_ut = v("--ra-ut").map(PathBuf::from);
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2260(&fron, inkor_ar, tavling_ar, tradar, ra_ut.as_deref()) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2260, "Alt 2260", &format!("FEL i sparb2260: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("slaihops2260") => {
            let in_str = v("--in").unwrap_or_default();
            let filer: Vec<PathBuf> = in_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2267/sparb2260-sammanslagen.md"));
            match slaihops_2260(&filer) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL slaihops2260: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2267") => {
            let fron_str = v("--fron").unwrap_or_else(|| "261-280".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let tavling_ar: i64 = v("--ar").and_then(|x| x.parse().ok()).unwrap_or(50);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2267/sparb2267-resultat.md"));
            let ra_ut = v("--ra-ut").map(PathBuf::from);
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2267(&fron, inkor_ar, tavling_ar, tradar, ra_ut.as_deref()) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2267, "Alt 2267", &format!("FEL i sparb2267: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("slaihops2267") => {
            let in_str = v("--in").unwrap_or_default();
            let filer: Vec<PathBuf> = in_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2267/sparb2267-sammanslagen.md"));
            match slaihops_2267(&filer) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL slaihops2267: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("sparb2270") => {
            let fron_str = v("--fron").unwrap_or_else(|| "321-340".to_string());
            let inkor_ar: i64 = v("--inkor").and_then(|x| x.parse().ok()).unwrap_or(10);
            let tavling_ar: i64 = v("--ar").and_then(|x| x.parse().ok()).unwrap_or(50);
            let tradar: usize = v("--tradar").and_then(|x| x.parse().ok()).unwrap_or(10);
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2270/sparb2270-resultat.md"));
            let ra_ut = v("--ra-ut").map(PathBuf::from);
            let fron = tolka_fron(&fron_str).unwrap_or_else(|e| {
                eprintln!("FEL --fron: {e}");
                std::process::exit(2);
            });
            match kora_sparb2270(&fron, inkor_ar, tavling_ar, tradar, ra_ut.as_deref()) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    live_logg_med(EGEN_LOGG_2270, "Alt 2270", &format!("FEL i sparb2270: {e}"));
                    eprintln!("FEL: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("slaihops2270") => {
            let in_str = v("--in").unwrap_or_default();
            let filer: Vec<PathBuf> = in_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .collect();
            let ut_fil = v("--ut")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/home/hakan/sandbox/pi/work/hcsim/alt2270/sparb2270-sammanslagen.md"));
            match slaihops_2270(&filer) {
                Ok(t) => {
                    if let Some(p) = ut_fil.parent() {
                        let _ = create_dir_all(p);
                    }
                    let _ = std::fs::write(&ut_fil, &t);
                    println!("{t}");
                }
                Err(e) => {
                    eprintln!("FEL slaihops2270: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("användning: prov2008 spara|spara2053|spara2056|sparb|diag2059|sparb2073|kartbild2073|sparb2116|sparb2144|sparb2159|sparb2172|sparb2199|sparb2223|sparb2238|sparb2249|sparb2260|slaihops2260|sparb2267|slaihops2267|sparb2270|slaihops2270 ...");
            std::process::exit(2);
        }
    }
}

// ---------------------------------------------------------------- Spår B: levande simulering (Alt 2059)

const SPARB_ARMAR: [(&str, Formaga); 9] = [
    ("Allvetande", Formaga::Allvetande),
    ("Allvetande10", Formaga::AllvetandeSlump(0.10)),
    ("TrangselOrakel", Formaga::TrangselOrakel),
    ("Slump", Formaga::Slump),
    ("Tabell", Formaga::Tabell),
    ("Lar", Formaga::Lar),
    ("Lar2059", Formaga::Lar2059),
    ("Frusen2059", Formaga::Frusen2059),
    ("Stryk2059", Formaga::Stryk2059),
];

#[derive(Clone, Default)]
struct SparbArmUtfall {
    medelintag: f64,
    slutpop: f64,
    medelenergi: f64,
    svalt_andel: f64,
    allv_traff: f64,
    hj_r2: f64,
    hj_fk: f64,
    g_vaxt: f64,
    g_djur: f64,
    g_energi: f64,
}

fn tolka_fron(s: &str) -> Result<Vec<u64>, String> {
    if let Some((a, b)) = s.split_once('-') {
        let lo: u64 = a.parse().map_err(|_| format!("ogiltigt frö: {a}"))?;
        let hi: u64 = b.parse().map_err(|_| format!("ogiltigt frö: {b}"))?;
        if lo > hi {
            return Err(format!("tomt fröspann {lo}..{hi}"));
        }
        Ok((lo..=hi).collect())
    } else {
        let x: u64 = s.parse().map_err(|_| format!("ogiltigt frö: {s}"))?;
        Ok(vec![x])
    }
}

fn bootstrap_ci(vals: &[f64]) -> (f64, f64, f64) {
    let rena: Vec<f64> = vals.iter().copied().filter(|x| x.is_finite()).collect();
    if rena.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let med = median(rena.clone());
    let mut rng = Rng::new(2059);
    let mut boots = Vec::with_capacity(2000);
    let n = rena.len();
    for _ in 0..2000 {
        let mut samp = Vec::with_capacity(n);
        for _ in 0..n {
            samp.push(rena[rng.under(n)]);
        }
        boots.push(samp.iter().sum::<f64>() / n as f64);
    }
    boots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo = boots[(0.025 * (boots.len() - 1) as f64).round() as usize];
    let hi = boots[(0.975 * (boots.len() - 1) as f64).round() as usize];
    (med, lo, hi)
}

fn bygg_varld_2059(fro: u64) -> Result<Sim, String> {
    let arm = utforsk_armar()
        .into_iter()
        .find(|a| a.namn == "e_lar" && !a.spegel)
        .ok_or("armen e_lar saknas")?;
    let rot_buf = if let Ok(p) = std::env::var("HCSIM_ROT") {
        PathBuf::from(p)
    } else if Path::new("../paleokarta/data").exists() {
        PathBuf::from(".")
    } else {
        PathBuf::from("/home/hakan/sandbox/pi-vmodell3/n10/hcsim")
    };
    let rot = rot_buf.as_path();
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv"))
        .map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    let mut sim = bygg_sim_balte(&kalla, &tt, B_FULL, H_FULL, BALTE_K, fro, &arm)?;
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    satt_rattningar_utdod(&mut sim);
    Ok(sim)
}

/// Bygger Hell Creek-världen med SIMULERINGENs flodversion 2, 6 växtgrupper och låsta `atervaxt.tsv` (`Alt 1954` + `Alt 1955` / `Alt 2238`).
fn bygg_varld_2238(fro: u64) -> Result<Sim, String> {
    use hckarta::{karta_full_version, Omrade};
    use hcsim::sim::MarkLage;
    use hcsim::svep::{LAT0, LON0, RUTA_M, T_KARTA};

    let arm = utforsk_armar()
        .into_iter()
        .find(|a| a.namn == "e_lar" && !a.spegel)
        .ok_or("armen e_lar saknas")?;
    let rot_buf = if let Ok(p) = std::env::var("HCSIM_ROT") {
        PathBuf::from(p)
    } else if Path::new("../paleokarta/data").exists() {
        PathBuf::from(".")
    } else {
        PathBuf::from("/home/hakan/sandbox/pi-vmodell3/n10/hcsim")
    };
    let rot = rot_buf.as_path();
    let kalla = Kalla::las(&rot.join("../paleokarta/data")).map_err(|e| format!("karta: {e}"))?;
    let tt_text = std::fs::read_to_string(rot.join("../hckarta/tidtabell.tsv"))
        .map_err(|e| format!("tidtabell: {e}"))?;
    let tt = Tidtabell::las(&tt_text).map_err(|e| format!("tidtabell: {e}"))?;
    let k = BALTE_K;
    let lat0 = LAT0 - (k as f64 * RUTA_M) / 111_320.0;
    let lon0 = LON0 - (k as f64 * RUTA_M) / (111_320.0 * lat0.to_radians().cos());
    let (b, h) = (B_FULL + 2 * k, H_FULL + 2 * k);
    let omr = Omrade { lat0, lon0, b, h, ruta_m: RUTA_M };
    let (kart, andelar_opt) = karta_full_version(&kalla, &tt, &omr, T_KARTA, fro, 2, 1)?;
    let andelar = andelar_opt.ok_or("flodversion 2 saknade FlodAndelar")?;
    let n0 = ((b * h) * 2000) / (B_FULL * H_FULL);
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    sim.mark_lage = MarkLage::Noll;
    sim.hjarn_arm = Some(arm);
    sim.kant = KantLage::Torus;
    sim.fonster_k = BALTE_K;
    sim.turordning = false;
    sim.flytt_i_tur = false;
    sim.bete_i_tur = false;
    satt_rattningar_utdod(&mut sim);
    sim.applicera_atervaxt_2238(&kart, &andelar)?;
    Ok(sim)
}

fn avstand_torus(sim: &Sim, a: usize, b: usize) -> usize {
    let (da, db) = (&sim.djur[a], &sim.djur[b]);
    let dx = (da.i as i64 - db.i as i64).unsigned_abs() as usize;
    let dy = (da.j as i64 - db.j as i64).unsigned_abs() as usize;
    dx.min(sim.b - dx).max(dy.min(sim.h - dy))
}

fn kor_hj2059_diag(sim: &Sim, fro: u64) -> (f64, f64, f64, f64, f64) {
    let mut alla: Vec<usize> = (0..sim.djur.len())
        .filter(|&i| {
            let d = &sim.djur[i];
            d.lev
                && d.age >= 1500
                && d.hj2059
                    .as_ref()
                    .is_some_and(|h| h.steg >= 500 && h.skala_init && h.z_skala_init)
        })
        .collect();
    let mut rng = Rng::new(fro ^ 0x2059);
    rng.blanda(&mut alla);
    let mut kand = Vec::new();
    for i in alla {
        if kand.len() >= 20 {
            break;
        }
        if kand.iter().all(|&v| avstand_torus(sim, v, i) >= 7) {
            kand.push(i);
        }
    }
    kand.sort_unstable();
    if kand.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }

    let uids: Vec<u64> = kand.iter().map(|&i| sim.djur[i].uid).collect();
    let hjs: Vec<Hj2008> = kand.iter().map(|&i| (**sim.djur[i].hj2059.as_ref().unwrap()).clone()).collect();
    let mut z0_v = Vec::with_capacity(kand.len());
    let mut y_v = Vec::with_capacity(kand.len());
    for (n, &idx) in kand.iter().enumerate() {
        let s0 = sim.sinnen_for(idx);
        let s0_sk = hjs[n].skala_grind(&s0);
        let z0 = hjs[n].kod_online(&s0_sk);
        y_v.push((0..A_N).map(|a| hjs[n].forutsag(&z0, a).to_vec()).collect::<Vec<_>>());
        z0_v.push(z0);
    }

    let mut zn_v: Vec<Vec<Option<Vec<f64>>>> = vec![vec![None; kand.len()]; A_N];
    for a in 0..A_N {
        let mut k = sim.clone();
        k.tvinga_grupp = Some((kand.clone(), a));
        k.steg();
        k.tvinga_grupp = None;
        let mut vaxt = false;
        for (n, &idx) in kand.iter().enumerate() {
            if k.djur[idx].lev && k.djur[idx].uid == uids[n] {
                let s2 = if vaxt {
                    k.sinnen_for(idx)
                } else {
                    vaxt = true;
                    k.sinnen_nasta_steg(idx)
                };
                let s2_sk = hjs[n].skala_grind(&s2);
                zn_v[a][n] = Some(hjs[n].kod_mal(&s2_sk).to_vec());
            }
        }
    }

    let mut summa = Summa::default();
    let mut gv_v = Vec::new();
    let mut gd_v = Vec::new();
    let mut ge_v = Vec::new();
    for n in 0..kand.len() {
        if (0..A_N).any(|a| zn_v[a][n].is_none()) {
            continue;
        }
        let zn_alla: Vec<Vec<f64>> = (0..A_N).map(|a| zn_v[a][n].as_ref().unwrap().clone()).collect();
        summa.lagg(&bidrag(&y_v[n], &zn_alla, &z0_v[n]));
        let gv = hjs[n].aktuell_kanal_vikt();
        gv_v.push((0..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0);
        gd_v.push((1..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0);
        ge_v.push(gv[S_N - 1]);
    }
    if summa.prov == 0 {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }
    (
        summa.r2(),
        summa.kvot(),
        median(gv_v),
        median(gd_v),
        median(ge_v),
    )
}

fn sparb_varld(fro: u64, inkor_ar: i64, mat_ar: i64) -> Result<(u64, Vec<SparbArmUtfall>), String> {
    let t0 = std::time::Instant::now();
    let mut sim_bas = bygg_varld_2059(fro)?;
    let inkor_steg = inkor_ar * STEG_PER_AR;
    for _ in 0..inkor_steg {
        sim_bas.steg();
    }
    let bas_pop = sim_bas.antal();
    live_logg_med(
        EGEN_LOGG_2059,
        "Alt 2059",
        &format!("Frö {fro}: inkörning {inkor_ar} år klar ({bas_pop} djur, {:.1}s)", t0.elapsed().as_secs_f64()),
    );

    let mat_steg = mat_ar * STEG_PER_AR;
    let halva = mat_steg / 2;
    let mut utfall = Vec::with_capacity(SPARB_ARMAR.len());

    for &(arm_namn, formaga) in &SPARB_ARMAR {
        let mut sim = sim_bas.clone();
        sim.hjarn_arm = Some(HjarnArm {
            namn: Box::leak(arm_namn.to_string().into_boxed_str()),
            markta: formaga,
            omarkta: formaga,
            spegel: false,
        });
        sim.markering_skedd = true;
        for d in sim.djur.iter_mut().filter(|d| d.lev) {
            d.formaga = formaga;
            d.hj2059 = None;
            d.tabell = Minnestabell::ny();
        }

        let (mut sum_intag, mut n_steg) = (0.0f64, 0u64);
        let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
        let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
        let (mut hj_r2, mut hj_fk, mut gv, mut gd, mut ge) =
            (f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN);

        for t in 0..mat_steg {
            if t == halva {
                bas_svalt = sim.rak.dod_svalt;
                bas_alder = sim.rak.dod_alder;
            }
            // Årlig Allvetande-träff (engångsprov utan att störa djur):
            let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
            if t >= halva && t % STEG_PER_AR == 0 {
                for (idx, d) in sim.djur.iter().enumerate() {
                    if d.lev {
                        let a_allv = sim.allvetande_handling(d.i, d.j);
                        let (di, dj) = GRANNAR[a_allv];
                        let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                        if ni >= 0 && nj >= 0 {
                            mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                            if mal_par.len() >= 80 {
                                break;
                            }
                        }
                    }
                }
            }
            if t == mat_steg - 300 && formaga.cfg_2059().is_some() {
                (hj_r2, hj_fk, gv, gd, ge) = kor_hj2059_diag(&sim, fro);
            }
            sim.steg();

            for (idx, uid, mal) in mal_par {
                let d = &sim.djur[idx];
                if d.lev && d.uid == uid {
                    allv_tot += 1;
                    if (d.i, d.j) == mal {
                        allv_ratt += 1;
                    }
                }
            }
            if t >= halva {
                for d in sim.djur.iter().filter(|d| d.lev) {
                    if let Some(x) = d.forra_intag {
                        sum_intag += x;
                        n_steg += 1;
                    }
                }
            }
        }

        let lev_vec: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
        let slutpop = lev_vec.len() as f64;
        let medelenergi = if lev_vec.is_empty() { 0.0 } else { lev_vec.iter().sum::<f64>() / slutpop };
        let ds = sim.rak.dod_svalt - bas_svalt;
        let da = sim.rak.dod_alder - bas_alder;
        let svalt_andel = if ds + da == 0 { 0.0 } else { ds as f64 / (ds + da) as f64 };
        let u = SparbArmUtfall {
            medelintag: sum_intag / n_steg.max(1) as f64,
            slutpop,
            medelenergi,
            svalt_andel,
            allv_traff: allv_ratt as f64 / allv_tot.max(1) as f64,
            hj_r2,
            hj_fk,
            g_vaxt: gv,
            g_djur: gd,
            g_energi: ge,
        };
        if matches!(arm_namn, "Lar" | "Lar2059") {
            live_logg_med(
                EGEN_LOGG_2059,
                "Alt 2059",
                &format!(
                    "Frö {fro} {arm_namn}: intag={:.3} pop={:.0} ({:.1}s)",
                    u.medelintag,
                    u.slutpop,
                    t0.elapsed().as_secs_f64()
                ),
            );
        }
        utfall.push(u);
    }

    live_logg_med(
        EGEN_LOGG_2059,
        "Alt 2059",
        &format!(
            "Spår B frö {fro} ({:.1}s): Allv {:.3}/pop{:.0} Slump {:.3}/pop{:.0} | Lar {:.3}/pop{:.0} | Lar2059 {:.3}/pop{:.0} hjR²={} fk={} g(v/d/E)={}/{}/{} | Frus2059 {:.3}/pop{:.0} Stryk2059 {:.3}/pop{:.0}",
            t0.elapsed().as_secs_f64(),
            utfall[0].medelintag, utfall[0].slutpop,
            utfall[3].medelintag, utfall[3].slutpop,
            utfall[5].medelintag, utfall[5].slutpop,
            utfall[6].medelintag, utfall[6].slutpop,
            f3(utfall[6].hj_r2), f3(utfall[6].hj_fk),
            f3(utfall[6].g_vaxt), f3(utfall[6].g_djur), f3(utfall[6].g_energi),
            utfall[7].medelintag, utfall[7].slutpop,
            utfall[8].medelintag, utfall[8].slutpop,
        ),
    );
    Ok((fro, utfall))
}

fn kora_sparb(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    live_logg_med(
        EGEN_LOGG_2059,
        "Alt 2059",
        &format!("Startar Alt 2059 Spår B på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)", fron.len()),
    );
    let mut svar = Vec::with_capacity(fron.len());
    for del in fron.chunks(tradar.max(1)) {
        let res: Vec<Result<_, String>> = std::thread::scope(|s| {
            let h: Vec<_> = del.iter().map(|&f| s.spawn(move || sparb_varld(f, inkor_ar, mat_ar))).collect();
            h.into_iter().map(|x| x.join().unwrap_or_else(|_| Err("tråd föll".into()))).collect()
        });
        for r in res {
            svar.push(r?);
        }
    }
    svar.sort_by_key(|x| x.0);

    let n = svar.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2059 Spår B (levande simulering, {} världar): EKOLOGISKT UTFALL + VÄRLDSMODELL\n", n);
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2059-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`); mätning sista 10 åren (`hjR²` vid steg `2 700`); krav {krav} av {n}.\n", mat_ar * STEG_PER_AR);

    let _ = writeln!(t, "## 1. Alla 9 armar (median [95 %-bootstrap-intervall] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff |\n|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci(&svar.iter().map(|v| v.1[ai].medelintag).collect::<Vec<_>>());
        let (pm, pl, ph) = bootstrap_ci(&svar.iter().map(|v| v.1[ai].slutpop).collect::<Vec<_>>());
        let (em, el, eh) = bootstrap_ci(&svar.iter().map(|v| v.1[ai].medelenergi).collect::<Vec<_>>());
        let (sm, sl, sh) = bootstrap_ci(&svar.iter().map(|v| v.1[ai].svalt_andel).collect::<Vec<_>>());
        let (am, al, ah) = bootstrap_ci(&svar.iter().map(|v| v.1[ai].allv_traff).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah)
        );
    }

    let _ = writeln!(t, "\n## 2. Världsmodell-mått i levande värld (`steg 2 700`, median över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (handl. verkan) | Felkvot | Blinda grindvikter (växt / djur / energi) |\n|---|---|---|---|");
    for ai in [6usize, 7, 8] {
        let namn = SPARB_ARMAR[ai].0;
        let r2 = median(svar.iter().map(|v| v.1[ai].hj_r2).collect());
        let fk = median(svar.iter().map(|v| v.1[ai].hj_fk).collect());
        let gv = median(svar.iter().map(|v| v.1[ai].g_vaxt).collect());
        let gd = median(svar.iter().map(|v| v.1[ai].g_djur).collect());
        let ge = median(svar.iter().map(|v| v.1[ai].g_energi).collect());
        let _ = writeln!(t, "| {namn} | {} | {} | {} / {} / {} |", f3(r2), f3(fk), f3(gv), f3(gd), f3(ge));
    }

    let vinst = |a: usize, b: usize| -> (usize, usize) {
        let vi = svar.iter().filter(|v| v.1[a].medelintag > v.1[b].medelintag).count();
        let vp = svar.iter().filter(|v| v.1[a].slutpop > v.1[b].slutpop).count();
        (vi, vp)
    };

    let _ = writeln!(t, "\n## 3. Förgrind (tak mot `Slump`) och parade jämförelser\n");
    for &(namn, ai) in &[("Allvetande", 0usize), ("Allvetande10", 1), ("TrangselOrakel", 2), ("Tabell", 4), ("Lar", 5)] {
        let (vi, vp) = vinst(ai, 3);
        let _ = writeln!(t, "- {namn} > Slump: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
    }
    let mut lar2059_gron_bet = true;
    for &(mot_namn, bi) in &[("Lar", 5usize), ("Frusen2059", 7), ("Stryk2059", 8), ("Slump", 3), ("Tabell", 4)] {
        let (vi, vp) = vinst(6, bi);
        let _ = writeln!(t, "- Lar2059 > {mot_namn}: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
        if bi != 4 && (vi < krav || vp < krav) {
            lar2059_gron_bet = false;
        }
    }
    let forgrind_gron = [0usize, 1, 2].iter().any(|&ai| {
        let (vi, vp) = vinst(ai, 3);
        vi >= krav && vp >= krav
    });
    let hjr2_pos = svar.iter().filter(|v| v.1[6].hj_r2 > 0.0).count();
    let hjfk_med = median(svar.iter().map(|v| v.1[6].hj_fk).collect());
    let wm_gron = hjr2_pos >= krav && hjfk_med < 1.0;

    let _ = writeln!(t, "\n## 4. Automatisk grinddom\n");
    let _ = writeln!(t, "- **Förgrind (tak > Slump i ≥ {krav}/{n}):** **{}**", if forgrind_gron { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Världsmodell (`Lar2059` `hjR² > 0` i ≥ {krav}/{n} & `fk < 1,0`):** **{hjr2_pos}/{n}**, fk median **{}** -> **{}**", f3(hjfk_med), if wm_gron { "GRÖNT" } else { "RÖTT" });
    let _ = writeln!(t, "- **Ekologiskt beteende (`Lar2059` > Lar, Frusen2059, Stryk2059, Slump i ≥ {krav}/{n}):** **{}**", if lar2059_gron_bet { "GRÖNT (behöver bekräftelse på 281–300)" } else { "RÖTT" });

    live_logg_med(
        EGEN_LOGG_2059,
        "Alt 2059",
        &format!("KLAR Spår B: förgrind={}, hjR²={hjr2_pos}/{n} (fk={}), beteende={}", if forgrind_gron {"PASS"} else {"FAIL"}, f3(hjfk_med), if lar2059_gron_bet {"GRÖNT"} else {"RÖTT"}),
    );
    Ok(t)
}

#[derive(Clone, Default)]
struct Diag2059Rad {
    namn: &'static str,
    medelintag: f64,
    slutpop: usize,
    andel_stanna: f64,
    andel_samma: f64,
    vaxt_diff: f64,
    djur_diff: f64,
    djur_per_ruta: f64,
    aktiva_grind: f64,
    topp1_andel: f64,
}

fn kora_diag2059(fro: u64) -> Result<String, String> {
    let t0 = std::time::Instant::now();
    live_logg_med(EGEN_LOGG_2059, "Alt 2059", &format!("Startar diag2059 på frö {fro}"));
    let mut sim_bas = bygg_varld_2059(fro)?;
    let inkor_steg = 10 * STEG_PER_AR;
    for _ in 0..inkor_steg {
        sim_bas.steg();
    }
    let mat_steg = 20 * STEG_PER_AR;
    let halva = mat_steg / 2;

    let armar: [(&'static str, Formaga); 5] = [
        ("TrangselOrakel", Formaga::TrangselOrakel),
        ("Slump", Formaga::Slump),
        ("Lar", Formaga::Lar),
        ("Lar2059", Formaga::Lar2059),
        ("Stryk2059", Formaga::Stryk2059),
    ];

    let bas_ref = &sim_bas;
    let rader: Vec<Diag2059Rad> = std::thread::scope(|s| {
        let handles: Vec<_> = armar
            .iter()
            .map(|&(namn, formaga)| {
                s.spawn(move || {
                    let mut sim = bas_ref.clone();
                    sim.hjarn_arm = Some(HjarnArm {
                        namn,
                        markta: formaga,
                        omarkta: formaga,
                        spegel: false,
                    });
                    sim.markering_skedd = true;
                    for d in sim.djur.iter_mut().filter(|d| d.lev) {
                        d.formaga = formaga;
                        d.hj2059 = None;
                        d.tabell = Minnestabell::ny();
                    }
                    let n_celler = sim.v.len();
                    let hb = (sim.b as i32) / 2;
                    let hh = (sim.h as i32) / 2;
                    let mut forra_a: HashMap<u64, usize> = HashMap::new();
                    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
                    let (mut n_steg, mut n_stanna, mut n_par, mut n_samma) = (0u64, 0u64, 0u64, 0u64);
                    let (mut sum_vdiff, mut sum_ddiff) = (0.0f64, 0.0f64);
                    let (mut aktiva_grind, mut topp1_andel) = (f64::NAN, f64::NAN);

                    for t in 0..mat_steg {
                        if t == mat_steg - 300 && formaga.cfg_2059().is_some() {
                            let mut act_v = Vec::new();
                            let mut top_v = Vec::new();
                            for d in sim.djur.iter().filter(|d| d.lev && d.age >= 1500) {
                                if let Some(ref h) = d.hj2059 {
                                    act_v.push(h.grind_n.iter().filter(|&&n| n >= 5.0).count() as f64);
                                    let sum_n: f64 = h.grind_n.iter().sum();
                                    if sum_n > 0.0 {
                                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                                        top_v.push(max_n / sum_n);
                                    }
                                }
                            }
                            if !act_v.is_empty() {
                                aktiva_grind = median(act_v);
                                topp1_andel = median(top_v);
                            }
                        }

                        let mut antal_fore = Vec::new();
                        let mut v_norm_fore = Vec::new();
                        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
                        if t >= halva {
                            antal_fore.resize(n_celler, 0);
                            v_norm_fore = sim
                                .v
                                .iter()
                                .zip(sim.vmax.iter())
                                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                                .collect();
                            for d in sim.djur.iter().filter(|d| d.lev) {
                                antal_fore[sim.cell(d.i, d.j)] += 1;
                                fore_pos.insert(d.uid, (d.i, d.j));
                            }
                        }

                        sim.steg();

                        if t >= halva {
                            for d in sim.djur.iter().filter(|d| d.lev) {
                                if let Some(x) = d.forra_intag {
                                    sum_intag += x;
                                    n_intag += 1;
                                }
                                if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                                    let mut di = d.i as i32 - i0 as i32;
                                    if di > hb {
                                        di -= sim.b as i32;
                                    } else if di < -hb {
                                        di += sim.b as i32;
                                    }
                                    let mut dj = d.j as i32 - j0 as i32;
                                    if dj > hh {
                                        dj -= sim.h as i32;
                                    } else if dj < -hh {
                                        dj += sim.h as i32;
                                    }
                                    if let Some(a) = GRANNAR.iter().position(|&p| p == (di, dj)) {
                                        n_steg += 1;
                                        if a == 0 {
                                            n_stanna += 1;
                                        }
                                        if let Some(&prev_a) = forra_a.get(&d.uid) {
                                            n_par += 1;
                                            if prev_a == a {
                                                n_samma += 1;
                                            }
                                        }
                                        forra_a.insert(d.uid, a);

                                        let mut sv = 0.0;
                                        let mut sd = 0.0;
                                        for &(gdi, gdj) in &GRANNAR {
                                            let (ni, nj) = sim.kant_granne(i0 as i32 + gdi, j0 as i32 + gdj);
                                            if ni >= 0 && nj >= 0 && (ni as usize) < sim.b && (nj as usize) < sim.h {
                                                let c = sim.cell(ni as usize, nj as usize);
                                                sv += v_norm_fore[c];
                                                sd += antal_fore[c] as f64;
                                            }
                                        }
                                        let c_ny = sim.cell(d.i, d.j);
                                        sum_vdiff += v_norm_fore[c_ny] - sv / 9.0;
                                        sum_ddiff += antal_fore[c_ny] as f64 - sd / 9.0;
                                    }
                                }
                            }
                        }
                    }

                    let mut besatta = vec![false; n_celler];
                    let mut slutpop = 0usize;
                    for d in sim.djur.iter().filter(|d| d.lev) {
                        slutpop += 1;
                        besatta[sim.cell(d.i, d.j)] = true;
                    }
                    let n_besatta = besatta.iter().filter(|&&b| b).count();
                    Diag2059Rad {
                        namn,
                        medelintag: sum_intag / n_intag.max(1) as f64,
                        slutpop,
                        andel_stanna: n_stanna as f64 / n_steg.max(1) as f64,
                        andel_samma: n_samma as f64 / n_par.max(1) as f64,
                        vaxt_diff: sum_vdiff / n_steg.max(1) as f64,
                        djur_diff: sum_ddiff / n_steg.max(1) as f64,
                        djur_per_ruta: slutpop as f64 / n_besatta.max(1) as f64,
                        aktiva_grind,
                        topp1_andel,
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2059 mekanismkontroll på frö {fro} (sista 10 åren `t = 1500..3000`)\n");
    let _ = writeln!(
        t,
        "| Arm | Medelintag | Slutpop | Andel stanna (a=0) | Andel samma riktning | Växt-diff (vald − medel9) | Djur-diff (vald − medel9) | Djur/besatt ruta (slut) | Aktiva grind-handl. (av 9) | Topp-1 handlingsandel |\n|---|---|---|---|---|---|---|---|---|---|"
    );
    for r in &rader {
        let akt = if r.aktiva_grind.is_finite() { format!("{:.1}", r.aktiva_grind) } else { "–".into() };
        let top = if r.topp1_andel.is_finite() { format!("{:.3}", r.topp1_andel) } else { "–".into() };
        let _ = writeln!(
            t,
            "| {} | {:.4} | {} | {:.3} | {:.3} | {:+.4} | {:+.4} | {:.2} | {} | {} |",
            r.namn, r.medelintag, r.slutpop, r.andel_stanna, r.andel_samma, r.vaxt_diff, r.djur_diff, r.djur_per_ruta, akt, top
        );
    }
    live_logg_med(
        EGEN_LOGG_2059,
        "Alt 2059",
        &format!("KLAR diag2059 frö {fro} ({:.1}s)", t0.elapsed().as_secs_f64()),
    );
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2073 Spår B

const SPARB2073_ARMAR: [(&'static str, Formaga); 8] = [
    ("TrangselOrakel", Formaga::TrangselOrakel),
    ("Slump", Formaga::Slump),
    ("Stanna68", Formaga::StannaSlump(0.68)),
    ("Lar", Formaga::Lar),
    ("Lar2059", Formaga::Lar2059),
    ("Lar2073_05", Formaga::Lar2073(0.05)),
    ("Lar2073_10", Formaga::Lar2073(0.10)),
    ("Stryk2059", Formaga::Stryk2059),
];

#[derive(Clone, Default)]
struct Sparb2073Utfall {
    medelintag: f64,
    slutpop: f64,
    medelenergi: f64,
    svalt_andel: f64,
    allv_traff: f64,
    andel_stanna: f64,
    vaxt_diff: f64,
    hj_r2: f64,
    hj_fk: f64,
    g_vaxt: f64,
    g_djur: f64,
    g_energi: f64,
    topp1_andel: f64,
}

fn bootstrap_ci_seed(vals: &[f64], seed: u64) -> (f64, f64, f64) {
    let rena: Vec<f64> = vals.iter().copied().filter(|x| x.is_finite()).collect();
    if rena.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    let med = median(rena.clone());
    let mut rng = Rng::new(seed);
    let mut boots = Vec::with_capacity(2000);
    let n = rena.len();
    for _ in 0..2000 {
        let mut samp = Vec::with_capacity(n);
        for _ in 0..n {
            samp.push(rena[rng.under(n)]);
        }
        boots.push(samp.iter().sum::<f64>() / n as f64);
    }
    boots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lo = boots[(0.025 * (boots.len() - 1) as f64).round() as usize];
    let hi = boots[(0.975 * (boots.len() - 1) as f64).round() as usize];
    (med, lo, hi)
}

fn klocka_unix_nu() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

const SPARB2116_ARMAR: [(&'static str, Formaga); 8] = [
    ("TrangselOrakel", Formaga::TrangselOrakel),
    ("Slump", Formaga::Slump),
    ("Stanna68", Formaga::StannaSlump(0.68)),
    ("Lar", Formaga::Lar),
    ("Lar2059", Formaga::Lar2059),
    ("Lar2116_S20", Formaga::Lar2073(0.20)),
    ("Lar2116_M20", Formaga::Lar2116Minst(0.20)),
    ("Stryk2059", Formaga::Stryk2059),
];

fn format_forlopp(progress: Option<&(AtomicUsize, usize, std::time::Instant)>) -> String {
    let Some((klara, tot, t_start)) = progress else {
        return String::new();
    };
    let kl = klara.fetch_add(1, Ordering::Relaxed) + 1;
    let kort_min = t_start.elapsed().as_secs_f64() / 60.0;
    let kvar_min = if *tot > kl {
        kort_min * (*tot - kl) as f64 / kl as f64
    } else {
        0.0
    };
    format!(" [klara {kl}/{tot}, kört {kort_min:.1} min, kvar ~{kvar_min:.1} min]")
}

fn sparb_gem_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    formaga: Formaga,
    mat_ar: i64,
    kartbild_ut: Option<&Path>,
    egen_logg: &str,
    tagg: &str,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> Sparb2073Utfall {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta: formaga,
        omarkta: formaga,
        spegel: false,
    });
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut().filter(|d| d.lev) {
        d.formaga = formaga;
        d.hj2059 = None;
        d.tabell = Minnestabell::ny();
    }

    let kb_rubrik = format!("Alt 2073 ({arm_namn}: blind KanalGrind + z-norm + RLS), frö {fro}, torus + bälte {BALTE_K}");
    let mut kb_db = if let Some(ut) = kartbild_ut {
        if let Some(p) = ut.parent() {
            let _ = create_dir_all(p);
        }
        let _ = skriv_bild(&sim, &kb_rubrik, ut);
        let _ = std::fs::remove_file(historik_vag(ut));
        Skrivare::ny(&db_vag(ut), &fast_ur_sim(&sim, &kb_rubrik)).ok().and_then(|mut db| {
            let _ = db.lagg_till(&lage_ur_sim(&sim, klocka_unix_nu()));
            Some(db)
        })
    } else {
        None
    };
    let mut kb_hist_s = 10u64;
    let mut kb_senast = std::time::Instant::now();

    let mat_steg = mat_ar * STEG_PER_AR;
    let halva = mat_steg / 2;
    let hb = (sim.b as i32) / 2;
    let hh = (sim.h as i32) / 2;

    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
    let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
    let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
    let (mut n_stanna_prov, mut n_steg_prov, mut sum_vdiff) = (0u64, 0u64, 0.0f64);
    let (mut hj_r2, mut hj_fk, mut gv, mut gd, mut ge, mut topp1) =
        (f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN);

    for t in 0..mat_steg {
        if t == halva {
            bas_svalt = sim.rak.dod_svalt;
            bas_alder = sim.rak.dod_alder;
        }
        let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
        if t >= halva && t % STEG_PER_AR == 0 {
            for (idx, d) in sim.djur.iter().enumerate() {
                if d.lev {
                    let a_allv = sim.allvetande_handling(d.i, d.j);
                    let (di, dj) = GRANNAR[a_allv];
                    let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                    if ni >= 0 && nj >= 0 {
                        mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                        if mal_par.len() >= 80 {
                            break;
                        }
                    }
                }
            }
        }
        if t == mat_steg - 300 && formaga.cfg_2059().is_some() {
            (hj_r2, hj_fk, gv, gd, ge) = kor_hj2059_diag(&sim, fro);
            let mut top_v = Vec::new();
            for d in sim.djur.iter().filter(|d| d.lev && d.age >= 1500) {
                if let Some(ref h) = d.hj2059 {
                    let sum_n: f64 = h.grind_n.iter().sum();
                    if sum_n > 0.0 {
                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                        top_v.push(max_n / sum_n);
                    }
                }
            }
            if !top_v.is_empty() {
                topp1 = median(top_v);
            }
        }

        let stickprov = t >= halva && t % 10 == 0;
        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
        let mut v_norm_fore: Vec<f64> = Vec::new();
        if stickprov {
            v_norm_fore = sim
                .v
                .iter()
                .zip(sim.vmax.iter())
                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                .collect();
            for d in sim.djur.iter().filter(|d| d.lev) {
                fore_pos.insert(d.uid, (d.i, d.j));
            }
        }

        sim.steg();
        if let Some(ut) = kartbild_ut {
            if (t + 1) % 30 == 0 {
                let _ = skriv_bild(&sim, &kb_rubrik, ut);
            }
            if let Some(ref mut db) = kb_db {
                if kb_senast.elapsed().as_secs() >= kb_hist_s {
                    if db.lagg_till(&lage_ur_sim(&sim, klocka_unix_nu())).unwrap_or(false) {
                        kb_hist_s *= 2;
                    }
                    kb_senast = std::time::Instant::now();
                }
            }
        }

        for (idx, uid, mal) in mal_par {
            let d = &sim.djur[idx];
            if d.lev && d.uid == uid {
                allv_tot += 1;
                if (d.i, d.j) == mal {
                    allv_ratt += 1;
                }
            }
        }
        if t >= halva {
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some(x) = d.forra_intag {
                    sum_intag += x;
                    n_intag += 1;
                }
                if stickprov {
                    if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                        let mut di = d.i as i32 - i0 as i32;
                        if di > hb {
                            di -= sim.b as i32;
                        } else if di < -hb {
                            di += sim.b as i32;
                        }
                        let mut dj = d.j as i32 - j0 as i32;
                        if dj > hh {
                            dj -= sim.h as i32;
                        } else if dj < -hh {
                            dj += sim.h as i32;
                        }
                        if di.abs() <= 1 && dj.abs() <= 1 {
                            n_steg_prov += 1;
                            if di == 0 && dj == 0 {
                                n_stanna_prov += 1;
                            }
                            let mut sv = 0.0;
                            for &(gdi, gdj) in &GRANNAR {
                                let (ni, nj) = sim.kant_granne(i0 as i32 + gdi, j0 as i32 + gdj);
                                if ni >= 0 && nj >= 0 && (ni as usize) < sim.b && (nj as usize) < sim.h {
                                    sv += v_norm_fore[sim.cell(ni as usize, nj as usize)];
                                }
                            }
                            let c_ny = sim.cell(d.i, d.j);
                            sum_vdiff += v_norm_fore[c_ny] - sv / 9.0;
                        }
                    }
                }
            }
        }
    }

    let lev_vec: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
    let slutpop = lev_vec.len() as f64;
    let medelenergi = if lev_vec.is_empty() { 0.0 } else { lev_vec.iter().sum::<f64>() / slutpop };
    let ds = sim.rak.dod_svalt - bas_svalt;
    let da = sim.rak.dod_alder - bas_alder;
    let u = Sparb2073Utfall {
        medelintag: sum_intag / n_intag.max(1) as f64,
        slutpop,
        medelenergi,
        svalt_andel: if ds + da == 0 { 0.0 } else { ds as f64 / (ds + da) as f64 },
        allv_traff: allv_ratt as f64 / allv_tot.max(1) as f64,
        andel_stanna: n_stanna_prov as f64 / n_steg_prov.max(1) as f64,
        vaxt_diff: sum_vdiff / n_steg_prov.max(1) as f64,
        hj_r2,
        hj_fk,
        g_vaxt: gv,
        g_djur: gd,
        g_energi: ge,
        topp1_andel: topp1,
    };

    let forlopp = format_forlopp(progress);
    if formaga.cfg_2059().is_some() {
        live_logg_med(
            egen_logg,
            tagg,
            &format!(
                "Frö {fro} {arm_namn}: intag={:.4} pop={:.0} stanna={:.3} vdiff={:+.4} hjR²={} fk={} g(v/d/E)={}/{}/{} top1={} ({:.1}s){forlopp}",
                u.medelintag, u.slutpop, u.andel_stanna, u.vaxt_diff,
                f3(u.hj_r2), f3(u.hj_fk), f3(u.g_vaxt), f3(u.g_djur), f3(u.g_energi), f3(u.topp1_andel),
                t0.elapsed().as_secs_f64(),
            ),
        );
    } else {
        live_logg_med(
            egen_logg,
            tagg,
            &format!(
                "Frö {fro} {arm_namn}: intag={:.4} pop={:.0} stanna={:.3} vdiff={:+.4} allv={:.3} ({:.1}s){forlopp}",
                u.medelintag, u.slutpop, u.andel_stanna, u.vaxt_diff, u.allv_traff,
                t0.elapsed().as_secs_f64(),
            ),
        );
    }
    u
}

fn kor_8armar_sparb(
    fron: &[u64],
    inkor_ar: i64,
    mat_ar: i64,
    tradar: usize,
    armar: &[(&'static str, Formaga); 8],
    kartbild_ut: Option<&Path>,
    egen_logg: &str,
    tagg: &str,
) -> Result<(Vec<Vec<Sparb2073Utfall>>, f64), String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        egen_logg,
        tagg,
        &format!(
            "Startar {tagg} Spår B på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            egen_logg,
                            tagg,
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    const ARM_ORDNING: [usize; 8] = [6, 5, 4, 7, 3, 0, 2, 1];
    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 8);
    for &ai in &ARM_ORDNING {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2073Utfall::default(); 8]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = armar[ai];
                let kb_fil = if fi == 0 && ai == 5 { kartbild_ut } else { None };
                let u = sparb_gem_en_arm(sim_bas, fro, arm_namn, formaga, mat_ar, kb_fil, egen_logg, tagg, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });
    Ok((grid, t_start.elapsed().as_secs_f64()))
}

fn skriv_gem_tabell12(
    t: &mut String,
    grid: &[Vec<Sparb2073Utfall>],
    armar: &[(&'static str, Formaga); 8],
    boot_seed: u64,
) {
    let n = grid.len();
    let _ = writeln!(t, "## 1. Alla 8 armar (median [95 %-bootstrap-intervall] över {n} världar)
");
    let _ = writeln!(
        t,
        "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna (a=0) | Växt-diff (vald − medel9) |
|---|---|---|---|---|---|---|---|"
    );
    for (ai, &(namn, _)) in armar.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), boot_seed);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), boot_seed);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), boot_seed);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), boot_seed);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), boot_seed);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), boot_seed);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), boot_seed);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0,
            f3(am), f3(al), f3(ah),
            stm, stl, sth,
            vdm, vdl, vdh
        );
    }
    let _ = writeln!(t, "
## 2. Världsmodell-mått i levande värld (`steg 2 700`, median över {n} världar)
");
    let _ = writeln!(t, "| Arm | hjR² (handl. verkan) | Felkvot | Blinda grindvikter (växt / djur / energi) | Topp-1 handlingsandel | hjR² > 0 |
|---|---|---|---|---|---|");
    for ai in [4usize, 5, 6, 7] {
        let namn = armar[ai].0;
        let r2 = median(grid.iter().map(|r| r[ai].hj_r2).collect());
        let fk = median(grid.iter().map(|r| r[ai].hj_fk).collect());
        let gv = median(grid.iter().map(|r| r[ai].g_vaxt).collect());
        let gd = median(grid.iter().map(|r| r[ai].g_djur).collect());
        let ge = median(grid.iter().map(|r| r[ai].g_energi).collect());
        let top1 = median(grid.iter().map(|r| r[ai].topp1_andel).collect());
        let pos = grid.iter().filter(|r| r[ai].hj_r2 > 0.0).count();
        let _ = writeln!(t, "| {namn} | {} | {} | {} / {} / {} | {} | {pos}/{n} |", f3(r2), f3(fk), f3(gv), f3(gd), f3(ge), f3(top1));
    }
}

fn kora_sparb2073(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize, kartbild_ut: Option<&Path>) -> Result<String, String> {
    let (grid, sec) = kor_8armar_sparb(fron, inkor_ar, mat_ar, tradar, &SPARB2073_ARMAR, kartbild_ut, EGEN_LOGG_2073, "Alt 2073")?;
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2073 Spår B (lätt utforskning + `Stanna68`-baslinje, {n} världar): UTFALL + MEKANISM
");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2073-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`); mätning sista 10 åren (`hjR²` vid steg `2 700`); krav {krav} av {n}.
",
        mat_ar * STEG_PER_AR
    );
    skriv_gem_tabell12(&mut t, &grid, &SPARB2073_ARMAR, 2073);



    let vinst = |a: usize, b: usize| -> (usize, usize) {
        let vi = grid.iter().filter(|r| r[a].medelintag > r[b].medelintag).count();
        let vp = grid.iter().filter(|r| r[a].slutpop > r[b].slutpop).count();
        (vi, vp)
    };
    let r2_vinst = |a: usize, b: usize| -> usize {
        grid.iter().filter(|r| r[a].hj_r2 > r[b].hj_r2).count()
    };

    let _ = writeln!(t, "\n## 3. Förgrind och parade jämförelser\n");
    let (fg_i, fg_p) = vinst(0, 1);
    let _ = writeln!(t, "- TrangselOrakel > Slump (förgrind): intag **{fg_i}/{n}**, slutpop **{fg_p}/{n}**");
    for (label, ai) in [("Stanna68", 2usize), ("Lar", 3), ("Lar2059", 4), ("Lar2073_05", 5), ("Lar2073_10", 6)] {
        let (vi, vp) = vinst(ai, 1);
        let _ = writeln!(t, "- {label} > Slump: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
    }
    for (label, ai) in [("Lar2059", 4usize), ("Lar2073_05", 5), ("Lar2073_10", 6)] {
        for &(mot_namn, bi) in &[("Lar", 3usize), ("Stryk2059", 7), ("Slump", 1), ("Stanna68", 2)] {
            let (vi, vp) = vinst(ai, bi);
            let _ = writeln!(t, "- {label} > {mot_namn}: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
        }
    }
    for (label, ai) in [("Lar2073_05", 5usize), ("Lar2073_10", 6)] {
        let wr = r2_vinst(ai, 4);
        let _ = writeln!(t, "- {label} > Lar2059 på hjR²: **{wr}/{n}**");
    }

    let forgrind_gron = fg_i >= krav && fg_p >= krav;
    let wm_pass = |ai: usize| -> bool {
        let pos = grid.iter().filter(|r| r[ai].hj_r2 > 0.0).count();
        let r2_med = median(grid.iter().map(|r| r[ai].hj_r2).collect());
        let fk_med = median(grid.iter().map(|r| r[ai].hj_fk).collect());
        let gd_med = median(grid.iter().map(|r| r[ai].g_djur).collect());
        let gv_med = median(grid.iter().map(|r| r[ai].g_vaxt).collect());
        let ge_med = median(grid.iter().map(|r| r[ai].g_energi).collect());
        pos >= krav
            && r2_med >= 0.080
            && fk_med < 0.850
            && r2_vinst(ai, 4) >= krav
            && gd_med < 0.100
            && gd_med < gv_med.min(ge_med)
    };
    let wm_05 = wm_pass(5);
    let wm_10 = wm_pass(6);
    let wm_gron = wm_05 || wm_10;

    let bet_pass = |ai: usize| -> bool {
        [3usize, 1, 7, 2].iter().all(|&bi| {
            let (vi, vp) = vinst(ai, bi);
            vi >= krav && vp >= krav
        })
    };
    let bet_gron = bet_pass(5) || bet_pass(6);

    let _ = writeln!(t, "\n## 4. Automatisk grinddom\n");
    let _ = writeln!(t, "- **Förgrind (TrangselOrakel > Slump i ≥ {krav}/{n}):** **{}**", if forgrind_gron { "PASS" } else { "FAIL" });
    let _ = writeln!(
        t,
        "- **Världsmodell (utforskningslyft: `Lar2073_05`={} / `Lar2073_10`={}):** **{}**",
        if wm_05 { "PASS" } else { "FAIL" },
        if wm_10 { "PASS" } else { "FAIL" },
        if wm_gron { "GRÖNT" } else { "RÖTT" }
    );
    let _ = writeln!(
        t,
        "- **Ekologiskt beteende bortom `Stanna68` (`Lar2073_05`={} / `Lar2073_10`={}):** **{}**",
        if bet_pass(5) { "PASS" } else { "FAIL" },
        if bet_pass(6) { "PASS" } else { "FAIL" },
        if bet_gron { "GRÖNT (behöver bekräftelse på 281–300)" } else { "EJ BORTOM STANNA68" }
    );

    live_logg_med(
        EGEN_LOGG_2073,
        "Alt 2073",
        &format!(
            "KLAR Spår B ({:.1}s): förgrind={}, världsmodell={}, beteende_bortom_Stanna68={}",
            sec,
            if forgrind_gron { "PASS" } else { "FAIL" },
            if wm_gron { "GRÖNT" } else { "RÖTT" },
            if bet_gron { "GRÖNT" } else { "EJ_BORTOM" },
        ),
    );
    Ok(t)
}

fn kora_kartbild2073(
    arm_str: &str,
    fro: u64,
    var_steg: i64,
    historik_s: u64,
    ar: i64,
    ut: &Path,
) -> Result<(), String> {
    let (arm_namn, formaga): (&'static str, Formaga) = match arm_str {
        "lar2144_d2_a4" => ("Lar2144_D2_a4", Formaga::Lar2144D2(0.20, 40)),
        "lar2116_m20" => ("Lar2116_M20", Formaga::Lar2116Minst(0.20)),
        "lar2073_05" => ("Lar2073_05", Formaga::Lar2073(0.05)),
        "lar2073_10" => ("Lar2073_10", Formaga::Lar2073(0.10)),
        "lar2059" => ("Lar2059", Formaga::Lar2059),
        _ => return Err(format!("okänd arm för kartbild2073: {arm_str}")),
    };
    if let Some(p) = ut.parent() {
        let _ = create_dir_all(p);
    }
    let mut sim = bygg_varld_2059(fro)?;
    let rubrik = if arm_str == "lar2144_d2_a4" {
        format!(
            "Alt 2159 ({arm_namn}: 9 fristående modeller, riktad 20%), frö {fro}, torus + bälte {BALTE_K}"
        )
    } else if arm_str == "lar2116_m20" {
        format!(
            "Alt 2116 ({arm_namn}: blind KanalGrind + z-norm + RLS, riktad 20%), frö {fro}, torus + bälte {BALTE_K}"
        )
    } else {
        format!(
            "Alt 2073 ({arm_namn}: blind KanalGrind + z-norm + RLS), frö {fro}, torus + bälte {BALTE_K}"
        )
    };
    skriv_bild(&sim, &rubrik, ut)?;
    let _ = std::fs::remove_file(historik_vag(ut));
    let mut db = Skrivare::ny(&db_vag(ut), &fast_ur_sim(&sim, &rubrik))?;
    db.lagg_till(&lage_ur_sim(&sim, klocka_unix_nu()))?;

    let inkor_steg = 10 * STEG_PER_AR;
    let slut_steg = ar.max(1) * STEG_PER_AR;
    let mut hist_s = historik_s.max(1);
    let mut senast_hist = std::time::Instant::now();
    let var_st = var_steg.max(1);
    live_logg_med(
        EGEN_LOGG_2073,
        "Alt 2073",
        &format!("Startar kartbild2073 ({arm_namn}, frö {fro}, var {var_st} steg -> {})", ut.display()),
    );

    while sim.t < slut_steg {
        if sim.t == inkor_steg && !sim.markering_skedd {
            sim.hjarn_arm = Some(HjarnArm {
                namn: arm_namn,
                markta: formaga,
                omarkta: formaga,
                spegel: false,
            });
            sim.markering_skedd = true;
            for d in sim.djur.iter_mut().filter(|d| d.lev) {
                d.formaga = formaga;
                d.hj2059 = None;
                d.tabell = Minnestabell::ny();
            }
        }
        sim.steg();
        if sim.t % var_st == 0 {
            skriv_bild(&sim, &rubrik, ut)?;
        }
        if sim.antal() == 0 {
            live_logg_med(
                EGEN_LOGG_2073,
                "Alt 2073",
                &format!("kartbild2073: alla djur döda vid år {:.2}", sim.t as f64 / STEG_PER_AR as f64),
            );
            break;
        }
        if senast_hist.elapsed().as_secs() >= hist_s {
            if db.lagg_till(&lage_ur_sim(&sim, klocka_unix_nu()))? {
                hist_s *= 2;
            }
            senast_hist = std::time::Instant::now();
        }
    }
    skriv_bild(&sim, &rubrik, ut)?;
    db.lagg_till(&lage_ur_sim(&sim, klocka_unix_nu()))?;
    Ok(())
}

fn kora_sparb2116(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    let (grid, sec) = kor_8armar_sparb(fron, inkor_ar, mat_ar, tradar, &SPARB2116_ARMAR, None, EGEN_LOGG_2116, "Alt 2116")?;
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2116 Spår B (riktad minsta-besök-utforskning vs 20 % slump, {n} världar): UTFALL + MEKANISM
");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2116-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`); mätning sista 10 åren (`hjR²` vid steg `2 700`); krav {krav} av {n}.
",
        mat_ar * STEG_PER_AR
    );
    skriv_gem_tabell12(&mut t, &grid, &SPARB2116_ARMAR, 2116);

    let vinst = |a: usize, b: usize| -> (usize, usize) {
        let vi = grid.iter().filter(|r| r[a].medelintag > r[b].medelintag).count();
        let vp = grid.iter().filter(|r| r[a].slutpop > r[b].slutpop).count();
        (vi, vp)
    };
    let r2_vinst = |a: usize, b: usize| -> usize {
        grid.iter().filter(|r| r[a].hj_r2 > r[b].hj_r2).count()
    };

    let _ = writeln!(t, "
## 3. Förgrind, riktad vs slump och parade jämförelser
");
    let (fg_i, fg_p) = vinst(0, 1);
    let _ = writeln!(t, "- TrangselOrakel > Slump (förgrind): intag **{fg_i}/{n}**, slutpop **{fg_p}/{n}**");
    for (label, ai) in [("Stanna68", 2usize), ("Lar", 3), ("Lar2059", 4), ("Lar2116_S20", 5), ("Lar2116_M20", 6)] {
        let (vi, vp) = vinst(ai, 1);
        let _ = writeln!(t, "- {label} > Slump: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
    }
    for (label, ai) in [("Lar2116_S20", 5usize), ("Lar2116_M20", 6)] {
        for &(mot_namn, bi) in &[("Lar", 3usize), ("Stryk2059", 7), ("Slump", 1), ("Stanna68", 2)] {
            let (vi, vp) = vinst(ai, bi);
            let _ = writeln!(t, "- {label} > {mot_namn}: intag **{vi}/{n}**, slutpop **{vp}/{n}**");
        }
        let wr = r2_vinst(ai, 4);
        let _ = writeln!(t, "- {label} > Lar2059 på hjR²: **{wr}/{n}**");
    }
    let _ = writeln!(t, "- Lar2116_M20 > Lar2116_S20 på hjR²: **{}/{n}**", r2_vinst(6, 5));

    let forgrind_gron = fg_i >= krav && fg_p >= krav;
    let wm_pass = |ai: usize| -> bool {
        let pos = grid.iter().filter(|r| r[ai].hj_r2 > 0.0).count();
        let r2_med = median(grid.iter().map(|r| r[ai].hj_r2).collect());
        let fk_med = median(grid.iter().map(|r| r[ai].hj_fk).collect());
        let gd_med = median(grid.iter().map(|r| r[ai].g_djur).collect());
        let gv_med = median(grid.iter().map(|r| r[ai].g_vaxt).collect());
        let ge_med = median(grid.iter().map(|r| r[ai].g_energi).collect());
        pos >= krav
            && r2_med >= 0.080
            && fk_med < 0.800
            && r2_vinst(ai, 4) >= krav
            && gd_med < 0.050
            && gd_med < gv_med.min(ge_med)
    };
    let wm_s20 = wm_pass(5);
    let wm_m20 = wm_pass(6);
    let wm_gron = wm_s20 || wm_m20;

    let bet_pass = |ai: usize| -> bool {
        [3usize, 1, 7, 2].iter().all(|&bi| {
            let (vi, vp) = vinst(ai, bi);
            vi >= krav && vp >= krav
        })
    };
    let bet_gron = bet_pass(5) || bet_pass(6);

    let _ = writeln!(t, "
## 4. Automatisk grinddom
");
    let _ = writeln!(t, "- **Förgrind (TrangselOrakel > Slump i ≥ {krav}/{n}):** **{}**", if forgrind_gron { "PASS" } else { "FAIL" });
    let _ = writeln!(
        t,
        "- **Världsmodell (`Lar2116_S20`={} / `Lar2116_M20`={}):** **{}**",
        if wm_s20 { "PASS" } else { "FAIL" },
        if wm_m20 { "PASS" } else { "FAIL" },
        if wm_gron { "GRÖNT" } else { "RÖTT" }
    );
    let _ = writeln!(
        t,
        "- **Ekologiskt beteende bortom `Stanna68` (`Lar2116_S20`={} / `Lar2116_M20`={}):** **{}**",
        if bet_pass(5) { "PASS" } else { "FAIL" },
        if bet_pass(6) { "PASS" } else { "FAIL" },
        if bet_gron { "GRÖNT (behöver bekräftelse på 281–300)" } else { "EJ BORTOM STANNA68" }
    );

    live_logg_med(
        EGEN_LOGG_2116,
        "Alt 2116",
        &format!(
            "KLAR Spår B ({:.1}s): förgrind={}, världsmodell={}, beteende_bortom_Stanna68={}",
            sec,
            if forgrind_gron { "PASS" } else { "FAIL" },
            if wm_gron { "GRÖNT" } else { "RÖTT" },
            if bet_gron { "GRÖNT" } else { "EJ_BORTOM" },
        ),
    );
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2144 D2 (9 fristående RLS) + extern probering

const SPARB2144_ARMAR: [(&'static str, Formaga); 4] = [
    ("Lar2116_M20", Formaga::Lar2116Minst(0.20)),
    ("Lar2144_D2_a2", Formaga::Lar2144D2(0.20, 20)),
    ("Lar2144_D2_a4", Formaga::Lar2144D2(0.20, 40)),
    ("Lar2144_D2_a8", Formaga::Lar2144D2(0.20, 80)),
];

#[derive(Clone, Default)]
struct Diag2144 {
    hj_r2: f64,
    hj_fk: f64,
    verk_var: f64,
    g_vaxt: f64,
    g_djur: f64,
    g_energi: f64,
    probe_y_intag: f64,
    probe_y_de: f64,
    probe_z_intag: f64,
    probe_z_de: f64,
}

#[derive(Clone, Default)]
struct Sparb2144Utfall {
    medelintag: f64,
    slutpop: f64,
    medelenergi: f64,
    svalt_andel: f64,
    allv_traff: f64,
    andel_stanna: f64,
    vaxt_diff: f64,
    topp1_andel: f64,
    d600: Diag2144,
    d1500: Diag2144,
    d2700: Diag2144,
}

fn iqr_och_median(vals: &[f64]) -> (f64, f64, f64) {
    let mut r: Vec<f64> = vals.iter().copied().filter(|x| x.is_finite()).collect();
    if r.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| -> f64 {
        let idx = p * (r.len() - 1) as f64;
        let lo = idx.floor() as usize;
        let hi = idx.ceil() as usize;
        if lo == hi { r[lo] } else { r[lo] * (1.0 - (idx - lo as f64)) + r[hi] * (idx - lo as f64) }
    };
    (q(0.50), q(0.25), q(0.75))
}

/// Grupperad 5-faldig yttre korsvalidering (alla 9 handlingar från samma individ alltid i samma fold).
fn extern_probe_r2(x: &[Vec<f64>], y: &[f64], grupp: &[usize]) -> f64 {
    if x.len() < 15 || x.len() != y.len() || x.len() != grupp.len() {
        return f64::NAN;
    }
    let mut g_map: HashMap<usize, usize> = HashMap::new();
    for &gr in grupp {
        if !g_map.contains_key(&gr) {
            let id = g_map.len();
            g_map.insert(gr, id);
        }
    }
    if g_map.len() < 5 {
        return f64::NAN;
    }
    let my = y.iter().sum::<f64>() / y.len() as f64;
    let sst: f64 = y.iter().map(|&v| (v - my).powi(2)).sum();
    if sst < 1e-12 {
        return f64::NAN;
    }
    let mut sse = 0.0;
    for f in 0..5 {
        let mut tr_x = Vec::new();
        let mut tr_y = Vec::new();
        let mut tr_del = Vec::new();
        let mut te_idx = Vec::new();
        for i in 0..x.len() {
            let fold = g_map[&grupp[i]] % 5;
            if fold == f {
                te_idx.push(i);
            } else {
                tr_x.push(x[i].clone());
                tr_y.push(vec![y[i]]);
                tr_del.push(if fold < f { fold } else { fold - 1 });
            }
        }
        let Ok(m) = Ridge::anpassa(&tr_x, &tr_y, &tr_del) else {
            return f64::NAN;
        };
        for i in te_idx {
            let pred = m.gissa(&x[i])[0];
            sse += (pred - y[i]).powi(2);
        }
    }
    1.0 - sse / sst
}

fn kor_hj2144_diag(sim: &Sim, fro: u64, min_age: i64, min_steg: u64, med_probe: bool) -> Diag2144 {
    let mut alla: Vec<usize> = (0..sim.djur.len())
        .filter(|&i| {
            let d = &sim.djur[i];
            d.lev
                && d.age >= min_age
                && d.hj2059
                    .as_ref()
                    .is_some_and(|h| h.steg >= min_steg && h.skala_init && h.z_skala_init)
        })
        .collect();
    let mut rng = Rng::new(fro ^ 0x2059);
    rng.blanda(&mut alla);
    let mut kand = Vec::new();
    for i in alla {
        if kand.len() >= 20 {
            break;
        }
        if kand.iter().all(|&v| avstand_torus(sim, v, i) >= 7) {
            kand.push(i);
        }
    }
    kand.sort_unstable();
    if kand.is_empty() {
        return Diag2144 {
            hj_r2: f64::NAN,
            hj_fk: f64::NAN,
            verk_var: f64::NAN,
            g_vaxt: f64::NAN,
            g_djur: f64::NAN,
            g_energi: f64::NAN,
            probe_y_intag: f64::NAN,
            probe_y_de: f64::NAN,
            probe_z_intag: f64::NAN,
            probe_z_de: f64::NAN,
        };
    }

    let uids: Vec<u64> = kand.iter().map(|&i| sim.djur[i].uid).collect();
    let e0: Vec<f64> = kand.iter().map(|&i| sim.djur[i].e).collect();
    let hjs: Vec<Hj2008> = kand.iter().map(|&i| (**sim.djur[i].hj2059.as_ref().unwrap()).clone()).collect();
    let mut z0_v = Vec::with_capacity(kand.len());
    let mut y_v = Vec::with_capacity(kand.len());
    for (n, &idx) in kand.iter().enumerate() {
        let s0 = sim.sinnen_for(idx);
        let s0_sk = hjs[n].skala_grind(&s0);
        let z0 = hjs[n].kod_online(&s0_sk);
        y_v.push((0..A_N).map(|a| hjs[n].forutsag(&z0, a).to_vec()).collect::<Vec<_>>());
        z0_v.push(z0);
    }

    let mut zn_v: Vec<Vec<Option<Vec<f64>>>> = vec![vec![None; kand.len()]; A_N];
    let mut intag_v: Vec<Vec<Option<f64>>> = vec![vec![None; kand.len()]; A_N];
    let mut de_v: Vec<Vec<Option<f64>>> = vec![vec![None; kand.len()]; A_N];
    for a in 0..A_N {
        let mut k = sim.clone();
        k.tvinga_grupp = Some((kand.clone(), a));
        k.steg();
        k.tvinga_grupp = None;
        let mut vaxt = false;
        for (n, &idx) in kand.iter().enumerate() {
            if k.djur[idx].lev && k.djur[idx].uid == uids[n] {
                let s2 = if vaxt {
                    k.sinnen_for(idx)
                } else {
                    vaxt = true;
                    k.sinnen_nasta_steg(idx)
                };
                let s2_sk = hjs[n].skala_grind(&s2);
                zn_v[a][n] = Some(hjs[n].kod_mal(&s2_sk).to_vec());
                intag_v[a][n] = Some(k.djur[idx].forra_intag.unwrap_or(0.0));
                de_v[a][n] = Some(k.djur[idx].e - e0[n]);
            }
        }
    }

    let mut summa = Summa::default();
    let mut gv_v = Vec::new();
    let mut gd_v = Vec::new();
    let mut ge_v = Vec::new();
    let mut px_y = Vec::new();
    let mut px_z = Vec::new();
    let mut py_intag = Vec::new();
    let mut py_de = Vec::new();
    let mut pgrupp = Vec::new();
    let mut gid = 0usize;

    for n in 0..kand.len() {
        if (0..A_N).any(|a| zn_v[a][n].is_none()) {
            continue;
        }
        let zn_alla: Vec<Vec<f64>> = (0..A_N).map(|a| zn_v[a][n].as_ref().unwrap().clone()).collect();
        summa.lagg(&bidrag(&y_v[n], &zn_alla, &z0_v[n]));
        let gv = hjs[n].aktuell_kanal_vikt();
        gv_v.push((0..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0);
        gd_v.push((1..S_N - 1).step_by(2).map(|j| gv[j]).sum::<f64>() / 9.0);
        ge_v.push(gv[S_N - 1]);

        if med_probe {
            let mut my = [0.0; Z_N];
            let mut mz = [0.0; Z_N];
            let mut mi = 0.0;
            let mut mde = 0.0;
            for a in 0..A_N {
                for d in 0..Z_N {
                    my[d] += y_v[n][a][d] / A_N as f64;
                    mz[d] += zn_alla[a][d] / A_N as f64;
                }
                mi += intag_v[a][n].unwrap() / A_N as f64;
                mde += de_v[a][n].unwrap() / A_N as f64;
            }
            for a in 0..A_N {
                px_y.push((0..Z_N).map(|d| y_v[n][a][d] - my[d]).collect());
                px_z.push((0..Z_N).map(|d| zn_alla[a][d] - mz[d]).collect());
                py_intag.push(intag_v[a][n].unwrap() - mi);
                py_de.push(de_v[a][n].unwrap() - mde);
                pgrupp.push(gid);
            }
            gid += 1;
        }
    }
    if summa.prov == 0 {
        return Diag2144 {
            hj_r2: f64::NAN,
            hj_fk: f64::NAN,
            verk_var: f64::NAN,
            g_vaxt: f64::NAN,
            g_djur: f64::NAN,
            g_energi: f64::NAN,
            probe_y_intag: f64::NAN,
            probe_y_de: f64::NAN,
            probe_z_intag: f64::NAN,
            probe_z_de: f64::NAN,
        };
    }
    Diag2144 {
        hj_r2: summa.r2(),
        hj_fk: summa.kvot(),
        verk_var: summa.verklig / (summa.prov as usize * A_N * Z_N) as f64,
        g_vaxt: median(gv_v),
        g_djur: median(gd_v),
        g_energi: median(ge_v),
        probe_y_intag: if med_probe { extern_probe_r2(&px_y, &py_intag, &pgrupp) } else { f64::NAN },
        probe_y_de: if med_probe { extern_probe_r2(&px_y, &py_de, &pgrupp) } else { f64::NAN },
        probe_z_intag: if med_probe { extern_probe_r2(&px_z, &py_intag, &pgrupp) } else { f64::NAN },
        probe_z_de: if med_probe { extern_probe_r2(&px_z, &py_de, &pgrupp) } else { f64::NAN },
    }
}

fn sparb2144_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    formaga: Formaga,
    inkor_ar: i64,
    mat_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> Sparb2144Utfall {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta: formaga,
        omarkta: formaga,
        spegel: false,
    });
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut().filter(|d| d.lev) {
        d.formaga = formaga;
        d.hj2059 = None;
        d.tabell = Minnestabell::ny();
    }

    let mat_steg = mat_ar * STEG_PER_AR;
    let halva = mat_steg / 2;
    let hb = (sim.b as i32) / 2;
    let hh = (sim.h as i32) / 2;
    let t_tidig = 600.min((mat_steg / 4).max(1));
    let t_mitten = 1500.min((mat_steg / 2).max(t_tidig + 1));
    let t_sen = mat_steg - 300.min((mat_steg / 3).max(1));
    let min_age_skala = |maal: i64| -> i64 {
        if inkor_ar >= 10 { maal } else { (inkor_ar * STEG_PER_AR).min(maal) }
    };
    let min_steg_skala = |t_nu: i64, maal: u64| -> u64 {
        if inkor_ar >= 10 { maal } else { ((t_nu as u64) / 2).max(20).min(maal) }
    };

    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
    let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
    let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
    let (mut n_stanna_prov, mut n_steg_prov, mut sum_vdiff) = (0u64, 0u64, 0.0f64);
    let mut topp1 = f64::NAN;
    let (mut d600, mut d1500, mut d2700) = (Diag2144::default(), Diag2144::default(), Diag2144::default());

    for t in 0..mat_steg {
        if t == halva {
            bas_svalt = sim.rak.dod_svalt;
            bas_alder = sim.rak.dod_alder;
        }
        let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
        if t >= halva && t % STEG_PER_AR == 0 {
            for (idx, d) in sim.djur.iter().enumerate() {
                if d.lev {
                    let a_allv = sim.allvetande_handling(d.i, d.j);
                    let (di, dj) = GRANNAR[a_allv];
                    let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                    if ni >= 0 && nj >= 0 {
                        mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                        if mal_par.len() >= 80 {
                            break;
                        }
                    }
                }
            }
        }
        if t == t_tidig {
            d600 = kor_hj2144_diag(&sim, fro, min_age_skala(600), min_steg_skala(t, 400), false);
        }
        if t == t_mitten {
            d1500 = kor_hj2144_diag(&sim, fro, min_age_skala(1000), min_steg_skala(t, 500), false);
        }
        if t == t_sen {
            d2700 = kor_hj2144_diag(&sim, fro, min_age_skala(1500), min_steg_skala(t, 500), true);
            let mut top_v = Vec::new();
            let mage = min_age_skala(1500);
            for d in sim.djur.iter().filter(|d| d.lev && d.age >= mage) {
                if let Some(ref h) = d.hj2059 {
                    let sum_n: f64 = h.grind_n.iter().sum();
                    if sum_n > 0.0 {
                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                        top_v.push(max_n / sum_n);
                    }
                }
            }
            if !top_v.is_empty() {
                topp1 = median(top_v);
            }
        }

        let stickprov = t >= halva && t % 10 == 0;
        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
        let mut v_norm_fore: Vec<f64> = Vec::new();
        if stickprov {
            v_norm_fore = sim
                .v
                .iter()
                .zip(sim.vmax.iter())
                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                .collect();
            for d in sim.djur.iter().filter(|d| d.lev) {
                fore_pos.insert(d.uid, (d.i, d.j));
            }
        }

        sim.steg();

        for (idx, uid, mal) in mal_par {
            let d = &sim.djur[idx];
            if d.lev && d.uid == uid {
                allv_tot += 1;
                if (d.i, d.j) == mal {
                    allv_ratt += 1;
                }
            }
        }
        if t >= halva {
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some(x) = d.forra_intag {
                    sum_intag += x;
                    n_intag += 1;
                }
                if stickprov {
                    if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                        let mut di = d.i as i32 - i0 as i32;
                        if di > hb { di -= sim.b as i32; } else if di < -hb { di += sim.b as i32; }
                        let mut dj = d.j as i32 - j0 as i32;
                        if dj > hh { dj -= sim.h as i32; } else if dj < -hh { dj += sim.h as i32; }
                        if di.abs() <= 1 && dj.abs() <= 1 {
                            n_steg_prov += 1;
                            if di == 0 && dj == 0 {
                                n_stanna_prov += 1;
                            }
                            let mut sv = 0.0;
                            for &(gi, gj) in &GRANNAR {
                                let (ni, nj) = sim.kant_granne(i0 as i32 + gi, j0 as i32 + gj);
                                if ni >= 0 && nj >= 0 && (ni as usize) < sim.b && (nj as usize) < sim.h {
                                    sv += v_norm_fore[sim.cell(ni as usize, nj as usize)];
                                }
                            }
                            let c_ny = sim.cell(d.i, d.j);
                            sum_vdiff += v_norm_fore[c_ny] - sv / 9.0;
                        }
                    }
                }
            }
        }
    }

    let lev_vec: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
    let slutpop = lev_vec.len() as f64;
    let medelenergi = if lev_vec.is_empty() { 0.0 } else { lev_vec.iter().sum::<f64>() / slutpop };
    let ds = sim.rak.dod_svalt - bas_svalt;
    let da = sim.rak.dod_alder - bas_alder;
    let u = Sparb2144Utfall {
        medelintag: sum_intag / n_intag.max(1) as f64,
        slutpop,
        medelenergi,
        svalt_andel: if ds + da == 0 { 0.0 } else { ds as f64 / (ds + da) as f64 },
        allv_traff: allv_ratt as f64 / allv_tot.max(1) as f64,
        andel_stanna: n_stanna_prov as f64 / n_steg_prov.max(1) as f64,
        vaxt_diff: sum_vdiff / n_steg_prov.max(1) as f64,
        topp1_andel: topp1,
        d600,
        d1500,
        d2700,
    };
    let forlopp = format_forlopp(progress);
    live_logg_med(
        EGEN_LOGG_2144,
        "Alt 2144",
        &format!(
            "Frö {fro} {arm_namn}: hjR²(600/1500/2700)={}/{}/{} fk={} py_in={} pz_in={} g(v/d/E)={}/{}/{} pop={:.0} ({:.1}s){forlopp}",
            f3(u.d600.hj_r2), f3(u.d1500.hj_r2), f3(u.d2700.hj_r2),
            f3(u.d2700.hj_fk), f3(u.d2700.probe_y_intag), f3(u.d2700.probe_z_intag),
            f3(u.d2700.g_vaxt), f3(u.d2700.g_djur), f3(u.d2700.g_energi),
            u.slutpop, t0.elapsed().as_secs_f64(),
        ),
    );
    u
}

fn kora_sparb2144(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2144,
        "Alt 2144",
        &format!(
            "Startar Alt 2144 (D2 vs Lar2116_M20) på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2144,
                            "Alt 2144",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 4);
    for ai in 0..4 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2144Utfall::default(); 4]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2144_ARMAR[ai];
                let u = sparb2144_en_arm(sim_bas, fro, arm_namn, formaga, inkor_ar, mat_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2144 (Experiment 1: Exakt `D2` 9 fristående RLS vs `Lar2116_M20`, {n} världar, {sec:.1}s)\n");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2144-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`); mätning vid steg `600`, `1 500`, `2 700`.\n",
        mat_ar * STEG_PER_AR
    );

    let _ = writeln!(t, "## 1. Världsmodell över livsålder (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (600) | hjR² (1500) | hjR² (2700) | fk (600) | fk (1500) | fk (2700) | hjR²(2700)>0 | Grind (v/d/E) | Topp-1 | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2144_ARMAR.iter().enumerate() {
        let (r6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_r2).collect::<Vec<_>>());
        let (r15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_r2).collect::<Vec<_>>());
        let (r27, r27_q1, r27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (f6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_fk).collect::<Vec<_>>());
        let (f15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_fk).collect::<Vec<_>>());
        let (f27, f27_q1, f27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_energi).collect::<Vec<_>>());
        let (top, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].topp1_andel).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} | {} | {} [{}; {}] | {} | {} | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} | {:.4} |",
            f3(r6), f3(r15), f3(r27), f3(r27_q1), f3(r27_q3),
            f3(f6), f3(f15), f3(f27), f3(f27_q1), f3(f27_q3),
            f3(gv), f3(gd), f3(ge), f3(top), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Extern diagnostisk handlingsprobering vid `steg 2 700` (grupperad 5-faldig CV, enbart mätning)\n");
    let _ = writeln!(t, "| Arm | R²(y_a -> intag) | R²(y_a -> ΔE) | R²(z'_a -> intag) | R²(z'_a -> ΔE) |");
    let _ = writeln!(t, "|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2144_ARMAR.iter().enumerate() {
        let (pyi, pyi1, pyi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_intag).collect::<Vec<_>>());
        let (pyd, pyd1, pyd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_de).collect::<Vec<_>>());
        let (pzi, pzi1, pzi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_intag).collect::<Vec<_>>());
        let (pzd, pzd1, pzd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_de).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] |",
            f3(pyi), f3(pyi1), f3(pyi3),
            f3(pyd), f3(pyd1), f3(pyd3),
            f3(pzi), f3(pzi1), f3(pzi3),
            f3(pzd), f3(pzd1), f3(pzd3),
        );
    }

    let _ = writeln!(t, "\n## 3. Ekologiska mått (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2144_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2144);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2144);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2144);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2144);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2144);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2144);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2144);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 4. Parvisa jämförelser mot `Lar2116_M20` samt urvalskandidat\n");
    let mut bast: Option<(usize, f64, f64)> = None;
    for ai in 1..4 {
        let namn = SPARB2144_ARMAR[ai].0;
        let w_r27 = grid.iter().filter(|r| r[ai].d2700.hj_r2 > r[0].d2700.hj_r2).count();
        let w_r6 = grid.iter().filter(|r| r[ai].d600.hj_r2 > r[0].d600.hj_r2).count();
        let w_fk = grid.iter().filter(|r| r[ai].d2700.hj_fk < r[0].d2700.hj_fk).count();
        let w_py = grid.iter().filter(|r| r[ai].d2700.probe_y_intag > r[0].d2700.probe_y_intag).count();
        let w_pop = grid.iter().filter(|r| r[ai].slutpop > r[0].slutpop).count();
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (r2m, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (fkm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let (gdm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let ok = pos >= krav && r2m >= 0.080 && fkm < 0.800 && gdm < 0.050 && w_r27 >= 6;
        let _ = writeln!(
            t,
            "- **{namn} vs Lar2116_M20:** hjR²(2700) **{w_r27}/{n}**, hjR²(600) **{w_r6}/{n}**, lägre fk(2700) **{w_fk}/{n}**, probe(y->intag) **{w_py}/{n}**, slutpop **{w_pop}/{n}** | urvalskrav: **{}**",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok {
            let byta = match bast {
                None => true,
                Some((_, br, bf)) => r2m > br || (r2m == br && fkm < bf),
            };
            if byta {
                bast = Some((ai, r2m, fkm));
            }
        }
    }
    let val_txt = match bast {
        Some((ai, r2m, fkm)) => format!("{} (median hjR² = {}, fk = {}) — redo för bekräftelse på frö 301–320", SPARB2144_ARMAR[ai].0, f3(r2m), f3(fkm)),
        None => "INGEN (ingen D2-arm klarade urvalskraven)".to_string(),
    };
    let _ = writeln!(t, "\n- **Vald kandidat på utvecklingsmängden:** **{val_txt}**");

    live_logg_med(EGEN_LOGG_2144, "Alt 2144", &format!("KLAR sparb2144 ({sec:.1}s): {val_txt}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2159 Bekräftelse (frö 301–320) + inom-individ extern probering

const SPARB2159_ARMAR: [(&'static str, Formaga); 2] = [
    ("Lar2116_M20", Formaga::Lar2116Minst(0.20)),
    ("Lar2144_D2_a4", Formaga::Lar2144D2(0.20, 40)),
];

#[derive(Clone, Default)]
struct IndividProbeBuffert {
    px_y: Vec<Vec<f64>>,
    px_z: Vec<Vec<f64>>,
    py_intag: Vec<f64>,
    py_de: Vec<f64>,
    pgrupp: Vec<usize>,
    snaps: usize,
}

fn valj_probe_uids(sim: &Sim, fro: u64, min_age: i64, min_steg: u64) -> Vec<u64> {
    let mut alla: Vec<usize> = (0..sim.djur.len())
        .filter(|&i| {
            let d = &sim.djur[i];
            d.lev
                && d.age >= min_age
                && d.hj2059
                    .as_ref()
                    .is_some_and(|h| h.steg >= min_steg && h.skala_init && h.z_skala_init)
        })
        .collect();
    let mut rng = Rng::new(fro ^ 0x2159);
    rng.blanda(&mut alla);
    let mut kand = Vec::new();
    for i in alla {
        if kand.len() >= 20 {
            break;
        }
        if kand.iter().all(|&v| avstand_torus(sim, v, i) >= 7) {
            kand.push(i);
        }
    }
    kand.sort_unstable();
    kand.into_iter().map(|i| sim.djur[i].uid).collect()
}

fn samla_inom_individ_snitt(sim: &Sim, uids: &[u64], snap_idx: usize, bufs: &mut [IndividProbeBuffert]) {
    let mut kand = Vec::new();
    let mut k_pos = Vec::new();
    for (u_i, &uid) in uids.iter().enumerate() {
        if let Some(idx) = sim.djur.iter().position(|d| d.lev && d.uid == uid && d.hj2059.is_some()) {
            kand.push(idx);
            k_pos.push(u_i);
        }
    }
    if kand.is_empty() {
        return;
    }
    let e0: Vec<f64> = kand.iter().map(|&i| sim.djur[i].e).collect();
    let hjs: Vec<Hj2008> = kand.iter().map(|&i| (**sim.djur[i].hj2059.as_ref().unwrap()).clone()).collect();
    let mut y_v = Vec::with_capacity(kand.len());
    for (n, &idx) in kand.iter().enumerate() {
        let s0 = sim.sinnen_for(idx);
        let s0_sk = hjs[n].skala_grind(&s0);
        let z0 = hjs[n].kod_online(&s0_sk);
        y_v.push((0..A_N).map(|a| hjs[n].forutsag(&z0, a).to_vec()).collect::<Vec<_>>());
    }
    let mut zn_v: Vec<Vec<Option<Vec<f64>>>> = vec![vec![None; kand.len()]; A_N];
    let mut intag_v: Vec<Vec<Option<f64>>> = vec![vec![None; kand.len()]; A_N];
    let mut de_v: Vec<Vec<Option<f64>>> = vec![vec![None; kand.len()]; A_N];
    for a in 0..A_N {
        let mut k = sim.clone();
        k.tvinga_grupp = Some((kand.clone(), a));
        k.steg();
        k.tvinga_grupp = None;
        let mut vaxt = false;
        for (n, &idx) in kand.iter().enumerate() {
            let uid = uids[k_pos[n]];
            if k.djur[idx].lev && k.djur[idx].uid == uid {
                let s2 = if vaxt {
                    k.sinnen_for(idx)
                } else {
                    vaxt = true;
                    k.sinnen_nasta_steg(idx)
                };
                let s2_sk = hjs[n].skala_grind(&s2);
                zn_v[a][n] = Some(hjs[n].kod_mal(&s2_sk).to_vec());
                intag_v[a][n] = Some(k.djur[idx].forra_intag.unwrap_or(0.0));
                de_v[a][n] = Some(k.djur[idx].e - e0[n]);
            }
        }
    }
    for (n, &u_i) in k_pos.iter().enumerate() {
        if (0..A_N).any(|a| zn_v[a][n].is_none()) {
            continue;
        }
        let mut my = [0.0; Z_N];
        let mut mz = [0.0; Z_N];
        let mut mi = 0.0;
        let mut mde = 0.0;
        for a in 0..A_N {
            let zn = zn_v[a][n].as_ref().unwrap();
            for d in 0..Z_N {
                my[d] += y_v[n][a][d] / A_N as f64;
                mz[d] += zn[d] / A_N as f64;
            }
            mi += intag_v[a][n].unwrap() / A_N as f64;
            mde += de_v[a][n].unwrap() / A_N as f64;
        }
        let b = &mut bufs[u_i];
        for a in 0..A_N {
            let zn = zn_v[a][n].as_ref().unwrap();
            b.px_y.push((0..Z_N).map(|d| y_v[n][a][d] - my[d]).collect());
            b.px_z.push((0..Z_N).map(|d| zn[d] - mz[d]).collect());
            b.py_intag.push(intag_v[a][n].unwrap() - mi);
            b.py_de.push(de_v[a][n].unwrap() - mde);
            b.pgrupp.push(snap_idx);
        }
        b.snaps += 1;
    }
}

fn utvardera_inom_individ(bufs: &[IndividProbeBuffert]) -> (f64, f64, f64, f64, usize) {
    let mut r_yi = Vec::new();
    let mut r_yd = Vec::new();
    let mut r_zi = Vec::new();
    let mut r_zd = Vec::new();
    let mut antal = 0usize;
    for b in bufs {
        if b.snaps < 5 {
            continue;
        }
        antal += 1;
        let pyi = extern_probe_r2(&b.px_y, &b.py_intag, &b.pgrupp);
        let pyd = extern_probe_r2(&b.px_y, &b.py_de, &b.pgrupp);
        let pzi = extern_probe_r2(&b.px_z, &b.py_intag, &b.pgrupp);
        let pzd = extern_probe_r2(&b.px_z, &b.py_de, &b.pgrupp);
        if pyi.is_finite() { r_yi.push(pyi); }
        if pyd.is_finite() { r_yd.push(pyd); }
        if pzi.is_finite() { r_zi.push(pzi); }
        if pzd.is_finite() { r_zd.push(pzd); }
    }
    (median(r_yi), median(r_yd), median(r_zi), median(r_zd), antal)
}

fn sparb2159_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    formaga: Formaga,
    inkor_ar: i64,
    mat_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> (Sparb2144Utfall, usize) {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta: formaga,
        omarkta: formaga,
        spegel: false,
    });
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut().filter(|d| d.lev) {
        d.formaga = formaga;
        d.hj2059 = None;
        d.tabell = Minnestabell::ny();
    }

    let mat_steg = mat_ar * STEG_PER_AR;
    let halva = mat_steg / 2;
    let hb = (sim.b as i32) / 2;
    let hh = (sim.h as i32) / 2;
    let t_tidig = 600.min((mat_steg / 4).max(1));
    let t_mitten = 1500.min((mat_steg / 2).max(t_tidig + 1));
    let t_sen = mat_steg - 300.min((mat_steg / 3).max(1));
    let snap_steg = if t_sen >= 2700 { 20 } else { (t_sen / 12).max(1) };
    let t_probe_start = t_sen - 9 * snap_steg;
    let min_age_skala = |maal: i64| -> i64 {
        if inkor_ar >= 10 { maal } else { (inkor_ar * STEG_PER_AR).min(maal) }
    };
    let min_steg_skala = |t_nu: i64, maal: u64| -> u64 {
        if inkor_ar >= 10 { maal } else { ((t_nu as u64) / 2).max(20).min(maal) }
    };

    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
    let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
    let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
    let (mut n_stanna_prov, mut n_steg_prov, mut sum_vdiff) = (0u64, 0u64, 0.0f64);
    let mut topp1 = f64::NAN;
    let (mut d600, mut d1500, mut d2700) = (Diag2144::default(), Diag2144::default(), Diag2144::default());
    let mut probe_uids: Vec<u64> = Vec::new();
    let mut probe_bufs: Vec<IndividProbeBuffert> = Vec::new();

    for t in 0..mat_steg {
        if t == halva {
            bas_svalt = sim.rak.dod_svalt;
            bas_alder = sim.rak.dod_alder;
        }
        let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
        if t >= halva && t % STEG_PER_AR == 0 {
            for (idx, d) in sim.djur.iter().enumerate() {
                if d.lev {
                    let a_allv = sim.allvetande_handling(d.i, d.j);
                    let (di, dj) = GRANNAR[a_allv];
                    let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                    if ni >= 0 && nj >= 0 {
                        mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                        if mal_par.len() >= 80 {
                            break;
                        }
                    }
                }
            }
        }
        if t == t_tidig {
            d600 = kor_hj2144_diag(&sim, fro, min_age_skala(600), min_steg_skala(t, 400), false);
        }
        if t == t_mitten {
            d1500 = kor_hj2144_diag(&sim, fro, min_age_skala(1000), min_steg_skala(t, 500), false);
        }
        if t == t_probe_start {
            probe_uids = valj_probe_uids(&sim, fro, min_age_skala(1500), min_steg_skala(t, 500));
            probe_bufs = vec![IndividProbeBuffert::default(); probe_uids.len()];
        }
        if t >= t_probe_start && t <= t_sen && (t - t_probe_start) % snap_steg == 0 {
            let snap_idx = ((t - t_probe_start) / snap_steg) as usize;
            if snap_idx < 10 && !probe_uids.is_empty() {
                samla_inom_individ_snitt(&sim, &probe_uids, snap_idx, &mut probe_bufs);
            }
        }
        if t == t_sen {
            d2700 = kor_hj2144_diag(&sim, fro, min_age_skala(1500), min_steg_skala(t, 500), false);
            let mut top_v = Vec::new();
            let mage = min_age_skala(1500);
            for d in sim.djur.iter().filter(|d| d.lev && d.age >= mage) {
                if let Some(ref h) = d.hj2059 {
                    let sum_n: f64 = h.grind_n.iter().sum();
                    if sum_n > 0.0 {
                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                        top_v.push(max_n / sum_n);
                    }
                }
            }
            if !top_v.is_empty() {
                topp1 = median(top_v);
            }
        }

        let stickprov = t >= halva && t % 10 == 0;
        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
        let mut v_norm_fore: Vec<f64> = Vec::new();
        if stickprov {
            v_norm_fore = sim
                .v
                .iter()
                .zip(sim.vmax.iter())
                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                .collect();
            for d in sim.djur.iter().filter(|d| d.lev) {
                fore_pos.insert(d.uid, (d.i, d.j));
            }
        }

        sim.steg();

        for (idx, uid, mal) in mal_par {
            let d = &sim.djur[idx];
            if d.lev && d.uid == uid {
                allv_tot += 1;
                if (d.i, d.j) == mal {
                    allv_ratt += 1;
                }
            }
        }
        if t >= halva {
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some(x) = d.forra_intag {
                    sum_intag += x;
                    n_intag += 1;
                }
                if stickprov {
                    if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                        let mut di = d.i as i32 - i0 as i32;
                        if di > hb { di -= sim.b as i32; } else if di < -hb { di += sim.b as i32; }
                        let mut dj = d.j as i32 - j0 as i32;
                        if dj > hh { dj -= sim.h as i32; } else if dj < -hh { dj += sim.h as i32; }
                        if di.abs() <= 1 && dj.abs() <= 1 {
                            n_steg_prov += 1;
                            if di == 0 && dj == 0 {
                                n_stanna_prov += 1;
                            }
                            let mut sv = 0.0;
                            for &(gi, gj) in &GRANNAR {
                                let (ni, nj) = sim.kant_granne(i0 as i32 + gi, j0 as i32 + gj);
                                if ni >= 0 && nj >= 0 && (ni as usize) < sim.b && (nj as usize) < sim.h {
                                    sv += v_norm_fore[sim.cell(ni as usize, nj as usize)];
                                }
                            }
                            let c_ny = sim.cell(d.i, d.j);
                            sum_vdiff += v_norm_fore[c_ny] - sv / 9.0;
                        }
                    }
                }
            }
        }
    }

    let (pyi, pyd, pzi, pzd, n_indiv) = utvardera_inom_individ(&probe_bufs);
    d2700.probe_y_intag = pyi;
    d2700.probe_y_de = pyd;
    d2700.probe_z_intag = pzi;
    d2700.probe_z_de = pzd;

    let lev_vec: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
    let slutpop = lev_vec.len() as f64;
    let medelenergi = if lev_vec.is_empty() { 0.0 } else { lev_vec.iter().sum::<f64>() / slutpop };
    let ds = sim.rak.dod_svalt - bas_svalt;
    let da = sim.rak.dod_alder - bas_alder;
    let u = Sparb2144Utfall {
        medelintag: sum_intag / n_intag.max(1) as f64,
        slutpop,
        medelenergi,
        svalt_andel: if ds + da == 0 { 0.0 } else { ds as f64 / (ds + da) as f64 },
        allv_traff: allv_ratt as f64 / allv_tot.max(1) as f64,
        andel_stanna: n_stanna_prov as f64 / n_steg_prov.max(1) as f64,
        vaxt_diff: sum_vdiff / n_steg_prov.max(1) as f64,
        topp1_andel: topp1,
        d600,
        d1500,
        d2700,
    };
    let forlopp = format_forlopp(progress);
    live_logg_med(
        EGEN_LOGG_2159,
        "Alt 2159",
        &format!(
            "Frö {fro} {arm_namn}: hjR²(600/1500/2700)={}/{}/{} fk={} inom_py(in/dE)={}/{} inom_pz(in/dE)={}/{} (n={n_indiv}) g(v/d/E)={}/{}/{} pop={:.0} ({:.1}s){forlopp}",
            f3(u.d600.hj_r2), f3(u.d1500.hj_r2), f3(u.d2700.hj_r2),
            f3(u.d2700.hj_fk), f3(u.d2700.probe_y_intag), f3(u.d2700.probe_y_de),
            f3(u.d2700.probe_z_intag), f3(u.d2700.probe_z_de),
            f3(u.d2700.g_vaxt), f3(u.d2700.g_djur), f3(u.d2700.g_energi),
            u.slutpop, t0.elapsed().as_secs_f64(),
        ),
    );
    (u, n_indiv)
}

fn kora_sparb2159(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2159,
        "Alt 2159",
        &format!(
            "Startar Alt 2159 bekräftelse (Lar2144_D2_a4 vs Lar2116_M20) på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2159,
                            "Alt 2159",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 2);
    for ai in 0..2 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2144Utfall::default(); 2]; bas_arc.len()];
    let mut n_indiv_grid = vec![vec![0usize; 2]; bas_arc.len()];
    let grid_mtx = Mutex::new((&mut grid, &mut n_indiv_grid));
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2159_ARMAR[ai];
                let (u, ni) = sparb2159_en_arm(sim_bas, fro, arm_namn, formaga, inkor_ar, mat_ar, Some(&*prog_c));
                let mut g = mtx_ref.lock().unwrap();
                g.0[fi][ai] = u;
                g.1[fi][ai] = ni;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2159 (Bekräftelse `Lar2144_D2_a4` vs `Lar2116_M20` på nya frön, {n} världar, {sec:.1}s)\n");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2159-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`); inom-individ-probering över `10` tidssnitt per spårad individ.\n",
        mat_ar * STEG_PER_AR
    );

    let _ = writeln!(t, "## 1. Världsmodell över livsålder (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (600) | hjR² (1500) | hjR² (2700) | fk (600) | fk (1500) | fk (2700) | hjR²(2700)>0 | Grind (v/d/E) | Topp-1 | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2159_ARMAR.iter().enumerate() {
        let (r6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_r2).collect::<Vec<_>>());
        let (r15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_r2).collect::<Vec<_>>());
        let (r27, r27_q1, r27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (f6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_fk).collect::<Vec<_>>());
        let (f15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_fk).collect::<Vec<_>>());
        let (f27, f27_q1, f27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_energi).collect::<Vec<_>>());
        let (top, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].topp1_andel).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} | {} | {} [{}; {}] | {} | {} | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} | {:.4} |",
            f3(r6), f3(r15), f3(r27), f3(r27_q1), f3(r27_q3),
            f3(f6), f3(f15), f3(f27), f3(f27_q1), f3(f27_q3),
            f3(gv), f3(gd), f3(ge), f3(top), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Inom-individ extern handlingsprobering (`10` tidssnitt/individ, median [Q1; Q3] över världar)\n");
    let _ = writeln!(t, "| Arm | R²(y_a -> intag) | R²(y_a -> ΔE) | R²(z'_a -> intag) | R²(z'_a -> ΔE) | Spårade individer (med) |");
    let _ = writeln!(t, "|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2159_ARMAR.iter().enumerate() {
        let (pyi, pyi1, pyi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_intag).collect::<Vec<_>>());
        let (pyd, pyd1, pyd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_de).collect::<Vec<_>>());
        let (pzi, pzi1, pzi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_intag).collect::<Vec<_>>());
        let (pzd, pzd1, pzd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_de).collect::<Vec<_>>());
        let ni_med = median(n_indiv_grid.iter().map(|r| r[ai] as f64).collect());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {:.0} |",
            f3(pyi), f3(pyi1), f3(pyi3),
            f3(pyd), f3(pyd1), f3(pyd3),
            f3(pzi), f3(pzi1), f3(pzi3),
            f3(pzd), f3(pzd1), f3(pzd3),
            ni_med,
        );
    }

    let _ = writeln!(t, "\n## 3. Ekologiska mått (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2159_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2159);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2159);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2159);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2159);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2159);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2159);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2159);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 4. Parvis jämförelse och bekräftelsedom (`Lar2144_D2_a4 vs Lar2116_M20`)\n");
    let w_r27 = grid.iter().filter(|r| r[1].d2700.hj_r2 > r[0].d2700.hj_r2).count();
    let w_r6 = grid.iter().filter(|r| r[1].d600.hj_r2 > r[0].d600.hj_r2).count();
    let w_fk = grid.iter().filter(|r| r[1].d2700.hj_fk < r[0].d2700.hj_fk).count();
    let w_py = grid.iter().filter(|r| r[1].d2700.probe_y_intag > r[0].d2700.probe_y_intag).count();
    let w_pop = grid.iter().filter(|r| r[1].slutpop > r[0].slutpop).count();
    let pos_d2 = grid.iter().filter(|r| r[1].d2700.hj_r2 > 0.0).count();
    let (r2_m20, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].d2700.hj_r2).collect::<Vec<_>>());
    let (r2_d2, _, _) = iqr_och_median(&grid.iter().map(|r| r[1].d2700.hj_r2).collect::<Vec<_>>());
    let (fk_d2, _, _) = iqr_och_median(&grid.iter().map(|r| r[1].d2700.hj_fk).collect::<Vec<_>>());
    let (gv_d2, _, _) = iqr_och_median(&grid.iter().map(|r| r[1].d2700.g_vaxt).collect::<Vec<_>>());
    let (gd_d2, _, _) = iqr_och_median(&grid.iter().map(|r| r[1].d2700.g_djur).collect::<Vec<_>>());
    let (ge_d2, _, _) = iqr_och_median(&grid.iter().map(|r| r[1].d2700.g_energi).collect::<Vec<_>>());
    let min_par = (n * 3).div_ceil(10); // 6/20
    let ok = pos_d2 >= krav
        && r2_d2 >= 0.080
        && fk_d2 < 0.800
        && gd_d2 < 0.050
        && gd_d2 < gv_d2.min(ge_d2)
        && ((w_r27 >= min_par && r2_d2 >= r2_m20 - 0.020) || w_fk >= n.div_ceil(2));
    let dom = if ok { "BEKRÄFTAD NY BASLINJE" } else { "EJ BEKRÄFTAD" };

    let _ = writeln!(
        t,
        "- **Lar2144_D2_a4 vs Lar2116_M20:** hjR²(2700) **{w_r27}/{n}**, hjR²(600) **{w_r6}/{n}**, lägre fk(2700) **{w_fk}/{n}**, probe(y->intag) **{w_py}/{n}**, slutpop **{w_pop}/{n}**"
    );
    let _ = writeln!(t, "- **Automatisk bekräftelsedom:** **{dom}**");

    live_logg_med(EGEN_LOGG_2159, "Alt 2159", &format!("KLAR sparb2159 ({sec:.1}s): {dom}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2172 Test 2 (sinnes- och z-skalning ovanpå D2_a4)

const SPARB2172_ARMAR: [(&'static str, Formaga); 4] = [
    ("D2_Bas", Formaga::Lar2144D2(0.20, 40)),
    ("D2_Adaptiv", Formaga::Lar2172Norm(0.20, 40, 1)),
    ("D2_RMS", Formaga::Lar2172Norm(0.20, 40, 2)),
    ("D2_FrysZ", Formaga::Lar2172Norm(0.20, 40, 3)),
];

fn pearson_corr(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.len() < 3 || xs.len() != ys.len() {
        return f64::NAN;
    }
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let (mut cov, mut vx, mut vy) = (0.0, 0.0, 0.0);
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        let dx = x - mx;
        let dy = y - my;
        cov += dx * dy;
        vx += dx * dx;
        vy += dy * dy;
    }
    if vx < 1e-12 || vy < 1e-12 {
        return f64::NAN;
    }
    cov / (vx * vy).sqrt()
}

fn kor_corr_energi(sim: &Sim, min_age: i64) -> f64 {
    let mut e_real = Vec::new();
    let mut s_energi = Vec::new();
    for (idx, d) in sim.djur.iter().enumerate() {
        if d.lev && d.age >= min_age {
            if let Some(ref h) = d.hj2059 {
                if h.skala_init {
                    let s = sim.sinnen_for(idx);
                    let s_sk = h.skala(&s);
                    e_real.push(d.e);
                    s_energi.push(s_sk[S_N - 1]);
                }
            }
        }
    }
    pearson_corr(&e_real, &s_energi)
}

fn sparb_4arm_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    formaga: Formaga,
    inkor_ar: i64,
    mat_ar: i64,
    egen_logg: &str,
    tagg: &str,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> (Sparb2144Utfall, f64, usize) {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta: formaga,
        omarkta: formaga,
        spegel: false,
    });
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut().filter(|d| d.lev) {
        d.formaga = formaga;
        d.hj2059 = None;
        d.tabell = Minnestabell::ny();
    }

    let mat_steg = mat_ar * STEG_PER_AR;
    let halva = mat_steg / 2;
    let hb = (sim.b as i32) / 2;
    let hh = (sim.h as i32) / 2;
    let t_tidig = 600.min((mat_steg / 4).max(1));
    let t_mitten = 1500.min((mat_steg / 2).max(t_tidig + 1));
    let t_sen = mat_steg - 300.min((mat_steg / 3).max(1));
    let snap_steg = if t_sen >= 2700 { 20 } else { (t_sen / 12).max(1) };
    let t_probe_start = t_sen - 9 * snap_steg;
    let min_age_skala = |maal: i64| -> i64 {
        if inkor_ar >= 10 { maal } else { (inkor_ar * STEG_PER_AR).min(maal) }
    };
    let min_steg_skala = |t_nu: i64, maal: u64| -> u64 {
        if inkor_ar >= 10 { maal } else { ((t_nu as u64) / 2).max(20).min(maal) }
    };

    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
    let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
    let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
    let (mut n_stanna_prov, mut n_steg_prov, mut sum_vdiff) = (0u64, 0u64, 0.0f64);
    let mut topp1 = f64::NAN;
    let mut corr_e = f64::NAN;
    let (mut d600, mut d1500, mut d2700) = (Diag2144::default(), Diag2144::default(), Diag2144::default());
    let mut probe_uids: Vec<u64> = Vec::new();
    let mut probe_bufs: Vec<IndividProbeBuffert> = Vec::new();

    for t in 0..mat_steg {
        if t == halva {
            bas_svalt = sim.rak.dod_svalt;
            bas_alder = sim.rak.dod_alder;
        }
        let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
        if t >= halva && t % STEG_PER_AR == 0 {
            for (idx, d) in sim.djur.iter().enumerate() {
                if d.lev {
                    let a_allv = sim.allvetande_handling(d.i, d.j);
                    let (di, dj) = GRANNAR[a_allv];
                    let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                    if ni >= 0 && nj >= 0 {
                        mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                        if mal_par.len() >= 80 {
                            break;
                        }
                    }
                }
            }
        }
        if t == t_tidig {
            d600 = kor_hj2144_diag(&sim, fro, min_age_skala(600), min_steg_skala(t, 400), false);
        }
        if t == t_mitten {
            d1500 = kor_hj2144_diag(&sim, fro, min_age_skala(1000), min_steg_skala(t, 500), false);
        }
        if t == t_probe_start {
            probe_uids = valj_probe_uids(&sim, fro, min_age_skala(1500), min_steg_skala(t, 500));
            probe_bufs = vec![IndividProbeBuffert::default(); probe_uids.len()];
        }
        if t >= t_probe_start && t <= t_sen && (t - t_probe_start) % snap_steg == 0 {
            let snap_idx = ((t - t_probe_start) / snap_steg) as usize;
            if snap_idx < 10 && !probe_uids.is_empty() {
                samla_inom_individ_snitt(&sim, &probe_uids, snap_idx, &mut probe_bufs);
            }
        }
        if t == t_sen {
            let mage = min_age_skala(1500);
            d2700 = kor_hj2144_diag(&sim, fro, mage, min_steg_skala(t, 500), false);
            corr_e = kor_corr_energi(&sim, mage);
            let mut top_v = Vec::new();
            for d in sim.djur.iter().filter(|d| d.lev && d.age >= mage) {
                if let Some(ref h) = d.hj2059 {
                    let sum_n: f64 = h.grind_n.iter().sum();
                    if sum_n > 0.0 {
                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                        top_v.push(max_n / sum_n);
                    }
                }
            }
            if !top_v.is_empty() {
                topp1 = median(top_v);
            }
        }

        let stickprov = t >= halva && t % 10 == 0;
        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
        let mut v_norm_fore: Vec<f64> = Vec::new();
        if stickprov {
            v_norm_fore = sim
                .v
                .iter()
                .zip(sim.vmax.iter())
                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                .collect();
            for d in sim.djur.iter().filter(|d| d.lev) {
                fore_pos.insert(d.uid, (d.i, d.j));
            }
        }

        sim.steg();

        for (idx, uid, mal) in mal_par {
            let d = &sim.djur[idx];
            if d.lev && d.uid == uid {
                allv_tot += 1;
                if (d.i, d.j) == mal {
                    allv_ratt += 1;
                }
            }
        }
        if t >= halva {
            for d in sim.djur.iter().filter(|d| d.lev) {
                if let Some(x) = d.forra_intag {
                    sum_intag += x;
                    n_intag += 1;
                }
                if stickprov {
                    if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                        let mut di = d.i as i32 - i0 as i32;
                        if di > hb { di -= sim.b as i32; } else if di < -hb { di += sim.b as i32; }
                        let mut dj = d.j as i32 - j0 as i32;
                        if dj > hh { dj -= sim.h as i32; } else if dj < -hh { dj += sim.h as i32; }
                        if di.abs() <= 1 && dj.abs() <= 1 {
                            n_steg_prov += 1;
                            if di == 0 && dj == 0 {
                                n_stanna_prov += 1;
                            }
                            let mut sv = 0.0;
                            for &(gi, gj) in &GRANNAR {
                                let (ni, nj) = sim.kant_granne(i0 as i32 + gi, j0 as i32 + gj);
                                if ni >= 0 && nj >= 0 && (ni as usize) < sim.b && (nj as usize) < sim.h {
                                    sv += v_norm_fore[sim.cell(ni as usize, nj as usize)];
                                }
                            }
                            let c_ny = sim.cell(d.i, d.j);
                            sum_vdiff += v_norm_fore[c_ny] - sv / 9.0;
                        }
                    }
                }
            }
        }
    }

    let (pyi, pyd, pzi, pzd, n_indiv) = utvardera_inom_individ(&probe_bufs);
    d2700.probe_y_intag = pyi;
    d2700.probe_y_de = pyd;
    d2700.probe_z_intag = pzi;
    d2700.probe_z_de = pzd;

    let lev_vec: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
    let slutpop = lev_vec.len() as f64;
    let medelenergi = if lev_vec.is_empty() { 0.0 } else { lev_vec.iter().sum::<f64>() / slutpop };
    let ds = sim.rak.dod_svalt - bas_svalt;
    let da = sim.rak.dod_alder - bas_alder;
    let u = Sparb2144Utfall {
        medelintag: sum_intag / n_intag.max(1) as f64,
        slutpop,
        medelenergi,
        svalt_andel: if ds + da == 0 { 0.0 } else { ds as f64 / (ds + da) as f64 },
        allv_traff: allv_ratt as f64 / allv_tot.max(1) as f64,
        andel_stanna: n_stanna_prov as f64 / n_steg_prov.max(1) as f64,
        vaxt_diff: sum_vdiff / n_steg_prov.max(1) as f64,
        topp1_andel: topp1,
        d600,
        d1500,
        d2700,
    };
    let forlopp = format_forlopp(progress);
    live_logg_med(
        egen_logg,
        tagg,
        &format!(
            "Frö {fro} {arm_namn}: hjR²(600/1500/2700)={}/{}/{} fk={} corr_E={} py_in={} pz_in={} (n={n_indiv}) g_d={} pop={:.0} ({:.1}s){forlopp}",
            f3(u.d600.hj_r2), f3(u.d1500.hj_r2), f3(u.d2700.hj_r2),
            f3(u.d2700.hj_fk), f3(corr_e), f3(u.d2700.probe_y_intag), f3(u.d2700.probe_z_intag),
            f3(u.d2700.g_djur), u.slutpop, t0.elapsed().as_secs_f64(),
        ),
    );
    (u, corr_e, n_indiv)
}

fn kora_sparb2172(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2172,
        "Alt 2172",
        &format!(
            "Startar Alt 2172 (Test 2: sinnes- och z-skalning på D2_a4) på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2172,
                            "Alt 2172",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 4);
    for ai in 0..4 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2144Utfall::default(); 4]; bas_arc.len()];
    let mut corr_grid = vec![vec![f64::NAN; 4]; bas_arc.len()];
    let mut ni_grid = vec![vec![0usize; 4]; bas_arc.len()];
    let grid_mtx = Mutex::new((&mut grid, &mut corr_grid, &mut ni_grid));
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2172_ARMAR[ai];
                let (u, ce, ni) = sparb_4arm_en_arm(
                    sim_bas,
                    fro,
                    arm_namn,
                    formaga,
                    inkor_ar,
                    mat_ar,
                    EGEN_LOGG_2172,
                    "Alt 2172",
                    Some(&*prog_c),
                );
                let mut g = mtx_ref.lock().unwrap();
                g.0[fi][ai] = u;
                g.1[fi][ai] = ce;
                g.2[fi][ai] = ni;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2172 (Test 2: Sinnes- och z-skalning ovanpå `D2_a4`, {n} världar, {sec:.1}s)\n");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2172-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`).\n",
        mat_ar * STEG_PER_AR
    );

    let _ = writeln!(t, "## 1. Världsmodell över livsålder + absolut energikoppling (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (600) | hjR² (1500) | hjR² (2700) | fk (600) | fk (1500) | fk (2700) | hjR²>0 | Grind (v/d/E) | corr(s_E, E) | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2172_ARMAR.iter().enumerate() {
        let (r6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_r2).collect::<Vec<_>>());
        let (r15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_r2).collect::<Vec<_>>());
        let (r27, r27_q1, r27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (f6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_fk).collect::<Vec<_>>());
        let (f15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_fk).collect::<Vec<_>>());
        let (f27, f27_q1, f27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_energi).collect::<Vec<_>>());
        let (ce, ce1, ce3) = iqr_och_median(&corr_grid.iter().map(|r| r[ai]).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} | {} | {} [{}; {}] | {} | {} | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} [{}; {}] | {:.4} |",
            f3(r6), f3(r15), f3(r27), f3(r27_q1), f3(r27_q3),
            f3(f6), f3(f15), f3(f27), f3(f27_q1), f3(f27_q3),
            f3(gv), f3(gd), f3(ge), f3(ce), f3(ce1), f3(ce3), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Inom-individ extern handlingsprobering (`10` tidssnitt/individ, median [Q1; Q3] över världar)\n");
    let _ = writeln!(t, "| Arm | R²(y_a -> intag) | R²(y_a -> ΔE) | R²(z'_a -> intag) | R²(z'_a -> ΔE) | Spårade individer (med) |");
    let _ = writeln!(t, "|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2172_ARMAR.iter().enumerate() {
        let (pyi, pyi1, pyi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_intag).collect::<Vec<_>>());
        let (pyd, pyd1, pyd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_de).collect::<Vec<_>>());
        let (pzi, pzi1, pzi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_intag).collect::<Vec<_>>());
        let (pzd, pzd1, pzd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_de).collect::<Vec<_>>());
        let ni_med = median(ni_grid.iter().map(|r| r[ai] as f64).collect());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {:.0} |",
            f3(pyi), f3(pyi1), f3(pyi3),
            f3(pyd), f3(pyd1), f3(pyd3),
            f3(pzi), f3(pzi1), f3(pzi3),
            f3(pzd), f3(pzd1), f3(pzd3),
            ni_med,
        );
    }

    let _ = writeln!(t, "\n## 3. Ekologiska mått (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2172_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2172);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2172);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2172);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2172);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2172);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2172);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2172);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 4. Parvisa jämförelser mot `D2_Bas` samt urvalskandidat\n");
    let mut bast: Option<(usize, usize, f64)> = None;
    let par_krav = (n * 3).div_ceil(5); // 12 av 20
    let ej_samre_krav = (n * 2).div_ceil(5); // 8 av 20
    for ai in 1..4 {
        let namn = SPARB2172_ARMAR[ai].0;
        let w_r27 = grid.iter().filter(|r| r[ai].d2700.hj_r2 > r[0].d2700.hj_r2).count();
        let w_r6 = grid.iter().filter(|r| r[ai].d600.hj_r2 > r[0].d600.hj_r2).count();
        let w_fk = grid.iter().filter(|r| r[ai].d2700.hj_fk < r[0].d2700.hj_fk).count();
        let w_py = grid.iter().filter(|r| r[ai].d2700.probe_y_intag > r[0].d2700.probe_y_intag).count();
        let w_ce = (0..n).filter(|&fi| corr_grid[fi][ai] > corr_grid[fi][0]).count();
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (r2m, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (fkm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let (gdm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let max_vinst = w_r27.max(w_r6).max(w_fk).max(w_py);
        let ok = pos >= krav && r2m >= 0.080 && fkm < 0.800 && gdm < 0.050 && max_vinst >= par_krav && w_r27 >= ej_samre_krav;
        let _ = writeln!(
            t,
            "- **{namn} vs D2_Bas:** hjR²(2700) **{w_r27}/{n}**, hjR²(600) **{w_r6}/{n}**, lägre fk(2700) **{w_fk}/{n}**, probe(y->intag) **{w_py}/{n}**, högre corr(s_E, E) **{w_ce}/{n}** | urvalskrav: **{}**",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok {
            let byta = match bast {
                None => true,
                Some((_, bw, br)) => w_r27 > bw || (w_r27 == bw && r2m > br),
            };
            if byta {
                bast = Some((ai, w_r27, r2m));
            }
        }
    }
    let val_txt = match bast {
        Some((ai, _, r2m)) => format!("{} (median hjR² = {}) — redo för bekräftelse på frö 321–340", SPARB2172_ARMAR[ai].0, f3(r2m)),
        None => "INGEN (behåll D2_Bas oförändrad inför Test 3)".to_string(),
    };
    let _ = writeln!(t, "\n- **Vald kandidat på utvecklingsmängden:** **{val_txt}**");

    live_logg_med(EGEN_LOGG_2172, "Alt 2172", &format!("KLAR sparb2172 ({sec:.1}s): {val_txt}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2199 Test 3 (tillståndsberoende osäkerhetsutforskning)

const SPARB2199_ARMAR: [(&'static str, Formaga); 4] = [
    ("D2_Bas", Formaga::Lar2144D2(0.20, 40)),
    ("D2_Osak20", Formaga::Lar2199Osak(0.20, 40, 0)),
    ("D2_OsakAvt", Formaga::Lar2199Osak(0.20, 40, 1)),
    ("D2_UCB", Formaga::Lar2199Osak(1.50, 40, 2)),
];

fn kora_sparb2199(fron: &[u64], inkor_ar: i64, mat_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2199,
        "Alt 2199",
        &format!(
            "Startar Alt 2199 (Test 3: tillståndsberoende osäkerhetsutforskning på D2_a4) på {} världar (inkörning {inkor_ar} år, mätning {mat_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2199,
                            "Alt 2199",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 4);
    for ai in 0..4 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2144Utfall::default(); 4]; bas_arc.len()];
    let mut ni_grid = vec![vec![0usize; 4]; bas_arc.len()];
    let grid_mtx = Mutex::new((&mut grid, &mut ni_grid));
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2199_ARMAR[ai];
                let (u, _, ni) = sparb_4arm_en_arm(
                    sim_bas,
                    fro,
                    arm_namn,
                    formaga,
                    inkor_ar,
                    mat_ar,
                    EGEN_LOGG_2199,
                    "Alt 2199",
                    Some(&*prog_c),
                );
                let mut g = mtx_ref.lock().unwrap();
                g.0[fi][ai] = u;
                g.1[fi][ai] = ni;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4);
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2199 (Test 3: Tillståndsberoende osäkerhetsutforskning ovanpå `D2_a4`, {n} världar, {sec:.1}s)\n");
    let _ = writeln!(
        t,
        "Regler: piharness `docs/evolution/alt2199-utfallsregel.md`. Inkörning {inkor_ar} år + {mat_ar} år (`{} steg`).\n",
        mat_ar * STEG_PER_AR
    );

    let _ = writeln!(t, "## 1. Världsmodell över livsålder (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (600) | hjR² (1500) | hjR² (2700) | fk (600) | fk (1500) | fk (2700) | hjR²>0 | Grind (v/d/E) | Topp-1 | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2199_ARMAR.iter().enumerate() {
        let (r6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_r2).collect::<Vec<_>>());
        let (r15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_r2).collect::<Vec<_>>());
        let (r27, r27_q1, r27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (f6, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_fk).collect::<Vec<_>>());
        let (f15, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d1500.hj_fk).collect::<Vec<_>>());
        let (f27, f27_q1, f27_q3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_energi).collect::<Vec<_>>());
        let (t1, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].topp1_andel).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} | {} | {} [{}; {}] | {} | {} | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} | {:.4} |",
            f3(r6), f3(r15), f3(r27), f3(r27_q1), f3(r27_q3),
            f3(f6), f3(f15), f3(f27), f3(f27_q1), f3(f27_q3),
            f3(gv), f3(gd), f3(ge), f3(t1), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Inom-individ extern handlingsprobering (`10` tidssnitt/individ, median [Q1; Q3] över världar)\n");
    let _ = writeln!(t, "| Arm | R²(y_a -> intag) | R²(y_a -> ΔE) | R²(z'_a -> intag) | R²(z'_a -> ΔE) | Spårade individer (med) |");
    let _ = writeln!(t, "|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2199_ARMAR.iter().enumerate() {
        let (pyi, pyi1, pyi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_intag).collect::<Vec<_>>());
        let (pyd, pyd1, pyd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_y_de).collect::<Vec<_>>());
        let (pzi, pzi1, pzi3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_intag).collect::<Vec<_>>());
        let (pzd, pzd1, pzd3) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.probe_z_de).collect::<Vec<_>>());
        let ni_med = median(ni_grid.iter().map(|r| r[ai] as f64).collect());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {} [{}; {}] | {:.0} |",
            f3(pyi), f3(pyi1), f3(pyi3),
            f3(pyd), f3(pyd1), f3(pyd3),
            f3(pzi), f3(pzi1), f3(pzi3),
            f3(pzd), f3(pzd1), f3(pzd3),
            ni_med,
        );
    }

    let _ = writeln!(t, "\n## 3. Ekologiska mått (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2199_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2199);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2199);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2199);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2199);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2199);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2199);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2199);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 4. Parvisa jämförelser mot `D2_Bas` samt urvalskandidat\n");
    let mut bast: Option<(usize, f64, f64)> = None;
    let par_krav = (n * 3).div_ceil(5); // 12 av 20
    let ej_samre_krav = (n * 2).div_ceil(5); // 8 av 20
    for ai in 1..4 {
        let namn = SPARB2199_ARMAR[ai].0;
        let w_r27 = grid.iter().filter(|r| r[ai].d2700.hj_r2 > r[0].d2700.hj_r2).count();
        let w_r6 = grid.iter().filter(|r| r[ai].d600.hj_r2 > r[0].d600.hj_r2).count();
        let w_fk = grid.iter().filter(|r| r[ai].d2700.hj_fk < r[0].d2700.hj_fk).count();
        let w_py = grid.iter().filter(|r| r[ai].d2700.probe_y_intag > r[0].d2700.probe_y_intag).count();
        let w_pop = grid.iter().filter(|r| r[ai].slutpop > r[0].slutpop).count();
        let pos = grid.iter().filter(|r| r[ai].d2700.hj_r2 > 0.0).count();
        let (r2m, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_r2).collect::<Vec<_>>());
        let (r6m, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d600.hj_r2).collect::<Vec<_>>());
        let (fkm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.hj_fk).collect::<Vec<_>>());
        let (gdm, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].d2700.g_djur).collect::<Vec<_>>());
        let max_vinst = w_r27.max(w_r6).max(w_fk).max(w_py);
        let ok = pos >= krav && r2m >= 0.080 && fkm < 0.800 && gdm < 0.050 && max_vinst >= par_krav && w_r27 >= ej_samre_krav;
        let _ = writeln!(
            t,
            "- **{namn} vs D2_Bas:** hjR²(2700) **{w_r27}/{n}**, hjR²(600) **{w_r6}/{n}**, lägre fk(2700) **{w_fk}/{n}**, probe(y->intag) **{w_py}/{n}**, slutpop **{w_pop}/{n}** | urvalskrav: **{}**",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok {
            let byta = match bast {
                None => true,
                Some((_, br27, br6)) => r2m > br27 || ((r2m - br27).abs() < 1e-12 && r6m > br6),
            };
            if byta {
                bast = Some((ai, r2m, r6m));
            }
        }
    }
    let val_txt = match bast {
        Some((ai, r2m, _)) => format!("{} (median hjR² = {}) — redo för bekräftelse på frö 321–340", SPARB2199_ARMAR[ai].0, f3(r2m)),
        None => "INGEN (behåll D2_Bas oförändrad inför Test 4)".to_string(),
    };
    let _ = writeln!(t, "\n- **Vald kandidat på utvecklingsmängden:** **{val_txt}**");

    live_logg_med(EGEN_LOGG_2199, "Alt 2199", &format!("KLAR sparb2199 ({sec:.1}s): {val_txt}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2223 Test 4 (evolution av W_k och v med påslagen mot avstängd världsmodell)

const SPARB2223_ARMAR: [(&'static str, Formaga); 3] = [
    ("D2_Evo", Formaga::Lar2144D2(0.20, 40)),
    ("D2_Evo_Stryk", Formaga::Stryk2144D2(0.20, 40)),
    ("D2_SlumpGen", Formaga::Lar2144D2SlumpGen(0.20, 40)),
];

#[derive(Clone, Default)]
struct EvoPunkt2223 {
    ar: i64,
    pop: f64,
    medelintag: f64,
    vaxt_diff: f64,
}

#[derive(Clone, Default)]
struct Sparb2223Utfall {
    slut: Diag2144,
    topp1_andel: f64,
    medelintag: f64,
    medelpop: f64,
    slutpop: f64,
    medelenergi: f64,
    svalt_andel: f64,
    allv_traff: f64,
    andel_stanna: f64,
    vaxt_diff: f64,
    tidserie: Vec<EvoPunkt2223>,
}

fn sparb2223_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    formaga: Formaga,
    inkor_ar: i64,
    evo_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> Sparb2223Utfall {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta: formaga,
        omarkta: formaga,
        spegel: false,
    });
    sim.markering_skedd = true;
    for d in sim.djur.iter_mut().filter(|d| d.lev) {
        d.formaga = formaga;
        d.hj2059 = None;
        d.tabell = Minnestabell::ny();
    }

    let evo_steg = evo_ar.max(2) * STEG_PER_AR;
    let fonster_ar = 20.min((evo_ar / 2).max(1));
    let fonster_start = evo_steg - fonster_ar * STEG_PER_AR;
    let t_sen = evo_steg - 300.min((evo_steg / 3).max(1));
    let hb = (sim.b as i32) / 2;
    let hh = (sim.h as i32) / 2;

    let min_age_skala = |maal: i64| -> i64 {
        if inkor_ar >= 10 { maal } else { (inkor_ar * STEG_PER_AR).min(maal) }
    };
    let min_steg_skala = |t_nu: i64, maal: u64| -> u64 {
        if inkor_ar >= 10 { maal } else { ((t_nu as u64) / 2).max(20).min(maal) }
    };

    // 5 kontrollpunkter längs evolutionen (t.ex. år 20, 40, 60, 80, 100 vid 100 år)
    let mut knut_ar: Vec<i64> = (1..=5).map(|k| ((evo_ar * k) / 5).max(1)).collect();
    knut_ar.dedup();
    let mut nasta_knut = 0usize;
    let mut tidserie: Vec<EvoPunkt2223> = Vec::with_capacity(knut_ar.len());
    let (mut block_intag, mut block_n_intag) = (0.0f64, 0u64);
    let (mut block_vdiff, mut block_n_vdiff) = (0.0f64, 0u64);

    let (mut sum_intag, mut n_intag) = (0.0f64, 0u64);
    let (mut sum_pop, mut n_pop) = (0.0f64, 0u64);
    let (mut bas_svalt, mut bas_alder) = (0u64, 0u64);
    let (mut allv_ratt, mut allv_tot) = (0u64, 0u64);
    let (mut n_stanna_prov, mut n_steg_prov, mut sum_vdiff) = (0u64, 0u64, 0.0f64);
    let mut topp1 = f64::NAN;
    let mut slut_diag = Diag2144::default();

    for t in 0..evo_steg {
        if t == fonster_start {
            bas_svalt = sim.rak.dod_svalt;
            bas_alder = sim.rak.dod_alder;
        }
        let mut mal_par: Vec<(usize, u64, (usize, usize))> = Vec::new();
        if t >= fonster_start && t % STEG_PER_AR == 0 {
            for (idx, d) in sim.djur.iter().enumerate() {
                if d.lev {
                    let a_allv = sim.allvetande_handling(d.i, d.j);
                    let (di, dj) = GRANNAR[a_allv];
                    let (ni, nj) = sim.kant_granne(d.i as i32 + di, d.j as i32 + dj);
                    if ni >= 0 && nj >= 0 {
                        mal_par.push((idx, d.uid, (ni as usize, nj as usize)));
                        if mal_par.len() >= 80 {
                            break;
                        }
                    }
                }
            }
        }
        if t == t_sen {
            let mage = min_age_skala(1500);
            slut_diag = kor_hj2144_diag(&sim, fro, mage, min_steg_skala(t, 500), false);
            let mut top_v = Vec::new();
            for d in sim.djur.iter().filter(|d| d.lev && d.age >= mage) {
                if let Some(ref h) = d.hj2059 {
                    let sum_n: f64 = h.grind_n.iter().sum();
                    if sum_n > 0.0 {
                        let max_n = h.grind_n.iter().copied().fold(0.0f64, f64::max);
                        top_v.push(max_n / sum_n);
                    }
                }
            }
            if !top_v.is_empty() {
                topp1 = median(top_v);
            }
        }

        let i_slut = t >= fonster_start;
        let stickprov = (i_slut && t % 10 == 0) || (!i_slut && t % 50 == 0);
        let mut fore_pos: HashMap<u64, (usize, usize)> = HashMap::new();
        let mut v_norm_fore: Vec<f64> = Vec::new();
        if stickprov {
            v_norm_fore = sim
                .v
                .iter()
                .zip(sim.vmax.iter())
                .map(|(&v, &vmax)| v / vmax.max(1e-12))
                .collect();
            for d in sim.djur.iter().filter(|d| d.lev) {
                fore_pos.insert(d.uid, (d.i, d.j));
            }
        }

        sim.steg();
        if i_slut {
            sum_pop += sim.antal() as f64;
            n_pop += 1;
        }

        for (idx, uid, mal) in mal_par {
            let d = &sim.djur[idx];
            if d.lev && d.uid == uid {
                allv_tot += 1;
                if (d.i, d.j) == mal {
                    allv_ratt += 1;
                }
            }
        }

        for d in sim.djur.iter().filter(|d| d.lev) {
            if let Some(x) = d.forra_intag {
                block_intag += x;
                block_n_intag += 1;
                if i_slut {
                    sum_intag += x;
                    n_intag += 1;
                }
            }
            if stickprov {
                if let Some(&(i0, j0)) = fore_pos.get(&d.uid) {
                    let mut di = d.i as i32 - i0 as i32;
                    if di > hb { di -= sim.b as i32; } else if di < -hb { di += sim.b as i32; }
                    let mut dj = d.j as i32 - j0 as i32;
                    if dj > hh { dj -= sim.h as i32; } else if dj < -hh { dj += sim.h as i32; }
                    if di.abs() <= 1 && dj.abs() <= 1 {
                        let mut sv = 0.0;
                        for &(gi, gj) in &GRANNAR {
                            let (ni, nj) = sim.kant_granne(i0 as i32 + gi, j0 as i32 + gj);
                            let c = if ni >= 0 && nj >= 0 { sim.cell(ni as usize, nj as usize) } else { sim.cell(i0, j0) };
                            sv += v_norm_fore[c];
                        }
                        let vdiff = v_norm_fore[sim.cell(d.i, d.j)] - sv / 9.0;
                        block_vdiff += vdiff;
                        block_n_vdiff += 1;
                        if i_slut {
                            n_steg_prov += 1;
                            if di == 0 && dj == 0 {
                                n_stanna_prov += 1;
                            }
                            sum_vdiff += vdiff;
                        }
                    }
                }
            }
        }

        let ar_nu = (t + 1) / STEG_PER_AR;
        if (t + 1) % STEG_PER_AR == 0 && nasta_knut < knut_ar.len() && ar_nu == knut_ar[nasta_knut] {
            tidserie.push(EvoPunkt2223 {
                ar: ar_nu,
                pop: sim.antal() as f64,
                medelintag: if block_n_intag > 0 { block_intag / block_n_intag as f64 } else { f64::NAN },
                vaxt_diff: if block_n_vdiff > 0 { block_vdiff / block_n_vdiff as f64 } else { f64::NAN },
            });
            block_intag = 0.0;
            block_n_intag = 0;
            block_vdiff = 0.0;
            block_n_vdiff = 0;
            nasta_knut += 1;
        }
    }

    let lev_e: Vec<f64> = sim.djur.iter().filter(|d| d.lev).map(|d| d.e).collect();
    let ds = sim.rak.dod_svalt.saturating_sub(bas_svalt);
    let da = sim.rak.dod_alder.saturating_sub(bas_alder);
    let u = Sparb2223Utfall {
        slut: slut_diag,
        topp1_andel: topp1,
        medelintag: if n_intag > 0 { sum_intag / n_intag as f64 } else { f64::NAN },
        medelpop: if n_pop > 0 { sum_pop / n_pop as f64 } else { f64::NAN },
        slutpop: lev_e.len() as f64,
        medelenergi: if !lev_e.is_empty() { lev_e.iter().sum::<f64>() / lev_e.len() as f64 } else { f64::NAN },
        svalt_andel: if ds + da > 0 { ds as f64 / (ds + da) as f64 } else { f64::NAN },
        allv_traff: if allv_tot > 0 { allv_ratt as f64 / allv_tot as f64 } else { f64::NAN },
        andel_stanna: if n_steg_prov > 0 { n_stanna_prov as f64 / n_steg_prov as f64 } else { f64::NAN },
        vaxt_diff: if n_steg_prov > 0 { sum_vdiff / n_steg_prov as f64 } else { f64::NAN },
        tidserie,
    };

    let prog_txt = if let Some((klara_a, totalt, t_tot)) = progress {
        let k = klara_a.fetch_add(1, Ordering::Relaxed) + 1;
        let ganget = t_tot.elapsed().as_secs_f64();
        let kvar = if k > 0 && k < *totalt { ganget * (*totalt - k) as f64 / k as f64 } else { 0.0 };
        format!(" [klara {k}/{totalt}, kört {:.1} min, kvar ~{:.1} min]", ganget / 60.0, kvar / 60.0)
    } else {
        String::new()
    };
    live_logg_med(
        EGEN_LOGG_2223,
        "Alt 2223",
        &format!(
            "Frö {fro} {arm_namn} klar ({:.1}s): hjR²={} fk={} g_d={} intag={:.4} vdiff={:+.4} pop={:.0}{prog_txt}",
            t0.elapsed().as_secs_f64(),
            f3(u.slut.hj_r2),
            f3(u.slut.hj_fk),
            f3(u.slut.g_djur),
            u.medelintag,
            u.vaxt_diff,
            u.slutpop,
        ),
    );
    u
}

fn kora_sparb2223(fron: &[u64], inkor_ar: i64, evo_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2223,
        "Alt 2223",
        &format!(
            "Startar Alt 2223 (Test 4: evolution av ärvd värdeavläsare/synkodare med påslagen mot avstängd världsmodell) på {} världar (inkörning {inkor_ar} år, evolution {evo_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2059(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2223,
                            "Alt 2223",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 3);
    for ai in 0..3 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2223Utfall::default(); 3]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2223_ARMAR[ai];
                let u = sparb2223_en_arm(sim_bas, fro, arm_namn, formaga, inkor_ar, evo_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2223 (Test 4: Evolution av ärvd värdeavläsare och synkodare, {n} världar, {evo_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2223-utfallsregel.md`. Inkörning {inkor_ar} år + evolution {evo_ar} år.\n");

    let _ = writeln!(t, "## 1. Världsmodell vid slutperioden (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (slut) | fk (slut) | hjR²>0 | Grind (v/d/E) | Topp-1 | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        let (r2, r2_1, r2_3) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.hj_r2).collect::<Vec<_>>());
        let (fk, fk_1, fk_3) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].slut.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_energi).collect::<Vec<_>>());
        let (t1, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].topp1_andel).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} | {:.4} |",
            f3(r2), f3(r2_1), f3(r2_3), f3(fk), f3(fk_1), f3(fk_3), f3(gv), f3(gd), f3(ge), f3(t1), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Ekologiskt födosök och överlevnad i slutperioden (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2223);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2223);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2223);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2223);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2223);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2223);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2223);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 3. Tidsserie över evolutionen (median över {n} världar)\n");
    let _ = writeln!(t, "| Arm | År | Population | Medelintag | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        for ki in 0..n_knut {
            let ar = grid[0][ai].tidserie[ki].ar;
            let pm = median(grid.iter().map(|r| r[ai].tidserie[ki].pop).collect());
            let im = median(grid.iter().map(|r| r[ai].tidserie[ki].medelintag).collect());
            let vdm = median(grid.iter().map(|r| r[ai].tidserie[ki].vaxt_diff).collect());
            let _ = writeln!(t, "| {namn} | {ar} | {pm:.0} | {im:.4} | {vdm:+.4} |");
        }
    }

    // Låst utfallsbedömning (Krav 1, Krav 2, Krav 3)
    let pos_evo = grid.iter().filter(|r| r[0].slut.hj_r2 > 0.0).count();
    let (r2_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.hj_r2).collect::<Vec<_>>());
    let (fk_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.hj_fk).collect::<Vec<_>>());
    let (gd_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.g_djur).collect::<Vec<_>>());
    let (vd_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].vaxt_diff).collect::<Vec<_>>());
    let krav1 = pos_evo >= krav && r2_evo >= 0.080 && fk_evo < 0.800 && gd_evo < 0.050;

    let w_stryk_pop = grid.iter().filter(|r| r[0].slutpop > r[1].slutpop).count();
    let w_stryk_intag = grid.iter().filter(|r| r[0].medelintag > r[1].medelintag).count();
    let w_stryk_vd = grid.iter().filter(|r| r[0].vaxt_diff > r[1].vaxt_diff).count();
    let krav2 = w_stryk_pop >= krav && (w_stryk_intag >= krav || w_stryk_vd >= krav);

    let w_slump_vd = grid.iter().filter(|r| r[0].vaxt_diff > r[2].vaxt_diff).count();
    let w_slump_allv = grid.iter().filter(|r| r[0].allv_traff > r[2].allv_traff).count();
    let w_slump_intag = grid.iter().filter(|r| r[0].medelintag > r[2].medelintag).count();
    let w_slump_pop = grid.iter().filter(|r| r[0].slutpop > r[2].slutpop).count();
    let slump_vinster = [w_slump_vd >= krav, w_slump_allv >= krav, w_slump_intag >= krav, w_slump_pop >= krav]
        .iter()
        .filter(|&&b| b)
        .count();
    let krav3 = slump_vinster >= 2 && vd_evo > 0.005;

    let dom = if krav1 && krav2 && krav3 {
        "GRÖNT — redo för oberoende bekräftelse på frö 321–340"
    } else {
        "RÖTT / EJ VISAT — evolutionär världsmodellsnytta ej bekräftad i dagens jämna miljö"
    };

    let _ = writeln!(t, "\n## 4. Låst utfallsbedömning (krav {krav}/{n})\n");
    let _ = writeln!(t, "- **Krav 1 (Bevarad världsmodell i `D2_Evo`):** hjR²>0 **{pos_evo}/{n}**, median hjR²={}, fk={}, g_d={} -> **{}**", f3(r2_evo), f3(fk_evo), f3(gd_evo), if krav1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2 (`D2_Evo` vs `D2_Evo_Stryk`):** slutpop **{w_stryk_pop}/{n}**, intag **{w_stryk_intag}/{n}**, Växt-diff **{w_stryk_vd}/{n}** -> **{}**", if krav2 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 3 (`D2_Evo` vs `D2_SlumpGen`):** Växt-diff **{w_slump_vd}/{n}**, Allv-träff **{w_slump_allv}/{n}**, intag **{w_slump_intag}/{n}**, slutpop **{w_slump_pop}/{n}**, median Växt-diff `{vd_evo:+.4}` (> +0,005) -> **{}**", if krav3 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");

    live_logg_med(EGEN_LOGG_2223, "Alt 2223", &format!("KLAR sparb2223 ({sec:.1}s): {dom}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2238 (Ojämn växtåterväxt Alt 1954+1955 + evolutionstest med medelpop)

fn kora_sparb2238(fron: &[u64], inkor_ar: i64, evo_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2238,
        "Alt 2238",
        &format!(
            "Startar Alt 2238 (ojämn växtåterväxt Alt 1954+1955 + 100-årsevolution) på {} världar (inkörning {inkor_ar} år, evolution {evo_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2238(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2238,
                            "Alt 2238",
                            &format!("Inkörning (ojämn återväxt) frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 3);
    for ai in 0..3 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![Sparb2223Utfall::default(); 3]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (arm_namn, formaga) = SPARB2223_ARMAR[ai];
                let u = sparb2223_en_arm(sim_bas, fro, arm_namn, formaga, inkor_ar, evo_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2238 (Ojämn växtåterväxt Alt 1954+1955 + Evolutionstest, {n} världar, {evo_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2238-utfallsregel.md`. Inkörning {inkor_ar} år + evolution {evo_ar} år.\n");

    let _ = writeln!(t, "## 1. Världsmodell vid slutperioden (median [Q1; Q3] över {n} världar)\n");
    let _ = writeln!(t, "| Arm | hjR² (slut) | fk (slut) | hjR²>0 | Grind (v/d/E) | Topp-1 | Var(z') |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        let (r2, r2_1, r2_3) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.hj_r2).collect::<Vec<_>>());
        let (fk, fk_1, fk_3) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.hj_fk).collect::<Vec<_>>());
        let pos = grid.iter().filter(|r| r[ai].slut.hj_r2 > 0.0).count();
        let (gv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_vaxt).collect::<Vec<_>>());
        let (gd, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_djur).collect::<Vec<_>>());
        let (ge, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.g_energi).collect::<Vec<_>>());
        let (t1, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].topp1_andel).collect::<Vec<_>>());
        let (vv, _, _) = iqr_och_median(&grid.iter().map(|r| r[ai].slut.verk_var).collect::<Vec<_>>());
        let _ = writeln!(
            t,
            "| {namn} | {} [{}; {}] | {} [{}; {}] | {pos}/{n} | {} / {} / {} | {} | {:.4} |",
            f3(r2), f3(r2_1), f3(r2_3), f3(fk), f3(fk_1), f3(fk_3), f3(gv), f3(gd), f3(ge), f3(t1), vv
        );
    }

    let _ = writeln!(t, "\n## 2. Ekologiskt födosök och överlevnad i slutperioden (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelintag | Medelpop (år 80–100) | Slutpop | Medelenergi | Svältandel | Allv-träff | Andel stanna | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|");
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        let (im, il, ih) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelintag).collect::<Vec<_>>(), 2238);
        let (mpm, mpl, mph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelpop).collect::<Vec<_>>(), 2238);
        let (pm, pl, ph) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slutpop).collect::<Vec<_>>(), 2238);
        let (em, el, eh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medelenergi).collect::<Vec<_>>(), 2238);
        let (sm, sl, sh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].svalt_andel).collect::<Vec<_>>(), 2238);
        let (am, al, ah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].allv_traff).collect::<Vec<_>>(), 2238);
        let (stm, stl, sth) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].andel_stanna).collect::<Vec<_>>(), 2238);
        let (vdm, vdl, vdh) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].vaxt_diff).collect::<Vec<_>>(), 2238);
        let _ = writeln!(
            t,
            "| {namn} | {im:.4} [{il:.4}; {ih:.4}] | {mpm:.0} [{mpl:.0}; {mph:.0}] | {pm:.0} [{pl:.0}; {ph:.0}] | {em:.1} [{el:.1}; {eh:.1}] | {:.1} % [{:.1}; {:.1}] | {} [{}; {}] | {:.3} [{:.3}; {:.3}] | {:+.4} [{:+.4}; {:+.4}] |",
            sm * 100.0, sl * 100.0, sh * 100.0, f3(am), f3(al), f3(ah), stm, stl, sth, vdm, vdl, vdh
        );
    }

    let _ = writeln!(t, "\n## 3. Tidsserie över evolutionen (median över {n} världar)\n");
    let _ = writeln!(t, "| Arm | År | Population | Medelintag | Växt-diff |");
    let _ = writeln!(t, "|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for (ai, &(namn, _)) in SPARB2223_ARMAR.iter().enumerate() {
        for ki in 0..n_knut {
            let ar = grid[0][ai].tidserie[ki].ar;
            let pm = median(grid.iter().map(|r| r[ai].tidserie[ki].pop).collect());
            let im = median(grid.iter().map(|r| r[ai].tidserie[ki].medelintag).collect());
            let vdm = median(grid.iter().map(|r| r[ai].tidserie[ki].vaxt_diff).collect());
            let _ = writeln!(t, "| {namn} | {ar} | {pm:.0} | {im:.4} | {vdm:+.4} |");
        }
    }

    // Låst utfallsbedömning (Krav 1, Krav 2 med medelpop, Krav 3 med medelpop + Växt-diff > +0.005)
    let pos_evo = grid.iter().filter(|r| r[0].slut.hj_r2 > 0.0).count();
    let (r2_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.hj_r2).collect::<Vec<_>>());
    let (fk_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.hj_fk).collect::<Vec<_>>());
    let (gd_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].slut.g_djur).collect::<Vec<_>>());
    let (vd_evo, _, _) = iqr_och_median(&grid.iter().map(|r| r[0].vaxt_diff).collect::<Vec<_>>());
    let krav1 = pos_evo >= krav && r2_evo >= 0.080 && fk_evo < 0.800 && gd_evo < 0.050;

    let w_stryk_mpop = grid.iter().filter(|r| r[0].medelpop > r[1].medelpop).count();
    let w_stryk_intag = grid.iter().filter(|r| r[0].medelintag > r[1].medelintag).count();
    let w_stryk_vd = grid.iter().filter(|r| r[0].vaxt_diff > r[1].vaxt_diff).count();
    let krav2 = w_stryk_mpop >= krav && (w_stryk_intag >= krav || w_stryk_vd >= krav);

    let w_slump_vd = grid.iter().filter(|r| r[0].vaxt_diff > r[2].vaxt_diff).count();
    let w_slump_allv = grid.iter().filter(|r| r[0].allv_traff > r[2].allv_traff).count();
    let w_slump_intag = grid.iter().filter(|r| r[0].medelintag > r[2].medelintag).count();
    let w_slump_mpop = grid.iter().filter(|r| r[0].medelpop > r[2].medelpop).count();
    let slump_vinster = [w_slump_vd >= krav, w_slump_allv >= krav, w_slump_intag >= krav, w_slump_mpop >= krav]
        .iter()
        .filter(|&&b| b)
        .count();
    let krav3 = slump_vinster >= 2 && vd_evo > 0.005;

    let dom = if krav1 && krav2 && krav3 {
        "GRÖNT — redo för oberoende bekräftelse på frö 321–340"
    } else {
        "RÖTT / EJ VISAT — evolutionär världsmodellsnytta ej bekräftad i ojämn växtmiljö"
    };

    let _ = writeln!(t, "\n## 4. Låst utfallsbedömning (krav {krav}/{n})\n");
    let _ = writeln!(t, "- **Krav 1 (Bevarad världsmodell i `D2_Evo`):** hjR²>0 **{pos_evo}/{n}**, median hjR²={}, fk={}, g_d={} -> **{}**", f3(r2_evo), f3(fk_evo), f3(gd_evo), if krav1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2 (`D2_Evo` vs `D2_Evo_Stryk`):** medelpop **{w_stryk_mpop}/{n}**, intag **{w_stryk_intag}/{n}**, Växt-diff **{w_stryk_vd}/{n}** -> **{}**", if krav2 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 3 (`D2_Evo` vs `D2_SlumpGen`):** Växt-diff **{w_slump_vd}/{n}**, Allv-träff **{w_slump_allv}/{n}**, intag **{w_slump_intag}/{n}**, medelpop **{w_slump_mpop}/{n}**, median Växt-diff `{vd_evo:+.4}` (> +0,005) -> **{}**", if krav3 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");

    live_logg_med(EGEN_LOGG_2238, "Alt 2238", &format!("KLAR sparb2238 ({sec:.1}s): {dom}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2249 (Direkt 50/50-tävling i samma värld + spegel- och nollkontroll)

#[derive(Clone, Default)]
struct TavlingUtfall2249 {
    slut_andel_fokus: f64,
    medel_andel_fokus: f64,
    energi_fokus: f64,
    energi_mot: f64,
    intag_fokus: f64,
    intag_mot: f64,
    slutpop: f64,
    tidserie: Vec<(i64, f64)>,
}

fn sparb2249_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    markta: Formaga,
    omarkta: Formaga,
    fokus_ar_markt: bool,
    tavling_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> TavlingUtfall2249 {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta,
        omarkta,
        spegel: !fokus_ar_markt,
    });
    sim.markera_halften(fro);

    let total_steg = tavling_ar.max(2) * STEG_PER_AR;
    let fonster_ar = 10.min((tavling_ar / 2).max(1));
    let fonster_start = total_steg - fonster_ar * STEG_PER_AR;
    let mut knut_ar: Vec<i64> = (1..=5).map(|k| ((tavling_ar * k) / 5).max(1)).collect();
    knut_ar.dedup();
    let mut nasta_knut = 0usize;
    let mut tidserie: Vec<(i64, f64)> = Vec::with_capacity(knut_ar.len());

    let (mut sum_andel, mut n_andel) = (0.0f64, 0u64);
    let (mut sum_e_f, mut n_e_f, mut sum_e_m, mut n_e_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut sum_i_f, mut n_i_f, mut sum_i_m, mut n_i_m) = (0.0f64, 0u64, 0.0f64, 0u64);

    for t in 0..total_steg {
        sim.steg();
        let i_slut = t >= fonster_start;
        if i_slut {
            let (mut nf, mut nm) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                let ar_f = d.markt == fokus_ar_markt;
                if ar_f {
                    nf += 1;
                    sum_e_f += d.e;
                    n_e_f += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_f += x;
                        n_i_f += 1;
                    }
                } else {
                    nm += 1;
                    sum_e_m += d.e;
                    n_e_m += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_m += x;
                        n_i_m += 1;
                    }
                }
            }
            if nf + nm > 0 {
                sum_andel += nf as f64 / (nf + nm) as f64;
                n_andel += 1;
            }
        }
        let ar_nu = (t + 1) / STEG_PER_AR;
        if (t + 1) % STEG_PER_AR == 0 && nasta_knut < knut_ar.len() && ar_nu == knut_ar[nasta_knut] {
            let (mut nf, mut tot) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                tot += 1;
                if d.markt == fokus_ar_markt {
                    nf += 1;
                }
            }
            let andel = if tot > 0 { nf as f64 / tot as f64 } else { f64::NAN };
            tidserie.push((ar_nu, andel));
            nasta_knut += 1;
        }
    }

    let (mut nf_slut, mut tot_slut) = (0usize, 0usize);
    for d in sim.djur.iter().filter(|d| d.lev) {
        tot_slut += 1;
        if d.markt == fokus_ar_markt {
            nf_slut += 1;
        }
    }
    let u = TavlingUtfall2249 {
        slut_andel_fokus: if tot_slut > 0 { nf_slut as f64 / tot_slut as f64 } else { f64::NAN },
        medel_andel_fokus: if n_andel > 0 { sum_andel / n_andel as f64 } else { f64::NAN },
        energi_fokus: if n_e_f > 0 { sum_e_f / n_e_f as f64 } else { f64::NAN },
        energi_mot: if n_e_m > 0 { sum_e_m / n_e_m as f64 } else { f64::NAN },
        intag_fokus: if n_i_f > 0 { sum_i_f / n_i_f as f64 } else { f64::NAN },
        intag_mot: if n_i_m > 0 { sum_i_m / n_i_m as f64 } else { f64::NAN },
        slutpop: tot_slut as f64,
        tidserie,
    };

    let prog_txt = if let Some((klara_a, totalt, t_tot)) = progress {
        let k = klara_a.fetch_add(1, Ordering::Relaxed) + 1;
        let ganget = t_tot.elapsed().as_secs_f64();
        let kvar = if k > 0 && k < *totalt { ganget * (*totalt - k) as f64 / k as f64 } else { 0.0 };
        format!(" [klara {k}/{totalt}, kört {:.1} min, kvar ~{:.1} min]", ganget / 60.0, kvar / 60.0)
    } else {
        String::new()
    };
    live_logg_med(
        EGEN_LOGG_2249,
        "Alt 2249",
        &format!(
            "Frö {fro} {arm_namn} klar ({:.1}s): medelandel={:.1}% slutandel={:.1}% E={:.1}/{:.1} pop={:.0}{prog_txt}",
            t0.elapsed().as_secs_f64(),
            u.medel_andel_fokus * 100.0,
            u.slut_andel_fokus * 100.0,
            u.energi_fokus,
            u.energi_mot,
            u.slutpop,
        ),
    );
    u
}

fn kora_sparb2249(fron: &[u64], inkor_ar: i64, tavling_ar: i64, tradar: usize) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2249,
        "Alt 2249",
        &format!(
            "Startar Alt 2249 (direkt 50/50-tävling i samma värld, ojämn växtåterväxt) på {} världar (inkörning {inkor_ar} år, tävling {tavling_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2238(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2249,
                            "Alt 2249",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let d2_on = Formaga::Lar2144D2(0.20, 40);
    let d2_off = Formaga::Stryk2144D2(0.20, 40);
    let armar: [(&'static str, Formaga, Formaga, bool); 3] = [
        ("Tavling_A", d2_on, d2_off, true),
        ("Tavling_Spegel", d2_off, d2_on, false),
        ("Noll_Kontroll", d2_on, d2_on, true),
    ];

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 3);
    for ai in 0..3 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![TavlingUtfall2249::default(); 3]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (namn, fm_m, fm_o, fokus_m) = armar[ai];
                let u = sparb2249_en_arm(sim_bas, fro, namn, fm_m, fm_o, fokus_m, tavling_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2249: Direkt 50/50-tävling i samma värld ({n} världar, {tavling_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2249-utfallsregel.md`. Ojämn växtåterväxt (`Alt 1954` + `Alt 1955`). Inkörning {inkor_ar} år + tävling {tavling_ar} år.\n");

    let _ = writeln!(t, "## 1. Tävlingsutfall per arm (median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelandel världsmodell (sista 10 år) | Slutandel | Energi (VM / Mot) | Intag (VM / Mot) | Slutpop | >50 % |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|");
    for (ai, &(namn, _, _, _)) in armar.iter().enumerate() {
        let (ma, mal, mah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medel_andel_fokus).collect::<Vec<_>>(), 2249);
        let (sa, sal, sah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slut_andel_fokus).collect::<Vec<_>>(), 2249);
        let ef = median(grid.iter().map(|r| r[ai].energi_fokus).collect());
        let em = median(grid.iter().map(|r| r[ai].energi_mot).collect());
        let inf = median(grid.iter().map(|r| r[ai].intag_fokus).collect());
        let inm = median(grid.iter().map(|r| r[ai].intag_mot).collect());
        let sp = median(grid.iter().map(|r| r[ai].slutpop).collect());
        let w = grid.iter().filter(|r| r[ai].medel_andel_fokus > 0.50).count();
        let _ = writeln!(
            t,
            "| {namn} | {:.1} % [{:.1}; {:.1}] | {:.1} % [{:.1}; {:.1}] | {ef:.1} / {em:.1} | {inf:.4} / {inm:.4} | {sp:.0} | {w}/{n} |",
            ma * 100.0, mal * 100.0, mah * 100.0, sa * 100.0, sal * 100.0, sah * 100.0
        );
    }

    let par_medel: Vec<f64> = grid
        .iter()
        .map(|r| 0.5 * (r[0].medel_andel_fokus + r[1].medel_andel_fokus))
        .collect();
    let (pm, pml, pmh) = bootstrap_ci_seed(&par_medel, 2249);
    let w_a = grid.iter().filter(|r| r[0].medel_andel_fokus > 0.50).count();
    let w_s = grid.iter().filter(|r| r[1].medel_andel_fokus > 0.50).count();
    let w_par = par_medel.iter().filter(|&&x| x > 0.50).count();
    let noll_med = median(grid.iter().map(|r| r[2].medel_andel_fokus).collect());

    let _ = writeln!(t, "\n## 2. Tidsserie för världsmodellens andel (median över {n} världar)\n");
    let _ = writeln!(t, "| År | Tavling_A | Tavling_Spegel | Spegelmedel | Noll_Kontroll |");
    let _ = writeln!(t, "|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for ki in 0..n_knut {
        let ar = grid[0][0].tidserie[ki].0;
        let a_m = median(grid.iter().map(|r| r[0].tidserie[ki].1).collect());
        let s_m = median(grid.iter().map(|r| r[1].tidserie[ki].1).collect());
        let p_m = median(grid.iter().map(|r| 0.5 * (r[0].tidserie[ki].1 + r[1].tidserie[ki].1)).collect());
        let n_m = median(grid.iter().map(|r| r[2].tidserie[ki].1).collect());
        let _ = writeln!(
            t,
            "| {ar} | {:.1} % | {:.1} % | {:.1} % | {:.1} % |",
            a_m * 100.0, s_m * 100.0, p_m * 100.0, n_m * 100.0
        );
    }

    let krav1 = (0.45..=0.55).contains(&noll_med);
    let krav2 = w_a >= krav && w_s >= krav && w_par >= krav;
    let krav3 = pm >= 0.60 && pml > 0.55;
    let dom = if krav1 && krav2 && krav3 {
        "GRÖNT — världsmodellen vinner direkt evolutionär konkurrens i samma värld (redo för bekräftelse på frö 321–340)"
    } else {
        "RÖTT / EJ VISAT — direkt evolutionär seger ej bekräftad enligt låsta krav"
    };

    let _ = writeln!(t, "\n## 3. Låst utfallsbedömning (krav {krav}/{n})\n");
    let _ = writeln!(t, "- **Krav 1 (Neutral nollkontroll inom 45–55 %):** median `{:.1} %` -> **{}**", noll_med * 100.0, if krav1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2 (Seger >50 % i båda spegelriktningarna):** Tavling_A **{w_a}/{n}**, Tavling_Spegel **{w_s}/{n}**, Spegelmedel **{w_par}/{n}** -> **{}**", if krav2 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 3 (Effektstorlek ≥60 % och nedre 95 %-CI >55 %):** Spegelmedel `{:.1} % [{:.1}; {:.1}]` -> **{}**", pm * 100.0, pml * 100.0, pmh * 100.0, if krav3 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");

    live_logg_med(EGEN_LOGG_2249, "Alt 2249", &format!("KLAR sparb2249 ({sec:.1}s): {dom}"));
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2260 (100-årstävling kors vs linjeparning, dubbel nollkontroll och gendiagnostik)

#[derive(Clone, Default, Serialize, Deserialize)]
struct TidPunkt2260 {
    ar: i64,
    #[serde(with = "opt_nan")]
    andel: f64,
    #[serde(with = "opt_nan")]
    v_div: f64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct TavlingUtfall2260 {
    #[serde(with = "opt_nan")]
    slut_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    medel_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_mot: f64,
    #[serde(with = "opt_nan")]
    intag_fokus: f64,
    #[serde(with = "opt_nan")]
    intag_mot: f64,
    #[serde(with = "opt_nan")]
    slutpop: f64,
    #[serde(with = "opt_nan")]
    korspar_andel: f64,
    #[serde(with = "opt_nan")]
    v_norm_fokus: f64,
    #[serde(with = "opt_nan")]
    v_norm_mot: f64,
    #[serde(with = "opt_nan")]
    v_divergens: f64,
    tidserie: Vec<TidPunkt2260>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Block2260Json {
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
    rader: Vec<(u64, Vec<TavlingUtfall2260>)>,
}

fn mat_v_diagnostik(sim: &Sim, fokus_ar_markt: bool) -> (f64, f64, f64) {
    let v_start = hcsim::hjarna::NK + hcsim::hjarna::NP;
    let v_len = Z_N + 1;
    let mut norm_f = Vec::new();
    let mut norm_m = Vec::new();
    let mut sum_f = [0.0; Z_N + 1];
    let mut sum_m = [0.0; Z_N + 1];
    let (mut nf, mut nm) = (0usize, 0usize);
    for d in sim.djur.iter().filter(|d| d.lev) {
        if d.hj.gen.len() < v_start + v_len {
            continue;
        }
        let sl = &d.hj.gen[v_start..v_start + v_len];
        let norm = sl.iter().map(|x| x * x).sum::<f64>().sqrt();
        if d.markt == fokus_ar_markt {
            norm_f.push(norm);
            for k in 0..v_len {
                sum_f[k] += sl[k];
            }
            nf += 1;
        } else {
            norm_m.push(norm);
            for k in 0..v_len {
                sum_m[k] += sl[k];
            }
            nm += 1;
        }
    }
    let nf_med = if norm_f.is_empty() { f64::NAN } else { median(norm_f) };
    let nm_med = if norm_m.is_empty() { f64::NAN } else { median(norm_m) };
    let div = if nf > 0 && nm > 0 {
        (0..v_len)
            .map(|k| {
                let diff = sum_f[k] / nf as f64 - sum_m[k] / nm as f64;
                diff * diff
            })
            .sum::<f64>()
            .sqrt()
    } else {
        f64::NAN
    };
    (nf_med, nm_med, div)
}

fn sparb2260_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    markta: Formaga,
    omarkta: Formaga,
    fokus_ar_markt: bool,
    iso: bool,
    tavling_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> TavlingUtfall2260 {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.parning_inom_linje = iso;
    sim.parningar_totalt = 0;
    sim.parningar_kors = 0;
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta,
        omarkta,
        spegel: !fokus_ar_markt,
    });
    sim.markera_halften(fro);

    let total_steg = tavling_ar.max(2) * STEG_PER_AR;
    let fonster_ar = 10.min((tavling_ar / 2).max(1));
    let fonster_start = total_steg - fonster_ar * STEG_PER_AR;
    let mut knut_ar: Vec<i64> = (1..=5).map(|k| ((tavling_ar * k) / 5).max(1)).collect();
    knut_ar.dedup();
    let mut nasta_knut = 0usize;
    let mut tidserie: Vec<TidPunkt2260> = Vec::with_capacity(knut_ar.len());

    let (mut sum_andel, mut n_andel) = (0.0f64, 0u64);
    let (mut sum_e_f, mut n_e_f, mut sum_e_m, mut n_e_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut sum_i_f, mut n_i_f, mut sum_i_m, mut n_i_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut bas_par_tot, mut bas_par_kors) = (0u64, 0u64);

    for t in 0..total_steg {
        if t == fonster_start {
            bas_par_tot = sim.parningar_totalt;
            bas_par_kors = sim.parningar_kors;
        }
        sim.steg();
        if t >= fonster_start {
            let (mut nf, mut nm) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                if d.markt == fokus_ar_markt {
                    nf += 1;
                    sum_e_f += d.e;
                    n_e_f += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_f += x;
                        n_i_f += 1;
                    }
                } else {
                    nm += 1;
                    sum_e_m += d.e;
                    n_e_m += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_m += x;
                        n_i_m += 1;
                    }
                }
            }
            if nf + nm > 0 {
                sum_andel += nf as f64 / (nf + nm) as f64;
                n_andel += 1;
            }
        }
        let ar_nu = (t + 1) / STEG_PER_AR;
        if (t + 1) % STEG_PER_AR == 0 && nasta_knut < knut_ar.len() && ar_nu == knut_ar[nasta_knut] {
            let (mut nf, mut tot) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                tot += 1;
                if d.markt == fokus_ar_markt {
                    nf += 1;
                }
            }
            let andel = if tot > 0 { nf as f64 / tot as f64 } else { f64::NAN };
            let (_, _, div) = mat_v_diagnostik(&sim, fokus_ar_markt);
            tidserie.push(TidPunkt2260 { ar: ar_nu, andel, v_div: div });
            nasta_knut += 1;
        }
    }

    let (mut nf_slut, mut tot_slut) = (0usize, 0usize);
    for d in sim.djur.iter().filter(|d| d.lev) {
        tot_slut += 1;
        if d.markt == fokus_ar_markt {
            nf_slut += 1;
        }
    }
    let (vn_f, vn_m, v_div) = mat_v_diagnostik(&sim, fokus_ar_markt);
    let dp_tot = sim.parningar_totalt.saturating_sub(bas_par_tot);
    let dp_kors = sim.parningar_kors.saturating_sub(bas_par_kors);
    let u = TavlingUtfall2260 {
        slut_andel_fokus: if tot_slut > 0 { nf_slut as f64 / tot_slut as f64 } else { f64::NAN },
        medel_andel_fokus: if n_andel > 0 { sum_andel / n_andel as f64 } else { f64::NAN },
        energi_fokus: if n_e_f > 0 { sum_e_f / n_e_f as f64 } else { f64::NAN },
        energi_mot: if n_e_m > 0 { sum_e_m / n_e_m as f64 } else { f64::NAN },
        intag_fokus: if n_i_f > 0 { sum_i_f / n_i_f as f64 } else { f64::NAN },
        intag_mot: if n_i_m > 0 { sum_i_m / n_i_m as f64 } else { f64::NAN },
        slutpop: tot_slut as f64,
        korspar_andel: if dp_tot > 0 { dp_kors as f64 / dp_tot as f64 } else { 0.0 },
        v_norm_fokus: vn_f,
        v_norm_mot: vn_m,
        v_divergens: v_div,
        tidserie,
    };

    let prog_txt = if let Some((klara_a, totalt, t_tot)) = progress {
        let k = klara_a.fetch_add(1, Ordering::Relaxed) + 1;
        let ganget = t_tot.elapsed().as_secs_f64();
        let kvar = if k > 0 && k < *totalt { ganget * (*totalt - k) as f64 / k as f64 } else { 0.0 };
        format!(" [klara {k}/{totalt}, kört {:.1} min, kvar ~{:.1} min]", ganget / 60.0, kvar / 60.0)
    } else {
        String::new()
    };
    live_logg_med(
        EGEN_LOGG_2260,
        "Alt 2260",
        &format!(
            "Frö {fro} {arm_namn} klar ({:.1}s): medel={:.1}% slut={:.1}% korspar={:.1}% div={:.3} E={:.1}/{:.1} pop={:.0}{prog_txt}",
            t0.elapsed().as_secs_f64(),
            u.medel_andel_fokus * 100.0,
            u.slut_andel_fokus * 100.0,
            u.korspar_andel * 100.0,
            u.v_divergens,
            u.energi_fokus,
            u.energi_mot,
            u.slutpop,
        ),
    );
    u
}

fn formatera_rapport_2260(
    rader: &[(u64, Vec<TavlingUtfall2260>)],
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
) -> (String, String) {
    let arm_namn = ["Kors_A", "Kors_Spegel", "Kors_Noll", "Iso_A", "Iso_Spegel", "Iso_Noll"];
    let grid: Vec<&Vec<TavlingUtfall2260>> = rader.iter().map(|(_, v)| v).collect();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2260: 100-årig direkt 50/50-tävling (Korsparning vs Linjeparning, dubbel nollkontroll & gendiagnostik, {n} världar, {tavling_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2260-utfallsregel.md`. Ojämn växtåterväxt (`Alt 1954` + `Alt 1955`). Inkörning {inkor_ar} år + tävling {tavling_ar} år.\n");

    let _ = writeln!(t, "## 1. Tävlingsutfall och gendiagnostik per arm (sista 10 år, median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelandel VM | Slutandel | Energi (VM / Mot) | Intag (VM / Mot) | Korsparning | ‖v‖ (VM / Mot) | v-divergens | >50 % |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|");
    for (ai, &namn) in arm_namn.iter().enumerate() {
        let (ma, mal, mah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medel_andel_fokus).collect::<Vec<_>>(), 2260);
        let (sa, sal, sah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slut_andel_fokus).collect::<Vec<_>>(), 2260);
        let ef = median(grid.iter().map(|r| r[ai].energi_fokus).collect());
        let em = median(grid.iter().map(|r| r[ai].energi_mot).collect());
        let inf = median(grid.iter().map(|r| r[ai].intag_fokus).collect());
        let inm = median(grid.iter().map(|r| r[ai].intag_mot).collect());
        let kp = median(grid.iter().map(|r| r[ai].korspar_andel).collect());
        let vnf = median(grid.iter().map(|r| r[ai].v_norm_fokus).collect());
        let vnm = median(grid.iter().map(|r| r[ai].v_norm_mot).collect());
        let vdiv = median(grid.iter().map(|r| r[ai].v_divergens).collect());
        let w = grid.iter().filter(|r| r[ai].medel_andel_fokus > 0.50).count();
        let _ = writeln!(
            t,
            "| {namn} | {:.1} % [{:.1}; {:.1}] | {:.1} % [{:.1}; {:.1}] | {ef:.1} / {em:.1} | {inf:.4} / {inm:.4} | {:.1} % | {vnf:.3} / {vnm:.3} | {vdiv:.3} | {w}/{n} |",
            ma * 100.0, mal * 100.0, mah * 100.0, sa * 100.0, sal * 100.0, sah * 100.0, kp * 100.0
        );
    }

    let _ = writeln!(t, "\n## 2. Tidsserie över 100 år (spegelmedel och gen-divergens, median över {n} världar)\n");
    let _ = writeln!(t, "| År | Kors spegelmedel | Kors noll | Iso spegelmedel | Iso noll | Kors v-div | Iso v-div |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for ki in 0..n_knut {
        let ar = grid[0][0].tidserie[ki].ar;
        let km = median(grid.iter().map(|r| 0.5 * (r[0].tidserie[ki].andel + r[1].tidserie[ki].andel)).collect());
        let kn = median(grid.iter().map(|r| r[2].tidserie[ki].andel).collect());
        let im = median(grid.iter().map(|r| 0.5 * (r[3].tidserie[ki].andel + r[4].tidserie[ki].andel)).collect());
        let inoll = median(grid.iter().map(|r| r[5].tidserie[ki].andel).collect());
        let kd = median(grid.iter().map(|r| 0.5 * (r[0].tidserie[ki].v_div + r[1].tidserie[ki].v_div)).collect());
        let id = median(grid.iter().map(|r| 0.5 * (r[3].tidserie[ki].v_div + r[4].tidserie[ki].v_div)).collect());
        let _ = writeln!(
            t,
            "| {ar} | {:.1} % | {:.1} % | {:.1} % | {:.1} % | {kd:.3} | {id:.3} |",
            km * 100.0, kn * 100.0, im * 100.0, inoll * 100.0
        );
    }

    let noll_kors = median(grid.iter().map(|r| r[2].medel_andel_fokus).collect());
    let noll_iso = median(grid.iter().map(|r| r[5].medel_andel_fokus).collect());
    let k1 = (0.45..=0.55).contains(&noll_kors) && (0.45..=0.55).contains(&noll_iso);

    let kors_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[0].medel_andel_fokus + r[1].medel_andel_fokus)).collect();
    let iso_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[3].medel_andel_fokus + r[4].medel_andel_fokus)).collect();
    let delta_iso_kors: Vec<f64> = iso_par.iter().zip(kors_par.iter()).map(|(a, b)| a - b).collect();

    let (km, kml, kmh) = bootstrap_ci_seed(&kors_par, 2260);
    let (im, iml, imh) = bootstrap_ci_seed(&iso_par, 2260);
    let (dm, dml, dmh) = bootstrap_ci_seed(&delta_iso_kors, 2260);

    let wk_a = grid.iter().filter(|r| r[0].medel_andel_fokus > 0.50).count();
    let wk_s = grid.iter().filter(|r| r[1].medel_andel_fokus > 0.50).count();
    let wk_p = kors_par.iter().filter(|&&x| x > 0.50).count();
    let wk_e = grid.iter().filter(|r| (r[0].energi_fokus > r[0].energi_mot) && (r[1].energi_fokus > r[1].energi_mot)).count();
    let wk_i = grid.iter().filter(|r| (r[0].intag_fokus > r[0].intag_mot) && (r[1].intag_fokus > r[1].intag_mot)).count();
    let kors_gron = k1 && wk_a >= krav && wk_s >= krav && wk_p >= krav && km >= 0.60 && kml > 0.55 && wk_e >= krav && wk_i >= krav;

    let wi_a = grid.iter().filter(|r| r[3].medel_andel_fokus > 0.50).count();
    let wi_s = grid.iter().filter(|r| r[4].medel_andel_fokus > 0.50).count();
    let wi_p = iso_par.iter().filter(|&&x| x > 0.50).count();
    let wi_e = grid.iter().filter(|r| (r[3].energi_fokus > r[3].energi_mot) && (r[4].energi_fokus > r[4].energi_mot)).count();
    let wi_i = grid.iter().filter(|r| (r[3].intag_fokus > r[3].intag_mot) && (r[4].intag_fokus > r[4].intag_mot)).count();
    let iso_gron = k1 && wi_a >= krav && wi_s >= krav && wi_p >= krav && im >= 0.60 && iml > 0.55 && wi_e >= krav && wi_i >= krav;

    let dom = if kors_gron && iso_gron {
        "GRÖNT PÅ BÅDA (Kors + Iso) — direkt evolutionär seger även under öppet genflöde, redo för bekräftelse på frö 321–340"
    } else if kors_gron {
        "GRÖNT PÅ KORS — direkt evolutionär seger under öppet genflöde, redo för bekräftelse på frö 321–340"
    } else if iso_gron {
        "GRÖNT ENDAST PÅ ISO — världsmodellen vinner stort under linjeisolering men bromsas av genflöde vid öppen parning"
    } else {
        "RÖTT / EJ VISAT — varken Kors eller Iso nådde låsta 60 %-gränsen"
    };

    let _ = writeln!(t, "\n## 3. Låst utfallsbedömning (krav {krav}/{n}, bibehållen 60 %-gräns)\n");
    let _ = writeln!(t, "- **Krav 1 (Dubbel neutral nollkontroll inom 45–55 %):** Kors_Noll `{:.1} %`, Iso_Noll `{:.1} %` -> **{}**", noll_kors * 100.0, noll_iso * 100.0, if k1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Korsparning (K2+K3):** A **{wk_a}/{n}**, Spegel **{wk_s}/{n}**, Spegelmedel **{wk_p}/{n}** (`{:.1} % [{:.1}; {:.1}]`), Energi **{wk_e}/{n}**, Intag **{wk_i}/{n}** -> **{}**", km * 100.0, kml * 100.0, kmh * 100.0, if kors_gron { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Linjeparning / Iso (K2+K3):** A **{wi_a}/{n}**, Spegel **{wi_s}/{n}**, Spegelmedel **{wi_p}/{n}** (`{:.1} % [{:.1}; {:.1}]`), Energi **{wi_e}/{n}**, Intag **{wi_i}/{n}** -> **{}**", im * 100.0, iml * 100.0, imh * 100.0, if iso_gron { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Mekanismskillnad Δ (Iso − Kors):** `{:+.1} procentenheter [{:+.1}; {:+.1}]`", dm * 100.0, dml * 100.0, dmh * 100.0);
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");
    (t, dom.to_string())
}

fn kora_sparb2260(fron: &[u64], inkor_ar: i64, tavling_ar: i64, tradar: usize, ra_ut: Option<&Path>) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2260,
        "Alt 2260",
        &format!(
            "Startar Alt 2260 (100-årstävling kors vs linjeparning, 6 armar + gendiagnostik) på {} världar (inkörning {inkor_ar} år, tävling {tavling_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2238(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2260,
                            "Alt 2260",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let d2_on = Formaga::Lar2144D2(0.20, 40);
    let d2_off = Formaga::Stryk2144D2(0.20, 40);
    let armar: [(&'static str, Formaga, Formaga, bool, bool); 6] = [
        ("Kors_A", d2_on, d2_off, true, false),
        ("Kors_Spegel", d2_off, d2_on, false, false),
        ("Kors_Noll", d2_on, d2_on, true, false),
        ("Iso_A", d2_on, d2_off, true, true),
        ("Iso_Spegel", d2_off, d2_on, false, true),
        ("Iso_Noll", d2_on, d2_on, true, true),
    ];

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 6);
    for ai in 0..6 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![TavlingUtfall2260::default(); 6]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (namn, fm_m, fm_o, fokus_m, iso) = armar[ai];
                let u = sparb2260_en_arm(sim_bas, fro, namn, fm_m, fm_o, fokus_m, iso, tavling_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let rader: Vec<(u64, Vec<TavlingUtfall2260>)> = bas_arc
        .iter()
        .zip(grid.into_iter())
        .map(|((fro, _), rad)| (*fro, rad))
        .collect();
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = create_dir_all(par);
        }
        let block = Block2260Json { inkor_ar, tavling_ar, sec, rader: rader.clone() };
        let j = serde_json::to_string_pretty(&block).map_err(|e| format!("json 2260: {e}"))?;
        std::fs::write(p, j).map_err(|e| format!("skriv {}: {e}", p.display()))?;
    }
    let (t, dom) = formatera_rapport_2260(&rader, inkor_ar, tavling_ar, sec);
    live_logg_med(EGEN_LOGG_2260, "Alt 2260", &format!("KLAR sparb2260 ({sec:.1}s): {dom}"));
    Ok(t)
}

fn slaihops_2260(filer: &[PathBuf]) -> Result<String, String> {
    if filer.is_empty() {
        return Err("inga blockfiler angivna med --in".into());
    }
    let mut rader: Vec<(u64, Vec<TavlingUtfall2260>)> = Vec::new();
    let mut inkor_ar = None;
    let mut tavling_ar = None;
    let mut max_sec = 0.0f64;
    for f in filer {
        let s = std::fs::read_to_string(f).map_err(|e| format!("läs {}: {e}", f.display()))?;
        let b: Block2260Json = serde_json::from_str(&s).map_err(|e| format!("tolka {}: {e}", f.display()))?;
        match inkor_ar {
            None => inkor_ar = Some(b.inkor_ar),
            Some(x) if x != b.inkor_ar => return Err(format!("olika inkor_ar: {x} vs {}", b.inkor_ar)),
            _ => {}
        }
        match tavling_ar {
            None => tavling_ar = Some(b.tavling_ar),
            Some(x) if x != b.tavling_ar => return Err(format!("olika tavling_ar: {x} vs {}", b.tavling_ar)),
            _ => {}
        }
        if b.sec > max_sec {
            max_sec = b.sec;
        }
        rader.extend(b.rader);
    }
    rader.sort_by_key(|(fro, _)| *fro);
    for w in rader.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(format!("dubblettfrö {} i blockfilerna", w[0].0));
        }
    }
    if rader.is_empty() {
        return Err("inga världar i blockfilerna".into());
    }
    let (t, _) = formatera_rapport_2260(&rader, inkor_ar.unwrap(), tavling_ar.unwrap(), max_sec);
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2267 Gren B (LeCuns lärda kritiker: ärvd startinstinkt + livstids-RLS mot netto-ΔE)

#[derive(Clone, Default, Serialize, Deserialize)]
struct TidPunkt2267 {
    ar: i64,
    #[serde(with = "opt_nan")]
    andel: f64,
    #[serde(with = "opt_nan")]
    krit_r2: f64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct TavlingUtfall2267 {
    #[serde(with = "opt_nan")]
    slut_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    medel_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_mot: f64,
    #[serde(with = "opt_nan")]
    intag_fokus: f64,
    #[serde(with = "opt_nan")]
    intag_mot: f64,
    #[serde(with = "opt_nan")]
    slutpop: f64,
    #[serde(with = "opt_nan")]
    korspar_andel: f64,
    #[serde(with = "opt_nan")]
    krit_r2_fokus: f64,
    #[serde(with = "opt_nan")]
    krit_r2_mot: f64,
    #[serde(with = "opt_nan")]
    vaxt_diff_fokus: f64,
    #[serde(with = "opt_nan")]
    allv_traff_fokus: f64,
    tidserie: Vec<TidPunkt2267>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Block2267Json {
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
    rader: Vec<(u64, Vec<TavlingUtfall2267>)>,
}

fn mat_krit_och_beteende(sim: &Sim, fokus_ar_markt: bool) -> (f64, f64, f64, f64) {
    let mut r2_f = Vec::new();
    let mut r2_m = Vec::new();
    let mut vdiffs = Vec::new();
    let mut allv_hits = Vec::new();
    for (idx, d) in sim.djur.iter().enumerate() {
        if !d.lev {
            continue;
        }
        if let Some(ref hj2) = d.hj2059 {
            let kr2 = hj2.krit_r2();
            if kr2.is_finite() {
                if d.markt == fokus_ar_markt {
                    r2_f.push(kr2);
                } else {
                    r2_m.push(kr2);
                }
            }
            if d.markt == fokus_ar_markt && vdiffs.len() < 200 {
                let s = sim.sinnen_for(idx);
                let s_in = hj2.skala_grind(&s);
                let z = hj2.kod_online(&s_in);
                let mut varden = [0.0; A_N];
                let y_stryk = if hj2.cfg.stryk_handling_i_val {
                    Some(hj2.forutsag_medel(&z))
                } else {
                    None
                };
                for a in 0..A_N {
                    let y = y_stryk.unwrap_or_else(|| hj2.forutsag(&z, a));
                    varden[a] = hj2.varde(&y);
                }
                let ba = argmax(&varden);
                let mut vaxt = [0.0; A_N];
                let mut max_v = f64::NEG_INFINITY;
                let mut sum_v = 0.0;
                for a in 0..A_N {
                    vaxt[a] = s[2 * a];
                    sum_v += vaxt[a];
                    if vaxt[a] > max_v {
                        max_v = vaxt[a];
                    }
                }
                vdiffs.push(vaxt[ba] - sum_v / A_N as f64);
                let n_max = vaxt.iter().filter(|&&x| (x - max_v).abs() < 1e-12).count().max(1);
                allv_hits.push(if (vaxt[ba] - max_v).abs() < 1e-12 { 1.0 / n_max as f64 } else { 0.0 });
            }
        }
    }
    let kf = if r2_f.is_empty() { f64::NAN } else { median(r2_f) };
    let km = if r2_m.is_empty() { f64::NAN } else { median(r2_m) };
    let vd = if vdiffs.is_empty() { f64::NAN } else { vdiffs.iter().sum::<f64>() / vdiffs.len() as f64 };
    let ah = if allv_hits.is_empty() { f64::NAN } else { allv_hits.iter().sum::<f64>() / allv_hits.len() as f64 };
    (kf, km, vd, ah)
}

fn sparb2267_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    markta: Formaga,
    omarkta: Formaga,
    fokus_ar_markt: bool,
    tavling_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> TavlingUtfall2267 {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.parning_inom_linje = true;
    sim.parningar_totalt = 0;
    sim.parningar_kors = 0;
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta,
        omarkta,
        spegel: !fokus_ar_markt,
    });
    sim.markera_halften(fro);

    let total_steg = tavling_ar.max(2) * STEG_PER_AR;
    let fonster_ar = 10.min((tavling_ar / 2).max(1));
    let fonster_start = total_steg - fonster_ar * STEG_PER_AR;
    let mut knut_ar: Vec<i64> = (1..=5).map(|k| ((tavling_ar * k) / 5).max(1)).collect();
    knut_ar.dedup();
    let mut nasta_knut = 0usize;
    let mut tidserie: Vec<TidPunkt2267> = Vec::with_capacity(knut_ar.len());

    let (mut sum_andel, mut n_andel) = (0.0f64, 0u64);
    let (mut sum_e_f, mut n_e_f, mut sum_e_m, mut n_e_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut sum_i_f, mut n_i_f, mut sum_i_m, mut n_i_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut bas_par_tot, mut bas_par_kors) = (0u64, 0u64);

    for t in 0..total_steg {
        if t == fonster_start {
            bas_par_tot = sim.parningar_totalt;
            bas_par_kors = sim.parningar_kors;
        }
        sim.steg();
        if t >= fonster_start {
            let (mut nf, mut nm) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                if d.markt == fokus_ar_markt {
                    nf += 1;
                    sum_e_f += d.e;
                    n_e_f += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_f += x;
                        n_i_f += 1;
                    }
                } else {
                    nm += 1;
                    sum_e_m += d.e;
                    n_e_m += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_m += x;
                        n_i_m += 1;
                    }
                }
            }
            if nf + nm > 0 {
                sum_andel += nf as f64 / (nf + nm) as f64;
                n_andel += 1;
            }
        }
        let ar_nu = (t + 1) / STEG_PER_AR;
        if (t + 1) % STEG_PER_AR == 0 && nasta_knut < knut_ar.len() && ar_nu == knut_ar[nasta_knut] {
            let (mut nf, mut tot) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                tot += 1;
                if d.markt == fokus_ar_markt {
                    nf += 1;
                }
            }
            let andel = if tot > 0 { nf as f64 / tot as f64 } else { f64::NAN };
            let (kf, _, _, _) = mat_krit_och_beteende(&sim, fokus_ar_markt);
            tidserie.push(TidPunkt2267 { ar: ar_nu, andel, krit_r2: kf });
            nasta_knut += 1;
        }
    }

    let (mut nf_slut, mut tot_slut) = (0usize, 0usize);
    for d in sim.djur.iter().filter(|d| d.lev) {
        tot_slut += 1;
        if d.markt == fokus_ar_markt {
            nf_slut += 1;
        }
    }
    let (kf, km, vd, ah) = mat_krit_och_beteende(&sim, fokus_ar_markt);
    let dp_tot = sim.parningar_totalt.saturating_sub(bas_par_tot);
    let dp_kors = sim.parningar_kors.saturating_sub(bas_par_kors);
    let u = TavlingUtfall2267 {
        slut_andel_fokus: if tot_slut > 0 { nf_slut as f64 / tot_slut as f64 } else { f64::NAN },
        medel_andel_fokus: if n_andel > 0 { sum_andel / n_andel as f64 } else { f64::NAN },
        energi_fokus: if n_e_f > 0 { sum_e_f / n_e_f as f64 } else { f64::NAN },
        energi_mot: if n_e_m > 0 { sum_e_m / n_e_m as f64 } else { f64::NAN },
        intag_fokus: if n_i_f > 0 { sum_i_f / n_i_f as f64 } else { f64::NAN },
        intag_mot: if n_i_m > 0 { sum_i_m / n_i_m as f64 } else { f64::NAN },
        slutpop: tot_slut as f64,
        korspar_andel: if dp_tot > 0 { dp_kors as f64 / dp_tot as f64 } else { 0.0 },
        krit_r2_fokus: kf,
        krit_r2_mot: km,
        vaxt_diff_fokus: vd,
        allv_traff_fokus: ah,
        tidserie,
    };

    let prog_txt = if let Some((klara_a, totalt, t_tot)) = progress {
        let k = klara_a.fetch_add(1, Ordering::Relaxed) + 1;
        let ganget = t_tot.elapsed().as_secs_f64();
        let kvar = if k > 0 && k < *totalt { ganget * (*totalt - k) as f64 / k as f64 } else { 0.0 };
        format!(" [klara {k}/{totalt}, kört {:.1} min, kvar ~{:.1} min]", ganget / 60.0, kvar / 60.0)
    } else {
        String::new()
    };
    live_logg_med(
        EGEN_LOGG_2267,
        "Alt 2267",
        &format!(
            "Frö {fro} {arm_namn} klar ({:.1}s): medel={:.1}% slut={:.1}% kritR²={:.3} vdiff={:+.4} allv={:.3} E={:.1}/{:.1} pop={:.0}{prog_txt}",
            t0.elapsed().as_secs_f64(),
            u.medel_andel_fokus * 100.0,
            u.slut_andel_fokus * 100.0,
            u.krit_r2_fokus,
            u.vaxt_diff_fokus,
            u.allv_traff_fokus,
            u.energi_fokus,
            u.energi_mot,
            u.slutpop,
        ),
    );
    u
}

fn median_finita(v: Vec<f64>) -> f64 {
    let f: Vec<f64> = v.into_iter().filter(|x| x.is_finite()).collect();
    if f.is_empty() { f64::NAN } else { median(f) }
}

fn formatera_rapport_2267(
    rader: &[(u64, Vec<TavlingUtfall2267>)],
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
) -> (String, String) {
    let arm_namn = [
        "Krit_Stryk_A",
        "Krit_Stryk_Spegel",
        "Krit_Noll",
        "Krit_Evo_A",
        "Krit_Evo_Spegel",
        "Krit_Tabell_A",
    ];
    let grid: Vec<&Vec<TavlingUtfall2267>> = rader.iter().map(|(_, v)| v).collect();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2267 (Gren B): LeCuns lärda kritiker (ärvd startinstinkt + livstids-RLS mot netto-ΔE, {n} världar, {tavling_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2267-utfallsregel.md`. Ojämn växtåterväxt (`Alt 1954` + `Alt 1955`), linjeparning (`0 %` korsparning). Inkörning {inkor_ar} år + tävling {tavling_ar} år.\n");

    let _ = writeln!(t, "## 1. Tävlingsutfall och kritikerdiagnostik per arm (sista 10 år, median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelandel Krit | Slutandel | Energi (Krit / Mot) | Intag (Krit / Mot) | Krit R² (Krit / Mot) | Växt-diff | Allv-träff | >50 % |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|");
    for (ai, &namn) in arm_namn.iter().enumerate() {
        let (ma, mal, mah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medel_andel_fokus).collect::<Vec<_>>(), 2267);
        let (sa, sal, sah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slut_andel_fokus).collect::<Vec<_>>(), 2267);
        let ef = median_finita(grid.iter().map(|r| r[ai].energi_fokus).collect());
        let em = median_finita(grid.iter().map(|r| r[ai].energi_mot).collect());
        let inf = median_finita(grid.iter().map(|r| r[ai].intag_fokus).collect());
        let inm = median_finita(grid.iter().map(|r| r[ai].intag_mot).collect());
        let kf = median_finita(grid.iter().map(|r| r[ai].krit_r2_fokus).collect());
        let km = median_finita(grid.iter().map(|r| r[ai].krit_r2_mot).collect());
        let vd = median_finita(grid.iter().map(|r| r[ai].vaxt_diff_fokus).collect());
        let ah = median_finita(grid.iter().map(|r| r[ai].allv_traff_fokus).collect());
        let w = grid.iter().filter(|r| r[ai].medel_andel_fokus > 0.50).count();
        let km_txt = if km.is_finite() { format!("{km:.3}") } else { "—".to_string() };
        let _ = writeln!(
            t,
            "| {namn} | {:.1} % [{:.1}; {:.1}] | {:.1} % [{:.1}; {:.1}] | {ef:.1} / {em:.1} | {inf:.4} / {inm:.4} | {kf:.3} / {km_txt} | {vd:+.4} | {ah:.3} | {w}/{n} |",
            ma * 100.0, mal * 100.0, mah * 100.0, sa * 100.0, sal * 100.0, sah * 100.0
        );
    }

    let _ = writeln!(t, "\n## 2. Tidsserie (median över {n} världar)\n");
    let _ = writeln!(t, "| År | Krit vs Stryk (spegelmedel) | Krit Noll | Krit vs Evo (spegelmedel) | Krit vs Tabell | Krit R² |");
    let _ = writeln!(t, "|---|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for ki in 0..n_knut {
        let ar = grid[0][0].tidserie[ki].ar;
        let sm = median_finita(grid.iter().map(|r| 0.5 * (r[0].tidserie[ki].andel + r[1].tidserie[ki].andel)).collect());
        let kn = median_finita(grid.iter().map(|r| r[2].tidserie[ki].andel).collect());
        let em = median_finita(grid.iter().map(|r| 0.5 * (r[3].tidserie[ki].andel + r[4].tidserie[ki].andel)).collect());
        let tm = median_finita(grid.iter().map(|r| r[5].tidserie[ki].andel).collect());
        let kr = median_finita(grid.iter().map(|r| r[0].tidserie[ki].krit_r2).collect());
        let _ = writeln!(
            t,
            "| {ar} | {:.1} % | {:.1} % | {:.1} % | {:.1} % | {kr:.3} |",
            sm * 100.0, kn * 100.0, em * 100.0, tm * 100.0
        );
    }

    let noll = median_finita(grid.iter().map(|r| r[2].medel_andel_fokus).collect());
    let k1 = (0.45..=0.55).contains(&noll);

    let stryk_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[0].medel_andel_fokus + r[1].medel_andel_fokus)).collect();
    let evo_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[3].medel_andel_fokus + r[4].medel_andel_fokus)).collect();
    let tab_vec: Vec<f64> = grid.iter().map(|r| r[5].medel_andel_fokus).collect();

    let (sm, sml, smh) = bootstrap_ci_seed(&stryk_par, 2267);
    let (em, eml, emh) = bootstrap_ci_seed(&evo_par, 2267);
    let (tm, tml, tmh) = bootstrap_ci_seed(&tab_vec, 2267);

    let ws_a = grid.iter().filter(|r| r[0].medel_andel_fokus > 0.50).count();
    let ws_s = grid.iter().filter(|r| r[1].medel_andel_fokus > 0.50).count();
    let ws_p = stryk_par.iter().filter(|&&x| x > 0.50).count();
    let ws_e = grid.iter().filter(|r| (r[0].energi_fokus > r[0].energi_mot) && (r[1].energi_fokus > r[1].energi_mot)).count();
    let ws_i = grid.iter().filter(|r| (r[0].intag_fokus > r[0].intag_mot) && (r[1].intag_fokus > r[1].intag_mot)).count();
    let k2 = ws_a >= krav && ws_s >= krav && ws_p >= krav && sm >= 0.60 && sml > 0.55 && ws_e >= krav && ws_i >= krav;

    let we_a = grid.iter().filter(|r| r[3].medel_andel_fokus > 0.50).count();
    let we_s = grid.iter().filter(|r| r[4].medel_andel_fokus > 0.50).count();
    let k3 = we_a >= krav && we_s >= krav && em > 0.55 && eml > 0.50;

    let wt = grid.iter().filter(|r| r[5].medel_andel_fokus > 0.50).count();
    let k4 = wt >= krav && tm >= 0.60;

    let dom = if k1 && k2 && k3 && k4 {
        "GRÖNT (K1+K2+K3+K4) — världsmodell + lärd kritiker slår strykprov, ren evolution och minnestabell (redo för bekräftelse på frö 321–340)"
    } else if k1 && k2 && k4 {
        "DELVIS GRÖNT (K1+K2+K4, ej K3) — världsmodell + lärd kritiker vinner stort mot strykprov och tabell men slår inte ren evolution"
    } else {
        "RÖTT / EJ VISAT — världsmodell + lärd kritiker klarade inte samtliga låsta huvudkrav"
    };

    let _ = writeln!(t, "\n## 3. Låst utfallsbedömning (krav {krav}/{n})\n");
    let _ = writeln!(t, "- **Krav 1 (Neutral nollkontroll inom 45–55 %):** Krit_Noll `{:.1} %` -> **{}**", noll * 100.0, if k1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2 (Kausal världsmodellsvinst Krit vs Stryk ≥60 %):** A **{ws_a}/{n}**, Spegel **{ws_s}/{n}**, Spegelmedel **{ws_p}/{n}** (`{:.1} % [{:.1}; {:.1}]`), Energi **{ws_e}/{n}**, Intag **{ws_i}/{n}** -> **{}**", sm * 100.0, sml * 100.0, smh * 100.0, if k2 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 3 (Merverkan mot ren evolution Krit vs Evo >55 %):** A **{we_a}/{n}**, Spegel **{we_s}/{n}**, Spegelmedel `{:.1} % [{:.1}; {:.1}]` -> **{}**", em * 100.0, eml * 100.0, emh * 100.0, if k3 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 4 (Regel 14 mot minnestabell Krit vs Tabell ≥60 %):** **{wt}/{n}** (`{:.1} % [{:.1}; {:.1}]`) -> **{}**", tm * 100.0, tml * 100.0, tmh * 100.0, if k4 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");
    (t, dom.to_string())
}

fn kora_sparb2267(fron: &[u64], inkor_ar: i64, tavling_ar: i64, tradar: usize, ra_ut: Option<&Path>) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2267,
        "Alt 2267",
        &format!(
            "Startar Alt 2267 Gren B (LeCuns lärda kritiker, 6 armar + linjeparning) på {} världar (inkörning {inkor_ar} år, tävling {tavling_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2238(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2267,
                            "Alt 2267",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let k_on = Formaga::Lar2267Krit(0.20, 40);
    let k_off = Formaga::Stryk2267Krit(0.20, 40);
    let evo_on = Formaga::Lar2144D2(0.20, 40);
    let armar: [(&'static str, Formaga, Formaga, bool); 6] = [
        ("Krit_Stryk_A", k_on, k_off, true),
        ("Krit_Stryk_Spegel", k_off, k_on, false),
        ("Krit_Noll", k_on, k_on, true),
        ("Krit_Evo_A", k_on, evo_on, true),
        ("Krit_Evo_Spegel", evo_on, k_on, false),
        ("Krit_Tabell_A", k_on, Formaga::Tabell, true),
    ];

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 6);
    for ai in 0..6 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![TavlingUtfall2267::default(); 6]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (namn, fm_m, fm_o, fokus_m) = armar[ai];
                let u = sparb2267_en_arm(sim_bas, fro, namn, fm_m, fm_o, fokus_m, tavling_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let rader: Vec<(u64, Vec<TavlingUtfall2267>)> = bas_arc
        .iter()
        .zip(grid.into_iter())
        .map(|((fro, _), rad)| (*fro, rad))
        .collect();
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = create_dir_all(par);
        }
        let block = Block2267Json { inkor_ar, tavling_ar, sec, rader: rader.clone() };
        let j = serde_json::to_string_pretty(&block).map_err(|e| format!("json 2267: {e}"))?;
        std::fs::write(p, j).map_err(|e| format!("skriv {}: {e}", p.display()))?;
    }
    let (t, dom) = formatera_rapport_2267(&rader, inkor_ar, tavling_ar, sec);
    live_logg_med(EGEN_LOGG_2267, "Alt 2267", &format!("KLAR sparb2267 ({sec:.1}s): {dom}"));
    Ok(t)
}

fn slaihops_2267(filer: &[PathBuf]) -> Result<String, String> {
    if filer.is_empty() {
        return Err("inga blockfiler angivna med --in".into());
    }
    let mut rader: Vec<(u64, Vec<TavlingUtfall2267>)> = Vec::new();
    let mut inkor_ar = None;
    let mut tavling_ar = None;
    let mut max_sec = 0.0f64;
    for f in filer {
        let s = std::fs::read_to_string(f).map_err(|e| format!("läs {}: {e}", f.display()))?;
        let b: Block2267Json = serde_json::from_str(&s).map_err(|e| format!("tolka {}: {e}", f.display()))?;
        match inkor_ar {
            None => inkor_ar = Some(b.inkor_ar),
            Some(x) if x != b.inkor_ar => return Err(format!("olika inkor_ar: {x} vs {}", b.inkor_ar)),
            _ => {}
        }
        match tavling_ar {
            None => tavling_ar = Some(b.tavling_ar),
            Some(x) if x != b.tavling_ar => return Err(format!("olika tavling_ar: {x} vs {}", b.tavling_ar)),
            _ => {}
        }
        if b.sec > max_sec {
            max_sec = b.sec;
        }
        rader.extend(b.rader);
    }
    rader.sort_by_key(|(fro, _)| *fro);
    for w in rader.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(format!("dubblettfrö {} i blockfilerna", w[0].0));
        }
    }
    if rader.is_empty() {
        return Err("inga världar i blockfilerna".into());
    }
    let (t, _) = formatera_rapport_2267(&rader, inkor_ar.unwrap(), tavling_ar.unwrap(), max_sec);
    Ok(t)
}

// ---------------------------------------------------------------- Alt 2270 (Oberoende bekräftelse av lärd kritiker på osedda kartor 321–340, Iso + Kors + Tabell)

#[derive(Clone, Default, Serialize, Deserialize)]
struct TavlingUtfall2270 {
    #[serde(with = "opt_nan")]
    slut_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    medel_andel_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_fokus: f64,
    #[serde(with = "opt_nan")]
    energi_mot: f64,
    #[serde(with = "opt_nan")]
    intag_fokus: f64,
    #[serde(with = "opt_nan")]
    intag_mot: f64,
    fodslar_fokus: u64,
    fodslar_mot: u64,
    #[serde(with = "opt_nan")]
    slutpop: f64,
    #[serde(with = "opt_nan")]
    korspar_andel: f64,
    #[serde(with = "opt_nan")]
    krit_r2_fokus: f64,
    #[serde(with = "opt_nan")]
    krit_r2_mot: f64,
    #[serde(with = "opt_nan")]
    vaxt_diff_fokus: f64,
    #[serde(with = "opt_nan")]
    allv_traff_fokus: f64,
    tidserie: Vec<TidPunkt2267>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Block2270Json {
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
    rader: Vec<(u64, Vec<TavlingUtfall2270>)>,
}

fn sparb2270_en_arm(
    sim_bas: &Sim,
    fro: u64,
    arm_namn: &'static str,
    markta: Formaga,
    omarkta: Formaga,
    fokus_ar_markt: bool,
    iso: bool,
    tavling_ar: i64,
    progress: Option<&(AtomicUsize, usize, std::time::Instant)>,
) -> TavlingUtfall2270 {
    let t0 = std::time::Instant::now();
    let mut sim = sim_bas.clone();
    sim.parning_inom_linje = iso;
    sim.parningar_totalt = 0;
    sim.parningar_kors = 0;
    sim.fodslar_markt = 0;
    sim.fodslar_omarkt = 0;
    sim.hjarn_arm = Some(HjarnArm {
        namn: arm_namn,
        markta,
        omarkta,
        spegel: !fokus_ar_markt,
    });
    sim.markera_halften(fro);

    let total_steg = tavling_ar.max(2) * STEG_PER_AR;
    let fonster_ar = 10.min((tavling_ar / 2).max(1));
    let fonster_start = total_steg - fonster_ar * STEG_PER_AR;
    let mut knut_ar: Vec<i64> = (1..=5).map(|k| ((tavling_ar * k) / 5).max(1)).collect();
    knut_ar.dedup();
    let mut nasta_knut = 0usize;
    let mut tidserie: Vec<TidPunkt2267> = Vec::with_capacity(knut_ar.len());

    let (mut sum_andel, mut n_andel) = (0.0f64, 0u64);
    let (mut sum_e_f, mut n_e_f, mut sum_e_m, mut n_e_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut sum_i_f, mut n_i_f, mut sum_i_m, mut n_i_m) = (0.0f64, 0u64, 0.0f64, 0u64);
    let (mut bas_par_tot, mut bas_par_kors) = (0u64, 0u64);
    let (mut bas_fod_m, mut bas_fod_o) = (0u64, 0u64);

    for t in 0..total_steg {
        if t == fonster_start {
            bas_par_tot = sim.parningar_totalt;
            bas_par_kors = sim.parningar_kors;
            bas_fod_m = sim.fodslar_markt;
            bas_fod_o = sim.fodslar_omarkt;
        }
        sim.steg();
        if t >= fonster_start {
            let (mut nf, mut nm) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                if d.markt == fokus_ar_markt {
                    nf += 1;
                    sum_e_f += d.e;
                    n_e_f += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_f += x;
                        n_i_f += 1;
                    }
                } else {
                    nm += 1;
                    sum_e_m += d.e;
                    n_e_m += 1;
                    if let Some(x) = d.forra_intag {
                        sum_i_m += x;
                        n_i_m += 1;
                    }
                }
            }
            if nf + nm > 0 {
                sum_andel += nf as f64 / (nf + nm) as f64;
                n_andel += 1;
            }
        }
        let ar_nu = (t + 1) / STEG_PER_AR;
        if (t + 1) % STEG_PER_AR == 0 && nasta_knut < knut_ar.len() && ar_nu == knut_ar[nasta_knut] {
            let (mut nf, mut tot) = (0usize, 0usize);
            for d in sim.djur.iter().filter(|d| d.lev) {
                tot += 1;
                if d.markt == fokus_ar_markt {
                    nf += 1;
                }
            }
            let andel = if tot > 0 { nf as f64 / tot as f64 } else { f64::NAN };
            let (kf, _, _, _) = mat_krit_och_beteende(&sim, fokus_ar_markt);
            tidserie.push(TidPunkt2267 { ar: ar_nu, andel, krit_r2: kf });
            nasta_knut += 1;
        }
    }

    let (mut nf_slut, mut tot_slut) = (0usize, 0usize);
    for d in sim.djur.iter().filter(|d| d.lev) {
        tot_slut += 1;
        if d.markt == fokus_ar_markt {
            nf_slut += 1;
        }
    }
    let (kf, km, vd, ah) = mat_krit_och_beteende(&sim, fokus_ar_markt);
    let dp_tot = sim.parningar_totalt.saturating_sub(bas_par_tot);
    let dp_kors = sim.parningar_kors.saturating_sub(bas_par_kors);
    let df_m = sim.fodslar_markt.saturating_sub(bas_fod_m);
    let df_o = sim.fodslar_omarkt.saturating_sub(bas_fod_o);
    let (fod_f, fod_m) = if fokus_ar_markt { (df_m, df_o) } else { (df_o, df_m) };

    let u = TavlingUtfall2270 {
        slut_andel_fokus: if tot_slut > 0 { nf_slut as f64 / tot_slut as f64 } else { f64::NAN },
        medel_andel_fokus: if n_andel > 0 { sum_andel / n_andel as f64 } else { f64::NAN },
        energi_fokus: if n_e_f > 0 { sum_e_f / n_e_f as f64 } else { f64::NAN },
        energi_mot: if n_e_m > 0 { sum_e_m / n_e_m as f64 } else { f64::NAN },
        intag_fokus: if n_i_f > 0 { sum_i_f / n_i_f as f64 } else { f64::NAN },
        intag_mot: if n_i_m > 0 { sum_i_m / n_i_m as f64 } else { f64::NAN },
        fodslar_fokus: fod_f,
        fodslar_mot: fod_m,
        slutpop: tot_slut as f64,
        korspar_andel: if dp_tot > 0 { dp_kors as f64 / dp_tot as f64 } else { 0.0 },
        krit_r2_fokus: kf,
        krit_r2_mot: km,
        vaxt_diff_fokus: vd,
        allv_traff_fokus: ah,
        tidserie,
    };

    let prog_txt = if let Some((klara_a, totalt, t_tot)) = progress {
        let k = klara_a.fetch_add(1, Ordering::Relaxed) + 1;
        let ganget = t_tot.elapsed().as_secs_f64();
        let kvar = if k > 0 && k < *totalt { ganget * (*totalt - k) as f64 / k as f64 } else { 0.0 };
        format!(" [klara {k}/{totalt}, kört {:.1} min, kvar ~{:.1} min]", ganget / 60.0, kvar / 60.0)
    } else {
        String::new()
    };
    live_logg_med(
        EGEN_LOGG_2270,
        "Alt 2270",
        &format!(
            "Frö {fro} {arm_namn} klar ({:.1}s): medel={:.1}% slut={:.1}% föd={}/{} korspar={:.1}% kritR²={:.3} intag={:.3}/{:.3}{prog_txt}",
            t0.elapsed().as_secs_f64(),
            u.medel_andel_fokus * 100.0,
            u.slut_andel_fokus * 100.0,
            u.fodslar_fokus,
            u.fodslar_mot,
            u.korspar_andel * 100.0,
            u.krit_r2_fokus,
            u.intag_fokus,
            u.intag_mot,
        ),
    );
    u
}

fn formatera_rapport_2270(
    rader: &[(u64, Vec<TavlingUtfall2270>)],
    inkor_ar: i64,
    tavling_ar: i64,
    sec: f64,
) -> (String, String) {
    let arm_namn = [
        "Iso_Krit_Stryk_A",
        "Iso_Krit_Stryk_Spegel",
        "Iso_Krit_Noll",
        "Kors_Krit_Stryk_A",
        "Kors_Krit_Stryk_Spegel",
        "Kors_Krit_Noll",
        "Kors_Krit_Tabell_A",
    ];
    let grid: Vec<&Vec<TavlingUtfall2270>> = rader.iter().map(|(_, v)| v).collect();
    let n = grid.len();
    let krav = (n * 3).div_ceil(4); // 15 av 20
    let mut t = String::new();
    let _ = writeln!(t, "# Alt 2270: Oberoende bekräftelse av LeCuns lärda kritiker (Linjeparning + Fri parning + Tabell, {n} världar, {tavling_ar} år, {sec:.1}s)\n");
    let _ = writeln!(t, "Regler: piharness `docs/evolution/alt2270-utfallsregel.md`. Ojämn växtåterväxt (`Alt 1954` + `Alt 1955`). Inkörning {inkor_ar} år + tävling {tavling_ar} år.\n");

    let _ = writeln!(t, "## 1. Tävlingsutfall, födslar och kritikerdiagnostik per arm (sista 10 år, median [95 %-bootstrap-intervall])\n");
    let _ = writeln!(t, "| Arm | Medelandel Krit | Slutandel | Födslar (Krit / Mot) | Intag (Krit / Mot) | Energi (Krit / Mot) | Korsparning | Krit R² | >50 % |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|---|---|");
    for (ai, &namn) in arm_namn.iter().enumerate() {
        let (ma, mal, mah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].medel_andel_fokus).collect::<Vec<_>>(), 2270);
        let (sa, sal, sah) = bootstrap_ci_seed(&grid.iter().map(|r| r[ai].slut_andel_fokus).collect::<Vec<_>>(), 2270);
        let ff = median_finita(grid.iter().map(|r| r[ai].fodslar_fokus as f64).collect());
        let fm = median_finita(grid.iter().map(|r| r[ai].fodslar_mot as f64).collect());
        let inf = median_finita(grid.iter().map(|r| r[ai].intag_fokus).collect());
        let inm = median_finita(grid.iter().map(|r| r[ai].intag_mot).collect());
        let ef = median_finita(grid.iter().map(|r| r[ai].energi_fokus).collect());
        let em = median_finita(grid.iter().map(|r| r[ai].energi_mot).collect());
        let kp = median_finita(grid.iter().map(|r| r[ai].korspar_andel).collect());
        let kf = median_finita(grid.iter().map(|r| r[ai].krit_r2_fokus).collect());
        let w = grid.iter().filter(|r| r[ai].medel_andel_fokus > 0.50).count();
        let _ = writeln!(
            t,
            "| {namn} | {:.1} % [{:.1}; {:.1}] | {:.1} % [{:.1}; {:.1}] | {ff:.0} / {fm:.0} | {inf:.4} / {inm:.4} | {ef:.1} / {em:.1} | {:.1} % | {kf:.3} | {w}/{n} |",
            ma * 100.0, mal * 100.0, mah * 100.0, sa * 100.0, sal * 100.0, sah * 100.0, kp * 100.0
        );
    }

    let _ = writeln!(t, "\n## 2. Tidsserie (median över {n} världar)\n");
    let _ = writeln!(t, "| År | Iso Krit vs Stryk | Iso Noll | Kors Krit vs Stryk | Kors Noll | Kors Krit vs Tabell | Krit R² |");
    let _ = writeln!(t, "|---|---|---|---|---|---|---|");
    let n_knut = grid[0][0].tidserie.len();
    for ki in 0..n_knut {
        let ar = grid[0][0].tidserie[ki].ar;
        let iso_m = median_finita(grid.iter().map(|r| 0.5 * (r[0].tidserie[ki].andel + r[1].tidserie[ki].andel)).collect());
        let iso_n = median_finita(grid.iter().map(|r| r[2].tidserie[ki].andel).collect());
        let kor_m = median_finita(grid.iter().map(|r| 0.5 * (r[3].tidserie[ki].andel + r[4].tidserie[ki].andel)).collect());
        let kor_n = median_finita(grid.iter().map(|r| r[5].tidserie[ki].andel).collect());
        let tab_m = median_finita(grid.iter().map(|r| r[6].tidserie[ki].andel).collect());
        let kr = median_finita(grid.iter().map(|r| r[3].tidserie[ki].krit_r2).collect());
        let _ = writeln!(
            t,
            "| {ar} | {:.1} % | {:.1} % | {:.1} % | {:.1} % | {:.1} % | {kr:.3} |",
            iso_m * 100.0, iso_n * 100.0, kor_m * 100.0, kor_n * 100.0, tab_m * 100.0
        );
    }

    let noll_iso = median_finita(grid.iter().map(|r| r[2].medel_andel_fokus).collect());
    let noll_kors = median_finita(grid.iter().map(|r| r[5].medel_andel_fokus).collect());
    let k1 = (0.45..=0.55).contains(&noll_iso) && (0.45..=0.55).contains(&noll_kors);

    let iso_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[0].medel_andel_fokus + r[1].medel_andel_fokus)).collect();
    let kors_par: Vec<f64> = grid.iter().map(|r| 0.5 * (r[3].medel_andel_fokus + r[4].medel_andel_fokus)).collect();
    let tab_vec: Vec<f64> = grid.iter().map(|r| r[6].medel_andel_fokus).collect();

    let (im, iml, imh) = bootstrap_ci_seed(&iso_par, 2270);
    let (km, kml, kmh) = bootstrap_ci_seed(&kors_par, 2270);
    let (tm, tml, tmh) = bootstrap_ci_seed(&tab_vec, 2270);

    let wi_a = grid.iter().filter(|r| r[0].medel_andel_fokus > 0.50).count();
    let wi_s = grid.iter().filter(|r| r[1].medel_andel_fokus > 0.50).count();
    let wi_p = iso_par.iter().filter(|&&x| x > 0.50).count();
    let wi_i = grid.iter().filter(|r| (r[0].intag_fokus > r[0].intag_mot) && (r[1].intag_fokus > r[1].intag_mot)).count();
    let wi_f = grid.iter().filter(|r| (r[0].fodslar_fokus > r[0].fodslar_mot) && (r[1].fodslar_fokus > r[1].fodslar_mot)).count();
    let iso_ok = wi_a >= krav && wi_s >= krav && wi_p >= krav && im >= 0.60 && iml > 0.55 && wi_i >= krav && wi_f >= krav;

    let wk_a = grid.iter().filter(|r| r[3].medel_andel_fokus > 0.50).count();
    let wk_s = grid.iter().filter(|r| r[4].medel_andel_fokus > 0.50).count();
    let wk_p = kors_par.iter().filter(|&&x| x > 0.50).count();
    let wk_i = grid.iter().filter(|r| (r[3].intag_fokus > r[3].intag_mot) && (r[4].intag_fokus > r[4].intag_mot)).count();
    let wk_f = grid.iter().filter(|r| (r[3].fodslar_fokus > r[3].fodslar_mot) && (r[4].fodslar_fokus > r[4].fodslar_mot)).count();
    let kors_ok = wk_a >= krav && wk_s >= krav && wk_p >= krav && km >= 0.60 && kml > 0.55 && wk_i >= krav && wk_f >= krav;

    let wt = grid.iter().filter(|r| r[6].medel_andel_fokus > 0.50).count();
    let k3 = wt >= krav && tm >= 0.60 && tml > 0.55;

    let dom = if k1 && iso_ok && kors_ok && k3 {
        "BEKRÄFTAT GRÖNT PÅ BÅDA (Iso + Kors + Tabell) — världsmodell + lärd kritiker är oberoende bekräftad på osedda kartor"
    } else if k1 && iso_ok && k3 {
        "BEKRÄFTAT GRÖNT ENDAST PÅ ISO (Iso + Tabell PASS, Kors FAIL)"
    } else if k1 && kors_ok && k3 {
        "BEKRÄFTAT GRÖNT ENDAST PÅ KORS (Kors + Tabell PASS, Iso FAIL)"
    } else {
        "EJ BEKRÄFTAT / RÖTT — oberoende bekräftelse klarade inte låsta krav"
    };

    let _ = writeln!(t, "\n## 3. Låst utfallsbedömning (krav {krav}/{n})\n");
    let _ = writeln!(t, "- **Krav 1 (Dubbel neutral nollkontroll inom 45–55 %):** Iso_Noll `{:.1} %`, Kors_Noll `{:.1} %` -> **{}**", noll_iso * 100.0, noll_kors * 100.0, if k1 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2a (Linjeparning Iso_Krit vs Stryk ≥60 % + intag + födslar):** A **{wi_a}/{n}**, Spegel **{wi_s}/{n}**, Spegelmedel **{wi_p}/{n}** (`{:.1} % [{:.1}; {:.1}]`), Intag **{wi_i}/{n}**, Födslar **{wi_f}/{n}** -> **{}**", im * 100.0, iml * 100.0, imh * 100.0, if iso_ok { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 2b (Vanlig fri parning Kors_Krit vs Stryk ≥60 % + intag + födslar):** A **{wk_a}/{n}**, Spegel **{wk_s}/{n}**, Spegelmedel **{wk_p}/{n}** (`{:.1} % [{:.1}; {:.1}]`), Intag **{wk_i}/{n}**, Födslar **{wk_f}/{n}** -> **{}**", km * 100.0, kml * 100.0, kmh * 100.0, if kors_ok { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Krav 3 (Regel 14 mot minnestabell Kors_Krit vs Tabell ≥60 %):** **{wt}/{n}** (`{:.1} % [{:.1}; {:.1}]`) -> **{}**", tm * 100.0, tml * 100.0, tmh * 100.0, if k3 { "PASS" } else { "FAIL" });
    let _ = writeln!(t, "- **Samlad dom:** **{dom}**");
    (t, dom.to_string())
}

fn kora_sparb2270(fron: &[u64], inkor_ar: i64, tavling_ar: i64, tradar: usize, ra_ut: Option<&Path>) -> Result<String, String> {
    if fron.is_empty() {
        return Err("inga frön".into());
    }
    let t_start = std::time::Instant::now();
    live_logg_med(
        EGEN_LOGG_2270,
        "Alt 2270",
        &format!(
            "Startar Alt 2270 (oberoende bekräftelse av lärd kritiker: Iso + Kors + Tabell, 7 armar) på {} världar (inkörning {inkor_ar} år, tävling {tavling_ar} år, {tradar} trådar)",
            fron.len()
        ),
    );

    let next_f = AtomicUsize::new(0);
    let n_tr = tradar.max(1).min(fron.len());
    let mut bas_par: Vec<(u64, Sim)> = std::thread::scope(|s| -> Result<Vec<(u64, Sim)>, String> {
        let hs: Vec<_> = (0..n_tr)
            .map(|_| {
                s.spawn(|| -> Result<Vec<(u64, Sim)>, String> {
                    let mut out = Vec::new();
                    loop {
                        let fi = next_f.fetch_add(1, Ordering::Relaxed);
                        if fi >= fron.len() {
                            break;
                        }
                        let fro = fron[fi];
                        let t0 = std::time::Instant::now();
                        let mut sim = bygg_varld_2238(fro)?;
                        for _ in 0..inkor_ar * STEG_PER_AR {
                            sim.steg();
                        }
                        live_logg_med(
                            EGEN_LOGG_2270,
                            "Alt 2270",
                            &format!("Inkörning frö {fro} klar ({} djur, {:.1}s)", sim.antal(), t0.elapsed().as_secs_f64()),
                        );
                        out.push((fro, sim));
                    }
                    Ok(out)
                })
            })
            .collect();
        let mut all = Vec::with_capacity(fron.len());
        for h in hs {
            all.extend(h.join().map_err(|_| "inkörningstråd föll".to_string())??);
        }
        Ok(all)
    })?;
    bas_par.sort_by_key(|x| x.0);
    let bas_arc = Arc::new(bas_par);

    let k_on = Formaga::Lar2267Krit(0.20, 40);
    let k_off = Formaga::Stryk2267Krit(0.20, 40);
    let armar: [(&'static str, Formaga, Formaga, bool, bool); 7] = [
        ("Iso_Krit_Stryk_A", k_on, k_off, true, true),
        ("Iso_Krit_Stryk_Spegel", k_off, k_on, false, true),
        ("Iso_Krit_Noll", k_on, k_on, true, true),
        ("Kors_Krit_Stryk_A", k_on, k_off, true, false),
        ("Kors_Krit_Stryk_Spegel", k_off, k_on, false, false),
        ("Kors_Krit_Noll", k_on, k_on, true, false),
        ("Kors_Krit_Tabell_A", k_on, Formaga::Tabell, true, false),
    ];

    let mut jobb: Vec<(usize, usize)> = Vec::with_capacity(fron.len() * 7);
    for ai in 0..7 {
        for fi in 0..bas_arc.len() {
            jobb.push((fi, ai));
        }
    }
    let next_j = AtomicUsize::new(0);
    let progress = Arc::new((AtomicUsize::new(0), jobb.len(), std::time::Instant::now()));
    let mut grid = vec![vec![TavlingUtfall2270::default(); 7]; bas_arc.len()];
    let grid_mtx = Mutex::new(&mut grid);
    let n_jw = tradar.max(1).min(jobb.len());
    std::thread::scope(|s| {
        for _ in 0..n_jw {
            let bas_c = Arc::clone(&bas_arc);
            let prog_c = Arc::clone(&progress);
            let jobb_ref = &jobb;
            let next_ref = &next_j;
            let mtx_ref = &grid_mtx;
            s.spawn(move || loop {
                let ji = next_ref.fetch_add(1, Ordering::Relaxed);
                if ji >= jobb_ref.len() {
                    break;
                }
                let (fi, ai) = jobb_ref[ji];
                let (fro, ref sim_bas) = bas_c[fi];
                let (namn, fm_m, fm_o, fokus_m, iso) = armar[ai];
                let u = sparb2270_en_arm(sim_bas, fro, namn, fm_m, fm_o, fokus_m, iso, tavling_ar, Some(&*prog_c));
                mtx_ref.lock().unwrap()[fi][ai] = u;
            });
        }
    });

    let sec = t_start.elapsed().as_secs_f64();
    let rader: Vec<(u64, Vec<TavlingUtfall2270>)> = bas_arc
        .iter()
        .zip(grid.into_iter())
        .map(|((fro, _), rad)| (*fro, rad))
        .collect();
    if let Some(p) = ra_ut {
        if let Some(par) = p.parent() {
            let _ = create_dir_all(par);
        }
        let block = Block2270Json { inkor_ar, tavling_ar, sec, rader: rader.clone() };
        let j = serde_json::to_string_pretty(&block).map_err(|e| format!("json 2270: {e}"))?;
        std::fs::write(p, j).map_err(|e| format!("skriv {}: {e}", p.display()))?;
    }
    let (t, dom) = formatera_rapport_2270(&rader, inkor_ar, tavling_ar, sec);
    live_logg_med(EGEN_LOGG_2270, "Alt 2270", &format!("KLAR sparb2270 ({sec:.1}s): {dom}"));
    Ok(t)
}

fn slaihops_2270(filer: &[PathBuf]) -> Result<String, String> {
    if filer.is_empty() {
        return Err("inga blockfiler angivna med --in".into());
    }
    let mut rader: Vec<(u64, Vec<TavlingUtfall2270>)> = Vec::new();
    let mut inkor_ar = None;
    let mut tavling_ar = None;
    let mut max_sec = 0.0f64;
    for f in filer {
        let s = std::fs::read_to_string(f).map_err(|e| format!("läs {}: {e}", f.display()))?;
        let b: Block2270Json = serde_json::from_str(&s).map_err(|e| format!("tolka {}: {e}", f.display()))?;
        match inkor_ar {
            None => inkor_ar = Some(b.inkor_ar),
            Some(x) if x != b.inkor_ar => return Err(format!("olika inkor_ar: {x} vs {}", b.inkor_ar)),
            _ => {}
        }
        match tavling_ar {
            None => tavling_ar = Some(b.tavling_ar),
            Some(x) if x != b.tavling_ar => return Err(format!("olika tavling_ar: {x} vs {}", b.tavling_ar)),
            _ => {}
        }
        if b.sec > max_sec {
            max_sec = b.sec;
        }
        rader.extend(b.rader);
    }
    rader.sort_by_key(|(fro, _)| *fro);
    for w in rader.windows(2) {
        if w[0].0 == w[1].0 {
            return Err(format!("dubblettfrö {} i blockfilerna", w[0].0));
        }
    }
    if rader.is_empty() {
        return Err("inga världar i blockfilerna".into());
    }
    let (t, _) = formatera_rapport_2270(&rader, inkor_ar.unwrap(), tavling_ar.unwrap(), max_sec);
    Ok(t)
}

#[cfg(test)]
mod prov {
    use super::*;

    #[test]
    fn alt2159_extern_probe_r2_gron_pa_signal_och_rod_pa_blandning() {
        let mut g = Rng::new(2159);
        let mut x = Vec::with_capacity(90);
        let mut y = Vec::with_capacity(90);
        let mut grupp = Vec::with_capacity(90);
        for snap in 0..10usize {
            let mut sx = [[0.0; Z_N]; A_N];
            let mut sy = [0.0; A_N];
            let mut mx = [0.0; Z_N];
            let mut my = 0.0;
            for a in 0..A_N {
                for d in 0..Z_N {
                    sx[a][d] = g.f() * 2.0 - 1.0;
                    mx[d] += sx[a][d] / A_N as f64;
                }
                sy[a] = 2.0 * sx[a][0] - 1.5 * sx[a][1] + (g.f() - 0.5) * 0.1;
                my += sy[a] / A_N as f64;
            }
            for a in 0..A_N {
                x.push((0..Z_N).map(|d| sx[a][d] - mx[d]).collect());
                y.push(sy[a] - my);
                grupp.push(snap);
            }
        }
        let r2_sig = extern_probe_r2(&x, &y, &grupp);
        let mut y_bl = y.clone();
        g.blanda(&mut y_bl);
        let r2_rod = extern_probe_r2(&x, &y_bl, &grupp);
        eprintln!("alt2159 probe: signal R²={r2_sig:.4}, blandad R²={r2_rod:.4}");
        assert!(r2_sig > 0.90, "inom-individ-proben borde ge >0.90 på ren signal, fick {r2_sig}");
        assert!(r2_rod <= 0.05, "inom-individ-proben borde falla <=0.05 på blandat mål, fick {r2_rod}");
    }

    #[test]
    fn alt2267_block_sammanslagning_matchar_hel_korning_och_fangar_dubblett() {
        let gor_rad = |fro: u64, andel: f64| -> (u64, Vec<TavlingUtfall2267>) {
            let mut v = Vec::new();
            for ai in 0..6 {
                v.push(TavlingUtfall2267 {
                    slut_andel_fokus: andel,
                    medel_andel_fokus: if ai == 2 { 0.50 } else { andel },
                    energi_fokus: 80.0,
                    energi_mot: 70.0,
                    intag_fokus: 1.2,
                    intag_mot: 1.0,
                    slutpop: 10000.0,
                    korspar_andel: 0.0,
                    krit_r2_fokus: 0.25,
                    krit_r2_mot: if ai >= 3 { f64::NAN } else { 0.20 },
                    vaxt_diff_fokus: 0.01,
                    allv_traff_fokus: 0.22,
                    tidserie: vec![TidPunkt2267 { ar: 10, andel, krit_r2: 0.25 }],
                });
            }
            (fro, v)
        };
        let alla = vec![gor_rad(261, 0.65), gor_rad(262, 0.70), gor_rad(263, 0.68), gor_rad(264, 0.72)];
        let (hel, _) = formatera_rapport_2267(&alla, 10, 50, 12.0);
        let dir = std::env::temp_dir().join("alt2267_merge_prov");
        let _ = std::fs::create_dir_all(&dir);
        let f1 = dir.join("b1.json");
        let f2 = dir.join("b2.json");
        // Skriv i omkastad ordning för att bevisa frösortering och NaN-rundtur.
        let b1 = Block2267Json { inkor_ar: 10, tavling_ar: 50, sec: 10.0, rader: vec![alla[2].clone(), alla[0].clone()] };
        let b2 = Block2267Json { inkor_ar: 10, tavling_ar: 50, sec: 12.0, rader: vec![alla[3].clone(), alla[1].clone()] };
        std::fs::write(&f1, serde_json::to_string(&b1).unwrap()).unwrap();
        std::fs::write(&f2, serde_json::to_string(&b2).unwrap()).unwrap();
        let ihop = slaihops_2267(&[f2.clone(), f1.clone()]).unwrap();
        assert_eq!(ihop, hel, "sammanslagen blockrapport måste vara bitvis identisk med hel körning");
        assert!(slaihops_2267(&[f1.clone(), f1.clone()]).is_err(), "dubblettfrö måste avvisas");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn alt2270_block_sammanslagning_och_fodselkrav() {
        let gor_rad = |fro: u64, andel: f64, fod_f: u64, fod_m: u64| -> (u64, Vec<TavlingUtfall2270>) {
            let mut v = Vec::new();
            for ai in 0..7 {
                let ar_noll = ai == 2 || ai == 5;
                v.push(TavlingUtfall2270 {
                    slut_andel_fokus: if ar_noll { 0.50 } else { andel },
                    medel_andel_fokus: if ar_noll { 0.50 } else { andel },
                    energi_fokus: 85.0,
                    energi_mot: 86.0,
                    intag_fokus: 1.10,
                    intag_mot: 1.08,
                    fodslar_fokus: if ar_noll { 500 } else { fod_f },
                    fodslar_mot: if ar_noll { 500 } else { fod_m },
                    slutpop: 10000.0,
                    korspar_andel: if ai < 3 { 0.0 } else { 0.45 },
                    krit_r2_fokus: 0.60,
                    krit_r2_mot: if ai == 6 { f64::NAN } else { 0.60 },
                    vaxt_diff_fokus: 0.005,
                    allv_traff_fokus: 0.12,
                    tidserie: vec![TidPunkt2267 { ar: 10, andel: if ar_noll { 0.50 } else { andel }, krit_r2: 0.60 }],
                });
            }
            (fro, v)
        };
        let gron = vec![
            gor_rad(321, 0.70, 900, 400),
            gor_rad(322, 0.68, 880, 410),
            gor_rad(323, 0.71, 910, 390),
            gor_rad(324, 0.69, 890, 405),
        ];
        let (hel_g, dom_g) = formatera_rapport_2270(&gron, 10, 50, 15.0);
        assert!(dom_g.starts_with("BEKRÄFTAT GRÖNT PÅ BÅDA"), "väntade grön dom, fick {dom_g}");

        // Röd kontroll: om födslar vänder (fod_f < fod_m) måste domen bli RÖTT trots 70 % andel!
        let rod = vec![
            gor_rad(321, 0.70, 300, 600),
            gor_rad(322, 0.68, 300, 600),
            gor_rad(323, 0.71, 300, 600),
            gor_rad(324, 0.69, 300, 600),
        ];
        let (_, dom_r) = formatera_rapport_2270(&rod, 10, 50, 15.0);
        assert!(dom_r.starts_with("EJ BEKRÄFTAT / RÖTT"), "födslar < mot ska ge röd dom, fick {dom_r}");

        let dir = std::env::temp_dir().join("alt2270_merge_prov");
        let _ = std::fs::create_dir_all(&dir);
        let f1 = dir.join("b1.json");
        let f2 = dir.join("b2.json");
        let b1 = Block2270Json { inkor_ar: 10, tavling_ar: 50, sec: 12.0, rader: vec![gron[2].clone(), gron[0].clone()] };
        let b2 = Block2270Json { inkor_ar: 10, tavling_ar: 50, sec: 15.0, rader: vec![gron[3].clone(), gron[1].clone()] };
        std::fs::write(&f1, serde_json::to_string(&b1).unwrap()).unwrap();
        std::fs::write(&f2, serde_json::to_string(&b2).unwrap()).unwrap();
        let ihop = slaihops_2270(&[f2.clone(), f1.clone()]).unwrap();
        assert_eq!(ihop, hel_g, "sammanslagen Alt 2270-rapport måste vara bitvis identisk med hel körning");
        assert!(slaihops_2270(&[f1.clone(), f1.clone()]).is_err(), "dubblettfrö måste avvisas");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
