using System;

namespace TechStyle.JsonCompress;

/// <summary>
/// Thrown when a document cannot be read as a json-compress envelope.
/// </summary>
/// <remarks>
/// The decoder is deliberately strict. An out-of-range token, a stray hole
/// marker or a table whose rows do not line up with its columns all raise this
/// rather than quietly handing back something that is nearly right.
/// </remarks>
[Serializable]
public class JsonCompressException : Exception
{
    /// <summary>Create the exception with a message.</summary>
    /// <param name="message">What is wrong, in plain words.</param>
    public JsonCompressException(string message)
        : base("json-compress: " + message)
    {
    }

    /// <summary>Create the exception with a message and an underlying cause.</summary>
    /// <param name="message">What is wrong, in plain words.</param>
    /// <param name="innerException">The failure this one grew out of.</param>
    public JsonCompressException(string message, Exception innerException)
        : base("json-compress: " + message, innerException)
    {
    }

    /// <summary>Create the exception with no message.</summary>
    public JsonCompressException()
        : base("json-compress: the document could not be read")
    {
    }

#if NETSTANDARD2_0
    /// <summary>Deserialisation constructor.</summary>
    /// <param name="info">Serialised member data.</param>
    /// <param name="context">Streaming context.</param>
    protected JsonCompressException(
        System.Runtime.Serialization.SerializationInfo info,
        System.Runtime.Serialization.StreamingContext context)
        : base(info, context)
    {
    }
#endif
}
