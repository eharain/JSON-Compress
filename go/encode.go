package jsoncompress

import "sort"

// The encoder.
//
// Two passes. The first walks the document and counts things: how often each key
// is used, how often each string value appears, and which slices are worth
// turning into tables. The second builds the output using catalogues ordered by
// those counts, so the most-used key gets the one-character token.
//
// Splitting it this way costs a second walk and buys a materially smaller
// document, because token length is decided by frequency rather than by the
// accident of which key the encoder happened to meet first.

// Compress packs a value into a json-compress envelope.
//
// The result is an ordinary JSON-shaped value, so it can be stored, posted or
// logged anywhere plain JSON goes; [Marshal] does the same thing and hands back
// the text. The input is left untouched, and anything encoding/json can marshal
// can be given ([Normalize] describes how each kind of value is read).
//
// At most one set of options may be given; with none, [DefaultOptions] apply.
func Compress(value any, options ...Options) (*Object, error) {
	resolved, err := resolveOptions(options)
	if err != nil {
		return nil, err
	}

	source, err := Normalize(value)
	if err != nil {
		return nil, err
	}

	state := newAnalysis(resolved)
	prepared := state.value(source)

	builder := &encoder{
		keys:    state.keyCatalogue(),
		strings: state.stringCatalogue(),
	}
	data := builder.value(prepared)

	envelope := newObject(4)
	envelope.Set(envVersion, int64(FormatVersion))
	envelope.Set(envKeys, builder.keys.list)
	if len(builder.strings.list) > 0 {
		envelope.Set(envStrings, builder.strings.list)
	}
	envelope.Set(envData, data)
	return envelope, nil
}

/* ------------------------------------------------------------------ *
 * Pass one - analysis
 * ------------------------------------------------------------------ */

// plannedTable is a slice the analysis pass decided to pack, carrying the plan
// and the rows with their nested values already prepared.
//
// The JavaScript encoder keys its plans by the identity of the array. A Go slice
// has no identity a map can hold, so the analysis pass returns a prepared tree
// instead and the build pass reads the type rather than performing a lookup.
type plannedTable struct {
	plan *tablePlan
	rows []*Object
}

type analysis struct {
	options Options

	keyOrder []string       // keys in the order they were first met
	keyCount map[string]int // and how often each is written

	stringOrder []string
	stringCount map[string]int
	forced      map[string]bool // values a dictionary column has to catalogue
}

func newAnalysis(options Options) *analysis {
	return &analysis{
		options:     options,
		keyCount:    map[string]int{},
		stringCount: map[string]int{},
		forced:      map[string]bool{},
	}
}

func (a *analysis) countKey(key string) {
	if _, seen := a.keyCount[key]; !seen {
		a.keyOrder = append(a.keyOrder, key)
	}
	a.keyCount[key]++
}

func (a *analysis) countString(value string) {
	if !a.options.Strings {
		return
	}
	if _, seen := a.stringCount[value]; !seen {
		a.stringOrder = append(a.stringOrder, value)
	}
	a.stringCount[value]++
}

// value counts what is in a value and returns it prepared for the build pass.
func (a *analysis) value(value any) any {
	switch typed := value.(type) {
	case string:
		a.countString(typed)
		return typed

	case []any:
		if plan := planTable(typed, a.options); plan != nil {
			return a.table(typed, plan)
		}
		out, changed := typed, false
		for i, item := range typed {
			next := a.value(item)
			if !changed && !identical(next, item) {
				// Copy on the first difference, so a subtree with no table in
				// it is carried through rather than rebuilt.
				out = make([]any, len(typed))
				copy(out, typed[:i])
				changed = true
			}
			if changed {
				out[i] = next
			}
		}
		return out

	case *Object:
		var prepared *Object
		for i := 0; i < typed.Len(); i++ {
			key, item := typed.At(i)
			a.countKey(key)
			next := a.value(item)
			if prepared == nil && !identical(next, item) {
				prepared = newObject(typed.Len())
				for j := 0; j < i; j++ {
					prepared.Set(typed.At(j))
				}
			}
			if prepared != nil {
				prepared.Set(key, next)
			}
		}
		if prepared != nil {
			return prepared
		}
		return typed

	default:
		return value
	}
}

