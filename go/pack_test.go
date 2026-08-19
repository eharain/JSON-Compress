package jsoncompress

// The standalone table pack, and the tokens the format is built on.

import (
	"regexp"
	"strconv"
	"strings"
	"testing"
)

func TestPackFlattensRecordsAndUnpackRestoresThem(t *testing.T) {
	records := []any{obj("id", 1.0, "name", "Ada"), obj("id", 2.0, "name", "Bob")}

	packed, ok := Pack(records)
	if !ok {
		t.Fatal("a record set should pack")
	}
	if actual := text(t, packed); actual != `{"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}` {
		t.Errorf("unexpected packed form %s", actual)
	}

	restored, ok := Unpack(packed)
	if !ok {
		t.Fatal("a packed table should unpack")
	}
	if actual := text(t, restored); actual != text(t, records) {
		t.Errorf("expected %s, got %s", text(t, records), actual)
	}
}

func TestPackFillsAMissingFieldWithNull(t *testing.T) {
	packed, ok := Pack([]any{obj("a", 1.0), obj("b", 2.0)})
	if !ok {
		t.Fatal("a ragged record set should still pack")
	}
	if actual := text(t, packed.Fields); actual != `["a","b"]` {
		t.Errorf("unexpected fields %s", actual)
	}
	if actual := text(t, packed.Data); actual != `[[1,null],[null,2]]` {
		t.Errorf("unexpected rows %s", actual)
	}
}

func TestPackLeavesAnythingThatIsNotARecordSetAlone(t *testing.T) {
	for _, value := range []any{nil, "text", 42.0, NewObject(), []any{1.0, obj("a", 2.0)}} {
		if _, ok := Pack(value); ok {
			t.Errorf("%s should not pack", text(t, value))
		}
	}

	empty, ok := Pack([]any{})
	if !ok {
		t.Fatal("an empty record set should pack")
	}
	if actual := text(t, empty); actual != `{"fields":[],"data":[],"count":0}` {
		t.Errorf("unexpected empty table %s", actual)
	}
}

func TestPackReadsMapsAndStructsToo(t *testing.T) {
	type row struct {
		ID   int    `json:"id"`
		Name string `json:"name"`
	}
	packed, ok := Pack([]row{{1, "Ada"}, {2, "Bob"}})
	if !ok {
		t.Fatal("a slice of structs should pack")
	}
	if actual := text(t, packed); actual != `{"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}` {
		t.Errorf("unexpected packed form %s", actual)
	}
}

func TestUnpackLeavesAnythingThatIsNotAPackedTableAlone(t *testing.T) {
	for _, value := range []any{nil, 42.0, obj("fields", "no", "data", []any{}), []any{}} {
		if _, ok := Unpack(value); ok {
			t.Errorf("%v should not unpack", value)
		}
	}

	records, ok := Unpack(obj("fields", []any{}, "data", []any{}, "count", 0.0))
	if !ok || len(records) != 0 {
		t.Errorf("an empty packed table should unpack to nothing, got %v", records)
	}
}

func TestIsPackedRecognisesTheShape(t *testing.T) {
	packed, _ := Pack([]any{obj("a", 1.0)})
	if !IsPacked(packed) {
		t.Error("a packed table should be recognised")
	}
	if IsPacked([]any{obj("a", 1.0)}) {
		t.Error("a record set is not a packed table")
	}
	if IsPacked(nil) {
		t.Error("nil is not a packed table")
	}
}

func TestPackAllAndUnpackAllHandleSeveralResultSets(t *testing.T) {
	sets := []any{
		[]any{obj("a", 1.0), obj("a", 2.0)},
		[]any{obj("b", 3.0)},
	}

	packed, ok := PackAll(sets)
	if !ok || len(packed) != 2 {
		t.Fatalf("expected two packed sets, got %v", packed)
	}
	if table, ok := packed[0].(*PackedTable); !ok || table.Count != 2 {
		t.Errorf("unexpected first set %v", packed[0])
	}

	restored, ok := UnpackAll(packed)
	if !ok {
		t.Fatal("packed sets should unpack")
	}
	if actual := text(t, restored); actual != text(t, sets) {
		t.Errorf("expected %s, got %s", text(t, sets), actual)
	}
}

