package jsoncompress

import (
	"encoding"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"math"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"sync"
)

// Normalize brings an arbitrary Go value down to the JSON value model: nil,
// bool, string, the three number types the model keeps, []any and [*Object].
//
// The rules are the ones encoding/json follows, so that a value compressed here
// and a value marshalled there describe the same document:
//
//   - a [json.Marshaler] is asked for its JSON, so a time.Time becomes the
//     string it always writes, and a [json.RawMessage] is read as the document
//     it holds;
//   - an [encoding.TextMarshaler] becomes its text;
//   - struct fields follow their json tags, including omitempty, and embedded
//     structs are flattened where encoding/json flattens them;
//   - a []byte becomes base64 text;
//   - a nil slice, map, pointer or interface becomes null;
//   - map keys are sorted, because a Go map has no order of its own.
//
// Two rules come from the wire format rather than from encoding/json, and are
// the ones every implementation shares: a non-finite number becomes null rather
// than an error, and negative zero becomes zero. A cycle is an error, not a
// hang.
//
// Integers keep their exact 64-bit value rather than being folded through a
// double, so a number too large for a float64 survives here where it would not
// in JavaScript.
func Normalize(value any) (any, error) {
	var state normalizer
	return state.value(value)
}

type normalizer struct {
	seen map[visited]bool
}

// visited identifies one container on the path from the root, which is all
// cycle detection needs: a value may appear many times in a document, but only
// a cycle has it appear inside itself.
type visited struct {
	pointer uintptr
	kind    reflect.Kind
}

func (n *normalizer) enter(pointer uintptr, kind reflect.Kind) error {
	if pointer == 0 {
		return nil
	}
	mark := visited{pointer, kind}
	if n.seen[mark] {
		return fmt.Errorf("json-compress: circular structure cannot be compressed")
	}
	if n.seen == nil {
		n.seen = make(map[visited]bool, 8)
	}
	n.seen[mark] = true
	return nil
}

func (n *normalizer) leave(pointer uintptr, kind reflect.Kind) {
	if pointer != 0 {
		delete(n.seen, visited{pointer, kind})
	}
}

// value normalises the types the model is built from directly, and hands
// everything else to reflection. The common case - a document that is already
// in the model, or is built from maps and slices of any - therefore costs a
// walk and no reflection at all.
func (n *normalizer) value(value any) (any, error) {
	switch typed := value.(type) {
	case nil:
		return nil, nil
	case bool, string:
		return typed, nil
	case float64:
		return finite(typed), nil
	case float32:
		return finite(float64(typed)), nil
	case int:
		return int64(typed), nil
	case int8:
		return int64(typed), nil
	case int16:
		return int64(typed), nil
	case int32:
		return int64(typed), nil
	case int64:
		return typed, nil
	case uint:
		return uint64(typed), nil
	case uint8:
		return uint64(typed), nil
	case uint16:
		return uint64(typed), nil
	case uint32:
		return uint64(typed), nil
	case uint64:
		return typed, nil
	case json.Number:
		return numberOf(typed)
	case []any:
		return n.slice(typed)
	case map[string]any:
		return n.stringMap(typed)
	case *Object:
		return n.object(typed)
	default:
		return n.reflect(reflect.ValueOf(value))
	}
}

// finite folds away the two things JSON cannot spell.
func finite(value float64) any {
	if math.IsNaN(value) || math.IsInf(value, 0) {
		return nil
	}
	if value == 0 {
		return float64(0) // including negative zero
	}
	return value
}

func numberOf(number json.Number) (any, error) {
	text := string(number)
	if !strings.ContainsAny(text, ".eE") {
		if value, err := strconv.ParseInt(text, 10, 64); err == nil {
			return value, nil
		}
		if value, err := strconv.ParseUint(text, 10, 64); err == nil {
			return value, nil
		}
	}
	value, err := strconv.ParseFloat(text, 64)
	if err != nil {
		return nil, fmt.Errorf("json-compress: %q is not a number", text)
	}
	return finite(value), nil
}

func (n *normalizer) slice(items []any) (any, error) {
	if items == nil {
		return nil, nil
	}
	pointer := reflect.ValueOf(items).Pointer()
	if len(items) > 0 {
		if err := n.enter(pointer, reflect.Slice); err != nil {
			return nil, err
		}
		defer n.leave(pointer, reflect.Slice)
	}

	changed := false
	out := items
	for i, item := range items {
		next, err := n.value(item)
		if err != nil {
			return nil, err
		}
		if !changed && !identical(next, item) {
			// Copy on first difference, so a document that needs nothing done
			// to it costs a walk and no allocation.
			out = make([]any, len(items))
			copy(out, items[:i])
			changed = true
		}
		if changed {
			out[i] = next
		}
	}
	return out, nil
}

