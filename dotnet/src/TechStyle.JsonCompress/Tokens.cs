using System.Text;

namespace TechStyle.JsonCompress;

/// <summary>
/// Short tokens for catalogue indices.
/// </summary>
/// <remarks>
/// <para>
/// Two alphabets, because the two places a token appears have different rules.
/// </para>
/// <para>
/// String references sit in value position, where nothing constrains them, so
/// they use plain base-62: the first 62 entries cost one character.
/// </para>
/// <para>
/// Object keys cannot. A JavaScript object - and every JSON parser that builds
/// one - reorders keys that look like array indices to the front, in numeric
/// order, so a key of <c>"10"</c> would jump ahead of a key of <c>"2"</c>.
/// Encoding a key as <c>"7"</c> would therefore quietly shuffle the document
/// for anyone reading it from JavaScript. Key tokens instead start with a
/// letter and continue in base-62, which is never index-like: 52 keys still
/// cost one character, the next 3,224 cost two, and the original key order
/// survives the round trip exactly.
/// </para>
/// </remarks>
internal static class Tokens
{
    private const string Digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    private const string Letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    private const int DigitBase = 62;
    private const int LetterCount = 52;

    /// <summary>Value of each ASCII character as a base-62 digit, or -1.</summary>
    private static readonly sbyte[] DigitValues = BuildLookup(Digits);

    /// <summary>Value of each ASCII character as a leading letter, or -1.</summary>
    private static readonly sbyte[] LetterValues = BuildLookup(Letters);

    /// <summary>Returned by the decoders for anything that is not a token.</summary>
    public const int NotAToken = -1;

    private static sbyte[] BuildLookup(string alphabet)
    {
        var table = new sbyte[128];
        for (var i = 0; i < table.Length; i++)
        {
            table[i] = -1;
        }

        for (var i = 0; i < alphabet.Length; i++)
        {
            table[alphabet[i]] = (sbyte)i;
        }

        return table;
    }

    /* ------------------------------------------------------------------ *
     * Base-62, for string references
     * ------------------------------------------------------------------ */

    /// <summary>Encode a non-negative integer as a base-62 token.</summary>
    /// <param name="n">The index to encode.</param>
    /// <returns>The token.</returns>
    public static string ToBase62(int n)
    {
        if (n < DigitBase)
        {
            return Digits[n].ToString();
        }

        var builder = new StringBuilder(6);
        var value = n;
        while (value > 0)
        {
            builder.Insert(0, Digits[value % DigitBase]);
            value /= DigitBase;
        }

        return builder.ToString();
    }

    /// <summary>Decode a base-62 token.</summary>
    /// <param name="token">The token, without its leading tilde.</param>
    /// <returns>The index, or <see cref="NotAToken"/> when the text is not one.</returns>
    /// <remarks>
    /// The accumulator is 64 bit so that an over-long token from a hostile
    /// document overflows into "out of range" rather than wrapping around into
    /// a plausible-looking index.
    /// </remarks>
    public static int FromBase62(string token)
    {
        if (token.Length == 0)
        {
            return NotAToken;
        }

        long n = 0;
        for (var i = 0; i < token.Length; i++)
        {
            var c = token[i];
            var digit = c < 128 ? DigitValues[c] : -1;
            if (digit < 0)
            {
                return NotAToken;
            }

            n = (n * DigitBase) + digit;
            if (n > int.MaxValue)
            {
                return NotAToken;
            }
        }

        return (int)n;
    }

    /// <summary>Length in characters of the base-62 token for an index, without building it.</summary>
    /// <param name="n">The index to measure.</param>
    /// <returns>How many characters the token takes.</returns>
    public static int Base62Length(int n)
    {
        if (n < 62)
        {
            return 1;
        }

        if (n < 3844)
        {
            return 2;
        }

        if (n < 238328)
        {
            return 3;
        }

        var length = 0;
        var value = n;
        while (value > 0)
        {
            length++;
            value /= DigitBase;
        }

        return length;
    }

    /* ------------------------------------------------------------------ *
     * Letter-first tokens, for object keys
     * ------------------------------------------------------------------ */

    /// <summary>
    /// Encode a non-negative integer as a key token: a letter, then base-62 digits.
    /// </summary>
    /// <param name="n">The index to encode.</param>
    /// <returns>The token.</returns>
    /// <remarks>
    /// The numbering is bijective - each token length covers a contiguous block
    /// of indices and no index has two spellings - so <see cref="FromKeyToken"/>
    /// inverts it exactly.
    /// </remarks>
    public static string ToKeyToken(int n)
    {
        if (n < LetterCount)
        {
            return Letters[n].ToString();
        }

        // Find the block this index falls in: 52 one-character tokens, then
        // 52 * 62 two-character ones, and so on.
        long start = 0;
        long size = LetterCount;
        var length = 1;
        while (n >= start + size)
        {
            start += size;
            size *= DigitBase;
            length++;
        }

        var value = n - start;
        var tail = new char[length - 1];
        for (var i = length - 2; i >= 0; i--)
        {
            tail[i] = Digits[(int)(value % DigitBase)];
            value /= DigitBase;
        }

        return Letters[(int)value] + new string(tail);
    }

    /// <summary>Decode a key token.</summary>
    /// <param name="token">The token.</param>
    /// <returns>The index, or <see cref="NotAToken"/> when the text is not one.</returns>
    public static int FromKeyToken(string token)
    {
        if (token.Length == 0)
        {
            return NotAToken;
        }

        var first = token[0];
        long n = first < 128 ? LetterValues[first] : -1;
        if (n < 0)
        {
            return NotAToken;
        }

        for (var i = 1; i < token.Length; i++)
        {
            var c = token[i];
            var digit = c < 128 ? DigitValues[c] : -1;
            if (digit < 0)
            {
                return NotAToken;
            }

            n = (n * DigitBase) + digit;
            if (n > int.MaxValue)
            {
                return NotAToken;
            }
        }

        long start = 0;
        long size = LetterCount;
        for (var i = 1; i < token.Length; i++)
        {
            start += size;
            size *= DigitBase;
            if (start > int.MaxValue)
            {
                return NotAToken;
            }
        }

        var index = start + n;
        return index > int.MaxValue ? NotAToken : (int)index;
    }
}
