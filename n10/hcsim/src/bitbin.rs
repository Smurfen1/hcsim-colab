//! Binärt serde-format med `f64::to_bits` / `f64::from_bits` (bit-exakt rundtur).

use serde::de::{
    self, DeserializeOwned, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::ser::{
    self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Deserializer, Serialize, Serializer};
use std::fmt;

pub fn till_bytes<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    let mut ser = BitSer { buf: Vec::new() };
    v.serialize(&mut ser).map_err(|e| e.0)?;
    Ok(ser.buf)
}

pub fn fran_bytes<T: DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    let mut de = BitDe { input: data, pos: 0 };
    let v = T::deserialize(&mut de).map_err(|e| e.0)?;
    if de.pos != data.len() {
        return Err(format!(
            "bitbin: {} bytes kvar efter deserialisering",
            data.len() - de.pos
        ));
    }
    Ok(v)
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
impl de::Error for Fel {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Fel(msg.to_string())
    }
}

mod tag {
    pub const UNIT: u8 = 0;
    pub const BOOL: u8 = 1;
    pub const U8: u8 = 2;
    pub const U16: u8 = 3;
    pub const U32: u8 = 4;
    pub const U64: u8 = 5;
    pub const I8: u8 = 6;
    pub const I16: u8 = 7;
    pub const I32: u8 = 8;
    pub const I64: u8 = 9;
    pub const F32: u8 = 10;
    pub const F64: u8 = 11;
    pub const CHAR: u8 = 12;
    pub const STR: u8 = 13;
    pub const BYTES: u8 = 14;
    pub const NONE: u8 = 15;
    pub const SOME: u8 = 16;
    pub const SEQ: u8 = 17;
    pub const MAP: u8 = 18;
    pub const STRUCT: u8 = 19;
    pub const UNIT_VAR: u8 = 20;
    pub const NEWTYPE_VAR: u8 = 21;
    pub const TUPLE_VAR: u8 = 22;
    pub const STRUCT_VAR: u8 = 23;
}

struct BitSer {
    buf: Vec<u8>,
}

impl BitSer {
    fn t(&mut self, tag: u8) {
        self.buf.push(tag);
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.u64(b.len() as u64);
        self.buf.extend_from_slice(b);
    }
}

