<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversNothing;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\CompressOptions;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use TechStyle\JsonCompress\Normalizer;
use stdClass;

/**
 * Round-trip tests.
 *
 * The contract the whole library rests on is that decompressing a compressed
 * value gives back that value - not something equivalent, the same document,
 * keys in the same order. Everything here checks that, on shapes chosen to poke
 * at the places where the format could plausibly lose information.
 */
#[CoversNothing]
final class RoundTripTest extends TestCase
{
    /** What JSON itself would make of a value - the standard the format meets. */
    private static function asJson(mixed $value): mixed
    {
        return Json::decode(Json::encode(Normalizer::normalize($value)));
    }

    /** Assert that a value survives compression, keys and all. */
    private function roundTrips(mixed $value, string $note = 'value'): mixed
    {
        $expected = self::asJson($value);
        $restored = JsonCompress::decompress(JsonCompress::compress($value));

        $this->assertSame(
            Json::encode($expected),
            Json::encode($restored),
            $note . ': the document did not come back as it went in'
        );

        return $restored;
    }

    public function testScalarsSurviveIntact(): void
    {
        foreach ([null, true, false, 0, -1, 1.5, 1e21, -1e-7, '', 'plain'] as $value) {
            $this->roundTrips($value, var_export($value, true));
        }
    }

    public function testTheTildeEscapeIsReversible(): void
    {
        foreach (['~', '~~', '~~~', '~0', '~z', '~ref', '~t', '~n', 'a~b'] as $value) {
            $this->roundTrips($value, $value);
        }
        $this->roundTrips(['~', '~', '~', '~0', '~0', '~0'], 'repeated tilde strings');
        $this->roundTrips((object) ['~key' => '~value'], 'tilde in both key and value');
    }

    public function testUnicodeAndEscapesSurvive(): void
    {
        $this->roundTrips(
            (object) [
                'emoji' => "compressed \u{1F4E6} nicely",
                'quote' => 'she said "hello"',
                'backslash' => 'C:\\Users\\path',
                'newline' => "one\ntwo\tthree",
                'surrogate' => "\u{1F600}\u{1F601}",
                'arabic' => 'مرحبا بالعالم',
                'nul' => "a\u{0}b",
            ],
            'awkward text'
        );
    }

    public function testEmptyContainersSurvive(): void
    {
        $this->roundTrips([], 'empty array');
        $this->roundTrips(new stdClass(), 'empty object');
        $this->roundTrips([[], new stdClass(), [[]], [new stdClass()]], 'nested empties');
        $this->roundTrips([new stdClass(), new stdClass(), new stdClass()], 'array of empty objects');
    }

    public function testAnEmptyObjectStaysAnObjectAndAnEmptyArrayStaysAnArray(): void
    {
        // PHP cannot tell the two apart in an array, which is exactly why this
        // library speaks stdClass. The distinction has to survive the trip.
        $this->assertSame('{"jc":1,"k":[],"d":{}}', JsonCompress::stringify(new stdClass()));
        $this->assertSame('{"jc":1,"k":[],"d":[]}', JsonCompress::stringify([]));
    }

    public function testArraysOfLikeShapedObjectsBecomeTablesAndComeBack(): void
    {
        $value = Json::decode('[{"id":1,"name":"Ada","role":"admin"},'
            . '{"id":2,"name":"Bob","role":"admin"},{"id":3,"name":"Cy","role":"user"}]');

        $this->assertStringContainsString('~t', JsonCompress::stringify($value));
        $this->roundTrips($value, 'uniform rows');
    }

    public function testRowsWithDifferentKeySetsSurvive(): void
    {
        $this->roundTrips(Json::decode('[{"a":1},{"b":2},{"a":3,"b":4}]'), 'disjoint keys');
        $this->roundTrips(Json::decode('[{"a":1,"b":2,"c":3},{"a":4},{"a":5,"c":6}]'), 'ragged rows');
    }

