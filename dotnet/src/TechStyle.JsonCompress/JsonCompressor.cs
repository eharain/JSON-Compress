using System;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// Lossless JSON compression that is still JSON.
/// </summary>
/// <remarks>
/// <para>
/// Three ideas, applied together:
/// </para>
/// <list type="number">
/// <item>
/// Every object key is replaced by a short token and the real names are listed
/// once, in a catalogue. A key that occurs ten thousand times is spelled out
/// once.
/// </item>
/// <item>
/// An array of like-shaped objects becomes a table - one column list and a
/// matrix of rows - so the keys disappear from the rows entirely. Columns whose
/// value never changes are hoisted out of the rows altogether, and columns
/// drawn from a small set of strings become integer indices.
/// </item>
/// <item>
/// String values that repeat are moved into a second catalogue and referenced
/// by token.
/// </item>
/// </list>
/// <para>
/// The output is an ordinary JSON value with no binary framing, so it survives
/// anything that carries JSON: an HTTP body, a WebSocket frame, a jsonb column,
/// a queue message, a log line.
/// </para>
/// <para>
/// The methods that take and return a <see cref="JsonNode"/> are the format
/// itself and use no reflection. The ones with a type parameter are a
/// convenience over <see cref="JsonSerializer"/> for callers whose data starts
/// out as ordinary objects.
/// </para>
/// </remarks>
public static class JsonCompressor
{
    /// <summary>The wire-format version this build reads and writes.</summary>
    public const int FormatVersion = Wire.FormatVersion;

    /// <summary>
    /// How this library writes JSON.
    /// </summary>
    /// <remarks>
    /// Non-ASCII characters are written as themselves rather than as escape
    /// sequences, which is both smaller and what every other implementation of
    /// this format does. That means the output is JSON for a JSON reader, not
    /// text to be dropped straight into an HTML page - escape it at the point
    /// of embedding, as you would any other JSON.
    /// </remarks>
    public static JsonSerializerOptions SerializerOptions { get; } = CreateSerializerOptions(indented: false);

    /// <summary>The same options, indented for reading.</summary>
    public static JsonSerializerOptions IndentedSerializerOptions { get; } = CreateSerializerOptions(indented: true);

    /// <summary>
    /// How this library parses JSON: deeper than the default, because a
    /// document that nests past 64 levels is unusual but not wrong.
    /// </summary>
    public static JsonDocumentOptions DocumentOptions { get; } = new JsonDocumentOptions
    {
        MaxDepth = Wire.MaxDepth,
        CommentHandling = JsonCommentHandling.Disallow,
        AllowTrailingCommas = false,
    };

    private static JsonSerializerOptions CreateSerializerOptions(bool indented)
    {
        var options = new JsonSerializerOptions
        {
            Encoder = MinimalJsonEncoder.Instance,
            WriteIndented = indented,
            MaxDepth = Wire.MaxDepth,
        };

        options.Converters.Add(new FiniteDoubleConverter());
        options.Converters.Add(new FiniteSingleConverter());
        return options;
    }

    /* ------------------------------------------------------------------ *
     * The format
     * ------------------------------------------------------------------ */

    /// <summary>
    /// Compress a document into a json-compress envelope.
    /// </summary>
    /// <param name="value">The document to compress, which may be null.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <returns>The envelope: a new tree that shares nothing with the input.</returns>
    /// <exception cref="JsonCompressException">The document nests too deeply to walk.</exception>
    public static JsonObject Compress(JsonNode? value, CompressOptions? options = null)
    {
        return Encoder.Compress(value, CompressOptions.Resolve(options));
    }

    /// <summary>
    /// Restore the original document from a json-compress envelope.
    /// </summary>
    /// <param name="envelope">The envelope to read.</param>
    /// <returns>The original document: a new tree that shares nothing with the envelope.</returns>
    /// <exception cref="JsonCompressException">The envelope is not one this build can read.</exception>
    public static JsonNode? Decompress(JsonNode? envelope)
    {
        return Decoder.Decompress(envelope);
    }

