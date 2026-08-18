using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// The decoder.
/// </summary>
/// <remarks>
/// A straight structural walk of the encoded document, resolving catalogue
/// references as it goes. It is deliberately strict: anything that cannot be
/// mapped back onto the original - an out-of-range token, a stray hole marker,
/// a table whose rows do not line up with its columns - throws rather than
/// quietly handing back something that is nearly right.
/// </remarks>
internal sealed class Decoder
{
    private readonly IReadOnlyList<string> keys;
    private readonly IReadOnlyList<string> strings;

    private Decoder(IReadOnlyList<string> keys, IReadOnlyList<string> strings)
    {
        this.keys = keys;
        this.strings = strings;
    }

    /// <summary>
    /// Restore the original document from a json-compress envelope.
    /// </summary>
    /// <param name="envelope">The envelope to read.</param>
    /// <returns>The original document, a new tree that shares nothing with the envelope.</returns>
    /// <exception cref="JsonCompressException">The envelope cannot be read.</exception>
    public static JsonNode? Decompress(JsonNode? envelope)
    {
        if (envelope is not JsonObject root)
        {
            throw new JsonCompressException("expected a compressed envelope object");
        }

        if (!root.TryGetPropertyValue(Wire.EnvVersion, out var versionNode))
        {
            throw new JsonCompressException("this is not a compressed document (no \"jc\" version field)");
        }

        if (!JsonNodes.TryGetInt32(versionNode, out var version) || version != Wire.FormatVersion)
        {
            throw new JsonCompressException(
                "format version " + Describe(versionNode) + " is not supported by this build (expected "
                + JsonNodes.Describe(Wire.FormatVersion) + ")");
        }

        if (!root.TryGetPropertyValue(Wire.EnvKeys, out var keyNode) || keyNode is not JsonArray keyArray)
        {
            throw new JsonCompressException("the key catalogue is missing or is not an array");
        }

        var keys = ReadCatalogue(keyArray, "key catalogue");

        var strings = Array.Empty<string>() as IReadOnlyList<string>;
        if (root.TryGetPropertyValue(Wire.EnvStrings, out var stringNode) && stringNode is not null)
        {
            if (stringNode is not JsonArray stringArray)
            {
                throw new JsonCompressException("the string catalogue is not an array");
            }

            strings = ReadCatalogue(stringArray, "string catalogue");
        }

        root.TryGetPropertyValue(Wire.EnvData, out var data);
        return new Decoder(keys, strings).DecodeValue(data, 0);
    }

    /// <summary>
    /// True when a value looks like a json-compress envelope this build can read.
    /// </summary>
    /// <param name="value">The value to inspect.</param>
    /// <returns>Whether it is worth handing to the decoder.</returns>
    public static bool IsCompressed(JsonNode? value)
    {
        return value is JsonObject root
            && root.TryGetPropertyValue(Wire.EnvVersion, out var version)
            && JsonNodes.TryGetInt32(version, out var number)
            && number == Wire.FormatVersion
            && root.TryGetPropertyValue(Wire.EnvKeys, out var keys)
            && keys is JsonArray;
    }

    private static IReadOnlyList<string> ReadCatalogue(JsonArray array, string what)
    {
        var entries = new string[array.Count];
        for (var i = 0; i < array.Count; i++)
        {
            if (!JsonNodes.TryGetString(array[i], out var text))
            {
                throw new JsonCompressException(
                    "the " + what + " holds something that is not a string, at index " + JsonNodes.Describe(i));
            }

            entries[i] = text;
        }

        return entries;
    }

    private static string Describe(JsonNode? node)
    {
        try
        {
            return node is null ? "null" : node.ToJsonString();
        }
        catch (Exception error) when (error is JsonException || error is NotSupportedException)
        {
            return "unreadable";
        }
    }

    private JsonNode? DecodeValue(JsonNode? node, int depth)
    {
        if (depth > Wire.MaxDepth)
        {
            throw new JsonCompressException(
                "the document is nested more than " + JsonNodes.Describe(Wire.MaxDepth) + " levels deep");
        }

        switch (node)
        {
            case null:
                return null;

            case JsonArray array:
            {
                var decoded = new JsonArray();
                for (var i = 0; i < array.Count; i++)
                {
                    decoded.Add(this.DecodeValue(array[i], depth + 1));
                }

                return decoded;
            }

            case JsonObject obj:
            {
                if (obj.ContainsKey(Wire.TableColumns))
                {
                    return this.DecodeTable(obj, depth);
                }

                var decoded = new JsonObject();
                foreach (var property in obj)
                {
                    decoded[this.DecodeKey(property.Key)] = this.DecodeValue(property.Value, depth + 1);
                }

                return decoded;
            }

            default:
            {
                if (JsonNodes.TryGetString(node, out var text))
                {
                    return JsonValue.Create(this.DecodeString(text));
                }

                return JsonNodes.CopyScalar(node);
            }
        }
    }

    private string DecodeString(string value)
    {
        if (value.Length == 0 || value[0] != Wire.Tilde)
        {
            return value;
        }

        if (value.Length == 1)
        {
            throw new JsonCompressException("a row hole marker appeared outside a table row");
        }

        if (value[1] == Wire.Tilde)
        {
            return value.Substring(1);
        }

        var at = Tokens.FromBase62(value.Substring(1));
        if (at < 0 || at >= this.strings.Count)
        {
            throw new JsonCompressException("string reference \"" + value + "\" is out of range");
        }

        return this.strings[at];
    }

