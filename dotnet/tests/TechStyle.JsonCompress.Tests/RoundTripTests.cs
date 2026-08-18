using System.Text;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// Round-trip tests.
/// </summary>
/// <remarks>
/// The contract the whole library rests on is that decompressing a compressed
/// document gives back the document - not something equivalent, the same
/// document, keys in the same order. Everything here checks that, on shapes
/// chosen to poke at the places where the format could plausibly lose
/// information.
/// </remarks>
public class RoundTripTests
{
    [Theory]
    [InlineData("null")]
    [InlineData("true")]
    [InlineData("false")]
    [InlineData("0")]
    [InlineData("-1")]
    [InlineData("1.5")]
    [InlineData("1e21")]
    [InlineData("-1e-7")]
    [InlineData("123456789012345678901234567890")]
    [InlineData("\"\"")]
    [InlineData("\"plain\"")]
    public void ScalarsSurviveIntact(string json)
    {
        Doc.RoundTrips(json);
    }

    [Theory]
    [InlineData("\"~\"")]
    [InlineData("\"~~\"")]
    [InlineData("\"~~~\"")]
    [InlineData("\"~0\"")]
    [InlineData("\"~z\"")]
    [InlineData("\"~ref\"")]
    [InlineData("\"~t\"")]
    [InlineData("\"~n\"")]
    [InlineData("\"a~b\"")]
    public void TheTildeEscapeIsReversible(string json)
    {
        Doc.RoundTrips(json);
    }

    [Fact]
    public void RepeatedTildeStringsSurvive()
    {
        Doc.RoundTrips("[\"~\",\"~\",\"~\",\"~0\",\"~0\",\"~0\"]");
        Doc.RoundTrips("{\"~key\":\"~value\"}");
    }

    [Theory]
    [InlineData("[]")]
    [InlineData("{}")]
    [InlineData("[[],{},[[]],[{}]]")]
    [InlineData("[{},{},{}]")]
    public void EmptyContainersSurvive(string json)
    {
        Doc.RoundTrips(json);
    }

