// Command json-compress compresses, restores and measures JSON documents.
//
// It reads from a file or from stdin and writes to a file or to stdout, so it
// drops into a pipeline without ceremony:
//
//	curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
//	json-compress stats orders.json
//
// Install it with:
//
//	go install github.com/eharain/JSON-Compress/go/cmd/json-compress@latest
package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	jsoncompress "github.com/eharain/JSON-Compress/go"
)

const usage = `json-compress - lossless JSON compression that is still JSON

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
  -o, --out <file>    Write to a file instead of stdout
  -p, --pretty        Indent the output
  -i, --indent <n>    Indent width for --pretty and beautify (default 2)
  --no-strings        Do not build a string-value catalogue
  --no-tables         Do not pack arrays of objects into tables
  --no-constants      Do not hoist unchanging columns out of rows
  --no-dict-columns   Do not encode low-cardinality string columns as indices
  --min-rows <n>      Smallest array that may become a table (default 2)
  -h, --help          Show this text
  -v, --version       Show the version

With no input file, the document is read from stdin.
`

func main() {
	os.Exit(run(os.Args[1:], os.Stdin, os.Stdout, os.Stderr))
}

type settings struct {
	command  string
	input    string
	out      string
	pretty   bool
	indent   int
	compress jsoncompress.Options
}

// failure is how every command reports that it cannot go on: a message for
// stderr and the exit code that goes with it.
type failure struct {
	message string
}

func (f *failure) Error() string {
	return f.message
}

func fail(format string, args ...any) error {
	return &failure{message: fmt.Sprintf(format, args...)}
}

func run(argv []string, stdin io.Reader, stdout, stderr io.Writer) int {
	err := execute(argv, stdin, stdout)
	if err == nil {
		return 0
	}
	fmt.Fprintln(stderr, "json-compress: "+err.Error())
	return 1
}

func execute(argv []string, stdin io.Reader, stdout io.Writer) error {
	options, done, err := parseArgs(argv, stdout)
	if err != nil || done {
		return err
	}

	if options.command == "" {
		fmt.Fprint(stdout, usage)
		return nil
	}

	text, err := readInput(options.input, stdin)
	if err != nil {
		return err
	}

	indent := 0
	if options.pretty {
		indent = options.indent
	}

	switch options.command {
	case "validate":
		found := jsoncompress.Validate(text)
		if found.Valid {
			fmt.Fprintln(stdout, "valid JSON")
			return nil
		}
		where := ""
		if found.Line > 0 {
			where = fmt.Sprintf(" at line %d, column %d", found.Line, found.Column)
		}
		return fail("invalid JSON%s: %s", where, found.Error)

	case "minify":
		out, err := jsoncompress.Minify(text)
		if err != nil {
			return fail("input is not valid JSON: %s", err)
		}
		return writeOutput(out, options.out, stdout)

	case "beautify":
		out, err := jsoncompress.Beautify(text, options.indent)
		if err != nil {
			return fail("input is not valid JSON: %s", err)
		}
		return writeOutput(out, options.out, stdout)
	}

	parsed, err := jsoncompress.ParseJSONString(text)
	if err != nil {
		return fail("input is not valid JSON: %s", err)
	}

	switch options.command {
	case "compress":
		out, err := jsoncompress.MarshalIndent(parsed, indent, options.compress)
		if err != nil {
			return err
		}
		return writeOutput(string(out), options.out, stdout)

	case "decompress":
		if !jsoncompress.IsCompressed(parsed) {
			return fail("input is not a compressed document")
		}
		restored, err := jsoncompress.Decompress(parsed)
		if err != nil {
			return err
		}
		out, err := jsoncompress.FormatJSONIndent(restored, indent)
		if err != nil {
			return err
		}
		return writeOutput(string(out), options.out, stdout)

	case "stats":
		report, err := jsoncompress.MeasureText(text, options.compress)
		if err != nil {
			return err
		}
		name := "stdin"
		if options.input != "" {
			name = filepath.Base(options.input)
		}
		fmt.Fprintln(stdout, stats(report, name))
		return nil

	default:
		return fail("unknown command %q. Try --help.", options.command)
	}
}

