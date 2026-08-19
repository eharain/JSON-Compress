<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use JsonException;
use stdClass;

/**
 * A set of options, bound once and reused.
 *
 * A codec is the natural unit when the same options apply everywhere - a
 * transport layer, a cache adapter, a storage helper - and it keeps the option
 * object off every call site. It holds nothing but its options, which are
 * immutable, so one instance can be built at boot and shared by every request.
 */
final class Codec
{
    public readonly CompressOptions $options;

    /**
     * @param CompressOptions|array<string,mixed>|null $options
     */
    public function __construct(CompressOptions|array|null $options = null)
    {
        $this->options = CompressOptions::resolve($options);
    }

    /** Compress a value with this codec's options. */
    public function compress(mixed $value): stdClass
    {
        return Encoder::compress($value, $this->options);
    }

    /**
     * Restore a compressed value.
     *
     * @throws DecodeError if the envelope is not a readable json-compress document.
     */
    public function decompress(mixed $envelope): mixed
    {
        return Decoder::decompress($envelope);
    }

    /** Restore a value whether or not it was compressed. */
    public function decompressIfNeeded(mixed $value): mixed
    {
        return JsonCompress::decompressIfNeeded($value);
    }

    /** Compress a value and serialise it. */
    public function stringify(mixed $value, int|string|null $indent = null): string
    {
        return Json::encode($this->compress($value), $indent);
    }

    /**
     * Parse and restore a compressed document.
     *
     * @throws JsonException if the text is not valid JSON.
     * @throws DecodeError   if it is valid JSON but not a compressed document.
     */
    public function parse(string $text): mixed
    {
        return $this->decompress(Json::decode($text));
    }

    /** Measure what this codec's options buy on a value. */
    public function measure(mixed $value, ?string $source = null, bool $gzip = true): SizeReport
    {
        return JsonCompress::measure($value, $this->options, $source, $gzip);
    }
}
