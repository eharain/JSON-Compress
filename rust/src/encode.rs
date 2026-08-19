//! The encoder.
//!
//! Two passes. The first walks the document and counts things: how often each
//! key is used, how often each string value appears, and which arrays are worth
//! turning into tables. The second builds the output using catalogues ordered by
//! those counts, so the most-used key gets the one-character token.
//!
//! Splitting it this way costs a second walk and buys a materially smaller
//! document, because token length is decided by frequency rather than by the
//! accident of which key the encoder happened to meet first.

use std::collections::{HashMap, HashSet};

use crate::constants::*;
use crate::error::{Error, Result};
use crate::format::utf16_length;
use crate::options::Options;
use crate::parse::MAX_COMPRESS_DEPTH;
use crate::table::{folded, plan_table, TablePlan, COL_CONST, COL_DICT, COL_RAW};
use crate::tokens::{base62_length, to_base62, to_key_token};
use crate::value::{Object, Value};

/// Packs a value into a json-compress envelope, with [`Options::default`].
///
/// The result is an ordinary JSON value, so it can be stored, posted or logged
/// anywhere plain JSON goes; [`crate::to_string`] does the same thing and hands
/// back the text.
///
/// ```
/// use json_compress::{compress, decompress, parse};
///
/// let source = parse(r#"[{"id": 1, "role": "admin"}, {"id": 2, "role": "admin"}]"#).unwrap();
/// let envelope = compress(&source).unwrap();
///
/// assert_eq!(envelope.to_json(), r#"{"jc":1,"k":["id","role"],"d":{"~t":[0,1],"~e":[0,2],"~cv":["admin"],"~r":[[1],[2]]}}"#);
/// assert_eq!(decompress(&envelope).unwrap(), source);
/// ```
pub fn compress(value: &Value) -> Result<Value> {
    compress_with(value, &Options::default())
}

/// [`compress`] with the encoder's choices spelled out.
///
/// Nothing here changes what a decoder has to do: an envelope written with one
/// set of options restores identically under any other.
pub fn compress_with(value: &Value, options: &Options) -> Result<Value> {
    options.validate()?;

    let mut analysis = Analysis::new(*options);
    analysis.value(value, 0)?;

    let keys = analysis.key_catalogue();
    let strings = analysis.string_catalogue();

    let mut builder = Encoder {
        keys,
        strings,
        plans: analysis.plans,
        next: 0,
    };
    let data = builder.value(value);

    let mut envelope = Object::with_capacity(4);
    envelope.insert(ENV_VERSION, Value::Int(FORMAT_VERSION));
    envelope.insert(ENV_KEYS, catalogue_array(&builder.keys.list));
    if !builder.strings.list.is_empty() {
        envelope.insert(ENV_STRINGS, catalogue_array(&builder.strings.list));
    }
    envelope.insert(ENV_DATA, data);
    Ok(Value::Object(envelope))
}

fn catalogue_array(entries: &[String]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|entry| Value::String(entry.clone()))
            .collect(),
    )
}

/* ------------------------------------------------------------------ *
 * Pass one - analysis
 * ------------------------------------------------------------------ */

/// The counting pass.
///
/// The JavaScript encoder keys its table plans by the identity of the array, and
/// the Go and PHP ones rebuild a prepared tree because their arrays have no
/// identity to key by. Rust needs neither: both passes walk the same tree in the
/// same order, so numbering the arrays as they are met is enough to hand each
/// plan back to the pass that needs it, and the document is never copied.
struct Analysis {
    options: Options,

    key_order: Vec<String>,            // keys in the order they were first met
    key_count: HashMap<String, usize>, // and how often each is written

    string_order: Vec<String>,
    string_count: HashMap<String, usize>,
    forced: HashSet<String>, // values a dictionary column has to catalogue

    /// One entry per array in the document, in the order the arrays are met.
    plans: Vec<Option<TablePlan>>,
}

impl Analysis {
    fn new(options: Options) -> Analysis {
        Analysis {
            options,
            key_order: Vec::new(),
            key_count: HashMap::new(),
            string_order: Vec::new(),
            string_count: HashMap::new(),
            forced: HashSet::new(),
            plans: Vec::new(),
        }
    }

    fn count_key(&mut self, key: &str) {
        if let Some(count) = self.key_count.get_mut(key) {
            *count += 1;
            return;
        }
        self.key_order.push(key.to_owned());
        self.key_count.insert(key.to_owned(), 1);
    }

    fn count_string(&mut self, value: &str) {
        if !self.options.strings {
            return;
        }
        if let Some(count) = self.string_count.get_mut(value) {
            *count += 1;
            return;
        }
        self.string_order.push(value.to_owned());
        self.string_count.insert(value.to_owned(), 1);
    }

