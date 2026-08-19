package jsoncompress

// The reader, on the cases that separate a JSON parser from something that
// merely looks like one.

import (
	"encoding/json"
	"strconv"
	"strings"
	"testing"
)

func TestStringsAreReadTheWayEncodingJsonReadsThem(t *testing.T) {
	escape := func(code int) string { return string(rune(92)) + "u" + hexFour(code) }
	character := func(code int) string { return string(rune(code)) }

	cases := []struct {
		document string
		want     string
	}{
		{`"plain"`, "plain"},
		{`""`, ""},
		{`"quote \" backslash \\ solidus \/"`, "quote \" backslash \\ solidus /"},
		{`"\b\f\n\r\t"`, "\b\f\n\r\t"},
		{`"` + escape(0) + `"`, character(0)},
		{`"` + escape(0x1F) + `"`, character(0x1F)},
		{`"` + escape(0x00E9) + `"`, character(0x00E9)},
		{`"` + escape(0x2028) + `"`, character(0x2028)},
		// A surrogate pair, written as two escapes, is one character.
		{`"` + escape(0xD83D) + escape(0xDE00) + `"`, character(0x1F600)},
		// An unpaired surrogate is not, and becomes the replacement character -
		// which is the only thing a Go string can hold and what encoding/json
		// makes of it too.
		{`"` + escape(0xD800) + `"`, character(0xFFFD)},
		{`"` + escape(0xDC00) + `"`, character(0xFFFD)},
		// A low surrogate followed by a high one is two unpaired halves, not a
		// pair. Reading the second escape must not swallow the wrong bytes.
		{`"` + escape(0xDC00) + escape(0xD800) + `"`, character(0xFFFD) + character(0xFFFD)},
		{`"` + escape(0xD800) + escape(0xD800) + `"`, character(0xFFFD) + character(0xFFFD)},
		{`"` + escape(0xD800) + `x"`, character(0xFFFD) + "x"},
		{`"a` + escape(0xD83D) + escape(0xDE00) + `b"`, "a" + character(0x1F600) + "b"},
		{"\"raw \xF0\x9F\x98\x80 emoji\"", "raw " + character(0x1F600) + " emoji"},
	}

	for _, item := range cases {
		value, err := ParseJSONString(item.document)
		if err != nil {
			t.Errorf("%s: %v", item.document, err)
			continue
		}
		actual, ok := value.(string)
		if !ok {
			t.Errorf("%s: expected a string, got %T", item.document, value)
			continue
		}
		if actual != item.want {
			t.Errorf("%s: expected %q, got %q", item.document, item.want, actual)
		}

		// And the standard library agrees, which is the real bar.
		var reference string
		if err := json.Unmarshal([]byte(item.document), &reference); err != nil {
			t.Errorf("%s: encoding/json refused it: %v", item.document, err)
			continue
		}
		if actual != reference {
			t.Errorf("%s: encoding/json read %q, we read %q", item.document, reference, actual)
		}
	}
}

func hexFour(code int) string {
	out := []byte("0000")
	for i := 3; i >= 0; i-- {
		out[i] = hexDigits[code&0xF]
		code >>= 4
	}
	return string(out)
}

func TestTheReaderRefusesWhatJSONHasNoSpellingFor(t *testing.T) {
	documents := []string{
		"", " ", "NaN", "Infinity", "-Infinity", "undefined", "None", "True",
		"{a:1}", `{'a':1}`, `{"a":1,}`, `[1,]`, `[1 2]`, `{"a" 1}`, `{"a":}`,
		`01`, `-`, `.5`, `1.`, `1e`, `1e+`, `+1`, `0x10`,
		`"unterminated`, `"\q"`, `"\u12"`, `"\uZZZZ"`, "\"\x01\"",
		`{"a":1} {"b":2}`, `[1][2]`, `1e999`, `-1e999`,
		`{"a":1`, `[1`, `{`, `[`, `"`,
	}

	for _, document := range documents {
		_, err := ParseJSONString(document)
		if err == nil {
			t.Errorf("%q should not parse", document)
			continue
		}
		if !strings.HasPrefix(err.Error(), "json-compress: ") {
			t.Errorf("%q: unexpected message %q", document, err)
		}

		// The standard library refuses it too - the reader is hand-written for
		// speed, not to be more permissive.
		var reference any
		if json.Unmarshal([]byte(document), &reference) == nil {
			t.Errorf("%q: encoding/json accepted what we refused", document)
		}
	}
}

