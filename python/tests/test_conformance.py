"""Agreement with the other implementations.

``SPEC.md`` asks only that an implementation restore what a conforming encoder
wrote. This asserts the stronger claim the released implementations already
meet: that they all write the same envelope for the same document, byte for
byte.

The corpus is 144 source/envelope pairs produced by the JavaScript encoder and
checked in at the path below, so this runs without Node or .NET installed. It
is shared with the .NET suite rather than duplicated, because two copies of a
conformance corpus is one copy too many.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from generator import create_random, make_value
from json_compress import compress, decompress
from json_compress.jsontext import dumps, loads

CORPUS = (
    Path(__file__).resolve().parents[2]
    / "dotnet"
    / "tests"
    / "TechStyle.JsonCompress.Tests"
    / "fixtures"
    / "conformance.json"
)


def _load_corpus() -> list:
    if not CORPUS.exists():  # pragma: no cover - only when the file is missing
        raise unittest.SkipTest("the conformance corpus is not checked out at " + str(CORPUS))
    with CORPUS.open(encoding="utf-8") as handle:
        return json.load(handle)["cases"]


class ConformanceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.cases = _load_corpus()  # type: ignore[attr-defined]

    def test_the_corpus_is_the_size_it_should_be(self) -> None:
        self.assertGreaterEqual(len(self.cases), 144)

    def test_every_envelope_the_reference_encoder_wrote_is_restored(self) -> None:
        """Clause 3 of the specification: what a conforming encoder wrote, we read."""
        failures = []
        for case in self.cases:
            try:
                restored = dumps(decompress(loads(case["compressed"])))
            except Exception as error:  # noqa: BLE001 - collect, then report together
                failures.append(case["name"] + ": threw " + repr(error))
                continue
            if restored != case["source"]:
                failures.append(
                    case["name"]
                    + ":\n  expected "
                    + case["source"]
                    + "\n  actual   "
                    + restored
                )
        self.assertEqual(failures, [], "\n\n".join(failures))

    def test_every_envelope_is_written_the_same_way(self) -> None:
        """Stronger than the specification asks: the same bytes, not merely the same value."""
        failures = []
        for case in self.cases:
            ours = dumps(compress(loads(case["source"])))
            if ours != case["compressed"]:
                failures.append(
                    case["name"]
                    + ":\n  reference " + case["compressed"]
                    + "\n  python    " + ours
                )
        self.assertEqual(failures, [], "\n\n".join(failures))

    def test_the_seeded_cases_are_reproduced_from_their_seeds(self) -> None:
        """The generator is shared, so a seed names the same document everywhere."""
        checked = 0
        for case in self.cases:
            if "seed" not in case:
                continue
            value = make_value(create_random(case["seed"]), 5)
            self.assertEqual(dumps(value), case["source"], "seed " + str(case["seed"]))
            checked += 1
        self.assertGreater(checked, 0, "the corpus carries no seeded cases")


if __name__ == "__main__":
    unittest.main()
