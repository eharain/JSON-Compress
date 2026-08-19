//! The reader and the writer, on their own.
//!
//! The reader is written against the JSON grammar directly rather than over
//! another crate's, so most of this holds it to `serde_json`'s answers: the same
//! documents accepted, the same refused, the same values read out of them. The
//! two places the two deliberately part company are called out at the bottom,
//! with the reason.

mod common;

use json_compress::{beautify, format_json, format_json_indent, minify, parse, validate, Value};

use common::{make_value, Random};

/// Whether two readers read the same document.
///
/// Key order is ignored, because `serde_json` builds a sorted map unless its
/// `preserve_order` feature is on while this crate always keeps the document's
/// own order, and order is not what this comparison is about.
///
/// Numbers are allowed to differ by one unit in the last place. Both readers aim
/// at the nearest double and `serde_json` occasionally lands one bit away from
/// it - `376276.80669538677` is one such document, and the standard library
/// agrees with this crate rather than with `serde_json` on which double that
/// text names. A real reader fault - the wrong value, the wrong escape, the
/// wrong verdict - is nowhere near a single bit and still fails here.
fn agree(ours: &Value, theirs: &Value) -> bool {
    match (ours, theirs) {
        (Value::Array(ours), Value::Array(theirs)) => {
            ours.len() == theirs.len() && ours.iter().zip(theirs.iter()).all(|(a, b)| agree(a, b))
        }
        (Value::Object(ours), Value::Object(theirs)) => {
            ours.len() == theirs.len()
                && ours
                    .iter()
                    .all(|(key, value)| theirs.get(key).is_some_and(|other| agree(value, other)))
        }
        (Value::Float(ours), Value::Float(theirs)) => within_one_ulp(*ours, *theirs),
        (ours, theirs) => ours == theirs,
    }
}

fn within_one_ulp(left: f64, right: f64) -> bool {
    if left == right {
        return true;
    }
    if !left.is_finite()
        || !right.is_finite()
        || left.is_sign_negative() != right.is_sign_negative()
    {
        return false;
    }
    (left.to_bits() as i64 - right.to_bits() as i64).abs() <= 1
}

/// The same document, read by `serde_json`, in this crate's model. Numbers all
/// become doubles, which is what this format carries and what every other
/// implementation of it reads.
fn from_serde(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(found) => Value::Bool(*found),
        serde_json::Value::Number(found) => Value::Float(found.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::String(found) => Value::String(found.clone()),
        serde_json::Value::Array(items) => Value::Array(items.iter().map(from_serde).collect()),
        serde_json::Value::Object(members) => {
            let mut out = json_compress::Object::with_capacity(members.len());
            for (key, value) in members {
                out.insert(key.clone(), from_serde(value));
            }
            Value::Object(out)
        }
    }
}

#[track_caller]
fn agrees_with_serde_json(text: &str) {
    let ours = parse(text);
    let theirs = serde_json::from_str::<serde_json::Value>(text);

    match (ours, theirs) {
        (Ok(ours), Ok(theirs)) => {
            let theirs = from_serde(&theirs);
            assert!(
                agree(&ours, &theirs),
                "the two readers disagree about {text}\n  ours   {ours}\n  theirs {theirs}"
            );
        }
        (Err(_), Err(_)) => {}
        (Ok(ours), Err(error)) => {
            panic!("we read {text} as {ours}, serde_json refused it: {error}")
        }
        (Err(error), Ok(theirs)) => {
            panic!("serde_json read {text} as {theirs}, we refused it: {error}")
        }
    }
}

