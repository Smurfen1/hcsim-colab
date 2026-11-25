//! Rättat instrument (Alt 1423) och orsaken i loggen (Alt 1424): omräkning på
//! `v1251-logg.tsv`, märkt sökning. Förberedelse för Alt 1425 ligger i `signal`.

use std::path::Path;

use crate::minstakvadrat;
use crate::rng::Rng;
use crate::signal::{
    handling0_matt, ingrepp_traff_negativ, ingrepp_traff_positiv, ingrepp_traff_trangsel,
    kvot_fran_matriser, kvot_fran_matriser_riktningar, las_logg_tsv, negativ_varden, positiv_varden,
    valj_fran_hj, Handling0Matt, MIN_RADER_DJUR,
};
use crate::sim::SignalLoggRad;
use crate::tal::{A_N, Z_N};

/// Eget val: nollkontrollens blandning sås med `fro ^ R2_NOLL_FRO_XOR`
/// (Fisher–Yates via `Rng::blanda` per djur, stigande djur_id).
pub const R2_NOLL_FRO_XOR: u64 = 0x1424_B1A2_D4ED;

/// Gränser skrivna före omräkningen (underlaget); ändras inte efteråt.
pub const POS_KVOT_MIN: f64 = 0.5;
pub const NEG_KVOT_MAX: f64 = 0.05;
pub const NEG_TRAFF_MAX: f64 = 1.0 / 8.0;
pub const TRANGSEL_TRAFF: f64 = 1.0 / 9.0;
pub const FABLE_VANLIGASTE_MIN: f64 = 0.6;
pub const FABLE_FRO_MIN: usize = 15;

#[derive(Clone, Debug)]
pub struct InstrumentFro {
    pub fro: u64,
    pub kvot_v: Option<f64>,
    pub kvot_1_8: Option<f64>,
    pub handling0: Handling0Matt,
    pub k_pos_v: Option<f64>,
    pub k_pos_1_8: Option<f64>,
    pub k_neg_1_8: Option<f64>,
    pub traff_pos: f64,
    pub traff_neg: f64,
    pub traff_trangsel: f64,
    pub n_djur: usize,
    pub n_rader: usize,
}

#[derive(Clone, Debug)]
pub struct OrsakFro {
    pub fro: u64,
    /// Medel över djur av vanligaste handlingens andel.
    pub vanligaste_andel: Option<f64>,
    /// Andel djur vars vanligaste val är handling 0.
    pub p0: Option<f64>,
    /// Fables (1 − p0) / 8.
    pub fable_bas: Option<f64>,
    /// Medel över djur av antalet aldrig valda handlingar (av 9).
    pub aldrig_valda: Option<f64>,
    /// Andel rader där valet = argmax(fast) men inte argmax(lage).
    pub fast_dominans: Option<f64>,
    /// Medel över z-talen av var_inom / var_mellan.
    pub z_inom_mellan: Option<f64>,
    /// R² per djur: medel över riktningarna 1–8 per djur, sedan oviktat medel över djuren.
    pub r2_riktning: Option<f64>,
    /// Samma, vägt med djurets antal rader.
    pub r2_riktning_viktat: Option<f64>,
    /// Nollkontroll: samma anpassning med y blandad inom djuret (låst frö).
    pub r2_noll: Option<f64>,
    /// Nollkontrollen, vägt med antal rader.
    pub r2_noll_viktat: Option<f64>,
    /// r2_riktning − r2_noll (oviktat).
    pub r2_skillnad: Option<f64>,
    /// r2_riktning_viktat − r2_noll_viktat.
    pub r2_skillnad_viktat: Option<f64>,
    /// Samma anpassning med alla djur i en gemensam minsta kvadrat (jämförelse).
    pub r2_riktning_gemensam: Option<f64>,
    /// Antal anpassningar som inte gick (singulär matris eller ingen spridning i y).
    pub r2_misslyckade: usize,
    /// Antal rader per djur (bland djur med minst MIN_RADER_DJUR): minsta.
    pub rader_per_djur_min: usize,
    /// Median.
    pub rader_per_djur_median: usize,
    /// Största.
    pub rader_per_djur_max: usize,
    pub n_djur: usize,
    pub n_rader: usize,
}

