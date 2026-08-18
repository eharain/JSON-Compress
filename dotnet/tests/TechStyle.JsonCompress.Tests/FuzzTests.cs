namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// Randomised round-trip testing.
/// </summary>
/// <remarks>
/// Hand-written cases only cover the shapes somebody thought of. This generates
/// documents from a seeded pseudo-random source - so a failure is reproducible
/// from the seed reported with it - and insists that every one survives the
/// round trip character for character.
/// </remarks>
public class FuzzTests
{
    [Fact]
    public void TwoThousandRandomDocumentsRoundTripCharacterForCharacter()
    {
        for (uint seed = 1; seed <= 2000; seed++)
        {
            var value = Corpus.Document(seed, 5);
            var expected = Doc.Text(value);

            string actual;
            try
            {
                actual = Doc.Text(JsonCompressor.Decompress(JsonCompressor.Compress(value)));
            }
            catch (JsonCompressException error)
            {
                Assert.Fail("seed " + seed + " threw: " + error.Message + "\n" + expected);
                return;
            }

            if (!string.Equals(actual, expected, StringComparison.Ordinal))
            {
                Assert.Fail("seed " + seed + " did not round-trip\n  expected " + expected + "\n  actual   " + actual);
            }
        }
    }

    [Fact]
    public void RandomDocumentsRoundTripUnderEveryOptionCombination()
    {
        var combinations = new List<CompressOptions>();
        foreach (var strings in new[] { true, false })
        {
            foreach (var tables in new[] { true, false })
            {
                foreach (var constants in new[] { true, false })
                {
                    foreach (var dictionaries in new[] { true, false })
                    {
                        combinations.Add(new CompressOptions
                        {
                            Strings = strings,
                            Tables = tables,
                            Constants = constants,
                            DictionaryColumns = dictionaries,
                        });
                    }
                }
            }
        }

        for (uint seed = 1; seed <= 120; seed++)
        {
            var value = Corpus.Document(seed * 7919, 4);
            var expected = Doc.Text(value);

            foreach (var options in combinations)
            {
                var actual = Doc.Text(JsonCompressor.Decompress(JsonCompressor.Compress(value, options)));
                if (!string.Equals(actual, expected, StringComparison.Ordinal))
                {
                    Assert.Fail(
                        "seed " + seed + " with strings=" + options.Strings + " tables=" + options.Tables
                        + " constants=" + options.Constants + " dictionaries=" + options.DictionaryColumns
                        + " did not round-trip\n  expected " + expected + "\n  actual   " + actual);
                }
            }
        }
    }

    [Fact]
    public void CompressionNeverInflatesARealisticRecordSet()
    {
        for (uint seed = 1; seed <= 200; seed++)
        {
            var value = Corpus.RecordSet(seed * 104729);
            var plain = Doc.Text(value).Length;
            var packed = JsonCompressor.Stringify(value).Length;

            Assert.True(packed < plain, "seed " + seed + ": compressed " + packed + " against plain " + plain);
        }
    }
}
