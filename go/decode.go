package jsoncompress

import "fmt"

// The decoder.
//
// A straight structural walk of the encoded document, resolving catalogue
// references as it goes. It is deliberately strict: anything that cannot be
// mapped back onto the original - an out-of-range token, a stray hole marker, a
// table whose rows do not line up with its columns - is an error rather than
// something quietly handed back nearly right.

// maxTableRows bounds what a single table node may claim its row count is. A
// table this large is not a document anybody wrote; it is a corrupt or hostile
// ~n, and refusing it keeps a bad byte from turning into an allocation the
// process cannot survive.
const maxTableRows = 1 << 26

// DecodeError says that a document is not a readable json-compress envelope.
type DecodeError struct {
	message string
}

func (e *DecodeError) Error() string {
	return "json-compress: " + e.message
}

func decodeErrorf(format string, args ...any) *DecodeError {
	return &DecodeError{message: fmt.Sprintf(format, args...)}
}

// context is the two catalogues, which is all the decoder needs to carry.
type context struct {
	keys    []string
	strings []string
}

// IsCompressed reports whether a value looks like a json-compress envelope this
// build can read.
func IsCompressed(value any) bool {
	object, ok := objectOf(value)
	if !ok {
		return false
	}
	version, present := object.Get(envVersion)
	if !present {
		return false
	}
	if at, ok := wholeNumber(version); !ok || at != FormatVersion {
		return false
	}
	keys, present := object.Get(envKeys)
	if !present {
		return false
	}
	_, isArray := keys.([]any)
	return isArray
}

// Decompress restores the original value from a json-compress envelope.
//
// The envelope may be an [*Object] - from [Compress], or from [ParseJSON] - or a
// map[string]any, as encoding/json would have decoded it. Note that a map has no
// key order to give back, so an envelope that has been through encoding/json
// restores its ordinary object nodes in catalogue order rather than the order
// they were written in; packed tables carry their own order and are unaffected.
// [Unmarshal] avoids the question by parsing the text itself.
func Decompress(envelope any) (any, error) {
	object, ok := objectOf(envelope)
	if !ok {
		return nil, decodeErrorf("expected a compressed envelope object")
	}

	version, present := object.Get(envVersion)
	if !present {
		return nil, decodeErrorf("this is not a compressed document (no %q version field)", envVersion)
	}
	if at, ok := wholeNumber(version); !ok || at != FormatVersion {
		return nil, decodeErrorf("format version %v is not supported by this build (expected %d)", version, FormatVersion)
	}

	rawKeys, present := object.Get(envKeys)
	if !present {
		return nil, decodeErrorf("the key catalogue is missing or is not an array")
	}
	keys, err := catalogueOf(rawKeys, "key")
	if err != nil {
		return nil, err
	}

	var strings []string
	if rawStrings, present := object.Get(envStrings); present && rawStrings != nil {
		strings, err = catalogueOf(rawStrings, "string")
		if err != nil {
			return nil, err
		}
	}

	return decodeValue(object.Value(envData), &context{keys: keys, strings: strings})
}

func catalogueOf(value any, what string) ([]string, error) {
	items, ok := value.([]any)
	if !ok {
		return nil, decodeErrorf("the %s catalogue is missing or is not an array", what)
	}
	out := make([]string, len(items))
	for i, item := range items {
		text, ok := item.(string)
		if !ok {
			return nil, decodeErrorf("the %s catalogue holds something that is not a string at index %d", what, i)
		}
		out[i] = text
	}
	return out, nil
}

func decodeValue(node any, ctx *context) (any, error) {
	switch typed := node.(type) {
	case string:
		return decodeString(typed, ctx)

	case []any:
		out := make([]any, len(typed))
		for i, item := range typed {
			value, err := decodeValue(item, ctx)
			if err != nil {
				return nil, err
			}
			out[i] = value
		}
		return out, nil

	case *Object, map[string]any:
		object, _ := objectOf(typed)
		if object.Has(tableColumns) {
			return decodeTable(object, ctx)
		}
		out := newObject(object.Len())
		for i := 0; i < object.Len(); i++ {
			token, item := object.At(i)
			key, err := decodeKey(token, ctx)
			if err != nil {
				return nil, err
			}
			value, err := decodeValue(item, ctx)
			if err != nil {
				return nil, err
			}
			out.Set(key, value)
		}
		return out, nil

	default:
		return node, nil
	}
}

func decodeString(value string, ctx *context) (string, error) {
	if len(value) == 0 || value[0] != tilde {
		return value, nil
	}
	if len(value) == 1 {
		return "", decodeErrorf("a row hole marker appeared outside a table row")
	}
	if value[1] == tilde {
		return value[1:], nil
	}
	at := fromBase62(value[1:])
	if at < 0 || at >= len(ctx.strings) {
		return "", decodeErrorf("string reference %q is out of range", value)
	}
	return ctx.strings[at], nil
}

