using System.Text.Json;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// Saying where a document stops being JSON.
/// </summary>
public class LocatorTests
{
    [Theory]
    [InlineData("{'a': 1}", "double quotes", 1, 2)]
    [InlineData("{\"a\": 1,}", "trailing comma", 1, 8)]
    [InlineData("[1, 2,]", "trailing comma", 1, 6)]
    [InlineData("{\"a\": True}", "lower case", 1, 7)]
    [InlineData("{\"a\": None}", "use null", 1, 7)]
    [InlineData("{\"a\": 01}", "leading zero", 1, 7)]
    [InlineData("{\"a\": 1.}", "decimal point", 1, 8)]
    [InlineData("{\"a\": 1e}", "exponent", 1, 8)]
    [InlineData("{\"a\": \"unclosed", "never closed", 1, 7)]
    [InlineData("{\"a\": 1", "never closed", 1, 8)]
    [InlineData("[1, 2", "never closed", 1, 6)]
    [InlineData("{a: 1}", "quoted property name", 1, 2)]
    [InlineData("{\"a\" 1}", "colon", 1, 6)]
    [InlineData("{\"a\": 1} extra", "after the end", 1, 10)]
    [InlineData("", "ends here", 1, 1)]
    [InlineData("{\"a\": \"\\q\"}", "not a valid escape", 1, 8)]
    [InlineData("{\"a\": \"\\u12\"}", "four hexadecimal", 1, 8)]
    public void LocateNamesTheCommonMistakes(string text, string because, int line, int column)
    {
        var found = JsonCompressor.Locate(text);
        Assert.False(found.Valid, text + " should be invalid");
        Assert.Contains(because, found.Error!, StringComparison.Ordinal);
        Assert.Equal(line, found.Line);
        Assert.Equal(column, found.Column);
    }

    [Theory]
    [InlineData("{}")]
    [InlineData("[]")]
    [InlineData("null")]
    [InlineData("true")]
    [InlineData("false")]
    [InlineData("0")]
    [InlineData("-0")]
    [InlineData("1e10")]
    [InlineData("-1.5E-3")]
    [InlineData("\"x\"")]
    [InlineData("  {  \"a\" : [ 1 , 2 ] }  ")]
    [InlineData("{\"a\":{\"b\":{\"c\":[]}}}")]
    [InlineData("\"\\u00e9\\n\\t\\\\\"")]
    [InlineData("{\"\": 1}")]
    [InlineData("[[[[[1]]]]]")]
    [InlineData("{\"a\": 1e+10}")]
    [InlineData("\"😀\"")]
    public void LocateAgreesWithTheParserAboutWhatIsValid(string text)
    {
        Assert.True(JsonCompressor.Locate(text).Valid, text + " should be valid");
        using (JsonDocument.Parse(text))
        {
        }
    }

    [Fact]
    public void LocateFindsTheSameLineOnAMultiLineDocument()
    {
        var text = string.Join("\n", "{", "  \"one\": 1,", "  \"two\": [", "    1,", "    2,", "  ]", "}");
        var found = JsonCompressor.Locate(text);

        Assert.False(found.Valid);
        Assert.Contains("trailing comma", found.Error!, StringComparison.Ordinal);
        Assert.Equal(5, found.Line);
    }

    [Fact]
    public void ALocationDescribesItselfInOneLine()
    {
        var found = JsonCompressor.Locate("{\"a\": oops}");
        Assert.Equal("line 1, column 7: expected a value, found \"o\"", found.ToString());
        Assert.Equal("valid JSON", JsonCompressor.Locate("{}").ToString()!);
    }

    [Fact]
    public void ValidateFallsBackToTheParserVerdict()
    {
        // Nested past what the parser will follow. The grammar the scan checks
        // has no such limit, so it finds nothing wrong - and the verdict has to
        // come from the parser rather than from the scan.
        const int Levels = 600;
        var text = new string('[', Levels) + new string(']', Levels);

        Assert.True(JsonCompressor.Locate(text).Valid);

        var found = JsonCompressor.Validate(text);
        Assert.False(found.Valid);
        Assert.False(string.IsNullOrEmpty(found.Error));
    }

    [Fact]
    public void ValidateAgreesWithTheScanWhenTheScanFindsSomething()
    {
        var found = JsonCompressor.Validate("{\"a\": 1,}");
        Assert.False(found.Valid);
        Assert.Contains("trailing comma", found.Error!, StringComparison.Ordinal);
        Assert.Equal(8, found.Column);
    }
}