    /// <summary>True when a value looks like an envelope this build can read.</summary>
    /// <param name="value">The value to inspect.</param>
    /// <returns>Whether it is worth handing to <see cref="Decompress"/>.</returns>
    public static bool IsCompressed(JsonNode? value)
    {
        return Decoder.IsCompressed(value);
    }

    /// <summary>
    /// Restore a value whether or not it was compressed.
    /// </summary>
    /// <param name="value">The value, compressed or not.</param>
    /// <returns>The original document, or a copy of the input when it was never compressed.</returns>
    /// <remarks>
    /// Handy at a boundary you do not fully control - an endpoint being
    /// migrated, a cache holding entries written by two versions of the same
    /// service.
    /// </remarks>
    public static JsonNode? DecompressIfNeeded(JsonNode? value)
    {
        return IsCompressed(value) ? Decompress(value) : value?.DeepClone();
    }

    /* ------------------------------------------------------------------ *
     * Text in, text out
     * ------------------------------------------------------------------ */

    /// <summary>Compress a document and serialise it in one step.</summary>
    /// <param name="value">The document to compress.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <param name="indented">Whether to indent the output.</param>
    /// <returns>The compressed document, as JSON text.</returns>
    public static string Stringify(JsonNode? value, CompressOptions? options = null, bool indented = false)
    {
        return Write(Compress(value, options), indented);
    }

    /// <summary>Serialise a node, indented to this library house style or not at all.</summary>
    /// <param name="node">The document to write.</param>
    /// <param name="indented">Whether to indent it.</param>
    /// <returns>The JSON text.</returns>
    private static string Write(JsonNode node, bool indented)
    {
        return indented
            ? Reindent(node.ToJsonString(IndentedSerializerOptions), DefaultIndent)
            : node.ToJsonString(SerializerOptions);
    }

    /// <summary>Spaces per level for the indented forms.</summary>
    private const int DefaultIndent = 2;

    /// <summary>Parse a compressed document and restore the original.</summary>
    /// <param name="text">The compressed document, as JSON text.</param>
    /// <returns>The original document.</returns>
    /// <exception cref="JsonException">The text is not JSON.</exception>
    /// <exception cref="JsonCompressException">The text is JSON, but not an envelope this build can read.</exception>
    public static JsonNode? Parse(string text)
    {
        return Decompress(ParseJson(text));
    }