fn medel_opt(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

fn argmax9(v: &[f64; A_N]) -> usize {
    valj_fran_hj(v)
}

/// Räknar instrumentets kvoter per frö ur loggen (sökning).
pub fn instrument_per_fro(rader: &[SignalLoggRad]) -> Result<Vec<InstrumentFro>, String> {
    if rader.is_empty() {
        return Err("tom signallogg".into());
    }
    let traff_pos = ingrepp_traff_positiv();
    let traff_neg = ingrepp_traff_negativ();
    let traff_trangsel = ingrepp_traff_trangsel();
    let mut fron: Vec<u64> = rader.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&SignalLoggRad> = rader.iter().filter(|r| r.fro == fro).collect();
        let mut djur: Vec<u64> = i_fro.iter().map(|r| r.djur_id).collect();
        djur.sort_unstable();
        djur.dedup();
        let mut kv_v = Vec::new();
        let mut kv_18 = Vec::new();
        let mut kpos_v = Vec::new();
        let mut kpos_18 = Vec::new();
        let mut kneg_18 = Vec::new();
        let mut h0_medel = Vec::new();
        let mut h0_andel = Vec::new();
        let mut n_ok = 0usize;
        for &d in &djur {
            let rd: Vec<&SignalLoggRad> = i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
            if rd.len() < MIN_RADER_DJUR {
                continue;
            }
            n_ok += 1;
            let vard: Vec<[f64; A_N]> = rd.iter().map(|r| r.hj_varden).collect();
            let val: Vec<usize> = vard.iter().map(valj_fran_hj).collect();
            let pos: Vec<[f64; A_N]> = rd.iter().map(|r| positiv_varden(&r.s)).collect();
            let neg: Vec<[f64; A_N]> = rd.iter().map(|r| negativ_varden(&r.s)).collect();
            if let Some(k) = kvot_fran_matriser(&vard) {
                kv_v.push(k);
            }
            if let Some(k) = kvot_fran_matriser_riktningar(&vard) {
                kv_18.push(k);
            }
            if let Some(k) = kvot_fran_matriser(&pos) {
                kpos_v.push(k);
            }
            if let Some(k) = kvot_fran_matriser_riktningar(&pos) {
                kpos_18.push(k);
            }
            if let Some(k) = kvot_fran_matriser_riktningar(&neg) {
                kneg_18.push(k);
            }
            let h0 = handling0_matt(&vard, &val);
            h0_medel.push(h0.medel_centrerad);
            h0_andel.push(h0.andel_val);
        }
        ut.push(InstrumentFro {
            fro,
            kvot_v: medel_opt(&kv_v),
            kvot_1_8: medel_opt(&kv_18),
            handling0: Handling0Matt {
                medel_centrerad: medel_opt(&h0_medel).unwrap_or(0.0),
                andel_val: medel_opt(&h0_andel).unwrap_or(0.0),
            },
            k_pos_v: medel_opt(&kpos_v),
            k_pos_1_8: medel_opt(&kpos_18),
            k_neg_1_8: medel_opt(&kneg_18),
            traff_pos,
            traff_neg,
            traff_trangsel,
            n_djur: n_ok,
            n_rader: i_fro.len(),
        });
    }
    Ok(ut)
}

/// Kontrollerar att V:s gamla kvot upprepas exakt per frö.
pub fn kontrollera_v_kvot(
    instrument: &[InstrumentFro],
    v_signal: &[(u64, f64)],
) -> Result<(), String> {
    if v_signal.is_empty() {
        return Err("V:s signal.tsv saknar rader".into());
    }
    for &(fro, kvot) in v_signal {
        let m = instrument
            .iter()
            .find(|m| m.fro == fro)
            .ok_or_else(|| format!("frö {fro} saknas i omräkningen"))?;
        let k = m
            .kvot_v
            .ok_or_else(|| format!("frö {fro}: omräknad V-kvot saknas"))?;
        if (k - kvot).abs() > 1e-12 {
            return Err(format!(
                "frö {fro}: V-kvot {kvot} upprepades inte (fick {k})"
            ));
        }
    }
    if instrument.len() != v_signal.len() {
        return Err(format!(
            "antal frön skiljer: omräkning {}, V {}",
            instrument.len(),
            v_signal.len()
        ));
    }
    Ok(())
}

