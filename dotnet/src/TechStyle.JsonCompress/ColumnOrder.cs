using System.Collections.Generic;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// Choosing the column order for a table.
/// </summary>
/// <remarks>
/// <para>
/// A table has one column list, but the rows that feed it each have their own
/// key order. The decoder rebuilds every row by walking the columns, so the
/// order it produces is only faithful if each row keys appear in the column
/// list in the same sequence they appeared in the row.
/// </para>
/// <para>
/// That is a topological sort. Every row contributes the constraint "this key
/// came before that one", and any linear extension of those constraints
/// reproduces every row exactly. Where the constraints contradict each other -
/// one row says name before id, another says id before name - no column order
/// can satisfy both, and the array is left as ordinary objects rather than
/// quietly reordered.
/// </para>
/// <para>
/// Ties are broken by first appearance, so the same input always produces the
/// same output.
/// </para>
/// </remarks>
internal static class ColumnOrder
{
    /// <summary>
    /// Work out a column order that reproduces the key order of every row.
    /// </summary>
    /// <param name="rows">The objects that would become rows.</param>
    /// <returns>The columns, or null when no order can satisfy every row.</returns>
    public static List<string>? Order(IReadOnlyList<JsonObject> rows)
    {
        var position = new Dictionary<string, int>(System.StringComparer.Ordinal);
        var columns = new List<string>();

        // First pass: collect the keys, in the order the document first mentions
        // them. That order is the tie-break, and gives a stable result.
        for (var i = 0; i < rows.Count; i++)
        {
            foreach (var property in rows[i])
            {
                if (!position.ContainsKey(property.Key))
                {
                    position.Add(property.Key, columns.Count);
                    columns.Add(property.Key);
                }
            }
        }

        var count = columns.Count;
        if (count <= 1)
        {
            return columns;
        }

        var after = new HashSet<int>[count];
        var incoming = new int[count];
        for (var i = 0; i < count; i++)
        {
            after[i] = new HashSet<int>();
        }

        // Second pass: one edge per adjacent pair of keys in each row.
        // Transitivity along the chain covers the rest of the row ordering.
        var sawEdge = false;
        for (var i = 0; i < rows.Count; i++)
        {
            var previous = -1;
            foreach (var property in rows[i])
            {
                var current = position[property.Key];
                if (previous != -1 && after[previous].Add(current))
                {
                    incoming[current]++;
                    sawEdge = true;
                }

                previous = current;
            }
        }

        // Every row was a single key or empty: nothing constrains the order.
        if (!sawEdge)
        {
            return columns;
        }

        var ready = new MinHeap(count);
        for (var i = 0; i < count; i++)
        {
            if (incoming[i] == 0)
            {
                ready.Push(i);
            }
        }

        var ordered = new List<string>(count);
        while (ready.Count > 0)
        {
            var at = ready.Pop();
            ordered.Add(columns[at]);
            foreach (var next in after[at])
            {
                if (--incoming[next] == 0)
                {
                    ready.Push(next);
                }
            }
        }

        // Fewer keys placed than we started with means the constraints form a
        // cycle: two rows disagree about which key comes first.
        return ordered.Count == count ? ordered : null;
    }

    /// <summary>
    /// A binary min-heap over integers.
    /// </summary>
    /// <remarks>
    /// Column counts are usually small, but a document can always be the one
    /// that is not, and a heap keeps the sort linear rather than quadratic in
    /// that case. Always taking the lowest-numbered ready column is what makes
    /// first appearance the tie-break.
    /// </remarks>
    private sealed class MinHeap
    {
        private readonly List<int> items;

        public MinHeap(int capacity)
        {
            this.items = new List<int>(capacity);
        }

        public int Count => this.items.Count;

        public void Push(int value)
        {
            this.items.Add(value);
            var at = this.items.Count - 1;
            while (at > 0)
            {
                var parent = (at - 1) >> 1;
                if (this.items[parent] <= this.items[at])
                {
                    break;
                }

                (this.items[parent], this.items[at]) = (this.items[at], this.items[parent]);
                at = parent;
            }
        }

        public int Pop()
        {
            var top = this.items[0];
            var last = this.items[this.items.Count - 1];
            this.items.RemoveAt(this.items.Count - 1);

            if (this.items.Count > 0)
            {
                this.items[0] = last;
                var at = 0;
                while (true)
                {
                    var left = (at * 2) + 1;
                    var right = left + 1;
                    var smallest = at;
                    if (left < this.items.Count && this.items[left] < this.items[smallest])
                    {
                        smallest = left;
                    }

                    if (right < this.items.Count && this.items[right] < this.items[smallest])
                    {
                        smallest = right;
                    }

                    if (smallest == at)
                    {
                        break;
                    }

                    (this.items[smallest], this.items[at]) = (this.items[at], this.items[smallest]);
                    at = smallest;
                }
            }

            return top;
        }
    }
}