impl<'a> Serializer for &'a mut BitSer {
    type Ok = ();
    type Error = Fel;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    fn serialize_bool(self, v: bool) -> Result<(), Fel> {
        self.t(tag::BOOL);
        self.buf.push(v as u8);
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), Fel> {
        self.t(tag::I8);
        self.buf.push(v as u8);
        Ok(())
    }
    fn serialize_i16(self, v: i16) -> Result<(), Fel> {
        self.t(tag::I16);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    fn serialize_i32(self, v: i32) -> Result<(), Fel> {
        self.t(tag::I32);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    fn serialize_i64(self, v: i64) -> Result<(), Fel> {
        self.t(tag::I64);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), Fel> {
        self.t(tag::U8);
        self.buf.push(v);
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<(), Fel> {
        self.t(tag::U16);
        self.buf.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<(), Fel> {
        self.t(tag::U32);
        self.u32(v);
        Ok(())
    }
    fn serialize_u64(self, v: u64) -> Result<(), Fel> {
        self.t(tag::U64);
        self.u64(v);
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<(), Fel> {
        self.t(tag::F32);
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
        Ok(())
    }
    fn serialize_f64(self, v: f64) -> Result<(), Fel> {
        self.t(tag::F64);
        self.u64(v.to_bits());
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<(), Fel> {
        self.t(tag::CHAR);
        self.u32(v as u32);
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), Fel> {
        self.t(tag::STR);
        self.bytes(v.as_bytes());
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Fel> {
        self.t(tag::BYTES);
        self.bytes(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Fel> {
        self.t(tag::NONE);
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<(), Fel> {
        self.t(tag::SOME);
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Fel> {
        self.t(tag::UNIT);
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Fel> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        variant_index: u32,
        _: &'static str,
    ) -> Result<(), Fel> {
        self.t(tag::UNIT_VAR);
        self.u32(variant_index);
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
        variant_index: u32,
        _: &'static str,
        v: &T,
    ) -> Result<(), Fel> {
        self.t(tag::NEWTYPE_VAR);
        self.u32(variant_index);
        v.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Compound<'a>, Fel> {
        let n = len.ok_or_else(|| Fel("seq utan längd".into()))?;
        self.t(tag::SEQ);
        self.u64(n as u64);
        Ok(Compound { ser: self })
    }
    fn serialize_tuple(self, len: usize) -> Result<Compound<'a>, Fel> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Compound<'a>, Fel> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        variant_index: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, Fel> {
        self.t(tag::TUPLE_VAR);
        self.u32(variant_index);
        self.u64(len as u64);
        Ok(Compound { ser: self })
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Compound<'a>, Fel> {
        let n = len.ok_or_else(|| Fel("map utan längd".into()))?;
        self.t(tag::MAP);
        self.u64(n as u64);
        Ok(Compound { ser: self })
    }
    fn serialize_struct(self, _: &'static str, len: usize) -> Result<Compound<'a>, Fel> {
        self.t(tag::STRUCT);
        self.u64(len as u64);
        Ok(Compound { ser: self })
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        variant_index: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, Fel> {
        self.t(tag::STRUCT_VAR);
        self.u32(variant_index);
        self.u64(len as u64);
        Ok(Compound { ser: self })
    }
}

struct Compound<'a> {
    ser: &'a mut BitSer,
}

impl SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeTupleStruct for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeTupleVariant for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Fel> {
        key.serialize(&mut *self.ser)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Fel> {
        value.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, _: &'static str, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}
impl SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = Fel;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, _: &'static str, v: &T) -> Result<(), Fel> {
        v.serialize(&mut *self.ser)
    }
    fn end(self) -> Result<(), Fel> {
        Ok(())
    }
}

struct BitDe<'de> {
    input: &'de [u8],
    pos: usize,
}

impl<'de> BitDe<'de> {
    fn read_u8(&mut self) -> Result<u8, Fel> {
        if self.pos >= self.input.len() {
            return Err(Fel("bitbin: oväntat slut".into()));
        }
        let x = self.input[self.pos];
        self.pos += 1;
        Ok(x)
    }
    fn read_n(&mut self, n: usize) -> Result<&'de [u8], Fel> {
        if self.pos + n > self.input.len() {
            return Err(Fel("bitbin: oväntat slut".into()));
        }
        let s = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn read_u32(&mut self) -> Result<u32, Fel> {
        Ok(u32::from_le_bytes(self.read_n(4)?.try_into().unwrap()))
    }
    fn read_u64(&mut self) -> Result<u64, Fel> {
        Ok(u64::from_le_bytes(self.read_n(8)?.try_into().unwrap()))
    }
    fn expect(&mut self, t: u8) -> Result<(), Fel> {
        let x = self.read_u8()?;
        if x != t {
            return Err(Fel(format!("bitbin: tag {t} förväntades, fick {x}")));
        }
        Ok(())
    }
}

impl<'a, 'de> Deserializer<'de> for &'a mut BitDe<'de> {
    type Error = Fel;

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Fel> {
        Err(Fel("bitbin: deserialize_any stöds inte".into()))
    }
    fn deserialize_bool<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::BOOL)?;
        v.visit_bool(self.read_u8()? != 0)
    }
    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::I8)?;
        v.visit_i8(self.read_u8()? as i8)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::I16)?;
        v.visit_i16(i16::from_le_bytes(self.read_n(2)?.try_into().unwrap()))
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::I32)?;
        v.visit_i32(i32::from_le_bytes(self.read_n(4)?.try_into().unwrap()))
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::I64)?;
        v.visit_i64(i64::from_le_bytes(self.read_n(8)?.try_into().unwrap()))
    }
    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::U8)?;
        v.visit_u8(self.read_u8()?)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::U16)?;
        v.visit_u16(u16::from_le_bytes(self.read_n(2)?.try_into().unwrap()))
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::U32)?;
        v.visit_u32(self.read_u32()?)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::U64)?;
        v.visit_u64(self.read_u64()?)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::F32)?;
        v.visit_f32(f32::from_bits(u32::from_le_bytes(
            self.read_n(4)?.try_into().unwrap(),
        )))
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::F64)?;
        v.visit_f64(f64::from_bits(self.read_u64()?))
    }
    fn deserialize_char<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::CHAR)?;
        let c = char::from_u32(self.read_u32()?).ok_or_else(|| Fel("ogiltig char".into()))?;
        v.visit_char(c)
    }
    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.deserialize_string(v)
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::STR)?;
        let n = self.read_u64()? as usize;
        let s = std::str::from_utf8(self.read_n(n)?).map_err(|e| Fel(e.to_string()))?;
        v.visit_string(s.to_owned())
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.deserialize_byte_buf(v)
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::BYTES)?;
        let n = self.read_u64()? as usize;
        v.visit_byte_buf(self.read_n(n)?.to_vec())
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        match self.read_u8()? {
            tag::NONE => v.visit_none(),
            tag::SOME => v.visit_some(self),
            t => Err(Fel(format!("option-tag {t}"))),
        }
    }
    fn deserialize_unit<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::UNIT)?;
        v.visit_unit()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        v: V,
    ) -> Result<V::Value, Fel> {
        self.deserialize_unit(v)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        v: V,
    ) -> Result<V::Value, Fel> {
        v.visit_newtype_struct(self)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::SEQ)?;
        let n = self.read_u64()? as usize;
        v.visit_seq(LenAccess { de: self, kvar: n })
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::SEQ)?;
        let n = self.read_u64()? as usize;
        if n != len {
            return Err(Fel(format!("tuple {n} != {len}")));
        }
        v.visit_seq(LenAccess { de: self, kvar: n })
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        v: V,
    ) -> Result<V::Value, Fel> {
        self.deserialize_tuple(len, v)
    }
    fn deserialize_map<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.expect(tag::MAP)?;
        let n = self.read_u64()? as usize;
        v.visit_map(LenAccess { de: self, kvar: n })
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Fel> {
        self.expect(tag::STRUCT)?;
        let n = self.read_u64()? as usize;
        if n != fields.len() {
            return Err(Fel(format!(
                "struct {n} fält, väntade {} ({fields:?})",
                fields.len()
            )));
        }
        v.visit_seq(LenAccess { de: self, kvar: n })
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Fel> {
        let form = self.read_u8()?;
        let idx = self.read_u32()?;
        v.visit_enum(EnumDe {
            de: self,
            form,
            idx,
        })
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.deserialize_string(v)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Fel> {
        self.deserialize_any(v)
    }
}

struct LenAccess<'a, 'de> {
    de: &'a mut BitDe<'de>,
    kvar: usize,
}

impl<'de> SeqAccess<'de> for LenAccess<'_, 'de> {
    type Error = Fel;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Fel> {
        if self.kvar == 0 {
            return Ok(None);
        }
        self.kvar -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
}

impl<'de> MapAccess<'de> for LenAccess<'_, 'de> {
    type Error = Fel;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Fel> {
        if self.kvar == 0 {
            return Ok(None);
        }
        self.kvar -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Fel> {
        seed.deserialize(&mut *self.de)
    }
}

struct EnumDe<'a, 'de> {
    de: &'a mut BitDe<'de>,
    form: u8,
    idx: u32,
}

impl<'de> EnumAccess<'de> for EnumDe<'_, 'de> {
    type Error = Fel;
    type Variant = Self;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Fel> {
        let val = seed.deserialize(self.idx.into_deserializer())?;
        Ok((val, self))
    }
}

impl<'de> VariantAccess<'de> for EnumDe<'_, 'de> {
    type Error = Fel;

    fn unit_variant(self) -> Result<(), Fel> {
        if self.form != tag::UNIT_VAR {
            return Err(Fel(format!("väntade unit-variant, form {}", self.form)));
        }
        Ok(())
    }
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Fel> {
        if self.form != tag::NEWTYPE_VAR {
            return Err(Fel(format!("väntade newtype-variant, form {}", self.form)));
        }
        seed.deserialize(self.de)
    }
    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Fel> {
        if self.form != tag::TUPLE_VAR {
            return Err(Fel(format!("väntade tuple-variant, form {}", self.form)));
        }
        let n = self.de.read_u64()? as usize;
        if n != len {
            return Err(Fel(format!("tuple-variant {n} != {len}")));
        }
        visitor.visit_seq(LenAccess {
            de: self.de,
            kvar: n,
        })
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Fel> {
        if self.form != tag::STRUCT_VAR {
            return Err(Fel(format!("väntade struct-variant, form {}", self.form)));
        }
        let n = self.de.read_u64()? as usize;
        if n != fields.len() {
            return Err(Fel(format!(
                "struct-variant {n} != {}",
                fields.len()
            )));
        }
        visitor.visit_seq(LenAccess {
            de: self.de,
            kvar: n,
        })
    }
}