/// Läser `fro` och `kvot` ur V:s signal.tsv.
pub fn las_v_signal_kvot(text: &str) -> Result<Vec<(u64, f64)>, String> {
    let mut ut = Vec::new();
    let mut forsta = true;
    let mut kvot_i: Option<usize> = None;
    let mut fro_i: Option<usize> = None;
    for (nr, rad) in text.lines().enumerate() {
        if forsta {
            let h: Vec<&str> = rad.split('\t').collect();
            fro_i = h.iter().position(|&x| x == "fro");
            kvot_i = h.iter().position(|&x| x == "kvot");
            forsta = false;
            continue;
        }
        if rad.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = rad.split('\t').collect();
        let fi = fro_i.ok_or("kolumn fro saknas")?;
        let ki = kvot_i.ok_or("kolumn kvot saknas")?;
        if f.len() <= fi.max(ki) {
            return Err(format!("rad {}: för få fält", nr + 1));
        }
        let fro: u64 = f[fi]
            .parse()
            .map_err(|_| format!("rad {}: fro", nr + 1))?;
        let kvot: f64 = f[ki]
            .parse()
            .map_err(|_| format!("rad {}: kvot", nr + 1))?;
        ut.push((fro, kvot));
    }
    Ok(ut)
}

fn vanligaste(val: &[usize]) -> (usize, f64) {
    let mut ant = [0usize; A_N];
    for &a in val {
        ant[a] += 1;
    }
    let mut ba = 0usize;
    let mut bn = 0usize;
    for a in 0..A_N {
        if ant[a] > bn {
            ba = a;
            bn = ant[a];
        }
    }
    (ba, bn as f64 / val.len() as f64)
}

fn aldrig_valda(val: &[usize]) -> usize {
    let mut sett = [false; A_N];
    for &a in val {
        sett[a] = true;
    }
    sett.iter().filter(|&&s| !s).count()
}

fn varians(xs: &[f64]) -> f64 {
    let n = xs.len();
    if n < 2 {
        return 0.0;
    }
    let m = xs.iter().sum::<f64>() / n as f64;
    xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1) as f64
}

/// Alt 1424: sex mått per frö (medel per djur, minst 20 rader). Tom delmängd → fel.
pub fn orsak_per_fro(rader: &[SignalLoggRad]) -> Result<Vec<OrsakFro>, String> {
    if rader.is_empty() {
        return Err("tom delmängd: inga loggrader för Alt 1424".into());
    }
    let mut fron: Vec<u64> = rader.iter().map(|r| r.fro).collect();
    fron.sort_unstable();
    fron.dedup();
    let mut ut = Vec::new();
    for fro in fron {
        let i_fro: Vec<&SignalLoggRad> = rader.iter().filter(|r| r.fro == fro).collect();
        if i_fro.is_empty() {
            return Err(format!("tom delmängd: frö {fro}"));
        }
        let mut djur: Vec<u64> = i_fro.iter().map(|r| r.djur_id).collect();
        djur.sort_unstable();
        djur.dedup();
        let mut vanligaste_andelar = Vec::new();
        let mut vanligaste_ar_0 = 0usize;
        let mut aldrig = Vec::new();
        let mut fast_dom = Vec::new();
        let mut djur_ok = Vec::new();
        for &d in &djur {
            let rd: Vec<&SignalLoggRad> = i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
            if rd.len() < MIN_RADER_DJUR {
                continue;
            }
            djur_ok.push(d);
            let val: Vec<usize> = rd.iter().map(|r| valj_fran_hj(&r.hj_varden)).collect();
            let (ba, andel) = vanligaste(&val);
            vanligaste_andelar.push(andel);
            if ba == 0 {
                vanligaste_ar_0 += 1;
            }
            aldrig.push(aldrig_valda(&val) as f64);
            let mut n_dom = 0usize;
            for r in &rd {
                let v = valj_fran_hj(&r.hj_varden);
                let af = argmax9(&r.fast);
                let al = argmax9(&r.lage);
                if v == af && v != al {
                    n_dom += 1;
                }
            }
            fast_dom.push(n_dom as f64 / rd.len() as f64);
        }
        if djur_ok.is_empty() {
            return Err(format!(
                "tom delmängd: inga djur med minst {MIN_RADER_DJUR} rader i frö {fro}"
            ));
        }
        let p0 = vanligaste_ar_0 as f64 / djur_ok.len() as f64;
        let z_im = z_inom_mellan(&i_fro, &djur_ok)?;
        let r2 = r2_riktning_ur_z(fro, &i_fro, &djur_ok)?;
        ut.push(OrsakFro {
            fro,
            vanligaste_andel: medel_opt(&vanligaste_andelar),
            p0: Some(p0),
            fable_bas: Some((1.0 - p0) / 8.0),
            aldrig_valda: medel_opt(&aldrig),
            fast_dominans: medel_opt(&fast_dom),
            z_inom_mellan: z_im,
            r2_riktning: r2.per_djur,
            r2_riktning_viktat: r2.per_djur_viktat,
            r2_noll: r2.noll,
            r2_noll_viktat: r2.noll_viktat,
            r2_skillnad: skillnad_opt(r2.per_djur, r2.noll),
            r2_skillnad_viktat: skillnad_opt(r2.per_djur_viktat, r2.noll_viktat),
            r2_riktning_gemensam: r2.gemensam,
            r2_misslyckade: r2.misslyckade,
            rader_per_djur_min: r2.rader_min,
            rader_per_djur_median: r2.rader_median,
            rader_per_djur_max: r2.rader_max,
            n_djur: djur_ok.len(),
            n_rader: i_fro.len(),
        });
    }
    Ok(ut)
}

