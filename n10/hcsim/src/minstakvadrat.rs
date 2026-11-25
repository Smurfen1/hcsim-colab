//! Minsta kvadrat och R² utan externa beroenden (normalekvationer + Cholesky).
//! Används av ledprovets G1/G2/G3 (Alt 1221).

use crate::sim::LedLoggRad;
use crate::tal::{A_N, S_N};

/// Resultat av en linjär anpassning på en handling.
#[derive(Clone, Debug)]
pub struct R2Rad {
    pub handling: usize,
    pub n_anp: usize,
    pub n_prov: usize,
    pub r2: Option<f64>,
    pub singulaer: bool,
    pub meddelande: String,
}

/// Samlad R² per gissare.
#[derive(Clone, Debug)]
pub struct R2Samling {
    pub namn: &'static str,
    pub per_handling: Vec<R2Rad>,
    /// Medel över handlingarna 1..8 (flyttarna).
    pub medel_flytt: Option<f64>,
    /// R² för stå kvar (handling 0).
    pub sta_kvar: Option<f64>,
}

fn cholesky(a: &mut [Vec<f64>]) -> Result<(), String> {
    let n = a.len();
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i][j];
            for k in 0..j {
                s -= a[i][k] * a[j][k];
            }
            if i == j {
                if s <= 1e-14 {
                    return Err(format!("singulär matris (Cholesky, diagonal {i} ≤ 0)"));
                }
                a[i][j] = s.sqrt();
            } else {
                if a[j][j].abs() < 1e-15 {
                    return Err(format!("singulär matris (Cholesky, pivot {j})"));
                }
                a[i][j] = s / a[j][j];
            }
        }
        for j in (i + 1)..n {
            a[i][j] = 0.0;
        }
    }
    Ok(())
}

fn los_cholesky(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = l.len();
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut s = b[i];
        for j in 0..i {
            s -= l[i][j] * y[j];
        }
        y[i] = s / l[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = y[i];
        for j in (i + 1)..n {
            s -= l[j][i] * x[j];
        }
        x[i] = s / l[i][i];
    }
    x
}

/// Löser `XᵀX β = Xᵀy`. Returnerar β eller fel om matrisen är singulär.
pub fn minstakvadrat(x: &[Vec<f64>], y: &[f64]) -> Result<Vec<f64>, String> {
    assert_eq!(x.len(), y.len());
    if x.is_empty() {
        return Err("inga rader för minsta kvadrat".into());
    }
    let p = x[0].len();
    for rad in x {
        assert_eq!(rad.len(), p);
    }
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for (rad, &yi) in x.iter().zip(y.iter()) {
        for i in 0..p {
            xty[i] += rad[i] * yi;
            for j in 0..p {
                xtx[i][j] += rad[i] * rad[j];
            }
        }
    }
    cholesky(&mut xtx)?;
    Ok(los_cholesky(&xtx, &xty))
}

/// R² = 1 − SS_res/SS_tot på givna rader med redan anpassade β.
pub fn r2_pa(x: &[Vec<f64>], y: &[f64], beta: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len());
    if y.is_empty() {
        return f64::NAN;
    }
    let medel = y.iter().sum::<f64>() / y.len() as f64;
    let mut ss_tot = 0.0;
    let mut ss_res = 0.0;
    for (rad, &yi) in x.iter().zip(y.iter()) {
        let pred: f64 = rad.iter().zip(beta.iter()).map(|(xi, bi)| xi * bi).sum();
        ss_res += (yi - pred) * (yi - pred);
        ss_tot += (yi - medel) * (yi - medel);
    }
    if ss_tot < 1e-18 {
        return f64::NAN;
    }
    1.0 - ss_res / ss_tot
}

fn overlappar(a: &[u64], b: &[u64]) -> bool {
    for &x in a {
        if b.contains(&x) {
            return true;
        }
    }
    false
}

