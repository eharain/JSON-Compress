# TechStyle.JsonCompress

**Lossless JSON compression that is still JSON.** No dependencies on .NET 8 and
later, no binary framing, no code generation. Typically 45–75% smaller, and an
exact round trip.

[![NuGet](https://img.shields.io/nuget/v/TechStyle.JsonCompress.svg)](https://www.nuget.org/packages/TechStyle.JsonCompress)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](../LICENSE)

```bash
dotnet add package TechStyle.JsonCompress
```

```csharp
using System.Text.Json.Nodes;
using TechStyle.JsonCompress;

var users = JsonNode.Parse("""
    [{"id":1,"name":"Ada","role":"admin","country":"United Kingdom"},
     {"id":2,"name":"Bob","role":"admin","country":"United Kingdom"},
     {"id":3,"name":"Cy", "role":"user", "country":"United Kingdom"}]
    """);

var small = JsonCompressor.Compress(users);
// {"jc":1,
//  "k":["id","name","role","country"],
//  "d":{"~t":[0,1,2,3],
//       "~e":[0,0,0,2],
//       "~cv":["United Kingdom"],
//       "~r":[[1,"Ada","admin"],[2,"Bob","admin"],[3,"Cy","user"]]}}
// 'country' never changes, so it is stored once. Three rows is too few for the
// key catalogue to pay off on 'role' as well - at a thousand rows it would.

JsonCompressor.Decompress(small);   // exactly the array you started with
```

This is the .NET implementation of the
[json-compress format](../SPEC.md). A document written here reads correctly in
the [JavaScript](../javascript/) and [Python](../python/) implementations, and
the other way round — which is not a claim, but a
[test](tests/TechStyle.JsonCompress.Tests/ConformanceTests.cs) run against a
corpus the JavaScript encoder produced.

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
a WebSocket frame, a `jsonb` column, a queue message, a log line. Nothing in
your stack needs to learn a new format.

## What it measures

From `dotnet run --project examples/Benchmark -c Release`, which round-trips
every case before reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.9 KB | 16% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.8 KB | 3% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 12.4 KB | 13% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.8 KB | 9% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 7.2 KB | 3% |
| Config document (deeply nested, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −24% |

The last two columns are the honest ones. Most JSON meets gzip on the way out,
and gzip already removes a lot of this redundancy — so the question is what is
left. On record-shaped data, a further 9–16% on top of gzip. On a small config
file with little repetition, the envelope costs more than it saves, and the
table says so.

The first four columns match the JavaScript benchmark figure for figure, because
both build the same documents from the same generator and both encoders write
the same bytes. The gzip columns are a little different: `GZipStream` and the
browser `CompressionStream` do not tune deflate identically.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with
`JsonCompressor.Measure` before you commit.

## Where it helps most

- **API responses.** Less to serialise, less to send, less to parse.
- **Distributed cache entries.** Redis and friends charge by the byte.
- **WebSocket and SignalR traffic**, where per-message compression is often off.
- **Queue and Service Bus messages** with per-message size limits.
- **JSON columns in a database.** Postgres `jsonb`, SQL Server `nvarchar(max)`:
  smaller rows, smaller indexes, smaller backups.
- **Logs and telemetry**, which are almost entirely repeated keys.

---

## Install

```bash
dotnet add package TechStyle.JsonCompress
```

| Target | Dependencies |
|---|---|
| `net8.0`, `net10.0` | none — `System.Text.Json` is in the shared framework |
| `netstandard2.0` | `System.Text.Json`, for .NET Framework 4.6.2+ and Unity |

The command line is a separate package, so a service never carries it:

```bash
dotnet tool install --global TechStyle.JsonCompress.Cli
```

---

## API

Everything lives on `JsonCompressor`. The methods that take a `JsonNode` are the
format itself and use no reflection; the ones ending in `Value` are a
convenience over `System.Text.Json` for data that starts out as ordinary
objects.

### `Compress(JsonNode?, CompressOptions?) → JsonObject`

Compress a document. The result is a new tree that shares nothing with the
input, so it can be attached anywhere and the input is left untouched.

### `Decompress(JsonNode?) → JsonNode?`

Restore it. Throws `JsonCompressException` if the envelope is not readable,
rather than returning something nearly right.

### `Stringify(value, options?, indented?)` and `Parse(text)`

The same pair at the string level.

### `CompressValue<T>`, `DecompressValue<T>`, `StringifyValue<T>`, `ParseValue<T>`

For data that is not already a `JsonNode`:

```csharp
var text = JsonCompressor.StringifyValue(orders);            // List<Order> in
var back = JsonCompressor.ParseValue<List<Order>>(text);     // List<Order> out
```

The value is serialised to a JSON tree first, so what gets compressed is exactly
what `JsonSerializer` would have written. Pass your own `JsonSerializerOptions`
if you have naming policies or converters to apply.

### `DecompressIfNeeded(value)` and `IsCompressed(value)`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `Measure(value, options?, source?, gzip?) → SizeReport`

What compression would buy, before you commit to it:

```csharp
var report = JsonCompressor.Measure(users);
// Original 211145, Minified 211145, Compressed 51386,
// Saved 159759, Ratio 0.243, Percent 75.7,
// GzipMinified 15850, GzipCompressed 12316
```

`Ratio` and `Percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. Pass `source:` if you want `Original`
to reflect the real input, and `gzip: false` to skip the gzip measurements.

### `new JsonCompressCodec(options?)`

Bind options once — the natural unit for a transport layer or a storage adapter.
It copies the options it is given and holds nothing else, so one instance is
safe to share across threads:

```csharp
static readonly JsonCompressCodec Codec = new(new CompressOptions { MinRows = 10 });

Codec.Stringify(payload);
Codec.Parse(text);
```

### `Validate(text)` and `Locate(text)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```csharp
JsonCompressor.Locate("{\n  \"a\": 1,\n  \"b\": oops\n}");
// Valid false, Error "expected a value, found \"o\"",
// Position 19, Line 3, Column 8, Excerpt "  \"b\": oops"
```

Parser messages disagree about what they include and often carry no position at
all, so this scans the document itself against RFC 8259. It knows about single
quotes, trailing commas, `True`/`None`, leading zeros, unterminated strings and
bad escapes.

### `Minify(text)` and `Beautify(text, indent)`

Whitespace only, nothing else touched. Both write line feeds whatever the
platform, so a document beautified on Windows matches the same document
beautified anywhere else.

### `Bytes.ByteLength`, `Bytes.GZip`, `Bytes.GUnzip`, `Bytes.GZipSize`

UTF-8 byte counts — what actually goes over the wire — and gzip over
`GZipStream`.

### `TablePack.Pack(records)` and `TablePack.Unpack(packed)`

The columnar idea on its own, with no envelope and no catalogue:

```csharp
TablePack.Pack(JsonNode.Parse("""[{"id":1,"name":"Ada"},{"id":2,"name":"Bob"}]"""));
// {"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}
```

Useful when the other end is not .NET, or when you want something a human can
read straight off a network panel. `PackAll` / `UnpackAll` do the same for a list
of result sets, which is what multi-statement database drivers return. Both
leave anything that is not the expected shape alone, so they can be dropped into
a pipeline without a shape check first.

---

## Options

```csharp
JsonCompressor.Compress(value, new CompressOptions
{
    Strings = true,            // de-duplicate repeated string values
    Tables = true,             // pack arrays of like-shaped objects into tables
    Constants = true,          // hoist unchanging columns out of the rows
    DictionaryColumns = true,  // encode low-cardinality string columns as indices
    MinRows = 2,               // smallest array that may become a table
    MaxHoleRatio = 0.3,        // largest share of missing cells a table may carry
    MinStringCount = 2,        // times a string must occur before it can be catalogued
    MinStringLength = 1,       // shortest string that may be catalogued
});
```

Every default is safe on arbitrary input, and every property validates on
assignment, so a nonsense value surfaces at the call site rather than as a
mysteriously large output. The two worth reaching for are `MinRows`, if you have
many tiny arrays and want them left alone, and `MaxHoleRatio`, if your records
are sparse and you would rather pack them anyway.

---

## What survives the round trip

Everything JSON can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the array is left as objects rather than quietly reordered.
- **A missing key versus a null one.** `{"a":1}` and `{"a":1,"b":null}` in the
  same array stay different.
- **Any string**, including ones starting with a tilde, containing control
  characters, or made of astral-plane characters.
- **Keys that look like numbers**, which most catalogue schemes reorder — and
  which would be silently reordered by a JavaScript reader if the tokens were
  not letter-first.
- **A key literally named `__proto__`**, which is ordinary data here and a trap
  in some of the languages this format travels to.

Values JSON cannot carry are treated the way JSON treats them: non-finite
numbers become `null`, negative zero becomes zero, and `DateTime` becomes the
string `JsonSerializer` would have written. A cycle throws rather than hanging.

This is checked by 2,000 randomly generated documents on every test run, each
compared character for character against what it started as, plus every
combination of the options above, plus a corpus written by the JavaScript
implementation.

---

## Command line

```bash
dotnet tool install --global TechStyle.JsonCompress.Cli

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

The wire format is fully specified in [`SPEC.md`](../SPEC.md) — enough to write
an implementation from, in any language, without reading this one. In outline:

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

## A note on escaping

Output is written with an encoder that escapes exactly what JSON reserves — the
quote, the backslash, control characters and lone surrogates — and nothing else.
The stock .NET encoders escape more: even the relaxed one writes every character
above the Basic Multilingual Plane as a pair of six-character escapes, turning a
four-byte emoji into twelve. In a library whose purpose is to make documents
smaller, and whose output has to match what other implementations of this format
write, that will not do.

The consequence is the same one that applies to `JSON.stringify`: the output is
JSON for a JSON reader, not text to paste straight into an HTML page or a script
tag. Escape it at the point of embedding, as you would any other JSON.

---

## Building and testing

```bash
dotnet test                                       # 492 tests, including the JavaScript corpus
dotnet run --project examples/Benchmark -c Release
dotnet pack src/TechStyle.JsonCompress -c Release
```

The conformance corpus in `tests/TechStyle.JsonCompress.Tests/fixtures/` is
produced by the JavaScript implementation and checked in, so the .NET tests
prove the two agree without Node being installed. To rebuild it, or to check the
other direction as well:

```bash
node tests/interop/generate-fixtures.mjs   # rewrite the corpus from the JavaScript encoder
node tests/interop/verify-interop.mjs      # run every case through both implementations
```

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
