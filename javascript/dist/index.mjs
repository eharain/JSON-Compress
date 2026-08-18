/*! @tech-style/json-compress v1.0.2 | MIT | https://github.com/eharain/JSON-Compress */

// src/constants.js
var FORMAT_VERSION = 1;
var ENV_VERSION = "jc";
var ENV_KEYS = "k";
var ENV_STRINGS = "s";
var ENV_DATA = "d";
var T_COLUMNS = "~t";
var T_ENCODINGS = "~e";
var T_CONSTS = "~cv";
var T_ROWS = "~r";
var T_ROWCOUNT = "~n";
var HOLE = "~";
var DICT_HOLE = -1;
var COL_RAW = 0;
var COL_DICT = 1;
var COL_CONST = 2;
var TILDE = 126;

// src/base62.js
var DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
var LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
var DIGIT_VALUES = /* @__PURE__ */ new Map();
for (let i = 0; i < DIGITS.length; i++) DIGIT_VALUES.set(DIGITS[i], i);
var LETTER_VALUES = /* @__PURE__ */ new Map();
for (let i = 0; i < LETTERS.length; i++) LETTER_VALUES.set(LETTERS[i], i);
function toBase62(n) {
  if (n < 62) return DIGITS[n];
  let out = "";
  let value = n;
  while (value > 0) {
    out = DIGITS[value % 62] + out;
    value = Math.floor(value / 62);
  }
  return out;
}
function fromBase62(token) {
  if (token.length === 0) return -1;
  let n = 0;
  for (let i = 0; i < token.length; i++) {
    const digit = DIGIT_VALUES.get(token[i]);
    if (digit === void 0) return -1;
    n = n * 62 + digit;
  }
  return n;
}
function base62Length(n) {
  if (n < 62) return 1;
  if (n < 3844) return 2;
  if (n < 238328) return 3;
  let length = 0;
  let value = n;
  while (value > 0) {
    length++;
    value = Math.floor(value / 62);
  }
  return length;
}
function toKeyToken(n) {
  if (n < 52) return LETTERS[n];
  let start = 0;
  let size = 52;
  let length = 1;
  while (n >= start + size) {
    start += size;
    size *= 62;
    length++;
  }
  let value = n - start;
  let tail = "";
  for (let i = 1; i < length; i++) {
    tail = DIGITS[value % 62] + tail;
    value = Math.floor(value / 62);
  }
  return LETTERS[value] + tail;
}
function fromKeyToken(token) {
  if (token.length === 0) return -1;
  let n = LETTER_VALUES.get(token[0]);
  if (n === void 0) return -1;
  for (let i = 1; i < token.length; i++) {
    const digit = DIGIT_VALUES.get(token[i]);
    if (digit === void 0) return -1;
    n = n * 62 + digit;
  }
  let start = 0;
  let size = 52;
  for (let i = 1; i < token.length; i++) {
    start += size;
    size *= 62;
  }
  return start + n;
}

// src/options.js
var DEFAULTS = Object.freeze({
  strings: true,
  tables: true,
  constants: true,
  dictColumns: true,
  minRows: 2,
  maxHoleRatio: 0.3,
  minStringCount: 2,
  minStringLength: 1
});
function resolveOptions(options) {
  if (options === void 0 || options === null) return DEFAULTS;
  if (typeof options !== "object") {
    throw new TypeError("json-compress: options must be an object");
  }
  const merged = { ...DEFAULTS, ...options };
  if (!(merged.minRows >= 1)) {
    throw new RangeError("json-compress: minRows must be at least 1");
  }
  if (!(merged.maxHoleRatio >= 0 && merged.maxHoleRatio <= 1)) {
    throw new RangeError("json-compress: maxHoleRatio must be between 0 and 1");
  }
  if (!(merged.minStringCount >= 2)) {
    throw new RangeError("json-compress: minStringCount must be at least 2");
  }
  return merged;
}

