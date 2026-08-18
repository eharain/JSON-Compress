/**
 * The standalone table pack, and the tokens the format is built on.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { pack, unpack, packAll, unpackAll, isPacked, textToArray } from '../src/index.js';
import { toBase62, fromBase62, base62Length, toKeyToken, fromKeyToken } from '../src/base62.js';
import { orderColumns } from '../src/column-order.js';

test('pack flattens records and unpack restores them', () => {
  const records = [
    { id: 1, name: 'Ada' },
    { id: 2, name: 'Bob' },
  ];
  const packed = pack(records);
  assert.deepEqual(packed, {
    fields: ['id', 'name'],
    data: [[1, 'Ada'], [2, 'Bob']],
    count: 2,
  });
  assert.deepEqual(unpack(packed), records);
});

test('pack fills a missing field with null', () => {
  const packed = pack([{ a: 1 }, { b: 2 }]);
  assert.deepEqual(packed.fields, ['a', 'b']);
  assert.deepEqual(packed.data, [[1, null], [null, 2]]);
});

test('pack leaves anything that is not a record set alone', () => {
  assert.equal(pack(null), null);
  assert.equal(pack(undefined), undefined);
  assert.equal(pack('text'), 'text');
  const mixed = [1, { a: 2 }];
  assert.equal(pack(mixed), mixed);
  assert.deepEqual(pack([]), { fields: [], data: [], count: 0 });
});

test('unpack leaves anything that is not a packed table alone', () => {
  assert.equal(unpack(null), null);
  assert.equal(unpack(42), 42);
  const notPacked = { fields: 'no', data: [] };
  assert.equal(unpack(notPacked), notPacked);
  assert.deepEqual(unpack({ fields: [], data: [], count: 0 }), []);
});

test('isPacked recognises the shape', () => {
  assert.equal(isPacked(pack([{ a: 1 }])), true);
  assert.equal(isPacked([{ a: 1 }]), false);
  assert.equal(isPacked(null), false);
});

test('packAll and unpackAll handle several result sets', () => {
  const sets = [
    [{ a: 1 }, { a: 2 }],
    [{ b: 3 }],
  ];
  const packed = packAll(sets);
  assert.equal(packed.length, 2);
  assert.equal(packed[0].count, 2);
  assert.deepEqual(unpackAll(packed), sets);
});

test('textToArray reads a bracketed integer column', () => {
  assert.deepEqual(textToArray({ ids: '[[[1,2,3]]]' }, 'ids').ids, [1, 2, 3]);
  assert.deepEqual(textToArray({ ids: null }, 'ids').ids, []);
  assert.deepEqual(textToArray({ ids: '{1|2}' }, 'ids', { open: 1, close: 1, separator: '|' }).ids, [1, 2]);
  assert.deepEqual(textToArray({ other: 1 }, 'ids'), { other: 1 });
});

test('base-62 tokens round-trip', () => {
  for (const n of [0, 1, 61, 62, 63, 3843, 3844, 238327, 238328, 1e6]) {
    assert.equal(fromBase62(toBase62(n)), n, 'index ' + n);
    assert.equal(base62Length(n), toBase62(n).length, 'length of ' + n);
  }
  assert.equal(fromBase62(''), -1);
  assert.equal(fromBase62('~'), -1);
  assert.equal(fromBase62('a-b'), -1);
});

test('key tokens round-trip and never look like array indices', () => {
  for (let n = 0; n < 20000; n++) {
    const token = toKeyToken(n);
    assert.equal(fromKeyToken(token), n, 'index ' + n);
    assert.equal(/^[A-Za-z]/.test(token), true, 'token ' + token + ' starts with a letter');
    assert.equal(String(Number(token)) === token, false, 'token ' + token + ' looks numeric');
  }
  assert.equal(fromKeyToken(''), -1);
  assert.equal(fromKeyToken('0'), -1);
  assert.equal(fromKeyToken('~'), -1);
});

test('key tokens are compact for the sizes that matter', () => {
  assert.equal(toKeyToken(0).length, 1);
  assert.equal(toKeyToken(51).length, 1);
  assert.equal(toKeyToken(52).length, 2);
  assert.equal(toKeyToken(3275).length, 2);
  assert.equal(toKeyToken(3276).length, 3);
});

test('column ordering respects every row', () => {
  assert.deepEqual(orderColumns([{ a: 1, b: 2 }, { a: 3, b: 4 }]), ['a', 'b']);
  assert.deepEqual(orderColumns([{ a: 1, b: 2 }, { b: 3, c: 4 }]), ['a', 'b', 'c']);

  // The rows disagree about where c goes, but an order exists that suits both.
  assert.deepEqual(orderColumns([{ a: 1, b: 2 }, { c: 3, a: 4 }]), ['c', 'a', 'b']);

  // Here they contradict each other outright.
  assert.equal(orderColumns([{ a: 1, b: 2 }, { b: 3, a: 4 }]), null);
});

test('an array whose rows disagree about key order stays an array of objects', () => {
  const value = [
    { a: 1, b: 2 },
    { b: 3, a: 4 },
  ];
  assert.equal(JSON.stringify(pack(value)).includes('fields'), true);
});
