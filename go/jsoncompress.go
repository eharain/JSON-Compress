package jsoncompress

// The public surface around the codec: the string-level pair, the helpers for a
// boundary that carries both compressed and plain documents, and the numbers
// that say whether any of it is worth doing on a given payload.

// Marshal compresses a value and serialises it in one step. It is [Compress]
// followed by [FormatJSON].
func Marshal(value any, options ...Options) ([]byte, error) {
	envelope, err := Compress(value, options...)
	if err != nil {
		return nil, err
	}
	return formatModel(envelope, "")
}

// MarshalIndent is [Marshal] with the envelope indented by the given number of
// spaces.
func MarshalIndent(value any, indent int, options ...Options) ([]byte, error) {
	envelope, err := Compress(value, options...)
	if err != nil {
		return nil, err
	}
	return formatModel(envelope, indentPad(indent))
}

// Unmarshal parses a compressed document and restores the original value. It is
// [ParseJSON] followed by [Decompress], and because it does the parsing itself
// the restored document keeps the key order it was written with.
func Unmarshal(data []byte) (any, error) {
	envelope, err := ParseJSON(data)
	if err != nil {
		return nil, err
	}
	return Decompress(envelope)
}

// DecompressIfNeeded restores a value whether or not it was compressed.
//
// Handy at a boundary you do not fully control - an endpoint being migrated, a
// cache holding entries written by two versions of the same service.
func DecompressIfNeeded(value any) (any, error) {
	if IsCompressed(value) {
		return Decompress(value)
	}
	return value, nil
}

// Minify strips the whitespace from a JSON document without changing what it
// means.
func Minify(text string) (string, error) {
	value, err := ParseJSONString(text)
	if err != nil {
		return "", err
	}
	out, err := FormatJSON(value)
	if err != nil {
		return "", err
	}
	return string(out), nil
}

// Beautify re-indents a JSON document. Line feeds are written whatever the
// platform, so a document beautified on Windows matches the same document
// beautified anywhere else.
func Beautify(text string, indent int) (string, error) {
	value, err := ParseJSONString(text)
	if err != nil {
		return "", err
	}
	out, err := FormatJSONIndent(value, indent)
	if err != nil {
		return "", err
	}
	return string(out), nil
}

// Validate checks that a string is valid JSON, and says exactly where it stops
// being so.
//
// The verdict comes from the parser, so it is the same verdict [ParseJSON] would
// give. The location comes from a scan of the document, because parser messages
// are written for parser authors and often name the wrong culprit.
func Validate(text string) Location {
	_, err := ParseJSONString(text)
	if err == nil {
		return Location{Valid: true}
	}

	found := Locate(text)
	if !found.Valid {
		return found
	}

	// The scan found nothing but the parse still failed - report what the parser
	// said rather than claiming the document is fine.
	return Location{Valid: false, Error: err.Error()}
}

// Report is what compression bought on one value.
type Report struct {
	// Original is the bytes of the input as given.
	Original int
	// Minified is the bytes once whitespace is removed.
	Minified int
	// Compressed is the bytes of the compressed envelope.
	Compressed int
	// Saved is the bytes saved against the minified form.
	Saved int
	// Ratio is Compressed divided by Minified, 0 to 1.
	Ratio float64
	// Percent is the percentage saved against the minified form.
	Percent float64
	// GzipMinified is the minified form after gzip.
	GzipMinified int
	// GzipCompressed is the compressed form after gzip.
	GzipCompressed int
}

// Measure reports what compression buys on a given value.
//
// The honest comparison is against minified JSON, not against whatever
// whitespace the input happened to arrive with, so Ratio and Percent both use
// the minified size as their baseline. Gzip figures are included because that is
// what most payloads meet in production, and the interesting question is whether
// the format still helps once gzip has had its turn.
func Measure(value any, options ...Options) (Report, error) {
	minified, err := FormatJSON(value)
	if err != nil {
		return Report{}, err
	}
	return measure(value, minified, len(minified), options)
}

// MeasureText is [Measure] on a document that arrived as text, so that Original
// reflects what really came in, whitespace and all.
func MeasureText(text string, options ...Options) (Report, error) {
	value, err := ParseJSONString(text)
	if err != nil {
		return Report{}, err
	}
	minified, err := FormatJSON(value)
	if err != nil {
		return Report{}, err
	}
	return measure(value, minified, len(text), options)
}

func measure(value any, minified []byte, original int, options []Options) (Report, error) {
	compressed, err := Marshal(value, options...)
	if err != nil {
		return Report{}, err
	}

	report := Report{
		Original:       original,
		Minified:       len(minified),
		Compressed:     len(compressed),
		Saved:          len(minified) - len(compressed),
		Ratio:          1,
		GzipMinified:   GzipSize(minified),
		GzipCompressed: GzipSize(compressed),
	}
	if len(minified) > 0 {
		report.Ratio = float64(len(compressed)) / float64(len(minified))
		report.Percent = float64(report.Saved) / float64(len(minified)) * 100
	}
	return report, nil
}

// Codec is a set of options, bound once and reused.
//
// A codec is the natural unit when the same options apply everywhere - a
// transport layer, a cache adapter, a storage helper - and it keeps the option
// struct off every call site. It holds nothing but its options, which it does
// not change, so one instance built at start-up is safe to share across
// goroutines.
type Codec struct {
	options Options
}

// NewCodec binds a set of options, validating them once rather than on every
// call.
func NewCodec(options ...Options) (*Codec, error) {
	resolved, err := resolveOptions(options)
	if err != nil {
		return nil, err
	}
	return &Codec{options: resolved}, nil
}

// Options returns the options this codec was built with.
func (c *Codec) Options() Options {
	return c.options
}

// Compress packs a value with this codec's options.
func (c *Codec) Compress(value any) (*Object, error) {
	return Compress(value, c.options)
}

// Decompress restores a compressed value.
func (c *Codec) Decompress(envelope any) (any, error) {
	return Decompress(envelope)
}

// Marshal compresses a value with this codec's options and serialises it.
func (c *Codec) Marshal(value any) ([]byte, error) {
	return Marshal(value, c.options)
}

// Unmarshal parses and restores a compressed document.
func (c *Codec) Unmarshal(data []byte) (any, error) {
	return Unmarshal(data)
}

// Measure reports what this codec's options buy on a value.
func (c *Codec) Measure(value any) (Report, error) {
	return Measure(value, c.options)
}
