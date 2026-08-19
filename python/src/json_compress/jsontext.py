"""Reading and writing JSON text the way the rest of the format expects.

Python's :mod:`json` reads exactly what is wanted, so :func:`loads` is a thin
wrapper over it - strict about ``NaN`` and ``Infinity``, which JSON has no
spelling for and which ``json.loads`` otherwise accepts.

Writing needs more care. The other implementations of this format agree with
each other byte for byte, and two details of ``json.dumps`` would break that:

* **Numbers.** ``repr`` switches to exponent form at 1e16 and below 1e-4, so
  ``1e18`` comes out as ``1e+18`` and ``0.00001`` as ``1e-05``. The format
  follows the ECMAScript rule instead - positional between 1e-7 and 1e21, and
  ``1e+21`` rather than ``1e21`` outside it - because that is what every
  document in the conformance corpus was written with.
* **Lone surrogates.** A document may carry an unpaired surrogate, which is a
  perfectly good JSON escape but not encodable as UTF-8. Those are written back
  out as an escape sequence rather than emitted raw.

Everything else - which characters are escaped, where the spaces go when
indenting - already matches, so the fast C string encoder is used as it stands.
"""

from __future__ import annotations

import json
import re
from json.encoder import encode_basestring
from typing import Any, Union

__all__ = ["dumps", "loads", "format_number", "key_text", "utf16_length"]

_INFINITY = float("inf")
_SURROGATES = re.compile("[\ud800-\udfff]")


def _reject_constant(name: str) -> Any:
    raise ValueError("json-compress: " + name + " is not valid JSON")


def loads(text: str) -> Any:
    """Parse JSON text, rejecting the constants JSON has no spelling for."""
    return json.loads(text, parse_constant=_reject_constant)


def utf16_length(text: str) -> int:
    """Length of a string in UTF-16 code units.

    This is what ``String.length`` counts in JavaScript, and the encoder's cost
    model is calibrated against it - so an emoji weighs two, not one, and the
    same string is catalogued here and there or in neither.
    """
    if text.isascii():
        return len(text)
    return len(text.encode("utf-16-le", "surrogatepass")) >> 1


def format_number(value: float) -> str:
    """Render a float the way ECMAScript's ``Number::toString`` would.

    Python and JavaScript agree on the digits - both print the shortest decimal
    that reads back as the same double - and disagree only on where the decimal
    point goes and when to switch to an exponent. This applies the ECMAScript
    placement rules to Python's digits.
    """
    # Non-finite values have no JSON spelling; JSON writers substitute null.
    if value != value or value == _INFINITY or value == -_INFINITY:
        return "null"
    if value == 0:
        return "0"

    sign = ""
    if value < 0:
        sign = "-"
        value = -value

    # Split the shortest round-trip repr into its digits and a decimal exponent,
    # then normalise to `value == digits * 10 ** (n - k)`, which is the form the
    # ECMAScript rules are written against.
    text = repr(value)
    mantissa, _, exponent_text = text.partition("e")
    exponent = int(exponent_text) if exponent_text else 0

    whole, _, fraction = mantissa.partition(".")
    digits = (whole + fraction).lstrip("0")
    n = len(whole) + exponent - (len(whole) + len(fraction) - len(digits))
    s = digits.rstrip("0") or "0"
    k = len(s)

    if k <= n <= 21:
        return sign + s + "0" * (n - k)
    if 0 < n <= 21:
        return sign + s[:n] + "." + s[n:]
    if -6 < n <= 0:
        return sign + "0." + "0" * -n + s
    tail = "e+" + str(n - 1) if n - 1 >= 0 else "e-" + str(1 - n)
    if k == 1:
        return sign + s + tail
    return sign + s[0] + "." + s[1:] + tail


