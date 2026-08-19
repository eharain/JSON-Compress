<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversNothing;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use Throwable;

/**
 * Agreement with the other implementations.
 *
 * `SPEC.md` asks only that an implementation restore what a conforming encoder
 * wrote. This asserts the stronger claim the released implementations already
 * meet: that they all write the same envelope for the same document, byte for
 * byte.
 *
 * The corpus is 144 source/envelope pairs produced by the JavaScript encoder and
 * checked in at the path below, so this runs without Node, .NET or Python
 * installed. It is shared with the .NET and Python suites rather than
 * duplicated, because two copies of a conformance corpus is one copy too many.
 */
#[CoversNothing]
final class ConformanceTest extends TestCase
{
    private const CORPUS = __DIR__
        . '/../../dotnet/tests/TechStyle.JsonCompress.Tests/fixtures/conformance.json';

    /** @var list<array{name:string,source:string,compressed:string,seed?:int}> */
    private static array $cases = [];

    public static function setUpBeforeClass(): void
    {
        if (!is_file(self::CORPUS)) {
            self::markTestSkipped('the conformance corpus is not checked out at ' . self::CORPUS);
        }

        $text = file_get_contents(self::CORPUS);
        self::assertIsString($text);
        self::$cases = json_decode($text, true, 512, JSON_THROW_ON_ERROR)['cases'];
    }

    public function testTheCorpusIsTheSizeItShouldBe(): void
    {
        $this->assertGreaterThanOrEqual(144, count(self::$cases));
    }

    /** Clause 3 of the specification: what a conforming encoder wrote, we read. */
    public function testEveryEnvelopeTheReferenceEncoderWroteIsRestored(): void
    {
        $failures = [];

        foreach (self::$cases as $case) {
            try {
                $restored = Json::encode(JsonCompress::parse($case['compressed']));
            } catch (Throwable $error) {
                $failures[] = $case['name'] . ': threw ' . get_class($error) . ' ' . $error->getMessage();
                continue;
            }

            if ($restored !== $case['source']) {
                $failures[] = $case['name']
                    . "\n  expected " . $case['source']
                    . "\n  actual   " . $restored;
            }
        }

        $this->assertSame([], $failures, implode("\n\n", $failures));
    }

    /** Stronger than the specification asks: the same bytes, not merely the same value. */
    public function testEveryEnvelopeIsWrittenTheSameWay(): void
    {
        $failures = [];

        foreach (self::$cases as $case) {
            $ours = JsonCompress::stringify(Json::decode($case['source']));
            if ($ours !== $case['compressed']) {
                $failures[] = $case['name']
                    . "\n  reference " . $case['compressed']
                    . "\n  php       " . $ours;
            }
        }

        $this->assertSame([], $failures, implode("\n\n", $failures));
    }

    /** The generator is shared, so a seed names the same document everywhere. */
    public function testTheSeededCasesAreReproducedFromTheirSeeds(): void
    {
        $checked = 0;

        foreach (self::$cases as $case) {
            if (!isset($case['seed'])) {
                continue;
            }
            $value = (new Generator($case['seed']))->makeValue(5);
            $this->assertSame($case['source'], Json::encode($value), 'seed ' . $case['seed']);
            $checked++;
        }

        $this->assertGreaterThan(0, $checked, 'the corpus carries no seeded cases');
    }
}
