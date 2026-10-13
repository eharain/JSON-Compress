// Package jsoncompress is lossless JSON compression that is still JSON.
//
// Three ideas, applied together:
//
//  1. Every object key is replaced by a short token and the real names are
//     listed once, in a catalogue. A key that occurs ten thousand times is
//     spelled out once.
//  2. A slice of like-shaped objects becomes a table - one column list and a
//     matrix of rows - so the keys disappear from the rows entirely. Columns
//     whose value never changes are hoisted out of the rows altogether, and
//     columns drawn from a small set of strings become integer indices.
//  3. String values that repeat are moved into a second catalogue and
//     referenced by token.
//
// The output is an ordinary JSON value with no binary framing, so it survives
// anything that carries JSON: an HTTP body, a WebSocket frame, a jsonb column,
// a queue message, a log line.
//
//	envelope, err := jsoncompress.Compress([]any{
//		map[string]any{"id": 1, "role": "admin"},
//		map[string]any{"id": 2, "role": "admin"},
//	})
//	back, err := jsoncompress.Decompress(envelope)
//
// The wire format is specified in SPEC.md at the root of the repository, and
// this implementation writes the same bytes as the JavaScript, .NET, Python and
// PHP ones.
//
// # The value model
//
// A restored document is built from the types JSON has and no others: nil,
// bool, float64, string, []any and [*Object]. Objects are [*Object] rather than
// map[string]any because a Go map has no order and this format promises to give
// key order back exactly as it arrived.
//
// Input is more forgiving. Anything encoding/json can marshal can be
// compressed - structs with their tags, maps, slices, [json.Marshaler],
// [encoding.TextMarshaler], time.Time - and is read the way encoding/json reads
// it, with map keys sorted because a Go map cannot say what order it wants.
package jsoncompress

// Version is the version of this implementation, which is not the same thing as
// the version of the wire format ([FormatVersion]).
const Version = "1.0.3"
