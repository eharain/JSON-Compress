using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// The public surface: the helpers around the codec, and the way it behaves at
/// the edges - bad input, values JSON itself cannot carry, documents from
/// another version of the format.
/// </summary>
public class ApiTests
{
    [Fact]
    public void TheEnvelopeCarriesAVersionAndAKeyCatalogue()
    {
        var envelope = JsonCompressor.Compress(Doc.Parse("{\"hello\":\"world\"}"));
        Assert.Equal(JsonCompressor.FormatVersion, envelope["jc"]!.GetValue<int>());
        Assert.Equal("[\"hello\"]", Doc.Text(envelope["k"]));
        Assert.True(JsonCompressor.IsCompressed(envelope));
    }

    [Fact]
    public void TheStringCatalogueIsLeftOutWhenNothingIsWorthCataloguing()
    {
        Assert.False(JsonCompressor.Compress(Doc.Parse("{\"a\":1}")).ContainsKey("s"));
    }

    [Theory]
    [InlineData("null")]
    [InlineData("0")]
    [InlineData("\"x\"")]
    [InlineData("[]")]
    [InlineData("{}")]
    [InlineData("{\"jc\":2,\"k\":[]}")]
    [InlineData("{\"jc\":1}")]
    public void IsCompressedRejectsEverythingThatIsNotAnEnvelope(string json)
    {
        Assert.False(JsonCompressor.IsCompressed(Doc.Parse(json)));
    }

    [Fact]
    public void DecompressIfNeededPassesOrdinaryValuesStraightThrough()
    {
        var plain = Doc.Parse("{\"a\":1}");
        Assert.Equal("{\"a\":1}", Doc.Text(JsonCompressor.DecompressIfNeeded(plain)));
        Assert.Equal("{\"a\":1}", Doc.Text(JsonCompressor.DecompressIfNeeded(JsonCompressor.Compress(plain))));
    }

