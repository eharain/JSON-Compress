/**
 * Randomised round-trip testing.
 *
 * Hand-written cases only cover the shapes somebody thought of. This generates
 * documents from a seeded pseudo-random source - so a failure is reproducible
 * from the seed printed with it - and insists that every one survives the round
 * trip byte for byte against what `JSON` itself would produce.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { compress, decompress } from '../src/index.js';

/** A small deterministic generator, so failures can be replayed. */
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
    // A record set: the shape the format is built for, with ragged rows and
    // low-cardinality columns thrown in on purpose.
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

test('2000 random documents round-trip byte for byte', () => {
  for (let seed = 1; seed <= 2000; seed++) {
    const random = createRandom(seed);
    const value = makeValue(random, 5);

    const expected = JSON.stringify(value);
    let actual;
    try {
      actual = JSON.stringify(decompress(compress(value)));
    } catch (error) {
      assert.fail('seed ' + seed + ' threw: ' + error.message + '\n' + expected);
    }

    if (actual !== expected) {
      assert.fail(
        'seed ' + seed + ' did not round-trip\n  expected ' + expected + '\n  actual   ' + actual
      );
    }
  }
});

test('random documents round-trip under every option combination', () => {
  const combinations = [];
  for (const strings of [true, false]) {
    for (const tables of [true, false]) {
      for (const constants of [true, false]) {
        for (const dictColumns of [true, false]) {
          combinations.push({ strings, tables, constants, dictColumns });
        }
      }
    }
  }

  for (let seed = 1; seed <= 120; seed++) {
    const value = makeValue(createRandom(seed * 7919), 4);
    const expected = JSON.stringify(value);
    for (const options of combinations) {
      const actual = JSON.stringify(decompress(compress(value, options)));
      if (actual !== expected) {
        assert.fail(
          'seed ' + seed + ' with ' + JSON.stringify(options) +
          ' did not round-trip\n  expected ' + expected + '\n  actual   ' + actual
        );
      }
    }
  }
});

test('compression never inflates a realistic record set', () => {
  for (let seed = 1; seed <= 200; seed++) {
    const random = createRandom(seed * 104729);
    const rows = 20 + Math.floor(random() * 40);
    const value = Array.from({ length: rows }, (_, i) => ({
      id: i,
      reference: 'REF-' + (10000 + i),
      status: pick(random, ['open', 'closed', 'pending']),
      region: pick(random, ['north', 'south', 'east', 'west']),
      amount: Math.round(random() * 100000) / 100,
      currency: 'GBP',
      createdAt: '2026-0' + (1 + Math.floor(random() * 9)) + '-15T09:30:00.000Z',
    }));

    const plain = JSON.stringify(value).length;
    const packed = JSON.stringify(compress(value)).length;
    assert.ok(
      packed < plain,
      'seed ' + seed + ': compressed ' + packed + ' bytes vs plain ' + plain
    );
  }
});
