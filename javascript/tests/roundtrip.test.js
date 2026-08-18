/**
 * Round-trip tests.
 *
 * The contract the whole library rests on is that `decompress(compress(x))`
 * gives back `x` - not something equivalent, the same document, keys in the
 * same order. Everything here checks that, on shapes chosen to poke at the
 * places where the format could plausibly lose information.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { compress, decompress, stringify, parse } from '../src/index.js';

/** What `JSON` itself would make of a value - the standard the format meets. */
function asJson(value) {
  const text = JSON.stringify(value);
  return text === undefined ? undefined : JSON.parse(text);
}

function roundTrips(value, note) {
  const expected = asJson(value);
  const restored = decompress(compress(value));
  assert.deepEqual(restored, expected, note);
  assert.ok(
    JSON.stringify(restored) === JSON.stringify(expected),
    (note || 'value') + ': keys came back in a different order'
  );
}

test('scalars survive intact', () => {
  for (const value of [null, true, false, 0, -1, 1.5, 1e21, -1e-7, '', 'plain']) {
    roundTrips(value, JSON.stringify(value));
  }
});

test('the tilde escape is reversible', () => {
  for (const value of ['~', '~~', '~~~', '~0', '~z', '~ref', '~t', '~n', 'a~b']) {
    roundTrips(value, JSON.stringify(value));
  }
  roundTrips(['~', '~', '~', '~0', '~0', '~0'], 'repeated tilde strings');
  roundTrips({ '~key': '~value' }, 'tilde in both key and value');
});

test('empty containers survive', () => {
  roundTrips([], 'empty array');
  roundTrips({}, 'empty object');
  roundTrips([[], {}, [[]], [{}]], 'nested empties');
  roundTrips([{}, {}, {}], 'array of empty objects');
});

test('arrays of like-shaped objects become tables and come back', () => {
  roundTrips(
    [
      { id: 1, name: 'Ada', role: 'admin' },
      { id: 2, name: 'Bob', role: 'admin' },
      { id: 3, name: 'Cy', role: 'user' },
    ],
    'uniform rows'
  );
});

test('rows with different key sets survive', () => {
  roundTrips([{ a: 1 }, { b: 2 }, { a: 3, b: 4 }], 'disjoint keys');
  roundTrips([{ a: 1, b: 2, c: 3 }, { a: 4 }, { a: 5, c: 6 }], 'ragged rows');
});

test('a missing key is not the same as a null one', () => {
  const value = [
    { a: 1, b: null },
    { a: 2 },
    { a: 3, b: null },
    { a: 4, b: 5 },
  ];
  const restored = decompress(compress(value));
  assert.equal('b' in restored[0], true, 'explicit null kept');
  assert.equal(restored[0].b, null);
  assert.equal('b' in restored[1], false, 'absent key stays absent');
});

test('a missing key is not the same as a null one in a dictionary column', () => {
  const value = [
    { k: 'x', n: 1 },
    { k: null, n: 2 },
    { n: 3 },
    { k: 'x', n: 4 },
    { k: 'y', n: 5 },
    { k: 'x', n: 6 },
  ];
  const restored = decompress(compress(value));
  assert.equal(restored[1].k, null);
  assert.equal('k' in restored[1], true);
  assert.equal('k' in restored[2], false);
  assert.deepEqual(restored, value);
});

test('constant columns are restored on every row', () => {
  const value = [
    { type: 'order', id: 1 },
    { type: 'order', id: 2 },
    { type: 'order', id: 3 },
  ];
  const envelope = compress(value);
  assert.equal(JSON.stringify(envelope).includes('order'), true);
  assert.deepEqual(decompress(envelope), value);
});

test('a table where every column is constant keeps its row count', () => {
  const value = [
    { a: 1, b: 'x' },
    { a: 1, b: 'x' },
    { a: 1, b: 'x' },
    { a: 1, b: 'x' },
  ];
  assert.deepEqual(decompress(compress(value)), value);
});

test('nested tables survive', () => {
  roundTrips(
    {
      groups: [
        { name: 'a', items: [{ q: 1, p: 2 }, { q: 3, p: 4 }] },
        { name: 'b', items: [{ q: 5, p: 6 }, { q: 7, p: 8 }] },
      ],
    },
    'tables inside tables'
  );
});

