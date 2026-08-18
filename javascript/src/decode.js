/**
 * The decoder.
 *
 * A straight structural walk of the encoded document, resolving catalogue
 * references as it goes. It is deliberately strict: anything that cannot be
 * mapped back onto the original - an out-of-range token, a stray hole marker,
 * a table whose rows do not line up with its columns - throws rather than
 * quietly handing back something that is nearly right.
 */

import {
  FORMAT_VERSION,
  ENV_VERSION,
  ENV_KEYS,
  ENV_STRINGS,
  ENV_DATA,
  T_COLUMNS,
  T_ENCODINGS,
  T_CONSTS,
  T_ROWS,
  T_ROWCOUNT,
  DICT_HOLE,
  COL_DICT,
  COL_CONST,
  TILDE,
} from './constants.js';
import { fromBase62, fromKeyToken } from './base62.js';

const hasOwn = Object.prototype.hasOwnProperty;

/**
 * Set a property whose name may be `__proto__`.
 *
 * Plain assignment to that name replaces an object's prototype instead of
 * adding a property, so a document containing it would come back a key short.
 * `JSON.parse` defines the property instead, and so does this.
 */
function setKey(target, key, value) {
  if (key === '__proto__') {
    Object.defineProperty(target, key, {
      value,
      writable: true,
      enumerable: true,
      configurable: true,
    });
    return;
  }
  target[key] = value;
}

class DecodeError extends Error {
  constructor(message) {
    super('json-compress: ' + message);
    this.name = 'DecodeError';
  }
}

function decodeString(value, ctx) {
  if (value.charCodeAt(0) !== TILDE) return value;
  if (value.length === 1) {
    throw new DecodeError('a row hole marker appeared outside a table row');
  }
  if (value.charCodeAt(1) === TILDE) return value.slice(1);

  const at = fromBase62(value.slice(1));
  if (at < 0 || at >= ctx.strings.length) {
    throw new DecodeError('string reference "' + value + '" is out of range');
  }
  return ctx.strings[at];
}

function decodeKey(token, ctx) {
  const at = fromKeyToken(token);
  if (at < 0 || at >= ctx.keys.length) {
    throw new DecodeError('key token "' + token + '" is out of range');
  }
  return ctx.keys[at];
}

function decodeValue(node, ctx) {
  if (node === null) return null;
  const type = typeof node;
  if (type === 'string') return decodeString(node, ctx);
  if (type !== 'object') return node;

  if (Array.isArray(node)) {
    const out = new Array(node.length);
    for (let i = 0; i < node.length; i++) out[i] = decodeValue(node[i], ctx);
    return out;
  }

  if (hasOwn.call(node, T_COLUMNS)) return decodeTable(node, ctx);

  const out = {};
  const tokens = Object.keys(node);
  for (let i = 0; i < tokens.length; i++) {
    setKey(out, decodeKey(tokens[i], ctx), decodeValue(node[tokens[i]], ctx));
  }
  return out;
}

function decodeTable(node, ctx) {
  const columnIndices = node[T_COLUMNS];
  if (!Array.isArray(columnIndices)) {
    throw new DecodeError('table column list is not an array');
  }

  const columnCount = columnIndices.length;
  const columns = new Array(columnCount);
  for (let c = 0; c < columnCount; c++) {
    const at = columnIndices[c];
    if (!Number.isInteger(at) || at < 0 || at >= ctx.keys.length) {
      throw new DecodeError('table column index ' + at + ' is out of range');
    }
    columns[c] = ctx.keys[at];
  }

  const encodings = hasOwn.call(node, T_ENCODINGS) ? node[T_ENCODINGS] : null;
  if (encodings !== null && (!Array.isArray(encodings) || encodings.length !== columnCount)) {
    throw new DecodeError('table encoding list does not match its columns');
  }

  const rawConstants = hasOwn.call(node, T_CONSTS) ? node[T_CONSTS] : [];
  if (!Array.isArray(rawConstants)) {
    throw new DecodeError('table constant list is not an array');
  }
  const constants = new Array(rawConstants.length);
  for (let i = 0; i < rawConstants.length; i++) {
    constants[i] = decodeValue(rawConstants[i], ctx);
  }

  const matrix = hasOwn.call(node, T_ROWS) ? node[T_ROWS] : null;
  if (matrix !== null && !Array.isArray(matrix)) {
    throw new DecodeError('table rows are not an array');
  }

  const rowCount = matrix !== null ? matrix.length : (node[T_ROWCOUNT] | 0);
  const out = new Array(rowCount);

  for (let i = 0; i < rowCount; i++) {
    const cells = matrix !== null ? matrix[i] : null;
    if (cells !== null && !Array.isArray(cells)) {
      throw new DecodeError('table row ' + i + ' is not an array');
    }

    const row = {};
    let constantAt = 0;
    let cellAt = 0;

    for (let c = 0; c < columnCount; c++) {
      const encoding = encodings === null ? 0 : encodings[c];

      if (encoding === COL_CONST) {
        if (constantAt >= constants.length) {
          throw new DecodeError('table has fewer constants than constant columns');
        }
        setKey(row, columns[c], constants[constantAt++]);
        continue;
      }

      // A row shorter than the column list is missing everything past its end.
      const cell = cells !== null && cellAt < cells.length ? cells[cellAt] : undefined;
      cellAt++;

      if (encoding === COL_DICT) {
        if (cell === undefined || cell === DICT_HOLE) continue;
        if (cell === null) {
          setKey(row, columns[c], null);
          continue;
        }
        if (!Number.isInteger(cell) || cell < 0 || cell >= ctx.strings.length) {
          throw new DecodeError('dictionary index ' + cell + ' is out of range');
        }
        setKey(row, columns[c], ctx.strings[cell]);
        continue;
      }

      if (cell === undefined) continue;
      if (cell === '~') continue;
      setKey(row, columns[c], decodeValue(cell, ctx));
    }

    out[i] = row;
  }

  return out;
}

/**
 * True when `value` looks like a json-compress envelope this build can read.
 * @param {unknown} value
 * @returns {boolean}
 */
export function isCompressed(value) {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    /** @type {any} */ (value)[ENV_VERSION] === FORMAT_VERSION &&
    Array.isArray(/** @type {any} */ (value)[ENV_KEYS])
  );
}

/**
 * Restore the original value from a json-compress envelope.
 *
 * @param {unknown} envelope
 * @returns {unknown}
 * @throws {Error} if the envelope is not a readable json-compress document
 */
export function decompress(envelope) {
  if (envelope === null || typeof envelope !== 'object' || Array.isArray(envelope)) {
    throw new DecodeError('expected a compressed envelope object');
  }

  const version = /** @type {any} */ (envelope)[ENV_VERSION];
  if (version === undefined) {
    throw new DecodeError('this is not a compressed document (no "jc" version field)');
  }
  if (version !== FORMAT_VERSION) {
    throw new DecodeError(
      'format version ' + version + ' is not supported by this build (expected ' + FORMAT_VERSION + ')'
    );
  }

  const keys = /** @type {any} */ (envelope)[ENV_KEYS];
  if (!Array.isArray(keys)) {
    throw new DecodeError('the key catalogue is missing or is not an array');
  }

  const strings = /** @type {any} */ (envelope)[ENV_STRINGS];
  if (strings !== undefined && !Array.isArray(strings)) {
    throw new DecodeError('the string catalogue is not an array');
  }

  return decodeValue(/** @type {any} */ (envelope)[ENV_DATA], {
    keys,
    strings: strings || [],
  });
}

export { DecodeError };
