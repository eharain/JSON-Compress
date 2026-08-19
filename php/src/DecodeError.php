<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use RuntimeException;
use Throwable;

/**
 * Raised when a document is not a readable json-compress envelope.
 *
 * Every message names what was wrong with the document rather than where the
 * reader gave up, because a document that fails to decode is nearly always one
 * that was written by something other than a conforming encoder.
 */
final class DecodeError extends RuntimeException
{
    public function __construct(string $message, ?Throwable $previous = null)
    {
        parent::__construct('json-compress: ' . $message, 0, $previous);
    }
}