    [Theory]
    [InlineData("null", "envelope object")]
    [InlineData("[1,2]", "envelope object")]
    [InlineData("{\"k\":[]}", "not a compressed document")]
    [InlineData("{\"jc\":99,\"k\":[]}", "version 99 is not supported")]
    [InlineData("{\"jc\":1}", "key catalogue")]
    [InlineData("{\"jc\":1,\"k\":[],\"s\":\"nope\"}", "string catalogue")]
    public void DecompressRefusesDocumentsItCannotRead(string json, string because)
    {
        var error = Assert.Throws<JsonCompressException>(() => JsonCompressor.Decompress(Doc.Parse(json)));
        Assert.Contains(because, error.Message, StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("{\"jc\":1,\"k\":[],\"d\":{\"A\":1}}", "key token")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"s\":[],\"d\":\"~5\"}", "string reference")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":\"~\"}", "hole marker")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"~t\":\"x\"}}", "column list")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"~t\":[7]}}", "column index")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"~t\":[0],\"~e\":[1,1],\"~r\":[]}}", "encoding list")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"~t\":[0],\"~e\":[2],\"~n\":2}}", "fewer constants")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"s\":[],\"d\":{\"~t\":[0],\"~e\":[1],\"~r\":[[4]]}}", "dictionary index")]
    [InlineData("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"~t\":[0],\"~r\":[1]}}", "is not an array")]
    [InlineData("{\"jc\":1,\"k\":[1],\"d\":null}", "not a string")]
    public void DecompressRefusesReferencesThatPointNowhere(string json, string because)
    {
        var error = Assert.Throws<JsonCompressException>(() => JsonCompressor.Decompress(Doc.Parse(json)));
        Assert.Contains(because, error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void ATokenTooLargeToBeAnIndexIsRefusedRatherThanWrappingAround()
    {
        // Twelve base-62 digits overflow a 32-bit index. The decoder has to
        // notice that rather than folding it into something in range.
        var error = Assert.Throws<JsonCompressException>(
            () => JsonCompressor.Decompress(Doc.Parse("{\"jc\":1,\"k\":[\"a\"],\"d\":{\"Azzzzzzzzzzzz\":1}}")));
        Assert.Contains("key token", error.Message, StringComparison.Ordinal);
    }

    [Fact]
    public void ASharedReferenceIsNotMistakenForACycle()
    {
        var shared = Doc.Parse("{\"a\":1}");
        var value = new JsonObject { ["x"] = shared!.DeepClone(), ["y"] = shared.DeepClone() };
        Assert.Equal("{\"x\":{\"a\":1},\"y\":{\"a\":1}}", Doc.Text(JsonCompressor.Decompress(JsonCompressor.Compress(value))));
    }

    [Fact]
    public void ValuesJsonCannotCarryAreTreatedTheWayJsonTreatsThem()
    {
        var value = new Awkward
        {
            When = new DateTimeOffset(2026, 8, 18, 10, 20, 30, TimeSpan.Zero),
            Nan = double.NaN,
            Inf = double.PositiveInfinity,
            List = new List<double> { double.NaN, 1 },
        };

        var restored = JsonCompressor.Decompress(JsonCompressor.CompressValue(value));
        Assert.Equal(
            "{\"When\":\"2026-08-18T10:20:30+00:00\",\"Nan\":null,\"Inf\":null,\"List\":[null,1]}",
            Doc.Text(restored));
    }

    [Fact]
    public void ATypedValueCanBeCompressedAndRestored()
    {
        var people = new List<Person>
        {
            new Person { Id = 1, Name = "Ada", Role = "admin" },
            new Person { Id = 2, Name = "Bob", Role = "admin" },
            new Person { Id = 3, Name = "Cy", Role = "user" },
        };

        var text = JsonCompressor.StringifyValue(people);
        var restored = JsonCompressor.ParseValue<List<Person>>(text);

        Assert.NotNull(restored);
        Assert.Equal(3, restored!.Count);
        Assert.Equal("Cy", restored[2].Name);
        Assert.Equal("admin", restored[0].Role);
    }

    [Fact]
    public void OptionsAreValidated()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new CompressOptions { MinRows = 0 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new CompressOptions { MaxHoleRatio = 2 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new CompressOptions { MinStringCount = 1 });
        Assert.Throws<ArgumentOutOfRangeException>(() => new CompressOptions { MinStringLength = -1 });
    }

    [Fact]
    public void TheDefaultsAreWhatTheSpecificationSays()
    {
        var defaults = CompressOptions.Default();
        Assert.True(defaults.Strings);
        Assert.True(defaults.Tables);
        Assert.True(defaults.Constants);
        Assert.True(defaults.DictionaryColumns);
        Assert.Equal(2, defaults.MinRows);
        Assert.Equal(0.3, defaults.MaxHoleRatio);
        Assert.Equal(2, defaults.MinStringCount);
        Assert.Equal(1, defaults.MinStringLength);
    }

    [Fact]
    public void StringifyAndParseAreTheStringLevelPair()
    {
        var value = Doc.Parse("{\"a\":[1,2,3]}");
        var text = JsonCompressor.Stringify(value);
        Assert.Equal("{\"a\":[1,2,3]}", Doc.Text(JsonCompressor.Parse(text)));
    }

    [Fact]
    public void MinifyAndBeautifyOnlyTouchWhitespace()
    {
        Assert.Equal("{\"a\":1,\"b\":[1,2]}", JsonCompressor.Minify("{\n  \"a\" : 1,\n  \"b\" : [1, 2]\n}"));
        Assert.Equal("{\n  \"a\": 1\n}", Normalise(JsonCompressor.Beautify("{\"a\":1}")));
        Assert.Equal("{\n    \"a\": 1\n}", Normalise(JsonCompressor.Beautify("{\"a\":1}", 4)));
        Assert.Equal("{\"a\":1}", JsonCompressor.Beautify("{\"a\":1}", 0));
    }

    [Fact]
    public void BeautifyKeepsTextInsideStringsIntact()
    {
        const string Json = "{\"a\":\"  two leading spaces\",\"b\":[{\"c\":1}]}";
        var pretty = JsonCompressor.Beautify(Json, 3);
        Assert.Equal(Json, JsonCompressor.Minify(pretty));
        Assert.Contains("   \"a\"", pretty, StringComparison.Ordinal);
    }

    [Fact]
    public void ValidateReportsWhereADocumentGoesWrong()
    {
        Assert.True(JsonCompressor.Validate("{\"a\":1}").Valid);

        var bad = JsonCompressor.Validate("{\n  \"a\": 1,\n  \"b\": oops\n}");
        Assert.False(bad.Valid);
        Assert.Equal(3, bad.Line);
        Assert.Equal(8, bad.Column);
        Assert.Equal("  \"b\": oops", bad.Excerpt);
    }

    [Fact]
    public void MeasureReportsARealSavingOnARecordSet()
    {
        var value = new JsonArray();
        for (var i = 0; i < 100; i++)
        {
            value.Add(new JsonObject
            {
                ["id"] = i,
                ["firstName"] = "Given" + i.ToString(System.Globalization.CultureInfo.InvariantCulture),
                ["lastName"] = "Family",
                ["department"] = i % 2 == 1 ? "engineering" : "operations",
            });
        }

        var report = JsonCompressor.Measure(value);
        Assert.Equal(Bytes.ByteLength(Doc.Text(value)), report.Minified);
        Assert.True(report.Compressed < report.Minified);
        Assert.Equal(report.Minified - report.Compressed, report.Saved);
        Assert.True(report.Percent > 40, "expected better than 40%, got " + report.Percent);
        Assert.True(report.Ratio < 1);
        Assert.NotNull(report.GzipMinified);
        Assert.NotNull(report.GzipCompressed);
    }

    [Fact]
    public void MeasureCanBeToldWhatTheInputReallyWeighed()
    {
        var report = JsonCompressor.Measure(Doc.Parse("{\"a\":1}"), source: "{\n  \"a\": 1\n}", gzip: false);
        Assert.Equal(12, report.Original);
        Assert.Equal(7, report.Minified);
        Assert.Null(report.GzipMinified);
    }

    [Fact]
    public void GzipRoundTrips()
    {
        const string Text = "{\"hello\":\"world\",\"list\":[1,2,3]}";
        var bytes = Bytes.GZip(Text);
        Assert.Equal(Text, Bytes.GUnzip(bytes));
        Assert.Equal(bytes.Length, Bytes.GZipSize(Text));
    }

    [Theory]
    [InlineData("abc", 3)]
    [InlineData("£", 2)]
    [InlineData("😀", 4)]
    public void ByteLengthCountsUtf8BytesNotCodeUnits(string text, int expected)
    {
        Assert.Equal(expected, Bytes.ByteLength(text));
    }

    [Fact]
    public void ACodecBindsItsOptionsOnce()
    {
        var codec = new JsonCompressCodec(new CompressOptions { Tables = false });
        var value = Doc.Parse("[{\"a\":1},{\"a\":2},{\"a\":3}]");

        Assert.DoesNotContain("~t", Doc.Text(codec.Compress(value)), StringComparison.Ordinal);
        Assert.Equal(Doc.Text(value), Doc.Text(codec.Decompress(codec.Compress(value))));
        Assert.Equal(Doc.Text(value), Doc.Text(codec.Parse(codec.Stringify(value))));
        Assert.False(codec.Options().Tables);

        // The codec took a copy, so nothing a caller does afterwards reaches it.
        var borrowed = codec.Options();
        borrowed.Tables = true;
        Assert.False(codec.Options().Tables);
    }

    [Fact]
    public void NonAsciiTextIsWrittenAsItselfRatherThanAsEscapes()
    {
        var text = JsonCompressor.Stringify(Doc.Parse("{\"a\":\"café\"}"));
        Assert.Contains("café", text, StringComparison.Ordinal);
    }

    [Fact]
    public void ADocumentNestedPastTheLimitIsRefusedRatherThanOverflowingTheStack()
    {
        var value = new JsonArray();
        var deepest = value;
        for (var i = 0; i < Wire.MaxDepth + 10; i++)
        {
            var next = new JsonArray();
            deepest.Add(next);
            deepest = next;
        }

        Assert.Throws<JsonCompressException>(() => JsonCompressor.Compress(value));
    }

    [Fact]
    public void TheEncoderNeverTouchesTheDocumentItWasGiven()
    {
        var value = Doc.Parse("[{\"a\":1,\"b\":\"x\"},{\"a\":2,\"b\":\"x\"}]");
        var before = Doc.Text(value);
        JsonCompressor.Compress(value);
        Assert.Equal(before, Doc.Text(value));
    }

    private static string Normalise(string text)
    {
        return text.Replace("\r\n", "\n", StringComparison.Ordinal);
    }

    private sealed class Person
    {
        public int Id { get; set; }

        public string Name { get; set; } = string.Empty;

        public string Role { get; set; } = string.Empty;
    }

    private sealed class Awkward
    {
        public DateTimeOffset When { get; set; }

        public double Nan { get; set; }

        public double Inf { get; set; }

        public List<double> List { get; set; } = new List<double>();
    }
}
