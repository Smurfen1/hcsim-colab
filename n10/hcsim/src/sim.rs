//! Världen på Hell Creek-kartan: växter (S5a) och Edmontosaurus (S5b).
//! Växttillväxt ur `flockdal.rs` `vaxer_med` (rad ~121); bete ur `bete5`/`betas` (rad ~136–249);
//! födsel/död ur `dal_halv2` / `fortplantning8` (kön, E_DELA, LIV, fadern bland grannar).

use std::collections::BTreeMap;

use hckarta::{Karta, Vatten};
use serde::{Deserialize, Serialize};

use crate::formaga::{Formaga, HjarnArm, Intagslasare, Minnestabell, EPS_ARV_XOR, EPS_START};
use crate::hjarna::{self, gauss, lott_bland_max, Hj};
use crate::hjarna2008::Hj2008;
use crate::klimat::Klimat;
use crate::rng::Rng;
use crate::tal::{self, vmax_ruta, A_N, SIGMA_MUT};

/// Grannoffsets: handling 0 = stå kvar; 1..8 = N, NO, Ö, SO, S, SV, V, NV (i öster, j norr).
/// Kartans kant (Håkans Alt 1712 och 1725; agenternas Alt 1717–1719; strykprovet i piharness docs/evolution/kantvagg-strykprov.txt):
/// `Vagg` = ett steg utanför kartan ger att djuret står kvar (standard; alla försök och prov utom visningskörningen); `Torus` = motsatta
/// sidor sitter ihop, så kartan saknar kant. Strykprovet: varje stängd kant samlade djuren (kanten och ett bältes ytterkant); sammanfogade
/// sidor gav jämn fördelning. Visningskörningen (`kartbild`) använder `Torus` med ett bälte runt fönstret, så skarven ligger utanför det som visas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KantLage {
    #[default]
    Vagg,
    Torus,
}

fn ett() -> f64 {
    1.0
}

/// Årets steg för födslar med växeln `fodsel_arlig` (fas 0,3 av året, våren; eget val).
pub const FODSEL_STEG: i64 = 45;

pub const GRANNAR: [(i32, i32); 9] = [
    (0, 0),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// Arm för populationssvepet (Alt 1198): märke utan fördel, med rabatt, eller inget märke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkLage {
    /// Inget märke sätts; samma värld som före Alt 1198.
    Utan,
    /// Hälften märks vid år 30; märket ger ingen fördel.
    Noll,
    /// Hälften märks vid år 30; märkta betalar MARKT_RABATT · KOST_STEG.
    Positiv,
}

impl MarkLage {
    pub fn fran_str(s: &str) -> Option<MarkLage> {
        match s {
            "utan" => Some(MarkLage::Utan),
            "noll" => Some(MarkLage::Noll),
            "positiv" => Some(MarkLage::Positiv),
            _ => None,
        }
    }

    pub fn som_str(self) -> &'static str {
        match self {
            MarkLage::Utan => "utan",
            MarkLage::Noll => "noll",
            MarkLage::Positiv => "positiv",
        }
    }
}

/// Separat sådd för märkningen så världens Rng-följd inte ändras (frö XOR denna).
pub const MARKT_FRO_XOR: u64 = 0xA11E_1198_4D52_4B54;

/// Fast tillägg till fröet för turordningens egen slumpström (Alt 1246).
/// Världens `g` rörs inte av lotteriet.
pub const TURORDNING_FRO_TILLAGG: u64 = 0x1246_7A10_0D15_0001;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dino {
    pub lev: bool,
    pub i: usize,
    pub j: usize,
    pub e: f64,
    pub age: i64,
    /// true = hona
    pub hona: bool,
    pub hj: Hj,
    pub uid: u64,
    /// Ärftligt märke (Alt 1198); ungen ärver moderns värde.
    pub markt: bool,
    /// Beteendeförmåga (hjärnprovet); före märkning alltid `Lar`.
    pub formaga: Formaga,
    /// Minnestabell (används vid `Tabell` / `TabellUtanSlump`).
    pub tabell: Minnestabell,
    /// Intagsläsare (används vid `LarIntagslasare`); börjar på 0, ärvs inte.
    pub lasare: Intagslasare,
    /// Egen slumpföljd (slump/tabell/lottning); rör inte världens Rng.
    pub egen_rng: Rng,
    /// Intaget i förra betessteget (för ledprovets logg); None före första betet.
    pub forra_intag: Option<f64>,
    /// Ärvd utforskningsgen ε ∈ [0, 1] (Alt 1463); används bara av `lar_utforsk`.
    #[serde(default = "default_eps")]
    pub eps: f64,
    /// Antal gånger hjärnan själv valt varje handling efter märkningen (läser bara).
    #[serde(default)]
    pub hj_handlingar: [u64; A_N],
    /// Djursteg efter märkningen (alla handlingar; för "aldrig valda"-tröskeln).
    #[serde(default)]
    pub steg_efter_mark: u64,
    /// Alt 2059-hjärna (används bara av `Lar2059`, `Frusen2059`, `Stryk2059`).
    #[serde(default)]
    pub hj2059: Option<Box<Hj2008>>,
}

fn default_eps() -> f64 {
    EPS_START
}

/// Minsta antal djursteg efter märkning för "aldrig valda av hjärnan" (Alt 1463).
pub const ALDRIG_MIN_STEG: u64 = 100;

/// Ärver ε: blandning av föräldrarna + N(0, SIGMA_MUT), klämpt till [0, 1].
/// Använder ungens uid (inte världens Rng) så `hcsim kor` behåller samma djur_summa.
pub fn arv_eps(mor: f64, far: f64, uid: u64) -> f64 {
    let mut g = Rng::new(uid ^ EPS_ARV_XOR);
    let bas = if g.f() < 0.5 { mor } else { far };
    (bas + SIGMA_MUT * gauss(&mut g)).clamp(0.0, 1.0)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Raknare {
    pub fodda: u64,
    pub dod_svalt: u64,
    pub dod_alder: u64,
    /// Per år (nollställs vid årsrapport).
    pub fodda_ar: u64,
    pub dod_svalt_ar: u64,
    pub dod_alder_ar: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArRad {
    pub ar: i64,
    pub antal: usize,
    pub fodda: u64,
    pub dod_svalt: u64,
    pub dod_alder: u64,
    pub v_medel: f64,
}

/// Ackumulator för förutsägarens medelfel (Alt 1203, bara redovisning).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FelAck {
    pub sum: f64,
    pub n: u64,
}

impl FelAck {
    pub fn lagg(&mut self, fel: f64) {
        self.sum += fel;
        self.n += 1;
    }

    pub fn medel(&self) -> Option<f64> {
        if self.n == 0 {
            None
        } else {
            Some(self.sum / self.n as f64)
        }
    }
}

/// En rad i ledprovets/rangprovets loggkörning `r_lar` (läser bara; Alt 1221/1228).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedLoggRad {
    pub fro: u64,
    /// Djurets uid (rangprovet; 0 i äldre ledprovsloggar).
    pub djur_id: u64,
    pub handling: usize,
    pub s: [f64; tal::S_N],
    /// Hjärnvärde för den valda handlingen (ledprovets G2).
    pub hj_varde: f64,
    /// Hjärnvärde `v · y_a + v0` för alla 9 handlingar (rangprovet).
    pub hj_varden: [f64; A_N],
    pub intag_fore: f64,
    pub intag_efter: f64,
}

/// Logg av sinnena/värdet/intaget (läser bara; ingen världsändring).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedLogg {
    pub pa: bool,
    pub fro: u64,
    /// Logga vart `var_n`:e djursteg (underlaget: 10).
    pub var_n: u64,
    pub max_rader: usize,
    pub djursteg: u64,
    pub rader: Vec<LedLoggRad>,
    /// Väntar på intag från samma stegs `betar` (djurindex, ofullständig rad).
    pub pending: Vec<(usize, LedLoggRad)>,
}

impl Default for LedLogg {
    fn default() -> Self {
        Self {
            pa: false,
            fro: 0,
            var_n: 10,
            max_rader: 50_000,
            djursteg: 0,
            rader: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl LedLogg {
    pub fn krav_data(&self) -> Result<(), String> {
        if self.pa && self.rader.is_empty() {
            Err("tom logg: inga rader efter märkningen".into())
        } else {
            Ok(())
        }
    }

    fn ska_logga(&self) -> bool {
        self.pa
            && self.var_n > 0
            && self.djursteg > 0
            && self.djursteg % self.var_n == 0
            && self.rader.len() + self.pending.len() < self.max_rader
    }
}

/// Andel träff för en riktning: 1/n_max om riktningen är bland max, annars 0.
pub fn traff_andel(vard: &[f64; A_N], riktning: usize) -> f64 {
    let max_v = vard.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut n_max = 0usize;
    let mut har = false;
    for (a, &v) in vard.iter().enumerate() {
        if v == max_v {
            n_max += 1;
            if a == riktning {
                har = true;
            }
        }
    }
    if har && n_max > 0 {
        1.0 / n_max as f64
    } else {
        0.0
    }
}

/// En rad i signalprovets loggkörning `v_lar` (läser bara; Alt 1251).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalLoggRad {
    pub fro: u64,
    pub djur_id: u64,
    pub s: [f64; tal::S_N],
    pub z: [f64; tal::Z_N],
    pub hj_varden: [f64; A_N],
    /// Lägesberoende del `v · W_a · z` per handling.
    pub lage: [f64; A_N],
    /// Fast del `v · b_a` per handling.
    pub fast: [f64; A_N],
}

/// Logg av sinnen/z/värde/delar (läser bara; ingen världsändring; Alt 1251).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalLogg {
    pub pa: bool,
    pub fro: u64,
    pub var_n: u64,
    pub max_rader: usize,
    pub djursteg: u64,
    pub rader: Vec<SignalLoggRad>,
    /// Ingreppets träff vid märkningen (medel över levande lar).
    pub ingrepp_mark: Option<f64>,
    /// Ingreppets träff vid slutet.
    pub ingrepp_slut: Option<f64>,
}

impl Default for SignalLogg {
    fn default() -> Self {
        Self {
            pa: false,
            fro: 0,
            var_n: 10,
            max_rader: 50_000,
            djursteg: 0,
            rader: Vec::new(),
            ingrepp_mark: None,
            ingrepp_slut: None,
        }
    }
}

impl SignalLogg {
    pub fn krav_data(&self) -> Result<(), String> {
        if self.pa && self.rader.is_empty() {
            Err("tom signallogg: inga rader efter märkningen".into())
        } else {
            Ok(())
        }
    }

    fn ska_logga(&self) -> bool {
        self.pa
            && self.var_n > 0
            && self.djursteg > 0
            && self.djursteg % self.var_n == 0
            && self.rader.len() < self.max_rader
    }
}

/// Ett block om 1 000 steg efter märkningen (Alt 1253; läser bara).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BokforingBlock {
    pub fro: u64,
    /// Armen (u_to / a_to / b_to); sätts av körningen.
    pub arm: String,
    pub formaga: String,
    pub block: u64,
    pub fodda: u64,
    pub dod_svalt: u64,
    pub dod_alder: u64,
    pub antal_slut: usize,
    pub intag_sum: f64,
    pub djursteg: u64,
}

impl BokforingBlock {
    pub fn medelintag(&self) -> Option<f64> {
        if self.djursteg == 0 {
            None
        } else {
            Some(self.intag_sum / self.djursteg as f64)
        }
    }
}

/// Ackumulator per förmåga inom aktuellt block.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct BokAck {
    fodda: u64,
    dod_svalt: u64,
    dod_alder: u64,
    intag_sum: f64,
    djursteg: u64,
}

/// Bokföring per 1 000 steg (Alt 1253; av som standard; läser bara).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Bokforing {
    pub pa: bool,
    pub fro: u64,
    pub arm: String,
    /// Stegtal vid märkningen (sätts när bokföringen startar).
    pub t0: i64,
    pub block_langd: i64,
    /// Levande per förmåga vid märkningen.
    pub antal_markering: BTreeMap<String, usize>,
    /// Summa födslar/döda sedan märkningen (för bokföringskontrollen).
    pub fodda_tot: BTreeMap<String, u64>,
    pub dod_tot: BTreeMap<String, u64>,
    ack: BTreeMap<String, BokAck>,
    pub block: Vec<BokforingBlock>,
    pub nasta_block: u64,
}

impl Bokforing {
    pub fn starta(&mut self, fro: u64, arm: &str, t0: i64, antal: BTreeMap<String, usize>) {
        self.pa = true;
        self.fro = fro;
        self.arm = arm.to_string();
        self.t0 = t0;
        self.block_langd = 1000;
        self.antal_markering = antal;
        self.fodda_tot.clear();
        self.dod_tot.clear();
        self.ack.clear();
        self.block.clear();
        self.nasta_block = 1;
    }

    fn buckla(&mut self, formaga: &str) -> &mut BokAck {
        self.ack.entry(formaga.to_string()).or_default()
    }

