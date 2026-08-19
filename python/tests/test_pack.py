"""The standalone table pack: the columnar idea on its own, with no envelope
and no catalogue.
"""

from __future__ import annotations

import unittest

from json_compress import (
    is_packed,
    pack,
    pack_all,
    text_to_array,
    unpack,
    unpack_all,
)
from json_compress.jsontext import dumps


class PackTests(unittest.TestCase):
    def test_pack_flattens_records_and_unpack_restores_them(self) -> None:
        records = [{"id": 1, "name": "Ada"}, {"id": 2, "name": "Bob"}]
        packed = pack(records)
        self.assertEqual(
            packed,
            {"fields": ["id", "name"], "data": [[1, "Ada"], [2, "Bob"]], "count": 2},
        )
        self.assertEqual(unpack(packed), records)

    def test_pack_fills_a_missing_field_with_null(self) -> None:
        packed = pack([{"a": 1}, {"b": 2}])
        self.assertEqual(packed["fields"], ["a", "b"])
        self.assertEqual(packed["data"], [[1, None], [None, 2]])

    def test_fields_are_the_union_in_first_seen_order(self) -> None:
        packed = pack([{"b": 1}, {"a": 2, "c": 3}, {"b": 4, "a": 5}])
        self.assertEqual(packed["fields"], ["b", "a", "c"])

    def test_pack_leaves_anything_that_is_not_a_record_set_alone(self) -> None:
        self.assertIsNone(pack(None))
        self.assertEqual(pack("text"), "text")
        mixed = [1, {"a": 2}]
        self.assertIs(pack(mixed), mixed)
        self.assertEqual(pack([]), {"fields": [], "data": [], "count": 0})

    def test_unpack_leaves_anything_that_is_not_a_packed_table_alone(self) -> None:
        self.assertIsNone(unpack(None))
        self.assertEqual(unpack(42), 42)
        not_packed = {"fields": "no", "data": []}
        self.assertIs(unpack(not_packed), not_packed)
        self.assertEqual(unpack({"fields": [], "data": [], "count": 0}), [])

    def test_a_short_row_is_missing_its_last_fields_rather_than_holding_nulls(
        self,
    ) -> None:
        self.assertEqual(unpack({"fields": ["a", "b"], "data": [[1]]}), [{"a": 1}])

    def test_is_packed_recognises_the_shape(self) -> None:
        self.assertTrue(is_packed(pack([{"a": 1}])))
        self.assertFalse(is_packed([{"a": 1}]))
        self.assertFalse(is_packed(None))

    def test_pack_all_and_unpack_all_handle_several_result_sets(self) -> None:
        sets = [[{"a": 1}, {"a": 2}], [{"b": 3}]]
        packed = pack_all(sets)
        self.assertEqual(len(packed), 2)
        self.assertEqual(packed[0]["count"], 2)
        self.assertEqual(unpack_all(packed), sets)

    def test_pack_all_leaves_anything_else_alone(self) -> None:
        self.assertIsNone(pack_all(None))
        self.assertEqual(pack_all([]), [])
        self.assertEqual(pack_all("text"), "text")
        self.assertIsNone(unpack_all(None))

    def test_an_array_whose_rows_disagree_about_key_order_still_packs(self) -> None:
        # Unlike a table node, the plain pack has one field list and fills the
        # gaps, so contradicting key orders are not a problem for it.
        value = [{"a": 1, "b": 2}, {"b": 3, "a": 4}]
        self.assertIn("fields", dumps(pack(value)))
        self.assertEqual(unpack(pack(value)), [{"a": 1, "b": 2}, {"a": 4, "b": 3}])

    def test_a_key_called_proto_is_ordinary_data(self) -> None:
        records = [{"__proto__": 1, "a": 2}, {"__proto__": 3, "a": 4}]
        self.assertEqual(unpack(pack(records)), records)


class TextToArrayTests(unittest.TestCase):
    def test_it_reads_a_bracketed_integer_column(self) -> None:
        self.assertEqual(text_to_array({"ids": "[[[1,2,3]]]"}, "ids")["ids"], [1, 2, 3])

    def test_a_null_column_becomes_an_empty_list(self) -> None:
        self.assertEqual(text_to_array({"ids": None}, "ids")["ids"], [])

    def test_the_delimiters_can_be_described(self) -> None:
        record = text_to_array({"ids": "{1|2}"}, "ids", prefix=1, suffix=1, separator="|")
        self.assertEqual(record["ids"], [1, 2])

    def test_a_field_that_is_not_there_is_left_alone(self) -> None:
        self.assertEqual(text_to_array({"other": 1}, "ids"), {"other": 1})

    def test_an_empty_column_becomes_an_empty_list(self) -> None:
        self.assertEqual(text_to_array({"ids": "[[[]]]"}, "ids")["ids"], [])


if __name__ == "__main__":
    unittest.main()
