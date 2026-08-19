<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use JsonException;

/**
 * The json-compress command line.
 *
 * Reads from a file or from stdin, writes to a file or to stdout, so it drops
 * into a pipeline without ceremony:
 *
 *     curl -s https://api.example.com/orders | json-compress compress > orders.jc.json
 *     json-compress stats orders.json
 */
final class Cli
{
    private const COMMANDS = ['compress', 'decompress', 'stats', 'minify', 'beautify', 'validate'];

    private const USAGE = <<<'TEXT'
        json-compress - lossless JSON compression that is still JSON

        usage: json-compress <command> [file] [options]

        commands:
          compress [file]     Compress a JSON document
          decompress [file]   Restore a compressed document
          stats [file]        Report what compression would save, and change nothing
          minify [file]       Remove whitespace only
          beautify [file]     Re-indent
          validate [file]     Check the document parses, and say where it does not

        options:
          -o, --out FILE      Write to a file instead of stdout
          -p, --pretty        Indent the output
          -i, --indent N      Indent width for --pretty and beautify (default 2)
              --no-strings    Do not build a string-value catalogue
              --no-tables     Do not pack arrays of objects into tables
              --no-constants  Do not hoist unchanging columns out of rows
              --no-dict-columns
                              Do not encode low-cardinality string columns as indices
              --min-rows N    Smallest array that may become a table (default 2)
          -v, --version       Print the version
          -h, --help          Print this help

        With no input file, the document is read from stdin.
        TEXT;

    private function __construct()
    {
    }

    /**
     * Run the command line.
     *
     * @param list<string> $argv The arguments, without the script name.
     *
     * @return int The process exit code.
     */
    public static function main(array $argv): int
    {
        try {
            return self::run($argv);
        } catch (CliError $error) {
            fwrite(STDERR, 'json-compress: ' . $error->getMessage() . "\n");

            return 1;
        }
    }

    /**
     * @param list<string> $argv
     */
    private static function run(array $argv): int
    {
        $command = null;
        $input = null;
        $out = null;
        $pretty = false;
        $indent = 2;
        $options = [];

        for ($at = 0; $at < count($argv); $at++) {
            $argument = $argv[$at];

            switch ($argument) {
                case '-h':
                case '--help':
                    fwrite(STDOUT, self::USAGE . "\n");

                    return 0;
                case '-v':
                case '--version':
                    fwrite(STDOUT, JsonCompress::VERSION . "\n");

                    return 0;
                case '-o':
                case '--out':
                    $out = $argv[++$at] ?? throw new CliError('--out needs a file name');
                    continue 2;
                case '-p':
                case '--pretty':
                    $pretty = true;
                    continue 2;
                case '-i':
                case '--indent':
                    $indent = (int) ($argv[++$at] ?? throw new CliError('--indent needs a number'));
                    continue 2;
                case '--no-strings':
                    $options['strings'] = false;
                    continue 2;
                case '--no-tables':
                    $options['tables'] = false;
                    continue 2;
                case '--no-constants':
                    $options['constants'] = false;
                    continue 2;
                case '--no-dict-columns':
                    $options['dictColumns'] = false;
                    continue 2;
                case '--min-rows':
                    $options['minRows'] = (int) ($argv[++$at] ?? throw new CliError('--min-rows needs a number'));
                    continue 2;
            }

            if (str_starts_with($argument, '-') && $argument !== '-') {
                throw new CliError('unknown option ' . $argument);
            }

            if ($command === null) {
                if (!in_array($argument, self::COMMANDS, true)) {
                    throw new CliError(
                        'unknown command ' . $argument
                        . ' (expected one of ' . implode(', ', self::COMMANDS) . ')'
                    );
                }
                $command = $argument;
                continue;
            }

            if ($input === null) {
                $input = $argument;
                continue;
            }

            throw new CliError('unexpected argument ' . $argument);
        }

        if ($command === null) {
            fwrite(STDOUT, self::USAGE . "\n");

            return 0;
        }

        $text = self::read($input);

        if ($command === 'validate') {
            $found = JsonCompress::validate($text);
            if ($found->valid) {
                fwrite(STDOUT, "valid JSON\n");

                return 0;
            }

            throw new CliError((string) $found);
        }

        if ($command === 'minify') {
            self::write(self::guardJson($text, static fn (): string => JsonCompress::minify($text)), $out);

            return 0;
        }

        if ($command === 'beautify') {
            self::write(
                self::guardJson($text, static fn (): string => JsonCompress::beautify($text, $indent)),
                $out
            );

            return 0;
        }

        $parsed = self::guardJson($text, static fn (): mixed => JsonCompress::parseJson($text));
        $width = $pretty ? $indent : null;

        if ($command === 'compress') {
            self::write(Json::encode(JsonCompress::compress($parsed, $options), $width), $out);

            return 0;
        }

        if ($command === 'decompress') {
            if (!JsonCompress::isCompressed($parsed)) {
                throw new CliError('input is not a compressed document');
            }
            self::write(Json::encode(JsonCompress::decompress($parsed), $width), $out);

            return 0;
        }

        // $command === 'stats'
        $report = JsonCompress::measure($parsed, $options, $text);
        $name = $input === null ? 'stdin' : basename($input);
        fwrite(STDOUT, self::stats($report, $name) . "\n");

        return 0;
    }

