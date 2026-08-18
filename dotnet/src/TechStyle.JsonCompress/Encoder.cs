using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// The encoder.
/// </summary>
/// <remarks>
/// <para>
/// Two passes. The first walks the document and counts things: how often each
/// key is used, how often each string value appears, and which arrays are worth
/// turning into tables. The second builds the output using catalogues ordered
/// by those counts, so the most-used key gets the one-character token.
/// </para>
/// <para>
/// Splitting it this way costs a second walk and buys a materially smaller
/// document, because token length is decided by frequency rather than by the
/// accident of which key the encoder happened to meet first.
/// </para>
/// </remarks>
internal sealed class Encoder
{
    private readonly CompressOptions options;
    private readonly Counter keyCounts = new Counter();
    private readonly Counter stringCounts = new Counter();
    private readonly HashSet<string> forcedStrings = new HashSet<string>(StringComparer.Ordinal);

    private readonly Dictionary<JsonArray, TablePlan?> plans =
        new Dictionary<JsonArray, TablePlan?>(JsonNodes.ByReference<JsonArray>.Instance);

    private Dictionary<string, int> keyIndex = new Dictionary<string, int>(StringComparer.Ordinal);
    private Dictionary<string, string> keyTokens = new Dictionary<string, string>(StringComparer.Ordinal);
    private Dictionary<string, int> stringIndex = new Dictionary<string, int>(StringComparer.Ordinal);

    private Encoder(CompressOptions options)
    {
        this.options = options;
    }

    /// <summary>
    /// Compress a document into a json-compress envelope.
    /// </summary>
    /// <param name="source">The document to compress, which may be null.</param>
    /// <param name="options">The encoder options in force.</param>
    /// <returns>The envelope, a new tree that shares nothing with the source.</returns>
    public static JsonObject Compress(JsonNode? source, CompressOptions options)
    {
        var encoder = new Encoder(options);
        encoder.Analyse(source, 0);

        var keyList = encoder.BuildKeyCatalogue();
        var stringList = encoder.BuildStringCatalogue();

        var data = encoder.EncodeValue(source, 0);

        var envelope = new JsonObject
        {
            [Wire.EnvVersion] = JsonValue.Create(Wire.FormatVersion),
            [Wire.EnvKeys] = ToJsonArray(keyList),
        };

        if (stringList.Count > 0)
        {
            envelope[Wire.EnvStrings] = ToJsonArray(stringList);
        }

        envelope[Wire.EnvData] = data;
        return envelope;
    }

    private static JsonArray ToJsonArray(IReadOnlyList<string> values)
    {
        var array = new JsonArray();
        for (var i = 0; i < values.Count; i++)
        {
            array.Add(JsonValue.Create(values[i]));
        }

        return array;
    }

    /* ------------------------------------------------------------------ *
     * Pass one - analysis
     * ------------------------------------------------------------------ */

    private void Analyse(JsonNode? node, int depth)
    {
        Guard(depth);

        switch (node)
        {
            case null:
                return;

            case JsonArray array:
            {
                var plan = TablePlanner.Plan(array, this.options);
                this.plans[array] = plan;
                if (plan is not null)
                {
                    this.AnalyseTable(array, plan, depth);
                }
                else
                {
                    for (var i = 0; i < array.Count; i++)
                    {
                        this.Analyse(array[i], depth + 1);
                    }
                }

                return;
            }

            case JsonObject obj:
            {
                foreach (var property in obj)
                {
                    this.keyCounts.Add(property.Key);
                    this.Analyse(property.Value, depth + 1);
                }

                return;
            }

            default:
            {
                if (JsonNodes.TryGetString(node, out var text))
                {
                    this.CountString(text);
                }

                return;
            }
        }
    }

    private void AnalyseTable(JsonArray rows, TablePlan plan, int depth)
    {
        // A table spells each key out once, in the column list, however many
        // rows it has - so it contributes one to the frequency of that key, not
        // one per row.
        for (var c = 0; c < plan.Columns.Count; c++)
        {
            this.keyCounts.Add(plan.Columns[c]);
        }

        for (var i = 0; i < plan.Constants.Count; i++)
        {
            this.Analyse(plan.Constants[i], depth + 1);
        }

        for (var i = 0; i < rows.Count; i++)
        {
            var row = (JsonObject)rows[i]!;
            for (var c = 0; c < plan.Columns.Count; c++)
            {
                if (plan.Encodings[c] == Wire.ColumnConstant)
                {
                    continue;
                }

                if (!row.TryGetPropertyValue(plan.Columns[c], out var value))
                {
                    continue;
                }

                if (plan.Encodings[c] == Wire.ColumnDictionary)
                {
                    if (JsonNodes.TryGetString(value, out var text))
                    {
                        this.CountString(text);
                        this.forcedStrings.Add(text);
                    }
                }
                else
                {
                    this.Analyse(value, depth + 1);
                }
            }
        }
    }

