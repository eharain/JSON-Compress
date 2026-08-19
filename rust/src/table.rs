//! Columnar table planning.
//!
//! An array of like-shaped objects is the single most wasteful thing JSON does:
//! every key is spelled out again on every row. This decides, per array, whether
//! it is worth turning into a table - a column list plus a rows matrix - and how
//! each individual column should be encoded.

use std::collections::HashSet;

use crate::column_order::order_columns;
use crate::format::utf16_length;
use crate::options::Options;
use crate::value::{Object, Value};

/// Cells are ordinary encoded values.
pub(crate) const COL_RAW: i64 = 0;
/// Cells are integer indices into the string catalogue.
pub(crate) const COL_DICT: i64 = 1;
/// The column never changes; its value lives in `~cv`.
pub(crate) const COL_CONST: i64 = 2;

/// The decision taken about one array during the analysis pass.
pub(crate) struct TablePlan {
    /// Every column, in an order that suits every row.
    pub columns: Vec<String>,
    /// One of [`COL_RAW`], [`COL_DICT`] or [`COL_CONST`] per column.
    pub encodings: Vec<i64>,
    /// Values of the [`COL_CONST`] columns, in column order.
    pub constants: Vec<Value>,
    /// How many columns actually appear in the rows.
    pub variable_count: usize,
}

/// A non-finite float has no JSON spelling and is written as `null`. Reading one
/// as null here as well is what keeps the encoder's decisions consistent with
/// what it will actually write - a column of `NaN` is a constant column of
/// `null`, not a column of values that are never equal to each other.
pub(crate) static NULL: Value = Value::Null;

pub(crate) fn folded(value: &Value) -> &Value {
    match value {
        Value::Float(number) if !number.is_finite() => &NULL,
        _ => value,
    }
}

/// What one pass over the cells needs to gather to choose an encoding for a
/// column.
struct ColumnStats<'a> {
    present: usize,
    strings: usize,
    only_strings: bool,
    constant: bool,
    constant_value: Option<&'a Value>,
    total_length: usize,
    distinct: HashSet<&'a str>,
}

impl<'a> ColumnStats<'a> {
    fn new() -> ColumnStats<'a> {
        ColumnStats {
            present: 0,
            strings: 0,
            only_strings: true,
            constant: true,
            constant_value: None,
            total_length: 0,
            distinct: HashSet::new(),
        }
    }
}

/// Decides how to encode an array: a plan, or `None` to write the array element
/// by element.
pub(crate) fn plan_table<'a>(items: &'a [Value], options: &Options) -> Option<TablePlan> {
    if !options.tables {
        return None;
    }

    let row_count = items.len();
    if row_count < options.min_rows {
        return None;
    }

    let mut rows: Vec<&'a Object> = Vec::with_capacity(row_count);
    for item in items {
        match item {
            Value::Object(row) => rows.push(row),
            _ => return None,
        }
    }

    // One column order has to stand in for every row's key order. When no such
    // order exists the array is left as objects, which is fatter on the wire but
    // hands the document back exactly as it arrived.
    let columns = order_columns(&rows)?;

    let column_count = columns.len();
    if column_count == 0 {
        return None;
    }

    // One pass over the cells, gathering everything the encoding choice needs.
    let mut stats: Vec<ColumnStats<'a>> = (0..column_count).map(|_| ColumnStats::new()).collect();

    let mut holes = 0usize;
    for &row in &rows {
        for (at, column) in columns.iter().enumerate() {
            let stat = &mut stats[at];
            let value = match row.get(column) {
                Some(value) => folded(value),
                None => {
                    holes += 1;
                    stat.constant = false;
                    continue;
                }
            };

            stat.present += 1;
            if stat.present == 1 {
                stat.constant_value = Some(value);
            } else if stat.constant && !same_value(stat.constant_value.unwrap(), value) {
                stat.constant = false;
            }

            match value {
                Value::String(text) => {
                    stat.strings += 1;
                    stat.total_length += utf16_length(text);
                    stat.distinct.insert(text);
                }
                Value::Null => {}
                _ => stat.only_strings = false,
            }
        }
    }

    if holes as f64 / (row_count * column_count) as f64 > options.max_hole_ratio {
        return None;
    }

    let mut plan = TablePlan {
        columns,
        encodings: vec![COL_RAW; column_count],
        constants: Vec::new(),
        variable_count: 0,
    };

    for (at, stat) in stats.iter().enumerate() {
        let constant_value = stat.constant_value.unwrap_or(&NULL);
        let is_constant = options.constants
            && stat.constant
            && stat.present == row_count
            && is_primitive(constant_value);

        if is_constant {
            plan.encodings[at] = COL_CONST;
            plan.constants.push(constant_value.clone());
            continue;
        }

        plan.variable_count += 1;

        // A dictionary column pays for itself when the same handful of strings
        // keep coming back: each cell shrinks to a small integer, and the
        // strings move into the catalogue where they are written exactly once.
        let worth_dictionary = options.dict_columns
            && options.strings
            && stat.only_strings
            && stat.strings >= 3
            && !stat.distinct.is_empty()
            && stat.distinct.len() as f64 <= stat.strings as f64 * 0.6
            && stat.total_length as f64 / stat.strings as f64 >= 2.0;

        plan.encodings[at] = if worth_dictionary { COL_DICT } else { COL_RAW };
    }

    Some(plan)
}

/// True for a value that can safely be hoisted out of the rows and shared, which
/// rules out objects and arrays: every restored row would otherwise hold a copy
/// of one value rather than its own.
fn is_primitive(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// Compares two cells the way the constant-column test needs: by value for the
/// scalars a constant column may hold, and never equal for anything else, which
/// could not be hoisted into `~cv` anyway.
fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Uint(left), Value::Uint(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left == right,
        _ => false,
    }
}
