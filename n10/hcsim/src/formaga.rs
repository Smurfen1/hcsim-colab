//! Förmågor för hjärnprovet (Alt 1200–1202), diagnosen (Alt 1207–1209) och avläsarprovet (Alt 1213).

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
use crate::tal::{self, A_N, LASARE_ETA, TABELL_SLUMP_P, TABELL_VIKT, Z_N};

/// En grupps beteendeförmåga efter märkningen. Före märkningen har alla `Lar`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Formaga {
    /// Hjärnan väljer; förutsägaren lär sig.
    Lar,
    /// Hjärnan väljer; förutsägaren lär sig inte (fryst från märkningen; ungar från generna).
    Frusen,
    /// En av 9 handlingar jämnt ur djurets egen slumpföljd; betalar hjärnpris.
    Slump,
    /// Minnestabell över 9 handlingar; betalar hjärnpris.
    Tabell,
    /// Går till rutan med mest växter bland de 9; lika → stå kvar; betalar hjärnpris.
    Allvetande,
    /// Som `Lar`, men grundkostnaden gånger x (hjärnpriset oförändrat).
    Rabatt(f64),
    /// Bland de 9: mest växter / (andra djur + 1); lika → stå kvar.
    VaxtPerDjur,
    /// Allvetande, men med sannolikheten `p` en slumpvis handling.
    AllvetandeSlump(f64),
    /// Lärande hjärna, men med sannolikheten `p` en slumpvis handling.
    LarSlump(f64),
    /// Lärande hjärna med ärvd utforskning ε (Alt 1463): med sannolikheten ε
    /// jämnt slumpad handling; förutsägaren lär sig av den valda handlingen.
    LarUtforsk,
    /// Som `LarUtforsk` men med fast sannolikhet `p` i stället för den ärvda ε (Alt 1463, Claudes tillägg:
    /// ren jämförelse utan arv; lär sig av slumpvalet som `LarUtforsk`, till skillnad från `LarSlump`).
    LarUtforskFast(f64),
    /// Minnestabell med p = 0 (provar aldrig slumpvis).
    TabellUtanSlump,
    /// Som `Lar`, men lika värden lottas (djurets egen slumpföljd).
    LarLott,
    /// Förutsägare som `Lar`; handlingen väljs av intagsläsare `u · y + u0` (lika lottas).
    LarIntagslasare,
    /// Ärvd avläsare `v` värderar den sanna kodade bilden i målrutan (lika lottas).
    OrakelForutsagare,
    /// Ur egna sinnen: `v_andel / (n + 1)`; lika lottas (Alt 1230).
    TrangselOrakel,
    /// Alt 2059: skärpt blind KanalGrind + kausal z-norm + buffertfri RLS + fryst ärvd kodare (Regel 14).
    Lar2059,
    /// Alt 2059 med all vikt-, norm- och grindinlärning fryst vid märkningen.
    Frusen2059,
    /// Alt 2059 som stryker handlingens verkan vid val (behåller lärandet).
    Stryk2059,
    /// Alt 2073: samma som `Lar2059`, plus fast slump-utforskning `eps` (hjärnan lär sig av slumpvalet).
    Lar2073(f64),
    /// Alt 2116: samma som `Lar2059`, plus riktad blind utforskning mot handlingen med lägst `grind_n`.
    Lar2116Minst(f64),
    /// Alt 2144: samma som `Lar2116Minst(eps)`, men med 9 fristående regulariserade RLS-modeller (D2, alpha = alpha_tio / 10).
    Lar2144D2(f64, u16),
    /// Alt 2172: samma som `Lar2144D2`, med `norm_lage` (1=adaptiv centrerad, 2=RMS utan centrering, 3=fryst z efter 200 steg).
    Lar2172Norm(f64, u16, u8),
    /// Alt 2199: samma som `Lar2144D2`, men med tillståndsberoende RLS-osäkerhetsutforskning (`osak_lage`: 0=fast takt, 1=åldersavtagande, 2=UCB).
    Lar2199Osak(f64, u16, u8),
    /// Alt 2223 strykprov: samma som `Lar2144D2(eps, alpha_tio)` men med världsmodellen avstängd i handlingsvalet (`stryk_handling_i_val = true`).
    Stryk2144D2(f64, u16),
    /// Alt 2223 arvskontroll: samma som `Lar2144D2(eps, alpha_tio)` men ungar får slumpade gener vid födseln i stället för arv.
    Lar2144D2SlumpGen(f64, u16),
    /// Alt 2267: `Lar2144D2(eps, alpha_tio)` + LeCuns lärda kritiker (ärvd startinstinkt `v` + livstids-RLS mot egen netto-ΔE).
    Lar2267Krit(f64, u16),
    /// Alt 2267 strykprov: exakt samma lärda kritiker som `Lar2267Krit`, men världsmodellen avstängd i handlingsvalet.
    Stryk2267Krit(f64, u16),
    /// Alt 2282 (Skede B, B1): 29-kanalig JEPA + lärd kritiker + ärvd skadeviktsgen `w_skada`.
    LarB1Krit(f64, u16),
    /// Alt 2282 strykprov: exakt samma som `LarB1Krit`, men världsmodellen avstängd i handlingsvalet.
    StrykB1Krit(f64, u16),
    /// Alt 2282 / Alt 2277: samma världsmodell som `LarB1Krit`, men livstidskritikern fryst (`w_krit = 0`, bara ärvd `v_0`).
    FrystKritB1(f64, u16),
    /// Alt 2282 / Alt 2277 / Alt 1882: samma som `LarB1Krit`, men med `w_skada ≡ 0` (ingen smärtsignal).
    LarB1UtanSkada(f64, u16),
    /// Alt 2282 (Regel 14): 9-handlings minnestabell som får exakt samma `r_b1 = ΔE − w_skada · skada`-signal.
    TabellB1,
    /// Enkel stillastående-baslinje utan hjärna (Regel 14): väljer `a = 0` med sannolikhet `p`, annars jämnt bland `1..8`.
    StannaSlump(f64),
}

