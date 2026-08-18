using System.Collections.Concurrent;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// Agreement with the reference implementation.
/// </summary>
/// <remarks>
/// <para>
/// The corpus in <c>fixtures/conformance.json</c> was produced by the
/// JavaScript implementation: each case carries a source document and the
/// envelope that encoder wrote for it, both as text. Two things are checked
/// against it, and they are not the same thing.
/// </para>
/// <para>
/// Reading is the contract: a document written by one implementation must be
/// readable by every other, and must restore to exactly what went in. Writing
/// the same bytes is not required by the specification - two conforming
/// encoders may make different choices - but this implementation aims for it
/// anyway, because agreeing byte for byte is the sharpest available evidence
/// that the two are making the same decisions for the same reasons.
/// </para>
/// </remarks>
public class ConformanceTests
{
    private static readonly ConcurrentDictionary<string, Case> Cases = Corpus.Load();

    public static TheoryData<string> Names()
    {
        var names = new TheoryData<string>();
        foreach (var name in Cases.Keys.OrderBy(name => name, StringComparer.Ordinal))
        {
            names.Add(name);
        }

        return names;
    }

    [Theory]
    [MemberData(nameof(Names))]
    public void ADocumentWrittenByTheJavaScriptEncoderRestoresExactly(string name)
    {
        var subject = Cases[name];
        var restored = JsonCompressor.Decompress(Doc.Parse(subject.Compressed));
        Assert.Equal(subject.Source, Doc.Text(restored));
    }

    [Theory]
    [MemberData(nameof(Names))]
    public void ThisEncoderWritesWhatTheJavaScriptEncoderWrites(string name)
    {
        var subject = Cases[name];
        Assert.Equal(subject.Compressed, JsonCompressor.Stringify(Doc.Parse(subject.Source)));
    }

    [Fact]
    public void TheCorpusIsActuallyThere()
    {
        Assert.True(Cases.Count > 100, "expected a corpus, found " + Cases.Count + " cases");
    }

    private sealed record Case(string Name, string Source, string Compressed);

    private static class Corpus
    {
        public static ConcurrentDictionary<string, Case> Load()
        {
            var path = Path.Combine(AppContext.BaseDirectory, "fixtures", "conformance.json");
            if (!File.Exists(path))
            {
                throw new FileNotFoundException(
                    "The conformance corpus is missing. Rebuild it with "
                    + "node dotnet/tests/interop/generate-fixtures.mjs",
                    path);
            }

            var document = (JsonObject)JsonCompressor.ParseJson(File.ReadAllText(path))!;
            var cases = new ConcurrentDictionary<string, Case>(StringComparer.Ordinal);

            foreach (var entry in (JsonArray)document["cases"]!)
            {
                var obj = (JsonObject)entry!;
                var name = obj["name"]!.GetValue<string>();
                cases[name] = new Case(
                    name,
                    obj["source"]!.GetValue<string>(),
                    obj["compressed"]!.GetValue<string>());
            }

            return cases;
        }
    }
}