pub fn bygg_x_g1(r: &LedLoggRad) -> Vec<f64> {
    let mut x = Vec::with_capacity(S_N + 1);
    x.extend_from_slice(&r.s);
    x.push(1.0);
    x
}

/// G2:s indata — måste vara hjärnvärdet (inte intaget). Mutanten "intag som indata" blir röd.
pub fn bygg_x_g2(r: &LedLoggRad) -> Vec<f64> {
    vec![r.hj_varde, 1.0]
}

/// Mutant för provet: G2 med intaget som indata (ska ge högt R² där riktiga G2 ger lågt).
pub fn bygg_x_g2_mutant_intag(r: &LedLoggRad) -> Vec<f64> {
    vec![r.intag_efter, 1.0]
}

fn bygg_x_g3(r: &LedLoggRad) -> Vec<f64> {
    vec![r.intag_fore, 1.0]
}

fn filtrera<'a>(rader: &'a [LedLoggRad], fron: &[u64], handling: usize) -> Vec<&'a LedLoggRad> {
    rader
        .iter()
        .filter(|r| r.handling == handling && fron.contains(&r.fro))
        .collect()
}

fn rakna_en(
    namn: &'static str,
    handling: usize,
    anp: &[&LedLoggRad],
    prov: &[&LedLoggRad],
    bygg: fn(&LedLoggRad) -> Vec<f64>,
) -> R2Rad {
    if anp.is_empty() || prov.is_empty() {
        return R2Rad {
            handling,
            n_anp: anp.len(),
            n_prov: prov.len(),
            r2: None,
            singulaer: false,
            meddelande: format!("{namn} handling {handling}: för få rader"),
        };
    }
    let x_anp: Vec<Vec<f64>> = anp.iter().map(|r| bygg(r)).collect();
    let y_anp: Vec<f64> = anp.iter().map(|r| r.intag_efter).collect();
    match minstakvadrat(&x_anp, &y_anp) {
        Ok(beta) => {
            let x_prov: Vec<Vec<f64>> = prov.iter().map(|r| bygg(r)).collect();
            let y_prov: Vec<f64> = prov.iter().map(|r| r.intag_efter).collect();
            let r2 = r2_pa(&x_prov, &y_prov, &beta);
            R2Rad {
                handling,
                n_anp: anp.len(),
                n_prov: prov.len(),
                r2: if r2.is_finite() { Some(r2) } else { None },
                singulaer: false,
                meddelande: String::new(),
            }
        }
        Err(e) => R2Rad {
            handling,
            n_anp: anp.len(),
            n_prov: prov.len(),
            r2: None,
            singulaer: true,
            meddelande: format!("{namn} handling {handling}: {e}"),
        },
    }
}

fn samla(
    namn: &'static str,
    rader: &[LedLoggRad],
    anp_fron: &[u64],
    prov_fron: &[u64],
    bygg: fn(&LedLoggRad) -> Vec<f64>,
) -> R2Samling {
    let mut per = Vec::with_capacity(A_N);
    for h in 0..A_N {
        let anp = filtrera(rader, anp_fron, h);
        let prov = filtrera(rader, prov_fron, h);
        per.push(rakna_en(namn, h, &anp, &prov, bygg));
    }
    let flytt: Vec<f64> = per[1..]
        .iter()
        .filter_map(|r| r.r2)
        .collect();
    let medel_flytt = if flytt.len() == 8 {
        Some(flytt.iter().sum::<f64>() / 8.0)
    } else if flytt.is_empty() {
        None
    } else {
        Some(flytt.iter().sum::<f64>() / flytt.len() as f64)
    };
    R2Samling {
        namn,
        sta_kvar: per[0].r2,
        per_handling: per,
        medel_flytt,
    }
}