impl Formaga {
    pub fn som_str(self) -> String {
        match self {
            Formaga::Lar => "lar".into(),
            Formaga::Frusen => "frusen".into(),
            Formaga::Slump => "slump".into(),
            Formaga::Tabell => "tabell".into(),
            Formaga::Allvetande => "allvetande".into(),
            Formaga::Rabatt(x) => format!("rabatt({x})"),
            Formaga::VaxtPerDjur => "vaxt_per_djur".into(),
            Formaga::AllvetandeSlump(p) => format!("allvetande_slump({p})"),
            Formaga::LarSlump(p) => format!("lar_slump({p})"),
            Formaga::LarUtforsk => "lar_utforsk".into(),
            Formaga::LarUtforskFast(p) => format!("lar_utforsk_fast({p})"),
            Formaga::TabellUtanSlump => "tabell_utan_slump".into(),
            Formaga::LarLott => "lar_lott".into(),
            Formaga::LarIntagslasare => "lar_intagslasare".into(),
            Formaga::OrakelForutsagare => "orakel_forutsagare".into(),
            Formaga::TrangselOrakel => "trangsel_orakel".into(),
            Formaga::Lar2059 => "lar2059".into(),
            Formaga::Frusen2059 => "frusen2059".into(),
            Formaga::Stryk2059 => "stryk2059".into(),
            Formaga::Lar2073(p) => format!("lar2073({p})"),
            Formaga::Lar2116Minst(p) => format!("lar2116_minst({p})"),
            Formaga::Lar2144D2(p, a) => format!("lar2144_d2({p}, {a})"),
            Formaga::Lar2172Norm(p, a, n) => format!("lar2172_norm({p}, {a}, {n})"),
            Formaga::Lar2199Osak(p, a, l) => format!("lar2199_osak({p}, {a}, {l})"),
            Formaga::Stryk2144D2(p, a) => format!("stryk2144_d2({p}, {a})"),
            Formaga::Lar2144D2SlumpGen(p, a) => format!("lar2144_d2_slumpgen({p}, {a})"),
            Formaga::Lar2267Krit(p, a) => format!("lar2267_krit({p}, {a})"),
            Formaga::Stryk2267Krit(p, a) => format!("stryk2267_krit({p}, {a})"),
            Formaga::LarB1Krit(p, a) => format!("lar_b1_krit({p}, {a})"),
            Formaga::StrykB1Krit(p, a) => format!("stryk_b1_krit({p}, {a})"),
            Formaga::FrystKritB1(p, a) => format!("fryst_krit_b1({p}, {a})"),
            Formaga::LarB1UtanSkada(p, a) => format!("lar_b1_utan_skada({p}, {a})"),
            Formaga::TabellB1 => "tabell_b1".into(),
            Formaga::StannaSlump(p) => format!("stanna_slump({p})"),
        }
    }

