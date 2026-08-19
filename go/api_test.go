package jsoncompress

// The public surface: the helpers around the codec, and the way it behaves at
// the edges - bad input, values JSON itself cannot carry, documents from another
// version of the format.

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestTheEnvelopeCarriesAVersionAndAKeyCatalogue(t *testing.T) {
	envelope, err := Compress(obj("hello", "world"))
	if err != nil {
		t.Fatal(err)
	}
	if version, _ := envelope.Get(envVersion); version != int64(FormatVersion) {
		t.Errorf("expected version %d, got %v", FormatVersion, version)
	}
	if keys := text(t, envelope.Value(envKeys)); keys != `["hello"]` {
		t.Errorf("expected the key catalogue to hold the one key, got %s", keys)
	}
	if !IsCompressed(envelope) {
		t.Error("an envelope should look compressed")
	}
}

func TestTheStringCatalogueIsLeftOutWhenNothingIsWorthCataloguing(t *testing.T) {
	envelope, err := Compress(obj("a", 1.0))
	if err != nil {
		t.Fatal(err)
	}
	if envelope.Has(envStrings) {
		t.Errorf("expected no string catalogue, got %s", text(t, envelope))
	}
}

func TestIsCompressedRejectsEverythingThatIsNotAnEnvelope(t *testing.T) {
	for _, value := range []any{nil, 0.0, "x", []any{}, NewObject(), obj("jc", 2.0, "k", []any{}), obj("jc", 1.0)} {
		if IsCompressed(value) {
			t.Errorf("%s should not look compressed", text(t, value))
		}
	}
}

func TestDecompressIfNeededPassesOrdinaryValuesStraightThrough(t *testing.T) {
	plain := obj("a", 1.0)
	same, err := DecompressIfNeeded(plain)
	if err != nil {
		t.Fatal(err)
	}
	if same != any(plain) {
		t.Error("an ordinary value should come back as it went in")
	}

	envelope, err := Compress(plain)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := DecompressIfNeeded(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != `{"a":1}` {
		t.Errorf("expected the original document, got %s", actual)
	}
}

func TestDecompressRefusesDocumentsItCannotRead(t *testing.T) {
	cases := []struct {
		envelope any
		wants    string
	}{
		{nil, "expected a compressed envelope"},
		{[]any{1.0, 2.0}, "expected a compressed envelope"},
		{obj("k", []any{}), "not a compressed document"},
		{obj("jc", 99.0, "k", []any{}), "version 99 is not supported"},
		{obj("jc", 1.0), "key catalogue"},
		{obj("jc", 1.0, "k", []any{}, "s", "nope"), "string catalogue"},
		{obj("jc", 1.0, "k", []any{}, "d", obj("A", 1.0)), "key token"},
		{obj("jc", 1.0, "k", []any{"a"}, "s", []any{}, "d", "~5"), "string reference"},
		{obj("jc", 1.0, "k", []any{"a"}, "d", "~"), "hole marker"},
		{obj("jc", 1.0, "k", []any{"a"}, "d", obj("~t", "x")), "column list"},
		{obj("jc", 1.0, "k", []any{"a"}, "d", obj("~t", []any{7.0})), "column index"},
		{obj("jc", 1.0, "k", []any{"a"}, "d", obj("~t", []any{0.0}, "~e", []any{0.0, 0.0})), "encoding list"},
	}

	for _, item := range cases {
		_, err := Decompress(item.envelope)
		if err == nil {
			t.Errorf("%s should have been refused", text(t, item.envelope))
			continue
		}
		var decodeError *DecodeError
		if !errors.As(err, &decodeError) {
			t.Errorf("%v should be a DecodeError", err)
		}
		if !strings.Contains(err.Error(), item.wants) {
			t.Errorf("expected an error about %q, got %q", item.wants, err)
		}
	}
}

func TestCircularStructuresAreRefusedNotHungOn(t *testing.T) {
	value := obj("name", "loop")
	value.Set("self", value)

	if _, err := Compress(value); err == nil || !strings.Contains(err.Error(), "circular") {
		t.Errorf("expected a circular-structure error, got %v", err)
	}

	type node struct {
		Name string `json:"name"`
		Self *node  `json:"self,omitempty"`
	}
	loop := &node{Name: "loop"}
	loop.Self = loop
	if _, err := Compress(loop); err == nil || !strings.Contains(err.Error(), "circular") {
		t.Errorf("expected a circular-structure error from a struct, got %v", err)
	}
}

func TestASharedReferenceIsNotMistakenForACycle(t *testing.T) {
	shared := obj("a", 1.0)
	roundTrips(t, obj("x", shared, "y", shared), "a value used twice")

	list := []any{1.0}
	roundTrips(t, []any{list, list}, "an array used twice")
}

func TestValuesJSONCannotCarryAreTreatedTheWayJSONTreatsThem(t *testing.T) {
	value := obj(
		"when", time.Date(2026, 8, 18, 10, 20, 30, 0, time.UTC),
		"nan", math.NaN(),
		"inf", math.Inf(1),
		"negativeZero", math.Copysign(0, -1),
		"list", []any{math.NaN(), 1.0},
		"nilSlice", []string(nil),
		"bytes", []byte("hello"),
	)

	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}

	const expected = `{"when":"2026-08-18T10:20:30Z","nan":null,"inf":null,"negativeZero":0,` +
		`"list":[null,1],"nilSlice":null,"bytes":"aGVsbG8="}`
	if actual := text(t, restored); actual != expected {
		t.Errorf("expected %s\n  actual   %s", expected, actual)
	}
}