func (a *analysis) table(items []any, plan *tablePlan) *plannedTable {
	// A table spells each key out once, in the column list, however many rows it
	// has - so it contributes one to the key's frequency, not one per row.
	for _, key := range plan.columns {
		a.countKey(key)
	}
	// Constant values are scalars by construction, so this only counts strings.
	for _, constant := range plan.constants {
		a.value(constant)
	}

	rows := make([]*Object, len(items))
	for i, item := range items {
		row := item.(*Object)
		var prepared *Object

		for c, key := range plan.columns {
			if plan.encodings[c] == colConst {
				continue
			}
			item, present := row.Get(key)
			if !present {
				continue
			}
			if plan.encodings[c] == colDict {
				if text, ok := item.(string); ok {
					a.countString(text)
					a.forced[text] = true
				}
				continue
			}
			next := a.value(item)
			if prepared == nil && !identical(next, item) {
				prepared = row.Clone()
			}
			if prepared != nil {
				prepared.Set(key, next)
			}
		}

		if prepared != nil {
			rows[i] = prepared
		} else {
			rows[i] = row
		}
	}

	return &plannedTable{plan: plan, rows: rows}
}

/* ------------------------------------------------------------------ *
 * Catalogues
 * ------------------------------------------------------------------ */

type keyCatalogue struct {
	list   []any          // the names, as they go into the envelope
	index  map[string]int // name to catalogue index
	tokens map[string]string
}

// keyCatalogue orders the keys by how often they are written, most-used first,
// so the busiest keys get the shortest tokens. Ties keep the order the document
// first mentioned them in, which makes the result reproducible.
func (a *analysis) keyCatalogue() keyCatalogue {
	order := make([]string, len(a.keyOrder))
	copy(order, a.keyOrder)
	sort.SliceStable(order, func(i, j int) bool {
		return a.keyCount[order[i]] > a.keyCount[order[j]]
	})

	out := keyCatalogue{
		list:   make([]any, len(order)),
		index:  make(map[string]int, len(order)),
		tokens: make(map[string]string, len(order)),
	}
	for i, key := range order {
		out.list[i] = key
		out.index[key] = i
		out.tokens[key] = toKeyToken(i)
	}
	return out
}

type stringCatalogue struct {
	list  []any
	index map[string]int
}

type stringCandidate struct {
	value  string
	count  int
	length int
	forced bool
}

// stringCatalogue chooses which string values are worth moving into the
// catalogue.
//
// A catalogued string is written once in the catalogue and referenced by a short
// ~token everywhere it occurs, so the trade is:
//
//	inline    = count * (length + 2)                  // quotes on every copy
//	catalogue = count * (tokenLength + 3) + length + 3
//
// Candidates are considered in frequency order, so the token length used in that
// comparison is the one the string would really be given.
//
// Values pulled into a dictionary column are forced in regardless: their cells
// are bare integers, which is cheaper still, and the comparison above does not
// describe them.
func (a *analysis) stringCatalogue() stringCatalogue {
	out := stringCatalogue{index: map[string]int{}}
	if !a.options.Strings {
		return out
	}

	candidates := make([]stringCandidate, 0, len(a.stringOrder))
	for _, value := range a.stringOrder {
		forced := a.forced[value]
		length := utf16Length(value)
		if !forced {
			if a.stringCount[value] < a.options.MinStringCount {
				continue
			}
			if length < a.options.MinStringLength {
				continue
			}
			// A value that already begins with a tilde is escaped inline and can
			// never be written as a reference, so cataloguing it would only burn
			// an index.
			if len(value) > 0 && value[0] == tilde {
				continue
			}
		}
		candidates = append(candidates, stringCandidate{value, a.stringCount[value], length, forced})
	}

	sort.SliceStable(candidates, func(i, j int) bool {
		if candidates[i].count != candidates[j].count {
			return candidates[i].count > candidates[j].count
		}
		return candidates[i].length > candidates[j].length
	})

	for _, candidate := range candidates {
		if !candidate.forced {
			tokenLength := base62Length(len(out.list))
			inline := candidate.count * (candidate.length + 2)
			catalogued := candidate.count*(tokenLength+3) + candidate.length + 3
			if catalogued >= inline {
				continue
			}
		}
		out.index[candidate.value] = len(out.list)
		out.list = append(out.list, candidate.value)
	}

	return out
}

