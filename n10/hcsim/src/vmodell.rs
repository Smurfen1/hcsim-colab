//! Fas 1 (Håkan 2026-11-23): prövar om Edmontosaurus hjärna är en handlingsvillkorad världsmodell.
//! Mått och gränser: piharness `docs/evolution/vmodell-fas1-utfallsregel.md` (skrivna före körningen).
//! Ändrar ingenting i hjärnan eller världen: varje handling prövas på en KOPIA av världen (`Sim.tvinga`).
//! Rust (AGENTS.md regel 11).

use crate::sim::Sim;
use crate::tal::{A_N, STEG_PER_AR, Z_N};
use crate::Rng;

/// Gränser ur utfallsregeln, skrivna som tal.
pub const PAR_VERKLIG_MIN: f64 = 1e-9;
pub const PAR_IDENTISK: f64 = 1e-18;
pub const PAR_ANDEL_KVADRAT: f64 = 0.01;
/// Erfarenhetsgrupperna (Håkan 2026-11-23): 0, 1–9, 10–49, 50 eller fler träningar före provet.
pub const GRUPPER: usize = 4;
pub const GRUPP_NAMN: [&str; GRUPPER] = ["0", "1–9", "10–49", "50+"];
pub fn grupp(n: u32) -> usize {
    match n {
        0 => 0,
        1..=9 => 1,
        10..=49 => 2,
        _ => 3,
    }
}

/// Utfallet av en handling för ett djur.
#[derive(Clone, Debug)]
pub struct Utfall {
    /// Hjärnans förutsägelse för handlingen (ur hjärnans egen kod i steget).
    pub y: [f64; Z_N],
    /// Den inre bild djuret verkligen fick i nästa steg.
    pub zn: [f64; Z_N],
    pub v_pred: f64,
    pub v_sann: f64,
    pub intag: f64,
    pub flyttade: bool,
}

/// Ett prov: ett djur vid ett tillfälle, alla nio handlingar från samma läge.
#[derive(Clone, Debug)]
pub struct Prov {
    pub uid: u64,
    pub z: [f64; Z_N],
    pub utfall: Vec<Utfall>,
    /// Handlingen hjärnan själv hade valt (högsta värde; lika → den första, som i `Hj::valj`).
    pub hjarnans_val: usize,
    /// K0: z var bit för bit lika i de nio kopiorna och i den rena räkningen.
    pub samma_z: bool,
    /// Största skillnaden mellan den rena förutsägelsen på originalet (före stegets inlärning) och hjärnans egen i steget.
    pub ren_diff: f64,
    /// Antal gånger djuret självt har tränat förutsägaren för varje handling före provet (None = inte räknat).
    pub erfarenhet: Option<[u32; A_N]>,
}

fn arr(v: &[f64]) -> [f64; Z_N] {
    let mut a = [0.0; Z_N];
    a.copy_from_slice(&v[..Z_N]);
    a
}

pub fn kvad(a: &[f64; Z_N], b: &[f64; Z_N]) -> f64 {
    (0..Z_N).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

/// Varför ett prov inte gick att räkna.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bortfall {
    DogIKopia,
}

