//! Wire-format constants for the json-compress envelope.
//!
//! Everything the encoder emits is still ordinary JSON. The format therefore has
//! to be self-describing without any out-of-band type tags, which it achieves by
//! reserving a single character - the tilde - inside object keys and strings.
//!
//! Because every original object key is replaced by a catalogue token, and key
//! tokens are drawn from `[A-Za-z][0-9A-Za-z]*`, no original key can ever appear
//! as an object key in the encoded document. That makes every key that starts
//! with a tilde safe to reserve for structural nodes.
//!
//! String values are escaped on the way in (a value that already starts with a
//! tilde gains one more), which frees the tilde-prefixed string space for
//! catalogue references and the row hole marker.

/// The version of the wire format this build reads and writes. It is bumped only
/// on a breaking format change, and is not the version of this crate.
pub const FORMAT_VERSION: i64 = 1;

/// Envelope member: the format version.
pub(crate) const ENV_VERSION: &str = "jc";
/// Envelope member: the key catalogue, an array of the original key names.
pub(crate) const ENV_KEYS: &str = "k";
/// Envelope member: the string-value catalogue; omitted when empty.
pub(crate) const ENV_STRINGS: &str = "s";
/// Envelope member: the encoded document.
pub(crate) const ENV_DATA: &str = "d";

/// Table member: the column list, as indices into the key catalogue.
pub(crate) const TABLE_COLUMNS: &str = "~t";
/// Table member: per-column encoding; omitted when every column is raw.
pub(crate) const TABLE_ENCODINGS: &str = "~e";
/// Table member: values of the constant columns, in column order.
pub(crate) const TABLE_CONSTANTS: &str = "~cv";
/// Table member: the rows, one array of cells per row.
pub(crate) const TABLE_ROWS: &str = "~r";
/// Table member: the row count, only present when every column is constant.
pub(crate) const TABLE_ROW_COUNT: &str = "~n";

/// Marks a cell whose row does not have this key at all.
pub(crate) const HOLE: &str = "~";
/// The dictionary-column equivalent of [`HOLE`].
pub(crate) const DICT_HOLE: i64 = -1;
/// The one reserved character in the format.
pub(crate) const TILDE: char = '~';