func TestStructsAreReadTheWayEncodingJsonReadsThem(t *testing.T) {
	type inner struct {
		Deep string `json:"deep"`
	}
	type record struct {
		inner
		ID      int      `json:"id"`
		Name    string   `json:"name"`
		Tags    []string `json:"tags,omitempty"`
		Missing string   `json:"missing,omitempty"`
		Hidden  string   `json:"-"`
		private string
	}

	value := record{inner: inner{Deep: "down"}, ID: 7, Name: "Ada", Hidden: "no", private: "no"}

	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}

	// The same fields, in the same order, as encoding/json would have written.
	reference, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != string(reference) {
		t.Errorf("expected %s\n  actual   %s", reference, actual)
	}
}

func TestMapKeysAreSortedTheWayEncodingJsonSortsThem(t *testing.T) {
	value := map[string]any{"b": 2.0, "a": 1.0, "c": 3.0}
	envelope, err := Compress(value)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != `{"a":1,"b":2,"c":3}` {
		t.Errorf("expected sorted keys, got %s", actual)
	}
}

func TestAKeyCalledProtoSurvivesAsARealKey(t *testing.T) {
	value, err := ParseJSONString(`{"__proto__":1,"safe":2}`)
	if err != nil {
		t.Fatal(err)
	}
	roundTrips(t, value, "__proto__ as a key")

	rows, err := ParseJSONString(`[{"__proto__":1,"a":2},{"__proto__":3,"a":4},{"__proto__":5,"a":6}]`)
	if err != nil {
		t.Fatal(err)
	}
	roundTrips(t, rows, "__proto__ as a column")
}

func TestOptionsAreValidated(t *testing.T) {
	broken := []Options{
		{},
		func() Options { o := DefaultOptions(); o.MinRows = 0; return o }(),
		func() Options { o := DefaultOptions(); o.MaxHoleRatio = 2; return o }(),
		func() Options { o := DefaultOptions(); o.MinStringCount = 1; return o }(),
		func() Options { o := DefaultOptions(); o.MinStringLength = -1; return o }(),
	}
	for _, options := range broken {
		if _, err := Compress(NewObject(), options); err == nil {
			t.Errorf("%+v should have been refused", options)
		}
	}

	if _, err := Compress(NewObject(), DefaultOptions(), DefaultOptions()); err == nil {
		t.Error("two option sets should have been refused")
	}
}

func TestMinifyAndBeautifyOnlyTouchWhitespace(t *testing.T) {
	minified, err := Minify("{\n  \"a\" : 1,\n  \"b\" : [1, 2]\n}")
	if err != nil {
		t.Fatal(err)
	}
	if minified != `{"a":1,"b":[1,2]}` {
		t.Errorf("unexpected minified form %s", minified)
	}

	for _, item := range []struct {
		indent int
		want   string
	}{
		{2, "{\n  \"a\": 1\n}"},
		{4, "{\n    \"a\": 1\n}"},
		{0, `{"a":1}`},
	} {
		out, err := Beautify(`{"a":1}`, item.indent)
		if err != nil {
			t.Fatal(err)
		}
		if out != item.want {
			t.Errorf("indent %d: expected %q, got %q", item.indent, item.want, out)
		}
	}

	if _, err := Minify("{oops}"); err == nil {
		t.Error("invalid JSON should not minify")
	}
}

