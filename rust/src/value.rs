//! The JSON value model.
//!
//! A restored document is built from the types JSON has and no others. Rust has
//! no untyped map and no dynamic value of its own, so the model is an enum, and
//! objects are an ordered type rather than a `HashMap` - a hash map has no
//! order, and this format promises to give key order back exactly as it
//! arrived.

use std::collections::HashMap;
use std::fmt;

use crate::format::{format_json, format_json_indent};

/// A JSON value.
///
/// Numbers keep three shapes rather than one. Text read by [`crate::parse`]
/// always becomes [`Value::Float`], as it does in JavaScript and in every other
/// implementation of this format; [`Value::Int`] and [`Value::Uint`] are there
/// so that a 64-bit integer built in Rust survives with its exact value rather
/// than being folded through a double.
///
/// A non-finite float has no JSON spelling and is written as `null`, which is
/// what JSON writers everywhere do with one.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// `null`.
    #[default]
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A signed integer, written with its exact 64-bit value.
    Int(i64),
    /// An unsigned integer, written with its exact 64-bit value.
    Uint(u64),
    /// A double. Non-finite values are written as `null`.
    Float(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object, which remembers the order its keys arrived in.
    Object(Object),
}

impl Value {
    /// An empty object.
    pub fn object() -> Value {
        Value::Object(Object::new())
    }

    /// An empty array.
    pub fn array() -> Value {
        Value::Array(Vec::new())
    }

    /// Whether this is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Whether this is a number, in any of the three shapes.
    pub fn is_number(&self) -> bool {
        matches!(self, Value::Int(_) | Value::Uint(_) | Value::Float(_))
    }

