<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * An array the analysis pass decided to write as a table, with its cells
 * already gathered.
 *
 * The encoder runs in two passes - count everything, then write it - and the
 * second pass has to know which arrays the first pass chose to pack. The other
 * implementations keep that in a map keyed by the array's identity, which PHP
 * cannot do: arrays here are values, not references, so there is no identity to
 * key on.
 *
 * So the analysis pass hands its work forward instead. It returns a tree in
 * which each packed array has been replaced by one of these, holding the plan
 * and the cells in the order the rows will be written. The build pass then
 * needs no lookup at all - it can see what a node is from its type - and no
 * assumption that the two passes walk the document in the same order.
 *
 * @internal
 */
final class PlannedTable
{
    /**
     * @param TablePlan          $plan
     * @param list<list<mixed>>  $rows     One list per row, holding a cell for each
     *                                     non-constant column in column order.
     *                                     {@see Absent} marks a missing key.
     * @param int                $rowCount How many rows the table has, which is all
     *                                     that survives when every column is constant.
     */
    public function __construct(
        public readonly TablePlan $plan,
        public readonly array $rows,
        public readonly int $rowCount,
    ) {
    }
}
