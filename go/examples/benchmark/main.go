// Command benchmark reports what the format is worth, on shapes that occur in
// real systems.
//
//	go run ./examples/benchmark
//
// Every figure in the README is produced by running this file, so it can be
// checked rather than taken on trust. The interesting column is the last one:
// whether the format still helps once gzip has had its turn, which is the only
// question that matters for anything served over HTTP.
//
// The generator and the scenarios are the same as in javascript/examples/, so
// the first four columns can be compared line for line with the JavaScript,
// .NET, Python and PHP benchmarks.
package main

import (
	"fmt"
	"math"
	"os"
	"strconv"
	"strings"

	jsoncompress "github.com/eharain/JSON-Compress/go"
)

// A deterministic generator, so the numbers are the same on every machine.
const seed = 20260818

var state uint32 = seed

func random() float64 {
	state = state*1664525 + 1013904223
	return float64(state) / 4294967296
}

func pick(values []string) string {
	return values[int(random()*float64(len(values)))]
}

func integer(low, high int) float64 {
	return float64(low + int(random()*float64(high-low+1)))
}

// rounded rounds half away from zero towards positive infinity, as JavaScript's
// Math.round does.
func rounded(value float64) float64 {
	return math.Floor(value + 0.5)
}

func object(pairs ...any) *jsoncompress.Object {
	out := jsoncompress.NewObject()
	for i := 0; i+1 < len(pairs); i += 2 {
		out.Set(pairs[i].(string), pairs[i+1])
	}
	return out
}

var (
	first  = []string{"Amara", "Bilal", "Chen", "Dara", "Eesa", "Farah", "Gita", "Hana", "Idris", "Jae"}
	last   = []string{"Ahmed", "Bright", "Chowdhury", "Doyle", "Evans", "Fischer", "Gomez", "Haddad"}
	cities = []string{"London", "Manchester", "Birmingham", "Leeds", "Glasgow", "Cardiff", "Belfast"}
)

func apiRecordSet() any {
	out := make([]any, 1000)
	for i := range out {
		out[i] = object(
			"id", float64(i+1),
			"firstName", pick(first),
			"lastName", pick(last),
			"email", "user"+strconv.Itoa(i+1)+"@example.com",
			"role", pick([]string{"admin", "editor", "viewer", "viewer", "viewer"}),
			"active", random() > 0.2,
			"city", pick(cities),
			"country", "United Kingdom",
			"createdAt", "2026-0"+number(integer(1, 9))+"-"+number(integer(10, 28))+"T09:30:00.000Z",
			"loginCount", integer(0, 4000),
		)
	}
	return out
}

func nestedAPIResponse() any {
	data := make([]any, 200)
	for i := range data {
		// The generator is shared with the other implementations, so the values
		// have to be drawn in the order the JavaScript object literal draws
		// them - the rows would otherwise differ and the figures with them.
		status := pick([]string{"placed", "picked", "shipped", "delivered"})
		placedAt := "2026-08-" + number(integer(10, 18)) + "T" + number(integer(10, 20)) + ":15:00.000Z"
		customer := object("id", integer(1, 5000), "name", pick(first)+" "+pick(last), "city", pick(cities))

		lines := make([]any, int(integer(1, 6)))
		for j := range lines {
			lines[j] = object(
				"sku", "SKU-"+number(integer(1000, 9999)),
				"description", pick([]string{"Cotton shirt", "Wool scarf", "Leather belt", "Linen trousers"}),
				"quantity", integer(1, 4),
				"unitPrice", integer(500, 12000)/100,
				"taxRate", 0.2,
			)
		}

		data[i] = object(
			"orderId", "ORD-"+strconv.Itoa(100000+i),
			"status", status,
			"currency", "GBP",
			"placedAt", placedAt,
			"customer", customer,
			"lines", lines,
		)
	}
	return object(
		"meta", object("page", 1.0, "pageSize", 200.0, "total", 8214.0, "generatedAt", "2026-08-18T12:00:00.000Z"),
		"data", data,
	)
}

func timeSeries() any {
	out := make([]any, 2000)
	for i := range out {
		out[i] = object(
			"t", float64(1755500000+i*60),
			"sensor", pick([]string{"temp-01", "temp-02", "temp-03", "humidity-01"}),
			"unit", "celsius",
			"value", rounded(random()*3500)/100,
			"quality", pick([]string{"good", "good", "good", "suspect"}),
		)
	}
	return out
}

