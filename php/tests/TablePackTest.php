<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversClass;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\TablePack;

/**
 * The standalone table pack: the columnar idea on its own, with no envelope and
 * no catalogue.
 */
#[CoversClass(TablePack::class)]
final class TablePackTest extends TestCase
{
    public function testPackFlattensRecordsAndUnpackRestoresThem(): void
    {
        $records = [['id' => 1, 'name' => 'Ada'], ['id' => 2, 'name' => 'Bob']];
        $packed = TablePack::pack($records);

        $this->assertSame(
            '{"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}',
            Json::encode($packed)
        );
        $this->assertSame(Json::encode($records), Json::encode(TablePack::unpack($packed, true)));
    }

    public function testItReadsObjectsAndAssociativeArraysAlike(): void
    {
        $fromArrays = TablePack::pack([['a' => 1], ['a' => 2]]);
        $fromObjects = TablePack::pack([(object) ['a' => 1], (object) ['a' => 2]]);

        $this->assertSame(Json::encode($fromArrays), Json::encode($fromObjects));
    }

    public function testPackFillsAMissingFieldWithNull(): void
    {
        $packed = TablePack::pack([['a' => 1], ['b' => 2]]);

        $this->assertSame(['a', 'b'], $packed->fields);
        $this->assertSame([[1, null], [null, 2]], $packed->data);
    }

    public function testFieldsAreTheUnionInFirstSeenOrder(): void
    {
        $packed = TablePack::pack([['b' => 1], ['a' => 2, 'c' => 3], ['b' => 4, 'a' => 5]]);

        $this->assertSame(['b', 'a', 'c'], $packed->fields);
    }

    public function testPackLeavesAnythingThatIsNotARecordSetAlone(): void
    {
        $this->assertNull(TablePack::pack(null));
        $this->assertSame('text', TablePack::pack('text'));

        $mixed = [1, ['a' => 2]];
        $this->assertSame($mixed, TablePack::pack($mixed));

        $this->assertSame('{"fields":[],"data":[],"count":0}', Json::encode(TablePack::pack([])));
    }

    public function testUnpackLeavesAnythingThatIsNotAPackedTableAlone(): void
    {
        $this->assertNull(TablePack::unpack(null));
        $this->assertSame(42, TablePack::unpack(42));

        $notPacked = (object) ['fields' => 'no', 'data' => []];
        $this->assertSame($notPacked, TablePack::unpack($notPacked));

        $this->assertSame([], TablePack::unpack((object) ['fields' => [], 'data' => [], 'count' => 0]));
    }

    public function testAShortRowIsMissingItsLastFieldsRatherThanHoldingNulls(): void
    {
        $restored = TablePack::unpack((object) ['fields' => ['a', 'b'], 'data' => [[1]]], true);

        $this->assertSame([['a' => 1]], $restored);
    }

    public function testIsPackedRecognisesTheShape(): void
    {
        $this->assertTrue(TablePack::isPacked(TablePack::pack([['a' => 1]])));
        $this->assertFalse(TablePack::isPacked([['a' => 1]]));
        $this->assertFalse(TablePack::isPacked(null));
        $this->assertFalse(TablePack::isPacked((object) ['fields' => 'no', 'data' => []]));
    }

    public function testPackAllAndUnpackAllHandleSeveralResultSets(): void
    {
        $sets = [[['a' => 1], ['a' => 2]], [['b' => 3]]];
        $packed = TablePack::packAll($sets);

        $this->assertCount(2, $packed);
        $this->assertSame(2, $packed[0]->count);
        $this->assertSame(Json::encode($sets), Json::encode(TablePack::unpackAll($packed, true)));
    }

    public function testPackAllLeavesAnythingElseAlone(): void
    {
        $this->assertNull(TablePack::packAll(null));
        $this->assertSame([], TablePack::packAll([]));
        $this->assertSame('text', TablePack::packAll('text'));
        $this->assertNull(TablePack::unpackAll(null));
    }

    public function testAnArrayWhoseRowsDisagreeAboutKeyOrderStillPacks(): void
    {
        // Unlike a table node, the plain pack has one field list and fills the
        // gaps, so contradicting key orders are not a problem for it.
        $value = [['a' => 1, 'b' => 2], ['b' => 3, 'a' => 4]];

        $this->assertStringContainsString('fields', Json::encode(TablePack::pack($value)));
        $this->assertSame(
            '[{"a":1,"b":2},{"a":4,"b":3}]',
            Json::encode(TablePack::unpack(TablePack::pack($value)))
        );
    }

    public function testAKeyCalledProtoIsOrdinaryData(): void
    {
        $records = [['__proto__' => 1, 'a' => 2], ['__proto__' => 3, 'a' => 4]];

        $this->assertSame(
            Json::encode($records),
            Json::encode(TablePack::unpack(TablePack::pack($records), true))
        );
    }

    public function testTextToArrayReadsABracketedIntegerColumn(): void
    {
        $record = TablePack::textToArray(['ids' => '[[[1,2,3]]]'], 'ids');

        $this->assertSame([1, 2, 3], $record['ids']);
    }

    public function testTextToArrayTurnsANullColumnIntoAnEmptyList(): void
    {
        $this->assertSame([], TablePack::textToArray(['ids' => null], 'ids')['ids']);
        $this->assertSame([], TablePack::textToArray(['ids' => '[[[]]]'], 'ids')['ids']);
    }

    public function testTextToArrayTakesTheDelimiters(): void
    {
        $record = TablePack::textToArray(['ids' => '{1|2}'], 'ids', prefix: 1, suffix: 1, separator: '|');

        $this->assertSame([1, 2], $record['ids']);
    }

    public function testTextToArrayLeavesAFieldThatIsNotThereAlone(): void
    {
        $this->assertSame(['other' => 1], TablePack::textToArray(['other' => 1], 'ids'));
    }

    public function testTextToArrayWorksOnAnObjectToo(): void
    {
        $record = TablePack::textToArray((object) ['ids' => '[[[4,5]]]'], 'ids');

        $this->assertSame([4, 5], $record->ids);
    }
}
