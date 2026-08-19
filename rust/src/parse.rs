//! Reading JSON text.
//!
//! A direct scan of the document, against the grammar in RFC 8259, into the
//! ordered value model. It is `JSON.parse` with objects that remember their key
//! order: numbers become [`Value::Float`], as they do in every other
//! implementation of this format, and a repeated key keeps the position it
//! first had.

use crate::error::{Error, Result};
use crate::value::{Object, Value};

/// The nesting a document may reach before it is refused.
///
/// The reader walks the document with the call stack, so the limit exists to
/// make a hostile document exhaust a counter rather than the stack. It is set
/// against the worst case rather than the usual one: an unoptimised build has
/// frames several times the size of a release build's, and a test binary is the
/// place a deeply nested document is most likely to be thrown at this. Four
/// times this depth is what an unoptimised build survives, and this is a quarter
/// of it - still four times what `serde_json` allows by default.
///
/// The two limits are load-bearing together. An envelope nests about half again
/// as deep as the document it was written from - a chain of two-row tables turns
/// each source level into a `~t` object, a `~r` array and a row array - so
/// [`MAX_COMPRESS_DEPTH`] is set low enough that anything this crate compresses,
/// it can also read back.
pub const MAX_PARSE_DEPTH: usize = 512;

/// The nesting a document may reach and still be compressed, which is lower than
/// [`MAX_PARSE_DEPTH`] by the margin an envelope adds.
pub const MAX_COMPRESS_DEPTH: usize = 256;

/// Reads JSON text into the value model.
///
/// ```
/// use json_compress::{parse, Value};
///
/// let value = parse(r#"{"b": 1, "a": 2}"#).unwrap();
///
/// // The order the members arrived in, not the order a hash map would give.
/// assert_eq!(value.as_object().unwrap().keys().collect::<Vec<_>>(), ["b", "a"]);
/// assert_eq!(value.get("a"), Some(&Value::Float(2.0)));
/// ```
pub fn parse(text: &str) -> Result<Value> {
    let mut reader = Parser {
        text,
        data: text.as_bytes(),
        at: 0,
    };

    let value = reader.value(0)?;

    // A JSON document is one value. Anything after it is a second document, and
    // ignoring it silently would hide a truncated write.
    reader.skip_space();
    if reader.at < reader.data.len() {
        return Err(reader.fail_at(reader.at, "unexpected data after the end of the document"));
    }
    Ok(value)
}

/// [`parse`] for bytes that are not known to be UTF-8. A byte that is not part
/// of any character becomes U+FFFD, which is what the other implementations of
/// this format do with one.
pub fn parse_slice(data: &[u8]) -> Result<Value> {
    parse(&String::from_utf8_lossy(data))
}

