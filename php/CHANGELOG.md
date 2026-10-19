# Changelog

All notable changes to `tech-style/json-compress` are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.4] - 2026-10-20

### Changed

- The README shows the introduction video. No change in behaviour or in the
  wire format.

## [1.0.3] - 2026-10-13

First release of the PHP implementation. Its version number matches the other
implementations; 1.0.0 was never published.

### Added

- `JsonCompress::compress()` and `JsonCompress::decompress()`, a lossless round
  trip through a JSON-only wire format: a key catalogue, columnar tables for
  lists of like-shaped objects, constant and dictionary columns, and a catalogue
  of repeated string values.
- Objects are `stdClass` and arrays are PHP lists, so an empty object stays
  distinct from an empty array and a key of `"7"` stays a string key. Input is
  more forgiving: an associative array is read as an object and a list as an
  array, the way `json_encode` reads them, so records from PDO go straight in.
- Key order is preserved, including inside packed tables. Where no single column
  order can satisfy every row, the list is left as objects rather than reordered.
- A missing key and a null one stay distinct through the round trip.
- `stringify()` and `parse()`, the same pair at the string level.
- `decompressIfNeeded()` and `isCompressed()`, for boundaries carrying both forms.
- `measure()`, reporting minified, compressed and gzipped sizes before you commit.
- `codec()` and `Codec`, binding a set of options once. `CompressOptions` is
  immutable and validates on construction.
- `locate()` and `validate()`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying `json_decode`'s "Syntax error".
  The scan is iterative, so a pathologically nested document is reported as too
  deep rather than exhausting the stack, and offsets count characters rather
  than bytes.
- `minify()` and `beautify()`, and `toJson()` / `parseJson()` for ordinary JSON
  written the way the rest of the format writes it.
- Depth limits chosen so the round trip cannot be broken by nesting: anything
  `json_decode` accepts by default can be compressed, and the reader accepts
  enough beyond that to read back the envelope, which nests half again as deep
  as its source in the worst case.
- `Sizes::byteLength()`, `gzip()`, `gunzip()`, `gzipSize()` and `canGzip()`, with
  a zeroed gzip timestamp so sizes are stable, and a null size rather than a
  throw on a build without zlib.
- `TablePack::pack()`, `unpack()`, `packAll()`, `unpackAll()`, `isPacked()` and
  `textToArray()`, the columnar transform on its own for consumers outside PHP.
  Records may be associative arrays or objects.
- A command line covering `compress`, `decompress`, `stats`, `minify`,
  `beautify` and `validate`, reading a file or stdin, installed as
  `vendor/bin/json-compress`.
- A bundled PSR-4 autoloader, so the package can be used from a checkout with no
  Composer involved.
- Float formatting that follows the ECMAScript rule rather than PHP's, and that
  ignores the `precision` and `serialize_precision` settings, so this
  implementation writes the same bytes as the JavaScript, .NET and Python ones.
  Integers keep PHP's exact 64-bit precision.
- A conformance suite run against the 144-case corpus written by the JavaScript
  encoder, asserting both that every envelope is restored and that every
  envelope is reproduced byte for byte, plus the shared seeded generator so a
  seed names the same document in every language.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/php-v1.0.0
