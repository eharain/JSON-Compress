using System;
using System.Globalization;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace TechStyle.JsonCompress.Cli;

/// <summary>
/// The json-compress command line.
/// </summary>
/// <remarks>
/// Reads from a file or from standard input, writes to a file or to standard
/// output, so it drops into a pipeline without ceremony:
/// <code>
/// curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
/// json-compress stats orders.json
/// </code>
/// </remarks>
public static class Program
{
    private const string Usage = @"json-compress - lossless JSON compression that is still JSON

Usage
  json-compress <command> [input] [options]

Commands
  compress [file]     Compress a JSON document
  decompress [file]   Restore a compressed document
  stats [file]        Report what compression would save, and change nothing
  minify [file]       Remove whitespace only
  beautify [file]     Re-indent
  validate [file]     Check the document parses, and say where it does not

Options
  -o, --out <file>    Write to a file instead of standard output
  -p, --pretty        Indent the output
  -i, --indent <n>    Indent width for --pretty and beautify (default 2)
      --no-strings    Do not build a string-value catalogue
      --no-tables     Do not pack arrays of objects into tables
      --no-constants  Do not hoist unchanging columns out of rows
      --no-dict-columns   Do not encode low-cardinality string columns as indices
      --min-rows <n>  Smallest array that may become a table (default 2)
  -h, --help          Show this text
  -v, --version       Show the version

With no input file, the document is read from standard input.
";

    /// <summary>Run the command line.</summary>
    /// <param name="args">The arguments, without the executable name.</param>
    /// <returns>Zero on success, one on failure.</returns>
    public static int Main(string[] args)
    {
        try
        {
            return Run(args);
        }
        catch (CommandLineException error)
        {
            Error(error.Message);
            return 1;
        }
        catch (JsonCompressException error)
        {
            Error(error.Message);
            return 1;
        }
        catch (JsonException error)
        {
            Error("input is not valid JSON: " + error.Message);
            return 1;
        }
        catch (IOException error)
        {
            Error(error.Message);
            return 1;
        }
        catch (UnauthorizedAccessException error)
        {
            Error(error.Message);
            return 1;
        }
    }

    private static int Run(string[] args)
    {
        var options = Options.Parse(args);

        if (options.ShowHelp || options.Command is null)
        {
            Write(Usage, null);
            return 0;
        }

        if (options.ShowVersion)
        {
            Write(Version(), null);
            return 0;
        }

        var text = ReadInput(options.Input);

        switch (options.Command)
        {
            case "validate":
            {
                var found = JsonCompressor.Validate(text);
                if (found.Valid)
                {
                    Write("valid JSON", null);
                    return 0;
                }

                Error("invalid JSON at line " + Number(found.Line) + ", column " + Number(found.Column)
                    + ": " + found.Error);
                return 1;
            }

            case "minify":
                Write(JsonCompressor.Minify(text), options.Out);
                return 0;

            case "beautify":
                Write(JsonCompressor.Beautify(text, options.Indent), options.Out);
                return 0;

            case "compress":
            {
                var envelope = JsonCompressor.Compress(JsonCompressor.ParseJson(text), options.Compress);
                Write(Serialise(envelope, options), options.Out);
                return 0;
            }

            case "decompress":
            {
                var parsed = JsonCompressor.ParseJson(text);
                if (!JsonCompressor.IsCompressed(parsed))
                {
                    throw new CommandLineException("input is not a compressed document");
                }

                Write(Serialise(JsonCompressor.Decompress(parsed), options), options.Out);
                return 0;
            }

            case "stats":
                Write(Stats(text, options), null);
                return 0;

            default:
                throw new CommandLineException(
                    "unknown command \"" + options.Command + "\". Try --help.");
        }
    }

    private static string Stats(string text, Options options)
    {
        var parsed = JsonCompressor.ParseJson(text);
        var report = JsonCompressor.Measure(parsed, options.Compress, source: text);
        var name = options.Input is null ? "standard input" : Path.GetFileName(options.Input);

        var lines = new StringBuilder();
        lines.Append(name).Append('\n');
        lines.Append("  as given          ").Append(Size(report.Original)).Append('\n');
        lines.Append("  minified          ").Append(Size(report.Minified)).Append('\n');
        lines.Append("  compressed        ").Append(Size(report.Compressed)).Append('\n');
        lines.Append("  saved             ").Append(Size(report.Saved))
            .Append("   ").Append(Change(report.Percent)).Append(" than minified").Append('\n');

        if (report.GzipMinified is int gzipMinified && report.GzipCompressed is int gzipCompressed)
        {
            var saved = gzipMinified - gzipCompressed;
            var percent = gzipMinified == 0 ? 0d : (double)saved / gzipMinified * 100d;

            lines.Append('\n');
            lines.Append("  minified + gzip   ").Append(Size(gzipMinified)).Append('\n');
            lines.Append("  compressed + gzip ").Append(Size(gzipCompressed)).Append('\n');
            lines.Append("  saved             ").Append(Size(saved))
                .Append("   ").Append(Change(percent)).Append(" than minified + gzip");
        }

        return lines.ToString().TrimEnd('\n');
    }

