/**
 * Finding the exact place a JSON document goes wrong.
 *
 * `JSON.parse` throws a message, and what that message contains depends on the
 * engine and its version: sometimes a byte offset, sometimes a line and column,
 * increasingly often just a fragment of the text with no position at all. None
 * of that is enough to put a cursor on the offending character.
 *
 * So this scans the document itself, against the grammar in RFC 8259, and
 * reports the first thing that does not belong - with a byte offset, a line, a
 * column and a description written for whoever has to fix it.
 *
 * It is only ever run after `JSON.parse` has already failed, so the cost of a
 * second pass falls on documents that were not going to work anyway.
 */

const TAB = 9;
const LF = 10;
const CR = 13;
const SPACE = 32;
const QUOTE = 34;
const PLUS = 43;
const COMMA = 44;
const MINUS = 45;
const DOT = 46;
const ZERO = 48;
const NINE = 57;
const COLON = 58;
const OPEN_BRACKET = 91;
const BACKSLASH = 92;
const CLOSE_BRACKET = 93;
const OPEN_BRACE = 123;
const CLOSE_BRACE = 125;

/** Somewhere to unwind to, carrying the offset the problem was found at. */
class ScanError extends Error {
  constructor(message, at) {
    super(message);
    this.at = at;
  }
}

function describe(text, at) {
  if (at >= text.length) return 'the document ends here';
  const char = text[at];
  const code = text.charCodeAt(at);
  if (code < 32) return 'an unescaped control character (0x' + code.toString(16).padStart(2, '0') + ')';
  return '"' + char + '"';
}

class Scanner {
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
      throw new ScanError('expected ' + what + ', found ' + describe(this.text, this.at), this.at);
    }
    this.at++;
  }

  scanValue(depth) {
    if (depth > 5000) {
      throw new ScanError('the document is nested too deeply to check', this.at);
    }

    this.skipWhitespace();
    if (this.at >= this.text.length) {
      throw new ScanError('expected a value, but the document ends here', this.at);
    }

    const code = this.text.charCodeAt(this.at);

    if (code === OPEN_BRACE) return this.scanObject(depth);
    if (code === OPEN_BRACKET) return this.scanArray(depth);
    if (code === QUOTE) return this.scanString();
    if (code === MINUS || (code >= ZERO && code <= NINE)) return this.scanNumber();

    if (this.text.startsWith('true', this.at)) {
      this.at += 4;
      return;
    }
    if (this.text.startsWith('false', this.at)) {
      this.at += 5;
      return;
    }
    if (this.text.startsWith('null', this.at)) {
      this.at += 4;
      return;
    }

    // The three mistakes that account for most hand-edited JSON.
    if (code === 39) {
      throw new ScanError('strings must use double quotes, not single quotes', this.at);
    }
    if (this.text.startsWith('True', this.at) || this.text.startsWith('False', this.at)) {
      throw new ScanError('true and false must be lower case in JSON', this.at);
    }
    if (this.text.startsWith('None', this.at) || this.text.startsWith('undefined', this.at)) {
      throw new ScanError('JSON has no None or undefined - use null', this.at);
    }

    throw new ScanError('expected a value, found ' + describe(this.text, this.at), this.at);
  }

  scanObject(depth) {
    this.at++; // the opening brace
    this.skipWhitespace();

    if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACE) {
      this.at++;
      return;
    }

    for (;;) {
      this.skipWhitespace();
      if (this.at < this.text.length && this.text.charCodeAt(this.at) !== QUOTE) {
        if (this.text.charCodeAt(this.at) === 39) {
          throw new ScanError('strings must use double quotes, not single quotes', this.at);
        }
        throw new ScanError(
          'expected a quoted property name, found ' + describe(this.text, this.at),
          this.at
        );
      }
      this.scanString();

      this.skipWhitespace();
      this.expect(COLON, 'a colon after the property name');

      this.scanValue(depth + 1);
      this.skipWhitespace();

      if (this.at >= this.text.length) {
        throw new ScanError('this object is never closed', this.at);
      }
      const code = this.text.charCodeAt(this.at);
      if (code === COMMA) {
        const commaAt = this.at;
        this.at++;
        this.skipWhitespace();
        if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACE) {
          throw new ScanError('a trailing comma is not allowed in JSON', commaAt);
        }
        continue;
      }
      if (code === CLOSE_BRACE) {
        this.at++;
        return;
      }
      throw new ScanError(
        'expected a comma or a closing brace, found ' + describe(this.text, this.at),
        this.at
      );
    }
  }

  scanArray(depth) {
    this.at++; // the opening bracket
    this.skipWhitespace();

    if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACKET) {
      this.at++;
      return;
    }

    for (;;) {
      this.scanValue(depth + 1);
      this.skipWhitespace();

      if (this.at >= this.text.length) {
        throw new ScanError('this array is never closed', this.at);
      }
      const code = this.text.charCodeAt(this.at);
      if (code === COMMA) {
        const commaAt = this.at;
        this.at++;
        this.skipWhitespace();
        if (this.at < this.text.length && this.text.charCodeAt(this.at) === CLOSE_BRACKET) {
          throw new ScanError('a trailing comma is not allowed in JSON', commaAt);
        }
        continue;
      }
      if (code === CLOSE_BRACKET) {
        this.at++;
        return;
      }
      throw new ScanError(
        'expected a comma or a closing bracket, found ' + describe(this.text, this.at),
        this.at
      );
    }
  }

  scanString() {
    const { text } = this;
    const start = this.at;
    this.at++; // the opening quote

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
        if (escape === 'u') {
          for (let i = 1; i <= 4; i++) {
            const digit = text[this.at + i];
            if (digit === undefined || !/[0-9a-fA-F]/.test(digit)) {
              throw new ScanError('\\u must be followed by four hexadecimal digits', this.at - 1);
            }
          }
          this.at += 5;
          continue;
        }
        throw new ScanError('"\\' + escape + '" is not a valid escape sequence', this.at - 1);
      }

      if (code < 32) {
        throw new ScanError(
          'a raw control character inside a string - escape it as \\u' +
            code.toString(16).padStart(4, '0'),
          this.at
        );
      }

      this.at++;
    }

    throw new ScanError('this string is never closed', start);
  }

  scanNumber() {
    const { text } = this;
    const start = this.at;

    if (text.charCodeAt(this.at) === MINUS) this.at++;

    if (this.at >= text.length) {
      throw new ScanError('this number has no digits', start);
    }

    if (text.charCodeAt(this.at) === ZERO) {
      this.at++;
      if (this.at < text.length) {
        const next = text.charCodeAt(this.at);
        if (next >= ZERO && next <= NINE) {
          throw new ScanError('a number may not have a leading zero', start);
        }
      }
    } else {
      const first = text.charCodeAt(this.at);
      if (!(first >= ZERO && first <= NINE)) {
        throw new ScanError('expected a digit, found ' + describe(text, this.at), this.at);
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
        throw new ScanError('a decimal point must be followed by at least one digit', this.at - 1);
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
          throw new ScanError('an exponent must have at least one digit', this.at - 1);
        }
      }
    }
  }
}

