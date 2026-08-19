"""Round-trip tests.

The contract the whole library rests on is that ``decompress(compress(x))``
gives back ``x`` - not something equivalent, the same document, keys in the same
order. Everything here checks that, on shapes chosen to poke at the places where
the format could plausibly lose information.
"""

from __future__ import annotations

import itertools
import unittest
from typing import Any

from json_compress import compress, decompress, parse, stringify
from json_compress.jsontext import dumps, loads
from json_compress.normalize import normalize


def as_json(value: Any) -> Any:
    """What JSON itself would make of a value - the standard the format meets."""
    return loads(dumps(normalize(value)))


class RoundTripCase(unittest.TestCase):
    def round_trips(self, value: Any, note: str = "value") -> Any:
        expected = as_json(value)
        restored = decompress(compress(value))
        self.assertEqual(restored, expected, note)
        self.assertEqual(
            dumps(restored),
            dumps(expected),
            note + ": keys came back in a different order",
        )
        return restored


class ScalarTests(RoundTripCase):
    def test_scalars_survive_intact(self) -> None:
        for value in [None, True, False, 0, -1, 1.5, 1e21, -1e-7, "", "plain"]:
            self.round_trips(value, repr(value))

    def test_the_tilde_escape_is_reversible(self) -> None:
        for value in ["~", "~~", "~~~", "~0", "~z", "~ref", "~t", "~n", "a~b"]:
            self.round_trips(value, repr(value))
        self.round_trips(["~", "~", "~", "~0", "~0", "~0"], "repeated tilde strings")
        self.round_trips({"~key": "~value"}, "tilde in both key and value")

    def test_unicode_and_escapes_survive(self) -> None:
        self.round_trips(
            {
                "emoji": "compressed \U0001F4E6 nicely",
                "quote": 'she said "hello"',
                "backslash": "C:\\Users\\path",
                "newline": "one\ntwo\tthree",
                "surrogate": "\U0001F600\U0001F601",
                "arabic": "مرحبا بالعالم",
                "nul": "a\u0000b",
            },
            "awkward text",
        )


class ContainerTests(RoundTripCase):
    def test_empty_containers_survive(self) -> None:
        self.round_trips([], "empty array")
        self.round_trips({}, "empty object")
        self.round_trips([[], {}, [[]], [{}]], "nested empties")
        self.round_trips([{}, {}, {}], "array of empty objects")

    def test_arrays_of_like_shaped_objects_become_tables_and_come_back(self) -> None:
        value = [
            {"id": 1, "name": "Ada", "role": "admin"},
            {"id": 2, "name": "Bob", "role": "admin"},
            {"id": 3, "name": "Cy", "role": "user"},
        ]
        self.assertIn("~t", dumps(compress(value)))
        self.round_trips(value, "uniform rows")

    def test_rows_with_different_key_sets_survive(self) -> None:
        self.round_trips([{"a": 1}, {"b": 2}, {"a": 3, "b": 4}], "disjoint keys")
        self.round_trips(
            [{"a": 1, "b": 2, "c": 3}, {"a": 4}, {"a": 5, "c": 6}], "ragged rows"
        )

    def test_rows_whose_key_orders_disagree_stay_objects(self) -> None:
        value = [{"a": 1, "b": 2}, {"b": 3, "a": 4}]
        self.assertNotIn("~t", dumps(compress(value)))
        self.round_trips(value, "contradicting key orders")

    def test_arrays_of_arrays_are_left_alone(self) -> None:
        self.round_trips([[1, 2], [3, 4], [5, 6]], "matrix")
        self.round_trips([[{"a": 1}, {"a": 2}], [{"a": 3}, {"a": 4}]], "arrays of tables")

    def test_mixed_arrays_are_left_alone(self) -> None:
        self.round_trips([1, "two", None, {"three": 3}, [4]], "mixed")
        self.round_trips([{"a": 1}, 2, {"a": 3}], "objects interrupted by a scalar")

    def test_nested_tables_survive(self) -> None:
        self.round_trips(
            {
                "groups": [
                    {"name": "a", "items": [{"q": 1, "p": 2}, {"q": 3, "p": 4}]},
                    {"name": "b", "items": [{"q": 5, "p": 6}, {"q": 7, "p": 8}]},
                ]
            },
            "tables inside tables",
        )

    def test_deep_nesting_survives(self) -> None:
        value: Any = {"leaf": True}
        for level in range(200):
            value = {"level" + str(level): value}
        self.round_trips(value, "200 levels")

    def test_nesting_deep_enough_to_matter_survives(self) -> None:
        # The encoder walks the document once per pass, so the depth it reaches
        # is the interpreter's own recursion limit rather than a fraction of it.
        # 500 is well past anything a real document carries and well inside that.
        value: Any = {"leaf": True}
        for level in range(500):
            value = {"level" + str(level): value, "sibling": level}
        self.round_trips(value, "500 levels")

        nested: Any = [1]
        for level in range(500):
            nested = [nested, level]
        self.round_trips(nested, "500 nested arrays")

    def test_nesting_past_the_recursion_limit_fails_cleanly(self) -> None:
        # Deeper than the interpreter can walk is a RecursionError, the same way
        # json.dumps reports it - not a crash and not a half-written document.
        value: Any = {"leaf": True}
        for level in range(5000):
            value = {"level" + str(level): value}
        with self.assertRaises(RecursionError):
            compress(value)


