<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use stdClass;

/**
 * The shared document generator.
 *
 * Kept character for character with the one in `javascript/tests/fuzz.test.js`,
 * so a seed means the same document in every language that reproduces it. That
 * is what lets the conformance corpus be described by its seeds rather than only
 * by its contents, and what makes a fuzz failure reproducible across ports.
 */
final class Generator
{
    /** @var list<string> */
    public const WORDS = [
        'id', 'name', 'status', 'created', 'updated', 'owner', 'tags', 'value',
        'count', 'total', 'items', 'meta', 'type', 'code', 'label', 'notes',
        '~tilde', '0', '1', '10', 'a', 'z', 'A', '~t', '~r', 'toString',
        '__proto__', 'constructor', 'hasOwnProperty', '', ' ', 'ключ', '键',
    ];

    /** @var list<string> */
    public const STRINGS = [
        '', 'a', 'active', 'pending', 'cancelled', '~', '~~', '~0', '~zz',
        'a longer value that repeats across many records',
        "quote \" backslash \\ newline \n tab \t",
        "\u{1F600} emoji", 'مرحبا', 'null', 'true', '0',
    ];

    /** The state of the small deterministic generator, so failures can be replayed. */
    private int $state;

    public function __construct(int $seed)
    {
        $this->state = $seed & 0xFFFFFFFF;
    }

    /** The next value in [0, 1). */
    public function next(): float
    {
        $this->state = ($this->state * 1664525 + 1013904223) & 0xFFFFFFFF;

        return $this->state / 4294967296;
    }

    /**
     * @param list<mixed> $values
     */
    public function pick(array $values): mixed
    {
        return $values[(int) ($this->next() * count($values))];
    }

    public function makeScalar(): mixed
    {
        $roll = $this->next();
        if ($roll < 0.2) {
            return null;
        }
        if ($roll < 0.3) {
            return $this->next() < 0.5;
        }
        if ($roll < 0.55) {
            return (int) ($this->next() * 2000) - 1000;
        }
        if ($roll < 0.62) {
            return $this->next() * 1e6 - 5e5;
        }

        return $this->pick(self::STRINGS);
    }

    public function makeValue(int $depth): mixed
    {
        if ($depth <= 0) {
            return $this->makeScalar();
        }

        $roll = $this->next();

        if ($roll < 0.45) {
            return $this->makeScalar();
        }

        if ($roll < 0.6) {
            $length = (int) ($this->next() * 6);
            $out = [];
            for ($at = 0; $at < $length; $at++) {
                $out[] = $this->makeValue($depth - 1);
            }

            return $out;
        }

        if ($roll < 0.8) {
            // A record set: the shape the format is built for, with ragged rows
            // and low-cardinality columns thrown in on purpose.
            $rows = (int) ($this->next() * 8);
            $columns = [];
            $columnCount = 1 + (int) ($this->next() * 5);
            for ($at = 0; $at < $columnCount; $at++) {
                $columns[] = (string) $this->pick(self::WORDS);
            }

            $out = [];
            for ($row = 0; $row < $rows; $row++) {
                $record = new stdClass();
                foreach ($columns as $column) {
                    if ($this->next() < 0.15) {
                        continue;
                    }
                    $record->{$column} = $this->next() < 0.5
                        ? $this->pick(self::STRINGS)
                        : $this->makeValue($depth - 2);
                }
                $out[] = $record;
            }

            return $out;
        }

        $size = (int) ($this->next() * 6);
        $out = new stdClass();
        for ($at = 0; $at < $size; $at++) {
            $out->{(string) $this->pick(self::WORDS)} = $this->makeValue($depth - 1);
        }

        return $out;
    }
}