/// Räknar G1/G2/G3. Vägrar om anpassnings- och provfrön överlappar.
pub fn rakna_alla(
    rader: &[LedLoggRad],
    anp_fron: &[u64],
    prov_fron: &[u64],
) -> Result<(R2Samling, R2Samling, R2Samling), String> {
    if rader.is_empty() {
        return Err("tom logg: inga rader att räkna R² på".into());
    }
    if overlappar(anp_fron, prov_fron) {
        return Err(
            "anpassningens och prövningens frön överlappar; vägrar R²".into(),
        );
    }
    let g1 = samla("G1", rader, anp_fron, prov_fron, bygg_x_g1);
    let g2 = samla("G2", rader, anp_fron, prov_fron, bygg_x_g2);
    let g3 = samla("G3", rader, anp_fron, prov_fron, bygg_x_g3);
    Ok((g1, g2, g3))
}

/// Tolkar en TSV-rad från ledprov1221-logg.tsv.
pub fn las_tsv_rad(rad: &str) -> Result<LedLoggRad, String> {
    let f: Vec<&str> = rad.split('\t').collect();
    if f.len() < 4 + S_N {
        return Err(format!("för få fält ({})", f.len()));
    }
    let fro: u64 = f[0]
        .parse()
        .map_err(|_| format!("fro: {}", f[0]))?;
    let handling: usize = f[1]
        .parse()
        .map_err(|_| format!("handling: {}", f[1]))?;
    let hj_varde: f64 = f[2]
        .parse()
        .map_err(|_| format!("hj_varde: {}", f[2]))?;
    let intag_fore: f64 = f[3]
        .parse()
        .map_err(|_| format!("intag_fore: {}", f[3]))?;
    let intag_efter: f64 = f[4]
        .parse()
        .map_err(|_| format!("intag_efter: {}", f[4]))?;
    let mut s = [0.0; S_N];
    for i in 0..S_N {
        s[i] = f[5 + i]
            .parse()
            .map_err(|_| format!("s{i}: {}", f[5 + i]))?;
    }
    Ok(LedLoggRad {
        fro,
        djur_id: 0,
        handling,
        s,
        hj_varde,
        hj_varden: [0.0; A_N],
        intag_fore,
        intag_efter,
    })
}

pub fn tsv_huvud() -> String {
    let mut h = String::from("fro\thandling\thj_varde\tintag_fore\tintag_efter");
    for i in 0..S_N {
        h.push_str(&format!("\ts{i}"));
    }
    h.push('\n');
    h
}

pub fn tsv_rad(r: &LedLoggRad) -> String {
    let mut s = format!(
        "{}\t{}\t{}\t{}\t{}",
        r.fro, r.handling, r.hj_varde, r.intag_fore, r.intag_efter
    );
    for &x in &r.s {
        s.push('\t');
        s.push_str(&format!("{x}"));
    }
    s.push('\n');
    s
}

pub fn las_tsv(text: &str) -> Result<Vec<LedLoggRad>, String> {
    let mut ut = Vec::new();
    for (i, rad) in text.lines().enumerate() {
        if i == 0 {
            if !rad.starts_with("fro\t") {
                return Err("TSV saknar förväntat huvud".into());
            }
            continue;
        }
        if rad.trim().is_empty() {
            continue;
        }
        ut.push(las_tsv_rad(rad).map_err(|e| format!("rad {}: {e}", i + 1))?);
    }
    Ok(ut)
}

/// G1-vikter per handling (β med S_N+1 tal), anpassade på givna frön.
#[derive(Clone, Debug)]
pub struct G1Vikter {
    /// β per handling; None om matrisen var singulär eller för få rader.
    pub beta: [Option<Vec<f64>>; A_N],
    pub singulaer: [bool; A_N],
    pub meddelande: [String; A_N],
    pub n_anp: [usize; A_N],
}

