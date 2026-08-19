"""Choosing the column order for a table.

A table has one column list, but the rows that feed it each have their own key
order. The decoder rebuilds every row by walking the columns, so the order it
produces is only faithful if each row's keys appear in the column list in the
same sequence they appeared in the row.

That is a topological sort. Every row contributes the constraint "this key came
before that one", and any linear extension of those constraints reproduces every
row exactly. Where the constraints contradict each other - one row says name
before id, another says id before name - no column order can satisfy both, and
the array is left as ordinary objects rather than quietly reordered.

Ties are broken by first appearance, so the same input always produces the same
output.
"""

from __future__ import annotations

import heapq
from typing import Any, Dict, List, Mapping, Optional, Sequence

__all__ = ["order_columns"]


def order_columns(rows: Sequence[Mapping[str, Any]]) -> Optional[List[str]]:
    """Work out a column order that reproduces every row's key order.

    Returns the columns, or ``None`` when no order can satisfy every row.
    """
    position: Dict[str, int] = {}
    columns: List[str] = []

    # First pass: collect the keys, in the order the document first mentions
    # them. That order is the tie-break, and gives a stable result.
    for row in rows:
        for key in row:
            if key not in position:
                position[key] = len(columns)
                columns.append(key)

    count = len(columns)
    if count <= 1:
        return columns

    after: List[set] = [set() for _ in range(count)]
    incoming = [0] * count

    # Second pass: one edge per adjacent pair of keys in each row. Transitivity
    # along the chain covers the rest of the row's ordering.
    saw_edge = False
    for row in rows:
        previous = -1
        for key in row:
            current = position[key]
            if previous != -1 and current not in after[previous]:
                after[previous].add(current)
                incoming[current] += 1
                saw_edge = True
            previous = current

    # Every row was a single key or empty: nothing constrains the order.
    if not saw_edge:
        return columns

    # A heap rather than a queue, so ties break towards the key the document
    # mentioned first however many columns there are.
    ready = [at for at in range(count) if incoming[at] == 0]
    heapq.heapify(ready)

    ordered: List[str] = []
    while ready:
        at = heapq.heappop(ready)
        ordered.append(columns[at])
        for following in after[at]:
            incoming[following] -= 1
            if incoming[following] == 0:
                heapq.heappush(ready, following)

    # Fewer keys placed than we started with means the constraints form a cycle:
    # two rows disagree about which key comes first.
    return ordered if len(ordered) == count else None