func TestTheReaderAcceptsWhatEncodingJsonAccepts(t *testing.T) {
	documents := []string{
		"{}", "[]", "null", "true", "false", "0", "-0", "1e10", "-1.5E-3", `"x"`,
		"  {  \"a\" : [ 1 , 2 ] }  ", `{"a":{"b":{"c":[]}}}`, `{"":1}`, `{"a":1,"a":2}`,
		"[[[[[1]]]]]", `{"a": 1e+10}`, "1e308", "5e-324", "9007199254740993",
		"\t\r\n[1]\t\r\n", `{"A":1}`,
		// Underflow to zero is not an error in Go, nor in encoding/json, nor
		// here; only a magnitude too large to hold is.
		"1e-999", "1e-400",
	}

	for _, document := range documents {
		if _, err := ParseJSONString(document); err != nil {
			t.Errorf("%q should parse: %v", document, err)
		}
		var reference any
		if err := json.Unmarshal([]byte(document), &reference); err != nil {
			t.Errorf("%q: encoding/json refused it: %v", document, err)
		}
	}
}

func TestARepeatedKeyKeepsItsPlaceAndTakesTheLastValue(t *testing.T) {
	value, err := ParseJSONString(`{"a":1,"b":2,"a":3}`)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, value); actual != `{"a":3,"b":2}` {
		t.Errorf("expected the first position and the last value, got %s", actual)
	}
}

func TestAnObjectPastTheIndexThresholdStillKeepsItsOrder(t *testing.T) {
	// Object switches from scanning its keys to hashing them at a threshold, so
	// both sides of it are worth a look.
	for _, count := range []int{1, indexThreshold - 1, indexThreshold, indexThreshold + 1, 200} {
		document := &strings.Builder{}
		document.WriteByte('{')
		for i := 0; i < count; i++ {
			if i > 0 {
				document.WriteByte(',')
			}
			// Descending names, so a sorted order would be an obvious failure.
			document.WriteString(`"k` + hexFour(count-i) + `":` + strconv.Itoa(i))
		}
		document.WriteByte('}')

		value, err := ParseJSONString(document.String())
		if err != nil {
			t.Fatalf("%d keys: %v", count, err)
		}
		if actual := text(t, value); actual != document.String() {
			t.Errorf("%d keys did not come back as written\n  expected %s\n  actual   %s",
				count, document.String(), actual)
		}

		object := value.(*Object)
		if object.Len() != count {
			t.Errorf("%d keys: Len reported %d", count, object.Len())
		}
		for i := 0; i < count; i++ {
			key, item := object.At(i)
			if got, present := object.Get(key); !present || got != item {
				t.Errorf("%d keys: %q reads back as %v, not %v", count, key, got, item)
			}
		}

		// Deleting the first key must not disturb what is behind it.
		if count > 1 {
			first, _ := object.At(0)
			second, _ := object.At(1)
			object.Delete(first)
			if object.Has(first) {
				t.Errorf("%d keys: %q survived deletion", count, first)
			}
			if key, _ := object.At(0); key != second {
				t.Errorf("%d keys: expected %q to move up, got %q", count, second, key)
			}
			if _, present := object.Get(second); !present {
				t.Errorf("%d keys: %q was lost when %q was deleted", count, second, first)
			}
		}
	}
}
