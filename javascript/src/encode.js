/**
 * The encoder.
 *
 * Two passes. The first walks the document and counts things: how often each
 * key is used, how often each string value appears, and which arrays are worth
 * turning into tables. The second builds the output using catalogues ordered by
 * those counts, so the most-used key gets the one-character token.
 *
 * Splitting it this way costs a second walk and buys a materially smaller
 * document, because token length is decided by frequency rather than by the
 * accident of which key the encoder happened to meet first.
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
  HOLE,
  DICT_HOLE,
  COL_DICT,
  COL_CONST,
  TILDE,
} from './constants.js';
import { toBase62, toKeyToken, base62Length } from './base62.js';
import { resolveOptions } from './options.js';
import { normalize } from './normalize.js';
import { planTable } from './table.js';

const hasOwn = Object.prototype.hasOwnProperty;

/* ------------------------------------------------------------------ *
 * Pass one - analysis
 * ------------------------------------------------------------------ */

function createAnalysis(options) {
  return {
    options,
    keyCounts: new Map(),
    stringCounts: new Map(),
    /** @type {Map<unknown[], import('./table.js').TablePlan|null>} */
    plans: new Map(),
    forcedStrings: new Set(),
  };
}

function countKey(state, key) {
  state.keyCounts.set(key, (state.keyCounts.get(key) || 0) + 1);
}

function countString(state, value) {
  if (!state.options.strings) return;
  state.stringCounts.set(value, (state.stringCounts.get(value) || 0) + 1);
}

function analyseValue(state, value) {
  if (value === null) return;
  const type = typeof value;
  if (type === 'string') {
    countString(state, value);
    return;
  }
  if (type !== 'object') return;

  if (Array.isArray(value)) {
    const plan = planTable(value, state.options);
    state.plans.set(value, plan);
    if (plan) {
      analyseTable(state, value, plan);
    } else {
      for (let i = 0; i < value.length; i++) analyseValue(state, value[i]);
    }
    return;
  }

  const keys = Object.keys(value);
  for (let i = 0; i < keys.length; i++) {
    countKey(state, keys[i]);
    analyseValue(state, value[keys[i]]);
  }
}

function analyseTable(state, rows, plan) {
  const { columns, encodings, constants } = plan;

  // A table spells each key out once, in the column list, however many rows it
  // has - so it contributes one to the key's frequency, not one per row.
  for (let c = 0; c < columns.length; c++) countKey(state, columns[c]);
  for (let i = 0; i < constants.length; i++) analyseValue(state, constants[i]);

  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    for (let c = 0; c < columns.length; c++) {
      if (encodings[c] === COL_CONST) continue;
      const key = columns[c];
      if (!hasOwn.call(row, key) || row[key] === undefined) continue;
      const value = row[key];
      if (encodings[c] === COL_DICT) {
        if (typeof value === 'string') {
          countString(state, value);
          state.forcedStrings.add(value);
        }
      } else {
        analyseValue(state, value);
      }
    }
  }
}

/* ------------------------------------------------------------------ *
 * Catalogues
 * ------------------------------------------------------------------ */

/** Most-used key first, so the busiest keys get the shortest tokens. */
function buildKeyCatalogue(state) {
  const entries = [...state.keyCounts.keys()];
  entries.sort((a, b) => state.keyCounts.get(b) - state.keyCounts.get(a));

  const list = new Array(entries.length);
  const index = new Map();
  const tokens = new Map();
  for (let i = 0; i < entries.length; i++) {
    list[i] = entries[i];
    index.set(entries[i], i);
    tokens.set(entries[i], toKeyToken(i));
  }
  return { list, index, tokens };
}

/**
 * Choose which string values are worth moving into the catalogue.
 *
 * A catalogued string is written once in the catalogue and referenced by a
 * short `~token` everywhere it occurs, so the trade is:
 *
 *     inline    = count * (length + 2)                  // quotes on every copy
 *     catalogue = count * (tokenLength + 3) + length + 3
 *
 * Candidates are considered in frequency order, so the token length used in
 * that comparison is the one the string would really be given.
 *
 * Values pulled into a dictionary column are forced in regardless: their cells
 * are bare integers, which is cheaper still, and the comparison above does not
 * describe them.
 */
