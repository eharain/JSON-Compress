using System;
using System.Globalization;

namespace TechStyle.JsonCompress;

/// <summary>
/// Finding the exact place a JSON document goes wrong.
/// </summary>
/// <remarks>
/// <para>
/// A JSON parser throws a message, and what that message contains depends on
/// the parser and its version: sometimes a byte offset, sometimes a line and
/// column, sometimes a fragment of the text with no position at all. None of
/// that is reliably enough to put a cursor on the offending character.
/// </para>
/// <para>
/// So this scans the document itself, against the grammar in RFC 8259, and
/// reports the first thing that does not belong - with an offset, a line, a
/// column and a description written for whoever has to fix it. It is only ever
/// run after a parse has already failed, so the cost of a second pass falls on
/// documents that were not going to work anyway.
/// </para>
/// </remarks>
public static class JsonLocator
{
    /// <summary>How deep the scan will follow a document before giving up.</summary>
    private const int MaxDepth = 5000;

    /// <summary>
    /// Check a JSON document and, if it is broken, say exactly where.
    /// </summary>
    /// <param name="text">The document to check.</param>
    /// <returns>The verdict.</returns>
    /// <exception cref="ArgumentNullException">The text is null.</exception>
    public static JsonLocation Locate(string text)
    {
        if (text is null)
        {
            throw new ArgumentNullException(nameof(text));
        }

        var scanner = new Scanner(text);
        try
        {
            scanner.ScanValue(0);
            scanner.SkipWhitespace();
            if (scanner.At < text.Length)
            {
                throw new ScanException(
                    "unexpected " + Describe(text, scanner.At) + " after the end of the document",
                    scanner.At);
            }
        }
        catch (ScanException error)
        {
            return Report(text, error);
        }

        return JsonLocation.Ok();
    }

    private static JsonLocation Report(string text, ScanException error)
    {
        var at = Math.Max(0, Math.Min(error.At, text.Length));

        var line = 1;
        var lineStart = 0;
        for (var i = 0; i < at; i++)
        {
            if (text[i] == '\n')
            {
                line++;
                lineStart = i + 1;
            }
        }

        var lineEnd = text.IndexOf('\n', lineStart);
        if (lineEnd < 0)
        {
            lineEnd = text.Length;
        }

        var excerpt = text.Substring(lineStart, lineEnd - lineStart).TrimEnd('\r');

        return new JsonLocation
        {
            Valid = false,
            Error = error.Reason,
            Position = at,
            Line = line,
            Column = at - lineStart + 1,
            Excerpt = excerpt,
        };
    }

    private static string Describe(string text, int at)
    {
        if (at >= text.Length)
        {
            return "the document ends here";
        }

        var c = text[at];
        if (c < ' ')
        {
            return "an unescaped control character (0x" + ((int)c).ToString("x2", CultureInfo.InvariantCulture) + ")";
        }

        return "\"" + c + "\"";
    }

    /// <summary>Somewhere to unwind to, carrying the offset the problem was found at.</summary>
    private sealed class ScanException : Exception
    {
        public ScanException(string reason, int at)
            : base(reason)
        {
            this.Reason = reason;
            this.At = at;
        }

        public string Reason { get; }

        public int At { get; }
    }

    private sealed class Scanner
    {
        private readonly string text;

        public Scanner(string text)
        {
            this.text = text;
        }

        public int At { get; private set; }

        public void SkipWhitespace()
        {
            while (this.At < this.text.Length)
            {
                var c = this.text[this.At];
                if (c == ' ' || c == '\t' || c == '\n' || c == '\r')
                {
                    this.At++;
                }
                else
                {
                    break;
                }
            }
        }

        public void ScanValue(int depth)
        {
            if (depth > MaxDepth)
            {
                throw new ScanException("the document is nested too deeply to check", this.At);
            }

            this.SkipWhitespace();
            if (this.At >= this.text.Length)
            {
                throw new ScanException("expected a value, but the document ends here", this.At);
            }

            var c = this.text[this.At];

            switch (c)
            {
                case '{':
                    this.ScanObject(depth);
                    return;
                case '[':
                    this.ScanArray(depth);
                    return;
                case '"':
                    this.ScanString();
                    return;
                default:
                    break;
            }

            if (c == '-' || (c >= '0' && c <= '9'))
            {
                this.ScanNumber();
                return;
            }

            if (this.Starts("true"))
            {
                this.At += 4;
                return;
            }

            if (this.Starts("false"))
            {
                this.At += 5;
                return;
            }

            if (this.Starts("null"))
            {
                this.At += 4;
                return;
            }

            // The three mistakes that account for most hand-edited JSON.
            if (c == '\'')
            {
                throw new ScanException("strings must use double quotes, not single quotes", this.At);
            }

            if (this.Starts("True") || this.Starts("False"))
            {
                throw new ScanException("true and false must be lower case in JSON", this.At);
            }

            if (this.Starts("None") || this.Starts("undefined"))
            {
                throw new ScanException("JSON has no None or undefined - use null", this.At);
            }

            throw new ScanException("expected a value, found " + Describe(this.text, this.At), this.At);
        }

        private bool Starts(string word)
        {
            return string.CompareOrdinal(this.text, this.At, word, 0, word.Length) == 0
                && this.At + word.Length <= this.text.Length;
        }

        private void Expect(char expected, string what)
        {
            if (this.At >= this.text.Length || this.text[this.At] != expected)
            {
                throw new ScanException("expected " + what + ", found " + Describe(this.text, this.At), this.At);
            }

            this.At++;
        }

