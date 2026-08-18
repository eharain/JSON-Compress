/**
 * Choosing the column order for a table.
 *
 * A table has one column list, but the rows that feed it each have their own
 * key order. The decoder rebuilds every row by walking the columns, so the
 * order it produces is only faithful if each row's keys appear in the column
 * list in the same sequence they appeared in the row.
 *
 * That is a topological sort. Every row contributes the constraint "this key
 * came before that one", and any linear extension of those constraints
 * reproduces every row exactly. Where the constraints contradict each other -
 * one row says name before id, another says id before name - no column order
 * can satisfy both, and the array is left as ordinary objects rather than
 * quietly reordered.
 *
 * Ties are broken by first appearance, so the same input always produces the
 * same output.
 */

/**
 * A binary min-heap over integers. Column counts are usually small, but a
 * document can always be the one that is not, and a heap keeps the sort linear
 * rather than quadratic in that case.
 */
class MinHeap {
  constructor() {
    this.items = [];
  }

  get size() {
    return this.items.length;
  }

  push(value) {
    const items = this.items;
    items.push(value);
    let at = items.length - 1;
    while (at > 0) {
      const parent = (at - 1) >> 1;
      if (items[parent] <= items[at]) break;
      const swap = items[parent];
      items[parent] = items[at];
      items[at] = swap;
      at = parent;
    }
  }

  pop() {
    const items = this.items;
    const top = items[0];
    const last = items.pop();
    if (items.length > 0) {
      items[0] = last;
      let at = 0;
      for (;;) {
        const left = at * 2 + 1;
        const right = left + 1;
        let smallest = at;
        if (left < items.length && items[left] < items[smallest]) smallest = left;
        if (right < items.length && items[right] < items[smallest]) smallest = right;
        if (smallest === at) break;
        const swap = items[smallest];
        items[smallest] = items[at];
        items[at] = swap;
        at = smallest;
      }
    }
    return top;
  }
}

/**
 * Work out a column order that reproduces every row's key order.
 *
 * @param {Array<Record<string, unknown>>} rows
 * @returns {string[]|null} the columns, or null when no order can satisfy every row
 */
export function orderColumns(rows) {
  const position = new Map();
  const columns = [];

  // First pass: collect the keys, in the order the document first mentions
  // them. That order is the tie-break, and gives a stable result.
  for (let i = 0; i < rows.length; i++) {
    const keys = Object.keys(rows[i]);
    for (let j = 0; j < keys.length; j++) {
      const key = keys[j];
      if (rows[i][key] === undefined) continue;
      if (!position.has(key)) {
        position.set(key, columns.length);
        columns.push(key);
      }
    }
  }

  const count = columns.length;
  if (count === 0) return columns;
  if (count === 1) return columns;

  /** @type {Array<Set<number>>} */
  const after = new Array(count);
  const incoming = new Int32Array(count);
  for (let i = 0; i < count; i++) after[i] = new Set();

  // Second pass: one edge per adjacent pair of keys in each row. Transitivity
  // along the chain covers the rest of the row's ordering.
  let sawEdge = false;
  for (let i = 0; i < rows.length; i++) {
    const keys = Object.keys(rows[i]);
    let previous = -1;
    for (let j = 0; j < keys.length; j++) {
      const key = keys[j];
      if (rows[i][key] === undefined) continue;
      const current = position.get(key);
      if (previous !== -1 && !after[previous].has(current)) {
        after[previous].add(current);
        incoming[current]++;
        sawEdge = true;
      }
      previous = current;
    }
  }

  // Every row was a single key or empty: nothing constrains the order.
  if (!sawEdge) return columns;

  const ready = new MinHeap();
  for (let i = 0; i < count; i++) {
    if (incoming[i] === 0) ready.push(i);
  }

  const ordered = [];
  while (ready.size > 0) {
    const at = ready.pop();
    ordered.push(columns[at]);
    for (const next of after[at]) {
      if (--incoming[next] === 0) ready.push(next);
    }
  }

  // Fewer keys placed than we started with means the constraints form a cycle:
  // two rows disagree about which key comes first.
  return ordered.length === count ? ordered : null;
}
