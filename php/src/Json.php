<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use JsonException;
use stdClass;

/**
 * Reading and writing JSON text the way the rest of the format expects.
 *
 * Reading is `json_decode` without the associative flag. That matters: PHP
 * arrays cannot tell an object from a list, and they turn a key of "7" into the
 * integer 7, so decoding into arrays would quietly lose both the
 * empty-object/empty-array distinction and the original key order. Objects
 * therefore arrive as {@see stdClass} and arrays as PHP lists, which is the
 * value model the whole library speaks. The one other departure from the
 * defaults is the depth limit, which is raised for the reason given on
 * {@see Json::MAX_DEPTH}.
 *
 * Writing needs more care. The other implementations of this format agree with
 * each other byte for byte, and two details of `json_encode` would break that:
 *
 * - **Numbers.** PHP prints a float the way `serialize_precision` says to, so
 *   `1.0` comes out as `1.0` and `1e21` as `1.0e+21`. The format follows the
 *   ECMAScript rule instead - `1` and `1e+21` - because that is what every
 *   document in the conformance corpus was written with.
 * - **Line separators.** U+2028 and U+2029 are escaped by default; JavaScript's
 *   `JSON.stringify` leaves them alone.
 *
 * Everything else - which characters are escaped, where the spaces go when
 * indenting - already matches, so `json_encode` writes the strings and this
 * class writes the structure around them.
 */
final class Json
{
    /**
     * Flags that make `json_encode` agree with `JSON.stringify` on a string.
     *
     * Slashes and non-ASCII stay as they are, and so do the two line separators
     * that PHP escapes for the benefit of JavaScript source text rather than of
     * JSON.
     */
    private const STRING_FLAGS = JSON_UNESCAPED_SLASHES
        | JSON_UNESCAPED_UNICODE
        | JSON_UNESCAPED_LINE_TERMINATORS
        | JSON_THROW_ON_ERROR;

    /**
     * How deeply a document may nest before the reader refuses it.
     *
     * Deliberately far above the depth a document may be and still be
     * compressed ({@see Normalizer::MAX_DEPTH}), because an envelope nests
     * deeper than the document it carries: one level for the envelope itself,
     * and one more for every packed table, whose rows matrix is an array of
     * arrays where the source had an array of objects. Measured against the
     * worst shape there is - a chain of two-row tables - an envelope nests
     * half again as deep as its source, so a reader that only matched the
     * encoder's ceiling would refuse envelopes the encoder had just written.
     */
    public const MAX_DEPTH = 2048;

    private function __construct()
    {
    }

    /**
     * Parse JSON text into nulls, booleans, numbers, strings, lists and stdClass.
     *
     * @param int $depth How deeply the document may nest before it is refused.
     *
     * @throws JsonException if the text is not valid JSON.
     */
    public static function decode(string $text, int $depth = self::MAX_DEPTH): mixed
    {
        return json_decode($text, false, $depth, JSON_THROW_ON_ERROR);
    }

    /**
     * Serialise a JSON value, agreeing with `JSON.stringify` character for character.
     *
     * `$indent` follows the same rules as the third argument to
     * `JSON.stringify`: a number of spaces or a literal string, either capped at
     * ten characters, and no indentation at all when it is null or empty.
     *
     * @throws JsonException if the value holds something JSON cannot carry.
     */
    public static function encode(mixed $value, int|string|null $indent = null): string
    {
        if ($indent === null) {
            $pad = '';
        } elseif (is_int($indent)) {
            $pad = str_repeat(' ', max(0, min(10, $indent)));
        } else {
            $pad = substr($indent, 0, 10);
        }

        $out = [];
        self::write($value, $out, $pad, 0);

        return implode('', $out);
    }

    /**
     * Quote and escape one string, exactly as `JSON.stringify` would.
     *
     * @throws JsonException if the string is not valid UTF-8.
     */
    public static function quote(string $text): string
    {
        return json_encode($text, self::STRING_FLAGS);
    }

    /**
     * Length of a string in UTF-16 code units.
     *
     * This is what `String.length` counts in JavaScript, and the encoder's cost
     * model is calibrated against it - so an emoji weighs two, not one, and the
     * same string is catalogued here and there or in neither.
     *
     * PHP strings are UTF-8 bytes, and the count follows from two facts about
     * that encoding without any need to walk it character by character: every
     * byte that is not a continuation byte starts one character, and a character
     * outside the basic plane - the only kind JavaScript holds as a surrogate
     * pair, and so counts twice - is exactly one that starts with a four-byte
     * lead. So one tally of the bytes answers both questions at C speed, and
     * needs no extension to do it.
     */
    public static function utf16Length(string $text): int
    {
        // The overwhelmingly common case: every byte is its own code unit.
        if (!preg_match('/[\x80-\xff]/', $text)) {
            return strlen($text);
        }

        $tally = count_chars($text, 1);

        $continuations = 0;
        for ($byte = 0x80; $byte <= 0xBF; $byte++) {
            $continuations += $tally[$byte] ?? 0;
        }

        $astral = 0;
        for ($byte = 0xF0; $byte <= 0xF7; $byte++) {
            $astral += $tally[$byte] ?? 0;
        }

        return strlen($text) - $continuations + $astral;
    }

