<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * What compression bought on one value.
 *
 * The honest comparison is against minified JSON, not against whatever
 * whitespace the input happened to arrive with, so `ratio` and `percent` both
 * use the minified size as their baseline.
 */
final class SizeReport
{
    /**
     * @param int      $original       Bytes of the input as given.
     * @param int      $minified       Bytes once whitespace is removed.
     * @param int      $compressed     Bytes of the compressed envelope.
     * @param int      $saved          Bytes saved against the minified form.
     * @param float    $ratio          Compressed divided by minified, 0 to 1.
     * @param float    $percent        Percentage saved against the minified form.
     * @param int|null $gzipMinified   Minified bytes after gzip, or null when not measured.
     * @param int|null $gzipCompressed Compressed bytes after gzip, or null when not measured.
     */
    public function __construct(
        public readonly int $original,
        public readonly int $minified,
        public readonly int $compressed,
        public readonly int $saved,
        public readonly float $ratio,
        public readonly float $percent,
        public readonly ?int $gzipMinified = null,
        public readonly ?int $gzipCompressed = null,
    ) {
    }

    /** @return array<string,int|float|null> */
    public function toArray(): array
    {
        return [
            'original' => $this->original,
            'minified' => $this->minified,
            'compressed' => $this->compressed,
            'saved' => $this->saved,
            'ratio' => $this->ratio,
            'percent' => $this->percent,
            'gzipMinified' => $this->gzipMinified,
            'gzipCompressed' => $this->gzipCompressed,
        ];
    }
}
