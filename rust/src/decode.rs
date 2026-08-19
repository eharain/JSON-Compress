//! The decoder.
//!
//! A straight structural walk of the encoded document, resolving catalogue
//! references as it goes. It is deliberately strict: anything that cannot be
//! mapped back onto the original - an out-of-range token, a stray hole marker, a
//! table whose rows do not line up with its columns - is an error rather than
//! something quietly handed back nearly right.

use crate::constants::*;
use crate::error::{Error, Result};
use crate::parse::MAX_PARSE_DEPTH;
use crate::table::{COL_CONST, COL_DICT, NULL};
use crate::tokens::{from_base62, from_key_token};
use crate::value::{Object, Value};

/// What a single table node may claim its row count is.
///
/// A table this large is not a document anybody wrote; it is a corrupt or
/// hostile `~n`, and refusing it keeps a bad byte from turning into an
/// allocation the process cannot survive.
const MAX_TABLE_ROWS: i64 = 1 << 26;

/// The two catalogues, which is all the decoder needs to carry.
struct Context<'a> {
    keys: Vec<&'a str>,
    strings: Vec<&'a str>,
}

/// Whether a value looks like a json-compress envelope this build can read.
///
/// ```
/// use json_compress::{compress, is_compressed, parse};
///
/// let source = parse(r#"{"a": 1}"#).unwrap();
///
/// assert!(!is_compressed(&source));
/// assert!(is_compressed(&compress(&source).unwrap()));
/// ```
pub fn is_compressed(value: &Value) -> bool {
    let object = match value.as_object() {
        Some(object) => object,
        None => return false,
    };
    match object.get(ENV_VERSION).and_then(whole_number) {
        Some(version) if version == FORMAT_VERSION => {}
        _ => return false,
    }
    matches!(object.get(ENV_KEYS), Some(Value::Array(_)))
}

/// Restores the original value from a json-compress envelope.
///
/// ```
/// use json_compress::{compress, decompress, parse};
///
/// let source = parse(r#"[{"id": 1}, {"id": 2}]"#).unwrap();
///
/// assert_eq!(decompress(&compress(&source).unwrap()).unwrap(), source);
/// ```
pub fn decompress(envelope: &Value) -> Result<Value> {
    let object = envelope
        .as_object()
        .ok_or_else(|| Error::decode("expected a compressed envelope object"))?;

    let version = object.get(ENV_VERSION).ok_or_else(|| {
        Error::decode(format!(
            "this is not a compressed document (no \"{ENV_VERSION}\" version field)"
        ))
    })?;
    if whole_number(version) != Some(FORMAT_VERSION) {
        return Err(Error::decode(format!(
            "format version {version} is not supported by this build (expected {FORMAT_VERSION})"
        )));
    }

    let keys = catalogue_of(object.get(ENV_KEYS), "key")?;
    let strings = match object.get(ENV_STRINGS) {
        Some(Value::Null) | None => Vec::new(),
        found => catalogue_of(found, "string")?,
    };

    let data = object.get(ENV_DATA).unwrap_or(&NULL);
    decode_value(data, &Context { keys, strings }, 0)
}

fn catalogue_of<'a>(value: Option<&'a Value>, what: &str) -> Result<Vec<&'a str>> {
    let items = match value {
        Some(Value::Array(items)) => items,
        _ => {
            return Err(Error::decode(format!(
                "the {what} catalogue is missing or is not an array"
            )))
        }
    };

    let mut out = Vec::with_capacity(items.len());
    for (at, item) in items.iter().enumerate() {
        match item {
            Value::String(text) => out.push(text.as_str()),
            _ => {
                return Err(Error::decode(format!(
                    "the {what} catalogue holds something that is not a string at index {at}"
                )))
            }
        }
    }
    Ok(out)
}

fn decode_value(node: &Value, ctx: &Context<'_>, depth: usize) -> Result<Value> {
    if depth > MAX_PARSE_DEPTH {
        return Err(Error::TooDeep(MAX_PARSE_DEPTH));
    }

    match node {
        Value::String(text) => Ok(Value::String(decode_string(text, ctx)?)),

        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(decode_value(item, ctx, depth + 1)?);
            }
            Ok(Value::Array(out))
        }

        Value::Object(object) => {
            if object.contains_key(TABLE_COLUMNS) {
                return decode_table(object, ctx, depth);
            }
            let mut out = Object::with_capacity(object.len());
            for (token, item) in object.iter() {
                let key = decode_key(token, ctx)?;
                let value = decode_value(item, ctx, depth + 1)?;
                out.insert(key, value);
            }
            Ok(Value::Object(out))
        }

        other => Ok(other.clone()),
    }
}

fn decode_string(value: &str, ctx: &Context<'_>) -> Result<String> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes[0] != b'~' {
        return Ok(value.to_owned());
    }
    if bytes.len() == 1 {
        return Err(Error::decode(
            "a row hole marker appeared outside a table row",
        ));
    }
    if bytes[1] == b'~' {
        return Ok(value[1..].to_owned());
    }
    match from_base62(&value[1..]) {
        Some(at) if at < ctx.strings.len() => Ok(ctx.strings[at].to_owned()),
        _ => Err(Error::decode(format!(
            "string reference \"{value}\" is out of range"
        ))),
    }
}