        private void ScanObject(int depth)
        {
            this.At++; // the opening brace
            this.SkipWhitespace();

            if (this.At < this.text.Length && this.text[this.At] == '}')
            {
                this.At++;
                return;
            }

            while (true)
            {
                this.SkipWhitespace();
                if (this.At < this.text.Length && this.text[this.At] != '"')
                {
                    if (this.text[this.At] == '\'')
                    {
                        throw new ScanException("strings must use double quotes, not single quotes", this.At);
                    }

                    throw new ScanException(
                        "expected a quoted property name, found " + Describe(this.text, this.At),
                        this.At);
                }

                this.ScanString();

                this.SkipWhitespace();
                this.Expect(':', "a colon after the property name");

                this.ScanValue(depth + 1);
                this.SkipWhitespace();

                if (this.At >= this.text.Length)
                {
                    throw new ScanException("this object is never closed", this.At);
                }

                var c = this.text[this.At];
                if (c == ',')
                {
                    var commaAt = this.At;
                    this.At++;
                    this.SkipWhitespace();
                    if (this.At < this.text.Length && this.text[this.At] == '}')
                    {
                        throw new ScanException("a trailing comma is not allowed in JSON", commaAt);
                    }

                    continue;
                }

                if (c == '}')
                {
                    this.At++;
                    return;
                }

                throw new ScanException(
                    "expected a comma or a closing brace, found " + Describe(this.text, this.At),
                    this.At);
            }
        }

        private void ScanArray(int depth)
        {
            this.At++; // the opening bracket
            this.SkipWhitespace();

            if (this.At < this.text.Length && this.text[this.At] == ']')
            {
                this.At++;
                return;
            }

            while (true)
            {
                this.ScanValue(depth + 1);
                this.SkipWhitespace();

                if (this.At >= this.text.Length)
                {
                    throw new ScanException("this array is never closed", this.At);
                }

                var c = this.text[this.At];
                if (c == ',')
                {
                    var commaAt = this.At;
                    this.At++;
                    this.SkipWhitespace();
                    if (this.At < this.text.Length && this.text[this.At] == ']')
                    {
                        throw new ScanException("a trailing comma is not allowed in JSON", commaAt);
                    }

                    continue;
                }

                if (c == ']')
                {
                    this.At++;
                    return;
                }

                throw new ScanException(
                    "expected a comma or a closing bracket, found " + Describe(this.text, this.At),
                    this.At);
            }
        }

        private void ScanString()
        {
            var start = this.At;
            this.At++; // the opening quote

            while (this.At < this.text.Length)
            {
                var c = this.text[this.At];

                if (c == '"')
                {
                    this.At++;
                    return;
                }

                if (c == '\\')
                {
                    this.At++;
                    if (this.At >= this.text.Length)
                    {
                        break;
                    }

                    var escape = this.text[this.At];
                    if ("\"\\/bfnrt".IndexOf(escape) >= 0)
                    {
                        this.At++;
                        continue;
                    }

                    if (escape == 'u')
                    {
                        for (var i = 1; i <= 4; i++)
                        {
                            var at = this.At + i;
                            if (at >= this.text.Length || !IsHex(this.text[at]))
                            {
                                throw new ScanException(
                                    "\\u must be followed by four hexadecimal digits",
                                    this.At - 1);
                            }
                        }

                        this.At += 5;
                        continue;
                    }

                    throw new ScanException("\"\\" + escape + "\" is not a valid escape sequence", this.At - 1);
                }

                if (c < ' ')
                {
                    throw new ScanException(
                        "a raw control character inside a string - escape it as \\u"
                        + ((int)c).ToString("x4", CultureInfo.InvariantCulture),
                        this.At);
                }

                this.At++;
            }

            throw new ScanException("this string is never closed", start);
        }

        private static bool IsHex(char c)
        {
            return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
        }

        private void ScanNumber()
        {
            var start = this.At;

            if (this.text[this.At] == '-')
            {
                this.At++;
            }

            if (this.At >= this.text.Length)
            {
                throw new ScanException("this number has no digits", start);
            }

            if (this.text[this.At] == '0')
            {
                this.At++;
                if (this.At < this.text.Length && this.text[this.At] >= '0' && this.text[this.At] <= '9')
                {
                    throw new ScanException("a number may not have a leading zero", start);
                }
            }
            else
            {
                if (!(this.text[this.At] >= '0' && this.text[this.At] <= '9'))
                {
                    throw new ScanException("expected a digit, found " + Describe(this.text, this.At), this.At);
                }

                while (this.At < this.text.Length && this.text[this.At] >= '0' && this.text[this.At] <= '9')
                {
                    this.At++;
                }
            }

            if (this.At < this.text.Length && this.text[this.At] == '.')
            {
                this.At++;
                var digits = 0;
                while (this.At < this.text.Length && this.text[this.At] >= '0' && this.text[this.At] <= '9')
                {
                    this.At++;
                    digits++;
                }

                if (digits == 0)
                {
                    throw new ScanException("a decimal point must be followed by at least one digit", this.At - 1);
                }
            }

            if (this.At < this.text.Length && (this.text[this.At] == 'e' || this.text[this.At] == 'E'))
            {
                this.At++;
                if (this.At < this.text.Length && (this.text[this.At] == '+' || this.text[this.At] == '-'))
                {
                    this.At++;
                }

                var digits = 0;
                while (this.At < this.text.Length && this.text[this.At] >= '0' && this.text[this.At] <= '9')
                {
                    this.At++;
                    digits++;
                }

                if (digits == 0)
                {
                    throw new ScanException("an exponent must have at least one digit", this.At - 1);
                }
            }
        }
    }
}