func (n *normalizer) stringMap(items map[string]any) (any, error) {
	if items == nil {
		return nil, nil
	}
	pointer := reflect.ValueOf(items).Pointer()
	if err := n.enter(pointer, reflect.Map); err != nil {
		return nil, err
	}
	defer n.leave(pointer, reflect.Map)

	keys := make([]string, 0, len(items))
	for key := range items {
		keys = append(keys, key)
	}
	sort.Strings(keys)

	out := newObject(len(keys))
	for _, key := range keys {
		value, err := n.value(items[key])
		if err != nil {
			return nil, err
		}
		out.Set(key, value)
	}
	return out, nil
}

func (n *normalizer) object(object *Object) (any, error) {
	if object == nil {
		return nil, nil
	}
	pointer := reflect.ValueOf(object).Pointer()
	if err := n.enter(pointer, reflect.Ptr); err != nil {
		return nil, err
	}
	defer n.leave(pointer, reflect.Ptr)

	var out *Object
	for i := 0; i < object.Len(); i++ {
		key, item := object.At(i)
		next, err := n.value(item)
		if err != nil {
			return nil, err
		}
		if out == nil && !identical(next, item) {
			out = newObject(object.Len())
			for j := 0; j < i; j++ {
				out.Set(object.At(j))
			}
		}
		if out != nil {
			out.Set(key, next)
		}
	}
	if out == nil {
		return object, nil
	}
	return out, nil
}

// identical reports whether normalising left a value exactly as it was, which
// is what decides whether a container has to be copied.
func identical(a, b any) bool {
	switch typed := a.(type) {
	case nil:
		return b == nil
	case bool:
		other, ok := b.(bool)
		return ok && other == typed
	case string:
		other, ok := b.(string)
		return ok && other == typed
	case float64:
		other, ok := b.(float64)
		return ok && other == typed
	case int64:
		other, ok := b.(int64)
		return ok && other == typed
	case uint64:
		other, ok := b.(uint64)
		return ok && other == typed
	case *Object:
		other, ok := b.(*Object)
		return ok && other == typed
	case []any:
		other, ok := b.([]any)
		return ok && len(other) == len(typed) && sameSlice(typed, other)
	default:
		return false
	}
}

func sameSlice(a, b []any) bool {
	if len(a) == 0 {
		return len(b) == 0
	}
	return reflect.ValueOf(a).Pointer() == reflect.ValueOf(b).Pointer()
}

/* ------------------------------------------------------------------ *
 * Reflection
 * ------------------------------------------------------------------ */

var (
	jsonMarshaler = reflect.TypeOf((*json.Marshaler)(nil)).Elem()
	textMarshaler = reflect.TypeOf((*encoding.TextMarshaler)(nil)).Elem()
)

func (n *normalizer) reflect(value reflect.Value) (any, error) {
	if !value.IsValid() {
		return nil, nil
	}

	if out, done, err := n.marshaler(value); done {
		return out, err
	}

	switch value.Kind() {
	case reflect.Bool:
		return value.Bool(), nil
	case reflect.String:
		return value.String(), nil
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return value.Int(), nil
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		return value.Uint(), nil
	case reflect.Float32, reflect.Float64:
		return finite(value.Float()), nil

	case reflect.Interface:
		if value.IsNil() {
			return nil, nil
		}
		return n.value(value.Elem().Interface())

	case reflect.Ptr:
		if value.IsNil() {
			return nil, nil
		}
		pointer := value.Pointer()
		if err := n.enter(pointer, reflect.Ptr); err != nil {
			return nil, err
		}
		defer n.leave(pointer, reflect.Ptr)
		return n.reflectElem(value.Elem())

	case reflect.Slice:
		if value.IsNil() {
			return nil, nil // encoding/json writes a nil slice as null
		}
		if value.Type().Elem().Kind() == reflect.Uint8 {
			return base64.StdEncoding.EncodeToString(bytesOf(value)), nil
		}
		pointer := value.Pointer()
		if err := n.enter(pointer, reflect.Slice); err != nil {
			return nil, err
		}
		defer n.leave(pointer, reflect.Slice)
		return n.list(value)

	case reflect.Array:
		return n.list(value)

	case reflect.Map:
		if value.IsNil() {
			return nil, nil
		}
		pointer := value.Pointer()
		if err := n.enter(pointer, reflect.Map); err != nil {
			return nil, err
		}
		defer n.leave(pointer, reflect.Map)
		return n.mapping(value)

	case reflect.Struct:
		return n.structure(value)

	default:
		return nil, fmt.Errorf("json-compress: a %s cannot be serialised as JSON", value.Type())
	}
}

