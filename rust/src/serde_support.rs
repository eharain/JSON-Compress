//! Serde support, behind the `serde` feature.
//!
//! [`Value`] and [`Object`] are `Serialize` and `Deserialize` themselves, so an
//! envelope drops straight into anything that already speaks serde, and
//! [`to_value`] / [`from_value`] carry an arbitrary type into and out of the
//! value model - which is what makes `compress(&to_value(&my_records)?)` the
//! ordinary way to use this crate from Rust.
//!
//! Two rules come from the wire format rather than from serde, and are the ones
//! every implementation of this format shares: a non-finite number becomes
//! `null` rather than an error, and negative zero is written as zero. Both are
//! what JSON itself does with those values.

use std::fmt;

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use serde::ser::{self, Impossible, SerializeMap as _, SerializeSeq as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};
use crate::value::{Object, Value};

/* ------------------------------------------------------------------ *
 * Errors
 * ------------------------------------------------------------------ */

impl ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Error {
        Error::Conversion(message.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Error {
        Error::Conversion(message.to_string())
    }
}

/* ------------------------------------------------------------------ *
 * The model as serde types
 * ------------------------------------------------------------------ */

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(value) => serializer.serialize_bool(*value),
            Value::Int(value) => serializer.serialize_i64(*value),
            Value::Uint(value) => serializer.serialize_u64(*value),
            Value::Float(value) => serializer.serialize_f64(*value),
            Value::String(value) => serializer.serialize_str(value),
            Value::Array(items) => {
                let mut sequence = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    sequence.serialize_element(item)?;
                }
                sequence.end()
            }
            Value::Object(object) => object.serialize(serializer),
        }
    }
}

