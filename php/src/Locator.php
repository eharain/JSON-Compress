<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * Finding the exact place a JSON document goes wrong.
 *
 * `json_decode` reports "Syntax error" and nothing else - no offset, no line,
 * no hint at which of the handful of common mistakes caused it.
 *
 * So this scans the document itself, against the grammar in RFC 8259, and
 * reports the first thing that does not belong: an offset, a line, a column,
 * the line itself, and a description written for a person. It knows about
 * single quotes, trailing commas, PHP and Python spellings of the literals,
 * leading zeros, unterminated strings and bad escapes.
 *
 * The scan is iterative rather than recursive, so a pathologically nested
 * document is reported as too deep rather than exhausting the stack. It walks
 * bytes rather than characters, which is safe because every character JSON
 * gives a meaning to is ASCII and UTF-8 never hides an ASCII byte inside a
 * longer sequence; offsets are converted to character counts only when there is
 * something to report.
 */
final class Locator
{
    private const MAX_DEPTH = 5000;

    private const WHITESPACE = " \t\n\r";
    private const DIGITS = '0123456789';
    private const HEX = '0123456789abcdefABCDEF';
    private const SIMPLE_ESCAPES = '"\\/bfnrt';

    private const IN_OBJECT = 0;
    private const IN_ARRAY = 1;

    private string $text;
    private int $length;
    private int $at = 0;

    private function __construct(string $text)
    {
        $this->text = $text;
        $this->length = strlen($text);
    }

    /** Check a JSON document and, if it is broken, say exactly where. */
    public static function locate(string $text): JsonLocation
    {
        $scanner = new self($text);

        try {
            $scanner->scan();
            $scanner->skipWhitespace();
            if ($scanner->at < $scanner->length) {
                $scanner->fail(
                    'unexpected ' . $scanner->describe($scanner->at) . ' after the end of the document',
                    $scanner->at
                );
            }
        } catch (ScanError $error) {
            return self::report($text, $error);
        }

        return new JsonLocation(true);
    }

    private static function report(string $text, ScanError $error): JsonLocation
    {
        $at = max(0, min($error->at, strlen($text)));
        $before = substr($text, 0, $at);

        $line = substr_count($before, "\n") + 1;
        $lineStart = strrpos($before, "\n");
        $lineStart = $lineStart === false ? 0 : $lineStart + 1;

        $lines = explode("\n", $text);

        return new JsonLocation(
            valid: false,
            error: $error->getMessage(),
            position: self::characters($before),
            line: $line,
            column: self::characters(substr($text, $lineStart, $at - $lineStart)) + 1,
            excerpt: $lines[$line - 1] ?? '',
        );
    }

    /** How many characters a run of UTF-8 bytes spells. */
    private static function characters(string $text): int
    {
        // Every byte that is not a continuation byte starts a character.
        return strlen($text) - strlen(preg_replace('/[^\x80-\xBF]/', '', $text) ?? '');
    }

    /** @throws ScanError */
    private function fail(string $message, int $at): never
    {
        throw new ScanError($message, $at);
    }

    private function describe(int $at): string
    {
        if ($at >= $this->length) {
            return 'the document ends here';
        }

        $byte = ord($this->text[$at]);
        if ($byte < 32) {
            return sprintf('an unescaped control character (0x%02x)', $byte);
        }

        // A multi-byte character is reported whole rather than as its lead byte.
        $width = 1;
        if ($byte >= 0xF0) {
            $width = 4;
        } elseif ($byte >= 0xE0) {
            $width = 3;
        } elseif ($byte >= 0xC0) {
            $width = 2;
        }

        return '"' . substr($this->text, $at, $width) . '"';
    }

    private function skipWhitespace(): void
    {
        $this->at += strspn($this->text, self::WHITESPACE, $this->at);
    }

    private function peek(): string
    {
        return $this->at < $this->length ? $this->text[$this->at] : '';
    }

    private function expect(string $character, string $what): void
    {
        if ($this->at >= $this->length || $this->text[$this->at] !== $character) {
            $this->fail('expected ' . $what . ', found ' . $this->describe($this->at), $this->at);
        }
        $this->at++;
    }

    private function starts(string $word): bool
    {
        return substr_compare($this->text, $word, $this->at, strlen($word)) === 0;
    }