struct Parser<'a> {
    text: &'a str,
    data: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn fail(&self, message: impl Into<String>) -> Error {
        self.fail_at(self.at, message)
    }

    fn fail_at(&self, at: usize, message: impl Into<String>) -> Error {
        if at >= self.data.len() {
            return Error::truncated();
        }
        Error::parse(message, at)
    }

    fn skip_space(&mut self) {
        while self.at < self.data.len() {
            match self.data[self.at] {
                b' ' | b'\t' | b'\n' | b'\r' => self.at += 1,
                _ => return,
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_PARSE_DEPTH {
            return Err(self.fail("the document is nested too deeply to read"));
        }

        self.skip_space();
        if self.at >= self.data.len() {
            return Err(self.fail("expected a value"));
        }

        match self.data[self.at] {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(Value::String(self.string()?)),
            b't' => self.literal("true", Value::Bool(true)),
            b'f' => self.literal("false", Value::Bool(false)),
            b'n' => self.literal("null", Value::Null),
            b'-' | b'0'..=b'9' => self.number(),
            byte => Err(self.fail(format!(
                "unexpected character {:?}",
                char::from_u32(byte as u32).unwrap_or('\u{fffd}')
            ))),
        }
    }

    fn literal(&mut self, want: &str, value: Value) -> Result<Value> {
        let end = self.at + want.len();
        if end > self.data.len() || &self.data[self.at..end] != want.as_bytes() {
            return Err(self.fail(format!("expected {want}")));
        }
        self.at = end;
        Ok(value)
    }

    fn object(&mut self, depth: usize) -> Result<Value> {
        self.at += 1; // the opening brace
        let mut out = Object::new();

        self.skip_space();
        if self.at < self.data.len() && self.data[self.at] == b'}' {
            self.at += 1;
            return Ok(Value::Object(out));
        }

        loop {
            self.skip_space();
            if self.at >= self.data.len() || self.data[self.at] != b'"' {
                return Err(self.fail("expected a quoted property name"));
            }
            let key = self.string()?;

            self.skip_space();
            if self.at >= self.data.len() || self.data[self.at] != b':' {
                return Err(self.fail("expected a colon after the property name"));
            }
            self.at += 1;

            let value = self.value(depth + 1)?;
            out.insert(key, value);

            self.skip_space();
            if self.at >= self.data.len() {
                return Err(self.fail("this object is never closed"));
            }
            match self.data[self.at] {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Ok(Value::Object(out));
                }
                _ => return Err(self.fail("expected a comma or a closing brace")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value> {
        self.at += 1; // the opening bracket
        let mut out = Vec::new();

        self.skip_space();
        if self.at < self.data.len() && self.data[self.at] == b']' {
            self.at += 1;
            return Ok(Value::Array(out));
        }

        loop {
            out.push(self.value(depth + 1)?);

            self.skip_space();
            if self.at >= self.data.len() {
                return Err(self.fail("this array is never closed"));
            }
            match self.data[self.at] {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Ok(Value::Array(out));
                }
                _ => return Err(self.fail("expected a comma or a closing bracket")),
            }
        }
    }

    /// Reads a string. The common case - no escapes - copies the slice between
    /// the quotes and nothing else. A multi-byte character cannot be mistaken
    /// for one of the bytes this looks at, because every continuation byte is
    /// 0x80 or above.
    fn string(&mut self) -> Result<String> {
        let start = self.at;
        self.at += 1; // the opening quote

        let mut at = self.at;
        while at < self.data.len() {
            match self.data[at] {
                b'"' => {
                    let plain = &self.text[self.at..at];
                    self.at = at + 1;
                    return Ok(plain.to_owned());
                }
                b'\\' => return self.escaped_string(start),
                byte if byte < 0x20 => {
                    return Err(self.fail_at(at, "a raw control character inside a string"))
                }
                _ => at += 1,
            }
        }

        Err(self.fail_at(start, "this string is never closed"))
    }

    /// Finishes a string that turned out to hold an escape sequence.
    fn escaped_string(&mut self, start: usize) -> Result<String> {
        // Unescaping only ever shortens, so the distance to the closing quote is
        // an upper bound on the result and the buffer is sized once. Sizing it
        // from what is left of the document instead would allocate a megabyte
        // per string in a document full of escapes.
        let mut out = String::with_capacity(self.string_extent());

        while self.at < self.data.len() {
            match self.data[self.at] {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    if self.at >= self.data.len() {
                        return Err(self.fail_at(start, "this string is never closed"));
                    }
                    match self.data[self.at] {
                        escape @ (b'"' | b'\\' | b'/') => {
                            out.push(escape as char);
                            self.at += 1;
                        }
                        b'b' => {
                            out.push('\u{8}');
                            self.at += 1;
                        }
                        b'f' => {
                            out.push('\u{c}');
                            self.at += 1;
                        }
                        b'n' => {
                            out.push('\n');
                            self.at += 1;
                        }
                        b'r' => {
                            out.push('\r');
                            self.at += 1;
                        }
                        b't' => {
                            out.push('\t');
                            self.at += 1;
                        }
                        b'u' => out.push(self.escaped_char()?),
                        escape => {
                            return Err(self.fail_at(
                                self.at - 1,
                                format!(
                                    "\"\\{}\" is not a valid escape sequence",
                                    char::from_u32(escape as u32).unwrap_or('\u{fffd}')
                                ),
                            ))
                        }
                    }
                }
                byte if byte < 0x20 => {
                    return Err(self.fail("a raw control character inside a string"))
                }
                byte if byte < 0x80 => {
                    out.push(byte as char);
                    self.at += 1;
                }
                _ => {
                    // The input is a &str, so this is a whole character and its
                    // width is what the leading byte says it is.
                    let width = char_width(self.data[self.at]);
                    out.push_str(&self.text[self.at..self.at + width]);
                    self.at += width;
                }
            }
        }

        Err(self.fail_at(start, "this string is never closed"))
    }

    /// How many bytes are left before this string's closing quote, or what is
    /// left of the document if it never closes. Escaped quotes are stepped over;
    /// a wrong answer here only costs a resize, never a correct result.
    fn string_extent(&self) -> usize {
        let mut at = self.at;
        while at < self.data.len() {
            match self.data[at] {
                b'\\' => at += 2,
                b'"' => return at - self.at,
                _ => at += 1,
            }
        }
        self.data.len() - self.at
    }

    /// Reads a `\u` escape, and the second half of a surrogate pair when there
    /// is one. An unpaired surrogate becomes U+FFFD, which is the only thing a
    /// Rust string can hold and what every JSON reader that builds UTF-8 does
    /// with one.
    fn escaped_char(&mut self) -> Result<char> {
        let first = self.hex4()?;

        if !(0xD800..0xE000).contains(&first) {
            return Ok(char::from_u32(first).unwrap_or('\u{fffd}'));
        }

        // A high surrogate may be followed by its low half, written as a second
        // escape; anything else leaves it unpaired.
        if (0xD800..0xDC00).contains(&first)
            && self.at + 1 < self.data.len()
            && self.data[self.at] == b'\\'
            && self.data[self.at + 1] == b'u'
        {
            let mark = self.at;
            self.at += 1; // hex4 reads from the u, not from the backslash
            let second = self.hex4()?;
            if (0xDC00..0xE000).contains(&second) {
                let paired = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                if let Some(character) = char::from_u32(paired) {
                    return Ok(character);
                }
            }
            self.at = mark; // the second escape stands on its own
        }

        Ok('\u{fffd}')
    }

    /// Reads the four hexadecimal digits of a `\u` escape, having already read
    /// the backslash and the u.
    fn hex4(&mut self) -> Result<u32> {
        let at = self.at + 1; // the u
        if at + 4 > self.data.len() {
            return Err(self.fail_at(
                self.at - 1,
                "\\u must be followed by four hexadecimal digits",
            ));
        }

        let mut value = 0u32;
        for offset in 0..4 {
            let digit = match self.data[at + offset] {
                digit @ b'0'..=b'9' => digit - b'0',
                digit @ b'a'..=b'f' => digit - b'a' + 10,
                digit @ b'A'..=b'F' => digit - b'A' + 10,
                _ => {
                    return Err(self.fail_at(
                        self.at - 1,
                        "\\u must be followed by four hexadecimal digits",
                    ))
                }
            };
            value = value * 16 + digit as u32;
        }

        self.at = at + 4;
        Ok(value)
    }

    /// Reads a JSON number, which is stricter than Rust's own float parser is:
    /// no leading plus, no leading zero, no bare decimal point, no `inf`.
    fn number(&mut self) -> Result<Value> {
        let start = self.at;

        if self.data[self.at] == b'-' {
            self.at += 1;
            if self.at >= self.data.len() {
                return Err(self.fail_at(start, "this number has no digits"));
            }
        }

        match self.data[self.at] {
            b'0' => self.at += 1,
            b'1'..=b'9' => {
                while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                    self.at += 1;
                }
            }
            _ => return Err(self.fail_at(start, "this number has no digits")),
        }

        if self.at < self.data.len() && self.data[self.at] == b'.' {
            self.at += 1;
            if self.at >= self.data.len() || !self.data[self.at].is_ascii_digit() {
                return Err(self.fail_at(
                    start,
                    "a decimal point must be followed by at least one digit",
                ));
            }
            while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                self.at += 1;
            }
        }

        if self.at < self.data.len() && matches!(self.data[self.at], b'e' | b'E') {
            self.at += 1;
            if self.at < self.data.len() && matches!(self.data[self.at], b'+' | b'-') {
                self.at += 1;
            }
            if self.at >= self.data.len() || !self.data[self.at].is_ascii_digit() {
                return Err(self.fail_at(start, "an exponent must have at least one digit"));
            }
            while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                self.at += 1;
            }
        }

        let text = &self.text[start..self.at];
        match text.parse::<f64>() {
            // A magnitude a double cannot hold saturates to infinity, which JSON
            // cannot spell; the other implementations read it the same way.
            Ok(value) => Ok(Value::Float(value)),
            Err(_) => Err(Error::parse(
                format!("{text} cannot be read as a number"),
                start,
            )),
        }
    }
}

