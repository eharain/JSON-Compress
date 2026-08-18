/**
 * Short tokens for catalogue indices.
 *
 * Two alphabets, because the two places a token appears have different rules.
 *
 * String references sit in value position, where nothing constrains them, so
 * they use plain base-62: the first 62 entries cost one character.
 *
 * Object keys cannot. A JavaScript object - and every JSON parser that builds
 * one - reorders keys that look like array indices to the front, in numeric
 * order, so a key of `"10"` would jump ahead of `"2"`. Encoding a key as `"7"`
 * would therefore quietly shuffle the document. Key tokens instead start with a
 * letter and continue in base-62, which is never index-like: 52 keys still cost
 * one character, the next 3,224 cost two, and the original key order survives
 * the round trip exactly.
 */

const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
const LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

const DIGIT_VALUES = new Map();
for (let i = 0; i < DIGITS.length; i++) DIGIT_VALUES.set(DIGITS[i], i);

const LETTER_VALUES = new Map();
for (let i = 0; i < LETTERS.length; i++) LETTER_VALUES.set(LETTERS[i], i);

/* ------------------------------------------------------------------ *
 * Base-62, for string references
 * ------------------------------------------------------------------ */

/**
 * Encode a non-negative integer as a base-62 token.
 * @param {number} n
 * @returns {string}
 */
export function toBase62(n) {
  if (n < 62) return DIGITS[n];
  let out = '';
  let value = n;
  while (value > 0) {
    out = DIGITS[value % 62] + out;
    value = Math.floor(value / 62);
  }
  return out;
}

/**
 * Decode a base-62 token, or -1 if it is not one.
 * @param {string} token
 * @returns {number}
 */
export function fromBase62(token) {
  if (token.length === 0) return -1;
  let n = 0;
  for (let i = 0; i < token.length; i++) {
    const digit = DIGIT_VALUES.get(token[i]);
    if (digit === undefined) return -1;
    n = n * 62 + digit;
  }
  return n;
}

/** Length in characters of the base-62 token for `n`, without building it. */
export function base62Length(n) {
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

/* ------------------------------------------------------------------ *
 * Letter-first tokens, for object keys
 * ------------------------------------------------------------------ */

/**
 * Encode a non-negative integer as a key token: a letter, then base-62 digits.
 *
 * The numbering is bijective - each token length covers a contiguous block of
 * indices and no index has two spellings - so `fromKeyToken` inverts it exactly.
 *
 * @param {number} n
 * @returns {string}
 */
export function toKeyToken(n) {
  if (n < 52) return LETTERS[n];

  // Find the block this index falls in: 52 one-character tokens, then
  // 52 * 62 two-character ones, and so on.
  let start = 0;
  let size = 52;
  let length = 1;
  while (n >= start + size) {
    start += size;
    size *= 62;
    length++;
  }

  let value = n - start;
  let tail = '';
  for (let i = 1; i < length; i++) {
    tail = DIGITS[value % 62] + tail;
    value = Math.floor(value / 62);
  }
  return LETTERS[value] + tail;
}

/**
 * Decode a key token, or -1 if it is not one.
 * @param {string} token
 * @returns {number}
 */
export function fromKeyToken(token) {
  if (token.length === 0) return -1;

  let n = LETTER_VALUES.get(token[0]);
  if (n === undefined) return -1;

  for (let i = 1; i < token.length; i++) {
    const digit = DIGIT_VALUES.get(token[i]);
    if (digit === undefined) return -1;
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
