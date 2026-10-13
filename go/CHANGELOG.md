# Changelog

All notable changes to the Go implementation are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.3] - 2026-10-13

First release of the Go implementation. Its version number matches the other
implementations; 1.0.0 was never published.

### Added

- `Compress` and `Decompress`, a lossless round trip through a JSON-only wire
  format: a key catalogue, columnar tables for slices of like-shaped objects,
  constant and dictionary columns, and a catalogue of repeated string values.
- `Object`, an ordered JSON object, so key order survives the round trip -
  including inside packed tables. Where no single column order can satisfy every
  row, the slice is left as objects rather than reordered. A Go map is written
  in sorted order, as encoding/json writes one.
- A missing key and a nil one stay distinct through the round trip, which
  `Object.Get` reports separately.
- `Marshal`, `MarshalIndent` and `Unmarshal`, the same pair at the byte level.
- `DecompressIfNeeded` and `IsCompressed`, for boundaries carrying both forms.
- `Measure` and `MeasureText`, reporting minified, compressed and gzipped sizes
  before you commit.
- `Codec`, binding a set of options once and validating them once.
- `Locate` and `Validate`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying the parser's message. The scan
  reports a pathologically nested document as too deep rather than exhausting
  the stack, and columns are counted in characters while the offset is a byte
  offset, which is what a Go caller can slice with.
- `Minify` and `Beautify`, and `ParseJSON` / `FormatJSON` for ordinary JSON
  written the way the rest of the format writes it.
- `Normalize`, reading anything encoding/json can marshal - structs with their
  tags, maps, slices, `json.Marshaler`, `encoding.TextMarshaler`, `time.Time` -
  into the value model, with the format's own rules for the values JSON cannot
  carry: a non-finite number becomes null, negative zero becomes zero, and a
  cycle is an error rather than a hang.
- `ByteLength`, `GzipBytes`, `Gunzip` and `GzipSize`, over the standard library
  with no dependency and a zeroed gzip timestamp so sizes are stable.
- `Pack`, `Unpack`, `PackAll`, `UnpackAll`, `IsPacked` and `TextToArray`, the
  columnar transform on its own for consumers outside Go.
- A command line covering `compress`, `decompress`, `stats`, `minify`,
  `beautify` and `validate`, reading a file or stdin, installable with
  `go install github.com/eharain/JSON-Compress/go/cmd/json-compress@latest`.
- Float formatting that follows the ECMAScript rule rather than
  `strconv.FormatFloat`'s, so this implementation writes the same bytes as the
  JavaScript, .NET, Python and PHP ones. Integers keep Go's exact 64-bit
  precision, signed and unsigned.
- A reader written against the JSON grammar directly rather than over
  encoding/json's token stream, which boxes every string and number and costs an
  allocation apiece. It accepts and refuses exactly what encoding/json does,
  including how an unpaired surrogate is folded to U+FFFD, and a fuzz target
  holds it to that.
- A conformance suite run against the 144-case corpus written by the JavaScript
  encoder, asserting both that every envelope is restored and that every
  envelope is reproduced byte for byte, plus the shared seeded generator so a
  seed names the same document in every language.
- A `FuzzRoundTrip` target, so `go test -fuzz` pokes at the parser as well as
  the encoder.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/go%2Fv1.0.0
