"""Saying exactly where a JSON document goes wrong."""

from __future__ import annotations

import unittest

from json_compress import locate, validate
from json_compress.jsontext import loads


class ValidDocumentTests(unittest.TestCase):
    def test_locate_agrees_with_the_parser_about_what_is_valid(self) -> None:
        documents = [
            "{}", "[]", "null", "true", "false", "0", "-0", "1e10", "-1.5E-3", '"x"',
            '  {  "a" : [ 1 , 2 ] }  ', '{"a":{"b":{"c":[]}}}', '"\\u00e9\\n\\t\\\\"',
            '{"": 1}', "[[[[[1]]]]]", '{"a": 1e+10}', '"\U0001F600"',
            '{"a":[{"b":[1,2,{"c":null}]}]}', "[1,2,3]", '{"a":"~"}',
        ]
        for text in documents:
            self.assertTrue(locate(text).valid, repr(text) + " should be valid")
            loads(text)

    def test_validate_reports_a_valid_document(self) -> None:
        self.assertTrue(validate('{"a":1}').valid)
        self.assertTrue(bool(validate('{"a":1}')))


class BrokenDocumentTests(unittest.TestCase):
    def test_locate_names_the_common_mistakes(self) -> None:
        cases = [
            ("{'a': 1}", "double quotes", 1, 2),
            ('{"a": 1,}', "trailing comma", 1, 8),
            ("[1, 2,]", "trailing comma", 1, 6),
            ('{"a": True}', "lower case", 1, 7),
            ('{"a": None}', "use null", 1, 7),
            ('{"a": 01}', "leading zero", 1, 7),
            ('{"a": 1.}', "decimal point", 1, 8),
            ('{"a": 1e}', "exponent", 1, 8),
            ('{"a": "unclosed', "never closed", 1, 7),
            ('{"a": 1', "never closed", 1, 8),
            ("[1, 2", "never closed", 1, 6),
            ("{a: 1}", "quoted property name", 1, 2),
            ('{"a" 1}', "colon", 1, 6),
            ('{"a": 1} extra', "after the end", 1, 10),
            ("", "ends here", 1, 1),
            ('{"a": "\\q"}', "not a valid escape", 1, 8),
            ('{"a": "\\u12"}', "four hexadecimal", 1, 8),
            ('{"a": -}', "expected a digit", 1, 8),
            ('["a" "b"]', "comma or a closing bracket", 1, 6),
            ('{"a": 1 "b": 2}', "comma or a closing brace", 1, 9),
        ]

        for text, fragment, line, column in cases:
            found = locate(text)
            self.assertFalse(found.valid, repr(text) + " should be invalid")
            self.assertIn(fragment, str(found.error), repr(text))
            self.assertEqual(found.line, line, repr(text) + " line")
            self.assertEqual(found.column, column, repr(text) + " column")

    def test_a_raw_control_character_inside_a_string_is_named(self) -> None:
        found = locate('{"a": "one\ntwo"}')
        self.assertFalse(found.valid)
        self.assertIn("raw control character", str(found.error))

    def test_locate_finds_the_same_line_on_a_multi_line_document(self) -> None:
        text = "\n".join(["{", '  "one": 1,', '  "two": [', "    1,", "    2,", "  ]", "}"])
        found = locate(text)
        self.assertFalse(found.valid)
        self.assertIn("trailing comma", str(found.error))
        self.assertEqual(found.line, 5)
        self.assertEqual(found.excerpt, "    2,")

    def test_validate_reports_where_a_document_goes_wrong(self) -> None:
        found = validate('{\n  "a": 1,\n  "b": oops\n}')
        self.assertFalse(found.valid)
        self.assertEqual(found.line, 3, "the bad token is on line 3")
        self.assertEqual(found.column, 8, "and starts at column 8")
        self.assertEqual(found.excerpt, '  "b": oops')

    def test_a_document_nested_too_deeply_is_reported_not_crashed_on(self) -> None:
        found = locate("[" * 20000 + "]" * 20000)
        self.assertFalse(found.valid)
        self.assertIn("nested too deeply", str(found.error))

    def test_locate_refuses_anything_that_is_not_a_string(self) -> None:
        found = locate(None)  # type: ignore[arg-type]
        self.assertFalse(found.valid)
        self.assertIn("expected a string", str(found.error))


if __name__ == "__main__":
    unittest.main()
