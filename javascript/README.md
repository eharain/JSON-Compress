# @tech-style/json-compress

**Lossless JSON compression that is still JSON.** No dependencies, no binary
framing, no build step required. Typically 45–75% smaller, and a byte-perfect
round trip.

[![npm](https://img.shields.io/npm/v/@tech-style/json-compress.svg)](https://www.npmjs.com/package/@tech-style/json-compress)
[![licence](https://img.shields.io/npm/l/@tech-style/json-compress.svg)](https://github.com/eharain/JSON-Compress/blob/main/LICENSE)
[![no dependencies](https://img.shields.io/badge/dependencies-0-brightgreen.svg)](package.json)
[![six implementations](https://img.shields.io/badge/implementations-6_languages-a3e635.svg)](https://github.com/eharain/JSON-Compress#one-format-six-languages-the-same-bytes)

**One of six implementations of one wire format** — JavaScript, .NET, Python,
PHP, Go and Rust. Compress in any of them, restore in any other, byte for byte.
[How that is guaranteed ↓](#the-same-format-in-six-languages)

```bash
npm install @tech-style/json-compress
```

```js
import { compress, decompress } from '@tech-style/json-compress';

const users = [
  { id: 1, name: 'Ada', role: 'admin', country: 'United Kingdom' },
  { id: 2, name: 'Bob', role: 'admin', country: 'United Kingdom' },
  { id: 3, name: 'Cy',  role: 'user',  country: 'United Kingdom' },
];

const small = compress(users);
// { jc: 1,
//   k: ['id', 'name', 'role', 'country'],
//   d: { '~t': [0, 1, 2, 3],
//        '~e': [0, 0, 0, 2],
//        '~cv': ['United Kingdom'],
//        '~r': [[1, 'Ada', 'admin'], [2, 'Bob', 'admin'], [3, 'Cy', 'user']] } }
// 'country' never changes, so it is stored once. Three rows is too few for the
// key catalogue to pay off on 'role' as well - at a thousand rows it would.

decompress(small);   // exactly the array you started with
```

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
a WebSocket frame, a `jsonb` column, `localStorage`, a queue message, a log
line. Nothing in your stack needs to learn a new format.

## What it measures

From `npm run bench`, which round-trips every case before reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.3 KB | 20% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.4 KB | 6% |
| Time series (2,000 readings) | 166.7 KB | 44.3 KB | **73%** | 11.4 KB | 19% |
| Structured logs (1,500 lines) | 237.3 KB | 57.3 KB | **76%** | 21.2 KB | 13% |
| GeoJSON feature collection (400 points) | 58.9 KB | 31.6 KB | **46%** | 6.9 KB | 5% |
| Config document (small, few repeats) | 3.1 KB | 2.2 KB | **29%** | 0.5 KB | −22% |

The last two columns are the honest ones. Most JSON meets gzip on the way out,
and gzip already removes a lot of this redundancy — so the question is what is
left. On record-shaped data, a further 13–20% on top of gzip. On a small config
file with little repetition, the envelope costs more than it saves, and the
table says so.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with `measure()`
before you commit.

## Where it helps most

- **API responses.** Less to serialise, less to send, less to parse.
- **`localStorage` and `IndexedDB`.** Browser storage quotas are small, and this
  is a straight multiplier on how much fits.
- **WebSocket and SSE traffic**, where per-message gzip is often not on.
- **Queue messages and job payloads** with per-message size limits.
- **JSON columns in a database.** Postgres `jsonb`, MySQL `json`: smaller rows,
  smaller indexes, smaller backups.
- **Logs and telemetry**, which are almost entirely repeated keys.

---

## Install

```bash
npm install @tech-style/json-compress
```

ESM, CommonJS and a browser global are all published:

```js
import { compress, decompress } from '@tech-style/json-compress';        // ESM
const { compress, decompress } = require('@tech-style/json-compress');   // CommonJS
```

### Or nothing installed at all

The built bundles are committed to the repository, so jsDelivr serves them
straight from GitHub. Paste this into a page and you are done:

```html
<script src="https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.2/javascript/dist/json-compress.min.js"></script>
<script>
  const small = jsonCompress.compress(myData);
  const back  = jsonCompress.decompress(small);
</script>
```

Or as an ES module, with no global and no bundler:

```html
<script type="module">
  import { compress, measure } from
    'https://cdn.jsdelivr.net/gh/eharain/JSON-Compress@1.0.2/javascript/dist/index.mjs';
</script>
```

The same files are on the npm CDNs:

```html
<script src="https://unpkg.com/@tech-style/json-compress/dist/json-compress.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/@tech-style/json-compress/dist/json-compress.min.js"></script>
```

The browser build is about 21 KB minified, 7 KB over the wire, and defines one
global — `jsonCompress` — carrying the whole API. Nothing in the library touches
the network. See [browser.md](https://github.com/eharain/JSON-Compress/blob/main/javascript/browser.md) for version pinning, subresource
integrity and a complete copy-and-paste page.

---

## API

### `compress(value, options?) → envelope`

Compress any JSON-serialisable value. Returns a plain object you can
`JSON.stringify`, store, or post.

### `decompress(envelope) → value`

Restore it. Throws a `DecodeError` if the envelope is not readable, rather than
returning something nearly right.

### `stringify(value, options?) → string` and `parse(text) → value`

The same pair at the string level — `stringify` is `JSON.stringify(compress(…))`
and `parse` is `decompress(JSON.parse(…))`.

### `decompressIfNeeded(value) → value`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `isCompressed(value) → boolean`

True when the value looks like an envelope this build can read.

### `measure(value, options?) → Promise<report>`

What compression would buy, before you commit to it:

```js
const report = await measure(users);
// {
//   original: 211145, minified: 211145, compressed: 51386,
//   saved: 159759, ratio: 0.243, percent: 75.7,
//   gzipMinified: 15850, gzipCompressed: 12316
// }
```

`ratio` and `percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. Pass `{ source: originalText }` if
you want `original` to reflect the real input, and `{ gzip: false }` to skip the
gzip measurements.

### `createCodec(options?) → codec`

Bind options once — the natural unit for a transport layer or a storage adapter:

```js
const codec = createCodec({ minRows: 10 });
codec.stringify(payload);
codec.parse(text);
```

### `validate(text)` and `locate(text)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```js
locate('{\n  "a": 1,\n  "b": oops\n}');
// { valid: false,
//   error: 'expected a value, found "o"',
//   position: 19, line: 3, column: 8, excerpt: '  "b": oops' }
```

Engine error messages disagree about what they include and often carry no
position at all, so this scans the document itself against RFC 8259. It knows
about single quotes, trailing commas, `True`/`None`, leading zeros, unterminated
strings and bad escapes.

### `minify(text)` and `beautify(text, indent?)`

Whitespace only, nothing else touched.

### `gzip(text)`, `gunzip(bytes)`, `gzipSize(text)`, `canGzip()`

Thin wrappers over the platform `CompressionStream`, present in Node 18+, Deno,
Bun and every current browser. No dependency, no polyfill.

### `pack(records)` and `unpack(packed)`

The columnar idea on its own, with no envelope and no catalogue:

```js
pack([{ id: 1, name: 'Ada' }, { id: 2, name: 'Bob' }]);
// { fields: ['id', 'name'], data: [[1, 'Ada'], [2, 'Bob']], count: 2 }
```

Useful when the other end is not JavaScript, or when you want something a human
can read straight off a network panel. `packAll` / `unpackAll` do the same for a
list of result sets, which is what multi-statement database drivers return.

Both leave anything that is not the expected shape completely alone, so they can
be dropped into a pipeline without a shape check first.

---

## Options

```js
compress(value, {
  strings: true,        // de-duplicate repeated string values
  tables: true,         // pack arrays of like-shaped objects into tables
  constants: true,      // hoist unchanging columns out of the rows
  dictColumns: true,    // encode low-cardinality string columns as indices
  minRows: 2,           // smallest array that may become a table
  maxHoleRatio: 0.3,    // largest share of missing cells a table may carry
  minStringCount: 2,    // times a string must occur before it can be catalogued
  minStringLength: 1,   // shortest string that may be catalogued
});
```

Every default is safe on arbitrary input. The two worth reaching for are
`minRows`, if you have many tiny arrays and want them left alone, and
`maxHoleRatio`, if your records are sparse and you would rather pack them
anyway.

---

## What survives the round trip

Everything `JSON.stringify` can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the array is left as objects rather than quietly reordered.
- **A missing key versus a null one.** `{a: 1}` and `{a: 1, b: null}` in the same
  array stay different.
- **Any string**, including ones starting with a tilde, containing control
  characters, or made of astral-plane characters.
- **A key literally named `__proto__`**, which plain assignment would otherwise
  swallow.
- **Keys that look like numbers**, which most catalogue schemes reorder.

Values JSON cannot carry are treated the way JSON treats them: `toJSON()` is
honoured so Dates become ISO strings, non-finite numbers become `null`,
`undefined` is dropped from objects and becomes `null` in arrays. A circular
reference throws rather than hanging. A `BigInt` throws with a message that says
so.

This is checked by 2,000 randomly generated documents on every test run, each
compared byte for byte against what `JSON` itself produces, plus every
combination of the options above.

---

## Command line

```bash
npx @tech-style/json-compress stats orders.json
npx @tech-style/json-compress compress orders.json -o orders.min.json
npx @tech-style/json-compress decompress orders.min.json --pretty
curl -s https://api.example.com/orders | npx @tech-style/json-compress compress > orders.jc.json
```

`stats` changes nothing and reports what compression would save, with and
without gzip. `validate`, `minify` and `beautify` are there too. Run
`json-compress --help` for the full list. Once the package is installed, the
binary is called `json-compress` without the scope.

---

## The format

The wire format is fully specified in [`SPEC.md`](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md) — enough to write
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

## The same format, in six languages

This is one of six implementations of the same wire format. They do not merely
interoperate: given the same document they emit **the same bytes**. A payload
compressed by any one of them is restored exactly by any other, in any
direction.

| Language | Package | Install | |
|---|---|---|---|
| **JavaScript / TypeScript** | [`@tech-style/json-compress`](https://www.npmjs.com/package/@tech-style/json-compress) on npm | `npm install @tech-style/json-compress` | **this one** |
| .NET | [`TechStyle.JsonCompress`](https://www.nuget.org/packages/TechStyle.JsonCompress) on NuGet | `dotnet add package TechStyle.JsonCompress` | |
| Python | [`json-compress`](https://pypi.org/project/json-compress/) on PyPI | `pip install json-compress` | |
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

## Compatibility

Node 18+, Deno, Bun, and every browser with `Object.entries` — which in practice
is everything since 2017. The gzip helpers additionally need
`CompressionStream`; `canGzip()` tells you, and `measure()` degrades to `null`
figures rather than throwing.

TypeScript definitions ship with the package. No `@types` install.

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
