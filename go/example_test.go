package jsoncompress_test

import (
	"fmt"

	jsoncompress "github.com/eharain/JSON-Compress/go"
)

type user struct {
	ID      int    `json:"id"`
	Name    string `json:"name"`
	Role    string `json:"role"`
	Country string `json:"country"`
}

// The shape the format is built for: a record set, where every row repeats every
// key name and most rows repeat the values too.
func Example() {
	users := []user{
		{1, "Ada", "admin", "United Kingdom"},
		{2, "Bob", "admin", "United Kingdom"},
		{3, "Cy", "user", "United Kingdom"},
	}

	small, err := jsoncompress.Marshal(users)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(small))

	back, err := jsoncompress.Unmarshal(small)
	if err != nil {
		panic(err)
	}
	restored, err := jsoncompress.FormatJSON(back)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(restored))

	// Output:
	// {"jc":1,"k":["id","name","role","country"],"d":{"~t":[0,1,2,3],"~e":[0,0,0,2],"~cv":["United Kingdom"],"~r":[[1,"Ada","admin"],[2,"Bob","admin"],[3,"Cy","user"]]}}
	// [{"id":1,"name":"Ada","role":"admin","country":"United Kingdom"},{"id":2,"name":"Bob","role":"admin","country":"United Kingdom"},{"id":3,"name":"Cy","role":"user","country":"United Kingdom"}]
}

// An Object keeps the order its keys arrived in, and can say whether a key is
// present at all - which a nil value cannot.
func ExampleObject() {
	row := jsoncompress.NewObject().Set("id", 1.0).Set("name", "Ada").Set("note", nil)

	fmt.Println(row.Len(), row.Keys())

	note, present := row.Get("note")
	fmt.Println(note, present)

	missing, present := row.Get("nickname")
	fmt.Println(missing, present)

	text, _ := jsoncompress.FormatJSON(row)
	fmt.Println(string(text))

	// Output:
	// 3 [id name note]
	// <nil> true
	// <nil> false
	// {"id":1,"name":"Ada","note":null}
}

// Measure says what compression would buy before anything is committed to.
func ExampleMeasure() {
	rows := make([]any, 100)
	for i := range rows {
		rows[i] = jsoncompress.NewObject().
			Set("id", float64(i)).
			Set("status", "delivered").
			Set("currency", "GBP")
	}

	report, err := jsoncompress.Measure(rows)
	if err != nil {
		panic(err)
	}
	fmt.Println(report.Minified, report.Compressed, report.Saved)

	// Output:
	// 4791 594 4197
}

// Locate says exactly where a document stops being JSON.
func ExampleLocate() {
	found := jsoncompress.Locate("{\n  \"a\": 1,\n  \"b\": oops\n}")

	fmt.Println(found.Valid)
	fmt.Println(found.Error)
	fmt.Println(found.Line, found.Column)
	fmt.Println(found.Excerpt)
	fmt.Println(found)

	// Output:
	// false
	// expected a value, found "o"
	// 3 8
	//   "b": oops
	// invalid JSON at line 3, column 8: expected a value, found "o"
}

// A codec binds a set of options once, for a transport or storage layer that
// uses the same ones everywhere.
func ExampleCodec() {
	options := jsoncompress.DefaultOptions()
	options.MinRows = 10

	codec, err := jsoncompress.NewCodec(options)
	if err != nil {
		panic(err)
	}

	rows := []any{
		jsoncompress.NewObject().Set("a", 1.0),
		jsoncompress.NewObject().Set("a", 2.0),
	}

	// Two rows is under the threshold, so they stay as objects.
	data, err := codec.Marshal(rows)
	if err != nil {
		panic(err)
	}
	fmt.Println(string(data))

	// Output:
	// {"jc":1,"k":["a"],"d":[{"A":1},{"A":2}]}
}

// Pack is the columnar idea on its own, with no envelope and no catalogue.
func ExamplePack() {
	table, ok := jsoncompress.Pack([]user{{1, "Ada", "admin", "UK"}, {2, "Bob", "user", "UK"}})
	if !ok {
		panic("not a record set")
	}

	text, _ := jsoncompress.FormatJSON(table)
	fmt.Println(string(text))

	// Output:
	// {"fields":["id","name","role","country"],"data":[[1,"Ada","admin","UK"],[2,"Bob","user","UK"]],"count":2}
}