// src/normalize.js
function walk(value, seen) {
  if (value === null) return null;
  const type = typeof value;
  if (type === "string" || type === "boolean") return value;
  if (type === "number") return Number.isFinite(value) ? value === 0 ? 0 : value : null;
  if (type === "undefined" || type === "function" || type === "symbol") return void 0;
  if (type === "bigint") {
    throw new TypeError("json-compress: BigInt values cannot be serialised");
  }
  if (typeof /** @type {any} */
  value.toJSON === "function") {
    return walk(
      /** @type {any} */
      value.toJSON(),
      seen
    );
  }
  const obj = (
    /** @type {object} */
    value
  );
  if (seen.has(obj)) {
    throw new TypeError("json-compress: circular structure cannot be compressed");
  }
  seen.add(obj);
  let result;
  if (Array.isArray(obj)) {
    let changed = false;
    const out = new Array(obj.length);
    for (let i = 0; i < obj.length; i++) {
      const next = walk(obj[i], seen);
      out[i] = next === void 0 ? null : next;
      if (out[i] !== obj[i]) changed = true;
    }
    result = changed ? out : obj;
  } else {
    let changed = false;
    const out = {};
    const keys = Object.keys(obj);
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      const next = walk(obj[key], seen);
      if (next === void 0) {
        changed = true;
        continue;
      }
      out[key] = next;
      if (next !== obj[key]) changed = true;
    }
    result = changed ? out : obj;
  }
  seen.delete(obj);
  return result;
}
function normalize(value) {
  const result = walk(value, /* @__PURE__ */ new Set());
  return result === void 0 ? null : result;
}

// src/column-order.js
var MinHeap = class {
  constructor() {
    this.items = [];
  }
  get size() {
    return this.items.length;
  }
  push(value) {
    const items = this.items;
    items.push(value);
    let at = items.length - 1;
    while (at > 0) {
      const parent = at - 1 >> 1;
      if (items[parent] <= items[at]) break;
      const swap = items[parent];
      items[parent] = items[at];
      items[at] = swap;
      at = parent;
    }
  }
  pop() {
    const items = this.items;
    const top = items[0];
    const last = items.pop();
    if (items.length > 0) {
      items[0] = last;
      let at = 0;
      for (; ; ) {
        const left = at * 2 + 1;
        const right = left + 1;
        let smallest = at;
        if (left < items.length && items[left] < items[smallest]) smallest = left;
        if (right < items.length && items[right] < items[smallest]) smallest = right;
        if (smallest === at) break;
        const swap = items[smallest];
        items[smallest] = items[at];
        items[at] = swap;
        at = smallest;
      }
    }
    return top;
  }
};
function orderColumns(rows) {
  const position = /* @__PURE__ */ new Map();
  const columns = [];
  for (let i = 0; i < rows.length; i++) {
    const keys = Object.keys(rows[i]);
    for (let j = 0; j < keys.length; j++) {
      const key = keys[j];
      if (rows[i][key] === void 0) continue;
      if (!position.has(key)) {
        position.set(key, columns.length);
        columns.push(key);
      }
    }
  }
  const count = columns.length;
  if (count === 0) return columns;
  if (count === 1) return columns;
  const after = new Array(count);
  const incoming = new Int32Array(count);
  for (let i = 0; i < count; i++) after[i] = /* @__PURE__ */ new Set();
  let sawEdge = false;
  for (let i = 0; i < rows.length; i++) {
    const keys = Object.keys(rows[i]);
    let previous = -1;
    for (let j = 0; j < keys.length; j++) {
      const key = keys[j];
      if (rows[i][key] === void 0) continue;
      const current = position.get(key);
      if (previous !== -1 && !after[previous].has(current)) {
        after[previous].add(current);
        incoming[current]++;
        sawEdge = true;
      }
      previous = current;
    }
  }
  if (!sawEdge) return columns;
  const ready = new MinHeap();
  for (let i = 0; i < count; i++) {
    if (incoming[i] === 0) ready.push(i);
  }
  const ordered = [];
  while (ready.size > 0) {
    const at = ready.pop();
    ordered.push(columns[at]);
    for (const next of after[at]) {
      if (--incoming[next] === 0) ready.push(next);
    }
  }
  return ordered.length === count ? ordered : null;
}

