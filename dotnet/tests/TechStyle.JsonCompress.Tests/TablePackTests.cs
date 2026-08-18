using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// The standalone table pack, and the tokens the format is built on.
/// </summary>
public class TablePackTests
{
    [Fact]
    public void PackFlattensRecordsAndUnpackRestoresThem()
    {
        var records = Doc.Parse("[{\"id\":1,\"name\":\"Ada\"},{\"id\":2,\"name\":\"Bob\"}]");
        var packed = TablePack.Pack(records);

        Assert.Equal(
            "{\"fields\":[\"id\",\"name\"],\"data\":[[1,\"Ada\"],[2,\"Bob\"]],\"count\":2}",
            Doc.Text(packed));

        Assert.Equal(Doc.Text(records), Doc.Text(TablePack.Unpack(packed)));
    }

    [Fact]
    public void PackFillsAMissingFieldWithNull()
    {
        var packed = TablePack.Pack(Doc.Parse("[{\"a\":1},{\"b\":2}]"));
        Assert.Equal("{\"fields\":[\"a\",\"b\"],\"data\":[[1,null],[null,2]],\"count\":2}", Doc.Text(packed));
    }

    [Theory]
    [InlineData("null")]
    [InlineData("\"text\"")]
    [InlineData("42")]
    [InlineData("[1,{\"a\":2}]")]
    public void PackLeavesAnythingThatIsNotARecordSetAlone(string json)
    {
        Assert.Equal(json, Doc.Text(TablePack.Pack(Doc.Parse(json))));
    }

    [Fact]
    public void PackingAnEmptyArrayGivesAnEmptyTable()
    {
        Assert.Equal("{\"fields\":[],\"data\":[],\"count\":0}", Doc.Text(TablePack.Pack(Doc.Parse("[]"))));
    }

    [Theory]
    [InlineData("null")]
    [InlineData("42")]
    [InlineData("{\"fields\":\"no\",\"data\":[]}")]
    public void UnpackLeavesAnythingThatIsNotAPackedTableAlone(string json)
    {
        Assert.Equal(json, Doc.Text(TablePack.Unpack(Doc.Parse(json))));
    }

    [Fact]
    public void UnpackingAnEmptyTableGivesAnEmptyArray()
    {
        Assert.Equal("[]", Doc.Text(TablePack.Unpack(Doc.Parse("{\"fields\":[],\"data\":[],\"count\":0}"))));
    }

    [Fact]
    public void IsPackedRecognisesTheShape()
    {
        Assert.True(TablePack.IsPacked(TablePack.Pack(Doc.Parse("[{\"a\":1}]"))));
        Assert.False(TablePack.IsPacked(Doc.Parse("[{\"a\":1}]")));
        Assert.False(TablePack.IsPacked(null));
    }

    [Fact]
    public void PackAllAndUnpackAllHandleSeveralResultSets()
    {
        var sets = Doc.Parse("[[{\"a\":1},{\"a\":2}],[{\"b\":3}]]");
        var packed = (JsonArray)TablePack.PackAll(sets)!;

        Assert.Equal(2, packed.Count);
        Assert.Equal(2, ((JsonObject)packed[0]!)["count"]!.GetValue<int>());
        Assert.Equal(Doc.Text(sets), Doc.Text(TablePack.UnpackAll(packed)));
    }

    [Fact]
    public void TextToArrayReadsABracketedIntegerColumn()
    {
        Assert.Equal(
            "[1,2,3]",
            Doc.Text(TablePack.TextToArray((JsonObject)Doc.Parse("{\"ids\":\"[[[1,2,3]]]\"}")!, "ids")["ids"]));

        Assert.Equal(
            "[]",
            Doc.Text(TablePack.TextToArray((JsonObject)Doc.Parse("{\"ids\":null}")!, "ids")["ids"]));

        Assert.Equal(
            "[1,2]",
            Doc.Text(TablePack.TextToArray((JsonObject)Doc.Parse("{\"ids\":\"{1|2}\"}")!, "ids", 1, 1, "|")["ids"]));

        Assert.Equal(
            "{\"other\":1}",
            Doc.Text(TablePack.TextToArray((JsonObject)Doc.Parse("{\"other\":1}")!, "ids")));
    }

    [Fact]
    public void PackingIsNotFooledByRowsWhoseKeyOrdersDisagree()
    {
        // The pack has one field list, so a disagreement over key order is
        // simply resolved in favour of first appearance; nothing is lost
        // because every row carries a cell for every field.
        var packed = TablePack.Pack(Doc.Parse("[{\"a\":1,\"b\":2},{\"b\":3,\"a\":4}]"));
        Assert.Equal("{\"fields\":[\"a\",\"b\"],\"data\":[[1,2],[4,3]],\"count\":2}", Doc.Text(packed));
    }

    [Fact]
    public void PackedResultsAreDetachedFromTheInput()
    {
        var records = (JsonArray)Doc.Parse("[{\"a\":{\"deep\":1}}]")!;
        var packed = TablePack.Pack(records);

        // Nothing in the result is still owned by the input, so it can be
        // attached anywhere without the tree complaining.
        var host = new JsonObject { ["packed"] = packed };
        Assert.Equal("{\"packed\":{\"fields\":[\"a\"],\"data\":[[{\"deep\":1}]],\"count\":1}}", Doc.Text(host));
        Assert.Equal("[{\"a\":{\"deep\":1}}]", Doc.Text(records));
    }
}