def key_text(key: Any) -> str:
    """The name an object key is written under.

    JSON has string keys and nothing else, so the values ``json.dumps`` is
    willing to convert are converted the same way here, and anything else is
    refused rather than silently stringified.
    """
    if type(key) is str:
        return key
    if key is None:
        return "null"
    if key is True:
        return "true"
    if key is False:
        return "false"
    if isinstance(key, str):
        return str(key)
    if isinstance(key, bool):
        return "true" if key else "false"
    if isinstance(key, int):
        return str(int(key))
    if isinstance(key, float):
        return format_number(key)
    raise TypeError(
        "json-compress: a "
        + type(key).__name__
        + " cannot be an object key - convert it to a string first"
    )


def _quote(text: str) -> str:
    out = encode_basestring(text)
    if _SURROGATES.search(out):
        out = _SURROGATES.sub(lambda match: "\\u%04x" % ord(match.group()), out)
    return out


def _write(value: Any, out: list, pad: str, level: int) -> None:
    if value is None:
        out.append("null")
        return
    if value is True:
        out.append("true")
        return
    if value is False:
        out.append("false")
        return

    kind = type(value)
    if kind is str:
        out.append(_quote(value))
        return
    if kind is int:
        out.append(str(value))
        return
    if kind is float:
        out.append(format_number(value))
        return

    if kind is list or kind is tuple:
        if not value:
            out.append("[]")
            return
        if pad:
            inner = "\n" + pad * (level + 1)
            out.append("[")
            for index, item in enumerate(value):
                out.append("," if index else "")
                out.append(inner)
                _write(item, out, pad, level + 1)
            out.append("\n" + pad * level + "]")
        else:
            out.append("[")
            for index, item in enumerate(value):
                if index:
                    out.append(",")
                _write(item, out, pad, level)
            out.append("]")
        return

    if kind is dict:
        if not value:
            out.append("{}")
            return
        if pad:
            inner = "\n" + pad * (level + 1)
            out.append("{")
            for index, (key, item) in enumerate(value.items()):
                out.append("," if index else "")
                out.append(inner)
                out.append(_quote(key if type(key) is str else key_text(key)))
                out.append(": ")
                _write(item, out, pad, level + 1)
            out.append("\n" + pad * level + "}")
        else:
            out.append("{")
            for index, (key, item) in enumerate(value.items()):
                if index:
                    out.append(",")
                out.append(_quote(key if type(key) is str else key_text(key)))
                out.append(":")
                _write(item, out, pad, level)
            out.append("}")
        return

    # Subclasses and the odd duck-typed value: fall back to the slower checks
    # rather than refusing something json.dumps would have written.
    if isinstance(value, bool):
        out.append("true" if value else "false")
    elif isinstance(value, str):
        out.append(_quote(str(value)))
    elif isinstance(value, int):
        out.append(str(int(value)))
    elif isinstance(value, float):
        out.append(format_number(float(value)))
    elif isinstance(value, (list, tuple)):
        _write(list(value), out, pad, level)
    elif isinstance(value, dict):
        _write(dict(value), out, pad, level)
    else:
        raise TypeError(
            "json-compress: "
            + type(value).__name__
            + " is not a JSON value - normalise it before writing"
        )


def dumps(value: Any, indent: Union[int, str, None] = None) -> str:
    """Serialise a JSON value, agreeing with ``JSON.stringify`` character for character.

    ``indent`` follows the same rules as the third argument to
    ``JSON.stringify``: an integer number of spaces or a literal string, either
    capped at ten characters, and no indentation at all when it is ``None`` or
    empty.
    """
    if indent is None:
        pad = ""
    elif isinstance(indent, bool):
        raise TypeError("json-compress: indent must be a number or a string")
    elif isinstance(indent, int):
        pad = " " * min(10, max(0, indent))
    elif isinstance(indent, str):
        pad = indent[:10]
    else:
        raise TypeError("json-compress: indent must be a number or a string")

    out: list = []
    _write(value, out, pad, 0)
    return "".join(out)
