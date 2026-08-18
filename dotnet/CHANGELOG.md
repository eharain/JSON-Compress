# Changelog

All notable changes to `TechStyle.JsonCompress` are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-18

First release. Implements version 1 of the json-compress wire format, the same
one the JavaScript implementation writes.

### Added

- `JsonCompressor.Compress` and `Decompress`, a lossless round trip through a
  JSON-only wire format: a key catalogue, columnar tables for arrays of
  like-shaped objects, constant and dictionary columns, and a catalogue of
  repeated string values.
- Key order is preserved, including inside packed tables. Where no single column
  order can satisfy every row, the array is left as objects rather than
  reordered.
- A missing key and a null one stay distinct through the round trip.
- `Stringify` and `Parse`, the same pair at the string level.
- `CompressValue<T>`, `DecompressValue<T>`, `StringifyValue<T>` and
  `ParseValue<T>`, for data that is not already a `JsonNode`.
- `DecompressIfNeeded` and `IsCompressed`, for boundaries carrying both forms.
- `Measure`, reporting minified, compressed and gzipped sizes before you commit.
- `JsonCompressCodec`, binding a set of options once and safe to share across
  threads.
- `Locate` and `Validate`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying the parser message.
- `Minify` and `Beautify`, which write line feeds whatever the platform.
- `Bytes.ByteLength`, `GZip`, `GUnzip` and `GZipSize`.
- `TablePack.Pack`, `Unpack`, `PackAll`, `UnpackAll` and `TextToArray`, the
  columnar transform on its own for consumers outside .NET.
- A minimal JSON escaper, so text above the Basic Multilingual Plane is written
  as itself rather than as pairs of six-character escapes.
- A conformance corpus produced by the JavaScript implementation, checked in and
  asserted against in both directions.
- `TechStyle.JsonCompress.Cli`, a `dotnet tool` covering `compress`,
  `decompress`, `stats`, `minify`, `beautify` and `validate`, reading a file or
  standard input.
- Targets `net8.0` and `net10.0` with no dependencies at all, and
  `netstandard2.0` for .NET Framework 4.6.2 and later.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/dotnet-v1.0.0
