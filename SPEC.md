# The json-compress format, version 1

This document defines the wire format completely enough to write an
implementation from, in any language, without reading the JavaScript.

It is the contract between implementations. A document written by one must be
readable by all of them, byte for byte identical in what it restores.

---

## 1. What the format is

A json-compress document is **ordinary JSON**. There is no binary framing, no
custom parser, no base64. Anything that can carry JSON can carry this: an HTTP
body, a WebSocket frame, a `jsonb` column, `localStorage`, a log line.

The compression comes from three transformations, applied together:

| | Transformation | What it removes |
|---|---|---|
| 1 | **Key catalogue** | Every distinct key name is written once and referenced by a short token. |
| 2 | **Columnar tables** | An array of like-shaped objects becomes a column list plus a rows matrix, so keys vanish from the rows entirely. |
| 3 | **String catalogue** | String values that repeat are written once and referenced by a short token. |

Every transformation is reversible. `decompress(compress(x))` must return a
document identical to `x` — the same values, and the same key order.

---

## 2. The envelope

The top level is always a JSON object with these members:

```json
{
  "jc": 1,
  "k": ["id", "name", "status"],
  "s": ["active", "pending"],
  "d": { ... }
}
```

| Member | Type | Required | Meaning |
|---|---|---|---|
| `jc` | integer | yes | Format version. This document defines version `1`. |
| `k` | array of strings | yes | The key catalogue. May be empty. |
| `s` | array of strings | no | The string-value catalogue. Omitted when empty. |
| `d` | any | yes | The encoded document. May be `null`. |

A reader must reject a document whose `jc` it does not recognise, rather than
attempt a partial read. A reader must reject a document with no `k`.

Members other than these four should be ignored by a reader, so that a later
version can add one without breaking older readers on the parts they do
understand.

---

## 3. Tokens

Catalogue indices are written as short text tokens. Two alphabets are used,
because the two places a token appears have different constraints.

### 3.1 Base-62 tokens — for string references

```
digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
```

Index `n` is written in base 62, most significant digit first, with no padding.
Index `0` is `"0"`, index `61` is `"z"`, index `62` is `"10"`.

### 3.2 Key tokens — for object keys

A key token is **one letter followed by zero or more base-62 digits**:

```
letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"   (52)
digits  = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"   (62)
```

The numbering is bijective and organised in blocks by length:

| Length | Indices | Count |
|---|---|---|
| 1 | 0 – 51 | 52 |
| 2 | 52 – 3,275 | 52 × 62 = 3,224 |
| 3 | 3,276 – 203,163 | 52 × 62² |
| *n* | … | 52 × 62<sup>n-1</sup> |

To encode index `n`: find the block it falls in, subtract that block's starting
index, write the remainder as `length - 1` base-62 digits, and prefix the
letter for what remains.

To decode: read the first character as a letter value, fold the remaining
characters in as base-62 digits, then add the starting index of the block
implied by the token's length.

**Why keys cannot use base-62.** JavaScript objects — and the JSON parsers in
most languages that build a hash map — hold integer-like keys in a separate,
numerically ordered section. A key of `"10"` would sort ahead of a key of `"2"`,
silently reordering the document. Starting every key token with a letter makes
that impossible.

---

## 4. Reserved characters

Exactly one character is reserved: the tilde, `~` (U+007E).

- **As the first character of an object key**, it marks a structural node
  (section 6). No key token can begin with a tilde, so there is no ambiguity.
- **As the first character of a string value**, it marks either an escape or a
  reference (section 5).

Nothing else is reserved. Key names in the catalogue and string values in the
catalogue are stored verbatim, tildes and all.

---

## 5. Encoding values

Given the key catalogue `k` and the string catalogue `s`, a source value is
encoded as follows.

### 5.1 `null`, booleans, numbers

Written unchanged.

Numbers follow JSON's own rules: non-finite values (`NaN`, `Infinity`) become
`null`, and negative zero is written as `0`.

### 5.2 Strings

Let `v` be the source string.

| Condition | Encoded as |
|---|---|
| `v` begins with `~` | `"~" + v` (one extra tilde) |
| `v` is in the string catalogue at index `i` | `"~" + base62(i)` |
| otherwise | `v` unchanged |

The escape is tested **first**. That ordering is what keeps the three forms
disjoint:

- an escaped literal always begins `~~`;
- a reference always begins with `~` followed by a base-62 digit, never a tilde;
- a lone `"~"` is neither, and is reserved as the row hole marker (section 6.4).

A string beginning with `~` therefore never appears as a reference, and an
implementation should not place such strings in the catalogue for the sake of
inline references. It may still place them there for dictionary columns, whose
cells are integers and bypass this encoding entirely.

