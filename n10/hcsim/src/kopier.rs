//! Kopieringsprovet på dagens hjärna (Alt 1462): tre kvadratfel mot `v_lar`-världen.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use hckarta::{karta_full, Kalla, Omrade, Tidtabell};

use crate::formaga::{Formaga, HjarnArm};
use crate::sim::{KopierArAck, KopierMatning, MarkLage, Sim};
use crate::svep::{n0_for, sat_nice, LAT0, LON0, MAX_TRADAR, N_FRO_TORR, RUTA_M, T_KARTA};
use crate::tal::{MARKT_EFTER_AR, MARKT_INKOR_AR};

pub const B_FULL: usize = 64;
pub const H_FULL: usize = 48;
pub const B_TORR: usize = 16;
pub const H_TORR: usize = 12;

/// Nya frön 241–260 (inte använda förut).
pub const FRO_START: u64 = 241;
pub const N_FRO_FULL: u64 = 20;

/// Utfallsgränser (underlaget; ändras aldrig).
pub const VUNNA_KRAV: usize = 15;
pub const VANA_KVOT_MAX: f64 = 1.05;
pub const LAGE_KVOT_MAX: f64 = 0.90;

#[derive(Clone, Debug)]
pub struct ArRad {
    pub ar: i64,
    pub djursteg: u64,
    pub medel_forutsag: f64,
    pub medel_kopiering: f64,
    pub medel_fast: f64,
}

#[derive(Clone, Debug)]
pub struct Korning {
    pub fro: u64,
    pub antal_markering: usize,
    pub antal_slut: usize,
    pub kortid_s: f64,
    pub torr: bool,
    pub djursteg: u64,
    pub medel_forutsag: f64,
    pub medel_kopiering: f64,
    pub medel_fast: f64,
    pub kvot_kopiering: f64,
    pub kvot_fast: f64,
    pub per_ar: Vec<ArRad>,
}

impl Korning {
    fn fran(fro: u64, torr: bool, kortid_s: f64, m: &KopierMatning, antal_markering: usize, antal_slut: usize) -> Result<Self, String> {
        m.krav_data()?;
        let mf = m.tot.medel_forutsag().ok_or("medel_forutsag saknas")?;
        let mk = m.tot.medel_kopiering().ok_or("medel_kopiering saknas")?;
        let mfa = m.tot.medel_fast().ok_or("medel_fast saknas")?;
        let mut per_ar: Vec<ArRad> = m
            .per_ar
            .iter()
            .map(|(&ar, ack)| ArRad {
                ar,
                djursteg: ack.n,
                medel_forutsag: ack.medel_forutsag().unwrap_or(f64::NAN),
                medel_kopiering: ack.medel_kopiering().unwrap_or(f64::NAN),
                medel_fast: ack.medel_fast().unwrap_or(f64::NAN),
            })
            .collect();
        per_ar.sort_by_key(|r| r.ar);
        Ok(Self {
            fro,
            antal_markering,
            antal_slut,
            kortid_s,
            torr,
            djursteg: m.tot.n,
            medel_forutsag: mf,
            medel_kopiering: mk,
            medel_fast: mfa,
            kvot_kopiering: if mk > 0.0 { mf / mk } else { f64::NAN },
            kvot_fast: if mfa > 0.0 { mf / mfa } else { f64::NAN },
            per_ar,
        })
    }

