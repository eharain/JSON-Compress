"""Byte measurement and gzip.

What actually goes over the wire is UTF-8 bytes, not characters, so that is what
these count. The gzip helpers are here because the interesting question about
any compression scheme is what it is still worth once gzip has had its turn, and
answering it should not need a second import at the call site.
"""

from __future__ import annotations

import gzip as _gzip

__all__ = ["byte_length", "gzip_bytes", "gunzip", "gzip_size"]

#: Deflate level 6 is zlib's own default and what browsers use for
#: ``CompressionStream``, so a figure measured here is comparable with one
#: measured in the JavaScript implementation.
DEFAULT_LEVEL = 6


def _utf8(text: str) -> bytes:
    # surrogatepass, because a JSON document may legitimately carry an unpaired
    # surrogate and measuring it should not be the thing that fails.
    return text.encode("utf-8", "surrogatepass")


def byte_length(text: str) -> int:
    """Length of a string in UTF-8 bytes."""
    return len(_utf8(text))


def gzip_bytes(text: str, level: int = DEFAULT_LEVEL) -> bytes:
    """Gzip a string.

    The header carries no timestamp, so the same input always gives the same
    bytes and a size can be compared across runs.
    """
    return _gzip.compress(_utf8(text), compresslevel=level, mtime=0)


def gunzip(data: bytes) -> str:
    """Reverse :func:`gzip_bytes`."""
    return _gzip.decompress(data).decode("utf-8", "surrogatepass")


def gzip_size(text: str, level: int = DEFAULT_LEVEL) -> int:
    """How many bytes a string takes once gzipped."""
    return len(gzip_bytes(text, level))
