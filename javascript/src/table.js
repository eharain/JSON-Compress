/**
 * Columnar table planning.
 *
 * An array of like-shaped objects is the single most wasteful thing JSON does:
 * every key is spelled out again on every row. This module decides, per array,
 * whether it is worth turning into a table - a column list plus a rows matrix -
 * and how each individual column should be encoded.
 *
 * The decision is taken once, during the analysis pass, and cached against the
 * array's identity so the build pass does not have to work it out again.
 */

import { COL_RAW, COL_DICT, COL_CONST } from './constants.js';
import { orderColumns } from './column-order.js';

const hasOwn = Object.prototype.hasOwnProperty;

/**
 * @typedef {object} TablePlan
 * @property {string[]} columns    Every column, in first-seen order.
 * @property {number[]} encodings  One of COL_RAW / COL_DICT / COL_CONST per column.
 * @property {unknown[]} constants Values of the COL_CONST columns, in column order.
 * @property {number} variableCount How many columns actually appear in the rows.
 */

/** True for a value that can safely be hoisted out of the rows and shared. */
function isPrimitive(value) {
  if (value === null) return true;
  const type = typeof value;
  return type === 'string' || type === 'number' || type === 'boolean';
}

/** True for something that can be a table row. */
function isRowObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/**
 * Decide how to encode an array.
 *
 * @param {unknown[]} array
 * @param {import('./options.js').CompressOptions} options
 * @returns {TablePlan|null} a plan, or null to encode the array element by element
 */
export function planTable(array, options) {
  if (!options.tables) return null;

  const rowCount = array.length;
  if (rowCount < options.minRows) return null;

  for (let i = 0; i < rowCount; i++) {
    if (!isRowObject(array[i])) return null;
  }

  // One column order has to stand in for every row's key order. When no such
  // order exists the array is left as objects, which is slower on the wire but
  // hands the document back exactly as it arrived.
  const columns = orderColumns(array);
  if (columns === null) return null;

  const columnCount = columns.length;
  if (columnCount === 0) return null;

  // One pass over the cells, gathering everything the encoding choice needs.
  const stats = new Array(columnCount);
  for (let c = 0; c < columnCount; c++) {
    stats[c] = {
      present: 0,
      strings: 0,
      nulls: 0,
      onlyStrings: true,
      constant: true,
      constantValue: undefined,
      totalLength: 0,
      distinct: new Set(),
    };
  }

  let holes = 0;
  for (let i = 0; i < rowCount; i++) {
    const row = array[i];
    for (let c = 0; c < columnCount; c++) {
      const key = columns[c];
      const stat = stats[c];
      if (!hasOwn.call(row, key) || row[key] === undefined) {
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
      if (typeof value === 'string') {
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

    const isConstant =
      options.constants &&
      stat.constant &&
      stat.present === rowCount &&
      isPrimitive(stat.constantValue);

    if (isConstant) {
      encodings[c] = COL_CONST;
      constants.push(stat.constantValue);
      continue;
    }

    variableCount++;

    // A dictionary column pays for itself when the same handful of strings keep
    // coming back: each cell shrinks to a small integer, and the strings move
    // into the catalogue where they are written exactly once.
    const worthDictionary =
      options.dictColumns &&
      options.strings &&
      stat.onlyStrings &&
      stat.strings >= 3 &&
      stat.distinct.size > 0 &&
      stat.distinct.size <= stat.strings * 0.6 &&
      stat.totalLength / stat.strings >= 2;

    encodings[c] = worthDictionary ? COL_DICT : COL_RAW;
  }

  return { columns, encodings, constants, variableCount };
}

/** Iterate the dictionary-encoded values of a planned table. */
export function forEachDictionaryValue(array, plan, visit) {
  const { columns, encodings } = plan;
  for (let i = 0; i < array.length; i++) {
    const row = array[i];
    for (let c = 0; c < columns.length; c++) {
      if (encodings[c] !== COL_DICT) continue;
      const key = columns[c];
      if (!hasOwn.call(row, key)) continue;
      const value = row[key];
      if (typeof value === 'string') visit(value);
    }
  }
}
