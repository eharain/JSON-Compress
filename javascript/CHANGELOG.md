# Changelog

All notable changes to `json-compress` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-18

First release.

### Added

- `compress` and `decompress`, a lossless round trip through a JSON-only wire
  format: a key catalogue, columnar tables for arrays of like-shaped objects,
  constant and dictionary columns, and a catalogue of repeated string values.
- Key order is preserved, including inside packed tables. Where no single column
  order can satisfy every row, the array is left as objects rather than
  reordered.
- A missing key and a null one stay distinct through the round trip.
- `stringify` and `parse`, the same pair at the string level.
- `decompressIfNeeded` and `isCompressed`, for boundaries carrying both forms.
- `measure`, reporting minified, compressed and gzipped sizes before you commit.
- `createCodec`, binding a set of options once.
- `locate` and `validate`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying the engine's message.
- `minify` and `beautify`.
- `gzip`, `gunzip`, `gzipSize` and `canGzip`, over the platform
  `CompressionStream` with no dependency.
- `pack`, `unpack`, `packAll` and `unpackAll`, the columnar transform on its own
  for consumers outside JavaScript.
- A command line covering `compress`, `decompress`, `stats`, `minify`,
  `beautify` and `validate`, reading a file or stdin.
- TypeScript definitions, an ESM build, a CommonJS build and a browser global.
- The wire format specification, in `SPEC.md` at the repository root.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/v1.0.0
