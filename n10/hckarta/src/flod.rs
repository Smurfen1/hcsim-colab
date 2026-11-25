//! Flodnätet (S4): fåror österut nedför PaleoDEM, med träsk och sjöar. Allt finare än DEM:s ruta är simulering.
//! Version 2 (Alt 1343/1344 + Alt 1386 + Alt 1371): fåra, bälte och strandvall som andelar; bredd per fåra i 100–320 m.
//! Flodriktning 2 (Alt 1372/1472): steg öst/sydost/söder, starter längs västra+norra kanten, L = ruta_m·√(di²+dj²).

use crate::{
    karta_med, las_vaxtgrupper_inbyggd, rakna_vaxt_tathet_med, Kalla, Karta, Lager, Marke, Omrade,
    Period, Tidtabell, Vatten, VAXT_HOJD_M, VAXT_N,
};

/// Reglerna för flodnätet i en period (alla tal är egna val utom riktningen; skriv det i LASMIG.md).
#[derive(Clone, Debug)]
pub struct Flodregler {
    pub foror: usize,
    pub bredd_rutor: usize,
    pub trask_avstand: usize,
    pub sjoar_per_100_rutor: f64,
}

/// En fåras bredd, bälte och strandvall (flodversion 2, Alt 1386 + Alt 1371).
#[derive(Clone, Debug, PartialEq)]
pub struct ForaInfo {
    pub bredd_m: f64,
    pub balte_m: f64,
    pub rutor: usize,
    /// Vallens bredd `min(600, 4 · w_f)` m (båda sidorna tillsammans). Alt 1371.
    pub vall_m: f64,
}

/// Andelslager för flodversion 2 (egna lager; ligger inte i `Karta` så hcsim:s `Karta { … }` fortsätter bygga).
#[derive(Clone, Debug)]
pub struct FlodAndelar {
    pub fara_andel: Lager<f32>,
    pub balte_andel: Lager<f32>,
    /// En post per fåra (ordningen i `kor_mittlinjer`). Tom om kartan lästs utan `foror.tsv` (Alt 1343).
    pub foror: Vec<ForaInfo>,
    /// `true` om vallagren hör till kartan (Alt 1371); `false` för Alt 1343/1386 utan vallfiler.
    pub har_vall: bool,
    pub vall_andel: Lager<f32>,
    pub vall_hojd_m: Lager<f32>,
    /// `true` om växtgrupperna hör till kartan (Alt 1433); `false` för äldre mappar utan filerna.
    pub har_vaxtgrupper: bool,
    /// Täthet per grupp (sex tal) per ruta; tom/noll om `!har_vaxtgrupper`.
    pub vaxt_tathet: Lager<[f32; VAXT_N]>,
    /// Höjd per grupp (m); från underlaget vid skrivning, ur `vaxtgrupper.json` vid läsning.
    pub vaxt_hojd: [f32; VAXT_N],
    /// Antal arter per grupp ur `vaxtgrupper.tsv` (sex tal; noll om `!har_vaxtgrupper`).
    pub vaxt_arter: [usize; VAXT_N],
}

/// Lägsta fårbredd i meter (Butler 1980 [A], lägsta mätta; Huff "100 to 120 m"). Simulering.
pub const FARA_BREDD_MIN_M: f64 = 100.0;
/// Högsta fårbredd i meter (Hasler och Renne 2025 [A], medel före gränsen 315 m avrundat). Simulering.
pub const FARA_BREDD_MAX_M: f64 = 320.0;
/// Bälte = faktor · fårbredd (eget val inom Butlers 2,3–5,5). Simulering.
pub const BALTE_FAKTOR: f64 = 4.0;
/// Övre gräns för `balte_andel` (eget val, Alt 1344).
pub const BALTE_MAX: f32 = 0.85;
/// Vall = faktor · fårbredd (eget val inom Butlers 3–5). Simulering.
pub const VALL_FAKTOR: f64 = 4.0;
/// Tak för vallens bredd i meter (Butler 1980 [A], "as much as 600 m wide"). Simulering.
pub const VALL_MAX_M: f64 = 600.0;
/// Vallens höjd i meter över slätten (Butler 1980 [A], medelhöjden 2 m). Simulering.
pub const VALL_HOJD_M: f32 = 2.0;
/// Frö-xor för breddströmmen (ASCII "BREDD138"); rör inte vägsökningens `rng`.
const BREDD_FRO_XOR: u64 = 0x4252_4544_4431_3338;
/// Frö-xor för urvalet av levande fåror (ASCII "LEVFAROR"); eget val (Alt 1401).
const LEV_FRO_XOR: u64 = 0x4C45_5646_4152_4F52;
/// Frö-xor för loppbyten (ASCII "LOPPBYTE"); Alt 1395.
const LOPP_FRO_XOR: u64 = 0x4C4F_5050_4259_5445;
/// Minsta mellanrum mellan loppbyten (år). Räknat: 6 m · 151 000 / 11 m (Butler/Yamamura).
pub const LOPP_MIN_AR: u64 = 82_000;
/// Största mellanrum mellan loppbyten (år). Räknat: 6 m · 278 000 / 11 m.
pub const LOPP_MAX_AR: u64 = 152_000;
/// Bonus (m lägre kostnad) på rutor i andra lopp vid nytt lopp. Eget val (Alt 1395).
pub const LOPP_BONUS_M: f32 = 8.0;
/// Straff (m) på den egna gamla nedströmsdelen vid nytt lopp. Eget val (Alt 1395).
pub const LOPP_STRAFF_M: f32 = 8.0;
/// Största tillåtna stigning (m) över lägsta höjd uppströms på samma lopp efter loppbyte.
/// Eget val (Alt 1604): tidsmodell 1 har som störst djupet 6,932 m (stigning 4,932 m).
pub const LOPP_MAX_STIGNING_M: f32 = 5.0;

/// Provmutant för Alt 1604/1605 (bara proven). Produktionsvägen skickar alltid `Ingen`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoppProvMutant {
    /// Produktion: stigningsgräns [`LOPP_MAX_STIGNING_M`] och möte på (Alt 1605).
    Ingen,
    /// Annan max-stigning (m). 10 m ger rött i ryggprovet (Alt 1604-mutant). Möte av.
    MaxStigning(f32),
    /// Möte med annat levande lopp avslaget (Alt 1605-mutant; rött i korsmåttet).
    MotetAvslaget,
    /// Explicit möte på (samma som `Ingen` i produktionen efter Alt 1643/1605).
    MedMote,
}

impl LoppProvMutant {
    fn max_stigning_m(self) -> f32 {
        match self {
            LoppProvMutant::MaxStigning(m) => m,
            _ => LOPP_MAX_STIGNING_M,
        }
    }

    fn mote_pagatt(self) -> bool {
        matches!(self, LoppProvMutant::Ingen | LoppProvMutant::MedMote)
    }

    /// `true` = gammal propagerande delad yta (möte av); `false` = Alt 1605 mötesyta.
    fn propagera_delad_yta(self) -> bool {
        !self.mote_pagatt()
    }
}
/// Nedre andelen av fårans längd där bytet får ske. Eget val (Alt 1395).
const LOPP_PUNKT_MIN: f64 = 0.20;
/// Övre andelen av fårans längd där bytet får ske. Eget val (Alt 1395).
const LOPP_PUNKT_MAX: f64 = 0.80;
/// År som övergiven del är träsk innan den blir land (Alt 1494). Eget val; låst före mätningen.
pub const FYLLNADSTID_AR: f64 = 1000.0;

/// Ett loppbyte: år före gränsen, fåra (Aldre-nummer) och punktens ruta.
#[derive(Clone, Debug, PartialEq)]
pub struct Loppbyte {
    pub ar: f64,
    pub fara: usize,
    pub punkt: (usize, usize),
}

/// Riktningsstraff (m) på södersteg vid `--flodriktning 2`.
/// Val: minsta k ∈ {0, 2, 4, 8, 16, 32} där medelriktning frö 1–50 Aldre ∈ [125°, 140°]
/// (mätt som första→sista ruta; 0° = norr, 90° = öst).
/// Mätning (64×48, ruta 2000 m, frö 1–50 Aldre): k=0…32 ger alla medel 132,0° (söder
/// väljs bara när Ö/SO saknas, typiskt längs östra kanten); därför k = 0.
/// Intervallet 125–140° och listan = **eget val** (Alt 1472).
pub const FLODRIKTNING_STRAFF_M: f32 = 0.0;
/// Kandidater som programmet prövar när straffet väljs (samma lista som underlaget).
pub const FLODRIKTNING_K_KANDIDATER: [f32; 6] = [0.0, 2.0, 4.0, 8.0, 16.0, 32.0];
/// Målintervall för medelriktning (°), riktning 2, frö 1–50 Aldre. Eget val.
pub const FLODRIKTNING_MAL_MIN: f64 = 125.0;
pub const FLODRIKTNING_MAL_MAX: f64 = 140.0;

/// Lutning 2: fårans lutning 0,30 m/km (Butler 1980 [A], räknat med Schumms ekvation). Används för landet.
pub const LUTNING2_M_PER_KM: f64 = 0.30;
/// Lutning 2: nedför mot 135° (Hasler och Renne NW–SE-axel; exakt vinkel = **eget val**).
pub const LUTNING2_RIKTNING_GRADER: f64 = 135.0;

/// Planmätning över landrutor (höjd ≥ 0): lutning (m/km), riktning nedför (°; 0 = norr, 90 = öst),
/// medelhöjd (m). Samma metod som piharness `kartrask-plan.rs`.
pub fn mat_plan(h: &[f32], b: usize, hh: usize, ruta_km: f64) -> (f64, f64, f64) {
    let (mut sx, mut sy, mut sz, mut sxx, mut syy, mut sxy, mut sxz, mut syz, mut n) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for j in 0..hh {
        for i in 0..b {
            let z = h[i + j * b] as f64;
            if z < 0.0 {
                continue;
            }
            let x = i as f64 * ruta_km;
            let y = j as f64 * ruta_km;
            sx += x;
            sy += y;
            sz += z;
            sxx += x * x;
            syy += y * y;
            sxy += x * y;
            sxz += x * z;
            syz += y * z;
            n += 1.0;
        }
    }
    if n == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let (mx, my, mz) = (sx / n, sy / n, sz / n);
    let cxx = sxx / n - mx * mx;
    let cyy = syy / n - my * my;
    let cxy = sxy / n - mx * my;
    let cxz = sxz / n - mx * mz;
    let cyz = syz / n - my * mz;
    let det = cxx * cyy - cxy * cxy;
    let gx = (cxz * cyy - cyz * cxy) / det;
    let gy = (cyz * cxx - cxz * cxy) / det;
    let lut = (gx * gx + gy * gy).sqrt();
    let riktning = (90.0 + (-gy).atan2(-gx).to_degrees()).rem_euclid(360.0);
    (lut, riktning, mz)
}

