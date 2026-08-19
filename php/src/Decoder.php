<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use stdClass;

/**
 * The decoder.
 *
 * A straight structural walk of the encoded document, resolving catalogue
 * references as it goes. It is deliberately strict: anything that cannot be
 * mapped back onto the original - an out-of-range token, a stray hole marker, a
 * table whose rows do not line up with its columns - raises rather than quietly
 * handing back something that is nearly right.
 *
 * @internal
 */
final class Decoder
{
    /** @var list<string> */
    private array $keys;

    /** @var list<string> */
    private array $strings;

    /**
     * @param list<string> $keys
     * @param list<string> $strings
     */
    private function __construct(array $keys, array $strings)
    {
        $this->keys = $keys;
        $this->strings = $strings;
    }

    /**
     * Restore the original value from a json-compress envelope.
     *
     * @throws DecodeError if the envelope is not a readable json-compress document.
     */
    public static function decompress(mixed $envelope): mixed
    {
        if (!$envelope instanceof stdClass) {
            throw new DecodeError('expected a compressed envelope object');
        }

        if (!property_exists($envelope, Wire::ENV_VERSION)) {
            throw new DecodeError('this is not a compressed document (no "jc" version field)');
        }

        // A number, and the right one. PHP would happily call the string "1"
        // equal to 1, which is not what the format says the field holds.
        $version = $envelope->{Wire::ENV_VERSION};
        if (!is_int($version) && !is_float($version)) {
            throw new DecodeError(
                'the format version is a ' . get_debug_type($version) . ', not a number'
            );
        }
        if ($version != Wire::FORMAT_VERSION) {
            throw new DecodeError(
                'format version ' . self::describe($version)
                . ' is not supported by this build (expected ' . Wire::FORMAT_VERSION . ')'
            );
        }

        $keys = $envelope->{Wire::ENV_KEYS} ?? null;
        if (!is_array($keys)) {
            throw new DecodeError('the key catalogue is missing or is not an array');
        }

        $strings = $envelope->{Wire::ENV_STRINGS} ?? null;
        if ($strings !== null && !is_array($strings)) {
            throw new DecodeError('the string catalogue is not an array');
        }

        /** @var list<string> $keys */
        /** @var list<string> $strings */
        $decoder = new self($keys, $strings ?? []);

        return $decoder->decodeValue($envelope->{Wire::ENV_DATA} ?? null);
    }

    /** True when a value looks like a json-compress envelope this build can read. */
    public static function isCompressed(mixed $value): bool
    {
        if (!$value instanceof stdClass || !property_exists($value, Wire::ENV_VERSION)) {
            return false;
        }

        $version = $value->{Wire::ENV_VERSION};

        return (is_int($version) || is_float($version))
            && $version == Wire::FORMAT_VERSION
            && property_exists($value, Wire::ENV_KEYS)
            && is_array($value->{Wire::ENV_KEYS});
    }

    private function decodeValue(mixed $node): mixed
    {
        if (is_string($node)) {
            return $this->decodeString($node);
        }

        if (is_array($node)) {
            $out = [];
            foreach ($node as $item) {
                $out[] = $this->decodeValue($item);
            }

            return $out;
        }

        if ($node instanceof stdClass) {
            // A table node is identified by its column list, and has to be
            // checked for before the node is read as an ordinary object.
            if (property_exists($node, Wire::T_COLUMNS)) {
                return $this->decodeTable($node);
            }

            $out = new stdClass();
            foreach ($node as $token => $item) {
                $out->{$this->decodeKey((string) $token)} = $this->decodeValue($item);
            }

            return $out;
        }

        return $node;
    }

    private function decodeString(string $value): string
    {
        if ($value === '' || $value[0] !== Wire::HOLE) {
            return $value;
        }
        if ($value === Wire::HOLE) {
            throw new DecodeError('a row hole marker appeared outside a table row');
        }
        if ($value[1] === Wire::HOLE) {
            return substr($value, 1);
        }

        $at = Tokens::fromBase62(substr($value, 1));
        if ($at < 0 || $at >= count($this->strings)) {
            throw new DecodeError('string reference "' . $value . '" is out of range');
        }

        return $this->stringAt($at);
    }

    /** One entry of the string catalogue, which the format says is always a string. */
    private function stringAt(int $at): string
    {
        $value = $this->strings[$at];
        if (!is_string($value)) {
            throw new DecodeError('string catalogue entry ' . $at . ' is not a string');
        }

        return $value;
    }

    private function decodeKey(string $token): string
    {
        $at = Tokens::fromKeyToken($token);
        if ($at < 0 || $at >= count($this->keys)) {
            throw new DecodeError('key token "' . $token . '" is out of range');
        }

        return $this->keyAt($at);
    }

