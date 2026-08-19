"""The public surface: the helpers around the codec, and the way it behaves at
the edges - bad input, values JSON itself cannot carry, documents from another
version of the format.
"""

from __future__ import annotations

import unittest
from dataclasses import FrozenInstanceError, dataclass
from datetime import date, datetime, timezone
from decimal import Decimal
from enum import Enum
from uuid import UUID

from json_compress import (
    DEFAULT_OPTIONS,
    FORMAT_VERSION,
    CompressOptions,
    DecodeError,
    beautify,
    byte_length,
    compress,
    create_codec,
    decompress,
    decompress_if_needed,
    gunzip,
    gzip_bytes,
    gzip_size,
    is_compressed,
    measure,
    minify,
    parse,
    stringify,
)
from json_compress.jsontext import dumps, loads


class EnvelopeTests(unittest.TestCase):
    def test_the_envelope_carries_a_version_and_a_key_catalogue(self) -> None:
        envelope = compress({"hello": "world"})
        self.assertEqual(envelope["jc"], FORMAT_VERSION)
        self.assertEqual(envelope["k"], ["hello"])
        self.assertTrue(is_compressed(envelope))

    def test_the_string_catalogue_is_left_out_when_nothing_is_worth_cataloguing(
        self,
    ) -> None:
        self.assertNotIn("s", compress({"a": 1}))

    def test_is_compressed_rejects_everything_that_is_not_an_envelope(self) -> None:
        for value in [None, 0, "x", [], {}, {"jc": 2, "k": []}, {"jc": 1}]:
            self.assertFalse(is_compressed(value), repr(value))

    def test_decompress_if_needed_passes_ordinary_values_straight_through(self) -> None:
        plain = {"a": 1}
        self.assertIs(decompress_if_needed(plain), plain)
        self.assertEqual(decompress_if_needed(compress(plain)), plain)


class DecoderTests(unittest.TestCase):
    def test_decompress_refuses_documents_it_cannot_read(self) -> None:
        for envelope, pattern in [
            (None, "envelope object"),
            ([1, 2], "envelope object"),
            ({"k": []}, "not a compressed document"),
            ({"jc": 99, "k": []}, "version 99 is not supported"),
            ({"jc": 1}, "key catalogue"),
            ({"jc": 1, "k": [], "s": "nope"}, "string catalogue"),
        ]:
            with self.assertRaisesRegex(DecodeError, pattern):
                decompress(envelope)

    def test_decompress_refuses_references_that_point_nowhere(self) -> None:
        for envelope, pattern in [
            ({"jc": 1, "k": [], "d": {"A": 1}}, "key token"),
            ({"jc": 1, "k": ["a"], "s": [], "d": "~5"}, "string reference"),
            ({"jc": 1, "k": ["a"], "d": "~"}, "hole marker"),
            ({"jc": 1, "k": ["a"], "d": {"~t": "x"}}, "column list"),
            ({"jc": 1, "k": ["a"], "d": {"~t": [7]}}, "column index"),
            (
                {"jc": 1, "k": ["a"], "d": {"~t": [0], "~e": [1, 1]}},
                "encoding list does not match",
            ),
            (
                {"jc": 1, "k": ["a"], "s": [], "d": {"~t": [0], "~e": [1], "~r": [[3]]}},
                "dictionary index",
            ),
            (
                {"jc": 1, "k": ["a"], "d": {"~t": [0], "~e": [2], "~n": 1}},
                "fewer constants",
            ),
            ({"jc": 1, "k": ["a"], "d": {"~t": [0], "~r": "no"}}, "rows are not an array"),
            ({"jc": 1, "k": ["a"], "d": {"~t": [0], "~r": ["no"]}}, "row 0 is not an array"),
        ]:
            with self.assertRaisesRegex(DecodeError, pattern):
                decompress(envelope)

    def test_a_decode_error_is_also_a_value_error(self) -> None:
        with self.assertRaises(ValueError):
            decompress({"jc": 1})


