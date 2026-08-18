namespace TechStyle.JsonCompress;

/// <summary>
/// Wire-format constants for the json-compress envelope.
/// </summary>
/// <remarks>
/// <para>
/// Everything the encoder emits is still ordinary JSON. The format therefore
/// has to be self-describing without any out-of-band type tags, which it
/// achieves by reserving a single character - the tilde - inside object keys
/// and string values.
/// </para>
/// <para>
/// Every original object key is replaced by a catalogue token drawn from
/// letters and digits, so no encoded object key can begin with a tilde. That
/// makes tilde-prefixed keys safe to reserve for structural nodes. String
/// values are escaped on the way in - a value that already starts with a tilde
/// gains one more - which frees the tilde-prefixed string space for catalogue
/// references and the row hole marker.
/// </para>
/// </remarks>
internal static class Wire
{
    /// <summary>Envelope tag and format version. Bumped only on a breaking format change.</summary>
    public const int FormatVersion = 1;

    /// <summary>Envelope field: format version.</summary>
    public const string EnvVersion = "jc";

    /// <summary>Envelope field: key catalogue, an array of the original key names.</summary>
    public const string EnvKeys = "k";

    /// <summary>Envelope field: string-value catalogue. Omitted when empty.</summary>
    public const string EnvStrings = "s";

    /// <summary>Envelope field: the encoded document.</summary>
    public const string EnvData = "d";

    /// <summary>Table node: column list, as indices into the key catalogue.</summary>
    public const string TableColumns = "~t";

    /// <summary>Table node: per-column encoding. Omitted when every column is raw.</summary>
    public const string TableEncodings = "~e";

    /// <summary>Table node: values of the constant columns, in column order.</summary>
    public const string TableConstants = "~cv";

    /// <summary>Table node: the rows, each an array of cells for the non-constant columns.</summary>
    public const string TableRows = "~r";

    /// <summary>Table node: row count. Only present when every column is constant.</summary>
    public const string TableRowCount = "~n";

    /// <summary>Cell marker for "this row does not have this key at all".</summary>
    public const string Hole = "~";

    /// <summary>Dictionary-column equivalent of <see cref="Hole"/>.</summary>
    public const int DictionaryHole = -1;

    /// <summary>Column encoding: cells are ordinary encoded values.</summary>
    public const int ColumnRaw = 0;

    /// <summary>Column encoding: cells are integer indices into the string catalogue.</summary>
    public const int ColumnDictionary = 1;

    /// <summary>Column encoding: the column is constant; its value lives in <see cref="TableConstants"/>.</summary>
    public const int ColumnConstant = 2;

    /// <summary>The one reserved character.</summary>
    public const char Tilde = '~';

    /// <summary>
    /// How deep a document may nest before the walk gives up.
    /// </summary>
    /// <remarks>
    /// A recursive descent over an attacker-supplied document would otherwise
    /// end in a stack overflow, which .NET cannot catch. This is well past
    /// anything a real document needs, and eight times the depth
    /// <see cref="System.Text.Json.JsonDocumentOptions"/> allows by default.
    /// </remarks>
    public const int MaxDepth = 512;
}
