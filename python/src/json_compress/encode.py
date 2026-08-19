"""The encoder.

Two passes. The first walks the document and counts things: how often each key
is used, how often each string value appears, and which arrays are worth turning
into tables. The second builds the output using catalogues ordered by those
counts, so the most-used key gets the one-character token.

Splitting it this way costs a second walk and buys a materially smaller
document, because token length is decided by frequency rather than by the
accident of which key the encoder happened to meet first.
"""

from __future__ import annotations

from typing import Any, Dict, List, Mapping, Optional, Sequence, Union

from .constants import (
    COL_CONST,
    COL_DICT,
    DICT_HOLE,
    ENV_DATA,
    ENV_KEYS,
    ENV_STRINGS,
    ENV_VERSION,
    FORMAT_VERSION,
    HOLE,
    T_COLUMNS,
    T_CONSTS,
    T_ENCODINGS,
    T_ROWCOUNT,
    T_ROWS,
)
from .jsontext import utf16_length
from .normalize import normalize
from .options import CompressOptions, resolve_options
from .table import TablePlan, plan_table
from .tokens import base62_length, to_base62, to_key_token

__all__ = ["compress"]

_MISSING = object()


class _Analysis:
    """What the first pass learned about the document."""

    __slots__ = ("options", "key_counts", "string_counts", "plans", "forced_strings")

    def __init__(self, options: CompressOptions) -> None:
        self.options = options
        self.key_counts: Dict[str, int] = {}
        self.string_counts: Dict[str, int] = {}
        # Keyed by id(), because a list is not hashable and the source tree is
        # held alive for the whole of compress().
        self.plans: Dict[int, Optional[TablePlan]] = {}
        self.forced_strings: set = set()


# --------------------------------------------------------------------------- #
# Pass one - analysis
# --------------------------------------------------------------------------- #


def _analyse_value(state: _Analysis, value: Any) -> None:
    kind = type(value)
    if kind is str:
        if state.options.strings:
            counts = state.string_counts
            counts[value] = counts.get(value, 0) + 1
        return
    if kind is list:
        plan = plan_table(value, state.options)
        state.plans[id(value)] = plan
        if plan is not None:
            _analyse_table(state, value, plan)
        else:
            for item in value:
                _analyse_value(state, item)
        return
    if kind is dict:
        counts = state.key_counts
        for key, item in value.items():
            counts[key] = counts.get(key, 0) + 1
            _analyse_value(state, item)


def _analyse_table(state: _Analysis, rows: Sequence[dict], plan: TablePlan) -> None:
    columns = plan.columns
    encodings = plan.encodings

    # A table spells each key out once, in the column list, however many rows it
    # has - so it contributes one to the key frequency, not one per row.
    counts = state.key_counts
    for key in columns:
        counts[key] = counts.get(key, 0) + 1
    for value in plan.constants:
        _analyse_value(state, value)

    string_counts = state.string_counts
    catalogue_strings = state.options.strings

    for row in rows:
        for at, encoding in enumerate(encodings):
            if encoding == COL_CONST:
                continue
            value = row.get(columns[at], _MISSING)
            if value is _MISSING:
                continue
            if encoding == COL_DICT:
                if type(value) is str:
                    if catalogue_strings:
                        string_counts[value] = string_counts.get(value, 0) + 1
                    state.forced_strings.add(value)
            else:
                _analyse_value(state, value)


# --------------------------------------------------------------------------- #
# Catalogues
# --------------------------------------------------------------------------- #


class _Catalogue:
    __slots__ = ("values", "index", "tokens")

    def __init__(self) -> None:
        self.values: List[str] = []
        self.index: Dict[str, int] = {}
        self.tokens: Dict[str, str] = {}


def _build_key_catalogue(state: _Analysis) -> _Catalogue:
    """Most-used key first, so the busiest keys get the shortest tokens."""
    counts = state.key_counts
    catalogue = _Catalogue()
    # The sort is stable and the counts are in first-seen order, so keys used
    # equally often keep the order the document introduced them in.
    for at, key in enumerate(sorted(counts, key=lambda name: -counts[name])):
        catalogue.values.append(key)
        catalogue.index[key] = at
        catalogue.tokens[key] = to_key_token(at)
    return catalogue


