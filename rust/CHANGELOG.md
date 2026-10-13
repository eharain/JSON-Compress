# Changelog

All notable changes to the Rust implementation are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.3] - 2026-10-13

First release of the Rust implementation. Its version number matches the other
implementations; 1.0.0 was never published.

### Added

- `compress` and `decompress`, a lossless round trip through a JSON-only wire
  format: a key catalogue, columnar tables for arrays of like-shaped objects,
  constant and dictionary columns, and a catalogue of repeated string values.
- `Value` and `Object`, the value model. `Object` is ordered, so key order
  survives the round trip - including inside packed tables. Where no single
  column order can satisfy every row, the array is left as objects rather than
  reordered.
- A missing key and a null one stay distinct through the round trip, which
  `Object::contains_key` reports separately.
- `to_string`, `to_string_pretty` and `from_str`, the same pair at the text
  level.
- `decompress_if_needed` and `is_compressed`, for boundaries carrying both forms.
- `measure` and `measure_text`, reporting minified, compressed and gzipped sizes
  before you commit.
- `Codec`, binding a set of options once and validating them once.
- `locate` and `validate`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying the parser's message. Columns are
  counted in characters while the offset is a byte offset, which is what a Rust
  caller can slice with.
- `minify` and `beautify`, and `parse` / `format_json` for ordinary JSON written
  the way the rest of the format writes it.
- `pack`, `unpack`, `pack_all`, `unpack_all`, `is_packed` and `text_to_array`,
  the columnar transform on its own for consumers outside Rust.
- A command line covering `compress`, `decompress`, `stats`, `minify`,
  `beautify` and `validate`, reading a file or stdin, installable with
  `cargo install json-compress`.
- An optional `serde` feature: `Serialize` and `Deserialize` for `Value` and
  `Object`, plus `to_value` and `from_value` for any serde type. Struct fields
  keep their declared order, a map keyed by numbers survives the round trip its
  own `Serialize` makes necessary, and an integer field is filled from the double
  the reader produced only when it holds a whole number exactly.
- An optional `gzip` feature: the gzip figures in `measure`, and `gzip_bytes` /
  `gunzip` / `gzip_size`, with a zeroed timestamp so sizes are stable. It is a
  feature rather than a fact of the crate because the standard library has no
  deflate; with both features off there are no dependencies at all.
- Float formatting that follows the ECMAScript rule rather than Rust's, which
  never uses exponent form, so this implementation writes the same bytes as the
  JavaScript, .NET, Python, PHP and Go ones. `Value::Int` and `Value::Uint` keep
  their exact 64-bit precision.
- A reader written against the JSON grammar directly. It parts company with
  `serde_json` in two places, both towards what `JSON.parse` does: a magnitude no
  double can hold reads as infinity and writes back as `null`, and an unpaired
  surrogate escape becomes U+FFFD - where `serde_json` refuses the document
  outright. `tests/json_text.rs` holds it to `serde_json` on everything else.
- A conformance suite run against the 144-case corpus written by the JavaScript
  encoder, asserting both that every envelope is restored and that every envelope
  is reproduced byte for byte, plus the shared seeded generator so a seed names
  the same document in every language.
- A stress suite that throws random, truncated and mutated documents at the
  reader and the decoder, and insists on the only two outcomes either is allowed
  to have.

### Known limits

- A document nested more than 512 levels deep is refused rather than read, and
  `compress` draws its own line at 256, because an envelope nests about half
  again as deep as the document it was written from. Both walk the document with
  the call stack, and the limits are set against an unoptimised build's stack
  frames. `serde_json` allows 128 by comparison.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/rust-v1.0.0
