using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;
using System.Text.Json.Nodes;
using TechStyle.JsonCompress;

namespace TechStyle.JsonCompress.Examples;

/// <summary>
/// What the format is worth, on shapes that occur in real systems.
/// </summary>
/// <remarks>
/// <para>
/// <c>dotnet run --project examples/Benchmark -c Release</c>
/// </para>
/// <para>
/// Every figure the README quotes is produced by running this, so it can be
/// checked rather than taken on trust. The interesting column is the last one:
/// whether the format still helps once gzip has had its turn, which is the only
/// question that matters for anything served over HTTP.
/// </para>
/// </remarks>
public static class Program
{
    private static readonly string[] First =
    {
        "Amara", "Bilal", "Chen", "Dara", "Eesa", "Farah", "Gita", "Hana", "Idris", "Jae",
    };

    private static readonly string[] Last =
    {
        "Ahmed", "Bright", "Chowdhury", "Doyle", "Evans", "Fischer", "Gomez", "Haddad",
    };

    private static readonly string[] City =
    {
        "London", "Manchester", "Birmingham", "Leeds", "Glasgow", "Cardiff", "Belfast",
    };

    private static uint seed;

    /// <summary>Run every scenario and print the table.</summary>
    public static void Main()
    {
        var scenarios = new (string Name, Func<JsonNode> Build)[]
        {
            ("API record set (1,000 users)", Users),
            ("Nested API response (orders with lines)", Orders),
            ("Time series (2,000 readings)", Readings),
            ("Structured logs (1,500 lines)", Logs),
            ("GeoJSON feature collection (400 points)", Features),
            ("Config document (deeply nested, few repeats)", Config),
        };

        var header = "scenario".PadRight(46)
            + "minified".PadLeft(10)
            + "compressed".PadLeft(12)
            + "saved".PadLeft(8)
            + "+gzip".PadLeft(10)
            + "saved".PadLeft(8);

        var output = new StringBuilder();
        output.Append('\n').Append(header).Append('\n').Append('-', header.Length).Append('\n');

        foreach (var (name, build) in scenarios)
        {
            seed = 20260818;
            var value = build();

            var minified = value.ToJsonString(JsonCompressor.SerializerOptions);
            var packed = JsonCompressor.Stringify(value);

            // A benchmark that quietly loses data is not a benchmark.
            var restored = JsonCompressor.Parse(packed);
            if (!string.Equals(
                    restored is null ? "null" : restored.ToJsonString(JsonCompressor.SerializerOptions),
                    minified,
                    StringComparison.Ordinal))
            {
                throw new InvalidOperationException(name + " did not round-trip");
            }

            var minifiedBytes = Bytes.ByteLength(minified);
            var packedBytes = Bytes.ByteLength(packed);
            var minifiedGzip = Bytes.GZipSize(minified);
            var packedGzip = Bytes.GZipSize(packed);

            output.Append(name.PadRight(46))
                .Append(Kb(minifiedBytes).PadLeft(10))
                .Append(Kb(packedBytes).PadLeft(12))
                .Append(Percent(minifiedBytes, packedBytes).PadLeft(8))
                .Append(Kb(packedGzip).PadLeft(10))
                .Append(Percent(minifiedGzip, packedGzip).PadLeft(8))
                .Append('\n');
        }

        output.Append('\n')
            .Append("\"saved\" is against minified JSON; the gzip columns compare compressed + gzip\n")
            .Append("against minified + gzip, which is what an HTTP response actually costs.\n");

        Console.Out.Write(output.ToString());
    }

    /* ------------------------------------------------------------------ *
     * The scenarios
     * ------------------------------------------------------------------ */

    private static JsonNode Users()
    {
        var users = new JsonArray();
        for (var i = 0; i < 1000; i++)
        {
            users.Add(new JsonObject
            {
                ["id"] = i + 1,
                ["firstName"] = Pick(First),
                ["lastName"] = Pick(Last),
                ["email"] = "user" + Text(i + 1) + "@example.com",
                ["role"] = Pick(new[] { "admin", "editor", "viewer", "viewer", "viewer" }),
                ["active"] = Random() > 0.2,
                ["city"] = Pick(City),
                ["country"] = "United Kingdom",
                ["createdAt"] = "2026-0" + Text(Integer(1, 9)) + "-" + Text(Integer(10, 28)) + "T09:30:00.000Z",
                ["loginCount"] = Integer(0, 4000),
            });
        }

        return users;
    }

