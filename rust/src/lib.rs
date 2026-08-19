//! Lossless JSON compression that is still JSON.
//!
//! Three ideas, applied together:
//!
//! 1. Every object key is replaced by a short token and the real names are
//!    listed once, in a catalogue. A key that occurs ten thousand times is
//!    spelled out once.
//! 2. An array of like-shaped objects becomes a table - one column list and a
//!    matrix of rows - so the keys disappear from the rows entirely. Columns
//!    whose value never changes are hoisted out of the rows altogether, and
//!    columns drawn from a small set of strings become integer indices.
//! 3. String values that repeat are moved into a second catalogue and referenced
//!    by token.
//!
//! The output is an ordinary JSON value with no binary framing, so it survives
//! anything that carries JSON: an HTTP body, a WebSocket frame, a `jsonb`
//! column, a queue message, a log line.
//!
//! ```
//! use json_compress::{parse, to_string, from_str};
//!
//! let users = parse(r#"[
//!     {"id": 1, "name": "Ada", "role": "admin", "country": "United Kingdom"},
//!     {"id": 2, "name": "Bob", "role": "admin", "country": "United Kingdom"},
//!     {"id": 3, "name": "Cy",  "role": "user",  "country": "United Kingdom"}
//! ]"#).unwrap();
//!
//! let small = to_string(&users).unwrap();
//! // {"jc":1,"k":["id","name","role","country"],
//! //  "d":{"~t":[0,1,2,3],"~e":[0,0,0,2],"~cv":["United Kingdom"],
//! //       "~r":[[1,"Ada","admin"],[2,"Bob","admin"],[3,"Cy","user"]]}}
//!
//! assert_eq!(from_str(&small).unwrap(), users);
//! ```
//!
//! The wire format is specified in `SPEC.md` at the root of the repository, and
//! this implementation writes the same bytes as the JavaScript, .NET, Python,
//! PHP and Go ones.
//!
//! # The value model
//!
//! A document is a [`Value`], and objects are [`Object`] - an ordered type
//! rather than a `HashMap`, because a hash map has no order and this format
//! promises to give key order back exactly as it arrived, including inside
//! packed tables.
//!
//! With the `serde` feature on, [`to_value`] and [`from_value`] carry any
//! `Serialize` or `Deserialize` type in and out of the model, and [`Value`] and
//! [`Object`] implement both traits themselves.
//!
//! # Features
//!
//! Neither is on by default, and with both off the crate has no dependencies at
//! all.
//!
//! - `serde` - `Serialize` and `Deserialize` for the value model, plus
//!   [`to_value`] and [`from_value`].
//! - `gzip` - the gzip figures in [`measure`], and [`gzip_bytes`] / [`gunzip`].
//!   The standard library has no deflate, which is the only reason this is not
//!   simply always on.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod column_order;
mod constants;
mod decode;
mod encode;
mod error;
mod format;
mod locate;
mod options;
mod pack;
mod parse;
mod sizes;
mod table;
mod tokens;
mod value;

#[cfg(feature = "serde")]
mod serde_support;

pub use constants::FORMAT_VERSION;
pub use decode::{decompress, is_compressed};
pub use encode::{compress, compress_with};
pub use error::{Error, Result};
pub use format::{format_float, format_json, format_json_indent};
pub use locate::{locate, Location};
pub use options::Options;
pub use pack::{
    is_packed, pack, pack_all, text_to_array, unpack, unpack_all, PackedTable, TextShape,
};
pub use parse::{parse, parse_slice, MAX_COMPRESS_DEPTH, MAX_PARSE_DEPTH};
pub use sizes::byte_length;
pub use value::{Object, Value};

#[cfg(feature = "gzip")]
pub use sizes::{gunzip, gzip_bytes, gzip_size};

#[cfg(feature = "serde")]
pub use serde_support::{from_value, to_value};

/// The version of this crate, which is not the same thing as the version of the
/// wire format ([`FORMAT_VERSION`]).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compresses a value and serialises it in one step: [`compress`] followed by
/// [`format_json`].
pub fn to_string(value: &Value) -> Result<String> {
    Ok(format_json(&compress(value)?))
}

/// [`to_string`] with the encoder's choices spelled out.
pub fn to_string_with(value: &Value, options: &Options) -> Result<String> {
    Ok(format_json(&compress_with(value, options)?))
}

/// [`to_string`] with the envelope indented by `indent` spaces.
pub fn to_string_pretty(value: &Value, indent: usize) -> Result<String> {
    Ok(format_json_indent(&compress(value)?, indent))
}

/// Parses a compressed document and restores the original value: [`parse`]
/// followed by [`decompress`].
pub fn from_str(text: &str) -> Result<Value> {
    decompress(&parse(text)?)
}

/// Restores a value whether or not it was compressed.
///
/// Handy at a boundary you do not fully control - an endpoint being migrated, a
/// cache holding entries written by two versions of the same service.
pub fn decompress_if_needed(value: &Value) -> Result<Value> {
    if is_compressed(value) {
        return decompress(value);
    }
    Ok(value.clone())
}

/// Strips the whitespace from a JSON document without changing what it means.
pub fn minify(text: &str) -> Result<String> {
    Ok(format_json(&parse(text)?))
}

/// Re-indents a JSON document.
///
/// Line feeds are written whatever the platform, so a document beautified on
/// Windows matches the same document beautified anywhere else.
pub fn beautify(text: &str, indent: usize) -> Result<String> {
    Ok(format_json_indent(&parse(text)?, indent))
}

