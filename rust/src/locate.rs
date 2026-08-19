//! Finding the exact place a JSON document goes wrong.
//!
//! A parser reports a message, and what that message contains depends on the
//! parser: sometimes a byte offset, sometimes a line and column, increasingly
//! often just a fragment of the text with no position at all. None of that is
//! enough to put a cursor on the offending character.
//!
//! So this scans the document itself, against the grammar in RFC 8259, and
//! reports the first thing that does not belong - with an offset, a line, a
//! column and a description written for whoever has to fix it.

use std::fmt;

/// What a scan of a JSON document found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Location {
    /// True when the document is well-formed JSON, in which case every other
    /// field is empty.
    pub valid: bool,
    /// What is wrong, in plain words.
    pub error: String,
    /// The byte offset of the problem, so `&text[position..]` starts at it.
    pub position: usize,
    /// The line number, counted from one.
    pub line: usize,
    /// The column number, counted from one in characters - so a column means
    /// what it means in an editor even when the line above holds an emoji.
    pub column: usize,
    /// The line the problem is on.
    pub excerpt: String,
}

impl Location {
    /// The location of a document that is fine.
    pub fn valid() -> Location {
        Location {
            valid: true,
            ..Location::default()
        }
    }
}

/// A one-line summary, ready to log.
impl fmt::Display for Location {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.valid {
            return out.write_str("valid JSON");
        }
        if self.line == 0 {
            return write!(out, "invalid JSON: {}", self.error);
        }
        write!(
            out,
            "invalid JSON at line {}, column {}: {}",
            self.line, self.column, self.error
        )
    }
}

/// Somewhere to unwind to, carrying the offset the problem was found at.
struct ScanError {
    message: String,
    at: usize,
}

fn scan_error(message: impl Into<String>, at: usize) -> ScanError {
    ScanError {
        message: message.into(),
        at,
    }
}

type Scan = Result<(), ScanError>;

/// Checks a JSON document and, if it is broken, says exactly where.
///
/// ```
/// use json_compress::locate;
///
/// let found = locate("{\n  \"a\": 1,\n  \"b\": oops\n}");
///
/// assert!(!found.valid);
/// assert_eq!(found.line, 3);
/// assert_eq!(found.column, 8);
/// assert_eq!(found.excerpt, "  \"b\": oops");
/// ```
pub fn locate(text: &str) -> Location {
    let mut scanner = Scanner {
        text,
        data: text.as_bytes(),
        at: 0,
    };

    let mut outcome = scanner.value(0);
    if outcome.is_ok() {
        scanner.skip_whitespace();
        if scanner.at < scanner.data.len() {
            let at = scanner.at;
            outcome = Err(scan_error(
                format!(
                    "unexpected {} after the end of the document",
                    describe(text, at)
                ),
                at,
            ));
        }
    }

    let failure = match outcome {
        Ok(()) => return Location::valid(),
        Err(failure) => failure,
    };

    let at = failure.at.min(text.len());
    let (line, column, excerpt) = position_of(text, at);
    Location {
        valid: false,
        error: failure.message,
        position: at,
        line,
        column,
        excerpt: excerpt.to_owned(),
    }
}

/// Turns a byte offset into a line and column, both counted from one, and the
/// line the offset falls on.
fn position_of(text: &str, at: usize) -> (usize, usize, &str) {
    let before = &text[..at];
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |newline| newline + 1);
    let column = before[start..].chars().count() + 1;

    let rest = &text[start..];
    let excerpt = match rest.find('\n') {
        Some(end) => &rest[..end],
        None => rest,
    };
    (line, column, excerpt)
}

fn describe(text: &str, at: usize) -> String {
    let data = text.as_bytes();
    if at >= data.len() {
        return "the document ends here".to_owned();
    }
    let byte = data[at];
    if byte < 0x20 {
        return format!("an unescaped control character (0x{byte:02x})");
    }
    let character = text[at..].chars().next().unwrap_or('\u{fffd}');
    format!("\"{character}\"")
}

struct Scanner<'a> {
    text: &'a str,
    data: &'a [u8],
    at: usize,
}

impl<'a> Scanner<'a> {
    fn skip_whitespace(&mut self) {
        while self.at < self.data.len() {
            match self.data[self.at] {
                b' ' | b'\t' | b'\n' | b'\r' => self.at += 1,
                _ => return,
            }
        }
    }

