"""The tokens the format is built on, and the column ordering that keeps a
packed table faithful to the rows that fed it.
"""

from __future__ import annotations

import unittest

from json_compress.column_order import order_columns
from json_compress.jsontext import utf16_length
from json_compress.tokens import (
    base62_length,
    from_base62,
    from_key_token,
    to_base62,
    to_key_token,
)


class Base62Tests(unittest.TestCase):
    def test_tokens_round_trip(self) -> None:
        for n in [0, 1, 61, 62, 63, 3843, 3844, 238327, 238328, 1000000]:
            self.assertEqual(from_base62(to_base62(n)), n, "index " + str(n))
            self.assertEqual(
                base62_length(n), len(to_base62(n)), "length of " + str(n)
            )

    def test_the_first_and_last_single_digits(self) -> None:
        self.assertEqual(to_base62(0), "0")
        self.assertEqual(to_base62(61), "z")
        self.assertEqual(to_base62(62), "10")

    def test_anything_that_is_not_a_token_is_refused(self) -> None:
        for token in ["", "~", "a-b", "~0", " "]:
            self.assertEqual(from_base62(token), -1, repr(token))


class KeyTokenTests(unittest.TestCase):
    def test_tokens_round_trip_and_never_look_like_array_indices(self) -> None:
        for n in range(20000):
            token = to_key_token(n)
            self.assertEqual(from_key_token(token), n, "index " + str(n))
            self.assertTrue(
                token[0].isascii() and token[0].isalpha(),
                "token " + token + " starts with a letter",
            )
            # A key that reads back as its own number would be hoisted to the
            # front of a JavaScript object, silently reordering the document.
            self.assertNotEqual(_as_number_text(token), token, "token " + token)

    def test_tokens_are_compact_for_the_sizes_that_matter(self) -> None:
        self.assertEqual(len(to_key_token(0)), 1)
        self.assertEqual(len(to_key_token(51)), 1)
        self.assertEqual(len(to_key_token(52)), 2)
        self.assertEqual(len(to_key_token(3275)), 2)
        self.assertEqual(len(to_key_token(3276)), 3)

    def test_anything_that_is_not_a_token_is_refused(self) -> None:
        for token in ["", "0", "~", "0a", " a"]:
            self.assertEqual(from_key_token(token), -1, repr(token))


def _as_number_text(token: str) -> str:
    try:
        return repr(int(token))
    except ValueError:
        return ""


class Utf16LengthTests(unittest.TestCase):
    def test_length_is_counted_the_way_javascript_counts_it(self) -> None:
        self.assertEqual(utf16_length(""), 0)
        self.assertEqual(utf16_length("abc"), 3)
        self.assertEqual(utf16_length("£"), 1)
        # An astral character is two code units, which is what the cost model
        # in the other implementations is calibrated against.
        self.assertEqual(utf16_length("\U0001F600"), 2)
        self.assertEqual(utf16_length("a\U0001F600b"), 4)


class ColumnOrderTests(unittest.TestCase):
    def test_ordering_respects_every_row(self) -> None:
        self.assertEqual(order_columns([{"a": 1, "b": 2}, {"a": 3, "b": 4}]), ["a", "b"])
        self.assertEqual(
            order_columns([{"a": 1, "b": 2}, {"b": 3, "c": 4}]), ["a", "b", "c"]
        )

    def test_an_order_is_found_where_one_exists(self) -> None:
        # The rows disagree about where c goes, but an order exists that suits
        # both.
        self.assertEqual(order_columns([{"a": 1, "b": 2}, {"c": 3, "a": 4}]), ["c", "a", "b"])

    def test_contradicting_rows_have_no_order(self) -> None:
        self.assertIsNone(order_columns([{"a": 1, "b": 2}, {"b": 3, "a": 4}]))

    def test_degenerate_inputs(self) -> None:
        self.assertEqual(order_columns([]), [])
        self.assertEqual(order_columns([{}, {}]), [])
        self.assertEqual(order_columns([{"a": 1}, {"a": 2}]), ["a"])


if __name__ == "__main__":
    unittest.main()
