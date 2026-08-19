<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use RuntimeException;

/**
 * Somewhere for the scanner to unwind to, carrying the offset the problem was
 * found at.
 *
 * Never escapes {@see Locator}: it is caught there and turned into a
 * {@see JsonLocation}.
 *
 * @internal
 */
final class ScanError extends RuntimeException
{
    public function __construct(string $message, public readonly int $at)
    {
        parent::__construct($message);
    }
}
