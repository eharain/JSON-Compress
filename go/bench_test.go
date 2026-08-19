package jsoncompress

// What each stage costs, on the shape the format is built for.
//
//	go test -bench . -benchmem
//
// The document is a record set of the size an API hands back, built here rather
// than read from disk so the numbers mean the same thing on every machine. Set
// JC_BENCH to a JSON file to measure something of your own instead.

import (
	"os"
	"strconv"
	"testing"
)

func benchmarkDocument(b *testing.B) []byte {
	b.Helper()

	if path := os.Getenv("JC_BENCH"); path != "" {
		data, err := os.ReadFile(path)
		if err != nil {
			b.Fatal(err)
		}
		return data
	}

	random := createRandom(20260819)
	statuses := []string{"placed", "picked", "shipped", "delivered"}
	regions := []string{"north", "south", "east", "west"}

	rows := make([]any, 20000)
	for i := range rows {
		row := NewObject()
		row.Set("id", float64(i))
		row.Set("reference", "REF-"+strconv.Itoa(100000+i))
		row.Set("status", pick(random, statuses))
		row.Set("region", pick(random, regions))
		row.Set("currency", "GBP")
		row.Set("amount", float64(int(random()*1000000))/100)
		row.Set("createdAt", "2026-08-19T09:30:00.000Z")
		row.Set("note", "a repeated note that is worth cataloguing")
		if i%7 != 0 {
			row.Set("assignee", "user-"+strconv.Itoa(int(random()*50)))
		}
		rows[i] = row
	}

	data, err := FormatJSON(rows)
	if err != nil {
		b.Fatal(err)
	}
	return data
}

func BenchmarkParseJSON(b *testing.B) {
	data := benchmarkDocument(b)
	b.SetBytes(int64(len(data)))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := ParseJSON(data); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkFormatJSON(b *testing.B) {
	value := parsedDocument(b)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := FormatJSON(value); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkCompress(b *testing.B) {
	value := parsedDocument(b)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := Compress(value); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkDecompress(b *testing.B) {
	envelope, _ := compressedDocument(b)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := Decompress(envelope); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkMarshal(b *testing.B) {
	value := parsedDocument(b)
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := Marshal(value); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkUnmarshal(b *testing.B) {
	_, data := compressedDocument(b)
	b.SetBytes(int64(len(data)))
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := Unmarshal(data); err != nil {
			b.Fatal(err)
		}
	}
}

func parsedDocument(b *testing.B) any {
	b.Helper()
	data := benchmarkDocument(b)
	value, err := ParseJSON(data)
	if err != nil {
		b.Fatal(err)
	}
	b.SetBytes(int64(len(data)))
	return value
}

func compressedDocument(b *testing.B) (*Object, []byte) {
	b.Helper()
	envelope, err := Compress(parsedDocument(b))
	if err != nil {
		b.Fatal(err)
	}
	data, err := formatModel(envelope, "")
	if err != nil {
		b.Fatal(err)
	}
	return envelope, data
}
