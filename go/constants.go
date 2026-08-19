package jsoncompress

// Wire-format constants for the json-compress envelope.
//
// Everything the encoder emits is still ordinary JSON. The format therefore has
// to be self-describing without any out-of-band type tags, which it achieves by
// reserving a single character - the tilde - inside object keys and strings.
//
// Because every original object key is replaced by a catalogue token, and key
// tokens are drawn from [A-Za-z][0-9A-Za-z]*, no original key can ever appear as
// an object key in the encoded document. That makes every key that starts with
// a tilde safe to reserve for structural nodes.
//
// String values are escaped on the way in (a value that already starts with a
// tilde gains one more), which frees the tilde-prefixed string space for
// catalogue references and the row hole marker.

// FormatVersion is the version of the wire format this build reads and writes.
// It is bumped only on a breaking format change.
const FormatVersion = 1

// Envelope members.
const (
	envVersion = "jc" // format version
	envKeys    = "k"  // the key catalogue, an array of the original key names
	envStrings = "s"  // the string-value catalogue; omitted when empty
	envData    = "d"  // the encoded document
)

// Table-node members.
const (
	tableColumns   = "~t"  // column list, as indices into the key catalogue
	tableEncodings = "~e"  // per-column encoding; omitted when every column is raw
	tableConstants = "~cv" // values of the constant columns, in column order
	tableRows      = "~r"  // the rows, one array of cells per row
	tableRowCount  = "~n"  // row count, only present when every column is constant
)

const (
	// hole marks a cell whose row does not have this key at all.
	hole = "~"
	// dictHole is the dictionary-column equivalent of hole.
	dictHole = -1
	// tilde is the one reserved character in the format.
	tilde = '~'
)