    pub fn till_json(&self) -> String {
        let ar_json: Vec<String> = self
            .per_ar
            .iter()
            .map(|r| {
                format!(
                    "{{\"ar\":{},\"djursteg\":{},\"medel_forutsag\":{},\"medel_kopiering\":{},\"medel_fast\":{}}}",
                    r.ar,
                    r.djursteg,
                    fmt_f64(r.medel_forutsag),
                    fmt_f64(r.medel_kopiering),
                    fmt_f64(r.medel_fast),
                )
            })
            .collect();
        let mut s = format!(
            "{{\"arm\":\"v_lar\",\"fro\":{},\"antal_levande_vid_markning\":{},\"antal_levande_vid_slut\":{},\"djursteg\":{},\"medel_forutsag\":{},\"medel_kopiering\":{},\"medel_fast\":{},\"kvot_kopiering\":{},\"kvot_fast\":{},\"per_ar\":[{}],\"kortid_s\":{}}}",
            self.fro,
            self.antal_markering,
            self.antal_slut,
            self.djursteg,
            fmt_f64(self.medel_forutsag),
            fmt_f64(self.medel_kopiering),
            fmt_f64(self.medel_fast),
            fmt_f64(self.kvot_kopiering),
            fmt_f64(self.kvot_fast),
            ar_json.join(","),
            fmt_f64(self.kortid_s),
        );
        if self.torr {
            s.push_str(" TORRKÖRNING");
        }
        s
    }
}

fn fmt_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{x}")
    } else {
        "null".into()
    }
}

fn fro_lista(torr: bool) -> Vec<u64> {
    let n = if torr { N_FRO_TORR } else { N_FRO_FULL };
    (0..n).map(|i| FRO_START + i).collect()
}

fn bygg_sim(kalla: &Kalla, tt: &Tidtabell, b: usize, h: usize, fro: u64) -> Result<Sim, String> {
    let omr = Omrade {
        lat0: LAT0,
        lon0: LON0,
        b,
        h,
        ruta_m: RUTA_M,
    };
    let kart = karta_full(kalla, tt, &omr, T_KARTA, fro)?;
    let n0 = n0_for(b, h);
    let mut sim = Sim::fran_karta(&kart, fro, n0);
    sim.mark_lage = MarkLage::Noll;
    sim.hjarn_arm = Some(HjarnArm {
        namn: "v_lar",
        markta: Formaga::Lar,
        omarkta: Formaga::Lar,
        spegel: false,
    });
    sim.atervaxt = 1.0;
    // Samtidig klocka (som strykprovets v_lar).
    sim.turordning = false;
    sim.flytt_i_tur = false;
    sim.bete_i_tur = false;
    sim.kopier_matning = KopierMatning {
        pa: true,
        ..KopierMatning::default()
    };
    Ok(sim)
}

pub fn kor_en(
    kalla: &Kalla,
    tt: &Tidtabell,
    b: usize,
    h: usize,
    fro: u64,
    torr: bool,
) -> Result<Korning, String> {
    let start = Instant::now();
    let mut sim = bygg_sim(kalla, tt, b, h, fro)?;
    let _ = sim.kor_ar(MARKT_INKOR_AR, MARKT_INKOR_AR);
    sim.markera_halften(fro);
    let antal_markering = sim.antal();
    let slut_ar = MARKT_INKOR_AR + MARKT_EFTER_AR;
    let _ = sim.kor_ar(slut_ar, MARKT_EFTER_AR);
    Korning::fran(
        fro,
        torr,
        start.elapsed().as_secs_f64(),
        &sim.kopier_matning,
        antal_markering,
        sim.antal(),
    )
}