// reflectElem re-enters through value() where it can, so that a pointer to
// something the model already knows - an *Object inside a struct, say - is
// recognised rather than taken apart field by field.
func (n *normalizer) reflectElem(value reflect.Value) (any, error) {
	if value.CanInterface() {
		return n.value(value.Interface())
	}
	return n.reflect(value)
}

func (n *normalizer) marshaler(value reflect.Value) (any, bool, error) {
	valueType := value.Type()

	if valueType.Implements(jsonMarshaler) {
		if isNilable(value) && value.IsNil() {
			return nil, true, nil
		}
		text, err := value.Interface().(json.Marshaler).MarshalJSON()
		if err != nil {
			return nil, true, fmt.Errorf("json-compress: %s could not write itself as JSON: %w", valueType, err)
		}
		parsed, err := ParseJSON(text)
		if err != nil {
			return nil, true, fmt.Errorf("json-compress: %s wrote something that is not JSON: %w", valueType, err)
		}
		return parsed, true, nil
	}

	if valueType.Implements(textMarshaler) {
		if isNilable(value) && value.IsNil() {
			return nil, true, nil
		}
		text, err := value.Interface().(encoding.TextMarshaler).MarshalText()
		if err != nil {
			return nil, true, fmt.Errorf("json-compress: %s could not write itself as text: %w", valueType, err)
		}
		return string(text), true, nil
	}

	return nil, false, nil
}

func isNilable(value reflect.Value) bool {
	switch value.Kind() {
	case reflect.Ptr, reflect.Interface, reflect.Map, reflect.Slice, reflect.Func, reflect.Chan:
		return true
	default:
		return false
	}
}

func bytesOf(value reflect.Value) []byte {
	if value.Type().Elem() == reflect.TypeOf(byte(0)) {
		return value.Bytes()
	}
	out := make([]byte, value.Len())
	for i := range out {
		out[i] = byte(value.Index(i).Uint())
	}
	return out
}

func (n *normalizer) list(value reflect.Value) (any, error) {
	count := value.Len()
	out := make([]any, count)
	for i := 0; i < count; i++ {
		item, err := n.reflectElem(value.Index(i))
		if err != nil {
			return nil, err
		}
		out[i] = item
	}
	return out, nil
}

func (n *normalizer) mapping(value reflect.Value) (any, error) {
	keys := value.MapKeys()
	names := make([]string, len(keys))
	for i, key := range keys {
		name, err := keyText(key)
		if err != nil {
			return nil, err
		}
		names[i] = name
	}
	order := make([]int, len(keys))
	for i := range order {
		order[i] = i
	}
	sort.SliceStable(order, func(a, b int) bool { return names[order[a]] < names[order[b]] })

	out := newObject(len(keys))
	for _, at := range order {
		item, err := n.reflectElem(value.MapIndex(keys[at]))
		if err != nil {
			return nil, err
		}
		out.Set(names[at], item)
	}
	return out, nil
}

// keyText is the name a map key is written under, following encoding/json: a
// string as it stands, an integer in decimal, anything else through its text
// form or refused.
func keyText(key reflect.Value) (string, error) {
	if key.Kind() != reflect.String && key.Type().Implements(textMarshaler) {
		text, err := key.Interface().(encoding.TextMarshaler).MarshalText()
		if err != nil {
			return "", fmt.Errorf("json-compress: a %s key could not write itself as text: %w", key.Type(), err)
		}
		return string(text), nil
	}

	switch key.Kind() {
	case reflect.String:
		return key.String(), nil
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return strconv.FormatInt(key.Int(), 10), nil
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		return strconv.FormatUint(key.Uint(), 10), nil
	default:
		return "", fmt.Errorf("json-compress: a %s cannot be an object key - convert it to a string first", key.Type())
	}
}

func (n *normalizer) structure(value reflect.Value) (any, error) {
	fields := fieldsOf(value.Type())
	out := newObject(len(fields))
	for i := range fields {
		field := &fields[i]
		member := fieldValue(value, field.index)
		if !member.IsValid() {
			continue // an embedded nil pointer on the way to this field
		}
		if field.omitEmpty && isEmpty(member) {
			continue
		}
		item, err := n.reflectElem(member)
		if err != nil {
			return nil, err
		}
		out.Set(field.name, item)
	}
	return out, nil
}

func fieldValue(value reflect.Value, index []int) reflect.Value {
	for at, step := range index {
		if at > 0 {
			if value.Kind() == reflect.Ptr {
				if value.IsNil() {
					return reflect.Value{}
				}
				value = value.Elem()
			}
		}
		value = value.Field(step)
	}
	return value
}

