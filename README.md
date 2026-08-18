# JSON-Compress

**Lossless JSON compression that is still JSON.**

One wire format, implemented across languages, so a document compressed by a
Node service reads correctly in a PHP worker, a Java batch job or a Python
notebook.

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
| PHP | Packagist | Planned |
| Java | Maven Central | Planned |
| Python | PyPI | Planned |

Every implementation targets the same wire format, defined in
[**SPEC.md**](SPEC.md). That document is complete enough to write a port from
without reading any of the existing code, and it carries the conformance rules a
port has to meet.

The two released implementations do more than target the same format: they write
the same bytes. A corpus of 143 documents — the awkward cases from the
specification, plus 120 generated from a seeded source both suites reproduce —
is compressed by each and restored by the other on every CI run. That is
stronger than the specification requires, and it is the sharpest evidence
available that the two are making the same decisions for the same reasons.

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
<script src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.1/javascript/dist/json-compress.min.js"></script>
<script>
  const small = jsonCompress.compress(myData);
  const back  = jsonCompress.decompress(small);
</script>
```

Or as an ES module, no bundler involved:

```html
<script type="module">
  import { compress, measure } from
    'https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.1/javascript/dist/index.mjs';
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
   It is 143 source documents paired with the envelope the reference encoder
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