    pub fn lagg_fodsel(&mut self, formaga: Formaga) {
        if !self.pa {
            return;
        }
        let k = formaga.som_str().to_string();
        self.buckla(&k).fodda += 1;
        *self.fodda_tot.entry(k).or_default() += 1;
    }

    pub fn lagg_dod(&mut self, formaga: Formaga, svalt: bool) {
        if !self.pa {
            return;
        }
        let k = formaga.som_str().to_string();
        {
            let a = self.buckla(&k);
            if svalt {
                a.dod_svalt += 1;
            } else {
                a.dod_alder += 1;
            }
        }
        *self.dod_tot.entry(k).or_default() += 1;
    }

    pub fn lagg_intag(&mut self, formaga: Formaga, intag: f64) {
        if !self.pa {
            return;
        }
        let k = formaga.som_str().to_string();
        let a = self.buckla(&k);
        a.intag_sum += intag;
        a.djursteg += 1;
    }

    /// Stänger ett färdigt block med korrekt antal per förmåga.
    pub fn stang_om_dags(&mut self, t: i64, antal_nu: &BTreeMap<String, usize>) {
        if !self.pa || self.block_langd <= 0 {
            return;
        }
        let passerat = t - self.t0;
        if passerat <= 0 {
            return;
        }
        while passerat >= self.block_langd * self.nasta_block as i64 {
            let bnr = self.nasta_block;
            let mut nycklar: Vec<String> = self.ack.keys().cloned().collect();
            for k in antal_nu.keys() {
                if !nycklar.contains(k) {
                    nycklar.push(k.clone());
                }
            }
            for k in self.antal_markering.keys() {
                if !nycklar.contains(k) {
                    nycklar.push(k.clone());
                }
            }
            nycklar.sort();
            for k in &nycklar {
                let a = self.ack.remove(k).unwrap_or_default();
                self.block.push(BokforingBlock {
                    fro: self.fro,
                    arm: self.arm.clone(),
                    formaga: k.clone(),
                    block: bnr,
                    fodda: a.fodda,
                    dod_svalt: a.dod_svalt,
                    dod_alder: a.dod_alder,
                    antal_slut: antal_nu.get(k).copied().unwrap_or(0),
                    intag_sum: a.intag_sum,
                    djursteg: a.djursteg,
                });
            }
            self.nasta_block += 1;
        }
    }

    /// Bokföringskontrollen: antal vid märkning + födslar − döda = antal vid slut, per grupp.
    pub fn kontroll(&self, antal_slut: &BTreeMap<String, usize>) -> Result<(), String> {
        let mut nycklar: Vec<String> = self.antal_markering.keys().cloned().collect();
        for k in antal_slut.keys() {
            if !nycklar.contains(k) {
                nycklar.push(k.clone());
            }
        }
        for k in &nycklar {
            let n0 = self.antal_markering.get(k).copied().unwrap_or(0) as i64;
            let fodda = self.fodda_tot.get(k).copied().unwrap_or(0) as i64;
            let dod = self.dod_tot.get(k).copied().unwrap_or(0) as i64;
            let n1 = antal_slut.get(k).copied().unwrap_or(0) as i64;
            if n0 + fodda - dod != n1 {
                return Err(format!(
                    "bokföring bruten för {k}: {n0} + {fodda} − {dod} ≠ {n1}"
                ));
            }
        }
        Ok(())
    }
}

/// Mätfält per förmåga (Alt 1207 / 1213–1214): läser bara, drar inga slumptal.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FormagaMatning {
    pub handlingar: [u64; A_N],
    pub djursteg: u64,
    pub intag_sum: f64,
    pub e_vid_dod_sum: f64,
    pub e_vid_dod_n: u64,
    pub dod_svalt: u64,
    pub dod_alder: u64,
    pub andra_efter_sum: f64,
    pub andra_efter_n: u64,
    /// Medätare efter att stegets alla flyttar är gjorda (Alt 1229).
    pub medatare_sum: f64,
    pub medatare_n: u64,
    /// Djursteg där rutans växter inte räckte (alla fick mindre andel; Alt 1229).
    pub knapp_steg: u64,
    pub fel_tidigt: FelAck,
    pub fel_sent: FelAck,
    /// Förutsägarens fel tidigt/sent per handling (alla med hjärna).
    pub fel_tidigt_per: [FelAck; A_N],
    pub fel_sent_per: [FelAck; A_N],
    /// Steg där `v` på samma `y` valt annat än intagsläsaren.
    pub v_annorlunda: u64,
    pub v_jamforda: u64,
}

impl FormagaMatning {
    pub fn andel_handling(&self, a: usize) -> f64 {
        if self.djursteg == 0 {
            0.0
        } else {
            self.handlingar[a] as f64 / self.djursteg as f64
        }
    }

    pub fn andel_sta_kvar(&self) -> f64 {
        self.andel_handling(0)
    }

    /// Entropi −Σ p ln p över handlingarna (0 om inga steg).
    pub fn entropi(&self) -> f64 {
        if self.djursteg == 0 {
            return 0.0;
        }
        let n = self.djursteg as f64;
        let mut h = 0.0;
        for &c in &self.handlingar {
            if c == 0 {
                continue;
            }
            let p = c as f64 / n;
            h -= p * p.ln();
        }
        h
    }

    pub fn intag_per_steg(&self) -> Option<f64> {
        if self.djursteg == 0 {
            None
        } else {
            Some(self.intag_sum / self.djursteg as f64)
        }
    }

    pub fn e_vid_dod_medel(&self) -> Option<f64> {
        if self.e_vid_dod_n == 0 {
            None
        } else {
            Some(self.e_vid_dod_sum / self.e_vid_dod_n as f64)
        }
    }

    pub fn andra_medel(&self) -> Option<f64> {
        if self.andra_efter_n == 0 {
            None
        } else {
            Some(self.andra_efter_sum / self.andra_efter_n as f64)
        }
    }

    pub fn medatare_medel(&self) -> Option<f64> {
        if self.medatare_n == 0 {
            None
        } else {
            Some(self.medatare_sum / self.medatare_n as f64)
        }
    }

    pub fn andel_knapp(&self) -> Option<f64> {
        if self.djursteg == 0 {
            None
        } else {
            Some(self.knapp_steg as f64 / self.djursteg as f64)
        }
    }

    pub fn andel_v_annorlunda(&self) -> Option<f64> {
        if self.v_jamforda == 0 {
            None
        } else {
            Some(self.v_annorlunda as f64 / self.v_jamforda as f64)
        }
    }

    pub fn fel_tidigt_handling(&self, a: usize) -> Option<f64> {
        self.fel_tidigt_per[a].medel()
    }

    pub fn fel_sent_handling(&self, a: usize) -> Option<f64> {
        self.fel_sent_per[a].medel()
    }
}

/// Ackumulator för de tre kvadratfelen (Alt 1462); läser bara.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KopierArAck {
    pub forutsag_sum: f64,
    pub kopiering_sum: f64,
    pub fast_sum: f64,
    /// Antal djursteg där felet räknades.
    pub n: u64,
}

impl KopierArAck {
    pub fn lagg(&mut self, forutsag: f64, kopiering: f64, fast: f64) {
        self.forutsag_sum += forutsag;
        self.kopiering_sum += kopiering;
        self.fast_sum += fast;
        self.n += 1;
    }

    pub fn medel_forutsag(&self) -> Option<f64> {
        if self.n == 0 {
            None
        } else {
            Some(self.forutsag_sum / self.n as f64)
        }
    }

    pub fn medel_kopiering(&self) -> Option<f64> {
        if self.n == 0 {
            None
        } else {
            Some(self.kopiering_sum / self.n as f64)
        }
    }

    pub fn medel_fast(&self) -> Option<f64> {
        if self.n == 0 {
            None
        } else {
            Some(self.fast_sum / self.n as f64)
        }
    }
}

/// Kopieringsmätning per år efter märkningen och totalt (Alt 1462; av som standard).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KopierMatning {
    pub pa: bool,
    pub tot: KopierArAck,
    pub per_ar: BTreeMap<i64, KopierArAck>,
}

/// Ackumulator för "aldrig valda av hjärnan" per grupp (Alt 1463).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AldrigAck {
    pub sum_aldrig: f64,
    pub sum_vanligaste_andel: f64,
    pub n_djur: u64,
}

impl AldrigAck {
    pub fn lagg(&mut self, aldrig: f64, vanligaste_andel: f64) {
        self.sum_aldrig += aldrig;
        self.sum_vanligaste_andel += vanligaste_andel;
        self.n_djur += 1;
    }

    pub fn medel_aldrig(&self) -> Option<f64> {
        if self.n_djur == 0 {
            None
        } else {
            Some(self.sum_aldrig / self.n_djur as f64)
        }
    }

    pub fn medel_vanligaste_andel(&self) -> Option<f64> {
        if self.n_djur == 0 {
            None
        } else {
            Some(self.sum_vanligaste_andel / self.n_djur as f64)
        }
    }
}

/// ε-provtagning vid ett år (levande märkta).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EpsArAck {
    pub sum: f64,
    pub n: u64,
}

impl EpsArAck {
    pub fn medel(&self) -> Option<f64> {
        if self.n == 0 {
            None
        } else {
            Some(self.sum / self.n as f64)
        }
    }
}

/// Mätning för Alt 1463 (av som standard; läser bara / räknar).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UtforskMatning {
    pub pa: bool,
    /// Kopieringsfel per förmåga (grupp).
    pub kopier_per: BTreeMap<String, KopierArAck>,
    /// Aldrig valda / vanligaste andel per förmåga.
    pub aldrig_per: BTreeMap<String, AldrigAck>,
    /// ε-medel bland levande märkta per år (provtas var 10:e år).
    pub eps_per_ar: BTreeMap<i64, EpsArAck>,
    /// Uid som redan bokförts i aldrig_per (död eller slut).
    pub bokforda_uid: Vec<u64>,
}

impl UtforskMatning {
    pub fn lagg_kopier(&mut self, formaga: Formaga, forutsag: f64, kopiering: f64, fast: f64) {
        if !self.pa {
            return;
        }
        self.kopier_per
            .entry(formaga.som_str())
            .or_default()
            .lagg(forutsag, kopiering, fast);
    }

    pub fn redan_bokford(&self, uid: u64) -> bool {
        self.bokforda_uid.contains(&uid)
    }

    pub fn lagg_aldrig_djur(&mut self, formaga: Formaga, uid: u64, aldrig: f64, vanligaste: f64) {
        if !self.pa || self.redan_bokford(uid) {
            return;
        }
        self.bokforda_uid.push(uid);
        self.aldrig_per
            .entry(formaga.som_str())
            .or_default()
            .lagg(aldrig, vanligaste);
    }

    pub fn provta_eps(&mut self, ar: i64, eps_sum: f64, n: u64) {
        if !self.pa || n == 0 {
            return;
        }
        let a = self.eps_per_ar.entry(ar).or_default();
        a.sum += eps_sum;
        a.n += n;
    }
}

impl KopierMatning {
    pub fn lagg(&mut self, ar_efter: i64, forutsag: f64, kopiering: f64, fast: f64) {
        if !self.pa || ar_efter < 0 {
            return;
        }
        self.tot.lagg(forutsag, kopiering, fast);
        self.per_ar
            .entry(ar_efter)
            .or_default()
            .lagg(forutsag, kopiering, fast);
    }

    pub fn krav_data(&self) -> Result<(), String> {
        if self.tot.n == 0 {
            Err("tom kopieringsmätning: inga djursteg efter märkningen".into())
        } else {
            Ok(())
        }
    }
}

/// Samlad mätning per förmåga. När `pa` är false skrivs inget (världen orörd).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Matning {
    pub pa: bool,
    pub per: BTreeMap<String, FormagaMatning>,
}

impl Matning {
    fn buckla(&mut self, formaga: Formaga) -> &mut FormagaMatning {
        let nyckel = formaga.som_str();
        self.per.entry(nyckel).or_default()
    }