    private void CountString(string value)
    {
        if (!this.options.Strings)
        {
            return;
        }

        this.stringCounts.Add(value);
    }

    /* ------------------------------------------------------------------ *
     * Catalogues
     * ------------------------------------------------------------------ */

    /// <summary>Most-used key first, so the busiest keys get the shortest tokens.</summary>
    private List<string> BuildKeyCatalogue()
    {
        // OrderByDescending is a stable sort, so keys used the same number of
        // times stay in the order the document first mentioned them.
        var ordered = this.keyCounts.Names
            .OrderByDescending(this.keyCounts.CountOf)
            .ToList();

        this.keyIndex = new Dictionary<string, int>(ordered.Count, StringComparer.Ordinal);
        this.keyTokens = new Dictionary<string, string>(ordered.Count, StringComparer.Ordinal);
        for (var i = 0; i < ordered.Count; i++)
        {
            this.keyIndex[ordered[i]] = i;
            this.keyTokens[ordered[i]] = Tokens.ToKeyToken(i);
        }

        return ordered;
    }

    /// <summary>
    /// Choose which string values are worth moving into the catalogue.
    /// </summary>
    /// <returns>The catalogue, in the order it will be written.</returns>
    /// <remarks>
    /// <para>
    /// A catalogued string is written once in the catalogue and referenced by a
    /// short tilde token everywhere it occurs, so the trade is:
    /// </para>
    /// <code>
    /// inline    = count * (length + 2)                   // quotes on every copy
    /// catalogue = count * (tokenLength + 3) + length + 3
    /// </code>
    /// <para>
    /// Candidates are considered in frequency order, so the token length used
    /// in that comparison is the one the string would really be given.
    /// </para>
    /// <para>
    /// Values pulled into a dictionary column are forced in regardless: their
    /// cells are bare integers, which is cheaper still, and the comparison
    /// above does not describe them.
    /// </para>
    /// </remarks>
    private List<string> BuildStringCatalogue()
    {
        var list = new List<string>();
        this.stringIndex = new Dictionary<string, int>(StringComparer.Ordinal);
        if (!this.options.Strings)
        {
            return list;
        }

        var candidates = new List<Candidate>();
        foreach (var value in this.stringCounts.Names)
        {
            var forced = this.forcedStrings.Contains(value);
            if (!forced)
            {
                if (this.stringCounts.CountOf(value) < this.options.MinStringCount)
                {
                    continue;
                }

                if (value.Length < this.options.MinStringLength)
                {
                    continue;
                }

                // A value that already begins with a tilde is escaped inline and
                // can never be written as a reference, so cataloguing it would
                // only burn an index.
                if (value.Length > 0 && value[0] == Wire.Tilde)
                {
                    continue;
                }
            }

            candidates.Add(new Candidate(value, this.stringCounts.CountOf(value), forced));
        }

        var ordered = candidates
            .OrderByDescending(candidate => candidate.Count)
            .ThenByDescending(candidate => candidate.Value.Length);

        foreach (var candidate in ordered)
        {
            if (!candidate.Forced)
            {
                var tokenLength = Tokens.Base62Length(list.Count);
                var inline = (long)candidate.Count * (candidate.Value.Length + 2);
                var catalogued = ((long)candidate.Count * (tokenLength + 3)) + candidate.Value.Length + 3;
                if (catalogued >= inline)
                {
                    continue;
                }
            }

            this.stringIndex[candidate.Value] = list.Count;
            list.Add(candidate.Value);
        }

        return list;
    }

    /* ------------------------------------------------------------------ *
     * Pass two - build
     * ------------------------------------------------------------------ */

    private JsonNode? EncodeValue(JsonNode? node, int depth)
    {
        Guard(depth);

        switch (node)
        {
            case null:
                return null;

            case JsonArray array:
            {
                if (this.plans.TryGetValue(array, out var plan) && plan is not null)
                {
                    return this.EncodeTable(array, plan, depth);
                }

                var encoded = new JsonArray();
                for (var i = 0; i < array.Count; i++)
                {
                    encoded.Add(this.EncodeValue(array[i], depth + 1));
                }

                return encoded;
            }

            case JsonObject obj:
            {
                var encoded = new JsonObject();
                foreach (var property in obj)
                {
                    encoded[this.keyTokens[property.Key]] = this.EncodeValue(property.Value, depth + 1);
                }

                return encoded;
            }

            default:
            {
                if (JsonNodes.TryGetString(node, out var text))
                {
                    return JsonValue.Create(this.EncodeString(text));
                }

                return JsonNodes.CopyScalar(node);
            }
        }
    }