func TestValidateReportsWhereADocumentGoesWrong(t *testing.T) {
	if found := Validate(`{"a":1}`); !found.Valid {
		t.Errorf("expected valid, got %v", found)
	}

	bad := Validate("{\n  \"a\": 1,\n  \"b\": oops\n}")
	if bad.Valid {
		t.Fatal("expected invalid")
	}
	if bad.Line != 3 || bad.Column != 8 {
		t.Errorf("expected line 3, column 8, got line %d, column %d", bad.Line, bad.Column)
	}
	if bad.Excerpt != `  "b": oops` {
		t.Errorf("unexpected excerpt %q", bad.Excerpt)
	}
	if !strings.Contains(bad.String(), "line 3") {
		t.Errorf("unexpected summary %q", bad.String())
	}
}

func TestLocateNamesTheCommonMistakes(t *testing.T) {
	cases := []struct {
		text   string
		wants  string
		line   int
		column int
	}{
		{"{'a': 1}", "double quotes", 1, 2},
		{`{"a": 1,}`, "trailing comma", 1, 8},
		{"[1, 2,]", "trailing comma", 1, 6},
		{`{"a": True}`, "lower case", 1, 7},
		{`{"a": None}`, "use null", 1, 7},
		{`{"a": 01}`, "leading zero", 1, 7},
		{`{"a": 1.}`, "decimal point", 1, 8},
		{`{"a": 1e}`, "exponent", 1, 8},
		{`{"a": "unclosed`, "never closed", 1, 7},
		{`{"a": 1`, "never closed", 1, 8},
		{"[1, 2", "never closed", 1, 6},
		{"{a: 1}", "quoted property name", 1, 2},
		{`{"a" 1}`, "colon", 1, 6},
		{`{"a": 1} extra`, "after the end", 1, 10},
		{"", "ends here", 1, 1},
		{`{"a": "\q"}`, "not a valid escape", 1, 8},
		{`{"a": "\u12"}`, "four hexadecimal", 1, 8},
	}

	for _, item := range cases {
		found := Locate(item.text)
		if found.Valid {
			t.Errorf("%q should be invalid", item.text)
			continue
		}
		if !strings.Contains(found.Error, item.wants) {
			t.Errorf("%q: expected an error about %q, got %q", item.text, item.wants, found.Error)
		}
		if found.Line != item.line || found.Column != item.column {
			t.Errorf("%q: expected line %d column %d, got line %d column %d",
				item.text, item.line, item.column, found.Line, found.Column)
		}
	}
}

func TestLocateAgreesWithTheParserAboutWhatIsValid(t *testing.T) {
	documents := []string{
		"{}", "[]", "null", "true", "false", "0", "-0", "1e10", "-1.5E-3", `"x"`,
		`  {  "a" : [ 1 , 2 ] }  `, `{"a":{"b":{"c":[]}}}`, `"é\n\t\\"`,
		`{"": 1}`, "[[[[[1]]]]]", `{"a": 1e+10}`, `"😀"`,
	}
	for _, document := range documents {
		if found := Locate(document); !found.Valid {
			t.Errorf("%q should be valid: %s", document, found.Error)
		}
		if _, err := ParseJSONString(document); err != nil {
			t.Errorf("%q should parse: %v", document, err)
		}
	}
}

func TestLocateFindsTheSameLineOnAMultiLineDocument(t *testing.T) {
	document := strings.Join([]string{"{", `  "one": 1,`, `  "two": [`, "    1,", "    2,", "  ]", "}"}, "\n")
	found := Locate(document)
	if found.Valid || !strings.Contains(found.Error, "trailing comma") || found.Line != 5 {
		t.Errorf("expected a trailing comma on line 5, got %v", found)
	}
}

