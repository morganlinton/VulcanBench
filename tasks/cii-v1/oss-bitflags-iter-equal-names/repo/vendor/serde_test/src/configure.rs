use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, Error, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::fmt::{self, Display};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Readable<T: ?Sized>(T);
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Compact<T: ?Sized>(T);

/// Trait to determine whether a value is represented in human-readable or
/// compact form.
///
/// ```
/// use serde::{Deserialize, Deserializer, Serialize, Serializer};
/// use serde_test::{assert_tokens, Configure, Token};
///
/// #[derive(Debug, PartialEq)]
/// struct Example(u8, u8);
///
/// impl Serialize for Example {
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
///     where
///         S: Serializer,
///     {
///         if serializer.is_human_readable() {
///             format!("{}.{}", self.0, self.1).serialize(serializer)
///         } else {
///             (self.0, self.1).serialize(serializer)
///         }
///     }
/// }
///
/// impl<'de> Deserialize<'de> for Example {
///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
///     where
///         D: Deserializer<'de>,
///     {
///         use serde::de::Error;
///         if deserializer.is_human_readable() {
///             let s = String::deserialize(deserializer)?;
///             let parts: Vec<_> = s.split('.').collect();
///             Ok(Example(
///                 parts[0].parse().map_err(D::Error::custom)?,
///                 parts[1].parse().map_err(D::Error::custom)?,
///             ))
///         } else {
///             let (x, y) = Deserialize::deserialize(deserializer)?;
///             Ok(Example(x, y))
///         }
///     }
/// }
///
/// fn main() {
///     assert_tokens(
///         &Example(1, 0).compact(),
///         &[
///             Token::Tuple { len: 2 },
///             Token::U8(1),
///             Token::U8(0),
///             Token::TupleEnd,
///         ],
///     );
///     assert_tokens(&Example(1, 0).readable(), &[Token::Str("1.0")]);
/// }
/// ```
pub trait Configure {
    /// Marks `self` as using `is_human_readable == true`
    fn readable(self) -> Readable<Self>
    where
        Self: Sized,
    {
        Readable(self)
    }
    /// Marks `self` as using `is_human_readable == false`
    fn compact(self) -> Compact<Self>
    where
        Self: Sized,
    {
        Compact(self)
    }
}

impl<T> Configure for T where T: ?Sized {}

impl<T> Serialize for Readable<T>
where
    T: ?Sized + Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(Readable(serializer))
    }
}

impl<T> Serialize for Compact<T>
where
    T: ?Sized + Serialize,
{
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(Compact(serializer))
    }
}

impl<'de, T> Deserialize<'de> for Readable<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(Readable(deserializer)).map(Readable)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_in_place(Readable(deserializer), &mut place.0)
    }
}

impl<'de, T> Deserialize<'de> for Compact<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(Compact(deserializer)).map(Compact)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_in_place(Compact(deserializer), &mut place.0)
    }
}

impl<'de, T> DeserializeSeed<'de> for Readable<T>
where
    T: DeserializeSeed<'de>,
{
    type Value = T::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.deserialize(Readable(deserializer))
    }
}

impl<'de, T> DeserializeSeed<'de> for Compact<T>
where
    T: DeserializeSeed<'de>,
{
    type Value = T::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.deserialize(Compact(deserializer))
    }
}

macro_rules! forward_method {
    ($name: ident (self $(, $arg: ident : $arg_type: ty)* ) -> $return_type: ty) => {
        fn $name (self $(, $arg : $arg_type)* ) -> $return_type {
            (self.0).$name( $($arg),* )
        }
    };
}

macro_rules! forward_serialize_methods {
    ( $( $name: ident $arg_type: ty ),* ) => {
        $(
            forward_method!($name(self, v : $arg_type) -> Result<Self::Ok, Self::Error>);
        )*
    };
}

