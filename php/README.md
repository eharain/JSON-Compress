# json-compress

**Lossless JSON compression that is still JSON.** No dependencies, no binary
framing, no extension to install. Typically 45–75% smaller, and an exact round
trip.

[![Packagist](https://img.shields.io/packagist/v/tech-style/json-compress.svg)](https://packagist.org/packages/tech-style/json-compress)
[![PHP version](https://img.shields.io/packagist/php-v/tech-style/json-compress.svg)](https://packagist.org/packages/tech-style/json-compress)
[![licence](https://img.shields.io/badge/licence-MIT-blue.svg)](https://github.com/eharain/JSON-Compress/blob/main/LICENSE)
[![six implementations](https://img.shields.io/badge/implementations-6_languages-a3e635.svg)](https://github.com/eharain/JSON-Compress#one-format-six-languages-the-same-bytes)

**One of six implementations of one wire format** — JavaScript, .NET, Python,
PHP, Go and Rust. Compress in any of them, restore in any other, byte for byte.
[How that is guaranteed ↓](#the-same-format-in-six-languages)

```bash
composer require tech-style/json-compress
```

```php
use TechStyle\JsonCompress\JsonCompress;

$users = [
    ['id' => 1, 'name' => 'Ada', 'role' => 'admin', 'country' => 'United Kingdom'],
    ['id' => 2, 'name' => 'Bob', 'role' => 'admin', 'country' => 'United Kingdom'],
    ['id' => 3, 'name' => 'Cy',  'role' => 'user',  'country' => 'United Kingdom'],
];

$small = JsonCompress::compress($users);
// {"jc": 1,
//  "k": ["id", "name", "role", "country"],
//  "d": {"~t": [0, 1, 2, 3],
//        "~e": [0, 0, 0, 2],
//        "~cv": ["United Kingdom"],
//        "~r": [[1, "Ada", "admin"], [2, "Bob", "admin"], [3, "Cy", "user"]]}}
// 'country' never changes, so it is stored once. Three rows is too few for the
// string catalogue to pay off on 'role' as well - at a thousand rows it would.

JsonCompress::decompress($small);   // exactly the records you started with
```

This is the PHP implementation of the
[json-compress format](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md).
A document written here reads correctly in the
[JavaScript](https://github.com/eharain/JSON-Compress/tree/main/javascript),
[.NET](https://github.com/eharain/JSON-Compress/tree/main/dotnet) and
[Python](https://github.com/eharain/JSON-Compress/tree/main/python)
implementations, and the other way round — which is not a claim, but a
[test](tests/ConformanceTest.php) run against a corpus the JavaScript encoder
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
| **Columnar tables** | A list of like-shaped objects becomes a column list and a rows matrix — the keys leave the rows entirely. |
| **Constant columns** | A column whose value never changes is lifted out of the rows and stored once. |
| **Dictionary columns** | A column drawn from a small set of strings becomes integer indices. |
| **String catalogue** | Any string value that repeats often enough to pay for itself is written once. |

The output is still ordinary JSON, so it goes anywhere JSON goes: an HTTP body,
a WebSocket frame, a MySQL `JSON` column, a queue message, a log line. Nothing
in your stack needs to learn a new format.

## What it measures

From `php examples/benchmark.php`, which round-trips every case before
reporting it:

| Scenario | Minified | Compressed | Saved | + gzip | Saved |
|---|---:|---:|---:|---:|---:|
| API record set (1,000 users) | 206.2 KB | 54.8 KB | **73%** | 13.4 KB | 20% |
| Nested API response (orders with lines) | 96.5 KB | 39.9 KB | **59%** | 9.5 KB | 6% |
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
all six encoders write the same bytes.

**Use it for record sets, logs, time series and API responses.** For a small
document you already gzip, it will not earn its keep — measure with `measure()`
before you commit.

## Where it helps most

- **API responses.** Less to serialise, less to send, less to parse.
- **Cache entries.** Redis and Memcached charge by the byte.
- **WebSocket traffic**, where per-message compression is often off.
- **Queue and job payloads** with per-message size limits.
- **JSON columns in a database.** MySQL `JSON`, Postgres `jsonb`, SQLite `TEXT`:
  smaller rows, smaller indexes, smaller backups.
- **Logs and telemetry**, which are almost entirely repeated keys.

---

## Install

```bash
composer require tech-style/json-compress
```

PHP 8.1 and later. No dependencies — `ext-json` is bundled with PHP and is all
it needs. `ext-zlib` is optional, and only the gzip helpers use it.

The command line comes with the package:

```bash
vendor/bin/json-compress stats orders.json
```

There is no Composer requirement either. The package is plain PSR-4 with nothing
to build, so a checkout works as it stands:

```php
require __DIR__ . '/json-compress/autoload.php';
```

---

## The value model

Objects are `stdClass` and arrays are PHP lists — what `json_decode($text)`
gives you when you do not ask for associative arrays.

That is not a stylistic choice. A PHP array cannot tell an object from a list,
and it turns a key of `"7"` into the integer `7`, so decoding into arrays would
lose both the empty-object/empty-array distinction and the original key order —
and this format promises to preserve both.

```php
JsonCompress::decompress($envelope);   // stdClass objects, list arrays
```

Input is more forgiving. An associative array is read as an object and a list as
an array, exactly as `json_encode` reads them, so the records your database
driver hands you go straight in:

```php
JsonCompress::compress($pdo->query('select * from orders')->fetchAll(PDO::FETCH_ASSOC));
```

An empty PHP array is ambiguous and is read as `[]`. Pass `new stdClass()` — or
`(object) []` — when you mean an empty object.

---

## API

Everything hangs off `TechStyle\JsonCompress\JsonCompress`.

### `compress(mixed $value, CompressOptions|array|null $options = null): stdClass`

Compress a document. The result is an ordinary JSON-shaped value, so it can be
handed to `json_encode`, stored, posted or logged anywhere plain JSON goes. The
input is left untouched.

### `decompress(mixed $envelope): mixed`

Restore it. Throws `DecodeError` if the envelope is not readable, rather than
returning something nearly right. `DecodeError` extends `RuntimeException`, so
it is caught by the handler you already have.

### `stringify($value, $options = null, $indent = null)` and `parse(string $text)`

The same pair at the string level.

```php
$text = JsonCompress::stringify($orders);   // compress, then serialise
$back = JsonCompress::parse($text);         // parse, then restore
```

### `decompressIfNeeded(mixed $value)` and `isCompressed(mixed $value)`

Restores a compressed value, passes anything else straight through. Useful at a
boundary you do not fully control — an endpoint mid-migration, a cache holding
entries written by two versions of the same service.

### `measure($value, $options = null, ?string $source = null, bool $gzip = true): SizeReport`

What compression would buy, before you commit to it — here on the 1,000-user
record set the benchmark builds:

```php
$report = JsonCompress::measure($users);
$report->minified;         // 211180
$report->compressed;       // 56136
$report->percent;          // 73.4
$report->gzipMinified;     // 17161
$report->gzipCompressed;   // 13742
```

`ratio` and `percent` are measured against **minified** JSON, not against
whatever whitespace the input arrived with. Pass `$source` if you want
`original` to reflect the real input, and `$gzip: false` to skip the gzip
measurements.

### `codec($options = null): Codec`

Bind options once — the natural unit for a transport layer or a storage adapter.
It holds nothing but its immutable options, so one instance can be built at boot
and shared by every request:

```php
final class Payloads
{
    private static ?Codec $codec = null;

    public static function codec(): Codec
    {
        return self::$codec ??= JsonCompress::codec(['minRows' => 10]);
    }
}

Payloads::codec()->stringify($payload);
Payloads::codec()->parse($text);
```

### `validate(string $text)` and `locate(string $text)`

Check a JSON document and say exactly where it breaks — line, column, the line
itself, and what is wrong in plain words:

```php
JsonCompress::locate("{\n  \"a\": 1,\n  \"b\": oops\n}");
// valid:    false
// error:    expected a value, found "o"
// position: 19,  line: 3,  column: 8
// excerpt:  '  "b": oops'
```

`json_decode` reports "Syntax error" and nothing else, so this scans the
document itself against RFC 8259. It knows about single quotes, trailing commas,
`True`/`None`/`NaN`, leading zeros, unterminated strings and bad escapes. A
`JsonLocation` casts to a one-line summary, so `(string) $found` is ready to log.

Offsets count characters rather than bytes, so a column number means what it
means in an editor even when the line above holds an emoji.

### `minify(string $text)` and `beautify(string $text, $indent = 2)`

Whitespace only, nothing else touched. Both write line feeds whatever the
platform, so a document beautified on Windows matches the same document
beautified anywhere else.

### `toJson(mixed $value, $indent = null)` and `parseJson(string $text)`

Ordinary JSON, no envelope — but written byte for byte as `JSON.stringify`
would write it, which `json_encode` does not quite do (see
[Numbers](#numbers-are-written-the-way-every-implementation-writes-them) below).

### `Sizes::byteLength`, `gzip`, `gunzip`, `gzipSize`, `canGzip`

UTF-8 byte counts — what actually goes over the wire — and gzip with a zeroed
timestamp, so the same input always measures the same. `gzipSize` returns `null`
on a build with no zlib rather than throwing, so a caller can present the figure
as unavailable.

### `TablePack::pack($records)` and `TablePack::unpack($packed, bool $assoc = false)`

The columnar idea on its own, with no envelope and no catalogue:

```php
TablePack::pack([['id' => 1, 'name' => 'Ada'], ['id' => 2, 'name' => 'Bob']]);
// {"fields": ["id", "name"], "data": [[1, "Ada"], [2, "Bob"]], "count": 2}
```

Useful when the other end is not PHP, or when you want something a human can
read straight off a network panel. `packAll` / `unpackAll` do the same for a
list of result sets, which is what a multi-statement query returns. Both leave
anything that is not the expected shape alone, so they can be dropped into a
pipeline without a shape check first. Records may be associative arrays or
objects; pass `$assoc: true` to `unpack` to get arrays back.

---

## Options

```php
use TechStyle\JsonCompress\CompressOptions;

JsonCompress::compress($value, new CompressOptions(
    strings: true,          // de-duplicate repeated string values
    tables: true,           // pack lists of like-shaped objects into tables
    constants: true,        // hoist unchanging columns out of the rows
    dictColumns: true,      // encode low-cardinality string columns as indices
    minRows: 2,             // smallest list that may become a table
    maxHoleRatio: 0.3,      // largest share of missing cells a table may carry
    minStringCount: 2,      // times a string must occur before it can be catalogued
    minStringLength: 1,     // shortest string that may be catalogued
));
```

Options may equally be a plain array, whichever reads better at the call site:

```php
JsonCompress::compress($value, ['tables' => false]);
JsonCompress::compress($value, ['minRows' => 10]);
```

Every default is safe on arbitrary input, and every field validates on
construction, so a nonsense value surfaces at the call site rather than as a
mysteriously large output. The two worth reaching for are `minRows`, if you have
many tiny lists and want them left alone, and `maxHoleRatio`, if your records
are sparse and you would rather pack them anyway.

An instance is immutable; `with()` makes a changed copy.

---

## What survives the round trip

Everything JSON can express, exactly:

- **Key order**, including in packed tables. Where one column order cannot
  satisfy every row, the list is left as objects rather than quietly reordered.
- **A missing key versus a null one.** `{"a": 1}` and `{"a": 1, "b": null}` in
  the same list stay different.
- **An empty object versus an empty array**, which is why objects are
  `stdClass` here.
- **Any string**, including ones starting with a tilde, containing control
  characters, or made of astral-plane characters.
- **Keys that look like numbers**, which most catalogue schemes reorder — and
  which PHP itself would turn into integers if the tokens were not letter-first.
- **A key literally named `__proto__`**, which is ordinary data here and a trap
  in some of the languages this format travels to.

Values JSON cannot carry are treated the way JSON treats them: non-finite
numbers become `null`, negative zero becomes zero, and a cycle throws rather
than hanging. Objects are converted the way `json_encode` converts them — a
`JsonSerializable` is asked for its JSON form, a `DateTimeInterface` becomes ISO
8601 text, a backed enum becomes its value and a pure enum its name, and
anything else contributes its public properties.

This is checked by 2,000 randomly generated documents on every test run, each
compared character for character against what it started as, plus every
combination of the options above, plus a corpus written by the JavaScript
implementation.

Two bounds are worth knowing. Compressing walks the document recursively and
stops at 1,024 levels, reporting the document as too deep rather than exhausting
the stack. That is twice the ceiling `json_decode` uses by default, so anything
PHP itself will parse can be compressed; and the reader here goes to 2,048,
because an envelope nests deeper than the document it carries — one level for
the envelope, and one more for every packed table, whose rows matrix is an array
of arrays where the source had an array of objects. `locate` is iterative and
has no such limit.

The other bound is that an unpaired UTF-16 surrogate cannot make it this far:
`json_decode` refuses one before the library ever sees it. The JavaScript and
.NET implementations carry them.

### Numbers are written the way every implementation writes them

The one place this package deliberately parts company with `json_encode` is
float formatting. PHP writes `1.0` for a whole float and `1.0e+21` beyond the
exponent threshold; the format follows the ECMAScript rule instead — `1`,
positional between 1e-7 and 1e21, and `1e+21` outside it — because that is what
the wire format is defined against and what every document in the conformance
corpus was written with.

The digits themselves are identical: both find the shortest decimal that reads
back as the same double. Integers keep PHP's exact 64-bit precision rather than
being folded through a float, so `9223372036854775807` survives here and comes
back as `9223372036854776000` from JavaScript. Past `PHP_INT_MAX` an integer
literal becomes a float, as it does in JavaScript, and loses the same digits;
Python and .NET keep those too. That is a difference between the languages
rather than between the implementations — each restores its own document
exactly, which is what the format promises.

The result is also independent of the `precision` and `serialize_precision`
settings in `php.ini`, which `json_encode` is not.

---

## Command line

```bash
vendor/bin/json-compress stats orders.json
vendor/bin/json-compress compress orders.json -o orders.jc.json
vendor/bin/json-compress decompress orders.jc.json --pretty
curl -s https://api.example.com/orders | vendor/bin/json-compress compress > orders.jc.json
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
| **PHP** | [`tech-style/json-compress`](https://packagist.org/packages/tech-style/json-compress) on Packagist | `composer require tech-style/json-compress` | **this one** |
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
composer install
composer test           # 154 tests, including the JavaScript corpus
composer benchmark
```

The conformance corpus lives at
`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json` and is
shared with the .NET and Python suites rather than duplicated, so the PHP tests
prove this implementation agrees with the reference without Node being
installed. To check the other direction as well:

```bash
node tests/interop/verify-interop.mjs    # every case through both implementations
```

---

## Licence

MIT. Free for commercial use.

Built by [Tech Style Ltd](https://tech-style.co/) — issues and pull requests
welcome at [github.com/eharain/JSON-Compress](https://github.com/eharain/JSON-Compress).