pub fn kor_parallellt(
    paleo: &Path,
    tt_path: &Path,
    b: usize,
    h: usize,
    fron: &[u64],
    torr: bool,
) -> Result<Vec<Korning>, String> {
    if fron.is_empty() {
        return Ok(Vec::new());
    }
    let ko: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(fron.to_vec()));
    let ut: Arc<Mutex<Vec<Result<Korning, String>>>> = Arc::new(Mutex::new(Vec::new()));
    let n_trad = MAX_TRADAR.min(fron.len()).max(1);
    let paleo = paleo.to_path_buf();
    let tt_path = tt_path.to_path_buf();
    let mut handles = Vec::with_capacity(n_trad);
    for _ in 0..n_trad {
        let ko = Arc::clone(&ko);
        let ut = Arc::clone(&ut);
        let paleo = paleo.clone();
        let tt_path = tt_path.clone();
        handles.push(thread::spawn(move || {
            let kalla = match Kalla::las(&paleo) {
                Ok(k) => k,
                Err(e) => {
                    ut.lock().unwrap().push(Err(format!("karta: {e}")));
                    return;
                }
            };
            let tt_text = match std::fs::read_to_string(&tt_path) {
                Ok(t) => t,
                Err(e) => {
                    ut.lock().unwrap().push(Err(format!("tidtabell: {e}")));
                    return;
                }
            };
            let tt = match Tidtabell::las(&tt_text) {
                Ok(t) => t,
                Err(e) => {
                    ut.lock().unwrap().push(Err(format!("tidtabell: {e}")));
                    return;
                }
            };
            loop {
                let fro = {
                    let mut q = ko.lock().unwrap();
                    q.pop()
                };
                let Some(fro) = fro else { break };
                let r = kor_en(&kalla, &tt, b, h, fro, torr);
                ut.lock().unwrap().push(r);
            }
        }));
    }
    for hnd in handles {
        hnd.join().map_err(|_| "tråd panikade".to_string())?;
    }
    let raw = Arc::try_unwrap(ut).unwrap().into_inner().unwrap();
    let mut ut = Vec::with_capacity(raw.len());
    for r in raw {
        ut.push(r?);
    }
    ut.sort_by_key(|k| k.fro);
    Ok(ut)
}

/// Utfallsregeln (underlaget, skrivna före).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Utfall {
    /// Rött (Fable-3): förutsägaren slår inte kopieringen i minst 15/20.
    RodtFable3,
    /// Vana per handling: förutsägaren ≤ 1,05 × fasta delen i minst 15/20.
    VanaPerHandling,
    /// Lär sig läget: lägre än båda med minst 10 % i minst 15/20.
    LarSigLaget,
    /// Annars.
    IngenVisadSkillnad,
}

impl Utfall {
    pub fn som_str(self) -> &'static str {
        match self {
            Utfall::RodtFable3 => "Rött (Fable-3)",
            Utfall::VanaPerHandling => "Vana per handling",
            Utfall::LarSigLaget => "Lär sig läget",
            Utfall::IngenVisadSkillnad => "Ingen visad skillnad",
        }
    }
}

#[derive(Clone, Debug)]
pub struct UtfallsRakning {
    pub n_fro: usize,
    /// Förutsägarens medelfel < kopieringens.
    pub n_slag_kopiering: usize,
    /// Förutsägarens medelfel ≤ 1,05 × fasta delen.
    pub n_vana: usize,
    /// Förutsägaren lägre än båda med minst 10 %.
    pub n_lage: usize,
    pub utfall: Utfall,
}

pub fn rakna_utfall(korningar: &[Korning]) -> UtfallsRakning {
    let n_fro = korningar.len();
    let n_slag_kopiering = korningar
        .iter()
        .filter(|k| k.medel_forutsag < k.medel_kopiering)
        .count();
    let n_vana = korningar
        .iter()
        .filter(|k| k.medel_forutsag <= VANA_KVOT_MAX * k.medel_fast)
        .count();
    let n_lage = korningar
        .iter()
        .filter(|k| {
            k.medel_forutsag < LAGE_KVOT_MAX * k.medel_kopiering
                && k.medel_forutsag < LAGE_KVOT_MAX * k.medel_fast
        })
        .count();
    // Rött om förutsägaren INTE är lägre än kopieringen i minst 15 frön
    // = antalet där den ÄR lägre är < 15.
    let utfall = if n_slag_kopiering < VUNNA_KRAV {
        Utfall::RodtFable3
    } else if n_vana >= VUNNA_KRAV {
        Utfall::VanaPerHandling
    } else if n_lage >= VUNNA_KRAV {
        Utfall::LarSigLaget
    } else {
        Utfall::IngenVisadSkillnad
    };
    UtfallsRakning {
        n_fro,
        n_slag_kopiering,
        n_vana,
        n_lage,
        utfall,
    }
}

