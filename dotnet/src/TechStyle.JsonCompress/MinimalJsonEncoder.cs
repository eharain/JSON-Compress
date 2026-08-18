using System;
using System.Text.Encodings.Web;

namespace TechStyle.JsonCompress;

/// <summary>
/// Escapes exactly what JSON requires escaped, and nothing else.
/// </summary>
/// <remarks>
/// <para>
/// The stock encoders all escape more than the grammar asks for. The relaxed
/// one leaves the Basic Multilingual Plane alone but still writes anything
/// above it - every emoji, every historic script - as a pair of six-character
/// escapes, turning four bytes into twelve. In a library whose whole purpose is
/// to make documents smaller, and whose output has to match what other
/// implementations of the format write, that will not do.
/// </para>
/// <para>
/// So this escapes the two characters JSON reserves, the control characters it
/// forbids, and lone surrogates, which are not text at all. Everything else is
/// written as itself. That is the same set a JavaScript <c>JSON.stringify</c>
/// escapes.
/// </para>
/// <para>
/// The output is JSON for a JSON reader, not text to be pasted straight into an
/// HTML page or a script tag. Escape it at the point of embedding, as you would
/// any other JSON.
/// </para>
/// </remarks>
internal sealed class MinimalJsonEncoder : JavaScriptEncoder
{
    /// <summary>The shared instance.</summary>
    public static readonly MinimalJsonEncoder Instance = new MinimalJsonEncoder();

    private const string HexDigits = "0123456789abcdef";

    private MinimalJsonEncoder()
    {
    }

    /// <inheritdoc />
    /// <remarks>The longest escape is the six characters of a hex form.</remarks>
    public override int MaxOutputCharactersPerInputCharacter => 6;

    /// <inheritdoc />
    public override bool WillEncode(int unicodeScalar)
    {
        return NeedsEscape(unicodeScalar);
    }

    /// <inheritdoc />
    public override unsafe int FindFirstCharacterToEncode(char* text, int textLength)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        for (var i = 0; i < textLength; i++)
        {
            var c = text[i];

            if (c == '"' || c == '\\' || c < ' ')
            {
                return i;
            }

            if (char.IsSurrogate(c))
            {
                // A well-formed pair is one character as far as JSON is
                // concerned and is written as itself. A surrogate on its own is
                // not text, and has to be escaped to survive at all.
                if (char.IsHighSurrogate(c) && i + 1 < textLength && char.IsLowSurrogate(text[i + 1]))
                {
                    i++;
                    continue;
                }

                return i;
            }
        }

        return -1;
    }

    /// <inheritdoc />
    public override unsafe bool TryEncodeUnicodeScalar(
        int unicodeScalar,
        char* buffer,
        int bufferLength,
        out int numberOfCharactersWritten)
    {
        if (buffer is null)
        {
            throw new ArgumentNullException(nameof(buffer));
        }

        numberOfCharactersWritten = 0;

        var shorthand = Shorthand(unicodeScalar);
        if (shorthand != '\0')
        {
            if (bufferLength < 2)
            {
                return false;
            }

            buffer[0] = '\\';
            buffer[1] = shorthand;
            numberOfCharactersWritten = 2;
            return true;
        }

        if (bufferLength < 6)
        {
            return false;
        }

        buffer[0] = '\\';
        buffer[1] = 'u';
        buffer[2] = HexDigits[(unicodeScalar >> 12) & 0xF];
        buffer[3] = HexDigits[(unicodeScalar >> 8) & 0xF];
        buffer[4] = HexDigits[(unicodeScalar >> 4) & 0xF];
        buffer[5] = HexDigits[unicodeScalar & 0xF];
        numberOfCharactersWritten = 6;
        return true;
    }

    private static bool NeedsEscape(int unicodeScalar)
    {
        return unicodeScalar == '"'
            || unicodeScalar == '\\'
            || unicodeScalar < ' '
            || (unicodeScalar >= 0xD800 && unicodeScalar <= 0xDFFF);
    }

    /// <summary>The one-letter escape for a character that has one, or NUL.</summary>
    /// <param name="unicodeScalar">The character.</param>
    /// <returns>The letter that follows the backslash.</returns>
    private static char Shorthand(int unicodeScalar)
    {
        switch (unicodeScalar)
        {
            case '"': return '"';
            case '\\': return '\\';
            case '\b': return 'b';
            case '\t': return 't';
            case '\n': return 'n';
            case '\f': return 'f';
            case '\r': return 'r';
            default: return '\0';
        }
    }
}
