<?php

/**
 * What the format is worth, on shapes that occur in real systems.
 *
 *     php examples/benchmark.php
 *
 * Every figure in the README is produced by running this file, so it can be
 * checked rather than taken on trust. The interesting column is the last one:
 * whether the format still helps once gzip has had its turn, which is the only
 * question that matters for anything served over HTTP.
 *
 * The generator and the scenarios are the same as in `javascript/examples/` and
 * `python/examples/`, so the first four columns can be compared line for line
 * with the JavaScript, .NET and Python benchmarks.
 */

declare(strict_types=1);

namespace TechStyle\JsonCompress\Examples;

use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use TechStyle\JsonCompress\Sizes;

require is_file(__DIR__ . '/../vendor/autoload.php')
    ? __DIR__ . '/../vendor/autoload.php'
    : __DIR__ . '/../autoload.php';

/** A deterministic generator, so the numbers are the same on every machine. */
final class Source
{
    private const SEED = 20260818;

    private int $state = self::SEED;

    public function reset(): void
    {
        $this->state = self::SEED;
    }

    public function next(): float
    {
        $this->state = ($this->state * 1664525 + 1013904223) & 0xFFFFFFFF;

        return $this->state / 4294967296;
    }

    /** @param list<mixed> $values */
    public function pick(array $values): mixed
    {
        return $values[(int) ($this->next() * count($values))];
    }

    public function integer(int $low, int $high): int
    {
        return $low + (int) ($this->next() * ($high - $low + 1));
    }

    /** Round half away from zero towards positive infinity, as JavaScript does. */
    public function rounded(float $value): int
    {
        return (int) floor($value + 0.5);
    }
}

$random = new Source();

const FIRST = ['Amara', 'Bilal', 'Chen', 'Dara', 'Eesa', 'Farah', 'Gita', 'Hana', 'Idris', 'Jae'];
const LAST = ['Ahmed', 'Bright', 'Chowdhury', 'Doyle', 'Evans', 'Fischer', 'Gomez', 'Haddad'];
const CITY = ['London', 'Manchester', 'Birmingham', 'Leeds', 'Glasgow', 'Cardiff', 'Belfast'];

$apiRecordSet = static function () use ($random): array {
    $out = [];
    for ($at = 0; $at < 1000; $at++) {
        $out[] = (object) [
            'id' => $at + 1,
            'firstName' => $random->pick(FIRST),
            'lastName' => $random->pick(LAST),
            'email' => 'user' . ($at + 1) . '@example.com',
            'role' => $random->pick(['admin', 'editor', 'viewer', 'viewer', 'viewer']),
            'active' => $random->next() > 0.2,
            'city' => $random->pick(CITY),
            'country' => 'United Kingdom',
            'createdAt' => '2026-0' . $random->integer(1, 9) . '-'
                . $random->integer(10, 28) . 'T09:30:00.000Z',
            'loginCount' => $random->integer(0, 4000),
        ];
    }

    return $out;
};

$nestedApiResponse = static function () use ($random): object {
    // The lines are drawn inside the object literal rather than before it, so
    // that the generator is consumed in the same order as in the JavaScript,
    // .NET and Python benchmarks and the documents come out identical.
    $makeLines = static function () use ($random): array {
        $lines = [];
        $lineCount = $random->integer(1, 6);
        for ($line = 0; $line < $lineCount; $line++) {
            $lines[] = (object) [
                'sku' => 'SKU-' . $random->integer(1000, 9999),
                'description' => $random->pick(
                    ['Cotton shirt', 'Wool scarf', 'Leather belt', 'Linen trousers']
                ),
                'quantity' => $random->integer(1, 4),
                'unitPrice' => $random->integer(500, 12000) / 100,
                'taxRate' => 0.2,
            ];
        }

        return $lines;
    };

    $data = [];
    for ($at = 0; $at < 200; $at++) {
        $data[] = (object) [
            'orderId' => 'ORD-' . (100000 + $at),
            'status' => $random->pick(['placed', 'picked', 'shipped', 'delivered']),
            'currency' => 'GBP',
            'placedAt' => '2026-08-' . $random->integer(10, 18) . 'T'
                . $random->integer(10, 20) . ':15:00.000Z',
            'customer' => (object) [
                'id' => $random->integer(1, 5000),
                'name' => $random->pick(FIRST) . ' ' . $random->pick(LAST),
                'city' => $random->pick(CITY),
            ],
            'lines' => $makeLines(),
        ];
    }

    return (object) [
        'meta' => (object) [
            'page' => 1,
            'pageSize' => 200,
            'total' => 8214,
            'generatedAt' => '2026-08-18T12:00:00.000Z',
        ],
        'data' => $data,
    ];
};

