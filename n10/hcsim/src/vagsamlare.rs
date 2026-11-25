//! Serde-`Serializer` som bara samlar fältvägar (Alt 1302 prov 5).
//! Vägar som `djur[].hj.wp`, `klimat.m`, `ledlogg.pending` — listor skrivs med `[]`.

use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Serialize, Serializer};
use std::fmt;

/// Samlar kanoniska fältvägar ur en `Serialize`-värld.
pub fn samla_vagar<T: Serialize>(v: &T) -> Result<Vec<String>, String> {
    let mut ser = VagSer {
        stack: Vec::new(),
        vagar: Vec::new(),
    };
    v.serialize(&mut ser).map_err(|e| e.0)?;
    ser.vagar.sort();
    ser.vagar.dedup();
    Ok(ser.vagar)
}

/// Normalisera konkret väg (`djur[3].e`) till kanonisk (`djur[].e`).
pub fn kanonisk(vag: &str) -> String {
    let mut ut = String::with_capacity(vag.len());
    let bytes = vag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            ut.push_str("[]");
            while i < bytes.len() && bytes[i] != b']' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // hoppa över ']'
            }
        } else {
            ut.push(bytes[i] as char);
            i += 1;
        }
    }
    ut
}

#[derive(Debug)]
struct Fel(String);
impl fmt::Display for Fel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Fel {}
impl ser::Error for Fel {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Fel(msg.to_string())
    }
}

struct VagSer {
    stack: Vec<String>,
    vagar: Vec<String>,
}

impl VagSer {
    fn vag(&self) -> String {
        self.stack.join(".")
    }
    fn lagg_lov(&mut self) {
        let v = self.vag();
        if !v.is_empty() {
            self.vagar.push(v);
        }
    }
    fn push_falt(&mut self, namn: &str) {
        self.stack.push(namn.to_string());
    }
    fn pop(&mut self) {
        self.stack.pop();
    }
}

impl<'a> Serializer for &'a mut VagSer {
    type Ok = ();
    type Error = Fel;
    type SerializeSeq = VagCompound<'a>;
    type SerializeTuple = VagCompound<'a>;
    type SerializeTupleStruct = VagCompound<'a>;
    type SerializeTupleVariant = VagCompound<'a>;
    type SerializeMap = VagCompound<'a>;
    type SerializeStruct = VagCompound<'a>;
    type SerializeStructVariant = VagCompound<'a>;

    fn serialize_bool(self, _: bool) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_i8(self, _: i8) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_i16(self, _: i16) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_i32(self, _: i32) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_i64(self, _: i64) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_u8(self, _: u8) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_u16(self, _: u16) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_u32(self, _: u32) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_u64(self, _: u64) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_f32(self, _: f32) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_f64(self, _: f64) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_char(self, _: char) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_str(self, _: &str) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<(), Fel> {
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<(), Fel> {
        self.lagg_lov();
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        v: &T,
    ) -> Result<(), Fel> {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        v: &T,
    ) -> Result<(), Fel> {
        self.push_falt(variant);
        let r = v.serialize(&mut *self);
        self.pop();
        r
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<VagCompound<'a>, Fel> {
        // Tom lista: registrera vägen till listan själv.
        if len == Some(0) {
            self.lagg_lov();
        }
        Ok(VagCompound {
            ser: self,
            first: true,
            pop_variant: false,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<VagCompound<'a>, Fel> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<VagCompound<'a>, Fel> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VagCompound<'a>, Fel> {
        self.push_falt(variant);
        if len == 0 {
            self.lagg_lov();
        }
        Ok(VagCompound {
            ser: self,
            first: true,
            pop_variant: true,
        })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<VagCompound<'a>, Fel> {
        if len == Some(0) {
            self.lagg_lov();
        }
        Ok(VagCompound {
            ser: self,
            first: true,
            pop_variant: false,
        })
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<VagCompound<'a>, Fel> {
        if len == 0 {
            self.lagg_lov();
        }
        Ok(VagCompound {
            ser: self,
            first: true,
            pop_variant: false,
        })
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VagCompound<'a>, Fel> {
        self.push_falt(variant);
        if len == 0 {
            self.lagg_lov();
        }
        Ok(VagCompound {
            ser: self,
            first: true,
            pop_variant: true,
        })
    }
}

struct VagCompound<'a> {
    ser: &'a mut VagSer,
    first: bool,
    pop_variant: bool,
}

impl VagCompound<'_> {
    fn enter_seq_elem(&mut self) {
        // Lägg `[]` på sista stack-delen en gång; element delar kanonisk väg.
        if self.first {
            if let Some(sista) = self.ser.stack.last_mut() {
                if !sista.ends_with("[]") {
                    sista.push_str("[]");
                }
            } else {
                self.ser.stack.push("[]".into());
            }
            self.first = false;
        }
    }
}

impl SerializeSeq for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        self.enter_seq_elem();
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        slut_seq(self.ser, self.first);
        Ok(())
    }
}
impl SerializeTuple for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<(), Fel> {
        SerializeSeq::end(self)
    }
}
impl SerializeTupleStruct for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        SerializeSeq::serialize_element(self, v)
    }
    fn end(self) -> Result<(), Fel> {
        SerializeSeq::end(self)
    }
}
impl SerializeTupleVariant for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        self.enter_seq_elem();
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        slut_seq(self.ser, self.first);
        if self.pop_variant {
            self.ser.pop();
        }
        Ok(())
    }
}
impl SerializeMap for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, _key: &T) -> Result<(), Fel> {
        // Nycklar registreras inte som egna löv; värdets väg blir `falt[]…`.
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Fel> {
        self.enter_seq_elem();
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        slut_seq(self.ser, self.first);
        Ok(())
    }
}

fn slut_seq(ser: &mut VagSer, first: bool) {
    if first {
        return;
    }
    if let Some(sista) = ser.stack.last_mut() {
        if sista == "[]" {
            ser.stack.pop();
        } else if sista.ends_with("[]") {
            sista.truncate(sista.len() - 2);
        }
    }
}
impl SerializeStruct for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Fel> {
        self.ser.push_falt(key);
        let r = v.serialize(&mut *self.ser);
        self.ser.pop();
        r
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeStructVariant for VagCompound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Fel> {
        self.ser.push_falt(key);
        let r = v.serialize(&mut *self.ser);
        self.ser.pop();
        r
    }
    fn end(self) -> Result<(), Fel> {
        if self.pop_variant {
            self.ser.pop();
        }
        Ok(())
    }
}


#[cfg(test)]
mod prov {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Liten {
        a: i32,
        b: Vec<f64>,
        c: Inre,
    }
    #[derive(Serialize)]
    struct Inre { m: f64, xs: Vec<u8> }

    #[test]
    fn vagar_exempel() {
        let v = Liten { a: 1, b: vec![1.0, 2.0], c: Inre { m: 3.0, xs: vec![9] } };
        let vagar = samla_vagar(&v).unwrap();
        assert!(vagar.iter().any(|x| x == "a"), "{vagar:?}");
        assert!(vagar.iter().any(|x| x == "b[]"), "{vagar:?}");
        assert!(vagar.iter().any(|x| x == "c.m"), "{vagar:?}");
        assert!(vagar.iter().any(|x| x == "c.xs[]"), "{vagar:?}");
    }
}