    pub fn lagg_handling(&mut self, formaga: Formaga, a: usize) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        m.handlingar[a] += 1;
        m.djursteg += 1;
    }

    pub fn lagg_andra(&mut self, formaga: Formaga, andra: usize) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        m.andra_efter_sum += andra as f64;
        m.andra_efter_n += 1;
    }

    pub fn lagg_medatare(&mut self, formaga: Formaga, medatare: usize) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        m.medatare_sum += medatare as f64;
        m.medatare_n += 1;
    }

    pub fn lagg_knapp(&mut self, formaga: Formaga) {
        if !self.pa {
            return;
        }
        self.buckla(formaga).knapp_steg += 1;
    }

    pub fn lagg_intag(&mut self, formaga: Formaga, intag: f64) {
        if !self.pa {
            return;
        }
        self.buckla(formaga).intag_sum += intag;
    }

    pub fn lagg_dod(&mut self, formaga: Formaga, e: f64, svalt: bool) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        m.e_vid_dod_sum += e;
        m.e_vid_dod_n += 1;
        if svalt {
            m.dod_svalt += 1;
        } else {
            m.dod_alder += 1;
        }
    }

    pub fn lagg_fel(&mut self, formaga: Formaga, tidigt: bool, fel: f64, handling: Option<usize>) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        if tidigt {
            m.fel_tidigt.lagg(fel);
            if let Some(a) = handling {
                m.fel_tidigt_per[a].lagg(fel);
            }
        } else {
            m.fel_sent.lagg(fel);
            if let Some(a) = handling {
                m.fel_sent_per[a].lagg(fel);
            }
        }
    }

    pub fn lagg_v_jamforelse(&mut self, formaga: Formaga, annorlunda: bool) {
        if !self.pa {
            return;
        }
        let m = self.buckla(formaga);
        m.v_jamforda += 1;
        if annorlunda {
            m.v_annorlunda += 1;
        }
    }

    /// Stoppar med fel om inga djursteg registrerats (tom mätning).
    pub fn krav_data(&self) -> Result<(), String> {
        let tot: u64 = self.per.values().map(|m| m.djursteg).sum();
        if tot == 0 {
            Err("tom mätning: inga djursteg efter märkningen".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sim {
    pub b: usize,
    pub h: usize,
    pub ruta_m: f64,
    /// Växtbiomassa per ruta (absolut).
    pub v: Vec<f64>,
    /// Tak per ruta (beräknat vid start ur karta; ändras inte).
    pub vmax: Vec<f64>,
    /// Landandel 0..1 (0 för hav/flod/sjö).
    pub landandel: Vec<f64>,
    /// Vattentyp-kopia för rörelse (läst från karta, skrivs aldrig tillbaka).
    #[serde(with = "crate::serdej::vatten_lista")]
    pub vatten: Vec<Vatten>,
    pub djur: Vec<Dino>,
    pub klimat: Klimat,
    pub g: Rng,
    pub t: i64,
    pub rak: Raknare,
    pub nasta_uid: u64,
    /// Om true: djur som försöker gå till hav står kvar (alltid).
    pub hav_stangt: bool,
    /// Kartans kant (se `KantLage`). Standard `Vagg`. Sparas inte (sparade lägen och deras kontrollsummor oförändrade; laddat = `Vagg`).
    #[serde(skip)]
    pub kant: KantLage,
    /// Bältets bredd i rutor runt det fönster som kartbilden visar (0 = hela kartan visas). Sparas inte.
    #[serde(skip)]
    pub fonster_k: usize,
    /// Försöksväxel (utdöendet i visningskörningen, 2026-11-22; av som standard): ett djurs intag följer rutans fyllnad `V / tak`
    /// (högst `AT_DINO` per steg, som i låset v3.0), inte rutans hela växtmängd `V / VMAX`. Sparas inte.
    #[serde(skip)]
    pub intag_per_fyllnad: bool,
    /// Försöksväxel (av som standard): en hona föder först som vuxen (`LITEN` steg, samma gräns som hanarna). Sparas inte.
    #[serde(skip)]
    pub hona_vuxen: bool,
    /// Försöksväxel (av som standard): födslar sker bara i ett steg per år (`FODSEL_STEG`), högst en unge per hona och år. Sparas inte.
    #[serde(skip)]
    pub fodsel_arlig: bool,
    /// Försöksväxel: energins tak i steg av grundkostnaden (`None` = `E_HANE`, som i dag). Sparas inte.
    #[serde(skip)]
    pub e_tak: Option<f64>,
    /// Försöksväxel: energitalen `E_START`, `E_DELA` och `E_HANE` gånger detta tal (1 = som i dag). Sparas inte.
    #[serde(skip, default = "ett")]
    pub e_skala: f64,
    /// Försöksväxel: åldern i steg då ett djur kan få ungar (`None` = `LITEN`). Gäller hanar, och honor när `hona_vuxen` är på. Sparas inte.
    #[serde(skip)]
    pub mogen_steg: Option<i64>,
    /// Försöksväxel: energin en hona minst måste ha för att föda (`None` = `E_DELA` gånger `e_skala`). Kostnaden per unge ändras inte. Sparas inte.
    #[serde(skip)]
    pub fodsel_troskel: Option<f64>,
    /// Försöksväxel (agenternas Alt 1858, 2026-11-23): högsta ålder i steg (`None` = `LIV`). Sparas inte.
    #[serde(skip)]
    pub liv_steg: Option<i64>,
    /// Valideringen av hjärnans förutsägelser (Håkan 2026-11-23, Fas 1): djuret på platsen `.0` tar handlingen `.1` i nästa steg
    /// i stället för sitt eget val. Bara i `ror_sig`; används på en kopia av världen. None = vanlig körning.
    #[serde(skip)]
    pub tvinga: Option<(usize, usize)>,
    /// Informationstestet (Håkans Alt 1974): alla djur på platserna `.0` (sorterade) tar handlingen `.1` i nästa steg. Bara i `ror_sig`; bara på en kopia.
    #[serde(skip)]
    pub tvinga_grupp: Option<(Vec<usize>, usize)>,
    /// Populationssvepets arm (styr märkningens kostnadseffekt).
    pub mark_lage: MarkLage,
    /// Hjärnprovets arm (None = vanligt läge / svep).
    #[serde(with = "crate::serdej::hjarn_arm_opt")]
    pub hjarn_arm: Option<HjarnArm>,
    /// True efter att hälften märkts och förmågor satts.
    pub markering_skedd: bool,
    /// Förutsägarfel tidigt/sent × märkt/omärkt (Alt 1203).
    pub fel_tidigt_markt: FelAck,
    pub fel_tidigt_omarkt: FelAck,
    pub fel_sent_markt: FelAck,
    pub fel_sent_omarkt: FelAck,
    /// Mätfält Alt 1207 (av som standard).
    pub matning: Matning,
    /// Återväxtfaktor på R_V och SOL (ledprovet; 1 = samma värld som i dag).
    pub atervaxt: f64,
    /// Ledprovets logg (av som standard; läser bara).
    pub ledlogg: LedLogg,
    /// Turordning i stället för samtidiga flyttar (Alt 1246; av som standard).
    pub turordning: bool,
    /// Egen slumpström för turordningen (frö + TURORDNING_FRO_TILLAGG); rör inte `g`.
    pub tur_rng: Rng,
    /// Uid-ordning i senaste turordningssteget (för prov); tom om läget är av.
    pub senaste_turordning: Vec<u64>,
    /// Läge a: flytt i tur, bete samtidigt (Alt 1252; av som standard).
    pub flytt_i_tur: bool,
    /// Läge b: sinnen från stegets början, flytt och bete i tur (Alt 1252; av som standard).
    pub bete_i_tur: bool,
    /// Signallogg `v_lar` (Alt 1251; av som standard; läser bara).
    pub signal_logg: SignalLogg,
    /// Bokföring per 1 000 steg (Alt 1253; av som standard; läser bara).
    pub bokforing: Bokforing,
    /// Kopieringsmätning Alt 1462 (av som standard; läser bara).
    #[serde(default)]
    pub kopier_matning: KopierMatning,
    /// Utforskningsmätning Alt 1463 (av som standard; läser bara).
    #[serde(default)]
    pub utforsk_matning: UtforskMatning,
    /// Alt 1954 + Alt 1955 / Alt 2238 (av som standard): återväxt ur obetad reserv med
    /// kartans 6 växtgrupper och låsta `atervaxt.tsv`. Sparas inte.
    #[serde(skip)]
    pub atervaxt_reserv: bool,
    /// Per-ruta återväxt per steg ur obetad reserv (används bara när `atervaxt_reserv == true`).
    #[serde(skip)]
    pub p_steg: Vec<f64>,
    /// Valfritt krontäcke per ruta ur `hckarta::rakna_basunderlag` (för `kartbild-live.json`).
    #[serde(skip)]
    pub kron_tathet: Vec<f32>,
    /// Valfria 6-grupps växttätheter per ruta ur `hckarta::rakna_vaxt_tathet` (för `kartbild-live.json`).
    #[serde(skip)]
    pub vaxt_grupper: Vec<[f32; hckarta::VAXT_N]>,
    /// Alt 2260 (av som standard): när true väljs fadern bara bland hanar med samma `markt` som modern (reproduktiv isolering).
    #[serde(skip)]
    pub parning_inom_linje: bool,
    /// Läsbar gendiagnostik (Alt 2258): antal genomförda födslar efter skapande/nollställning.
    #[serde(skip)]
    pub parningar_totalt: u64,
    /// Läsbar gendiagnostik (Alt 2258): antal födslar där `far.markt != mor.markt`.
    #[serde(skip)]
    pub parningar_kors: u64,
}

impl Sim {
    /// Bygger världen ur en karta (läser bara; muterar aldrig kartan).
    /// Grannrutan efter kantregeln: med `Torus` förs ett läge utanför kartan till motsatta sidan; med `Vagg` oförändrat.
    pub fn vill(&self, c: usize) -> f64 {
        if self.intag_per_fyllnad {
            tal::AT_DINO * self.v[c] / self.vmax[c].max(1e-12)
        } else {
            tal::AT_DINO * self.v[c] / tal::VMAX
        }
    }

    /// Grannrutan efter kantregeln (se `KantLage`).
    pub fn kant_granne(&self, ni: i32, nj: i32) -> (i32, i32) {
        match self.kant {
            KantLage::Vagg => (ni, nj),
            KantLage::Torus => (ni.rem_euclid(self.b as i32), nj.rem_euclid(self.h as i32)),
        }
    }

    pub fn fran_karta(kart: &Karta, fro: u64, n0: usize) -> Sim {
        let b = kart.omrade.b;
        let h = kart.omrade.h;
        let ruta_m = kart.omrade.ruta_m;
        let n = b * h;
        let mut landandel = vec![0.0; n];
        let mut vmax = vec![0.0; n];
        let mut v = vec![0.0; n];
        let mut vatten = Vec::with_capacity(n);
        for c in 0..n {
            let vt = kart.vattentyp.varden[c];
            vatten.push(vt);
            let la = landandel_av(vt, ruta_m);
            landandel[c] = la;
            let vaxt = kart.vaxt.varden[c] as f64;
            let tak = vmax_ruta(ruta_m, la, vaxt);
            vmax[c] = tak;
            v[c] = tak * tal::V0_ANDEL;
        }
        let klimat = Klimat::ny(tal::STEG_PER_AR as f64);
        let mut g = Rng::new(fro);
        let mut land_rutor: Vec<(usize, usize)> = Vec::new();
        for j in 0..h {
            for i in 0..b {
                if landandel[j * b + i] > 0.0 && !ar_hav(vatten[j * b + i]) {
                    // Starta bara på land/träsk (inte flod/sjö).
                    if matches!(vatten[j * b + i], Vatten::Land | Vatten::Trask) {
                        land_rutor.push((i, j));
                    }
                }
            }
        }
        // Om bara hav/flod: inga startplatser → tom flock (utdöende).
        let mut djur = Vec::with_capacity(n0);
        let mut nasta_uid = 1u64;
        if !land_rutor.is_empty() {
            for k in 0..n0 {
                let (i, j) = land_rutor[g.under(land_rutor.len())];
                let hona = k % 2 == 0; // hälften honor (jämnt index)
                djur.push(Dino {
                    lev: true,
                    i,
                    j,
                    e: tal::E_START,
                    age: g.under(tal::LIV as usize) as i64,
                    hona,
                    hj: Hj::slump(&mut g),
                    uid: nasta_uid,
                    markt: false,
                    formaga: Formaga::Lar,
                    tabell: Minnestabell::ny(),
                    lasare: Intagslasare::ny(),
                    egen_rng: Rng::new(nasta_uid),
                    forra_intag: None,
                    eps: EPS_START,
                    hj_handlingar: [0; A_N],
                    steg_efter_mark: 0,
                    hj2059: None,
                });
                nasta_uid += 1;
            }
        }
        Sim {
            b,
            h,
            ruta_m,
            v,
            vmax,
            landandel,
            vatten,
            djur,
            klimat,
            g,
            t: 0,
            rak: Raknare::default(),
            nasta_uid,
            hav_stangt: true,
            kant: KantLage::Vagg,
            fonster_k: 0,
            intag_per_fyllnad: false,
            hona_vuxen: false,
            fodsel_arlig: false,
            e_tak: None,
            e_skala: 1.0,
            mogen_steg: None,
            fodsel_troskel: None,
            liv_steg: None,
            tvinga: None,
            tvinga_grupp: None,
            mark_lage: MarkLage::Utan,
            hjarn_arm: None,
            markering_skedd: false,
            fel_tidigt_markt: FelAck::default(),
            fel_tidigt_omarkt: FelAck::default(),
            fel_sent_markt: FelAck::default(),
            fel_sent_omarkt: FelAck::default(),
            matning: Matning::default(),
            atervaxt: 1.0,
            ledlogg: LedLogg::default(),
            turordning: false,
            tur_rng: Rng::new(fro.wrapping_add(TURORDNING_FRO_TILLAGG)),
            senaste_turordning: Vec::new(),
            flytt_i_tur: false,
            bete_i_tur: false,
            signal_logg: SignalLogg::default(),
            bokforing: Bokforing::default(),
            kopier_matning: KopierMatning::default(),
            utforsk_matning: UtforskMatning::default(),
            atervaxt_reserv: false,
            p_steg: Vec::new(),
            kron_tathet: Vec::new(),
            vaxt_grupper: Vec::new(),
            parning_inom_linje: false,
            parningar_totalt: 0,
            parningar_kors: 0,
        }
    }

    /// Kopplar in SIMULERINGENs kartlager för 6 växtgrupper och låsta `atervaxt.tsv` (`Alt 1954` + `Alt 1955` / `Alt 2238`).
    /// Skalar kartans tak och återväxt så att vanlig skogsmark behåller samma energiskala som standard-`hcsim`,
    /// medan relativa skillnader mellan flodslätt/örter, lövskog, sumpcypress/träsk och sjö styrs direkt av källtabellen.
    pub fn applicera_atervaxt_2238(&mut self, kart: &Karta, andelar: &hckarta::FlodAndelar) -> Result<(), String> {
        let n = self.v.len();
        if kart.vattentyp.varden.len() != n {
            return Err(format!("kartlängd {} != sim {}", kart.vattentyp.varden.len(), n));
        }
        let tathet = hckarta::rakna_vaxt_tathet(
            &kart.vattentyp.varden,
            &kart.period.varden,
            &andelar.balte_andel.varden,
            &andelar.vall_andel.varden,
        );
        let bas = hckarta::rakna_basunderlag(kart, Some(andelar))?;

        // Låsta källtal ur `n10/hckarta/atervaxt.tsv` i ordningen [lovtrad, barrtrad, ort, ormbunke, kottepalm_ginkgo, vattenvaxt]:
        // B_kg_m2: betbart tak (kg torrvikt/m²) — Conner & Day 1976, Sonnier 2020, Le Duc m.fl. 2003, Hall & Okali 1974.
        const B_KG_M2: [f64; hckarta::VAXT_N] = [0.200, 0.020, 0.474, 0.345, 0.200, 0.300];
        // P_kg_m2_ar: årsproduktion (kg torrvikt/m²/år) — Conner & Day 1976, McNaughton 1985, Lowday & Marrs 1992, Hall & Okali 1974.
        const P_KG_M2_AR: [f64; hckarta::VAXT_N] = [0.574, 0.062, 0.664, 0.345, 0.574, 0.600];
        // Referensruta (vanlig lövskogsmark utan flodbälte): [0.50, 0.0, 0.10, 0.02, 0.01, 0.0]
        const B_REF: f64 = 0.50 * 0.200 + 0.10 * 0.474 + 0.02 * 0.345 + 0.01 * 0.200; // 0.1563
        const P_REF: f64 = 0.50 * 0.574 + 0.10 * 0.664 + 0.02 * 0.345 + 0.01 * 0.574; // 0.36604
        // Referens-återväxt per steg: 0.0012 * tak_ref matchar standard-hcsims maximala logistiska tillväxt (bärkraft ~15 000 djur).
        let tak_ref = vmax_ruta(self.ruta_m, 1.0, 0.50);
        let p_steg_ref = 0.0012 * tak_ref;

        let mut p_steg = vec![0.0; n];
        for c in 0..n {
            let la = self.landandel[c];
            if la <= 0.0 {
                self.vmax[c] = 0.0;
                self.v[c] = 0.0;
                p_steg[c] = 0.0;
                continue;
            }
            let mut b_sum = 0.0;
            let mut p_sum = 0.0;
            for g in 0..hckarta::VAXT_N {
                let t_g = tathet[c][g] as f64;
                b_sum += t_g * B_KG_M2[g];
                p_sum += t_g * P_KG_M2_AR[g];
            }
            let tak = (b_sum / B_REF) * tak_ref * la;
            let ps = (p_sum / P_REF) * p_steg_ref * la;
            self.vmax[c] = tak;
            self.v[c] = tak * tal::V0_ANDEL;
            p_steg[c] = ps;
        }
        self.p_steg = p_steg;
        self.atervaxt_reserv = true;
        self.kron_tathet = bas.kron_tathet.varden;
        self.vaxt_grupper = tathet;
        Ok(())
    }

    /// Levande per förmåga (för bokföring).
    pub fn antal_per_formaga(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for d in self.djur.iter().filter(|d| d.lev) {
            *m.entry(d.formaga.som_str().to_string()).or_default() += 1;
        }
        m
    }

    /// Startar bokföringen efter märkningen (läser bara räknare).
    pub fn starta_bokforing(&mut self, fro: u64, arm: &str) {
        let antal = self.antal_per_formaga();
        self.bokforing.starta(fro, arm, self.t, antal);
    }

    /// Ingreppet (Alt 1251 / 1423): för varje riktning 1..8, sinnen med växtandel 1 där och 0
    /// annars; träff om handlingen med högst värde enligt `avlasare` är den riktningen
    /// (lika delar träffen). Avläsaren tar djur och sinnen → 9 värden.
    /// Läser bara; rör inte hjärna, växter eller slumpföljd.
    pub fn ingrepp_traff_avlasare<F>(&self, mut avlasare: F) -> f64
    where
        F: FnMut(&Dino, &[f64]) -> [f64; A_N],
    {
        let mut sum = 0.0;
        let mut n = 0usize;
        for d in self.djur.iter().filter(|d| d.lev) {
            let mut s = vec![0.0; tal::S_N];
            s[18] = (d.e / (tal::E_HANE * self.e_skala)).clamp(0.0, 2.0);
            let mut traff_d = 0.0;
            for riktning in 1..A_N {
                for a in 0..A_N {
                    s[a * 2] = if a == riktning { 1.0 } else { 0.0 };
                    s[a * 2 + 1] = 0.0;
                }
                let vard = avlasare(d, &s);
                traff_d += traff_andel(&vard, riktning);
            }
            sum += traff_d / 8.0;
            n += 1;
        }
        if n == 0 {
            0.0
        } else {
            sum / n as f64
        }
    }

    /// Ingreppet med `lar`-avläsaren (`vardets_delar`), samma tal som före Alt 1423.
    pub fn ingrepp_traff(&self) -> f64 {
        self.ingrepp_traff_avlasare(|d, s| {
            let (_, vard, _, _) = d.hj.vardets_delar(s);
            vard
        })
    }

    /// Kör ingreppet och sparar träffen (markering eller slut).
    pub fn kor_ingrepp(&mut self, vid_slut: bool) {
        if !self.signal_logg.pa {
            return;
        }
        let t = self.ingrepp_traff();
        if vid_slut {
            self.signal_logg.ingrepp_slut = Some(t);
        } else {
            self.signal_logg.ingrepp_mark = Some(t);
        }
    }

    pub fn cell(&self, i: usize, j: usize) -> usize {
        j * self.b + i
    }

    pub fn antal(&self) -> usize {
        self.djur.iter().filter(|d| d.lev).count()
    }

    pub fn antal_markta(&self) -> usize {
        self.djur.iter().filter(|d| d.lev && d.markt).count()
    }

    /// Andel märkta bland levande; 0 om inga lever.
    pub fn andel_markta(&self) -> f64 {
        let n = self.antal();
        if n == 0 {
            0.0
        } else {
            self.antal_markta() as f64 / n as f64
        }
    }

    /// Grundkostnad per steg (bara KOST_STEG; inte hjärna eller rörelseextra).
    pub fn grundkostnad(&self, markt: bool) -> f64 {
        if self.markering_skedd {
            if let Some(arm) = &self.hjarn_arm {
                let f = if markt { arm.markta } else { arm.omarkta };
                if let Formaga::Rabatt(x) = f {
                    return tal::KOST_STEG * x;
                }
            }
        }
        if self.mark_lage == MarkLage::Positiv && markt {
            tal::KOST_STEG * tal::MARKT_RABATT
        } else {
            tal::KOST_STEG
        }
    }

    /// Förmåga för ett djur givet märke och om märkning skett.
    pub fn formaga_for(&self, markt: bool) -> Formaga {
        if !self.markering_skedd {
            return Formaga::Lar;
        }
        match &self.hjarn_arm {
            Some(arm) => {
                if markt {
                    arm.markta
                } else {
                    arm.omarkta
                }
            }
            None => Formaga::Lar,
        }
    }

    /// Märker hälften av de levande med egen Rng (frö XOR MARKT_FRO_XOR).
    /// Rör inte världens `self.g`.
    pub fn markera_halften(&mut self, fro: u64) {
        let mut levande: Vec<usize> = self
            .djur
            .iter()
            .enumerate()
            .filter(|(_, d)| d.lev)
            .map(|(i, _)| i)
            .collect();
        if levande.is_empty() {
            return;
        }
        let mut mg = Rng::new(fro ^ MARKT_FRO_XOR);
        mg.blanda(&mut levande);
        let n_mark = levande.len() / 2;
        for &idx in levande.iter().take(n_mark) {
            self.djur[idx].markt = true;
        }
        if let Some(arm) = self.hjarn_arm.clone() {
            self.markering_skedd = true;
            for d in self.djur.iter_mut().filter(|d| d.lev) {
                d.formaga = if d.markt { arm.markta } else { arm.omarkta };
                if d.formaga.ar_tabell() {
                    d.tabell = Minnestabell::ny();
                }
                if d.formaga.har_intagslasare() {
                    d.lasare = Intagslasare::ny();
                }
                if d.formaga.cfg_2059().is_some() {
                    d.hj2059 = None;
                }
            }
        }
    }

    /// År sedan märkningen (−1 före); används för tidigt/sent fel (Alt 1203).
    fn ar_efter_markering(&self) -> i64 {
        if !self.markering_skedd {
            return -1;
        }
        self.t / tal::STEG_PER_AR - tal::MARKT_INKOR_AR
    }

    /// Tidigt = första 30 åren efter märkning; sent = sista 30 av 150.
    fn fel_fonster(&self) -> Option<&'static str> {
        let a = self.ar_efter_markering();
        if a < 0 {
            None
        } else if a < 30 {
            Some("tidigt")
        } else if a >= tal::MARKT_EFTER_AR - 30 {
            Some("sent")
        } else {
            None
        }
    }

    fn ackum_fel(&mut self, markt: bool, formaga: Formaga, fel: f64, handling: Option<usize>) {
        match self.fel_fonster() {
            Some("tidigt") => {
                if markt {
                    self.fel_tidigt_markt.lagg(fel);
                } else {
                    self.fel_tidigt_omarkt.lagg(fel);
                }
                if self.matning.pa && self.markering_skedd {
                    self.matning.lagg_fel(formaga, true, fel, handling);
                }
            }
            Some("sent") => {
                if markt {
                    self.fel_sent_markt.lagg(fel);
                } else {
                    self.fel_sent_omarkt.lagg(fel);
                }
                if self.matning.pa && self.markering_skedd {
                    self.matning.lagg_fel(formaga, false, fel, handling);
                }
            }
            _ => {}
        }
    }

    fn ackum_hj_fel(&mut self, idx: usize, formaga: Formaga) {
        let markt = self.djur[idx].markt;
        if let Some(fel) = self.djur[idx].hj.senaste_fel {
            let h = self.djur[idx].hj.senaste_fel_handling;
            self.ackum_fel(markt, formaga, fel, h);
        }
        self.ackum_kopier(idx, formaga);
        self.ackum_utforsk_kopier(idx, formaga);
    }

    /// Tre kvadratfel för `lar`-djur efter märkningen (Alt 1462; läser bara).
    fn ackum_kopier(&mut self, idx: usize, formaga: Formaga) {
        if !self.kopier_matning.pa || !self.markering_skedd {
            return;
        }
        if formaga != Formaga::Lar {
            return;
        }
        let hj = &self.djur[idx].hj;
        let (Some(f), Some(k), Some(fa)) = (
            hj.senaste_fel,
            hj.senaste_fel_kopiering,
            hj.senaste_fel_fast,
        ) else {
            return;
        };
        let ar = self.ar_efter_markering();
        self.kopier_matning.lagg(ar, f, k, fa);
    }

    /// Kopieringsfel per förmåga för Alt 1463 (läser bara).
    fn ackum_utforsk_kopier(&mut self, idx: usize, formaga: Formaga) {
        if !self.utforsk_matning.pa || !self.markering_skedd {
            return;
        }
        if !matches!(
            formaga,
            Formaga::Lar
                | Formaga::LarUtforsk
                | Formaga::LarUtforskFast(_)
                | Formaga::LarSlump(_)
                | Formaga::Rabatt(_)
        ) {
            return;
        }
        let hj = &self.djur[idx].hj;
        let (Some(f), Some(k), Some(fa)) = (
            hj.senaste_fel,
            hj.senaste_fel_kopiering,
            hj.senaste_fel_fast,
        ) else {
            return;
        };
        self.utforsk_matning.lagg_kopier(formaga, f, k, fa);
    }

    /// Notera djursteg / hjärnval för "aldrig valda" (Alt 1463).
    fn notera_utforsk_val(&mut self, idx: usize, a: usize, hjarnval: bool) {
        if !self.utforsk_matning.pa || !self.markering_skedd {
            return;
        }
        self.djur[idx].steg_efter_mark += 1;
        if hjarnval {
            self.djur[idx].hj_handlingar[a] += 1;
        }
    }

    /// Bokför ett djurs aldrig-valda-mått om det levt ≥ ALDRIG_MIN_STEG efter märkning.
    pub fn bokfor_aldrig_djur(&mut self, idx: usize) {
        if !self.utforsk_matning.pa {
            return;
        }
        let d = &self.djur[idx];
        if d.steg_efter_mark < ALDRIG_MIN_STEG {
            return;
        }
        let uid = d.uid;
        let formaga = d.formaga;
        let mut aldrig = 0usize;
        let mut tot = 0u64;
        let mut max_c = 0u64;
        for &c in &d.hj_handlingar {
            if c == 0 {
                aldrig += 1;
            }
            tot += c;
            if c > max_c {
                max_c = c;
            }
        }
        let vanligaste = if tot == 0 {
            0.0
        } else {
            max_c as f64 / tot as f64
        };
        self.utforsk_matning
            .lagg_aldrig_djur(formaga, uid, aldrig as f64, vanligaste);
    }

    /// Bokför alla levande (och tidigare döda som inte bokförts) vid körningens slut.
    pub fn bokfor_aldrig_alla(&mut self) {
        if !self.utforsk_matning.pa {
            return;
        }
        let n = self.djur.len();
        for i in 0..n {
            self.bokfor_aldrig_djur(i);
        }
    }

    /// Provta ε-medel bland levande märkta (anropas var 10:e år efter märkning).
    pub fn provta_eps_ar(&mut self) {
        if !self.utforsk_matning.pa || !self.markering_skedd {
            return;
        }
        let ar = self.t / tal::STEG_PER_AR;
        if ar % 10 != 0 {
            return;
        }
        let mut sum = 0.0;
        let mut n = 0u64;
        for d in self.djur.iter().filter(|d| d.lev && d.markt) {
            sum += d.eps;
            n += 1;
        }
        self.utforsk_matning.provta_eps(ar, sum, n);
    }

    /// ε-statistik vid slut bland levande märkta: medel, median, andel < 0,01.
    pub fn eps_slut_markta(&self) -> Option<(f64, f64, f64)> {
        self.eps_slut(true)
    }

    /// ε vid slutet i en grupp (Claudes tillägg: den omärkta gruppen ärver ε men använder den inte,
    /// så dess ε är en neutral jämförelse för drift och för gränsen vid 0).
    pub fn eps_slut(&self, markt: bool) -> Option<(f64, f64, f64)> {
        let mut v: Vec<f64> = self
            .djur
            .iter()
            .filter(|d| d.lev && d.markt == markt)
            .map(|d| d.eps)
            .collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = v.len();
        let medel = v.iter().sum::<f64>() / n as f64;
        let median = if n % 2 == 1 {
            v[n / 2]
        } else {
            0.5 * (v[n / 2 - 1] + v[n / 2])
        };
        let under: usize = v.iter().filter(|&&e| e < 0.01).count();
        let andel_under = under as f64 / n as f64;
        Some((medel, median, andel_under))
    }

    pub fn v_medel(&self) -> f64 {
        let mut s = 0.0;
        let mut n = 0usize;
        for c in 0..self.v.len() {
            if self.landandel[c] > 0.0 {
                s += self.v[c];
                n += 1;
            }
        }
        if n == 0 {
            0.0
        } else {
            s / n as f64
        }
    }

    /// Kontrollsumma över djurens tillstånd (för samma_fro_samma_summa).
    /// Märket ingår inte — nollarmen får samma summa som utan märke.
    pub fn djur_summa(&self) -> u64 {
        const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
        const FNV_PRIME: u64 = 1_099_511_628_211;
        let mut h = FNV_OFFSET;
        let mut feed = |bs: &[u8]| {
            for &b in bs {
                h ^= u64::from(b);
                h = h.wrapping_mul(FNV_PRIME);
            }
        };
        let mut levande: Vec<&Dino> = self.djur.iter().filter(|d| d.lev).collect();
        levande.sort_by_key(|d| d.uid);
        for d in levande {
            feed(&d.uid.to_le_bytes());
            feed(&(d.i as u64).to_le_bytes());
            feed(&(d.j as u64).to_le_bytes());
            feed(&d.e.to_le_bytes());
            feed(&d.age.to_le_bytes());
            feed(&[d.hona as u8]);
            for &x in &d.hj.gen {
                feed(&x.to_le_bytes());
            }
            for &x in &d.hj.wp {
                feed(&x.to_le_bytes());
            }
        }
        for &x in &self.v {
            feed(&x.to_le_bytes());
        }
        feed(&self.t.to_le_bytes());
        h
    }

    /// Ett steg: tillväxt → hjärna/rörelse → bete → kostnad → ålder → födsel/död.
    /// Med `turordning`: varje djur väljer, flyttar och betar i tur (egen slumpström).
    /// Med `flytt_i_tur` (a): flytt i tur, bete samtidigt. Med `bete_i_tur` (b):
    /// sinnen från stegets början, flytt och bete i tur. `turordning` oförändrad.
    pub fn steg(&mut self) {
        self.vaxer();
        if self.turordning {
            self.ror_sig_tur();
        } else if self.flytt_i_tur {
            self.ror_sig_flytt_i_tur();
            self.betar();
        } else if self.bete_i_tur {
            self.ror_sig_bete_i_tur();
        } else {
            self.senaste_turordning.clear();
            self.ror_sig();
            self.betar();
        }
        self.alder_fodsel_dod();
        self.t += 1;
        if self.utforsk_matning.pa && self.markering_skedd && self.t % tal::STEG_PER_AR == 0 {
            self.provta_eps_ar();
        }
        if self.bokforing.pa && self.markering_skedd {
            // Bara räkna antal vid blockgräns (var 1000:e steg) — inte varje steg.
            let passerat = self.t - self.bokforing.t0;
            if passerat > 0
                && self.bokforing.block_langd > 0
                && passerat % self.bokforing.block_langd == 0
            {
                let antal = self.antal_per_formaga();
                self.bokforing.stang_om_dags(self.t, &antal);
            }
        }
    }

    /// Kör `ar` år; returnerar rader var `var_ar`:e år (standard 10) plus sista.
    pub fn kor_ar(&mut self, ar: i64, var_ar: i64) -> Vec<ArRad> {
        let mut rader = Vec::new();
        let steg_tot = ar * tal::STEG_PER_AR;
        let mut nasta_rapport = var_ar;
        while self.t < steg_tot {
            let ar_fore = self.t / tal::STEG_PER_AR;
            self.steg();
            let ar_nu = self.t / tal::STEG_PER_AR;
            if ar_nu > ar_fore {
                // årsskifte: om rapportår
                if ar_nu == nasta_rapport || ar_nu == ar {
                    rader.push(self.ar_rad(ar_nu));
                    self.rak.fodda_ar = 0;
                    self.rak.dod_svalt_ar = 0;
                    self.rak.dod_alder_ar = 0;
                    if ar_nu == nasta_rapport {
                        nasta_rapport += var_ar;
                    }
                } else {
                    // nollställ årsräknare även mellan rapporter? Nej: ackumulera till nästa rapport.
                }
            }
        }
        // Om sista året inte råkade vara rapportår redan
        if rader.last().map(|r| r.ar) != Some(ar) {
            rader.push(self.ar_rad(ar));
        }
        rader
    }

    fn ar_rad(&self, ar: i64) -> ArRad {
        ArRad {
            ar,
            antal: self.antal(),
            fodda: self.rak.fodda_ar,
            dod_svalt: self.rak.dod_svalt_ar,
            dod_alder: self.rak.dod_alder_ar,
            v_medel: self.v_medel(),
        }
    }

    /// Tillväxt: `vaxer_med` i flockdal.rs rad 121–127, med M = 0 (inga möss)
    /// → fr = 1 − FRO. Tak per ruta i stället för globalt VMAX. Ingen tillväxt om landandel = 0.
    /// `atervaxt` multiplicerar R_V och SOL (1 = oförändrad värld; ledprovet Alt 1224).
    fn vaxer(&mut self) {
        let fas = self.klimat.fas(self.t);
        let g = self.klimat.g(fas);
        let l = self.klimat.l(fas);
        if self.atervaxt_reserv && self.p_steg.len() == self.v.len() {
            for c in 0..self.v.len() {
                let tak = self.vmax[c];
                if tak <= 0.0 {
                    self.v[c] = 0.0;
                    continue;
                }
                let v = self.v[c];
                let d_v = self.p_steg[c] * g * self.atervaxt * (1.0 - v / tak).max(0.0);
                self.v[c] = tak.min(v + d_v);
            }
            return;
        }
        let r = tal::R_V * g * self.atervaxt;
        let sol = tal::SOL * l * self.atervaxt;
        let fr = 1.0 - tal::FRO; // M = 0
        for c in 0..self.v.len() {
            let tak = self.vmax[c];
            if tak <= 0.0 {
                self.v[c] = 0.0;
                continue;
            }
            let v = self.v[c];
            self.v[c] = tak.min(v + fr * r * v * (1.0 - v / tak) + sol * (tak / tal::VMAX));
            // sol skalas med tak/VMAX så att solbidraget följer rutans storlek (absolut biomassa).
        }
    }

    fn ror_sig(&mut self) {
        let n = self.v.len();
        let mut antal = vec![0usize; n];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal[self.cell(d.i, d.j)] += 1;
        }
        // Separat räknare för mätningen — muteras efter flytt; rör inte sinnena.
        let matar = self.matning.pa && self.markering_skedd;
        let loggar = self.ledlogg.pa && self.markering_skedd;
        let signalerar = self.signal_logg.pa && self.markering_skedd;
        let mut antal_nu = if matar { antal.clone() } else { Vec::new() };
        if loggar {
            self.ledlogg.pending.clear();
        }
        let hjkost = hjarna::hjarnkost(tal::KOST_STEG);
        let mut ordn: Vec<usize> = (0..self.djur.len()).filter(|&i| self.djur[i].lev).collect();
        self.g.blanda(&mut ordn);
        for idx in ordn {
            let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
            let s = self.sinnen(i0, j0, self.djur[idx].e, &antal);
            let formaga = self.djur[idx].formaga;
            let intag_fore = self.djur[idx].forra_intag.unwrap_or(0.0);
            let tvingad = match (&self.tvinga, &self.tvinga_grupp) {
                (Some((ti, ta)), _) if *ti == idx => Some(*ta),
                (_, Some((g, ta))) if g.binary_search(&idx).is_ok() => Some(*ta),
                _ => None,
            };
            let a = match tvingad {
                Some(ta) => {
                    // Tvingad handling: hjärnan lär sig av förra steget och förutsäger för `ta`, som i `lar_utforsk`:s slumpgren.
                    let lar = self.djur[idx].formaga.lar_sig();
                    if let Some(cfg) = self.djur[idx].formaga.cfg_2059() {
                        let d = &mut self.djur[idx];
                        if d.hj2059.is_none() {
                            d.hj2059 = Some(Box::new(Hj2008::fran_gen(&d.hj.gen, cfg)));
                        }
                        let hj2 = d.hj2059.as_mut().unwrap();
                        hj2.cfg = cfg;
                        hj2.lar_och_satt(&s, lar, ta);
                    } else {
                        self.djur[idx].hj.lar_och_satt(&s, lar, 0, ta);
                    }
                    ta
                }
                _ => self.valj_handling(idx, &s, &antal),
            };
            // Hjärnvärden: läses efter valet (ingen världsändring; samma wp som vid valet).
            let hj_varden = self.hj_varden_alla(idx, &s);
            let hj_varde = hj_varden[a];
            self.djur[idx].e -= hjkost;
            let (di, dj) = GRANNAR[a];
            let ni = i0 as i32 + di;
            let nj = j0 as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let mut ny_i = i0;
            let mut ny_j = j0;
            if ni >= 0 && nj >= 0 && (ni as usize) < self.b && (nj as usize) < self.h {
                let (ti, tj) = (ni as usize, nj as usize);
                let c = self.cell(ti, tj);
                let vt = self.vatten[c];
                if !(self.hav_stangt && ar_hav(vt)) {
                    let extra = match vt {
                        Vatten::Trask => tal::KOST_TRASK,
                        Vatten::Flodfara => tal::KOST_FLOD,
                        Vatten::Sjo => tal::KOST_SJO,
                        _ => 0.0,
                    };
                    self.djur[idx].e -= extra;
                    self.djur[idx].i = ti;
                    self.djur[idx].j = tj;
                    ny_i = ti;
                    ny_j = tj;
                }
            }
            if matar {
                let gammal = self.cell(i0, j0);
                let ny = self.cell(ny_i, ny_j);
                antal_nu[gammal] -= 1;
                antal_nu[ny] += 1;
                let andra = antal_nu[ny].saturating_sub(1);
                self.matning.lagg_handling(formaga, a);
                self.matning.lagg_andra(formaga, andra);
            }
            if loggar {
                self.ledlogg.djursteg += 1;
                if self.ledlogg.ska_logga() {
                    let mut s_arr = [0.0; tal::S_N];
                    s_arr.copy_from_slice(&s[..tal::S_N]);
                    self.ledlogg.pending.push((
                        idx,
                        LedLoggRad {
                            fro: self.ledlogg.fro,
                            djur_id: self.djur[idx].uid,
                            handling: a,
                            s: s_arr,
                            hj_varde,
                            hj_varden,
                            intag_fore,
                            intag_efter: 0.0,
                        },
                    ));
                }
            }
            if signalerar {
                self.signal_logg.djursteg += 1;
                if self.signal_logg.ska_logga() {
                    let (z, vard, lage, fast) = self.djur[idx].hj.vardets_delar(&s);
                    let mut s_arr = [0.0; tal::S_N];
                    s_arr.copy_from_slice(&s[..tal::S_N]);
                    let mut z_arr = [0.0; tal::Z_N];
                    z_arr.copy_from_slice(&z[..tal::Z_N]);
                    let _ = vard; // samma som hj_varden efter lärning; delar läses ur aktuell hjärna
                    self.signal_logg.rader.push(SignalLoggRad {
                        fro: self.signal_logg.fro,
                        djur_id: self.djur[idx].uid,
                        s: s_arr,
                        z: z_arr,
                        hj_varden,
                        lage,
                        fast,
                    });
                }
            }
        }
        // Medätare efter att alla flyttar är gjorda (Alt 1229; läser bara).
        if matar {
            let mut antal_slut = vec![0usize; n];
            for d in self.djur.iter().filter(|d| d.lev) {
                antal_slut[self.cell(d.i, d.j)] += 1;
            }
            for d in self.djur.iter().filter(|d| d.lev) {
                let andra = antal_slut[self.cell(d.i, d.j)].saturating_sub(1);
                self.matning.lagg_medatare(d.formaga, andra);
            }
        }
    }

    /// Läge a (Alt 1252): djuren väljer och flyttar i lottad ordning; antalet uppdateras;
    /// växterna rörs inte i flyttloopen; alla betar samtidigt efteråt (`betar`).
    fn ror_sig_flytt_i_tur(&mut self) {
        let n = self.v.len();
        let mut antal = vec![0usize; n];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal[self.cell(d.i, d.j)] += 1;
        }
        let matar = self.matning.pa && self.markering_skedd;
        let hjkost = hjarna::hjarnkost(tal::KOST_STEG);
        let mut ordn: Vec<usize> = (0..self.djur.len()).filter(|&i| self.djur[i].lev).collect();
        self.tur_rng.blanda(&mut ordn);
        self.senaste_turordning = ordn.iter().map(|&i| self.djur[i].uid).collect();
        for idx in ordn {
            if !self.djur[idx].lev {
                continue;
            }
            let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
            let s = self.sinnen(i0, j0, self.djur[idx].e, &antal);
            let formaga = self.djur[idx].formaga;
            let a = self.valj_handling(idx, &s, &antal);
            self.djur[idx].e -= hjkost;
            let (di, dj) = GRANNAR[a];
            let ni = i0 as i32 + di;
            let nj = j0 as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let mut ny_i = i0;
            let mut ny_j = j0;
            if ni >= 0 && nj >= 0 && (ni as usize) < self.b && (nj as usize) < self.h {
                let (ti, tj) = (ni as usize, nj as usize);
                let c = self.cell(ti, tj);
                let vt = self.vatten[c];
                if !(self.hav_stangt && ar_hav(vt)) {
                    let extra = match vt {
                        Vatten::Trask => tal::KOST_TRASK,
                        Vatten::Flodfara => tal::KOST_FLOD,
                        Vatten::Sjo => tal::KOST_SJO,
                        _ => 0.0,
                    };
                    self.djur[idx].e -= extra;
                    self.djur[idx].i = ti;
                    self.djur[idx].j = tj;
                    ny_i = ti;
                    ny_j = tj;
                }
            }
            let gammal = self.cell(i0, j0);
            let ny = self.cell(ny_i, ny_j);
            antal[gammal] -= 1;
            antal[ny] += 1;
            if matar {
                let andra = antal[ny].saturating_sub(1);
                self.matning.lagg_handling(formaga, a);
                self.matning.lagg_andra(formaga, andra);
            }
            // Växterna rörs inte här — betet sker i `betar` efteråt.
        }
        if matar {
            let mut antal_slut = vec![0usize; n];
            for d in self.djur.iter().filter(|d| d.lev) {
                antal_slut[self.cell(d.i, d.j)] += 1;
            }
            for d in self.djur.iter().filter(|d| d.lev) {
                let andra = antal_slut[self.cell(d.i, d.j)].saturating_sub(1);
                self.matning.lagg_medatare(d.formaga, andra);
            }
        }
    }

    /// Läge b (Alt 1252): sinnen ur ögonblicksbilden vid stegets början; flytt och bete i tur.
    fn ror_sig_bete_i_tur(&mut self) {
        let n = self.v.len();
        let mut antal_start = vec![0usize; n];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal_start[self.cell(d.i, d.j)] += 1;
        }
        // Hela ögonblicksbilden: växter och antal som vid stegets början.
        let v_start = self.v.clone();
        let matar = self.matning.pa && self.markering_skedd;
        let hjkost = hjarna::hjarnkost(tal::KOST_STEG);
        let mut ordn: Vec<usize> = (0..self.djur.len()).filter(|&i| self.djur[i].lev).collect();
        self.tur_rng.blanda(&mut ordn);
        self.senaste_turordning = ordn.iter().map(|&i| self.djur[i].uid).collect();
        for idx in ordn {
            if !self.djur[idx].lev {
                continue;
            }
            let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
            // Sinnena ur stegets början (v och antal frysta; energi är djurets egen).
            let s = self.sinnen_med_v(i0, j0, self.djur[idx].e, &antal_start, &v_start);
            let formaga = self.djur[idx].formaga;
            let a = self.valj_handling(idx, &s, &antal_start);
            self.djur[idx].e -= hjkost;
            let (di, dj) = GRANNAR[a];
            let ni = i0 as i32 + di;
            let nj = j0 as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let mut ny_i = i0;
            let mut ny_j = j0;
            if ni >= 0 && nj >= 0 && (ni as usize) < self.b && (nj as usize) < self.h {
                let (ti, tj) = (ni as usize, nj as usize);
                let c = self.cell(ti, tj);
                let vt = self.vatten[c];
                if !(self.hav_stangt && ar_hav(vt)) {
                    let extra = match vt {
                        Vatten::Trask => tal::KOST_TRASK,
                        Vatten::Flodfara => tal::KOST_FLOD,
                        Vatten::Sjo => tal::KOST_SJO,
                        _ => 0.0,
                    };
                    self.djur[idx].e -= extra;
                    self.djur[idx].i = ti;
                    self.djur[idx].j = tj;
                    ny_i = ti;
                    ny_j = tj;
                }
            }
            if matar {
                // Antal efter flytt för mätningen (inte för sinnena).
                let mut antal_nu = antal_start.clone();
                // Räkna om från aktuella positioner vore dyrare; använd slutposition.
                let _ = (ny_i, ny_j);
                let andra = {
                    let mut n_andra = 0usize;
                    for (i, d) in self.djur.iter().enumerate() {
                        if d.lev && i != idx && d.i == ny_i && d.j == ny_j {
                            n_andra += 1;
                        }
                    }
                    let _ = &mut antal_nu;
                    n_andra
                };
                self.matning.lagg_handling(formaga, a);
                self.matning.lagg_andra(formaga, andra);
            }
            let _ = self.betar_en(idx);
        }
        if matar {
            let mut antal_slut = vec![0usize; n];
            for d in self.djur.iter().filter(|d| d.lev) {
                antal_slut[self.cell(d.i, d.j)] += 1;
            }
            for d in self.djur.iter().filter(|d| d.lev) {
                let andra = antal_slut[self.cell(d.i, d.j)].saturating_sub(1);
                self.matning.lagg_medatare(d.formaga, andra);
            }
        }
    }

    /// Turordning (Alt 1246): ett djur i taget — sinnen ur aktuellt läge, välj, flytta, beta direkt.
    /// Ordningen lottas ur `tur_rng` (rör inte världens `g`).
    fn ror_sig_tur(&mut self) {
        let n = self.v.len();
        let mut antal = vec![0usize; n];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal[self.cell(d.i, d.j)] += 1;
        }
        let matar = self.matning.pa && self.markering_skedd;
        let loggar = self.ledlogg.pa && self.markering_skedd;
        if loggar {
            self.ledlogg.pending.clear();
        }
        let hjkost = hjarna::hjarnkost(tal::KOST_STEG);
        let mut ordn: Vec<usize> = (0..self.djur.len()).filter(|&i| self.djur[i].lev).collect();
        self.tur_rng.blanda(&mut ordn);
        self.senaste_turordning = ordn.iter().map(|&i| self.djur[i].uid).collect();
        for idx in ordn {
            if !self.djur[idx].lev {
                continue;
            }
            let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
            // Sinnena ur aktuellt läge (antal och växter efter tidigare djurs flyttar/bete).
            let s = self.sinnen(i0, j0, self.djur[idx].e, &antal);
            let formaga = self.djur[idx].formaga;
            let intag_fore = self.djur[idx].forra_intag.unwrap_or(0.0);
            let a = self.valj_handling(idx, &s, &antal);
            let hj_varden = self.hj_varden_alla(idx, &s);
            let hj_varde = hj_varden[a];
            self.djur[idx].e -= hjkost;
            let (di, dj) = GRANNAR[a];
            let ni = i0 as i32 + di;
            let nj = j0 as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let mut ny_i = i0;
            let mut ny_j = j0;
            if ni >= 0 && nj >= 0 && (ni as usize) < self.b && (nj as usize) < self.h {
                let (ti, tj) = (ni as usize, nj as usize);
                let c = self.cell(ti, tj);
                let vt = self.vatten[c];
                if !(self.hav_stangt && ar_hav(vt)) {
                    let extra = match vt {
                        Vatten::Trask => tal::KOST_TRASK,
                        Vatten::Flodfara => tal::KOST_FLOD,
                        Vatten::Sjo => tal::KOST_SJO,
                        _ => 0.0,
                    };
                    self.djur[idx].e -= extra;
                    self.djur[idx].i = ti;
                    self.djur[idx].j = tj;
                    ny_i = ti;
                    ny_j = tj;
                }
            }
            let gammal = self.cell(i0, j0);
            let ny = self.cell(ny_i, ny_j);
            antal[gammal] -= 1;
            antal[ny] += 1;
            if matar {
                let andra = antal[ny].saturating_sub(1);
                self.matning.lagg_handling(formaga, a);
                self.matning.lagg_andra(formaga, andra);
            }
            // Beta direkt (AT_DINO · V / VMAX, högst det som finns kvar).
            let intag = self.betar_en(idx);
            if loggar {
                self.ledlogg.djursteg += 1;
                if self.ledlogg.ska_logga() {
                    let mut s_arr = [0.0; tal::S_N];
                    s_arr.copy_from_slice(&s[..tal::S_N]);
                    if self.ledlogg.rader.len() < self.ledlogg.max_rader {
                        self.ledlogg.rader.push(LedLoggRad {
                            fro: self.ledlogg.fro,
                            djur_id: self.djur[idx].uid,
                            handling: a,
                            s: s_arr,
                            hj_varde,
                            hj_varden,
                            intag_fore,
                            intag_efter: intag,
                        });
                    }
                }
            }
        }
        // Medätare efter att alla flyttar är gjorda (samma mätning som rangprovet).
        if matar {
            let mut antal_slut = vec![0usize; n];
            for d in self.djur.iter().filter(|d| d.lev) {
                antal_slut[self.cell(d.i, d.j)] += 1;
            }
            for d in self.djur.iter().filter(|d| d.lev) {
                let andra = antal_slut[self.cell(d.i, d.j)].saturating_sub(1);
                self.matning.lagg_medatare(d.formaga, andra);
            }
        }
    }

    /// Ett djurs bete i turordning: vill = AT_DINO · V / VMAX, högst kvarvarande V.
    /// Returnerar energiintaget. Uppdaterar tabell/läsare/mätning som `betar`.
    fn betar_en(&mut self, idx: usize) -> f64 {
        if !self.djur[idx].lev {
            return 0.0;
        }
        let c = self.cell(self.djur[idx].i, self.djur[idx].j);
        let vill = self.vill(c);
        let tagen = vill.min(self.v[c]);
        let knapp = vill > self.v[c] + 1e-15;
        self.v[c] -= tagen;
        if self.v[c] < 0.0 {
            self.v[c] = 0.0;
        }
        let intag = tal::E_VAXT * tagen;
        let kost = self.grundkostnad(self.djur[idx].markt);
        self.djur[idx].e += intag - kost;
        let formaga = self.djur[idx].formaga;
        if self.matning.pa && self.markering_skedd {
            self.matning.lagg_intag(formaga, intag);
            if knapp {
                self.matning.lagg_knapp(formaga);
            }
        }
        if self.bokforing.pa && self.markering_skedd {
            self.bokforing.lagg_intag(formaga, intag);
        }
        if formaga.ar_tabell() {
            if let Some(h) = self.djur[idx].tabell.forra_handling {
                self.djur[idx].tabell.uppdatera(h, intag);
            }
        }
        if formaga.har_intagslasare() {
            if let Some(y) = self.djur[idx].lasare.forra_y.take() {
                self.djur[idx].lasare.uppdatera(&y, intag);
            }
        }
        self.djur[idx].forra_intag = Some(intag);
        intag
    }

    /// Hjärnvärde för alla 9 handlingar ur samma sinnen (läser bara; ingen lärning).
    pub fn hj_varden_alla(&self, idx: usize, s: &[f64]) -> [f64; A_N] {
        let z = self.djur[idx].hj.kod(s);
        let mut v = [0.0; A_N];
        for a in 0..A_N {
            let y = self.djur[idx].hj.forutsag(&z, a);
            v[a] = self.djur[idx].hj.varde(&y);
        }
        v
    }

    /// Välj handling efter djurets förmåga. Hjärnan betalas alltid (anroparen).
    /// `antal` = levande per cell vid stegets start (samma ögonblicksbild som sinnena).
    fn valj_handling(&mut self, idx: usize, s: &[f64], antal: &[usize]) -> usize {
        let formaga = self.djur[idx].formaga;
        match formaga {
            Formaga::Lar | Formaga::Rabatt(_) => {
                let a = self.djur[idx].hj.valj(s, true, 0);
                self.ackum_hj_fel(idx, formaga);
                self.notera_utforsk_val(idx, a, true);
                a
            }
            Formaga::Frusen => {
                let a = self.djur[idx].hj.valj(s, false, 0);
                self.ackum_hj_fel(idx, formaga);
                self.notera_utforsk_val(idx, a, true);
                a
            }
            Formaga::LarSlump(p) => {
                if p > 0.0 && self.djur[idx].egen_rng.f() < p {
                    let a = self.djur[idx].egen_rng.under(A_N);
                    self.notera_utforsk_val(idx, a, false);
                    a
                } else {
                    let a = self.djur[idx].hj.valj(s, true, 0);
                    self.ackum_hj_fel(idx, formaga);
                    self.notera_utforsk_val(idx, a, true);
                    a
                }
            }
            Formaga::LarUtforsk | Formaga::LarUtforskFast(_) => {
                let eps = match formaga {
                    Formaga::LarUtforskFast(p) => p,
                    _ => self.djur[idx].eps,
                };
                if eps > 0.0 && self.djur[idx].egen_rng.f() < eps {
                    let a = self.djur[idx].egen_rng.under(A_N);
                    self.djur[idx].hj.lar_och_satt(s, true, 0, a);
                    self.ackum_hj_fel(idx, formaga);
                    self.notera_utforsk_val(idx, a, false);
                    a
                } else {
                    let a = self.djur[idx].hj.valj(s, true, 0);
                    self.ackum_hj_fel(idx, formaga);
                    self.notera_utforsk_val(idx, a, true);
                    a
                }
            }
            Formaga::LarLott => {
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = self.djur[idx].hj.valj_lott(s, true, 0, &mut rng);
                self.djur[idx].egen_rng = rng;
                self.ackum_hj_fel(idx, formaga);
                self.notera_utforsk_val(idx, a, true);
                a
            }
            Formaga::LarIntagslasare => {
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let u = self.djur[idx].lasare.u;
                let u0 = self.djur[idx].lasare.u0;
                let (a, v_a, y) = self.djur[idx].hj.valj_med_poang(s, true, 0, &mut rng, |y| {
                    u0 + (0..tal::Z_N).map(|j| u[j] * y[j]).sum::<f64>()
                });
                self.djur[idx].egen_rng = rng;
                self.djur[idx].lasare.forra_y = Some(y);
                self.ackum_hj_fel(idx, formaga);
                self.notera_utforsk_val(idx, a, true);
                if self.matning.pa && self.markering_skedd {
                    self.matning.lagg_v_jamforelse(formaga, a != v_a);
                }
                a
            }
            Formaga::OrakelForutsagare => {
                let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
                let e = self.djur[idx].e;
                let mut varden = [0.0; A_N];
                for (a, &(di, dj)) in GRANNAR.iter().enumerate() {
                    let ni = i0 as i32 + di;
                    let nj = j0 as i32 + dj;
                    let (ni, nj) = self.kant_granne(ni, nj);
                    if ni < 0 || nj < 0 || ni as usize >= self.b || nj as usize >= self.h {
                        // Utanför kartan: aldrig vald om något innanför finns.
                        varden[a] = f64::NEG_INFINITY;
                        continue;
                    }
                    let (ti, tj) = (ni as usize, nj as usize);
                    // Sinnena i målrutan före betet; rör inte räknare eller världens Rng.
                    let sm = self.sinnen(ti, tj, e, antal);
                    let z = self.djur[idx].hj.kod(&sm);
                    varden[a] = self.djur[idx].hj.varde(&z);
                }
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = lott_bland_max(&varden, &mut rng);
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::Slump => {
                let a = self.djur[idx].egen_rng.under(A_N);
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::Tabell => {
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = self.djur[idx].tabell.valj(&mut rng);
                self.djur[idx].egen_rng = rng;
                self.djur[idx].tabell.forra_handling = Some(a);
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::TabellUtanSlump => {
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = self.djur[idx].tabell.valj_med_p(&mut rng, 0.0);
                self.djur[idx].egen_rng = rng;
                self.djur[idx].tabell.forra_handling = Some(a);
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::Allvetande => {
                let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
                let a = self.allvetande_handling(i0, j0);
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::AllvetandeSlump(p) => {
                let a = if p > 0.0 && self.djur[idx].egen_rng.f() < p {
                    self.djur[idx].egen_rng.under(A_N)
                } else {
                    let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
                    self.allvetande_handling(i0, j0)
                };
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::VaxtPerDjur => {
                let (i0, j0) = (self.djur[idx].i, self.djur[idx].j);
                let a = self.vaxt_per_djur_handling(i0, j0, antal);
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::TrangselOrakel => {
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = Self::trangsel_orakel_handling(s, &mut rng);
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, false);
                a
            }
            Formaga::Lar2059 | Formaga::Frusen2059 | Formaga::Stryk2059 => {
                let lar = formaga.lar_sig();
                let cfg = formaga.cfg_2059().unwrap();
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let a = {
                    let d = &mut self.djur[idx];
                    if d.hj2059.is_none() {
                        d.hj2059 = Some(Box::new(Hj2008::fran_gen(&d.hj.gen, cfg)));
                    }
                    let hj2 = d.hj2059.as_mut().unwrap();
                    hj2.cfg = cfg;
                    hj2.valj_lott(s, lar, &mut rng)
                };
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, true);
                a
            }
            Formaga::Lar2073(eps) => {
                let cfg = formaga.cfg_2059().unwrap();
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let slump = eps > 0.0 && rng.f() < eps;
                let a = {
                    let d = &mut self.djur[idx];
                    if d.hj2059.is_none() {
                        d.hj2059 = Some(Box::new(Hj2008::fran_gen(&d.hj.gen, cfg)));
                    }
                    let hj2 = d.hj2059.as_mut().unwrap();
                    hj2.cfg = cfg;
                    if slump {
                        let a_s = rng.under(A_N);
                        hj2.lar_och_satt(s, true, a_s);
                        a_s
                    } else {
                        hj2.valj_lott(s, true, &mut rng)
                    }
                };
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, !slump);
                a
            }
            Formaga::Lar2116Minst(eps)
            | Formaga::Lar2144D2(eps, _)
            | Formaga::Lar2172Norm(eps, _, _)
            | Formaga::Stryk2144D2(eps, _)
            | Formaga::Lar2144D2SlumpGen(eps, _) => {
                let cfg = formaga.cfg_2059().unwrap();
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let slump = eps > 0.0 && rng.f() < eps;
                let a = {
                    let d = &mut self.djur[idx];
                    if d.hj2059.is_none() {
                        d.hj2059 = Some(Box::new(Hj2008::fran_gen(&d.hj.gen, cfg)));
                    }
                    let hj2 = d.hj2059.as_mut().unwrap();
                    hj2.cfg = cfg;
                    if slump {
                        let a_m = hj2.minst_besokta_handling(&mut rng);
                        hj2.lar_och_satt(s, true, a_m);
                        a_m
                    } else {
                        hj2.valj_lott(s, true, &mut rng)
                    }
                };
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, !slump);
                a
            }
            Formaga::Lar2199Osak(param, _, osak_lage) => {
                let cfg = formaga.cfg_2059().unwrap();
                let mut rng = std::mem::replace(&mut self.djur[idx].egen_rng, Rng::new(0));
                let (a, var_girig) = {
                    let d = &mut self.djur[idx];
                    if d.hj2059.is_none() {
                        d.hj2059 = Some(Box::new(Hj2008::fran_gen(&d.hj.gen, cfg)));
                    }
                    let hj2 = d.hj2059.as_mut().unwrap();
                    hj2.cfg = cfg;
                    hj2.valj_med_osakerhet(s, true, &mut rng, osak_lage, param)
                };
                self.djur[idx].egen_rng = rng;
                self.notera_utforsk_val(idx, a, var_girig);
                a
            }
            Formaga::StannaSlump(p) => {
                let a = if p > 0.0 && self.djur[idx].egen_rng.f() < p {
                    0
                } else {
                    1 + self.djur[idx].egen_rng.under(A_N - 1)
                };
                self.notera_utforsk_val(idx, a, false);
                a
            }
        }
    }

    /// Ur sinnena: `v_andel / (n + 1)` per målruta; lika lottas (Alt 1230).
    /// `n` = sinnet × 4 (antal djur vid stegets början). Betalar samma hjärnpris.
    pub fn trangsel_orakel_handling(s: &[f64], g: &mut Rng) -> usize {
        assert!(s.len() >= 18);
        let mut varden = [0.0; A_N];
        for a in 0..A_N {
            let v_andel = s[a * 2];
            let n = s[a * 2 + 1] * 4.0;
            varden[a] = v_andel / (n + 1.0);
        }
        lott_bland_max(&varden, g)
    }

    /// Bland de 9 rutorna: gå till den med mest växter; vid lika maximum → stå kvar (0).
    pub fn allvetande_handling(&self, i: usize, j: usize) -> usize {
        let mut max_v = f64::NEG_INFINITY;
        let mut n_max = 0usize;
        let mut ba = 0usize;
        for (a, &(di, dj)) in GRANNAR.iter().enumerate() {
            let ni = i as i32 + di;
            let nj = j as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let vv = if ni < 0 || nj < 0 || ni as usize >= self.b || nj as usize >= self.h {
                0.0
            } else {
                self.v[self.cell(ni as usize, nj as usize)]
            };
            if vv > max_v {
                max_v = vv;
                n_max = 1;
                ba = a;
            } else if (vv - max_v).abs() < 1e-15 {
                n_max += 1;
            }
        }
        if n_max > 1 {
            0
        } else {
            ba
        }
    }

    /// Bland de 9: mest växter / (andra djur + 1); vid lika maximum → stå kvar (0).
    /// `antal` = levande per cell; djuret som väljer räknas bort i egen ruta.
    pub fn vaxt_per_djur_handling(&self, i: usize, j: usize, antal: &[usize]) -> usize {
        let egen = self.cell(i, j);
        let mut max_s = f64::NEG_INFINITY;
        let mut n_max = 0usize;
        let mut ba = 0usize;
        for (a, &(di, dj)) in GRANNAR.iter().enumerate() {
            let ni = i as i32 + di;
            let nj = j as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let (vv, andra) =
                if ni < 0 || nj < 0 || ni as usize >= self.b || nj as usize >= self.h {
                    (0.0, 0usize)
                } else {
                    let c = self.cell(ni as usize, nj as usize);
                    let andra = if c == egen {
                        antal[c].saturating_sub(1)
                    } else {
                        antal[c]
                    };
                    (self.v[c], andra)
                };
            let score = vv / (andra as f64 + 1.0);
            if score > max_s {
                max_s = score;
                n_max = 1;
                ba = a;
            } else if (score - max_s).abs() < 1e-15 {
                n_max += 1;
            }
        }
        if n_max > 1 {
            0
        } else {
            ba
        }
    }

    /// Sinnena som djuret på platsen `idx` har i början av ett steg (läser bara; samma räkning som i `ror_sig`).
    pub fn sinnen_for(&self, idx: usize) -> Vec<f64> {
        let mut antal = vec![0usize; self.v.len()];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal[self.cell(d.i, d.j)] += 1;
        }
        let d = &self.djur[idx];
        self.sinnen(d.i, d.j, d.e, &antal)
    }

    /// Sinnena som djuret får i NÄSTA steg: växterna växer först, som i `steg`. Ändrar världen (växterna); bara för en kopia som kastas.
    pub fn sinnen_nasta_steg(&mut self, idx: usize) -> Vec<f64> {
        self.vaxer();
        self.sinnen_for(idx)
    }

    fn sinnen(&self, i: usize, j: usize, e: f64, antal: &[usize]) -> Vec<f64> {
        self.sinnen_med_v(i, j, e, antal, &self.v)
    }

    /// Sinnen med given växtvektor (läge b fryser `v` vid stegets början).
    fn sinnen_med_v(&self, i: usize, j: usize, e: f64, antal: &[usize], v: &[f64]) -> Vec<f64> {
        let mut s = vec![0.0; tal::S_N];
        for (k, &(di, dj)) in GRANNAR.iter().enumerate() {
            let ni = i as i32 + di;
            let nj = j as i32 + dj;
            let (ni, nj) = self.kant_granne(ni, nj);
            let (v_norm, n_norm) = if ni < 0 || nj < 0 || ni as usize >= self.b || nj as usize >= self.h
            {
                (0.0, 0.0)
            } else {
                let c = self.cell(ni as usize, nj as usize);
                let tak = self.vmax[c].max(1e-12);
                (v[c] / tak, antal[c] as f64 / 4.0)
            };
            s[k * 2] = v_norm;
            s[k * 2 + 1] = n_norm;
        }
        s[18] = (e / (tal::E_HANE * self.e_skala)).clamp(0.0, 2.0);
        s
    }

    /// Bete som `betas` + `bete5`: alla i rutan vill äta AT_DINO · V / VMAX; räcker det inte får alla samma andel.
    /// Sedan `e += E_VAXT * at − KOST_STEG`.
    fn betar(&mut self) {
        let n = self.v.len();
        let mut antal = vec![0usize; n];
        for d in self.djur.iter().filter(|d| d.lev) {
            antal[self.cell(d.i, d.j)] += 1;
        }
        // flockdal betas: vill = AT_DINO · V / VMAX (absolut). Med skalad ruta (tak = sf · VMAX) ska
        // vill_cell = sf · vill_plätt = AT_DINO · V / VMAX (samma formel; mer mat i större rutor vid samma fyllnad).
        let mut at = vec![0.0; n];
        let mut knapp = vec![false; n];
        for c in 0..n {
            let vill = self.vill(c);
            let tot = antal[c] as f64 * vill;
            let skala = if tot > self.v[c] {
                knapp[c] = true;
                self.v[c] / tot.max(1e-12)
            } else {
                1.0
            };
            at[c] = vill * skala;
            self.v[c] -= tot * skala;
            if self.v[c] < 0.0 {
                self.v[c] = 0.0;
            }
        }
        let loggar = self.ledlogg.pa && self.markering_skedd;
        for i in 0..self.djur.len() {
            if !self.djur[i].lev {
                continue;
            }
            let c = self.djur[i].j * self.b + self.djur[i].i;
            let intag = tal::E_VAXT * at[c];
            let kost = self.grundkostnad(self.djur[i].markt);
            self.djur[i].e += intag - kost;
            let formaga = self.djur[i].formaga;
            if self.matning.pa && self.markering_skedd {
                self.matning.lagg_intag(formaga, intag);
                if knapp[c] {
                    self.matning.lagg_knapp(formaga);
                }
            }
            if self.bokforing.pa && self.markering_skedd {
                self.bokforing.lagg_intag(formaga, intag);
            }
            if formaga.ar_tabell() {
                if let Some(h) = self.djur[i].tabell.forra_handling {
                    self.djur[i].tabell.uppdatera(h, intag);
                }
            }
            if formaga.har_intagslasare() {
                if let Some(y) = self.djur[i].lasare.forra_y.take() {
                    self.djur[i].lasare.uppdatera(&y, intag);
                }
            }
            // Läs-bara fält för ledloggen; påverkar inte världen.
            self.djur[i].forra_intag = Some(intag);
        }
        if loggar {
            let pending = std::mem::take(&mut self.ledlogg.pending);
            for (idx, mut rad) in pending {
                if idx < self.djur.len() && self.djur[idx].lev {
                    rad.intag_efter = self.djur[idx].forra_intag.unwrap_or(0.0);
                }
                if self.ledlogg.rader.len() < self.ledlogg.max_rader {
                    self.ledlogg.rader.push(rad);
                }
            }
        }
    }

    fn alder_fodsel_dod(&mut self) {
        // ålder++
        for d in self.djur.iter_mut().filter(|d| d.lev) {
            d.age += 1;
        }
        // hitta fäderkandidater per cell (vuxna hanar)
        let n = self.v.len();
        let mut hanar_i_cell: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (idx, d) in self.djur.iter().enumerate() {
            if d.lev && !d.hona && d.age >= self.mogen_steg.unwrap_or(tal::LITEN) {
                hanar_i_cell[self.cell(d.i, d.j)].push(idx);
            }
        }
        // honor redo att föda
        let mut par: Vec<(usize, usize)> = Vec::new(); // (mor, far)
        let ix: Vec<usize> = (0..self.djur.len()).filter(|&i| self.djur[i].lev).collect();
        for &i in &ix {
            let d = &self.djur[i];
            if !d.hona || d.e < self.fodsel_troskel.unwrap_or(tal::E_DELA * self.e_skala) {
                continue;
            }
            if self.hona_vuxen && d.age < self.mogen_steg.unwrap_or(tal::LITEN) {
                continue;
            }
            if self.fodsel_arlig && self.t % tal::STEG_PER_AR != FODSEL_STEG {
                continue;
            }
            // fadern bland vuxna hanar i egen ruta och 8 grannar (fortplantning8 / didel)
            let mut kand = Vec::new();
            for &(di, dj) in &GRANNAR {
                let ni = d.i as i32 + di;
                let nj = d.j as i32 + dj;
                let (ni, nj) = self.kant_granne(ni, nj);
                if ni < 0 || nj < 0 || ni as usize >= self.b || nj as usize >= self.h {
                    continue;
                }
                for &h_idx in &hanar_i_cell[self.cell(ni as usize, nj as usize)] {
                    if !self.parning_inom_linje || self.djur[h_idx].markt == d.markt {
                        kand.push(h_idx);
                    }
                }
            }
            if kand.is_empty() {
                continue;
            }
            let far = kand[self.g.under(kand.len())];
            par.push((i, far));
        }
        for &(mor, _) in &par {
            self.djur[mor].e -= tal::E_DELA * self.e_skala / 2.0;
        }
        // död
        for &i in &ix {
            if !self.djur[i].lev {
                continue;
            }
            if self.djur[i].e <= 0.0 {
                let f = self.djur[i].formaga;
                if self.matning.pa && self.markering_skedd {
                    let e = self.djur[i].e;
                    self.matning.lagg_dod(f, e, true);
                }
                if self.bokforing.pa && self.markering_skedd {
                    self.bokforing.lagg_dod(f, true);
                }
                self.bokfor_aldrig_djur(i);
                self.djur[i].lev = false;
                self.djur[i].hj2059 = None;
                self.rak.dod_svalt += 1;
                self.rak.dod_svalt_ar += 1;
            } else if self.djur[i].age >= self.liv_steg.unwrap_or(tal::LIV) {
                let f = self.djur[i].formaga;
                if self.matning.pa && self.markering_skedd {
                    let e = self.djur[i].e;
                    self.matning.lagg_dod(f, e, false);
                }
                if self.bokforing.pa && self.markering_skedd {
                    self.bokforing.lagg_dod(f, false);
                }
                self.bokfor_aldrig_djur(i);
                self.djur[i].lev = false;
                self.djur[i].hj2059 = None;
                self.rak.dod_alder += 1;
                self.rak.dod_alder_ar += 1;
            }
        }
        // födslar
        self.g.blanda(&mut par);
        for (mor, far) in par {
            if !self.djur[mor].lev {
                continue; // modern dog
            }
            // faderns gener: om fadern dog, hoppa över? fortplantning8 använder fortfarande far-index före död.
            // Vi parar före död-check på far; far kan ha dött. Kräv levande far.
            if !self.djur[far].lev {
                // modern har redan betalat — som utan plats; ingen unge
                continue;
            }
            let hona = self.g.f() < 0.5;
            self.parningar_totalt += 1;
            if self.djur[far].markt != self.djur[mor].markt {
                self.parningar_kors += 1;
            }
            let mut hj = Hj::arv(&self.djur[mor].hj.gen, &self.djur[far].hj.gen, &mut self.g);
            let markt = self.djur[mor].markt;
            let formaga = self.formaga_for(markt);
            let (i, j) = (self.djur[mor].i, self.djur[mor].j);
            // återanvänd död plats eller push
            let plats = self.djur.iter().position(|d| !d.lev);
            let uid = self.nasta_uid;
            if matches!(formaga, Formaga::Lar2144D2SlumpGen(_, _)) {
                let mut g_unge = Rng::new(uid ^ 0x2223_51A4_0001);
                hj = Hj::slump(&mut g_unge);
            }
            let eps = arv_eps(self.djur[mor].eps, self.djur[far].eps, uid);
            let unge = Dino {
                lev: true,
                i,
                j,
                e: tal::E_START * self.e_skala,
                age: 0,
                hona,
                hj,
                uid,
                markt,
                formaga,
                tabell: Minnestabell::ny(),
                lasare: Intagslasare::ny(),
                egen_rng: Rng::new(uid),
                forra_intag: None,
                eps,
                hj_handlingar: [0; A_N],
                steg_efter_mark: 0,
                hj2059: None,
            };
            self.nasta_uid += 1;
            let formaga_unge = unge.formaga;
            match plats {
                Some(p) => self.djur[p] = unge,
                None => self.djur.push(unge),
            }
            self.rak.fodda += 1;
            self.rak.fodda_ar += 1;
            if self.bokforing.pa && self.markering_skedd {
                self.bokforing.lagg_fodsel(formaga_unge);
            }
        }
        // energitak
        let tak = self.e_tak.unwrap_or(tal::E_HANE * self.e_skala);
        for d in self.djur.iter_mut().filter(|d| d.lev) {
            if d.e > tak {
                d.e = tak;
            }
        }
    }
}

/// Fårans bredd i en flodruta (Alt 1624, 1625; underlaget `n10/docs/2026-11-22-lasunderlag-flodkant1624.md`): √(100 · 320) m, medianen i
/// logaritmen av hckartas band 100–320 m (Butler 1980, Hasler och Renne 2025; bandets form och medianen är eget val).
pub fn fara_bredd_m() -> f64 {
    (hckarta::FARA_BREDD_MIN_M * hckarta::FARA_BREDD_MAX_M).sqrt()
}

/// Landandel per markslag. Flodfåra: rutan är flodkant med en fåra rakt genom (eget val), så landet är 1 − fårans bredd / rutans sida
/// (Alt 1624: Lyson och Longrich 2011, "hadrosaurs ... preferred habitats in close proximity to rivers"). Sjö, hav och bräckt vatten: 0
/// (ingen läst källa om sjöar).
pub fn landandel_av(vt: Vatten, ruta_m: f64) -> f64 {
    match vt {
        Vatten::Land | Vatten::Trask => 1.0,
        Vatten::Flodfara => (1.0 - fara_bredd_m() / ruta_m).clamp(0.0, 1.0),
        Vatten::Hav | Vatten::Sjo | Vatten::Brackt => 0.0,
    }
}

pub fn ar_hav(vt: Vatten) -> bool {
    matches!(vt, Vatten::Hav | Vatten::Brackt)
}

/// Sammanfattning på en JSON-rad.
pub fn sammanfattning_json(
    fro: u64,
    ar: i64,
    n0: usize,
    rader: &[ArRad],
    slut_antal: usize,
    djur_summa: u64,
    karta_summa: u64,
) -> String {
    let ar_json: Vec<String> = rader
        .iter()
        .map(|r| {
            format!(
                "{{\"ar\":{},\"antal\":{},\"fodda\":{},\"dod_svalt\":{},\"dod_alder\":{},\"v_medel\":{}}}",
                r.ar, r.antal, r.fodda, r.dod_svalt, r.dod_alder, fmt_f64(r.v_medel)
            )
        })
        .collect();
    format!(
        "{{\"fro\":{},\"ar\":{},\"n0\":{},\"slut_antal\":{},\"djur_summa\":{},\"karta_summa\":{},\"ar_rader\":[{}]}}",
        fro,
        ar,
        n0,
        slut_antal,
        djur_summa,
        karta_summa,
        ar_json.join(",")
    )
}

fn fmt_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{}", x)
    } else {
        "null".into()
    }
}