/// Anpassar G1 per handling på `anp_fron` (ledprovets frön 141–150).
pub fn anpassa_g1(rader: &[LedLoggRad], anp_fron: &[u64]) -> G1Vikter {
    let mut beta: [Option<Vec<f64>>; A_N] = std::array::from_fn(|_| None);
    let mut singulaer = [false; A_N];
    let mut meddelande: [String; A_N] = std::array::from_fn(|_| String::new());
    let mut n_anp = [0usize; A_N];
    for h in 0..A_N {
        let anp = filtrera(rader, anp_fron, h);
        n_anp[h] = anp.len();
        if anp.is_empty() {
            meddelande[h] = format!("G1 handling {h}: inga rader");
            continue;
        }
        let x: Vec<Vec<f64>> = anp.iter().map(|r| bygg_x_g1(r)).collect();
        let y: Vec<f64> = anp.iter().map(|r| r.intag_efter).collect();
        match minstakvadrat(&x, &y) {
            Ok(b) => beta[h] = Some(b),
            Err(e) => {
                singulaer[h] = true;
                meddelande[h] = format!("G1 handling {h}: {e}");
            }
        }
    }
    G1Vikter {
        beta,
        singulaer,
        meddelande,
        n_anp,
    }
}

/// G1:s gissning för handling `h` ur sinnena `s`.
pub fn g1_gissning(vikter: &G1Vikter, h: usize, s: &[f64]) -> Option<f64> {
    let b = vikter.beta[h].as_ref()?;
    assert_eq!(s.len(), S_N);
    assert_eq!(b.len(), S_N + 1);
    let mut pred = b[S_N];
    for j in 0..S_N {
        pred += b[j] * s[j];
    }
    Some(pred)
}

/// G1-gissningar för alla 9 handlingar ur samma sinnen.
pub fn g1_gissningar(vikter: &G1Vikter, s: &[f64]) -> Option<[f64; A_N]> {
    let mut ut = [0.0; A_N];
    for h in 0..A_N {
        ut[h] = g1_gissning(vikter, h, s)?;
    }
    Some(ut)
}

/// Träff: andelen av handlingarna med högst värde som också har högst G1-gissning.
/// Lika värden delar träffen (|A∩B|/|A|).
pub fn traff(varden: &[f64], gissningar: &[f64]) -> f64 {
    assert_eq!(varden.len(), gissningar.len());
    assert!(!varden.is_empty());
    let max_v = varden.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let max_g = gissningar.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut n_a = 0usize;
    let mut n_snitt = 0usize;
    for i in 0..varden.len() {
        if varden[i] == max_v {
            n_a += 1;
            if gissningar[i] == max_g {
                n_snitt += 1;
            }
        }
    }
    if n_a == 0 {
        0.0
    } else {
        n_snitt as f64 / n_a as f64
    }
}

/// Medelrang (1-baserad) med delad rang vid lika värden.
pub fn medelrang(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap_or(std::cmp::Ordering::Equal));
    let mut rang = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && x[idx[j]] == x[idx[i]] {
            j += 1;
        }
        let medel = ((i + 1) + j) as f64 / 2.0;
        for k in i..j {
            rang[idx[k]] = medel;
        }
        i = j;
    }
    rang
}

