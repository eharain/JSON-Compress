# json-compress

**Lossless JSON compression that is still JSON.** No dependencies, no binary
framing, nothing but the standard library. Typically 45–75% smaller, and an
exact round trip.

[![crates.io](https://img.shields.io/crates/v/json-compress.svg)](https://crates.io/crates/json-compress)
[![docs.rs](https://img.shields.io/docsrs/json-compress)](https://docs.rs/json-compress)
[![rust version](https://img.shields.io/badge/rust-1.70%2B-dea584.svg)](https://www.rust-lang.org/)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](https://github.com/eharain/JSON-Compress/blob/main/LICENSE)
[![six implementations](https://img.shields.io/badge/implementations-6_languages-a3e635.svg)](https://github.com/eharain/JSON-Compress#one-format-six-languages-the-same-bytes)

**One of six implementations of one wire format** — JavaScript, .NET, Python,
PHP, Go and Rust. Compress in any of them, restore in any other, byte for byte.
[How that is guaranteed ↓](#the-same-format-in-six-languages)

```bash
cargo add json-compress --features serde
```

```rust
use serde::{Deserialize, Serialize};
use json_compress::{to_value, from_value, to_string, from_str};

#[derive(Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    role: String,
    country: String,
}

let users = vec![
    User { id: 1, name: "Ada".into(), role: "admin".into(), country: "United Kingdom".into() },
    User { id: 2, name: "Bob".into(), role: "admin".into(), country: "United Kingdom".into() },
    User { id: 3, name: "Cy".into(),  role: "user".into(),  country: "United Kingdom".into() },
];

let small = to_string(&to_value(&users)?)?;
// {"jc": 1,
//  "k": ["id", "name", "role", "country"],
//  "d": {"~t": [0, 1, 2, 3],
//        "~e": [0, 0, 0, 2],
//        "~cv": ["United Kingdom"],
//        "~r": [[1, "Ada", "admin"], [2, "Bob", "admin"], [3, "Cy", "user"]]}}
// 'country' never changes, so it is stored once. Three rows is too few for the
// string catalogue to pay off on 'role' as well - at a thousand rows it would.

let back: Vec<User> = from_value(from_str(&small)?)?;   // exactly the records you started with
```

This is the Rust implementation of the
[json-compress format](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md).
A document written here reads correctly in the
[JavaScript](https://github.com/eharain/JSON-Compress/tree/main/javascript),
[.NET](https://github.com/eharain/JSON-Compress/tree/main/dotnet),
[Python](https://github.com/eharain/JSON-Compress/tree/main/python),
[PHP](https://github.com/eharain/JSON-Compress/tree/main/php) and
[Go](https://github.com/eharain/JSON-Compress/tree/main/go)
implementations, and the other way round — which is not a claim, but a
[test](tests/conformance.rs) run against a corpus the JavaScript encoder
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
| **Columnar tables** | An array of like-shaped objects becomes a column list and a rows matrix — the keys leave the rows entirely. |
| **Constant columns** | A column whose value never changes is lifted out of the rows and stored once. |
| **Dictionary columns** | A column drawn from a small set of strings becomes integer indices. |
| **String catalogue** | Any string value that repeats often enough to pay for itself is written once. |

The output is still ordinary JSON, so it goes anywhere JSON goes: an HTTP body,
a WebSocket frame, a Postgres `jsonb` column, a queue message, a log line.
Nothing in your stack needs to learn a new format.

## What it measures

From `cargo run --release --example benchmark --features gzip`, which
round-trips every case before reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 12.7 KB | 16% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.5 KB | 0% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 11.5 KB | 15% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.7 KB | 13% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 6.9 KB | 3% |
| Config document (deeply nested, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −21% |

The last two columns are the honest ones. Most JSON meets gzip on the way out,
and gzip already removes a lot of this redundancy — so the question is what is
left. On record-shaped data, a further 10–20% on top of gzip. On a small config
file with little repetition, the envelope costs more than it saves, and the
table says so.

The first four columns match the other five implementations' benchmarks figure
for figure, because all six build the same documents from the same generator and
all six encoders write the same bytes. The gzip columns differ by a few
percentage points: `miniz_oxide` and zlib make different choices, and nothing in
this library can change that.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with `measure`
before you commit.

## Where it helps most

- **API responses.** Less to serialise, less to send, less to parse.
- **Cache entries.** Redis and Memcached charge by the byte.
- **WebSocket traffic**, where per-message compression is often off.
- **Queue and job payloads** with per-message size limits.
- **JSON columns in a database.** Postgres `jsonb`, MySQL `JSON`, SQLite `TEXT`:
  smaller rows, smaller indexes, smaller backups.
- **Logs and telemetry**, which are almost entirely repeated keys.

---

## Install

```bash
cargo add json-compress
```

Rust 1.70 and later.

### Features

Neither is on by default, and with both off the crate has **no dependencies at
all** — everything it needs is in the standard library.

| Feature | What it adds | What it costs |
|---|---|---|
| `serde` | `Serialize` and `Deserialize` for the value model, plus `to_value` and `from_value`, so any serde type can be compressed. | `serde` |
| `gzip` | The gzip figures in `measure`, and `gzip_bytes` / `gunzip` / `gzip_size`. | `flate2` |

```toml
[dependencies]
json-compress = { version = "1", features = ["serde"] }
```

`gzip` is a feature rather than a fact of the crate because the standard library
has no deflate. Without it, `Report::gzip_minified` and `Report::gzip_compressed`
are `None` and everything else works exactly the same.

The command line installs on its own:

```bash
cargo install json-compress --features gzip
json-compress stats orders.json
```

---

## The value model

A document is a `Value`, and it holds the shapes JSON has and no others:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Object(Object),
}
```

`Object` is an ordered map, and it has to be. A `HashMap` has no order and a
`BTreeMap` remembers the wrong one, so decoding into either would shuffle every
document that passes through — and this format promises to give key order back
exactly as it arrived, including inside packed tables.

```rust
use json_compress::{Object, Value};

let row = Object::new().with("id", 1).with("name", "Ada");

row.len();                    // 2
row.keys().collect::<Vec<_>>();   // ["id", "name"]
row.at(0);                    // Some(("id", &Value::Int(1)))
row.get("name");              // Some(&Value::String("Ada"))
row.contains_key("email");    // false - which is not the same as a null value
```

Numbers keep three shapes rather than one. Text read by `parse` always becomes
`Value::Float`, as it does in JavaScript and in every other implementation of
this format; `Int` and `Uint` are there so a 64-bit integer built in Rust
survives with its exact value rather than being folded through a double.

### With serde

With the `serde` feature on, anything that implements `Serialize` goes in and
anything that implements `Deserialize` comes back out:

```rust
use json_compress::{to_value, from_value, to_string, from_str};

let envelope = to_string(&to_value(&orders)?)?;
let back: Vec<Order> = from_value(from_str(&envelope)?)?;
```

Struct fields keep the order they are declared in, which is the order they come
back in. `Value` and `Object` are themselves `Serialize` and `Deserialize`, so an
envelope drops straight into an existing `serde_json` boundary — an `axum`
handler, a `reqwest` body, a `sqlx` `Json` column.

An integer field is filled from the double the reader produced when it holds a
whole number exactly, and refused when it does not, so `{"n": 1.5}` will not
quietly become `1`.

---

## API

Everything is at the crate root.

### `compress(&Value)` and `decompress(&Value)`

Compress a document into an envelope, and restore it. The envelope is an
ordinary JSON value, so it can be stored, posted or logged anywhere plain JSON
goes. `decompress` returns an `Error::Decode` if the envelope is not readable,
rather than something nearly right.

`compress_with(&value, &options)` spells out the encoder's choices.

### `to_string(&Value)` / `to_string_pretty(&Value, indent)` and `from_str(&str)`

The same pair at the text level: compress then serialise, parse then restore.

```rust
let text = json_compress::to_string(&orders)?;
let back = json_compress::from_str(&text)?;
```

### `decompress_if_needed(&Value)` and `is_compressed(&Value)`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `measure(&Value)` and `measure_text(&str)`

What compression would buy, before you commit to it — here on the 1,000-user
record set the benchmark builds:

```rust
let report = json_compress::measure(&users)?;
report.minified;         // 211180
report.compressed;       // 56136
report.percent;          // 73.4
report.gzip_minified;    // Some(15471) with the gzip feature, None without
report.gzip_compressed;  // Some(13039)
```

`ratio` and `percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. `measure_text` takes the document as
it really came in, so `original` reflects the bytes on the wire.

### `Codec::new(options)`

Bind options once — the natural unit for a transport layer or a storage adapter.
It holds nothing but its options and never changes them, so one built at start-up
is safe to share across threads:

```rust
use json_compress::{Codec, Options};

let payloads = Codec::new(Options { min_rows: 10, ..Options::default() })?;

let text = payloads.to_string(&payload)?;
let back = payloads.from_str(&text)?;
```

### `validate(&str)` and `locate(&str)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```rust
let found = json_compress::locate("{\n  \"a\": 1,\n  \"b\": oops\n}");
// valid:    false
// error:    expected a value, found "o"
// position: 19,  line: 3,  column: 8
// excerpt:  `  "b": oops`
```

Most parsers report something like `expected value at line 3 column 8`, which is
not enough to put a cursor anywhere useful, so this scans the document itself
against RFC 8259. It knows about single quotes, trailing commas, `True`/`None`,
leading zeros, unterminated strings and bad escapes. `Location` implements
`Display`, so a found problem is ready to log.

`position` is a byte offset, so `&text[found.position..]` starts at the problem;
`column` counts characters, so it means what it means in an editor even when the
line above holds an emoji.

### `minify(&str)` and `beautify(&str, indent)`

Whitespace only, nothing else touched. Both write line feeds whatever the
platform, so a document beautified on Windows matches the same document
beautified anywhere else.

### `parse(&str)`, `format_json(&Value)` and `format_json_indent(&Value, indent)`

Ordinary JSON, no envelope — but written byte for byte as `JSON.stringify` would
write it (see [Numbers](#numbers-are-written-the-way-every-implementation-writes-them)
below). `parse_slice` reads bytes that are not known to be UTF-8.

### `byte_length`, `gzip_bytes`, `gunzip`, `gzip_size`

UTF-8 byte counts — what actually goes over the wire — and, with the `gzip`
feature, gzip with a zeroed timestamp so the same input always measures the same.

### `pack(&Value)` and `unpack(&Value)`

The columnar idea on its own, with no envelope and no catalogue:

```rust
let table = json_compress::pack(&records).unwrap();
// {"fields": ["id", "name"], "data": [[1, "Ada"], [2, "Bob"]], "count": 2}
```

Useful when the other end is not Rust, or when you want something a human can
read straight off a network panel. `pack_all` / `unpack_all` do the same for an
array of result sets, which is what a multi-statement query returns. The return
is an `Option`, so a value of the wrong shape is refused rather than silently
mangled.

---

## Options

```rust
use json_compress::Options;

let options = Options {
    strings: true,           // de-duplicate repeated string values
    tables: true,            // pack arrays of like-shaped objects into tables
    constants: true,         // hoist unchanging columns out of the rows
    dict_columns: true,      // encode low-cardinality string columns as indices
    min_rows: 2,             // smallest array that may become a table
    max_hole_ratio: 0.3,     // largest share of missing cells a table may carry
    min_string_count: 2,     // times a string must occur before it can be catalogued
    min_string_length: 1,    // shortest string that may be catalogued
};

let envelope = json_compress::compress_with(&value, &options)?;
```

`Options::default()` is that exact set, so reach for it and change what you need
with `..Options::default()`. Nonsense is refused rather than quietly obeyed —
every entry point validates, and says which field is wrong.

The two worth reaching for are `min_rows`, if you have many tiny arrays and want
them left alone, and `max_hole_ratio`, if your records are sparse and you would
rather pack them anyway.

---

## What survives the round trip

Everything JSON can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the array is left as objects rather than quietly reordered.
- **A missing key versus a null one.** `{"a": 1}` and `{"a": 1, "b": null}` in
  the same array stay different, and `Object::contains_key` tells them apart.
- **An empty object versus an empty array**, which is why objects are `Object`
  here.
- **Any string**, including ones starting with a tilde, containing control
  characters, or made of astral-plane characters.
- **Keys that look like numbers**, which most catalogue schemes reorder.
- **A key literally named `__proto__`**, which is ordinary data here and a trap
  in some of the languages this format travels to.

Values JSON cannot carry are treated the way JSON treats them: a non-finite
number is written as `null` and negative zero as `0`. A `Value` is a tree, so
there is no cycle to detect.

This is checked by 2,000 randomly generated documents on every test run, each
compared byte for byte against what it started as, plus every combination of the
options above, plus a corpus written by the JavaScript implementation, plus a
stress pass that throws random, truncated and mutated documents at the reader and
the decoder.

One bound is worth knowing: **a document nested more than 512 levels deep is
refused rather than read**, and `compress` draws its own line at 256, because an
envelope nests about half again as deep as the document it was written from. Both
walk the document with the call stack, and the limits are set against an
unoptimised build's stack frames rather than a release build's. For comparison,
`serde_json` allows 128.

### Numbers are written the way every implementation writes them

The one place this crate deliberately parts company with the rest of the Rust
ecosystem is number formatting. Rust's own `f64` formatting never uses exponent
form, so `1e21` prints as twenty-two digits; the format follows the ECMAScript
rule — positional between 1e-7 and 1e21, exponent form outside it — because that
is what the wire format is defined against and what every document in the
conformance corpus was written with.

The digits themselves are identical: both find the shortest decimal that reads
back as the same double. `Value::Int` and `Value::Uint` keep their exact 64-bit
precision rather than being folded through a float, so a value too large for a
double survives here where it would not in JavaScript.

Strings are escaped the same way too, which means `<`, `>`, `&`, `/` and the line
separators U+2028 and U+2029 are written raw. The document means the same thing
either way; the bytes only match one of them.

The reader has two deliberate differences from `serde_json`, both in the same
direction — towards what `JSON.parse` does, which is what this format is defined
against. A magnitude no double can hold reads as infinity and writes back as
`null`, rather than making the document invalid; and an unpaired surrogate escape
becomes U+FFFD, rather than making the document invalid.

---

## Command line

```bash
cargo install json-compress --features gzip

json-compress stats orders.json
json-compress compress orders.json -o orders.jc.json
json-compress decompress orders.jc.json --pretty
curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
```

`stats` changes nothing and reports what compression would save, with and
without gzip. `validate`, `minify` and `beautify` are there too. Run
`json-compress --help` for the full list.

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
| Python | [`json-compress`](https://pypi.org/project/json-compress/) on PyPI | `pip install json-compress` | |
| PHP | [`tech-style/json-compress`](https://packagist.org/packages/tech-style/json-compress) on Packagist | `composer require tech-style/json-compress` | |
| Go | [`github.com/eharain/JSON-Compress/go`](https://pkg.go.dev/github.com/eharain/JSON-Compress/go) | `go get github.com/eharain/JSON-Compress/go` | |
| **Rust** | [`json-compress`](https://crates.io/crates/json-compress) on crates.io | `cargo add json-compress` | **this one** |

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
cargo test --all-features
cargo clippy --all-targets --all-features
cargo fmt --check
cargo run --release --example benchmark --features gzip
```

The conformance corpus lives at
`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json` and is
shared with the .NET, Python, PHP and Go suites rather than duplicated, so the
Rust tests prove this implementation agrees with the reference without Node being
installed. To check the other direction as well:

```bash
node tests/interop/verify-interop.mjs    # every case through both implementations
```

The reader is written against the JSON grammar directly rather than over another
crate's, so `tests/json_text.rs` holds it to `serde_json`'s answers on several
hundred documents: the same documents accepted, the same refused, the same values
read out of them, bar the two divergences described above.

The crate lives in a subdirectory of a repository that holds six implementations,
so its release tags carry that subdirectory: `rust-v1.0.0`.

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
