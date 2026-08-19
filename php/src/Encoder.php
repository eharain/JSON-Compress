<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use stdClass;

/**
 * The encoder.
 *
 * Two passes. The first walks the document and counts things: how often each
 * key is used, how often each string value appears, and which arrays are worth
 * turning into tables. The second builds the output using catalogues ordered by
 * those counts, so the most-used key gets the one-character token.
 *
 * Splitting it this way costs a second walk and buys a materially smaller
 * document, because token length is decided by frequency rather than by the
 * accident of which key the encoder happened to meet first.
 *
 * The first pass hands its findings to the second as a prepared tree, in which
 * every array it chose to pack has become a {@see PlannedTable}. The other
 * implementations pass that decision along in a map keyed by the array's
 * identity instead; PHP has no identity to key an array on, and threading the
 * plans through the tree turns out to be both simpler and one lookup cheaper.
 *
 * @internal
 */
final class Encoder
{
    /** @var array<array-key,int> Key name to the number of times a token for it will be written. */
    private array $keyCounts = [];

    /** @var array<array-key,int> String value to the number of times it occurs. */
    private array $stringCounts = [];

    /** @var array<array-key,true> Values a dictionary column needs in the catalogue whatever it costs. */
    private array $forcedStrings = [];

    /** @var list<string> */
    private array $keyList = [];

    /** @var array<array-key,int> */
    private array $keyIndex = [];

    /** @var array<array-key,string> */
    private array $keyTokens = [];

    /** @var list<string> */
    private array $stringList = [];

    /** @var array<array-key,int> */
    private array $stringIndex = [];

    private function __construct(private readonly CompressOptions $options)
    {
    }

    /**
     * Compress a value into a json-compress envelope.
     *
     * The result is an ordinary JSON value, so it can be stored, posted or
     * logged anywhere plain JSON goes.
     *
     * @param CompressOptions|array<string,mixed>|null $options
     */
    public static function compress(mixed $value, CompressOptions|array|null $options = null): stdClass
    {
        $encoder = new self(CompressOptions::resolve($options));

        return $encoder->run(Normalizer::normalize($value));
    }

    private function run(mixed $source): stdClass
    {
        $prepared = $this->prepare($source);

        $this->buildKeyCatalogue();
        $this->buildStringCatalogue();

        $data = $this->build($prepared);

        $envelope = new stdClass();
        $envelope->{Wire::ENV_VERSION} = Wire::FORMAT_VERSION;
        $envelope->{Wire::ENV_KEYS} = $this->keyList;
        if ($this->stringList !== []) {
            $envelope->{Wire::ENV_STRINGS} = $this->stringList;
        }
        $envelope->{Wire::ENV_DATA} = $data;

        return $envelope;
    }

    /* ------------------------------------------------------------------ *
     * Pass one - analysis
     * ------------------------------------------------------------------ */

    private function prepare(mixed $value): mixed
    {
        if (is_string($value)) {
            $this->countString($value);

            return $value;
        }

        if (is_array($value)) {
            $plan = TablePlanner::plan($value, $this->options);
            if ($plan !== null) {
                /** @var list<stdClass> $value */
                return $this->prepareTable($value, $plan);
            }

            $out = [];
            foreach ($value as $item) {
                $out[] = $this->prepare($item);
            }

            return $out;
        }

        if ($value instanceof stdClass) {
            $out = new stdClass();
            foreach ($value as $key => $item) {
                $key = (string) $key;
                $this->keyCounts[$key] = ($this->keyCounts[$key] ?? 0) + 1;
                $out->{$key} = $this->prepare($item);
            }

            return $out;
        }

        return $value;
    }