fn skillnad_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x - y),
        _ => None,
    }
}

fn z_inom_mellan(i_fro: &[&SignalLoggRad], djur_ok: &[u64]) -> Result<Option<f64>, String> {
    let mut kvoter = Vec::new();
    for i in 0..Z_N {
        let mut medel_per_djur = Vec::new();
        let mut inom_sum = 0.0;
        let mut inom_n = 0usize;
        for &d in djur_ok {
            let kol: Vec<f64> = i_fro
                .iter()
                .filter(|r| r.djur_id == d)
                .map(|r| r.z[i])
                .collect();
            if kol.len() < MIN_RADER_DJUR {
                continue;
            }
            inom_sum += varians(&kol);
            inom_n += 1;
            medel_per_djur.push(kol.iter().sum::<f64>() / kol.len() as f64);
        }
        if inom_n == 0 {
            continue;
        }
        let var_inom = inom_sum / inom_n as f64;
        let var_mellan = varians(&medel_per_djur);
        if var_mellan <= 1e-18 {
            kvoter.push(0.0);
        } else {
            kvoter.push(var_inom / var_mellan);
        }
    }
    Ok(medel_opt(&kvoter))
}

/// Resultat av R²-måttet: per djur (oviktat/viktat), nollkontroll, gemensam, rader.
#[derive(Clone, Debug)]
pub struct R2Riktning {
    pub per_djur: Option<f64>,
    pub per_djur_viktat: Option<f64>,
    pub noll: Option<f64>,
    pub noll_viktat: Option<f64>,
    pub gemensam: Option<f64>,
    pub misslyckade: usize,
    pub rader_min: usize,
    pub rader_median: usize,
    pub rader_max: usize,
}

fn xy_z_till_s(rader: &[&SignalLoggRad], s_idx: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut x = Vec::with_capacity(rader.len());
    let mut y = Vec::with_capacity(rader.len());
    for r in rader {
        let mut rad = vec![0.0; Z_N + 1];
        for i in 0..Z_N {
            rad[i] = r.z[i];
        }
        rad[Z_N] = 1.0;
        x.push(rad);
        y.push(r.s[s_idx]);
    }
    (x, y)
}

