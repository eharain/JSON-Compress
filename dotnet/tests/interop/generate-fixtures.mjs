/**
 * Build the conformance corpus.
 *
 * The corpus is produced by the JavaScript implementation and checked in, so
 * the .NET tests can prove the two agree without Node being installed. Every
 * case carries the source document and the envelope the JavaScript encoder
 * wrote for it, both as text, so a port has something exact to compare against
 * rather than something equivalent.
 *
 * Run from this directory:
 *
 *     node generate-fixtures.mjs
 *
 * The generator below is the one in javascript/tests/fuzz.test.js, kept
 * character for character so a seed means the same document in every language
 * that reproduces it.
 */

import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compress } from '../../../javascript/src/index.js';

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, '..', 'TechStyle.JsonCompress.Tests', 'fixtures', 'conformance.json');

/* ------------------------------------------------------------------ *
 * The shared generator
 * ------------------------------------------------------------------ */

function createRandom(seed) {
  let state = seed >>> 0;
  return function next() {
    state = (state * 1664525 + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

const WORDS = [
  'id', 'name', 'status', 'created', 'updated', 'owner', 'tags', 'value',
  'count', 'total', 'items', 'meta', 'type', 'code', 'label', 'notes',
  '~tilde', '0', '1', '10', 'a', 'z', 'A', '~t', '~r', 'toString',
  '__proto__', 'constructor', 'hasOwnProperty', '', ' ', 'ключ', '键',
];

const STRINGS = [
  '', 'a', 'active', 'pending', 'cancelled', '~', '~~', '~0', '~zz',
  'a longer value that repeats across many records',
  'quote " backslash \\ newline \n tab \t',
  '😀 emoji', 'مرحبا', 'null', 'true', '0',
];

function pick(random, list) {
  return list[Math.floor(random() * list.length)];
}

function makeScalar(random) {
  const roll = random();
  if (roll < 0.2) return null;
  if (roll < 0.3) return random() < 0.5;
  if (roll < 0.55) return Math.floor(random() * 2000) - 1000;
  if (roll < 0.62) return random() * 1e6 - 5e5;
  return pick(random, STRINGS);
}

function makeValue(random, depth) {
  if (depth <= 0) return makeScalar(random);

  const roll = random();

  if (roll < 0.45) return makeScalar(random);

  if (roll < 0.6) {
    const length = Math.floor(random() * 6);
    const out = new Array(length);
    for (let i = 0; i < length; i++) out[i] = makeValue(random, depth - 1);
    return out;
  }

  if (roll < 0.8) {
    const rows = Math.floor(random() * 8);
    const columns = [];
    const columnCount = 1 + Math.floor(random() * 5);
    for (let c = 0; c < columnCount; c++) columns.push(pick(random, WORDS));

    const out = new Array(rows);
    for (let i = 0; i < rows; i++) {
      const row = {};
      for (const column of columns) {
        if (random() < 0.15) continue;
        row[column] = random() < 0.5 ? pick(random, STRINGS) : makeValue(random, depth - 2);
      }
      out[i] = row;
    }
    return out;
  }

  const size = Math.floor(random() * 6);
  const out = {};
  for (let i = 0; i < size; i++) {
    out[pick(random, WORDS)] = makeValue(random, depth - 1);
  }
  return out;
}

/* ------------------------------------------------------------------ *
 * The named cases - the places ports go wrong
 * ------------------------------------------------------------------ */

const named = [
  ['spec worked example', [
    { id: 1, name: 'Ada', role: 'admin', team: 'core' },
    { id: 2, name: 'Bob', role: 'admin', team: 'core' },
    { id: 3, name: 'Cy', role: 'user', team: 'core' },
    { id: 4, name: 'Dee', role: 'user', team: 'core', note: 'on leave' },
  ]],
  ['spec key catalogue example', { user: { id: 7, name: 'Ada', tags: ['a', 'b'] } }],
  ['tilde escape', ['~', '~~', '~~~', '~0', '~z', '~ref', '~t', 'a~b']],
  ['tilde in key and value', { '~key': '~value', '~t': 1, '~r': 2 }],
  ['holes against nulls', [
    { a: 1, b: null }, { a: 2 }, { a: 3, b: null }, { a: 4, b: 5 },
  ]],
  ['holes against nulls in a dictionary column', [
    { k: 'x', n: 1 }, { k: null, n: 2 }, { n: 3 },
    { k: 'x', n: 4 }, { k: 'y', n: 5 }, { k: 'x', n: 6 },
  ]],
  ['every column constant', [
    { a: 1, b: 'x' }, { a: 1, b: 'x' }, { a: 1, b: 'x' }, { a: 1, b: 'x' },
  ]],
  ['proto as a key', JSON.parse('{"__proto__": 1, "safe": 2}')],
  ['proto as a column', JSON.parse('[{"__proto__":1,"a":2},{"__proto__":3,"a":4},{"__proto__":5,"a":6}]')],
  ['token-shaped keys', JSON.parse('{"0":1,"1":2,"a":3,"z":4,"10":5,"~t":6,"~r":7}')],
  ['rows whose key orders disagree', [{ a: 1, b: 2 }, { b: 3, a: 4 }]],
  ['rows whose key orders can be reconciled', [{ a: 1, b: 2 }, { c: 3, a: 4 }]],
  ['ragged rows', [{ a: 1, b: 2, c: 3 }, { a: 4 }, { a: 5, c: 6 }]],
  ['empty containers', [[], {}, [[]], [{}]]],
  ['array of empty objects', [{}, {}, {}]],
  ['nested tables', {
    groups: [
      { name: 'a', items: [{ q: 1, p: 2 }, { q: 3, p: 4 }] },
      { name: 'b', items: [{ q: 5, p: 6 }, { q: 7, p: 8 }] },
    ],
  }],
  ['arrays of arrays', [[1, 2], [3, 4], [5, 6]]],
  ['mixed array', [1, 'two', null, { three: 3 }, [4]]],
  ['unicode and escapes', {
    emoji: 'compressed 📦 nicely',
    quote: 'she said "hello"',
    backslash: 'C:\\Users\\path',
    newline: 'one\ntwo\tthree',
    surrogate: '\u{1F600}\u{1F601}',
    arabic: 'مرحبا بالعالم',
    nul: 'a\u0000b',
  }],
  ['a large key set', (() => {
    const value = {};
    for (let i = 0; i < 200; i++) value['key_number_' + i] = i;
    return value;
  })()],
  ['a large string catalogue', (() => {
    const distinct = Array.from({ length: 120 }, (_, i) => 'a distinctive value number ' + i);
    return distinct.concat(distinct);
  })()],
  ['a record set worth compressing', Array.from({ length: 60 }, (_, i) => ({
    id: i,
    reference: 'REF-' + (10000 + i),
    status: ['open', 'closed', 'pending'][i % 3],
    region: ['north', 'south', 'east', 'west'][i % 4],
    amount: (i * 137) / 100,
    currency: 'GBP',
    createdAt: '2026-08-18T09:30:00.000Z',
  }))],
  ['scalars', [null, true, false, 0, -1, 1.5, 1e21, -1e-7, '', 'plain']],
];

/* ------------------------------------------------------------------ *
 * Build it
 * ------------------------------------------------------------------ */

const cases = [];

for (const [name, value] of named) {
  cases.push({
    name,
    source: JSON.stringify(value),
    compressed: JSON.stringify(compress(value)),
  });
}

const SEEDS = 120;
for (let seed = 1; seed <= SEEDS; seed++) {
  const value = makeValue(createRandom(seed), 5);
  cases.push({
    name: 'seed ' + seed,
    seed,
    source: JSON.stringify(value),
    compressed: JSON.stringify(compress(value)),
  });
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(
  out,
  JSON.stringify(
    {
      note:
        'Produced by dotnet/tests/interop/generate-fixtures.mjs from the JavaScript implementation. ' +
        'Each case carries a source document and the envelope the JavaScript encoder wrote for it, both as text.',
      format: 1,
      cases,
    },
    null,
    2
  ) + '\n',
  'utf8'
);

process.stdout.write('wrote ' + cases.length + ' cases to ' + out + '\n');