impl Serialize for Object {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (key, value) in self.iter() {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Value, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Object, D::Error> {
        match deserializer.deserialize_map(ValueVisitor)? {
            Value::Object(object) => Ok(object),
            _ => Err(de::Error::custom("expected a JSON object")),
        }
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("any JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(Value::Int(value))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(Value::Uint(value))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Value, E> {
        Ok(number(value))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        Deserialize::deserialize(deserializer)
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<Value, D::Error> {
        Deserialize::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> std::result::Result<Value, A::Error> {
        let mut out = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some(item) = access.next_element()? {
            out.push(item);
        }
        Ok(Value::Array(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> std::result::Result<Value, A::Error> {
        let mut out = Object::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((key, value)) = access.next_entry::<String, Value>()? {
            out.insert(key, value);
        }
        Ok(Value::Object(out))
    }
}

/// Folds away the two shapes JSON cannot spell.
fn number(value: f64) -> Value {
    if !value.is_finite() {
        return Value::Null;
    }
    if value == 0.0 {
        return Value::Float(0.0); // including negative zero
    }
    Value::Float(value)
}

/* ------------------------------------------------------------------ *
 * Into the model
 * ------------------------------------------------------------------ */

/// Reads any `Serialize` type into the value model, so it can be compressed.
///
/// ```
/// use serde::Serialize;
/// use json_compress::{to_value, to_string};
///
/// #[derive(Serialize)]
/// struct User { id: u32, name: &'static str, role: &'static str }
///
/// let users = vec![
///     User { id: 1, name: "Ada", role: "admin" },
///     User { id: 2, name: "Bob", role: "admin" },
/// ];
///
/// let small = to_string(&to_value(&users).unwrap()).unwrap();
/// assert!(small.starts_with(r#"{"jc":1,"k":["id","name","role"]"#));
/// ```
///
/// Struct fields keep the order they are declared in, which is the order they
/// will come back in.
pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value> {
    value.serialize(ValueSerializer)
}

/// Builds the model out of whatever serde hands it.
struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = SerializeArray;
    type SerializeTuple = SerializeArray;
    type SerializeTupleStruct = SerializeArray;
    type SerializeTupleVariant = SerializeArrayVariant;
    type SerializeMap = SerializeObject;
    type SerializeStruct = SerializeObject;
    type SerializeStructVariant = SerializeObjectVariant;

    fn serialize_bool(self, value: bool) -> Result<Value> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Value> {
        Ok(Value::Int(value.into()))
    }

    fn serialize_i16(self, value: i16) -> Result<Value> {
        Ok(Value::Int(value.into()))
    }

    fn serialize_i32(self, value: i32) -> Result<Value> {
        Ok(Value::Int(value.into()))
    }

    fn serialize_i64(self, value: i64) -> Result<Value> {
        Ok(Value::Int(value))
    }

    fn serialize_i128(self, value: i128) -> Result<Value> {
        match i64::try_from(value) {
            Ok(value) => Ok(Value::Int(value)),
            Err(_) => Err(Error::Conversion(format!(
                "{value} does not fit in the 64 bits this format carries"
            ))),
        }
    }

    fn serialize_u8(self, value: u8) -> Result<Value> {
        Ok(Value::Uint(value.into()))
    }

    fn serialize_u16(self, value: u16) -> Result<Value> {
        Ok(Value::Uint(value.into()))
    }

    fn serialize_u32(self, value: u32) -> Result<Value> {
        Ok(Value::Uint(value.into()))
    }

    fn serialize_u64(self, value: u64) -> Result<Value> {
        Ok(Value::Uint(value))
    }

    fn serialize_u128(self, value: u128) -> Result<Value> {
        match u64::try_from(value) {
            Ok(value) => Ok(Value::Uint(value)),
            Err(_) => Err(Error::Conversion(format!(
                "{value} does not fit in the 64 bits this format carries"
            ))),
        }
    }

    fn serialize_f32(self, value: f32) -> Result<Value> {
        Ok(number(value.into()))
    }

    fn serialize_f64(self, value: f64) -> Result<Value> {
        Ok(number(value))
    }

    fn serialize_char(self, value: char) -> Result<Value> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Value> {
        Ok(Value::String(value.to_owned()))
    }

    /// Bytes become an array of numbers, which is what a `Vec<u8>` serialises to
    /// anyway and what JSON can carry without a convention either end has to
    /// agree on.
    fn serialize_bytes(self, value: &[u8]) -> Result<Value> {
        Ok(Value::Array(
            value.iter().map(|byte| Value::Uint(*byte as u64)).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Value> {
        Ok(Value::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value> {
        Ok(Value::Object(
            Object::new().with(variant, value.serialize(ValueSerializer)?),
        ))
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<SerializeArray> {
        Ok(SerializeArray {
            items: Vec::with_capacity(length.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, length: usize) -> Result<SerializeArray> {
        self.serialize_seq(Some(length))
    }

    fn serialize_tuple_struct(self, _name: &'static str, length: usize) -> Result<SerializeArray> {
        self.serialize_seq(Some(length))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<SerializeArrayVariant> {
        Ok(SerializeArrayVariant {
            variant,
            items: Vec::with_capacity(length),
        })
    }

    fn serialize_map(self, length: Option<usize>) -> Result<SerializeObject> {
        Ok(SerializeObject {
            object: Object::with_capacity(length.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, length: usize) -> Result<SerializeObject> {
        self.serialize_map(Some(length))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<SerializeObjectVariant> {
        Ok(SerializeObjectVariant {
            variant,
            object: Object::with_capacity(length),
        })
    }
}

struct SerializeArray {
    items: Vec<Value>,
}

impl ser::SerializeSeq for SerializeArray {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Array(self.items))
    }
}

impl ser::SerializeTuple for SerializeArray {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SerializeArray {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value> {
        ser::SerializeSeq::end(self)
    }
}

struct SerializeArrayVariant {
    variant: &'static str,
    items: Vec<Value>,
}

impl ser::SerializeTupleVariant for SerializeArrayVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Object(
            Object::new().with(self.variant, Value::Array(self.items)),
        ))
    }
}

struct SerializeObject {
    object: Object,
    key: Option<String>,
}

impl ser::SerializeMap for SerializeObject {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        self.key = Some(key.serialize(KeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let key = self
            .key
            .take()
            .ok_or_else(|| Error::Conversion("a map value arrived before its key".to_owned()))?;
        self.object.insert(key, value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Object(self.object))
    }
}

impl ser::SerializeStruct for SerializeObject {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.object.insert(key, value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Object(self.object))
    }
}

struct SerializeObjectVariant {
    variant: &'static str,
    object: Object,
}

impl ser::SerializeStructVariant for SerializeObjectVariant {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.object.insert(key, value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(Value::Object(
            Object::new().with(self.variant, Value::Object(self.object)),
        ))
    }
}

/// A JSON object key is a string. Numbers and booleans are spelled out rather
/// than refused, so a `HashMap<u32, _>` goes through; anything else is a shape
/// JSON has no way to write down.
struct KeySerializer;

fn not_a_key<T>() -> Result<T> {
    Err(Error::Conversion(
        "a JSON object key has to be a string, a number or a boolean".to_owned(),
    ))
}

impl Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;

    type SerializeSeq = Impossible<String, Error>;
    type SerializeTuple = Impossible<String, Error>;
    type SerializeTupleStruct = Impossible<String, Error>;
    type SerializeTupleVariant = Impossible<String, Error>;
    type SerializeMap = Impossible<String, Error>;
    type SerializeStruct = Impossible<String, Error>;
    type SerializeStructVariant = Impossible<String, Error>;

    fn serialize_bool(self, value: bool) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_i8(self, value: i8) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_i16(self, value: i16) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_i32(self, value: i32) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_i64(self, value: i64) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_u8(self, value: u8) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_u16(self, value: u16) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_u32(self, value: u32) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_u64(self, value: u64) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_f32(self, value: f32) -> Result<String> {
        Ok(crate::format::format_float(value.into()))
    }

    fn serialize_f64(self, value: f64) -> Result<String> {
        Ok(crate::format::format_float(value))
    }

    fn serialize_char(self, value: char) -> Result<String> {
        Ok(value.to_string())
    }

    fn serialize_str(self, value: &str) -> Result<String> {
        Ok(value.to_owned())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<String> {
        not_a_key()
    }

    fn serialize_none(self) -> Result<String> {
        not_a_key()
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<String> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<String> {
        not_a_key()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String> {
        not_a_key()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<String> {
        Ok(variant.to_owned())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String> {
        not_a_key()
    }

    fn serialize_seq(self, _length: Option<usize>) -> Result<Self::SerializeSeq> {
        not_a_key()
    }

    fn serialize_tuple(self, _length: usize) -> Result<Self::SerializeTuple> {
        not_a_key()
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        not_a_key()
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        not_a_key()
    }

    fn serialize_map(self, _length: Option<usize>) -> Result<Self::SerializeMap> {
        not_a_key()
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStruct> {
        not_a_key()
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStructVariant> {
        not_a_key()
    }
}

/* ------------------------------------------------------------------ *
 * Out of the model
 * ------------------------------------------------------------------ */

/// Reads a restored value back into any `Deserialize` type.
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use json_compress::{from_str, from_value, to_string, to_value};
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// struct User { id: u32, name: String }
///
/// let users = vec![User { id: 1, name: "Ada".into() }, User { id: 2, name: "Bob".into() }];
///
/// let small = to_string(&to_value(&users).unwrap()).unwrap();
/// let back: Vec<User> = from_value(from_str(&small).unwrap()).unwrap();
///
/// assert_eq!(back, users);
/// ```
///
/// A number read from JSON text is a double, because that is what the format
/// carries; an integer field is filled from one when it holds a whole number
/// exactly, and refused when it does not.
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    T::deserialize(value)
}

macro_rules! deserialize_integer {
    ($($method:ident => $visit:ident, $type:ty),* $(,)?) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let number = whole_number(&self)?;
            match <$type>::try_from(number) {
                Ok(number) => visitor.$visit(number),
                Err(_) => Err(Error::Conversion(format!(
                    "{number} does not fit in a {}", stringify!($type)
                ))),
            }
        }
    )*};
}

impl<'de> Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(value) => visitor.visit_bool(value),
            Value::Int(value) => visitor.visit_i64(value),
            Value::Uint(value) => visitor.visit_u64(value),
            Value::Float(value) => visitor.visit_f64(value),
            Value::String(value) => visitor.visit_string(value),
            Value::Array(items) => visitor.visit_seq(SeqReader {
                items: items.into_iter(),
            }),
            Value::Object(object) => visitor.visit_map(MapReader::new(object)),
        }
    }

    deserialize_integer! {
        deserialize_i8 => visit_i8, i8,
        deserialize_i16 => visit_i16, i16,
        deserialize_i32 => visit_i32, i32,
        deserialize_i64 => visit_i64, i64,
        deserialize_u8 => visit_u8, u8,
        deserialize_u16 => visit_u16, u16,
        deserialize_u32 => visit_u32, u32,
        deserialize_u64 => visit_u64, u64,
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.as_f64() {
            Some(number) => visitor.visit_f32(number as f32),
            None => self.deserialize_any(visitor),
        }
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.as_f64() {
            Some(number) => visitor.visit_f64(number),
            None => self.deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self {
            Value::String(variant) => visitor.visit_enum(EnumReader {
                variant,
                value: None,
            }),
            Value::Object(object) if object.len() == 1 => {
                let (variant, value) = object.at(0).unwrap();
                visitor.visit_enum(EnumReader {
                    variant: variant.to_owned(),
                    value: Some(value.clone()),
                })
            }
            _ => Err(Error::Conversion(
                "expected a string or a single-member object for an enum".to_owned(),
            )),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool char str string bytes byte_buf unit seq tuple tuple_struct map
        struct identifier ignored_any
    }
}

fn whole_number(value: &Value) -> Result<i64> {
    match value {
        Value::Int(number) => Ok(*number),
        Value::Uint(number) => i64::try_from(*number)
            .map_err(|_| Error::Conversion(format!("{number} does not fit in an i64"))),
        Value::Float(number) => {
            let whole = *number as i64;
            if whole as f64 == *number {
                Ok(whole)
            } else {
                Err(Error::Conversion(format!("{number} is not a whole number")))
            }
        }
        other => Err(Error::Conversion(format!(
            "expected a number, found {}",
            shape(other)
        ))),
    }
}

fn shape(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Int(_) | Value::Uint(_) | Value::Float(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

struct SeqReader {
    items: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for SeqReader {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        match self.items.next() {
            Some(item) => seed.deserialize(item).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len())
    }
}

struct MapReader {
    members: std::vec::IntoIter<(String, Value)>,
    value: Option<Value>,
}

impl MapReader {
    fn new(object: Object) -> MapReader {
        MapReader {
            members: object.into_iter().collect::<Vec<_>>().into_iter(),
            value: None,
        }
    }
}

impl<'de> MapAccess<'de> for MapReader {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match self.members.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(KeyReader { key }).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let value = self.value.take().ok_or_else(|| {
            Error::Conversion("a map value was asked for before its key".to_owned())
        })?;
        seed.deserialize(value)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.members.len())
    }
}

/// A JSON object key is text, whatever the map was keyed by on the way in. This
/// reads the text back into whatever the map is keyed by on the way out, so a
/// `BTreeMap<u32, _>` survives the round trip its own `Serialize` made necessary.
struct KeyReader {
    key: String,
}

macro_rules! deserialize_key_as {
    ($($method:ident => $visit:ident, $type:ty),* $(,)?) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self.key.parse::<$type>() {
                Ok(parsed) => visitor.$visit(parsed),
                Err(_) => visitor.visit_string(self.key),
            }
        }
    )*};
}

impl<'de> Deserializer<'de> for KeyReader {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_string(self.key)
    }

    deserialize_key_as! {
        deserialize_bool => visit_bool, bool,
        deserialize_i8 => visit_i8, i8,
        deserialize_i16 => visit_i16, i16,
        deserialize_i32 => visit_i32, i32,
        deserialize_i64 => visit_i64, i64,
        deserialize_u8 => visit_u8, u8,
        deserialize_u16 => visit_u16, u16,
        deserialize_u32 => visit_u32, u32,
        deserialize_u64 => visit_u64, u64,
        deserialize_f32 => visit_f32, f32,
        deserialize_f64 => visit_f64, f64,
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.key
            .into_deserializer()
            .deserialize_enum(name, variants, visitor)
    }

    serde::forward_to_deserialize_any! {
        char str string bytes byte_buf unit unit_struct seq tuple tuple_struct
        map struct identifier ignored_any
    }
}

struct EnumReader {
    variant: String,
    value: Option<Value>,
}

impl<'de> EnumAccess<'de> for EnumReader {
    type Error = Error;
    type Variant = VariantReader;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, VariantReader)> {
        let variant = seed.deserialize(self.variant.into_deserializer())?;
        Ok((variant, VariantReader { value: self.value }))
    }
}

struct VariantReader {
    value: Option<Value>,
}

impl<'de> VariantAccess<'de> for VariantReader {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        match self.value {
            None => Ok(()),
            Some(_) => Err(Error::Conversion(
                "expected a unit variant, found one with a value".to_owned(),
            )),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        match self.value {
            Some(value) => seed.deserialize(value),
            None => Err(Error::Conversion(
                "expected a variant with a value, found a unit variant".to_owned(),
            )),
        }
    }

    fn tuple_variant<V: Visitor<'de>>(self, _length: usize, visitor: V) -> Result<V::Value> {
        match self.value {
            Some(Value::Array(items)) => visitor.visit_seq(SeqReader {
                items: items.into_iter(),
            }),
            _ => Err(Error::Conversion(
                "expected an array for a tuple variant".to_owned(),
            )),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self.value {
            Some(Value::Object(object)) => visitor.visit_map(MapReader::new(object)),
            _ => Err(Error::Conversion(
                "expected an object for a struct variant".to_owned(),
            )),
        }
    }
}
