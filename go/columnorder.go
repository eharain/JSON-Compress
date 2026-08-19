package jsoncompress

// Choosing the column order for a table.
//
// A table has one column list, but the rows that feed it each have their own
// key order. The decoder rebuilds every row by walking the columns, so the order
// it produces is only faithful if each row's keys appear in the column list in
// the same sequence they appeared in the row.
//
// That is a topological sort. Every row contributes the constraint "this key
// came before that one", and any linear extension of those constraints
// reproduces every row exactly. Where the constraints contradict each other -
// one row says name before id, another says id before name - no column order can
// satisfy both, and the slice is left as ordinary objects rather than quietly
// reordered.
//
// Ties are broken by first appearance, so the same input always produces the
// same output.

// orderColumns works out a column order that reproduces every row's key order,
// or reports that no such order exists.
func orderColumns(rows []*Object) ([]string, bool) {
	position := make(map[string]int)
	var columns []string

	// First pass: collect the keys, in the order the document first mentions
	// them. That order is the tie-break, and gives a stable result.
	for _, row := range rows {
		for i := 0; i < row.Len(); i++ {
			key, _ := row.At(i)
			if _, seen := position[key]; !seen {
				position[key] = len(columns)
				columns = append(columns, key)
			}
		}
	}

	count := len(columns)
	if count <= 1 {
		return columns, true
	}

	after := make([][]int, count)
	known := make([]map[int]bool, count)
	incoming := make([]int, count)

	// Second pass: one edge per adjacent pair of keys in each row. Transitivity
	// along the chain covers the rest of the row's ordering. Edges are recorded
	// once however many rows ask for them, so a long table costs no more than a
	// short one with the same columns.
	sawEdge := false
	for _, row := range rows {
		previous := -1
		for i := 0; i < row.Len(); i++ {
			key, _ := row.At(i)
			current := position[key]
			if previous != -1 && !known[previous][current] {
				if known[previous] == nil {
					known[previous] = make(map[int]bool, 2)
				}
				known[previous][current] = true
				after[previous] = append(after[previous], current)
				incoming[current]++
				sawEdge = true
			}
			previous = current
		}
	}

	// Every row was a single key or empty: nothing constrains the order.
	if !sawEdge {
		return columns, true
	}

	ready := make(minHeap, 0, count)
	for i := 0; i < count; i++ {
		if incoming[i] == 0 {
			ready.push(i)
		}
	}

	ordered := make([]string, 0, count)
	for len(ready) > 0 {
		at := ready.pop()
		ordered = append(ordered, columns[at])
		for _, next := range after[at] {
			incoming[next]--
			if incoming[next] == 0 {
				ready.push(next)
			}
		}
	}

	// Fewer keys placed than we started with means the constraints form a
	// cycle: two rows disagree about which key comes first.
	if len(ordered) != count {
		return nil, false
	}
	return ordered, true
}

// minHeap is a binary min-heap over column indices. Column counts are usually
// small, but a document can always be the one that is not, and a heap keeps the
// sort linear rather than quadratic in that case.
type minHeap []int

func (h *minHeap) push(value int) {
	items := append(*h, value)
	at := len(items) - 1
	for at > 0 {
		parent := (at - 1) / 2
		if items[parent] <= items[at] {
			break
		}
		items[parent], items[at] = items[at], items[parent]
		at = parent
	}
	*h = items
}

func (h *minHeap) pop() int {
	items := *h
	top := items[0]
	last := len(items) - 1
	items[0] = items[last]
	items = items[:last]

	for at := 0; ; {
		left, right := at*2+1, at*2+2
		smallest := at
		if left < len(items) && items[left] < items[smallest] {
			smallest = left
		}
		if right < len(items) && items[right] < items[smallest] {
			smallest = right
		}
		if smallest == at {
			break
		}
		items[smallest], items[at] = items[at], items[smallest]
		at = smallest
	}

	*h = items
	return top
}
