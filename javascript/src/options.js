/**
 * Option handling.
 *
 * Every knob has a default that is safe on arbitrary input; the defaults are
 * tuned for API responses and log batches, which is where the format earns
 * most of its keep.
 */

/**
 * @typedef {object} CompressOptions
 * @property {boolean} [strings=true]      De-duplicate repeated string values into a catalogue.
 * @property {boolean} [tables=true]       Pack arrays of like-shaped objects into columnar tables.
 * @property {boolean} [constants=true]    Hoist columns whose value never changes out of the rows.
 * @property {boolean} [dictColumns=true]  Encode low-cardinality string columns as integer indices.
 * @property {number}  [minRows=2]         Smallest array that may become a table.
 * @property {number}  [maxHoleRatio=0.3]  Largest share of missing cells a table may carry.
 * @property {number}  [minStringCount=2]  Times a string must occur before it can be catalogued.
 * @property {number}  [minStringLength=1] Shortest string that may be catalogued.
 */

/** @type {Required<CompressOptions>} */
export const DEFAULTS = Object.freeze({
  strings: true,
  tables: true,
  constants: true,
  dictColumns: true,
  minRows: 2,
  maxHoleRatio: 0.3,
  minStringCount: 2,
  minStringLength: 1,
});

/**
 * Merge user options over the defaults, rejecting nonsense values early so that
 * a typo surfaces at the call site rather than as a mysteriously large output.
 * @param {CompressOptions} [options]
 * @returns {Required<CompressOptions>}
 */
export function resolveOptions(options) {
  if (options === undefined || options === null) return DEFAULTS;
  if (typeof options !== 'object') {
    throw new TypeError('json-compress: options must be an object');
  }
  const merged = { ...DEFAULTS, ...options };
  if (!(merged.minRows >= 1)) {
    throw new RangeError('json-compress: minRows must be at least 1');
  }
  if (!(merged.maxHoleRatio >= 0 && merged.maxHoleRatio <= 1)) {
    throw new RangeError('json-compress: maxHoleRatio must be between 0 and 1');
  }
  if (!(merged.minStringCount >= 2)) {
    throw new RangeError('json-compress: minStringCount must be at least 2');
  }
  return merged;
}