    /// True om handlingen tas ur JEPA-hjärnan (med eller utan lärande).
    pub fn anvander_hjarna(self) -> bool {
        matches!(
            self,
            Formaga::Lar
                | Formaga::Frusen
                | Formaga::Rabatt(_)
                | Formaga::LarSlump(_)
                | Formaga::LarUtforsk
                | Formaga::LarUtforskFast(_)
                | Formaga::LarLott
                | Formaga::LarIntagslasare
                | Formaga::OrakelForutsagare
                | Formaga::Lar2059
                | Formaga::Frusen2059
                | Formaga::Stryk2059
                | Formaga::Lar2073(_)
                | Formaga::Lar2116Minst(_)
                | Formaga::Lar2144D2(_, _)
                | Formaga::Lar2172Norm(_, _, _)
                | Formaga::Lar2199Osak(_, _, _)
                | Formaga::Stryk2144D2(_, _)
                | Formaga::Lar2144D2SlumpGen(_, _)
                | Formaga::Lar2267Krit(_, _)
                | Formaga::Stryk2267Krit(_, _)
                | Formaga::LarB1Krit(_, _)
                | Formaga::StrykB1Krit(_, _)
                | Formaga::FrystKritB1(_, _)
                | Formaga::LarB1UtanSkada(_, _)
        )
    }

    pub fn lar_sig(self) -> bool {
        matches!(
            self,
            Formaga::Lar
                | Formaga::Rabatt(_)
                | Formaga::LarSlump(_)
                | Formaga::LarUtforsk
                | Formaga::LarUtforskFast(_)
                | Formaga::LarLott
                | Formaga::LarIntagslasare
                | Formaga::Lar2059
                | Formaga::Stryk2059
                | Formaga::Lar2073(_)
                | Formaga::Lar2116Minst(_)
                | Formaga::Lar2144D2(_, _)
                | Formaga::Lar2172Norm(_, _, _)
                | Formaga::Lar2199Osak(_, _, _)
                | Formaga::Stryk2144D2(_, _)
                | Formaga::Lar2144D2SlumpGen(_, _)
                | Formaga::Lar2267Krit(_, _)
                | Formaga::Stryk2267Krit(_, _)
                | Formaga::LarB1Krit(_, _)
                | Formaga::StrykB1Krit(_, _)
                | Formaga::FrystKritB1(_, _)
                | Formaga::LarB1UtanSkada(_, _)
        )
    }

    pub fn ar_tabell(self) -> bool {
        matches!(self, Formaga::Tabell | Formaga::TabellUtanSlump)
    }

    pub fn ar_tabell_b1(self) -> bool {
        matches!(self, Formaga::TabellB1)
    }

