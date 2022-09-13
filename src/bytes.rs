use std::{
    marker::PhantomData,
    fmt,
    mem::MaybeUninit
};

use serde::{Deserializer, Serializer, de::{Visitor, Error}};

pub fn serialize<T, S>(bytes: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: ?Sized + Serialize,
    S: Serializer,
{
    Serialize::serialize(bytes, serializer)
}

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer)
}

pub trait Serialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}

impl Serialize for Vec<u8> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&self[..])
    }
}

impl Serialize for [u8] {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self)
    }
}

impl<const N: usize> Serialize for [u8; N] {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self)
    }
}

impl<'a, T> Serialize for &'a T
where
    T: ?Sized + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<T> Serialize for Box<T>
where
    T: ?Sized + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (**self).serialize(serializer)
    }
}

impl<T> Serialize for Option<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        struct AsBytes<T>(T);

        impl<T> serde::Serialize for AsBytes<T>
        where
            T: Serialize,
        {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        match self {
            Some(b) => serializer.serialize_some(&AsBytes(b)),
            None => serializer.serialize_none(),
        }
    }
}

pub trait Deserialize<'de>: Sized {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de: 'a, 'a> Deserialize<'de> for &'a [u8] {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        serde::Deserialize::deserialize(deserializer)
    }
}

impl<'de, const N: usize> Deserialize<'de> for [u8; N] {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<const N: usize>;

        impl<'de, const N: usize> Visitor<'de> for ArrayVisitor<N> {
            type Value = [u8; N];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "an byte array of size {}", N)
            }

            fn visit_borrowed_bytes<E>(self, b: &'de [u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let mut arr: [MaybeUninit<u8>; N] = unsafe { MaybeUninit::uninit().assume_init() };

                let mut b_iter = b.iter(); 
                let mut place_iter = arr.iter_mut();
                let mut cnt_filled = 0;

                let err = loop {
                    match (b_iter.next(), place_iter.next()) {
                        (Some(val), Some(place)) => *place = MaybeUninit::new(*val),
                        (None, None) => break None,
                        (None, Some(_)) | (Some(_), None) => {
                            break Some(Error::invalid_length(cnt_filled, &self))
                        }
                    }
                    cnt_filled += 1;
                };

                if let Some(err) = err {
                    if std::mem::needs_drop::<u8>() {
                        for elem in arr.into_iter().take(cnt_filled) {
                            unsafe {
                                elem.assume_init();
                            }
                        }
                    }
                    return Err(err);
                }

                let ret = unsafe { std::mem::transmute_copy(&arr) };

                Ok(ret)
            }
        }

        deserializer.deserialize_bytes(ArrayVisitor)
    }
}

impl<'de> Deserialize<'de> for Vec<u8> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor;

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a byte vec")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(v)
            }
        }

        deserializer.deserialize_byte_buf(VecVisitor)
    }
}

impl<'de> Deserialize<'de> for Box<[u8]> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(Vec::into_boxed_slice)
    }
}

impl<'de, T> Deserialize<'de> for Option<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BytesVisitor<T> {
            out: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for BytesVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Option<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("optional byte array")
            }

            fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }

            fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                T::deserialize(deserializer).map(Some)
            }
        }

        let visitor = BytesVisitor { out: PhantomData };
        deserializer.deserialize_option(visitor)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Serialize, Deserialize};

    use crate::{from_bytes, to_bytes};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Test<'a> {
        #[serde(with = "super")]
        bytes: &'a [u8],
        #[serde(with = "super")]
        vec: Vec<u8>,
        #[serde(with = "super")]
        id: [u8; 20],
    }

    #[test]
    fn test_ser_bencode_bytes() {
        let t = Test { bytes: b"super test", vec: b"test".to_vec(), id: [48u8; 20]};

        assert_eq!(b"d5:bytes10:super test3:vec4:test2:id20:00000000000000000000e" as &[u8], to_bytes(&t).unwrap());
    }

    #[test]
    fn test_de_bencode_bytes() {
        let t: Test = from_bytes(b"d5:bytes10:super test3:vec4:test2:id20:00000000000000000000e").unwrap();

        assert_eq!(t, Test {bytes: b"super test", vec: b"test".to_vec(), id: [48u8; 20]})
    }
}
