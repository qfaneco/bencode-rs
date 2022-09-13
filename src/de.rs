
use std::{
    str,
    ops::{AddAssign, MulAssign}
};

use num_traits::FromPrimitive;
use serde::{
    Deserialize,
    de::{self, Visitor, DeserializeSeed, IntoDeserializer}
};

use super::error::{Error, Result, ErrorKind};

pub struct Deserializer<'de> {
    input: &'de [u8],
    index: usize,
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de [u8]) -> Self {
        Deserializer { input, index: 0 }
    }
}

pub fn from_bytes<'de, T>(bytes: &'de [u8]) -> Result<T>
where
    T: Deserialize<'de>,
{
    let mut de = Deserializer::new(bytes);
    let value = T::deserialize(&mut de)?;

    de.end()?;

    Ok(value)
}

impl<'de> Deserializer<'de> {
    #[inline]
    pub fn end(&mut self) -> Result<()> {
        if self.index >= self.input.len() {
            Ok(())
        } else {
            Err(Error::syntax(ErrorKind::TrailingCharacters, self.index))
        }
    }

    #[inline]
    fn peek_byte(&self) -> Result<u8> {
        if self.index < self.input.len() {
            Ok(self.input[self.index])
        } else {
            Err(Error::eof(self.index))
        }
    }

    #[inline]
    fn next_byte(&mut self) -> Result<u8> {
        if self.index < self.input.len() {
            let b = self.input[self.index];
            self.index += 1;
            Ok(b)
        } else {
            Err(Error::eof(self.index))
        }
    }

    #[cold]
    fn error(&self, reason: ErrorKind) -> Error {
        Error::syntax(reason, self.index - 1)
    }

    #[cold]
    fn error_with_index(&self, reason: ErrorKind, index: usize) -> Error {
        Error::syntax(reason, index)
    }

    fn parse_bool(&mut self) -> Result<bool> {
        let bytes = [self.next_byte()?; 3];

        match &bytes {
            b"i1e" => Ok(true),
            b"i0e" => Ok(false),
            _ => Err(self.error(ErrorKind::ExpectedBoolean)),
        }
    }

    fn parse_number<T>(&mut self) -> Result<T>
    where
        T: AddAssign<T> + MulAssign<T> + FromPrimitive,
    {
        if self.next_byte()? != b'i' {
            return Err(self.error(ErrorKind::ExpectedInteger));
        }

        self.parse_integer(false)
    }

    fn parse_bytes(&mut self) -> Result<&'de [u8]> {
        let length: usize = self.parse_integer(true)?;

        let s = self.input
            .get(self.index..self.index + length)
            .ok_or_else(|| Error::eof(self.input.len()))?;
        self.index += length;

