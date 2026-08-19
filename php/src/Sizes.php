<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use RuntimeException;

/**
 * Byte measurement and gzip.
 *
 * What actually goes over the wire is UTF-8 bytes, which is exactly what a PHP
 * string already holds, so measuring one is `strlen` and nothing more. The gzip
 * helpers are here because the interesting question about any compression
 * scheme is what it is still worth once gzip has had its turn, and answering it
 * should not need a second import at the call site.
 */
final class Sizes
{
    /**
     * Deflate level 6 is zlib's own default and what browsers use for
     * `CompressionStream`, so a figure measured here is comparable with one
     * measured in the JavaScript implementation.
     */
    public const DEFAULT_LEVEL = 6;

    private function __construct()
    {
    }

    /** Length of a string in UTF-8 bytes. */
    public static function byteLength(string $text): int
    {
        return strlen($text);
    }

    /** True when this build of PHP has zlib, which is what the gzip helpers need. */
    public static function canGzip(): bool
    {
        return function_exists('gzencode');
    }

    /**
     * Gzip a string.
     *
     * The header carries no timestamp, so the same input always gives the same
     * bytes and a size can be compared across runs.
     */
    public static function gzip(string $text, int $level = self::DEFAULT_LEVEL): string
    {
        if (!self::canGzip()) {
            throw new RuntimeException('json-compress: this build of PHP has no zlib extension');
        }

        $out = gzencode($text, $level);
        if ($out === false) {
            throw new RuntimeException('json-compress: gzip failed');
        }

        return $out;
    }

    /** Reverse {@see Sizes::gzip()}. */
    public static function gunzip(string $data): string
    {
        if (!function_exists('gzdecode')) {
            throw new RuntimeException('json-compress: this build of PHP has no zlib extension');
        }

        $out = gzdecode($data);
        if ($out === false) {
            throw new RuntimeException('json-compress: the data is not gzip');
        }

        return $out;
    }

    /**
     * How many bytes a string takes once gzipped.
     *
     * Returns null on a build with no zlib, so a caller can present the figure
     * as unavailable rather than having to guard the call.
     */
    public static function gzipSize(string $text, int $level = self::DEFAULT_LEVEL): ?int
    {
        return self::canGzip() ? strlen(self::gzip($text, $level)) : null;
    }
}
