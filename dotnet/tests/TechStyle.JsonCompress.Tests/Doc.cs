using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// The shorthand the tests are written in.
/// </summary>
/// <remarks>
/// Every assertion about a document compares serialised text rather than object
/// graphs. That is deliberate: the contract is not only that the same values
/// come back, but that they come back in the same order, and text is the only
/// comparison that sees the difference.
/// </remarks>
internal static class Doc
{
    /// <summary>Parse JSON text into a node.</summary>
    /// <param name="json">The text.</param>
    /// <returns>The parsed document.</returns>
    public static JsonNode? Parse(string json)
    {
        return JsonCompressor.ParseJson(json);
    }

    /// <summary>Serialise a node the way this library writes JSON.</summary>
    /// <param name="node">The document.</param>
    /// <returns>Its minified text.</returns>
    public static string Text(JsonNode? node)
    {
        return node is null ? "null" : node.ToJsonString(JsonCompressor.SerializerOptions);
    }

    /// <summary>Compress and restore a document, and insist nothing changed.</summary>
    /// <param name="json">The document, as JSON text.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <returns>The restored document, for further assertions.</returns>
    public static JsonNode? RoundTrips(string json, CompressOptions? options = null)
    {
        var source = Parse(json);
        var restored = JsonCompressor.Decompress(JsonCompressor.Compress(source, options));
        Assert.Equal(Text(source), Text(restored));
        return restored;
    }

    /// <summary>Compress and restore a document that is already a node.</summary>
    /// <param name="value">The document.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <returns>The restored document, for further assertions.</returns>
    public static JsonNode? RoundTrips(JsonNode? value, CompressOptions? options = null)
    {
        var restored = JsonCompressor.Decompress(JsonCompressor.Compress(value, options));
        Assert.Equal(Text(value), Text(restored));
        return restored;
    }

    /// <summary>The compressed form of a document, as text.</summary>
    /// <param name="json">The document, as JSON text.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <returns>The envelope text.</returns>
    public static string Compressed(string json, CompressOptions? options = null)
    {
        return JsonCompressor.Stringify(Parse(json), options);
    }
}