        Ok(s)
    }

    fn parse_integer<T>(&mut self, parsing_str: bool) -> Result<T>
    where
        T: AddAssign<T> + MulAssign<T> + FromPrimitive,
    {
        let start_index = if parsing_str { self.index } else { self.index - 1 };
        let end = if parsing_str { b':' } else { b'e' };
        let expected = if parsing_str { ErrorKind::ExpectedString }
                                else { ErrorKind::ExpectedInteger };
        let expected_end = if parsing_str { ErrorKind::ExpectedStringDelim }
                                    else { ErrorKind::ExpectedEnd };
        let mut positive = true;
        let mut first_iter = true;
        let mut n = 0u64;

        loop {
            match self.next_byte()? {
                b'-' if first_iter => {
                    match self.peek_byte()? {
                        // '-0'
                        b'0' => {
                            self.next_byte()?;
                            match self.peek_byte()? {
                                // '-0e'
                                c if c == end => return Err(self.error_with_index(ErrorKind::MinusZero, start_index)),
                                // '-0(0..9)'
                                b'0'..=b'9' => return Err(self.error(ErrorKind::LeadingZero)),
                                // '-0_'
                                _ => return Err(self.error_with_index(expected, start_index)),
                            }
                        },
                        // '-_'
                        c => {
                            if parsing_str {
                                return Err(self.error_with_index(expected, start_index));
                            } else {
                                // '--'
                                if c == b'-' {
                                    return Err(self.error_with_index(expected, start_index)); 
                                }
                                positive = false;
                                continue;
                            }
                        },
                    }
                },
                b'0' if first_iter => {
                    match self.peek_byte()? {
                        // '0e'
                        c if c == end => {
                            self.next_byte()?;
                            return Ok(FromPrimitive::from_u64(n).unwrap());
                        },
                        // '0(0..9)'
                        b'0'..=b'9' => return Err(self.error(ErrorKind::LeadingZero)),
                        // '0_'
                        _ => {},
                    }
                },
                c @ b'1'..=b'9' if first_iter => n = (c - b'0') as u64,
                c @ b'0'..=b'9' => n = n * 10 + (c - b'0') as u64,
                c if c == end && !first_iter => {
                    if positive {
                        return FromPrimitive::from_u64(n)
                            .ok_or_else(|| self.error_with_index(ErrorKind::IntegerOutOfRange, start_index));
                    } else {
                        return FromPrimitive::from_i64((n as i64).wrapping_neg())
                            .ok_or_else(|| self.error_with_index(ErrorKind::IntegerOutOfRange, start_index));
                    }
                },
                _ if first_iter => return Err(self.error_with_index(expected, start_index)),
                _ => return Err(self.error(expected_end)),
            }

            if first_iter { first_iter = false; }
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>
    {
        match self.peek_byte()? {
            b'i' => self.deserialize_i64(visitor),
            b'0'..=b'9' => self.deserialize_str(visitor),
            b'l' => self.deserialize_seq(visitor),
            b'd' => self.deserialize_map(visitor),
            _ => Err(self.error(ErrorKind::ExpectedSomeValue)),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parse_number()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parse_number()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parse_number()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parse_number()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parse_number()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parse_number()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parse_number()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parse_number()?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(self.parse_number()?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.parse_number()?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let ch = self.parse_bytes()?;
        if ch.len() == 1 {
            // TODO: maybe utf8 str
            visitor.visit_char(ch[0] as char)
        } else {
            Err(self.error(ErrorKind::ExpectedChar))
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match str::from_utf8(self.parse_bytes()?) {
            Ok(s) => visitor.visit_borrowed_str(s),
            Err(_) => Err(self.error(ErrorKind::StringNotUtf8)),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_bytes(self.parse_bytes()?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(self.parse_bytes()?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.peek_byte().is_err() {
            visitor.visit_unit()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.next_byte()? == b'l' {
            let value = visitor.visit_seq(SeqAccess::new(self))?;

            if self.next_byte()? != b'e' {
                Err(self.error(ErrorKind::ExpectedEnd))
            } else {
                Ok(value)
            }
        } else {
            Err(self.error(ErrorKind::ExpectedList))
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        if self.next_byte()? == b'd' {
            let value = visitor.visit_map(MapAccess::new(self))?;

            if self.next_byte()? != b'e' {
                Err(self.error(ErrorKind::ExpectedEnd))
            } else {
                Ok(value)
            }
        } else {
            Err(self.error(ErrorKind::ExpectedDict))
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.peek_byte()? {
            b'0'..=b'9' => {
                let s = str::from_utf8(self.parse_bytes()?)
                    .map_err(|_| self.error(ErrorKind::StringNotUtf8))?;
                visitor.visit_enum(s.into_deserializer())
            },
            b'd' => {
                self.next_byte()?;
                let value = visitor.visit_enum(EnumAccess::new(self))?;

                if self.next_byte()? != b'e' {
                    Err(self.error(ErrorKind::ExpectedEnd))
                } else {
                    Ok(value)
                }
            },
            _ => Err(self.error(ErrorKind::ExpectedEnum)),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct SeqAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de: 'a> SeqAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        SeqAccess { de }
    }
}

impl<'de, 'a> de::SeqAccess<'de> for SeqAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.de.peek_byte()? {
            b'e' => Ok(None),
            b'l' | b'd' | b'i' | b'0'..=b'9' => {
                seed.deserialize(&mut *self.de).map(Some)
            },
            _ => {
                Err(
                    self.de.error_with_index(
                        ErrorKind::ExpectedEnd,
                        self.de.index
                    )
                )
            },
        }
    }
}

struct MapAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de: 'a> MapAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        MapAccess { de }
    }
}

impl<'de, 'a> de::MapAccess<'de> for MapAccess<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: DeserializeSeed<'de>,
    {
        match self.de.peek_byte()? {
            b'e' => Ok(None),
            b'0'..=b'9' => seed.deserialize(&mut *self.de).map(Some),
            b'l' | b'd' | b'i' => Err(self.de.error(ErrorKind::KeyMustBeAString)),
            _ => {
                Err(
                    self.de.error_with_index(
                        ErrorKind::ExpectedEnd,
                        self.de.index
                    )
                )
            },
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

struct EnumAccess<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de: 'a> EnumAccess<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        EnumAccess { de }
    }
}

impl<'de, 'a> de::EnumAccess<'de> for EnumAccess<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let value = seed.deserialize(&mut *self.de)?;

        Ok((value, self))
    }
}

impl<'de, 'a> de::VariantAccess<'de> for EnumAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(self.de.error(ErrorKind::ExpectedString))
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use serde::Deserialize;

    use super::from_bytes;

    #[test]
    fn test_err_trailing_chars() {
        let i = from_bytes::<'_, i32>(b"i42eabc");

        assert_eq!(i.is_err(), true);
        assert_eq!(i.unwrap_err().to_string(), "trailing characters at index 4");
    }

    #[test]
    fn test_err_eof() {
        let i = from_bytes::<'_, i32>(b"");

        assert_eq!(i.is_err(), true);
        assert_eq!(i.unwrap_err().to_string(), "EOF while parsing at index 0");
    }

    #[test]
    fn test_int() {
        let i: i64 = from_bytes(b"i42e").unwrap();
        let j: i64 = from_bytes(b"i-42e").unwrap();
        let k: u8 = from_bytes(b"i0e").unwrap();
        let l: i32 = from_bytes(b"i-10200e").unwrap();

        assert_eq!(i, 42);
        assert_eq!(j, -42);
        assert_eq!(k, 0);
        assert_eq!(l, -10200);
    }

    #[test]
    fn test_int_err() {
        let i = from_bytes::<'_, i8>(b"i42000e");
        let j = from_bytes::<'_, u64>(b"i-1e");
        let k = from_bytes::<'_, u64>(b"i-0e");
        let l = from_bytes::<'_, i64>(b"i-0e");
        let m = from_bytes::<'_, u64>(b"i0022e");
        let n = from_bytes::<'_, i64>(b"i-022e");
        let o = from_bytes::<'_, i64>(b"iabc");
        let p = from_bytes::<'_, i64>(b"i22r");
        let q = from_bytes::<'_, i64>(b"i-0azertye");
        let r = from_bytes::<'_, i64>(b"i-azertye");

        assert_eq!(i.is_err(), true);
        assert_eq!(i.unwrap_err().to_string(), "integer out of range at index 0");
        assert_eq!(j.is_err(), true);
        assert_eq!(j.unwrap_err().to_string(), "integer out of range at index 0");
        assert_eq!(k.is_err(), true);
        assert_eq!(k.unwrap_err().to_string(), "`i-0e` is invalid at index 0");
        assert_eq!(l.is_err(), true);
        assert_eq!(l.unwrap_err().to_string(), "`i-0e` is invalid at index 0");
        assert_eq!(m.is_err(), true);
        assert_eq!(m.unwrap_err().to_string(), "leading zeros are invalid at index 1");
        assert_eq!(n.is_err(), true);
        assert_eq!(n.unwrap_err().to_string(), "leading zeros are invalid at index 2");
        assert_eq!(o.is_err(), true);
        assert_eq!(o.unwrap_err().to_string(), "expected integer at index 0");
        assert_eq!(p.is_err(), true);
        assert_eq!(p.unwrap_err().to_string(), "expected `e` at index 3");
        assert_eq!(q.is_err(), true);
        assert_eq!(q.unwrap_err().to_string(), "expected integer at index 0");
        assert_eq!(r.is_err(), true);
        assert_eq!(r.unwrap_err().to_string(), "expected integer at index 0");
    }

    #[test]
    fn test_string() {
        let s: &str = from_bytes(b"11:hello world").unwrap();
        let s2: &str = from_bytes(b"0:").unwrap();

        assert_eq!(s, "hello world");
        assert_eq!(s2, "");
    }

    #[test]
    fn test_string_err() {
        let a = from_bytes::<'_, &str>(b"5:blabla");
        let b = from_bytes::<'_, &str>(b"5:bla");
        let c = from_bytes::<'_, &str>(b"3a:bla");
        let d = from_bytes::<'_, &str>(b"55bla");
        let e = from_bytes::<'_, &str>(b"05:blabl");
        let f = from_bytes::<'_, &str>(b"-6:blabla");
        let g = from_bytes::<'_, &str>(b"blabla");

        assert_eq!(a.is_err(), true);
        assert_eq!(a.unwrap_err().to_string(), "trailing characters at index 7");
        assert_eq!(b.is_err(), true);
        assert_eq!(b.unwrap_err().to_string(), "EOF while parsing at index 5");
        assert_eq!(c.is_err(), true);
        assert_eq!(c.unwrap_err().to_string(), "expected `:` at index 1");
        assert_eq!(d.is_err(), true);
        assert_eq!(d.unwrap_err().to_string(), "expected `:` at index 2");
        assert_eq!(e.is_err(), true);
        assert_eq!(e.unwrap_err().to_string(), "leading zeros are invalid at index 0");
        assert_eq!(f.is_err(), true);
        assert_eq!(f.unwrap_err().to_string(), "expected string at index 0");
        assert_eq!(g.is_err(), true);
        assert_eq!(g.unwrap_err().to_string(), "expected string at index 0");
    }

    #[test]
    fn test_option() {
        let o: Option<i64> = from_bytes(b"i2e").unwrap();
        let o2: Option<i64> = from_bytes(b"").unwrap();

        assert_eq!(o, Some(2));
        assert_eq!(o2, None);
    }

    #[test]
    fn test_unit() {
        let u: () = from_bytes(b"").unwrap();

        assert_eq!(u, ());
    }

    #[test]
    fn test_unit_err() {
        let u = from_bytes::<'_, ()>(b"aa");

        assert_eq!(u.is_err(), true);
        assert_eq!(u.unwrap_err().to_string(), "trailing characters at index 0");
    }

    #[test]
    fn test_unit_struct() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Test;

        let us: Test = from_bytes(b"").unwrap();

        assert_eq!(us, Test);
    }

    #[test]
    fn test_unit_struct_err() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Test;

        let us = from_bytes::<'_, Test>(b"aa");

        assert_eq!(us.is_err(), true);
        assert_eq!(us.unwrap_err().to_string(), "trailing characters at index 0");
    }

    #[test]
    fn test_unit_variant() {
        #[derive(Debug, PartialEq, Deserialize)]
        enum Test { B }

        let uv: Test = from_bytes(b"1:B").unwrap();

        assert_eq!(uv, Test::B);
    }

    #[test]
    fn test_unit_variant_err() {
        #[derive(Debug, PartialEq, Deserialize)]
        enum Test { B }

        let uv = from_bytes::<'_, Test>(b"1:A");
        let uv2 = from_bytes::<'_, Test>(b"2:B");

        assert_eq!(uv.is_err(), true);
        assert_eq!(uv.unwrap_err().to_string(), "unknown variant `A`, expected `B`");
        assert_eq!(uv2.is_err(), true);
        assert_eq!(uv2.unwrap_err().to_string(), "EOF while parsing at index 3");
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Test(u16);

        let ns: Test = from_bytes(b"i5e").unwrap();

        assert_eq!(ns, Test(5));
    }

    #[test]
    fn test_newtype_variant() {
        #[derive(Debug, PartialEq, Deserialize)]
        enum Test { A(u16) }

        let nv: Test = from_bytes(b"d1:Ai69ee").unwrap();

        assert_eq!(nv, Test::A(69));
    }

    #[test]
    fn test_seq() {
        let v: Vec<i32> = from_bytes(b"li1ei2ei3ee").unwrap();
        let v2: Vec<&str> = from_bytes(b"l1:a2:ab3:abce").unwrap();
        let v3: (i32, &str, u32) = from_bytes(b"li-1e4:testi1ee").unwrap();

        assert_eq!(v, vec![1, 2, 3]);
        assert_eq!(v2, vec!["a", "ab", "abc"]);
        assert_eq!(v3, (-1, "test", 1));
    }

    #[test]
    fn test_seq_err() {
        let v = from_bytes::<'_, Vec<i32>>(b"li1ei2ea");
        let v2 = from_bytes::<'_, (i32, &str)>(b"li2e5:helloi3ee");
        let v3 = from_bytes::<'_, Vec<u32>>(b"i3ei2ee");
        let v4 = from_bytes::<'_, (u16, u16)>(b"le");
        let v5 = from_bytes::<'_, Vec<i32>>(b"li22e4:teste");

        assert_eq!(v.is_err(), true);
        assert_eq!(v.unwrap_err().to_string(), "expected `e` at index 7");
        assert_eq!(v2.is_err(), true);
        assert_eq!(v2.unwrap_err().to_string(), "expected `e` at index 11");
        assert_eq!(v3.is_err(), true);
        assert_eq!(v3.unwrap_err().to_string(), "expected list at index 0");
        assert_eq!(v4.is_err(), true);
        assert_eq!(v4.unwrap_err().to_string(), "invalid length 0, expected a tuple of size 2");
        assert_eq!(v5.is_err(), true);
        assert_eq!(v5.unwrap_err().to_string(), "expected integer at index 5");
    }

    #[test]
    fn test_tuple_struct() {
        #[derive(Debug, PartialEq, Deserialize)]
        struct Test(u16, String);

        let ts: Test = from_bytes(b"li21e4:teste").unwrap();

        assert_eq!(ts, Test(21, "test".to_string()));
    }

    #[test]
    fn test_tuple_variant() {
        #[derive(Debug, PartialEq, Deserialize)]
        enum Test { A(u16, String) }

        let tv: Test = from_bytes(b"d1:Ali20e4:okokee").unwrap();

        assert_eq!(tv, Test::A(20, "okok".to_string()));
    }

    #[test]
    fn test_map() {
        let m: BTreeMap<&str, i32> = from_bytes(b"d5:firsti1e6:secondi2ee").unwrap();

        assert_eq!(m.get("first"), Some(&1i32));
        assert_eq!(m.get("second"), Some(&2i32));
    }

    #[test]
    fn test_struct() {
        #[derive(Deserialize)]
        struct Test { a: Option<i64>, b: Vec<String>, c: u32 }

        let s: Test = from_bytes(b"d1:ci42e1:bl5:hello5:worldee").unwrap();

        assert_eq!(s.a, None);
        assert_eq!(s.b, ["hello".to_owned(), "world".to_owned()]);
        assert_eq!(s.c, 42);
    }

    #[test]
    fn test_struct_variant() {
        #[derive(Debug, PartialEq, Deserialize)]
        enum Test { A { a: i64, b: Vec<String> } }

        let sv: Test = from_bytes(b"d1:Ad1:ai12345e1:bl5:hello5:worldeee").unwrap();

        assert_eq!(sv, Test::A { a: 12345, b: vec!["hello".to_string(), "world".to_string()]});
    }
}
