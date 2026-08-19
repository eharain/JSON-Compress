//! Choosing the column order for a table.
//!
//! A table has one column list, but the rows that feed it each have their own
//! key order. The decoder rebuilds every row by walking the columns, so the
//! order it produces is only faithful if each row's keys appear in the column
//! list in the same sequence they appeared in the row.
//!
//! That is a topological sort. Every row contributes the constraint "this key
//! came before that one", and any linear extension of those constraints
//! reproduces every row exactly. Where the constraints contradict each other -
//! one row says `name` before `id`, another says `id` before `name` - no column
//! order can satisfy both, and the array is left as ordinary objects rather than
//! quietly reordered.
//!
//! Ties are broken by first appearance, so the same input always produces the
//! same output.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::value::Object;

/// Works out a column order that reproduces every row's key order, or reports
/// that no such order exists.
pub(crate) fn order_columns(rows: &[&Object]) -> Option<Vec<String>> {
    let mut position: HashMap<String, usize> = HashMap::new();
    let mut columns: Vec<String> = Vec::new();

    // First pass: collect the keys, in the order the document first mentions
    // them. That order is the tie-break, and gives a stable result.
    for row in rows {
        for key in row.keys() {
            if !position.contains_key(key) {
                position.insert(key.to_owned(), columns.len());
                columns.push(key.to_owned());
            }
        }
    }

    let count = columns.len();
    if count <= 1 {
        return Some(columns);
    }

    let mut after: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut known: Vec<HashSet<usize>> = vec![HashSet::new(); count];
    let mut incoming: Vec<usize> = vec![0; count];

    // Second pass: one edge per adjacent pair of keys in each row. Transitivity
    // along the chain covers the rest of the row's ordering. Edges are recorded
    // once however many rows ask for them, so a long table costs no more than a
    // short one with the same columns.
    let mut saw_edge = false;
    for row in rows {
        let mut previous: Option<usize> = None;
        for key in row.keys() {
            let current = position[key];
            if let Some(previous) = previous {
                if known[previous].insert(current) {
                    after[previous].push(current);
                    incoming[current] += 1;
                    saw_edge = true;
                }
            }
            previous = Some(current);
        }
    }

    // Every row was a single key or empty: nothing constrains the order.
    if !saw_edge {
        return Some(columns);
    }

    let mut ready: BinaryHeap<Reverse<usize>> = BinaryHeap::with_capacity(count);
    for (at, edges) in incoming.iter().enumerate() {
        if *edges == 0 {
            ready.push(Reverse(at));
        }
    }

    let mut ordered: Vec<String> = Vec::with_capacity(count);
    while let Some(Reverse(at)) = ready.pop() {
        ordered.push(std::mem::take(&mut columns[at]));
        for next in &after[at] {
            incoming[*next] -= 1;
            if incoming[*next] == 0 {
                ready.push(Reverse(*next));
            }
        }
    }

    // Fewer keys placed than we started with means the constraints form a cycle:
    // two rows disagree about which key comes first.
    if ordered.len() != count {
        return None;
    }
    Some(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(shapes: &[&[&str]]) -> Option<Vec<String>> {
        let rows: Vec<Object> = shapes
            .iter()
            .map(|keys| {
                let mut row = Object::new();
                for key in *keys {
                    row.insert(*key, 1);
                }
                row
            })
            .collect();
        order_columns(&rows.iter().collect::<Vec<_>>())
    }

    fn names(keys: &[&str]) -> Option<Vec<String>> {
        Some(keys.iter().map(|key| (*key).to_owned()).collect())
    }

    #[test]
    fn agreeing_rows_give_one_order() {
        assert_eq!(
            order(&[&["a", "b", "c"], &["a", "c"], &["b", "c"]]),
            names(&["a", "b", "c"])
        );
    }

    #[test]
    fn ties_are_broken_by_first_appearance() {
        assert_eq!(order(&[&["a"], &["b"], &["c"]]), names(&["a", "b", "c"]));
    }

    #[test]
    fn rows_that_disagree_have_no_order() {
        assert_eq!(order(&[&["a", "b"], &["b", "a"]]), None);
    }
}