test('arrays of arrays are left alone', () => {
  roundTrips([[1, 2], [3, 4], [5, 6]], 'matrix');
  roundTrips([[{ a: 1 }, { a: 2 }], [{ a: 3 }, { a: 4 }]], 'arrays of tables');
});

test('mixed arrays are left alone', () => {
  roundTrips([1, 'two', null, { three: 3 }, [4]], 'mixed');
  roundTrips([{ a: 1 }, 2, { a: 3 }], 'objects interrupted by a scalar');
});

test('deep nesting survives', () => {
  let value = { leaf: true };
  for (let i = 0; i < 200; i++) value = { ['level' + i]: value };
  roundTrips(value, '200 levels');
});

test('repeated strings are catalogued and restored', () => {
  const value = Array.from({ length: 50 }, (_, i) => ({
    id: i,
    country: i % 2 ? 'United Kingdom' : 'Republic of Ireland',
    note: 'a reasonably long repeated note value',
  }));
  const envelope = compress(value);
  assert.ok(envelope.s.includes('United Kingdom'), 'the repeated country was catalogued');
  assert.deepEqual(decompress(envelope), value);

  // The note never changes, so it is hoisted out of the rows entirely and
  // written down exactly once.
  const text = JSON.stringify(envelope);
  assert.equal(text.split('a reasonably long repeated note value').length - 1, 1);
});

test('unicode and escapes survive', () => {
  roundTrips(
    {
      emoji: 'compressed 📦 nicely',
      quote: 'she said "hello"',
      backslash: 'C:\\Users\\path',
      newline: 'one\ntwo\tthree',
      surrogate: '\u{1F600}\u{1F601}',
      arabic: 'مرحبا بالعالم',
      nul: 'a\u0000b',
    },
    'awkward text'
  );
});

test('keys that look like tokens are not confused with tokens', () => {
  roundTrips({ '0': 1, '1': 2, a: 3, z: 4, '10': 5, '~t': 6, '~r': 7 }, 'token-shaped keys');
});

test('a large key set pushes tokens past one character', () => {
  const value = {};
  for (let i = 0; i < 5000; i++) value['key_number_' + i] = i;
  roundTrips(value, '5000 keys');
});

test('a large string catalogue pushes references past one character', () => {
  const distinct = Array.from({ length: 500 }, (_, i) => 'a distinctive value number ' + i);
  const value = distinct.concat(distinct);
  roundTrips(value, '500 repeated strings');
});

test('stringify and parse agree with compress and decompress', () => {
  const value = { users: [{ id: 1, name: 'Ada' }, { id: 2, name: 'Bob' }] };
  assert.deepEqual(parse(stringify(value)), value);
  assert.equal(stringify(value), JSON.stringify(compress(value)));
});

test('every combination of options round-trips', () => {
  const value = {
    rows: Array.from({ length: 20 }, (_, i) => ({
      id: i,
      status: i % 2 ? 'open' : 'closed',
      fixed: 'always the same',
      note: i % 5 === 0 ? undefined : 'note ' + (i % 3),
    })),
  };
  const expected = asJson(value);

  for (const strings of [true, false]) {
    for (const tables of [true, false]) {
      for (const constants of [true, false]) {
        for (const dictColumns of [true, false]) {
          const options = { strings, tables, constants, dictColumns };
          const restored = decompress(compress(value, options));
          assert.deepEqual(restored, expected, JSON.stringify(options));
        }
      }
    }
  }
});

test('minRows controls when a table is built', () => {
  const two = [{ a: 1 }, { a: 2 }];
  assert.equal(JSON.stringify(compress(two, { minRows: 2 })).includes('~t'), true);
  assert.equal(JSON.stringify(compress(two, { minRows: 3 })).includes('~t'), false);
  assert.deepEqual(decompress(compress(two, { minRows: 3 })), two);
});

test('maxHoleRatio keeps very sparse arrays out of tables', () => {
  const sparse = [{ a: 1 }, { b: 2 }, { c: 3 }, { d: 4 }, { e: 5 }];
  assert.equal(JSON.stringify(compress(sparse)).includes('~t'), false);
  assert.deepEqual(decompress(compress(sparse)), sparse);
  assert.deepEqual(decompress(compress(sparse, { maxHoleRatio: 1 })), sparse);
});
