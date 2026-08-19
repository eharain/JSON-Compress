package jsoncompress

// Randomised round-trip testing.
//
// Hand-written cases only cover the shapes somebody thought of. This generates
// documents from a seeded pseudo-random source - so a failure is reproducible
// from the seed printed with it - and insists that every one survives the round
// trip byte for byte against what the JSON writer itself would produce.

import (
	"encoding/json"
	"strconv"
	"strings"
	"testing"
)

func TestRandomDocumentsRoundTripByteForByte(t *testing.T) {
	for seed := 1; seed <= 2000; seed++ {
		value := makeValue(createRandom(seed), 5)

		expected, err := FormatJSON(value)
		if err != nil {
			t.Fatalf("seed %d: %v", seed, err)
		}

		envelope, err := Compress(value)
		if err != nil {
			t.Fatalf("seed %d: %v\n%s", seed, err, expected)
		}
		restored, err := Decompress(envelope)
		if err != nil {
			t.Fatalf("seed %d: %v\n%s", seed, err, expected)
		}
		actual, err := FormatJSON(restored)
		if err != nil {
			t.Fatalf("seed %d: %v", seed, err)
		}

		if string(actual) != string(expected) {
			t.Fatalf("seed %d did not round-trip\n  expected %s\n  actual   %s", seed, expected, actual)
		}
	}
}

func TestRandomDocumentsRoundTripUnderEveryOptionCombination(t *testing.T) {
	combinations := everyCombination()

	for seed := 1; seed <= 120; seed++ {
		value := makeValue(createRandom(seed*7919), 4)
		expected, err := FormatJSON(value)
		if err != nil {
			t.Fatalf("seed %d: %v", seed, err)
		}

		for _, options := range combinations {
			envelope, err := Compress(value, options)
			if err != nil {
				t.Fatalf("seed %d with %+v: %v", seed, options, err)
			}
			restored, err := Decompress(envelope)
			if err != nil {
				t.Fatalf("seed %d with %+v: %v", seed, options, err)
			}
			actual, err := FormatJSON(restored)
			if err != nil {
				t.Fatalf("seed %d with %+v: %v", seed, options, err)
			}
			if string(actual) != string(expected) {
				t.Fatalf("seed %d with %+v did not round-trip\n  expected %s\n  actual   %s",
					seed, options, expected, actual)
			}
		}
	}
}

func TestCompressionNeverInflatesARealisticRecordSet(t *testing.T) {
	for seed := 1; seed <= 200; seed++ {
		random := createRandom(seed * 104729)
		rows := 20 + int(random()*40)

		value := make([]any, rows)
		for i := range value {
			value[i] = obj(
				"id", float64(i),
				"reference", "REF-"+strconv.Itoa(10000+i),
				"status", pick(random, []string{"open", "closed", "pending"}),
				"region", pick(random, []string{"north", "south", "east", "west"}),
				"amount", float64(int(random()*100000+0.5))/100,
				"currency", "GBP",
				"createdAt", "2026-0"+strconv.Itoa(1+int(random()*9))+"-15T09:30:00.000Z",
			)
		}

		plain, err := FormatJSON(value)
		if err != nil {
			t.Fatal(err)
		}
		packed, err := Marshal(value)
		if err != nil {
			t.Fatal(err)
		}
		if len(packed) >= len(plain) {
			t.Errorf("seed %d: compressed %d bytes vs plain %d", seed, len(packed), len(plain))
		}
	}
}

