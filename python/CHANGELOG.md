# Changelog

All notable changes to `json-compress` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.3] - 2026-10-13

First release of the Python implementation. Its version number matches the other
implementations; 1.0.0 was never published.

### Added

- `compress` and `decompress`, a lossless round trip through a JSON-only wire
  format: a key catalogue, columnar tables for lists of like-shaped dicts,
  constant and dictionary columns, and a catalogue of repeated string values.
- Key order is preserved, including inside packed tables. Where no single column
  order can satisfy every row, the list is left as dicts rather than reordered.
- A missing key and a null one stay distinct through the round trip.
- `stringify` and `parse`, the same pair at the string level.
- `decompress_if_needed` and `is_compressed`, for boundaries carrying both forms.
- `measure`, reporting minified, compressed and gzipped sizes before you commit.
- `create_codec` and `JsonCompressCodec`, binding a set of options once.
- `locate` and `validate`, reporting the exact line, column and cause of a
  malformed JSON document rather than relaying the parser's message. The scan is
  iterative, so a pathologically nested document is reported as too deep rather
  than exhausting the interpreter stack.
- `minify` and `beautify`.
- `byte_length`, `gzip_bytes`, `gunzip` and `gzip_size`, over the standard
  library with no dependency and a zeroed gzip timestamp so sizes are stable.
- `pack`, `unpack`, `pack_all`, `unpack_all` and `text_to_array`, the columnar
  transform on its own for consumers outside Python.
- A command line covering `compress`, `decompress`, `stats`, `minify`,
  `beautify` and `validate`, reading a file or stdin, also reachable as
  `python -m json_compress`.
- Type hints throughout, and a `py.typed` marker.
- Conversion of the standard library types a JSON writer is expected to handle -
  `datetime`, `date`, `time`, `Decimal`, `UUID`, `Enum`, dataclasses and tuples -
  and a `for_json()` hook for anything else.
- Float formatting that follows the ECMAScript rule rather than `repr`, so this
  implementation writes the same bytes as the JavaScript and .NET ones. Integers
  keep Python's exact precision.
- A conformance suite run against the 144-case corpus written by the JavaScript
  encoder, asserting both that every envelope is restored and that every
  envelope is reproduced byte for byte.

[1.0.0]: https://github.com/eharain/JSON-Compress/releases/tag/python-v1.0.0