$timeSeries = static function () use ($random): array {
    $out = [];
    for ($at = 0; $at < 2000; $at++) {
        $out[] = (object) [
            't' => 1755500000 + $at * 60,
            'sensor' => $random->pick(['temp-01', 'temp-02', 'temp-03', 'humidity-01']),
            'unit' => 'celsius',
            'value' => $random->rounded($random->next() * 3500) / 100,
            'quality' => $random->pick(['good', 'good', 'good', 'suspect']),
        ];
    }

    return $out;
};

$structuredLogs = static function () use ($random): array {
    $out = [];
    for ($at = 0; $at < 1500; $at++) {
        $out[] = (object) [
            'level' => $random->pick(['info', 'info', 'info', 'warn', 'error']),
            'service' => $random->pick(['api-gateway', 'orders', 'billing', 'search']),
            'env' => 'production',
            'region' => 'eu-west-2',
            'message' => $random->pick([
                'request completed',
                'cache miss',
                'upstream timeout, retrying',
                'validation failed',
            ]),
            'durationMs' => $random->integer(1, 4000),
            'traceId' => '00-' . $random->integer(100000000, 900000000) . '-'
                . $random->integer(1000000, 9000000) . '-01',
        ];
    }

    return $out;
};

$geojson = static function () use ($random): object {
    $features = [];
    for ($at = 0; $at < 400; $at++) {
        $features[] = (object) [
            'type' => 'Feature',
            'geometry' => (object) [
                'type' => 'Point',
                'coordinates' => [
                    $random->rounded(($random->next() * 8 - 6) * 1e6) / 1e6,
                    $random->rounded((49 + $random->next() * 9) * 1e6) / 1e6,
                ],
            ],
            'properties' => (object) [
                'id' => $at,
                'name' => $random->pick(CITY) . ' site ' . $at,
                'category' => $random->pick(['depot', 'store', 'hub']),
            ],
        ];
    }

    return (object) ['type' => 'FeatureCollection', 'features' => $features];
};

$configDocument = static function () use ($random): object {
    $services = new \stdClass();
    for ($at = 0; $at < 12; $at++) {
        $services->{'service-' . $at} = (object) [
            'image' => 'registry.example.com/team/service-' . $at . ':1.4.' . $at,
            'replicas' => $random->integer(1, 6),
            'resources' => (object) ['cpu' => '500m', 'memory' => '512Mi'],
            'env' => (object) [
                'NODE_ENV' => 'production',
                'LOG_LEVEL' => 'info',
                'REGION' => 'eu-west-2',
            ],
            'ports' => [(object) ['container' => 8080, 'published' => 8080 + $at, 'protocol' => 'tcp']],
        ];
    }

    return (object) ['version' => 3, 'services' => $services];
};

$scenarios = [
    ['API record set (1,000 users)', $apiRecordSet],
    ['Nested API response (orders with lines)', $nestedApiResponse],
    ['Time series (2,000 readings)', $timeSeries],
    ['Structured logs (1,500 lines)', $structuredLogs],
    ['GeoJSON feature collection (400 points)', $geojson],
    ['Config document (deeply nested, few repeats)', $configDocument],
];

$kilobytes = static fn (int $count): string => sprintf('%.1f', $count / 1024);
$left = static fn (string $text): string => str_pad($text, 46);
$right = static fn (string $text, int $width): string => str_pad($text, $width, ' ', STR_PAD_LEFT);

$header = $left('scenario') . $right('minified', 10) . $right('compressed', 12)
    . $right('saved', 8) . $right('+gzip', 10) . $right('saved', 8);

echo "\n", $header, "\n", str_repeat('-', strlen($header)), "\n";

foreach ($scenarios as [$name, $build]) {
    $random->reset();
    $value = $build();

    $minified = Json::encode($value);
    $packed = JsonCompress::stringify($value);

    // A benchmark that quietly loses data is not a benchmark.
    if (Json::encode(JsonCompress::parse($packed)) !== $minified) {
        fwrite(STDERR, $name . " did not round-trip\n");
        exit(1);
    }

    $minifiedBytes = Sizes::byteLength($minified);
    $packedBytes = Sizes::byteLength($packed);
    $minifiedGzip = Sizes::gzipSize($minified);
    $packedGzip = Sizes::gzipSize($packed);

    $saved = sprintf('%.0f%%', (1 - $packedBytes / $minifiedBytes) * 100);
    $savedGzip = $minifiedGzip === null || $packedGzip === null
        ? 'n/a'
        : sprintf('%.0f%%', (1 - $packedGzip / $minifiedGzip) * 100);

    echo $left($name),
        $right($kilobytes($minifiedBytes) . ' KB', 10),
        $right($kilobytes($packedBytes) . ' KB', 12),
        $right($saved, 8),
        $right($packedGzip === null ? 'n/a' : $kilobytes($packedGzip) . ' KB', 10),
        $right($savedGzip, 8),
        "\n";
}

echo "\n\"saved\" is against minified JSON; the gzip columns compare compressed + gzip\n";
echo "against minified + gzip, which is what an HTTP response actually costs.\n\n";