    /**
     * @param list<stdClass> $rows
     */
    private function prepareTable(array $rows, TablePlan $plan): PlannedTable
    {
        $columns = $plan->columns;
        $encodings = $plan->encodings;

        // A table spells each key out once, in the column list, however many
        // rows it has - so it contributes one to the key frequency, not one per
        // row.
        foreach ($columns as $key) {
            $this->keyCounts[$key] = ($this->keyCounts[$key] ?? 0) + 1;
        }
        foreach ($plan->constants as $constant) {
            if (is_string($constant)) {
                $this->countString($constant);
            }
        }

        $rowCount = count($rows);
        if ($plan->variableCount === 0) {
            // Every column turned out to be constant, so the rows carry no data
            // at all and only their number survives.
            return new PlannedTable($plan, [], $rowCount);
        }

        $columnCount = count($columns);
        $absent = Absent::it();
        $matrix = [];

        foreach ($rows as $row) {
            $cells = [];
            for ($at = 0; $at < $columnCount; $at++) {
                $encoding = $encodings[$at];
                if ($encoding === Wire::COL_CONST) {
                    continue;
                }

                $column = $columns[$at];
                if (!property_exists($row, $column)) {
                    $cells[] = $absent;
                    continue;
                }

                $value = $row->{$column};
                if ($encoding === Wire::COL_DICT) {
                    if (is_string($value)) {
                        $this->countString($value);
                        // A dictionary cell is a bare integer, so its value has
                        // to be in the catalogue whatever the cost model says.
                        $this->forcedStrings[$value] = true;
                    }
                    $cells[] = $value;
                } else {
                    $cells[] = $this->prepare($value);
                }
            }
            $matrix[] = $cells;
        }

        return new PlannedTable($plan, $matrix, $rowCount);
    }

    private function countString(string $value): void
    {
        if (!$this->options->strings) {
            return;
        }
        $this->stringCounts[$value] = ($this->stringCounts[$value] ?? 0) + 1;
    }

    /* ------------------------------------------------------------------ *
     * Catalogues
     * ------------------------------------------------------------------ */

    /** Most-used key first, so the busiest keys get the shortest tokens. */
    private function buildKeyCatalogue(): void
    {
        $entries = [];
        foreach ($this->keyCounts as $key => $count) {
            $entries[] = [(string) $key, $count];
        }

        // The sort is stable and the counts are in first-seen order, so keys
        // used equally often keep the order the document introduced them in.
        usort($entries, static fn (array $a, array $b): int => $b[1] <=> $a[1]);

        foreach ($entries as $at => [$key]) {
            $this->keyList[] = $key;
            $this->keyIndex[$key] = $at;
            $this->keyTokens[$key] = Tokens::toKeyToken($at);
        }
    }

    /**
     * Choose which string values are worth moving into the catalogue.
     *
     * A catalogued string is written once in the catalogue and referenced by a
     * short tilde token everywhere it occurs, so the trade is:
     *
     *     inline    = count * (length + 2)                  // quotes on every copy
     *     catalogue = count * (tokenLength + 3) + length + 3
     *
     * Candidates are considered in frequency order, so the token length used in
     * that comparison is the one the string would really be given.
     *
     * Values pulled into a dictionary column are forced in regardless: their
     * cells are bare integers, which is cheaper still, and the comparison above
     * does not describe them.
     */
    private function buildStringCatalogue(): void
    {
        if (!$this->options->strings) {
            return;
        }

        $minCount = $this->options->minStringCount;
        $minLength = $this->options->minStringLength;

        $candidates = [];
        foreach ($this->stringCounts as $value => $count) {
            $value = (string) $value;
            $forced = isset($this->forcedStrings[$value]);
            $length = Json::utf16Length($value);

            if (!$forced) {
                if ($count < $minCount) {
                    continue;
                }
                if ($length < $minLength) {
                    continue;
                }
                // A value that already begins with a tilde is escaped inline and
                // can never be written as a reference, so cataloguing it would
                // only burn an index.
                if ($value !== '' && $value[0] === Wire::HOLE) {
                    continue;
                }
            }

            $candidates[] = [$value, $count, $forced, $length];
        }

        usort(
            $candidates,
            static fn (array $a, array $b): int => ($b[1] <=> $a[1]) ?: ($b[3] <=> $a[3])
        );

        foreach ($candidates as [$value, $count, $forced, $length]) {
            if (!$forced) {
                $tokenLength = Tokens::base62Length(count($this->stringList));
                $inline = $count * ($length + 2);
                $catalogued = $count * ($tokenLength + 3) + $length + 3;
                if ($catalogued >= $inline) {
                    continue;
                }
            }

            $this->stringIndex[$value] = count($this->stringList);
            $this->stringList[] = $value;
        }
    }

