using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// The handful of things the encoder and decoder both need to ask of a
/// <see cref="JsonNode"/>, in one place.
/// </summary>
/// <remarks>
/// <see cref="JsonNode"/> is a tree of values that each know their parent, so a
/// node can only ever be attached in one place. Everything here that moves a
/// value from one document into another therefore copies it, and never hands
/// back a node that is still owned by another tree.
/// </remarks>
internal static class JsonNodes
{
    /// <summary>The value kind of a node, treating a missing node as null.</summary>
    /// <param name="node">The node to inspect, which may be null.</param>
    /// <returns>Its value kind.</returns>
    public static JsonValueKind KindOf(JsonNode? node)
    {
        return node is null ? JsonValueKind.Null : node.GetValueKind();
    }

    /// <summary>True for a value that can safely be hoisted out of the rows and shared.</summary>
    /// <param name="node">The candidate value.</param>
    /// <returns>Whether it is a scalar.</returns>
    public static bool IsScalar(JsonNode? node)
    {
        switch (KindOf(node))
        {
            case JsonValueKind.Object:
            case JsonValueKind.Array:
                return false;
            default:
                return true;
        }
    }

    /// <summary>Read a node as a string, when it is one.</summary>
    /// <param name="node">The node to read.</param>
    /// <param name="text">The string it holds.</param>
    /// <returns>Whether the node was a JSON string.</returns>
    public static bool TryGetString(JsonNode? node, out string text)
    {
        if (node is JsonValue value && value.GetValueKind() == JsonValueKind.String
            && value.TryGetValue<string>(out var read) && read is not null)
        {
            text = read;
            return true;
        }

        text = string.Empty;
        return false;
    }

    /// <summary>
    /// Read a node as a 32-bit integer, when it is one exactly.
    /// </summary>
    /// <param name="node">The node to read.</param>
    /// <param name="number">The integer it holds.</param>
    /// <returns>Whether the node was a JSON number with no fractional part.</returns>
    /// <remarks>
    /// A node parsed from text is backed by a <see cref="JsonElement"/> and
    /// answers this directly. One built in memory may be backed by any CLR
    /// numeric type, so the fallbacks below cover the cases a hand-assembled
    /// envelope can present. A fractional value is not an integer and is
    /// rejected rather than rounded.
    /// </remarks>
    public static bool TryGetInt32(JsonNode? node, out int number)
    {
        number = 0;
        if (node is not JsonValue value || value.GetValueKind() != JsonValueKind.Number)
        {
            return false;
        }

        if (value.TryGetValue<int>(out var asInt))
        {
            number = asInt;
            return true;
        }

        if (value.TryGetValue<long>(out var asLong))
        {
            if (asLong < int.MinValue || asLong > int.MaxValue)
            {
                return false;
            }

            number = (int)asLong;
            return true;
        }

        if (value.TryGetValue<double>(out var asDouble))
        {
            if (double.IsNaN(asDouble) || double.IsInfinity(asDouble))
            {
                return false;
            }

            if (asDouble != Math.Floor(asDouble) || asDouble < int.MinValue || asDouble > int.MaxValue)
            {
                return false;
            }

            number = (int)asDouble;
            return true;
        }

        if (value.TryGetValue<decimal>(out var asDecimal))
        {
            if (asDecimal != decimal.Truncate(asDecimal) || asDecimal < int.MinValue || asDecimal > int.MaxValue)
            {
                return false;
            }

            number = (int)asDecimal;
            return true;
        }

        return false;
    }

    /// <summary>
    /// Copy a scalar out of one document so it can be attached to another,
    /// applying the normalisation the format requires.
    /// </summary>
    /// <param name="node">The scalar to copy, which may be null.</param>
    /// <returns>A detached copy, or null.</returns>
    /// <remarks>
    /// JSON cannot carry a non-finite number, so those become null - the same
    /// substitution every JSON writer makes. Negative zero is folded to zero so
    /// that the in-memory round trip and the serialised one agree, since JSON
    /// itself cannot tell the two apart.
    /// </remarks>
    public static JsonNode? CopyScalar(JsonNode? node)
    {
        if (node is null)
        {
            return null;
        }

        if (node is JsonValue value && value.GetValueKind() == JsonValueKind.Number
            && value.TryGetValue<double>(out var number))
        {
            if (double.IsNaN(number) || double.IsInfinity(number))
            {
                return null;
            }

            if (number == 0d && IsNegativeZero(number))
            {
                return JsonValue.Create(0d);
            }
        }

        return node.DeepClone();
    }

    private static bool IsNegativeZero(double value)
    {
        // double.IsNegative is missing from some of the frameworks targeted
        // here, and the sign bit is the only thing separating the two zeroes.
        return BitConverter.DoubleToInt64Bits(value) < 0;
    }

    /// <summary>
    /// A text form of a scalar that two values share exactly when they are the
    /// same JSON value.
    /// </summary>
    /// <param name="node">The scalar to describe.</param>
    /// <returns>Its canonical text.</returns>
    /// <remarks>
    /// Used to decide whether a column never changes. Numbers are compared by
    /// the text they will be written as rather than by their numeric value, so
    /// a column holding both <c>1</c> and <c>1.0</c> is not treated as constant
    /// and each row keeps the form it arrived in.
    /// </remarks>
    public static string ScalarKey(JsonNode? node)
    {
        switch (KindOf(node))
        {
            case JsonValueKind.Null:
            case JsonValueKind.Undefined:
                return "null";
            case JsonValueKind.True:
                return "true";
            case JsonValueKind.False:
                return "false";
            case JsonValueKind.String:
                return TryGetString(node, out var text) ? "\"" + text : "\"";
            default:
                break;
        }

        if (node is JsonValue value && value.GetValueKind() == JsonValueKind.Number
            && value.TryGetValue<double>(out var number)
            && (double.IsNaN(number) || double.IsInfinity(number)))
        {
            return "null";
        }

        return node!.ToJsonString();
    }

    /// <summary>Format a number for a message, without a culture surprise.</summary>
    /// <param name="value">The number to format.</param>
    /// <returns>Its invariant text.</returns>
    public static string Describe(int value)
    {
        return value.ToString(CultureInfo.InvariantCulture);
    }

    /// <summary>
    /// Compares nodes by identity, so an analysis result can be filed against
    /// the very array it was worked out for.
    /// </summary>
    /// <typeparam name="T">The kind of node being keyed.</typeparam>
    internal sealed class ByReference<T> : IEqualityComparer<T>
        where T : class
    {
        /// <summary>The shared instance.</summary>
        public static readonly ByReference<T> Instance = new ByReference<T>();

        private ByReference()
        {
        }

        /// <inheritdoc />
        public bool Equals(T? left, T? right)
        {
            return ReferenceEquals(left, right);
        }

        /// <inheritdoc />
        public int GetHashCode(T value)
        {
            return System.Runtime.CompilerServices.RuntimeHelpers.GetHashCode(value);
        }
    }
}