func TestMeasureReportsARealSavingOnARecordSet(t *testing.T) {
	value := make([]any, 100)
	for i := range value {
		department := "operations"
		if i%2 == 1 {
			department = "engineering"
		}
		value[i] = obj("id", float64(i), "firstName", "Given", "lastName", "Family", "department", department)
	}

	report, err := Measure(value)
	if err != nil {
		t.Fatal(err)
	}
	if report.Minified != len(text(t, value)) {
		t.Errorf("expected the minified size to be measured, got %d", report.Minified)
	}
	if report.Compressed >= report.Minified {
		t.Errorf("expected a saving, got %d against %d", report.Compressed, report.Minified)
	}
	if report.Saved != report.Minified-report.Compressed {
		t.Error("saved should be the difference")
	}
	if report.Percent <= 40 {
		t.Errorf("expected better than 40%%, got %v", report.Percent)
	}
	if report.Ratio >= 1 {
		t.Errorf("expected a ratio below 1, got %v", report.Ratio)
	}
	if report.GzipMinified <= 0 || report.GzipCompressed <= 0 {
		t.Error("expected gzip sizes to be measured")
	}
}

func TestMeasureTextReportsWhatTheInputReallyWeighed(t *testing.T) {
	report, err := MeasureText("{\n  \"a\": 1\n}")
	if err != nil {
		t.Fatal(err)
	}
	if report.Original != 12 {
		t.Errorf("expected the original 12 bytes, got %d", report.Original)
	}
	if report.Minified != 7 {
		t.Errorf("expected 7 minified bytes, got %d", report.Minified)
	}
}

func TestGzipRoundTrips(t *testing.T) {
	document := []byte(`{"hello":"world","list":[1,2,3]}`)
	packed, err := GzipBytes(document)
	if err != nil {
		t.Fatal(err)
	}
	restored, err := Gunzip(packed)
	if err != nil {
		t.Fatal(err)
	}
	if string(restored) != string(document) {
		t.Errorf("gzip did not round-trip: %s", restored)
	}
	if GzipSize(document) != len(packed) {
		t.Error("GzipSize should agree with GzipBytes")
	}

	// No timestamp in the header, so the same input always measures the same.
	again, err := GzipBytes(document)
	if err != nil {
		t.Fatal(err)
	}
	if string(again) != string(packed) {
		t.Error("gzip should be reproducible")
	}
}

func TestByteLengthCountsUTF8Bytes(t *testing.T) {
	for _, item := range []struct {
		text string
		want int
	}{{"abc", 3}, {"£", 2}, {"😀", 4}} {
		if got := ByteLength(item.text); got != item.want {
			t.Errorf("ByteLength(%q) = %d, want %d", item.text, got, item.want)
		}
	}
}

func TestACodecBindsItsOptionsOnce(t *testing.T) {
	options := DefaultOptions()
	options.Tables = false
	codec, err := NewCodec(options)
	if err != nil {
		t.Fatal(err)
	}

	value := []any{obj("a", 1.0), obj("a", 2.0), obj("a", 3.0)}
	data, err := codec.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(data), tableColumns) {
		t.Errorf("expected no table, got %s", data)
	}

	restored, err := codec.Unmarshal(data)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != text(t, value) {
		t.Errorf("a codec did not round-trip: %s", actual)
	}
	if codec.Options().Tables {
		t.Error("the codec should keep the options it was built with")
	}

	if _, err := NewCodec(Options{}); err == nil {
		t.Error("a codec should validate its options once, at the start")
	}
}

func TestNumbersAreWrittenTheEcmascriptWay(t *testing.T) {
	// Added at run time: the compiler would fold this to an exact 0.3.
	tenth, fifth := 0.1, 0.2

	cases := []struct {
		value float64
		want  string
	}{
		{0, "0"},
		{math.Copysign(0, -1), "0"},
		{1, "1"},
		{-1.5, "-1.5"},
		{0.00001, "0.00001"},
		{1e-7, "1e-7"},
		{-1e-7, "-1e-7"},
		{1e20, "100000000000000000000"},
		{1e21, "1e+21"},
		{1.7976931348623157e308, "1.7976931348623157e+308"},
		{5e-324, "5e-324"},
		{123456789012345680000, "123456789012345680000"},
		{tenth + fifth, "0.30000000000000004"},
		{math.NaN(), "null"},
		{math.Inf(-1), "null"},
	}
	for _, item := range cases {
		if got := formatFloat(item.value); got != item.want {
			t.Errorf("formatFloat(%v) = %s, want %s", item.value, got, item.want)
		}
	}
}

