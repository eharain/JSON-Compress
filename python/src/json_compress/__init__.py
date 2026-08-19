"""json-compress: lossless JSON compression that is still JSON.

Three ideas, applied together:

1. Every object key is replaced by a short token and the real names are listed
   once, in a catalogue. A key that occurs ten thousand times is spelled out
   once.
2. A list of like-shaped dicts becomes a table - one column list and a matrix of
   rows - so the keys disappear from the rows entirely. Columns whose value
   never changes are hoisted out of the rows altogether, and columns drawn from
   a small set of strings become integer indices.
3. String values that repeat are moved into a second catalogue and referenced by
   token.

The output is an ordinary JSON value with no binary framing, so it survives
anything that carries JSON: an HTTP body, a WebSocket frame, a ``jsonb`` column,
a queue message, a log line.

    >>> from json_compress import compress, decompress
    >>> envelope = compress([{"id": 1, "role": "admin"}, {"id": 2, "role": "admin"}])
    >>> decompress(envelope)
    [{'id': 1, 'role': 'admin'}, {'id': 2, 'role': 'admin'}]

The wire format is specified in ``SPEC.md`` at the root of the repository, and
this implementation writes the same bytes as the JavaScript and .NET ones.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Optional, Union

from .constants import FORMAT_VERSION
from .decode import DecodeError, decompress, is_compressed
from .encode import compress
from .jsontext import dumps, loads
from .locate import JsonLocation, locate
from .normalize import normalize
from .options import DEFAULT_OPTIONS, CompressOptions, resolve_options
from .pack import is_packed, pack, pack_all, text_to_array, unpack, unpack_all
from .sizes import byte_length, gunzip, gzip_bytes, gzip_size

__version__ = "1.0.0"

__all__ = [
    "FORMAT_VERSION",
    "__version__",
    "CompressOptions",
    "DEFAULT_OPTIONS",
    "DecodeError",
    "JsonCompressCodec",
    "JsonLocation",
    "SizeReport",
    "beautify",
    "byte_length",
    "compress",
    "create_codec",
    "decompress",
    "decompress_if_needed",
    "dumps",
    "gunzip",
    "gzip_bytes",
    "gzip_size",
    "is_compressed",
    "is_packed",
    "loads",
    "locate",
    "measure",
    "minify",
    "pack",
    "pack_all",
    "parse",
    "stringify",
    "text_to_array",
    "unpack",
    "unpack_all",
    "validate",
]

_Options = Union[CompressOptions, Mapping[str, Any], None]


def stringify(
    value: Any,
    options: _Options = None,
    indent: Union[int, str, None] = None,
    **overrides: Any,
) -> str:
    """Compress a value and serialise it in one step."""
    return dumps(compress(value, options, **overrides), indent)


def parse(text: str) -> Any:
    """Parse a compressed document and restore the original value."""
    return decompress(loads(text))


def decompress_if_needed(value: Any) -> Any:
    """Restore a value whether or not it was compressed.

    Handy at a boundary you do not fully control - an endpoint being migrated, a
    cache holding entries written by two versions of the same service.
    """
    return decompress(value) if is_compressed(value) else value


def minify(text: str) -> str:
    """Strip the whitespace from a JSON document without changing what it means."""
    return dumps(loads(text))


def beautify(text: str, indent: Union[int, str] = 2) -> str:
    """Re-indent a JSON document."""
    return dumps(loads(text), indent)


def validate(text: str) -> JsonLocation:
    """Check that a string is valid JSON, and say exactly where it stops being so.

    The verdict comes from the standard library, so it is the same verdict
    ``json.loads`` would give. The location comes from a scan of the document,
    because parser messages are written for parser authors and often name the
    wrong culprit.
    """
    try:
        loads(text)
        return JsonLocation(True)
    except Exception as error:  # noqa: BLE001 - any parse failure is a failure
        found = locate(text)
        if not found.valid:
            return found

        # The scan found nothing but the parse still failed - report what the
        # parser said rather than claiming the document is fine.
        return JsonLocation(False, str(error))


@dataclass(frozen=True)
class SizeReport:
    """What compression bought on one value."""

    #: Bytes of the input as given.
    original: int
    #: Bytes once whitespace is removed.
    minified: int
    #: Bytes of the compressed envelope.
    compressed: int
    #: Bytes saved against the minified form.
    saved: int
    #: Compressed divided by minified, 0 to 1.
    ratio: float
    #: Percentage saved against the minified form.
    percent: float
    #: Minified bytes after gzip, or ``None`` when not measured.
    gzip_minified: Optional[int] = None
    #: Compressed bytes after gzip, or ``None`` when not measured.
    gzip_compressed: Optional[int] = None


def measure(
    value: Any,
    options: _Options = None,
    source: Optional[str] = None,
    gzip: bool = True,
    **overrides: Any,
) -> SizeReport:
    """Measure what compression buys on a given value.

    The honest comparison is against minified JSON, not against whatever
    whitespace the input happened to arrive with, so ``ratio`` and ``percent``
    both use the minified size as their baseline. Gzip figures are included
    because that is what most payloads meet in production, and the interesting
    question is whether the format still helps once gzip has had its turn.

    Pass ``source`` to have ``original`` reflect the text the document really
    arrived as, and ``gzip=False`` to skip the gzip measurements.
    """
    minified = dumps(normalize(value))
    compressed = dumps(compress(value, options, **overrides))

    minified_bytes = byte_length(minified)
    compressed_bytes = byte_length(compressed)

    return SizeReport(
        original=minified_bytes if source is None else byte_length(source),
        minified=minified_bytes,
        compressed=compressed_bytes,
        saved=minified_bytes - compressed_bytes,
        ratio=1.0 if minified_bytes == 0 else compressed_bytes / minified_bytes,
        percent=(
            0.0
            if minified_bytes == 0
            else (minified_bytes - compressed_bytes) / minified_bytes * 100
        ),
        gzip_minified=gzip_size(minified) if gzip else None,
        gzip_compressed=gzip_size(compressed) if gzip else None,
    )


class JsonCompressCodec:
    """A set of options, bound once and reused.

    A codec is the natural unit when the same options apply everywhere - a
    transport layer, a cache adapter, a storage helper - and it keeps the option
    object off every call site. It holds nothing but its options, which are
    frozen, so one instance is safe to share across threads.
    """

    __slots__ = ("options",)

    def __init__(self, options: _Options = None, **overrides: Any) -> None:
        self.options = resolve_options(options, overrides)

    def compress(self, value: Any) -> dict:
        """Compress a value with this codec's options."""
        return compress(value, self.options)

    def decompress(self, envelope: Any) -> Any:
        """Restore a compressed value."""
        return decompress(envelope)

    def stringify(self, value: Any, indent: Union[int, str, None] = None) -> str:
        """Compress a value and serialise it."""
        return dumps(compress(value, self.options), indent)

    def parse(self, text: str) -> Any:
        """Parse and restore a compressed document."""
        return decompress(loads(text))

    def measure(
        self, value: Any, source: Optional[str] = None, gzip: bool = True
    ) -> SizeReport:
        """Measure what this codec's options buy on a value."""
        return measure(value, self.options, source=source, gzip=gzip)

    def __repr__(self) -> str:
        return "JsonCompressCodec(" + repr(self.options) + ")"


def create_codec(options: _Options = None, **overrides: Any) -> JsonCompressCodec:
    """Bind a set of options once and reuse them."""
    return JsonCompressCodec(options, **overrides)