Decoding is the inverse: a string not beginning with `~` is itself; `~~…` loses
one tilde; `~` followed by a base-62 token is the catalogue entry at that index;
a lone `~` outside a table row is a corrupt document.

### 5.3 Arrays

An array is either **packed as a table** (section 6) or written as a JSON array
whose elements are each encoded by these rules.

Whether to pack is entirely the encoder's choice. A decoder must handle both.

### 5.4 Objects

An object is written as a JSON object in which every key is replaced by the key
token for its catalogue index, and every value is encoded by these rules.

**Key order is preserved.** The encoded object lists its members in the same
order as the source object.

Because every key becomes a key token, and key tokens start with a letter, an
encoded object node can never have a key starting with `~`. That is what makes
section 6 unambiguous.

---

## 6. Table nodes

A table node is a JSON object with a `~t` member. Its members are:

| Member | Type | Required | Meaning |
|---|---|---|---|
| `~t` | array of integers | yes | Columns, as indices into `k`. |
| `~e` | array of integers | no | Per-column encoding. Absent means every column is `0`. |
| `~cv` | array | no | Values of the constant columns, in column order. |
| `~r` | array of arrays | see below | The rows. |
| `~n` | integer | see below | Row count, when there are no varying columns. |

A reader identifies a table node by the presence of `~t`, and must check for it
before treating an object as an ordinary object node.

### 6.1 Column encodings

| Value | Name | Cells |
|---|---|---|
| `0` | raw | An encoded value (section 5), or the hole marker. |
| `1` | dictionary | An integer index into `s`, or `null`, or `-1` for a hole. |
| `2` | constant | No cell at all; the value is in `~cv`. |

`~e` must have exactly as many entries as `~t` when it is present.

### 6.2 Rows

`~r` holds one array per row. A row array carries a cell for each column whose
encoding is **not** `2`, in the order those columns appear in `~t`.

If every column is constant, `~r` is omitted and `~n` gives the row count.
Otherwise `~r` is present and the row count is `~r`'s length.

### 6.3 Constant columns

`~cv` holds one value per column whose encoding is `2`, in the order those
columns appear in `~t`. Each is an encoded value (section 5).

A constant column may only be used for a value that is `null`, a boolean, a
number or a string — never an object or array, because every restored row would
otherwise share one reference.

### 6.4 Holes

A hole means **the key is absent from that row**, which is not the same as the
key being present with a null value. Both must survive the round trip.

- In a raw column, a hole is the string `"~"`.
- In a dictionary column, a hole is the integer `-1`.

A row array may be **shorter** than the number of non-constant columns. Every
column past its end is a hole. Encoders should trim trailing holes; decoders
must accept both trimmed and untrimmed rows.

### 6.5 Restoring rows

For each row, walk `~t` in order and build the object:

```
constantAt = 0
cellAt     = 0

for each column c in ~t:
    encoding = ~e ? ~e[c] : 0

    if encoding == 2:
        set k[~t[c]] to decode(~cv[constantAt]); constantAt += 1
        continue

    cell = (cellAt < row.length) ? row[cellAt] : ABSENT ; cellAt += 1

    if encoding == 1:
        if cell is ABSENT or cell == -1: skip this key entirely
        if cell is null: set k[~t[c]] to null
        else:            set k[~t[c]] to s[cell]
    else:
        if cell is ABSENT or cell == "~": skip this key entirely
        else: set k[~t[c]] to decode(cell)
```

The keys therefore come out in `~t` order — which is why the encoder has to
choose that order carefully (section 7.2).

---

## 7. Encoder guidance

Nothing in this section changes what a decoder must do. Two encoders may make
different choices and both produce valid documents; only the restored value has
to agree.

### 7.1 Ordering the catalogues

Order `k` by descending frequency of use, so the most-used keys get
one-character tokens. Count a key **once per table** it heads a column in, and
once per occurrence in an ordinary object node — that reflects how often the
token is actually written.

Order `s` by descending frequency for the same reason.

### 7.2 Choosing the column order

A table has one column order, but every row has its own key order, and the
decoder restores keys in column order. The column order must therefore be one in
which **every row's keys appear in the same relative sequence they had in that
row**.

That is a topological sort. Take the union of the keys as the vertices; for each
row, add an edge from each key to the one that follows it. Any linear extension
of that graph restores every row faithfully. Break ties by first appearance, so
the result is deterministic.

If the graph has a cycle — one row says `a` before `b`, another says `b` before
`a` — no column order can satisfy both, and the array must not be packed as a
table. Encode it as an array of object nodes instead.

### 7.3 When to build a table

Reasonable defaults, all of which an implementation may expose:

