/**
 * Bring arbitrary input down to plain JSON values.
 *
 * The encoder is much simpler if it can assume it is only ever looking at
 * nulls, booleans, finite numbers, strings, arrays and plain objects. This pass
 * guarantees that, applying the same rules `JSON.stringify` does:
 *
 *   - `toJSON()` is honoured, so Dates become ISO strings;
 *   - non-finite numbers become `null`;
 *   - `undefined`, functions and symbols are dropped from objects and become
 *     `null` inside arrays;
 *   - circular references throw, rather than hanging.
 *
 * It returns the original reference whenever nothing had to change, so clean
 * input - which is the common case - costs a walk but no allocation.
 */

/**
 * @param {unknown} value
 * @param {Set<object>} seen  Ancestors on the current path, for cycle detection.
 * @returns {unknown} the normalised value, or `undefined` if it must be dropped
 */
function walk(value, seen) {
  if (value === null) return null;

  const type = typeof value;
  if (type === 'string' || type === 'boolean') return value;
  // Negative zero is folded to zero so that the in-memory round trip and the
  // serialised one agree; JSON itself cannot tell them apart.
  if (type === 'number') return Number.isFinite(value) ? (value === 0 ? 0 : value) : null;
  if (type === 'undefined' || type === 'function' || type === 'symbol') return undefined;
  if (type === 'bigint') {
    throw new TypeError('json-compress: BigInt values cannot be serialised');
  }

  if (typeof (/** @type {any} */ (value).toJSON) === 'function') {
    return walk(/** @type {any} */ (value).toJSON(), seen);
  }

  const obj = /** @type {object} */ (value);
  if (seen.has(obj)) {
    throw new TypeError('json-compress: circular structure cannot be compressed');
  }
  seen.add(obj);

  let result;
  if (Array.isArray(obj)) {
    let changed = false;
    const out = new Array(obj.length);
    for (let i = 0; i < obj.length; i++) {
      const next = walk(obj[i], seen);
      out[i] = next === undefined ? null : next;
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
      if (next === undefined) {
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

/**
 * @param {unknown} value
 * @returns {unknown}
 */
export function normalize(value) {
  const result = walk(value, new Set());
  return result === undefined ? null : result;
}
