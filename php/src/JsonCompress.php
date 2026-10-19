<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use JsonException;
use stdClass;
use Throwable;

/**
 * json-compress: lossless JSON compression that is still JSON.
 *
 * Three ideas, applied together:
 *
 * 1. Every object key is replaced by a short token and the real names are
 *    listed once, in a catalogue. A key that occurs ten thousand times is
 *    spelled out once.
 * 2. A list of like-shaped objects becomes a table - one column list and a
 *    matrix of rows - so the keys disappear from the rows entirely. Columns
 *    whose value never changes are hoisted out of the rows altogether, and
 *    columns drawn from a small set of strings become integer indices.
 * 3. String values that repeat are moved into a second catalogue and referenced
 *    by token.
 *
 * The output is an ordinary JSON value with no binary framing, so it survives
 * anything that carries JSON: an HTTP body, a WebSocket frame, a `json` column,
 * a queue message, a log line.
 *
 *     use TechStyle\JsonCompress\JsonCompress;
 *
 *     $envelope = JsonCompress::compress([
 *         ['id' => 1, 'role' => 'admin'],
 *         ['id' => 2, 'role' => 'admin'],
 *     ]);
 *     $back = JsonCompress::decompress($envelope);
 *
 * **The value model.** Objects are {@see stdClass} and arrays are PHP lists,
 * which is what `json_decode` produces when you do not ask for associative
 * arrays. That is not a stylistic choice: a PHP array cannot tell an object
 * from a list, and it turns a key of "7" into the integer 7, so decoding into
 * arrays would lose both the empty-object/empty-array distinction and the
 * original key order - and this format promises to preserve both. Input is
 * more forgiving: an associative array is read as an object and a list as an
 * array, exactly as `json_encode` reads them.
 *
 * The wire format is specified in `SPEC.md` at the root of the repository, and
 * this implementation writes the same bytes as the JavaScript, .NET and Python
 * ones.
 */
final class JsonCompress
{
    /** The format version this build reads and writes. */
    public const FORMAT_VERSION = Wire::FORMAT_VERSION;

    /** The version of this package. */
    public const VERSION = '1.0.4';

    private function __construct()
    {
    }

    /**
     * Compress a value into a json-compress envelope.
     *
     * @param CompressOptions|array<string,mixed>|null $options
     */
    public static function compress(mixed $value, CompressOptions|array|null $options = null): stdClass
    {
        return Encoder::compress($value, $options);
    }

    /**
     * Restore the original value from a json-compress envelope.
     *
     * @throws DecodeError if the envelope is not a readable json-compress document.
     */
    public static function decompress(mixed $envelope): mixed
    {
        return Decoder::decompress($envelope);
    }

    /** True when a value looks like a json-compress envelope this build can read. */
    public static function isCompressed(mixed $value): bool
    {
        return Decoder::isCompressed($value);
    }

    /**
     * Restore a value whether or not it was compressed.
     *
     * Handy at a boundary you do not fully control - an endpoint being migrated,
     * a cache holding entries written by two versions of the same service.
     */
    public static function decompressIfNeeded(mixed $value): mixed
    {
        return self::isCompressed($value) ? self::decompress($value) : $value;
    }

    /**
     * Compress a value and serialise it in one step.
     *
     * @param CompressOptions|array<string,mixed>|null $options
     * @param int|string|null                          $indent  Spaces, a literal string, or null for none.
     */
    public static function stringify(
        mixed $value,
        CompressOptions|array|null $options = null,
        int|string|null $indent = null,
    ): string {
        return Json::encode(self::compress($value, $options), $indent);
    }

    /**
     * Parse a compressed document and restore the original value.
     *
     * @throws JsonException if the text is not valid JSON.
     * @throws DecodeError   if it is valid JSON but not a compressed document.
     */
    public static function parse(string $text): mixed
    {
        return self::decompress(Json::decode($text));
    }

    /**
     * Parse ordinary JSON text into the value model this library speaks.
     *
     * @throws JsonException if the text is not valid JSON.
     */
    public static function parseJson(string $text): mixed
    {
        return Json::decode($text);
    }

    /**
     * Serialise a value as ordinary JSON, byte for byte as `JSON.stringify` would.
     *
     * @param int|string|null $indent Spaces, a literal string, or null for none.
     */
    public static function toJson(mixed $value, int|string|null $indent = null): string
    {
        return Json::encode(Normalizer::normalize($value), $indent);
    }

    /**
     * Strip the whitespace from a JSON document without changing what it means.
     *
     * @throws JsonException if the text is not valid JSON.
     */
    public static function minify(string $text): string
    {
        return Json::encode(Json::decode($text));
    }

    /**
     * Re-indent a JSON document.
     *
     * @throws JsonException if the text is not valid JSON.
     */
    public static function beautify(string $text, int|string $indent = 2): string
    {
        return Json::encode(Json::decode($text), $indent);
    }

    /**
     * Check that a string is valid JSON, and say exactly where it stops being so.
     *
     * The verdict comes from `json_decode`, so it is the same verdict the rest
     * of PHP would give. The location comes from a scan of the document, because
     * "Syntax error" is all `json_decode` has to say about where the trouble is.
     */
    public static function validate(string $text): JsonLocation
    {
        try {
            Json::decode($text);

            return new JsonLocation(true);
        } catch (Throwable $error) {
            $found = self::locate($text);
            if (!$found->valid) {
                return $found;
            }

            // The scan found nothing but the parse still failed - report what
            // the parser said rather than claiming the document is fine.
            return new JsonLocation(false, $error->getMessage());
        }
    }

    /** Report the exact line, column and cause of a malformed JSON document. */
    public static function locate(string $text): JsonLocation
    {
        return Locator::locate($text);
    }

    /**
     * Measure what compression buys on a given value.
     *
     * Gzip figures are included because that is what most payloads meet in
     * production, and the interesting question is whether the format still helps
     * once gzip has had its turn.
     *
     * @param CompressOptions|array<string,mixed>|null $options
     * @param string|null                              $source  The text the document really arrived as.
     * @param bool                                     $gzip    Whether to measure the gzipped sizes too.
     */
    public static function measure(
        mixed $value,
        CompressOptions|array|null $options = null,
        ?string $source = null,
        bool $gzip = true,
    ): SizeReport {
        $minified = Json::encode(Normalizer::normalize($value));
        $compressed = Json::encode(self::compress($value, $options));

        $minifiedBytes = Sizes::byteLength($minified);
        $compressedBytes = Sizes::byteLength($compressed);

        return new SizeReport(
            original: $source === null ? $minifiedBytes : Sizes::byteLength($source),
            minified: $minifiedBytes,
            compressed: $compressedBytes,
            saved: $minifiedBytes - $compressedBytes,
            ratio: $minifiedBytes === 0 ? 1.0 : $compressedBytes / $minifiedBytes,
            percent: $minifiedBytes === 0
                ? 0.0
                : ($minifiedBytes - $compressedBytes) / $minifiedBytes * 100,
            gzipMinified: $gzip ? Sizes::gzipSize($minified) : null,
            gzipCompressed: $gzip ? Sizes::gzipSize($compressed) : null,
        );
    }

    /**
     * Bind a set of options once and reuse them.
     *
     * @param CompressOptions|array<string,mixed>|null $options
     */
    public static function codec(CompressOptions|array|null $options = null): Codec
    {
        return new Codec($options);
    }
}