function buildStringCatalogue(state) {
  const list = [];
  const index = new Map();
  if (!state.options.strings) return { list, index };

  const { minStringCount, minStringLength } = state.options;

  const candidates = [];
  for (const [value, count] of state.stringCounts) {
    const forced = state.forcedStrings.has(value);
    if (!forced) {
      if (count < minStringCount) continue;
      if (value.length < minStringLength) continue;
      // A value that already begins with a tilde is escaped inline and can
      // never be written as a reference, so cataloguing it would only burn an
      // index.
      if (value.charCodeAt(0) === TILDE) continue;
    }
    candidates.push({ value, count, forced });
  }

  candidates.sort((a, b) => b.count - a.count || b.value.length - a.value.length);

  for (let i = 0; i < candidates.length; i++) {
    const { value, count, forced } = candidates[i];
    if (!forced) {
      const tokenLength = base62Length(list.length);
      const inline = count * (value.length + 2);
      const catalogued = count * (tokenLength + 3) + value.length + 3;
      if (catalogued >= inline) continue;
    }
    index.set(value, list.length);
    list.push(value);
  }

  return { list, index };
}

/* ------------------------------------------------------------------ *
 * Pass two - build
 * ------------------------------------------------------------------ */

function encodeString(value, strings) {
  // Escape first: an escaped literal always starts with two tildes, a reference
  // is a tilde followed by a base-62 digit, and a lone tilde is the row hole.
  // Those three spaces cannot overlap.
  if (value.charCodeAt(0) === TILDE) return '~' + value;
  const at = strings.index.get(value);
  return at === undefined ? value : '~' + toBase62(at);
}

function encodeValue(value, ctx) {
  if (value === null) return null;
  const type = typeof value;
  if (type === 'string') return encodeString(value, ctx.strings);
  if (type !== 'object') return value;

  if (Array.isArray(value)) {
    const plan = ctx.plans.get(value);
    if (plan) return encodeTable(value, plan, ctx);
    const out = new Array(value.length);
    for (let i = 0; i < value.length; i++) out[i] = encodeValue(value[i], ctx);
    return out;
  }

  const out = {};
  const keys = Object.keys(value);
  for (let i = 0; i < keys.length; i++) {
    out[ctx.keys.tokens.get(keys[i])] = encodeValue(value[keys[i]], ctx);
  }
  return out;
}

function encodeTable(rows, plan, ctx) {
  const { columns, encodings, constants, variableCount } = plan;

  const node = {};
  node[T_COLUMNS] = columns.map((key) => ctx.keys.index.get(key));

  let uniform = true;
  for (let c = 0; c < encodings.length; c++) {
    if (encodings[c] !== 0) {
      uniform = false;
      break;
    }
  }
  if (!uniform) node[T_ENCODINGS] = encodings;

  if (constants.length > 0) {
    node[T_CONSTS] = constants.map((value) => encodeValue(value, ctx));
  }

  if (variableCount === 0) {
    // Every column turned out to be constant, so the rows carry no data at all
    // and only their number survives.
    node[T_ROWCOUNT] = rows.length;
    return node;
  }

  const matrix = new Array(rows.length);
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    const cells = new Array(variableCount);
    let at = 0;
    for (let c = 0; c < columns.length; c++) {
      const encoding = encodings[c];
      if (encoding === COL_CONST) continue;
      const key = columns[c];
      const missing = !hasOwn.call(row, key) || row[key] === undefined;
      if (encoding === COL_DICT) {
        if (missing) {
          cells[at] = DICT_HOLE;
        } else if (row[key] === null) {
          cells[at] = null;
        } else {
          cells[at] = ctx.strings.index.get(row[key]);
        }
      } else {
        cells[at] = missing ? HOLE : encodeValue(row[key], ctx);
      }
      at++;
    }

    // Trailing gaps need not be written down; a short row is understood to be
    // missing everything past its end.
    let end = cells.length;
    while (end > 0) {
      const cell = cells[end - 1];
      if (cell === HOLE || cell === DICT_HOLE) end--;
      else break;
    }
    matrix[i] = end === cells.length ? cells : cells.slice(0, end);
  }

  node[T_ROWS] = matrix;
  return node;
}

/* ------------------------------------------------------------------ *
 * Entry point
 * ------------------------------------------------------------------ */

/**
 * Compress a value into a json-compress envelope.
 *
 * The result is an ordinary JSON-serialisable object, so it can be stored,
 * posted or logged anywhere plain JSON goes.
 *
 * @param {unknown} value
 * @param {import('./options.js').CompressOptions} [options]
 * @returns {{jc: number, k: string[], s?: string[], d: unknown}}
 */
export function compress(value, options) {
  const resolved = resolveOptions(options);
  const source = normalize(value);

  const state = createAnalysis(resolved);
  analyseValue(state, source);

  const keys = buildKeyCatalogue(state);
  const strings = buildStringCatalogue(state);

  const ctx = { keys, strings, plans: state.plans };
  const data = encodeValue(source, ctx);

  const envelope = {};
  envelope[ENV_VERSION] = FORMAT_VERSION;
  envelope[ENV_KEYS] = keys.list;
  if (strings.list.length > 0) envelope[ENV_STRINGS] = strings.list;
  envelope[ENV_DATA] = data;
  return envelope;
}