// src/table.js
var hasOwn = Object.prototype.hasOwnProperty;
function isPrimitive(value) {
  if (value === null) return true;
  const type = typeof value;
  return type === "string" || type === "number" || type === "boolean";
}
function isRowObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function planTable(array, options) {
  if (!options.tables) return null;
  const rowCount = array.length;
  if (rowCount < options.minRows) return null;
  for (let i = 0; i < rowCount; i++) {
    if (!isRowObject(array[i])) return null;
  }
  const columns = orderColumns(array);
  if (columns === null) return null;
  const columnCount = columns.length;
  if (columnCount === 0) return null;
  const stats = new Array(columnCount);
  for (let c = 0; c < columnCount; c++) {
    stats[c] = {
      present: 0,
      strings: 0,
      nulls: 0,
      onlyStrings: true,
      constant: true,
      constantValue: void 0,
      totalLength: 0,
      distinct: /* @__PURE__ */ new Set()
    };
  }
  let holes = 0;
  for (let i = 0; i < rowCount; i++) {
    const row = array[i];
    for (let c = 0; c < columnCount; c++) {
      const key = columns[c];
      const stat = stats[c];
      if (!hasOwn.call(row, key) || row[key] === void 0) {
        holes++;
        stat.constant = false;
        continue;
      }
      const value = row[key];
      stat.present++;
      if (stat.present === 1) {
        stat.constantValue = value;
      } else if (stat.constant && !Object.is(stat.constantValue, value)) {
        stat.constant = false;
      }
      if (typeof value === "string") {
        stat.strings++;
        stat.totalLength += value.length;
        stat.distinct.add(value);
      } else if (value === null) {
        stat.nulls++;
      } else {
        stat.onlyStrings = false;
      }
    }
  }
  if (holes / (rowCount * columnCount) > options.maxHoleRatio) return null;
  const encodings = new Array(columnCount);
  const constants = [];
  let variableCount = 0;
  for (let c = 0; c < columnCount; c++) {
    const stat = stats[c];
    const isConstant = options.constants && stat.constant && stat.present === rowCount && isPrimitive(stat.constantValue);
    if (isConstant) {
      encodings[c] = COL_CONST;
      constants.push(stat.constantValue);
      continue;
    }
    variableCount++;
    const worthDictionary = options.dictColumns && options.strings && stat.onlyStrings && stat.strings >= 3 && stat.distinct.size > 0 && stat.distinct.size <= stat.strings * 0.6 && stat.totalLength / stat.strings >= 2;
    encodings[c] = worthDictionary ? COL_DICT : COL_RAW;
  }
  return { columns, encodings, constants, variableCount };
}

