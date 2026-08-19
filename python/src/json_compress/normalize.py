"""Bring arbitrary input down to plain JSON values.

The encoder is much simpler if it can assume it is only ever looking at ``None``,
booleans, finite numbers, strings, lists and dicts. This pass guarantees that,
applying the rules a JSON writer applies:

* the types the standard library knows how to describe are converted -
  ``datetime`` and friends to ISO 8601 text, ``Decimal`` to a number, ``UUID``
  and ``Enum`` to their obvious form, dataclasses to their fields;
* an object with a ``for_json`` method is asked for its JSON form, which is the
  nearest thing Python has to JavaScript's ``toJSON``;
* non-finite numbers become ``None``, and negative zero becomes zero, because
  JSON cannot tell the two apart;
* dict keys that are not strings are converted the way ``json.dumps`` converts
  them;
* a circular reference raises, rather than hanging.

It returns the original object whenever nothing had to change, so clean input -
which is the common case - costs a walk but no allocation.
"""

from __future__ import annotations

import dataclasses
from collections.abc import Mapping, Sequence
from datetime import date, datetime, time
from decimal import Decimal
from enum import Enum
from typing import Any
from uuid import UUID

from .jsontext import key_text

__all__ = ["normalize"]

_INFINITY = float("inf")


def _finite(value: float) -> Any:
    if value != value or value == _INFINITY or value == -_INFINITY:
        return None
    # Negative zero is folded to zero so that the in-memory round trip and the
    # serialised one agree; JSON itself cannot tell them apart.
    return 0.0 if value == 0 else value


def _walk(value: Any, seen: dict) -> Any:
    # Containers are handled inline rather than in a helper, because this is the
    # deepest recursion in the library: one frame per level of nesting rather
    # than two doubles the document depth the interpreter's stack allows.
    if value is None or value is True or value is False:
        return value

    kind = type(value)
    if kind is str or kind is int:
        return value
    if kind is float:
        return _finite(value)

    if kind is list or kind is dict:
        changed = False
    else:
        converted = _convert(value)
        if converted is not value:
            return _walk(converted, seen)

        # A subclass of something JSON already understands: keep the value, drop
        # the subclass, so the encoder only ever meets the plain types.
        if isinstance(value, bool):
            return bool(value)
        if isinstance(value, str):
            return str(value)
        if isinstance(value, int):
            return int(value)
        if isinstance(value, float):
            return _finite(float(value))
        changed = True

    marker = id(value)
    if marker in seen:
        raise ValueError("json-compress: circular structure cannot be compressed")
    seen[marker] = value

    try:
        if kind is dict or isinstance(value, dict):
            out_map: dict = {}
            for key, item in value.items():
                name = key_text(key)
                if name is not key:
                    changed = True
                item_out = _walk(item, seen)
                if item_out is not item:
                    changed = True
                out_map[name] = item_out
            return out_map if changed else value

        out = []
        for item in value:
            item_out = _walk(item, seen)
            if item_out is not item:
                changed = True
            out.append(item_out)
        return out if changed else value
    finally:
        del seen[marker]


def _convert(value: Any) -> Any:
    """Turn a value the encoder does not know into one it does, or return it as is."""
    for_json = getattr(value, "for_json", None)
    if callable(for_json):
        return for_json()

    if isinstance(value, Enum):
        return value.value
    if isinstance(value, (datetime, date, time)):
        return value.isoformat()
    if isinstance(value, Decimal):
        return float(value)
    if isinstance(value, UUID):
        return str(value)
    if dataclasses.is_dataclass(value) and not isinstance(value, type):
        return {
            field.name: getattr(value, field.name)
            for field in dataclasses.fields(value)
        }
    if isinstance(value, (bool, int, float, str, list, tuple, dict)):
        return value
    if isinstance(value, Mapping):
        return dict(value)
    if isinstance(value, (bytes, bytearray, memoryview)):
        raise TypeError(
            "json-compress: bytes have no JSON form - decode or encode them first"
        )
    if isinstance(value, (set, frozenset)):
        raise TypeError(
            "json-compress: a set has no JSON form - convert it to a list first"
        )
    if isinstance(value, Sequence):
        return list(value)

    raise TypeError(
        "json-compress: "
        + type(value).__name__
        + " has no JSON form - give it a for_json method or convert it first"
    )


def normalize(value: Any) -> Any:
    """Reduce ``value`` to nulls, booleans, numbers, strings, lists and dicts."""
    return _walk(value, {})