/// En anpassning: Ok(R²) eller Err (singulär / ingen spridning i y / icke-ändligt).
fn en_anpassning(x: &[Vec<f64>], y: &[f64]) -> Result<f64, ()> {
    let beta = minstakvadrat::minstakvadrat(x, y).map_err(|_| ())?;
    let r2 = minstakvadrat::r2_pa(x, y, &beta);
    if r2.is_finite() {
        Ok(r2)
    } else {
        Err(())
    }
}

/// Medel R² över riktningarna 1–8. Om `rng` är Some blandas y före varje anpassning.
fn r2_over_riktningar(
    rader: &[&SignalLoggRad],
    misslyckade: &mut usize,
    mut rng: Option<&mut Rng>,
) -> Option<f64> {
    let mut r2or = Vec::new();
    for riktning in 1..A_N {
        let s_idx = riktning * 2;
        let (x, mut y) = xy_z_till_s(rader, s_idx);
        if let Some(r) = rng.as_deref_mut() {
            r.blanda(&mut y);
        }
        match en_anpassning(&x, &y) {
            Ok(r2) => r2or.push(r2),
            Err(()) => *misslyckade += 1,
        }
    }
    medel_opt(&r2or)
}

fn viktat_medel(par: &[(f64, usize)]) -> Option<f64> {
    let w: usize = par.iter().map(|(_, n)| n).sum();
    if w == 0 || par.is_empty() {
        None
    } else {
        Some(par.iter().map(|(v, n)| v * *n as f64).sum::<f64>() / w as f64)
    }
}

fn median_usize(mut xs: Vec<usize>) -> usize {
    if xs.is_empty() {
        return 0;
    }
    xs.sort_unstable();
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2
    }
}

/// R² per djur (oviktat och viktat), nollkontroll med blandad y, gemensam anpassning.
/// Misslyckade anpassningar (riktiga + gemensamma) räknas; nollkontrollens fel räknas inte här.
fn r2_riktning_ur_z(
    fro: u64,
    i_fro: &[&SignalLoggRad],
    djur_ok: &[u64],
) -> Result<R2Riktning, String> {
    let mut misslyckade = 0usize;
    let mut per_par: Vec<(f64, usize)> = Vec::new();
    let mut noll_par: Vec<(f64, usize)> = Vec::new();
    let mut n_rader_per = Vec::new();
    // Låst följd ur loggens frö (eget val R2_NOLL_FRO_XOR); djur i stigande id.
    let mut noll_rng = Rng::new(fro ^ R2_NOLL_FRO_XOR);
    for &d in djur_ok {
        let rd: Vec<&SignalLoggRad> = i_fro.iter().copied().filter(|r| r.djur_id == d).collect();
        if rd.len() < MIN_RADER_DJUR {
            continue;
        }
        n_rader_per.push(rd.len());
        if let Some(m) = r2_over_riktningar(&rd, &mut misslyckade, None) {
            per_par.push((m, rd.len()));
        }
        // Nollkontroll: blanda y; fel räknas lokalt så misslyckade behåller gammal betydelse.
        let mut noll_fel = 0usize;
        if let Some(m) = r2_over_riktningar(&rd, &mut noll_fel, Some(&mut noll_rng)) {
            noll_par.push((m, rd.len()));
        }
    }
    let gemensam = r2_over_riktningar(i_fro, &mut misslyckade, None);
    let per_varden: Vec<f64> = per_par.iter().map(|(v, _)| *v).collect();
    let noll_varden: Vec<f64> = noll_par.iter().map(|(v, _)| *v).collect();
    let rader_min = n_rader_per.iter().copied().min().unwrap_or(0);
    let rader_max = n_rader_per.iter().copied().max().unwrap_or(0);
    let rader_median = median_usize(n_rader_per);
    Ok(R2Riktning {
        per_djur: medel_opt(&per_varden),
        per_djur_viktat: viktat_medel(&per_par),
        noll: medel_opt(&noll_varden),
        noll_viktat: viktat_medel(&noll_par),
        gemensam,
        misslyckade,
        rader_min,
        rader_median,
        rader_max,
    })
}

