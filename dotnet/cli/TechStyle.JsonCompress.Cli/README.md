# json-compress command line

Compress, restore, measure, minify, beautify and validate JSON documents from
the shell. The compressed form is still ordinary JSON, and the round trip is
exact.

```bash
dotnet tool install --global TechStyle.JsonCompress.Cli
```

```bash
json-compress stats orders.json
json-compress compress orders.json -o orders.jc.json
json-compress decompress orders.jc.json --pretty
json-compress validate orders.json

curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
```

`stats` changes nothing and reports what compression would save, with and
without gzip:

```
orders.json
  as given            206.2 KB
  minified            206.2 KB
  compressed           54.8 KB
  saved               151.4 KB   73.4% smaller than minified

  minified + gzip      16.5 KB
  compressed + gzip    13.9 KB
  saved                 2.7 KB   16.0% smaller than minified + gzip
```

`validate` says exactly where a document stops being JSON — line, column and
what is wrong in plain words — and exits non-zero, so it works in a build:

```
json-compress: invalid JSON at line 3, column 8: expected a value, found "o"
```

With no input file the document is read from standard input, and with no `--out`
the result goes to standard output, always as UTF-8 whatever the console code
page happens to be.

## Options

```
  -o, --out <file>    Write to a file instead of standard output
  -p, --pretty        Indent the output
  -i, --indent <n>    Indent width for --pretty and beautify (default 2)
      --no-strings    Do not build a string-value catalogue
      --no-tables     Do not pack arrays of objects into tables
      --no-constants  Do not hoist unchanging columns out of rows
      --no-dict-columns   Do not encode low-cardinality string columns as indices
      --min-rows <n>  Smallest array that may become a table (default 2)
  -h, --help          Show the usage text
  -v, --version       Show the version
```

The library behind it is
[TechStyle.JsonCompress](https://www.nuget.org/packages/TechStyle.JsonCompress),
and the wire format is specified in
[SPEC.md](https://github.com/eharain/JSON-Compress/blob/main/SPEC.md), so a
document compressed here reads correctly from JavaScript too.

## Licence

MIT. Built by [Tech Style Ltd](https://tech-style.co/).
