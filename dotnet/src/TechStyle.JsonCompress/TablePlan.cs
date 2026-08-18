using System.Collections.Generic;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// How one array is going to be written as a table.
/// </summary>
/// <remarks>
/// Worked out once, during the analysis pass, and filed against the array it
/// describes so the build pass does not have to work it out again.
/// </remarks>
internal sealed class TablePlan
{
    /// <summary>Every column, in an order that reproduces the key order of every row.</summary>
    public List<string> Columns { get; set; } = new List<string>();

    /// <summary>One of the column encodings from <see cref="Wire"/>, per column.</summary>
    public int[] Encodings { get; set; } = System.Array.Empty<int>();

    /// <summary>
    /// Values of the constant columns, in column order. These are still owned
    /// by the source document and must be copied before being attached to the
    /// output.
    /// </summary>
    public List<JsonNode?> Constants { get; set; } = new List<JsonNode?>();

    /// <summary>How many columns actually appear in the rows.</summary>
    public int VariableCount { get; set; }
}

/// <summary>
/// Columnar table planning.
/// </summary>
/// <remarks>
/// An array of like-shaped objects is the single most wasteful thing JSON does:
/// every key is spelled out again on every row. This decides, per array,
/// whether it is worth turning into a table - a column list plus a rows matrix
/// - and how each individual column should be encoded.
/// </remarks>
internal static class TablePlanner
{
    /// <summary>
    /// Decide how to encode an array.
    /// </summary>
    /// <param name="array">The array being considered.</param>
    /// <param name="options">The encoder options in force.</param>
    /// <returns>A plan, or null to encode the array element by element.</returns>
    public static TablePlan? Plan(JsonArray array, CompressOptions options)
    {
        if (!options.Tables)
        {
            return null;
        }

        var rowCount = array.Count;
        if (rowCount < options.MinRows)
        {
            return null;
        }

        var rows = new JsonObject[rowCount];
        for (var i = 0; i < rowCount; i++)
        {
            if (array[i] is not JsonObject row)
            {
                return null;
            }

            rows[i] = row;
        }

        // One column order has to stand in for the key order of every row. When
        // no such order exists the array is left as objects, which is fatter on
        // the wire but hands the document back exactly as it arrived.
        var columns = ColumnOrder.Order(rows);
        if (columns is null)
        {
            return null;
        }

        var columnCount = columns.Count;
        if (columnCount == 0)
        {
            return null;
        }

        // One pass over the cells, gathering everything the encoding choice needs.
        var stats = new ColumnStats[columnCount];
        for (var c = 0; c < columnCount; c++)
        {
            stats[c] = new ColumnStats();
        }

        long holes = 0;
        for (var i = 0; i < rowCount; i++)
        {
            var row = rows[i];
            for (var c = 0; c < columnCount; c++)
            {
                var stat = stats[c];
                if (!row.TryGetPropertyValue(columns[c], out var value))
                {
                    holes++;
                    stat.Constant = false;
                    continue;
                }

                stat.Present++;
                if (stat.Present == 1)
                {
                    stat.ConstantValue = value;
                    stat.ConstantKey = JsonNodes.ScalarKey(value);
                }
                else if (stat.Constant && !string.Equals(stat.ConstantKey, JsonNodes.ScalarKey(value), System.StringComparison.Ordinal))
                {
                    stat.Constant = false;
                }

                if (JsonNodes.TryGetString(value, out var text))
                {
                    stat.Strings++;
                    stat.TotalLength += text.Length;
                    stat.Distinct.Add(text);
                }
                else if (JsonNodes.KindOf(value) == System.Text.Json.JsonValueKind.Null)
                {
                    stat.Nulls++;
                }
                else
                {
                    stat.OnlyStrings = false;
                }
            }
        }

        if ((double)holes / ((double)rowCount * columnCount) > options.MaxHoleRatio)
        {
            return null;
        }

        var encodings = new int[columnCount];
        var constants = new List<JsonNode?>();
        var variableCount = 0;

        for (var c = 0; c < columnCount; c++)
        {
            var stat = stats[c];

            var isConstant =
                options.Constants
                && stat.Constant
                && stat.Present == rowCount
                && JsonNodes.IsScalar(stat.ConstantValue);

            if (isConstant)
            {
                encodings[c] = Wire.ColumnConstant;
                constants.Add(stat.ConstantValue);
                continue;
            }

            variableCount++;

            // A dictionary column pays for itself when the same handful of
            // strings keep coming back: each cell shrinks to a small integer,
            // and the strings move into the catalogue where they are written
            // exactly once.
            var worthDictionary =
                options.DictionaryColumns
                && options.Strings
                && stat.OnlyStrings
                && stat.Strings >= 3
                && stat.Distinct.Count > 0
                && stat.Distinct.Count <= stat.Strings * 0.6
                && (double)stat.TotalLength / stat.Strings >= 2d;

            encodings[c] = worthDictionary ? Wire.ColumnDictionary : Wire.ColumnRaw;
        }

        return new TablePlan
        {
            Columns = columns,
            Encodings = encodings,
            Constants = constants,
            VariableCount = variableCount,
        };
    }

    /// <summary>What one pass over the cells learned about a single column.</summary>
    private sealed class ColumnStats
    {
        public int Present { get; set; }

        public int Strings { get; set; }

        public int Nulls { get; set; }

        public bool OnlyStrings { get; set; } = true;

        public bool Constant { get; set; } = true;

        public JsonNode? ConstantValue { get; set; }

        public string ConstantKey { get; set; } = string.Empty;

        public long TotalLength { get; set; }

        public HashSet<string> Distinct { get; } = new HashSet<string>(System.StringComparer.Ordinal);
    }
}
