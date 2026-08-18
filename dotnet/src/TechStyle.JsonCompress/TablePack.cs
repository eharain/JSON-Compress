using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// The plain table pack.
/// </summary>
/// <remarks>
/// <para>
/// This is the smallest useful idea in the library, offered on its own because
/// it is often all you need and because the shape it produces - <c>fields</c>,
/// <c>data</c> and <c>count</c> - is trivial to consume from another language
/// or straight out of a database driver.
/// </para>
/// <para>
/// <see cref="Pack"/> turns an array of objects into one field list and a
/// matrix of rows. <see cref="Unpack"/> turns it back. Neither uses a
/// catalogue, an envelope or a version tag; what you get is a readable object
/// you can eyeball in a network panel.
/// </para>
/// <para>
/// Everything here returns a new tree. A <see cref="JsonNode"/> belongs to one
/// parent, so handing back a node that is still owned by the input would leave
/// the caller with something they cannot attach anywhere.
/// </para>
/// </remarks>
public static class TablePack
{
    /// <summary>Field name for the column list.</summary>
    private const string Fields = "fields";

    /// <summary>Field name for the rows matrix.</summary>
    private const string Data = "data";

    /// <summary>Field name for the record count.</summary>
    private const string Count = "count";

    /// <summary>
    /// Pack an array of records into a field list and a rows matrix.
    /// </summary>
    /// <param name="records">The records to pack.</param>
    /// <returns>The packed table, or a copy of the input when it is not an array of objects.</returns>
    /// <remarks>
    /// Fields are taken from the union of every record, in first-seen order, so
    /// a record that is missing one simply contributes null in that column.
    /// Anything that is not an array of objects is handed straight back, which
    /// lets you drop this into a response pipeline without first checking the
    /// shape.
    /// </remarks>
    public static JsonNode? Pack(JsonNode? records)
    {
        if (records is not JsonArray array)
        {
            return records?.DeepClone();
        }

        if (array.Count == 0)
        {
            return new JsonObject
            {
                [Fields] = new JsonArray(),
                [Data] = new JsonArray(),
                [Count] = JsonValue.Create(0),
            };
        }

        var rows = new JsonObject[array.Count];
        for (var i = 0; i < array.Count; i++)
        {
            if (array[i] is not JsonObject row)
            {
                return array.DeepClone();
            }

            rows[i] = row;
        }

        var seen = new HashSet<string>(StringComparer.Ordinal);
        var fields = new List<string>();
        for (var i = 0; i < rows.Length; i++)
        {
            foreach (var property in rows[i])
            {
                if (seen.Add(property.Key))
                {
                    fields.Add(property.Key);
                }
            }
        }

        var fieldArray = new JsonArray();
        for (var f = 0; f < fields.Count; f++)
        {
            fieldArray.Add(JsonValue.Create(fields[f]));
        }

        var matrix = new JsonArray();
        for (var i = 0; i < rows.Length; i++)
        {
            var cells = new JsonArray();
            for (var f = 0; f < fields.Count; f++)
            {
                rows[i].TryGetPropertyValue(fields[f], out var value);
                cells.Add(value?.DeepClone());
            }

            matrix.Add(cells);
        }

        return new JsonObject
        {
            [Fields] = fieldArray,
            [Data] = matrix,
            [Count] = JsonValue.Create(matrix.Count),
        };
    }

    /// <summary>
    /// Rebuild the array of records produced by <see cref="Pack"/>.
    /// </summary>
    /// <param name="packed">The packed table.</param>
    /// <returns>The records, or a copy of the input when it is not a packed table.</returns>
    /// <remarks>
    /// Anything that is not a packed table is handed straight back, so a
    /// consumer can call this unconditionally on a payload that may or may not
    /// have been packed at the other end.
    /// </remarks>
    public static JsonNode? Unpack(JsonNode? packed)
    {
        if (!IsPacked(packed))
        {
            return packed?.DeepClone();
        }

        var table = (JsonObject)packed!;
        var fields = (JsonArray)table[Fields]!;
        var matrix = (JsonArray)table[Data]!;

        var names = new string[fields.Count];
        for (var f = 0; f < fields.Count; f++)
        {
            names[f] = JsonNodes.TryGetString(fields[f], out var text) ? text : string.Empty;
        }

        var records = new JsonArray();
        for (var i = 0; i < matrix.Count; i++)
        {
            var cells = matrix[i] as JsonArray;
            var record = new JsonObject();
            for (var f = 0; f < names.Length; f++)
            {
                var value = cells is not null && f < cells.Count ? cells[f] : null;
                record[names[f]] = value?.DeepClone();
            }

            records.Add(record);
        }

        return records;
    }