macro_rules! impl_serializer {
    ($wrapper:ident, $is_human_readable:expr) => {
        impl<S> Serializer for $wrapper<S>
        where
            S: Serializer,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            type SerializeSeq = $wrapper<S::SerializeSeq>;
            type SerializeTuple = $wrapper<S::SerializeTuple>;
            type SerializeTupleStruct = $wrapper<S::SerializeTupleStruct>;
            type SerializeTupleVariant = $wrapper<S::SerializeTupleVariant>;
            type SerializeMap = $wrapper<S::SerializeMap>;
            type SerializeStruct = $wrapper<S::SerializeStruct>;
            type SerializeStructVariant = $wrapper<S::SerializeStructVariant>;

            fn is_human_readable(&self) -> bool {
                $is_human_readable
            }

            forward_serialize_methods! {
                serialize_bool bool,
                serialize_i8 i8,
                serialize_i16 i16,
                serialize_i32 i32,
                serialize_i64 i64,
                serialize_i128 i128,
                serialize_u8 u8,
                serialize_u16 u16,
                serialize_u32 u32,
                serialize_u64 u64,
                serialize_u128 u128,
                serialize_f32 f32,
                serialize_f64 f64,
                serialize_char char,
                serialize_str &str,
                serialize_bytes &[u8],
                serialize_unit_struct &'static str
            }

            fn serialize_unit(self) -> Result<S::Ok, S::Error> {
                self.0.serialize_unit()
            }

            fn serialize_unit_variant(
                self,
                name: &'static str,
                variant_index: u32,
                variant: &'static str,
            ) -> Result<S::Ok, S::Error> {
                self.0.serialize_unit_variant(name, variant_index, variant)
            }

            fn serialize_newtype_struct<T>(
                self,
                name: &'static str,
                value: &T,
            ) -> Result<S::Ok, S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_newtype_struct(name, &$wrapper(value))
            }

            fn serialize_newtype_variant<T>(
                self,
                name: &'static str,
                variant_index: u32,
                variant: &'static str,
                value: &T,
            ) -> Result<S::Ok, S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0
                    .serialize_newtype_variant(name, variant_index, variant, &$wrapper(value))
            }

            fn serialize_none(self) -> Result<S::Ok, Self::Error> {
                self.0.serialize_none()
            }

            fn serialize_some<T>(self, value: &T) -> Result<S::Ok, Self::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_some(&$wrapper(value))
            }

            fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
                self.0.serialize_seq(len).map($wrapper)
            }

            fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
                self.0.serialize_tuple(len).map($wrapper)
            }

            fn serialize_tuple_struct(
                self,
                name: &'static str,
                len: usize,
            ) -> Result<Self::SerializeTupleStruct, Self::Error> {
                self.0.serialize_tuple_struct(name, len).map($wrapper)
            }

            fn serialize_tuple_variant(
                self,
                name: &'static str,
                variant_index: u32,
                variant: &'static str,
                len: usize,
            ) -> Result<Self::SerializeTupleVariant, Self::Error> {
                self.0
                    .serialize_tuple_variant(name, variant_index, variant, len)
                    .map($wrapper)
            }

            fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
                self.0.serialize_map(len).map($wrapper)
            }

            fn serialize_struct(
                self,
                name: &'static str,
                len: usize,
            ) -> Result<Self::SerializeStruct, Self::Error> {
                self.0.serialize_struct(name, len).map($wrapper)
            }

            fn serialize_struct_variant(
                self,
                name: &'static str,
                variant_index: u32,
                variant: &'static str,
                len: usize,
            ) -> Result<Self::SerializeStructVariant, Self::Error> {
                self.0
                    .serialize_struct_variant(name, variant_index, variant, len)
                    .map($wrapper)
            }

            fn collect_seq<I>(self, iter: I) -> Result<Self::Ok, Self::Error>
            where
                I: IntoIterator,
                <I as IntoIterator>::Item: Serialize,
            {
                self.0
                    .collect_seq(iter.into_iter().map(|item| $wrapper(item)))
            }

            fn collect_map<K, V, I>(self, iter: I) -> Result<Self::Ok, Self::Error>
            where
                K: Serialize,
                V: Serialize,
                I: IntoIterator<Item = (K, V)>,
            {
                self.0.collect_map(
                    iter.into_iter()
                        .map(|(key, value)| ($wrapper(key), $wrapper(value))),
                )
            }

            fn collect_str<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
            where
                T: ?Sized + Display,
            {
                self.0.collect_str(value)
            }
        }

        impl<S> SerializeSeq for $wrapper<S>
        where
            S: SerializeSeq,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_element<T>(&mut self, value: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_element(&$wrapper(value))
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeTuple for $wrapper<S>
        where
            S: SerializeTuple,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_element<T>(&mut self, value: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_element(&$wrapper(value))
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeTupleStruct for $wrapper<S>
        where
            S: SerializeTupleStruct,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_field<T>(&mut self, value: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_field(&$wrapper(value))
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeTupleVariant for $wrapper<S>
        where
            S: SerializeTupleVariant,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_field<T>(&mut self, value: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_field(&$wrapper(value))
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeMap for $wrapper<S>
        where
            S: SerializeMap,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_key<T>(&mut self, key: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_key(&$wrapper(key))
            }

            fn serialize_value<T>(&mut self, value: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_value(&$wrapper(value))
            }

            fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), S::Error>
            where
                K: ?Sized + Serialize,
                V: ?Sized + Serialize,
            {
                self.0.serialize_entry(&$wrapper(key), &$wrapper(value))
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeStruct for $wrapper<S>
        where
            S: SerializeStruct,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_field<T>(&mut self, name: &'static str, field: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_field(name, &$wrapper(field))
            }

            fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
                self.0.skip_field(key)
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }

        impl<S> SerializeStructVariant for $wrapper<S>
        where
            S: SerializeStructVariant,
        {
            type Ok = S::Ok;
            type Error = S::Error;

            fn serialize_field<T>(&mut self, name: &'static str, field: &T) -> Result<(), S::Error>
            where
                T: ?Sized + Serialize,
            {
                self.0.serialize_field(name, &$wrapper(field))
            }

            fn skip_field(&mut self, key: &'static str) -> Result<(), Self::Error> {
                self.0.skip_field(key)
            }

            fn end(self) -> Result<S::Ok, S::Error> {
                self.0.end()
            }
        }
    };
}

impl_serializer!(Readable, true);
impl_serializer!(Compact, false);

macro_rules! forward_deserialize_methods {
    ( $wrapper : ident ( $( $name: ident ),* ) ) => {
        $(
            fn $name<V>(self, visitor: V) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                (self.0).$name($wrapper(visitor))
            }
        )*
    };
}

macro_rules! impl_deserializer {
    ($wrapper:ident, $is_human_readable:expr) => {
        impl<'de, D> Deserializer<'de> for $wrapper<D>
        where
            D: Deserializer<'de>,
        {
            type Error = D::Error;

            forward_deserialize_methods! {
                $wrapper (
                    deserialize_any,
                    deserialize_bool,
                    deserialize_u8,
                    deserialize_u16,
                    deserialize_u32,
                    deserialize_u64,
                    deserialize_u128,
                    deserialize_i8,
                    deserialize_i16,
                    deserialize_i32,
                    deserialize_i64,
                    deserialize_i128,
                    deserialize_f32,
                    deserialize_f64,
                    deserialize_char,
                    deserialize_str,
                    deserialize_string,
                    deserialize_bytes,
                    deserialize_byte_buf,
                    deserialize_option,
                    deserialize_unit,
                    deserialize_seq,
                    deserialize_map,
                    deserialize_identifier,
                    deserialize_ignored_any
                )
            }

            fn deserialize_unit_struct<V>(
                self,
                name: &'static str,
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.deserialize_unit_struct(name, $wrapper(visitor))
            }

            fn deserialize_newtype_struct<V>(
                self,
                name: &'static str,
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.deserialize_newtype_struct(name, $wrapper(visitor))
            }

            fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.deserialize_tuple(len, $wrapper(visitor))
            }

            fn deserialize_tuple_struct<V>(
                self,
                name: &'static str,
                len: usize,
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0
                    .deserialize_tuple_struct(name, len, $wrapper(visitor))
            }

            fn deserialize_struct<V>(
                self,
                name: &'static str,
                fields: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.deserialize_struct(name, fields, $wrapper(visitor))
            }

            fn deserialize_enum<V>(
                self,
                name: &'static str,
                variants: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.deserialize_enum(name, variants, $wrapper(visitor))
            }

            fn is_human_readable(&self) -> bool {
                $is_human_readable
            }
        }

        impl<'de, D> Visitor<'de> for $wrapper<D>
        where
            D: Visitor<'de>,
        {
            type Value = D::Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                self.0.expecting(formatter)
            }

            fn visit_bool<E>(self, v: bool) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_bool(v)
            }

            fn visit_i8<E>(self, v: i8) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_i8(v)
            }

            fn visit_i16<E>(self, v: i16) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_i16(v)
            }

            fn visit_i32<E>(self, v: i32) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_i32(v)
            }

            fn visit_i64<E>(self, v: i64) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_i64(v)
            }

            fn visit_i128<E>(self, v: i128) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_i128(v)
            }

            fn visit_u8<E>(self, v: u8) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_u8(v)
            }

            fn visit_u16<E>(self, v: u16) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_u16(v)
            }

            fn visit_u32<E>(self, v: u32) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_u32(v)
            }

            fn visit_u64<E>(self, v: u64) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_u64(v)
            }

            fn visit_u128<E>(self, v: u128) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_u128(v)
            }

            fn visit_f32<E>(self, v: f32) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_f32(v)
            }

            fn visit_f64<E>(self, v: f64) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_f64(v)
            }

            fn visit_char<E>(self, v: char) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_char(v)
            }

            fn visit_str<E>(self, v: &str) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_str(v)
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_borrowed_str(v)
            }

            fn visit_string<E>(self, v: String) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_string(v)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_bytes(v)
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_borrowed_bytes(v)
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_byte_buf(v)
            }

            fn visit_none<E>(self) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_none()
            }

            fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
            where
                D2: Deserializer<'de>,
            {
                self.0.visit_some($wrapper(deserializer))
            }

            fn visit_unit<E>(self) -> Result<D::Value, E>
            where
                E: Error,
            {
                self.0.visit_unit()
            }

            fn visit_newtype_struct<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
            where
                D2: Deserializer<'de>,
            {
                self.0.visit_newtype_struct($wrapper(deserializer))
            }

            fn visit_seq<V>(self, seq: V) -> Result<D::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                self.0.visit_seq($wrapper(seq))
            }

            fn visit_map<V>(self, map: V) -> Result<D::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                self.0.visit_map($wrapper(map))
            }

            fn visit_enum<V>(self, data: V) -> Result<D::Value, V::Error>
            where
                V: EnumAccess<'de>,
            {
                self.0.visit_enum($wrapper(data))
            }
        }

        impl<'de, D> SeqAccess<'de> for $wrapper<D>
        where
            D: SeqAccess<'de>,
        {
            type Error = D::Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, D::Error>
            where
                T: DeserializeSeed<'de>,
            {
                self.0.next_element_seed($wrapper(seed))
            }

            fn size_hint(&self) -> Option<usize> {
                self.0.size_hint()
            }
        }

        impl<'de, D> MapAccess<'de> for $wrapper<D>
        where
            D: MapAccess<'de>,
        {
            type Error = D::Error;

            fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, D::Error>
            where
                K: DeserializeSeed<'de>,
            {
                self.0.next_key_seed($wrapper(seed))
            }

            fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, D::Error>
            where
                V: DeserializeSeed<'de>,
            {
                self.0.next_value_seed($wrapper(seed))
            }

            fn next_entry_seed<K, V>(
                &mut self,
                kseed: K,
                vseed: V,
            ) -> Result<Option<(K::Value, V::Value)>, D::Error>
            where
                K: DeserializeSeed<'de>,
                V: DeserializeSeed<'de>,
            {
                self.0.next_entry_seed($wrapper(kseed), $wrapper(vseed))
            }

            fn size_hint(&self) -> Option<usize> {
                self.0.size_hint()
            }
        }

        impl<'de, D> EnumAccess<'de> for $wrapper<D>
        where
            D: EnumAccess<'de>,
        {
            type Error = D::Error;
            type Variant = $wrapper<D::Variant>;

            fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
            where
                V: DeserializeSeed<'de>,
            {
                self.0
                    .variant_seed($wrapper(seed))
                    .map(|(value, variant)| (value, $wrapper(variant)))
            }
        }

        impl<'de, D> VariantAccess<'de> for $wrapper<D>
        where
            D: VariantAccess<'de>,
        {
            type Error = D::Error;

            fn unit_variant(self) -> Result<(), D::Error> {
                self.0.unit_variant()
            }

            fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, D::Error>
            where
                T: DeserializeSeed<'de>,
            {
                self.0.newtype_variant_seed($wrapper(seed))
            }

            fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.tuple_variant(len, $wrapper(visitor))
            }

            fn struct_variant<V>(
                self,
                fields: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, D::Error>
            where
                V: Visitor<'de>,
            {
                self.0.struct_variant(fields, $wrapper(visitor))
            }
        }
    };
}

impl_deserializer!(Readable, true);
impl_deserializer!(Compact, false);
