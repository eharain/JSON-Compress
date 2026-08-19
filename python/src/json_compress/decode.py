"""The decoder.

A straight structural walk of the encoded document, resolving catalogue
references as it goes. It is deliberately strict: anything that cannot be mapped
back onto the original - an out-of-range token, a stray hole marker, a table
whose rows do not line up with its columns - raises rather than quietly handing
back something that is nearly right.
"""

from __future__ import annotations

from typing import Any, Dict, List, Sequence

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
from .tokens import from_base62, from_key_token

__all__ = ["DecodeError", "decompress", "is_compressed"]

_ABSENT = object()


class DecodeError(ValueError):
    """Raised when a document is not a readable json-compress envelope."""

    def __init__(self, message: str) -> None:
        super().__init__("json-compress: " + message)


class _Context:
    __slots__ = ("keys", "strings")

    def __init__(self, keys: Sequence[str], strings: Sequence[str]) -> None:
        self.keys = keys
        self.strings = strings


def _decode_string(value: str, ctx: _Context) -> str:
    if not value.startswith(HOLE):
        return value
    if len(value) == 1:
        raise DecodeError("a row hole marker appeared outside a table row")
    if value[1] == HOLE:
        return value[1:]

    at = from_base62(value[1:])
    if at < 0 or at >= len(ctx.strings):
        raise DecodeError('string reference "' + value + '" is out of range')
    return ctx.strings[at]


def _decode_key(token: str, ctx: _Context) -> str:
    at = from_key_token(token)
    if at < 0 or at >= len(ctx.keys):
        raise DecodeError('key token "' + token + '" is out of range')
    return ctx.keys[at]


def _decode_value(node: Any, ctx: _Context) -> Any:
    kind = type(node)
    if kind is str:
        return _decode_string(node, ctx)
    if kind is list:
        return [_decode_value(item, ctx) for item in node]
    if kind is dict:
        if T_COLUMNS in node:
            return _decode_table(node, ctx)
        return {
            _decode_key(token, ctx): _decode_value(item, ctx)
            for token, item in node.items()
        }
    return node


def _is_index(value: Any) -> bool:
    if isinstance(value, bool):
        return False
    if isinstance(value, int):
        return True
    return isinstance(value, float) and value.is_integer()


def _decode_table(node: dict, ctx: _Context) -> List[dict]:
    column_indices = node[T_COLUMNS]
    if not isinstance(column_indices, list):
        raise DecodeError("table column list is not an array")

    columns: List[str] = []
    for at in column_indices:
        if not _is_index(at) or at < 0 or at >= len(ctx.keys):
            raise DecodeError("table column index " + str(at) + " is out of range")
        columns.append(ctx.keys[int(at)])

    column_count = len(columns)

    encodings = node.get(T_ENCODINGS)
    if encodings is not None and (
        not isinstance(encodings, list) or len(encodings) != column_count
    ):
        raise DecodeError("table encoding list does not match its columns")

    raw_constants = node.get(T_CONSTS, [])
    if not isinstance(raw_constants, list):
        raise DecodeError("table constant list is not an array")
    constants = [_decode_value(value, ctx) for value in raw_constants]

    matrix = node.get(T_ROWS)
    if matrix is not None and not isinstance(matrix, list):
        raise DecodeError("table rows are not an array")

    if matrix is not None:
        row_count = len(matrix)
    else:
        raw_count = node.get(T_ROWCOUNT, 0)
        if not _is_index(raw_count) or raw_count < 0:
            raise DecodeError("table row count " + str(raw_count) + " is not valid")
        row_count = int(raw_count)

    out: List[dict] = []
    for index in range(row_count):
        cells = matrix[index] if matrix is not None else None
        if cells is not None and not isinstance(cells, list):
            raise DecodeError("table row " + str(index) + " is not an array")

        row: Dict[str, Any] = {}
        constant_at = 0
        cell_at = 0

        for at in range(column_count):
            encoding = 0 if encodings is None else encodings[at]

            if encoding == COL_CONST:
                if constant_at >= len(constants):
                    raise DecodeError(
                        "table has fewer constants than constant columns"
                    )
                row[columns[at]] = constants[constant_at]
                constant_at += 1
                continue

            # A row shorter than the column list is missing everything past its
            # end.
            cell = (
                cells[cell_at]
                if cells is not None and cell_at < len(cells)
                else _ABSENT
            )
            cell_at += 1

            if encoding == COL_DICT:
                if cell is _ABSENT or cell == DICT_HOLE:
                    continue
                if cell is None:
                    row[columns[at]] = None
                    continue
                if not _is_index(cell) or cell < 0 or cell >= len(ctx.strings):
                    raise DecodeError(
                        "dictionary index " + str(cell) + " is out of range"
                    )
                row[columns[at]] = ctx.strings[int(cell)]
                continue

            if cell is _ABSENT or cell == HOLE:
                continue
            row[columns[at]] = _decode_value(cell, ctx)

        out.append(row)

    return out


def is_compressed(value: Any) -> bool:
    """True when ``value`` looks like a json-compress envelope this build can read."""
    return (
        isinstance(value, dict)
        and value.get(ENV_VERSION) == FORMAT_VERSION
        and isinstance(value.get(ENV_KEYS), list)
    )


def decompress(envelope: Any) -> Any:
    """Restore the original value from a json-compress envelope.

    Raises :class:`DecodeError` if the envelope is not a readable json-compress
    document.
    """
    if not isinstance(envelope, dict):
        raise DecodeError("expected a compressed envelope object")

    version = envelope.get(ENV_VERSION, _ABSENT)
    if version is _ABSENT:
        raise DecodeError('this is not a compressed document (no "jc" version field)')
    if version != FORMAT_VERSION or isinstance(version, bool):
        raise DecodeError(
            "format version "
            + str(version)
            + " is not supported by this build (expected "
            + str(FORMAT_VERSION)
            + ")"
        )

    keys = envelope.get(ENV_KEYS)
    if not isinstance(keys, list):
        raise DecodeError("the key catalogue is missing or is not an array")

    strings = envelope.get(ENV_STRINGS)
    if strings is not None and not isinstance(strings, list):
        raise DecodeError("the string catalogue is not an array")

    return _decode_value(envelope.get(ENV_DATA), _Context(keys, strings or []))