class NormalisationTests(unittest.TestCase):
    def test_circular_structures_are_refused_not_hung_on(self) -> None:
        value: dict = {"name": "loop"}
        value["self"] = value
        with self.assertRaisesRegex(ValueError, "circular"):
            compress(value)

        rows: list = [1]
        rows.append(rows)
        with self.assertRaisesRegex(ValueError, "circular"):
            compress(rows)

    def test_a_shared_reference_is_not_mistaken_for_a_cycle(self) -> None:
        shared = {"a": 1}
        self.assertEqual(
            decompress(compress({"x": shared, "y": shared})),
            {"x": {"a": 1}, "y": {"a": 1}},
        )

    def test_values_json_cannot_carry_are_treated_the_way_json_treats_them(self) -> None:
        restored = decompress(
            compress(
                {
                    "nan": float("nan"),
                    "inf": float("inf"),
                    "negative_inf": float("-inf"),
                    "negative_zero": -0.0,
                    "list": [float("nan"), 1],
                }
            )
        )
        self.assertEqual(
            restored,
            {
                "nan": None,
                "inf": None,
                "negative_inf": None,
                "negative_zero": 0,
                "list": [None, 1],
            },
        )
        self.assertEqual(dumps(restored["negative_zero"]), "0")

    def test_the_standard_library_types_are_converted(self) -> None:
        class Colour(Enum):
            RED = "red"

        @dataclass
        class Money:
            pence: int
            currency: str

        value = {
            "when": datetime(2026, 8, 18, 10, 20, 30, tzinfo=timezone.utc),
            "day": date(2026, 8, 18),
            "amount": Decimal("12.50"),
            "id": UUID("0e6b0d4c-1f2a-4f7e-9c3a-5b1d2e3f4a5b"),
            "colour": Colour.RED,
            "price": Money(1250, "GBP"),
            "rows": (1, 2, 3),
        }
        self.assertEqual(
            decompress(compress(value)),
            {
                "when": "2026-08-18T10:20:30+00:00",
                "day": "2026-08-18",
                "amount": 12.5,
                "id": "0e6b0d4c-1f2a-4f7e-9c3a-5b1d2e3f4a5b",
                "colour": "red",
                "price": {"pence": 1250, "currency": "GBP"},
                "rows": [1, 2, 3],
            },
        )

    def test_for_json_is_honoured(self) -> None:
        class Money:
            def __init__(self, amount: float) -> None:
                self.amount = amount

            def for_json(self) -> dict:
                return {"pence": round(self.amount * 100)}

        self.assertEqual(
            decompress(compress({"price": Money(12.5)})), {"price": {"pence": 1250}}
        )

    def test_values_with_no_json_form_are_refused_with_a_clear_message(self) -> None:
        for value, pattern in [
            ({"tags": {"a", "b"}}, "set has no JSON form"),
            ({"blob": b"bytes"}, "bytes have no JSON form"),
            ({"fn": len}, "has no JSON form"),
        ]:
            with self.assertRaisesRegex(TypeError, pattern):
                compress(value)

    def test_keys_that_are_not_strings_are_converted_the_way_json_converts_them(
        self,
    ) -> None:
        self.assertEqual(
            decompress(compress({1: "a", None: "b", 2.5: "c", False: "d"})),
            {"1": "a", "null": "b", "2.5": "c", "false": "d"},
        )
        with self.assertRaisesRegex(TypeError, "cannot be an object key"):
            compress({(1, 2): "a"})


class OptionTests(unittest.TestCase):
    def test_options_are_validated(self) -> None:
        with self.assertRaises(TypeError):
            compress({}, "nope")
        with self.assertRaises(ValueError):
            compress({}, min_rows=0)
        with self.assertRaises(ValueError):
            compress({}, max_hole_ratio=2)
        with self.assertRaises(ValueError):
            compress({}, min_string_count=1)
        with self.assertRaises(TypeError):
            compress({}, no_such_option=1)

    def test_the_defaults_are_frozen(self) -> None:
        self.assertTrue(DEFAULT_OPTIONS.tables)
        with self.assertRaises(FrozenInstanceError):
            DEFAULT_OPTIONS.tables = False  # type: ignore[misc]

    def test_options_can_be_given_as_an_object_a_mapping_or_keywords(self) -> None:
        value = [{"a": 1}, {"a": 2}, {"a": 3}]
        without_tables = dumps(compress(value, CompressOptions(tables=False)))
        self.assertEqual(dumps(compress(value, {"tables": False})), without_tables)
        self.assertEqual(dumps(compress(value, tables=False)), without_tables)
        self.assertNotIn("~t", without_tables)


