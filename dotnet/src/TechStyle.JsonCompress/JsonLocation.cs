namespace TechStyle.JsonCompress;

/// <summary>
/// Where a JSON document stops being JSON.
/// </summary>
/// <remarks>
/// A verdict of valid carries nothing else. A verdict of invalid carries enough
/// to put a cursor on the offending character: what is wrong in plain words, an
/// offset, a line and column counted from one, and the line itself.
/// </remarks>
public sealed class JsonLocation
{
    /// <summary>Whether the document parses.</summary>
    public bool Valid { get; set; }

    /// <summary>What is wrong, in plain words. Null when the document is valid.</summary>
    public string? Error { get; set; }

    /// <summary>Character offset of the problem.</summary>
    public int Position { get; set; }

    /// <summary>Line number, counted from one.</summary>
    public int Line { get; set; }

    /// <summary>Column number, counted from one.</summary>
    public int Column { get; set; }

    /// <summary>The line the problem is on.</summary>
    public string Excerpt { get; set; } = string.Empty;

    /// <summary>A location that says the document is fine.</summary>
    /// <returns>The verdict.</returns>
    public static JsonLocation Ok()
    {
        return new JsonLocation { Valid = true };
    }

    /// <summary>A one-line summary, for a command line or a log.</summary>
    /// <returns>The summary.</returns>
    public override string ToString()
    {
        if (this.Valid)
        {
            return "valid JSON";
        }

        return "line " + this.Line.ToString(System.Globalization.CultureInfo.InvariantCulture)
            + ", column " + this.Column.ToString(System.Globalization.CultureInfo.InvariantCulture)
            + ": " + this.Error;
    }
}