    public function testRowsWhoseKeyOrdersDisagreeStayObjects(): void
    {
        $value = Json::decode('[{"a":1,"b":2},{"b":3,"a":4}]');

        $this->assertStringNotContainsString('~t', JsonCompress::stringify($value));
        $this->roundTrips($value, 'contradicting key orders');
    }

    public function testArraysOfArraysAreLeftAlone(): void
    {
        $this->roundTrips([[1, 2], [3, 4], [5, 6]], 'matrix');
        $this->roundTrips(Json::decode('[[{"a":1},{"a":2}],[{"a":3},{"a":4}]]'), 'arrays of tables');
    }

    public function testMixedArraysAreLeftAlone(): void
    {
        $this->roundTrips(Json::decode('[1,"two",null,{"three":3},[4]]'), 'mixed');
        $this->roundTrips(Json::decode('[{"a":1},2,{"a":3}]'), 'objects interrupted by a scalar');
    }

    public function testNestedTablesSurvive(): void
    {
        $this->roundTrips(
            Json::decode('{"groups":[{"name":"a","items":[{"q":1,"p":2},{"q":3,"p":4}]},'
                . '{"name":"b","items":[{"q":5,"p":6},{"q":7,"p":8}]}]}'),
            'tables inside tables'
        );
    }

    public function testDeepNestingSurvives(): void
    {
        $value = (object) ['leaf' => true];
        for ($level = 0; $level < 200; $level++) {
            $value = (object) ['level' . $level => $value];
        }
        $this->roundTrips($value, '200 levels');
    }

    public function testNestingPastTheDepthLimitFailsCleanly(): void
    {
        // Deeper than the walk will go is an exception, not a crash and not a
        // half-written document.
        $value = (object) ['leaf' => true];
        for ($level = 0; $level < Normalizer::MAX_DEPTH + 40; $level++) {
            $value = (object) ['level' . $level => $value];
        }

        $this->expectExceptionMessage('json-compress:');
        JsonCompress::compress($value);
    }

    public function testAnythingPhpItselfCanParseCompressesAndReadsBack(): void
    {
        // PHP's own json_decode stops at 512 levels. Everything it accepts has
        // to survive the whole trip here, envelope and all - which takes some
        // arranging, because an envelope nests deeper than its source.
        $text = str_repeat('[', 511) . str_repeat(']', 511);

        $envelope = JsonCompress::stringify(Json::decode($text));
        $this->assertSame($text, Json::encode(JsonCompress::parse($envelope)));
    }

    public function testTheDeepestCompressibleDocumentReadsBackAsText(): void
    {
        // The worst shape there is for envelope depth: a chain of two-row
        // tables, whose rows matrix adds a level of its own at every step. The
        // envelope ends up half again as deep as the source, and the reader has
        // to cope with that.
        $text = '1';
        for ($at = 0; $at < 300; $at++) {
            $text = '[{"a":' . $text . '},{"a":1}]';
        }

        $envelope = JsonCompress::stringify(Json::decode($text));
        $this->assertStringContainsString('~t', $envelope, 'the tables really were packed');
        $this->assertSame($text, Json::encode(JsonCompress::parse($envelope)));
    }

    public function testTheReaderAcceptsMoreDepthThanTheEncoderWillCompress(): void
    {
        // The relationship the two limits have to keep, stated once so that
        // changing either without the other fails here rather than in the wild.
        $this->assertGreaterThan(
            Normalizer::MAX_DEPTH * 3 / 2 + 1,
            Json::MAX_DEPTH,
            'an envelope nests half again as deep as its source'
        );
        $this->assertGreaterThan(
            512,
            Normalizer::MAX_DEPTH,
            'everything PHP itself parses must be compressible'
        );
    }

    public function testAValueThatRefersToItselfIsRefused(): void
    {
        $value = new stdClass();
        $value->self = $value;

        $this->expectExceptionMessage('circular structure');
        JsonCompress::compress($value);
    }

