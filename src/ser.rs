use std::io;
use serde::{ser, Serialize};

use super::error::{Error, Result};

pub struct Serializer<W: io::Write> {
    writer: W,
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize
{
    let vec = Vec::with_capacity(128);
    let mut serializer = Serializer::new(vec);
    value.serialize(&mut serializer)?;
    Ok(serializer.writer)
}

pub fn to_writer<T, W>(value: &T, writer: &mut W) -> Result<()>
where
    T: Serialize,
    W: io::Write,
{
    let mut serializer = Serializer::new(writer);
    value.serialize(&mut serializer)
}

impl<W: io::Write> Serializer<W> {
    fn new(writer: W) -> Self {
        Serializer { writer }
    }
}

impl<'a, W: io::Write> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        if v { self.serialize_i64(1) }
        else { self.serialize_i64(0) }
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.writer.write_all(b"i")?;
        self.writer.write_all(itoa::Buffer::new().format(v).as_bytes())?;
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.writer.write_all(b"i")?;
        self.writer.write_all(itoa::Buffer::new().format(v).as_bytes())?;
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }

    // Bencode format does not support floats
    fn serialize_f32(self, v: f32) -> Result<()> {
        log::warn!(
            "WARNING: Possible data corruption detected with value \"{}\": \n\
            Bencoding only defines support for integers, value was converted to {}",
            v,
            v.trunc(),
        );
        self.serialize_i64(v.trunc() as i64)
    }

    // Bencode format does not support floats
    fn serialize_f64(self, v: f64) -> Result<()> {
        log::warn!(
            "WARNING: Possible data corruption detected with value \"{}\": \n\
            Bencoding only defines support for integers, value was converted to {}",
            v,
            v.trunc(),
        );
        self.serialize_i64(v.trunc() as i64)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.writer.write_all(itoa::Buffer::new().format(v.len()).as_bytes())?;
        self.writer.write_all(b":")?;
        self.writer.write_all(v.as_bytes())
            .map_err(Into::into)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.writer.write_all(itoa::Buffer::new().format(v.len()).as_bytes())?;
        self.writer.write_all(b":")?;
        self.writer.write_all(v)
            .map_err(Into::into)
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.writer.write_all(b"d")?;
        variant.serialize(&mut *self)?;
        value.serialize(&mut *self)?;
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.writer.write_all(b"l")?;
        Ok(self)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.writer.write_all(b"d")?;
        variant.serialize(&mut *self)?;
        self.writer.write_all(b"l")?;
        Ok(self)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        self.writer.write_all(b"d")?;
        Ok(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.writer.write_all(b"d")?;
        variant.serialize(&mut *self)?;
        self.writer.write_all(b"d")?;
        Ok(self)
    }
}

impl<'a, W: io::Write> ser::SerializeSeq for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeTuple for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeTupleStruct for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeTupleVariant for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"ee")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeMap for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeStruct for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"e")
            .map_err(Into::into)
    }
}

impl<'a, W: io::Write> ser::SerializeStructVariant for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        self.writer.write_all(b"ee")
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use serde::Serialize;
    use super::to_bytes;

    #[test]
    fn test_int() {
        let i = -42i32;

        assert_eq!(to_bytes(&i).unwrap(), b"i-42e");
    }

    #[test]
    fn test_uint() {
        let i = 42u32;

        assert_eq!(to_bytes(&i).unwrap(), b"i42e");
    }

    #[test]
    fn test_float() {
        let f = 42.69f32;

        assert_eq!(to_bytes(&f).unwrap(), b"i42e");
    }

    #[test]
    fn test_string() {
        let s = String::from("Hello World!");

        assert_eq!(to_bytes(&s).unwrap(), b"12:Hello World!");
    }

    #[test]
    fn test_option() {
        let o: Option<i64> = None;

        assert_eq!(to_bytes(&o).unwrap().len(), 0);
    }

    #[test]
    fn test_unit() {
        let u = ();

        assert_eq!(to_bytes(&u).unwrap().len(), 0);
    }

    #[test]
    fn test_unit_struct() {
        #[derive(Serialize)]
        struct Test;

        let us = Test;

        assert_eq!(to_bytes(&us).unwrap().len(), 0);
    }

    #[test]
    fn test_unit_variant() {
        #[derive(Serialize)]
        enum Test { B }

        let uv = Test::B;

        assert_eq!(to_bytes(&uv).unwrap(), b"1:B");
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(Serialize)]
        struct Test(u16);

        let ns = Test(5);

        assert_eq!(to_bytes(&ns).unwrap(), b"i5e");
    }

    #[test]
    fn test_newtype_variant() {
        #[derive(Serialize)]
        enum Test { A(u16) }

        let nv = Test::A(69);

        assert_eq!(to_bytes(&nv).unwrap(), b"d1:Ai69ee");
    }

    #[test]
    fn test_seq() {
        let v = vec![1u8, 2, 3, 4, 5];

        assert_eq!(to_bytes(&v).unwrap(), b"li1ei2ei3ei4ei5ee");
    }

    #[test]
    fn test_tuple() {
        let t = (42i8, "Hello", Some(vec!["ok1", "ok2"]));

        assert_eq!(to_bytes(&t).unwrap(), b"li42e5:Hellol3:ok13:ok2ee");
    }

    #[test]
    fn test_tuple_struct() {
        #[derive(Serialize)]
        struct Test(u16, String);

        let ts = Test(21, String::from("test"));

        assert_eq!(to_bytes(&ts).unwrap(), b"li21e4:teste");
    }

    #[test]
    fn test_tuple_variant() {
        #[derive(Serialize)]
        enum Test { A(u16, String) }

        let tv = Test::A(20, String::from("okok"));

        assert_eq!(to_bytes(&tv).unwrap(), b"d1:Ali20e4:okokee");
    }

    #[test]
    fn test_map() {
        let mut m = BTreeMap::new();
        m.insert("test", 24u32);

        assert_eq!(to_bytes(&m).unwrap(), b"d4:testi24ee");
    }

    #[test]
    fn test_struct() {
        #[derive(Serialize)]
        struct Test { a: i64, b: Vec<String> }

        let s = Test {
            a: 12345,
            b: vec![String::from("hello"), String::from("world")],
        };

        assert_eq!(to_bytes(&s).unwrap(), b"d1:ai12345e1:bl5:hello5:worldee");
    }

    #[test]
    fn test_struct_variant() {
        #[derive(Serialize)]
        enum Test { A { a: i64, b: Vec<String> } }

        let sv = Test::A {
            a: 12345,
            b: vec![String::from("hello"), String::from("world")],
        };

        assert_eq!(to_bytes(&sv).unwrap(), b"d1:Ad1:ai12345e1:bl5:hello5:worldeee");
    }
}