    private string DecodeKey(string token)
    {
        var at = Tokens.FromKeyToken(token);
        if (at < 0 || at >= this.keys.Count)
        {
            throw new JsonCompressException("key token \"" + token + "\" is out of range");
        }

        return this.keys[at];
    }

    private JsonArray DecodeTable(JsonObject node, int depth)
    {
        if (node[Wire.TableColumns] is not JsonArray columnIndices)
        {
            throw new JsonCompressException("table column list is not an array");
        }

        var columnCount = columnIndices.Count;
        var columns = new string[columnCount];
        for (var c = 0; c < columnCount; c++)
        {
            if (!JsonNodes.TryGetInt32(columnIndices[c], out var at) || at < 0 || at >= this.keys.Count)
            {
                throw new JsonCompressException(
                    "table column index " + Describe(columnIndices[c]) + " is out of range");
            }

            columns[c] = this.keys[at];
        }

        int[]? encodings = null;
        if (node.TryGetPropertyValue(Wire.TableEncodings, out var encodingNode) && encodingNode is not null)
        {
            if (encodingNode is not JsonArray encodingArray || encodingArray.Count != columnCount)
            {
                throw new JsonCompressException("table encoding list does not match its columns");
            }

            encodings = new int[columnCount];
            for (var c = 0; c < columnCount; c++)
            {
                if (!JsonNodes.TryGetInt32(encodingArray[c], out var encoding))
                {
                    throw new JsonCompressException(
                        "table column encoding " + Describe(encodingArray[c]) + " is not a number");
                }

                encodings[c] = encoding;
            }
        }

        var constants = new List<JsonNode?>();
        if (node.TryGetPropertyValue(Wire.TableConstants, out var constantNode) && constantNode is not null)
        {
            if (constantNode is not JsonArray constantArray)
            {
                throw new JsonCompressException("table constant list is not an array");
            }

            for (var i = 0; i < constantArray.Count; i++)
            {
                constants.Add(this.DecodeValue(constantArray[i], depth + 1));
            }
        }

        JsonArray? matrix = null;
        if (node.TryGetPropertyValue(Wire.TableRows, out var rowNode) && rowNode is not null)
        {
            matrix = rowNode as JsonArray;
            if (matrix is null)
            {
                throw new JsonCompressException("table rows are not an array");
            }
        }

        int rowCount;
        if (matrix is not null)
        {
            rowCount = matrix.Count;
        }
        else
        {
            node.TryGetPropertyValue(Wire.TableRowCount, out var countNode);
            if (countNode is null)
            {
                rowCount = 0;
            }
            else if (!JsonNodes.TryGetInt32(countNode, out rowCount) || rowCount < 0)
            {
                throw new JsonCompressException("table row count " + Describe(countNode) + " is not a row count");
            }
        }

        var restored = new JsonArray();
        for (var i = 0; i < rowCount; i++)
        {
            JsonArray? cells = null;
            if (matrix is not null)
            {
                var cellNode = matrix[i];
                if (cellNode is not null)
                {
                    cells = cellNode as JsonArray;
                    if (cells is null)
                    {
                        throw new JsonCompressException("table row " + JsonNodes.Describe(i) + " is not an array");
                    }
                }
            }

            var row = new JsonObject();
            var constantAt = 0;
            var cellAt = 0;

            for (var c = 0; c < columnCount; c++)
            {
                var encoding = encodings is null ? Wire.ColumnRaw : encodings[c];

                if (encoding == Wire.ColumnConstant)
                {
                    if (constantAt >= constants.Count)
                    {
                        throw new JsonCompressException("table has fewer constants than constant columns");
                    }

                    // Every row gets its own copy: a node belongs to one parent,
                    // and a shared value would be a shared mutation besides.
                    row[columns[c]] = CopyForRow(constants[constantAt++]);
                    continue;
                }

                // A row shorter than the column list is missing everything past
                // its end.
                var present = cells is not null && cellAt < cells.Count;
                var cell = present ? cells![cellAt] : null;
                cellAt++;

                if (encoding == Wire.ColumnDictionary)
                {
                    if (!present || IsDictionaryHole(cell))
                    {
                        continue;
                    }

                    if (JsonNodes.KindOf(cell) == JsonValueKind.Null)
                    {
                        row[columns[c]] = null;
                        continue;
                    }

                    if (!JsonNodes.TryGetInt32(cell, out var at) || at < 0 || at >= this.strings.Count)
                    {
                        throw new JsonCompressException("dictionary index " + Describe(cell) + " is out of range");
                    }

                    row[columns[c]] = JsonValue.Create(this.strings[at]);
                    continue;
                }

                if (!present || IsHole(cell))
                {
                    continue;
                }

                row[columns[c]] = this.DecodeValue(cell, depth + 1);
            }

            restored.Add(row);
        }

        return restored;
    }

    private static bool IsHole(JsonNode? cell)
    {
        return JsonNodes.TryGetString(cell, out var text) && string.Equals(text, Wire.Hole, StringComparison.Ordinal);
    }

    private static bool IsDictionaryHole(JsonNode? cell)
    {
        return JsonNodes.TryGetInt32(cell, out var number) && number == Wire.DictionaryHole;
    }

    private static JsonNode? CopyForRow(JsonNode? value)
    {
        if (value is null)
        {
            return null;
        }

        return JsonNodes.IsScalar(value) ? JsonNodes.CopyScalar(value) : value.DeepClone();
    }
}
