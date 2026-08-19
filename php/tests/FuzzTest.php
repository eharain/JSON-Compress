<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversNothing;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\CompressOptions;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use TechStyle\JsonCompress\Normalizer;
use Throwable;

/**
 * Randomised round-trip testing.
 *
 * Hand-written cases only cover the shapes somebody thought of. This generates
 * documents from a seeded pseudo-random source - so a failure is reproducible
 * from the seed printed with it - and insists that every one survives the round
 * trip byte for byte against what JSON itself would produce.
 */
#[CoversNothing]
final class FuzzTest extends TestCase
{
    private const ROUNDS = 400;

    public function testRandomDocumentsSurviveTheRoundTrip(): void
    {
        for ($seed = 1; $seed <= self::ROUNDS; $seed++) {
            $value = (new Generator($seed))->makeValue(5);
            $expected = Json::encode(Normalizer::normalize($value));

            try {
                $restored = Json::encode(JsonCompress::decompress(JsonCompress::compress($value)));
            } catch (Throwable $error) {
                $this->fail('seed ' . $seed . ' threw ' . get_class($error) . ': ' . $error->getMessage());
            }

            $this->assertSame($expected, $restored, 'seed ' . $seed);
        }
    }

    public function testRandomDocumentsSurviveEveryCombinationOfOptions(): void
    {
        $combinations = [];
        foreach ([true, false] as $strings) {
            foreach ([true, false] as $tables) {
                foreach ([true, false] as $constants) {
                    foreach ([true, false] as $dictColumns) {
                        $combinations[] = new CompressOptions(
                            strings: $strings,
                            tables: $tables,
                            constants: $constants,
                            dictColumns: $dictColumns,
                        );
                    }
                }
            }
        }

        for ($seed = 1; $seed <= 60; $seed++) {
            $value = (new Generator($seed))->makeValue(5);
            $expected = Json::encode(Normalizer::normalize($value));

            foreach ($combinations as $options) {
                $restored = Json::encode(JsonCompress::decompress(JsonCompress::compress($value, $options)));
                $this->assertSame(
                    $expected,
                    $restored,
                    'seed ' . $seed . ' with ' . json_encode($options->toArray())
                );
            }
        }
    }

    public function testTheTextRoundTripAgreesWithTheValueRoundTrip(): void
    {
        for ($seed = 1; $seed <= 120; $seed++) {
            $value = (new Generator($seed))->makeValue(5);
            $text = Json::encode(Normalizer::normalize($value));

            // Everything the format promises, expressed on text alone: parse,
            // compress, serialise, parse, decompress, serialise - back to where
            // it started.
            $this->assertSame($text, Json::encode(JsonCompress::parse(JsonCompress::stringify($value))), 'seed ' . $seed);
            $this->assertSame($text, JsonCompress::minify($text), 'seed ' . $seed . ' minify');
        }
    }
}
