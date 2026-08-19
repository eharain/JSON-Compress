# JSON-Compress

**Lossless JSON compression that is still JSON.**
Six languages. One wire format. The same bytes out of every one of them.

Compress a payload in a Node service and a Go worker restores it exactly. Or a
PHP cron job, a .NET batch, a Rust service, a Python notebook — any of them, in
any direction. The compressed form is a published specification rather than a
private arrangement between a library and itself, which for a compression format
is the unusual part.

[![npm](https://img.shields.io/npm/v/@tech-style/json-compress?label=npm)](https://www.npmjs.com/package/@tech-style/json-compress)
[![NuGet](https://img.shields.io/nuget/v/TechStyle.JsonCompress?label=NuGet)](https://www.nuget.org/packages/TechStyle.JsonCompress)
[![PyPI](https://img.shields.io/pypi/v/json-compress?label=PyPI)](https://pypi.org/project/json-compress/)
[![Packagist](https://img.shields.io/packagist/v/tech-style/json-compress?label=Packagist)](https://packagist.org/packages/tech-style/json-compress)
[![Go reference](https://pkg.go.dev/badge/github.com/eharain/JSON-Compress/go.svg)](https://pkg.go.dev/github.com/eharain/JSON-Compress/go)
[![crates.io](https://img.shields.io/crates/v/json-compress?label=crates.io)](https://crates.io/crates/json-compress)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)

```
{"id":1,"name":"Ada","role":"admin","country":"United Kingdom"},
{"id":2,"name":"Bob","role":"admin","country":"United Kingdom"},   206 KB
{"id":3,"name":"Cy", "role":"user", "country":"United Kingdom"}     ↓  73%
                                                                    55 KB
{"jc":1,"k":["id","name","role","country"],"s":["admin","user"],
 "d":{"~t":[0,1,2,3],"~e":[0,0,1,2],"~cv":["United Kingdom"],
      "~r":[[1,"Ada",0],[2,"Bob",0],[3,"Cy",1]]}}
```

JSON spends most of its bytes repeating itself. A thousand records is a thousand
copies of every key name, a thousand copies of the three values a `status`
column can take, a thousand copies of the one value a `currency` column ever
holds. This writes each of them down once — and the result is still ordinary
JSON, so it travels anywhere JSON travels.

---

## Implementations

| Language | Package | Status |
|---|---|---|
| **JavaScript / TypeScript** | [`@tech-style/json-compress`](https://www.npmjs.com/package/@tech-style/json-compress) on npm | Released — [`javascript/`](javascript/) |
| **.NET** | [`TechStyle.JsonCompress`](https://www.nuget.org/packages/TechStyle.JsonCompress) on NuGet | Released — [`dotnet/`](dotnet/) |
| **Python** | [`json-compress`](https://pypi.org/project/json-compress/) on PyPI | Released — [`python/`](python/) |
| **PHP** | [`tech-style/json-compress`](https://packagist.org/packages/tech-style/json-compress) on Packagist | Released — [`php/`](php/) |
| **Go** | [`github.com/eharain/JSON-Compress/go`](https://pkg.go.dev/github.com/eharain/JSON-Compress/go) | Released — [`go/`](go/) |
| **Rust** | [`json-compress`](https://crates.io/crates/json-compress) on crates.io | Released — [`rust/`](rust/) |
| Java | Maven Central | Planned |

Every implementation targets the same wire format, defined in
[**SPEC.md**](SPEC.md). That document is complete enough to write a port from
without reading any of the existing code, and it carries the conformance rules a
port has to meet.

The released implementations do more than target the same format: they write the
same bytes. A document compressed by any one of them is restored by any other —
that is the point of the whole exercise, and it is checked on every CI run
against a shared corpus. See [one format, six
languages](#one-format-six-languages-the-same-bytes) below.

---

## One format, six languages, the same bytes

Compression libraries are usually a single-language affair: whatever compressed
the document has to be present to read it back, and the compressed form is a
private matter between a library and itself. This one is a **format** first,
written down in [`SPEC.md`](SPEC.md) and implemented six times against it.

| restored by ↓ &nbsp; compressed by → | JS | .NET | Python | PHP | Go | Rust |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| **JavaScript / TypeScript** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **.NET** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Python** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **PHP** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Go** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Rust** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

All thirty-six cells, and it is stronger than it looks. The implementations do
not merely *interoperate* — given the same document they emit **the same bytes**.
Any one of them can be the encoder and any other the decoder, in any direction,
and no cell is a special case with a compatibility shim behind it.

Every implementation also ships the same command line, called `json-compress`,
so the guarantee is something you can watch happen:

```bash
# a Python service writes the payload
json-compress compress orders.json > orders.jc.json

# a Go worker, a PHP cron job, a .NET batch or a Rust service reads it back —
# same command, same bytes, whichever one you run
json-compress decompress orders.jc.json > restored.json

diff orders.json restored.json    # no output
```

### How that is kept true

A corpus of **144 documents** — the awkward cases from the specification, plus
120 generated from a seeded source every suite reproduces independently — is
checked in once at
[`dotnet/tests/.../conformance.json`](dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json),
paired with the envelope the reference encoder wrote for each.

Every implementation asserts **both directions** against that one shared file on
every CI run: it must restore all 144 envelopes to the original value, and it
must write those same 144 envelopes itself. The specification only asks for the
first. Ports here do the second as well, which is what turns "compatible" into
"identical".

The corpus is the awkward cases on purpose: the tilde escape, holes standing
against real nulls, `__proto__` as a key name, key names that look like tokens,
catalogues large enough to need two-character tokens, rows whose key orders
disagree, and a raw column whose last cell is `-1`. Every one of those has
broken an implementation at some point, and each is now a case with a name.

### Where the languages are allowed to differ

Honest small print, because it is better read here than discovered later. The
six agree on every document JavaScript could have written. They diverge only
where the languages' own number models diverge, on JSON text `JSON.stringify`
could not have produced in the first place:

- **.NET preserves a number's raw literal**, so `1E2`, `10.0` and
  `12345678901234567890` survive verbatim; JavaScript, Python and PHP each
  canonicalise through their own parser.
- **`1` and `1.0` for one key across rows** make JavaScript alone treat the
  column as constant, because JavaScript alone cannot tell the two apart.
- **Go's gzip figures** in a size report differ by about a percentage point,
  because `compress/flate` is not zlib. The compressed size itself matches to
  the byte.

Each implementation still restores its own documents exactly, which is all the
format promises. These are differences between languages, not defects in ports.

---

## How it works

Three transformations, applied together:

**A key catalogue.** Every distinct key name is written once, at the top of the
document, and replaced throughout by a one- or two-character token. A key that
appears ten thousand times costs its full name once.

**Columnar tables.** An array of like-shaped objects becomes a column list and a
matrix of rows, so the keys leave the rows entirely. Columns whose value never
changes are lifted out and stored once; columns drawn from a small set of
strings become integer indices.

**A string catalogue.** Any string value that repeats often enough to pay for
itself is written once and referenced by a short token.

None of it is lossy. `decompress(compress(x))` returns `x` — the same values,
and the same key order, including inside packed tables.

---

## What it saves

Measured by `javascript/examples/benchmark.js`, which round-trips every case
before reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.3 KB | 20% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.4 KB | 6% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 11.4 KB | 19% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.2 KB | 13% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 6.9 KB | 5% |
| Config document (small, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −22% |

The gzip columns are the ones that matter for anything served over HTTP, and
they are reported as measured — including the last row, where a small document
with little repetition ends up slightly larger than gzip alone would have made
it. This format is for record-shaped data. On a small config file it will not
earn its keep, and the tooling will tell you so before you commit.

---

## Try it

```bash
npm install @tech-style/json-compress
```

```js
import { compress, decompress, measure } from '@tech-style/json-compress';

const small = compress(records);
const back = decompress(small);
console.log(await measure(records));
```

### In a browser, with nothing installed

The built bundles are committed here, so jsDelivr serves them straight out of
this repository. One script tag and you have it:

```html
<script src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.2/javascript/dist/json-compress.min.js"></script>
<script>
  const small = jsonCompress.compress(myData);
  const back  = jsonCompress.decompress(small);
</script>
```

Or as an ES module, no bundler involved:

```html
<script type="module">
  import { compress, measure } from
    'https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.2/javascript/dist/index.mjs';
</script>
```

About 7 KB over the wire. Nothing in the library touches the network — see
[`javascript/browser.md`](javascript/browser.md) for version pinning,
subresource integrity and a complete copy-and-paste page.

### In .NET

```bash
dotnet add package TechStyle.JsonCompress
```

```csharp
using System.Text.Json.Nodes;
using TechStyle.JsonCompress;

var small = JsonCompressor.Compress(records);
var back  = JsonCompressor.Decompress(small);
var report = JsonCompressor.Measure(records);
```

No dependencies on .NET 8 and later; `netstandard2.0` is there for .NET
Framework. There is a command line too:

```bash
dotnet tool install --global TechStyle.JsonCompress.Cli
json-compress stats orders.json
```

### In Python

```bash
pip install json-compress
```

```python
from json_compress import compress, decompress, measure

small  = compress(records)
back   = decompress(small)
report = measure(records)
```

Python 3.9 and later, no dependencies — everything it needs is in the standard
library. The same command line comes with the package:

```bash
json-compress stats orders.json
```

### In PHP

```bash
composer require tech-style/json-compress
```

```php
use TechStyle\JsonCompress\JsonCompress;

$small  = JsonCompress::compress($records);
$back   = JsonCompress::decompress($small);
$report = JsonCompress::measure($records);
```

PHP 8.1 and later, no dependencies — `ext-json` is bundled with PHP and is all
it needs. Objects come back as `stdClass` and arrays as lists, so an empty
object stays distinct from an empty array; associative arrays go in unchanged,
so records from PDO need no conversion. The same command line comes with the
package:

```bash
vendor/bin/json-compress stats orders.json
```

### In Go

```bash
go get github.com/eharain/JSON-Compress/go
```

```go
import jsoncompress "github.com/eharain/JSON-Compress/go"

small, err := jsoncompress.Marshal(records)
back, err := jsoncompress.Unmarshal(small)
report, err := jsoncompress.Measure(records)
```

Go 1.21 and later, no dependencies — the standard library is all it needs.
Objects come back as an ordered `*Object` rather than a `map[string]any`,
because a Go map cannot give key order back; anything `encoding/json` can
marshal goes in unchanged, struct tags and all. The same command line comes with
the module:

```bash
go install github.com/eharain/JSON-Compress/go/cmd/json-compress@latest
json-compress stats orders.json
```

### In Rust

```bash
cargo add json-compress --features serde
```

```rust
use json_compress::{to_value, from_value, to_string, from_str};

let small = to_string(&to_value(&records)?)?;
let back: Vec<Record> = from_value(from_str(&small)?)?;
let report = json_compress::measure(&to_value(&records)?)?;
```

Rust 1.70 and later, and with both features off, no dependencies at all —
`serde` is opt-in and `gzip` is opt-in because the standard library has no
deflate. Objects come back as an ordered `Object` rather than a `HashMap`,
because a hash map cannot give key order back; with the `serde` feature, anything
that implements `Serialize` goes in and struct fields keep their declared order.
The same command line comes with the crate:

```bash
cargo install json-compress --features gzip
json-compress stats orders.json
```

### Or online, with nothing at all

**[tech-style.co/json-minifier.html](https://tech-style.co/json-minifier.html)**
— minify, beautify, validate and compress in the browser. That page loads the
same bundle from the same URL above, and does all its work on your machine.
Nothing is uploaded.

---

## Repository layout

```
SPEC.md         The wire format. The contract every implementation meets.
javascript/     The JavaScript and TypeScript implementation, published to npm.
dotnet/         The .NET implementation, published to NuGet.
python/         The Python implementation, published to PyPI.
php/            The PHP implementation, published to Packagist.
go/             The Go implementation, a module in its own right.
rust/           The Rust implementation, published to crates.io.
LICENSE         MIT, for everything here.
```

Each implementation directory is self-contained: its own build, its own tests,
its own README, its own release process.

---

## Contributing a port

1. Read [`SPEC.md`](SPEC.md) end to end. It is short.
2. Reproduce the cases in `javascript/tests/` — especially the tilde escape,
   holes against nulls, `__proto__` as a key name, token-shaped key names, and
   rows whose key orders disagree. Those are where ports go wrong.
3. Check your output against
   [`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json`](dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json).
   It is 144 source documents paired with the envelope the reference encoder
   wrote for each, as text, in a form any language can read. Restoring all of
   them is the bar; writing the same envelopes means your encoder is making the
   same choices as well.
4. Open a pull request with the implementation in its own top-level directory.

Issues and pull requests are welcome.

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — a UK software company. If
you are using this and it is helping, we would genuinely like to hear about it.
