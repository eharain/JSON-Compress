<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversClass;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\Json;
use TechStyle\JsonCompress\JsonCompress;
use TechStyle\JsonCompress\Locator;

/**
 * Saying exactly where a JSON document goes wrong.
 */
#[CoversClass(Locator::class)]
final class LocateTest extends TestCase
{
    /** @return iterable<string,array{string}> */
    public static function validDocuments(): iterable
    {
        $documents = [
            '{}', '[]', 'null', 'true', 'false', '0', '-0', '1e10', '-1.5E-3', '"x"',
            '  {  "a" : [ 1 , 2 ] }  ', '{"a":{"b":{"c":[]}}}', '"\\u00e9\\n\\t\\\\"',
            '{"": 1}', '[[[[[1]]]]]', '{"a": 1e+10}', '"' . "\u{1F600}" . '"',
            '{"a":[{"b":[1,2,{"c":null}]}]}', '[1,2,3]', '{"a":"~"}',
        ];

        foreach ($documents as $text) {
            yield $text => [$text];
        }
    }

    #[DataProvider('validDocuments')]
    public function testLocateAgreesWithTheParserAboutWhatIsValid(string $text): void
    {
        $this->assertTrue(JsonCompress::locate($text)->valid, $text . ' should be valid');
        Json::decode($text);
    }

    public function testValidateReportsAValidDocument(): void
    {
        $found = JsonCompress::validate('{"a":1}');

        $this->assertTrue($found->valid);
        $this->assertSame('valid JSON', (string) $found);
    }

    /** @return iterable<string,array{string,string,int,int}> */
    public static function brokenDocuments(): iterable
    {
        $cases = [
            ["{'a': 1}", 'double quotes', 1, 2],
            ['{"a": 1,}', 'trailing comma', 1, 8],
            ['[1, 2,]', 'trailing comma', 1, 6],
            ['{"a": True}', 'lower case', 1, 7],
            ['{"a": None}', 'use null', 1, 7],
            ['{"a": NaN}', 'no NaN or Infinity', 1, 7],
            ['{"a": 01}', 'leading zero', 1, 7],
            ['{"a": 1.}', 'decimal point', 1, 8],
            ['{"a": 1e}', 'exponent', 1, 8],
            ['{"a": "unclosed', 'never closed', 1, 7],
            ['{"a": 1', 'never closed', 1, 8],
            ['[1, 2', 'never closed', 1, 6],
            ['{a: 1}', 'quoted property name', 1, 2],
            ['{"a" 1}', 'colon', 1, 6],
            ['{"a": 1} extra', 'after the end', 1, 10],
            ['', 'ends here', 1, 1],
            ['{"a": "\\q"}', 'not a valid escape', 1, 8],
            ['{"a": "\\u12"}', 'four hexadecimal', 1, 8],
            ['{"a": -}', 'expected a digit', 1, 8],
            ['["a" "b"]', 'comma or a closing bracket', 1, 6],
            ['{"a": 1 "b": 2}', 'comma or a closing brace', 1, 9],
        ];

        foreach ($cases as $case) {
            yield $case[0] => $case;
        }
    }

    #[DataProvider('brokenDocuments')]
    public function testLocateNamesTheCommonMistakes(
        string $text,
        string $fragment,
        int $line,
        int $column,
    ): void {
        $found = JsonCompress::locate($text);

        $this->assertFalse($found->valid, $text . ' should be invalid');
        $this->assertStringContainsString($fragment, (string) $found->error, $text);
        $this->assertSame($line, $found->line, $text . ' line');
        $this->assertSame($column, $found->column, $text . ' column');
    }

    public function testARawControlCharacterInsideAStringIsNamed(): void
    {
        $found = JsonCompress::locate("{\"a\": \"one\ntwo\"}");

        $this->assertFalse($found->valid);
        $this->assertStringContainsString('raw control character', (string) $found->error);
    }

    public function testLocateFindsTheSameLineOnAMultiLineDocument(): void
    {
        $text = implode("\n", ['{', '  "one": 1,', '  "two": [', '    1,', '    2,', '  ]', '}']);
        $found = JsonCompress::locate($text);

        $this->assertFalse($found->valid);
        $this->assertStringContainsString('trailing comma', (string) $found->error);
        $this->assertSame(5, $found->line);
        $this->assertSame('    2,', $found->excerpt);
    }

    public function testValidateReportsWhereADocumentGoesWrong(): void
    {
        $found = JsonCompress::validate("{\n  \"a\": 1,\n  \"b\": oops\n}");

        $this->assertFalse($found->valid);
        $this->assertSame(3, $found->line, 'the bad token is on line 3');
        $this->assertSame(8, $found->column, 'and starts at column 8');
        $this->assertSame('  "b": oops', $found->excerpt);
    }

    public function testColumnsAreCountedInCharactersNotBytes(): void
    {
        // Four characters before the problem, whatever they cost in bytes.
        $found = JsonCompress::locate('["مرحبا", oops]');

        $this->assertFalse($found->valid);
        $this->assertSame(11, $found->column);
        $this->assertSame(10, $found->position);
    }

    public function testADocumentNestedTooDeeplyIsReportedNotCrashedOn(): void
    {
        $found = JsonCompress::locate(str_repeat('[', 20000) . str_repeat(']', 20000));

        $this->assertFalse($found->valid);
        $this->assertStringContainsString('nested too deeply', (string) $found->error);
    }
}