// FuzzRoundTrip hands the round trip to the fuzzing engine as well, which pokes
// at the parser in ways a document generator does not: truncated text, stray
// escapes, numbers with no digits.
func FuzzRoundTrip(f *testing.F) {
	f.Add(`{"a":1,"b":[1,2,3]}`)
	f.Add(`[{"id":1,"role":"admin"},{"id":2,"role":"user"}]`)
	f.Add(`["~","~~","~0"]`)
	f.Add(`{"__proto__":1,"~t":2,"10":3}`)
	f.Add(`[{"a":1,"b":null},{"a":2}]`)

	f.Fuzz(func(t *testing.T, document string) {
		value, err := ParseJSONString(document)
		if err != nil {
			return // not JSON, so nothing is promised about it
		}

		expected, err := FormatJSON(value)
		if err != nil {
			t.Fatalf("a parsed document would not write back: %v", err)
		}

		data, err := Marshal(value)
		if err != nil {
			t.Fatalf("compressing %s failed: %v", expected, err)
		}
		restored, err := Unmarshal(data)
		if err != nil {
			t.Fatalf("restoring %s failed: %v", data, err)
		}
		actual, err := FormatJSON(restored)
		if err != nil {
			t.Fatalf("restored document would not write back: %v", err)
		}

		if string(actual) != string(expected) {
			t.Fatalf("did not round-trip\n  expected %s\n  actual   %s", expected, actual)
		}
	})
}

// FuzzParseAgreesWithEncodingJson holds the reader to the standard library's
// grammar: the same documents accepted, the same documents refused, and the
// same values read out of the ones that are accepted.
//
// The reader is hand-written rather than a walk over encoding/json's token
// stream, for speed, and this is what keeps that decision honest.
func FuzzParseAgreesWithEncodingJson(f *testing.F) {
	seeds := []string{
		`{"a":1,"b":[1,2,3]}`, `[]`, `{}`, `null`, `true`, `-0`, `1e309`, `1e-400`,
		`"é😀"`, `"\ud800"`, `"𐀀"`, `"\udc00\ud800"`,
		`"tab\tquote\"slash\/"`, `{"a":1,"a":2}`, `[1,]`, `{"a":}`, `01`, `.5`, `1.`,
		`"unterminated`, "\"\x01\"", `{"a" 1}`, `[1 2]`, ` {"a":1} `, `{"a":1} x`,
	}
	for _, seed := range seeds {
		f.Add(seed)
	}

	f.Fuzz(func(t *testing.T, document string) {
		var reference any
		referenceErr := json.Unmarshal([]byte(document), &reference)
		mine, mineErr := ParseJSONString(document)

		// The two draw their nesting limits in slightly different places, and
		// neither answer is more right than the other.
		if mentionsDepth(referenceErr) || mentionsDepth(mineErr) {
			return
		}

		if (referenceErr == nil) != (mineErr == nil) {
			t.Fatalf("disagreed about %q\n  encoding/json %v\n  json-compress %v",
				document, referenceErr, mineErr)
		}
		if referenceErr != nil {
			return
		}

		// Compared through encoding/json's own writer, which sorts map keys, so
		// this asks about the values rather than the order - which the corpus
		// and the round-trip tests cover.
		expected, err := json.Marshal(reference)
		if err != nil {
			t.Fatalf("encoding/json would not write back what it read from %q: %v", document, err)
		}
		actual, err := json.Marshal(asPlainGo(mine))
		if err != nil {
			t.Fatalf("encoding/json would not write back what we read from %q: %v", document, err)
		}
		if string(actual) != string(expected) {
			t.Fatalf("read %q differently\n  encoding/json %s\n  json-compress %s",
				document, expected, actual)
		}
	})
}

func mentionsDepth(err error) bool {
	return err != nil && (strings.Contains(err.Error(), "depth") || strings.Contains(err.Error(), "deeply"))
}

// asPlainGo turns the model into what encoding/json would have decoded into, so
// the two can be compared with one writer.
func asPlainGo(value any) any {
	switch typed := value.(type) {
	case *Object:
		out := make(map[string]any, typed.Len())
		for i := 0; i < typed.Len(); i++ {
			key, item := typed.At(i)
			out[key] = asPlainGo(item)
		}
		return out
	case []any:
		out := make([]any, len(typed))
		for i, item := range typed {
			out[i] = asPlainGo(item)
		}
		return out
	default:
		return value
	}
}