    /// <summary>True when a value has the shape <see cref="Unpack"/> knows how to read.</summary>
    /// <param name="value">The value to inspect.</param>
    /// <returns>Whether it is a packed table.</returns>
    public static bool IsPacked(JsonNode? value)
    {
        return value is JsonObject table
            && table.TryGetPropertyValue(Fields, out var fields) && fields is JsonArray
            && table.TryGetPropertyValue(Data, out var data) && data is JsonArray;
    }

    /// <summary>
    /// Pack every result set in a list of result sets.
    /// </summary>
    /// <param name="resultSets">The list of result sets.</param>
    /// <returns>The same outer list, with each inner array packed.</returns>
    /// <remarks>
    /// Database drivers that return several result sets from one call hand back
    /// an array of arrays; this packs each inner array and leaves the outer one
    /// alone.
    /// </remarks>
    public static JsonNode? PackAll(JsonNode? resultSets)
    {
        if (resultSets is not JsonArray sets)
        {
            return resultSets?.DeepClone();
        }

        var packed = new JsonArray();
        for (var i = 0; i < sets.Count; i++)
        {
            packed.Add(Pack(sets[i]));
        }

        return packed;
    }

    /// <summary>Unpack every result set packed by <see cref="PackAll"/>.</summary>
    /// <param name="resultSets">The list of packed result sets.</param>
    /// <returns>The same outer list, with each inner table unpacked.</returns>
    public static JsonNode? UnpackAll(JsonNode? resultSets)
    {
        if (resultSets is not JsonArray sets)
        {
            return resultSets?.DeepClone();
        }

        var unpacked = new JsonArray();
        for (var i = 0; i < sets.Count; i++)
        {
            unpacked.Add(Unpack(sets[i]));
        }

        return unpacked;
    }

    /// <summary>
    /// Read a delimited-integer column back into an array of numbers.
    /// </summary>
    /// <param name="record">The record holding the column.</param>
    /// <param name="property">Which property to convert.</param>
    /// <param name="open">How many characters to trim from the front.</param>
    /// <param name="close">How many characters to trim from the end.</param>
    /// <param name="separator">What separates one value from the next.</param>
    /// <returns>The same record, for chaining.</returns>
    /// <remarks>
    /// Some databases return a set-valued column as a bracketed,
    /// comma-separated string. This converts one such property in place,
    /// leaving an empty array where the column was null and a null entry where
    /// a part is not a number.
    /// </remarks>
    /// <exception cref="ArgumentNullException">The record or the property name is null.</exception>
    public static JsonObject TextToArray(
        JsonObject record,
        string property,
        int open = 3,
        int close = 3,
        string separator = ",")
    {
        if (record is null)
        {
            throw new ArgumentNullException(nameof(record));
        }

        if (property is null)
        {
            throw new ArgumentNullException(nameof(property));
        }

        if (!record.TryGetPropertyValue(property, out var value))
        {
            return record;
        }

        var numbers = new JsonArray();

        if (JsonNodes.TryGetString(value, out var text) && text.Length > open + close)
        {
            var inner = text.Substring(open, text.Length - close - open);
            if (inner.Length > 0)
            {
                var parts = inner.Split(new[] { separator }, StringSplitOptions.None);
                for (var i = 0; i < parts.Length; i++)
                {
                    numbers.Add(
                        long.TryParse(parts[i].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out var number)
                            ? JsonValue.Create(number)
                            : null);
                }
            }
        }

        record[property] = numbers;
        return record;
    }
}