pub fn skriv_sammanfattning(korningar: &[Korning], torr: bool, total_s: f64) -> String {
    let rak = rakna_utfall(korningar);
    let mut md = String::new();
    if torr {
        md.push_str("# Kopieringsprovet Alt 1462 — TORRKÖRNING\n\n");
        md.push_str(
            "Torrkörning: 2 frön, karta 16×12. Avgörandena nedan är **inte** giltiga.\n\n",
        );
    } else {
        md.push_str("# Kopieringsprovet Alt 1462\n\n");
        md.push_str(
            "Gränser ur `n10/docs/2026-11-22-lasunderlag-kopier1462.md` (skrivna före körningen).\n\n",
        );
    }
    md.push_str(&format!("Total körtid: {:.1} s\n\n", total_s));
    md.push_str("Armen `v_lar`: 64×48 (torr 16×12), samtidig klocka, båda grupperna `lar`, ");
    md.push_str(&format!(
        "frön {}–{}.\n\n",
        FRO_START,
        if torr {
            FRO_START + N_FRO_TORR - 1
        } else {
            FRO_START + N_FRO_FULL - 1
        }
    ));

    md.push_str("## Per frö (hela körningen efter märkningen)\n\n");
    md.push_str("| Frö | Djursteg | Medel förutsägare | Medel kopiering | Medel fast | Kvot/kopiering | Kvot/fast |\n");
    md.push_str("|-----|----------|-------------------|-----------------|------------|----------------|----------|\n");
    for k in korningar {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            k.fro,
            k.djursteg,
            fmt_f64(k.medel_forutsag),
            fmt_f64(k.medel_kopiering),
            fmt_f64(k.medel_fast),
            fmt_f64(k.kvot_kopiering),
            fmt_f64(k.kvot_fast),
        ));
    }

    md.push_str("\n## Utfall\n\n");
    md.push_str(&format!(
        "- Förutsägaren lägre än kopieringen: {}/{}\n",
        rak.n_slag_kopiering, rak.n_fro
    ));
    md.push_str(&format!(
        "- Förutsägaren ≤ 1,05 × fasta delen: {}/{}\n",
        rak.n_vana, rak.n_fro
    ));
    md.push_str(&format!(
        "- Förutsägaren lägre än båda med minst 10 %: {}/{}\n",
        rak.n_lage, rak.n_fro
    ));
    md.push_str(&format!("- **Utfallsregel:** {}\n", rak.utfall.som_str()));
    md.push_str("\nUtfallsreglerna (oförändrade):\n");
    md.push_str(&format!(
        "- **Rött (Fable-3):** förutsägarens medelfel är inte lägre än kopieringens i minst {} av {} frön.\n",
        VUNNA_KRAV, N_FRO_FULL
    ));
    md.push_str(&format!(
        "- **Vana per handling:** förutsägarens medelfel är högst {} gånger fasta delens i minst {} av {} frön.\n",
        VANA_KVOT_MAX, VUNNA_KRAV, N_FRO_FULL
    ));
    md.push_str(&format!(
        "- **Lär sig läget:** förutsägaren är lägre än både kopieringen och fasta delen med minst 10 % i minst {} av {} frön.\n",
        VUNNA_KRAV, N_FRO_FULL
    ));
    md.push_str("- Annars ingen visad skillnad.\n");

    // Medel över frön (programmet skriver talen).
    if !korningar.is_empty() {
        let n = korningar.len() as f64;
        let mf: f64 = korningar.iter().map(|k| k.medel_forutsag).sum::<f64>() / n;
        let mk: f64 = korningar.iter().map(|k| k.medel_kopiering).sum::<f64>() / n;
        let mfa: f64 = korningar.iter().map(|k| k.medel_fast).sum::<f64>() / n;
        md.push_str("\n## Medel över frön\n\n");
        md.push_str(&format!(
            "| Medel förutsägare | Medel kopiering | Medel fast | Kvot/kopiering | Kvot/fast |\n"
        ));
        md.push_str("|---|---|---|---|---|\n");
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            fmt_f64(mf),
            fmt_f64(mk),
            fmt_f64(mfa),
            fmt_f64(if mk > 0.0 { mf / mk } else { f64::NAN }),
            fmt_f64(if mfa > 0.0 { mf / mfa } else { f64::NAN }),
        ));
    }

    // Årsvis medel (medel över frön per år).
    if !torr && !korningar.is_empty() {
        let mut ar_map: std::collections::BTreeMap<i64, KopierArAck> =
            std::collections::BTreeMap::new();
        // Viktat? Underlaget: medel per djursteg redovisas per år — summera över frön.
        for k in korningar {
            for r in &k.per_ar {
                let ack = ar_map.entry(r.ar).or_default();
                // Återställ medel → summa via djursteg.
                ack.forutsag_sum += r.medel_forutsag * r.djursteg as f64;
                ack.kopiering_sum += r.medel_kopiering * r.djursteg as f64;
                ack.fast_sum += r.medel_fast * r.djursteg as f64;
                ack.n += r.djursteg;
            }
        }
        md.push_str("\n## Per år efter märkningen (alla frön)\n\n");
        md.push_str("| År | Djursteg | Medel förutsägare | Medel kopiering | Medel fast | Kvot/kopiering | Kvot/fast |\n");
        md.push_str("|----|----------|-------------------|-----------------|------------|----------------|----------|\n");
        for (ar, ack) in ar_map {
            let mf = ack.medel_forutsag().unwrap_or(f64::NAN);
            let mk = ack.medel_kopiering().unwrap_or(f64::NAN);
            let mfa = ack.medel_fast().unwrap_or(f64::NAN);
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                ar,
                ack.n,
                fmt_f64(mf),
                fmt_f64(mk),
                fmt_f64(mfa),
                fmt_f64(if mk > 0.0 { mf / mk } else { f64::NAN }),
                fmt_f64(if mfa > 0.0 { mf / mfa } else { f64::NAN }),
            ));
        }
    }

    md
}

