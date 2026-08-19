"""Short tokens for catalogue indices.

Two alphabets, because the two places a token appears have different rules.

String references sit in value position, where nothing constrains them, so they
use plain base-62: the first 62 entries cost one character.

Object keys cannot. A JavaScript object - and every JSON parser that builds one
- reorders keys that look like array indices to the front, in numeric order, so
a key of ``"10"`` would jump ahead of ``"2"``. Encoding a key as ``"7"`` would
therefore quietly shuffle the document for a JavaScript reader. Key tokens
instead start with a letter and continue in base-62, which is never index-like:
52 keys still cost one character, the next 3,224 cost two, and the original key
order survives the round trip exactly.
"""

from __future__ import annotations

DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
LETTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

_DIGIT_VALUES = {character: value for value, character in enumerate(DIGITS)}
_LETTER_VALUES = {character: value for value, character in enumerate(LETTERS)}


# --------------------------------------------------------------------------- #
# Base-62, for string references
# --------------------------------------------------------------------------- #


def to_base62(n: int) -> str:
    """Encode a non-negative integer as a base-62 token."""
    if n < 62:
        return DIGITS[n]
    out = []
    value = n
    while value > 0:
        out.append(DIGITS[value % 62])
        value //= 62
    return "".join(reversed(out))


def from_base62(token: str) -> int:
    """Decode a base-62 token, or return -1 if it is not one."""
    if not token:
        return -1
    n = 0
    for character in token:
        digit = _DIGIT_VALUES.get(character)
        if digit is None:
            return -1
        n = n * 62 + digit
    return n


def base62_length(n: int) -> int:
    """Length in characters of the base-62 token for ``n``, without building it."""
    if n < 62:
        return 1
    if n < 3844:
        return 2
    if n < 238328:
        return 3
    length = 0
    value = n
    while value > 0:
        length += 1
        value //= 62
    return length


# --------------------------------------------------------------------------- #
# Letter-first tokens, for object keys
# --------------------------------------------------------------------------- #


def to_key_token(n: int) -> str:
    """Encode a non-negative integer as a key token: a letter, then base-62 digits.

    The numbering is bijective - each token length covers a contiguous block of
    indices and no index has two spellings - so :func:`from_key_token` inverts
    it exactly.
    """
    if n < 52:
        return LETTERS[n]

    # Find the block this index falls in: 52 one-character tokens, then
    # 52 * 62 two-character ones, and so on.
    start = 0
    size = 52
    length = 1
    while n >= start + size:
        start += size
        size *= 62
        length += 1

    value = n - start
    tail = []
    for _ in range(1, length):
        tail.append(DIGITS[value % 62])
        value //= 62
    return LETTERS[value] + "".join(reversed(tail))


def from_key_token(token: str) -> int:
    """Decode a key token, or return -1 if it is not one."""
    if not token:
        return -1

    n = _LETTER_VALUES.get(token[0])
    if n is None:
        return -1

    for character in token[1:]:
        digit = _DIGIT_VALUES.get(character)
        if digit is None:
            return -1
        n = n * 62 + digit

    start = 0
    size = 52
    for _ in range(1, len(token)):
        start += size
        size *= 62
    return start + n