    pub fn cfg_b1(self, r_klamp: f64) -> Option<crate::hjarnab1::HjB1Config> {
        match self {
            Formaga::LarB1Krit(_, alpha_tio) => Some(crate::hjarnab1::HjB1Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: alpha_tio,
                med_kritiker: true,
                anvand_w_skada: true,
                r_klamp,
            }),
            Formaga::StrykB1Krit(_, alpha_tio) => Some(crate::hjarnab1::HjB1Config {
                stryk_handling_i_val: true,
                rls_alpha_tio: alpha_tio,
                med_kritiker: true,
                anvand_w_skada: true,
                r_klamp,
            }),
            Formaga::FrystKritB1(_, alpha_tio) => Some(crate::hjarnab1::HjB1Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: alpha_tio,
                med_kritiker: false,
                anvand_w_skada: true,
                r_klamp,
            }),
            Formaga::LarB1UtanSkada(_, alpha_tio) => Some(crate::hjarnab1::HjB1Config {
                stryk_handling_i_val: false,
                rls_alpha_tio: alpha_tio,
                med_kritiker: true,
                anvand_w_skada: false,
                r_klamp,
            }),
            _ => None,
        }
    }

    pub fn har_intagslasare(self) -> bool {
        matches!(self, Formaga::LarIntagslasare)
    }

    pub fn cfg_2059(self) -> Option<crate::hjarna2008::Hj2008Config> {
        match self {
            Formaga::Lar2059
            | Formaga::Frusen2059
            | Formaga::Stryk2059
            | Formaga::Lar2073(_)
            | Formaga::Lar2116Minst(_) => Some(crate::hjarna2008::Hj2008Config {
                lar_kodare: false,
                med_idm: false,
                additiv_endast: false,
                stryk_handling_i_val: matches!(self, Formaga::Stryk2059),
                med_kanalgrind: true,
                stryk_djurantal_diagnos: false,
                skarp_kanalgrind: true,
                med_z_norm: true,
                med_rls: true,
                rls_fristaende: false,
                rls_alpha_tio: 0,
                norm_lage: 0,
                med_kritiker: false,
            }),
            Formaga::Lar2144D2(_, alpha_tio)
            | Formaga::Lar2199Osak(_, alpha_tio, _)
            | Formaga::Stryk2144D2(_, alpha_tio)
            | Formaga::Lar2144D2SlumpGen(_, alpha_tio) => {
                Some(crate::hjarna2008::Hj2008Config {
                    lar_kodare: false,
                    med_idm: false,
                    additiv_endast: false,
                    stryk_handling_i_val: matches!(self, Formaga::Stryk2144D2(_, _)),
                    med_kanalgrind: true,
                    stryk_djurantal_diagnos: false,
                    skarp_kanalgrind: true,
                    med_z_norm: true,
                    med_rls: true,
                    rls_fristaende: true,
                    rls_alpha_tio: alpha_tio,
                    norm_lage: 0,
                    med_kritiker: false,
                })
            }
            Formaga::Lar2267Krit(_, alpha_tio) | Formaga::Stryk2267Krit(_, alpha_tio) => {
                Some(crate::hjarna2008::Hj2008Config {
                    lar_kodare: false,
                    med_idm: false,
                    additiv_endast: false,
                    stryk_handling_i_val: matches!(self, Formaga::Stryk2267Krit(_, _)),
                    med_kanalgrind: true,
                    stryk_djurantal_diagnos: false,
                    skarp_kanalgrind: true,
                    med_z_norm: true,
                    med_rls: true,
                    rls_fristaende: true,
                    rls_alpha_tio: alpha_tio,
                    norm_lage: 0,
                    med_kritiker: true,
                })
            }
            Formaga::Lar2172Norm(_, alpha_tio, norm_lage) => Some(crate::hjarna2008::Hj2008Config {
                lar_kodare: false,
                med_idm: false,
                additiv_endast: false,
                stryk_handling_i_val: false,
                med_kanalgrind: true,
                stryk_djurantal_diagnos: false,
                skarp_kanalgrind: true,
                med_z_norm: true,
                med_rls: true,
                rls_fristaende: true,
                rls_alpha_tio: alpha_tio,
                norm_lage,
                med_kritiker: false,
            }),
            _ => None,
        }
    }
}

/// Minnestabell: glidande medel av intaget per handling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Minnestabell {
    pub medel: [f64; A_N],
    pub forra_handling: Option<usize>,
    /// Antal gånger den slumpvisa grenen valdes (för provet).
    pub slumphandlingar: u64,
}

impl Default for Minnestabell {
    fn default() -> Self {
        Self::ny()
    }
}

impl Minnestabell {
    pub fn ny() -> Self {
        Self {
            medel: [0.0; A_N],
            forra_handling: None,
            slumphandlingar: 0,
        }
    }

    /// Välj handling: med sannolikhet TABELL_SLUMP_P slumpvis, annars högst medel (lika → 0).
    pub fn valj(&mut self, g: &mut Rng) -> usize {
        self.valj_med_p(g, TABELL_SLUMP_P)
    }