    /* ------------------------------------------------------------------ *
     * Pass two - build
     * ------------------------------------------------------------------ */

    private function build(mixed $node): mixed
    {
        if (is_string($node)) {
            return $this->encodeString($node);
        }

        if (is_array($node)) {
            $out = [];
            foreach ($node as $item) {
                $out[] = $this->build($item);
            }

            return $out;
        }

        if ($node instanceof PlannedTable) {
            return $this->buildTable($node);
        }

        if ($node instanceof stdClass) {
            $out = new stdClass();
            foreach ($node as $key => $item) {
                $out->{$this->keyTokens[(string) $key]} = $this->build($item);
            }

            return $out;
        }

        return $node;
    }

    private function encodeString(string $value): string
    {
        // Escape first: an escaped literal always starts with two tildes, a
        // reference is a tilde followed by a base-62 digit, and a lone tilde is
        // the row hole. Those three spaces cannot overlap.
        if ($value !== '' && $value[0] === Wire::HOLE) {
            return Wire::HOLE . $value;
        }

        $at = $this->stringIndex[$value] ?? null;

        return $at === null ? $value : Wire::HOLE . Tokens::toBase62($at);
    }

    private function buildTable(PlannedTable $table): stdClass
    {
        $plan = $table->plan;
        $encodings = $plan->encodings;

        $node = new stdClass();

        $indices = [];
        foreach ($plan->columns as $key) {
            $indices[] = $this->keyIndex[$key];
        }
        $node->{Wire::T_COLUMNS} = $indices;

        // The encodings are only written down when they are not all raw, which
        // is the common case and the one worth saving the bytes on.
        $cellEncodings = [];
        $uniform = true;
        foreach ($encodings as $encoding) {
            if ($encoding !== Wire::COL_RAW) {
                $uniform = false;
            }
            if ($encoding !== Wire::COL_CONST) {
                $cellEncodings[] = $encoding;
            }
        }
        if (!$uniform) {
            $node->{Wire::T_ENCODINGS} = $encodings;
        }

        if ($plan->constants !== []) {
            $constants = [];
            foreach ($plan->constants as $value) {
                $constants[] = $this->build($value);
            }
            $node->{Wire::T_CONSTS} = $constants;
        }

        if ($plan->variableCount === 0) {
            $node->{Wire::T_ROWCOUNT} = $table->rowCount;

            return $node;
        }

        $absent = Absent::it();
        $matrix = [];

        foreach ($table->rows as $cells) {
            // Trailing gaps need not be written down; a short row is understood
            // to be missing everything past its end. Whether a cell is a gap is
            // read off the marker, never guessed from what the cell holds: a raw
            // column may legitimately contain the number -1, which is exactly
            // what a dictionary column writes for an absent key.
            $end = count($cells);
            while ($end > 0 && $cells[$end - 1] === $absent) {
                $end--;
            }

            $row = [];
            for ($at = 0; $at < $end; $at++) {
                $cell = $cells[$at];
                if ($cellEncodings[$at] === Wire::COL_DICT) {
                    if ($cell === $absent) {
                        $row[] = Wire::DICT_HOLE;
                    } elseif ($cell === null) {
                        $row[] = null;
                    } else {
                        $row[] = $this->stringIndex[$cell];
                    }
                } else {
                    $row[] = $cell === $absent ? Wire::HOLE : $this->build($cell);
                }
            }

            $matrix[] = $row;
        }

        $node->{Wire::T_ROWS} = $matrix;

        return $node;
    }
}