func TestStringsAreEscapedTheWayJsonStringifyEscapesThem(t *testing.T) {
	// The awkward characters here are built from their code points, so that the
	// table says what it means without a layer of source escaping over the top.
	character := func(code int) string { return string(rune(code)) }
	escape := func(code int) string { return string(rune(92)) + "u" + fmt.Sprintf("%04x", code) }

	nul := character(0)
	unitSeparator := character(0x1F)
	lineSeparator := character(0x2028)
	emoji := character(0x1F600)
	replacement := character(0xFFFD)
	loneSurrogate := string([]byte{0xED, 0xA0, 0x80})
	strayByte := string([]byte{0xFF})

	cases := []struct {
		value string
		want  string
	}{
		{"plain", `"plain"`},
		{"a\"b", `"a\"b"`},
		{`a\b`, `"a\\b"`},
		{"a\nb\tc\rd\be\ff", `"a\nb\tc\rd\be\ff"`},
		{"a" + nul + "b", `"a` + escape(0) + `b"`},
		{"a" + unitSeparator + "b", `"a` + escape(0x1F) + `b"`},
		{"a/b", `"a/b"`},                                         // the solidus is not escaped
		{"<script>&", `"<script>&"`},                             // nor is anything HTML cares about
		{"a" + lineSeparator + "b", `"a` + lineSeparator + `b"`}, // nor the line separators
		{emoji, `"` + emoji + `"`},
		{loneSurrogate, `"` + escape(0xD800) + `"`}, // which JSON.stringify escapes
		{strayByte, `"` + replacement + `"`},        // and which nothing can carry
	}
	for _, item := range cases {
		if got := string(writeString(nil, item.value)); got != item.want {
			t.Errorf("writeString(%q) = %s, want %s", item.value, got, item.want)
		}
	}
}

func TestParseJSONRefusesWhatJSONHasNoSpellingFor(t *testing.T) {
	for _, document := range []string{"NaN", "Infinity", "{a:1}", "[1,]", "", "{} {}", `"unclosed`} {
		if _, err := ParseJSONString(document); err == nil {
			t.Errorf("%q should not parse", document)
		}
	}
}