def _build_string_catalogue(state: _Analysis) -> _Catalogue:
    """Choose which string values are worth moving into the catalogue.

    A catalogued string is written once in the catalogue and referenced by a
    short tilde token everywhere it occurs, so the trade is::

        inline    = count * (length + 2)                  # quotes on every copy
        catalogue = count * (token_length + 3) + length + 3

    Candidates are considered in frequency order, so the token length used in
    that comparison is the one the string would really be given.

    Values pulled into a dictionary column are forced in regardless: their cells
    are bare integers, which is cheaper still, and the comparison above does not
    describe them.
    """
    catalogue = _Catalogue()
    if not state.options.strings:
        return catalogue

    min_count = state.options.min_string_count
    min_length = state.options.min_string_length
    forced_values = state.forced_strings

    candidates = []
    for value, count in state.string_counts.items():
        forced = value in forced_values
        length = utf16_length(value)
        if not forced:
            if count < min_count:
                continue
            if length < min_length:
                continue
            # A value that already begins with a tilde is escaped inline and can
            # never be written as a reference, so cataloguing it would only burn
            # an index.
            if value.startswith(HOLE):
                continue
        candidates.append((value, count, forced, length))

    candidates.sort(key=lambda item: (-item[1], -item[3]))

    for value, count, forced, length in candidates:
        if not forced:
            token_length = base62_length(len(catalogue.values))
            inline = count * (length + 2)
            catalogued = count * (token_length + 3) + length + 3
            if catalogued >= inline:
                continue
        catalogue.index[value] = len(catalogue.values)
        catalogue.values.append(value)

    return catalogue


# --------------------------------------------------------------------------- #
# Pass two - build
# --------------------------------------------------------------------------- #


class _Context:
    __slots__ = ("keys", "strings", "plans")

    def __init__(self, keys: _Catalogue, strings: _Catalogue, plans: dict) -> None:
        self.keys = keys
        self.strings = strings
        self.plans = plans


def _encode_string(value: str, strings: _Catalogue) -> str:
    # Escape first: an escaped literal always starts with two tildes, a reference
    # is a tilde followed by a base-62 digit, and a lone tilde is the row hole.
    # Those three spaces cannot overlap.
    if value.startswith(HOLE):
        return HOLE + value
    at = strings.index.get(value)
    return value if at is None else HOLE + to_base62(at)


def _encode_value(value: Any, ctx: _Context) -> Any:
    kind = type(value)
    if kind is str:
        return _encode_string(value, ctx.strings)
    if kind is list:
        plan = ctx.plans.get(id(value))
        if plan is not None:
            return _encode_table(value, plan, ctx)
        return [_encode_value(item, ctx) for item in value]
    if kind is dict:
        tokens = ctx.keys.tokens
        return {tokens[key]: _encode_value(item, ctx) for key, item in value.items()}
    return value


def _encode_table(rows: Sequence[dict], plan: TablePlan, ctx: _Context) -> dict:
    columns = plan.columns
    encodings = plan.encodings

    node: Dict[str, Any] = {}
    node[T_COLUMNS] = [ctx.keys.index[key] for key in columns]

    if any(encoding != 0 for encoding in encodings):
        node[T_ENCODINGS] = list(encodings)

    if plan.constants:
        node[T_CONSTS] = [_encode_value(value, ctx) for value in plan.constants]

    if plan.variable_count == 0:
        # Every column turned out to be constant, so the rows carry no data at
        # all and only their number survives.
        node[T_ROWCOUNT] = len(rows)
        return node

    string_index = ctx.strings.index
    matrix = []

    for row in rows:
        cells: List[Any] = []
        # Whether a cell is a hole is tracked rather than inferred from its
        # value, because a raw column may legitimately hold the number -1, which
        # is what a dictionary column writes for an absent key.
        is_hole: List[bool] = []
        for at, encoding in enumerate(encodings):
            if encoding == COL_CONST:
                continue
            value = row.get(columns[at], _MISSING)
            missing = value is _MISSING
            if encoding == COL_DICT:
                if missing:
                    cells.append(DICT_HOLE)
                elif value is None:
                    cells.append(None)
                else:
                    cells.append(string_index[value])
            else:
                cells.append(HOLE if missing else _encode_value(value, ctx))
            is_hole.append(missing)

        # Trailing gaps need not be written down; a short row is understood to
        # be missing everything past its end.
        end = len(cells)
        while end > 0 and is_hole[end - 1]:
            end -= 1
        matrix.append(cells if end == len(cells) else cells[:end])

    node[T_ROWS] = matrix
    return node


# --------------------------------------------------------------------------- #
# Entry point
# --------------------------------------------------------------------------- #


def compress(
    value: Any,
    options: Union[CompressOptions, Mapping[str, Any], None] = None,
    **overrides: Any,
) -> Dict[str, Any]:
    """Compress a value into a json-compress envelope.

    The result is an ordinary JSON-serialisable dict, so it can be stored,
    posted or logged anywhere plain JSON goes.
    """
    resolved = resolve_options(options, overrides)
    source = normalize(value)

    state = _Analysis(resolved)
    _analyse_value(state, source)

    keys = _build_key_catalogue(state)
    strings = _build_string_catalogue(state)

    ctx = _Context(keys, strings, state.plans)
    data = _encode_value(source, ctx)

    envelope: Dict[str, Any] = {ENV_VERSION: FORMAT_VERSION, ENV_KEYS: keys.values}
    if strings.values:
        envelope[ENV_STRINGS] = strings.values
    envelope[ENV_DATA] = data
    return envelope