/**
 * Turn a character offset into a line and column, both counted from one.
 * @param {string} text
 * @param {number} at
 */
function positionOf(text, at) {
  const clamped = Math.max(0, Math.min(at, text.length));
  const before = text.slice(0, clamped);
  let line = 1;
  for (let i = 0; i < before.length; i++) {
    if (before.charCodeAt(i) === LF) line++;
  }
  return { line, column: clamped - (before.lastIndexOf('\n') + 1) + 1 };
}

/**
 * @typedef {object} Location
 * @property {boolean} valid
 * @property {string} [error]    What is wrong, in plain words.
 * @property {number} [position] Character offset of the problem.
 * @property {number} [line]     Line number, counted from one.
 * @property {number} [column]   Column number, counted from one.
 * @property {string} [excerpt]  The line the problem is on.
 */

/**
 * Check a JSON document and, if it is broken, say exactly where.
 *
 * @param {string} text
 * @returns {Location}
 */
export function locate(text) {
  if (typeof text !== 'string') {
    return { valid: false, error: 'expected a string' };
  }

  const scanner = new Scanner(text);
  try {
    scanner.scanValue(0);
    scanner.skipWhitespace();
    if (scanner.at < text.length) {
      throw new ScanError(
        'unexpected ' + describe(text, scanner.at) + ' after the end of the document',
        scanner.at
      );
    }
  } catch (error) {
    if (!(error instanceof ScanError)) throw error;
    const { line, column } = positionOf(text, error.at);
    const lines = text.split('\n');
    return {
      valid: false,
      error: error.message,
      position: Math.max(0, Math.min(error.at, text.length)),
      line,
      column,
      excerpt: lines[line - 1] === undefined ? '' : lines[line - 1],
    };
  }

  return { valid: true };
}
