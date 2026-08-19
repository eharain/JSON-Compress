"""The command line.

Driven in process rather than by spawning an interpreter, so the suite stays
fast and runs the same way on every platform.
"""

from __future__ import annotations

import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from typing import Optional, Tuple

from json_compress import compress, is_compressed
from json_compress.cli import main
from json_compress.jsontext import dumps, loads

RECORDS = [
    {"id": i, "status": "open" if i % 2 else "closed", "currency": "GBP"}
    for i in range(40)
]


def run(argv: list, stdin_text: Optional[str] = None) -> Tuple[int, str, str]:
    """Run the command line, returning (code, stdout, stderr)."""
    import sys

    out, err = io.StringIO(), io.StringIO()
    saved_stdin = sys.stdin
    if stdin_text is not None:
        sys.stdin = io.StringIO(stdin_text)
    try:
        with redirect_stdout(out), redirect_stderr(err):
            try:
                code = main(argv)
            except SystemExit as exit_code:
                code = int(exit_code.code or 0)
    finally:
        sys.stdin = saved_stdin
    return code, out.getvalue(), err.getvalue()


class CommandTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "records.json"
        self.path.write_text(dumps(RECORDS), encoding="utf-8")

    def test_compress_writes_an_envelope(self) -> None:
        code, out, _ = run(["compress", str(self.path)])
        self.assertEqual(code, 0)
        self.assertTrue(is_compressed(loads(out)))
        self.assertEqual(loads(out), compress(RECORDS))

    def test_decompress_restores_the_document(self) -> None:
        code, out, _ = run(["decompress"], dumps(compress(RECORDS)))
        self.assertEqual(code, 0)
        self.assertEqual(loads(out), RECORDS)

    def test_compress_and_decompress_meet_in_the_middle(self) -> None:
        _, envelope, _ = run(["compress"], dumps(RECORDS))
        _, restored, _ = run(["decompress"], envelope)
        self.assertEqual(restored.strip(), dumps(RECORDS))

    def test_decompress_refuses_a_document_that_is_not_an_envelope(self) -> None:
        code, _, err = run(["decompress"], '{"a":1}')
        self.assertEqual(code, 1)
        self.assertIn("not a compressed document", err)

    def test_the_encoder_options_are_wired_up(self) -> None:
        _, plain, _ = run(["compress", str(self.path)])
        _, without, _ = run(["compress", str(self.path), "--no-tables"])
        self.assertIn("~t", plain)
        self.assertNotIn("~t", without)

        _, no_constants, _ = run(["compress", str(self.path), "--no-constants"])
        self.assertNotIn("~cv", no_constants)

        _, min_rows, _ = run(["compress", str(self.path), "--min-rows", "100"])
        self.assertNotIn("~t", min_rows)

    def test_output_can_go_to_a_file(self) -> None:
        target = Path(self.directory.name) / "out.json"
        code, out, _ = run(["compress", str(self.path), "-o", str(target)])
        self.assertEqual(code, 0)
        self.assertEqual(out, "")
        self.assertEqual(loads(target.read_text(encoding="utf-8")), compress(RECORDS))

    def test_pretty_indents_the_output(self) -> None:
        _, out, _ = run(["compress", str(self.path), "--pretty"])
        self.assertIn("\n  ", out)

    def test_minify_and_beautify_only_touch_whitespace(self) -> None:
        _, out, _ = run(["minify"], '{\n  "a" : 1\n}')
        self.assertEqual(out.strip(), '{"a":1}')
        _, out, _ = run(["beautify"], '{"a":1}')
        self.assertEqual(out.strip(), '{\n  "a": 1\n}')
        _, out, _ = run(["beautify", "--indent", "4"], '{"a":1}')
        self.assertEqual(out.strip(), '{\n    "a": 1\n}')

    def test_validate_reports_both_verdicts(self) -> None:
        code, out, _ = run(["validate"], '{"a":1}')
        self.assertEqual(code, 0)
        self.assertIn("valid JSON", out)

        code, _, err = run(["validate"], '{"a": oops}')
        self.assertEqual(code, 1)
        self.assertIn("line 1", err)

    def test_stats_changes_nothing_and_reports_the_saving(self) -> None:
        code, out, _ = run(["stats", str(self.path)])
        self.assertEqual(code, 0)
        self.assertIn("records.json", out)
        self.assertIn("compressed", out)
        self.assertIn("smaller than minified", out)
        self.assertEqual(self.path.read_text(encoding="utf-8"), dumps(RECORDS))

    def test_bad_input_is_refused_with_a_message(self) -> None:
        code, _, err = run(["compress"], "{not json")
        self.assertEqual(code, 1)
        self.assertIn("not valid JSON", err)

        code, _, err = run(["compress", "no-such-file.json"])
        self.assertEqual(code, 1)
        self.assertIn("cannot read", err)

    def test_no_command_prints_the_help(self) -> None:
        code, out, _ = run([])
        self.assertEqual(code, 0)
        self.assertIn("compress", out)

    def test_unicode_survives_the_round_trip_through_the_command_line(self) -> None:
        value = {"emoji": "\U0001F4E6", "arabic": "مرحبا", "tilde": "~"}
        _, envelope, _ = run(["compress"], dumps(value))
        _, restored, _ = run(["decompress"], envelope)
        self.assertEqual(loads(restored), value)


if __name__ == "__main__":
    unittest.main()
