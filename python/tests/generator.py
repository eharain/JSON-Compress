"""The shared document generator.

Kept character for character with the one in ``javascript/tests/fuzz.test.js``,
so a seed means the same document in every language that reproduces it. That is
what lets the conformance corpus be described by its seeds rather than only by
its contents, and what makes a fuzz failure reproducible across ports.
"""

from __future__ import annotations

from typing import Any, Callable, List

WORDS: List[str] = [
    "id", "name", "status", "created", "updated", "owner", "tags", "value",
    "count", "total", "items", "meta", "type", "code", "label", "notes",
    "~tilde", "0", "1", "10", "a", "z", "A", "~t", "~r", "toString",
    "__proto__", "constructor", "hasOwnProperty", "", " ", "ключ", "键",
]

STRINGS: List[str] = [
    "", "a", "active", "pending", "cancelled", "~", "~~", "~0", "~zz",
    "a longer value that repeats across many records",
    'quote " backslash \\ newline \n tab \t',
    "\U0001F600 emoji", "مرحبا", "null", "true", "0",
]


def create_random(seed: int) -> Callable[[], float]:
    """A small deterministic generator, so failures can be replayed."""
    state = seed & 0xFFFFFFFF

    def next_value() -> float:
        nonlocal state
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        return state / 4294967296

    return next_value


def pick(random: Callable[[], float], values: List[Any]) -> Any:
    return values[int(random() * len(values))]


def make_scalar(random: Callable[[], float]) -> Any:
    roll = random()
    if roll < 0.2:
        return None
    if roll < 0.3:
        return random() < 0.5
    if roll < 0.55:
        return int(random() * 2000) - 1000
    if roll < 0.62:
        return random() * 1e6 - 5e5
    return pick(random, STRINGS)


def make_value(random: Callable[[], float], depth: int) -> Any:
    if depth <= 0:
        return make_scalar(random)

    roll = random()

    if roll < 0.45:
        return make_scalar(random)

    if roll < 0.6:
        length = int(random() * 6)
        return [make_value(random, depth - 1) for _ in range(length)]

    if roll < 0.8:
        # A record set: the shape the format is built for, with ragged rows and
        # low-cardinality columns thrown in on purpose.
        rows = int(random() * 8)
        columns = []
        column_count = 1 + int(random() * 5)
        for _ in range(column_count):
            columns.append(pick(random, WORDS))

        out = []
        for _ in range(rows):
            row: dict = {}
            for column in columns:
                if random() < 0.15:
                    continue
                row[column] = (
                    pick(random, STRINGS)
                    if random() < 0.5
                    else make_value(random, depth - 2)
                )
            out.append(row)
        return out

    size = int(random() * 6)
    out_map: dict = {}
    for _ in range(size):
        out_map[pick(random, WORDS)] = make_value(random, depth - 1)
    return out_map