    /// <summary>Parse JSON text into a node, with this library depth limit.</summary>
    /// <param name="text">The JSON text.</param>
    /// <returns>The parsed document.</returns>
    /// <exception cref="ArgumentNullException">The text is null.</exception>
    /// <exception cref="JsonException">The text is not JSON.</exception>
    public static JsonNode? ParseJson(string text)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        return JsonNode.Parse(text, nodeOptions: null, documentOptions: DocumentOptions);
    }

    /// <summary>Strip the whitespace from a JSON document without changing what it means.</summary>
    /// <param name="text">The document.</param>
    /// <returns>The same document, with nothing between the tokens.</returns>
    public static string Minify(string text)
    {
        var node = ParseJson(text);
        return node is null ? "null" : node.ToJsonString(SerializerOptions);
    }

    /// <summary>Re-indent a JSON document.</summary>
    /// <param name="text">The document.</param>
    /// <param name="indent">How many spaces per level. Zero or less gives the minified form.</param>
    /// <returns>The same document, laid out for reading.</returns>
    public static string Beautify(string text, int indent = 2)
    {
        if (indent <= 0)
        {
            return Minify(text);
        }

        var node = ParseJson(text);
        var written = node is null ? "null" : node.ToJsonString(IndentedSerializerOptions);
        return Reindent(written, indent);
    }

    /// <summary>
    /// Restate already-indented JSON at a given indent width, with line feeds
    /// as its line endings.
    /// </summary>
    /// <param name="text">The indented JSON.</param>
    /// <param name="indent">The width wanted.</param>
    /// <returns>The same document, laid out to order.</returns>
    /// <remarks>
    /// The width the writer indents at, and the line ending it uses, have both
    /// varied between framework versions and between platforms; a document
    /// beautified on one machine should not differ from the same document
    /// beautified on another. So the width is measured from what the writer
    /// actually produced and rescaled, and the line endings are normalised. A
    /// JSON string can never contain a raw newline, so every run of leading
    /// spaces in the output is structural and can be rescaled safely.
    /// </remarks>
    private static string Reindent(string text, int indent)
    {
        var unit = MeasureIndent(text);
        var builder = new StringBuilder(text.Length);
        var at = 0;
        var first = true;

        while (true)
        {
            var end = text.IndexOf('\n', at);
            if (end < 0)
            {
                end = text.Length;
            }

            var lineEnd = end > at && text[end - 1] == '\r' ? end - 1 : end;

            var spaces = at;
            while (spaces < lineEnd && text[spaces] == ' ')
            {
                spaces++;
            }

            if (!first)
            {
                builder.Append('\n');
            }

            first = false;

            if (unit > 0)
            {
                builder.Append(' ', (spaces - at) / unit * indent);
            }

            builder.Append(text, spaces, lineEnd - spaces);

            if (end >= text.Length)
            {
                return builder.ToString();
            }

            at = end + 1;
        }
    }

    /// <summary>How many spaces the writer used for one level of indent.</summary>
    /// <param name="text">The indented JSON.</param>
    /// <returns>The width, or zero when the document has no indented line.</returns>
    private static int MeasureIndent(string text)
    {
        var at = text.IndexOf('\n');
        if (at < 0)
        {
            return 0;
        }

        var spaces = at + 1;
        while (spaces < text.Length && text[spaces] == ' ')
        {
            spaces++;
        }

        return spaces - at - 1;
    }

    /// <summary>
    /// Check that a string is valid JSON, and say exactly where it stops being so.
    /// </summary>
    /// <param name="text">The document to check.</param>
    /// <returns>The verdict.</returns>
    /// <remarks>
    /// The verdict comes from the parser, so it is the same verdict a parse
    /// would give. The location comes from a scan of the document, because
    /// parser messages disagree with each other about what they include - and
    /// often include no position at all.
    /// </remarks>
    public static JsonLocation Validate(string text)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        try
        {
            using (JsonDocument.Parse(text, DocumentOptions))
            {
                return JsonLocation.Ok();
            }
        }
        catch (JsonException error)
        {
            var found = Locate(text);
            if (!found.Valid)
            {
                return found;
            }

            // The scan found nothing but the parse still failed - report what
            // the parser said rather than claiming the document is fine.
            return new JsonLocation { Valid = false, Error = error.Message };
        }
    }

    /// <summary>Say where a JSON document stops being JSON.</summary>
    /// <param name="text">The document to check.</param>
    /// <returns>The verdict.</returns>
    public static JsonLocation Locate(string text)
    {
        return JsonLocator.Locate(text);
    }

    /* ------------------------------------------------------------------ *
     * Measurement
     * ------------------------------------------------------------------ */

    /// <summary>
    /// Measure what compression buys on a given document.
    /// </summary>
    /// <param name="value">The document to measure.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <param name="source">The text the document arrived as, if you have it.</param>
    /// <param name="gzip">Whether to gzip both forms as well.</param>
    /// <returns>The report.</returns>
    public static SizeReport Measure(
        JsonNode? value,
        CompressOptions? options = null,
        string? source = null,
        bool gzip = true)
    {
        var minified = value is null ? "null" : value.ToJsonString(SerializerOptions);
        var compressed = Stringify(value, options);

        var minifiedBytes = Bytes.ByteLength(minified);
        var compressedBytes = Bytes.ByteLength(compressed);

        return new SizeReport
        {
            Original = source is null ? minifiedBytes : Bytes.ByteLength(source),
            Minified = minifiedBytes,
            Compressed = compressedBytes,
            Saved = minifiedBytes - compressedBytes,
            Ratio = minifiedBytes == 0 ? 1d : (double)compressedBytes / minifiedBytes,
            Percent = minifiedBytes == 0 ? 0d : (double)(minifiedBytes - compressedBytes) / minifiedBytes * 100d,
            GzipMinified = gzip ? Bytes.GZipSize(minified) : (int?)null,
            GzipCompressed = gzip ? Bytes.GZipSize(compressed) : (int?)null,
        };
    }

    /* ------------------------------------------------------------------ *
     * For callers whose data is not already a JsonNode
     * ------------------------------------------------------------------ */

    /// <summary>
    /// Compress any serialisable value.
    /// </summary>
    /// <typeparam name="T">The type of the value.</typeparam>
    /// <param name="value">The value to compress.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <param name="serializerOptions">Serializer options, or null for this library defaults.</param>
    /// <returns>The envelope.</returns>
    /// <remarks>
    /// The value is serialised to a JSON tree first, so what gets compressed is
    /// exactly what the serialiser would have written. Non-finite numbers
    /// become null on the way, as they do in any JSON.
    /// </remarks>
    public static JsonObject CompressValue<T>(
        T value,
        CompressOptions? options = null,
        JsonSerializerOptions? serializerOptions = null)
    {
        var node = JsonSerializer.SerializeToNode(value, serializerOptions ?? SerializerOptions);
        return Compress(node, options);
    }

    /// <summary>
    /// Restore a value of a known type from an envelope.
    /// </summary>
    /// <typeparam name="T">The type to restore.</typeparam>
    /// <param name="envelope">The envelope to read.</param>
    /// <param name="serializerOptions">Serializer options, or null for this library defaults.</param>
    /// <returns>The restored value.</returns>
    public static T? DecompressValue<T>(JsonNode? envelope, JsonSerializerOptions? serializerOptions = null)
    {
        var node = Decompress(envelope);
        return node is null
            ? default
            : node.Deserialize<T>(serializerOptions ?? SerializerOptions);
    }

    /// <summary>Compress any serialisable value and serialise it in one step.</summary>
    /// <typeparam name="T">The type of the value.</typeparam>
    /// <param name="value">The value to compress.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <param name="indented">Whether to indent the output.</param>
    /// <param name="serializerOptions">Serializer options, or null for this library defaults.</param>
    /// <returns>The compressed document, as JSON text.</returns>
    public static string StringifyValue<T>(
        T value,
        CompressOptions? options = null,
        bool indented = false,
        JsonSerializerOptions? serializerOptions = null)
    {
        return Write(CompressValue(value, options, serializerOptions), indented);
    }

    /// <summary>Parse a compressed document and restore a value of a known type.</summary>
    /// <typeparam name="T">The type to restore.</typeparam>
    /// <param name="text">The compressed document, as JSON text.</param>
    /// <param name="serializerOptions">Serializer options, or null for this library defaults.</param>
    /// <returns>The restored value.</returns>
    public static T? ParseValue<T>(string text, JsonSerializerOptions? serializerOptions = null)
    {
        return DecompressValue<T>(ParseJson(text), serializerOptions);
    }

    /// <summary>Measure what compression buys on any serialisable value.</summary>
    /// <typeparam name="T">The type of the value.</typeparam>
    /// <param name="value">The value to measure.</param>
    /// <param name="options">Encoder options, or null for the defaults.</param>
    /// <param name="gzip">Whether to gzip both forms as well.</param>
    /// <param name="serializerOptions">Serializer options, or null for this library defaults.</param>
    /// <returns>The report.</returns>
    public static SizeReport MeasureValue<T>(
        T value,
        CompressOptions? options = null,
        bool gzip = true,
        JsonSerializerOptions? serializerOptions = null)
    {
        var node = JsonSerializer.SerializeToNode(value, serializerOptions ?? SerializerOptions);
        return Measure(node, options, source: null, gzip: gzip);
    }
}