    [Fact]
    public void ArraysOfLikeShapedObjectsBecomeTablesAndComeBack()
    {
        const string Json = """
            [{"id":1,"name":"Ada","role":"admin"},
             {"id":2,"name":"Bob","role":"admin"},
             {"id":3,"name":"Cy","role":"user"}]
            """;

        Doc.RoundTrips(Json);
        Assert.Contains("~t", Doc.Compressed(Json), StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("[{\"a\":1},{\"b\":2},{\"a\":3,\"b\":4}]")]
    [InlineData("[{\"a\":1,\"b\":2,\"c\":3},{\"a\":4},{\"a\":5,\"c\":6}]")]
    public void RowsWithDifferentKeySetsSurvive(string json)
    {
        Doc.RoundTrips(json);
    }

    [Fact]
    public void AMissingKeyIsNotTheSameAsANullOne()
    {
        var restored = (JsonArray)Doc.RoundTrips(
            "[{\"a\":1,\"b\":null},{\"a\":2},{\"a\":3,\"b\":null},{\"a\":4,\"b\":5}]")!;

        var first = (JsonObject)restored[0]!;
        var second = (JsonObject)restored[1]!;

        Assert.True(first.ContainsKey("b"), "an explicit null is kept");
        Assert.Null(first["b"]);
        Assert.False(second.ContainsKey("b"), "an absent key stays absent");
    }

    [Fact]
    public void AMissingKeyIsNotTheSameAsANullOneInADictionaryColumn()
    {
        var restored = (JsonArray)Doc.RoundTrips(
            """
            [{"k":"x","n":1},{"k":null,"n":2},{"n":3},
             {"k":"x","n":4},{"k":"y","n":5},{"k":"x","n":6}]
            """)!;

        Assert.True(((JsonObject)restored[1]!).ContainsKey("k"));
        Assert.Null(((JsonObject)restored[1]!)["k"]);
        Assert.False(((JsonObject)restored[2]!).ContainsKey("k"));
    }

    [Fact]
    public void ConstantColumnsAreRestoredOnEveryRow()
    {
        const string Json = """
            [{"type":"order","id":1},{"type":"order","id":2},{"type":"order","id":3}]
            """;

        var compressed = Doc.Compressed(Json);
        Assert.Contains("~cv", compressed, StringComparison.Ordinal);

        // The constant is written down exactly once.
        Assert.Equal(1, Occurrences(compressed, "order"));
        Doc.RoundTrips(Json);
    }

    [Fact]
    public void ATableWhereEveryColumnIsConstantKeepsItsRowCount()
    {
        const string Json = """
            [{"a":1,"b":"x"},{"a":1,"b":"x"},{"a":1,"b":"x"},{"a":1,"b":"x"}]
            """;

        Assert.Contains("~n", Doc.Compressed(Json), StringComparison.Ordinal);
        Doc.RoundTrips(Json);
    }

    [Fact]
    public void NestedTablesSurvive()
    {
        Doc.RoundTrips(
            """
            {"groups":[
              {"name":"a","items":[{"q":1,"p":2},{"q":3,"p":4}]},
              {"name":"b","items":[{"q":5,"p":6},{"q":7,"p":8}]}]}
            """);
    }

    [Theory]
    [InlineData("[[1,2],[3,4],[5,6]]")]
    [InlineData("[[{\"a\":1},{\"a\":2}],[{\"a\":3},{\"a\":4}]]")]
    public void ArraysOfArraysAreLeftAlone(string json)
    {
        Doc.RoundTrips(json);
    }

    [Theory]
    [InlineData("[1,\"two\",null,{\"three\":3},[4]]")]
    [InlineData("[{\"a\":1},2,{\"a\":3}]")]
    public void MixedArraysAreLeftAlone(string json)
    {
        Doc.RoundTrips(json);
    }

    [Fact]
    public void DeepNestingSurvives()
    {
        var builder = new StringBuilder();
        const int Levels = 200;
        for (var i = 0; i < Levels; i++)
        {
            builder.Append("{\"level").Append(i).Append("\":");
        }

        builder.Append("{\"leaf\":true}");
        builder.Append('}', Levels);

        Doc.RoundTrips(builder.ToString());
    }

    [Fact]
    public void RowsWhoseKeyOrdersDisagreeStayAnArrayOfObjects()
    {
        const string Json = "[{\"a\":1,\"b\":2},{\"b\":3,\"a\":4}]";
        Assert.DoesNotContain("~t", Doc.Compressed(Json), StringComparison.Ordinal);
        Doc.RoundTrips(Json);
    }

    [Fact]
    public void RepeatedStringsAreCataloguedAndRestored()
    {
        var rows = new JsonArray();
        for (var i = 0; i < 50; i++)
        {
            rows.Add(new JsonObject
            {
                ["id"] = i,
                ["country"] = i % 2 == 1 ? "United Kingdom" : "Republic of Ireland",
                ["note"] = "a reasonably long repeated note value",
            });
        }

        var envelope = JsonCompressor.Compress(rows);
        var catalogue = (JsonArray)envelope["s"]!;
        Assert.Contains(catalogue, entry => entry!.GetValue<string>() == "United Kingdom");

        Assert.Equal(Doc.Text(rows), Doc.Text(JsonCompressor.Decompress(envelope)));

        // The note never changes, so it is hoisted out of the rows entirely and
        // written down exactly once.
        Assert.Equal(1, Occurrences(Doc.Text(envelope), "a reasonably long repeated note value"));
    }

    [Fact]
    public void UnicodeAndEscapesSurvive()
    {
        Doc.RoundTrips(
            """
            {"emoji":"compressed \ud83d\udce6 nicely",
             "quote":"she said \"hello\"",
             "backslash":"C:\\Users\\path",
             "newline":"one\ntwo\tthree",
             "surrogate":"\ud83d\ude00\ud83d\ude01",
             "arabic":"مرحبا بالعالم",
             "nul":"a\u0000b"}
            """);
    }

    [Fact]
    public void KeysThatLookLikeTokensAreNotConfusedWithTokens()
    {
        Doc.RoundTrips("{\"0\":1,\"1\":2,\"a\":3,\"z\":4,\"10\":5,\"~t\":6,\"~r\":7}");
    }

    [Fact]
    public void AKeyCalledProtoSurvivesAsARealKey()
    {
        Doc.RoundTrips("{\"__proto__\":1,\"safe\":2}");
        Doc.RoundTrips("[{\"__proto__\":1,\"a\":2},{\"__proto__\":3,\"a\":4},{\"__proto__\":5,\"a\":6}]");
    }

    [Fact]
    public void ALargeKeySetPushesTokensPastOneCharacter()
    {
        var value = new JsonObject();
        for (var i = 0; i < 5000; i++)
        {
            value["key_number_" + i.ToString(System.Globalization.CultureInfo.InvariantCulture)] = i;
        }

        Doc.RoundTrips(value);
    }

    [Fact]
    public void ALargeStringCataloguePushesReferencesPastOneCharacter()
    {
        var value = new JsonArray();
        for (var pass = 0; pass < 2; pass++)
        {
            for (var i = 0; i < 500; i++)
            {
                value.Add("a distinctive value number " + i.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }
        }

        Doc.RoundTrips(value);
    }

    [Fact]
    public void EveryCombinationOfOptionsRoundTrips()
    {
        var rows = new JsonArray();
        for (var i = 0; i < 20; i++)
        {
            var row = new JsonObject
            {
                ["id"] = i,
                ["status"] = i % 2 == 1 ? "open" : "closed",
                ["fixed"] = "always the same",
            };

            if (i % 5 != 0)
            {
                row["note"] = "note " + (i % 3).ToString(System.Globalization.CultureInfo.InvariantCulture);
            }

            rows.Add(row);
        }

        var value = new JsonObject { ["rows"] = rows };

        foreach (var strings in new[] { true, false })
        {
            foreach (var tables in new[] { true, false })
            {
                foreach (var constants in new[] { true, false })
                {
                    foreach (var dictionaries in new[] { true, false })
                    {
                        var options = new CompressOptions
                        {
                            Strings = strings,
                            Tables = tables,
                            Constants = constants,
                            DictionaryColumns = dictionaries,
                        };

                        var restored = JsonCompressor.Decompress(JsonCompressor.Compress(value, options));
                        Assert.Equal(Doc.Text(value), Doc.Text(restored));
                    }
                }
            }
        }
    }

    [Fact]
    public void MinRowsControlsWhenATableIsBuilt()
    {
        const string Json = "[{\"a\":1},{\"a\":2}]";
        Assert.Contains("~t", Doc.Compressed(Json, new CompressOptions { MinRows = 2 }), StringComparison.Ordinal);
        Assert.DoesNotContain("~t", Doc.Compressed(Json, new CompressOptions { MinRows = 3 }), StringComparison.Ordinal);
        Doc.RoundTrips(Json, new CompressOptions { MinRows = 3 });
    }

    [Fact]
    public void MaxHoleRatioKeepsVerySparseArraysOutOfTables()
    {
        const string Json = "[{\"a\":1},{\"b\":2},{\"c\":3},{\"d\":4},{\"e\":5}]";
        Assert.DoesNotContain("~t", Doc.Compressed(Json), StringComparison.Ordinal);
        Doc.RoundTrips(Json);
        Doc.RoundTrips(Json, new CompressOptions { MaxHoleRatio = 1 });
    }

    [Fact]
    public void NonFiniteNumbersBecomeNullAsTheyDoInAnyJson()
    {
        var value = new JsonObject
        {
            ["nan"] = JsonValue.Create(double.NaN),
            ["inf"] = JsonValue.Create(double.PositiveInfinity),
            ["minusInf"] = JsonValue.Create(double.NegativeInfinity),
            ["minusZero"] = JsonValue.Create(-0.0),
            ["fine"] = JsonValue.Create(1.5),
        };

        var restored = JsonCompressor.Decompress(JsonCompressor.Compress(value));
        Assert.Equal("{\"nan\":null,\"inf\":null,\"minusInf\":null,\"minusZero\":0,\"fine\":1.5}", Doc.Text(restored));
    }

    private static int Occurrences(string haystack, string needle)
    {
        var count = 0;
        var at = haystack.IndexOf(needle, StringComparison.Ordinal);
        while (at >= 0)
        {
            count++;
            at = haystack.IndexOf(needle, at + needle.Length, StringComparison.Ordinal);
        }

        return count;
    }
}
