<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use BackedEnum;
use Closure;
use DateTimeInterface;
use InvalidArgumentException;
use JsonSerializable;
use stdClass;
use UnitEnum;

/**
 * Bring arbitrary input down to plain JSON values.
 *
 * The encoder is much simpler if it can assume it is only ever looking at null,
 * booleans, finite numbers, strings, lists and {@see stdClass}. This pass
 * guarantees that, applying the rules a JSON writer applies:
 *
 * - `JsonSerializable::jsonSerialize()` is honoured, which is PHP's answer to
 *   JavaScript's `toJSON`;
 * - a `DateTimeInterface` becomes ISO 8601 text, an enum becomes its backing
 *   value or its name, and any other object becomes its public properties -
 *   the same choices `json_encode` makes;
 * - a PHP array becomes a JSON array when it is a list and a JSON object when
 *   it is not, again as `json_encode` decides it;
 * - non-finite numbers become null, and negative zero becomes zero, because
 *   JSON cannot tell the two apart;
 * - a circular reference is refused rather than allowed to run away.
 *
 * It returns the value it was given whenever nothing had to change, so clean
 * input - which is the common case - costs a walk but very little allocation.
 */
final class Normalizer
{
    /**
     * How deep the walk may go before the input is refused.
     *
     * Twice the ceiling `json_encode` and `json_decode` use by default, so
     * every document PHP itself will parse can be compressed with room to
     * spare, and comfortably inside the depth this library's own reader accepts
     * ({@see Json::MAX_DEPTH}), which has to be larger again because an
     * envelope nests deeper than the document it carries.
     *
     * A limit rather than an open walk because PHP arrays are values, not
     * references: an array cannot contain itself except through an explicit
     * reference, which no depth-free walk could survive.
     */
    public const MAX_DEPTH = 1024;

    private function __construct()
    {
    }

    /** Reduce a value to nulls, booleans, numbers, strings, lists and stdClass. */
    public static function normalize(mixed $value): mixed
    {
        return self::walk($value, [], 0);
    }

    /**
     * @param array<int,true> $seen Object ids on the current path, for cycle detection.
     */
    private static function walk(mixed $value, array $seen, int $depth): mixed
    {
        if ($value === null || is_bool($value) || is_string($value) || is_int($value)) {
            return $value;
        }

        if (is_float($value)) {
            return self::finite($value);
        }

        if ($depth >= self::MAX_DEPTH) {
            throw new InvalidArgumentException(
                'json-compress: the value nests more than ' . self::MAX_DEPTH
                . ' levels deep, or refers to itself'
            );
        }

        if (is_array($value)) {
            return array_is_list($value)
                ? self::walkList($value, $seen, $depth)
                : self::walkMembers($value, $seen, $depth);
        }

        if (is_object($value)) {
            $id = spl_object_id($value);
            if (isset($seen[$id])) {
                throw new InvalidArgumentException(
                    'json-compress: circular structure cannot be compressed'
                );
            }
            $seen[$id] = true;

            if ($value instanceof stdClass) {
                return self::walkMembers(get_object_vars($value), $seen, $depth);
            }
            if ($value instanceof Closure) {
                // Every other object has public properties to fall back on, so
                // this is the one that would quietly become an empty object.
                throw new InvalidArgumentException('json-compress: a closure has no JSON form');
            }
            if ($value instanceof JsonSerializable) {
                return self::walk($value->jsonSerialize(), $seen, $depth);
            }
            if ($value instanceof DateTimeInterface) {
                return $value->format(DateTimeInterface::ATOM);
            }
            if ($value instanceof BackedEnum) {
                return $value->value;
            }
            if ($value instanceof UnitEnum) {
                return $value->name;
            }

            // Anything else contributes its public properties, which is what
            // `json_encode` does with a plain object - and always as an object,
            // however index-like the property names happen to be.
            return self::walkMembers(get_object_vars($value), $seen, $depth);
        }

        throw new InvalidArgumentException(
            'json-compress: a ' . get_debug_type($value) . ' has no JSON form'
        );
    }

    /**
     * @param list<mixed>     $value
     * @param array<int,true> $seen
     */
    private static function walkList(array $value, array $seen, int $depth): array
    {
        $changed = false;
        $out = [];
        foreach ($value as $item) {
            $next = self::walk($item, $seen, $depth + 1);
            if ($next !== $item) {
                $changed = true;
            }
            $out[] = $next;
        }

        return $changed ? $out : $value;
    }

    /**
     * @param array<array-key,mixed> $members
     * @param array<int,true>        $seen
     */
    private static function walkMembers(array $members, array $seen, int $depth): stdClass
    {
        $out = new stdClass();
        foreach ($members as $key => $item) {
            // PHP turns an integer-like subscript into an integer on the way in,
            // and the cast back is exact for every name it does that to.
            $out->{(string) $key} = self::walk($item, $seen, $depth + 1);
        }

        return $out;
    }

    /** JSON cannot spell NaN or either infinity, and cannot tell -0 from 0. */
    private static function finite(float $value): float|null
    {
        if (!is_finite($value)) {
            return null;
        }

        // Negative zero is folded to zero so that the in-memory round trip and
        // the serialised one agree; JSON itself cannot tell them apart.
        return $value == 0 ? 0.0 : $value;
    }
}