// src/encode.js
var hasOwn2 = Object.prototype.hasOwnProperty;
function createAnalysis(options) {
  return {
    options,
    keyCounts: /* @__PURE__ */ new Map(),
    stringCounts: /* @__PURE__ */ new Map(),
    /** @type {Map<unknown[], import('./table.js').TablePlan|null>} */
    plans: /* @__PURE__ */ new Map(),
    forcedStrings: /* @__PURE__ */ new Set()
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
  if (type === "string") {
    countString(state, value);
    return;
  }
  if (type !== "object") return;
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
  for (let c = 0; c < columns.length; c++) countKey(state, columns[c]);
  for (let i = 0; i < constants.length; i++) analyseValue(state, constants[i]);
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    for (let c = 0; c < columns.length; c++) {
      if (encodings[c] === COL_CONST) continue;
      const key = columns[c];
      if (!hasOwn2.call(row, key) || row[key] === void 0) continue;
      const value = row[key];
      if (encodings[c] === COL_DICT) {
        if (typeof value === "string") {
          countString(state, value);
          state.forcedStrings.add(value);
        }
      } else {
        analyseValue(state, value);
      }
    }
  }
}
function buildKeyCatalogue(state) {
  const entries = [...state.keyCounts.keys()];
  entries.sort((a, b) => state.keyCounts.get(b) - state.keyCounts.get(a));
  const list = new Array(entries.length);
  const index = /* @__PURE__ */ new Map();
  const tokens = /* @__PURE__ */ new Map();
  for (let i = 0; i < entries.length; i++) {
    list[i] = entries[i];
    index.set(entries[i], i);
    tokens.set(entries[i], toKeyToken(i));
  }
  return { list, index, tokens };
}
function buildStringCatalogue(state) {
  const list = [];
  const index = /* @__PURE__ */ new Map();
  if (!state.options.strings) return { list, index };
  const { minStringCount, minStringLength } = state.options;
  const candidates = [];
  for (const [value, count] of state.stringCounts) {
    const forced = state.forcedStrings.has(value);
    if (!forced) {
      if (count < minStringCount) continue;
      if (value.length < minStringLength) continue;
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
function encodeString(value, strings) {
  if (value.charCodeAt(0) === TILDE) return "~" + value;
  const at = strings.index.get(value);
  return at === void 0 ? value : "~" + toBase62(at);
}
function encodeValue(value, ctx) {
  if (value === null) return null;
  const type = typeof value;
  if (type === "string") return encodeString(value, ctx.strings);
  if (type !== "object") return value;
  if (Array.isArray(value)) {
    const plan = ctx.plans.get(value);
    if (plan) return encodeTable(value, plan, ctx);
    const out2 = new Array(value.length);
    for (let i = 0; i < value.length; i++) out2[i] = encodeValue(value[i], ctx);
    return out2;
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
      const missing = !hasOwn2.call(row, key) || row[key] === void 0;
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
function compress(value, options) {
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

// src/decode.js
var hasOwn3 = Object.prototype.hasOwnProperty;
function setKey(target, key, value) {
  if (key === "__proto__") {
    Object.defineProperty(target, key, {
      value,
      writable: true,
      enumerable: true,
      configurable: true
    });
    return;
  }
  target[key] = value;
}
var DecodeError = class extends Error {
  constructor(message) {
    super("json-compress: " + message);
    this.name = "DecodeError";
  }
};
function decodeString(value, ctx) {
  if (value.charCodeAt(0) !== TILDE) return value;
  if (value.length === 1) {
    throw new DecodeError("a row hole marker appeared outside a table row");
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
  if (type === "string") return decodeString(node, ctx);
  if (type !== "object") return node;
  if (Array.isArray(node)) {
    const out2 = new Array(node.length);
    for (let i = 0; i < node.length; i++) out2[i] = decodeValue(node[i], ctx);
    return out2;
  }
  if (hasOwn3.call(node, T_COLUMNS)) return decodeTable(node, ctx);
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
    throw new DecodeError("table column list is not an array");
  }
  const columnCount = columnIndices.length;
  const columns = new Array(columnCount);
  for (let c = 0; c < columnCount; c++) {
    const at = columnIndices[c];
    if (!Number.isInteger(at) || at < 0 || at >= ctx.keys.length) {
      throw new DecodeError("table column index " + at + " is out of range");
    }
    columns[c] = ctx.keys[at];
  }
  const encodings = hasOwn3.call(node, T_ENCODINGS) ? node[T_ENCODINGS] : null;
  if (encodings !== null && (!Array.isArray(encodings) || encodings.length !== columnCount)) {
    throw new DecodeError("table encoding list does not match its columns");
  }
  const rawConstants = hasOwn3.call(node, T_CONSTS) ? node[T_CONSTS] : [];
  if (!Array.isArray(rawConstants)) {
    throw new DecodeError("table constant list is not an array");
  }
  const constants = new Array(rawConstants.length);
  for (let i = 0; i < rawConstants.length; i++) {
    constants[i] = decodeValue(rawConstants[i], ctx);
  }
  const matrix = hasOwn3.call(node, T_ROWS) ? node[T_ROWS] : null;
  if (matrix !== null && !Array.isArray(matrix)) {
    throw new DecodeError("table rows are not an array");
  }
  const rowCount = matrix !== null ? matrix.length : node[T_ROWCOUNT] | 0;
  const out = new Array(rowCount);
  for (let i = 0; i < rowCount; i++) {
    const cells = matrix !== null ? matrix[i] : null;
    if (cells !== null && !Array.isArray(cells)) {
      throw new DecodeError("table row " + i + " is not an array");
    }
    const row = {};
    let constantAt = 0;
    let cellAt = 0;
    for (let c = 0; c < columnCount; c++) {
      const encoding = encodings === null ? 0 : encodings[c];
      if (encoding === COL_CONST) {
        if (constantAt >= constants.length) {
          throw new DecodeError("table has fewer constants than constant columns");
        }
        setKey(row, columns[c], constants[constantAt++]);
        continue;
      }
      const cell = cells !== null && cellAt < cells.length ? cells[cellAt] : void 0;
      cellAt++;
      if (encoding === COL_DICT) {
        if (cell === void 0 || cell === DICT_HOLE) continue;
        if (cell === null) {
          setKey(row, columns[c], null);
          continue;
        }
        if (!Number.isInteger(cell) || cell < 0 || cell >= ctx.strings.length) {
          throw new DecodeError("dictionary index " + cell + " is out of range");
        }
        setKey(row, columns[c], ctx.strings[cell]);
        continue;
      }
      if (cell === void 0) continue;
      if (cell === "~") continue;
      setKey(row, columns[c], decodeValue(cell, ctx));
    }
    out[i] = row;
  }
  return out;
}
function isCompressed(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value) && /** @type {any} */
  value[ENV_VERSION] === FORMAT_VERSION && Array.isArray(
    /** @type {any} */
    value[ENV_KEYS]
  );
}
function decompress(envelope) {
  if (envelope === null || typeof envelope !== "object" || Array.isArray(envelope)) {
    throw new DecodeError("expected a compressed envelope object");
  }
  const version = (
    /** @type {any} */
    envelope[ENV_VERSION]
  );
  if (version === void 0) {
    throw new DecodeError('this is not a compressed document (no "jc" version field)');
  }
  if (version !== FORMAT_VERSION) {
    throw new DecodeError(
      "format version " + version + " is not supported by this build (expected " + FORMAT_VERSION + ")"
    );
  }
  const keys = (
    /** @type {any} */
    envelope[ENV_KEYS]
  );
  if (!Array.isArray(keys)) {
    throw new DecodeError("the key catalogue is missing or is not an array");
  }
  const strings = (
    /** @type {any} */
    envelope[ENV_STRINGS]
  );
  if (strings !== void 0 && !Array.isArray(strings)) {
    throw new DecodeError("the string catalogue is not an array");
  }
  return decodeValue(
    /** @type {any} */
    envelope[ENV_DATA],
    {
      keys,
      strings: strings || []
    }
  );
}

// src/table-pack.js
var hasOwn4 = Object.prototype.hasOwnProperty;
function pack(records) {
  if (records === void 0 || records === null) return records;
  if (!Array.isArray(records)) return records;
  if (records.length === 0) return { fields: [], data: [], count: 0 };
  for (let i = 0; i < records.length; i++) {
    const record = records[i];
    if (record === null || typeof record !== "object" || Array.isArray(record)) {
      return records;
    }
  }
  const seen = /* @__PURE__ */ new Set();
  const fields = [];
  for (let i = 0; i < records.length; i++) {
    const keys = Object.keys(records[i]);
    for (let j = 0; j < keys.length; j++) {
      if (!seen.has(keys[j])) {
        seen.add(keys[j]);
        fields.push(keys[j]);
      }
    }
  }
  const data = new Array(records.length);
  for (let i = 0; i < records.length; i++) {
    const record = records[i];
    const row = new Array(fields.length);
    for (let j = 0; j < fields.length; j++) {
      const value = record[fields[j]];
      row[j] = value === void 0 ? null : value;
    }
    data[i] = row;
  }
  return { fields, data, count: data.length };
}
function unpack(packed) {
  if (packed === null || typeof packed !== "object" || Array.isArray(packed)) return packed;
  const table = (
    /** @type {PackedTable} */
    packed
  );
  if (!Array.isArray(table.fields) || !Array.isArray(table.data)) return packed;
  const out = new Array(table.data.length);
  for (let i = 0; i < table.data.length; i++) {
    const row = table.data[i];
    const record = {};
    for (let j = 0; j < table.fields.length; j++) {
      const field = table.fields[j];
      const value = Array.isArray(row) ? row[j] : void 0;
      if (field === "__proto__") {
        Object.defineProperty(record, field, {
          value,
          writable: true,
          enumerable: true,
          configurable: true
        });
      } else {
        record[field] = value;
      }
    }
    out[i] = record;
  }
  return out;
}
function isPacked(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value) && Array.isArray(
    /** @type {any} */
    value.fields
  ) && Array.isArray(
    /** @type {any} */
    value.data
  );
}
function packAll(resultSets) {
  if (resultSets === void 0 || resultSets === null) return resultSets;
  if (!Array.isArray(resultSets) || resultSets.length === 0) return resultSets;
  return resultSets.map((set) => pack(set));
}
function unpackAll(resultSets) {
  if (resultSets === void 0 || resultSets === null) return resultSets;
  if (!Array.isArray(resultSets)) return resultSets;
  return resultSets.map((set) => unpack(set));
}
function textToArray(record, property, shape) {
  if (record === null || typeof record !== "object") return record;
  if (!hasOwn4.call(record, property)) return record;
  const open = shape && shape.open !== void 0 ? shape.open : 3;
  const close = shape && shape.close !== void 0 ? shape.close : 3;
  const separator = shape && shape.separator !== void 0 ? shape.separator : ",";
  const value = record[property];
  if (typeof value !== "string") {
    record[property] = [];
    return record;
  }
  const inner = value.substring(open, value.length - close);
  record[property] = inner.length === 0 ? [] : inner.split(separator).map((part) => parseInt(part, 10));
  return record;
}

// src/bytes.js
var encoder = typeof TextEncoder !== "undefined" ? new TextEncoder() : null;
function byteLength(text) {
  if (encoder) return encoder.encode(text).length;
  return Buffer.byteLength(text, "utf8");
}
function canGzip() {
  return typeof CompressionStream !== "undefined";
}
async function through(stream, bytes) {
  const writer = stream.writable.getWriter();
  writer.write(bytes);
  writer.close();
  const reader = stream.readable.getReader();
  const chunks = [];
  let total = 0;
  for (; ; ) {
    const { value, done } = await reader.read();
    if (done) break;
    chunks.push(value);
    total += value.length;
  }
  const out = new Uint8Array(total);
  let at = 0;
  for (let i = 0; i < chunks.length; i++) {
    out.set(chunks[i], at);
    at += chunks[i].length;
  }
  return out;
}
async function gzip(text, format = "gzip") {
  if (!canGzip()) {
    throw new Error("json-compress: this runtime has no CompressionStream");
  }
  const bytes = encoder ? encoder.encode(text) : new Uint8Array(Buffer.from(text, "utf8"));
  return through(new CompressionStream(format), bytes);
}
async function gunzip(bytes, format = "gzip") {
  if (typeof DecompressionStream === "undefined") {
    throw new Error("json-compress: this runtime has no DecompressionStream");
  }
  const out = await through(new DecompressionStream(format), bytes);
  return new TextDecoder().decode(out);
}
async function gzipSize(text) {
  if (!canGzip()) return null;
  const bytes = await gzip(text);
  return bytes.length;
}

// src/locate.js
var TAB = 9;
var LF = 10;
var CR = 13;
var SPACE = 32;
var QUOTE = 34;
var PLUS = 43;
var COMMA = 44;
var MINUS = 45;
var DOT = 46;
var ZERO = 48;
var NINE = 57;
var COLON = 58;
var OPEN_BRACKET = 91;
var BACKSLASH = 92;
var CLOSE_BRACKET = 93;
var OPEN_BRACE = 123;
var CLOSE_BRACE = 125;
var ScanError = class extends Error {
  constructor(message, at) {
    super(message);
    this.at = at;
  }
};
function describe(text, at) {
  if (at >= text.length) return "the document ends here";
  const char = text[at];
  const code = text.charCodeAt(at);
  if (code < 32) return "an unescaped control character (0x" + code.toString(16).padStart(2, "0") + ")";
  return '"' + char + '"';
}
var Scanner = class {
  constructor(text) {
    this.text = text;
    this.at = 0;
  }
  skipWhitespace() {
    const { text } = this;
    while (this.at < text.length) {
      const code = text.charCodeAt(this.at);
      if (code === SPACE || code === TAB || code === LF || code === CR) this.at++;
      else break;
    }
  }
  expect(code, what) {
    if (this.at >= this.text.length || this.text.charCodeAt(this.at) !== code) {
      throw new ScanError("expected " + what + ", found " + describe(this.text, this.at), this.at);
    }
    this.at++;
  }
  scanValue(depth) {
    if (depth > 5e3) {
      throw new ScanError("the document is nested too deeply to check", this.at);
    }
    this.skipWhitespace();
    if (this.at >= this.text.length) {
      throw new ScanError("expected a value, but the document ends here", this.at);
    }
    const code = this.text.charCodeAt(this.at);
    if (code === OPEN_BRACE) return this.scanObject(depth);
    if (code === OPEN_BRACKET) return this.scanArray(depth);
    if (code === QUOTE) return this.scanString();
    if (code === MINUS || code >= ZERO && code <= NINE) return this.scanNumber();
    if (this.text.startsWith("true", this.at)) {
      this.at += 4;
      return;
    }
    if (this.text.startsWith("false", this.at)) {
      this.at += 5;
      return;
    }
    if (this.text.startsWith("null", this.at)) {
      this.at += 4;
      return;
    }
    if (code === 39) {
      throw new ScanError("strings must use double quotes, not single quotes", this.at);
    }
    if (this.text.startsWith("True", this.at) || this.text.startsWith("False", this.at)) {
      throw new ScanError("true and false must be lower case in JSON", this.at);
    }
    if (this.text.startsWith("None", this.at) || this.text.startsWith("undefined", this.at)) {
      throw new ScanError("JSON has no None or undefined - use null", this.at);
    }
    throw new ScanError("expected a value, found " + describe(this.text, this.at), this.at);
  }
  scanObject(depth) {
    this.at++;
    this.skipWhitespace();
    if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACE) {
      this.at++;
      return;
    }
    for (; ; ) {
      this.skipWhitespace();
      if (this.at < this.text.length && this.text.charCodeAt(this.at) !== QUOTE) {
        if (this.text.charCodeAt(this.at) === 39) {
          throw new ScanError("strings must use double quotes, not single quotes", this.at);
        }
        throw new ScanError(
          "expected a quoted property name, found " + describe(this.text, this.at),
          this.at
        );
      }
      this.scanString();
      this.skipWhitespace();
      this.expect(COLON, "a colon after the property name");
      this.scanValue(depth + 1);
      this.skipWhitespace();
      if (this.at >= this.text.length) {
        throw new ScanError("this object is never closed", this.at);
      }
      const code = this.text.charCodeAt(this.at);
      if (code === COMMA) {
        const commaAt = this.at;
        this.at++;
        this.skipWhitespace();
        if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACE) {
          throw new ScanError("a trailing comma is not allowed in JSON", commaAt);
        }
        continue;
      }
      if (code === CLOSE_BRACE) {
        this.at++;
        return;
      }
      throw new ScanError(
        "expected a comma or a closing brace, found " + describe(this.text, this.at),
        this.at
      );
    }
  }
  scanArray(depth) {
    this.at++;
    this.skipWhitespace();
    if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACKET) {
      this.at++;
      return;
    }
    for (; ; ) {
      this.scanValue(depth + 1);
      this.skipWhitespace();
      if (this.at >= this.text.length) {
        throw new ScanError("this array is never closed", this.at);
      }
      const code = this.text.charCodeAt(this.at);
      if (code === COMMA) {
        const commaAt = this.at;
        this.at++;
        this.skipWhitespace();
        if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACKET) {
          throw new ScanError("a trailing comma is not allowed in JSON", commaAt);
        }
        continue;
      }
      if (code === CLOSE_BRACKET) {
        this.at++;
        return;
      }
      throw new ScanError(
        "expected a comma or a closing bracket, found " + describe(this.text, this.at),
        this.at
      );
    }
  }
  scanString() {
    const { text } = this;
    const start = this.at;
    this.at++;
    while (this.at < text.length) {
      const code = text.charCodeAt(this.at);
      if (code === QUOTE) {
        this.at++;
        return;
      }
      if (code === BACKSLASH) {
        this.at++;
        if (this.at >= text.length) break;
        const escape = text[this.at];
        if ('"\\/bfnrt'.includes(escape)) {
          this.at++;
          continue;
        }
        if (escape === "u") {
          for (let i = 1; i <= 4; i++) {
            const digit = text[this.at + i];
            if (digit === void 0 || !/[0-9a-fA-F]/.test(digit)) {
              throw new ScanError("\\u must be followed by four hexadecimal digits", this.at - 1);
            }
          }
          this.at += 5;
          continue;
        }
        throw new ScanError('"\\' + escape + '" is not a valid escape sequence', this.at - 1);
      }
      if (code < 32) {
        throw new ScanError(
          "a raw control character inside a string - escape it as \\u" + code.toString(16).padStart(4, "0"),
          this.at
        );
      }
      this.at++;
    }
    throw new ScanError("this string is never closed", start);
  }
  scanNumber() {
    const { text } = this;
    const start = this.at;
    if (text.charCodeAt(this.at) === MINUS) this.at++;
    if (this.at >= text.length) {
      throw new ScanError("this number has no digits", start);
    }
    if (text.charCodeAt(this.at) === ZERO) {
      this.at++;
      if (this.at < text.length) {
        const next = text.charCodeAt(this.at);
        if (next >= ZERO && next <= NINE) {
          throw new ScanError("a number may not have a leading zero", start);
        }
      }
    } else {
      const first = text.charCodeAt(this.at);
      if (!(first >= ZERO && first <= NINE)) {
        throw new ScanError("expected a digit, found " + describe(text, this.at), this.at);
      }
      while (this.at < text.length) {
        const code = text.charCodeAt(this.at);
        if (code >= ZERO && code <= NINE) this.at++;
        else break;
      }
    }
    if (this.at < text.length && text.charCodeAt(this.at) === DOT) {
      this.at++;
      let digits = 0;
      while (this.at < text.length) {
        const code = text.charCodeAt(this.at);
        if (code >= ZERO && code <= NINE) {
          this.at++;
          digits++;
        } else break;
      }
      if (digits === 0) {
        throw new ScanError("a decimal point must be followed by at least one digit", this.at - 1);
      }
    }
    if (this.at < text.length) {
      const code = text.charCodeAt(this.at);
      if (code === 101 || code === 69) {
        this.at++;
        if (this.at < text.length) {
          const sign = text.charCodeAt(this.at);
          if (sign === PLUS || sign === MINUS) this.at++;
        }
        let digits = 0;
        while (this.at < text.length) {
          const digit = text.charCodeAt(this.at);
          if (digit >= ZERO && digit <= NINE) {
            this.at++;
            digits++;
          } else break;
        }
        if (digits === 0) {
          throw new ScanError("an exponent must have at least one digit", this.at - 1);
        }
      }
    }
  }
};
function positionOf(text, at) {
  const clamped = Math.max(0, Math.min(at, text.length));
  const before = text.slice(0, clamped);
  let line = 1;
  for (let i = 0; i < before.length; i++) {
    if (before.charCodeAt(i) === LF) line++;
  }
  return { line, column: clamped - (before.lastIndexOf("\n") + 1) + 1 };
}
function locate(text) {
  if (typeof text !== "string") {
    return { valid: false, error: "expected a string" };
  }
  const scanner = new Scanner(text);
  try {
    scanner.scanValue(0);
    scanner.skipWhitespace();
    if (scanner.at < text.length) {
      throw new ScanError(
        "unexpected " + describe(text, scanner.at) + " after the end of the document",
        scanner.at
      );
    }
  } catch (error) {
    if (!(error instanceof ScanError)) throw error;
    const { line, column } = positionOf(text, error.at);
    const lines = text.split("\n");
    return {
      valid: false,
      error: error.message,
      position: Math.max(0, Math.min(error.at, text.length)),
      line,
      column,
      excerpt: lines[line - 1] === void 0 ? "" : lines[line - 1]
    };
  }
  return { valid: true };
}