    fn value(&mut self, value: &Value, depth: usize) -> Result<()> {
        if depth > MAX_COMPRESS_DEPTH {
            return Err(Error::TooDeep(MAX_COMPRESS_DEPTH));
        }

        match value {
            Value::String(text) => self.count_string(text),

            Value::Array(items) => {
                // The slot is reserved before anything nested is walked, so the
                // numbering is pre-order and the build pass, which walks the
                // same tree the same way, meets the slots in the same sequence.
                let slot = self.plans.len();
                self.plans.push(None);

                let options = self.options;
                match plan_table(items, &options) {
                    Some(plan) => {
                        self.table(items, &plan, depth)?;
                        self.plans[slot] = Some(plan);
                    }
                    None => {
                        for item in items {
                            self.value(item, depth + 1)?;
                        }
                    }
                }
            }

            Value::Object(object) => {
                for (key, item) in object.iter() {
                    self.count_key(key);
                    self.value(item, depth + 1)?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    fn table(&mut self, items: &[Value], plan: &TablePlan, depth: usize) -> Result<()> {
        // A table spells each key out once, in the column list, however many rows
        // it has - so it contributes one to the key's frequency, not one per row.
        for column in &plan.columns {
            self.count_key(column);
        }
        // Constant values are scalars by construction, so this only counts
        // strings.
        for constant in &plan.constants {
            self.value(constant, depth + 1)?;
        }

        for item in items {
            let row = match item {
                Value::Object(row) => row,
                _ => continue,
            };

            for (at, key) in plan.columns.iter().enumerate() {
                if plan.encodings[at] == COL_CONST {
                    continue;
                }
                let value = match row.get(key) {
                    Some(value) => folded(value),
                    None => continue,
                };
                if plan.encodings[at] == COL_DICT {
                    if let Value::String(text) = value {
                        self.count_string(text);
                        self.forced.insert(text.clone());
                    }
                    continue;
                }
                self.value(value, depth + 1)?;
            }
        }

        Ok(())
    }

    /// Orders the keys by how often they are written, most-used first, so the
    /// busiest keys get the shortest tokens. Ties keep the order the document
    /// first mentioned them in, which makes the result reproducible.
    fn key_catalogue(&self) -> KeyCatalogue {
        let mut order = self.key_order.clone();
        order.sort_by(|left, right| self.key_count[right].cmp(&self.key_count[left]));

        let mut out = KeyCatalogue {
            list: Vec::with_capacity(order.len()),
            index: HashMap::with_capacity(order.len()),
            tokens: Vec::with_capacity(order.len()),
        };
        for (at, key) in order.into_iter().enumerate() {
            out.index.insert(key.clone(), at);
            out.tokens.push(to_key_token(at));
            out.list.push(key);
        }
        out
    }

    /// Chooses which string values are worth moving into the catalogue.
    ///
    /// A catalogued string is written once in the catalogue and referenced by a
    /// short `~token` everywhere it occurs, so the trade is:
    ///
    /// ```text
    /// inline    = count * (length + 2)                  // quotes on every copy
    /// catalogue = count * (tokenLength + 3) + length + 3
    /// ```
    ///
    /// Candidates are considered in frequency order, so the token length used in
    /// that comparison is the one the string would really be given.
    ///
    /// Values pulled into a dictionary column are forced in regardless: their
    /// cells are bare integers, which is cheaper still, and the comparison above
    /// does not describe them.
    fn string_catalogue(&self) -> StringCatalogue {
        let mut out = StringCatalogue {
            list: Vec::new(),
            index: HashMap::new(),
        };
        if !self.options.strings {
            return out;
        }

        let mut candidates: Vec<Candidate> = Vec::with_capacity(self.string_order.len());
        for value in &self.string_order {
            let forced = self.forced.contains(value);
            let length = utf16_length(value);
            let count = self.string_count[value];
            if !forced {
                if count < self.options.min_string_count {
                    continue;
                }
                if length < self.options.min_string_length {
                    continue;
                }
                // A value that already begins with a tilde is escaped inline and
                // can never be written as a reference, so cataloguing it would
                // only burn an index.
                if value.starts_with(TILDE) {
                    continue;
                }
            }
            candidates.push(Candidate {
                value,
                count,
                length,
                forced,
            });
        }

        candidates.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then(right.length.cmp(&left.length))
        });

        for candidate in candidates {
            if !candidate.forced {
                let token_length = base62_length(out.list.len());
                let inline = candidate.count * (candidate.length + 2);
                let catalogued = candidate.count * (token_length + 3) + candidate.length + 3;
                if catalogued >= inline {
                    continue;
                }
            }
            out.index.insert(candidate.value.clone(), out.list.len());
            out.list.push(candidate.value.clone());
        }

        out
    }
}

struct Candidate<'a> {
    value: &'a String,
    count: usize,
    length: usize,
    forced: bool,
}

struct KeyCatalogue {
    /// The names, as they go into the envelope.
    list: Vec<String>,
    /// Name to catalogue index.
    index: HashMap<String, usize>,
    /// Catalogue index to token.
    tokens: Vec<String>,
}

struct StringCatalogue {
    list: Vec<String>,
    index: HashMap<String, usize>,
}

/* ------------------------------------------------------------------ *
 * Pass two - build
 * ------------------------------------------------------------------ */

struct Encoder {
    keys: KeyCatalogue,
    strings: StringCatalogue,
    plans: Vec<Option<TablePlan>>,
    next: usize,
}

impl Encoder {
    fn text(&self, value: &str) -> Value {
        // Escape first: an escaped literal always starts with two tildes, a
        // reference is a tilde followed by a base-62 digit, and a lone tilde is
        // the row hole. Those three spaces cannot overlap.
        if value.starts_with(TILDE) {
            let mut escaped = String::with_capacity(value.len() + 1);
            escaped.push(TILDE);
            escaped.push_str(value);
            return Value::String(escaped);
        }
        match self.strings.index.get(value) {
            Some(at) => Value::String(format!("~{}", to_base62(*at))),
            None => Value::String(value.to_owned()),
        }
    }

