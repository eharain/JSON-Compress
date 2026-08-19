"""Option handling.

Every knob has a default that is safe on arbitrary input; the defaults are tuned
for API responses and log batches, which is where the format earns most of its
keep.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, Mapping, Optional, Union

__all__ = ["CompressOptions", "DEFAULT_OPTIONS", "resolve_options"]


@dataclass(frozen=True)
class CompressOptions:
    """What the encoder is allowed to do.

    :param strings: de-duplicate repeated string values into a catalogue.
    :param tables: pack arrays of like-shaped objects into columnar tables.
    :param constants: hoist columns whose value never changes out of the rows.
    :param dict_columns: encode low-cardinality string columns as integer indices.
    :param min_rows: smallest array that may become a table.
    :param max_hole_ratio: largest share of missing cells a table may carry.
    :param min_string_count: times a string must occur before it can be catalogued.
    :param min_string_length: shortest string that may be catalogued.
    """

    strings: bool = True
    tables: bool = True
    constants: bool = True
    dict_columns: bool = True
    min_rows: int = 2
    max_hole_ratio: float = 0.3
    min_string_count: int = 2
    min_string_length: int = 1

    def __post_init__(self) -> None:
        # Validated here rather than at the point of use, so a typo surfaces at
        # the call site rather than as a mysteriously large output.
        if not isinstance(self.min_rows, int) or isinstance(self.min_rows, bool):
            raise TypeError("json-compress: min_rows must be an integer")
        if self.min_rows < 1:
            raise ValueError("json-compress: min_rows must be at least 1")
        if not isinstance(self.max_hole_ratio, (int, float)) or isinstance(
            self.max_hole_ratio, bool
        ):
            raise TypeError("json-compress: max_hole_ratio must be a number")
        if not 0 <= self.max_hole_ratio <= 1:
            raise ValueError("json-compress: max_hole_ratio must be between 0 and 1")
        if not isinstance(self.min_string_count, int) or isinstance(
            self.min_string_count, bool
        ):
            raise TypeError("json-compress: min_string_count must be an integer")
        if self.min_string_count < 2:
            raise ValueError("json-compress: min_string_count must be at least 2")
        if not isinstance(self.min_string_length, int) or isinstance(
            self.min_string_length, bool
        ):
            raise TypeError("json-compress: min_string_length must be an integer")
        if self.min_string_length < 0:
            raise ValueError("json-compress: min_string_length must not be negative")

    def replace(self, **changes: Any) -> CompressOptions:
        """A copy with some fields changed, validated like any other instance."""
        return replace(self, **changes)


#: The options used when none are given.
DEFAULT_OPTIONS = CompressOptions()

_FIELD_NAMES = frozenset(CompressOptions.__dataclass_fields__)


def resolve_options(
    options: Union[CompressOptions, Mapping[str, Any], None] = None,
    overrides: Optional[Mapping[str, Any]] = None,
) -> CompressOptions:
    """Merge whatever the caller passed into a validated :class:`CompressOptions`.

    ``options`` may be a :class:`CompressOptions`, a mapping of field names, or
    ``None``; ``overrides`` are keyword arguments applied on top of it.
    """
    if options is None:
        resolved = DEFAULT_OPTIONS
    elif isinstance(options, CompressOptions):
        resolved = options
    elif isinstance(options, Mapping):
        resolved = DEFAULT_OPTIONS.replace(**_checked(options))
    else:
        raise TypeError(
            "json-compress: options must be a CompressOptions or a mapping, not "
            + type(options).__name__
        )

    if overrides:
        resolved = resolved.replace(**_checked(overrides))
    return resolved


def _checked(values: Mapping[str, Any]) -> dict:
    unknown = [name for name in values if name not in _FIELD_NAMES]
    if unknown:
        raise TypeError(
            "json-compress: unknown option "
            + ", ".join(sorted(unknown))
            + " (expected one of "
            + ", ".join(sorted(_FIELD_NAMES))
            + ")"
        )
    return dict(values)
