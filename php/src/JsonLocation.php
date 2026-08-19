<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * Where a JSON document stops being valid, and why.
 *
 * Offsets count characters rather than bytes, so a column number means what it
 * means in an editor even when the line above it holds an emoji.
 */
final class JsonLocation
{
    /**
     * @param bool        $valid    Whether the document parses.
     * @param string|null $error    What is wrong, in plain words.
     * @param int|null    $position Character offset of the problem.
     * @param int|null    $line     Line number, counted from one.
     * @param int|null    $column   Column number, counted from one.
     * @param string|null $excerpt  The line the problem is on.
     */
    public function __construct(
        public readonly bool $valid,
        public readonly ?string $error = null,
        public readonly ?int $position = null,
        public readonly ?int $line = null,
        public readonly ?int $column = null,
        public readonly ?string $excerpt = null,
    ) {
    }

    /** A one-line summary, for a log or an error message. */
    public function __toString(): string
    {
        if ($this->valid) {
            return 'valid JSON';
        }

        $where = $this->line === null ? '' : ' at line ' . $this->line . ', column ' . $this->column;

        return 'invalid JSON' . $where . ': ' . (string) $this->error;
    }
}
