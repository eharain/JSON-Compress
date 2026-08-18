/**
 * The plain table pack.
 *
 * This is the smallest useful idea in the library, offered on its own because
 * it is often all you need and because the shape it produces - `fields`, `data`
 * and `count` - is trivial to consume from another language or straight out of
 * a SQL driver.
 *
 * `pack` turns an array of objects into one field list and a matrix of rows.
 * `unpack` turns it back. Neither uses a catalogue, an envelope or a version
 * tag; what you get is a readable object you can eyeball in a network panel.
 */

const hasOwn = Object.prototype.hasOwnProperty;

/**
 * @typedef {object} PackedTable
 * @property {string[]} fields Column names, in the order the row arrays use.
 * @property {unknown[][]} data One array of values per record.
 * @property {number} count Number of records, so consumers need not measure.
 */

/**
 * Pack an array of records into a field list and a rows matrix.
 *
 * Keys are taken from the union of every record, in first-seen order, so a
 * record that is missing a field simply contributes `null` in that column.
 * Anything that is not an array of objects is handed straight back, which lets
 * you drop `pack` into a response pipeline without first checking the shape.
 *
 * @param {unknown} records
 * @returns {PackedTable|unknown}
 */
export function pack(records) {
  if (records === undefined || records === null) return records;
  if (!Array.isArray(records)) return records;
  if (records.length === 0) return { fields: [], data: [], count: 0 };

  for (let i = 0; i < records.length; i++) {
    const record = records[i];
    if (record === null || typeof record !== 'object' || Array.isArray(record)) {
      return records;
    }
  }

  const seen = new Set();
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
      row[j] = value === undefined ? null : value;
    }
    data[i] = row;
  }

  return { fields, data, count: data.length };
}

/**
 * Rebuild the array of records produced by {@link pack}.
 *
 * Anything that is not a packed table is handed straight back, so a consumer
 * can call `unpack` unconditionally on a payload that may or may not have been
 * packed at the other end.
 *
 * @param {PackedTable|unknown} packed
 * @returns {unknown}
 */
export function unpack(packed) {
  if (packed === null || typeof packed !== 'object' || Array.isArray(packed)) return packed;

  const table = /** @type {PackedTable} */ (packed);
  if (!Array.isArray(table.fields) || !Array.isArray(table.data)) return packed;

  const out = new Array(table.data.length);
  for (let i = 0; i < table.data.length; i++) {
    const row = table.data[i];
    const record = {};
    for (let j = 0; j < table.fields.length; j++) {
      const field = table.fields[j];
      const value = Array.isArray(row) ? row[j] : undefined;
      if (field === '__proto__') {
        Object.defineProperty(record, field, {
          value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
      } else {
        record[field] = value;
      }
    }
    out[i] = record;
  }
  return out;
}

/**
 * True when `value` has the shape {@link unpack} knows how to read.
 * @param {unknown} value
 * @returns {boolean}
 */
export function isPacked(value) {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Array.isArray(/** @type {any} */ (value).fields) &&
    Array.isArray(/** @type {any} */ (value).data)
  );
}

/**
 * Pack every result set in a list of result sets.
 *
 * Database drivers that return several result sets from one call hand back an
 * array of arrays; this packs each inner array and leaves the outer one alone.
 *
 * @param {unknown} resultSets
 * @returns {unknown}
 */
export function packAll(resultSets) {
  if (resultSets === undefined || resultSets === null) return resultSets;
  if (!Array.isArray(resultSets) || resultSets.length === 0) return resultSets;
  return resultSets.map((set) => pack(set));
}

/**
 * Unpack every result set packed by {@link packAll}.
 * @param {unknown} resultSets
 * @returns {unknown}
 */
export function unpackAll(resultSets) {
  if (resultSets === undefined || resultSets === null) return resultSets;
  if (!Array.isArray(resultSets)) return resultSets;
  return resultSets.map((set) => unpack(set));
}

/**
 * Read a delimited-integer column back into an array of numbers.
 *
 * Some databases return a set-valued column as a bracketed, comma-separated
 * string. This converts one such property in place, leaving an empty array
 * where the column was null.
 *
 * @param {Record<string, unknown>} record
 * @param {string} property
 * @param {{open?: number, close?: number, separator?: string}} [shape]
 *        How many characters to trim from each end, and what separates values.
 * @returns {Record<string, unknown>} the same record, for chaining
 */
export function textToArray(record, property, shape) {
  if (record === null || typeof record !== 'object') return record;
  if (!hasOwn.call(record, property)) return record;

  const open = shape && shape.open !== undefined ? shape.open : 3;
  const close = shape && shape.close !== undefined ? shape.close : 3;
  const separator = shape && shape.separator !== undefined ? shape.separator : ',';

  const value = record[property];
  if (typeof value !== 'string') {
    record[property] = [];
    return record;
  }

  const inner = value.substring(open, value.length - close);
  record[property] = inner.length === 0 ? [] : inner.split(separator).map((part) => parseInt(part, 10));
  return record;
}