    fn expect(&mut self, byte: u8, what: &str) -> Scan {
        if self.at >= self.data.len() || self.data[self.at] != byte {
            return Err(scan_error(
                format!("expected {what}, found {}", describe(self.text, self.at)),
                self.at,
            ));
        }
        self.at += 1;
        Ok(())
    }

    fn starts(&self, prefix: &str) -> bool {
        self.text[self.at..].starts_with(prefix)
    }

    fn value(&mut self, depth: usize) -> Scan {
        if depth > 5000 {
            return Err(scan_error(
                "the document is nested too deeply to check",
                self.at,
            ));
        }

        self.skip_whitespace();
        if self.at >= self.data.len() {
            return Err(scan_error(
                "expected a value, but the document ends here",
                self.at,
            ));
        }

        match self.data[self.at] {
            b'{' => return self.object(depth),
            b'[' => return self.array(depth),
            b'"' => return self.string(),
            b'-' | b'0'..=b'9' => return self.number(),
            _ => {}
        }

        if self.starts("true") {
            self.at += 4;
            return Ok(());
        }
        if self.starts("false") {
            self.at += 5;
            return Ok(());
        }
        if self.starts("null") {
            self.at += 4;
            return Ok(());
        }

        // The three mistakes that account for most hand-edited JSON.
        if self.data[self.at] == b'\'' {
            return Err(scan_error(
                "strings must use double quotes, not single quotes",
                self.at,
            ));
        }
        if self.starts("True") || self.starts("False") {
            return Err(scan_error(
                "true and false must be lower case in JSON",
                self.at,
            ));
        }
        if self.starts("None") || self.starts("undefined") {
            return Err(scan_error(
                "JSON has no None or undefined - use null",
                self.at,
            ));
        }

        Err(scan_error(
            format!("expected a value, found {}", describe(self.text, self.at)),
            self.at,
        ))
    }