    public function testAMissingKeyIsNotTheSameAsANullOne(): void
    {
        $value = Json::decode('[{"a":1,"b":null},{"a":2},{"a":3,"b":null},{"a":4,"b":5}]');
        $restored = JsonCompress::decompress(JsonCompress::compress($value));

        $this->assertTrue(property_exists($restored[0], 'b'), 'explicit null kept');
        $this->assertNull($restored[0]->b);
        $this->assertFalse(property_exists($restored[1], 'b'), 'absent key stays absent');
        $this->roundTrips($value, 'holes against nulls');
    }

    public function testAMissingKeyIsNotTheSameAsANullOneInADictionaryColumn(): void
    {
        // Long enough, repeated enough and drawn from few enough values to be
        // worth a dictionary column - a single-character column would not be,
        // and would come out raw.
        $value = Json::decode('[{"k":"alpha","n":1},{"k":null,"n":2},{"n":3},'
            . '{"k":"alpha","n":4},{"k":"beta","n":5},{"k":"alpha","n":6}]');

        $this->assertStringContainsString('"~e":[1,0]', JsonCompress::stringify($value));

        $restored = JsonCompress::decompress(JsonCompress::compress($value));
        $this->assertTrue(property_exists($restored[1], 'k'));
        $this->assertNull($restored[1]->k);
        $this->assertFalse(property_exists($restored[2], 'k'));
        $this->roundTrips($value, 'holes against nulls in a dictionary column');
    }

    public function testARawColumnMayHoldMinusOne(): void
    {
        // -1 is what a dictionary column writes for an absent key. A raw column
        // that happens to hold the number must not be mistaken for one.
        $this->roundTrips(
            Json::decode('[{"a":1,"b":-1},{"a":2,"b":5},{"a":3,"b":-1}]'),
            'minus one in a raw column'
        );
        $this->roundTrips(
            Json::decode('[{"a":-1},{"a":-1},{"a":2}]'),
            'minus one in the last column'
        );
        $this->roundTrips(
            Json::decode('[{"a":1,"b":-1},{"a":2,"b":5},{"a":3,"b":-1},{"a":4},{"a":5,"b":-1}]'),
            'minus one where a row really is short'
        );
    }

    public function testConstantColumnsAreRestoredOnEveryRow(): void
    {
        $value = Json::decode('[{"type":"order","id":1},{"type":"order","id":2},{"type":"order","id":3}]');
        $text = JsonCompress::stringify($value);

        $this->assertStringContainsString('"~cv"', $text);
        $this->assertSame(1, substr_count($text, 'order'), 'the constant is written down once');
        $this->roundTrips($value, 'constant column');
    }

    public function testATableWhereEveryColumnIsConstantKeepsItsRowCount(): void
    {
        $value = Json::decode('[{"a":1,"b":"x"},{"a":1,"b":"x"},{"a":1,"b":"x"},{"a":1,"b":"x"}]');

        $this->assertStringContainsString('"~n":4', JsonCompress::stringify($value));
        $this->roundTrips($value, 'every column constant');
    }

    public function testRepeatedStringsAreCataloguedAndRestored(): void
    {
        $rows = [];
        for ($at = 0; $at < 50; $at++) {
            $rows[] = (object) [
                'id' => $at,
                'country' => $at % 2 ? 'United Kingdom' : 'Republic of Ireland',
                'note' => 'a reasonably long repeated note value',
            ];
        }

        $envelope = JsonCompress::compress($rows);
        $this->assertContains('United Kingdom', $envelope->s);
        $this->roundTrips($rows, 'catalogued strings');

        // The note never changes, so it is hoisted out of the rows entirely and
        // written down exactly once.
        $this->assertSame(
            1,
            substr_count(Json::encode($envelope), 'a reasonably long repeated note value')
        );
    }

    public function testKeysThatLookLikeTokensAreNotConfusedWithTokens(): void
    {
        $this->roundTrips(
            Json::decode('{"0":1,"1":2,"a":3,"z":4,"10":5,"~t":6,"~r":7}'),
            'token-shaped keys'
        );
    }