/// The width in bytes of the character a leading byte starts.
fn char_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shapes_json_has() {
        assert_eq!(parse("null").unwrap(), Value::Null);
        assert_eq!(parse(" true ").unwrap(), Value::Bool(true));
        assert_eq!(parse("-1.5e2").unwrap(), Value::Float(-150.0));
        assert_eq!(parse("[]").unwrap(), Value::Array(vec![]));
        assert_eq!(parse("{}").unwrap(), Value::object());
    }

    #[test]
    fn keeps_key_order_and_the_first_position_of_a_repeated_key() {
        let value = parse(r#"{"b":1,"a":2,"b":3}"#).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.keys().collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(object.get("b"), Some(&Value::Float(3.0)));
    }

    #[test]
    fn reads_escapes_and_folds_an_unpaired_surrogate() {
        assert_eq!(
            parse(r#""a\u0062\n\"\\\/""#).unwrap(),
            Value::String("ab\n\"\\/".to_owned())
        );
        assert_eq!(
            parse(r#""\ud83d\ude00""#).unwrap(),
            Value::String("\u{1f600}".to_owned())
        );
        assert_eq!(
            parse(r#""\ud83d""#).unwrap(),
            Value::String("\u{fffd}".to_owned())
        );
        // The low half of a pair, followed by another escape that is not one.
        assert_eq!(
            parse(r#""\ud83d\u0041""#).unwrap(),
            Value::String("\u{fffd}A".to_owned())
        );
    }

    #[test]
    fn refuses_what_json_refuses() {
        for text in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "01",
            "1.",
            ".1",
            "+1",
            "1e",
            "'a'",
            "\"a",
            "nul",
            "{\"a\" 1}",
            "[1] [2]",
            "\"a\tb\"",
        ] {
            assert!(parse(text).is_err(), "{text} should not parse");
        }
    }

    #[test]
    fn refuses_a_document_nested_past_the_limit_rather_than_the_stack() {
        let deep = "[".repeat(MAX_PARSE_DEPTH + 2) + &"]".repeat(MAX_PARSE_DEPTH + 2);
        assert!(parse(&deep).is_err());

        let allowed = "[".repeat(MAX_PARSE_DEPTH - 1) + &"]".repeat(MAX_PARSE_DEPTH - 1);
        assert!(parse(&allowed).is_ok());
    }
}