/// Fem gränser per frö: (pos_kvot, pos_traff, neg_kvot, neg_traff, trangsel_traff).
pub fn instrument_granser(m: &InstrumentFro) -> [bool; 5] {
    let pos_k = m.k_pos_1_8.map(|k| k >= POS_KVOT_MIN).unwrap_or(false);
    let pos_t = (m.traff_pos - 1.0).abs() < 1e-12;
    let neg_k = m.k_neg_1_8.map(|k| k < NEG_KVOT_MAX).unwrap_or(false);
    let neg_t = m.traff_neg <= NEG_TRAFF_MAX + 1e-12;
    let tr_t = (m.traff_trangsel - TRANGSEL_TRAFF).abs() < 1e-12;
    [pos_k, pos_t, neg_k, neg_t, tr_t]
}

pub fn instrument_godkant(matt: &[InstrumentFro]) -> bool {
    !matt.is_empty() && matt.iter().all(|m| instrument_granser(m).iter().all(|&x| x))
}

pub fn fable_vanligaste_antal(orsak: &[OrsakFro]) -> usize {
    orsak
        .iter()
        .filter(|o| {
            o.vanligaste_andel
                .map(|a| a >= FABLE_VANLIGASTE_MIN)
                .unwrap_or(false)
        })
        .count()
}

fn fmt_opt(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{v}"),
        _ => "null".into(),
    }
}

pub fn skriv_instrument_tsv(matt: &[InstrumentFro]) -> String {
    let mut tsv = String::from(
        "fro\tkvot_v\tkvot_1_8\th0_medel_centrerad\th0_andel_val\tk_pos_v\tk_pos_1_8\tk_neg_1_8\ttraff_pos\ttraff_neg\ttraff_trangsel\tn_djur\tn_rader\n",
    );
    for m in matt {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            m.fro,
            fmt_opt(m.kvot_v),
            fmt_opt(m.kvot_1_8),
            m.handling0.medel_centrerad,
            m.handling0.andel_val,
            fmt_opt(m.k_pos_v),
            fmt_opt(m.k_pos_1_8),
            fmt_opt(m.k_neg_1_8),
            m.traff_pos,
            m.traff_neg,
            m.traff_trangsel,
            m.n_djur,
            m.n_rader
        ));
    }
    tsv
}

pub fn skriv_orsak_tsv(matt: &[OrsakFro]) -> String {
    let mut tsv = String::from(
        "fro\tvanligaste_andel\tp0\tfable_bas\taldrig_valda\tfast_dominans\tz_inom_mellan\tr2_riktning\tr2_riktning_viktat\tr2_noll\tr2_noll_viktat\tr2_skillnad\tr2_skillnad_viktat\tr2_gemensam\tr2_misslyckade\trader_per_djur_min\trader_per_djur_median\trader_per_djur_max\tn_djur\tn_rader\n",
    );
    for m in matt {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            m.fro,
            fmt_opt(m.vanligaste_andel),
            fmt_opt(m.p0),
            fmt_opt(m.fable_bas),
            fmt_opt(m.aldrig_valda),
            fmt_opt(m.fast_dominans),
            fmt_opt(m.z_inom_mellan),
            fmt_opt(m.r2_riktning),
            fmt_opt(m.r2_riktning_viktat),
            fmt_opt(m.r2_noll),
            fmt_opt(m.r2_noll_viktat),
            fmt_opt(m.r2_skillnad),
            fmt_opt(m.r2_skillnad_viktat),
            fmt_opt(m.r2_riktning_gemensam),
            m.r2_misslyckade,
            m.rader_per_djur_min,
            m.rader_per_djur_median,
            m.rader_per_djur_max,
            m.n_djur,
            m.n_rader
        ));
    }
    tsv
}

