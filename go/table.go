package jsoncompress

// Columnar table planning.
//
// A slice of like-shaped objects is the single most wasteful thing JSON does:
// every key is spelled out again on every row. This decides, per slice, whether
// it is worth turning into a table - a column list plus a rows matrix - and how
// each individual column should be encoded.

// Column encodings, as they appear in a table node's ~e member.
const (
	colRaw   = 0 // cells are ordinary encoded values
	colDict  = 1 // cells are integer indices into the string catalogue
	colConst = 2 // the column never changes; its value lives in ~cv
)

// tablePlan is the decision taken about one slice during the analysis pass.
type tablePlan struct {
	columns       []string // every column, in an order that suits every row
	encodings     []int    // one of colRaw / colDict / colConst per column
	constants     []any    // values of the colConst columns, in column order
	variableCount int      // how many columns actually appear in the rows
}

// columnStats is what one pass over the cells needs to gather to choose an
// encoding for a column.
type columnStats struct {
	present       int
	strings       int
	onlyStrings   bool
	constant      bool
	constantValue any
	totalLength   int
	distinct      map[string]bool
}

// planTable decides how to encode a slice: a plan, or nil to write the slice
// element by element.
func planTable(items []any, options Options) *tablePlan {
	if !options.Tables {
		return nil
	}

	rowCount := len(items)
	if rowCount < options.MinRows {
		return nil
	}

	rows := make([]*Object, rowCount)
	for i, item := range items {
		row, ok := item.(*Object)
		if !ok || row == nil {
			return nil
		}
		rows[i] = row
	}

	// One column order has to stand in for every row's key order. When no such
	// order exists the slice is left as objects, which is fatter on the wire but
	// hands the document back exactly as it arrived.
	columns, ok := orderColumns(rows)
	if !ok {
		return nil
	}

	columnCount := len(columns)
	if columnCount == 0 {
		return nil
	}

	// One pass over the cells, gathering everything the encoding choice needs.
	stats := make([]columnStats, columnCount)
	for c := range stats {
		stats[c].onlyStrings = true
		stats[c].constant = true
		stats[c].distinct = map[string]bool{}
	}

	holes := 0
	for _, row := range rows {
		for c := 0; c < columnCount; c++ {
			stat := &stats[c]
			value, present := row.Get(columns[c])
			if !present {
				holes++
				stat.constant = false
				continue
			}
			stat.present++
			if stat.present == 1 {
				stat.constantValue = value
			} else if stat.constant && !sameValue(stat.constantValue, value) {
				stat.constant = false
			}
			if text, isText := value.(string); isText {
				stat.strings++
				stat.totalLength += utf16Length(text)
				stat.distinct[text] = true
			} else if value != nil {
				stat.onlyStrings = false
			}
		}
	}

	if float64(holes)/float64(rowCount*columnCount) > options.MaxHoleRatio {
		return nil
	}

	plan := &tablePlan{
		columns:   columns,
		encodings: make([]int, columnCount),
	}

	for c := 0; c < columnCount; c++ {
		stat := &stats[c]

		isConstant := options.Constants &&
			stat.constant &&
			stat.present == rowCount &&
			isPrimitive(stat.constantValue)

		if isConstant {
			plan.encodings[c] = colConst
			plan.constants = append(plan.constants, stat.constantValue)
			continue
		}

		plan.variableCount++

		// A dictionary column pays for itself when the same handful of strings
		// keep coming back: each cell shrinks to a small integer, and the
		// strings move into the catalogue where they are written exactly once.
		worthDictionary := options.DictColumns &&
			options.Strings &&
			stat.onlyStrings &&
			stat.strings >= 3 &&
			len(stat.distinct) > 0 &&
			float64(len(stat.distinct)) <= float64(stat.strings)*0.6 &&
			float64(stat.totalLength)/float64(stat.strings) >= 2

		if worthDictionary {
			plan.encodings[c] = colDict
		} else {
			plan.encodings[c] = colRaw
		}
	}

	return plan
}

// isPrimitive is true for a value that can safely be hoisted out of the rows and
// shared, which rules out objects and arrays: every restored row would otherwise
// hold the same reference.
func isPrimitive(value any) bool {
	switch value.(type) {
	case nil, bool, string, float64, int, int64, uint64:
		return true
	default:
		return false
	}
}

// sameValue compares two cells the way the constant-column test needs: by value
// for the scalars a constant column may hold, and never equal for anything else,
// which a container could not be hoisted into ~cv anyway.
func sameValue(a, b any) bool {
	switch typed := a.(type) {
	case nil:
		return b == nil
	case bool:
		other, ok := b.(bool)
		return ok && other == typed
	case string:
		other, ok := b.(string)
		return ok && other == typed
	case float64:
		other, ok := b.(float64)
		return ok && other == typed
	case int64:
		other, ok := b.(int64)
		return ok && other == typed
	case uint64:
		other, ok := b.(uint64)
		return ok && other == typed
	default:
		return false
	}
}