/// Prövar djuret på platsen `idx`: nio kopior av `bas`, en per handling. `bas` läses bara.
pub fn prova_djur(bas: &Sim, idx: usize) -> Result<Prov, Bortfall> {
    assert!(!bas.turordning && !bas.flytt_i_tur && !bas.bete_i_tur, "kroken finns bara i ror_sig");
    assert!(bas.tvinga.is_none());
    let d0 = &bas.djur[idx];
    assert!(d0.lev);
    let (uid, i0, j0) = (d0.uid, d0.i, d0.j);
    // Ren räkning: sinnena som djuret får i steget (efter växternas tillväxt), kodade med djurets kodare.
    let z_ren = {
        let mut k = bas.clone();
        let s = k.sinnen_nasta_steg(idx);
        arr(&bas.djur[idx].hj.kod(&s))
    };
    let mut utfall = Vec::with_capacity(A_N);
    let mut samma_z = true;
    let mut ren_diff: f64 = 0.0;
    for a in 0..A_N {
        let mut k = bas.clone();
        k.tvinga = Some((idx, a));
        k.steg();
        k.tvinga = None;
        if !k.djur[idx].lev || k.djur[idx].uid != uid {
            return Err(Bortfall::DogIKopia);
        }
        let (z, a2, y) = k.djur[idx].hj.forra.clone().expect("hjärnan har förutsagt i steget");
        assert_eq!(a2, a, "kroken gav en annan handling");
        let (z, y) = (arr(&z), arr(&y));
        if z.iter().zip(z_ren.iter()).any(|(p, q)| p.to_bits() != q.to_bits()) {
            samma_z = false;
        }
        let y_ren = arr(&bas.djur[idx].hj.forutsag(&z_ren, a));
        for i in 0..Z_N {
            ren_diff = ren_diff.max((y_ren[i] - y[i]).abs());
        }
        let intag = k.djur[idx].forra_intag.unwrap_or(0.0);
        let flyttade = (k.djur[idx].i, k.djur[idx].j) != (i0, j0);
        let s2 = k.sinnen_nasta_steg(idx);
        let hj = &k.djur[idx].hj;
        let zn = arr(&hj.kod(&s2));
        utfall.push(Utfall { y, zn, v_pred: hj.varde(&y), v_sann: hj.varde(&zn), intag, flyttade });
    }
    let mut hjarnans_val = 0;
    for a in 1..A_N {
        if utfall[a].v_pred > utfall[hjarnans_val].v_pred {
            hjarnans_val = a;
        }
    }
    Ok(Prov { uid, z: z_ren, utfall, hjarnans_val, samma_z, ren_diff, erfarenhet: None })
}

/// K5: två kopior med samma handling ger bit för bit samma utfall och samma kontrollsumma.
pub fn samma_handling_samma_svar(bas: &Sim, idx: usize, a: usize) -> bool {
    let en = |()| {
        let mut k = bas.clone();
        k.tvinga = Some((idx, a));
        k.steg();
        k.tvinga = None;
        let summa = k.djur_summa();
        let s2 = k.sinnen_nasta_steg(idx);
        let zn = k.djur[idx].hj.kod(&s2);
        (summa, zn.iter().map(|x| x.to_bits()).collect::<Vec<u64>>())
    };
    en(()) == en(())
}

/// Djur som får prövas: levande, minst ett år, och inte så gamla att de dör av ålder i steget.
pub fn valj_djur(sim: &Sim, antal: usize, fro: u64) -> Vec<usize> {
    let liv = sim.liv_steg.unwrap_or(crate::tal::LIV);
    let mut kand: Vec<usize> = (0..sim.djur.len())
        .filter(|&i| {
            let d = &sim.djur[i];
            d.lev && d.age >= STEG_PER_AR && d.age + 2 < liv && d.hj.forra.is_some()
        })
        .collect();
    let mut g = Rng::new(fro ^ 0x76D0_DE11);
    g.blanda(&mut kand);
    kand.truncate(antal);
    kand
}

/// Summor för måtten K1–K3 och redovisningen. Läggs ihop per frö eller över alla frön.
#[derive(Clone, Debug, Default)]
pub struct Matt {
    pub prov: u64,
    pub bortfall: u64,
    pub ej_samma_z: u64,
    pub ren_diff_max: f64,
    /// K2: per handling.
    pub fel_pred: [f64; A_N],
    pub fel_kop: [f64; A_N],
    /// Samma, bara när hjärnan själv valde handlingen / inte valde den.
    pub fel_pred_vald: [f64; A_N],
    pub fel_kop_vald: [f64; A_N],
    pub fel_pred_ovald: [f64; A_N],
    pub fel_kop_ovald: [f64; A_N],
    pub val: [u64; A_N],
    /// K3.
    pub ss_verklig: f64,
    pub ss_rest: f64,
    pub ss_pred: f64,
    pub punkt: f64,
    /// K1: par.
    pub par_med_skillnad: u64,
    pub par_sarskiljda: u64,
    pub par_identiska: u64,
    pub par_pred: [[f64; A_N]; A_N],
    pub par_verklig: [[f64; A_N]; A_N],
    pub par_ident: [[u64; A_N]; A_N],
    /// Valet.
    pub topp_lika: u64,
    pub intag_vald: f64,
    pub intag_medel: f64,
    pub intag_bast: f64,
    pub flyttade: [u64; A_N],
    /// Per handling och erfarenhetsgrupp (0, 1–9, 10–49, 50 eller fler träningar före provet): antal, fel, persistensens fel,
    /// och handlingsdelens rest och verkliga storlek (R² per grupp = 1 − rest / verklig).
    pub grp_n: [[u64; GRUPPER]; A_N],
    pub grp_fel_pred: [[f64; GRUPPER]; A_N],
    pub grp_fel_kop: [[f64; GRUPPER]; A_N],
    pub grp_rest: [[f64; GRUPPER]; A_N],
    pub grp_verklig: [[f64; GRUPPER]; A_N],
    /// Träningsfördelningen: summan av djurens träningar per handling, antal djur, summan av antalet aldrig tränade förutsägare
    /// och summan av den vanligaste handlingens andel.
    pub erf_sum: [u64; A_N],
    pub erf_djur: u64,
    pub erf_aldrig: u64,
    pub erf_topp_andel: f64,
}