    /** Walk one complete JSON value. */
    private function scan(): void
    {
        /** @var list<int> $stack */
        $stack = [];

        // Three states: about to read a value, about to read a property name, or
        // having just finished a value and deciding what comes next.
        $state = 'value';

        for (;;) {
            if ($state === 'value') {
                $this->skipWhitespace();
                if ($this->at >= $this->length) {
                    $this->fail('expected a value, but the document ends here', $this->at);
                }

                $character = $this->text[$this->at];

                if ($character === '{' || $character === '[') {
                    $closer = $character === '{' ? '}' : ']';
                    $this->at++;
                    $this->skipWhitespace();
                    if ($this->peek() === $closer) {
                        $this->at++;
                        $state = 'after';
                        continue;
                    }
                    $stack[] = $character === '{' ? self::IN_OBJECT : self::IN_ARRAY;
                    if (count($stack) > self::MAX_DEPTH) {
                        $this->fail('the document is nested too deeply to check', $this->at);
                    }
                    $state = $character === '{' ? 'key' : 'value';
                    continue;
                }

                if ($character === '"') {
                    $this->scanString();
                } elseif ($character === '-' || strpos(self::DIGITS, $character) !== false) {
                    $this->scanNumber();
                } elseif ($this->starts('true')) {
                    $this->at += 4;
                } elseif ($this->starts('false')) {
                    $this->at += 5;
                } elseif ($this->starts('null')) {
                    $this->at += 4;
                    // The mistakes that account for most hand-edited JSON.
                } elseif ($character === "'") {
                    $this->fail('strings must use double quotes, not single quotes', $this->at);
                } elseif ($this->starts('True') || $this->starts('False')) {
                    $this->fail('true and false must be lower case in JSON', $this->at);
                } elseif ($this->starts('TRUE') || $this->starts('FALSE')) {
                    $this->fail('true and false must be lower case in JSON', $this->at);
                } elseif ($this->starts('None') || $this->starts('NULL') || $this->starts('undefined')) {
                    $this->fail('JSON has no None, NULL or undefined - use null', $this->at);
                } elseif ($this->starts('NaN') || $this->starts('Infinity') || $this->starts('-Infinity')) {
                    $this->fail('JSON has no NaN or Infinity - use null', $this->at);
                } else {
                    $this->fail('expected a value, found ' . $this->describe($this->at), $this->at);
                }

                $state = 'after';
                continue;
            }

            if ($state === 'key') {
                $this->skipWhitespace();
                if ($this->peek() !== '"') {
                    if ($this->peek() === "'") {
                        $this->fail('strings must use double quotes, not single quotes', $this->at);
                    }
                    $this->fail(
                        'expected a quoted property name, found ' . $this->describe($this->at),
                        $this->at
                    );
                }
                $this->scanString();
                $this->skipWhitespace();
                $this->expect(':', 'a colon after the property name');
                $state = 'value';
                continue;
            }

            // $state === 'after'
            if ($stack === []) {
                return;
            }

            $inObject = $stack[count($stack) - 1] === self::IN_OBJECT;
            $closer = $inObject ? '}' : ']';

            $this->skipWhitespace();
            if ($this->at >= $this->length) {
                $this->fail(
                    $inObject ? 'this object is never closed' : 'this array is never closed',
                    $this->at
                );
            }

            $character = $this->text[$this->at];
            if ($character === ',') {
                $commaAt = $this->at;
                $this->at++;
                $this->skipWhitespace();
                if ($this->peek() === $closer) {
                    $this->fail('a trailing comma is not allowed in JSON', $commaAt);
                }
                $state = $inObject ? 'key' : 'value';
                continue;
            }
            if ($character === $closer) {
                $this->at++;
                array_pop($stack);
                continue;
            }

            $this->fail(
                'expected a comma or a closing ' . ($inObject ? 'brace' : 'bracket')
                . ', found ' . $this->describe($this->at),
                $this->at
            );
        }
    }

    private function scanString(): void
    {
        $text = $this->text;
        $length = $this->length;
        $start = $this->at;
        $at = $this->at + 1; // the opening quote

        while ($at < $length) {
            $character = $text[$at];

            if ($character === '"') {
                $this->at = $at + 1;

                return;
            }

            if ($character === '\\') {
                $at++;
                if ($at >= $length) {
                    break;
                }
                $escape = $text[$at];
                if (strpos(self::SIMPLE_ESCAPES, $escape) !== false) {
                    $at++;
                    continue;
                }
                if ($escape === 'u') {
                    $digits = substr($text, $at + 1, 4);
                    if (strlen($digits) < 4 || strspn($digits, self::HEX) < 4) {
                        $this->fail('\\u must be followed by four hexadecimal digits', $at - 1);
                    }
                    $at += 5;
                    continue;
                }
                $this->fail('"\\' . $escape . '" is not a valid escape sequence', $at - 1);
            }

            if (ord($character) < 32) {
                $this->fail(
                    'a raw control character inside a string - escape it as '
                    . sprintf('\\u%04x', ord($character)),
                    $at
                );
            }

            $at++;
        }

        $this->fail('this string is never closed', $start);
    }

    private function scanNumber(): void
    {
        $text = $this->text;
        $length = $this->length;
        $start = $this->at;
        $at = $this->at;

        if ($text[$at] === '-') {
            $at++;
        }

        if ($at >= $length) {
            $this->fail('this number has no digits', $start);
        }

        if ($text[$at] === '0') {
            $at++;
            if ($at < $length && strpos(self::DIGITS, $text[$at]) !== false) {
                $this->fail('a number may not have a leading zero', $start);
            }
        } elseif (strpos(self::DIGITS, $text[$at]) !== false) {
            $at += strspn($text, self::DIGITS, $at);
        } else {
            $this->at = $at;
            $this->fail('expected a digit, found ' . $this->describe($at), $at);
        }

        if ($at < $length && $text[$at] === '.') {
            $at++;
            $digits = strspn($text, self::DIGITS, $at);
            if ($digits === 0) {
                $this->fail('a decimal point must be followed by at least one digit', $at - 1);
            }
            $at += $digits;
        }

        if ($at < $length && ($text[$at] === 'e' || $text[$at] === 'E')) {
            $at++;
            if ($at < $length && ($text[$at] === '+' || $text[$at] === '-')) {
                $at++;
            }
            $digits = strspn($text, self::DIGITS, $at);
            if ($digits === 0) {
                $this->fail('an exponent must have at least one digit', $at - 1);
            }
            $at += $digits;
        }

        $this->at = $at;
    }
}
