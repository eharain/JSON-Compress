<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use InvalidArgumentException;

/**
 * What the encoder is allowed to do.
 *
 * Every knob has a default that is safe on arbitrary input; the defaults are
 * tuned for API responses and log batches, which is where the format earns most
 * of its keep.
 *
 * An instance is immutable, so one can be built at boot and shared by every
 * request that follows. {@see CompressOptions::with()} makes a changed copy.
 */
final class CompressOptions
{
    /** The field names {@see CompressOptions::fromArray()} accepts. */
    private const FIELDS = [
        'strings',
        'tables',
        'constants',
        'dictColumns',
        'minRows',
        'maxHoleRatio',
        'minStringCount',
        'minStringLength',
    ];

    /**
     * @param bool  $strings         De-duplicate repeated string values into a catalogue.
     * @param bool  $tables          Pack arrays of like-shaped objects into columnar tables.
     * @param bool  $constants       Hoist columns whose value never changes out of the rows.
     * @param bool  $dictColumns     Encode low-cardinality string columns as integer indices.
     * @param int   $minRows         Smallest array that may become a table.
     * @param float $maxHoleRatio    Largest share of missing cells a table may carry.
     * @param int   $minStringCount  Times a string must occur before it can be catalogued.
     * @param int   $minStringLength Shortest string that may be catalogued.
     */
    public function __construct(
        public readonly bool $strings = true,
        public readonly bool $tables = true,
        public readonly bool $constants = true,
        public readonly bool $dictColumns = true,
        public readonly int $minRows = 2,
        public readonly float $maxHoleRatio = 0.3,
        public readonly int $minStringCount = 2,
        public readonly int $minStringLength = 1,
    ) {
        // Validated here rather than at the point of use, so a typo surfaces at
        // the call site rather than as a mysteriously large output.
        if ($minRows < 1) {
            throw new InvalidArgumentException('json-compress: minRows must be at least 1');
        }
        if (!($maxHoleRatio >= 0 && $maxHoleRatio <= 1)) {
            throw new InvalidArgumentException('json-compress: maxHoleRatio must be between 0 and 1');
        }
        if ($minStringCount < 2) {
            throw new InvalidArgumentException('json-compress: minStringCount must be at least 2');
        }
        if ($minStringLength < 0) {
            throw new InvalidArgumentException('json-compress: minStringLength must not be negative');
        }
    }

    /**
     * Build from an array of field names, which is what a config file hands you.
     *
     * @param array<string,mixed> $values
     */
    public static function fromArray(array $values): self
    {
        return (new self())->with($values);
    }

    /**
     * A copy with some fields changed, validated like any other instance.
     *
     * @param array<string,mixed> $changes
     */
    public function with(array $changes): self
    {
        $unknown = array_diff(array_keys($changes), self::FIELDS);
        if ($unknown !== []) {
            sort($unknown);

            throw new InvalidArgumentException(
                'json-compress: unknown option ' . implode(', ', $unknown)
                . ' (expected one of ' . implode(', ', self::FIELDS) . ')'
            );
        }

        $merged = array_merge($this->toArray(), $changes);

        return new self(
            strings: (bool) $merged['strings'],
            tables: (bool) $merged['tables'],
            constants: (bool) $merged['constants'],
            dictColumns: (bool) $merged['dictColumns'],
            minRows: (int) $merged['minRows'],
            maxHoleRatio: (float) $merged['maxHoleRatio'],
            minStringCount: (int) $merged['minStringCount'],
            minStringLength: (int) $merged['minStringLength'],
        );
    }

    /** @return array<string,bool|int|float> */
    public function toArray(): array
    {
        return [
            'strings' => $this->strings,
            'tables' => $this->tables,
            'constants' => $this->constants,
            'dictColumns' => $this->dictColumns,
            'minRows' => $this->minRows,
            'maxHoleRatio' => $this->maxHoleRatio,
            'minStringCount' => $this->minStringCount,
            'minStringLength' => $this->minStringLength,
        ];
    }

    /**
     * Turn whatever a caller passed into a validated instance.
     *
     * @param self|array<string,mixed>|null $options
     */
    public static function resolve(self|array|null $options): self
    {
        if ($options === null) {
            return self::defaults();
        }
        if ($options instanceof self) {
            return $options;
        }

        return self::fromArray($options);
    }

    /** The options used when none are given. */
    public static function defaults(): self
    {
        static $defaults = null;

        return $defaults ??= new self();
    }
}
