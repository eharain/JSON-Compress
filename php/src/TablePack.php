<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use stdClass;

/**
 * The plain table pack.
 *
 * This is the smallest useful idea in the library, offered on its own because
 * it is often all you need and because the shape it produces - `fields`, `data`
 * and `count` - is trivial to consume from another language or straight out of
 * a database cursor.
 *
 * {@see TablePack::pack()} turns a list of records into one field list and a
 * matrix of rows. {@see TablePack::unpack()} turns it back. Neither uses a
 * catalogue, an envelope or a version tag; what you get is a readable object
 * you can eyeball in a network panel.
 *
 * Records may arrive as associative arrays - which is what PDO hands you - or
 * as objects; either way the packed form is the same JSON.
 */
final class TablePack
{
    private function __construct()
    {
    }

    /**
     * Pack a list of records into a field list and a rows matrix.
     *
     * Keys are taken from the union of every record, in first-seen order, so a
     * record missing a field simply contributes null in that column. Anything
     * that is not a list of records is handed straight back, which lets you drop
     * this into a response pipeline without first checking the shape.
     */
    public static function pack(mixed $records): mixed
    {
        if (!is_array($records) || !array_is_list($records)) {
            return $records;
        }
        if ($records === []) {
            return self::table([], [], 0);
        }

        $rows = [];
        foreach ($records as $record) {
            $members = self::members($record);
            if ($members === null) {
                return $records;
            }
            $rows[] = $members;
        }

        /** @var array<array-key,true> $seen */
        $seen = [];
        /** @var list<string> $fields */
        $fields = [];
        foreach ($rows as $members) {
            foreach ($members as $key => $ignored) {
                if (!isset($seen[$key])) {
                    $seen[$key] = true;
                    $fields[] = (string) $key;
                }
            }
        }

        $data = [];
        foreach ($rows as $members) {
            $cells = [];
            foreach ($fields as $field) {
                $cells[] = array_key_exists($field, $members) ? $members[$field] : null;
            }
            $data[] = $cells;
        }

        return self::table($fields, $data, count($data));
    }

    /**
     * Rebuild the list of records produced by {@see TablePack::pack()}.
     *
     * Anything that is not a packed table is handed straight back, so a consumer
     * can call this unconditionally on a payload that may or may not have been
     * packed at the other end.
     *
     * @param bool $assoc Return each record as an associative array rather than an object.
     */
    public static function unpack(mixed $packed, bool $assoc = false): mixed
    {
        $members = self::members($packed);
        if ($members === null) {
            return $packed;
        }

        $fields = $members['fields'] ?? null;
        $data = $members['data'] ?? null;
        if (!is_array($fields) || !is_array($data)) {
            return $packed;
        }

        $out = [];
        foreach ($data as $row) {
            $record = [];
            foreach (array_values($fields) as $at => $field) {
                // A row shorter than the field list is missing everything past
                // its end, rather than holding nulls there.
                if (!is_array($row) || !array_key_exists($at, $row)) {
                    continue;
                }
                $record[(string) $field] = $row[$at];
            }
            $out[] = $assoc ? $record : (object) $record;
        }

        return $out;
    }

    /** True when a value has the shape {@see TablePack::unpack()} knows how to read. */
    public static function isPacked(mixed $value): bool
    {
        $members = self::members($value);

        return $members !== null
            && isset($members['fields'], $members['data'])
            && is_array($members['fields'])
            && is_array($members['data']);
    }

    /**
     * Pack every result set in a list of result sets.
     *
     * A driver that returns several result sets from one call hands back a list
     * of lists; this packs each inner list and leaves the outer one alone.
     */
    public static function packAll(mixed $resultSets): mixed
    {
        if (!is_array($resultSets) || !array_is_list($resultSets) || $resultSets === []) {
            return $resultSets;
        }

        return array_map(static fn (mixed $set): mixed => self::pack($set), $resultSets);
    }

    /** Unpack every result set packed by {@see TablePack::packAll()}. */
    public static function unpackAll(mixed $resultSets, bool $assoc = false): mixed
    {
        if (!is_array($resultSets) || !array_is_list($resultSets)) {
            return $resultSets;
        }

        return array_map(static fn (mixed $set): mixed => self::unpack($set, $assoc), $resultSets);
    }

    /**
     * Read a delimited-integer column back into a list of numbers.
     *
     * Some databases return a set-valued column as a bracketed, comma-separated
     * string. This converts one such field, leaving an empty list where the
     * column was null. `$prefix` and `$suffix` are how many characters to trim
     * from each end.
     *
     * Returns the record with the field replaced.
     */
    public static function textToArray(
        mixed $record,
        string $field,
        int $prefix = 3,
        int $suffix = 3,
        string $separator = ',',
    ): mixed {
        $members = self::members($record);
        if ($members === null || !array_key_exists($field, $members)) {
            return $record;
        }

        $value = $members[$field];
        if (!is_string($value)) {
            $numbers = [];
        } else {
            $inner = substr($value, $prefix, max(0, strlen($value) - $prefix - $suffix));
            $numbers = $inner === ''
                ? []
                : array_map(static fn (string $part): int => (int) $part, explode($separator, $inner));
        }

        if (is_array($record)) {
            $record[$field] = $numbers;

            return $record;
        }

        /** @var object $record */
        $record->{$field} = $numbers;

        return $record;
    }

    /**
     * The members of a record, whichever of the two shapes it arrived in, or
     * null when it is not a record at all.
     *
     * @return array<array-key,mixed>|null
     */
    private static function members(mixed $record): ?array
    {
        if ($record instanceof stdClass) {
            return get_object_vars($record);
        }
        if (is_array($record) && !array_is_list($record)) {
            return $record;
        }
        // An empty array is ambiguous - PHP cannot tell `[]` from `{}` - and is
        // read as the empty record, which is what a driver returning no columns
        // would hand over.
        if ($record === []) {
            return [];
        }

        return null;
    }

    /**
     * @param list<string>      $fields
     * @param list<list<mixed>> $data
     */
    private static function table(array $fields, array $data, int $count): stdClass
    {
        $table = new stdClass();
        $table->fields = $fields;
        $table->data = $data;
        $table->count = $count;

        return $table;
    }
}