/// Spearman-rangkorrelation (Pearson på medelranger). NaN om variansen är 0.
pub fn spearman(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let n = a.len();
    if n < 2 {
        return f64::NAN;
    }
    let ra = medelrang(a);
    let rb = medelrang(b);
    let ma = ra.iter().sum::<f64>() / n as f64;
    let mb = rb.iter().sum::<f64>() / n as f64;
    let mut cov = 0.0;
    let mut va = 0.0;
    let mut vb = 0.0;
    for i in 0..n {
        let da = ra[i] - ma;
        let db = rb[i] - mb;
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    if va < 1e-18 || vb < 1e-18 {
        return f64::NAN;
    }
    cov / (va.sqrt() * vb.sqrt())
}

/// Per-rad resultat för rangprovet.
#[derive(Clone, Debug)]
pub struct RangRadUt {
    pub fro: u64,
    pub djur_id: u64,
    pub traff: f64,
    pub spearman: f64,
}

/// Räknar träff och Spearman per loggrad; hoppar över rader där G1 saknar vikter.
pub fn rang_per_rad(rader: &[LedLoggRad], vikter: &G1Vikter) -> Result<Vec<RangRadUt>, String> {
    if rader.is_empty() {
        return Err("tom logg: inga rader att räkna rang på".into());
    }
    for h in 0..A_N {
        if vikter.singulaer[h] {
            return Err(format!(
                "G1-matris singulär för handling {h}: {}",
                vikter.meddelande[h]
            ));
        }
        if vikter.beta[h].is_none() {
            return Err(format!(
                "G1 saknar vikter för handling {h}: {}",
                vikter.meddelande[h]
            ));
        }
    }
    let mut ut = Vec::with_capacity(rader.len());
    for r in rader {
        let giss = g1_gissningar(vikter, &r.s).ok_or_else(|| {
            format!("G1-gissning saknas (fro={}, djur={})", r.fro, r.djur_id)
        })?;
        let t = traff(&r.hj_varden, &giss);
        let sp = spearman(&r.hj_varden, &giss);
        ut.push(RangRadUt {
            fro: r.fro,
            djur_id: r.djur_id,
            traff: t,
            spearman: sp,
        });
    }
    Ok(ut)
}

/// Medel först per djur, sedan per frö.
#[derive(Clone, Debug)]
pub struct RangFroSam {
    pub fro: u64,
    pub traff: f64,
    pub spearman: f64,
    pub n_djur: usize,
    pub n_rader: usize,
}

pub fn rang_per_fro(rader: &[RangRadUt]) -> Vec<RangFroSam> {
    let mut fron: Vec<u64> = rader.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&RangRadUt> = rader.iter().filter(|r| r.fro == fro).collect();
        let mut djur: Vec<u64> = i_fro.iter().map(|r| r.djur_id).collect();
        djur.sort_unstable();
        djur.dedup();
        let mut sum_t = 0.0;
        let mut sum_s = 0.0;
        let mut n_s = 0usize;
        for &d in &djur {
            let rader_d: Vec<&RangRadUt> = i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
            let nt = rader_d.len() as f64;
            sum_t += rader_d.iter().map(|r| r.traff).sum::<f64>() / nt;
            let sp: Vec<f64> = rader_d
                .iter()
                .map(|r| r.spearman)
                .filter(|x| x.is_finite())
                .collect();
            if !sp.is_empty() {
                sum_s += sp.iter().sum::<f64>() / sp.len() as f64;
                n_s += 1;
            }
        }
        let n_djur = djur.len();
        ut.push(RangFroSam {
            fro,
            traff: if n_djur == 0 {
                f64::NAN
            } else {
                sum_t / n_djur as f64
            },
            spearman: if n_s == 0 {
                f64::NAN
            } else {
                sum_s / n_s as f64
            },
            n_djur,
            n_rader: i_fro.len(),
        });
    }
    ut
}

/// TSV-huvud för rangprovets logg (alla 9 hjärnvärden).
pub fn rang_tsv_huvud() -> String {
    let mut h = String::from("fro\tdjur_id\thandling\tintag_efter");
    for i in 0..S_N {
        h.push_str(&format!("\ts{i}"));
    }
    for a in 0..A_N {
        h.push_str(&format!("\thj{a}"));
    }
    h.push('\n');
    h
}

pub fn rang_tsv_rad(r: &LedLoggRad) -> String {
    let mut s = format!(
        "{}\t{}\t{}\t{}",
        r.fro, r.djur_id, r.handling, r.intag_efter
    );
    for &x in &r.s {
        s.push('\t');
        s.push_str(&format!("{x}"));
    }
    for &x in &r.hj_varden {
        s.push('\t');
        s.push_str(&format!("{x}"));
    }
    s.push('\n');
    s
}