    /// The boolean this holds, or `None`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The string this holds, or `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    /// This number as a double, whatever shape it is held in, or `None`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(value) => Some(*value as f64),
            Value::Uint(value) => Some(*value as f64),
            Value::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// This number as an `i64`, when it is one exactly, or `None`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            Value::Uint(value) => i64::try_from(*value).ok(),
            Value::Float(value) => {
                let whole = *value as i64;
                if whole as f64 == *value {
                    Some(whole)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// The array this holds, or `None`.
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    /// The object this holds, or `None`.
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    /// The object this holds, to change, or `None`.
    pub fn as_object_mut(&mut self) -> Option<&mut Object> {
        match self {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }

    /// The member of this object stored under `key` - `None` for this value not
    /// being an object as much as for the key being absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|object| object.get(key))
    }

    /// This value as JSON text, written the way [`crate::format_json`] writes
    /// it.
    pub fn to_json(&self) -> String {
        format_json(self)
    }

    /// This value as indented JSON text.
    pub fn to_json_indent(&self, indent: usize) -> String {
        format_json_indent(self, indent)
    }
}

/// Writes the value as compact JSON text, so `println!("{value}")` is the
/// document itself.
impl fmt::Display for Value {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.to_json())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Value {
        Value::Bool(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Value {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Value {
        Value::String(value.to_owned())
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Value {
        Value::Object(value)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(values: Vec<T>) -> Value {
        Value::Array(values.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value> + Clone> From<&[T]> for Value {
    fn from(values: &[T]) -> Value {
        Value::Array(values.iter().cloned().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Value {
        match value {
            Some(value) => value.into(),
            None => Value::Null,
        }
    }
}

impl From<&Value> for Value {
    fn from(value: &Value) -> Value {
        value.clone()
    }
}

macro_rules! from_number {
    ($variant:ident, $($type:ty),*) => {$(
        impl From<$type> for Value {
            fn from(value: $type) -> Value {
                Value::$variant(value as _)
            }
        }
    )*};
}

from_number!(Int, i8, i16, i32, i64, isize);
from_number!(Uint, u8, u16, u32, u64, usize);
from_number!(Float, f32, f64);

impl<K: Into<String>, V: Into<Value>> FromIterator<(K, V)> for Object {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(members: I) -> Object {
        let mut out = Object::new();
        for (key, value) in members {
            out.insert(key.into(), value.into());
        }
        out
    }
}

impl<T: Into<Value>> FromIterator<T> for Value {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Value {
        Value::Array(items.into_iter().map(Into::into).collect())
    }
}

/// Where a linear scan stops being the cheaper way to find a key.
const INDEX_THRESHOLD: usize = 12;

/// A JSON object that remembers the order its keys arrived in.
///
/// A `HashMap` does not, and a `BTreeMap` remembers the wrong one, so a document
/// decoded into either comes back with its members shuffled. This format
/// promises the opposite - the same values and the same key order - so objects
/// are this type instead.
///
/// ```
/// use json_compress::Object;
///
/// let row = Object::new().with("id", 1).with("name", "Ada");
///
/// assert_eq!(row.len(), 2);
/// assert_eq!(row.keys().collect::<Vec<_>>(), ["id", "name"]);
/// assert_eq!(row.get("name").and_then(|value| value.as_str()), Some("Ada"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Object {
    keys: Vec<String>,
    values: Vec<Value>,
    /// Built only once an object is large enough to be worth hashing. Documents
    /// in this format are mostly small objects - a row of a table, a nested
    /// record - where a scan over a handful of keys beats a map, and the map's
    /// own allocation costs more than the scan ever will.
    index: Option<HashMap<String, usize>>,
}

impl Object {
    /// An empty object.
    pub fn new() -> Object {
        Object::default()
    }

    /// An empty object with room for `capacity` members.
    pub fn with_capacity(capacity: usize) -> Object {
        Object {
            keys: Vec::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
            index: if capacity > INDEX_THRESHOLD {
                Some(HashMap::with_capacity(capacity))
            } else {
                None
            },
        }
    }

    /// The number of members.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether there are no members at all.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The position of a key, or `None`.
    pub fn position(&self, key: &str) -> Option<usize> {
        match &self.index {
            Some(index) => index.get(key).copied(),
            None => self.keys.iter().position(|name| name == key),
        }
    }

    /// Whether a key is present. A key present with a null value and a key that
    /// is absent are different things, here as in the format itself.
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    /// The value stored under `key`, or `None`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position(key).map(|at| &self.values[at])
    }

    /// The value stored under `key`, to change, or `None`.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self.position(key) {
            Some(at) => Some(&mut self.values[at]),
            None => None,
        }
    }

    /// The member at `position`, counted from zero in key order.
    ///
    /// This is the allocation-free way to walk an object:
    ///
    /// ```
    /// # use json_compress::Object;
    /// # let object = Object::new().with("a", 1);
    /// for at in 0..object.len() {
    ///     let (key, value) = object.at(at).unwrap();
    ///     println!("{key} = {value}");
    /// }
    /// ```
    pub fn at(&self, position: usize) -> Option<(&str, &Value)> {
        if position >= self.keys.len() {
            return None;
        }
        Some((&self.keys[position], &self.values[position]))
    }

    /// The member names, in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.keys.iter().map(String::as_str)
    }

    /// The values, in key order.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }

    /// Every member, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.keys.iter().map(String::as_str).zip(self.values.iter())
    }

    /// Stores a value, appending the key when it is new and leaving it where it
    /// is when it is not - the same rule `JSON.parse` follows for a repeated
    /// key. Returns the value that was there before, if any.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Option<Value> {
        let key = key.into();
        if let Some(at) = self.position(&key) {
            return Some(std::mem::replace(&mut self.values[at], value.into()));
        }

        self.values.push(value.into());
        match &mut self.index {
            Some(index) => {
                index.insert(key.clone(), self.keys.len());
                self.keys.push(key);
            }
            None => {
                self.keys.push(key);
                if self.keys.len() > INDEX_THRESHOLD {
                    self.build_index();
                }
            }
        }
        None
    }

    /// [`Object::insert`], returning the object, so members can be chained on:
    ///
    /// ```
    /// # use json_compress::Object;
    /// let row = Object::new().with("id", 1).with("name", "Ada");
    /// assert_eq!(row.len(), 2);
    /// ```
    pub fn with(mut self, key: impl Into<String>, value: impl Into<Value>) -> Object {
        self.insert(key, value);
        self
    }

    /// Removes a member and returns its value. Removing a key that is not there
    /// does nothing.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let at = self.position(key)?;
        self.keys.remove(at);
        let value = self.values.remove(at);
        if self.index.is_some() {
            // Every position after the hole has moved, so the index is rebuilt
            // rather than patched.
            self.build_index();
        }
        Some(value)
    }

    /// Removes every member.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.values.clear();
        self.index = None;
    }

    fn build_index(&mut self) {
        let mut index = HashMap::with_capacity(self.keys.len() * 2);
        for (at, key) in self.keys.iter().enumerate() {
            index.insert(key.clone(), at);
        }
        self.index = Some(index);
    }
}

/// Two objects are equal when they carry the same members in the same order.
impl PartialEq for Object {
    fn eq(&self, other: &Object) -> bool {
        self.keys == other.keys && self.values == other.values
    }
}

impl IntoIterator for Object {
    type Item = (String, Value);
    type IntoIter = std::iter::Zip<std::vec::IntoIter<String>, std::vec::IntoIter<Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.keys.into_iter().zip(self.values)
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a str, &'a Value);
    type IntoIter = std::iter::Zip<
        std::iter::Map<std::slice::Iter<'a, String>, fn(&'a String) -> &'a str>,
        std::slice::Iter<'a, Value>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        let as_str: fn(&'a String) -> &'a str = String::as_str;
        self.keys.iter().map(as_str).zip(self.values.iter())
    }
}

impl fmt::Display for Object {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = String::new();
        crate::format::write_object(&mut text, self, "", 0);
        out.write_str(&text)
    }
}
