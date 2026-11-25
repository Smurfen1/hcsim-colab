//! Serde-hjälp för typer som inte själva har Serialize (Vatten) eller har `&'static str`.

use hckarta::Vatten;
use serde::de::{Error, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::formaga::{Formaga, HjarnArm};

/// Serde för `Vec<Vatten>` (hckarta har ingen Serialize).
pub mod vatten_lista {
    use super::*;

    pub fn serialize<S>(v: &Vec<Vatten>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = s.serialize_seq(Some(v.len()))?;
        for vt in v {
            seq.serialize_element(&(*vt as u8))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Vec<Vatten>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Vec<Vatten>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "lista av vattentyp som u8")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut ut = Vec::new();
                while let Some(x) = seq.next_element::<u8>()? {
                    ut.push(u8_till_vatten(x).map_err(A::Error::custom)?);
                }
                Ok(ut)
            }
        }
        d.deserialize_seq(V)
    }

    fn u8_till_vatten(x: u8) -> Result<Vatten, String> {
        match x {
            0 => Ok(Vatten::Land),
            1 => Ok(Vatten::Flodfara),
            2 => Ok(Vatten::Trask),
            3 => Ok(Vatten::Sjo),
            4 => Ok(Vatten::Brackt),
            5 => Ok(Vatten::Hav),
            _ => Err(format!("okänd vattentyp {x}")),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct HjarnArmDto {
    namn: String,
    markta: Formaga,
    omarkta: Formaga,
    spegel: bool,
}

fn intern_namn(s: String) -> &'static str {
    if let Some(kand) = KANDA.iter().copied().find(|&k| k == s) {
        return kand;
    }
    Box::leak(s.into_boxed_str())
}

const KANDA: &[&str] = &[
    "arm1",
    "arm2",
    "arm3",
    "trappa_0.99",
    "trappa_0.97",
    "trappa_0.95",
    "tak",
    "d_arm1",
    "d_arm2",
    "d_arm3",
    "d_tak",
    "tak_a",
    "tak_b10",
    "tak_b30",
    "tak_b50",
    "u1",
    "u2",
    "u3",
    "a_lott",
    "a_las",
    "a_las_lott",
    "a_las_tab",
    "a_las_ls",
    "a_orakel",
    "s_lar",
    "s_tab",
    "s_tab_tak",
    "s_lar_tak",
];

/// Serde för `Option<HjarnArm>` (undviker livstid på `&'static str` i derive).
pub mod hjarn_arm_opt {
    use super::*;

    pub fn serialize<S>(v: &Option<HjarnArm>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match v {
            None => s.serialize_none(),
            Some(a) => {
                let dto = HjarnArmDto {
                    namn: a.namn.to_string(),
                    markta: a.markta,
                    omarkta: a.omarkta,
                    spegel: a.spegel,
                };
                s.serialize_some(&dto)
            }
        }
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Option<HjarnArm>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<HjarnArmDto>::deserialize(d)?;
        Ok(opt.map(|dto| HjarnArm {
            namn: intern_namn(dto.namn),
            markta: dto.markta,
            omarkta: dto.omarkta,
            spegel: dto.spegel,
        }))
    }
}