/// Drar bort det anpassade planet (minsta kvadrat) ur landrutorna och lägger in ett plan som faller
/// `lutning_m_per_km` mot `riktning_grader`. Medelhöjden över land bevaras. Havsrutor (höjd < 0)
/// ändras inte. Hamnar en landruta under 0: `Err` (ingen tyst klämning). Sätter `hojd.marke` till
/// Simulering. Uppdaterar `vatten_yta` på land till den nya höjden (före flodnät).
pub fn applicera_lutning_plan(
    k: &mut Karta,
    lutning_m_per_km: f64,
    riktning_grader: f64,
) -> Result<(), String> {
    let b = k.omrade.b;
    let hh = k.omrade.h;
    let ruta_km = k.omrade.ruta_m / 1000.0;
    let (mut sx, mut sy, mut sz, mut sxx, mut syy, mut sxy, mut sxz, mut syz, mut n) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for j in 0..hh {
        for i in 0..b {
            let z = k.hojd.varden[idx(b, i, j)] as f64;
            if z < 0.0 {
                continue;
            }
            let x = i as f64 * ruta_km;
            let y = j as f64 * ruta_km;
            sx += x;
            sy += y;
            sz += z;
            sxx += x * x;
            syy += y * y;
            sxy += x * y;
            sxz += x * z;
            syz += y * z;
            n += 1.0;
        }
    }
    if n == 0.0 {
        return Ok(());
    }
    let (mx, my, mz) = (sx / n, sy / n, sz / n);
    let cxx = sxx / n - mx * mx;
    let cyy = syy / n - my * my;
    let cxy = sxy / n - mx * my;
    let cxz = sxz / n - mx * mz;
    let cyz = syz / n - my * mz;
    let det = cxx * cyy - cxy * cxy;
    let gx = (cxz * cyy - cyz * cxy) / det;
    let gy = (cyz * cxx - cxz * cxy) / det;
    let a = riktning_grader.to_radians();
    // Nedför mot θ: ∇h = (−lut·sin θ, +lut·cos θ) i (öst, söder); se kartrask-plan.rs.
    let gx2 = -lutning_m_per_km * a.sin();
    let gy2 = lutning_m_per_km * a.cos();
    for j in 0..hh {
        for i in 0..b {
            let p = idx(b, i, j);
            let h0 = k.hojd.varden[p] as f64;
            if h0 < 0.0 {
                continue;
            }
            let x = i as f64 * ruta_km;
            let y = j as f64 * ruta_km;
            let gammal = mz + gx * (x - mx) + gy * (y - my);
            let ny_plan = gx2 * (x - mx) + gy2 * (y - my);
            let h1 = h0 - gammal + mz + ny_plan;
            if h1 < 0.0 {
                return Err(format!(
                    "lutning: landruta ({i},{j}) hamnade under 0 ({h1:.3} m); stoppar"
                ));
            }
            k.hojd.varden[p] = h1 as f32;
            k.vatten_yta.varden[p] = h1 as f32;
        }
    }
    k.hojd.marke = Marke::Simulering;
    Ok(())
}

/// Fallande vattenyta längs varje fåras mittlinje: yta = min(förra, höjd − 2 m).
/// Vid delade rutor (anastomosed) itereras tills varje fåra är icke-stigande i rutnätet
/// (värden sänks bara). Returnerar unika mittlinjerutor med den ytan (för träskregeln).
/// Används i tidsmodell 1 (oförändrad).
fn fallande_yta_mittlinjer(
    k: &mut Karta,
    linjer: &[Vec<(usize, usize)>],
) -> Vec<(usize, usize, f32)> {
    fallande_yta_mittlinjer_lage(k, linjer, true)
}

/// Alt 1605 (tidsmodell 2): fallande yta per lopp för sig; i delade rutor (möte) sätts
/// lagrad yta till den lägre, men den lägre ytan propagerar **inte** som `prev` in i det
/// andra loppet nedströms (ingen lånad yta medan marken stiger).
fn fallande_yta_mittlinjer_mote(
    k: &mut Karta,
    linjer: &[Vec<(usize, usize)>],
) -> Vec<(usize, usize, f32)> {
    fallande_yta_mittlinjer_lage(k, linjer, false)
}

fn fallande_yta_mittlinjer_lage(
    k: &mut Karta,
    linjer: &[Vec<(usize, usize)>],
    propagera_delade: bool,
) -> Vec<(usize, usize, f32)> {
    let b = k.omrade.b;
    let n = b * k.omrade.h;
    let mut mitt = vec![false; n];
    for path in linjer {
        for &(i, j) in path {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            mitt[p] = true;
            k.vatten_yta.varden[p] = k.hojd.varden[p] - 2.0;
        }
    }
    if propagera_delade {
        loop {
            let mut andrad = false;
            for path in linjer {
                let mut prev: Option<f32> = None;
                for &(i, j) in path {
                    let p = idx(b, i, j);
                    if k.hojd.varden[p] < 0.0 {
                        continue;
                    }
                    let kandidat = match prev {
                        None => k.hojd.varden[p] - 2.0,
                        Some(py) => py.min(k.hojd.varden[p] - 2.0),
                    };
                    let y = kandidat.min(k.vatten_yta.varden[p]);
                    if (k.vatten_yta.varden[p] - y) > 1e-5 {
                        andrad = true;
                    }
                    k.vatten_yta.varden[p] = y;
                    prev = Some(y);
                }
            }
            if !andrad {
                break;
            }
        }
    } else {
        // Per lopp: egen yta längs stigen. Sedan min i delade rutor (mötesytan).
        let mut lokal = vec![f32::NAN; n];
        for path in linjer {
            let mut prev: Option<f32> = None;
            for &(i, j) in path {
                let p = idx(b, i, j);
                if k.hojd.varden[p] < 0.0 {
                    continue;
                }
                let kandidat = match prev {
                    None => k.hojd.varden[p] - 2.0,
                    Some(py) => py.min(k.hojd.varden[p] - 2.0),
                };
                prev = Some(kandidat);
                if lokal[p].is_nan() || kandidat < lokal[p] {
                    lokal[p] = kandidat;
                }
            }
        }
        for p in 0..n {
            if mitt[p] && !lokal[p].is_nan() {
                k.vatten_yta.varden[p] = lokal[p];
            }
        }
    }
    let mut sett = vec![false; n];
    let mut ut = Vec::new();
    for path in linjer {
        for &(i, j) in path {
            let p = idx(b, i, j);
            if !mitt[p] || sett[p] {
                continue;
            }
            sett[p] = true;
            ut.push((i, j, k.vatten_yta.varden[p]));
        }
    }
    ut
}

/// Reglerna per period: Aldre = flera fåror samtidigt (Roberts, "anastomosed"), Ovre och Gransen = färre, slingrande fåror (DePalma, "meandering").
pub fn flodregler(p: Period) -> Flodregler {
    match p {
        // Nedre delen: anastomosed — flera fåror, bredare träskbälten, fler sjöar.
        Period::Aldre => Flodregler {
            foror: 5,
            bredd_rutor: 2,
            trask_avstand: 3,
            sjoar_per_100_rutor: 2.0,
        },
        // Övre delen: meandrande, färre fåror.
        Period::Ovre => Flodregler {
            foror: 2,
            bredd_rutor: 2,
            trask_avstand: 3,
            sjoar_per_100_rutor: 1.5,
        },
        // Gränsen: samma fårebredd som Ovre (Butler 1980 [A]: bredden minskade);
        // mer stående vatten (Hasler och Renne 2025 [A]); talen 3 och 2,0 = eget val (Alt 1405).
        Period::Gransen => Flodregler {
            foror: 2,
            bredd_rutor: 2,
            trask_avstand: 3,
            sjoar_per_100_rutor: 2.0,
        },
    }
}

/// SplitMix64: samma frö ger samma talföljd på alla datorer.
struct Slump {
    state: u64,
}

impl Slump {
    fn ny(fro: u64) -> Self {
        Self { state: fro }
    }

    fn nasta(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn mellan(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.nasta() as usize) % n
    }

    /// Enhetligt i [0, 1).
    fn enhet(&mut self) -> f64 {
        (self.nasta() as f64) * (-64f64).exp2()
    }
}

/// Bredd per fåra: `w = exp(ln 100 + u · (ln 320 − ln 100))`, `u` ur egen ström.
pub fn fara_bredd_m(fro: u64, fara_nr: usize) -> f64 {
    let mut rng = Slump::ny(fro ^ BREDD_FRO_XOR);
    let mut w = FARA_BREDD_MIN_M;
    for _ in 0..=fara_nr {
        let u = rng.enhet();
        w = (FARA_BREDD_MIN_M.ln() + u * (FARA_BREDD_MAX_M.ln() - FARA_BREDD_MIN_M.ln())).exp();
    }
    w
}

/// Vallens bredd: `v_f = min(600, 4 · w_f)` m (båda sidorna tillsammans; Alt 1371).
pub fn vall_bredd_m(w_f: f64) -> f64 {
    (VALL_FAKTOR * w_f).min(VALL_MAX_M)
}

fn bredder_for_linjer(fro: u64, linjer: &[Vec<(usize, usize)>], k: &Karta) -> Vec<ForaInfo> {
    let nummer: Vec<usize> = (0..linjer.len()).collect();
    bredder_for_linjer_nr(fro, linjer, &nummer, k)
}

fn bredder_for_linjer_nr(
    fro: u64,
    linjer: &[Vec<(usize, usize)>],
    nummer: &[usize],
    k: &Karta,
) -> Vec<ForaInfo> {
    let b = k.omrade.b;
    let mut ut = Vec::with_capacity(linjer.len());
    for (fi, path) in linjer.iter().enumerate() {
        let bredd_m = fara_bredd_m(fro, nummer[fi]);
        let balte_m = BALTE_FAKTOR * bredd_m;
        let vall_m = vall_bredd_m(bredd_m);
        let rutor = path
            .iter()
            .filter(|&&(i, j)| k.hojd.varden[idx(b, i, j)] >= 0.0)
            .count();
        ut.push(ForaInfo {
            bredd_m,
            balte_m,
            rutor,
            vall_m,
        });
    }
    ut
}

/// Vilka Aldre-fåror (nummer 0..) som lever i perioden. Urvalet från Ovre är eget val (Alt 1401).
pub fn levande_foror_nr(fro: u64, period: Period) -> Vec<usize> {
    let alla = flodregler(Period::Aldre).foror;
    match period {
        Period::Aldre => (0..alla).collect(),
        Period::Ovre | Period::Gransen => {
            let n = flodregler(period).foror.min(alla);
            valj_levande(fro, alla, n)
        }
    }
}

fn valj_levande(fro: u64, alla: usize, n: usize) -> Vec<usize> {
    let mut rng = Slump::ny(fro ^ LEV_FRO_XOR);
    let mut valda = Vec::with_capacity(n);
    while valda.len() < n && valda.len() < alla {
        let x = rng.mellan(alla);
        if !valda.contains(&x) {
            valda.push(x);
        }
    }
    valda.sort_unstable();
    valda
}

fn idx(b: usize, i: usize, j: usize) -> usize {
    i + j * b
}

/// Höjdskillnad som räknas som "nästan lika" vid val bland östra grannar (eget val).
const NASTAN_LIKA_M: f32 = 0.5;

fn carve_cell(k: &mut Karta, i: usize, j: usize) {
    let b = k.omrade.b;
    let p = idx(b, i, j);
    if k.hojd.varden[p] < 0.0 {
        return; // Hav ändras aldrig
    }
    let h = k.hojd.varden[p];
    k.vattentyp.varden[p] = Vatten::Flodfara;
    k.vatten_yta.varden[p] = h - 2.0;
}

fn carve_med_bredd(k: &mut Karta, i: usize, j: usize, bredd: usize) {
    let h = k.omrade.h;
    let half = bredd.saturating_sub(1) / 2;
    let j0 = j.saturating_sub(half);
    let j1 = (j + half).min(h.saturating_sub(1));
    // Om bredden är jämn: half täcker bredd/2 norrut men en mindre söderut — lägg till en ruta söderut.
    let j1 = if bredd > 1 && bredd % 2 == 0 {
        (j1 + 1).min(h.saturating_sub(1))
    } else {
        j1
    };
    for jj in j0..=j1 {
        carve_cell(k, i, jj);
    }
}

