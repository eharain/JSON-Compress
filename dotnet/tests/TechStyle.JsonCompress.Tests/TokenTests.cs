using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// The tokens and the column ordering - the two pieces of the format that are
/// pure arithmetic, and the two most likely to differ between ports.
/// </summary>
public class TokenTests
{
    [Theory]
    [InlineData(0)]
    [InlineData(1)]
    [InlineData(61)]
    [InlineData(62)]
    [InlineData(63)]
    [InlineData(3843)]
    [InlineData(3844)]
    [InlineData(238327)]
    [InlineData(238328)]
    [InlineData(1000000)]
    public void Base62TokensRoundTrip(int n)
    {
        var token = Tokens.ToBase62(n);
        Assert.Equal(n, Tokens.FromBase62(token));
        Assert.Equal(token.Length, Tokens.Base62Length(n));
    }

    [Theory]
    [InlineData("")]
    [InlineData("~")]
    [InlineData("a-b")]
    [InlineData("é")]
    public void Base62RejectsWhatIsNotAToken(string token)
    {
        Assert.Equal(-1, Tokens.FromBase62(token));
    }

    [Fact]
    public void KeyTokensRoundTripAndNeverLookLikeArrayIndices()
    {
        for (var n = 0; n < 20000; n++)
        {
            var token = Tokens.ToKeyToken(n);
            Assert.Equal(n, Tokens.FromKeyToken(token));
            Assert.True(char.IsLetter(token[0]), "token " + token + " starts with a letter");
            Assert.False(
                int.TryParse(token, out _),
                "token " + token + " looks numeric and would be reordered by a JavaScript object");
        }
    }

    [Theory]
    [InlineData("")]
    [InlineData("0")]
    [InlineData("~")]
    [InlineData("A~")]
    public void KeyTokensRejectWhatIsNotOne(string token)
    {
        Assert.Equal(-1, Tokens.FromKeyToken(token));
    }

    [Fact]
    public void AKeyTokenTooLongToBeAnIndexIsRefused()
    {
        // Anything past a 32-bit index has to come back as "not a token" rather
        // than wrapping around into something that looks addressable.
        Assert.Equal(-1, Tokens.FromKeyToken("Azzzzzzzzzzzzzzzz"));
        Assert.Equal(-1, Tokens.FromBase62("zzzzzzzzzzzzzzzz"));
    }

    [Theory]
    [InlineData(0, 1)]
    [InlineData(51, 1)]
    [InlineData(52, 2)]
    [InlineData(3275, 2)]
    [InlineData(3276, 3)]
    public void KeyTokensAreCompactForTheSizesThatMatter(int index, int length)
    {
        Assert.Equal(length, Tokens.ToKeyToken(index).Length);
    }

    [Theory]
    [InlineData("[{\"a\":1,\"b\":2},{\"a\":3,\"b\":4}]", "a,b")]
    [InlineData("[{\"a\":1,\"b\":2},{\"b\":3,\"c\":4}]", "a,b,c")]

    // The rows disagree about where c goes, but an order exists that suits both.
    [InlineData("[{\"a\":1,\"b\":2},{\"c\":3,\"a\":4}]", "c,a,b")]
    public void ColumnOrderingRespectsEveryRow(string json, string expected)
    {
        var rows = ((JsonArray)Doc.Parse(json)!).Select(row => (JsonObject)row!).ToList();
        Assert.Equal(expected, string.Join(",", ColumnOrder.Order(rows)!));
    }

    [Fact]
    public void ColumnOrderingGivesUpWhenTheRowsContradictEachOther()
    {
        var rows = ((JsonArray)Doc.Parse("[{\"a\":1,\"b\":2},{\"b\":3,\"a\":4}]")!)
            .Select(row => (JsonObject)row!)
            .ToList();

        Assert.Null(ColumnOrder.Order(rows));
    }
}