impl Matt {
    pub fn lagg(&mut self, p: &Prov) {
        self.prov += 1;
        if !p.samma_z {
            self.ej_samma_z += 1;
        }
        self.ren_diff_max = self.ren_diff_max.max(p.ren_diff);
        let u = &p.utfall;
        let mut ym = [0.0; Z_N];
        let mut zm = [0.0; Z_N];
        for a in 0..A_N {
            for i in 0..Z_N {
                ym[i] += u[a].y[i] / A_N as f64;
                zm[i] += u[a].zn[i] / A_N as f64;
            }
        }
        self.val[p.hjarnans_val] += 1;
        let mut topp_sann = 0;
        let mut bast = f64::NEG_INFINITY;
        for a in 0..A_N {
            let fp = kvad(&u[a].y, &u[a].zn);
            let fk = kvad(&p.z, &u[a].zn);
            self.fel_pred[a] += fp;
            self.fel_kop[a] += fk;
            if a == p.hjarnans_val {
                self.fel_pred_vald[a] += fp;
                self.fel_kop_vald[a] += fk;
            } else {
                self.fel_pred_ovald[a] += fp;
                self.fel_kop_ovald[a] += fk;
            }
            for i in 0..Z_N {
                let pd = u[a].y[i] - ym[i];
                let vd = u[a].zn[i] - zm[i];
                self.ss_verklig += vd * vd;
                self.ss_pred += pd * pd;
                self.ss_rest += (pd - vd) * (pd - vd);
                self.punkt += pd * vd;
            }
            if let Some(e) = &p.erfarenhet {
                let g = grupp(e[a]);
                self.grp_n[a][g] += 1;
                self.grp_fel_pred[a][g] += fp;
                self.grp_fel_kop[a][g] += fk;
                for i in 0..Z_N {
                    let pd = u[a].y[i] - ym[i];
                    let vd = u[a].zn[i] - zm[i];
                    self.grp_rest[a][g] += (pd - vd) * (pd - vd);
                    self.grp_verklig[a][g] += vd * vd;
                }
            }
            if u[a].v_sann > u[topp_sann].v_sann {
                topp_sann = a;
            }
            self.intag_medel += u[a].intag / A_N as f64;
            bast = bast.max(u[a].intag);
            if u[a].flyttade {
                self.flyttade[a] += 1;
            }
        }
        if let Some(e) = &p.erfarenhet {
            let tot: u32 = e.iter().sum();
            self.erf_djur += 1;
            for a in 0..A_N {
                self.erf_sum[a] += e[a] as u64;
            }
            self.erf_aldrig += e.iter().filter(|&&x| x == 0).count() as u64;
            self.erf_topp_andel += *e.iter().max().unwrap() as f64 / tot.max(1) as f64;
        }
        self.intag_vald += u[p.hjarnans_val].intag;
        self.intag_bast += bast;
        if topp_sann == p.hjarnans_val {
            self.topp_lika += 1;
        }
        for a in 0..A_N {
            for b in a + 1..A_N {
                let dp = kvad(&u[a].y, &u[b].y);
                let dv = kvad(&u[a].zn, &u[b].zn);
                self.par_pred[a][b] += dp;
                self.par_verklig[a][b] += dv;
                if dp < PAR_IDENTISK {
                    self.par_identiska += 1;
                    self.par_ident[a][b] += 1;
                }
                if dv > PAR_VERKLIG_MIN {
                    self.par_med_skillnad += 1;
                    if dp >= PAR_ANDEL_KVADRAT * dv {
                        self.par_sarskiljda += 1;
                    }
                }
            }
        }
    }

