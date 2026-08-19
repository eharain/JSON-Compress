"""Randomised round-trip testing.

Hand-written cases only cover the shapes somebody thought of. This generates
documents from a seeded pseudo-random source - so a failure is reproducible from
the seed printed with it - and insists that every one survives the round trip
character for character against what JSON itself would produce.

The generator is shared with the other implementations, so seed 41 is the same
document here as it is in the JavaScript suite.
"""

from __future__ import annotations

import itertools
import unittest

from generator import create_random, make_value
from json_compress import compress, decompress
from json_compress.jsontext import dumps


class FuzzTests(unittest.TestCase):
    def test_2000_random_documents_round_trip_character_for_character(self) -> None:
        for seed in range(1, 2001):
            value = make_value(create_random(seed), 5)
            expected = dumps(value)
            try:
                actual = dumps(decompress(compress(value)))
            except Exception as error:  # noqa: BLE001 - report the seed, then fail
                self.fail("seed " + str(seed) + " threw: " + repr(error) + "\n" + expected)

            if actual != expected:
                self.fail(
                    "seed "
                    + str(seed)
                    + " did not round-trip\n  expected "
                    + expected
                    + "\n  actual   "
                    + actual
                )

    def test_random_documents_round_trip_under_every_option_combination(self) -> None:
        combinations = [
            {
                "strings": strings,
                "tables": tables,
                "constants": constants,
                "dict_columns": dict_columns,
            }
            for strings, tables, constants, dict_columns in itertools.product(
                [True, False], repeat=4
            )
        ]

        for seed in range(1, 121):
            value = make_value(create_random(seed * 7919), 4)
            expected = dumps(value)
            for options in combinations:
                actual = dumps(decompress(compress(value, options)))
                if actual != expected:
                    self.fail(
                        "seed "
                        + str(seed)
                        + " with "
                        + str(options)
                        + " did not round-trip\n  expected "
                        + expected
                        + "\n  actual   "
                        + actual
                    )

    def test_compression_never_inflates_a_realistic_record_set(self) -> None:
        for seed in range(1, 201):
            random = create_random(seed * 104729)
            rows = 20 + int(random() * 40)
            value = [
                {
                    "id": i,
                    "reference": "REF-" + str(10000 + i),
                    "status": ["open", "closed", "pending"][int(random() * 3)],
                    "region": ["north", "south", "east", "west"][int(random() * 4)],
                    "amount": round(random() * 100000) / 100,
                    "currency": "GBP",
                    "createdAt": "2026-0"
                    + str(1 + int(random() * 9))
                    + "-15T09:30:00.000Z",
                }
                for i in range(rows)
            ]

            plain = len(dumps(value))
            packed = len(dumps(compress(value)))
            self.assertLess(
                packed,
                plain,
                "seed " + str(seed) + ": compressed " + str(packed) + " vs plain " + str(plain),
            )


if __name__ == "__main__":
    unittest.main()