/// En fåra (riktning 1): startar vid (0, j_start) och går österut till östra kanten.
/// Returnerar mittlinjen: en ruta `(i, j)` per kolumn.
fn en_fora(k: &mut Karta, j_start: usize, bredd: usize, rng: &mut Slump) -> Vec<(usize, usize)> {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let mut path = Vec::with_capacity(b);
    if b == 0 || h == 0 {
        return path;
    }
    let mut j = j_start.min(h - 1);
    carve_med_bredd(k, 0, j, bredd);
    path.push((0, j));

    for i in 0..b.saturating_sub(1) {
        // Tre grannar österut: nordost, öst, sydost.
        // PaleoDEM är grov: lägg frö-brus (±2 m) så fårorna inte alltid följer samma bilinjära dal.
        // Redan Flodfara får +8 m straff så anastomosed fåror håller isär (Roberts).
        let mut kandidater: Vec<(usize, f32)> = Vec::with_capacity(3);
        let nasta_i = i + 1;
        for dj in [-1isize, 0, 1] {
            let jj = j as isize + dj;
            if jj >= 0 && (jj as usize) < h {
                let jj = jj as usize;
                let p = idx(b, nasta_i, jj);
                let brus = (rng.mellan(5) as f32) - 2.0; // −2‥+2 m
                let straff = if k.vattentyp.varden[p] == Vatten::Flodfara {
                    8.0
                } else {
                    0.0
                };
                let effektiv = k.hojd.varden[p] + brus + straff;
                kandidater.push((jj, effektiv));
            }
        }
        if kandidater.is_empty() {
            break;
        }
        let min_h = kandidater.iter().map(|c| c.1).fold(f32::INFINITY, f32::min);
        let lika: Vec<usize> = kandidater
            .iter()
            .filter(|c| c.1 <= min_h + NASTAN_LIKA_M)
            .map(|c| c.0)
            .collect();
        j = lika[rng.mellan(lika.len())];
        carve_med_bredd(k, nasta_i, j, bredd);
        path.push((nasta_i, j));
    }
    path
}

/// Stegen i riktning 2: öst (1,0), sydost (1,1), söder (0,1). Ingen bakåt.
const STEG_RIKTNING2: [(isize, isize); 3] = [(1, 0), (1, 1), (0, 1)];

/// En fåra (riktning 2): startar vid `(i0, j0)` (västra eller norra kanten), går med
/// stegen öst/sydost/söder tills nästa steg skulle gå ut ur kartan eller i hav.
/// Södersteg får `straff_k` meter extra. Ingen ruta besöks två gånger.
fn en_fora_riktning2(
    k: &mut Karta,
    i0: usize,
    j0: usize,
    bredd: usize,
    rng: &mut Slump,
    straff_k: f32,
) -> Vec<(usize, usize)> {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let mut path = Vec::with_capacity(b + h);
    if b == 0 || h == 0 {
        return path;
    }
    let mut i = i0.min(b - 1);
    let mut j = j0.min(h - 1);
    // Start i hav: ingen fåra.
    if k.hojd.varden[idx(b, i, j)] < 0.0 {
        return path;
    }
    let mut besokt = vec![false; b * h];
    carve_med_bredd(k, i, j, bredd);
    path.push((i, j));
    besokt[idx(b, i, j)] = true;

    loop {
        // Kandidater innanför kartan, inte hav, inte besökta.
        let mut kandidater: Vec<(usize, usize, f32)> = Vec::with_capacity(3);
        let mut kan_ga_ut_eller_hav = false;
        for &(di, dj) in &STEG_RIKTNING2 {
            let ni = i as isize + di;
            let nj = j as isize + dj;
            if ni < 0 || nj < 0 || (ni as usize) >= b || (nj as usize) >= h {
                kan_ga_ut_eller_hav = true;
                continue;
            }
            let ni = ni as usize;
            let nj = nj as usize;
            let p = idx(b, ni, nj);
            if k.hojd.varden[p] < 0.0 {
                kan_ga_ut_eller_hav = true;
                continue;
            }
            if besokt[p] {
                continue;
            }
            let brus = (rng.mellan(5) as f32) - 2.0;
            let straff_fara = if k.vattentyp.varden[p] == Vatten::Flodfara {
                8.0
            } else {
                0.0
            };
            let straff_soder = if di == 0 && dj == 1 { straff_k } else { 0.0 };
            let effektiv = k.hojd.varden[p] + brus + straff_fara + straff_soder;
            kandidater.push((ni, nj, effektiv));
        }
        if kandidater.is_empty() {
            // Slut: nästa steg ut ur kartan / hav, eller blockerad (inga giltiga steg).
            let _ = kan_ga_ut_eller_hav;
            break;
        }
        let min_h = kandidater.iter().map(|c| c.2).fold(f32::INFINITY, f32::min);
        let lika: Vec<(usize, usize)> = kandidater
            .iter()
            .filter(|c| c.2 <= min_h + NASTAN_LIKA_M)
            .map(|c| (c.0, c.1))
            .collect();
        let (ni, nj) = lika[rng.mellan(lika.len())];
        i = ni;
        j = nj;
        carve_med_bredd(k, i, j, bredd);
        path.push((i, j));
        besokt[idx(b, i, j)] = true;
        // Säkerhetsgräns (kan inte besöka mer än alla rutor).
        if path.len() >= b * h {
            break;
        }
    }
    path
}

fn chebyshev(i0: usize, j0: usize, i1: usize, j1: usize) -> usize {
    let di = i0.abs_diff(i1);
    let dj = j0.abs_diff(j1);
    di.max(dj)
}

/// Hur träskregeln väljer fårans yta inom `trask_avstand`.
/// `Lagsta`: lägsta ytan bland fåror inom avståndet (lutning 1 / oförändrat).
/// `Narmaste`: närmaste mittlinjeruta i Chebyshev-avstånd; vid lika den lägsta ytan (Alt 1501, lutning 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraskYta {
    Lagsta,
    Narmaste,
}

/// Mutant för träskregeln under lutning 2 (Alt 1533 / 1584-prov). `Ingen` = normal närmaste-regel.
/// Används bara av proven; produktionsvägen anropar alltid `Ingen`.
/// Gäller tidsmodell 1 (`lagg_flodnat_version_lutning_prov`) och tidsmodell 2
/// (`karta_for_ar_lutning_prov` → `lagg_version2_urval` / `lagg_foror_som_objekt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraskProvMutant {
    Ingen,
    /// Mutant A: närmaste := lägsta (`Lagsta` i stället för `Narmaste`).
    NarmasteSomLagsta,
    /// Mutant B: `trask_avstand` dubblat.
    DubbeltAvstand,
    /// Provmutant (Alt 1644 / steg 0-mätning): tvinga `trask_avstand` till angivet tal.
    Avstand(usize),
}

fn trask_val(lutning: u8, avstand: usize, mutant: TraskProvMutant) -> (TraskYta, usize) {
    let yta = if lutning >= 2 {
        TraskYta::Narmaste
    } else {
        TraskYta::Lagsta
    };
    match mutant {
        TraskProvMutant::Ingen => (yta, avstand),
        TraskProvMutant::NarmasteSomLagsta => (TraskYta::Lagsta, avstand),
        TraskProvMutant::DubbeltAvstand => (yta, avstand.saturating_mul(2).max(1)),
        TraskProvMutant::Avstand(a) => (yta, a),
    }
}

fn lagg_trask_fran(
    k: &mut Karta,
    foror: &[(usize, usize, f32)],
    avstand: usize,
    skip: &[bool],
    yta: TraskYta,
) {
    let b = k.omrade.b;
    let h = k.omrade.h;
    if foror.is_empty() || avstand == 0 {
        return;
    }
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            if skip[p] {
                continue;
            }
            let fh = match yta {
                TraskYta::Lagsta => {
                    let mut lagsta: Option<f32> = None;
                    for &(fi, fj, fy) in foror {
                        if chebyshev(i, j, fi, fj) <= avstand {
                            lagsta = Some(match lagsta {
                                None => fy,
                                Some(x) => x.min(fy),
                            });
                        }
                    }
                    lagsta
                }
                TraskYta::Narmaste => {
                    let mut vald: Option<(usize, f32)> = None;
                    for &(fi, fj, fy) in foror {
                        let d = chebyshev(i, j, fi, fj);
                        if d > avstand {
                            continue;
                        }
                        vald = Some(match vald {
                            None => (d, fy),
                            Some((d0, _)) if d < d0 => (d, fy),
                            Some((d0, x)) if d == d0 => (d, x.min(fy)),
                            Some(v) => v,
                        });
                    }
                    vald.map(|(_, fy)| fy)
                }
            };
            if let Some(fh) = fh {
                if k.hojd.varden[p] < fh + 1.0 {
                    k.vattentyp.varden[p] = Vatten::Trask;
                    // Vattenytan i träsk: något under markytan men ovanför flodfårans sänkning.
                    k.vatten_yta.varden[p] = k.hojd.varden[p] - 0.5;
                }
            }
        }
    }
}

fn lagg_trask(k: &mut Karta, avstand: usize) {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let n = b * h;
    let mut foror: Vec<(usize, usize, f32)> = Vec::new();
    let mut skip = vec![false; n];
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if k.vattentyp.varden[p] == Vatten::Flodfara {
                foror.push((i, j, k.hojd.varden[p]));
                skip[p] = true;
            }
        }
    }
    lagg_trask_fran(k, &foror, avstand, &skip, TraskYta::Lagsta);
}

fn lagg_sjoar_fran(
    k: &mut Karta,
    regler: &Flodregler,
    rng: &mut Slump,
    foror: &[(usize, usize)],
    skip: &[bool],
) {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let n = b * h;
    if n == 0 || foror.is_empty() {
        return;
    }
    let avstand = regler.trask_avstand.max(1);
    let mut kandidater: Vec<(usize, usize, f32)> = Vec::new();
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 || skip[p] || k.vattentyp.varden[p] == Vatten::Hav {
                continue;
            }
            let nara = foror
                .iter()
                .any(|&(fi, fj)| chebyshev(i, j, fi, fj) <= avstand);
            if nara {
                kandidater.push((i, j, k.hojd.varden[p]));
            }
        }
    }
    // Lägsta först — sjöar i låga rutor nära fårorna.
    kandidater.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    let antal = ((n as f64) * regler.sjoar_per_100_rutor / 100.0).round() as usize;
    let antal = antal.min(kandidater.len());
    // Ta bland de lägsta, med slump så samma höjd inte alltid ger samma ruta.
    let pool = (antal * 3).max(antal).min(kandidater.len());
    let mut valda = 0usize;
    let mut anvand: Vec<bool> = vec![false; pool];
    while valda < antal && valda < pool {
        let ix = rng.mellan(pool);
        if anvand[ix] {
            continue;
        }
        anvand[ix] = true;
        let (i, j, _) = kandidater[ix];
        let p = idx(b, i, j);
        if skip[p] || k.hojd.varden[p] < 0.0 {
            continue;
        }
        k.vattentyp.varden[p] = Vatten::Sjo;
        k.vatten_yta.varden[p] = k.hojd.varden[p] - 1.0;
        valda += 1;
    }
}

fn lagg_sjoar(k: &mut Karta, regler: &Flodregler, rng: &mut Slump) {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let n = b * h;
    let mut foror: Vec<(usize, usize)> = Vec::new();
    let mut skip = vec![false; n];
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if k.vattentyp.varden[p] == Vatten::Flodfara {
                foror.push((i, j));
                skip[p] = true;
            }
        }
    }
    lagg_sjoar_fran(k, regler, rng, &foror, &skip);
}

