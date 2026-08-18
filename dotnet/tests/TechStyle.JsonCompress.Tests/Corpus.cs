using System.Globalization;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Tests;

/// <summary>
/// A deterministic document generator, shared with the JavaScript test suite.
/// </summary>
/// <remarks>
/// The generator, the word lists and the shape probabilities are the same as
/// the ones in <c>javascript/tests/fuzz.test.js</c>, so a given seed produces
/// the same document in both languages. That makes the corpus usable twice:
/// once to prove this implementation round-trips, and once to prove the two
/// implementations agree on what they write.
/// </remarks>
internal static class Corpus
{
    private static readonly string[] Words =
    {
        "id", "name", "status", "created", "updated", "owner", "tags", "value",
        "count", "total", "items", "meta", "type", "code", "label", "notes",
        "~tilde", "0", "1", "10", "a", "z", "A", "~t", "~r", "toString",
        "__proto__", "constructor", "hasOwnProperty", "", " ", "ключ", "键",
    };

    private static readonly string[] Strings =
    {
        "", "a", "active", "pending", "cancelled", "~", "~~", "~0", "~zz",
        "a longer value that repeats across many records",
        "quote \" backslash \\ newline \n tab \t",
        "😀 emoji", "مرحبا", "null", "true", "0",
    };

    /// <summary>Build one document from a seed.</summary>
    /// <param name="seed">The seed.</param>
    /// <param name="depth">How deep the document may nest.</param>
    /// <returns>The document.</returns>
    public static JsonNode? Document(uint seed, int depth)
    {
        var random = new Lcg(seed);
        return MakeValue(random, depth);
    }

    private static JsonNode? MakeScalar(Lcg random)
    {
        var roll = random.Next();
        if (roll < 0.2)
        {
            return null;
        }

        if (roll < 0.3)
        {
            return JsonValue.Create(random.Next() < 0.5);
        }

        if (roll < 0.55)
        {
            return JsonValue.Create((int)Math.Floor(random.Next() * 2000) - 1000);
        }

        if (roll < 0.62)
        {
            return JsonValue.Create((random.Next() * 1e6) - 5e5);
        }

        return JsonValue.Create(Pick(random, Strings));
    }

    private static JsonNode? MakeValue(Lcg random, int depth)
    {
        if (depth <= 0)
        {
            return MakeScalar(random);
        }

        var roll = random.Next();

        if (roll < 0.45)
        {
            return MakeScalar(random);
        }

        if (roll < 0.6)
        {
            var length = (int)Math.Floor(random.Next() * 6);
            var list = new JsonArray();
            for (var i = 0; i < length; i++)
            {
                list.Add(MakeValue(random, depth - 1));
            }

            return list;
        }

        if (roll < 0.8)
        {
            // A record set: the shape the format is built for, with ragged rows
            // and low-cardinality columns thrown in on purpose.
            var rows = (int)Math.Floor(random.Next() * 8);
            var columnCount = 1 + (int)Math.Floor(random.Next() * 5);
            var columns = new string[columnCount];
            for (var c = 0; c < columnCount; c++)
            {
                columns[c] = Pick(random, Words);
            }

            var records = new JsonArray();
            for (var i = 0; i < rows; i++)
            {
                var record = new JsonObject();
                foreach (var column in columns)
                {
                    if (random.Next() < 0.15)
                    {
                        continue;
                    }

                    record[column] = random.Next() < 0.5
                        ? JsonValue.Create(Pick(random, Strings))
                        : MakeValue(random, depth - 2);
                }

                records.Add(record);
            }

            return records;
        }

        var size = (int)Math.Floor(random.Next() * 6);
        var obj = new JsonObject();
        for (var i = 0; i < size; i++)
        {
            obj[Pick(random, Words)] = MakeValue(random, depth - 1);
        }

        return obj;
    }

    private static string Pick(Lcg random, string[] list)
    {
        return list[(int)Math.Floor(random.Next() * list.Length)];
    }

    /// <summary>
    /// A realistic record set, for the tests that care about size rather than
    /// about awkward shapes.
    /// </summary>
    /// <param name="seed">The seed.</param>
    /// <returns>The records.</returns>
    public static JsonArray RecordSet(uint seed)
    {
        var random = new Lcg(seed);
        var rows = 20 + (int)Math.Floor(random.Next() * 40);
        var records = new JsonArray();

        for (var i = 0; i < rows; i++)
        {
            records.Add(new JsonObject
            {
                ["id"] = i,
                ["reference"] = "REF-" + (10000 + i).ToString(CultureInfo.InvariantCulture),
                ["status"] = Pick(random, new[] { "open", "closed", "pending" }),
                ["region"] = Pick(random, new[] { "north", "south", "east", "west" }),
                // Math.Floor(x + 0.5) rather than Math.Round, because .NET
                // rounds a half to the even neighbour and JavaScript rounds it up.
                ["amount"] = Math.Floor((random.Next() * 100000) + 0.5) / 100,
                ["currency"] = "GBP",
                ["createdAt"] = "2026-0"
                    + (1 + (int)Math.Floor(random.Next() * 9)).ToString(CultureInfo.InvariantCulture)
                    + "-15T09:30:00.000Z",
            });
        }

        return records;
    }

    /// <summary>
    /// The linear congruential generator from Numerical Recipes, which is what
    /// the JavaScript suite uses, down to the truncation to 32 bits.
    /// </summary>
    private sealed class Lcg
    {
        private uint state;

        public Lcg(uint seed)
        {
            this.state = seed;
        }

        public double Next()
        {
            unchecked
            {
                this.state = (this.state * 1664525) + 1013904223;
            }

            return this.state / 4294967296d;
        }
    }
}