fn decode_key(token: &str, ctx: &Context<'_>) -> Result<String> {
    match from_key_token(token) {
        Some(at) if at < ctx.keys.len() => Ok(ctx.keys[at].to_owned()),
        _ => Err(Error::decode(format!(
            "key token \"{token}\" is out of range"
        ))),
    }
}

fn decode_table(node: &Object, ctx: &Context<'_>, depth: usize) -> Result<Value> {
    let raw_columns = match node.get(TABLE_COLUMNS) {
        Some(Value::Array(items)) => items,
        _ => return Err(Error::decode("table column list is not an array")),
    };

    let mut columns = Vec::with_capacity(raw_columns.len());
    for item in raw_columns {
        match whole_number(item) {
            Some(at) if at >= 0 && (at as usize) < ctx.keys.len() => {
                columns.push(ctx.keys[at as usize])
            }
            _ => {
                return Err(Error::decode(format!(
                    "table column index {item} is out of range"
                )))
            }
        }
    }

    let mut encodings = vec![0i64; columns.len()];
    match node.get(TABLE_ENCODINGS) {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) if items.len() == columns.len() => {
            for (at, item) in items.iter().enumerate() {
                encodings[at] = whole_number(item).ok_or_else(|| {
                    Error::decode(format!("table encoding {item} is not a column encoding"))
                })?;
            }
        }
        _ => {
            return Err(Error::decode(
                "table encoding list does not match its columns",
            ))
        }
    }

    let mut constants = Vec::new();
    match node.get(TABLE_CONSTANTS) {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items {
                constants.push(decode_value(item, ctx, depth + 1)?);
            }
        }
        _ => return Err(Error::decode("table constant list is not an array")),
    }

    let matrix = match node.get(TABLE_ROWS) {
        None | Some(Value::Null) => None,
        Some(Value::Array(items)) => Some(items),
        _ => return Err(Error::decode("table rows are not an array")),
    };

    let row_count = match matrix {
        Some(items) => items.len(),
        None => match node.get(TABLE_ROW_COUNT) {
            None => 0,
            Some(found) => match whole_number(found) {
                Some(count) if count > MAX_TABLE_ROWS => {
                    return Err(Error::decode(format!(
                        "table row count {count} is larger than this build will read"
                    )))
                }
                Some(count) if count >= 0 => count as usize,
                _ => {
                    return Err(Error::decode(format!(
                        "table row count {found} is not a row count"
                    )))
                }
            },
        },
    };

    let mut out = Vec::with_capacity(row_count);
    for at in 0..row_count {
        let cells: &[Value] = match matrix {
            Some(items) => match &items[at] {
                Value::Array(cells) => cells,
                _ => return Err(Error::decode(format!("table row {at} is not an array"))),
            },
            None => &[],
        };

        let mut row = Object::with_capacity(columns.len());
        let (mut constant_at, mut cell_at) = (0usize, 0usize);

        for (column, name) in columns.iter().enumerate() {
            if encodings[column] == COL_CONST {
                if constant_at >= constants.len() {
                    return Err(Error::decode(
                        "table has fewer constants than constant columns",
                    ));
                }
                row.insert(*name, constants[constant_at].clone());
                constant_at += 1;
                continue;
            }

            // A row shorter than the column list is missing everything past its
            // end.
            let cell = cells.get(cell_at);
            cell_at += 1;

            if encodings[column] == COL_DICT {
                let found = match cell {
                    None => continue,
                    Some(found) => found,
                };
                if found.is_null() {
                    row.insert(*name, Value::Null);
                    continue;
                }
                match whole_number(found) {
                    Some(index) if index == DICT_HOLE => continue,
                    Some(index) if index >= 0 && (index as usize) < ctx.strings.len() => {
                        row.insert(*name, Value::String(ctx.strings[index as usize].to_owned()));
                    }
                    _ => {
                        return Err(Error::decode(format!(
                            "dictionary index {found} is out of range"
                        )))
                    }
                }
                continue;
            }

            match cell {
                None => continue,
                Some(Value::String(text)) if text == HOLE => continue,
                Some(found) => {
                    let value = decode_value(found, ctx, depth + 1)?;
                    row.insert(*name, value);
                }
            }
        }

        out.push(Value::Object(row));
    }

    Ok(Value::Array(out))
}

/// Reads an integer out of whatever a JSON reader turned it into: a double from
/// parsed text, or one of the exact integer shapes from a value that never left
/// the process.
fn whole_number(value: &Value) -> Option<i64> {
    match value {
        Value::Int(number) => Some(*number),
        Value::Uint(number) => i64::try_from(*number).ok(),
        Value::Float(number) => {
            let whole = *number as i64;
            if whole as f64 == *number {
                Some(whole)
            } else {
                None
            }
        }
        _ => None,
    }
}
