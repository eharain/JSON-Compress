using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress;

/// <summary>
/// A set of options bound once and reused.
/// </summary>
/// <remarks>
/// A codec is the natural unit when the same options apply everywhere - a
/// transport layer, a cache adapter, a storage helper - and it keeps the option
/// object off every call site. It holds no state beyond those options, so one
/// instance can be shared by any number of threads.
/// </remarks>
public sealed class JsonCompressCodec
{
    private readonly CompressOptions options;

    /// <summary>Create a codec.</summary>
    /// <param name="options">The options to bind, or null for the defaults.</param>
    public JsonCompressCodec(CompressOptions? options = null)
    {
        this.options = (options ?? CompressOptions.Default()).Clone();
    }

    /// <summary>The options this codec was built with.</summary>
    /// <returns>A copy, so adjusting it cannot change the codec.</returns>
    public CompressOptions Options()
    {
        return this.options.Clone();
    }

    /// <summary>Compress a document with the bound options.</summary>
    /// <param name="value">The document to compress.</param>
    /// <returns>The envelope.</returns>
    public JsonObject Compress(JsonNode? value)
    {
        return JsonCompressor.Compress(value, this.options);
    }

    /// <summary>Restore a document from an envelope.</summary>
    /// <param name="envelope">The envelope to read.</param>
    /// <returns>The original document.</returns>
    public JsonNode? Decompress(JsonNode? envelope)
    {
        return JsonCompressor.Decompress(envelope);
    }

    /// <summary>Compress a document and serialise it in one step.</summary>
    /// <param name="value">The document to compress.</param>
    /// <returns>The compressed document, as JSON text.</returns>
    public string Stringify(JsonNode? value)
    {
        return JsonCompressor.Stringify(value, this.options);
    }

    /// <summary>Parse a compressed document and restore the original.</summary>
    /// <param name="text">The compressed document, as JSON text.</param>
    /// <returns>The original document.</returns>
    public JsonNode? Parse(string text)
    {
        return JsonCompressor.Parse(text);
    }

    /// <summary>Measure what the bound options buy on a document.</summary>
    /// <param name="value">The document to measure.</param>
    /// <param name="gzip">Whether to gzip both forms as well.</param>
    /// <returns>The report.</returns>
    public SizeReport Measure(JsonNode? value, bool gzip = true)
    {
        return JsonCompressor.Measure(value, this.options, source: null, gzip: gzip);
    }
}
