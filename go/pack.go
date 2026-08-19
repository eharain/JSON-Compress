package jsoncompress

import (
	"strconv"
	"strings"
)

// The plain table pack.
//
// This is the smallest useful idea in the library, offered on its own because it
// is often all you need and because the shape it produces - fields, data and
// count - is trivial to consume from another language or straight out of a SQL
// driver.
//
// Pack turns a slice of records into one field list and a matrix of rows. Unpack
// turns it back. Neither uses a catalogue, an envelope or a version tag; what
// you get is a readable value you can eyeball in a network panel.

// PackedTable is a record set with its keys lifted out of the rows.
type PackedTable struct {
	// Fields are the column names, in the order the row slices use.
	Fields []string `json:"fields"`
	// Data is one slice of values per record.
	Data [][]any `json:"data"`
	// Count is the number of records, so consumers need not measure.
	Count int `json:"count"`
}

// Pack flattens a slice of records into a field list and a rows matrix, and
// reports whether it could.
//
// Fields are taken from the union of every record, in first-seen order, so a
// record that is missing one contributes nil in that column. Anything that is
// not a slice of objects is refused rather than mangled, which is what the
// second return value is for:
//
//	if table, ok := jsoncompress.Pack(rows); ok {
//		payload = table
//	}
//
// Records may be anything [Normalize] reads as an object - an [*Object], a map,
// a struct - so what a database driver hands back goes straight in.
func Pack(records any) (*PackedTable, bool) {
	items, ok := recordsOf(records)
	if !ok {
		return nil, false
	}
	if len(items) == 0 {
		return &PackedTable{Fields: []string{}, Data: [][]any{}}, true
	}

	rows := make([]*Object, len(items))
	for i, item := range items {
		row, ok := objectOf(item)
		if !ok {
			normalised, err := Normalize(item)
			if err != nil {
				return nil, false
			}
			if row, ok = objectOf(normalised); !ok {
				return nil, false
			}
		}
		rows[i] = row
	}

	seen := map[string]bool{}
	fields := []string{}
	for _, row := range rows {
		for i := 0; i < row.Len(); i++ {
			key, _ := row.At(i)
			if !seen[key] {
				seen[key] = true
				fields = append(fields, key)
			}
		}
	}

	data := make([][]any, len(rows))
	for i, row := range rows {
		cells := make([]any, len(fields))
		for j, field := range fields {
			cells[j] = row.Value(field)
		}
		data[i] = cells
	}

	return &PackedTable{Fields: fields, Data: data, Count: len(data)}, true
}

// Unpack rebuilds the records [Pack] flattened, and reports whether the value
// was a packed table at all.
func Unpack(packed any) ([]any, bool) {
	fields, data, ok := packedShape(packed)
	if !ok {
		return nil, false
	}

	out := make([]any, len(data))
	for i, row := range data {
		record := newObject(len(fields))
		for j, field := range fields {
			var value any
			if j < len(row) {
				value = row[j]
			}
			record.Set(field, value)
		}
		out[i] = record
	}
	return out, true
}

// IsPacked reports whether a value has the shape [Unpack] knows how to read.
func IsPacked(value any) bool {
	_, _, ok := packedShape(value)
	return ok
}

// PackAll packs every result set in a slice of result sets.
//
// Database drivers that return several result sets from one call hand back a
// slice of slices; this packs each inner one and leaves the outer one alone. A
// set that cannot be packed is carried through as it was, so the result holds a
// [*PackedTable] where packing worked and the original set where it did not.
func PackAll(resultSets any) ([]any, bool) {
	sets, ok := recordsOf(resultSets)
	if !ok {
		return nil, false
	}
	out := make([]any, len(sets))
	for i, set := range sets {
		if table, ok := Pack(set); ok {
			out[i] = table
		} else {
			out[i] = set
		}
	}
	return out, true
}

// UnpackAll unpacks every result set [PackAll] packed, carrying through anything
// that was not a packed table.
func UnpackAll(resultSets any) ([]any, bool) {
	sets, ok := recordsOf(resultSets)
	if !ok {
		return nil, false
	}
	out := make([]any, len(sets))
	for i, set := range sets {
		if records, ok := Unpack(set); ok {
			out[i] = records
		} else {
			out[i] = set
		}
	}
	return out, true
}

// recordsOf reads a value as a slice, through the normaliser so that a typed
// slice, an array or a driver's own row type is read the same way encoding/json
// would read it.
func recordsOf(value any) ([]any, bool) {
	if value == nil {
		return nil, false
	}
	if items, ok := value.([]any); ok {
		return items, true
	}
	normalised, err := Normalize(value)
	if err != nil {
		return nil, false
	}
	items, ok := normalised.([]any)
	return items, ok
}

// packedShape reads the fields and rows out of whatever form a packed table
// arrived in: the struct this package produces, or the object another
// implementation of the format wrote.
func packedShape(value any) ([]string, [][]any, bool) {
	if table, ok := value.(*PackedTable); ok {
		if table == nil {
			return nil, nil, false
		}
		return table.Fields, table.Data, true
	}

	object, ok := objectOf(value)
	if !ok {
		return nil, nil, false
	}
	rawFields, isArray := object.Value("fields").([]any)
	if !isArray {
		return nil, nil, false
	}
	rawData, isArray := object.Value("data").([]any)
	if !isArray {
		return nil, nil, false
	}

	fields := make([]string, len(rawFields))
	for i, field := range rawFields {
		text, isText := field.(string)
		if !isText {
			return nil, nil, false
		}
		fields[i] = text
	}

	data := make([][]any, len(rawData))
	for i, row := range rawData {
		cells, isArray := row.([]any)
		if !isArray {
			return nil, nil, false
		}
		data[i] = cells
	}
	return fields, data, true
}

// TextShape describes how a delimited column is written: how many characters to
// trim from each end, and what separates the values.
type TextShape struct {
	Open      int
	Close     int
	Separator string
}

// DefaultTextShape is the shape [TextToArray] assumes when none is given, which
// reads a column written as "[[[1,2,3]]]".
func DefaultTextShape() TextShape {
	return TextShape{Open: 3, Close: 3, Separator: ","}
}

// TextToArray reads a delimited-integer column back into a slice of numbers.
//
// Some databases return a set-valued column as a bracketed, comma-separated
// string. This converts one such member in place, leaving an empty slice where
// the column was null, and returns the record so calls can be chained.
func TextToArray(record *Object, property string, shape ...TextShape) *Object {
	if record == nil || !record.Has(property) {
		return record
	}

	form := DefaultTextShape()
	if len(shape) > 0 {
		form = shape[0]
		if form.Separator == "" {
			form.Separator = ","
		}
	}

	text, isText := record.Value(property).(string)
	if !isText {
		return record.Set(property, []any{})
	}

	head, tail := form.Open, form.Close
	if head < 0 {
		head = 0
	}
	if tail < 0 {
		tail = 0
	}
	if head+tail >= len(text) {
		return record.Set(property, []any{})
	}

	inner := text[head : len(text)-tail]
	if inner == "" {
		return record.Set(property, []any{})
	}

	parts := strings.Split(inner, form.Separator)
	out := make([]any, len(parts))
	for i, part := range parts {
		number, err := strconv.ParseFloat(strings.TrimSpace(part), 64)
		if err != nil {
			out[i] = nil // what a value that is not a number parses to
			continue
		}
		out[i] = number
	}
	return record.Set(property, out)
}