fn starter_foror(h: usize, foror_n: usize, rng: &mut Slump) -> Vec<usize> {
    let mut starter: Vec<usize> = Vec::with_capacity(foror_n);
    if h == 0 {
        return starter;
    }
    for f in 0..foror_n {
        let bas = ((f as f64 + 0.5) * (h as f64) / (foror_n as f64)) as usize;
        let jitter = rng.mellan(3) as isize - 1; // −1, 0, +1
        let j = (bas as isize + jitter).clamp(0, h as isize - 1) as usize;
        starter.push(j);
    }
    // Unika starter om möjligt (undvik att alla fåror börjar på samma rad).
    starter.sort_unstable();
    starter.dedup();
    while starter.len() < foror_n && starter.len() < h {
        let j = rng.mellan(h);
        if !starter.contains(&j) {
            starter.push(j);
        }
    }
    starter
}

/// Ruta längs den sammanhängande kanten V→N: västra kanten söder→norr, sedan norra kanten väster→öst
/// (hörnet NW räknas en gång). Index 0 = (0, h−1), index h−1 = (0, 0), index h+b−2 = (b−1, 0).
fn kant_ruta(b: usize, h: usize, ix: usize) -> (usize, usize) {
    if ix < h {
        (0, h - 1 - ix)
    } else {
        let i = ix - h + 1;
        (i.min(b.saturating_sub(1)), 0)
    }
}

/// Starter längs västra + norra kanten (riktning 2 / Alt 1472). Samma antal och jitter ±1 som riktning 1.
fn starter_foror_riktning2(
    b: usize,
    h: usize,
    foror_n: usize,
    rng: &mut Slump,
) -> Vec<(usize, usize)> {
    let mut starter: Vec<(usize, usize)> = Vec::with_capacity(foror_n);
    if b == 0 || h == 0 {
        return starter;
    }
    let n_kant = h + b - 1;
    for f in 0..foror_n {
        let bas = ((f as f64 + 0.5) * (n_kant as f64) / (foror_n as f64)) as usize;
        let jitter = rng.mellan(3) as isize - 1; // −1, 0, +1
        let ix = (bas as isize + jitter).clamp(0, n_kant as isize - 1) as usize;
        starter.push(kant_ruta(b, h, ix));
    }
    starter.sort_unstable();
    starter.dedup();
    while starter.len() < foror_n && starter.len() < n_kant {
        let ix = rng.mellan(n_kant);
        let r = kant_ruta(b, h, ix);
        if !starter.contains(&r) {
            starter.push(r);
        }
    }
    starter
}

/// Pathfinding + bredd-carving; returnerar varje fåras mittlinje.
/// `flodriktning` 1 = österut (NO/Ö/SO); 2 = sydost (Ö/SO/S) med straff `FLODRIKTNING_STRAFF_M`.
fn kor_mittlinjer(
    k: &mut Karta,
    fro: u64,
    flodriktning: u8,
) -> (Vec<Vec<(usize, usize)>>, Flodregler, Slump) {
    kor_mittlinjer_med_straff(k, fro, flodriktning, FLODRIKTNING_STRAFF_M)
}

/// Som `kor_mittlinjer`, men med valbart söderstraff (för mätning av k och mutanter).
fn kor_mittlinjer_med_straff(
    k: &mut Karta,
    fro: u64,
    flodriktning: u8,
    straff_k: f32,
) -> (Vec<Vec<(usize, usize)>>, Flodregler, Slump) {
    let period = k
        .period
        .varden
        .first()
        .copied()
        .unwrap_or(Period::Aldre);
    let regler = flodregler(period);
    let mut rng = Slump::ny(fro);
    let b = k.omrade.b;
    let h = k.omrade.h;
    let foror_n = regler.foror.max(1);
    let bredd = regler.bredd_rutor.max(1);
    let mut linjer = Vec::with_capacity(foror_n);
    if flodriktning >= 2 {
        let starter = starter_foror_riktning2(b, h, foror_n, &mut rng);
        for &(i0, j0) in &starter {
            linjer.push(en_fora_riktning2(k, i0, j0, bredd, &mut rng, straff_k));
        }
    } else {
        let starter = starter_foror(h, foror_n, &mut rng);
        for &j0 in &starter {
            linjer.push(en_fora(k, j0, bredd, &mut rng));
        }
    }
    (linjer, regler, rng)
}

fn rensa_flodfara(k: &mut Karta) {
    let b = k.omrade.b;
    let h = k.omrade.h;
    for j in 0..h {
        for i in 0..b {
            let p = idx(b, i, j);
            if k.vattentyp.varden[p] == Vatten::Flodfara {
                k.vattentyp.varden[p] = Vatten::Land;
                k.vatten_yta.varden[p] = k.hojd.varden[p];
            }
        }
    }
}

/// `alt1694`: träskets täthet i standardvägen (tidsmodell 2 med lutning ≥ 2, Alt 1694).
fn bygg_andelar(k: &Karta, linjer: &[Vec<(usize, usize)>], foror: &[ForaInfo], alt1694: bool) -> FlodAndelar {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let n = b * h;
    let ruta_m = k.omrade.ruta_m;
    let ruta_m2 = ruta_m * ruta_m;
    let mut fara = vec![0.0f32; n];
    let mut balte = vec![0.0f32; n];
    let mut vall = vec![0.0f32; n];

    for (fi, path) in linjer.iter().enumerate() {
        let w = foror[fi].bredd_m;
        let bw = foror[fi].balte_m;
        let vw = foror[fi].vall_m;
        for (ci, &(i, j)) in path.iter().enumerate() {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue; // mittlinje i hav: ingen andel
            }
            // L = ruta_m · √(di² + dj²) till nästa ruta; sista rutan: L = ruta_m (Alt 1472).
            // Riktning 1 har alltid di = 1, så √(1 + dj²) — samma som före.
            let l = if ci + 1 < path.len() {
                let di = path[ci + 1].0 as isize - i as isize;
                let dj = path[ci + 1].1 as isize - j as isize;
                ruta_m * ((di * di + dj * dj) as f64).sqrt()
            } else {
                ruta_m
            };
            let fa = (w * l / ruta_m2).min(1.0) as f32;
            let ba = (bw * l / ruta_m2).min(BALTE_MAX as f64) as f32;
            let va = (vw * l / ruta_m2) as f32;
            fara[p] = (fara[p] + fa).min(1.0);
            balte[p] = (balte[p] + ba).min(BALTE_MAX);
            vall[p] += va;
        }
    }
    for p in 0..n {
        if balte[p] < fara[p] {
            balte[p] = fara[p].min(BALTE_MAX);
        }
        // Vallen ligger utanför bältet (Butler); klipp så fåra+bälte+vall ≤ 1.
        let tak = (1.0 - balte[p]).max(0.0);
        if vall[p] > tak {
            vall[p] = tak;
        }
        if vall[p] < 0.0 {
            vall[p] = 0.0;
        }
    }
    let mut vall_hojd = vec![0.0f32; n];
    for p in 0..n {
        if vall[p] > 0.0 {
            vall_hojd[p] = VALL_HOJD_M;
        }
    }
    let lasning = las_vaxtgrupper_inbyggd().expect("vaxtgrupper.tsv");
    let tathet = rakna_vaxt_tathet_med(
        &k.vattentyp.varden,
        &k.period.varden,
        &balte,
        &vall,
        alt1694,
    );
    FlodAndelar {
        fara_andel: Lager {
            varden: fara,
            marke: Marke::Simulering,
        },
        balte_andel: Lager {
            varden: balte,
            marke: Marke::Simulering,
        },
        foror: foror.to_vec(),
        har_vall: true,
        vall_andel: Lager {
            varden: vall,
            marke: Marke::Simulering,
        },
        vall_hojd_m: Lager {
            varden: vall_hojd,
            marke: Marke::Simulering,
        },
        har_vaxtgrupper: true,
        vaxt_tathet: Lager {
            varden: tathet,
            marke: Marke::Simulering,
        },
        vaxt_hojd: VAXT_HOJD_M,
        vaxt_arter: lasning.arter_per_grupp,
    }
}

fn lagg_version2_mutant(
    k: &mut Karta,
    linjer: &[Vec<(usize, usize)>],
    regler: &Flodregler,
    rng: &mut Slump,
    fro: u64,
    lutning: u8,
    mutant: TraskProvMutant,
) -> FlodAndelar {
    let b = k.omrade.b;
    let n = b * k.omrade.h;
    rensa_flodfara(k);

    let mut mitt = vec![false; n];
    let mut foror_ij: Vec<(usize, usize)> = Vec::new();
    let foror_h: Vec<(usize, usize, f32)> = if lutning >= 2 {
        let ytor = fallande_yta_mittlinjer(k, linjer);
        for &(i, j, _) in &ytor {
            let p = idx(b, i, j);
            mitt[p] = true;
            foror_ij.push((i, j));
        }
        ytor
    } else {
        let mut foror_h = Vec::new();
        for path in linjer {
            for &(i, j) in path {
                let p = idx(b, i, j);
                if k.hojd.varden[p] < 0.0 {
                    continue;
                }
                if !mitt[p] {
                    mitt[p] = true;
                    foror_h.push((i, j, k.hojd.varden[p]));
                    foror_ij.push((i, j));
                }
            }
        }
        foror_h
    };

    let (trask_yta, avstand) = trask_val(lutning, regler.trask_avstand, mutant);
    lagg_trask_fran(k, &foror_h, avstand, &mitt, trask_yta);
    lagg_sjoar_fran(k, regler, rng, &foror_ij, &mitt);

    // Mittlinjerutor: Land (träsk/sjö hoppar över dem).
    // Lutning 1: vatten_yta = höjd − 2 m. Lutning 2: redan satt av fallande_yta_mittlinjer.
    for path in linjer {
        for &(i, j) in path {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            k.vattentyp.varden[p] = Vatten::Land;
            if lutning < 2 {
                k.vatten_yta.varden[p] = k.hojd.varden[p] - 2.0;
            }
        }
    }

    let foror = bredder_for_linjer(fro, linjer, k);
    bygg_andelar(k, linjer, &foror, false)
}

/// Övergivna delar: träsk i `fyllnadstid` år efter övergivandet, därefter land
/// (utom levande lopp / hav; träsk/sjö från levande regler får stå kvar). Alt 1494.
fn lagg_overgivna_trask(
    k: &mut Karta,
    overgivna: &[(Vec<(usize, usize)>, f64)],
    levande_mitt: &[bool],
    t: f64,
    fyllnadstid: f64,
) {
    let b = k.omrade.b;
    for (path, ar_overgiven) in overgivna {
        // Fylld när t ≤ ar − fyllnadstid (intervallet för träsk är (ar − fyllnadstid, ar]).
        if t <= *ar_overgiven - fyllnadstid {
            continue;
        }
        for &(i, j) in path {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            if levande_mitt[p] {
                continue;
            }
            if k.vattentyp.varden[p] == Vatten::Flodfara {
                continue;
            }
            // Övergiven fåra → träsk (eget val, Alt 1401/1494); vatten_yta = höjd.
            k.vattentyp.varden[p] = Vatten::Trask;
            k.vatten_yta.varden[p] = k.hojd.varden[p];
        }
    }
}