/// Tolkar en rad ur rangprovets logg.
pub fn las_rang_tsv_rad(rad: &str) -> Result<LedLoggRad, String> {
    let f: Vec<&str> = rad.split('\t').collect();
    let behov = 4 + S_N + A_N;
    if f.len() < behov {
        return Err(format!("för få fält ({}, behöver {behov})", f.len()));
    }
    let fro: u64 = f[0].parse().map_err(|_| format!("fro: {}", f[0]))?;
    let djur_id: u64 = f[1].parse().map_err(|_| format!("djur_id: {}", f[1]))?;
    let handling: usize = f[2].parse().map_err(|_| format!("handling: {}", f[2]))?;
    let intag_efter: f64 = f[3].parse().map_err(|_| format!("intag_efter: {}", f[3]))?;
    let mut s = [0.0; S_N];
    for i in 0..S_N {
        s[i] = f[4 + i]
            .parse()
            .map_err(|_| format!("s{i}: {}", f[4 + i]))?;
    }
    let mut hj_varden = [0.0; A_N];
    for a in 0..A_N {
        hj_varden[a] = f[4 + S_N + a]
            .parse()
            .map_err(|_| format!("hj{a}: {}", f[4 + S_N + a]))?;
    }
    Ok(LedLoggRad {
        fro,
        djur_id,
        handling,
        s,
        hj_varde: hj_varden.get(handling).copied().unwrap_or(0.0),
        hj_varden,
        intag_fore: 0.0,
        intag_efter,
    })
}

pub fn las_rang_tsv(text: &str) -> Result<Vec<LedLoggRad>, String> {
    let mut ut = Vec::new();
    for (i, rad) in text.lines().enumerate() {
        if i == 0 {
            if !rad.starts_with("fro\tdjur_id\t") {
                return Err("rang-TSV saknar förväntat huvud".into());
            }
            continue;
        }
        if rad.trim().is_empty() {
            continue;
        }
        ut.push(las_rang_tsv_rad(rad).map_err(|e| format!("rad {}: {e}", i + 1))?);
    }
    Ok(ut)
}

/// Per-rad resultat för omläsningen Alt 1245.
#[derive(Clone, Debug)]
pub struct OmlasRadUt {
    pub fro: u64,
    pub djur_id: u64,
    pub handling: usize,
    /// Träff bland de 8 flyttarna; None om hjärnan stod kvar (räknas inte i flyttmedel).
    pub flytt_traff: Option<f64>,
    /// Spearman bland de 8 flyttarna; None om hjärnan stod kvar.
    pub flytt_spearman: Option<f64>,
    /// True om G1 rankar stå kvar högst.
    pub g1_sta_kvar_hogst: bool,
    /// intag_efter − medel av G1:s 9 gissningar.
    pub intag_minus_g1_medel: f64,
}

/// Medel först per djur, sedan per frö (Alt 1245).
#[derive(Clone, Debug)]
pub struct OmlasFroSam {
    pub fro: u64,
    /// Medel flyttträff; None om inga flyttrader («saknas»).
    pub flytt_traff: Option<f64>,
    pub flytt_spearman: Option<f64>,
    pub andel_g1_sta_kvar: f64,
    pub intag_minus_g1_medel: f64,
    pub n_djur: usize,
    pub n_flytt_rader: usize,
    pub n_rader: usize,
}

