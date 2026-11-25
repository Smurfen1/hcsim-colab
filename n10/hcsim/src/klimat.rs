//! Klimatet: kopierat ur `n10/varld/klimat.rs` (Klimat::ny, fas, g, l, vat, ft, medel).
//! Radhänvisningar gäller källfilen 2026-11-21. Inga dalar; samma kurva överallt.

use serde::{Deserialize, Serialize};

use crate::tal;

/// Årsmedlen räknas över fasen i så många lika steg (`klimat.rs` rad 6).
pub const PUNKTER: usize = tal::PUNKTER;
/// Dygn per år i fasräkningen (`klimat.rs` rad 7).
pub const DYGN: f64 = tal::DYGN;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Klimat {
    pub ar: f64,
    pub phi: f64,
    pub eps: f64,
    pub m: f64,
    pub a: f64,
    pub fas_tmax: f64,
    pub vat_vinter: bool,
    pub w_torr: f64,
    pub platt: bool,
    q_medel: f64,
    g_norm: f64,
    pub g_medel: f64,
}

/// 0 vid 0 °C och lägre, 1 vid 20 °C och högre (`klimat.rs` rad 14).
pub fn ft(t: f64) -> f64 {
    (t / 20.0).clamp(0.0, 1.0)
}

impl Klimat {
    /// Som `Klimat::ny` i `klimat.rs` rad 18–27, med fasta tal från `tal` (KLIMAT alltid på).
    pub fn ny(ar_steg: f64) -> Klimat {
        let mut x = Klimat {
            ar: ar_steg,
            phi: tal::BREDD,
            eps: tal::LUTNING,
            m: tal::T_MEDEL,
            a: tal::T_AMPL,
            fas_tmax: 0.5 + tal::T_FORSKJ_DYGN / DYGN,
            vat_vinter: tal::VAT_VINTER > 0.0,
            w_torr: tal::W_TORR,
            platt: tal::PLATT > 0.0,
            q_medel: 1.0,
            g_norm: 1.0,
            g_medel: 1.0,
        };
        x.q_medel = medel(|f| x.q(f));
        x.g_norm = medel(|f| x.l(f) * ft(x.temp(f)));
        x.g_medel = medel(|f| x.g(f));
        x
    }

    /// Fas i året: 0 vid vintersolståndet (`klimat.rs` rad 29).
    pub fn fas(&self, t: i64) -> f64 {
        (t as f64 % self.ar) / self.ar
    }

    pub fn dekl(&self, fas: f64) -> f64 {
        -self.eps.to_radians() * (2.0 * std::f64::consts::PI * fas).cos()
    }

    fn h0(&self, fas: f64) -> f64 {
        (-(self.phi.to_radians().tan()) * self.dekl(fas).tan())
            .clamp(-1.0, 1.0)
            .acos()
    }

    fn q(&self, fas: f64) -> f64 {
        let (p, d, h) = (self.phi.to_radians(), self.dekl(fas), self.h0(fas));
        h * p.sin() * d.sin() + p.cos() * d.cos() * h.sin()
    }

    /// Ljusfaktorn L(t) (`klimat.rs` rad 36).
    pub fn l(&self, fas: f64) -> f64 {
        if self.platt {
            1.0
        } else {
            self.q(fas) / self.q_medel
        }
    }

    pub fn temp(&self, fas: f64) -> f64 {
        self.m + self.a * (2.0 * std::f64::consts::PI * (fas - self.fas_tmax)).cos()
    }

    /// Våt säsong (`klimat.rs` rad 40).
    pub fn vat(&self, fas: f64) -> bool {
        let vinter = fas >= 0.75 || fas < 0.25;
        if self.vat_vinter {
            vinter
        } else {
            !vinter
        }
    }

    pub fn w(&self, fas: f64) -> f64 {
        if self.platt || self.vat(fas) {
            1.0
        } else {
            self.w_torr
        }
    }

    /// Tillväxtfaktorn G(t) (`klimat.rs` rad 42).
    pub fn g(&self, fas: f64) -> f64 {
        if self.platt {
            1.0
        } else {
            self.l(fas) * ft(self.temp(fas)) * self.w(fas) / self.g_norm
        }
    }
}

fn medel(f: impl Fn(f64) -> f64) -> f64 {
    (0..PUNKTER)
        .map(|i| f((i as f64 + 0.5) / PUNKTER as f64))
        .sum::<f64>()
        / PUNKTER as f64
}