    private static string Serialise(JsonNode? value, Options options)
    {
        if (value is null)
        {
            return "null";
        }

        return options.Pretty
            ? JsonCompressor.Beautify(value.ToJsonString(JsonCompressor.SerializerOptions), options.Indent)
            : value.ToJsonString(JsonCompressor.SerializerOptions);
    }

    /// <summary>"31.0% smaller" or "31.0% larger" - never a negative percentage.</summary>
    /// <param name="percent">The change, where a positive number is a saving.</param>
    /// <returns>The phrase.</returns>
    private static string Change(double percent)
    {
        return Math.Abs(percent).ToString("F1", CultureInfo.InvariantCulture)
            + "% " + (percent < 0 ? "larger" : "smaller");
    }

    private static string Size(int bytes)
    {
        string text;
        if (Math.Abs(bytes) < 1024)
        {
            text = Number(bytes) + " B";
        }
        else if (Math.Abs(bytes) < 1024 * 1024)
        {
            text = ((double)bytes / 1024).ToString("F1", CultureInfo.InvariantCulture) + " KB";
        }
        else
        {
            text = ((double)bytes / (1024 * 1024)).ToString("F2", CultureInfo.InvariantCulture) + " MB";
        }

        return text.PadLeft(10);
    }

    private static string Number(int value)
    {
        return value.ToString(CultureInfo.InvariantCulture);
    }

    private static string Version()
    {
        var assembly = typeof(Program).Assembly;
        var version = assembly.GetName().Version;
        return version is null ? "unknown" : version.ToString(3);
    }

    private static string ReadInput(string? file)
    {
        if (file is not null)
        {
            if (!File.Exists(file))
            {
                throw new CommandLineException("cannot read " + file + " (no such file)");
            }

            return File.ReadAllText(file, Encoding.UTF8);
        }

        using var input = Console.OpenStandardInput();
        using var reader = new StreamReader(input, Encoding.UTF8);
        var text = reader.ReadToEnd();
        if (text.Length == 0)
        {
            throw new CommandLineException("no input file given and nothing on standard input");
        }

        return text;
    }

    /// <summary>
    /// Write text out as UTF-8, whatever the console code page happens to be.
    /// </summary>
    /// <param name="text">What to write.</param>
    /// <param name="file">Where to write it, or null for standard output.</param>
    private static void Write(string text, string? file)
    {
        if (file is not null)
        {
            File.WriteAllText(file, text, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
            return;
        }

        using var output = Console.OpenStandardOutput();
        using var writer = new StreamWriter(output, new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        writer.Write(text);
        writer.Write('\n');
    }

    private static void Error(string message)
    {
        Console.Error.WriteLine("json-compress: " + message);
    }

    /// <summary>Something the caller typed that cannot be acted on.</summary>
    private sealed class CommandLineException : Exception
    {
        public CommandLineException(string message)
            : base(message)
        {
        }
    }

    private sealed class Options
    {
        public string? Command { get; private set; }

        public string? Input { get; private set; }

        public string? Out { get; private set; }

        public bool Pretty { get; private set; }

        public int Indent { get; private set; } = 2;

        public bool ShowHelp { get; private set; }

        public bool ShowVersion { get; private set; }

        public CompressOptions Compress { get; } = CompressOptions.Default();

        public static Options Parse(string[] args)
        {
            var options = new Options();

            for (var i = 0; i < args.Length; i++)
            {
                var arg = args[i];
                switch (arg)
                {
                    case "-h":
                    case "--help":
                        options.ShowHelp = true;
                        break;

                    case "-v":
                    case "--version":
                        options.ShowVersion = true;
                        options.Command ??= "version";
                        break;

                    case "-o":
                    case "--out":
                        options.Out = Next(args, ref i, arg);
                        break;

                    case "-p":
                    case "--pretty":
                        options.Pretty = true;
                        break;

                    case "-i":
                    case "--indent":
                        options.Indent = Count(Next(args, ref i, arg), arg);
                        break;

                    case "--no-strings":
                        options.Compress.Strings = false;
                        break;

                    case "--no-tables":
                        options.Compress.Tables = false;
                        break;

                    case "--no-constants":
                        options.Compress.Constants = false;
                        break;

                    case "--no-dict-columns":
                        options.Compress.DictionaryColumns = false;
                        break;

                    case "--min-rows":
                        options.Compress.MinRows = Count(Next(args, ref i, arg), arg);
                        break;

                    default:
                        if (arg.StartsWith("-", StringComparison.Ordinal))
                        {
                            throw new CommandLineException("unknown option " + arg);
                        }

                        if (options.Command is null)
                        {
                            options.Command = arg;
                        }
                        else if (options.Input is null)
                        {
                            options.Input = arg;
                        }
                        else
                        {
                            throw new CommandLineException("unexpected argument " + arg);
                        }

                        break;
                }
            }

            return options;
        }

        private static string Next(string[] args, ref int at, string option)
        {
            if (at + 1 >= args.Length)
            {
                throw new CommandLineException(option + " needs a value");
            }

            return args[++at];
        }

        private static int Count(string text, string option)
        {
            if (!int.TryParse(text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var value))
            {
                throw new CommandLineException(option + " needs a number, not \"" + text + "\"");
            }

            return value;
        }
    }
}
