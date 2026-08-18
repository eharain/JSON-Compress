using System;
using System.IO;
using System.IO.Compression;
using System.Text;

namespace TechStyle.JsonCompress;

/// <summary>
/// Byte measurement and gzip.
/// </summary>
/// <remarks>
/// What matters on the wire is UTF-8 bytes, not characters, and what most
/// payloads meet in production is gzip. Both live here so the size report can
/// answer the only question worth asking: is the document smaller once it has
/// been through everything it is going to go through.
/// </remarks>
public static class Bytes
{
    private static readonly UTF8Encoding Utf8 = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);

    /// <summary>
    /// Length of a string in UTF-8 bytes - what actually goes over the wire, as
    /// opposed to the character count, which is UTF-16 code units.
    /// </summary>
    /// <param name="text">The text to measure.</param>
    /// <returns>How many bytes it takes.</returns>
    /// <exception cref="ArgumentNullException">The text is null.</exception>
    public static int ByteLength(string text)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        return Utf8.GetByteCount(text);
    }

    /// <summary>Gzip a string.</summary>
    /// <param name="text">The text to compress.</param>
    /// <param name="level">How hard to work at it.</param>
    /// <returns>The gzip stream, as bytes.</returns>
    /// <exception cref="ArgumentNullException">The text is null.</exception>
    public static byte[] GZip(string text, CompressionLevel level = CompressionLevel.Optimal)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        var bytes = Utf8.GetBytes(text);
        using var output = new MemoryStream();
        using (var gzip = new GZipStream(output, level, leaveOpen: true))
        {
            gzip.Write(bytes, 0, bytes.Length);
        }

        return output.ToArray();
    }

    /// <summary>Reverse <see cref="GZip(string, CompressionLevel)"/>.</summary>
    /// <param name="bytes">The gzip stream.</param>
    /// <returns>The text it holds.</returns>
    /// <exception cref="ArgumentNullException">The bytes are null.</exception>
    public static string GUnzip(byte[] bytes)
    {
        if (bytes is null)
        {
            throw new ArgumentNullException(nameof(bytes));
        }

        using var input = new MemoryStream(bytes, writable: false);
        using var gzip = new GZipStream(input, CompressionMode.Decompress);
        using var output = new MemoryStream();
        gzip.CopyTo(output);
        return Utf8.GetString(output.ToArray());
    }

    /// <summary>How many bytes a string takes once gzipped.</summary>
    /// <param name="text">The text to measure.</param>
    /// <returns>The gzipped length in bytes.</returns>
    public static int GZipSize(string text)
    {
        return GZip(text).Length;
    }
}