fn lagg_version2_urval(
    k: &mut Karta,
    linjer: &[Vec<(usize, usize)>],
    nummer: &[usize],
    overgivna: &[(Vec<(usize, usize)>, f64)],
    regler: &Flodregler,
    rng: &mut Slump,
    fro: u64,
    fyllnadstid: f64,
    lutning: u8,
    mutant: TraskProvMutant,
    lopp_mutant: LoppProvMutant,
) -> FlodAndelar {
    let b = k.omrade.b;
    let n = b * k.omrade.h;
    rensa_flodfara(k);

    let mut mitt = vec![false; n];
    let mut foror_ij: Vec<(usize, usize)> = Vec::new();
    let foror_h: Vec<(usize, usize, f32)> = if lutning >= 2 {
        // Alt 1605-mötesyta i produktion (`Ingen`) och via `MedMote`; `MotetAvslaget` = gammal delad yta.
        let ytor = if lopp_mutant.propagera_delad_yta() {
            fallande_yta_mittlinjer(k, linjer)
        } else {
            fallande_yta_mittlinjer_mote(k, linjer)
        };
        for &(i, j, _) in &ytor {
            let p = idx(b, i, j);
            mitt[p] = true;
            foror_ij.push((i, j));
        }
        ytor
    } else {
        let mut foror_h = Vec::new();
        for path in linjer {
            for &(i, j) in path {
                let p = idx(b, i, j);
                if k.hojd.varden[p] < 0.0 {
                    continue;
                }
                if !mitt[p] {
                    mitt[p] = true;
                    foror_h.push((i, j, k.hojd.varden[p]));
                    foror_ij.push((i, j));
                }
            }
        }
        foror_h
    };

    let (trask_yta, avstand) = trask_val(lutning, regler.trask_avstand, mutant);
    lagg_trask_fran(k, &foror_h, avstand, &mitt, trask_yta);
    lagg_sjoar_fran(k, regler, rng, &foror_ij, &mitt);

    for path in linjer {
        for &(i, j) in path {
            let p = idx(b, i, j);
            if k.hojd.varden[p] < 0.0 {
                continue;
            }
            k.vattentyp.varden[p] = Vatten::Land;
            if lutning < 2 {
                k.vatten_yta.varden[p] = k.hojd.varden[p] - 2.0;
            }
        }
    }

    lagg_overgivna_trask(k, overgivna, &mitt, k.t, fyllnadstid);

    let foror = bredder_for_linjer_nr(fro, linjer, nummer, k);
    // Alt 1694: standardvägens träsktäthet bara i tidsmodell 2 med lutning ≥ 2.
    bygg_andelar(k, linjer, &foror, lutning >= 2)
}

/// Delström per fåra för loppbytets tider och punktindex (ASCII LOPPBYTE).
fn fora_lopp_rng(fro: u64, fara: usize) -> Slump {
    Slump::ny(
        fro ^ LOPP_FRO_XOR ^ (fara as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1),
    )
}

/// Separat ström för pathfinding av det nya loppet (beroende av år och fåra).
fn fora_byte_path_rng(fro: u64, fara: usize, ar: f64) -> Slump {
    Slump::ny(
        fro ^ LOPP_FRO_XOR
            ^ 0x5041_5448_4C4F_5050 // "PATHLOPP"
            ^ (fara as u64).wrapping_mul(0x10001)
            ^ (ar as i64 as u64),
    )
}

fn lopp_mellanrum(rng: &mut Slump) -> u64 {
    LOPP_MIN_AR + rng.mellan((LOPP_MAX_AR - LOPP_MIN_AR + 1) as usize) as u64
}

fn punkt_index(len: usize, punkt_raw: u64) -> usize {
    if len == 0 {
        return 0;
    }
    // ceil(20 %) … floor(80 %) så att pidx/len ∈ [0,20; 0,80].
    let mut lo = (LOPP_PUNKT_MIN * len as f64).ceil() as usize;
    let mut hi = (LOPP_PUNKT_MAX * len as f64).floor() as usize;
    if lo >= len {
        lo = len - 1;
    }
    if hi >= len {
        hi = len - 1;
    }
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    let span = hi - lo + 1;
    lo + (punkt_raw as usize % span)
}

/// Schemalägger alla loppbyten (år, fåra, rått punktindex) från basen till gränsen.
fn schemalagg_loppbyten(tt: &Tidtabell, fro: u64) -> Vec<(f64, usize, u64)> {
    let basen = tt.last_tid("basen").unwrap_or(1_081_000.0);
    let dod = tt.last_tid("magnetvandning").unwrap_or(290_000.0);
    let lev_ovre = levande_foror_nr(fro, Period::Ovre);
    let n = flodregler(Period::Aldre).foror;
    let mut events = Vec::new();
    for fara in 0..n {
        let mut rng = fora_lopp_rng(fro, fara);
        let mut ar = basen;
        let lever_efter = lev_ovre.contains(&fara);
        loop {
            let dt = lopp_mellanrum(&mut rng) as f64;
            ar -= dt;
            if ar <= 0.0 {
                break;
            }
            if ar <= dod && !lever_efter {
                break;
            }
            let punkt_raw = rng.nasta();
            events.push((ar, fara, punkt_raw));
        }
    }
    events.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    events
}

/// Resultat av `fortsatt_fora`: väg eller avbrutet byte (ingen tillåten väg inom stigningsgränsen).
enum FortsattUt {
    Vag(Vec<(usize, usize)>),
    Avbryt,
}

/// Fortsätter en fåra från `start` (utan carving) med bonus/straff-masker.
/// `minh_uppstrom` = lägsta höjd från fårans start genom bytespunkten.
/// `max_stigning_m` = Alt 1604-gräns; `mot_levande` = mask för andra levande lopp (Alt 1605; None = avslaget).
fn fortsatt_fora(
    k: &Karta,
    start: (usize, usize),
    flodriktning: u8,
    rng: &mut Slump,
    bonus: &[bool],
    straff: &[bool],
    bonus_m: f32,
    straff_m: f32,
    straff_soder: f32,
    minh_uppstrom: f32,
    max_stigning_m: f32,
    mot_levande: Option<&[bool]>,
) -> FortsattUt {
    if flodriktning >= 2 {
        fortsatt_fora_riktning2(
            k,
            start,
            rng,
            bonus,
            straff,
            bonus_m,
            straff_m,
            straff_soder,
            minh_uppstrom,
            max_stigning_m,
            mot_levande,
        )
    } else {
        fortsatt_fora_riktning1(
            k,
            start,
            rng,
            bonus,
            straff,
            bonus_m,
            straff_m,
            minh_uppstrom,
            max_stigning_m,
            mot_levande,
        )
    }
}

fn kostnad(
    k: &Karta,
    p: usize,
    rng: &mut Slump,
    bonus: &[bool],
    straff: &[bool],
    bonus_m: f32,
    straff_m: f32,
    extra: f32,
) -> f32 {
    let brus = (rng.mellan(5) as f32) - 2.0;
    let bns = if bonus[p] { -bonus_m } else { 0.0 };
    let stf = if straff[p] { straff_m } else { 0.0 };
    k.hojd.varden[p] + brus + bns + stf + extra
}

/// Väljer nästa ruta: kostnadsval som i dag; om det bryter stigningsgränsen, bland tillåtna det som stiger minst.
/// `None` = geografiskt slut; `Avbryt` = kandidater finns men ingen håller gränsen.
fn valj_steg_med_stigning<T: Copy>(
    kandidater: &[(T, f32, f32)], // (val, effektiv_kostnad, hojd)
    rng: &mut Slump,
    minh: f32,
    max_stigning_m: f32,
) -> Result<Option<(T, f32)>, FortsattUt> {
    if kandidater.is_empty() {
        return Ok(None);
    }
    let min_kost = kandidater
        .iter()
        .map(|c| c.1)
        .fold(f32::INFINITY, f32::min);
    let lika: Vec<(T, f32)> = kandidater
        .iter()
        .filter(|c| c.1 <= min_kost + NASTAN_LIKA_M)
        .map(|c| (c.0, c.2))
        .collect();
    let (valt, hojd) = lika[rng.mellan(lika.len())];
    if hojd - minh <= max_stigning_m {
        return Ok(Some((valt, hojd)));
    }
    // Bryter gränsen: bland dem som håller den, stiger minst (lägst höjd).
    let mut tillatna: Vec<(T, f32)> = kandidater
        .iter()
        .filter(|c| c.2 - minh <= max_stigning_m)
        .map(|c| (c.0, c.2))
        .collect();
    if tillatna.is_empty() {
        return Err(FortsattUt::Avbryt);
    }
    let minst = tillatna
        .iter()
        .map(|c| c.1)
        .fold(f32::INFINITY, f32::min);
    let lika_stig: Vec<(T, f32)> = tillatna
        .drain(..)
        .filter(|c| (c.1 - minst).abs() <= 1e-5)
        .collect();
    Ok(Some(lika_stig[rng.mellan(lika_stig.len())]))
}

fn fortsatt_fora_riktning1(
    k: &Karta,
    start: (usize, usize),
    rng: &mut Slump,
    bonus: &[bool],
    straff: &[bool],
    bonus_m: f32,
    straff_m: f32,
    minh_uppstrom: f32,
    max_stigning_m: f32,
    mot_levande: Option<&[bool]>,
) -> FortsattUt {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let mut path = Vec::new();
    let (mut i, mut j) = start;
    if i >= b || j >= h || k.hojd.varden[idx(b, i, j)] < 0.0 {
        return FortsattUt::Vag(path);
    }
    path.push((i, j));
    let mut minh = minh_uppstrom;
    while i + 1 < b {
        let nasta_i = i + 1;
        let mut kandidater: Vec<(usize, f32, f32)> = Vec::with_capacity(3);
        for dj in [-1isize, 0, 1] {
            let jj = j as isize + dj;
            if jj >= 0 && (jj as usize) < h {
                let jj = jj as usize;
                let p = idx(b, nasta_i, jj);
                if k.hojd.varden[p] < 0.0 {
                    continue;
                }
                let effektiv = kostnad(k, p, rng, bonus, straff, bonus_m, straff_m, 0.0);
                kandidater.push((jj, effektiv, k.hojd.varden[p]));
            }
        }
        let val = match valj_steg_med_stigning(&kandidater, rng, minh, max_stigning_m) {
            Ok(v) => v,
            Err(avbryt) => return avbryt,
        };
        let Some((jj, hojd)) = val else {
            break;
        };
        j = jj;
        i = nasta_i;
        path.push((i, j));
        minh = minh.min(hojd);
        let p = idx(b, i, j);
        if mot_levande.map(|m| m[p]).unwrap_or(false) {
            break;
        }
    }
    FortsattUt::Vag(path)
}

fn fortsatt_fora_riktning2(
    k: &Karta,
    start: (usize, usize),
    rng: &mut Slump,
    bonus: &[bool],
    straff: &[bool],
    bonus_m: f32,
    straff_m: f32,
    straff_soder: f32,
    minh_uppstrom: f32,
    max_stigning_m: f32,
    mot_levande: Option<&[bool]>,
) -> FortsattUt {
    let b = k.omrade.b;
    let h = k.omrade.h;
    let mut path = Vec::new();
    let (mut i, mut j) = start;
    if i >= b || j >= h || k.hojd.varden[idx(b, i, j)] < 0.0 {
        return FortsattUt::Vag(path);
    }
    let mut besokt = vec![false; b * h];
    path.push((i, j));
    besokt[idx(b, i, j)] = true;
    let mut minh = minh_uppstrom;
    loop {
        let mut kandidater: Vec<((usize, usize), f32, f32)> = Vec::with_capacity(3);
        for &(di, dj) in &STEG_RIKTNING2 {
            let ni = i as isize + di;
            let nj = j as isize + dj;
            if ni < 0 || nj < 0 || (ni as usize) >= b || (nj as usize) >= h {
                continue;
            }
            let ni = ni as usize;
            let nj = nj as usize;
            let p = idx(b, ni, nj);
            if k.hojd.varden[p] < 0.0 || besokt[p] {
                continue;
            }
            let extra = if di == 0 && dj == 1 { straff_soder } else { 0.0 };
            let effektiv = kostnad(k, p, rng, bonus, straff, bonus_m, straff_m, extra);
            kandidater.push(((ni, nj), effektiv, k.hojd.varden[p]));
        }
        let val = match valj_steg_med_stigning(&kandidater, rng, minh, max_stigning_m) {
            Ok(v) => v,
            Err(avbryt) => return avbryt,
        };
        let Some(((ni, nj), hojd)) = val else {
            break;
        };
        i = ni;
        j = nj;
        path.push((i, j));
        minh = minh.min(hojd);
        besokt[idx(b, i, j)] = true;
        if mot_levande.map(|m| m[idx(b, i, j)]).unwrap_or(false) {
            break;
        }
        if path.len() >= b * h {
            break;
        }
    }
    FortsattUt::Vag(path)
}