/// Räknar (a)/(b)/(c) per loggrad. Tom delmängd → fel.
pub fn omlas_per_rad(rader: &[LedLoggRad], vikter: &G1Vikter) -> Result<Vec<OmlasRadUt>, String> {
    if rader.is_empty() {
        return Err("tom delmängd: inga rader att omläsa".into());
    }
    for h in 0..A_N {
        if vikter.singulaer[h] {
            return Err(format!(
                "G1-matris singulär för handling {h}: {}",
                vikter.meddelande[h]
            ));
        }
        if vikter.beta[h].is_none() {
            return Err(format!(
                "G1 saknar vikter för handling {h}: {}",
                vikter.meddelande[h]
            ));
        }
    }
    let mut ut = Vec::with_capacity(rader.len());
    for r in rader {
        let giss = g1_gissningar(vikter, &r.s).ok_or_else(|| {
            format!("G1-gissning saknas (fro={}, djur={})", r.fro, r.djur_id)
        })?;
        let max_g = giss.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let g1_sta_kvar_hogst = giss[0] == max_g;
        let g1_medel = giss.iter().sum::<f64>() / A_N as f64;
        let (flytt_traff, flytt_spearman) = if r.handling == 0 {
            (None, None)
        } else {
            let hv = &r.hj_varden[1..];
            let gv = &giss[1..];
            (Some(traff(hv, gv)), Some(spearman(hv, gv)))
        };
        ut.push(OmlasRadUt {
            fro: r.fro,
            djur_id: r.djur_id,
            handling: r.handling,
            flytt_traff,
            flytt_spearman,
            g1_sta_kvar_hogst,
            intag_minus_g1_medel: r.intag_efter - g1_medel,
        });
    }
    Ok(ut)
}

/// Medel först per djur, sedan per frö. Frö utan flyttrader → flyttträff/Spearman «saknas».
pub fn omlas_per_fro(rader: &[OmlasRadUt]) -> Result<Vec<OmlasFroSam>, String> {
    if rader.is_empty() {
        return Err("tom delmängd: inga omläsningsrader".into());
    }
    let mut fron: Vec<u64> = rader.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&OmlasRadUt> = rader.iter().filter(|r| r.fro == fro).collect();
        let mut djur: Vec<u64> = i_fro.iter().map(|r| r.djur_id).collect();
        djur.sort_unstable();
        djur.dedup();
        let mut sum_t = 0.0;
        let mut sum_s = 0.0;
        let mut n_t = 0usize;
        let mut n_s = 0usize;
        let mut sum_sta = 0.0;
        let mut sum_diff = 0.0;
        let mut n_flytt = 0usize;
        for &d in &djur {
            let rader_d: Vec<&OmlasRadUt> =
                i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
            let nt = rader_d.len() as f64;
            sum_sta += rader_d.iter().filter(|r| r.g1_sta_kvar_hogst).count() as f64 / nt;
            sum_diff += rader_d.iter().map(|r| r.intag_minus_g1_medel).sum::<f64>() / nt;
            let flytt: Vec<&OmlasRadUt> = rader_d
                .iter()
                .copied()
                .filter(|r| r.flytt_traff.is_some())
                .collect();
            n_flytt += flytt.len();
            if !flytt.is_empty() {
                let nf = flytt.len() as f64;
                sum_t += flytt.iter().filter_map(|r| r.flytt_traff).sum::<f64>() / nf;
                n_t += 1;
                let sp: Vec<f64> = flytt
                    .iter()
                    .filter_map(|r| r.flytt_spearman)
                    .filter(|x| x.is_finite())
                    .collect();
                if !sp.is_empty() {
                    sum_s += sp.iter().sum::<f64>() / sp.len() as f64;
                    n_s += 1;
                }
            }
        }
        let n_djur = djur.len();
        ut.push(OmlasFroSam {
            fro,
            flytt_traff: if n_t == 0 {
                None
            } else {
                Some(sum_t / n_t as f64)
            },
            flytt_spearman: if n_s == 0 {
                None
            } else {
                Some(sum_s / n_s as f64)
            },
            andel_g1_sta_kvar: if n_djur == 0 {
                f64::NAN
            } else {
                sum_sta / n_djur as f64
            },
            intag_minus_g1_medel: if n_djur == 0 {
                f64::NAN
            } else {
                sum_diff / n_djur as f64
            },
            n_djur,
            n_flytt_rader: n_flytt,
            n_rader: i_fro.len(),
        });
    }
    Ok(ut)
}
