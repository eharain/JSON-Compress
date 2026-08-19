<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversNothing;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\JsonCompress;

/**
 * The command line, driven the way a person or a pipeline drives it.
 *
 * Each case runs the real script in a real process, because half of what a
 * command line promises is about streams and exit codes rather than about
 * return values.
 */
#[CoversNothing]
final class CliTest extends TestCase
{
    private const SCRIPT = __DIR__ . '/../bin/json-compress';

    /** @var list<string> */
    private array $temporary = [];

    protected function tearDown(): void
    {
        foreach ($this->temporary as $path) {
            if (is_file($path)) {
                unlink($path);
            }
        }
        $this->temporary = [];
    }

    /**
     * Run the command line in its own process.
     *
     * @param list<string> $arguments
     *
     * @return array{out:string,err:string,code:int}
     */
    private function launch(array $arguments, string $input = ''): array
    {
        $command = array_merge([PHP_BINARY, self::SCRIPT], $arguments);

        $process = proc_open(
            $command,
            [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']],
            $pipes
        );
        $this->assertIsResource($process, 'could not start the command line');

        fwrite($pipes[0], $input);
        fclose($pipes[0]);

        $out = (string) stream_get_contents($pipes[1]);
        $err = (string) stream_get_contents($pipes[2]);
        fclose($pipes[1]);
        fclose($pipes[2]);

        return ['out' => $out, 'err' => $err, 'code' => proc_close($process)];
    }

    private function temporaryFile(string $contents): string
    {
        $path = tempnam(sys_get_temp_dir(), 'jc');
        $this->assertIsString($path);
        file_put_contents($path, $contents);
        $this->temporary[] = $path;

        return $path;
    }

    public function testCompressReadsStdinAndWritesStdout(): void
    {
        $result = $this->launch(['compress'], '[{"a":1},{"a":2}]');

        $this->assertSame(0, $result['code'], $result['err']);
        $this->assertSame('{"jc":1,"k":["a"],"d":{"~t":[0],"~r":[[1],[2]]}}', trim($result['out']));
    }

    public function testCompressAndDecompressAreInverses(): void
    {
        $source = '{"users":[{"id":1,"name":"Ada"},{"id":2,"name":"Bob"}]}';
        $compressed = $this->launch(['compress'], $source);
        $restored = $this->launch(['decompress'], $compressed['out']);

        $this->assertSame(0, $restored['code'], $restored['err']);
        $this->assertSame($source, trim($restored['out']));
    }

    public function testItReadsAFileAndWritesAFile(): void
    {
        $input = $this->temporaryFile('[{"a":1},{"a":2}]');
        $output = $this->temporaryFile('');

        $result = $this->launch(['compress', $input, '--out', $output]);

        $this->assertSame(0, $result['code'], $result['err']);
        $this->assertSame('', $result['out'], 'nothing goes to stdout when a file is named');
        $this->assertSame(
            '{"jc":1,"k":["a"],"d":{"~t":[0],"~r":[[1],[2]]}}',
            file_get_contents($output)
        );
    }

    public function testPrettyIndentsTheOutput(): void
    {
        $result = $this->launch(['compress', '--pretty'], '{"a":1}');

        $this->assertSame(0, $result['code'], $result['err']);
        $this->assertStringContainsString("\n  \"jc\": 1,", $result['out']);
    }

    public function testTheOptionFlagsReachTheEncoder(): void
    {
        $result = $this->launch(['compress', '--no-tables'], '[{"a":1},{"a":2}]');

        $this->assertSame(0, $result['code'], $result['err']);
        $this->assertStringNotContainsString('~t', $result['out']);
    }

    public function testMinifyAndBeautify(): void
    {
        $pretty = "{\n  \"a\": 1\n}";

        $this->assertSame('{"a":1}', trim($this->launch(['minify'], $pretty)['out']));
        $this->assertSame($pretty, trim($this->launch(['beautify'], '{"a":1}')['out']));
        $this->assertSame(
            "{\n    \"a\": 1\n}",
            trim($this->launch(['beautify', '--indent', '4'], '{"a":1}')['out'])
        );
    }

    public function testStatsChangesNothingAndReportsBothBaselines(): void
    {
        $result = $this->launch(['stats'], '[{"a":1,"b":"x"},{"a":2,"b":"x"}]');

        $this->assertSame(0, $result['code'], $result['err']);
        $this->assertStringContainsString('minified', $result['out']);
        $this->assertStringContainsString('compressed', $result['out']);
        $this->assertStringContainsString('stdin', $result['out']);
    }

    public function testValidateSaysWhereADocumentGoesWrong(): void
    {
        $good = $this->launch(['validate'], '{"a":1}');
        $this->assertSame(0, $good['code']);
        $this->assertStringContainsString('valid JSON', $good['out']);

        $bad = $this->launch(['validate'], "{\n  \"a\": 1,\n}");
        $this->assertSame(1, $bad['code']);
        $this->assertStringContainsString('line 2', $bad['err']);
        $this->assertStringContainsString('trailing comma', $bad['err']);
    }

    public function testBrokenInputIsReportedRatherThanCrashedOn(): void
    {
        $result = $this->launch(['compress'], '{"a":}');

        $this->assertSame(1, $result['code']);
        $this->assertStringContainsString('not valid JSON', $result['err']);
        $this->assertSame('', $result['out']);
    }

    public function testDecompressRefusesADocumentThatWasNeverCompressed(): void
    {
        $result = $this->launch(['decompress'], '{"a":1}');

        $this->assertSame(1, $result['code']);
        $this->assertStringContainsString('not a compressed document', $result['err']);
    }

    public function testAnUnknownCommandAndAnUnknownOptionAreRefused(): void
    {
        $command = $this->launch(['squash'], '{}');
        $this->assertSame(1, $command['code']);
        $this->assertStringContainsString('unknown command squash', $command['err']);

        $option = $this->launch(['compress', '--turbo'], '{}');
        $this->assertSame(1, $option['code']);
        $this->assertStringContainsString('unknown option --turbo', $option['err']);
    }

    public function testVersionAndHelp(): void
    {
        $version = $this->launch(['--version']);
        $this->assertSame(0, $version['code']);
        $this->assertSame(JsonCompress::VERSION, trim($version['out']));

        $help = $this->launch([]);
        $this->assertSame(0, $help['code']);
        $this->assertStringContainsString('usage: json-compress', $help['out']);
    }

    public function testAFileThatIsNotThereIsReported(): void
    {
        $result = $this->launch(['compress', 'no-such-file.json']);

        $this->assertSame(1, $result['code']);
        $this->assertStringContainsString('cannot read', $result['err']);
    }
}
