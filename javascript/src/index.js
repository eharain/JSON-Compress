/**
 * json-compress
 *
 * Lossless JSON compression that is still JSON.
 *
 * Three ideas, applied together:
 *
 *   1. Every object key is replaced by a short base-62 token and the real names
 *      are listed once, in a catalogue. A key that occurs ten thousand times is
 *      spelled out once.
 *   2. An array of like-shaped objects becomes a table - one column list and a
 *      matrix of rows - so the keys disappear from the rows entirely. Columns
 *      whose value never changes are hoisted out of the rows altogether, and
 *      columns drawn from a small set of strings become integer indices.
 *   3. String values that repeat are moved into a second catalogue and
 *      referenced by token.
 *
 * The output is an ordinary JSON value with no binary framing, so it survives
 * anything that carries JSON: an HTTP body, a WebSocket frame, localStorage, a
 * JSONB column, a log line.
 *
 * @module json-compress
 */

export { compress } from './encode.js';
export { decompress, isCompressed, DecodeError } from './decode.js';
export { pack, unpack, packAll, unpackAll, isPacked, textToArray } from './table-pack.js';
export { byteLength, gzip, gunzip, gzipSize, canGzip } from './bytes.js';
export { DEFAULTS as defaultOptions } from './options.js';
export { FORMAT_VERSION } from './constants.js';
export { locate } from './locate.js';

import { compress } from './encode.js';
import { decompress, isCompressed } from './decode.js';
import { byteLength, gzipSize } from './bytes.js';
import { pack, unpack, packAll, unpackAll, isPacked, textToArray } from './table-pack.js';
import { locate } from './locate.js';

/**
 * Compress a value and serialise it in one step.
 *
 * @param {unknown} value
 * @param {import('./options.js').CompressOptions} [options]
 * @returns {string}
 */
export function stringify(value, options) {
  return JSON.stringify(compress(value, options));
}

/**
 * Parse a compressed document and restore the original value.
 *
 * @param {string} text
 * @returns {unknown}
 */
export function parse(text) {
  return decompress(JSON.parse(text));
}

/**
 * Restore a value whether or not it was compressed.
 *
 * Handy at a boundary you do not fully control - an endpoint being migrated,
 * a cache holding entries written by two versions of the same service.
 *
 * @param {unknown} value
 * @returns {unknown}
 */
export function decompressIfNeeded(value) {
  return isCompressed(value) ? decompress(value) : value;
}

/**
 * Strip the whitespace from a JSON document without changing what it means.
 *
 * @param {string} text
 * @returns {string}
 */
export function minify(text) {
  return JSON.stringify(JSON.parse(text));
}

/**
 * Re-indent a JSON document.
 *
 * @param {string} text
 * @param {number|string} [indent=2]
 * @returns {string}
 */
export function beautify(text, indent = 2) {
  return JSON.stringify(JSON.parse(text), null, indent);
}

/**
 * Check that a string is valid JSON, and say exactly where it stops being so.
 *
 * The verdict comes from the engine, so it is the same verdict `JSON.parse`
 * would give. The location comes from a scan of the document, because engine
 * error messages disagree with each other about what they include - and often
 * include no position at all.
 *
 * @param {string} text
 * @returns {import('./locate.js').Location}
 */
export function validate(text) {
  try {
    JSON.parse(text);
    return { valid: true };
  } catch (error) {
    const found = locate(text);
    if (!found.valid) return found;

    // The scan found nothing but the parse still failed - report what the
    // engine said rather than claiming the document is fine.
    return { valid: false, error: error instanceof Error ? error.message : String(error) };
  }
}

/**
 * @typedef {object} SizeReport
 * @property {number} original   Bytes of the input as given.
 * @property {number} minified   Bytes once whitespace is removed.
 * @property {number} compressed Bytes of the compressed envelope.
 * @property {number} saved      Bytes saved against the minified form.
 * @property {number} ratio      Compressed divided by minified, 0 to 1.
 * @property {number} percent    Percentage saved against the minified form.
 * @property {number|null} gzipMinified   Minified bytes after gzip, if available.
 * @property {number|null} gzipCompressed Compressed bytes after gzip, if available.
 */

/**
 * Measure what compression buys on a given value.
 *
 * The honest comparison is against minified JSON, not against whatever
 * whitespace the input happened to arrive with, so `ratio` and `percent` both
 * use the minified size as their baseline. Gzip figures are included because
 * that is what most payloads meet in production, and the interesting question
 * is whether the format still helps once gzip has had its turn.
 *
 * @param {unknown} value
 * @param {import('./options.js').CompressOptions & {source?: string, gzip?: boolean}} [options]
 * @returns {Promise<SizeReport>}
 */
export async function measure(value, options = {}) {
  const { source, gzip: withGzip = true, ...compressOptions } = options;

  const minified = JSON.stringify(value);
  const compressed = JSON.stringify(compress(value, compressOptions));

  const originalBytes = source === undefined ? byteLength(minified) : byteLength(source);
  const minifiedBytes = byteLength(minified);
  const compressedBytes = byteLength(compressed);

  const report = {
    original: originalBytes,
    minified: minifiedBytes,
    compressed: compressedBytes,
    saved: minifiedBytes - compressedBytes,
    ratio: minifiedBytes === 0 ? 1 : compressedBytes / minifiedBytes,
    percent: minifiedBytes === 0 ? 0 : ((minifiedBytes - compressedBytes) / minifiedBytes) * 100,
    gzipMinified: null,
    gzipCompressed: null,
  };

  if (withGzip) {
    report.gzipMinified = await gzipSize(minified);
    report.gzipCompressed = await gzipSize(compressed);
  }

  return report;
}

/**
 * Bind a set of options once and reuse them.
 *
 * A codec is the natural unit when the same options apply everywhere - a
 * transport layer, a cache adapter, a storage helper - and it keeps the option
 * object off every call site.
 *
 * @param {import('./options.js').CompressOptions} [options]
 */
export function createCodec(options) {
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
    options,
  };
}

export default {
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
  textToArray,
};