    /** One entry of the key catalogue, which the format says is always a string. */
    private function keyAt(int $at): string
    {
        $key = $this->keys[$at];
        if (!is_string($key)) {
            throw new DecodeError('key catalogue entry ' . $at . ' is not a string');
        }

        return $key;
    }

    /** @return list<stdClass> */
    private function decodeTable(stdClass $node): array
    {
        $columnIndices = $node->{Wire::T_COLUMNS};
        if (!is_array($columnIndices)) {
            throw new DecodeError('table column list is not an array');
        }

        $keyCount = count($this->keys);
        $columns = [];
        foreach ($columnIndices as $at) {
            if (!self::isIndex($at) || $at < 0 || $at >= $keyCount) {
                throw new DecodeError('table column index ' . self::describe($at) . ' is out of range');
            }
            $columns[] = $this->keyAt((int) $at);
        }

        $columnCount = count($columns);

        $encodings = $node->{Wire::T_ENCODINGS} ?? null;
        if ($encodings !== null && (!is_array($encodings) || count($encodings) !== $columnCount)) {
            throw new DecodeError('table encoding list does not match its columns');
        }

        $rawConstants = $node->{Wire::T_CONSTS} ?? [];
        if (!is_array($rawConstants)) {
            throw new DecodeError('table constant list is not an array');
        }
        $constants = [];
        foreach ($rawConstants as $value) {
            $constants[] = $this->decodeValue($value);
        }
        $constantCount = count($constants);

        $matrix = $node->{Wire::T_ROWS} ?? null;
        if ($matrix !== null && !is_array($matrix)) {
            throw new DecodeError('table rows are not an array');
        }

        if ($matrix !== null) {
            $rowCount = count($matrix);
        } else {
            $rawCount = $node->{Wire::T_ROWCOUNT} ?? 0;
            if (!self::isIndex($rawCount) || $rawCount < 0) {
                throw new DecodeError('table row count ' . self::describe($rawCount) . ' is not valid');
            }
            $rowCount = (int) $rawCount;
        }

        $stringCount = count($this->strings);
        $out = [];

        for ($index = 0; $index < $rowCount; $index++) {
            $cells = $matrix === null ? null : $matrix[$index];
            if ($cells !== null && !is_array($cells)) {
                throw new DecodeError('table row ' . $index . ' is not an array');
            }
            $cellCount = $cells === null ? 0 : count($cells);

            $row = new stdClass();
            $constantAt = 0;
            $cellAt = 0;

            for ($at = 0; $at < $columnCount; $at++) {
                // Anything that is not one of the three numbers the format
                // defines is read as a raw column, which is the encoding that
                // can carry every value.
                $encoding = Wire::COL_RAW;
                if ($encodings !== null && self::isIndex($encodings[$at])) {
                    $encoding = (int) $encodings[$at];
                }

                if ($encoding === Wire::COL_CONST) {
                    if ($constantAt >= $constantCount) {
                        throw new DecodeError('table has fewer constants than constant columns');
                    }
                    $row->{$columns[$at]} = $constants[$constantAt];
                    $constantAt++;
                    continue;
                }

                // A row shorter than the column list is missing everything past
                // its end.
                $present = $cellAt < $cellCount;
                $cell = $present ? $cells[$cellAt] : null;
                $cellAt++;

                if ($encoding === Wire::COL_DICT) {
                    if (!$present || (self::isIndex($cell) && $cell == Wire::DICT_HOLE)) {
                        continue;
                    }
                    if ($cell === null) {
                        $row->{$columns[$at]} = null;
                        continue;
                    }
                    if (!self::isIndex($cell) || $cell < 0 || $cell >= $stringCount) {
                        throw new DecodeError('dictionary index ' . self::describe($cell) . ' is out of range');
                    }
                    $row->{$columns[$at]} = $this->stringAt((int) $cell);
                    continue;
                }

                if (!$present || $cell === Wire::HOLE) {
                    continue;
                }
                $row->{$columns[$at]} = $this->decodeValue($cell);
            }

            $out[] = $row;
        }

        return $out;
    }

    /** True for a number that can stand in for an array index. */
    private static function isIndex(mixed $value): bool
    {
        if (is_int($value)) {
            return true;
        }

        return is_float($value) && is_finite($value) && floor($value) === $value;
    }

    /** A value as it should appear in an error message. */
    private static function describe(mixed $value): string
    {
        if ($value === null) {
            return 'null';
        }
        if (is_bool($value)) {
            return $value ? 'true' : 'false';
        }
        if (is_int($value) || is_float($value)) {
            return Json::formatNumber($value);
        }
        if (is_string($value)) {
            return '"' . $value . '"';
        }

        return get_debug_type($value);
    }
}
