package jsoncompress

import "fmt"

// Options is what the encoder is allowed to do.
//
// The zero value is not a usable set of options - every switch would be off and
// MinRows would be nonsense - so build one from [DefaultOptions] and change what
// you need:
//
//	options := jsoncompress.DefaultOptions()
//	options.MinRows = 10
//	envelope, err := jsoncompress.Compress(value, options)
//
// Every default is safe on arbitrary input, and the defaults are tuned for API
// responses and log batches, which is where the format earns most of its keep.
type Options struct {
	// Strings de-duplicates repeated string values into a catalogue.
	Strings bool
	// Tables packs slices of like-shaped objects into columnar tables.
	Tables bool
	// Constants hoists columns whose value never changes out of the rows.
	Constants bool
	// DictColumns encodes low-cardinality string columns as integer indices.
	DictColumns bool
	// MinRows is the smallest slice that may become a table.
	MinRows int
	// MaxHoleRatio is the largest share of missing cells a table may carry.
	MaxHoleRatio float64
	// MinStringCount is how many times a string must occur before it can be
	// catalogued.
	MinStringCount int
	// MinStringLength is the shortest string that may be catalogued.
	MinStringLength int
}

// DefaultOptions returns the options used when none are given.
func DefaultOptions() Options {
	return Options{
		Strings:         true,
		Tables:          true,
		Constants:       true,
		DictColumns:     true,
		MinRows:         2,
		MaxHoleRatio:    0.3,
		MinStringCount:  2,
		MinStringLength: 1,
	}
}

// Validate checks that the options make sense, so that a typo surfaces at the
// call site rather than as a mysteriously large output.
func (o Options) Validate() error {
	if o.MinRows < 1 {
		return fmt.Errorf("json-compress: MinRows must be at least 1, not %d - build options from DefaultOptions()", o.MinRows)
	}
	if !(o.MaxHoleRatio >= 0 && o.MaxHoleRatio <= 1) {
		return fmt.Errorf("json-compress: MaxHoleRatio must be between 0 and 1, not %v", o.MaxHoleRatio)
	}
	if o.MinStringCount < 2 {
		return fmt.Errorf("json-compress: MinStringCount must be at least 2, not %d", o.MinStringCount)
	}
	if o.MinStringLength < 0 {
		return fmt.Errorf("json-compress: MinStringLength must not be negative, not %d", o.MinStringLength)
	}
	return nil
}

// resolveOptions takes the variadic options every entry point accepts and
// returns one validated set. More than one is a programming error rather than a
// merge: the second would silently win.
func resolveOptions(options []Options) (Options, error) {
	switch len(options) {
	case 0:
		return DefaultOptions(), nil
	case 1:
		if err := options[0].Validate(); err != nil {
			return Options{}, err
		}
		return options[0], nil
	default:
		return Options{}, fmt.Errorf("json-compress: expected at most one set of options, got %d", len(options))
	}
}
