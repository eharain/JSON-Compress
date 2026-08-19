//! Writing JSON text, character for character as `JSON.stringify` writes it.
//! The reader lives next door, in [`crate::parse`].

use crate::value::{Object, Value};

/// The widest indent `JSON.stringify` honours, and so the widest this honours.
const MAX_INDENT: usize = 10;

/// Serialises a value as JSON text, character for character as
/// `JSON.stringify` would write it.
///
/// That is not quite what most Rust JSON writers produce. Rust's own `f64`
/// formatting never uses exponent form, so `1e21` comes out as twenty-two
/// digits there and as `1e+21` here; the wire format is defined against the
/// ECMAScript spelling, and every implementation of it agrees byte for byte,
/// which is what this preserves.
///
/// Writing cannot fail: every [`Value`] is a JSON value, and the two shapes JSON
/// cannot spell - a non-finite float and negative zero - are written the way
/// JSON writers everywhere write them, as `null` and `0`.
pub fn format_json(value: &Value) -> String {
    let mut out = String::with_capacity(256);
    write_value(&mut out, value, "", 0);
    out
}

/// [`format_json`] with each level indented by `indent` spaces, capped at ten as
/// `JSON.stringify` caps it. An indent of zero writes the compact form.
pub fn format_json_indent(value: &Value, indent: usize) -> String {
    let mut out = String::with_capacity(256);
    write_value(&mut out, value, &indent_pad(indent), 0);
    out
}

pub(crate) fn indent_pad(indent: usize) -> String {
    " ".repeat(indent.min(MAX_INDENT))
}

pub(crate) fn write_value(out: &mut String, value: &Value, pad: &str, level: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(number) => out.push_str(itoa(*number).as_str()),
        Value::Uint(number) => out.push_str(utoa(*number).as_str()),
        Value::Float(number) => out.push_str(&format_float(*number)),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => write_array(out, items, pad, level),
        Value::Object(object) => write_object(out, object, pad, level),
    }
}

fn itoa(number: i64) -> String {
    number.to_string()
}

fn utoa(number: u64) -> String {
    number.to_string()
}

fn write_array(out: &mut String, items: &[Value], pad: &str, level: usize) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }

    out.push('[');
    for (at, item) in items.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        if !pad.is_empty() {
            push_newline(out, pad, level + 1);
        }
        write_value(out, item, pad, level + 1);
    }
    if !pad.is_empty() {
        push_newline(out, pad, level);
    }
    out.push(']');
}

pub(crate) fn write_object(out: &mut String, object: &Object, pad: &str, level: usize) {
    if object.is_empty() {
        out.push_str("{}");
        return;
    }

    out.push('{');
    for (at, (key, value)) in object.iter().enumerate() {
        if at > 0 {
            out.push(',');
        }
        if !pad.is_empty() {
            push_newline(out, pad, level + 1);
        }
        write_string(out, key);
        out.push(':');
        if !pad.is_empty() {
            out.push(' ');
        }
        write_value(out, value, pad, level + 1);
    }
    if !pad.is_empty() {
        push_newline(out, pad, level);
    }
    out.push('}');
}

fn push_newline(out: &mut String, pad: &str, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str(pad);
    }
}

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Quotes a string exactly as `JSON.stringify` quotes it: the two mandatory
/// escapes, the five short control-character escapes, `\u00xx` for the rest of
/// the control range, and everything else as it stands.
///
/// Notably the solidus, the line separators U+2028 and U+2029, and every
/// non-ASCII character are written raw, where several standard-library JSON
/// writers escape some of them. A Rust `String` is always valid UTF-8, so the
/// unpaired surrogates the other implementations have to think about cannot
/// reach here at all.
pub(crate) fn write_string(out: &mut String, text: &str) {
    out.push('"');

    let bytes = text.as_bytes();
    let mut start = 0;
    for at in 0..bytes.len() {
        let byte = bytes[at];
        if byte >= 0x20 && byte != b'"' && byte != b'\\' {
            continue;
        }

        out.push_str(&text[start..at]);
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x08 => out.push_str("\\b"),
            0x0C => out.push_str("\\f"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ => {
                out.push_str("\\u00");
                out.push(HEX_DIGITS[(byte >> 4) as usize] as char);
                out.push(HEX_DIGITS[(byte & 0x0F) as usize] as char);
            }
        }
        start = at + 1;
    }

    out.push_str(&text[start..]);
    out.push('"');
}

