package jsoncompress

// The shared document generator.
//
// Kept step for step with the one in javascript/tests/fuzz.test.js, so a seed
// means the same document in every language that reproduces it. That is what
// lets the conformance corpus be described by its seeds rather than only by its
// contents, and what makes a fuzz failure reproducible across ports.

var generatorWords = []string{
	"id", "name", "status", "created", "updated", "owner", "tags", "value",
	"count", "total", "items", "meta", "type", "code", "label", "notes",
	"~tilde", "0", "1", "10", "a", "z", "A", "~t", "~r", "toString",
	"__proto__", "constructor", "hasOwnProperty", "", " ", "ключ", "键",
}

var generatorStrings = []string{
	"", "a", "active", "pending", "cancelled", "~", "~~", "~0", "~zz",
	"a longer value that repeats across many records",
	"quote \" backslash \\ newline \n tab \t",
	"😀 emoji", "مرحبا", "null", "true", "0",
}

// createRandom is a small deterministic generator, so failures can be replayed.
func createRandom(seed int) func() float64 {
	state := uint32(seed)
	return func() float64 {
		state = state*1664525 + 1013904223
		return float64(state) / 4294967296
	}
}

func pick(random func() float64, values []string) string {
	return values[int(random()*float64(len(values)))]
}

func makeScalar(random func() float64) any {
	roll := random()
	switch {
	case roll < 0.2:
		return nil
	case roll < 0.3:
		return random() < 0.5
	case roll < 0.55:
		return float64(int(random()*2000) - 1000)
	case roll < 0.62:
		return random()*1e6 - 5e5
	default:
		return pick(random, generatorStrings)
	}
}

func makeValue(random func() float64, depth int) any {
	if depth <= 0 {
		return makeScalar(random)
	}

	roll := random()

	if roll < 0.45 {
		return makeScalar(random)
	}

	if roll < 0.6 {
		length := int(random() * 6)
		out := make([]any, length)
		for i := range out {
			out[i] = makeValue(random, depth-1)
		}
		return out
	}

	if roll < 0.8 {
		// A record set: the shape the format is built for, with ragged rows and
		// low-cardinality columns thrown in on purpose.
		rows := int(random() * 8)
		columnCount := 1 + int(random()*5)
		columns := make([]string, columnCount)
		for c := range columns {
			columns[c] = pick(random, generatorWords)
		}

		out := make([]any, rows)
		for i := range out {
			row := NewObject()
			for _, column := range columns {
				if random() < 0.15 {
					continue
				}
				if random() < 0.5 {
					row.Set(column, pick(random, generatorStrings))
				} else {
					row.Set(column, makeValue(random, depth-2))
				}
			}
			out[i] = row
		}
		return out
	}

	size := int(random() * 6)
	out := NewObject()
	for i := 0; i < size; i++ {
		out.Set(pick(random, generatorWords), makeValue(random, depth-1))
	}
	return out
}
