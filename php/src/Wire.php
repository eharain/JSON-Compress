<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

/**
 * Wire-format constants for the json-compress envelope.
 *
 * Everything the encoder emits is still ordinary JSON. The format therefore has
 * to be self-describing without any out-of-band type tags, which it achieves by
 * reserving a single character - the tilde - inside object keys and strings.
 *
 * Because every original object key is replaced by a catalogue token, and key
 * tokens are drawn from letters followed by base-62 digits, no original key can
 * ever appear as an object key in the encoded document. That makes every key
 * that starts with a tilde safe to reserve for structural nodes.
 *
 * String values are escaped on the way in (a value that already starts with a
 * tilde gains one more), which frees the tilde-prefixed string space for
 * catalogue references and the row hole marker.
 *
 * @internal
 */
final class Wire
{
    /** Envelope tag and format version. Bumped only on a breaking format change. */
    public const FORMAT_VERSION = 1;

    /** Envelope field: format version. */
    public const ENV_VERSION = 'jc';

    /** Envelope field: key catalogue, a list of the original key names. */
    public const ENV_KEYS = 'k';

    /** Envelope field: string-value catalogue. Omitted when empty. */
    public const ENV_STRINGS = 's';

    /** Envelope field: the encoded document. */
    public const ENV_DATA = 'd';

    /** Table node: column list, as indices into the key catalogue. */
    public const T_COLUMNS = '~t';

    /** Table node: per-column encoding. Omitted when every column is raw. */
    public const T_ENCODINGS = '~e';

    /** Table node: values of the constant columns, in column order. */
    public const T_CONSTS = '~cv';

    /** Table node: the rows, each a list of cells for the non-constant columns. */
    public const T_ROWS = '~r';

    /** Table node: row count. Only present when every column is constant. */
    public const T_ROWCOUNT = '~n';

    /** Cell marker for "this row does not have this key at all". */
    public const HOLE = '~';

    /** Dictionary-column equivalent of {@see Wire::HOLE}. */
    public const DICT_HOLE = -1;

    /** Column encoding: cells are ordinary encoded values. */
    public const COL_RAW = 0;

    /** Column encoding: cells are integer indices into the string catalogue. */
    public const COL_DICT = 1;

    /** Column encoding: the column is constant; its value lives in the constants list. */
    public const COL_CONST = 2;

    private function __construct()
    {
    }
}