// src/index.js
function stringify(value, options) {
  return JSON.stringify(compress(value, options));
}
function parse(text) {
  return decompress(JSON.parse(text));
}
function decompressIfNeeded(value) {
  return isCompressed(value) ? decompress(value) : value;
}
function minify(text) {
  return JSON.stringify(JSON.parse(text));
}
function beautify(text, indent = 2) {
  return JSON.stringify(JSON.parse(text), null, indent);
}
function validate(text) {
  try {
    JSON.parse(text);
    return { valid: true };
  } catch (error) {
    const found = locate(text);
    if (!found.valid) return found;
    return { valid: false, error: error instanceof Error ? error.message : String(error) };
  }
}
async function measure(value, options = {}) {
  const { source, gzip: withGzip = true, ...compressOptions } = options;
  const minified = JSON.stringify(value);
  const compressed = JSON.stringify(compress(value, compressOptions));
  const originalBytes = source === void 0 ? byteLength(minified) : byteLength(source);
  const minifiedBytes = byteLength(minified);
  const compressedBytes = byteLength(compressed);
  const report = {
    original: originalBytes,
    minified: minifiedBytes,
    compressed: compressedBytes,
    saved: minifiedBytes - compressedBytes,
    ratio: minifiedBytes === 0 ? 1 : compressedBytes / minifiedBytes,
    percent: minifiedBytes === 0 ? 0 : (minifiedBytes - compressedBytes) / minifiedBytes * 100,
    gzipMinified: null,
    gzipCompressed: null
  };
  if (withGzip) {
    report.gzipMinified = await gzipSize(minified);
    report.gzipCompressed = await gzipSize(compressed);
  }
  return report;
}
function createCodec(options) {
  return {
    /** @param {unknown} value */
    compress: (value) => compress(value, options),
    decompress,
    /** @param {unknown} value */
    stringify: (value) => JSON.stringify(compress(value, options)),
    /** @param {string} text */
    parse: (text) => decompress(JSON.parse(text)),
    /** @param {unknown} value */
    measure: (value) => measure(value, options),
    options
  };
}
var index_default = {
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
  pack,
  unpack,
  packAll,
  unpackAll,
  isPacked,
  textToArray
};
export {
  DecodeError,
  FORMAT_VERSION,
  beautify,
  byteLength,
  canGzip,
  compress,
  createCodec,
  decompress,
  decompressIfNeeded,
  index_default as default,
  DEFAULTS as defaultOptions,
  gunzip,
  gzip,
  gzipSize,
  isCompressed,
  isPacked,
  locate,
  measure,
  minify,
  pack,
  packAll,
  parse,
  stringify,
  textToArray,
  unpack,
  unpackAll,
  validate
};