func structuredLogs() any {
	out := make([]any, 1500)
	for i := range out {
		out[i] = object(
			"level", pick([]string{"info", "info", "info", "warn", "error"}),
			"service", pick([]string{"api-gateway", "orders", "billing", "search"}),
			"env", "production",
			"region", "eu-west-2",
			"message", pick([]string{"request completed", "cache miss", "upstream timeout, retrying", "validation failed"}),
			"durationMs", integer(1, 4000),
			"traceId", "00-"+number(integer(100000000, 900000000))+"-"+number(integer(1000000, 9000000))+"-01",
		)
	}
	return out
}

func geoJSON() any {
	features := make([]any, 400)
	for i := range features {
		features[i] = object(
			"type", "Feature",
			"geometry", object("type", "Point", "coordinates", []any{
				rounded((random()*8-6)*1e6) / 1e6,
				rounded((49+random()*9)*1e6) / 1e6,
			}),
			"properties", object(
				"id", float64(i),
				"name", pick(cities)+" site "+strconv.Itoa(i),
				"category", pick([]string{"depot", "store", "hub"}),
			),
		)
	}
	return object("type", "FeatureCollection", "features", features)
}

func configDocument() any {
	services := jsoncompress.NewObject()
	for i := 0; i < 12; i++ {
		at := strconv.Itoa(i)
		services.Set("service-"+at, object(
			"image", "registry.example.com/team/service-"+at+":1.4."+at,
			"replicas", integer(1, 6),
			"resources", object("cpu", "500m", "memory", "512Mi"),
			"env", object("NODE_ENV", "production", "LOG_LEVEL", "info", "REGION", "eu-west-2"),
			"ports", []any{object("container", 8080.0, "published", float64(8080+i), "protocol", "tcp")},
		))
	}
	return object("version", 3.0, "services", services)
}

var scenarios = []struct {
	name  string
	build func() any
}{
	{"API record set (1,000 users)", apiRecordSet},
	{"Nested API response (orders with lines)", nestedAPIResponse},
	{"Time series (2,000 readings)", timeSeries},
	{"Structured logs (1,500 lines)", structuredLogs},
	{"GeoJSON feature collection (400 points)", geoJSON},
	{"Config document (deeply nested, few repeats)", configDocument},
}

func main() {
	if err := report(); err != nil {
		fmt.Fprintln(os.Stderr, "benchmark:", err)
		os.Exit(1)
	}
}

func report() error {
	header := left("scenario", 46) + right("minified", 10) + right("compressed", 12) +
		right("saved", 8) + right("+gzip", 10) + right("saved", 8)
	fmt.Println("\n" + header)
	fmt.Println(strings.Repeat("-", len(header)))

	for _, scenario := range scenarios {
		state = seed
		value := scenario.build()

		minified, err := jsoncompress.FormatJSON(value)
		if err != nil {
			return err
		}
		packed, err := jsoncompress.Marshal(value)
		if err != nil {
			return err
		}

		// A benchmark that quietly loses data is not a benchmark.
		restored, err := jsoncompress.Unmarshal(packed)
		if err != nil {
			return err
		}
		again, err := jsoncompress.FormatJSON(restored)
		if err != nil {
			return err
		}
		if string(again) != string(minified) {
			return fmt.Errorf("%s did not round-trip", scenario.name)
		}

		minifiedGzip := jsoncompress.GzipSize(minified)
		packedGzip := jsoncompress.GzipSize(packed)

		fmt.Println(
			left(scenario.name, 46) +
				right(kilobytes(len(minified)), 10) +
				right(kilobytes(len(packed)), 12) +
				right(saved(len(packed), len(minified)), 8) +
				right(kilobytes(packedGzip), 10) +
				right(saved(packedGzip, minifiedGzip), 8),
		)
	}

	fmt.Println("\n\"saved\" is against minified JSON; the gzip columns compare compressed + gzip")
	fmt.Println("against minified + gzip, which is what an HTTP response actually costs.")
	return nil
}

func kilobytes(count int) string {
	return fmt.Sprintf("%.1f KB", float64(count)/1024)
}

func saved(packed, plain int) string {
	if plain == 0 {
		return "-"
	}
	return fmt.Sprintf("%.0f%%", (1-float64(packed)/float64(plain))*100)
}

// number writes a whole float the way the other benchmarks write it, so the
// generated documents are identical across implementations.
func number(value float64) string {
	return strconv.FormatFloat(value, 'f', -1, 64)
}

func left(text string, width int) string {
	if len(text) >= width {
		return text
	}
	return text + strings.Repeat(" ", width-len(text))
}

func right(text string, width int) string {
	if len(text) >= width {
		return text
	}
	return strings.Repeat(" ", width-len(text)) + text
}