/// Renders a number the way ECMAScript's `Number::toString` would.
///
/// Rust and JavaScript agree on the digits - both print the shortest decimal
/// that reads back as the same double - and disagree on where the decimal point
/// goes and when to switch to an exponent. Rust never switches; ECMAScript
/// switches outside the range 1e-7 to 1e21. This applies the ECMAScript
/// placement rules to Rust's digits.
pub fn format_float(value: f64) -> String {
    // Non-finite values have no JSON spelling; JSON writers substitute null.
    if !value.is_finite() {
        return "null".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned(); // including negative zero, which JSON cannot tell apart
    }

    let sign = if value < 0.0 { "-" } else { "" };
    let magnitude = value.abs();

    // Shortest round-trip digits, in the form d.ddde±dd, from which the digit
    // string and the decimal exponent fall out directly.
    let text = format!("{magnitude:e}");
    let at = match text.find('e') {
        Some(at) => at,
        None => return format!("{sign}{text}"),
    };
    let exponent: i32 = match text[at + 1..].parse() {
        Ok(exponent) => exponent,
        Err(_) => return format!("{sign}{text}"),
    };
    let digits: String = text[..at].chars().filter(|c| *c != '.').collect();

    let k = digits.len() as i32;
    let n = exponent + 1;

    if k <= n && n <= 21 {
        return format!("{sign}{digits}{}", "0".repeat((n - k) as usize));
    }
    if 0 < n && n <= 21 {
        let split = n as usize;
        return format!("{sign}{}.{}", &digits[..split], &digits[split..]);
    }
    if -6 < n && n <= 0 {
        return format!("{sign}0.{}{digits}", "0".repeat((-n) as usize));
    }

    let tail = if n - 1 < 0 {
        format!("e-{}", 1 - n)
    } else {
        format!("e+{}", n - 1)
    };
    if k == 1 {
        return format!("{sign}{digits}{tail}");
    }
    format!("{sign}{}.{}{tail}", &digits[..1], &digits[1..])
}

/// The length of a string in UTF-16 code units.
///
/// That is what `String.length` counts in JavaScript, and the encoder's cost
/// model is calibrated against it - so an emoji weighs two, not one, and the
/// same string is catalogued here and there or in neither.
pub(crate) fn utf16_length(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_follow_the_ecmascript_placement_rule() {
        assert_eq!(format_float(0.0), "0");
        assert_eq!(format_float(-0.0), "0");
        assert_eq!(format_float(1.0), "1");
        assert_eq!(format_float(-1.5), "-1.5");
        assert_eq!(format_float(100.0), "100");
        assert_eq!(format_float(0.1), "0.1");
        assert_eq!(format_float(1e-6), "0.000001");
        assert_eq!(format_float(1e-7), "1e-7");
        assert_eq!(format_float(1.5e-7), "1.5e-7");
        assert_eq!(format_float(1e20), "100000000000000000000");
        assert_eq!(format_float(1e21), "1e+21");
        assert_eq!(format_float(1.5e21), "1.5e+21");
        assert_eq!(format_float(f64::NAN), "null");
        assert_eq!(format_float(f64::INFINITY), "null");
        assert_eq!(format_float(0.1 + 0.2), "0.30000000000000004");
    }

    #[test]
    fn strings_are_escaped_the_way_json_stringify_escapes_them() {
        let mut out = String::new();
        write_string(&mut out, "a\"b\\c\nd\te/f");
        assert_eq!(out, r#""a\"b\\c\nd\te/f""#);

        let mut out = String::new();
        write_string(&mut out, "\u{1}\u{2028}\u{2029}\u{1f600}");
        assert_eq!(out, "\"\\u0001\u{2028}\u{2029}\u{1f600}\"");
    }

    #[test]
    fn utf16_length_counts_code_units() {
        assert_eq!(utf16_length(""), 0);
        assert_eq!(utf16_length("abc"), 3);
        assert_eq!(utf16_length("é"), 1);
        assert_eq!(utf16_length("😀"), 2);
    }
}