func isEmpty(value reflect.Value) bool {
	switch value.Kind() {
	case reflect.Array, reflect.Map, reflect.Slice, reflect.String:
		return value.Len() == 0
	case reflect.Bool:
		return !value.Bool()
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return value.Int() == 0
	case reflect.Uint, reflect.Uint8, reflect.Uint16, reflect.Uint32, reflect.Uint64, reflect.Uintptr:
		return value.Uint() == 0
	case reflect.Float32, reflect.Float64:
		return value.Float() == 0
	case reflect.Interface, reflect.Ptr:
		return value.IsNil()
	default:
		return false
	}
}

/* ------------------------------------------------------------------ *
 * Struct fields
 * ------------------------------------------------------------------ */

type structField struct {
	name      string
	index     []int
	depth     int
	tagged    bool
	omitEmpty bool
}

var fieldCache sync.Map // reflect.Type -> []structField

// fieldsOf lists the fields of a struct in the order and under the names
// encoding/json would write them: json tags honoured, unexported fields left
// out, embedded structs flattened, and a name claimed at two different depths
// going to the shallower one.
func fieldsOf(structType reflect.Type) []structField {
	if cached, ok := fieldCache.Load(structType); ok {
		return cached.([]structField)
	}

	var found []structField
	collectFields(structType, nil, 0, map[reflect.Type]bool{}, &found)

	// Resolve the fields that claim the same name.
	byName := map[string][]int{}
	for i := range found {
		byName[found[i].name] = append(byName[found[i].name], i)
	}
	keep := make([]structField, 0, len(found))
	for _, group := range byName {
		if winner, ok := dominantField(found, group); ok {
			keep = append(keep, winner)
		}
	}
	sort.Slice(keep, func(a, b int) bool { return lessIndex(keep[a].index, keep[b].index) })

	fieldCache.Store(structType, keep)
	return keep
}

// dominantField settles a name claimed by more than one field, by the rule
// encoding/json settles it with: the shallowest field wins, and where several
// are equally shallow, a tagged one wins - but only if it is the only tagged one.
// Where nothing wins, the name is written by nobody.
func dominantField(fields []structField, group []int) (structField, bool) {
	if len(group) == 1 {
		return fields[group[0]], true
	}

	shallowest := fields[group[0]].depth
	for _, at := range group[1:] {
		if fields[at].depth < shallowest {
			shallowest = fields[at].depth
		}
	}

	var contenders, tagged []int
	for _, at := range group {
		if fields[at].depth != shallowest {
			continue
		}
		contenders = append(contenders, at)
		if fields[at].tagged {
			tagged = append(tagged, at)
		}
	}

	if len(tagged) == 1 {
		return fields[tagged[0]], true
	}
	if len(tagged) == 0 && len(contenders) == 1 {
		return fields[contenders[0]], true
	}
	return structField{}, false
}

func collectFields(structType reflect.Type, prefix []int, depth int, seen map[reflect.Type]bool, out *[]structField) {
	if seen[structType] {
		return // an embedded type that reaches itself
	}
	seen[structType] = true
	defer delete(seen, structType)

	for i := 0; i < structType.NumField(); i++ {
		field := structType.Field(i)
		if field.PkgPath != "" && !field.Anonymous {
			continue // unexported
		}

		tag := field.Tag.Get("json")
		if tag == "-" {
			continue
		}
		name, rest, _ := strings.Cut(tag, ",")

		fieldType := field.Type
		if field.Anonymous && name == "" {
			embedded := fieldType
			if embedded.Kind() == reflect.Ptr {
				embedded = embedded.Elem()
			}
			if embedded.Kind() == reflect.Struct {
				// An embedded struct contributes its own fields at this
				// level, whether or not the field itself is exported.
				collectFields(embedded, append(append([]int{}, prefix...), i), depth+1, seen, out)
				continue
			}
			if field.PkgPath != "" {
				continue // an unexported embedded non-struct
			}
		}

		tagged := name != ""
		if !tagged {
			name = field.Name
		}
		*out = append(*out, structField{
			name:      name,
			index:     append(append([]int{}, prefix...), i),
			depth:     depth,
			tagged:    tagged,
			omitEmpty: hasOption(rest, "omitempty"),
		})
	}
}

func hasOption(options, want string) bool {
	for options != "" {
		var option string
		option, options, _ = strings.Cut(options, ",")
		if option == want {
			return true
		}
	}
	return false
}

func lessIndex(a, b []int) bool {
	for i := 0; i < len(a) && i < len(b); i++ {
		if a[i] != b[i] {
			return a[i] < b[i]
		}
	}
	return len(a) < len(b)
}
