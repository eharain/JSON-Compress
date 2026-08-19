"""The plain table pack.

This is the smallest useful idea in the library, offered on its own because it
is often all you need and because the shape it produces - ``fields``, ``data``
and ``count`` - is trivial to consume from another language or straight out of a
database cursor.

:func:`pack` turns a list of records into one field list and a matrix of rows.
:func:`unpack` turns it back. Neither uses a catalogue, an envelope or a version
tag; what you get is a readable object you can eyeball in a network panel.
"""

from __future__ import annotations

from typing import Any, Dict, List

__all__ = ["pack", "unpack", "pack_all", "unpack_all", "is_packed", "text_to_array"]

_ABSENT = object()


def pack(records: Any) -> Any:
    """Pack a list of records into a field list and a rows matrix.

    Keys are taken from the union of every record, in first-seen order, so a
    record missing a field simply contributes ``None`` in that column. Anything
    that is not a list of mappings is handed straight back, which lets you drop
    :func:`pack` into a response pipeline without first checking the shape.
    """
    if records is None or not isinstance(records, list):
        return records
    if not records:
        return {"fields": [], "data": [], "count": 0}

    for record in records:
        if not isinstance(record, dict):
            return records

    fields: List[str] = []
    seen = set()
    for record in records:
        for key in record:
            if key not in seen:
                seen.add(key)
                fields.append(key)

    data = [[record.get(field) for field in fields] for record in records]
    return {"fields": fields, "data": data, "count": len(data)}


def unpack(packed: Any) -> Any:
    """Rebuild the list of records produced by :func:`pack`.

    Anything that is not a packed table is handed straight back, so a consumer
    can call :func:`unpack` unconditionally on a payload that may or may not
    have been packed at the other end.
    """
    if not isinstance(packed, dict):
        return packed

    fields = packed.get("fields")
    data = packed.get("data")
    if not isinstance(fields, list) or not isinstance(data, list):
        return packed

    out: List[Dict[str, Any]] = []
    for row in data:
        record: Dict[str, Any] = {}
        for at, field in enumerate(fields):
            if not isinstance(row, list) or at >= len(row):
                # A row shorter than the field list is missing everything past
                # its end, rather than holding nulls there.
                continue
            record[field] = row[at]
        out.append(record)
    return out


def is_packed(value: Any) -> bool:
    """True when ``value`` has the shape :func:`unpack` knows how to read."""
    return (
        isinstance(value, dict)
        and isinstance(value.get("fields"), list)
        and isinstance(value.get("data"), list)
    )


def pack_all(result_sets: Any) -> Any:
    """Pack every result set in a list of result sets.

    Database drivers that return several result sets from one call hand back a
    list of lists; this packs each inner list and leaves the outer one alone.
    """
    if result_sets is None or not isinstance(result_sets, list) or not result_sets:
        return result_sets
    return [pack(result_set) for result_set in result_sets]


def unpack_all(result_sets: Any) -> Any:
    """Unpack every result set packed by :func:`pack_all`."""
    if result_sets is None or not isinstance(result_sets, list):
        return result_sets
    return [unpack(result_set) for result_set in result_sets]


def text_to_array(
    record: Any,
    field: str,
    prefix: int = 3,
    suffix: int = 3,
    separator: str = ",",
) -> Any:
    """Read a delimited-integer column back into a list of numbers.

    Some databases return a set-valued column as a bracketed, comma-separated
    string. This converts one such field in place, leaving an empty list where
    the column was null. ``prefix`` and ``suffix`` are how many characters to
    trim from each end.

    Returns the same record, for chaining.
    """
    if not isinstance(record, dict):
        return record
    if field not in record:
        return record

    value = record[field]
    if not isinstance(value, str):
        record[field] = []
        return record

    inner = value[prefix : len(value) - suffix]
    record[field] = [] if not inner else [int(part) for part in inner.split(separator)]
    return record