    /// Som `valj`, men med given slumpsannolikhet. Vid `p = 0` dras inget slumptal.
    pub fn valj_med_p(&mut self, g: &mut Rng, p: f64) -> usize {
        if p > 0.0 && g.f() < p {
            self.slumphandlingar += 1;
            return g.under(A_N);
        }
        let mut ba = 0usize;
        let mut bv = f64::NEG_INFINITY;
        for a in 0..A_N {
            if self.medel[a] > bv {
                ba = a;
                bv = self.medel[a];
            }
        }
        ba
    }

    pub fn uppdatera(&mut self, handling: usize, intag: f64) {
        let a = TABELL_VIKT;
        self.medel[handling] = (1.0 - a) * self.medel[handling] + a * intag;
    }
}

/// Linjär intagsläsare `u · y + u0` (Alt 1213). Börjar på 0; ärvs inte.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Intagslasare {
    pub u: [f64; Z_N],
    pub u0: f64,
    /// Senaste valda handlingens `y` (för övning efter betet).
    pub forra_y: Option<Vec<f64>>,
}

impl Default for Intagslasare {
    fn default() -> Self {
        Self::ny()
    }
}

impl Intagslasare {
    pub fn ny() -> Self {
        Self {
            u: [0.0; Z_N],
            u0: 0.0,
            forra_y: None,
        }
    }

    pub fn varde(&self, y: &[f64]) -> f64 {
        assert_eq!(y.len(), Z_N);
        self.u0 + (0..Z_N).map(|j| self.u[j] * y[j]).sum::<f64>()
    }

    /// `u += LASARE_ETA · (intag − (u·y+u0)) · [y, 1]`.
    pub fn uppdatera(&mut self, y: &[f64], intag: f64) {
        assert_eq!(y.len(), Z_N);
        let pred = self.varde(y);
        let fel = intag - pred;
        for j in 0..Z_N {
            self.u[j] += LASARE_ETA * fel * y[j];
        }
        self.u0 += LASARE_ETA * fel;
    }
}

/// En arm i hjärnprovet/diagnosen: märkt grupps förmåga mot omärkt.
/// Serialize/Deserialize via `serdej::hjarn_arm_opt` på `Sim::hjarn_arm` (p.g.a. `&'static str`).
#[derive(Clone, Debug, PartialEq)]
pub struct HjarnArm {
    /// Kort namn: arm1, arm2, d_arm1, tak_a, …
    pub namn: &'static str,
    pub markta: Formaga,
    pub omarkta: Formaga,
    pub spegel: bool,
}

impl HjarnArm {
    pub fn spegel_av(self) -> HjarnArm {
        HjarnArm {
            namn: self.namn,
            markta: self.omarkta,
            omarkta: self.markta,
            spegel: true,
        }
    }
}

/// De tio armarna i körordning (underlaget: arm 2 först).
pub fn alla_armar() -> Vec<HjarnArm> {
    let arm2 = HjarnArm {
        namn: "arm2",
        markta: Formaga::Frusen,
        omarkta: Formaga::Slump,
        spegel: false,
    };
    let arm1 = HjarnArm {
        namn: "arm1",
        markta: Formaga::Lar,
        omarkta: Formaga::Frusen,
        spegel: false,
    };
    let arm3 = HjarnArm {
        namn: "arm3",
        markta: Formaga::Lar,
        omarkta: Formaga::Tabell,
        spegel: false,
    };
    vec![
        arm2.clone(),
        arm1.clone(),
        arm3.clone(),
        arm2.spegel_av(),
        arm1.spegel_av(),
        arm3.spegel_av(),
        HjarnArm {
            namn: "trappa_0.99",
            markta: Formaga::Rabatt(0.99),
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "trappa_0.97",
            markta: Formaga::Rabatt(0.97),
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "trappa_0.95",
            markta: Formaga::Rabatt(0.95),
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "tak",
            markta: Formaga::Allvetande,
            omarkta: Formaga::Slump,
            spegel: false,
        },
    ]
}