    pub fn sla_ihop(&mut self, o: &Matt) {
        self.prov += o.prov;
        self.bortfall += o.bortfall;
        self.ej_samma_z += o.ej_samma_z;
        self.ren_diff_max = self.ren_diff_max.max(o.ren_diff_max);
        for a in 0..A_N {
            self.fel_pred[a] += o.fel_pred[a];
            self.fel_kop[a] += o.fel_kop[a];
            self.fel_pred_vald[a] += o.fel_pred_vald[a];
            self.fel_kop_vald[a] += o.fel_kop_vald[a];
            self.fel_pred_ovald[a] += o.fel_pred_ovald[a];
            self.fel_kop_ovald[a] += o.fel_kop_ovald[a];
            self.val[a] += o.val[a];
            self.flyttade[a] += o.flyttade[a];
            self.erf_sum[a] += o.erf_sum[a];
            for g in 0..GRUPPER {
                self.grp_n[a][g] += o.grp_n[a][g];
                self.grp_fel_pred[a][g] += o.grp_fel_pred[a][g];
                self.grp_fel_kop[a][g] += o.grp_fel_kop[a][g];
                self.grp_rest[a][g] += o.grp_rest[a][g];
                self.grp_verklig[a][g] += o.grp_verklig[a][g];
            }
            for b in 0..A_N {
                self.par_pred[a][b] += o.par_pred[a][b];
                self.par_verklig[a][b] += o.par_verklig[a][b];
                self.par_ident[a][b] += o.par_ident[a][b];
            }
        }
        self.ss_verklig += o.ss_verklig;
        self.ss_rest += o.ss_rest;
        self.ss_pred += o.ss_pred;
        self.punkt += o.punkt;
        self.par_med_skillnad += o.par_med_skillnad;
        self.par_sarskiljda += o.par_sarskiljda;
        self.par_identiska += o.par_identiska;
        self.topp_lika += o.topp_lika;
        self.intag_vald += o.intag_vald;
        self.intag_medel += o.intag_medel;
        self.intag_bast += o.intag_bast;
        self.erf_djur += o.erf_djur;
        self.erf_aldrig += o.erf_aldrig;
        self.erf_topp_andel += o.erf_topp_andel;
    }

    /// K1: andelen par med verklig skillnad där förutsägelserna är särskiljda.
    pub fn k1(&self) -> f64 {
        self.par_sarskiljda as f64 / (self.par_med_skillnad.max(1)) as f64
    }
    /// K2: kvoten förutsägelsens fel / persistensens fel för handling `a`.
    pub fn k2(&self, a: usize) -> f64 {
        self.fel_pred[a] / self.fel_kop[a].max(1e-300)
    }
    /// K3: R² för den del som beror på handlingen.
    pub fn k3(&self) -> f64 {
        1.0 - self.ss_rest / self.ss_verklig.max(1e-300)
    }
    pub fn cosinus(&self) -> f64 {
        self.punkt / (self.ss_pred * self.ss_verklig).sqrt().max(1e-300)
    }
    pub fn storlek(&self) -> f64 {
        (self.ss_pred / self.ss_verklig.max(1e-300)).sqrt()
    }
    /// Andelen av den verkliga förändringen (mot nuläget) som beror på handlingen.
    pub fn handlingens_andel(&self) -> f64 {
        self.ss_verklig / self.fel_kop.iter().sum::<f64>().max(1e-300)
    }
}

/// Domen för ett mått över frön enligt utfallsregeln.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dom {
    Pass,
    Partial,
    Fail,
}

impl Dom {
    pub fn text(self) -> &'static str {
        match self {
            Dom::Pass => "PASS",
            Dom::Partial => "PARTIAL",
            Dom::Fail => "FAIL",
        }
    }
}

