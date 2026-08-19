<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use DateTimeImmutable;
use InvalidArgumentException;
use JsonException;
use JsonSerializable;
use PHPUnit\Framework\Attributes\CoversNothing;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\CompressOptions;
use TechStyle\JsonCompress\DecodeError;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use TechStyle\JsonCompress\Sizes;

/**
 * The public surface: what each entry point promises, and what it refuses.
 */
#[CoversNothing]
final class ApiTest extends TestCase
{
    /* ------------------------------------------------------------------ *
     * The envelope
     * ------------------------------------------------------------------ */

    public function testTheEnvelopeCarriesTheFieldsTheSpecificationRequires(): void
    {
        $envelope = JsonCompress::compress(Json::decode('{"a":1}'));

        $this->assertSame(1, $envelope->jc);
        $this->assertSame(['a'], $envelope->k);
        $this->assertFalse(property_exists($envelope, 's'), 'an empty catalogue is left out');
        $this->assertTrue(property_exists($envelope, 'd'));
    }

    public function testTheEnvelopeFieldsComeOutInTheDocumentedOrder(): void
    {
        $text = JsonCompress::stringify(Json::decode('["repeated","repeated","repeated"]'));

        $this->assertStringStartsWith('{"jc":1,"k":[],"s":[', $text);
    }

    public function testIsCompressedRecognisesOnlyRealEnvelopes(): void
    {
        $this->assertTrue(JsonCompress::isCompressed(JsonCompress::compress([1, 2])));
        $this->assertFalse(JsonCompress::isCompressed(null));
        $this->assertFalse(JsonCompress::isCompressed([1, 2]));
        $this->assertFalse(JsonCompress::isCompressed(Json::decode('{"jc":1}')), 'no key catalogue');
        $this->assertFalse(JsonCompress::isCompressed(Json::decode('{"jc":2,"k":[]}')), 'another version');
        $this->assertFalse(JsonCompress::isCompressed(Json::decode('{"k":[],"d":1}')), 'no version');
    }

    public function testDecompressIfNeededPassesUncompressedValuesThrough(): void
    {
        $plain = Json::decode('{"a":1}');

        $this->assertSame($plain, JsonCompress::decompressIfNeeded($plain));
        $this->assertSame(
            '{"a":1}',
            Json::encode(JsonCompress::decompressIfNeeded(JsonCompress::compress($plain)))
        );
    }

    /* ------------------------------------------------------------------ *
     * What the decoder refuses
     * ------------------------------------------------------------------ */

    public function testDecompressRefusesWhatIsNotAnEnvelope(): void
    {
        foreach (['null', '1', '"text"', '[]', '{}', '{"k":[]}', '{"jc":1}'] as $text) {
            $caught = null;
            try {
                JsonCompress::decompress(Json::decode($text));
            } catch (DecodeError $error) {
                $caught = $error;
            }

            $this->assertNotNull($caught, $text . ' should not have decoded');
            $this->assertStringStartsWith('json-compress:', $caught->getMessage());
        }
    }

    public function testDecompressRefusesAVersionItDoesNotKnow(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('format version 2 is not supported');
        JsonCompress::decompress(Json::decode('{"jc":2,"k":[],"d":null}'));
    }

    public function testDecompressRefusesAVersionThatIsNotANumber(): void
    {
        // PHP would call the string "1" equal to 1. The format would not.
        foreach (['{"jc":"1","k":[],"d":null}', '{"jc":true,"k":[],"d":null}'] as $text) {
            $value = Json::decode($text);

            $this->assertFalse(JsonCompress::isCompressed($value), $text);

            try {
                JsonCompress::decompress($value);
                $this->fail($text . ' should not have decoded');
            } catch (DecodeError $error) {
                $this->assertStringContainsString('not a number', $error->getMessage());
            }
        }
    }

    public function testDecompressRefusesAKeyTokenOutOfRange(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('key token "B" is out of range');
        JsonCompress::decompress(Json::decode('{"jc":1,"k":["a"],"d":{"B":1}}'));
    }

    public function testDecompressRefusesAStringReferenceOutOfRange(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('is out of range');
        JsonCompress::decompress(Json::decode('{"jc":1,"k":[],"s":["x"],"d":"~5"}'));
    }

    public function testDecompressRefusesALoneHoleOutsideATableRow(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('hole marker appeared outside a table row');
        JsonCompress::decompress(Json::decode('{"jc":1,"k":[],"d":"~"}'));
    }

