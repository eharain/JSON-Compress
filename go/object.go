package jsoncompress

import "sort"

// Object is a JSON object that remembers the order its keys arrived in.
//
// A Go map does not, and encoding/json writes one out in sorted order, so a
// document decoded into map[string]any comes back with its members shuffled.
// This format promises the opposite - the same values and the same key order -
// so objects are represented by this type instead.
//
// The zero value is an empty object and is ready to use. An Object is not safe
// for concurrent modification.
type Object struct {
	keys   []string
	values []any
	// index is built only once an object is large enough to be worth hashing.
	// Documents in this format are mostly small objects - a row of a table, a
	// nested record - where a scan over a handful of keys beats a map, and the
	// map's own allocation costs more than the scan ever will.
	index map[string]int
}

// indexThreshold is where a linear scan stops being the cheaper way to find a
// key.
const indexThreshold = 12

// NewObject returns an empty object.
func NewObject() *Object {
	return &Object{}
}

// newObject returns an empty object with room for n members.
func newObject(n int) *Object {
	if n <= 0 {
		return &Object{}
	}
	out := &Object{keys: make([]string, 0, n), values: make([]any, 0, n)}
	if n > indexThreshold {
		out.index = make(map[string]int, n)
	}
	return out
}

// find is the position of a key, or -1.
func (o *Object) find(key string) int {
	if o.index != nil {
		if at, ok := o.index[key]; ok {
			return at
		}
		return -1
	}
	for at, name := range o.keys {
		if name == key {
			return at
		}
	}
	return -1
}

// buildIndex is called when an object grows past the point where scanning it is
// the cheaper option.
func (o *Object) buildIndex() {
	o.index = make(map[string]int, len(o.keys)*2)
	for at, key := range o.keys {
		o.index[key] = at
	}
}

// Len is the number of members.
func (o *Object) Len() int {
	if o == nil {
		return 0
	}
	return len(o.keys)
}

// Keys returns the member names, in order. The result is a copy; changing it
// does not change the object.
func (o *Object) Keys() []string {
	if o == nil || len(o.keys) == 0 {
		return nil
	}
	out := make([]string, len(o.keys))
	copy(out, o.keys)
	return out
}

// At returns the member at position i, counted from zero in key order. It
// panics if i is out of range, like any other index.
//
// This is the allocation-free way to walk an object:
//
//	for i := 0; i < obj.Len(); i++ {
//		key, value := obj.At(i)
//		...
//	}
func (o *Object) At(i int) (string, any) {
	return o.keys[i], o.values[i]
}

// Get returns the value stored under key, and whether the key is present at
// all. A key that is present with a nil value and a key that is absent are
// different things, here as in the format itself.
func (o *Object) Get(key string) (any, bool) {
	if o == nil {
		return nil, false
	}
	at := o.find(key)
	if at < 0 {
		return nil, false
	}
	return o.values[at], true
}

// Value returns the value stored under key, or nil when there is none.
func (o *Object) Value(key string) any {
	value, _ := o.Get(key)
	return value
}

// Has reports whether key is present.
func (o *Object) Has(key string) bool {
	_, ok := o.Get(key)
	return ok
}

// Set stores a value, appending the key when it is new and leaving it where it
// is when it is not - the same rule JSON.parse follows for a repeated key. It
// returns the object, so calls can be chained.
func (o *Object) Set(key string, value any) *Object {
	if at := o.find(key); at >= 0 {
		o.values[at] = value
		return o
	}

	o.keys = append(o.keys, key)
	o.values = append(o.values, value)

	switch {
	case o.index != nil:
		o.index[key] = len(o.keys) - 1
	case len(o.keys) > indexThreshold:
		o.buildIndex()
	}
	return o
}

// Delete removes a member. Removing a key that is not there does nothing.
func (o *Object) Delete(key string) *Object {
	if o == nil {
		return o
	}
	at := o.find(key)
	if at < 0 {
		return o
	}

	o.keys = append(o.keys[:at], o.keys[at+1:]...)
	o.values = append(o.values[:at], o.values[at+1:]...)
	if o.index != nil {
		// Every position after the hole has moved, so the index is rebuilt
		// rather than patched.
		o.buildIndex()
	}
	return o
}

// Clone returns a shallow copy: the same values under the same keys, in the
// same order, in an object that can be changed without touching this one.
func (o *Object) Clone() *Object {
	if o == nil {
		return nil
	}

	out := &Object{
		keys:   make([]string, len(o.keys)),
		values: make([]any, len(o.values)),
	}
	copy(out.keys, o.keys)
	copy(out.values, o.values)
	if o.index != nil {
		out.buildIndex()
	}
	return out
}

// MarshalJSON writes the object with its members in order.
//
// Note that encoding/json re-escapes what a [json.Marshaler] returns, so
// json.Marshal(obj) escapes <, > and & as Go always does. [FormatJSON] writes
// the bytes this format is defined in terms of.
func (o *Object) MarshalJSON() ([]byte, error) {
	return FormatJSON(o)
}

// UnmarshalJSON reads a JSON object, keeping the order its members arrived in.
func (o *Object) UnmarshalJSON(data []byte) error {
	value, err := ParseJSON(data)
	if err != nil {
		return err
	}
	parsed, ok := value.(*Object)
	if !ok {
		return &DecodeError{message: "expected a JSON object"}
	}
	*o = *parsed
	return nil
}

// objectOf reads whatever the caller handed us as an object, so that an
// envelope decoded by encoding/json - which gives map[string]any and loses the
// order - can still be restored. Sorted keys are as close to an order as a Go
// map has, and are at least the same on every run.
func objectOf(value any) (*Object, bool) {
	switch typed := value.(type) {
	case *Object:
		return typed, typed != nil
	case Object:
		return &typed, true
	case map[string]any:
		keys := make([]string, 0, len(typed))
		for key := range typed {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		out := newObject(len(keys))
		for _, key := range keys {
			out.Set(key, typed[key])
		}
		return out, true
	default:
		return nil, false
	}
}