#[test]
fn the_reader_agrees_with_serde_json_on_the_awkward_cases() {
    for text in [
        "null",
        "true",
        "false",
        "0",
        "-0",
        "1",
        "-1",
        "1.5",
        "1e5",
        "1E5",
        "1e+5",
        "1e-5",
        "-1.5e-5",
        "0.0000001",
        "1e21",
        "\"\"",
        "\"a\"",
        "\"\\u0041\"",
        "\"\\ud83d\\ude00\"",
        "\"\\\"\\\\\\/\\b\\f\\n\\r\\t\"",
        "\"\u{2028}\u{2029}\"",
        "\"\u{1f600}\"",
        "[]",
        "{}",
        "[1,2,3]",
        "{\"a\":1}",
        "{\"a\":{\"b\":[1,{\"c\":null}]}}",
        "  [ 1 , 2 ]  ",
        "[1,2,3\n]",
        // and the ones both must refuse
        "",
        " ",
        "[",
        "]",
        "{",
        "}",
        "[1,]",
        "{\"a\":1,}",
        "{'a':1}",
        "{a:1}",
        "01",
        "-",
        "+1",
        "1.",
        ".1",
        "1e",
        "1e+",
        "\"a",
        "\"a\tb\"",
        "\"\\x\"",
        "\"\\u12\"",
        "tru",
        "NaN",
        "Infinity",
        "undefined",
        "[1] [2]",
        "{} {}",
    ] {
        agrees_with_serde_json(text);
    }
}

#[test]
fn the_reader_agrees_with_serde_json_on_generated_documents() {
    for seed in 0..500u32 {
        let value = make_value(&mut Random::new(seed), 5);
        agrees_with_serde_json(&format_json(&value));
        agrees_with_serde_json(&format_json_indent(&value, 2));
    }
}

/// The two deliberate divergences, both in this crate's favour: the format is
/// defined against what JavaScript does, and JavaScript does these.
#[test]
fn the_two_places_the_reader_parts_company_with_serde_json() {
    // A magnitude no double can hold is infinity, as it is to `JSON.parse` -
    // and infinity has no JSON spelling, so writing it back gives null. serde_json
    // refuses the document instead.
    let huge = parse("1e999").unwrap();
    assert_eq!(huge, Value::Float(f64::INFINITY));
    assert_eq!(format_json(&huge), "null");
    assert!(serde_json::from_str::<serde_json::Value>("1e999").is_err());

    // An unpaired surrogate escape becomes U+FFFD, which is the only thing a
    // Rust string can hold and what every implementation of this format does
    // with one. serde_json refuses the document.
    assert_eq!(
        parse("\"\\ud800\"").unwrap(),
        Value::String("\u{fffd}".to_owned())
    );
    assert!(serde_json::from_str::<serde_json::Value>("\"\\ud800\"").is_err());
}

#[test]
fn the_writer_writes_what_json_stringify_writes() {
    let value = parse(r#"{"a":[1,2],"b":{"c":"x"},"d":null}"#).unwrap();
    assert_eq!(format_json(&value), r#"{"a":[1,2],"b":{"c":"x"},"d":null}"#);
    assert_eq!(
        format_json_indent(&value, 2),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {\n    \"c\": \"x\"\n  },\n  \"d\": null\n}"
    );
    assert_eq!(format_json(&Value::array()), "[]");
    assert_eq!(format_json(&Value::object()), "{}");
    assert_eq!(format_json_indent(&Value::array(), 2), "[]");

    // The solidus and the line separators are written raw, as JSON.stringify
    // writes them and as several standard-library writers do not.
    assert_eq!(
        format_json(&Value::from("a/b\u{2028}c<&>")),
        "\"a/b\u{2028}c<&>\""
    );
}

#[test]
fn an_indent_is_capped_where_json_stringify_caps_it() {
    let value = parse("[1]").unwrap();
    assert_eq!(format_json_indent(&value, 0), "[1]");
    assert_eq!(
        format_json_indent(&value, 20),
        format_json_indent(&value, 10)
    );
}

#[test]
fn minify_and_beautify_change_nothing_but_whitespace() {
    let text = "{\n  \"a\" : [ 1, 2 ],\n  \"b\": null\n}";
    assert_eq!(minify(text).unwrap(), r#"{"a":[1,2],"b":null}"#);
    assert_eq!(
        beautify(&minify(text).unwrap(), 2).unwrap(),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": null\n}"
    );
    assert!(minify("{oops}").is_err());
}

#[test]
fn validate_reports_the_same_verdict_the_reader_gives() {
    for seed in 0..200u32 {
        let text = format_json(&make_value(&mut Random::new(seed), 4));
        assert!(validate(&text).valid, "{text} should be valid");
    }
    for text in ["{", "[1,]", "{'a':1}", "nope"] {
        assert!(!validate(text).valid, "{text} should not be valid");
        assert!(!validate(text).error.is_empty());
    }
}
