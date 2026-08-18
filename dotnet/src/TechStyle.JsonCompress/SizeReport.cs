namespace TechStyle.JsonCompress;

/// <summary>
/// What compression bought on one document.
/// </summary>
/// <remarks>
/// The honest comparison is against minified JSON, not against whatever
/// whitespace the input happened to arrive with, so <see cref="Ratio"/> and
/// <see cref="Percent"/> both use the minified size as their baseline. The gzip
/// figures are here because that is what most payloads meet in production, and
/// the interesting question is whether the format still helps once gzip has had
/// its turn.
/// </remarks>
public sealed class SizeReport
{
    /// <summary>Bytes of the input as given.</summary>
    public int Original { get; set; }

    /// <summary>Bytes once whitespace is removed.</summary>
    public int Minified { get; set; }

    /// <summary>Bytes of the compressed envelope.</summary>
    public int Compressed { get; set; }

    /// <summary>Bytes saved against the minified form. Negative when the document grew.</summary>
    public int Saved { get; set; }

    /// <summary>Compressed divided by minified, where 1 is no change at all.</summary>
    public double Ratio { get; set; }

    /// <summary>Percentage saved against the minified form.</summary>
    public double Percent { get; set; }

    /// <summary>Minified bytes after gzip, or null when gzip was not measured.</summary>
    public int? GzipMinified { get; set; }

    /// <summary>Compressed bytes after gzip, or null when gzip was not measured.</summary>
    public int? GzipCompressed { get; set; }
}
