# TechStyle.JsonCompress

**Lossless JSON compression that is still JSON.** No dependencies on .NET 8 and
later, no binary framing, no code generation. Typically 45–75% smaller, and an
exact round trip.

## Video

[![JSON Compress: lossless JSON compression that is still JSON](https://raw.githubusercontent.com/eharain/JSON-Compress/main/docs/media/video-thumbnail.jpg)](https://www.youtube.com/watch?v=FlY6yl79UDo)

A 1:11 introduction on YouTube, with English captions.

```csharp
using System.Text.Json.Nodes;
using TechStyle.JsonCompress;

var users = JsonNode.Parse("""
    [{"id":1,"name":"Ada","role":"admin","country":"United Kingdom"},
     {"id":2,"name":"Bob","role":"admin","country":"United Kingdom"},
     {"id":3,"name":"Cy", "role":"user", "country":"United Kingdom"}]
    """);

var small = JsonCompressor.Compress(users);
// {"jc":1,"k":["id","name","role","country"],
//  "d":{"~t":[0,1,2,3],"~e":[0,0,0,2],"~cv":["United Kingdom"],
//       "~r":[[1,"Ada","admin"],[2,"Bob","admin"],[3,"Cy","user"]]}}

JsonCompressor.Decompress(small);   // exactly the array you started with
```

## What it does

JSON spends most of its bytes saying the same things over and over. A thousand
records is a thousand copies of every key name; a `status` column with three
possible values is a thousand copies of three short strings; a `currency` field
that is always `"GBP"` is a thousand copies of one. This writes each of those
down once.

| | |
|---|---|
| **Key catalogue** | Every key name is written once and referenced by a one- or two-character token. |
| **Columnar tables** | An array of like-shaped objects becomes a column list and a rows matrix. |
| **Constant columns** | A column whose value never changes is stored once. |
| **Dictionary columns** | A column drawn from a small set of strings becomes integer indices. |
| **String catalogue** | Any string value that repeats often enough to pay for itself is written once. |

The output is still ordinary JSON, so it goes anywhere JSON goes: an HTTP body,
a WebSocket frame, a `jsonb` column, a queue message, a log line.

## What it measures

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.9 KB | 16% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.8 KB | 3% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 12.4 KB | 13% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.8 KB | 9% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 7.2 KB | 3% |
| Config document (small, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −24% |

Use it for record sets, logs, time series and API responses. For a small
document you already gzip it will not earn its keep — `JsonCompressor.Measure`
will tell you so before you commit.

## The rest of the surface

- `Stringify` / `Parse` — the same pair at the string level.
- `CompressValue<T>` / `ParseValue<T>` — for data that is not already a `JsonNode`.
- `DecompressIfNeeded` / `IsCompressed` — for a boundary you do not fully control.
- `Measure` — what compression would buy, with and without gzip.
- `JsonCompressCodec` — options bound once, safe to share across threads.
- `Validate` / `Locate` — where a JSON document breaks, with line and column.
- `Minify` / `Beautify` — whitespace only.
- `TablePack` — the columnar idea on its own, with no envelope.

## One format, several languages

The wire format is specified in
[SPEC.md](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md), and a
document written here reads correctly in the
[JavaScript implementation](https://www.npmjs.com/package/@tech-style/json-compress)
— which is not a claim but a test, run against a corpus that encoder produced.

Full documentation:
[github.com/eharain/JSON-Compress/tree/main/dotnet](https://github.com/eharain/JSON-Compress/tree/main/dotnet)

## Licence

MIT. Built by [Tech Style Ltd](https://tech-style.co/).