- at least 2 rows;
- every element is an object, not an array and not a scalar;
- no more than 30% of the cells are holes;
- a consistent column order exists (7.2).

### 7.4 When to use a dictionary column

A dictionary column pays off when a column's values are drawn from a small set.
Reasonable defaults: every present value is a string, at least 3 of them, no
more than 60% of them distinct, and an average length of at least 2 characters.

Every distinct value of a dictionary column must be added to `s`.

### 7.5 When to catalogue a string

For inline references, compare the two costs directly:

```
inline    = count * (length + 2)
catalogue = count * (tokenLength + 3) + length + 3
```

Catalogue the string when `catalogue < inline`. Consider candidates in
descending frequency order so the token length used in that comparison is the
one the string will actually be given.

### 7.6 Values JSON cannot carry

Normalise before encoding, exactly as `JSON.stringify` would:

- honour a `toJSON` method where the language has one, so dates become strings;
- non-finite numbers become `null`;
- undefined-like values are dropped from objects and become `null` in arrays;
- a circular reference is an error, not a hang.

---

## 8. Decoder requirements

A decoder must:

- reject an envelope whose `jc` it does not implement;
- reject a key token or string reference that points outside its catalogue;
- reject a lone `"~"` outside a table row;
- reject a `~e` whose length does not match `~t`;
- treat a key literally named `__proto__` as data — in languages where plain
  assignment to that name mutates the prototype, define the property instead, as
  `JSON.parse` does;
- preserve key order as described in sections 5.4 and 6.5.

---

## 9. A worked example

Source:

```json
[
  { "id": 1, "name": "Ada",  "role": "admin", "team": "core" },
  { "id": 2, "name": "Bob",  "role": "admin", "team": "core" },
  { "id": 3, "name": "Cy",   "role": "user",  "team": "core" },
  { "id": 4, "name": "Dee",  "role": "user",  "team": "core", "note": "on leave" }
]
```

- `team` is present on every row with the same value, so it becomes a constant
  column.
- `role` is all strings, 4 of them, 2 distinct, so it becomes a dictionary
  column.
- `note` is present on one row of four, so its column has three holes — the last
  of which are trimmed from the ends of rows 1 to 3.

Compressed:

```json
{
  "jc": 1,
  "k": ["id", "name", "role", "team", "note"],
  "s": ["admin", "user"],
  "d": {
    "~t": [0, 1, 2, 3, 4],
    "~e": [0, 0, 1, 2, 0],
    "~cv": ["core"],
    "~r": [
      [1, "Ada", 0],
      [2, "Bob", 0],
      [3, "Cy", 1],
      [4, "Dee", 1, "on leave"]
    ]
  }
}
```

The rows carry cells for the four non-constant columns — `id`, `name`, `role`,
`note` — in that order. The first three rows have no `note`, so their fourth
cell is a hole and is trimmed away.

Reading row 3 back: column `id` is raw, cell `3`; column `name` is raw, cell
`"Cy"`; column `role` is a dictionary, cell `1`, so `s[1]` is `"user"`; column
`team` is constant, so its value is `~cv[0]`, `"core"`; column `note` is raw and
the row has no fourth cell, so the key is absent. That gives
`{"id":3,"name":"Cy","role":"user","team":"core"}` — the original row, keys in
the original order.

A smaller document, with no array to pack, shows the key catalogue on its own:

```json
{ "user": { "id": 7, "name": "Ada", "tags": ["a", "b"] } }
```

becomes

```json
{
  "jc": 1,
  "k": ["user", "id", "name", "tags"],
  "d": { "A": { "B": 7, "C": "Ada", "D": ["a", "b"] } }
}
```

Nothing repeats often enough to be worth cataloguing, so `s` is omitted
altogether.

---

## 10. Conformance

An implementation conforms when, for every JSON document `x`:

1. `decode(encode(x))` equals `x`, including key order;
2. `encode(x)` is valid JSON;
3. `decode(y)` for any `y` produced by a conforming encoder returns the same
   value that encoder was given;
4. an invalid document is rejected with an error rather than partially decoded.

The reference test corpus lives with the JavaScript implementation, in
`javascript/tests/`. Ports should reproduce its cases, in particular: the tilde
escape, holes against nulls, `__proto__` as a key name, token-shaped key names,
catalogues large enough to need multi-character tokens, and rows whose key
orders disagree.

A machine-readable form of the same corpus is checked in at
`dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json`: 144 cases,
each a source document and the envelope the reference encoder wrote for it, both
as JSON text. Restoring every one of them is what conformance means. Writing the
same envelopes is not required — clause 3 above asks only that the value agree —
but an encoder that does has demonstrated it makes the same choices as the
reference, and every released implementation does.
