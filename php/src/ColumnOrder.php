<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use SplMinHeap;
use stdClass;

/**
 * Choosing the column order for a table.
 *
 * A table has one column list, but the rows that feed it each have their own
 * key order. The decoder rebuilds every row by walking the columns, so the
 * order it produces is only faithful if each row's keys appear in the column
 * list in the same sequence they appeared in the row.
 *
 * That is a topological sort. Every row contributes the constraint "this key
 * came before that one", and any linear extension of those constraints
 * reproduces every row exactly. Where the constraints contradict each other -
 * one row says name before id, another says id before name - no column order
 * can satisfy both, and the array is left as ordinary objects rather than
 * quietly reordered.
 *
 * Ties are broken by first appearance, so the same input always produces the
 * same output.
 *
 * @internal
 */
final class ColumnOrder
{
    private function __construct()
    {
    }

    /**
     * Work out a column order that reproduces every row's key order.
     *
     * @param list<stdClass> $rows
     *
     * @return list<string>|null The columns, or null when no order satisfies every row.
     */
    public static function order(array $rows): ?array
    {
        /** @var array<array-key,int> $position */
        $position = [];
        /** @var list<string> $columns */
        $columns = [];

        // First pass: collect the keys, in the order the document first mentions
        // them. That order is the tie-break, and gives a stable result.
        foreach ($rows as $row) {
            foreach ($row as $key => $ignored) {
                if (!isset($position[$key])) {
                    $position[$key] = count($columns);
                    $columns[] = (string) $key;
                }
            }
        }

        $count = count($columns);
        if ($count <= 1) {
            return $columns;
        }

        /** @var list<array<int,true>> $after */
        $after = array_fill(0, $count, []);
        $incoming = array_fill(0, $count, 0);

        // Second pass: one edge per adjacent pair of keys in each row.
        // Transitivity along the chain covers the rest of the row's ordering.
        $sawEdge = false;
        foreach ($rows as $row) {
            $previous = -1;
            foreach ($row as $key => $ignored) {
                $current = $position[$key];
                if ($previous !== -1 && !isset($after[$previous][$current])) {
                    $after[$previous][$current] = true;
                    $incoming[$current]++;
                    $sawEdge = true;
                }
                $previous = $current;
            }
        }

        // Every row was a single key or empty: nothing constrains the order.
        if (!$sawEdge) {
            return $columns;
        }

        // A heap rather than a queue, so ties break towards the key the document
        // mentioned first however many columns there are.
        $ready = new SplMinHeap();
        for ($at = 0; $at < $count; $at++) {
            if ($incoming[$at] === 0) {
                $ready->insert($at);
            }
        }

        /** @var list<string> $ordered */
        $ordered = [];
        while (!$ready->isEmpty()) {
            $at = $ready->extract();
            $ordered[] = $columns[$at];
            foreach ($after[$at] as $following => $ignored) {
                if (--$incoming[$following] === 0) {
                    $ready->insert($following);
                }
            }
        }

        // Fewer keys placed than we started with means the constraints form a
        // cycle: two rows disagree about which key comes first.
        return count($ordered) === $count ? $ordered : null;
    }
}