class HoleTests(RoundTripCase):
    def test_a_missing_key_is_not_the_same_as_a_null_one(self) -> None:
        value = [
            {"a": 1, "b": None},
            {"a": 2},
            {"a": 3, "b": None},
            {"a": 4, "b": 5},
        ]
        restored = decompress(compress(value))
        self.assertIn("b", restored[0], "explicit null kept")
        self.assertIsNone(restored[0]["b"])
        self.assertNotIn("b", restored[1], "absent key stays absent")
        self.assertEqual(restored, value)

    def test_a_missing_key_is_not_the_same_as_a_null_one_in_a_dictionary_column(
        self,
    ) -> None:
        value = [
            {"k": "x", "n": 1},
            {"k": None, "n": 2},
            {"n": 3},
            {"k": "x", "n": 4},
            {"k": "y", "n": 5},
            {"k": "x", "n": 6},
        ]
        restored = decompress(compress(value))
        self.assertIsNone(restored[1]["k"])
        self.assertIn("k", restored[1])
        self.assertNotIn("k", restored[2])
        self.assertEqual(restored, value)

    def test_a_raw_column_may_hold_minus_one(self) -> None:
        # -1 is what a dictionary column writes for an absent key. A raw column
        # that happens to hold the number must not be mistaken for one.
        value = [{"a": 1, "b": -1}, {"a": 2, "b": 5}, {"a": 3, "b": -1}]
        self.round_trips(value, "minus one in a raw column")
        self.round_trips([{"a": -1}, {"a": -1}, {"a": 2}], "minus one in the last column")


class ColumnTests(RoundTripCase):
    def test_constant_columns_are_restored_on_every_row(self) -> None:
        value = [
            {"type": "order", "id": 1},
            {"type": "order", "id": 2},
            {"type": "order", "id": 3},
        ]
        envelope = compress(value)
        self.assertIn("order", dumps(envelope))
        self.assertEqual(decompress(envelope), value)

    def test_a_table_where_every_column_is_constant_keeps_its_row_count(self) -> None:
        value = [{"a": 1, "b": "x"}] * 4
        envelope = compress(value)
        self.assertIn("~n", dumps(envelope))
        self.assertEqual(decompress(envelope), value)

    def test_repeated_strings_are_catalogued_and_restored(self) -> None:
        value = [
            {
                "id": i,
                "country": "United Kingdom" if i % 2 else "Republic of Ireland",
                "note": "a reasonably long repeated note value",
            }
            for i in range(50)
        ]
        envelope = compress(value)
        self.assertIn("United Kingdom", envelope["s"])
        self.assertEqual(decompress(envelope), value)

        # The note never changes, so it is hoisted out of the rows entirely and
        # written down exactly once.
        self.assertEqual(dumps(envelope).count("a reasonably long repeated note value"), 1)


class CatalogueTests(RoundTripCase):
    def test_keys_that_look_like_tokens_are_not_confused_with_tokens(self) -> None:
        self.round_trips(
            {"0": 1, "1": 2, "a": 3, "z": 4, "10": 5, "~t": 6, "~r": 7},
            "token-shaped keys",
        )

    def test_a_key_called_proto_is_ordinary_data(self) -> None:
        value = loads('{"__proto__": 1, "safe": 2}')
        restored = self.round_trips(value, "__proto__ as a key")
        self.assertEqual(list(restored), ["__proto__", "safe"])

    def test_a_large_key_set_pushes_tokens_past_one_character(self) -> None:
        value = {"key_number_" + str(i): i for i in range(5000)}
        self.round_trips(value, "5000 keys")

    def test_a_large_string_catalogue_pushes_references_past_one_character(self) -> None:
        distinct = ["a distinctive value number " + str(i) for i in range(500)]
        self.round_trips(distinct + distinct, "500 repeated strings")


class OptionTests(RoundTripCase):
    def test_stringify_and_parse_agree_with_compress_and_decompress(self) -> None:
        value = {"users": [{"id": 1, "name": "Ada"}, {"id": 2, "name": "Bob"}]}
        self.assertEqual(parse(stringify(value)), value)
        self.assertEqual(stringify(value), dumps(compress(value)))

    def test_every_combination_of_options_round_trips(self) -> None:
        value = {
            "rows": [
                {
                    "id": i,
                    "status": "open" if i % 2 else "closed",
                    "fixed": "always the same",
                    **({} if i % 5 == 0 else {"note": "note " + str(i % 3)}),
                }
                for i in range(20)
            ]
        }
        expected = as_json(value)

        for strings, tables, constants, dict_columns in itertools.product(
            [True, False], repeat=4
        ):
            options = {
                "strings": strings,
                "tables": tables,
                "constants": constants,
                "dict_columns": dict_columns,
            }
            self.assertEqual(decompress(compress(value, options)), expected, str(options))

    def test_min_rows_controls_when_a_table_is_built(self) -> None:
        two = [{"a": 1}, {"a": 2}]
        self.assertIn("~t", dumps(compress(two, min_rows=2)))
        self.assertNotIn("~t", dumps(compress(two, min_rows=3)))
        self.assertEqual(decompress(compress(two, min_rows=3)), two)

    def test_max_hole_ratio_keeps_very_sparse_arrays_out_of_tables(self) -> None:
        sparse = [{"a": 1}, {"b": 2}, {"c": 3}, {"d": 4}, {"e": 5}]
        self.assertNotIn("~t", dumps(compress(sparse)))
        self.assertEqual(decompress(compress(sparse)), sparse)
        self.assertEqual(decompress(compress(sparse, max_hole_ratio=1)), sparse)


if __name__ == "__main__":
    unittest.main()
