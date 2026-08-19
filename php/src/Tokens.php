<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

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
 * order, so a key of "10" would jump ahead of "2". Encoding a key as "7" would
 * therefore quietly shuffle the document for a JavaScript reader. PHP has the
 * same trap one step earlier: an array subscript of "7" becomes the integer 7
 * on the way in. Key tokens instead start with a letter and continue in
 * base-62, which is never index-like: 52 keys still cost one character, the
 * next 3,224 cost two, and the original key order survives the round trip
 * exactly.
 *
 * @internal
 */
final class Tokens
{
    public const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
    public const LETTERS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';

    /** @var array<string,int>|null Character to value, built once on first use. */
    private static ?array $digitValues = null;

    /** @var array<string,int>|null */
    private static ?array $letterValues = null;

    private function __construct()
    {
    }

    /* ------------------------------------------------------------------ *
     * Base-62, for string references
     * ------------------------------------------------------------------ */

    /** Encode a non-negative integer as a base-62 token. */
    public static function toBase62(int $n): string
    {
        if ($n < 62) {
            return self::DIGITS[$n];
        }

        $out = '';
        $value = $n;
        while ($value > 0) {
            $out = self::DIGITS[$value % 62] . $out;
            $value = intdiv($value, 62);
        }

        return $out;
    }

    /** Decode a base-62 token, or return -1 if it is not one. */
    public static function fromBase62(string $token): int
    {
        $length = strlen($token);
        if ($length === 0) {
            return -1;
        }

        $values = self::$digitValues ??= array_flip(str_split(self::DIGITS));

        $n = 0;
        for ($at = 0; $at < $length; $at++) {
            $digit = $values[$token[$at]] ?? null;
            if ($digit === null) {
                return -1;
            }
            $n = $n * 62 + $digit;
        }

        return $n;
    }

    /** Length in characters of the base-62 token for a given index, without building it. */
    public static function base62Length(int $n): int
    {
        if ($n < 62) {
            return 1;
        }
        if ($n < 3844) {
            return 2;
        }
        if ($n < 238328) {
            return 3;
        }

        $length = 0;
        $value = $n;
        while ($value > 0) {
            $length++;
            $value = intdiv($value, 62);
        }

        return $length;
    }

    /* ------------------------------------------------------------------ *
     * Letter-first tokens, for object keys
     * ------------------------------------------------------------------ */

    /**
     * Encode a non-negative integer as a key token: a letter, then base-62 digits.
     *
     * The numbering is bijective - each token length covers a contiguous block
     * of indices and no index has two spellings - so {@see Tokens::fromKeyToken}
     * inverts it exactly.
     */
    public static function toKeyToken(int $n): string
    {
        if ($n < 52) {
            return self::LETTERS[$n];
        }

        // Find the block this index falls in: 52 one-character tokens, then
        // 52 * 62 two-character ones, and so on.
        $start = 0;
        $size = 52;
        $length = 1;
        while ($n >= $start + $size) {
            $start += $size;
            $size *= 62;
            $length++;
        }

        $value = $n - $start;
        $tail = '';
        for ($at = 1; $at < $length; $at++) {
            $tail = self::DIGITS[$value % 62] . $tail;
            $value = intdiv($value, 62);
        }

        return self::LETTERS[$value] . $tail;
    }

    /** Decode a key token, or return -1 if it is not one. */
    public static function fromKeyToken(string $token): int
    {
        $length = strlen($token);
        if ($length === 0) {
            return -1;
        }

        $letters = self::$letterValues ??= array_flip(str_split(self::LETTERS));
        $digits = self::$digitValues ??= array_flip(str_split(self::DIGITS));

        $n = $letters[$token[0]] ?? null;
        if ($n === null) {
            return -1;
        }

        for ($at = 1; $at < $length; $at++) {
            $digit = $digits[$token[$at]] ?? null;
            if ($digit === null) {
                return -1;
            }
            $n = $n * 62 + $digit;
        }

        $start = 0;
        $size = 52;
        for ($at = 1; $at < $length; $at++) {
            $start += $size;
            $size *= 62;
        }

        return $start + $n;
    }
}