    fn object(&mut self, depth: usize) -> Scan {
        self.at += 1; // the opening brace
        self.skip_whitespace();

        if self.at < self.data.len() && self.data[self.at] == b'}' {
            self.at += 1;
            return Ok(());
        }

        loop {
            self.skip_whitespace();
            if self.at < self.data.len() && self.data[self.at] != b'"' {
                if self.data[self.at] == b'\'' {
                    return Err(scan_error(
                        "strings must use double quotes, not single quotes",
                        self.at,
                    ));
                }
                return Err(scan_error(
                    format!(
                        "expected a quoted property name, found {}",
                        describe(self.text, self.at)
                    ),
                    self.at,
                ));
            }
            self.string()?;

            self.skip_whitespace();
            self.expect(b':', "a colon after the property name")?;

            self.value(depth + 1)?;
            self.skip_whitespace();

            if self.at >= self.data.len() {
                return Err(scan_error("this object is never closed", self.at));
            }
            match self.data[self.at] {
                b',' => {
                    let comma = self.at;
                    self.at += 1;
                    self.skip_whitespace();
                    if self.at < self.data.len() && self.data[self.at] == b'}' {
                        return Err(scan_error("a trailing comma is not allowed in JSON", comma));
                    }
                }
                b'}' => {
                    self.at += 1;
                    return Ok(());
                }
                _ => {
                    return Err(scan_error(
                        format!(
                            "expected a comma or a closing brace, found {}",
                            describe(self.text, self.at)
                        ),
                        self.at,
                    ))
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Scan {
        self.at += 1; // the opening bracket
        self.skip_whitespace();

        if self.at < self.data.len() && self.data[self.at] == b']' {
            self.at += 1;
            return Ok(());
        }

        loop {
            self.value(depth + 1)?;
            self.skip_whitespace();

            if self.at >= self.data.len() {
                return Err(scan_error("this array is never closed", self.at));
            }
            match self.data[self.at] {
                b',' => {
                    let comma = self.at;
                    self.at += 1;
                    self.skip_whitespace();
                    if self.at < self.data.len() && self.data[self.at] == b']' {
                        return Err(scan_error("a trailing comma is not allowed in JSON", comma));
                    }
                }
                b']' => {
                    self.at += 1;
                    return Ok(());
                }
                _ => {
                    return Err(scan_error(
                        format!(
                            "expected a comma or a closing bracket, found {}",
                            describe(self.text, self.at)
                        ),
                        self.at,
                    ))
                }
            }
        }
    }

    fn string(&mut self) -> Scan {
        let start = self.at;
        self.at += 1; // the opening quote

        while self.at < self.data.len() {
            let byte = self.data[self.at];

            if byte == b'"' {
                self.at += 1;
                return Ok(());
            }

            if byte == b'\\' {
                self.at += 1;
                if self.at >= self.data.len() {
                    break;
                }
                let escape = self.data[self.at];
                if br#""\/bfnrt"#.contains(&escape) {
                    self.at += 1;
                    continue;
                }
                if escape == b'u' {
                    for offset in 1..=4 {
                        if self.at + offset >= self.data.len()
                            || !self.data[self.at + offset].is_ascii_hexdigit()
                        {
                            return Err(scan_error(
                                "\\u must be followed by four hexadecimal digits",
                                self.at - 1,
                            ));
                        }
                    }
                    self.at += 5;
                    continue;
                }
                return Err(scan_error(
                    format!(
                        "\"\\{}\" is not a valid escape sequence",
                        char::from_u32(escape as u32).unwrap_or('\u{fffd}')
                    ),
                    self.at - 1,
                ));
            }

            if byte < 0x20 {
                return Err(scan_error(
                    format!("a raw control character inside a string - escape it as \\u{byte:04x}"),
                    self.at,
                ));
            }

            self.at += 1;
        }

        Err(scan_error("this string is never closed", start))
    }

    fn number(&mut self) -> Scan {
        let start = self.at;

        if self.data[self.at] == b'-' {
            self.at += 1;
        }

        if self.at >= self.data.len() {
            return Err(scan_error("this number has no digits", start));
        }

        if self.data[self.at] == b'0' {
            self.at += 1;
            if self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                return Err(scan_error("a number may not have a leading zero", start));
            }
        } else {
            if !self.data[self.at].is_ascii_digit() {
                return Err(scan_error(
                    format!("expected a digit, found {}", describe(self.text, self.at)),
                    self.at,
                ));
            }
            while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                self.at += 1;
            }
        }

        if self.at < self.data.len() && self.data[self.at] == b'.' {
            self.at += 1;
            let mut digits = 0;
            while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                self.at += 1;
                digits += 1;
            }
            if digits == 0 {
                return Err(scan_error(
                    "a decimal point must be followed by at least one digit",
                    self.at - 1,
                ));
            }
        }

        if self.at < self.data.len() && matches!(self.data[self.at], b'e' | b'E') {
            self.at += 1;
            if self.at < self.data.len() && matches!(self.data[self.at], b'+' | b'-') {
                self.at += 1;
            }
            let mut digits = 0;
            while self.at < self.data.len() && self.data[self.at].is_ascii_digit() {
                self.at += 1;
                digits += 1;
            }
            if digits == 0 {
                return Err(scan_error(
                    "an exponent must have at least one digit",
                    self.at - 1,
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_good_document_is_reported_as_good() {
        assert!(locate(r#"{"a": [1, 2, {"b": null}]}"#).valid);
        assert_eq!(locate("[]").to_string(), "valid JSON");
    }

    #[test]
    fn the_common_mistakes_are_named() {
        assert_eq!(
            locate("{'a': 1}").error,
            "strings must use double quotes, not single quotes"
        );
        assert_eq!(
            locate("[1, 2, ]").error,
            "a trailing comma is not allowed in JSON"
        );
        assert_eq!(
            locate("[True]").error,
            "true and false must be lower case in JSON"
        );
        assert_eq!(
            locate("[None]").error,
            "JSON has no None or undefined - use null"
        );
        assert_eq!(locate("[01]").error, "a number may not have a leading zero");
        assert_eq!(locate("[\"a]").error, "this string is never closed");
        assert_eq!(
            locate(r#"["\x"]"#).error,
            "\"\\x\" is not a valid escape sequence"
        );
    }

    #[test]
    fn the_column_is_counted_in_characters() {
        let found = locate("[\n  \"😀 emoji\", oops\n]");
        assert!(!found.valid);
        assert_eq!(found.line, 2);
        assert_eq!(found.column, 14);
        assert_eq!(&found.excerpt[..], "  \"😀 emoji\", oops");
    }
}
