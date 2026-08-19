<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * How one array is to be written as a table.
 *
 * @internal
 */
final class TablePlan
{
    /**
     * @param list<string> $columns    Every column, in an order that reproduces each row's key order.
     * @param list<int>    $encodings  One of the Wire::COL_* values per column.
     * @param list<mixed>  $constants  Values of the constant columns, in column order.
     * @param int          $variableCount How many columns actually appear in the rows.
     */
    public function __construct(
        public readonly array $columns,
        public readonly array $encodings,
        public readonly array $constants,
        public readonly int $variableCount,
    ) {
    }
}