pub fn kor_kopier(ut_dir: &Path, torr: bool) -> Result<(), String> {
    sat_nice();
    let start = Instant::now();
    let paleo = ut_dir.join("../paleokarta/data");
    let tt_path = ut_dir.join("../hckarta/tidtabell.tsv");
    let (b, h) = if torr {
        (B_TORR, H_TORR)
    } else {
        (B_FULL, H_FULL)
    };
    let fron = fro_lista(torr);
    let korningar = kor_parallellt(&paleo, &tt_path, b, h, &fron, torr)?;
    let total_s = start.elapsed().as_secs_f64();

    let prefix = "kopier1462";
    let jsonl_path = ut_dir.join(format!("{prefix}-korningar.jsonl"));
    let md_path = ut_dir.join(format!("{prefix}-sammanfattning.md"));
    let tab_path = ut_dir.join(format!("{prefix}-tabell.tsv"));

    let mut jsonl = String::new();
    for k in &korningar {
        let rad = k.till_json();
        println!("{rad}");
        jsonl.push_str(&rad);
        jsonl.push('\n');
    }
    std::fs::write(&jsonl_path, &jsonl)
        .map_err(|e| format!("skriv {}: {e}", jsonl_path.display()))?;

    let mut tsv = String::from(
        "fro\tdjursteg\tmedel_forutsag\tmedel_kopiering\tmedel_fast\tkvot_kopiering\tkvot_fast\n",
    );
    for k in &korningar {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            k.fro,
            k.djursteg,
            fmt_f64(k.medel_forutsag),
            fmt_f64(k.medel_kopiering),
            fmt_f64(k.medel_fast),
            fmt_f64(k.kvot_kopiering),
            fmt_f64(k.kvot_fast),
        ));
    }
    std::fs::write(&tab_path, &tsv).map_err(|e| format!("skriv {}: {e}", tab_path.display()))?;

    let md = skriv_sammanfattning(&korningar, torr, total_s);
    print!("{md}");
    std::fs::write(&md_path, &md).map_err(|e| format!("skriv {}: {e}", md_path.display()))?;
    eprintln!(
        "skrev {} ({} körningar) på {:.1} s",
        jsonl_path.display(),
        korningar.len(),
        total_s
    );
    Ok(())
}
