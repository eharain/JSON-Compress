package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// exercise runs the command line over some input and hands back what it wrote.
func exercise(t *testing.T, input string, argv ...string) (string, string, int) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	code := run(argv, strings.NewReader(input), &stdout, &stderr)
	return stdout.String(), stderr.String(), code
}

func TestCompressAndDecompressAreInverses(t *testing.T) {
	const document = `[{"id":1,"role":"admin"},{"id":2,"role":"admin"},{"id":3,"role":"user"}]`

	envelope, errors, code := exercise(t, document, "compress")
	if code != 0 {
		t.Fatalf("compress failed: %s", errors)
	}
	if !strings.Contains(envelope, `"jc":1`) {
		t.Errorf("expected an envelope, got %s", envelope)
	}

	restored, errors, code := exercise(t, envelope, "decompress")
	if code != 0 {
		t.Fatalf("decompress failed: %s", errors)
	}
	if strings.TrimSpace(restored) != document {
		t.Errorf("expected %s, got %s", document, restored)
	}
}

func TestDecompressRefusesAnOrdinaryDocument(t *testing.T) {
	_, errors, code := exercise(t, `{"a":1}`, "decompress")
	if code == 0 || !strings.Contains(errors, "not a compressed document") {
		t.Errorf("expected a refusal, got %q with code %d", errors, code)
	}
}

func TestTheOptionFlagsReachTheEncoder(t *testing.T) {
	const document = `[{"a":1},{"a":2}]`

	packed, _, _ := exercise(t, document, "compress")
	if !strings.Contains(packed, "~t") {
		t.Errorf("expected a table by default, got %s", packed)
	}

	loose, _, _ := exercise(t, document, "compress", "--no-tables")
	if strings.Contains(loose, "~t") {
		t.Errorf("--no-tables should have left the array alone, got %s", loose)
	}

	fewer, _, _ := exercise(t, document, "compress", "--min-rows", "3")
	if strings.Contains(fewer, "~t") {
		t.Errorf("--min-rows 3 should have left two rows alone, got %s", fewer)
	}
}

func TestMinifyBeautifyAndValidate(t *testing.T) {
	minified, _, code := exercise(t, "{\n  \"a\" : 1\n}", "minify")
	if code != 0 || strings.TrimSpace(minified) != `{"a":1}` {
		t.Errorf("unexpected minify output %q", minified)
	}

	beautified, _, code := exercise(t, `{"a":1}`, "beautify")
	if code != 0 || strings.TrimSpace(beautified) != "{\n  \"a\": 1\n}" {
		t.Errorf("unexpected beautify output %q", beautified)
	}

	indented, _, _ := exercise(t, `{"a":1}`, "beautify", "--indent", "4")
	if strings.TrimSpace(indented) != "{\n    \"a\": 1\n}" {
		t.Errorf("unexpected indent output %q", indented)
	}

	valid, _, code := exercise(t, `{"a":1}`, "validate")
	if code != 0 || strings.TrimSpace(valid) != "valid JSON" {
		t.Errorf("unexpected validate output %q", valid)
	}

	_, errors, code := exercise(t, "{\n  \"a\": oops\n}", "validate")
	if code == 0 || !strings.Contains(errors, "line 2") {
		t.Errorf("expected a located failure, got %q", errors)
	}
}

func TestPrettyPrintsTheEnvelope(t *testing.T) {
	out, _, code := exercise(t, `{"a":1}`, "compress", "--pretty")
	if code != 0 || !strings.Contains(out, "\n  \"jc\": 1") {
		t.Errorf("expected an indented envelope, got %q", out)
	}
}

func TestStatsReportsWithoutChangingAnything(t *testing.T) {
	document := `[{"id":1,"status":"open"},{"id":2,"status":"open"},{"id":3,"status":"open"}]`
	out, errors, code := exercise(t, document, "stats")
	if code != 0 {
		t.Fatalf("stats failed: %s", errors)
	}
	for _, want := range []string{"stdin", "as given", "minified", "compressed", "saved", "gzip"} {
		if !strings.Contains(out, want) {
			t.Errorf("expected the report to mention %q, got %s", want, out)
		}
	}
}

func TestFilesAreReadAndWritten(t *testing.T) {
	dir := t.TempDir()
	source := filepath.Join(dir, "orders.json")
	target := filepath.Join(dir, "orders.jc.json")

	if err := os.WriteFile(source, []byte(`[{"a":1},{"a":2}]`), 0o644); err != nil {
		t.Fatal(err)
	}

	if _, errors, code := exercise(t, "", "compress", source, "-o", target); code != 0 {
		t.Fatalf("compress to a file failed: %s", errors)
	}

	written, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(string(written), `{"jc":1`) {
		t.Errorf("unexpected file contents %s", written)
	}
	if strings.HasSuffix(string(written), "\n") {
		t.Error("a file should be written without a trailing newline")
	}

	out, _, _ := exercise(t, "", "stats", source)
	if !strings.Contains(out, "orders.json") {
		t.Errorf("expected the report to name the file, got %s", out)
	}
}

func TestHelpAndVersion(t *testing.T) {
	out, _, code := exercise(t, "", "--help")
	if code != 0 || !strings.Contains(out, "json-compress <command>") {
		t.Errorf("unexpected help output %q", out)
	}

	out, _, code = exercise(t, "", "--version")
	if code != 0 || strings.TrimSpace(out) == "" {
		t.Errorf("unexpected version output %q", out)
	}

	out, _, code = exercise(t, "")
	if code != 0 || !strings.Contains(out, "Usage") {
		t.Errorf("expected the usage text with no arguments, got %q", out)
	}
}

func TestBadUsageIsReported(t *testing.T) {
	cases := [][]string{
		{"--nonsense"},
		{"frobnicate"},
		{"compress", "one.json", "two.json"},
		{"compress", "--min-rows", "many"},
		{"compress", "missing-file.json"},
	}
	for _, argv := range cases {
		_, errors, code := exercise(t, `{"a":1}`, argv...)
		if code == 0 {
			t.Errorf("%v should have failed", argv)
		}
		if !strings.HasPrefix(errors, "json-compress: ") {
			t.Errorf("%v: unexpected message %q", argv, errors)
		}
	}
}

func TestInputThatIsNotJSONIsReported(t *testing.T) {
	_, errors, code := exercise(t, "{oops}", "compress")
	if code == 0 || !strings.Contains(errors, "not valid JSON") {
		t.Errorf("expected a parse failure, got %q", errors)
	}
}

func TestAByteOrderMarkIsIgnored(t *testing.T) {
	// Windows editors and PowerShell both put one in front of a UTF-8 document,
	// and JSON has nowhere to put it.
	out, errors, code := exercise(t, byteOrderMark+`{"a":1}`, "minify")
	if code != 0 {
		t.Fatalf("a document with a byte order mark should still read: %s", errors)
	}
	if strings.TrimSpace(out) != `{"a":1}` {
		t.Errorf("unexpected output %q", out)
	}
}