/// Checks that a string is valid JSON, and says exactly where it stops being so.
///
/// The verdict comes from the parser, so it is the same verdict [`parse`] would
/// give. The location comes from a scan of the document, because parser messages
/// are written for parser authors and often name the wrong culprit.
///
/// ```
/// use json_compress::validate;
///
/// let found = validate("{\n  \"a\": 1,\n}");
///
/// assert!(!found.valid);
/// assert_eq!(found.error, "a trailing comma is not allowed in JSON");
/// assert_eq!(found.line, 2);
/// ```
pub fn validate(text: &str) -> Location {
    let error = match parse(text) {
        Ok(_) => return Location::valid(),
        Err(error) => error,
    };

    let found = locate(text);
    if !found.valid {
        return found;
    }

    // The scan found nothing but the parse still failed - report what the parser
    // said rather than claiming the document is fine.
    Location {
        valid: false,
        error: error.to_string(),
        ..Location::default()
    }
}

/// What compression bought on one value.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The bytes of the input as given.
    pub original: usize,
    /// The bytes once whitespace is removed.
    pub minified: usize,
    /// The bytes of the compressed envelope.
    pub compressed: usize,
    /// The bytes saved against the minified form, which is negative when the
    /// envelope costs more than it saves.
    pub saved: i64,
    /// `compressed` divided by `minified`, 0 to 1.
    pub ratio: f64,
    /// The percentage saved against the minified form.
    pub percent: f64,
    /// The minified form after gzip, with the `gzip` feature on.
    pub gzip_minified: Option<usize>,
    /// The compressed form after gzip, with the `gzip` feature on.
    pub gzip_compressed: Option<usize>,
}

/// Reports what compression buys on a given value.
///
/// The honest comparison is against minified JSON, not against whatever
/// whitespace the input happened to arrive with, so `ratio` and `percent` both
/// use the minified size as their baseline. With the `gzip` feature on the
/// report also carries gzip figures, because that is what most payloads meet in
/// production and the interesting question is whether the format still helps
/// once gzip has had its turn.
pub fn measure(value: &Value) -> Result<Report> {
    measure_with(value, &Options::default())
}

/// [`measure`] with the encoder's choices spelled out.
pub fn measure_with(value: &Value, options: &Options) -> Result<Report> {
    let minified = format_json(value);
    let original = minified.len();
    build_report(value, minified, original, options)
}

/// [`measure`] on a document that arrived as text, so that `original` reflects
/// what really came in, whitespace and all.
pub fn measure_text(text: &str) -> Result<Report> {
    measure_text_with(text, &Options::default())
}

/// [`measure_text`] with the encoder's choices spelled out.
pub fn measure_text_with(text: &str, options: &Options) -> Result<Report> {
    let value = parse(text)?;
    let minified = format_json(&value);
    build_report(&value, minified, text.len(), options)
}

fn build_report(
    value: &Value,
    minified: String,
    original: usize,
    options: &Options,
) -> Result<Report> {
    let compressed = to_string_with(value, options)?;

    let mut report = Report {
        original,
        minified: minified.len(),
        compressed: compressed.len(),
        saved: minified.len() as i64 - compressed.len() as i64,
        ratio: 1.0,
        percent: 0.0,
        gzip_minified: None,
        gzip_compressed: None,
    };

    #[cfg(feature = "gzip")]
    {
        report.gzip_minified = Some(sizes::gzip_size(minified.as_bytes()));
        report.gzip_compressed = Some(sizes::gzip_size(compressed.as_bytes()));
    }

    if !minified.is_empty() {
        report.ratio = compressed.len() as f64 / minified.len() as f64;
        report.percent = report.saved as f64 / minified.len() as f64 * 100.0;
    }
    Ok(report)
}

/// A set of options, bound once and reused.
///
/// A codec is the natural unit when the same options apply everywhere - a
/// transport layer, a cache adapter, a storage helper - and it keeps the option
/// struct off every call site. It holds nothing but its options and never
/// changes them, so one built at start-up is safe to share across threads.
///
/// ```
/// use json_compress::{Codec, Options, parse};
///
/// let payloads = Codec::new(Options { min_rows: 10, ..Options::default() }).unwrap();
///
/// let value = parse(r#"[{"a": 1}, {"a": 2}]"#).unwrap();
/// let text = payloads.to_string(&value).unwrap();
///
/// // Two rows is under the codec's floor, so this one stays an array of objects.
/// assert_eq!(payloads.from_str(&text).unwrap(), value);
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Codec {
    options: Options,
}

impl Codec {
    /// Binds a set of options, validating them once rather than on every call.
    pub fn new(options: Options) -> Result<Codec> {
        options.validate()?;
        Ok(Codec { options })
    }

    /// The options this codec was built with.
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Packs a value with this codec's options.
    pub fn compress(&self, value: &Value) -> Result<Value> {
        compress_with(value, &self.options)
    }

    /// Restores a compressed value.
    pub fn decompress(&self, envelope: &Value) -> Result<Value> {
        decompress(envelope)
    }

    /// Compresses a value with this codec's options and serialises it.
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self, value: &Value) -> Result<String> {
        to_string_with(value, &self.options)
    }

    /// Parses and restores a compressed document.
    pub fn from_str(&self, text: &str) -> Result<Value> {
        from_str(text)
    }

    /// Reports what this codec's options buy on a value.
    pub fn measure(&self, value: &Value) -> Result<Report> {
        measure_with(value, &self.options)
    }
}