func parseArgs(argv []string, stdout io.Writer) (settings, bool, error) {
	options := settings{indent: 2, compress: jsoncompress.DefaultOptions()}

	next := func(i *int, flag string) (string, error) {
		*i++
		if *i >= len(argv) {
			return "", fail("%s needs a value", flag)
		}
		return argv[*i], nil
	}

	for i := 0; i < len(argv); i++ {
		argument := argv[i]
		switch argument {
		case "-h", "--help":
			fmt.Fprint(stdout, usage)
			return options, true, nil
		case "-v", "--version":
			fmt.Fprintln(stdout, jsoncompress.Version)
			return options, true, nil
		case "-o", "--out":
			value, err := next(&i, argument)
			if err != nil {
				return options, false, err
			}
			options.out = value
		case "-p", "--pretty":
			options.pretty = true
		case "-i", "--indent":
			value, err := next(&i, argument)
			if err != nil {
				return options, false, err
			}
			width, err := strconv.Atoi(value)
			if err != nil {
				return options, false, fail("%s must be a number, not %q", argument, value)
			}
			options.indent = width
		case "--no-strings":
			options.compress.Strings = false
		case "--no-tables":
			options.compress.Tables = false
		case "--no-constants":
			options.compress.Constants = false
		case "--no-dict-columns":
			options.compress.DictColumns = false
		case "--min-rows":
			value, err := next(&i, argument)
			if err != nil {
				return options, false, err
			}
			rows, err := strconv.Atoi(value)
			if err != nil {
				return options, false, fail("--min-rows must be a number, not %q", value)
			}
			options.compress.MinRows = rows
		default:
			switch {
			case strings.HasPrefix(argument, "-") && argument != "-":
				return options, false, fail("unknown option %s", argument)
			case options.command == "":
				options.command = argument
			case options.input == "":
				options.input = argument
			default:
				return options, false, fail("unexpected argument %s", argument)
			}
		}
	}

	return options, false, nil
}

// byteOrderMark is what a Windows editor, and PowerShell's own pipeline, put in
// front of a UTF-8 document. JSON has no place for it and every parser in this
// family refuses it, so the command line drops it rather than making people
// wonder why their file will not read.
const byteOrderMark = string(rune(0xFEFF))

func readInput(path string, stdin io.Reader) (string, error) {
	if path != "" {
		data, err := os.ReadFile(path)
		if err != nil {
			return "", fail("cannot read %s (%s)", path, errorReason(err))
		}
		return strings.TrimPrefix(string(data), byteOrderMark), nil
	}

	// Reading from a terminal would hang waiting for something nobody is going
	// to type, so say what is missing instead.
	if file, ok := stdin.(*os.File); ok {
		if info, err := file.Stat(); err == nil && info.Mode()&os.ModeCharDevice != 0 {
			return "", fail("no input file given and nothing on stdin")
		}
	}

	data, err := io.ReadAll(stdin)
	if err != nil {
		return "", fail("cannot read stdin (%s)", errorReason(err))
	}
	return strings.TrimPrefix(string(data), byteOrderMark), nil
}

func errorReason(err error) string {
	if pathError, ok := err.(*os.PathError); ok {
		return pathError.Err.Error()
	}
	return err.Error()
}

func writeOutput(text, path string, stdout io.Writer) error {
	if path != "" {
		if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
			return fail("cannot write %s (%s)", path, errorReason(err))
		}
		return nil
	}
	fmt.Fprintln(stdout, text)
	return nil
}

func stats(report jsoncompress.Report, name string) string {
	lines := []string{
		name,
		"  as given          " + pad(kilobytes(report.Original)),
		"  minified          " + pad(kilobytes(report.Minified)),
		"  compressed        " + pad(kilobytes(report.Compressed)),
		"  saved             " + pad(kilobytes(report.Saved)) + "   " + change(report.Percent) + " than minified",
	}

	saved := report.GzipMinified - report.GzipCompressed
	percent := 0.0
	if report.GzipMinified > 0 {
		percent = float64(saved) / float64(report.GzipMinified) * 100
	}
	lines = append(lines,
		"",
		"  minified + gzip   "+pad(kilobytes(report.GzipMinified)),
		"  compressed + gzip "+pad(kilobytes(report.GzipCompressed)),
		"  saved             "+pad(kilobytes(saved))+"   "+change(percent)+" than minified + gzip",
	)

	return strings.Join(lines, "\n")
}

func pad(text string) string {
	return fmt.Sprintf("%10s", text)
}

func kilobytes(count int) string {
	switch {
	case count < 1024 && count > -1024:
		return strconv.Itoa(count) + " B"
	case count < 1024*1024 && count > -1024*1024:
		return fmt.Sprintf("%.1f KB", float64(count)/1024)
	default:
		return fmt.Sprintf("%.2f MB", float64(count)/(1024*1024))
	}
}

// change reads as "31.0% smaller" or "31.0% larger" - never a negative
// percentage.
func change(percent float64) string {
	direction := "smaller"
	if percent < 0 {
		direction = "larger"
		percent = -percent
	}
	return fmt.Sprintf("%.1f%% %s", percent, direction)
}