fn markera_path(mask: &mut [bool], path: &[(usize, usize)], b: usize) {
    for &(i, j) in path {
        mask[idx(b, i, j)] = true;
    }
}

/// Spelar upp historiken från basen till `t`. Returnerar levande linjer (i nummerordning),
/// deras nummer, övergivna segment med övergivningsår, och byten skedda vid `t`
/// (händelser med år &gt; t). `lopp_mutant` styr stigningsgräns och möte (Alt 1604/1605-prov).
/// Alt 1604/1605 gäller vid `lutning >= 2` (mätningens fall); lutning 1 oförändrad.
fn spela_till(
    bas: &Karta,
    tt: &Tidtabell,
    fro: u64,
    flodriktning: u8,
    t: f64,
    bonus_m: f32,
    straff_m: f32,
    lopp_mutant: LoppProvMutant,
    lutning: u8,
) -> (
    Vec<Vec<(usize, usize)>>,
    Vec<usize>,
    Vec<(Vec<(usize, usize)>, f64)>,
    Vec<Loppbyte>,
) {
    let dod = tt.last_tid("magnetvandning").unwrap_or(290_000.0);
    let mut kopia = bas.clone();
    for p in &mut kopia.period.varden {
        *p = Period::Aldre;
    }
    let (mut alla, _, _) = kor_mittlinjer(&mut kopia, fro, flodriktning);
    let n_foror = alla.len();
    let lev_ovre = levande_foror_nr(fro, Period::Ovre);
    let mut lever = vec![true; n_foror];
    let mut overgivna: Vec<(Vec<(usize, usize)>, f64)> = Vec::new();
    let mut byten: Vec<Loppbyte> = Vec::new();
    let b = bas.omrade.b;
    let n = b * bas.omrade.h;
    let events = schemalagg_loppbyten(tt, fro);
    let mut dod_klad = false;
    let uppfor_pa = lutning >= 2;
    let max_stigning = if uppfor_pa {
        lopp_mutant.max_stigning_m()
    } else {
        f32::MAX
    };
    // Alt 1605: mötesstopp i produktion (`Ingen`) och via `MedMote`; mutant `MotetAvslaget` stänger av.
    let mote = uppfor_pa && lopp_mutant.mote_pagatt();

    for &(ar, fara, punkt_raw) in &events {
        if ar <= t {
            break;
        }
        // Periodbyte: döda fåror när vi passerar magnetvandningen.
        if !dod_klad && ar < dod {
            for (fi, path) in alla.iter().enumerate() {
                if lever[fi] && !lev_ovre.contains(&fi) {
                    overgivna.push((path.clone(), dod));
                    lever[fi] = false;
                }
            }
            dod_klad = true;
        }
        if !lever[fara] {
            continue;
        }
        let path = &alla[fara];
        if path.len() < 2 {
            continue;
        }
        let pidx = punkt_index(path.len(), punkt_raw);
        let punkt = path[pidx];
        let gammal_nedstrom: Vec<(usize, usize)> = path[pidx + 1..].to_vec();

        let mut bonus = vec![false; n];
        let mut straff = vec![false; n];
        let mut mot_levande = vec![false; n];
        for (fi, p) in alla.iter().enumerate() {
            if fi == fara || !lever[fi] {
                continue;
            }
            markera_path(&mut bonus, p, b);
            if mote {
                markera_path(&mut mot_levande, p, b);
            }
        }
        for (p, _) in &overgivna {
            markera_path(&mut bonus, p, b);
        }
        // Andra delarna av den egna fåran (uppströms) räknas också som "annat lopp"? Nej —
        // underlaget: bonus på *annat* lopp. Egen gammal nedström får straff.
        markera_path(&mut straff, &gammal_nedstrom, b);

        let mut minh = f32::MAX;
        for &(ci, cj) in &path[..=pidx] {
            let h = bas.hojd.varden[idx(b, ci, cj)];
            if h >= 0.0 {
                minh = minh.min(h);
            }
        }
        if !minh.is_finite() {
            minh = 0.0;
        }

        let mut rng = fora_byte_path_rng(fro, fara, ar);
        let mot_ref: Option<&[bool]> = if mote { Some(&mot_levande) } else { None };
        let forts = match fortsatt_fora(
            bas,
            punkt,
            flodriktning,
            &mut rng,
            &bonus,
            &straff,
            bonus_m,
            straff_m,
            FLODRIKTNING_STRAFF_M,
            minh,
            max_stigning,
            mot_ref,
        ) {
            FortsattUt::Avbryt => continue, // bytet sker inte
            FortsattUt::Vag(v) => v,
        };
        let mut ny = path[..=pidx].to_vec();
        if forts.len() > 1 {
            ny.extend_from_slice(&forts[1..]);
        }
        if !gammal_nedstrom.is_empty() {
            overgivna.push((gammal_nedstrom, ar));
        }
        alla[fara] = ny;
        byten.push(Loppbyte {
            ar,
            fara,
            punkt,
        });
    }

    // Om t ≤ dod och vi inte passerat döden via ett loppbyte: tillämpa den nu.
    if !dod_klad && t <= dod {
        for (fi, path) in alla.iter().enumerate() {
            if lever[fi] && !lev_ovre.contains(&fi) {
                overgivna.push((path.clone(), dod));
                lever[fi] = false;
            }
        }
    }

    let period = if t <= 0.0 {
        Period::Gransen
    } else if t <= dod {
        Period::Ovre
    } else {
        Period::Aldre
    };
    let levande = levande_foror_nr(fro, period);
    // Säkerställ att levande-masken stämmer (döda ska redan vara false).
    for fi in 0..n_foror {
        if !levande.contains(&fi) && lever[fi] {
            overgivna.push((alla[fi].clone(), dod));
            lever[fi] = false;
        }
    }
    let linjer: Vec<Vec<(usize, usize)>> = levande.iter().map(|&i| alla[i].clone()).collect();
    (linjer, levande, overgivna, byten)
}

/// Mittlinjer (levande) vid år `t` efter uppspelade loppbyten.
pub fn mittlinjer_vid_ar(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodriktning: u8,
) -> Result<(Vec<Vec<(usize, usize)>>, Vec<usize>), String> {
    let bas = karta_med(k, tt, omr, t)?;
    let (linjer, lev, _, _) = spela_till(
        &bas,
        tt,
        fro,
        flodriktning,
        t,
        LOPP_BONUS_M,
        LOPP_STRAFF_M,
        LoppProvMutant::Ingen,
        1, // mittlinjer_vid_ar: PaleoDEM, ingen Alt 1604/1605
    );
    Ok((linjer, lev))
}

/// Alla loppbyten från basen till gränsen (år, fåra, punkt).
pub fn loppbyten(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    fro: u64,
    _flodversion: u8,
    flodriktning: u8,
) -> Result<Vec<Loppbyte>, String> {
    let bas = karta_med(k, tt, omr, 0.0)?;
    let (_, _, _, byten) = spela_till(
        &bas,
        tt,
        fro,
        flodriktning,
        0.0,
        LOPP_BONUS_M,
        LOPP_STRAFF_M,
        LoppProvMutant::Ingen,
        1, // loppbyten: PaleoDEM-bas, schemaläggning oförändrad (lutning 1)
    );
    Ok(byten)
}

/// Andel av nya loppens rutor (nedströms punkten) som ligger på ett tidigare lopp,
/// medel över byten för fröna `fro_lo..=fro_hi` med given bonus.
pub fn andel_nytt_lopp_pa_tidigare(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    fro_lo: u64,
    fro_hi: u64,
    flodriktning: u8,
    bonus_m: f32,
) -> Result<f64, String> {
    let bas = karta_med(k, tt, omr, 500_000.0)?;
    let b = omr.b;
    let n = b * omr.h;
    let mut sum_andel = 0.0;
    let mut n_byten = 0usize;
    for fro in fro_lo..=fro_hi {
        // Spela med vald bonus; mät vid varje byte.
        let dod = tt.last_tid("magnetvandning").unwrap_or(290_000.0);
        let mut kopia = bas.clone();
        for p in &mut kopia.period.varden {
            *p = Period::Aldre;
        }
        let (mut alla, _, _) = kor_mittlinjer(&mut kopia, fro, flodriktning);
        let n_foror = alla.len();
        let lev_ovre = levande_foror_nr(fro, Period::Ovre);
        let mut lever = vec![true; n_foror];
        // Alt 1496 ingår inte: fyllda delar ligger kvar som tidigare lopp (bonus 8 m).
        let mut overgivna: Vec<(Vec<(usize, usize)>, f64)> = Vec::new();
        let events = schemalagg_loppbyten(tt, fro);
        let mut dod_klad = false;
        for &(ar, fara, punkt_raw) in &events {
            if !dod_klad && ar < dod {
                for (fi, path) in alla.iter().enumerate() {
                    if lever[fi] && !lev_ovre.contains(&fi) {
                        overgivna.push((path.clone(), dod));
                        lever[fi] = false;
                    }
                }
                dod_klad = true;
            }
            if !lever[fara] {
                continue;
            }
            let path = &alla[fara];
            if path.len() < 2 {
                continue;
            }
            let pidx = punkt_index(path.len(), punkt_raw);
            let punkt = path[pidx];
            let gammal_nedstrom: Vec<(usize, usize)> = path[pidx + 1..].to_vec();
            let mut bonus = vec![false; n];
            let mut straff = vec![false; n];
            for (fi, p) in alla.iter().enumerate() {
                if fi == fara || !lever[fi] {
                    continue;
                }
                markera_path(&mut bonus, p, b);
            }
            for (p, _) in &overgivna {
                markera_path(&mut bonus, p, b);
            }
            markera_path(&mut straff, &gammal_nedstrom, b);
            let mut minh = f32::MAX;
            for &(ci, cj) in &path[..=pidx] {
                let hh = bas.hojd.varden[idx(b, ci, cj)];
                if hh >= 0.0 {
                    minh = minh.min(hh);
                }
            }
            if !minh.is_finite() {
                minh = 0.0;
            }
            let mut rng = fora_byte_path_rng(fro, fara, ar);
            let forts = match fortsatt_fora(
                &bas,
                punkt,
                flodriktning,
                &mut rng,
                &bonus,
                &straff,
                bonus_m,
                LOPP_STRAFF_M,
                FLODRIKTNING_STRAFF_M,
                minh,
                f32::MAX, // andel_nytt: lutning 1 / PaleoDEM — ingen stigningsgräns
                None,
            ) {
                FortsattUt::Avbryt => continue,
                FortsattUt::Vag(v) => v,
            };
            let nytt: Vec<(usize, usize)> = if forts.len() > 1 {
                forts[1..].to_vec()
            } else {
                Vec::new()
            };
            if !nytt.is_empty() {
                let pa = nytt
                    .iter()
                    .filter(|&&(i, j)| bonus[idx(b, i, j)])
                    .count();
                sum_andel += pa as f64 / nytt.len() as f64;
                n_byten += 1;
            }
            let mut ny = path[..=pidx].to_vec();
            if forts.len() > 1 {
                ny.extend_from_slice(&forts[1..]);
            }
            if !gammal_nedstrom.is_empty() {
                overgivna.push((gammal_nedstrom, ar));
            }
            alla[fara] = ny;
        }
    }
    if n_byten == 0 {
        return Ok(0.0);
    }
    Ok(sum_andel / n_byten as f64)
}