    /**
     * Run something that parses JSON, and turn a parse failure into a message
     * that says where the trouble is rather than only that there was some.
     *
     * @template T
     *
     * @param callable():T $work
     *
     * @return T
     */
    private static function guardJson(string $text, callable $work): mixed
    {
        try {
            return $work();
        } catch (JsonException $error) {
            $found = JsonCompress::locate($text);
            if ($found->valid) {
                throw new CliError('input is not valid JSON: ' . $error->getMessage());
            }

            throw new CliError(
                'input is not valid JSON at line ' . $found->line . ', column ' . $found->column
                . ': ' . $found->error
            );
        }
    }

    private static function read(?string $path): string
    {
        if ($path !== null) {
            if (!is_file($path)) {
                throw new CliError('cannot read ' . $path . ' (no such file)');
            }
            $text = file_get_contents($path);
            if ($text === false) {
                throw new CliError('cannot read ' . $path);
            }

            return $text;
        }

        if (stream_isatty(STDIN)) {
            throw new CliError('no input file given and nothing on stdin');
        }

        $text = stream_get_contents(STDIN);

        return $text === false ? '' : $text;
    }

    private static function write(string $text, ?string $path): void
    {
        if ($path !== null) {
            if (file_put_contents($path, $text) === false) {
                throw new CliError('cannot write ' . $path);
            }

            return;
        }

        fwrite(STDOUT, $text . "\n");
    }

    private static function kilobytes(int $count): string
    {
        if ($count < 1024) {
            return $count . ' B';
        }
        if ($count < 1024 * 1024) {
            return sprintf('%.1f KB', $count / 1024);
        }

        return sprintf('%.2f MB', $count / (1024 * 1024));
    }

    /** "31.0% smaller" or "31.0% larger" - never a negative percentage. */
    private static function change(float $percent): string
    {
        return sprintf('%.1f%% %s', abs($percent), $percent < 0 ? 'larger' : 'smaller');
    }

    private static function stats(SizeReport $report, string $name): string
    {
        $pad = static fn (int $bytes): string => str_pad(self::kilobytes($bytes), 10, ' ', STR_PAD_LEFT);

        $lines = [
            $name,
            '  as given          ' . $pad($report->original),
            '  minified          ' . $pad($report->minified),
            '  compressed        ' . $pad($report->compressed),
            '  saved             ' . $pad($report->saved) . '   '
                . self::change($report->percent) . ' than minified',
        ];

        if ($report->gzipMinified !== null && $report->gzipCompressed !== null) {
            $saved = $report->gzipMinified - $report->gzipCompressed;
            $percent = $report->gzipMinified === 0 ? 0.0 : $saved / $report->gzipMinified * 100;
            $lines[] = '';
            $lines[] = '  minified + gzip   ' . $pad($report->gzipMinified);
            $lines[] = '  compressed + gzip ' . $pad($report->gzipCompressed);
            $lines[] = '  saved             ' . $pad($saved) . '   '
                . self::change($percent) . ' than minified + gzip';
        }

        return implode("\n", $lines);
    }
}
