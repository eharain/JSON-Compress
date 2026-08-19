package jsoncompress

// Agreement with the other implementations.
//
// SPEC.md asks only that an implementation restore what a conforming encoder
// wrote. This asserts the stronger claim the released implementations already
// meet: that they all write the same envelope for the same document, byte for
// byte.
//
// The corpus is 144 source/envelope pairs produced by the JavaScript encoder and
// checked in at the path below, so this runs with no other toolchain installed.
// It is shared with the .NET, Python and PHP suites rather than duplicated,
// because two copies of a conformance corpus is one copy too many.

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

var corpusPath = filepath.Join("..", "dotnet", "tests", "TechStyle.JsonCompress.Tests", "fixtures", "conformance.json")

type corpusCase struct {
	Name       string `json:"name"`
	Source     string `json:"source"`
	Compressed string `json:"compressed"`
	Seed       *int   `json:"seed"`
}

func loadCorpus(t *testing.T) []corpusCase {
	t.Helper()

	data, err := os.ReadFile(corpusPath)
	if err != nil {
		t.Skipf("the conformance corpus is not checked out at %s", corpusPath)
	}

	var corpus struct {
		Cases []corpusCase `json:"cases"`
	}
	if err := json.Unmarshal(data, &corpus); err != nil {
		t.Fatalf("the conformance corpus does not parse: %v", err)
	}
	return corpus.Cases
}

func TestTheCorpusIsTheSizeItShouldBe(t *testing.T) {
	cases := loadCorpus(t)
	if len(cases) < 144 {
		t.Fatalf("expected at least 144 cases, found %d", len(cases))
	}
}

// Clause 3 of the specification: what a conforming encoder wrote, we read.
func TestEveryEnvelopeTheReferenceEncoderWroteIsRestored(t *testing.T) {
	for _, item := range loadCorpus(t) {
		envelope, err := ParseJSONString(item.Compressed)
		if err != nil {
			t.Errorf("%s: the envelope does not parse: %v", item.Name, err)
			continue
		}
		restored, err := Decompress(envelope)
		if err != nil {
			t.Errorf("%s: %v", item.Name, err)
			continue
		}
		text, err := FormatJSON(restored)
		if err != nil {
			t.Errorf("%s: %v", item.Name, err)
			continue
		}
		if string(text) != item.Source {
			t.Errorf("%s:\n  expected %s\n  actual   %s", item.Name, item.Source, text)
		}
	}
}

// Stronger than the specification asks: the same bytes, not merely the same
// value.
func TestEveryEnvelopeIsWrittenTheSameWay(t *testing.T) {
	for _, item := range loadCorpus(t) {
		source, err := ParseJSONString(item.Source)
		if err != nil {
			t.Errorf("%s: the source does not parse: %v", item.Name, err)
			continue
		}
		ours, err := Marshal(source)
		if err != nil {
			t.Errorf("%s: %v", item.Name, err)
			continue
		}
		if string(ours) != item.Compressed {
			t.Errorf("%s:\n  reference %s\n  go        %s", item.Name, item.Compressed, ours)
		}
	}
}

// The generator is shared, so a seed names the same document everywhere.
func TestTheSeededCasesAreReproducedFromTheirSeeds(t *testing.T) {
	checked := 0
	for _, item := range loadCorpus(t) {
		if item.Seed == nil {
			continue
		}
		text, err := FormatJSON(makeValue(createRandom(*item.Seed), 5))
		if err != nil {
			t.Fatalf("seed %d: %v", *item.Seed, err)
		}
		if string(text) != item.Source {
			t.Errorf("seed %d:\n  expected %s\n  actual   %s", *item.Seed, item.Source, text)
		}
		checked++
	}
	if checked == 0 {
		t.Fatal("the corpus carries no seeded cases")
	}
}
