# json-compress

**Lossless JSON compression that is still JSON.** No dependencies, no binary
framing, no C extension. Typically 45–75% smaller, and an exact round trip.

[![PyPI](https://img.shields.io/pypi/v/json-compress.svg)](https://pypi.org/project/json-compress/)
[![Python versions](https://img.shields.io/pypi/pyversions/json-compress.svg)](https://pypi.org/project/json-compress/)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](https://github.com/eharain/JSON-Compress/blob/main/LICENSE)
[![six implementations](https://img.shields.io/badge/implementations-6_languages-a3e635.svg)](https://github.com/eharain/JSON-Compress#one-format-six-languages-the-same-bytes)

**One of six implementations of one wire format** — JavaScript, .NET, Python,
PHP, Go and Rust. Compress in any of them, restore in any other, byte for byte.
[How that is guaranteed ↓](#the-same-format-in-six-languages)

```bash
pip install json-compress
```

```python
from json_compress import compress, decompress

users = [
    {"id": 1, "name": "Ada", "role": "admin", "country": "United Kingdom"},
    {"id": 2, "name": "Bob", "role": "admin", "country": "United Kingdom"},
    {"id": 3, "name": "Cy",  "role": "user",  "country": "United Kingdom"},
]

small = compress(users)
# {"jc": 1,
#  "k": ["id", "name", "role", "country"],
#  "d": {"~t": [0, 1, 2, 3],
#        "~e": [0, 0, 0, 2],
#        "~cv": ["United Kingdom"],
#        "~r": [[1, "Ada", "admin"], [2, "Bob", "admin"], [3, "Cy", "user"]]}}
# 'country' never changes, so it is stored once. Three rows is too few for the
# string catalogue to pay off on 'role' as well - at a thousand rows it would.

decompress(small)   # exactly the list you started with
```

This is the Python implementation of the
[json-compress format](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md).
A document written here reads correctly in the
[JavaScript](https://github.com/eharain/JSON-Compress/tree/main/javascript) and
[.NET](https://github.com/eharain/JSON-Compress/tree/main/dotnet)
implementations, and the other way round — which is not a claim, but a
[test](tests/test_conformance.py) run against a corpus the JavaScript encoder
produced.

## Video

[![JSON Compress: lossless JSON compression that is still JSON](https://raw.githubusercontent.com/eharain/JSON-Compress/main/docs/media/video-thumbnail.jpg)](https://www.youtube.com/watch?v=FlY6yl79UDo)

A 1:11 introduction on YouTube, with English captions.

---

## Why

JSON spends most of its bytes saying the same things over and over. A thousand
records is a thousand copies of every key name; a `status` column with three
possible values is a thousand copies of three short strings; a `currency` field
that is always `"GBP"` is a thousand copies of one.

This writes each of those down once.

| | |
|---|---|
| **Key catalogue** | Every key name is written once and referenced by a one- or two-character token. |
| **Columnar tables** | A list of like-shaped dicts becomes a column list and a rows matrix — the keys leave the rows entirely. |
| **Constant columns** | A column whose value never changes is lifted out of the rows and stored once. |
| **Dictionary columns** | A column drawn from a small set of strings becomes integer indices. |
| **String catalogue** | Any string value that repeats often enough to pay for itself is written once. |

The output is still ordinary JSON, so it goes anywhere JSON goes: an HTTP body,
a WebSocket frame, a `jsonb` column, a queue message, a log line. Nothing in
your stack needs to learn a new format.

## What it measures

From `python examples/benchmark.py`, which round-trips every case before
reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.4 KB | 20% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.5 KB | 5% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 11.5 KB | 19% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.8 KB | 14% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 7.1 KB | 5% |
| Config document (deeply nested, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −21% |

The last two columns are the honest ones. Most JSON meets gzip on the way out,
and gzip already removes a lot of this redundancy — so the question is what is
left. On record-shaped data, a further 5–20% on top of gzip. On a small config
file with little repetition, the envelope costs more than it saves, and the
table says so.

The first four columns match the other five implementations' benchmarks figure
for figure, because all six build the same documents from the same generator and
all six encoders write the same bytes. The gzip columns are a little
different: zlib and the browser `CompressionStream` do not tune deflate
identically.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with
`measure()` before you commit.

## Where it helps most

- **API responses.** Less to serialise, less to send, less to parse.
- **Cache entries.** Redis and friends charge by the byte.
- **WebSocket and Channels traffic**, where per-message compression is often off.
- **Queue and task payloads** with per-message size limits.
- **JSON columns in a database.** Postgres `jsonb`, SQLite `TEXT`: smaller rows,
  smaller indexes, smaller backups.
- **Logs and telemetry**, which are almost entirely repeated keys.

---

## Install

```bash
pip install json-compress
```

Python 3.9 and later. No dependencies — everything it needs is in the standard
library. The command line comes with the package:

```bash
json-compress stats orders.json
```

---

## API

Everything is importable from the top-level package.

### `compress(value, options=None, **overrides) -> dict`

Compress a document. The result is an ordinary dict of JSON-shaped values, so it
can be handed to `json.dumps`, stored, posted or logged anywhere plain JSON
goes. The input is left untouched.

### `decompress(envelope) -> Any`

Restore it. Raises `DecodeError` if the envelope is not readable, rather than
returning something nearly right. `DecodeError` is a `ValueError`, so it is
caught by the handler you already have.

### `stringify(value, options=None, indent=None)` and `parse(text)`

The same pair at the string level.

```python
text = stringify(orders)         # compress, then serialise
back = parse(text)               # parse, then restore
```

### `decompress_if_needed(value)` and `is_compressed(value)`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `measure(value, options=None, source=None, gzip=True) -> SizeReport`

What compression would buy, before you commit to it — here on the 1,000-user
record set the benchmark builds:

```python
report = measure(users)
report.minified          # 211180
report.compressed        # 56136
report.percent           # 73.4
report.gzip_minified     # 17168
report.gzip_compressed   # 13740
```

`ratio` and `percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. Pass `source=` if you want
`original` to reflect the real input, and `gzip=False` to skip the gzip
measurements.

### `create_codec(options=None, **overrides)`

Bind options once — the natural unit for a transport layer or a storage adapter.
It holds nothing but its frozen options, so one instance is safe to share across
threads:

```python
CODEC = create_codec(min_rows=10)

CODEC.stringify(payload)
CODEC.parse(text)
```

### `validate(text)` and `locate(text)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```python
locate('{\n  "a": 1,\n  "b": oops\n}')
# JsonLocation(valid=False, error='expected a value, found "o"',
#              position=19, line=3, column=8, excerpt='  "b": oops')
```

Parser messages are written for parser authors and often name the wrong culprit,
so this scans the document itself against RFC 8259. It knows about single
quotes, trailing commas, `True`/`None`, leading zeros, unterminated strings and
bad escapes. A `JsonLocation` is truthy when the document is valid, so
`if validate(text):` reads the way you would expect.

### `minify(text)` and `beautify(text, indent=2)`

Whitespace only, nothing else touched. Both write line feeds whatever the
platform, so a document beautified on Windows matches the same document
beautified anywhere else.

### `byte_length`, `gzip_bytes`, `gunzip`, `gzip_size`

UTF-8 byte counts — what actually goes over the wire — and gzip with a zeroed
timestamp, so the same input always measures the same.

### `pack(records)` and `unpack(packed)`

The columnar idea on its own, with no envelope and no catalogue:

```python
pack([{"id": 1, "name": "Ada"}, {"id": 2, "name": "Bob"}])
# {'fields': ['id', 'name'], 'data': [[1, 'Ada'], [2, 'Bob']], 'count': 2}
```

Useful when the other end is not Python, or when you want something a human can
read straight off a network panel. `pack_all` / `unpack_all` do the same for a
list of result sets, which is what multi-statement database drivers return. Both
leave anything that is not the expected shape alone, so they can be dropped into
a pipeline without a shape check first.

---

## Options

```python
from json_compress import CompressOptions, compress

compress(value, CompressOptions(
    strings=True,           # de-duplicate repeated string values
    tables=True,            # pack lists of like-shaped dicts into tables
    constants=True,         # hoist unchanging columns out of the rows
    dict_columns=True,      # encode low-cardinality string columns as indices
    min_rows=2,             # smallest list that may become a table
    max_hole_ratio=0.3,     # largest share of missing cells a table may carry
    min_string_count=2,     # times a string must occur before it can be catalogued
    min_string_length=1,    # shortest string that may be catalogued
))
```

Options may equally be a plain mapping or keyword arguments, whichever reads
better at the call site:

```python
compress(value, {"tables": False})
compress(value, min_rows=10)
```

Every default is safe on arbitrary input, and every field validates on
construction, so a nonsense value surfaces at the call site rather than as a
mysteriously large output. The two worth reaching for are `min_rows`, if you
have many tiny lists and want them left alone, and `max_hole_ratio`, if your
records are sparse and you would rather pack them anyway.

---

## What survives the round trip

Everything JSON can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the list is left as dicts rather than quietly reordered.
- **A missing key versus a null one.** `{"a": 1}` and `{"a": 1, "b": None}` in
  the same list stay different.
- **Any string**, including ones starting with a tilde, containing control
  characters, made of astral-plane characters, or holding an unpaired surrogate.
- **Keys that look like numbers**, which most catalogue schemes reorder — and
  which would be silently reordered by a JavaScript reader if the tokens were
  not letter-first.
- **A key literally named `__proto__`**, which is ordinary data here and a trap
  in some of the languages this format travels to.

Values JSON cannot carry are treated the way JSON treats them: non-finite
numbers become `None`, negative zero becomes zero, and a cycle raises rather
than hanging. Types the standard library can describe are converted the obvious
way — `datetime`, `date` and `time` to ISO 8601 text, `Decimal` to a number,
`UUID` and `Enum` to their value, dataclasses to their fields, tuples to lists.
Anything else may define a `for_json()` method, which is the nearest thing
Python has to JavaScript's `toJSON`.

This is checked by 2,000 randomly generated documents on every test run, each
compared character for character against what it started as, plus every
combination of the options above, plus a corpus written by the JavaScript
implementation.

The one bound worth knowing is nesting depth. Compressing walks the document
recursively, so a document nested deeper than the interpreter's recursion limit
raises `RecursionError` rather than being silently truncated — around 1,000
levels by default, which `sys.setrecursionlimit` will raise if you genuinely
have such a document. `locate` is iterative and has no such limit.

### Numbers are written the way every implementation writes them

The one place this package deliberately parts company with `json.dumps` is
float formatting. `repr` switches to exponent form at 1e16 and below 1e-4, so
`json.dumps` writes `1e+18` and `1e-05`; the format follows the ECMAScript rule
instead — positional between 1e-7 and 1e21 — because that is what the wire
format is defined against and what every document in the conformance corpus was
written with.

The digits themselves are identical: both languages print the shortest decimal
that reads back as the same double. Integers keep Python's exact precision
rather than being folded through a float, so a value too large for a double
survives here where it would not in JavaScript.

---

## Command line

```bash
json-compress stats orders.json
json-compress compress orders.json -o orders.jc.json
json-compress decompress orders.jc.json --pretty
curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
```

`stats` changes nothing and reports what compression would save, with and
without gzip. `validate`, `minify` and `beautify` are there too. Run
`json-compress --help` for the full list, or `python -m json_compress` if you
would rather not put the script on your PATH.

---

## The format

The wire format is fully specified in
[`SPEC.md`](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md) — enough
to write an implementation from, in any language, without reading this one. In
outline:

```json
{
  "jc": 1,
  "k":  ["the", "key", "catalogue"],
  "s":  ["repeated", "string", "values"],
  "d":  "the encoded document"
}
```

Keys become short letter-first tokens; a tilde marks a catalogue reference or an
escaped literal; a `~t` member marks a columnar table. That is the whole of it.

---

## The same format, in six languages

This is one of six implementations of the same wire format. They do not merely
interoperate: given the same document they emit **the same bytes**. A payload
compressed by any one of them is restored exactly by any other, in any
direction.

| Language | Package | Install | |
|---|---|---|---|
| JavaScript / TypeScript | [`@tech-style/json-compress`](https://www.npmjs.com/package/@tech-style/json-compress) on npm | `npm install @tech-style/json-compress` | |
| .NET | [`TechStyle.JsonCompress`](https://www.nuget.org/packages/TechStyle.JsonCompress) on NuGet | `dotnet add package TechStyle.JsonCompress` | |
| **Python** | [`json-compress`](https://pypi.org/project/json-compress/) on PyPI | `pip install json-compress` | **this one** |
| PHP | [`tech-style/json-compress`](https://packagist.org/packages/tech-style/json-compress) on Packagist | `composer require tech-style/json-compress` | |
| Go | [`github.com/eharain/JSON-Compress/go`](https://pkg.go.dev/github.com/eharain/JSON-Compress/go) | `go get github.com/eharain/JSON-Compress/go` | |
| Rust | [`json-compress`](https://crates.io/crates/json-compress) on crates.io | `cargo add json-compress` | |

Java is planned, against the same specification.

Every one of them ships the same command line, called `json-compress`, so the
guarantee is something you can watch happen:

```bash
json-compress compress  orders.json    > orders.jc.json   # in any language
json-compress decompress orders.jc.json > restored.json    # in any other
diff orders.json restored.json                             # no output
```

### How that is kept true

A corpus of **144 documents** — the awkward cases from the specification, plus
120 generated from a seeded source every suite reproduces independently — is
checked in once at
[`dotnet/tests/.../conformance.json`](https://github.com/eharain/JSON-Compress/blob/main/dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json),
each paired with the envelope the reference encoder wrote for it.

Every implementation asserts **both directions** against that one shared file on
every CI run: it must restore all 144 envelopes to the original value, and it
must write those same 144 envelopes itself. The specification only requires the
first. Doing the second as well is what turns "compatible" into "identical".

The cases are the awkward ones on purpose — the tilde escape, holes standing
against real nulls, `__proto__` as a key name, key names shaped like tokens,
catalogues large enough to need two-character tokens, rows whose key orders
disagree, a raw column whose last cell is `-1`. Each of those has broken an
implementation at some point.

The six agree on every document JavaScript could have written, and diverge only
where the languages' own number models do — on JSON text `JSON.stringify` could
not have produced in the first place. The [root
README](https://github.com/eharain/JSON-Compress#where-the-languages-are-allowed-to-differ) lists those cases
precisely. Each implementation still restores its own documents exactly, which
is all the format promises.

---

## Building and testing

```bash
python -m unittest discover tests      # 110 tests, including the JavaScript corpus
python examples/benchmark.py
python -m build
```

`pytest` runs the same suite unchanged if you prefer it.

The conformance corpus lives at
`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json` and is
shared with the .NET suite rather than duplicated, so the Python tests prove
this implementation agrees with the reference without Node being installed. To
check the other direction as well:

```bash
node tests/interop/verify-interop.mjs    # every case through both implementations
```

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
