"""The json-compress command line.

Reads from a file or from stdin, writes to a file or to stdout, so it drops into
a pipeline without ceremony::

    curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
    json-compress stats orders.json
"""

from __future__ import annotations

import argparse
import contextlib
import os
import sys
from typing import Any, List, NoReturn, Optional

from . import (
    __version__,
    beautify,
    compress,
    decompress,
    dumps,
    is_compressed,
    loads,
    measure,
    minify,
    validate,
)

COMMANDS = ("compress", "decompress", "stats", "minify", "beautify", "validate")

_EPILOG = """
commands:
  compress [file]     Compress a JSON document
  decompress [file]   Restore a compressed document
  stats [file]        Report what compression would save, and change nothing
  minify [file]       Remove whitespace only
  beautify [file]     Re-indent
  validate [file]     Check the document parses, and say where it does not

With no input file, the document is read from stdin.
"""


def _fail(message: str) -> NoReturn:
    sys.stderr.write("json-compress: " + message + "\n")
    raise SystemExit(1)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="json-compress",
        description="lossless JSON compression that is still JSON",
        epilog=_EPILOG,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("command", nargs="?", choices=COMMANDS, help=argparse.SUPPRESS)
    parser.add_argument("input", nargs="?", help="the document to read (default stdin)")
    parser.add_argument(
        "-o", "--out", metavar="FILE", help="write to a file instead of stdout"
    )
    parser.add_argument(
        "-p", "--pretty", action="store_true", help="indent the output"
    )
    parser.add_argument(
        "-i",
        "--indent",
        type=int,
        default=2,
        metavar="N",
        help="indent width for --pretty and beautify (default 2)",
    )
    parser.add_argument(
        "--no-strings",
        dest="strings",
        action="store_false",
        help="do not build a string-value catalogue",
    )
    parser.add_argument(
        "--no-tables",
        dest="tables",
        action="store_false",
        help="do not pack arrays of objects into tables",
    )
    parser.add_argument(
        "--no-constants",
        dest="constants",
        action="store_false",
        help="do not hoist unchanging columns out of rows",
    )
    parser.add_argument(
        "--no-dict-columns",
        dest="dict_columns",
        action="store_false",
        help="do not encode low-cardinality string columns as indices",
    )
    parser.add_argument(
        "--min-rows",
        type=int,
        default=2,
        metavar="N",
        help="smallest array that may become a table (default 2)",
    )
    parser.add_argument(
        "-v", "--version", action="version", version=__version__
    )
    return parser


def _read_input(path: Optional[str]) -> str:
    if path:
        try:
            with open(path, encoding="utf-8") as handle:
                return handle.read()
        except OSError as error:
            _fail("cannot read " + path + " (" + (error.strerror or "failed") + ")")
    if sys.stdin is None or sys.stdin.isatty():
        _fail("no input file given and nothing on stdin")
    return sys.stdin.read()


def _write_output(text: str, path: Optional[str]) -> None:
    if path:
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
        return
    sys.stdout.write(text + "\n")


def _kilobytes(count: int) -> str:
    if count < 1024:
        return str(count) + " B"
    if count < 1024 * 1024:
        return "%.1f KB" % (count / 1024)
    return "%.2f MB" % (count / (1024 * 1024))


def _change(percent: float) -> str:
    """"31% smaller" or "31% larger" - never a negative percentage."""
    return "%.1f%% %s" % (abs(percent), "larger" if percent < 0 else "smaller")


def _stats(report: Any, name: str) -> str:
    lines = [
        name,
        "  as given          " + _kilobytes(report.original).rjust(10),
        "  minified          " + _kilobytes(report.minified).rjust(10),
        "  compressed        " + _kilobytes(report.compressed).rjust(10),
        "  saved             "
        + _kilobytes(report.saved).rjust(10)
        + "   "
        + _change(report.percent)
        + " than minified",
    ]
    if report.gzip_minified is not None:
        saved = report.gzip_minified - report.gzip_compressed
        percent = 0.0 if report.gzip_minified == 0 else saved / report.gzip_minified * 100
        lines += [
            "",
            "  minified + gzip   " + _kilobytes(report.gzip_minified).rjust(10),
            "  compressed + gzip " + _kilobytes(report.gzip_compressed).rjust(10),
            "  saved             "
            + _kilobytes(saved).rjust(10)
            + "   "
            + _change(percent)
            + " than minified + gzip",
        ]
    return "\n".join(lines)


def main(argv: Optional[List[str]] = None) -> int:
    """Run the command line. Returns the process exit code."""
    for stream in (sys.stdin, sys.stdout, sys.stderr):
        # A JSON document is UTF-8 whatever the console code page says, and the
        # output is compared byte for byte by the interoperability checks.
        if hasattr(stream, "reconfigure"):
            # An exotic console may refuse; the default encoding then stands.
            with contextlib.suppress(ValueError, OSError):
                stream.reconfigure(encoding="utf-8", newline="\n")

    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.command is None:
        parser.print_help()
        return 0

    text = _read_input(args.input)
    indent = args.indent if args.pretty else None

    if args.command == "validate":
        found = validate(text)
        if found.valid:
            sys.stdout.write("valid JSON\n")
            return 0
        where = (
            " at line " + str(found.line) + ", column " + str(found.column)
            if found.line
            else ""
        )
        _fail("invalid JSON" + where + ": " + str(found.error))

    if args.command == "minify":
        _write_output(minify(text), args.out)
        return 0

    if args.command == "beautify":
        _write_output(beautify(text, args.indent), args.out)
        return 0

    try:
        parsed = loads(text)
    except ValueError as error:
        _fail("input is not valid JSON: " + str(error))

    options = {
        "strings": args.strings,
        "tables": args.tables,
        "constants": args.constants,
        "dict_columns": args.dict_columns,
        "min_rows": args.min_rows,
    }

    if args.command == "compress":
        _write_output(dumps(compress(parsed, options), indent), args.out)
        return 0

    if args.command == "decompress":
        if not is_compressed(parsed):
            _fail("input is not a compressed document")
        _write_output(dumps(decompress(parsed), indent), args.out)
        return 0

    # args.command == "stats"
    report = measure(parsed, options, source=text)
    name = os.path.basename(args.input) if args.input else "stdin"
    sys.stdout.write(_stats(report, name) + "\n")
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