pub fn skriv_sammanfattning(
    instrument: &[InstrumentFro],
    orsak: &[OrsakFro],
    logg_vag: &str,
    logg_bytes: u64,
    logg_sha: &str,
) -> String {
    let mut md = String::from(
        "# Sökning: rättat instrument (Alt 1423) och orsaken i loggen (Alt 1424)\n\n",
    );
    md.push_str("**Sökning, inte bindande.** Gränserna skrevs före omräkningen. ");
    md.push_str("Ingreppets träff för `lar` kan inte räknas om ur loggen; V:s tal står kvar i `v1251-signal.tsv`.\n\n");
    md.push_str(&format!(
        "Logg: `{logg_vag}` ({logg_bytes} byte, sha256 {logg_sha}).\n\n"
    ));

    let godkant = instrument_godkant(instrument);
    md.push_str("## Instrumentets fem gränser (per frö)\n\n");
    md.push_str("| frö | pos kvot ≥0,5 | pos träff =1 | neg kvot <0,05 | neg träff ≤1/8 | trängsel =1/9 | klarat |\n");
    md.push_str("|---|---|---|---|---|---|---|\n");
    let mut n_klart = 0usize;
    for m in instrument {
        let g = instrument_granser(m);
        let klart = g.iter().all(|&x| x);
        if klart {
            n_klart += 1;
        }
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            m.fro,
            om(g[0]),
            om(g[1]),
            om(g[2]),
            om(g[3]),
            om(g[4]),
            if klart { "ja" } else { "nej" }
        ));
    }
    md.push_str(&format!(
        "\nInstrumentet: {} ({n_klart}/{} frön klarade alla fem).\n\n",
        if godkant { "godkänt" } else { "underkänt" },
        instrument.len()
    ));

    md.push_str("## Kvoter (programmets tal)\n\n");
    md.push_str("| frö | kvot V (9) | kvot 1–8 | k_pos 1–8 | k_neg 1–8 | h0 andel | n_djur |\n");
    md.push_str("|---|---|---|---|---|---|---|\n");
    for m in instrument {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.6} | {} |\n",
            m.fro,
            fmt_opt(m.kvot_v),
            fmt_opt(m.kvot_1_8),
            fmt_opt(m.k_pos_1_8),
            fmt_opt(m.k_neg_1_8),
            m.handling0.andel_val,
            m.n_djur
        ));
    }
    md.push_str(&format!(
        "\nTräff positiv={}, negativ={}, trängsel={} (samma i alla frön; räknat av programmet).\n\n",
        instrument.first().map(|m| m.traff_pos).unwrap_or(f64::NAN),
        instrument.first().map(|m| m.traff_neg).unwrap_or(f64::NAN),
        instrument
            .first()
            .map(|m| m.traff_trangsel)
            .unwrap_or(f64::NAN)
    ));

    md.push_str("## Orsaken i loggen (Alt 1424; loggen tar vart 10:e djursteg)\n\n");
    md.push_str(&format!(
        "Nollkontroll: y (`s[2a]`) blandas inom djuret med `Rng::new(fro ^ 0x{R2_NOLL_FRO_XOR:X})` (eget val).\n\n"
    ));
    md.push_str("| frö | vanligaste andel | p0 | (1−p0)/8 | aldrig valda | fast dominans | z inom/mellan | R² per djur | R² viktat | R² noll | R² noll viktat | skillnad | skillnad viktat | R² gemensam | misslyckade | rader min/median/max |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for o in orsak {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{}/{} |\n",
            o.fro,
            fmt_opt(o.vanligaste_andel),
            fmt_opt(o.p0),
            fmt_opt(o.fable_bas),
            fmt_opt(o.aldrig_valda),
            fmt_opt(o.fast_dominans),
            fmt_opt(o.z_inom_mellan),
            fmt_opt(o.r2_riktning),
            fmt_opt(o.r2_riktning_viktat),
            fmt_opt(o.r2_noll),
            fmt_opt(o.r2_noll_viktat),
            fmt_opt(o.r2_skillnad),
            fmt_opt(o.r2_skillnad_viktat),
            fmt_opt(o.r2_riktning_gemensam),
            o.r2_misslyckade,
            o.rader_per_djur_min,
            o.rader_per_djur_median,
            o.rader_per_djur_max
        ));
    }
    let n_fable = fable_vanligaste_antal(orsak);
    md.push_str(&format!(
        "\nFables gräns (eget val): vanligaste andel ≥ {FABLE_VANLIGASTE_MIN} i **{n_fable}** av {} frön",
        orsak.len()
    ));
    if n_fable >= FABLE_FRO_MIN {
        md.push_str(&format!(
            " (≥ {FABLE_FRO_MIN}): förklaringen «aldrig prövat» står kvar som möjlig.\n"
        ));
    } else if orsak
        .iter()
        .filter(|o| {
            o.vanligaste_andel
                .map(|a| a < FABLE_VANLIGASTE_MIN)
                .unwrap_or(false)
        })
        .count()
        >= FABLE_FRO_MIN
    {
        md.push_str(&format!(
            " (under 0,6 i minst {FABLE_FRO_MIN}): «aldrig prövat» är fel förklaring; Alt 1426 faller.\n"
        ));
    } else {
        md.push_str(": ingen visad skillnad.\n");
    }

    md.push_str("\n## Alt 1425\n\n");
    md.push_str(
        "Förberedd: `hcsim signal --fron 221-240 --bara v_lar` (full) och `--torr` (2 frön, TORRKÖRNING). ",
    );
    md.push_str("Körs inte här; Claude granskar först.\n");
    md
}

