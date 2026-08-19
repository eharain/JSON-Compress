package jsoncompress

// Round-trip tests.
//
// The contract the whole library rests on is that Decompress(Compress(x)) gives
// back x - not something equivalent, the same document, keys in the same order.
// Everything here checks that, on shapes chosen to poke at the places where the
// format could plausibly lose information.

import (
	"strconv"
	"strings"
	"testing"
)

// obj builds an object from alternating keys and values, which reads closer to
// the JSON it stands for than a series of Set calls would.
func obj(pairs ...any) *Object {
	out := newObject(len(pairs) / 2)
	for i := 0; i+1 < len(pairs); i += 2 {
		out.Set(pairs[i].(string), pairs[i+1])
	}
	return out
}

func text(t *testing.T, value any) string {
	t.Helper()
	out, err := FormatJSON(value)
	if err != nil {
		t.Fatalf("could not write %v: %v", value, err)
	}
	return string(out)
}

// roundTrips insists that a value survives compression exactly - the same
// values and the same key order, which comparing the JSON text checks in one
// go.
func roundTrips(t *testing.T, value any, note string) {
	t.Helper()

	expected := text(t, value)

	envelope, err := Compress(value)
	if err != nil {
		t.Fatalf("%s: compressing failed: %v", note, err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatalf("%s: restoring failed: %v", note, err)
	}

	if actual := text(t, restored); actual != expected {
		t.Errorf("%s did not round-trip\n  expected %s\n  actual   %s", note, expected, actual)
	}
}

func envelopeText(t *testing.T, value any, options ...Options) string {
	t.Helper()
	out, err := Marshal(value, options...)
	if err != nil {
		t.Fatalf("compressing failed: %v", err)
	}
	return string(out)
}

func TestScalarsSurviveIntact(t *testing.T) {
	for _, value := range []any{nil, true, false, 0.0, -1.0, 1.5, 1e21, -1e-7, "", "plain"} {
		roundTrips(t, value, text(t, value))
	}
}

func TestTheTildeEscapeIsReversible(t *testing.T) {
	for _, value := range []string{"~", "~~", "~~~", "~0", "~z", "~ref", "~t", "~n", "a~b"} {
		roundTrips(t, value, strconv.Quote(value))
	}
	roundTrips(t, []any{"~", "~", "~", "~0", "~0", "~0"}, "repeated tilde strings")
	roundTrips(t, obj("~key", "~value"), "tilde in both key and value")
}

func TestEmptyContainersSurvive(t *testing.T) {
	roundTrips(t, []any{}, "empty array")
	roundTrips(t, NewObject(), "empty object")
	roundTrips(t, []any{[]any{}, NewObject(), []any{[]any{}}, []any{NewObject()}}, "nested empties")
	roundTrips(t, []any{NewObject(), NewObject(), NewObject()}, "array of empty objects")
}

func TestArraysOfLikeShapedObjectsBecomeTablesAndComeBack(t *testing.T) {
	rows := []any{
		obj("id", 1.0, "name", "Ada", "role", "admin"),
		obj("id", 2.0, "name", "Bob", "role", "admin"),
		obj("id", 3.0, "name", "Cy", "role", "user"),
	}
	if !strings.Contains(envelopeText(t, rows), tableColumns) {
		t.Error("expected uniform rows to be packed as a table")
	}
	roundTrips(t, rows, "uniform rows")
}

func TestRowsWithDifferentKeySetsSurvive(t *testing.T) {
	roundTrips(t, []any{obj("a", 1.0), obj("b", 2.0), obj("a", 3.0, "b", 4.0)}, "disjoint keys")
	roundTrips(t, []any{obj("a", 1.0, "b", 2.0, "c", 3.0), obj("a", 4.0), obj("a", 5.0, "c", 6.0)}, "ragged rows")
}

func TestAMissingKeyIsNotTheSameAsANullOne(t *testing.T) {
	value := []any{
		obj("a", 1.0, "b", nil),
		obj("a", 2.0),
		obj("a", 3.0, "b", nil),
		obj("a", 4.0, "b", 5.0),
	}
	restored := restore(t, value)

	first := restored[0].(*Object)
	if !first.Has("b") {
		t.Error("an explicit null should be kept")
	}
	if value, _ := first.Get("b"); value != nil {
		t.Errorf("expected null, got %v", value)
	}
	if restored[1].(*Object).Has("b") {
		t.Error("an absent key should stay absent")
	}
	roundTrips(t, value, "nulls against holes")
}

func TestAMissingKeyIsNotTheSameAsANullOneInADictionaryColumn(t *testing.T) {
	value := []any{
		obj("k", "x", "n", 1.0),
		obj("k", nil, "n", 2.0),
		obj("n", 3.0),
		obj("k", "x", "n", 4.0),
		obj("k", "y", "n", 5.0),
		obj("k", "x", "n", 6.0),
	}
	restored := restore(t, value)

	if value, present := restored[1].(*Object).Get("k"); !present || value != nil {
		t.Errorf("expected an explicit null, got %v (present %v)", value, present)
	}
	if restored[2].(*Object).Has("k") {
		t.Error("an absent key should stay absent")
	}
	roundTrips(t, value, "nulls against holes in a dictionary column")
}

// -1 is what a dictionary column writes for an absent key. A raw column that
// happens to hold that number must not be trimmed away as though the key were
// missing - the two look alike on the wire and are not the same thing.
func TestARawColumnMayHoldTheNumberADictionaryColumnUsesForAHole(t *testing.T) {
	roundTrips(t, []any{obj("a", 1.0, "b", -1.0), obj("a", 2.0, "b", 5.0)}, "minus one in a trailing raw cell")
	roundTrips(t, []any{obj("a", 1.0, "b", -1.0), obj("a", 2.0, "b", -1.0), obj("a", 3.0, "b", 7.0)}, "minus one on several rows")
	roundTrips(t, []any{obj("a", -1.0), obj("a", -1.0), obj("a", 2.0)}, "minus one in the only column")
	roundTrips(t, []any{obj("a", 1.0), obj("a", 2.0, "b", -1.0)}, "a real hole beside a real minus one")

	value := []any{obj("a", 1.0, "b", -1.0), obj("a", 2.0, "b", 5.0)}
	const expected = `{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~r":[[1,-1],[2,5]]}}`
	if actual := envelopeText(t, value); actual != expected {
		t.Errorf("the cell should be written, not trimmed\n  expected %s\n  actual   %s", expected, actual)
	}
}

func TestConstantColumnsAreRestoredOnEveryRow(t *testing.T) {
	value := []any{
		obj("type", "order", "id", 1.0),
		obj("type", "order", "id", 2.0),
		obj("type", "order", "id", 3.0),
	}
	envelope := envelopeText(t, value)
	if !strings.Contains(envelope, tableConstants) {
		t.Errorf("expected a constant column, got %s", envelope)
	}
	if strings.Count(envelope, "order") != 1 {
		t.Errorf("the constant should be written once, got %s", envelope)
	}
	roundTrips(t, value, "constant columns")
}

func TestATableWhereEveryColumnIsConstantKeepsItsRowCount(t *testing.T) {
	row := func() *Object { return obj("a", 1.0, "b", "x") }
	value := []any{row(), row(), row(), row()}
	envelope := envelopeText(t, value)
	if !strings.Contains(envelope, tableRowCount) {
		t.Errorf("expected a row count, got %s", envelope)
	}
	roundTrips(t, value, "every column constant")
}

func TestNestedTablesSurvive(t *testing.T) {
	roundTrips(t, obj("groups", []any{
		obj("name", "a", "items", []any{obj("q", 1.0, "p", 2.0), obj("q", 3.0, "p", 4.0)}),
		obj("name", "b", "items", []any{obj("q", 5.0, "p", 6.0), obj("q", 7.0, "p", 8.0)}),
	}), "tables inside tables")
}

func TestArraysOfArraysAreLeftAlone(t *testing.T) {
	roundTrips(t, []any{[]any{1.0, 2.0}, []any{3.0, 4.0}, []any{5.0, 6.0}}, "matrix")
	roundTrips(t, []any{
		[]any{obj("a", 1.0), obj("a", 2.0)},
		[]any{obj("a", 3.0), obj("a", 4.0)},
	}, "arrays of tables")
}

func TestMixedArraysAreLeftAlone(t *testing.T) {
	roundTrips(t, []any{1.0, "two", nil, obj("three", 3.0), []any{4.0}}, "mixed")
	roundTrips(t, []any{obj("a", 1.0), 2.0, obj("a", 3.0)}, "objects interrupted by a scalar")
}

func TestDeepNestingSurvives(t *testing.T) {
	var value any = obj("leaf", true)
	for i := 0; i < 200; i++ {
		value = obj("level"+strconv.Itoa(i), value)
	}
	roundTrips(t, value, "200 levels")
}

func TestRepeatedStringsAreCataloguedAndRestored(t *testing.T) {
	value := make([]any, 50)
	for i := range value {
		country := "Republic of Ireland"
		if i%2 == 1 {
			country = "United Kingdom"
		}
		value[i] = obj("id", float64(i), "country", country, "note", "a reasonably long repeated note value")
	}

	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	catalogue, _ := envelope.Get(envStrings)
	if !containsString(catalogue, "United Kingdom") {
		t.Errorf("the repeated country should have been catalogued, got %v", catalogue)
	}
	roundTrips(t, value, "50 records with repeated strings")

	// The note never changes, so it is hoisted out of the rows entirely and
	// written down exactly once.
	if count := strings.Count(envelopeText(t, value), "a reasonably long repeated note value"); count != 1 {
		t.Errorf("expected the constant note to be written once, found it %d times", count)
	}
}

func TestUnicodeAndEscapesSurvive(t *testing.T) {
	roundTrips(t, obj(
		"emoji", "compressed 📦 nicely",
		"quote", `she said "hello"`,
		"backslash", `C:\Users\path`,
		"newline", "one\ntwo\tthree",
		"surrogate", "😀😁",
		"arabic", "مرحبا بالعالم",
		"nul", "a\u0000b",
	), "awkward text")
}

func TestKeysThatLookLikeTokensAreNotConfusedWithTokens(t *testing.T) {
	roundTrips(t, obj("0", 1.0, "1", 2.0, "a", 3.0, "z", 4.0, "10", 5.0, "~t", 6.0, "~r", 7.0), "token-shaped keys")
}

func TestALargeKeySetPushesTokensPastOneCharacter(t *testing.T) {
	value := NewObject()
	for i := 0; i < 5000; i++ {
		value.Set("key_number_"+strconv.Itoa(i), float64(i))
	}
	roundTrips(t, value, "5000 keys")
}

func TestALargeStringCataloguePushesReferencesPastOneCharacter(t *testing.T) {
	value := make([]any, 0, 1000)
	for i := 0; i < 500; i++ {
		value = append(value, "a distinctive value number "+strconv.Itoa(i))
	}
	value = append(value, value...)
	roundTrips(t, value, "500 repeated strings")
}

func TestMarshalAndUnmarshalAgreeWithCompressAndDecompress(t *testing.T) {
	value := obj("users", []any{obj("id", 1.0, "name", "Ada"), obj("id", 2.0, "name", "Bob")})

	data, err := Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Unmarshal(data)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != text(t, value) {
		t.Errorf("Unmarshal(Marshal(x)) changed the document: %s", actual)
	}

	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	if other := text(t, envelope); other != string(data) {
		t.Errorf("Marshal and Compress disagree:\n  %s\n  %s", data, other)
	}
}

func TestEveryCombinationOfOptionsRoundTrips(t *testing.T) {
	rows := make([]any, 20)
	for i := range rows {
		row := obj("id", float64(i), "status", "closed", "fixed", "always the same")
		if i%2 == 1 {
			row.Set("status", "open")
		}
		if i%5 != 0 {
			row.Set("note", "note "+strconv.Itoa(i%3))
		}
		rows[i] = row
	}
	value := obj("rows", rows)
	expected := text(t, value)

	for _, options := range everyCombination() {
		envelope, err := Compress(value, options)
		if err != nil {
			t.Fatalf("%+v: %v", options, err)
		}
		restored, err := Decompress(envelope)
		if err != nil {
			t.Fatalf("%+v: %v", options, err)
		}
		if actual := text(t, restored); actual != expected {
			t.Errorf("%+v did not round-trip\n  expected %s\n  actual   %s", options, expected, actual)
		}
	}
}

func everyCombination() []Options {
	var out []Options
	for _, strings := range []bool{true, false} {
		for _, tables := range []bool{true, false} {
			for _, constants := range []bool{true, false} {
				for _, dictColumns := range []bool{true, false} {
					options := DefaultOptions()
					options.Strings = strings
					options.Tables = tables
					options.Constants = constants
					options.DictColumns = dictColumns
					out = append(out, options)
				}
			}
		}
	}
	return out
}

func TestMinRowsControlsWhenATableIsBuilt(t *testing.T) {
	two := []any{obj("a", 1.0), obj("a", 2.0)}

	packing := DefaultOptions()
	packing.MinRows = 2
	if !strings.Contains(envelopeText(t, two, packing), tableColumns) {
		t.Error("two rows should pack when MinRows is 2")
	}

	leaving := DefaultOptions()
	leaving.MinRows = 3
	if strings.Contains(envelopeText(t, two, leaving), tableColumns) {
		t.Error("two rows should not pack when MinRows is 3")
	}

	envelope, err := Compress(two, leaving)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != text(t, two) {
		t.Errorf("an unpacked array did not round-trip: %s", actual)
	}
}

func TestMaxHoleRatioKeepsVerySparseArraysOutOfTables(t *testing.T) {
	sparse := []any{obj("a", 1.0), obj("b", 2.0), obj("c", 3.0), obj("d", 4.0), obj("e", 5.0)}
	if strings.Contains(envelopeText(t, sparse), tableColumns) {
		t.Error("a mostly empty table should not be built")
	}
	roundTrips(t, sparse, "sparse rows")

	packing := DefaultOptions()
	packing.MaxHoleRatio = 1
	envelope, err := Compress(sparse, packing)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != text(t, sparse) {
		t.Errorf("a sparse table did not round-trip: %s", actual)
	}
}

func restore(t *testing.T, value any) []any {
	t.Helper()
	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}
	items, ok := restored.([]any)
	if !ok {
		t.Fatalf("expected an array, got %T", restored)
	}
	return items
}

func containsString(value any, want string) bool {
	items, ok := value.([]any)
	if !ok {
		return false
	}
	for _, item := range items {
		if text, ok := item.(string); ok && text == want {
			return true
		}
	}
	return false
}
