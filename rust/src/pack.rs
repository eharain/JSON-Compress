//! The plain table pack.
//!
//! This is the smallest useful idea in the library, offered on its own because
//! it is often all you need and because the shape it produces - fields, data and
//! count - is trivial to consume from another language or straight out of a SQL
//! driver.
//!
//! [`pack`] turns an array of records into one field list and a matrix of rows.
//! [`unpack`] turns it back. Neither uses a catalogue, an envelope or a version
//! tag; what you get is a readable value you can eyeball in a network panel.

use std::collections::HashSet;

use crate::value::{Object, Value};

/// A record set with its keys lifted out of the rows.
///
/// ```
/// use json_compress::{pack, parse};
///
/// let records = parse(r#"[{"id": 1, "name": "Ada"}, {"id": 2, "name": "Bob"}]"#).unwrap();
/// let table = pack(&records).unwrap();
///
/// assert_eq!(table.fields, ["id", "name"]);
/// assert_eq!(table.count, 2);
/// assert_eq!(
///     table.to_value().to_json(),
///     r#"{"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}"#
/// );
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PackedTable {
    /// The column names, in the order the row arrays use.
    pub fields: Vec<String>,
    /// One array of values per record.
    pub data: Vec<Vec<Value>>,
    /// The number of records, so consumers need not measure.
    pub count: usize,
}

impl PackedTable {
    /// The table as a JSON value, ready to serialise.
    pub fn to_value(&self) -> Value {
        let mut out = Object::with_capacity(3);
        out.insert(
            "fields",
            Value::Array(
                self.fields
                    .iter()
                    .map(|field| Value::String(field.clone()))
                    .collect(),
            ),
        );
        out.insert(
            "data",
            Value::Array(
                self.data
                    .iter()
                    .map(|row| Value::Array(row.clone()))
                    .collect(),
            ),
        );
        out.insert("count", Value::Uint(self.count as u64));
        Value::Object(out)
    }

    /// Rebuilds the records this table was packed from.
    pub fn unpack(&self) -> Vec<Value> {
        self.data
            .iter()
            .map(|row| {
                let mut record = Object::with_capacity(self.fields.len());
                for (at, field) in self.fields.iter().enumerate() {
                    record.insert(field.clone(), row.get(at).cloned().unwrap_or(Value::Null));
                }
                Value::Object(record)
            })
            .collect()
    }
}

impl From<PackedTable> for Value {
    fn from(table: PackedTable) -> Value {
        table.to_value()
    }
}