/* ------------------------------------------------------------------ *
 * Pass two - build
 * ------------------------------------------------------------------ */

type encoder struct {
	keys    keyCatalogue
	strings stringCatalogue
}

func (e *encoder) text(value string) string {
	// Escape first: an escaped literal always starts with two tildes, a
	// reference is a tilde followed by a base-62 digit, and a lone tilde is the
	// row hole. Those three spaces cannot overlap.
	if len(value) > 0 && value[0] == tilde {
		return hole + value
	}
	if at, ok := e.strings.index[value]; ok {
		return hole + toBase62(at)
	}
	return value
}

func (e *encoder) value(value any) any {
	switch typed := value.(type) {
	case string:
		return e.text(typed)

	case []any:
		out := make([]any, len(typed))
		for i, item := range typed {
			out[i] = e.value(item)
		}
		return out

	case *plannedTable:
		return e.table(typed)

	case *Object:
		out := newObject(typed.Len())
		for i := 0; i < typed.Len(); i++ {
			key, item := typed.At(i)
			out.Set(e.keys.tokens[key], e.value(item))
		}
		return out

	default:
		return value
	}
}

func (e *encoder) table(table *plannedTable) *Object {
	plan := table.plan

	node := newObject(4)

	columns := make([]any, len(plan.columns))
	uniform := true
	encodings := make([]any, len(plan.encodings))
	for c, key := range plan.columns {
		columns[c] = int64(e.keys.index[key])
		encodings[c] = int64(plan.encodings[c])
		if plan.encodings[c] != colRaw {
			uniform = false
		}
	}
	node.Set(tableColumns, columns)
	if !uniform {
		node.Set(tableEncodings, encodings)
	}

	if len(plan.constants) > 0 {
		constants := make([]any, len(plan.constants))
		for i, constant := range plan.constants {
			constants[i] = e.value(constant)
		}
		node.Set(tableConstants, constants)
	}

	if plan.variableCount == 0 {
		// Every column turned out to be constant, so the rows carry no data at
		// all and only their number survives.
		node.Set(tableRowCount, int64(len(table.rows)))
		return node
	}

	matrix := make([]any, len(table.rows))
	cells := make([]any, plan.variableCount)
	// Whether a cell is a hole is recorded rather than inferred from what it
	// holds: a raw column may legitimately contain the number -1, which is
	// exactly what a dictionary column writes for an absent key.
	holes := make([]bool, plan.variableCount)

	for i, row := range table.rows {
		at := 0
		for c, key := range plan.columns {
			encoding := plan.encodings[c]
			if encoding == colConst {
				continue
			}
			value, present := row.Get(key)
			switch {
			case encoding == colDict && !present:
				cells[at] = int64(dictHole)
			case encoding == colDict && value == nil:
				cells[at] = nil
			case encoding == colDict:
				text, _ := value.(string)
				cells[at] = int64(e.strings.index[text])
			case !present:
				cells[at] = hole
			default:
				cells[at] = e.value(value)
			}
			holes[at] = !present
			at++
		}

		// Trailing gaps need not be written down; a short row is understood to
		// be missing everything past its end.
		end := len(cells)
		for end > 0 && holes[end-1] {
			end--
		}
		out := make([]any, end)
		copy(out, cells[:end])
		matrix[i] = out
	}

	node.Set(tableRows, matrix)
	return node
}
