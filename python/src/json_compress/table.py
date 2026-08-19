"""Columnar table planning.

An array of like-shaped objects is the single most wasteful thing JSON does:
every key is spelled out again on every row. This module decides, per array,
whether it is worth turning into a table - a column list plus a rows matrix -
and how each individual column should be encoded.

The decision is taken once, during the analysis pass, and cached against the
array's identity so the build pass does not have to work it out again.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, List, Optional, Sequence

from .column_order import order_columns
from .constants import COL_CONST, COL_DICT, COL_RAW
from .jsontext import utf16_length
from .options import CompressOptions

__all__ = ["TablePlan", "plan_table"]

_MISSING = object()


@dataclass(frozen=True)
class TablePlan:
    """How one array is to be written as a table."""

    #: Every column, in an order that reproduces each row's key order.
    columns: List[str]
    #: One of ``COL_RAW`` / ``COL_DICT`` / ``COL_CONST`` per column.
    encodings: List[int]
    #: Values of the ``COL_CONST`` columns, in column order.
    constants: List[Any]
    #: How many columns actually appear in the rows.
    variable_count: int


def _is_primitive(value: Any) -> bool:
    """True for a value that can safely be hoisted out of the rows and shared."""
    return value is None or isinstance(value, (str, int, float, bool))


def _same(a: Any, b: Any) -> bool:
    """True when two cells would be written down identically.

    Deliberately stricter than ``==``: ``True`` is not ``1``, and an integer is
    not the float that happens to equal it, because the two do not always
    serialise the same way and a constant column writes one value for all.
    """
    if a is b:
        return True
    if type(a) is not type(b):
        return False
    if isinstance(a, (str, int, float)):
        return a == b
    return False


class _ColumnStats:
    __slots__ = (
        "present",
        "strings",
        "only_strings",
        "constant",
        "constant_value",
        "total_length",
        "distinct",
    )

    def __init__(self) -> None:
        self.present = 0
        self.strings = 0
        self.only_strings = True
        self.constant = True
        self.constant_value: Any = _MISSING
        self.total_length = 0
        self.distinct: set = set()


def plan_table(array: Sequence[Any], options: CompressOptions) -> Optional[TablePlan]:
    """Decide how to encode an array, or return ``None`` to leave it alone."""
    if not options.tables:
        return None

    row_count = len(array)
    if row_count < options.min_rows:
        return None

    for row in array:
        if type(row) is not dict:
            return None

    # One column order has to stand in for every row's key order. When no such
    # order exists the array is left as objects, which is larger on the wire but
    # hands the document back exactly as it arrived.
    columns = order_columns(array)
    if columns is None:
        return None

    column_count = len(columns)
    if column_count == 0:
        return None

    # One pass over the cells, gathering everything the encoding choice needs.
    stats = [_ColumnStats() for _ in range(column_count)]

    holes = 0
    for row in array:
        for at in range(column_count):
            stat = stats[at]
            value = row.get(columns[at], _MISSING)
            if value is _MISSING:
                holes += 1
                stat.constant = False
                continue
            stat.present += 1
            if stat.present == 1:
                stat.constant_value = value
            elif stat.constant and not _same(stat.constant_value, value):
                stat.constant = False
            if type(value) is str:
                stat.strings += 1
                stat.total_length += utf16_length(value)
                stat.distinct.add(value)
            elif value is None:
                pass
            else:
                stat.only_strings = False

    if holes / (row_count * column_count) > options.max_hole_ratio:
        return None

    encodings: List[int] = []
    constants: List[Any] = []
    variable_count = 0

    for stat in stats:
        is_constant = (
            options.constants
            and stat.constant
            and stat.present == row_count
            and _is_primitive(stat.constant_value)
        )

        if is_constant:
            encodings.append(COL_CONST)
            constants.append(stat.constant_value)
            continue

        variable_count += 1

        # A dictionary column pays for itself when the same handful of strings
        # keep coming back: each cell shrinks to a small integer, and the strings
        # move into the catalogue where they are written exactly once.
        worth_dictionary = (
            options.dict_columns
            and options.strings
            and stat.only_strings
            and stat.strings >= 3
            and len(stat.distinct) > 0
            and len(stat.distinct) <= stat.strings * 0.6
            and stat.total_length / stat.strings >= 2
        )

        encodings.append(COL_DICT if worth_dictionary else COL_RAW)

    return TablePlan(columns, encodings, constants, variable_count)