    /**
     * Render a number the way ECMAScript's `Number::toString` would.
     *
     * Integers are written out as they stand, at PHP's full 64-bit precision.
     *
     * Floats take more work. PHP and JavaScript agree on the digits - both can
     * produce the shortest decimal that reads back as the same double - and
     * disagree on where the decimal point goes and when to switch to an
     * exponent. So the shortest form is found by asking for one significant
     * digit more until the text reads back as the value it came from, and the
     * ECMAScript placement rules are applied to the digits that come out. Doing
     * it this way also makes the result independent of the `precision` and
     * `serialize_precision` settings in php.ini.
     */
    public static function formatNumber(int|float $value): string
    {
        if (is_int($value)) {
            return (string) $value;
        }

        // Non-finite values have no JSON spelling; JSON writers substitute null.
        if (!is_finite($value)) {
            return 'null';
        }
        if ($value == 0) {
            return '0';
        }

        $sign = '';
        if ($value < 0) {
            $sign = '-';
            $value = -$value;
        }

        $text = '';
        for ($precision = 1; $precision <= 17; $precision++) {
            $text = sprintf('%.*e', $precision - 1, $value);
            if ((float) $text === $value) {
                break;
            }
        }

        // The text is now `d.ddde±X`. Normalise it to the form the ECMAScript
        // rules are written against: value == digits * 10 ** (n - k).
        [$mantissa, $exponent] = explode('e', $text, 2);
        $digits = rtrim(str_replace('.', '', $mantissa), '0');
        if ($digits === '') {
            $digits = '0';
        }

        $n = (int) $exponent + 1;
        $k = strlen($digits);

        if ($k <= $n && $n <= 21) {
            return $sign . $digits . str_repeat('0', $n - $k);
        }
        if (0 < $n && $n <= 21) {
            return $sign . substr($digits, 0, $n) . '.' . substr($digits, $n);
        }
        if (-6 < $n && $n <= 0) {
            return $sign . '0.' . str_repeat('0', -$n) . $digits;
        }

        $tail = $n - 1 >= 0 ? 'e+' . ($n - 1) : 'e-' . (1 - $n);
        if ($k === 1) {
            return $sign . $digits . $tail;
        }

        return $sign . $digits[0] . '.' . substr($digits, 1) . $tail;
    }

    /**
     * Write one value into the output parts.
     *
     * @param list<string> $out
     */
    private static function write(mixed $value, array &$out, string $pad, int $level): void
    {
        if ($value === null) {
            $out[] = 'null';

            return;
        }
        if ($value === true) {
            $out[] = 'true';

            return;
        }
        if ($value === false) {
            $out[] = 'false';

            return;
        }
        if (is_string($value)) {
            $out[] = self::quote($value);

            return;
        }
        if (is_int($value) || is_float($value)) {
            $out[] = self::formatNumber($value);

            return;
        }

        if (is_array($value)) {
            // A PHP array with gaps or names in its keys is an object; only a
            // list is a JSON array. That is the same call `json_encode` makes.
            if (array_is_list($value)) {
                self::writeArray($value, $out, $pad, $level);
            } else {
                self::writeObject($value, $out, $pad, $level);
            }

            return;
        }

        if ($value instanceof stdClass) {
            self::writeObject(get_object_vars($value), $out, $pad, $level);

            return;
        }

        throw new JsonException(
            'json-compress: ' . get_debug_type($value) . ' is not a JSON value - normalise it before writing'
        );
    }

    /**
     * @param list<mixed>  $value
     * @param list<string> $out
     */
    private static function writeArray(array $value, array &$out, string $pad, int $level): void
    {
        if ($value === []) {
            $out[] = '[]';

            return;
        }

        $out[] = '[';
        if ($pad !== '') {
            $inner = "\n" . str_repeat($pad, $level + 1);
            $first = true;
            foreach ($value as $item) {
                $out[] = $first ? $inner : ',' . $inner;
                $first = false;
                self::write($item, $out, $pad, $level + 1);
            }
            $out[] = "\n" . str_repeat($pad, $level) . ']';

            return;
        }

        $first = true;
        foreach ($value as $item) {
            if (!$first) {
                $out[] = ',';
            }
            $first = false;
            self::write($item, $out, $pad, $level);
        }
        $out[] = ']';
    }

    /**
     * @param array<array-key,mixed> $members
     * @param list<string>           $out
     */
    private static function writeObject(array $members, array &$out, string $pad, int $level): void
    {
        if ($members === []) {
            $out[] = '{}';

            return;
        }

        $out[] = '{';
        if ($pad !== '') {
            $inner = "\n" . str_repeat($pad, $level + 1);
            $first = true;
            foreach ($members as $key => $item) {
                $out[] = $first ? $inner : ',' . $inner;
                $first = false;
                $out[] = self::quote((string) $key);
                $out[] = ': ';
                self::write($item, $out, $pad, $level + 1);
            }
            $out[] = "\n" . str_repeat($pad, $level) . '}';

            return;
        }

        $first = true;
        foreach ($members as $key => $item) {
            if (!$first) {
                $out[] = ',';
            }
            $first = false;
            $out[] = self::quote((string) $key);
            $out[] = ':';
            self::write($item, $out, $pad, $level);
        }
        $out[] = '}';
    }
}