    public function testAKeyCalledProtoIsOrdinaryData(): void
    {
        $restored = $this->roundTrips(Json::decode('{"__proto__":1,"safe":2}'), '__proto__ as a key');

        $this->assertSame(['__proto__', 'safe'], array_map(
            static fn (int|string $key): string => (string) $key,
            array_keys(get_object_vars($restored))
        ));
    }

    public function testALargeKeySetPushesTokensPastOneCharacter(): void
    {
        $value = new stdClass();
        for ($at = 0; $at < 5000; $at++) {
            $value->{'key_number_' . $at} = $at;
        }
        $this->roundTrips($value, '5000 keys');
    }

    public function testALargeStringCatalogluePushesReferencesPastOneCharacter(): void
    {
        $distinct = [];
        for ($at = 0; $at < 500; $at++) {
            $distinct[] = 'a distinctive value number ' . $at;
        }
        $this->roundTrips(array_merge($distinct, $distinct), '500 repeated strings');
    }

    public function testStringifyAndParseAgreeWithCompressAndDecompress(): void
    {
        $value = Json::decode('{"users":[{"id":1,"name":"Ada"},{"id":2,"name":"Bob"}]}');

        $this->assertSame(Json::encode($value), Json::encode(JsonCompress::parse(JsonCompress::stringify($value))));
        $this->assertSame(Json::encode(JsonCompress::compress($value)), JsonCompress::stringify($value));
    }

    public function testEveryCombinationOfOptionsRoundTrips(): void
    {
        $rows = [];
        for ($at = 0; $at < 20; $at++) {
            $row = (object) [
                'id' => $at,
                'status' => $at % 2 ? 'open' : 'closed',
                'fixed' => 'always the same',
            ];
            if ($at % 5 !== 0) {
                $row->note = 'note ' . ($at % 3);
            }
            $rows[] = $row;
        }
        $value = (object) ['rows' => $rows];
        $expected = Json::encode(self::asJson($value));

        foreach ([true, false] as $strings) {
            foreach ([true, false] as $tables) {
                foreach ([true, false] as $constants) {
                    foreach ([true, false] as $dictColumns) {
                        $options = new CompressOptions(
                            strings: $strings,
                            tables: $tables,
                            constants: $constants,
                            dictColumns: $dictColumns,
                        );
                        $this->assertSame(
                            $expected,
                            Json::encode(JsonCompress::decompress(JsonCompress::compress($value, $options))),
                            json_encode($options->toArray())
                        );
                    }
                }
            }
        }
    }

    public function testMinRowsControlsWhenATableIsBuilt(): void
    {
        $two = Json::decode('[{"a":1},{"a":2}]');

        $this->assertStringContainsString('~t', JsonCompress::stringify($two, ['minRows' => 2]));
        $this->assertStringNotContainsString('~t', JsonCompress::stringify($two, ['minRows' => 3]));
        $this->assertSame(
            Json::encode($two),
            Json::encode(JsonCompress::decompress(JsonCompress::compress($two, ['minRows' => 3])))
        );
    }

    public function testMaxHoleRatioKeepsVerySparseArraysOutOfTables(): void
    {
        $sparse = Json::decode('[{"a":1},{"b":2},{"c":3},{"d":4},{"e":5}]');

        $this->assertStringNotContainsString('~t', JsonCompress::stringify($sparse));
        $this->roundTrips($sparse, 'sparse rows');
        $this->assertSame(
            Json::encode($sparse),
            Json::encode(JsonCompress::decompress(JsonCompress::compress($sparse, ['maxHoleRatio' => 1])))
        );
    }

    public function testAssociativeArraysAreReadAsObjectsAndListsAsArrays(): void
    {
        $this->assertSame(
            '{"jc":1,"k":["id","name"],"d":{"A":1,"B":"Ada"}}',
            JsonCompress::stringify(['id' => 1, 'name' => 'Ada'])
        );
        $this->assertSame(
            '{"jc":1,"k":["id"],"d":{"~t":[0],"~e":[2],"~cv":[1],"~n":2}}',
            JsonCompress::stringify([['id' => 1], ['id' => 1]])
        );
    }
}