    public function testDecompressRefusesAnEncodingListThatDoesNotMatchItsColumns(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('encoding list does not match');
        JsonCompress::decompress(Json::decode('{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~e":[0],"~r":[[1,2]]}}'));
    }

    public function testDecompressRefusesAColumnIndexOutOfRange(): void
    {
        $this->expectException(DecodeError::class);
        $this->expectExceptionMessage('column index 3 is out of range');
        JsonCompress::decompress(Json::decode('{"jc":1,"k":["a"],"d":{"~t":[3],"~r":[[1]]}}'));
    }

    public function testDecompressAcceptsATrimmedRow(): void
    {
        $restored = JsonCompress::decompress(
            Json::decode('{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~r":[[1,2],[3]]}}')
        );

        $this->assertSame('[{"a":1,"b":2},{"a":3}]', Json::encode($restored));
    }

    public function testDecompressIgnoresAnEnvelopeFieldItDoesNotKnow(): void
    {
        // Section 2: a later version may add a member without breaking older
        // readers on the parts they do understand.
        $restored = JsonCompress::decompress(Json::decode('{"jc":1,"k":["a"],"future":true,"d":{"A":1}}'));

        $this->assertSame('{"a":1}', Json::encode($restored));
    }

    /* ------------------------------------------------------------------ *
     * Options
     * ------------------------------------------------------------------ */

    public function testOptionsRejectNonsense(): void
    {
        $this->assertThrows(fn () => new CompressOptions(minRows: 0), 'minRows must be at least 1');
        $this->assertThrows(fn () => new CompressOptions(maxHoleRatio: 2), 'maxHoleRatio must be between 0 and 1');
        $this->assertThrows(fn () => new CompressOptions(minStringCount: 1), 'minStringCount must be at least 2');
        $this->assertThrows(fn () => new CompressOptions(minStringLength: -1), 'minStringLength must not be negative');
        $this->assertThrows(fn () => CompressOptions::fromArray(['nope' => 1]), 'unknown option nope');
    }

    public function testOptionsAreImmutableAndCopyOnChange(): void
    {
        $defaults = CompressOptions::defaults();
        $changed = $defaults->with(['minRows' => 5]);

        $this->assertSame(2, $defaults->minRows);
        $this->assertSame(5, $changed->minRows);
        $this->assertTrue($changed->strings, 'the other fields carried over');
    }

    public function testACodecBindsItsOptionsOnce(): void
    {
        $codec = JsonCompress::codec(['tables' => false]);
        $value = Json::decode('[{"a":1},{"a":2}]');

        $this->assertStringNotContainsString('~t', $codec->stringify($value));
        $this->assertSame(Json::encode($value), Json::encode($codec->parse($codec->stringify($value))));
        $this->assertFalse($codec->options->tables);
    }

    /* ------------------------------------------------------------------ *
     * Normalisation
     * ------------------------------------------------------------------ */

    public function testJsonSerializableIsHonoured(): void
    {
        $value = new class () implements JsonSerializable {
            public function jsonSerialize(): mixed
            {
                return ['kind' => 'custom'];
            }
        };

        $this->assertSame('{"kind":"custom"}', Json::encode(JsonCompress::decompress(JsonCompress::compress($value))));
    }

    public function testDatesBecomeText(): void
    {
        $when = new DateTimeImmutable('2026-08-19T12:00:00+00:00');
        $restored = JsonCompress::decompress(JsonCompress::compress(['when' => $when]));

        $this->assertSame('{"when":"2026-08-19T12:00:00+00:00"}', Json::encode($restored));
    }

    public function testAPlainObjectContributesItsPublicProperties(): void
    {
        $value = new class () {
            public int $shown = 1;
            protected int $hidden = 2;
            private int $secret = 3;

            public function unused(): int
            {
                return $this->hidden + $this->secret;
            }
        };

        $this->assertSame('{"shown":1}', Json::encode(JsonCompress::decompress(JsonCompress::compress($value))));
    }

    public function testSomethingWithNoJsonFormIsRefused(): void
    {
        $this->assertThrows(
            static fn () => JsonCompress::compress(static fn () => 1),
            'a closure has no JSON form'
        );

        $handle = fopen('php://memory', 'rb');
        try {
            $this->assertThrows(
                static fn () => JsonCompress::compress($handle),
                'has no JSON form'
            );
        } finally {
            fclose($handle);
        }
    }