class TextTests(unittest.TestCase):
    def test_stringify_and_parse_are_the_string_level_pair(self) -> None:
        value = {"a": [1, 2, 3]}
        text = stringify(value)
        self.assertIsInstance(text, str)
        self.assertEqual(parse(text), value)

    def test_minify_and_beautify_only_touch_whitespace(self) -> None:
        self.assertEqual(minify('{\n  "a" : 1,\n  "b" : [1, 2]\n}'), '{"a":1,"b":[1,2]}')
        self.assertEqual(beautify('{"a":1}'), '{\n  "a": 1\n}')
        self.assertEqual(beautify('{"a":1}', 4), '{\n    "a": 1\n}')
        self.assertEqual(beautify('{"a":1}', "\t"), '{\n\t"a": 1\n}')
        self.assertEqual(beautify('{"a":{},"b":[]}'), '{\n  "a": {},\n  "b": []\n}')

    def test_numbers_are_written_the_way_every_implementation_writes_them(self) -> None:
        for text in ["1e+21", "1e-7", "0.00001", "100000000000000000000", "1.5", "-0.0"]:
            self.assertEqual(minify("[" + text + "]"), "[" + _expected(text) + "]", text)

    def test_the_constants_json_has_no_spelling_for_are_refused(self) -> None:
        for text in ["NaN", "Infinity", "[-Infinity]"]:
            with self.assertRaises(ValueError):
                minify(text)


def _expected(text: str) -> str:
    return {"-0.0": "0"}.get(text, text)


class MeasureTests(unittest.TestCase):
    def test_measure_reports_a_real_saving_on_a_record_set(self) -> None:
        value = [
            {
                "id": i,
                "firstName": "Given" + str(i),
                "lastName": "Family",
                "department": "engineering" if i % 2 else "operations",
            }
            for i in range(100)
        ]
        report = measure(value)
        self.assertEqual(report.minified, byte_length(dumps(value)))
        self.assertLess(report.compressed, report.minified)
        self.assertEqual(report.saved, report.minified - report.compressed)
        self.assertGreater(report.percent, 40)
        self.assertLess(report.ratio, 1)
        self.assertIsInstance(report.gzip_minified, int)
        self.assertIsInstance(report.gzip_compressed, int)

    def test_measure_can_be_told_what_the_input_really_weighed(self) -> None:
        report = measure({"a": 1}, source='{\n  "a": 1\n}')
        self.assertEqual(report.original, 12)
        self.assertEqual(report.minified, 7)

    def test_gzip_can_be_skipped(self) -> None:
        report = measure({"a": 1}, gzip=False)
        self.assertIsNone(report.gzip_minified)
        self.assertIsNone(report.gzip_compressed)


class ByteTests(unittest.TestCase):
    def test_byte_length_counts_utf8_bytes_not_code_units(self) -> None:
        self.assertEqual(byte_length("abc"), 3)
        self.assertEqual(byte_length("£"), 2)
        self.assertEqual(byte_length("\U0001F600"), 4)

    def test_gzip_round_trips(self) -> None:
        text = dumps({"hello": "world", "list": [1, 2, 3]})
        data = gzip_bytes(text)
        self.assertIsInstance(data, bytes)
        self.assertEqual(gunzip(data), text)
        self.assertEqual(gzip_size(text), len(data))

    def test_gzip_output_is_stable_across_runs(self) -> None:
        self.assertEqual(gzip_bytes("hello"), gzip_bytes("hello"))


class CodecTests(unittest.TestCase):
    def test_a_codec_binds_its_options_once(self) -> None:
        codec = create_codec(tables=False)
        value = [{"a": 1}, {"a": 2}, {"a": 3}]
        self.assertNotIn("~t", dumps(codec.compress(value)))
        self.assertEqual(codec.decompress(codec.compress(value)), value)
        self.assertEqual(codec.parse(codec.stringify(value)), value)
        self.assertFalse(codec.options.tables)
        self.assertTrue(codec.options.strings)

    def test_a_codec_measures_with_its_own_options(self) -> None:
        codec = create_codec(tables=False)
        value = [{"a": i, "b": "x"} for i in range(20)]
        self.assertGreater(codec.measure(value).compressed, measure(value).compressed)


class LargeDocumentTests(unittest.TestCase):
    def test_a_realistic_record_set_is_meaningfully_smaller(self) -> None:
        value = [
            {
                "id": i,
                "reference": "REF-" + str(10000 + i),
                "status": ["open", "closed", "pending"][i % 3],
                "region": ["north", "south", "east", "west"][i % 4],
                "amount": (i * 137) / 100,
                "currency": "GBP",
                "createdAt": "2026-08-18T09:30:00.000Z",
            }
            for i in range(1000)
        ]
        report = measure(value, gzip=False)
        self.assertGreater(report.percent, 60)
        self.assertEqual(loads(stringify(value)), compress(value))
        self.assertEqual(parse(stringify(value)), value)


if __name__ == "__main__":
    unittest.main()