fn om(b: bool) -> &'static str {
    if b {
        "ja"
    } else {
        "nej"
    }
}

/// Omräkning på loggen: kontrollera V, skriv TSV + sammanfattning.
pub fn kor_omrakna(
    ut_dir: &Path,
    logg_path: &Path,
    v_signal_path: &Path,
) -> Result<(), String> {
    let meta = std::fs::metadata(logg_path)
        .map_err(|e| format!("kunde inte läsa metadata {}: {e}", logg_path.display()))?;
    let logg_bytes = meta.len();
    let logg_text = std::fs::read_to_string(logg_path)
        .map_err(|e| format!("kunde inte läsa {}: {e}", logg_path.display()))?;
    let logg_sha = sha256_hex(logg_text.as_bytes());
    let rader = las_logg_tsv(&logg_text)?;
    let instrument = instrument_per_fro(&rader)?;
    let v_text = std::fs::read_to_string(v_signal_path)
        .map_err(|e| format!("kunde inte läsa {}: {e}", v_signal_path.display()))?;
    let v_kvot = las_v_signal_kvot(&v_text)?;
    kontrollera_v_kvot(&instrument, &v_kvot)?;
    let orsak = orsak_per_fro(&rader)?;

    let inst_path = ut_dir.join("hjsignal1423-instrument.tsv");
    let orsak_path = ut_dir.join("hjsignal1423-orsak.tsv");
    let md_path = ut_dir.join("hjsignal1423-sammanfattning.md");
    std::fs::write(&inst_path, skriv_instrument_tsv(&instrument))
        .map_err(|e| format!("skriv {}: {e}", inst_path.display()))?;
    std::fs::write(&orsak_path, skriv_orsak_tsv(&orsak))
        .map_err(|e| format!("skriv {}: {e}", orsak_path.display()))?;
    let md = skriv_sammanfattning(
        &instrument,
        &orsak,
        &logg_path.display().to_string(),
        logg_bytes,
        &logg_sha,
    );
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    print!("{md}");
    eprintln!(
        "skrev {}, {}, {} (V-kvot upprepad för {} frön)",
        inst_path.display(),
        orsak_path.display(),
        md_path.display(),
        v_kvot.len()
    );
    Ok(())
}

fn sha256_hex(data: &[u8]) -> String {
    sha256::digest(data)
}

mod sha256 {
    // Minimal SHA-256 (FIPS 180-4) utan ny dependency.
    pub fn digest(data: &[u8]) -> String {
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let bits = (data.len() as u64) * 8;
        let mut buf = data.to_vec();
        buf.push(0x80);
        while (buf.len() % 64) != 56 {
            buf.push(0);
        }
        buf.extend_from_slice(&bits.to_be_bytes());
        for chunk in buf.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[i * 4],
                    chunk[i * 4 + 1],
                    chunk[i * 4 + 2],
                    chunk[i * 4 + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let mut a = h[0];
            let mut b = h[1];
            let mut c = h[2];
            let mut d = h[3];
            let mut e = h[4];
            let mut f = h[5];
            let mut g = h[6];
            let mut hh = h[7];
            const K: [u32; 64] = [
                0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
                0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
                0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
                0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
                0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
                0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
                0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
                0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
                0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
                0xc67178f2,
            ];
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }
        format!(
            "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
        )
    }
}