    public function testNonFiniteNumbersBecomeNullAndNegativeZeroBecomesZero(): void
    {
        $restored = JsonCompress::decompress(JsonCompress::compress([NAN, INF, -INF, -0.0]));

        $this->assertSame('[null,null,null,0]', Json::encode($restored));
    }

    /* ------------------------------------------------------------------ *
     * Text helpers
     * ------------------------------------------------------------------ */

    public function testMinifyAndBeautifyChangeOnlyTheWhitespace(): void
    {
        $text = "{\n  \"a\" : [ 1, 2 ]\n}";

        $this->assertSame('{"a":[1,2]}', JsonCompress::minify($text));
        $this->assertSame("{\n  \"a\": [\n    1,\n    2\n  ]\n}", JsonCompress::beautify($text));
        $this->assertSame("{\n\t\"a\": [\n\t\t1,\n\t\t2\n\t]\n}", JsonCompress::beautify($text, "\t"));
    }

    public function testToJsonAgreesWithJsonStringify(): void
    {
        $this->assertSame('[1,1.5,"a",null,true]', JsonCompress::toJson([1, 1.5, 'a', null, true]));
        $this->assertSame('1e+21', JsonCompress::toJson(1e21));
        $this->assertSame('1', JsonCompress::toJson(1.0));
        $this->assertSame('"a/b"', JsonCompress::toJson('a/b'), 'slashes are not escaped');
    }

    public function testMinifyReportsBrokenJson(): void
    {
        $this->expectException(JsonException::class);
        JsonCompress::minify('{"a":}');
    }

    public function testValidateAcceptsGoodJsonAndPlacesBadJson(): void
    {
        $this->assertTrue(JsonCompress::validate('{"a":1}')->valid);

        $found = JsonCompress::validate("{\n  \"a\": 1,\n}");
        $this->assertFalse($found->valid);
        $this->assertSame(2, $found->line);
        $this->assertStringContainsString('trailing comma', (string) $found->error);
    }

    /* ------------------------------------------------------------------ *
     * Measurement
     * ------------------------------------------------------------------ */

    public function testMeasureReportsAgainstTheMinifiedSize(): void
    {
        $rows = [];
        for ($at = 0; $at < 200; $at++) {
            $rows[] = (object) ['id' => $at, 'status' => 'active', 'label' => 'row number ' . $at];
        }

        $report = JsonCompress::measure($rows);

        $this->assertSame($report->minified, $report->original);
        $this->assertSame($report->minified - $report->compressed, $report->saved);
        $this->assertGreaterThan(0, $report->saved, 'a record set should get smaller');
        $this->assertEqualsWithDelta($report->compressed / $report->minified, $report->ratio, 1e-12);

        if (Sizes::canGzip()) {
            $this->assertNotNull($report->gzipMinified);
            $this->assertNotNull($report->gzipCompressed);
        }
    }

    public function testMeasureCanBeToldWhatTheInputReallyWas(): void
    {
        $source = "{\n  \"a\": 1\n}";
        $report = JsonCompress::measure(Json::decode($source), source: $source, gzip: false);

        $this->assertSame(strlen($source), $report->original);
        $this->assertSame(7, $report->minified);
        $this->assertNull($report->gzipMinified);
    }

    public function testGzipRoundTrips(): void
    {
        if (!Sizes::canGzip()) {
            $this->markTestSkipped('this build of PHP has no zlib');
        }

        $text = str_repeat('the same sentence over and over. ', 100);
        $this->assertSame($text, Sizes::gunzip(Sizes::gzip($text)));
        $this->assertLessThan(strlen($text), Sizes::gzipSize($text));
    }

    public function testByteLengthCountsBytesNotCharacters(): void
    {
        $this->assertSame(3, Sizes::byteLength('abc'));
        $this->assertSame(4, Sizes::byteLength("\u{1F4E6}"));
        $this->assertSame(2, Json::utf16Length("\u{1F4E6}"), 'and utf16Length counts code units');
        $this->assertSame(1, Json::utf16Length('م'));
    }

    /* ------------------------------------------------------------------ *
     * Helpers
     * ------------------------------------------------------------------ */

    private function assertThrows(callable $work, string $message): void
    {
        try {
            $work();
            $this->fail('expected "' . $message . '" but nothing was thrown');
        } catch (InvalidArgumentException $error) {
            $this->assertStringContainsString($message, $error->getMessage());
        }
    }
}
