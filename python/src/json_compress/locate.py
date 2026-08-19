"""Finding the exact place a JSON document goes wrong.

``json.loads`` raises a message with an offset, but that message is written for
whoever wrote the parser rather than for whoever has to fix the document, and it
stops at the first structural surprise without saying which of the handful of
common mistakes caused it.

So this scans the document itself, against the grammar in RFC 8259, and reports
the first thing that does not belong - with an offset, a line, a column, the
line itself, and a description written for a person. It knows about single
quotes, trailing commas, ``True`` and ``None``, leading zeros, unterminated
strings and bad escapes.

The scan is iterative rather than recursive, so a pathologically nested document
is reported as too deep rather than exhausting the interpreter stack.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

__all__ = ["JsonLocation", "locate"]

_MAX_DEPTH = 5000

_WHITESPACE = frozenset(" \t\n\r")
_DIGITS = frozenset("0123456789")

_OBJECT = 0
_ARRAY = 1


@dataclass(frozen=True)
class JsonLocation:
    """Where a JSON document stops being valid, and why."""

    valid: bool
    #: What is wrong, in plain words.
    error: Optional[str] = None
    #: Character offset of the problem.
    position: Optional[int] = None
    #: Line number, counted from one.
    line: Optional[int] = None
    #: Column number, counted from one.
    column: Optional[int] = None
    #: The line the problem is on.
    excerpt: Optional[str] = None

    def __bool__(self) -> bool:
        return self.valid


class _ScanError(Exception):
    """Somewhere to unwind to, carrying the offset the problem was found at."""

    def __init__(self, message: str, at: int) -> None:
        super().__init__(message)
        self.message = message
        self.at = at


def _describe(text: str, at: int) -> str:
    if at >= len(text):
        return "the document ends here"
    character = text[at]
    code = ord(character)
    if code < 32:
        return "an unescaped control character (0x%02x)" % code
    return '"' + character + '"'


class _Scanner:
    __slots__ = ("text", "length", "at")

    def __init__(self, text: str) -> None:
        self.text = text
        self.length = len(text)
        self.at = 0

    def skip_whitespace(self) -> None:
        text = self.text
        at = self.at
        length = self.length
        while at < length and text[at] in _WHITESPACE:
            at += 1
        self.at = at

    def peek(self) -> str:
        return self.text[self.at] if self.at < self.length else ""

    def expect(self, character: str, what: str) -> None:
        if self.at >= self.length or self.text[self.at] != character:
            raise _ScanError(
                "expected " + what + ", found " + _describe(self.text, self.at), self.at
            )
        self.at += 1

    def scan(self) -> None:
        """Walk one complete JSON value, then anything that follows it."""
        stack = []
        # Three states: about to read a value, about to read a property name,
        # or having just finished a value and deciding what comes next.
        state = "value"

        while True:
            if state == "value":
                self.skip_whitespace()
                if self.at >= self.length:
                    raise _ScanError(
                        "expected a value, but the document ends here", self.at
                    )

                character = self.text[self.at]

                if character == "{" or character == "[":
                    closer = "}" if character == "{" else "]"
                    self.at += 1
                    self.skip_whitespace()
                    if self.peek() == closer:
                        self.at += 1
                        state = "after"
                        continue
                    stack.append(_OBJECT if character == "{" else _ARRAY)
                    if len(stack) > _MAX_DEPTH:
                        raise _ScanError(
                            "the document is nested too deeply to check", self.at
                        )
                    state = "key" if character == "{" else "value"
                    continue

                if character == '"':
                    self.scan_string()
                elif character == "-" or character in _DIGITS:
                    self.scan_number()
                elif self.text.startswith("true", self.at):
                    self.at += 4
                elif self.text.startswith("false", self.at):
                    self.at += 5
                elif self.text.startswith("null", self.at):
                    self.at += 4
                # The three mistakes that account for most hand-edited JSON.
                elif character == "'":
                    raise _ScanError(
                        "strings must use double quotes, not single quotes", self.at
                    )
                elif self.text.startswith("True", self.at) or self.text.startswith(
                    "False", self.at
                ):
                    raise _ScanError(
                        "true and false must be lower case in JSON", self.at
                    )
                elif self.text.startswith("None", self.at) or self.text.startswith(
                    "undefined", self.at
                ):
                    raise _ScanError(
                        "JSON has no None or undefined - use null", self.at
                    )
                else:
                    raise _ScanError(
                        "expected a value, found " + _describe(self.text, self.at),
                        self.at,
                    )

                state = "after"
                continue

            if state == "key":
                self.skip_whitespace()
                if self.peek() != '"':
                    if self.peek() == "'":
                        raise _ScanError(
                            "strings must use double quotes, not single quotes", self.at
                        )
                    raise _ScanError(
                        "expected a quoted property name, found "
                        + _describe(self.text, self.at),
                        self.at,
                    )
                self.scan_string()
                self.skip_whitespace()
                self.expect(":", "a colon after the property name")
                state = "value"
                continue

            # state == "after"
            if not stack:
                return

            in_object = stack[-1] == _OBJECT
            closer = "}" if in_object else "]"

            self.skip_whitespace()
            if self.at >= self.length:
                raise _ScanError(
                    "this object is never closed"
                    if in_object
                    else "this array is never closed",
                    self.at,
                )

            character = self.text[self.at]
            if character == ",":
                comma_at = self.at
                self.at += 1
                self.skip_whitespace()
                if self.peek() == closer:
                    raise _ScanError(
                        "a trailing comma is not allowed in JSON", comma_at
                    )
                state = "key" if in_object else "value"
                continue
            if character == closer:
                self.at += 1
                stack.pop()
                continue
            raise _ScanError(
                "expected a comma or a closing "
                + ("brace" if in_object else "bracket")
                + ", found "
                + _describe(self.text, self.at),
                self.at,
            )

    def scan_string(self) -> None:
        text = self.text
        length = self.length
        start = self.at
        at = self.at + 1  # the opening quote

        while at < length:
            character = text[at]

            if character == '"':
                self.at = at + 1
                return

            if character == "\\":
                at += 1
                if at >= length:
                    break
                escape = text[at]
                if escape in '"\\/bfnrt':
                    at += 1
                    continue
                if escape == "u":
                    digits = text[at + 1 : at + 5]
                    if len(digits) < 4 or any(
                        character not in "0123456789abcdefABCDEF"
                        for character in digits
                    ):
                        self.at = at - 1
                        raise _ScanError(
                            "\\u must be followed by four hexadecimal digits", at - 1
                        )
                    at += 5
                    continue
                self.at = at - 1
                raise _ScanError(
                    '"\\' + escape + '" is not a valid escape sequence', at - 1
                )

            if ord(character) < 32:
                raise _ScanError(
                    "a raw control character inside a string - escape it as "
                    + "\\u%04x" % ord(character),
                    at,
                )

            at += 1

        raise _ScanError("this string is never closed", start)

    def scan_number(self) -> None:
        text = self.text
        length = self.length
        start = self.at
        at = self.at

        if text[at] == "-":
            at += 1

        if at >= length:
            raise _ScanError("this number has no digits", start)

        if text[at] == "0":
            at += 1
            if at < length and text[at] in _DIGITS:
                raise _ScanError("a number may not have a leading zero", start)
        elif text[at] in _DIGITS:
            while at < length and text[at] in _DIGITS:
                at += 1
        else:
            raise _ScanError("expected a digit, found " + _describe(text, at), at)

        if at < length and text[at] == ".":
            at += 1
            digits = 0
            while at < length and text[at] in _DIGITS:
                at += 1
                digits += 1
            if digits == 0:
                raise _ScanError(
                    "a decimal point must be followed by at least one digit", at - 1
                )

        if at < length and (text[at] == "e" or text[at] == "E"):
            at += 1
            if at < length and (text[at] == "+" or text[at] == "-"):
                at += 1
            digits = 0
            while at < length and text[at] in _DIGITS:
                at += 1
                digits += 1
            if digits == 0:
                raise _ScanError("an exponent must have at least one digit", at - 1)

        self.at = at


def _position_of(text: str, at: int) -> tuple:
    """Turn a character offset into a line and column, both counted from one."""
    clamped = max(0, min(at, len(text)))
    before = text[:clamped]
    line = before.count("\n") + 1
    column = clamped - (before.rfind("\n") + 1) + 1
    return line, column


def locate(text: str) -> JsonLocation:
    """Check a JSON document and, if it is broken, say exactly where."""
    if not isinstance(text, str):
        return JsonLocation(False, "expected a string")

    scanner = _Scanner(text)
    try:
        scanner.scan()
        scanner.skip_whitespace()
        if scanner.at < len(text):
            raise _ScanError(
                "unexpected "
                + _describe(text, scanner.at)
                + " after the end of the document",
                scanner.at,
            )
    except _ScanError as error:
        line, column = _position_of(text, error.at)
        lines = text.split("\n")
        return JsonLocation(
            valid=False,
            error=error.message,
            position=max(0, min(error.at, len(text))),
            line=line,
            column=column,
            excerpt=lines[line - 1] if line - 1 < len(lines) else "",
        )

    return JsonLocation(True)