pub fn median(mut v: Vec<f64>) -> f64 {
    assert!(!v.is_empty());
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// Minsta antal frön för grönt: 15 av 20 (skalas för färre frön i torrkörning: tre fjärdedelar, avrundat uppåt).
pub fn krav_fron(n: usize) -> usize {
    (3 * n + 3) / 4
}

pub fn dom_k1(per_fro: &[Matt]) -> Dom {
    let v: Vec<f64> = per_fro.iter().map(|m| m.k1()).collect();
    if v.iter().filter(|&&x| x >= 0.90).count() >= krav_fron(v.len()) {
        Dom::Pass
    } else if median(v) < 0.50 {
        Dom::Fail
    } else {
        Dom::Partial
    }
}

pub fn dom_k2(per_fro: &[Matt]) -> Dom {
    let alla_under = per_fro.iter().filter(|m| (0..A_N).all(|a| m.k2(a) < 1.0)).count();
    if alla_under >= krav_fron(per_fro.len()) {
        return Dom::Pass;
    }
    let over = (0..A_N).filter(|&a| median(per_fro.iter().map(|m| m.k2(a)).collect()) >= 1.0).count();
    if over >= 5 {
        Dom::Fail
    } else {
        Dom::Partial
    }
}

pub fn dom_k3(per_fro: &[Matt]) -> Dom {
    let v: Vec<f64> = per_fro.iter().map(|m| m.k3()).collect();
    let over = v.iter().filter(|&&x| x > 0.0).count();
    let n = v.len();
    if over >= krav_fron(n) && median(v) >= 0.10 {
        Dom::Pass
    } else if n - over >= krav_fron(n) {
        Dom::Fail
    } else {
        Dom::Partial
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Skuggmodellerna (Håkan 2026-11-23: jämför nio skilda förutsägare, en gemensam handlingsvillkorad förutsägare och persistensen, på
// samma världar, djur, erfarenheter och provtillfällen; den gemensamma får inte tränas på kontrafaktiska utfall).
// En skugga följer ett djur genom livet och lär sig bara av det djuret själv upplevde: (förra bilden, den valda handlingen) → nästa bild.
// Skuggorna styr ingenting; djurens val kommer hela tiden från den befintliga hjärnan.
// ---------------------------------------------------------------------------------------------------------------------------------

/// Skuggmodellerna i jämförelsen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modell {
    /// Nio skilda förutsägare som hjärnans, men utan ärvd start (börjar som persistensen).
    Nio,
    /// Gemensam: en delad matris för alla handlingar och en förskjutning per handling.
    GemForskjutning,
    /// Gemensam: handlingen som indata i form av sin riktning (1, dx, dy, rör sig), gånger delade matriser.
    GemRiktning,
}

pub const N_NIO: usize = A_N * Z_N * (Z_N + 1);
pub const N_GEM_F: usize = Z_N * Z_N + Z_N + A_N * Z_N;
pub const HANDLINGSDRAG: usize = 4;
pub const N_GEM_R: usize = HANDLINGSDRAG * Z_N * (Z_N + 1);

/// Handlingens drag för den gemensamma riktningsmodellen: (1, dx, dy, rör sig).
pub fn handlingsdrag(a: usize) -> [f64; HANDLINGSDRAG] {
    let (dx, dy) = crate::sim::GRANNAR[a];
    [1.0, dx as f64, dy as f64, if a == 0 { 0.0 } else { 1.0 }]
}

/// Ett djurs skuggor och dess erfarenhet. Alla tre börjar som persistensen (nästa bild = nuvarande) och har ingen ärvd start.
#[derive(Clone, Debug)]
pub struct Skugga {
    pub nio: Vec<f64>,
    pub gem_f: Vec<f64>,
    pub gem_r: Vec<f64>,
    /// (förra bilden, den valda handlingen) som väntar på sitt utfall.
    pub forra: Option<([f64; Z_N], usize)>,
    /// Antal träningar per handling (= gånger djuret valt handlingen och sett utfallet).
    pub erfarenhet: [u32; A_N],
}

impl Default for Skugga {
    fn default() -> Self {
        Self::ny()
    }
}

impl Skugga {
    pub fn ny() -> Skugga {
        let mut nio = vec![0.0; N_NIO];
        for a in 0..A_N {
            for i in 0..Z_N {
                nio[a * Z_N * (Z_N + 1) + i * (Z_N + 1) + i] = 1.0;
            }
        }
        let mut gem_f = vec![0.0; N_GEM_F];
        for i in 0..Z_N {
            gem_f[i * Z_N + i] = 1.0;
        }
        let mut gem_r = vec![0.0; N_GEM_R];
        for i in 0..Z_N {
            gem_r[i * (Z_N + 1) + i] = 1.0;
        }
        Skugga { nio, gem_f, gem_r, forra: None, erfarenhet: [0; A_N] }
    }

    pub fn forutsag(&self, m: Modell, z: &[f64; Z_N], a: usize) -> [f64; Z_N] {
        let mut y = [0.0; Z_N];
        match m {
            Modell::Nio => {
                let b = a * Z_N * (Z_N + 1);
                for i in 0..Z_N {
                    let r = &self.nio[b + i * (Z_N + 1)..b + (i + 1) * (Z_N + 1)];
                    y[i] = r[Z_N] + (0..Z_N).map(|j| r[j] * z[j]).sum::<f64>();
                }
            }
            Modell::GemForskjutning => {
                for i in 0..Z_N {
                    y[i] = (0..Z_N).map(|j| self.gem_f[i * Z_N + j] * z[j]).sum::<f64>()
                        + self.gem_f[Z_N * Z_N + i]
                        + self.gem_f[Z_N * Z_N + Z_N + a * Z_N + i];
                }
            }
            Modell::GemRiktning => {
                let f = handlingsdrag(a);
                for k in 0..HANDLINGSDRAG {
                    if f[k] == 0.0 {
                        continue;
                    }
                    let b = k * Z_N * (Z_N + 1);
                    for i in 0..Z_N {
                        let r = &self.gem_r[b + i * (Z_N + 1)..b + (i + 1) * (Z_N + 1)];
                        y[i] += f[k] * (r[Z_N] + (0..Z_N).map(|j| r[j] * z[j]).sum::<f64>());
                    }
                }
            }
        }
        y
    }

    /// Ett träningssteg för alla tre skuggorna på samma erfarenhet: från bilden `z` med handlingen `a` blev nästa bild `mal`.
    /// Samma regel och takt som hjärnans förutsägare (ETA gånger felet gånger indata).
    pub fn lar(&mut self, z: &[f64; Z_N], a: usize, mal: &[f64; Z_N]) {
        let eta = crate::tal::ETA;
        let y = self.forutsag(Modell::Nio, z, a);
        let b = a * Z_N * (Z_N + 1);
        for i in 0..Z_N {
            let fel = y[i] - mal[i];
            for j in 0..=Z_N {
                let x = if j < Z_N { z[j] } else { 1.0 };
                self.nio[b + i * (Z_N + 1) + j] -= eta * fel * x;
            }
        }
        let y = self.forutsag(Modell::GemForskjutning, z, a);
        for i in 0..Z_N {
            let fel = y[i] - mal[i];
            for j in 0..Z_N {
                self.gem_f[i * Z_N + j] -= eta * fel * z[j];
            }
            self.gem_f[Z_N * Z_N + i] -= eta * fel;
            self.gem_f[Z_N * Z_N + Z_N + a * Z_N + i] -= eta * fel;
        }
        let y = self.forutsag(Modell::GemRiktning, z, a);
        let f = handlingsdrag(a);
        for i in 0..Z_N {
            let fel = y[i] - mal[i];
            for k in 0..HANDLINGSDRAG {
                if f[k] == 0.0 {
                    continue;
                }
                let bk = k * Z_N * (Z_N + 1);
                for j in 0..=Z_N {
                    let x = if j < Z_N { z[j] } else { 1.0 };
                    self.gem_r[bk + i * (Z_N + 1) + j] -= eta * fel * f[k] * x;
                }
            }
        }
        self.erfarenhet[a] += 1;
    }

    /// Djuret har just valt i ett steg: `z` är bilden det såg och `a` handlingen det valde. Förra stegets erfarenhet får sitt utfall (`z`).
    pub fn steg(&mut self, z: &[f64; Z_N], a: usize) {
        if let Some((zf, af)) = self.forra.take() {
            self.lar(&zf, af, z);
        }
        self.forra = Some((*z, a));
    }
}

/// Samma prov med en skuggas förutsägelser i stället för hjärnans. Skuggan får först, som hjärnan i steget, lära sig av förra stegets
/// erfarenhet med provets bild som utfall. Utfallen (de nio kopiorna) är desamma; ingen skugga ser dem före förutsägelsen.
pub fn prov_med_skugga(p: &Prov, skugga: &Skugga, m: Modell) -> Prov {
    let mut s = skugga.clone();
    if let Some((zf, af)) = s.forra.take() {
        s.lar(&zf, af, &p.z);
    }
    let mut q = p.clone();
    for a in 0..A_N {
        q.utfall[a].y = s.forutsag(m, &p.z, a);
    }
    q.erfarenhet = Some(s.erfarenhet);
    q
}

/// Persistensen som modell: nuvarande bild gissas för alla handlingar.
pub fn prov_med_persistens(p: &Prov) -> Prov {
    let mut q = p.clone();
    for a in 0..A_N {
        q.utfall[a].y = p.z;
    }
    q
}

/// Erfarenheten som hjärnans egna förutsägare har vid förutsägelsen i provsteget (skuggans räkning plus stegets egen träning).
pub fn erfarenhet_vid_prov(skugga: &Skugga) -> [u32; A_N] {
    let mut e = skugga.erfarenhet;
    if let Some((_, af)) = skugga.forra {
        e[af] += 1;
    }
    e
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Kompletteringsanalysen (Håkan 2026-11-23, "analys efter misslyckad världsmodellvalidering"): djurets egen historia av utförda handlingar.
// Läser bara vad djuret gjorde; ändrar ingenting. Fönstret "de föregående 365 simuleringsdagarna" är ett år = STEG_PER_AR steg.
// ---------------------------------------------------------------------------------------------------------------------------------

/// Ett djurs utförda handlingar: de senaste STEG_PER_AR stegen, hela livet, och när varje handling senast utfördes.
#[derive(Clone, Debug)]
pub struct Historik {
    senaste: std::collections::VecDeque<u8>,
    pub liv: [u32; A_N],
    senast: [i64; A_N],
    pub steg: i64,
}

impl Default for Historik {
    fn default() -> Self {
        Historik { senaste: std::collections::VecDeque::with_capacity(STEG_PER_AR as usize + 1), liv: [0; A_N], senast: [-1; A_N], steg: 0 }
    }
}

impl Historik {
    /// Djuret utförde handlingen `a` i ett steg.
    pub fn utfor(&mut self, a: usize) {
        self.senaste.push_back(a as u8);
        if self.senaste.len() > STEG_PER_AR as usize {
            self.senaste.pop_front();
        }
        self.liv[a] += 1;
        self.senast[a] = self.steg;
        self.steg += 1;
    }

    /// Antal utföranden per handling under de senaste STEG_PER_AR stegen (ett år, 365 dagar).
    pub fn senaste_aret(&self) -> [u32; A_N] {
        let mut n = [0u32; A_N];
        for &a in &self.senaste {
            n[a as usize] += 1;
        }
        n
    }

    /// Steg sedan handlingen senast utfördes (1 = i förra steget); None = aldrig.
    pub fn sedan(&self, a: usize) -> Option<i64> {
        if self.senast[a] < 0 {
            None
        } else {
            Some(self.steg - self.senast[a])
        }
    }

    /// Handlingen i förra steget.
    pub fn forra(&self) -> Option<usize> {
        self.senaste.back().map(|&a| a as usize)
    }
}

/// Felet och handlingsdelens rest per handling för ett prov: (|y_a − z'_a|², |(y_a − medel y) − (z'_a − medel z')|²).
pub fn fel_och_rest(p: &Prov) -> [(f64, f64); A_N] {
    let u = &p.utfall;
    let mut ym = [0.0; Z_N];
    let mut zm = [0.0; Z_N];
    for a in 0..A_N {
        for i in 0..Z_N {
            ym[i] += u[a].y[i] / A_N as f64;
            zm[i] += u[a].zn[i] / A_N as f64;
        }
    }
    let mut ut = [(0.0, 0.0); A_N];
    for a in 0..A_N {
        let mut rest = 0.0;
        for i in 0..Z_N {
            let d = (u[a].y[i] - ym[i]) - (u[a].zn[i] - zm[i]);
            rest += d * d;
        }
        ut[a] = (kvad(&u[a].y, &u[a].zn), rest);
    }
    ut
}

/// Handlingsdelens verkliga storlek per handling: |z'_a − medel z'|².
pub fn verklig_del(p: &Prov) -> [f64; A_N] {
    let u = &p.utfall;
    let mut zm = [0.0; Z_N];
    for a in 0..A_N {
        for i in 0..Z_N {
            zm[i] += u[a].zn[i] / A_N as f64;
        }
    }
    let mut ut = [0.0; A_N];
    for a in 0..A_N {
        ut[a] = (0..Z_N).map(|i| (u[a].zn[i] - zm[i]) * (u[a].zn[i] - zm[i])).sum();
    }
    ut
}
