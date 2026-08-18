/**
 * The public surface: the helpers around the codec, and the way it behaves at
 * the edges - bad input, values JSON itself cannot carry, documents from
 * another version of the format.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  compress,
  decompress,
  decompressIfNeeded,
  isCompressed,
  stringify,
  parse,
  minify,
  beautify,
  validate,
  locate,
  measure,
  createCodec,
  byteLength,
  canGzip,
  gzip,
  gunzip,
  gzipSize,
  defaultOptions,
  FORMAT_VERSION,
  DecodeError,
} from '../src/index.js';
import bundled from '../src/index.js';

test('the envelope carries a version and a key catalogue', () => {
  const envelope = compress({ hello: 'world' });
  assert.equal(envelope.jc, FORMAT_VERSION);
  assert.deepEqual(envelope.k, ['hello']);
  assert.equal(isCompressed(envelope), true);
});

test('the string catalogue is left out when nothing is worth cataloguing', () => {
  assert.equal('s' in compress({ a: 1 }), false);
});

test('isCompressed rejects everything that is not an envelope', () => {
  for (const value of [null, undefined, 0, 'x', [], {}, { jc: 2, k: [] }, { jc: 1 }]) {
    assert.equal(isCompressed(value), false, JSON.stringify(value) + ' should not look compressed');
  }
});

test('decompressIfNeeded passes ordinary values straight through', () => {
  const plain = { a: 1 };
  assert.equal(decompressIfNeeded(plain), plain);
  assert.deepEqual(decompressIfNeeded(compress(plain)), plain);
});

test('decompress refuses documents it cannot read', () => {
  assert.throws(() => decompress(null), DecodeError);
  assert.throws(() => decompress([1, 2]), DecodeError);
  assert.throws(() => decompress({ k: [] }), /not a compressed document/);
  assert.throws(() => decompress({ jc: 99, k: [] }), /version 99 is not supported/);
  assert.throws(() => decompress({ jc: 1 }), /key catalogue/);
  assert.throws(() => decompress({ jc: 1, k: [], s: 'nope' }), /string catalogue/);
});

test('decompress refuses references that point nowhere', () => {
  assert.throws(() => decompress({ jc: 1, k: [], d: { A: 1 } }), /key token/);
  assert.throws(() => decompress({ jc: 1, k: ['a'], s: [], d: '~5' }), /string reference/);
  assert.throws(() => decompress({ jc: 1, k: ['a'], d: '~' }), /hole marker/);
  assert.throws(() => decompress({ jc: 1, k: ['a'], d: { '~t': 'x' } }), /column list/);
  assert.throws(() => decompress({ jc: 1, k: ['a'], d: { '~t': [7] } }), /column index/);
});

test('circular structures are refused, not hung on', () => {
  const value = { name: 'loop' };
  value.self = value;
  assert.throws(() => compress(value), /circular/);
});

test('a shared reference is not mistaken for a cycle', () => {
  const shared = { a: 1 };
  assert.deepEqual(decompress(compress({ x: shared, y: shared })), { x: { a: 1 }, y: { a: 1 } });
});

test('BigInt is refused with a clear message', () => {
  assert.throws(() => compress({ big: 1n }), /BigInt/);
});

test('values JSON cannot carry are treated the way JSON treats them', () => {
  const restored = decompress(
    compress({
      when: new Date('2026-08-18T10:20:30.000Z'),
      gone: undefined,
      fn() {},
      sym: Symbol('x'),
      nan: NaN,
      inf: Infinity,
      list: [undefined, NaN, 1],
    })
  );
  assert.deepEqual(restored, {
    when: '2026-08-18T10:20:30.000Z',
    nan: null,
    inf: null,
    list: [null, null, 1],
  });
});

test('toJSON is honoured', () => {
  class Money {
    constructor(amount) {
      this.amount = amount;
    }
    toJSON() {
      return { pence: Math.round(this.amount * 100) };
    }
  }
  assert.deepEqual(decompress(compress({ price: new Money(12.5) })), { price: { pence: 1250 } });
});

test('a key called __proto__ survives as a real key', () => {
  const value = JSON.parse('{"__proto__": 1, "safe": 2}');
  const restored = decompress(compress(value));
  assert.equal(Object.keys(restored).includes('__proto__'), true);
  assert.equal(Object.getPrototypeOf(restored), Object.prototype);
  assert.equal(JSON.stringify(restored), '{"__proto__":1,"safe":2}');
});

test('a __proto__ column in a table survives too', () => {
  const value = JSON.parse('[{"__proto__":1,"a":2},{"__proto__":3,"a":4},{"__proto__":5,"a":6}]');
  assert.equal(JSON.stringify(decompress(compress(value))), JSON.stringify(value));
});

test('options are validated', () => {
  assert.throws(() => compress({}, 'nope'), TypeError);
  assert.throws(() => compress({}, { minRows: 0 }), RangeError);
  assert.throws(() => compress({}, { maxHoleRatio: 2 }), RangeError);
  assert.throws(() => compress({}, { minStringCount: 1 }), RangeError);
});

test('the defaults are frozen', () => {
  assert.equal(Object.isFrozen(defaultOptions), true);
  assert.equal(defaultOptions.tables, true);
});

test('stringify and parse are the string-level pair', () => {
  const value = { a: [1, 2, 3] };
  const text = stringify(value);
  assert.equal(typeof text, 'string');
  assert.deepEqual(parse(text), value);
});

test('minify and beautify only touch whitespace', () => {
  const text = '{\n  "a" : 1,\n  "b" : [1, 2]\n}';
  assert.equal(minify(text), '{"a":1,"b":[1,2]}');
  assert.equal(beautify('{"a":1}'), '{\n  "a": 1\n}');
  assert.equal(beautify('{"a":1}', 4), '{\n    "a": 1\n}');
  assert.equal(beautify('{"a":1}', '\t'), '{\n\t"a": 1\n}');
});

test('validate reports where a document goes wrong', () => {
  assert.deepEqual(validate('{"a":1}'), { valid: true });

  const bad = validate('{\n  "a": 1,\n  "b": oops\n}');
  assert.equal(bad.valid, false);
  assert.equal(bad.line, 3, 'the bad token is on line 3');
  assert.equal(bad.column, 8, 'and starts at column 8');
  assert.equal(bad.excerpt, '  "b": oops');
});

test('locate names the common mistakes', () => {
  const cases = [
    ["{'a': 1}", /double quotes/, 1, 2],
    ['{"a": 1,}', /trailing comma/, 1, 8],
    ['[1, 2,]', /trailing comma/, 1, 6],
    ['{"a": True}', /lower case/, 1, 7],
    ['{"a": None}', /use null/, 1, 7],
    ['{"a": 01}', /leading zero/, 1, 7],
    ['{"a": 1.}', /decimal point/, 1, 8],
    ['{"a": 1e}', /exponent/, 1, 8],
    ['{"a": "unclosed', /never closed/, 1, 7],
    ['{"a": 1', /never closed/, 1, 8],
    ['[1, 2', /never closed/, 1, 6],
    ['{a: 1}', /quoted property name/, 1, 2],
    ['{"a" 1}', /colon/, 1, 6],
    ['{"a": 1} extra', /after the end/, 1, 10],
    ['', /ends here/, 1, 1],
    ['{"a": "\\q"}', /not a valid escape/, 1, 8],
    ['{"a": "\\u12"}', /four hexadecimal/, 1, 8],
  ];

  for (const [text, pattern, line, column] of cases) {
    const found = locate(text);
    assert.equal(found.valid, false, JSON.stringify(text) + ' should be invalid');
    assert.match(found.error, pattern, JSON.stringify(text));
    assert.equal(found.line, line, JSON.stringify(text) + ' line');
    assert.equal(found.column, column, JSON.stringify(text) + ' column');
  }
});

test('locate agrees with JSON.parse about what is valid', () => {
  const documents = [
    '{}', '[]', 'null', 'true', 'false', '0', '-0', '1e10', '-1.5E-3', '"x"',
    '  {  "a" : [ 1 , 2 ] }  ', '{"a":{"b":{"c":[]}}}', '"\\u00e9\\n\\t\\\\"',
    '{"": 1}', '[[[[[1]]]]]', '{"a": 1e+10}', '"😀"',
  ];
  for (const text of documents) {
    assert.equal(locate(text).valid, true, JSON.stringify(text) + ' should be valid');
    JSON.parse(text);
  }
});

test('locate finds the same line on a multi-line document', () => {
  const text = ['{', '  "one": 1,', '  "two": [', '    1,', '    2,', '  ]', '}'].join('\n');
  const found = locate(text);
  assert.equal(found.valid, false);
  assert.match(found.error, /trailing comma/);
  assert.equal(found.line, 5);
});

test('measure reports a real saving on a record set', async () => {
  const value = Array.from({ length: 100 }, (_, i) => ({
    id: i,
    firstName: 'Given' + i,
    lastName: 'Family',
    department: i % 2 ? 'engineering' : 'operations',
  }));

  const report = await measure(value);
  assert.equal(report.minified, byteLength(JSON.stringify(value)));
  assert.equal(report.compressed < report.minified, true);
  assert.equal(report.saved, report.minified - report.compressed);
  assert.equal(report.percent > 40, true, 'expected better than 40%, got ' + report.percent);
  assert.equal(report.ratio < 1, true);
});

test('measure can be told what the input really weighed', async () => {
  const value = { a: 1 };
  const report = await measure(value, { source: '{\n  "a": 1\n}' });
  assert.equal(report.original, 12);
  assert.equal(report.minified, 7);
});

test('measure reports gzip sizes where the runtime can gzip', async () => {
  const report = await measure({ a: [1, 2, 3] });
  if (canGzip()) {
    assert.equal(typeof report.gzipMinified, 'number');
    assert.equal(typeof report.gzipCompressed, 'number');
  } else {
    assert.equal(report.gzipMinified, null);
  }
});

test('gzip round-trips', async (t) => {
  if (!canGzip()) return t.skip('no CompressionStream on this runtime');
  const text = JSON.stringify({ hello: 'world', list: [1, 2, 3] });
  const bytes = await gzip(text);
  assert.equal(bytes instanceof Uint8Array, true);
  assert.equal(await gunzip(bytes), text);
  assert.equal(await gzipSize(text), bytes.length);
});

test('byteLength counts UTF-8 bytes, not code units', () => {
  assert.equal(byteLength('abc'), 3);
  assert.equal(byteLength('£'), 2);
  assert.equal(byteLength('😀'), 4);
});

test('a codec binds its options once', () => {
  const codec = createCodec({ tables: false });
  const value = [{ a: 1 }, { a: 2 }, { a: 3 }];
  assert.equal(JSON.stringify(codec.compress(value)).includes('~t'), false);
  assert.deepEqual(codec.decompress(codec.compress(value)), value);
  assert.deepEqual(codec.parse(codec.stringify(value)), value);
  assert.deepEqual(codec.options, { tables: false });
});

test('the default export carries the whole surface', () => {
  for (const name of [
    'compress', 'decompress', 'decompressIfNeeded', 'isCompressed',
    'stringify', 'parse', 'minify', 'beautify', 'validate', 'measure',
    'createCodec', 'pack', 'unpack', 'packAll', 'unpackAll', 'isPacked', 'textToArray',
  ]) {
    assert.equal(typeof bundled[name], 'function', name + ' is missing from the default export');
  }
});