    /// <summary>
    /// Put a string value into the form the format defines for it.
    /// </summary>
    /// <param name="value">The source string.</param>
    /// <returns>The encoded string.</returns>
    /// <remarks>
    /// Escape first: an escaped literal always starts with two tildes, a
    /// reference is a tilde followed by a base-62 digit, and a lone tilde is
    /// the row hole marker. Those three spaces cannot overlap.
    /// </remarks>
    private string EncodeString(string value)
    {
        if (value.Length > 0 && value[0] == Wire.Tilde)
        {
            return "~" + value;
        }

        return this.stringIndex.TryGetValue(value, out var at) ? "~" + Tokens.ToBase62(at) : value;
    }

    private JsonObject EncodeTable(JsonArray rows, TablePlan plan, int depth)
    {
        var node = new JsonObject();

        var columnIndices = new JsonArray();
        for (var c = 0; c < plan.Columns.Count; c++)
        {
            columnIndices.Add(JsonValue.Create(this.keyIndex[plan.Columns[c]]));
        }

        node[Wire.TableColumns] = columnIndices;

        var uniform = true;
        for (var c = 0; c < plan.Encodings.Length; c++)
        {
            if (plan.Encodings[c] != Wire.ColumnRaw)
            {
                uniform = false;
                break;
            }
        }

        if (!uniform)
        {
            var encodings = new JsonArray();
            for (var c = 0; c < plan.Encodings.Length; c++)
            {
                encodings.Add(JsonValue.Create(plan.Encodings[c]));
            }

            node[Wire.TableEncodings] = encodings;
        }

        if (plan.Constants.Count > 0)
        {
            var constants = new JsonArray();
            for (var i = 0; i < plan.Constants.Count; i++)
            {
                constants.Add(this.EncodeValue(plan.Constants[i], depth + 1));
            }

            node[Wire.TableConstants] = constants;
        }

        if (plan.VariableCount == 0)
        {
            // Every column turned out to be constant, so the rows carry no data
            // at all and only their number survives.
            node[Wire.TableRowCount] = JsonValue.Create(rows.Count);
            return node;
        }

        var matrix = new JsonArray();
        var cells = new JsonNode?[plan.VariableCount];
        var isHole = new bool[plan.VariableCount];

        for (var i = 0; i < rows.Count; i++)
        {
            var row = (JsonObject)rows[i]!;
            var at = 0;

            for (var c = 0; c < plan.Columns.Count; c++)
            {
                var encoding = plan.Encodings[c];
                if (encoding == Wire.ColumnConstant)
                {
                    continue;
                }

                var present = row.TryGetPropertyValue(plan.Columns[c], out var value);

                if (encoding == Wire.ColumnDictionary)
                {
                    if (!present)
                    {
                        cells[at] = JsonValue.Create(Wire.DictionaryHole);
                        isHole[at] = true;
                    }
                    else if (JsonNodes.KindOf(value) == JsonValueKind.Null)
                    {
                        cells[at] = null;
                        isHole[at] = false;
                    }
                    else
                    {
                        JsonNodes.TryGetString(value, out var text);
                        cells[at] = JsonValue.Create(this.stringIndex[text]);
                        isHole[at] = false;
                    }
                }
                else if (!present)
                {
                    cells[at] = JsonValue.Create(Wire.Hole);
                    isHole[at] = true;
                }
                else
                {
                    cells[at] = this.EncodeValue(value, depth + 1);
                    isHole[at] = false;
                }

                at++;
            }

            // Trailing gaps need not be written down; a short row is understood
            // to be missing everything past its end.
            var end = plan.VariableCount;
            while (end > 0 && isHole[end - 1])
            {
                end--;
            }

            var cellArray = new JsonArray();
            for (var j = 0; j < end; j++)
            {
                cellArray.Add(cells[j]);
            }

            matrix.Add(cellArray);
        }

        node[Wire.TableRows] = matrix;
        return node;
    }

    private static void Guard(int depth)
    {
        if (depth > Wire.MaxDepth)
        {
            throw new JsonCompressException(
                "the document is nested more than " + JsonNodes.Describe(Wire.MaxDepth) + " levels deep");
        }
    }

    /// <summary>
    /// How often each name was seen, and the order the document first mentioned
    /// them in - which is the tie-break when two names are used equally often.
    /// </summary>
    private sealed class Counter
    {
        private readonly Dictionary<string, int> counts = new Dictionary<string, int>(StringComparer.Ordinal);
        private readonly List<string> names = new List<string>();

        public IReadOnlyList<string> Names => this.names;

        public void Add(string name)
        {
            if (this.counts.TryGetValue(name, out var count))
            {
                this.counts[name] = count + 1;
                return;
            }

            this.counts.Add(name, 1);
            this.names.Add(name);
        }

        public int CountOf(string name)
        {
            return this.counts.TryGetValue(name, out var count) ? count : 0;
        }
    }

    private readonly struct Candidate
    {
        public Candidate(string value, int count, bool forced)
        {
            this.Value = value;
            this.Count = count;
            this.Forced = forced;
        }

        public string Value { get; }

        public int Count { get; }

        public bool Forced { get; }
    }
}
