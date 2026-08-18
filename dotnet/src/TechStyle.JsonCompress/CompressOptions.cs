using System;

namespace TechStyle.JsonCompress;

/// <summary>
/// The knobs on the encoder.
/// </summary>
/// <remarks>
/// Every one has a default that is safe on arbitrary input. The defaults are
/// tuned for API responses and log batches, which is where the format earns
/// most of its keep. Nothing here changes what a decoder has to do: two
/// encoders may make different choices and both produce valid documents.
/// </remarks>
public sealed class CompressOptions
{
    private int minRows = 2;
    private double maxHoleRatio = 0.3;
    private int minStringCount = 2;
    private int minStringLength = 1;

    /// <summary>De-duplicate repeated string values into a catalogue.</summary>
    public bool Strings { get; set; } = true;

    /// <summary>Pack arrays of like-shaped objects into columnar tables.</summary>
    public bool Tables { get; set; } = true;

    /// <summary>Hoist columns whose value never changes out of the rows.</summary>
    public bool Constants { get; set; } = true;

    /// <summary>Encode low-cardinality string columns as integer indices.</summary>
    public bool DictionaryColumns { get; set; } = true;

    /// <summary>Smallest array that may become a table. At least one.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is below one.</exception>
    public int MinRows
    {
        get => this.minRows;
        set
        {
            if (value < 1)
            {
                throw new ArgumentOutOfRangeException(nameof(value), value, "MinRows must be at least 1.");
            }

            this.minRows = value;
        }
    }

    /// <summary>Largest share of missing cells a table may carry, from zero to one.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is outside zero to one.</exception>
    public double MaxHoleRatio
    {
        get => this.maxHoleRatio;
        set
        {
            if (!(value >= 0d && value <= 1d))
            {
                throw new ArgumentOutOfRangeException(nameof(value), value, "MaxHoleRatio must be between 0 and 1.");
            }

            this.maxHoleRatio = value;
        }
    }

    /// <summary>Times a string must occur before it can be catalogued. At least two.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is below two.</exception>
    public int MinStringCount
    {
        get => this.minStringCount;
        set
        {
            if (value < 2)
            {
                throw new ArgumentOutOfRangeException(nameof(value), value, "MinStringCount must be at least 2.");
            }

            this.minStringCount = value;
        }
    }

    /// <summary>Shortest string that may be catalogued.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is negative.</exception>
    public int MinStringLength
    {
        get => this.minStringLength;
        set
        {
            if (value < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(value), value, "MinStringLength cannot be negative.");
            }

            this.minStringLength = value;
        }
    }

    /// <summary>A fresh set of options, all at their defaults.</summary>
    /// <returns>The default options.</returns>
    public static CompressOptions Default()
    {
        return new CompressOptions();
    }

    /// <summary>Copy these options, so a caller can adjust one without touching the original.</summary>
    /// <returns>An independent copy.</returns>
    public CompressOptions Clone()
    {
        return new CompressOptions
        {
            Strings = this.Strings,
            Tables = this.Tables,
            Constants = this.Constants,
            DictionaryColumns = this.DictionaryColumns,
            MinRows = this.MinRows,
            MaxHoleRatio = this.MaxHoleRatio,
            MinStringCount = this.MinStringCount,
            MinStringLength = this.MinStringLength,
        };
    }

    /// <summary>The options to use for a call, when the caller passed none.</summary>
    /// <param name="options">What the caller passed, which may be null.</param>
    /// <returns>Options that are safe to read from.</returns>
    internal static CompressOptions Resolve(CompressOptions? options)
    {
        return options ?? DefaultInstance;
    }

    /// <summary>
    /// A shared instance for the common case of no options at all. Every
    /// property validates on assignment, so this cannot be pushed into an
    /// invalid state by one caller and read by another.
    /// </summary>
    private static readonly CompressOptions DefaultInstance = new CompressOptions();
}