/// Flattens an array of records into a field list and a rows matrix, or `None`
/// when the value is not an array of objects.
///
/// Fields are taken from the union of every record, in first-seen order, so a
/// record that is missing one contributes `null` in that column. That is the one
/// thing the plain pack gives up next to [`crate::compress`], which keeps a
/// missing key and a null one apart.
pub fn pack(records: &Value) -> Option<PackedTable> {
    let items = records.as_array()?;
    if items.is_empty() {
        return Some(PackedTable {
            fields: Vec::new(),
            data: Vec::new(),
            count: 0,
        });
    }

    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        rows.push(item.as_object()?);
    }

    // First seen wins, and the set is what keeps a wide record set from costing
    // a scan of every field already found for every field of every row.
    let mut seen: HashSet<&str> = HashSet::new();
    let mut fields: Vec<String> = Vec::new();
    for row in &rows {
        for key in row.keys() {
            if seen.insert(key) {
                fields.push(key.to_owned());
            }
        }
    }

    let data = rows
        .iter()
        .map(|row| {
            fields
                .iter()
                .map(|field| row.get(field).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect::<Vec<Vec<Value>>>();

    Some(PackedTable {
        fields,
        count: data.len(),
        data,
    })
}

/// Rebuilds the records [`pack`] flattened, or `None` when the value is not a
/// packed table.
pub fn unpack(packed: &Value) -> Option<Vec<Value>> {
    Some(packed_shape(packed)?.unpack())
}

/// Whether a value has the shape [`unpack`] knows how to read.
pub fn is_packed(value: &Value) -> bool {
    packed_shape(value).is_some()
}

/// Packs every result set in an array of result sets.
///
/// Database drivers that return several result sets from one call hand back an
/// array of arrays; this packs each inner one and leaves the outer one alone. A
/// set that cannot be packed is carried through as it was.
pub fn pack_all(result_sets: &Value) -> Option<Vec<Value>> {
    let sets = result_sets.as_array()?;
    Some(
        sets.iter()
            .map(|set| match pack(set) {
                Some(table) => table.to_value(),
                None => set.clone(),
            })
            .collect(),
    )
}

/// Unpacks every result set [`pack_all`] packed, carrying through anything that
/// was not a packed table.
pub fn unpack_all(result_sets: &Value) -> Option<Vec<Value>> {
    let sets = result_sets.as_array()?;
    Some(
        sets.iter()
            .map(|set| match unpack(set) {
                Some(records) => Value::Array(records),
                None => set.clone(),
            })
            .collect(),
    )
}

/// Reads the fields and rows out of a packed table however it arrived: the
/// struct this module produces, or the object another implementation of the
/// format wrote.
fn packed_shape(value: &Value) -> Option<PackedTable> {
    let object = value.as_object()?;
    let raw_fields = object.get("fields")?.as_array()?;
    let raw_data = object.get("data")?.as_array()?;

    let mut fields = Vec::with_capacity(raw_fields.len());
    for field in raw_fields {
        fields.push(field.as_str()?.to_owned());
    }

    let mut data = Vec::with_capacity(raw_data.len());
    for row in raw_data {
        data.push(row.as_array()?.clone());
    }

    Some(PackedTable {
        fields,
        count: data.len(),
        data,
    })
}

/// How a delimited column is written: how many characters to trim from each end,
/// and what separates the values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextShape {
    /// Characters to trim from the start.
    pub open: usize,
    /// Characters to trim from the end.
    pub close: usize,
    /// What separates the values.
    pub separator: String,
}

/// The shape [`text_to_array`] assumes when none is given, which reads a column
/// written as `"[[[1,2,3]]]"`.
impl Default for TextShape {
    fn default() -> TextShape {
        TextShape {
            open: 3,
            close: 3,
            separator: ",".to_owned(),
        }
    }
}

/// Reads a delimited-integer column back into an array of numbers.
///
/// Some databases return a set-valued column as a bracketed, comma-separated
/// string. This converts one such member in place, leaving an empty array where
/// the column was null or was not text at all.
///
/// ```
/// use json_compress::{text_to_array, Object, TextShape};
///
/// let mut record = Object::new().with("ids", "[[[1,2,3]]]");
/// text_to_array(&mut record, "ids", &TextShape::default());
///
/// assert_eq!(record.get("ids").unwrap().to_json(), "[1,2,3]");
/// ```
pub fn text_to_array(record: &mut Object, property: &str, shape: &TextShape) {
    if !record.contains_key(property) {
        return;
    }

    let separator = if shape.separator.is_empty() {
        ","
    } else {
        shape.separator.as_str()
    };

    let text = match record.get(property).and_then(Value::as_str) {
        Some(text) => text.to_owned(),
        None => {
            record.insert(property, Value::array());
            return;
        }
    };

    let characters: Vec<char> = text.chars().collect();
    if shape.open + shape.close >= characters.len() {
        record.insert(property, Value::array());
        return;
    }

    let inner: String = characters[shape.open..characters.len() - shape.close]
        .iter()
        .collect();
    if inner.is_empty() {
        record.insert(property, Value::array());
        return;
    }

    let values: Vec<Value> = inner
        .split(separator)
        .map(|part| match part.trim().parse::<f64>() {
            Ok(number) => Value::Float(number),
            // What a value that is not a number parses to.
            Err(_) => Value::Null,
        })
        .collect();
    record.insert(property, Value::Array(values));
}
