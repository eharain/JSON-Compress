# json-compress

**Lossless JSON compression that is still JSON.** No dependencies, no binary
framing, nothing but the standard library. Typically 45–75% smaller, and an
exact round trip.

[![Go reference](https://pkg.go.dev/badge/github.com/eharain/JSON-Compress/go.svg)](https://pkg.go.dev/github.com/eharain/JSON-Compress/go)
[![Go version](https://img.shields.io/badge/go-1.21%2B-00ADD8.svg)](https://go.dev/dl/)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](https://github.com/eharain/JSON-Compress/blob/main/LICENSE)
[![six implementations](https://img.shields.io/badge/implementations-6_languages-a3e635.svg)](https://github.com/eharain/JSON-Compress#one-format-six-languages-the-same-bytes)

**One of six implementations of one wire format** — JavaScript, .NET, Python,
PHP, Go and Rust. Compress in any of them, restore in any other, byte for byte.
[How that is guaranteed ↓](#the-same-format-in-six-languages)

```bash
go get github.com/eharain/JSON-Compress/go
```

```go
import jsoncompress "github.com/eharain/JSON-Compress/go"

type User struct {
    ID      int    `json:"id"`
    Name    string `json:"name"`
    Role    string `json:"role"`
    Country string `json:"country"`
}

users := []User{
    {1, "Ada", "admin", "United Kingdom"},
    {2, "Bob", "admin", "United Kingdom"},
    {3, "Cy", "user", "United Kingdom"},
}

small, err := jsoncompress.Marshal(users)
// {"jc": 1,
//  "k": ["id", "name", "role", "country"],
//  "d": {"~t": [0, 1, 2, 3],
//        "~e": [0, 0, 0, 2],
//        "~cv": ["United Kingdom"],
//        "~r": [[1, "Ada", "admin"], [2, "Bob", "admin"], [3, "Cy", "user"]]}}
// 'country' never changes, so it is stored once. Three rows is too few for the
// string catalogue to pay off on 'role' as well - at a thousand rows it would.

back, err := jsoncompress.Unmarshal(small)   // exactly the records you started with
```

This is the Go implementation of the
[json-compress format](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md).
A document written here reads correctly in the
[JavaScript](https://github.com/eharain/JSON-Compress/tree/main/javascript),
[.NET](https://github.com/eharain/JSON-Compress/tree/main/dotnet),
[Python](https://github.com/eharain/JSON-Compress/tree/main/python) and
[PHP](https://github.com/eharain/JSON-Compress/tree/main/php)
implementations, and the other way round — which is not a claim, but a
[test](conformance_test.go) run against a corpus the JavaScript encoder
produced.

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
| **Columnar tables** | A slice of like-shaped objects becomes a column list and a rows matrix — the keys leave the rows entirely. |
| **Constant columns** | A column whose value never changes is lifted out of the rows and stored once. |
| **Dictionary columns** | A column drawn from a small set of strings becomes integer indices. |
| **String catalogue** | Any string value that repeats often enough to pay for itself is written once. |

The output is still ordinary JSON, so it goes anywhere JSON goes: an HTTP body,
a WebSocket frame, a Postgres `jsonb` column, a queue message, a log line.
Nothing in your stack needs to learn a new format.

## What it measures

From `go run ./examples/benchmark`, which round-trips every case before
reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.3 KB | 21% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.2 KB | 6% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 11.4 KB | 19% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 20.3 KB | 14% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 6.7 KB | 5% |
| Config document (deeply nested, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −21% |

The last two columns are the honest ones. Most JSON meets gzip on the way out,
and gzip already removes a lot of this redundancy — so the question is what is
left. On record-shaped data, a further 5–20% on top of gzip. On a small config
file with little repetition, the envelope costs more than it saves, and the
table says so.

The first four columns match the other five implementations' benchmarks figure
for figure, because all six build the same documents from the same generator and
all six encoders write the same bytes. The gzip columns differ by a
percentage point either way: `compress/flate` and zlib make slightly different
choices, and nothing in this library can change that.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with `Measure`
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
go get github.com/eharain/JSON-Compress/go
```

Go 1.21 and later. No dependencies — the standard library is all it needs.

The command line installs on its own:

```bash
go install github.com/eharain/JSON-Compress/go/cmd/json-compress@latest
json-compress stats orders.json
```

---

## The value model

A restored document is built from the types JSON has and no others: `nil`,
`bool`, `float64`, `string`, `[]any` and `*Object`.

```go
restored, err := jsoncompress.Unmarshal(envelope)
```

`Object` is an ordered map, and it has to be. A Go map has no order, and
`encoding/json` writes one out in sorted order, so decoding into
`map[string]any` would shuffle every document that passes through — and this
format promises to give key order back exactly as it arrived.

```go
row := jsoncompress.NewObject().Set("id", 1.0).Set("name", "Ada")

row.Len()             // 2
row.Keys()            // []string{"id", "name"}
key, value := row.At(0)
value, present := row.Get("name")   // present says whether the key is there at
                                    // all, which nil alone cannot
```

`Object` implements `json.Marshaler` and `json.Unmarshaler`, so it drops into
code that already uses `encoding/json` — though `json.Marshal` will escape `<`,
`>` and `&` the way Go always does. `jsoncompress.FormatJSON` writes the bytes
this format is defined in terms of.

Input is more forgiving. Anything `encoding/json` can marshal can be compressed,
and is read the same way it would be marshalled — struct tags and all:

```go
jsoncompress.Marshal(rows)   // []MyStruct, []map[string]any, []*Object, ...
```

A map is written in sorted key order, because that is the only order a Go map
has. Reach for `*Object` when the order matters to you.

---

## API

Everything is in the one package.

### `Compress(value any, options ...Options) (*Object, error)`

Compress a document into an envelope. The result is an ordinary JSON-shaped
value, so it can be stored, posted or logged anywhere plain JSON goes. The input
is left untouched.

### `Decompress(envelope any) (any, error)`

Restore it. Returns a `*DecodeError` if the envelope is not readable, rather than
something nearly right. The envelope may be an `*Object` or a `map[string]any`
that `encoding/json` produced.

### `Marshal(value)` / `MarshalIndent(value, indent)` and `Unmarshal(data)`

The same pair at the byte level, and the pair to prefer: because `Unmarshal`
parses the text itself, ordinary object nodes keep the order they were written
in as well.

```go
data, err := jsoncompress.Marshal(orders)   // compress, then serialise
back, err := jsoncompress.Unmarshal(data)   // parse, then restore
```

### `DecompressIfNeeded(value)` and `IsCompressed(value)`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `Measure(value, options ...Options)` and `MeasureText(text, options ...Options)`

What compression would buy, before you commit to it — here on the 1,000-user
record set the benchmark builds:

```go
report, err := jsoncompress.Measure(users)
report.Minified        // 211180
report.Compressed      // 56136
report.Percent         // 73.4
report.GzipMinified    // 17110
report.GzipCompressed  // 13596
```

`Ratio` and `Percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. `MeasureText` takes the document as
it really came in, so `Original` reflects the bytes on the wire.

### `NewCodec(options ...Options) (*Codec, error)`

Bind options once — the natural unit for a transport layer or a storage adapter.
It holds nothing but its options and never changes them, so one instance built at
start-up is safe to share across goroutines:

```go
options := jsoncompress.DefaultOptions()
options.MinRows = 10

payloads, err := jsoncompress.NewCodec(options)   // once, at start-up

data, err := payloads.Marshal(payload)            // and everywhere after
back, err := payloads.Unmarshal(data)
```

### `Validate(text)` and `Locate(text)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```go
found := jsoncompress.Locate("{\n  \"a\": 1,\n  \"b\": oops\n}")
// Valid:    false
// Error:    expected a value, found "o"
// Position: 19,  Line: 3,  Column: 8
// Excerpt:  `  "b": oops`
```

`encoding/json` reports `invalid character 'o' looking for beginning of value`
and an offset, which is not enough to put a cursor anywhere, so this scans the
document itself against RFC 8259. It knows about single quotes, trailing commas,
`True`/`None`, leading zeros, unterminated strings and bad escapes. `Location`
has a `String` method, so a found problem is ready to log.

`Position` is a byte offset, so `text[found.Position:]` starts at the problem;
`Column` counts characters, so it means what it means in an editor even when the
line above holds an emoji.

### `Minify(text)` and `Beautify(text, indent)`

Whitespace only, nothing else touched. Both write line feeds whatever the
platform, so a document beautified on Windows matches the same document
beautified anywhere else.

### `ParseJSON(data)`, `FormatJSON(value)` and `FormatJSONIndent(value, indent)`

Ordinary JSON, no envelope — but written byte for byte as `JSON.stringify` would
write it, which `encoding/json` does not quite do (see [Numbers](#numbers-are-written-the-way-every-implementation-writes-them)
below).

### `Normalize(value)`

The conversion every entry point runs first: an arbitrary Go value read down to
the model, following `encoding/json` for tags, embedded structs, `omitempty`,
`json.Marshaler`, `encoding.TextMarshaler`, `[]byte` and nil containers, and
following the wire format for the rest.

### `ByteLength`, `GzipBytes`, `Gunzip`, `GzipSize`

UTF-8 byte counts — what actually goes over the wire — and gzip with a zeroed
timestamp, so the same input always measures the same.

### `Pack(records)` and `Unpack(packed)`

The columnar idea on its own, with no envelope and no catalogue:

```go
table, ok := jsoncompress.Pack([]any{
    jsoncompress.NewObject().Set("id", 1.0).Set("name", "Ada"),
    jsoncompress.NewObject().Set("id", 2.0).Set("name", "Bob"),
})
// {"fields": ["id", "name"], "data": [[1, "Ada"], [2, "Bob"]], "count": 2}
```

Useful when the other end is not Go, or when you want something a human can read
straight off a network panel. `PackAll` / `UnpackAll` do the same for a slice of
result sets, which is what a multi-statement query returns. Records may be
`*Object`s, maps or structs. The second return value says whether the value was
the right shape, so nothing is silently mangled.

---

## Options

```go
options := jsoncompress.DefaultOptions()
options.Strings = true          // de-duplicate repeated string values
options.Tables = true           // pack slices of like-shaped objects into tables
options.Constants = true        // hoist unchanging columns out of the rows
options.DictColumns = true      // encode low-cardinality string columns as indices
options.MinRows = 2             // smallest slice that may become a table
options.MaxHoleRatio = 0.3      // largest share of missing cells a table may carry
options.MinStringCount = 2      // times a string must occur before it can be catalogued
options.MinStringLength = 1     // shortest string that may be catalogued

envelope, err := jsoncompress.Compress(value, options)
```

Always start from `DefaultOptions()`. The zero `Options` would turn every
transformation off and set a nonsense `MinRows`, so it is refused rather than
quietly obeyed — every entry point validates, and says which field is wrong.

The two worth reaching for are `MinRows`, if you have many tiny slices and want
them left alone, and `MaxHoleRatio`, if your records are sparse and you would
rather pack them anyway.

---

## What survives the round trip

Everything JSON can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the slice is left as objects rather than quietly reordered.
- **A missing key versus a null one.** `{"a": 1}` and `{"a": 1, "b": null}` in
  the same slice stay different, and `Object.Get` tells them apart.
- **An empty object versus an empty array**, which is why objects are `*Object`
  here.
- **Any string**, including ones starting with a tilde, containing control
  characters, or made of astral-plane characters.
- **Keys that look like numbers**, which most catalogue schemes reorder.
- **A key literally named `__proto__`**, which is ordinary data here and a trap
  in some of the languages this format travels to.

Values JSON cannot carry are treated the way JSON treats them: non-finite numbers
become `null`, negative zero becomes zero, and a cycle is an error rather than a
hang. Everything else follows `encoding/json`: a `json.Marshaler` is asked for
its JSON, an `encoding.TextMarshaler` for its text, a `time.Time` becomes RFC
3339 text, a `[]byte` becomes base64, and a nil slice or map becomes `null`.

This is checked by 2,000 randomly generated documents on every test run, each
compared byte for byte against what it started as, plus every combination of the
options above, plus a corpus written by the JavaScript implementation, plus a
`go test -fuzz` target for the parser.

One bound is worth knowing: a document nested more than 10,000 levels deep is
refused rather than read, which is where `encoding/json` draws its own line.

### Numbers are written the way every implementation writes them

The one place this package deliberately parts company with `encoding/json` is
number formatting. Go writes `1e-05` where ECMAScript writes `0.00001`, and
`1e+21` only above a different threshold; the format follows the ECMAScript rule
— positional between 1e-7 and 1e21, exponent form outside it — because that is
what the wire format is defined against and what every document in the
conformance corpus was written with.

The digits themselves are identical: both find the shortest decimal that reads
back as the same double. Integers keep Go's exact 64-bit precision, signed and
unsigned, rather than being folded through a float, so a value too large for a
double survives here where it would not in JavaScript.

Strings are escaped the same way too, which means `<`, `>`, `&`, `/` and the
line separators U+2028 and U+2029 are written raw where `encoding/json` escapes
them. The document means the same thing either way; the bytes only match one of
them.

---

## Command line

```bash
go install github.com/eharain/JSON-Compress/go/cmd/json-compress@latest

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
| **Go** | [`github.com/eharain/JSON-Compress/go`](https://pkg.go.dev/github.com/eharain/JSON-Compress/go) | `go get github.com/eharain/JSON-Compress/go` | **this one** |
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
go test ./...
go test -race ./...
go run ./examples/benchmark
go test -bench . -benchmem
go test -fuzz FuzzRoundTrip -fuzztime 60s
go test -fuzz FuzzParseAgreesWithEncodingJson -fuzztime 60s
```

The second fuzz target is there because the JSON reader is written against the
grammar directly rather than over `encoding/json`'s token stream, which boxes
every string and number into an interface and costs an allocation for each. It
is worth about five times the throughput on a record set, and the target holds
it to the standard library's own answers: the same documents accepted, the same
refused, the same values read out of them.

The conformance corpus lives at
`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json` and is
shared with the .NET, Python and PHP suites rather than duplicated, so the Go
tests prove this implementation agrees with the reference without Node being
installed. To check the other direction as well:

```bash
node tests/interop/verify-interop.mjs    # every case through both implementations
```

The module lives in a subdirectory of a repository that holds five of them, so
its release tags carry that subdirectory: `go/v1.0.0`, not `v1.0.0`. That is
what the module proxy looks for, and `go get github.com/eharain/JSON-Compress/go@v1.0.0`
resolves to it.

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