    fn value(&mut self, value: &Value) -> Value {
        match value {
            Value::String(text) => self.text(text),

            Value::Float(number) if !number.is_finite() => Value::Null,

            Value::Array(items) => {
                // The slot numbering only lines up because both passes walk the
                // same tree in the same order. If that ever stopped being true
                // the honest failure is to write the array out element by
                // element - larger on the wire, but still exactly this document.
                let slot = self.next;
                self.next += 1;
                debug_assert!(
                    slot < self.plans.len(),
                    "the two passes disagree about the arrays"
                );
                match self.plans.get_mut(slot).and_then(Option::take) {
                    Some(plan) => self.table(items, &plan),
                    None => Value::Array(items.iter().map(|item| self.value(item)).collect()),
                }
            }

            Value::Object(object) => {
                let mut out = Object::with_capacity(object.len());
                for (key, item) in object.iter() {
                    let token = self.keys.tokens[self.keys.index[key]].clone();
                    let encoded = self.value(item);
                    out.insert(token, encoded);
                }
                Value::Object(out)
            }

            other => other.clone(),
        }
    }

    fn table(&mut self, items: &[Value], plan: &TablePlan) -> Value {
        let mut node = Object::with_capacity(4);

        let mut columns = Vec::with_capacity(plan.columns.len());
        let mut encodings = Vec::with_capacity(plan.encodings.len());
        let mut uniform = true;
        for (at, key) in plan.columns.iter().enumerate() {
            columns.push(Value::Int(self.keys.index[key] as i64));
            encodings.push(Value::Int(plan.encodings[at]));
            if plan.encodings[at] != COL_RAW {
                uniform = false;
            }
        }
        node.insert(TABLE_COLUMNS, Value::Array(columns));
        if !uniform {
            node.insert(TABLE_ENCODINGS, Value::Array(encodings));
        }

        if !plan.constants.is_empty() {
            let constants = plan
                .constants
                .iter()
                .map(|constant| self.value(constant))
                .collect();
            node.insert(TABLE_CONSTANTS, Value::Array(constants));
        }

        if plan.variable_count == 0 {
            // Every column turned out to be constant, so the rows carry no data
            // at all and only their number survives.
            node.insert(TABLE_ROW_COUNT, Value::Int(items.len() as i64));
            return Value::Object(node);
        }

        let mut matrix = Vec::with_capacity(items.len());
        let mut cells: Vec<Value> = vec![Value::Null; plan.variable_count];
        // Whether a cell is a hole is recorded rather than inferred from what it
        // holds: a raw column may legitimately contain the number -1, which is
        // exactly what a dictionary column writes for an absent key.
        let mut holes = vec![false; plan.variable_count];

        for item in items {
            let row = match item {
                Value::Object(row) => row,
                _ => continue,
            };

            let mut at = 0;
            for (column, key) in plan.columns.iter().enumerate() {
                let encoding = plan.encodings[column];
                if encoding == COL_CONST {
                    continue;
                }

                let value = row.get(key).map(folded);
                cells[at] = match (encoding, value) {
                    (COL_DICT, None) => Value::Int(DICT_HOLE),
                    (COL_DICT, Some(Value::Null)) => Value::Null,
                    (COL_DICT, Some(Value::String(text))) => {
                        // Every value a dictionary column carries was forced
                        // into the catalogue during the analysis pass, so a miss
                        // here is a broken invariant rather than a document this
                        // encoder cannot write - and writing some other index
                        // would silently hand back the wrong string.
                        let at = self.strings.index.get(text.as_str()).copied().expect(
                            "a dictionary column value that was not forced into the catalogue",
                        );
                        Value::Int(at as i64)
                    }
                    (COL_DICT, Some(_)) => Value::Null,
                    (_, None) => Value::String(HOLE.to_owned()),
                    (_, Some(present)) => self.value(present),
                };
                holes[at] = value.is_none();
                at += 1;
            }

            // Trailing gaps need not be written down; a short row is understood
            // to be missing everything past its end.
            let mut end = cells.len();
            while end > 0 && holes[end - 1] {
                end -= 1;
            }
            matrix.push(Value::Array(cells[..end].to_vec()));
        }

        node.insert(TABLE_ROWS, Value::Array(matrix));
        Value::Object(node)
    }
}
