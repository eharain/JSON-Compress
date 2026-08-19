"""Wire-format constants for the json-compress envelope.

Everything the encoder emits is still ordinary JSON. The format therefore has
to be self-describing without any out-of-band type tags, which it achieves by
reserving a single character - the tilde - inside object keys and strings.

Because every original object key is replaced by a catalogue token, and key
tokens are drawn from ``[A-Za-z][0-9A-Za-z]*``, no original key can ever appear
as an object key in the encoded document. That makes every key that starts with
``~`` safe to reserve for structural nodes.

String values are escaped on the way in (a value that already starts with a
tilde gains one more), which frees the ``~``-prefixed string space for catalogue
references and the row hole marker.
"""

#: Envelope tag and format version. Bumped only on a breaking format change.
FORMAT_VERSION = 1

#: Envelope field: format version.
ENV_VERSION = "jc"
#: Envelope field: key catalogue, a list of the original key names.
ENV_KEYS = "k"
#: Envelope field: string-value catalogue. Omitted when empty.
ENV_STRINGS = "s"
#: Envelope field: the encoded document.
ENV_DATA = "d"

#: Table node: column list, as indices into the key catalogue.
T_COLUMNS = "~t"
#: Table node: per-column encoding. Omitted when every column is raw.
T_ENCODINGS = "~e"
#: Table node: values of the constant columns, in column order.
T_CONSTS = "~cv"
#: Table node: the rows, each a list of cells for the non-constant columns.
T_ROWS = "~r"
#: Table node: row count. Only present when every column is constant.
T_ROWCOUNT = "~n"

#: Cell marker for "this row does not have this key at all".
HOLE = "~"
#: Dictionary-column equivalent of :data:`HOLE`.
DICT_HOLE = -1

#: Column encoding: cells are ordinary encoded values.
COL_RAW = 0
#: Column encoding: cells are integer indices into the string catalogue.
COL_DICT = 1
#: Column encoding: the column is constant; its value lives in ``~cv``.
COL_CONST = 2