/// Diagnosarmarna (Alt 1207–1209): 11 basarmar + speglar.
pub fn diagnos_armar() -> Vec<HjarnArm> {
    let bas = [
        HjarnArm {
            namn: "d_arm2",
            markta: Formaga::Frusen,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "d_arm1",
            markta: Formaga::Lar,
            omarkta: Formaga::Frusen,
            spegel: false,
        },
        HjarnArm {
            namn: "d_arm3",
            markta: Formaga::Lar,
            omarkta: Formaga::Tabell,
            spegel: false,
        },
        HjarnArm {
            namn: "d_tak",
            markta: Formaga::Allvetande,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "tak_a",
            markta: Formaga::VaxtPerDjur,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "tak_b10",
            markta: Formaga::AllvetandeSlump(0.1),
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "tak_b30",
            markta: Formaga::AllvetandeSlump(0.3),
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "tak_b50",
            markta: Formaga::AllvetandeSlump(0.5),
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "u1",
            markta: Formaga::LarSlump(0.1),
            omarkta: Formaga::Tabell,
            spegel: false,
        },
        HjarnArm {
            namn: "u2",
            markta: Formaga::TabellUtanSlump,
            omarkta: Formaga::Tabell,
            spegel: false,
        },
        HjarnArm {
            namn: "u3",
            markta: Formaga::TabellUtanSlump,
            omarkta: Formaga::Lar,
            spegel: false,
        },
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for a in &bas {
        ut.push(a.clone());
    }
    for a in &bas {
        ut.push(a.clone().spegel_av());
    }
    ut
}

/// Avläsarprovets armar (Alt 1213 med 1215): 10 basarmar + speglar.
pub fn avlasare_armar() -> Vec<HjarnArm> {
    let bas = [
        HjarnArm {
            namn: "a_lott",
            markta: Formaga::LarLott,
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "a_las",
            markta: Formaga::LarIntagslasare,
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "a_las_lott",
            markta: Formaga::LarIntagslasare,
            omarkta: Formaga::LarLott,
            spegel: false,
        },
        HjarnArm {
            namn: "a_las_tab",
            markta: Formaga::LarIntagslasare,
            omarkta: Formaga::Tabell,
            spegel: false,
        },
        HjarnArm {
            namn: "a_las_ls",
            markta: Formaga::LarIntagslasare,
            omarkta: Formaga::LarSlump(0.1),
            spegel: false,
        },
        HjarnArm {
            namn: "a_orakel",
            markta: Formaga::OrakelForutsagare,
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "s_lar",
            markta: Formaga::Lar,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "s_tab",
            markta: Formaga::Tabell,
            omarkta: Formaga::Slump,
            spegel: false,
        },
        HjarnArm {
            namn: "s_tab_tak",
            markta: Formaga::Tabell,
            omarkta: Formaga::AllvetandeSlump(0.1),
            spegel: false,
        },
        HjarnArm {
            namn: "s_lar_tak",
            markta: Formaga::Lar,
            omarkta: Formaga::AllvetandeSlump(0.1),
            spegel: false,
        },
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for a in &bas {
        ut.push(a.clone());
    }
    for a in &bas {
        ut.push(a.clone().spegel_av());
    }
    ut
}

/// Hjärnans energipris per steg (samma för alla förmågor).
pub fn hjarnpris() -> f64 {
    crate::hjarna::hjarnkost(tal::KOST_STEG)
}

/// Grundarnas startvärde för ε (Fables val; Alt 1463).
pub const EPS_START: f64 = 0.05;

/// Sådd-xor för ε-arv: blandning + mutation utan att röra världens Rng (djur_summa).
pub const EPS_ARV_XOR: u64 = 0x1463_E951_0A17_0001;

/// Armarna e_lar, e_fast, e_tab + speglar (Alt 1463).
pub fn utforsk_armar() -> Vec<HjarnArm> {
    let bas = [
        HjarnArm {
            namn: "e_lar",
            markta: Formaga::LarUtforsk,
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "e_fast",
            markta: Formaga::LarSlump(EPS_START),
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "e_fast_lar",
            markta: Formaga::LarUtforskFast(EPS_START),
            omarkta: Formaga::Lar,
            spegel: false,
        },
        HjarnArm {
            namn: "e_tab",
            markta: Formaga::LarUtforsk,
            omarkta: Formaga::Tabell,
            spegel: false,
        },
    ];
    let mut ut = Vec::with_capacity(bas.len() * 2);
    for a in &bas {
        ut.push(a.clone());
    }
    for a in &bas {
        ut.push(a.clone().spegel_av());
    }
    ut
}
