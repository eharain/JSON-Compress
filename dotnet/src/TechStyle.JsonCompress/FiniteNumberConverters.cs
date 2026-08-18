using System;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace TechStyle.JsonCompress;

/// <summary>
/// Writes a non-finite number as null, the way every JSON writer does.
/// </summary>
/// <remarks>
/// JSON has no spelling for NaN or infinity. Left alone, the serialiser refuses
/// the value and the whole call fails; the format instead asks for the same
/// substitution JavaScript makes, so a stray NaN costs one field rather than
/// the entire document. Negative zero is folded to zero for the same reason:
/// JSON cannot tell the two apart, so writing it as anything else would make
/// the round trip disagree with itself.
/// </remarks>
internal sealed class FiniteDoubleConverter : JsonConverter<double>
{
    /// <inheritdoc />
    public override double Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        // A null here is a number that could not be written - the value it stood
        // for was not one JSON can carry, and NaN says so.
        return reader.TokenType == JsonTokenType.Null ? double.NaN : reader.GetDouble();
    }

    /// <inheritdoc />
    public override void Write(Utf8JsonWriter writer, double value, JsonSerializerOptions options)
    {
        if (writer is null)
        {
            throw new ArgumentNullException(nameof(writer));
        }

        if (double.IsNaN(value) || double.IsInfinity(value))
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteNumberValue(value == 0d ? 0d : value);
    }
}

/// <summary>
/// The single-precision counterpart of <see cref="FiniteDoubleConverter"/>.
/// </summary>
internal sealed class FiniteSingleConverter : JsonConverter<float>
{
    /// <inheritdoc />
    public override float Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        return reader.TokenType == JsonTokenType.Null ? float.NaN : reader.GetSingle();
    }

    /// <inheritdoc />
    public override void Write(Utf8JsonWriter writer, float value, JsonSerializerOptions options)
    {
        if (writer is null)
        {
            throw new ArgumentNullException(nameof(writer));
        }

        if (float.IsNaN(value) || float.IsInfinity(value))
        {
            writer.WriteNullValue();
            return;
        }

        writer.WriteNumberValue(value == 0f ? 0f : value);
    }
}
