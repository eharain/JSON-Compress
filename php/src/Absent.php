<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * "This row has no value for this column at all."
 *
 * A distinct marker rather than a distinguished value, because every value a
 * cell might hold is one a document may legitimately contain: null is a
 * present null, the string "~" is a hole only after encoding, and -1 is an
 * ordinary number in a raw column. Carrying absence as its own object keeps a
 * hole and a value that looks like one from ever being confused.
 *
 * @internal
 */
final class Absent
{
    private static ?self $instance = null;

    private function __construct()
    {
    }

    /** The one and only marker, so identity comparison is enough to spot it. */
    public static function it(): self
    {
        return self::$instance ??= new self();
    }
}
