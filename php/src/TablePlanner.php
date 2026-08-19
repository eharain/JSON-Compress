<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use stdClass;

/**
 * Columnar table planning.
 *
 * An array of like-shaped objects is the single most wasteful thing JSON does:
 * every key is spelled out again on every row. This decides, per array, whether
 * it is worth turning into a table - a column list plus a rows matrix - and how
 * each individual column should be encoded.
 *
 * The decision is taken once, during the analysis pass, and travels forward on
 * a {@see PlannedTable} so the build pass does not have to work it out again.
 *
 * @internal
 */
final class TablePlanner
{
    private function __construct()
    {
    }

    /**
     * Decide how to encode an array, or return null to leave it alone.
     *
     * @param list<mixed> $array
     */
    public static function plan(array $array, CompressOptions $options): ?TablePlan
    {
        if (!$options->tables) {
            return null;
        }

        $rowCount = count($array);
        if ($rowCount < $options->minRows) {
            return null;
        }

        foreach ($array as $row) {
            if (!$row instanceof stdClass) {
                return null;
            }
        }

        /** @var list<stdClass> $array */

        // One column order has to stand in for every row's key order. When no
        // such order exists the array is left as objects, which is larger on the
        // wire but hands the document back exactly as it arrived.
        $columns = ColumnOrder::order($array);
        if ($columns === null) {
            return null;
        }

        $columnCount = count($columns);
        if ($columnCount === 0) {
            return null;
        }

        // One pass over the cells, gathering everything the encoding choice
        // needs. Parallel arrays rather than objects, because this is the
        // hottest loop in the encoder.
        $present = array_fill(0, $columnCount, 0);
        $strings = array_fill(0, $columnCount, 0);
        $onlyStrings = array_fill(0, $columnCount, true);
        $constant = array_fill(0, $columnCount, true);
        $constantValue = array_fill(0, $columnCount, null);
        $totalLength = array_fill(0, $columnCount, 0);
        $distinct = array_fill(0, $columnCount, []);

        $holes = 0;
        foreach ($array as $row) {
            for ($at = 0; $at < $columnCount; $at++) {
                $column = $columns[$at];
                if (!property_exists($row, $column)) {
                    $holes++;
                    $constant[$at] = false;
                    continue;
                }

                $value = $row->{$column};
                $present[$at]++;
                if ($present[$at] === 1) {
                    $constantValue[$at] = $value;
                } elseif ($constant[$at] && !self::same($constantValue[$at], $value)) {
                    $constant[$at] = false;
                }

                if (is_string($value)) {
                    $strings[$at]++;
                    $totalLength[$at] += Json::utf16Length($value);
                    $distinct[$at][$value] = true;
                } elseif ($value !== null) {
                    $onlyStrings[$at] = false;
                }
            }
        }

        if ($holes / ($rowCount * $columnCount) > $options->maxHoleRatio) {
            return null;
        }

        /** @var list<int> $encodings */
        $encodings = [];
        /** @var list<mixed> $constants */
        $constants = [];
        $variableCount = 0;

        for ($at = 0; $at < $columnCount; $at++) {
            $isConstant = $options->constants
                && $constant[$at]
                && $present[$at] === $rowCount
                && self::isPrimitive($constantValue[$at]);

            if ($isConstant) {
                $encodings[] = Wire::COL_CONST;
                $constants[] = $constantValue[$at];
                continue;
            }

            $variableCount++;

            // A dictionary column pays for itself when the same handful of
            // strings keep coming back: each cell shrinks to a small integer,
            // and the strings move into the catalogue where they are written
            // exactly once.
            $distinctCount = count($distinct[$at]);
            $worthDictionary = $options->dictColumns
                && $options->strings
                && $onlyStrings[$at]
                && $strings[$at] >= 3
                && $distinctCount > 0
                && $distinctCount <= $strings[$at] * 0.6
                && $totalLength[$at] / $strings[$at] >= 2;

            $encodings[] = $worthDictionary ? Wire::COL_DICT : Wire::COL_RAW;
        }

        return new TablePlan($columns, $encodings, $constants, $variableCount);
    }

    /** True for a value that can safely be hoisted out of the rows and shared. */
    private static function isPrimitive(mixed $value): bool
    {
        return $value === null || is_scalar($value);
    }

    /**
     * True when two cells would be written down identically.
     *
     * Deliberately stricter than `==`: true is not 1, and an integer is not the
     * float that happens to equal it, because the two do not always serialise
     * the same way and a constant column writes one value for all.
     */
    private static function same(mixed $a, mixed $b): bool
    {
        return $a === $b;
    }
}