/// Närmaste t' &lt; t där perioden byts, ett lopp byts, eller en fyllning blir klar (Alt 1494).
pub fn nasta_andring_med_lopp(tt: &Tidtabell, t: f64, fro: u64) -> Option<f64> {
    let mut kandidater = Vec::new();
    for namn in ["magnetvandning", "gransen"] {
        if let Some(tg) = tt.last_tid(namn) {
            if tg < t {
                kandidater.push(tg);
            }
        }
    }
    // Alt 1401: tre fåror dör vid magnetvandningen → fyllning klar 1 000 år senare.
    if let Some(dod) = tt.last_tid("magnetvandning") {
        let klar = dod - FYLLNADSTID_AR;
        if klar < t {
            kandidater.push(klar);
        }
    }
    for (ar, _, _) in schemalagg_loppbyten(tt, fro) {
        if ar < t {
            kandidater.push(ar);
        }
        let klar = ar - FYLLNADSTID_AR;
        if klar < t {
            kandidater.push(klar);
        }
    }
    kandidater
        .into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

/// Lägger fåror som objekt (Alt 1401 + 1395 + 1494): historik från basen med loppbyten;
/// från Ovre lever två; övergivna mittlinjerutor blir Trask i `fyllnadstid` år, sedan land.
/// `mutant` går genom `trask_val` (Alt 1584); produktionsvägen skickar alltid `Ingen`.
fn lagg_foror_som_objekt(
    k: &mut Karta,
    tt: &Tidtabell,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    fyllnadstid: f64,
    lutning: u8,
    mutant: TraskProvMutant,
    lopp_mutant: LoppProvMutant,
) -> Option<FlodAndelar> {
    if k.omrade.h == 0 {
        k.vattentyp.marke = Marke::Simulering;
        k.vatten_yta.marke = Marke::Simulering;
        return None;
    }
    let period = k
        .period
        .varden
        .first()
        .copied()
        .unwrap_or(Period::Aldre);
    // Alt 1643: träskbälte 2 i Aldre/Ovre bara i tidsmodell 2 med lutning ≥ 2.
    // `flodregler` oförändrad (tidsmodell 1 / lutning 1 / Gransen behåller 3).
    let mut regler = flodregler(period);
    if lutning >= 2 {
        match period {
            Period::Aldre | Period::Ovre => regler.trask_avstand = 2,
            Period::Gransen => {}
        }
    }
    let t = k.t;

    // Sjöarnas rng: samma som efter initial kor_mittlinjer (orörd av loppbyten).
    let mut bas_rng = k.clone();
    for p in &mut bas_rng.period.varden {
        *p = Period::Aldre;
    }
    let (_, _, mut rng) = kor_mittlinjer(&mut bas_rng, fro, flodriktning);

    let (linjer, levande, overgivna, _) = spela_till(
        k,
        tt,
        fro,
        flodriktning,
        t,
        LOPP_BONUS_M,
        LOPP_STRAFF_M,
        lopp_mutant,
        lutning,
    );

    let andelar = if flodversion >= 2 {
        Some(lagg_version2_urval(
            k, &linjer, &levande, &overgivna, &regler, &mut rng, fro, fyllnadstid, lutning,
            mutant,
            lopp_mutant,
        ))
    } else {
        let bredd = regler.bredd_rutor.max(1);
        for path in &linjer {
            for &(i, j) in path {
                carve_med_bredd(k, i, j, bredd);
            }
        }
        let b = k.omrade.b;
        let n = b * k.omrade.h;
        let mut levande_mitt = vec![false; n];
        for path in &linjer {
            for &(i, j) in path {
                levande_mitt[idx(b, i, j)] = true;
            }
        }
        if lutning >= 2 {
            let foror_h = if lopp_mutant.propagera_delad_yta() {
                fallande_yta_mittlinjer(k, &linjer)
            } else {
                fallande_yta_mittlinjer_mote(k, &linjer)
            };
            let mut skip = vec![false; n];
            for j in 0..k.omrade.h {
                for i in 0..b {
                    let p = idx(b, i, j);
                    if k.vattentyp.varden[p] == Vatten::Flodfara {
                        skip[p] = true;
                    }
                }
            }
            let (trask_yta, avstand) = trask_val(lutning, regler.trask_avstand, mutant);
            lagg_trask_fran(k, &foror_h, avstand, &skip, trask_yta);
        } else {
            let avstand = match mutant {
                TraskProvMutant::DubbeltAvstand => {
                    regler.trask_avstand.saturating_mul(2).max(1)
                }
                TraskProvMutant::Avstand(a) => a,
                _ => regler.trask_avstand,
            };
            lagg_trask(k, avstand);
        }
        lagg_sjoar(k, &regler, &mut rng);
        lagg_overgivna_trask(k, &overgivna, &levande_mitt, t, fyllnadstid);
        None
    };
    k.vattentyp.marke = Marke::Simulering;
    k.vatten_yta.marke = Marke::Simulering;
    andelar
}

/// Mittlinjerna som pathfinding ger för fröet (riktning 1 = oförändrat beteende).
/// Kartan ska vara utan flodnät (t.ex. efter `karta_med`); den klonas internt.
pub fn mittlinjer(k: &Karta, fro: u64) -> Vec<Vec<(usize, usize)>> {
    mittlinjer_riktning(k, fro, 1)
}

/// Mittlinjer med valt `flodriktning` (1 eller 2).
pub fn mittlinjer_riktning(k: &Karta, fro: u64, flodriktning: u8) -> Vec<Vec<(usize, usize)>> {
    let mut kopia = k.clone();
    let (linjer, _, _) = kor_mittlinjer(&mut kopia, fro, flodriktning);
    linjer
}

/// Mittlinjer med valt riktning och söderstraff (för mätning av k och mutanter i proven).
pub fn mittlinjer_riktning_straff(
    k: &Karta,
    fro: u64,
    flodriktning: u8,
    straff_k: f32,
) -> Vec<Vec<(usize, usize)>> {
    let mut kopia = k.clone();
    let (linjer, _, _) = kor_mittlinjer_med_straff(&mut kopia, fro, flodriktning, straff_k);
    linjer
}

/// Aldres mittlinjer för fröet (hela uppsättningen fåror 0..), oberoende av kartans period (riktning 1).
pub fn mittlinjer_aldre(k: &Karta, fro: u64) -> Vec<Vec<(usize, usize)>> {
    mittlinjer_aldre_riktning(k, fro, 1)
}

/// Aldres mittlinjer med valt `flodriktning`.
pub fn mittlinjer_aldre_riktning(k: &Karta, fro: u64, flodriktning: u8) -> Vec<Vec<(usize, usize)>> {
    let mut kopia = k.clone();
    for p in &mut kopia.period.varden {
        *p = Period::Aldre;
    }
    let (linjer, _, _) = kor_mittlinjer(&mut kopia, fro, flodriktning);
    linjer
}

/// Kompassriktning (°) från fårans första till sista ruta: 0 = norr, 90 = öst, 180 = söder.
/// Rad j ökar söderut (samma mätsätt som `kartriktning-fore.rs`).
pub fn fora_riktning_grader(path: &[(usize, usize)]) -> Option<f64> {
    if path.len() < 2 {
        return None;
    }
    let (i0, j0) = path[0];
    let (i1, j1) = *path.last().unwrap();
    let di = i1 as f64 - i0 as f64;
    let dj = j1 as f64 - j0 as f64;
    Some(90.0 + dj.atan2(di).to_degrees())
}

/// Medelriktning (°), median och spann över fröna `fro_lo..=fro_hi` (periodens mittlinjer).
pub fn mat_riktning_statistik(
    k: &Karta,
    fro_lo: u64,
    fro_hi: u64,
    flodriktning: u8,
) -> (f64, f64, f64, f64, usize) {
    mat_riktning_statistik_straff(k, fro_lo, fro_hi, flodriktning, FLODRIKTNING_STRAFF_M)
}

/// Som `mat_riktning_statistik` men med valbart söderstraff.
pub fn mat_riktning_statistik_straff(
    k: &Karta,
    fro_lo: u64,
    fro_hi: u64,
    flodriktning: u8,
    straff_k: f32,
) -> (f64, f64, f64, f64, usize) {
    let mut alla = Vec::new();
    for fro in fro_lo..=fro_hi {
        for p in mittlinjer_riktning_straff(k, fro, flodriktning, straff_k) {
            if let Some(r) = fora_riktning_grader(&p) {
                alla.push(r);
            }
        }
    }
    alla.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = alla.len();
    if n == 0 {
        return (0.0, 0.0, 0.0, 0.0, 0);
    }
    let medel = alla.iter().sum::<f64>() / n as f64;
    let median = alla[n / 2];
    (medel, median, alla[0], alla[n - 1], n)
}

/// Väljer minsta k i `FLODRIKTNING_K_KANDIDATER` där medelriktning frö 1–50 Aldre ∈ [125, 140].
/// Returnerar `(valt_k, medel_per_k)`. Panik om ingen k duger (underlaget: stoppa och redovisa).
pub fn valj_riktning_straff(k_aldre: &Karta) -> (f32, Vec<(f32, f64)>) {
    let mut medel_per_k = Vec::new();
    let mut valt: Option<f32> = None;
    for &k in &FLODRIKTNING_K_KANDIDATER {
        let (medel, _, _, _, _) = mat_riktning_statistik_straff(k_aldre, 1, 50, 2, k);
        medel_per_k.push((k, medel));
        if valt.is_none() && (FLODRIKTNING_MAL_MIN..=FLODRIKTNING_MAL_MAX).contains(&medel) {
            valt = Some(k);
        }
    }
    let valt = valt.unwrap_or_else(|| {
        panic!(
            "ingen k i {:?} ger medelriktning i {}–{}°; medel per k: {:?}",
            FLODRIKTNING_K_KANDIDATER.to_vec(),
            FLODRIKTNING_MAL_MIN,
            FLODRIKTNING_MAL_MAX,
            medel_per_k
        )
    });
    (valt, medel_per_k)
}

/// Lägger flodnätet i kartan med fröet: fårorna börjar i västra kanten och går ruta för ruta österut, varje steg till den av de tre grannarna
/// österut (nordost, öst, sydost) som är lägst enligt höjdlagret, med en slumpdragning ur fröet vid nästan lika höjd; fåran sänker vatten_yta till
/// höjden − 2 m och sätts till Vatten::Flodfara; rutor inom trask_avstand från en fåra och lägre än fåran + 1 m blir Vatten::Trask; sjöar läggs i
/// låga rutor nära fårorna (Vatten::Sjo). Hav (höjd < 0) ändras aldrig. Vattentyps- och vatten_yta-lagret får märket Simulering.
/// Alltid flodriktning 1 (hcsim oförändrad).
pub fn lagg_flodnat(k: &mut Karta, fro: u64) {
    let _ = lagg_flodnat_version(k, fro, 1, 1);
}

/// Som `lagg_flodnat`, men med `flodversion` 1/2 och `flodriktning` 1/2 (lutning 1).
/// Version 2 returnerar `Some(FlodAndelar)`; version 1 returnerar `None`.
pub fn lagg_flodnat_version(
    k: &mut Karta,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
) -> Option<FlodAndelar> {
    lagg_flodnat_version_lutning(k, fro, flodversion, flodriktning, 1)
}

/// Som `lagg_flodnat_version`, med `lutning` 1 (oförändrat) eller 2 (fallande vattenyta / träsk mot
/// närmaste mittlinjeyta, Alt 1501). Höjdkorrektionen (`applicera_lutning_plan`) ska redan vara
/// gjord när `lutning == 2`.
pub fn lagg_flodnat_version_lutning(
    k: &mut Karta,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
) -> Option<FlodAndelar> {
    lagg_flodnat_version_lutning_prov(k, fro, flodversion, flodriktning, lutning, TraskProvMutant::Ingen)
}

/// Som `lagg_flodnat_version_lutning`, med valbar träskmutant (Alt 1533-prov). Mutanten går
/// genom samma `lagg_trask_fran` som kartan — inte en kopia i mätprogrammet.
pub fn lagg_flodnat_version_lutning_prov(
    k: &mut Karta,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
    mutant: TraskProvMutant,
) -> Option<FlodAndelar> {
    if k.omrade.h == 0 {
        k.vattentyp.marke = Marke::Simulering;
        k.vatten_yta.marke = Marke::Simulering;
        return None;
    }
    let (linjer, regler, mut rng) = kor_mittlinjer(k, fro, flodriktning);
    let andelar = if flodversion >= 2 {
        Some(lagg_version2_mutant(
            k, &linjer, &regler, &mut rng, fro, lutning, mutant,
        ))
    } else if lutning >= 2 {
        let foror_h = fallande_yta_mittlinjer(k, &linjer);
        let b = k.omrade.b;
        let n = b * k.omrade.h;
        let mut skip = vec![false; n];
        for j in 0..k.omrade.h {
            for i in 0..b {
                let p = idx(b, i, j);
                if k.vattentyp.varden[p] == Vatten::Flodfara {
                    skip[p] = true;
                }
            }
        }
        let (trask_yta, avstand) = trask_val(lutning, regler.trask_avstand, mutant);
        lagg_trask_fran(k, &foror_h, avstand, &skip, trask_yta);
        lagg_sjoar(k, &regler, &mut rng);
        None
    } else {
        let avstand = match mutant {
            TraskProvMutant::DubbeltAvstand => regler.trask_avstand.saturating_mul(2).max(1),
            TraskProvMutant::Avstand(a) => a,
            _ => regler.trask_avstand,
        };
        lagg_trask(k, avstand);
        lagg_sjoar(k, &regler, &mut rng);
        None
    };
    k.vattentyp.marke = Marke::Simulering;
    k.vatten_yta.marke = Marke::Simulering;
    andelar
}

/// Som karta_med, och med flodnätet för periodens regler och fröet (alltid version 1 och riktning 1 — hcsim oförändrad).
pub fn karta_full(k: &Kalla, tt: &Tidtabell, omr: &Omrade, t: f64, fro: u64) -> Result<Karta, String> {
    let mut kart = karta_med(k, tt, omr, t)?;
    lagg_flodnat(&mut kart, fro);
    Ok(kart)
}

/// Som `karta_full`, men med valt `flodversion` (1 eller 2) och `flodriktning` (1 eller 2).
/// Version 2 ger även andelslagren. `karta_full` (hcsim) anropar alltid riktning 1.
/// Alltid lutning 1 (oförändrad höjd/vattenyta).
pub fn karta_full_version(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_full_version_lutning(k, tt, omr, t, fro, flodversion, flodriktning, 1)
}

/// Som `karta_full_version`, med `lutning` 1 eller 2 (Alt 1485).
pub fn karta_full_version_lutning(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    let mut kart = karta_med(k, tt, omr, t)?;
    if lutning >= 2 {
        applicera_lutning_plan(&mut kart, LUTNING2_M_PER_KM, LUTNING2_RIKTNING_GRADER)?;
    }
    let andelar =
        lagg_flodnat_version_lutning(&mut kart, fro, flodversion, flodriktning, lutning);
    Ok((kart, andelar))
}

/// Kartan för år `t` med fåror som objekt (Alt 1401 / 1395 / 1494 / tidsmodell 2):
/// historik från basen med loppbyten; från Ovre lever två; övergivna blir träsk i
/// [`FYLLNADSTID_AR`] år, därefter land. Lutning 1.
pub fn karta_for_ar(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_lutning(k, tt, omr, t, fro, flodversion, flodriktning, 1)
}

/// Som [`karta_for_ar`], med `lutning` 1 eller 2.
pub fn karta_for_ar_lutning(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_lutning_prov(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        lutning,
        TraskProvMutant::Ingen,
    )
}

/// Som [`karta_for_ar_lutning`], med valbar träskmutant (Alt 1584-prov). Mutanten går
/// genom `trask_val` i tidsmodell 2:s båda vägar (`lagg_version2_urval` och
/// `lagg_foror_som_objekt` med lutning ≥ 2). Produktionsvägen anropar alltid `Ingen`.
pub fn karta_for_ar_lutning_prov(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
    mutant: TraskProvMutant,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_lutning_lopp_prov(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        lutning,
        mutant,
        LoppProvMutant::Ingen,
    )
}

/// Som [`karta_for_ar_lutning_prov`], med valbar loppmutant (Alt 1604/1605-prov).
pub fn karta_for_ar_lutning_lopp_prov(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    lutning: u8,
    trask_mutant: TraskProvMutant,
    lopp_mutant: LoppProvMutant,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_fyllnad_lutning_lopp_prov(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        FYLLNADSTID_AR,
        lutning,
        trask_mutant,
        lopp_mutant,
    )
}

/// Som [`karta_for_ar`], men med vald fyllnadstid (för känslighetsredovisning, Alt 1495). Lutning 1.
pub fn karta_for_ar_fyllnad(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    fyllnadstid_ar: f64,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_fyllnad_lutning(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        fyllnadstid_ar,
        1,
    )
}

/// Som [`karta_for_ar_fyllnad`], med `lutning` 1 eller 2.
pub fn karta_for_ar_fyllnad_lutning(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    fyllnadstid_ar: f64,
    lutning: u8,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_fyllnad_lutning_prov(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        fyllnadstid_ar,
        lutning,
        TraskProvMutant::Ingen,
    )
}

/// Som [`karta_for_ar_fyllnad_lutning`], med valbar träskmutant (Alt 1584-prov).
pub fn karta_for_ar_fyllnad_lutning_prov(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    fyllnadstid_ar: f64,
    lutning: u8,
    mutant: TraskProvMutant,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    karta_for_ar_fyllnad_lutning_lopp_prov(
        k,
        tt,
        omr,
        t,
        fro,
        flodversion,
        flodriktning,
        fyllnadstid_ar,
        lutning,
        mutant,
        LoppProvMutant::Ingen,
    )
}

/// Som [`karta_for_ar_fyllnad_lutning_prov`], med valbar loppmutant (Alt 1604/1605-prov).
pub fn karta_for_ar_fyllnad_lutning_lopp_prov(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodversion: u8,
    flodriktning: u8,
    fyllnadstid_ar: f64,
    lutning: u8,
    trask_mutant: TraskProvMutant,
    lopp_mutant: LoppProvMutant,
) -> Result<(Karta, Option<FlodAndelar>), String> {
    let mut kart = karta_med(k, tt, omr, t)?;
    if lutning >= 2 {
        applicera_lutning_plan(&mut kart, LUTNING2_M_PER_KM, LUTNING2_RIKTNING_GRADER)?;
    }
    let andelar = lagg_foror_som_objekt(
        &mut kart,
        tt,
        fro,
        flodversion,
        flodriktning,
        fyllnadstid_ar,
        lutning,
        trask_mutant,
        lopp_mutant,
    );
    Ok((kart, andelar))
}

/// Provhjälp Alt 1604: nytt lopp från `start` med bonus/straff och stigningsgräns.
/// `uppstrom` = fårans rutor från start till och med bytespunkten (lägsta höjd).
/// `None` = bytet avbryts (ingen tillåten väg inom gränsen).
pub fn prova_nytt_lopp(
    k: &Karta,
    start: (usize, usize),
    flodriktning: u8,
    rng_fro: u64,
    bonus: &[bool],
    straff: &[bool],
    uppstrom: &[(usize, usize)],
    max_stigning_m: f32,
) -> Option<Vec<(usize, usize)>> {
    let b = k.omrade.b;
    let mut minh = f32::MAX;
    for &(ci, cj) in uppstrom {
        let h = k.hojd.varden[idx(b, ci, cj)];
        if h >= 0.0 {
            minh = minh.min(h);
        }
    }
    if !minh.is_finite() {
        minh = 0.0;
    }
    let mut rng = Slump::ny(rng_fro);
    match fortsatt_fora(
        k,
        start,
        flodriktning,
        &mut rng,
        bonus,
        straff,
        LOPP_BONUS_M,
        LOPP_STRAFF_M,
        FLODRIKTNING_STRAFF_M,
        minh,
        max_stigning_m,
        None,
    ) {
        FortsattUt::Avbryt => None,
        FortsattUt::Vag(v) => Some(v),
    }
}

/// Största stigning (m) längs `path` relativt lägsta höjd uppströms (samma formel som kartstd-diag).
pub fn max_stigning_pa_linje(k: &Karta, path: &[(usize, usize)]) -> f32 {
    let b = k.omrade.b;
    let mut minh = f32::MAX;
    let mut max_s = 0.0_f32;
    for &(i, j) in path {
        let h = k.hojd.varden[idx(b, i, j)];
        if h < 0.0 {
            continue;
        }
        minh = minh.min(h);
        max_s = max_s.max(h - minh);
    }
    max_s
}

/// Mittlinjer vid år `t` i tidsmodell 2 på samma höjdkarta som `karta_for_ar_lutning`
/// (mätkopia av `mat_linjer_tm2` i kartstd-diag).
pub fn mat_linjer_tm2(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodriktning: u8,
    lutning: u8,
) -> Result<(Karta, Vec<Vec<(usize, usize)>>), String> {
    let (kart, linjer, _) =
        mat_linjer_tm2_prov(k, tt, omr, t, fro, flodriktning, lutning, LoppProvMutant::Ingen)?;
    Ok((kart, linjer))
}

/// Som [`mat_linjer_tm2`], med valbar loppmutant (för före/efter-mätning och Alt 1605-prov).
/// Returnerar även byten skedda vid `t` (år &gt; t) på samma höjdkarta.
pub fn mat_linjer_tm2_prov(
    k: &Kalla,
    tt: &Tidtabell,
    omr: &Omrade,
    t: f64,
    fro: u64,
    flodriktning: u8,
    lutning: u8,
    lopp_mutant: LoppProvMutant,
) -> Result<(Karta, Vec<Vec<(usize, usize)>>, Vec<Loppbyte>), String> {
    let mut kart = karta_med(k, tt, omr, t)?;
    if lutning >= 2 {
        applicera_lutning_plan(&mut kart, LUTNING2_M_PER_KM, LUTNING2_RIKTNING_GRADER)?;
    }
    let (linjer, _, _, byten) = spela_till(
        &kart,
        tt,
        fro,
        flodriktning,
        t,
        LOPP_BONUS_M,
        LOPP_STRAFF_M,
        lopp_mutant,
        lutning,
    );
    Ok((kart, linjer, byten))
}
