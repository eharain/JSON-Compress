//! The shared document generator.
//!
//! Kept step for step with the one in `javascript/tests/fuzz.test.js`, so a seed
//! means the same document in every language that reproduces it. That is what
//! lets the conformance corpus be described by its seeds rather than only by its
//! contents, and what makes a failure reproducible across ports.

use json_compress::{Object, Value};

pub const WORDS: [&str; 33] = [
    "id",
    "name",
    "status",
    "created",
    "updated",
    "owner",
    "tags",
    "value",
    "count",
    "total",
    "items",
    "meta",
    "type",
    "code",
    "label",
    "notes",
    "~tilde",
    "0",
    "1",
    "10",
    "a",
    "z",
    "A",
    "~t",
    "~r",
    "toString",
    "__proto__",
    "constructor",
    "hasOwnProperty",
    "",
    " ",
    "ключ",
    "键",
];

pub const STRINGS: [&str; 16] = [
    "",
    "a",
    "active",
    "pending",
    "cancelled",
    "~",
    "~~",
    "~0",
    "~zz",
    "a longer value that repeats across many records",
    "quote \" backslash \\ newline \n tab \t",
    "😀 emoji",
    "مرحبا",
    "null",
    "true",
    "0",
];

/// A small deterministic generator, so failures can be replayed.
pub struct Random {
    state: u32,
}

impl Random {
    pub fn new(seed: u32) -> Random {
        Random { state: seed }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state as f64 / 4294967296.0
    }

    fn pick(&mut self, values: &[&str]) -> String {
        let at = (self.next() * values.len() as f64) as usize;
        values[at].to_owned()
    }
}

pub fn make_scalar(random: &mut Random) -> Value {
    let roll = random.next();
    if roll < 0.2 {
        return Value::Null;
    }
    if roll < 0.3 {
        return Value::Bool(random.next() < 0.5);
    }
    if roll < 0.55 {
        return Value::Float(((random.next() * 2000.0) as i64 - 1000) as f64);
    }
    if roll < 0.62 {
        return Value::Float(random.next() * 1e6 - 5e5);
    }
    Value::String(random.pick(&STRINGS))
}

pub fn make_value(random: &mut Random, depth: i32) -> Value {
    if depth <= 0 {
        return make_scalar(random);
    }

    let roll = random.next();

    if roll < 0.45 {
        return make_scalar(random);
    }

    if roll < 0.6 {
        let length = (random.next() * 6.0) as usize;
        return Value::Array(
            (0..length)
                .map(|_| make_value(random, depth - 1))
                .collect::<Vec<_>>(),
        );
    }

    if roll < 0.8 {
        // A record set: the shape the format is built for, with ragged rows and
        // low-cardinality columns thrown in on purpose.
        let rows = (random.next() * 8.0) as usize;
        let column_count = 1 + (random.next() * 5.0) as usize;
        let columns: Vec<String> = (0..column_count).map(|_| random.pick(&WORDS)).collect();

        let mut out = Vec::with_capacity(rows);
        for _ in 0..rows {
            let mut row = Object::new();
            for column in &columns {
                if random.next() < 0.15 {
                    continue;
                }
                if random.next() < 0.5 {
                    let value = random.pick(&STRINGS);
                    row.insert(column.clone(), Value::String(value));
                } else {
                    let value = make_value(random, depth - 2);
                    row.insert(column.clone(), value);
                }
            }
            out.push(Value::Object(row));
        }
        return Value::Array(out);
    }

    let size = (random.next() * 6.0) as usize;
    let mut out = Object::new();
    for _ in 0..size {
        let key = random.pick(&WORDS);
        let value = make_value(random, depth - 1);
        out.insert(key, value);
    }
    Value::Object(out)
}