func decodeKey(token string, ctx *context) (string, error) {
	at := fromKeyToken(token)
	if at < 0 || at >= len(ctx.keys) {
		return "", decodeErrorf("key token %q is out of range", token)
	}
	return ctx.keys[at], nil
}

func decodeTable(node *Object, ctx *context) (any, error) {
	rawColumns, ok := node.Value(tableColumns).([]any)
	if !ok {
		return nil, decodeErrorf("table column list is not an array")
	}

	columns := make([]string, len(rawColumns))
	for c, item := range rawColumns {
		at, ok := wholeNumber(item)
		if !ok || at < 0 || at >= len(ctx.keys) {
			return nil, decodeErrorf("table column index %v is out of range", item)
		}
		columns[c] = ctx.keys[at]
	}

	encodings := make([]int, len(columns))
	if raw, present := node.Get(tableEncodings); present && raw != nil {
		items, ok := raw.([]any)
		if !ok || len(items) != len(columns) {
			return nil, decodeErrorf("table encoding list does not match its columns")
		}
		for c, item := range items {
			encoding, ok := wholeNumber(item)
			if !ok {
				return nil, decodeErrorf("table encoding %v is not a column encoding", item)
			}
			encodings[c] = encoding
		}
	}

	var constants []any
	if raw, present := node.Get(tableConstants); present && raw != nil {
		items, ok := raw.([]any)
		if !ok {
			return nil, decodeErrorf("table constant list is not an array")
		}
		constants = make([]any, len(items))
		for i, item := range items {
			value, err := decodeValue(item, ctx)
			if err != nil {
				return nil, err
			}
			constants[i] = value
		}
	}

	var matrix []any
	haveMatrix := false
	if raw, present := node.Get(tableRows); present && raw != nil {
		items, ok := raw.([]any)
		if !ok {
			return nil, decodeErrorf("table rows are not an array")
		}
		matrix, haveMatrix = items, true
	}

	rowCount := len(matrix)
	if !haveMatrix {
		count, ok := wholeNumber(node.Value(tableRowCount))
		if node.Has(tableRowCount) && (!ok || count < 0) {
			return nil, decodeErrorf("table row count %v is not a row count", node.Value(tableRowCount))
		}
		if count > maxTableRows {
			return nil, decodeErrorf("table row count %d is larger than this build will read", count)
		}
		rowCount = count
	}

	out := make([]any, rowCount)
	for i := 0; i < rowCount; i++ {
		var cells []any
		if haveMatrix {
			items, ok := matrix[i].([]any)
			if !ok {
				return nil, decodeErrorf("table row %d is not an array", i)
			}
			cells = items
		}

		row := newObject(len(columns))
		constantAt, cellAt := 0, 0

		for c := range columns {
			if encodings[c] == colConst {
				if constantAt >= len(constants) {
					return nil, decodeErrorf("table has fewer constants than constant columns")
				}
				row.Set(columns[c], constants[constantAt])
				constantAt++
				continue
			}

			// A row shorter than the column list is missing everything past its
			// end.
			var cell any
			present := cellAt < len(cells)
			if present {
				cell = cells[cellAt]
			}
			cellAt++

			if encodings[c] == colDict {
				if !present {
					continue
				}
				if cell == nil {
					row.Set(columns[c], nil)
					continue
				}
				at, ok := wholeNumber(cell)
				if ok && at == dictHole {
					continue
				}
				if !ok || at < 0 || at >= len(ctx.strings) {
					return nil, decodeErrorf("dictionary index %v is out of range", cell)
				}
				row.Set(columns[c], ctx.strings[at])
				continue
			}

			if !present {
				continue
			}
			if text, isText := cell.(string); isText && text == hole {
				continue
			}
			value, err := decodeValue(cell, ctx)
			if err != nil {
				return nil, err
			}
			row.Set(columns[c], value)
		}

		out[i] = row
	}

	return out, nil
}

// wholeNumber reads an integer from whatever a JSON reader turned it into: a
// float64 from parsed text, or one of Go's own integer types from a value that
// never left the process.
func wholeNumber(value any) (int, bool) {
	switch typed := value.(type) {
	case float64:
		at := int(typed)
		return at, float64(at) == typed
	case int:
		return typed, true
	case int64:
		at := int(typed)
		return at, int64(at) == typed
	case uint64:
		at := int(typed)
		return at, at >= 0 && uint64(at) == typed
	default:
		return 0, false
	}
}