    private static JsonNode Orders()
    {
        var data = new JsonArray();
        for (var i = 0; i < 200; i++)
        {
            // The generator is drawn from in the same order the JavaScript
            // benchmark draws from it, so both produce the same documents and
            // their figures can be set beside each other.
            var status = Pick(new[] { "placed", "picked", "shipped", "delivered" });
            var placedAt = "2026-08-" + Text(Integer(10, 18)) + "T" + Text(Integer(10, 20)) + ":15:00.000Z";
            var customer = new JsonObject
            {
                ["id"] = Integer(1, 5000),
                ["name"] = Pick(First) + " " + Pick(Last),
                ["city"] = Pick(City),
            };

            var lines = new JsonArray();
            var lineCount = Integer(1, 6);
            for (var line = 0; line < lineCount; line++)
            {
                lines.Add(new JsonObject
                {
                    ["sku"] = "SKU-" + Text(Integer(1000, 9999)),
                    ["description"] = Pick(new[] { "Cotton shirt", "Wool scarf", "Leather belt", "Linen trousers" }),
                    ["quantity"] = Integer(1, 4),
                    ["unitPrice"] = Integer(500, 12000) / 100d,
                    ["taxRate"] = 0.2,
                });
            }

            data.Add(new JsonObject
            {
                ["orderId"] = "ORD-" + Text(100000 + i),
                ["status"] = status,
                ["currency"] = "GBP",
                ["placedAt"] = placedAt,
                ["customer"] = customer,
                ["lines"] = lines,
            });
        }

        return new JsonObject
        {
            ["meta"] = new JsonObject
            {
                ["page"] = 1,
                ["pageSize"] = 200,
                ["total"] = 8214,
                ["generatedAt"] = "2026-08-18T12:00:00.000Z",
            },
            ["data"] = data,
        };
    }

    private static JsonNode Readings()
    {
        var readings = new JsonArray();
        for (var i = 0; i < 2000; i++)
        {
            readings.Add(new JsonObject
            {
                ["t"] = 1755500000 + (i * 60),
                ["sensor"] = Pick(new[] { "temp-01", "temp-02", "temp-03", "humidity-01" }),
                ["unit"] = "celsius",
                ["value"] = Math.Floor((Random() * 3500) + 0.5) / 100,
                ["quality"] = Pick(new[] { "good", "good", "good", "suspect" }),
            });
        }

        return readings;
    }

    private static JsonNode Logs()
    {
        var lines = new JsonArray();
        for (var i = 0; i < 1500; i++)
        {
            lines.Add(new JsonObject
            {
                ["level"] = Pick(new[] { "info", "info", "info", "warn", "error" }),
                ["service"] = Pick(new[] { "api-gateway", "orders", "billing", "search" }),
                ["env"] = "production",
                ["region"] = "eu-west-2",
                ["message"] = Pick(new[]
                {
                    "request completed",
                    "cache miss",
                    "upstream timeout, retrying",
                    "validation failed",
                }),
                ["durationMs"] = Integer(1, 4000),
                ["traceId"] = "00-" + Text(Integer(100000000, 900000000)) + "-"
                    + Text(Integer(1000000, 9000000)) + "-01",
            });
        }

        return lines;
    }

    private static JsonNode Features()
    {
        var features = new JsonArray();
        for (var i = 0; i < 400; i++)
        {
            features.Add(new JsonObject
            {
                ["type"] = "Feature",
                ["geometry"] = new JsonObject
                {
                    ["type"] = "Point",
                    ["coordinates"] = new JsonArray
                    {
                        Math.Floor(((Random() * 8) - 6) * 1e6 + 0.5) / 1e6,
                        Math.Floor((49 + (Random() * 9)) * 1e6 + 0.5) / 1e6,
                    },
                },
                ["properties"] = new JsonObject
                {
                    ["id"] = i,
                    ["name"] = Pick(City) + " site " + Text(i),
                    ["category"] = Pick(new[] { "depot", "store", "hub" }),
                },
            });
        }

        return new JsonObject
        {
            ["type"] = "FeatureCollection",
            ["features"] = features,
        };
    }

    private static JsonNode Config()
    {
        var services = new JsonObject();
        for (var i = 0; i < 12; i++)
        {
            services["service-" + Text(i)] = new JsonObject
            {
                ["image"] = "registry.example.com/team/service-" + Text(i) + ":1.4." + Text(i),
                ["replicas"] = Integer(1, 6),
                ["resources"] = new JsonObject { ["cpu"] = "500m", ["memory"] = "512Mi" },
                ["env"] = new JsonObject
                {
                    ["NODE_ENV"] = "production",
                    ["LOG_LEVEL"] = "info",
                    ["REGION"] = "eu-west-2",
                },
                ["ports"] = new JsonArray
                {
                    new JsonObject
                    {
                        ["container"] = 8080,
                        ["published"] = 8080 + i,
                        ["protocol"] = "tcp",
                    },
                },
            };
        }

        return new JsonObject
        {
            ["version"] = 3,
            ["services"] = services,
        };
    }

    /* ------------------------------------------------------------------ *
     * Plumbing
     * ------------------------------------------------------------------ */

    /// <summary>
    /// The same generator the JavaScript benchmark uses, so the two produce the
    /// same documents and their numbers can be compared directly.
    /// </summary>
    /// <returns>A number from zero up to but not including one.</returns>
    private static double Random()
    {
        unchecked
        {
            seed = (seed * 1664525) + 1013904223;
        }

        return seed / 4294967296d;
    }

    private static string Pick(IReadOnlyList<string> list)
    {
        return list[(int)Math.Floor(Random() * list.Count)];
    }

    private static int Integer(int from, int to)
    {
        return from + (int)Math.Floor(Random() * (to - from + 1));
    }

    private static string Text(int value)
    {
        return value.ToString(CultureInfo.InvariantCulture);
    }

    private static string Kb(int bytes)
    {
        return ((double)bytes / 1024).ToString("F1", CultureInfo.InvariantCulture) + " KB";
    }

    private static string Percent(int before, int after)
    {
        return ((1d - ((double)after / before)) * 100d).ToString("F0", CultureInfo.InvariantCulture) + "%";
    }
}
