/**
 * Byte measurement and gzip, using whatever the host runtime provides.
 *
 * `TextEncoder` and `CompressionStream` are web platform APIs that Node has
 * carried as globals since v18, so one implementation covers Node, Deno, Bun,
 * browsers and workers without a build-time branch and without a dependency.
 */

const encoder = typeof TextEncoder !== 'undefined' ? new TextEncoder() : null;

/**
 * Length of a string in UTF-8 bytes - what actually goes over the wire, as
 * opposed to `String.length`, which counts UTF-16 code units.
 * @param {string} text
 * @returns {number}
 */
export function byteLength(text) {
  if (encoder) return encoder.encode(text).length;
  /* c8 ignore next */
  return Buffer.byteLength(text, 'utf8');
}

/** True when the runtime can gzip without help. */
export function canGzip() {
  return typeof CompressionStream !== 'undefined';
}

async function through(stream, bytes) {
  const writer = stream.writable.getWriter();
  writer.write(bytes);
  writer.close();

  const reader = stream.readable.getReader();
  const chunks = [];
  let total = 0;
  for (;;) {
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

/**
 * Gzip a string.
 * @param {string} text
 * @param {'gzip'|'deflate'|'deflate-raw'} [format='gzip']
 * @returns {Promise<Uint8Array>}
 */
export async function gzip(text, format = 'gzip') {
  if (!canGzip()) {
    throw new Error('json-compress: this runtime has no CompressionStream');
  }
  const bytes = encoder ? encoder.encode(text) : new Uint8Array(Buffer.from(text, 'utf8'));
  return through(new CompressionStream(format), bytes);
}

/**
 * Reverse {@link gzip}.
 * @param {Uint8Array} bytes
 * @param {'gzip'|'deflate'|'deflate-raw'} [format='gzip']
 * @returns {Promise<string>}
 */
export async function gunzip(bytes, format = 'gzip') {
  if (typeof DecompressionStream === 'undefined') {
    throw new Error('json-compress: this runtime has no DecompressionStream');
  }
  const out = await through(new DecompressionStream(format), bytes);
  return new TextDecoder().decode(out);
}

/**
 * How many bytes a string takes once gzipped. Returns `null` on a runtime that
 * cannot gzip, so callers can present the figure as unavailable rather than
 * having to guard the call.
 * @param {string} text
 * @returns {Promise<number|null>}
 */
export async function gzipSize(text) {
  if (!canGzip()) return null;
  const bytes = await gzip(text);
  return bytes.length;
}