func TestTextToArrayReadsABracketedIntegerColumn(t *testing.T) {
	if actual := text(t, TextToArray(obj("ids", "[[[1,2,3]]]"), "ids")); actual != `{"ids":[1,2,3]}` {
		t.Errorf("unexpected result %s", actual)
	}
	if actual := text(t, TextToArray(obj("ids", nil), "ids")); actual != `{"ids":[]}` {
		t.Errorf("a null column should become an empty array, got %s", actual)
	}
	shaped := TextToArray(obj("ids", "{1|2}"), "ids", TextShape{Open: 1, Close: 1, Separator: "|"})
	if actual := text(t, shaped); actual != `{"ids":[1,2]}` {
		t.Errorf("unexpected result %s", actual)
	}
	if actual := text(t, TextToArray(obj("other", 1.0), "ids")); actual != `{"other":1}` {
		t.Errorf("a record without the column should be left alone, got %s", actual)
	}
}

func TestBase62TokensRoundTrip(t *testing.T) {
	for _, n := range []int{0, 1, 61, 62, 63, 3843, 3844, 238327, 238328, 1000000} {
		token := toBase62(n)
		if back := fromBase62(token); back != n {
			t.Errorf("index %d wrote %q and read back %d", n, token, back)
		}
		if base62Length(n) != len(token) {
			t.Errorf("base62Length(%d) = %d, but the token %q is %d long", n, base62Length(n), token, len(token))
		}
	}
	for _, token := range []string{"", "~", "a-b"} {
		if fromBase62(token) != -1 {
			t.Errorf("%q should not read as a token", token)
		}
	}
}

func TestKeyTokensRoundTripAndNeverLookLikeArrayIndices(t *testing.T) {
	letterFirst := regexp.MustCompile(`^[A-Za-z]`)
	for n := 0; n < 20000; n++ {
		token := toKeyToken(n)
		if back := fromKeyToken(token); back != n {
			t.Fatalf("index %d wrote %q and read back %d", n, token, back)
		}
		if !letterFirst.MatchString(token) {
			t.Fatalf("token %q should start with a letter", token)
		}
		if number, err := strconv.Atoi(token); err == nil && strconv.Itoa(number) == token {
			t.Fatalf("token %q looks numeric", token)
		}
	}
	for _, token := range []string{"", "0", "~"} {
		if fromKeyToken(token) != -1 {
			t.Errorf("%q should not read as a key token", token)
		}
	}
}

func TestKeyTokensAreCompactForTheSizesThatMatter(t *testing.T) {
	for _, item := range []struct {
		index  int
		length int
	}{{0, 1}, {51, 1}, {52, 2}, {3275, 2}, {3276, 3}} {
		if got := len(toKeyToken(item.index)); got != item.length {
			t.Errorf("toKeyToken(%d) is %d characters, want %d", item.index, got, item.length)
		}
	}
}

func TestColumnOrderingRespectsEveryRow(t *testing.T) {
	cases := []struct {
		rows []*Object
		want string
		ok   bool
	}{
		{[]*Object{obj("a", 1.0, "b", 2.0), obj("a", 3.0, "b", 4.0)}, "a,b", true},
		{[]*Object{obj("a", 1.0, "b", 2.0), obj("b", 3.0, "c", 4.0)}, "a,b,c", true},
		// The rows disagree about where c goes, but an order exists that suits
		// both.
		{[]*Object{obj("a", 1.0, "b", 2.0), obj("c", 3.0, "a", 4.0)}, "c,a,b", true},
		// Here they contradict each other outright.
		{[]*Object{obj("a", 1.0, "b", 2.0), obj("b", 3.0, "a", 4.0)}, "", false},
	}

	for _, item := range cases {
		columns, ok := orderColumns(item.rows)
		if ok != item.ok {
			t.Errorf("expected ok=%v for %v", item.ok, item.rows)
			continue
		}
		if ok && strings.Join(columns, ",") != item.want {
			t.Errorf("expected %s, got %s", item.want, strings.Join(columns, ","))
		}
	}
}

func TestAnArrayWhoseRowsDisagreeAboutKeyOrderStaysAnArrayOfObjects(t *testing.T) {
	value := []any{obj("a", 1.0, "b", 2.0), obj("b", 3.0, "a", 4.0)}
	if envelope := envelopeText(t, value); strings.Contains(envelope, tableColumns) {
		t.Errorf("rows that disagree should not be packed, got %s", envelope)
	}
	roundTrips(t, value, "rows whose key orders disagree")
}