func TestAnEnvelopeCanBeReadBackThroughEncodingJson(t *testing.T) {
	value := obj("users", []any{obj("id", 1.0, "name", "Ada"), obj("id", 2.0, "name", "Bob")})

	data, err := Marshal(value)
	if err != nil {
		t.Fatal(err)
	}

	// Somebody else's decoder, giving up the key order of ordinary object nodes
	// but not of packed tables, which carry their own.
	var envelope map[string]any
	if err := json.Unmarshal(data, &envelope); err != nil {
		t.Fatal(err)
	}
	if !IsCompressed(envelope) {
		t.Fatal("a map should still be recognised as an envelope")
	}
	restored, err := Decompress(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if actual := text(t, restored); actual != text(t, value) {
		t.Errorf("expected %s\n  actual   %s", text(t, value), actual)
	}
}

func TestObjectKeepsItsKeyOrder(t *testing.T) {
	value := NewObject().Set("b", 1.0).Set("a", 2.0).Set("b", 3.0)
	if actual := text(t, value); actual != `{"b":3,"a":2}` {
		t.Errorf("a repeated key should keep its place, got %s", actual)
	}

	keys := value.Keys()
	keys[0] = "changed"
	if first, _ := value.At(0); first != "b" {
		t.Error("Keys should hand back a copy")
	}

	value.Delete("b")
	if actual := text(t, value); actual != `{"a":2}` {
		t.Errorf("expected the deleted key to be gone, got %s", actual)
	}

	var decoded Object
	if err := json.Unmarshal([]byte(`{"z":1,"a":2,"m":3}`), &decoded); err != nil {
		t.Fatal(err)
	}
	if actual := text(t, &decoded); actual != `{"z":1,"a":2,"m":3}` {
		t.Errorf("UnmarshalJSON should keep the order, got %s", actual)
	}
}

func TestACodecIsSafeToShareAcrossGoroutines(t *testing.T) {
	// The struct-field cache is the one piece of shared state in the package,
	// so this is what `go test -race` has to look at.
	type record struct {
		ID   int    `json:"id"`
		Name string `json:"name"`
	}

	codec, err := NewCodec()
	if err != nil {
		t.Fatal(err)
	}

	const workers = 8
	failures := make(chan error, workers)
	var waiting sync.WaitGroup

	for worker := 0; worker < workers; worker++ {
		waiting.Add(1)
		go func(worker int) {
			defer waiting.Done()
			value := []record{{worker, "Ada"}, {worker + 1, "Bob"}}
			for i := 0; i < 50; i++ {
				data, err := codec.Marshal(value)
				if err != nil {
					failures <- err
					return
				}
				if _, err := codec.Unmarshal(data); err != nil {
					failures <- err
					return
				}
			}
		}(worker)
	}

	waiting.Wait()
	close(failures)
	for err := range failures {
		t.Error(err)
	}
}

// A stand-in for the types that write themselves as text rather than as a
// structure - net.IP, uuid.UUID, and most enums people hand-roll.
type postcode struct {
	outward string
	inward  string
}

func (p postcode) MarshalText() ([]byte, error) {
	return []byte(p.outward + " " + p.inward), nil
}

func TestNormalizeAgreesWithEncodingJson(t *testing.T) {
	type deep struct {
		Shared string `json:"shared"`
		Only   string `json:"only"`
	}
	type between struct {
		deep
	}
	// One name claimed twice at different depths, where the shallower wins, and
	// one claimed twice at the same depth, where neither does.
	type left struct{ Tie string }
	type right struct{ Tie string }
	type layered struct {
		between
		left
		right
		Shared string `json:"shared"`
	}
	type tags struct {
		Kept      string            `json:"kept"`
		Renamed   string            `json:"as-something-else"`
		Dropped   string            `json:"-"`
		Empty     string            `json:"empty,omitempty"`
		Zero      int               `json:"zero,omitempty"`
		Nothing   []string          `json:"nothing,omitempty"`
		Raw       json.RawMessage   `json:"raw"`
		When      time.Time         `json:"when"`
		Where     postcode          `json:"where"`
		Bytes     []byte            `json:"bytes"`
		NilSlice  []int             `json:"nilSlice"`
		NilMap    map[string]int    `json:"nilMap"`
		Numbers   map[int]string    `json:"numbers"`
		Pointer   *deep             `json:"pointer"`
		Interface any               `json:"interface"`
		Nested    map[string]string `json:"nested"`
	}

	values := []any{
		layered{between{deep{"deep", "only mine"}}, left{"first"}, right{"second"}, "shallow"},
		tags{
			Kept:      "yes",
			Renamed:   "also yes",
			Dropped:   "no",
			Raw:       json.RawMessage(`{"already":["json"]}`),
			When:      time.Date(2026, 8, 19, 9, 30, 0, 0, time.UTC),
			Where:     postcode{"SW1A", "1AA"},
			Bytes:     []byte{1, 2, 3},
			Numbers:   map[int]string{10: "ten", 2: "two"},
			Pointer:   &deep{"pointed at", "mine"},
			Interface: []any{1.0, "two"},
			Nested:    map[string]string{"b": "second", "a": "first"},
		},
		[]any{nil, true, "text", 1.5, int64(-9007199254740993), uint64(18446744073709551615)},
		map[string]any{"z": 1.0, "a": []any{}, "m": map[string]any{}},
		&deep{"pointer to a struct", "mine"},
		(*deep)(nil),
	}

	for _, value := range values {
		reference, err := json.Marshal(value)
		if err != nil {
			t.Fatalf("encoding/json could not write %#v: %v", value, err)
		}
		ours, err := FormatJSON(value)
		if err != nil {
			t.Errorf("%#v: %v", value, err)
			continue
		}
		if string(ours) != string(reference) {
			t.Errorf("%#v\n  encoding/json %s\n  json-compress %s", value, reference, ours)
		}

		// And the same value survives the round trip, which is the point of
		// reading it the same way in the first place.
		envelope, err := Compress(value)
		if err != nil {
			t.Errorf("%#v: %v", value, err)
			continue
		}
		restored, err := Decompress(envelope)
		if err != nil {
			t.Errorf("%#v: %v", value, err)
			continue
		}
		if actual := text(t, restored); actual != string(reference) {
			t.Errorf("%#v did not round-trip\n  expected %s\n  actual   %s", value, reference, actual)
		}
	}
}
