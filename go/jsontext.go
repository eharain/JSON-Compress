package jsoncompress

import (
	"fmt"
	"math"
	"strconv"
	"strings"
	"unicode/utf8"
)

// Writing JSON text, character for character as JSON.stringify writes it. The
// reader lives next door, in jsonparse.go.

// maxIndent is the widest indent JSON.stringify honours, and so the widest this
// honours.
const maxIndent = 10

/* ------------------------------------------------------------------ *
 * Writing
 * ------------------------------------------------------------------ */

// FormatJSON serialises a value as JSON text, character for character as
// JSON.stringify would write it.
//
// That is not quite what encoding/json writes. Go escapes <, > and & for the
// benefit of HTML documents, and switches to exponent form at different
// magnitudes than ECMAScript does - so 0.00001 comes out as 1e-05 there and
// 0.00001 here. The wire format is defined against the ECMAScript spelling, and
// every implementation of it agrees byte for byte, which is what this preserves.
//
// Values are normalised on the way in, so anything encoding/json can marshal
// can be written here.
func FormatJSON(value any) ([]byte, error) {
	return formatJSON(value, "")
}

// FormatJSONIndent is [FormatJSON] with each level indented by the given number
// of spaces, capped at ten as JSON.stringify caps it. An indent of zero or less
// writes the compact form.
func FormatJSONIndent(value any, indent int) ([]byte, error) {
	return formatJSON(value, indentPad(indent))
}

func indentPad(indent int) string {
	if indent <= 0 {
		return ""
	}
	if indent > maxIndent {
		indent = maxIndent
	}
	return strings.Repeat(" ", indent)
}

func formatJSON(value any, pad string) ([]byte, error) {
	normalised, err := Normalize(value)
	if err != nil {
		return nil, err
	}
	return formatModel(normalised, pad)
}

// formatModel writes a value that is already known to be in the model, which is
// what the encoder hands back - so an envelope is not walked a second time on
// its way out.
func formatModel(value any, pad string) ([]byte, error) {
	return writeValue(make([]byte, 0, 256), value, pad, 0)
}

func writeValue(out []byte, value any, pad string, level int) ([]byte, error) {
	switch typed := value.(type) {
	case nil:
		return append(out, "null"...), nil
	case bool:
		if typed {
			return append(out, "true"...), nil
		}
		return append(out, "false"...), nil
	case string:
		return writeString(out, typed), nil
	case float64:
		return append(out, formatFloat(typed)...), nil
	case int:
		return strconv.AppendInt(out, int64(typed), 10), nil
	case int64:
		return strconv.AppendInt(out, typed, 10), nil
	case uint64:
		return strconv.AppendUint(out, typed, 10), nil
	case []any:
		return writeArray(out, typed, pad, level)
	case *Object:
		return writeObject(out, typed, pad, level)
	default:
		return nil, fmt.Errorf("json-compress: %T is not a JSON value - normalise it before writing", value)
	}
}

func writeArray(out []byte, items []any, pad string, level int) ([]byte, error) {
	if len(items) == 0 {
		return append(out, "[]"...), nil
	}
	out = append(out, '[')
	for i, item := range items {
		if i > 0 {
			out = append(out, ',')
		}
		if pad != "" {
			out = appendNewline(out, pad, level+1)
		}
		var err error
		if out, err = writeValue(out, item, pad, level+1); err != nil {
			return nil, err
		}
	}
	if pad != "" {
		out = appendNewline(out, pad, level)
	}
	return append(out, ']'), nil
}

func writeObject(out []byte, object *Object, pad string, level int) ([]byte, error) {
	if object.Len() == 0 {
		return append(out, "{}"...), nil
	}
	out = append(out, '{')
	for i := 0; i < object.Len(); i++ {
		key, value := object.At(i)
		if i > 0 {
			out = append(out, ',')
		}
		if pad != "" {
			out = appendNewline(out, pad, level+1)
		}
		out = writeString(out, key)
		out = append(out, ':')
		if pad != "" {
			out = append(out, ' ')
		}
		var err error
		if out, err = writeValue(out, value, pad, level+1); err != nil {
			return nil, err
		}
	}
	if pad != "" {
		out = appendNewline(out, pad, level)
	}
	return append(out, '}'), nil
}

func appendNewline(out []byte, pad string, level int) []byte {
	out = append(out, '\n')
	for i := 0; i < level; i++ {
		out = append(out, pad...)
	}
	return out
}

const hexDigits = "0123456789abcdef"

// replacementCharacter is U+FFFD, written for a byte that is not part of any
// valid encoding - the same substitution encoding/json makes.
var replacementCharacter = []byte{0xEF, 0xBF, 0xBD}

// writeString quotes a string exactly as JSON.stringify quotes it: the two
// mandatory escapes, the five short control-character escapes, \u00xx for the
// rest of the control range, and everything else as it stands. Notably the
// solidus, the line separators U+2028 and U+2029, and every non-ASCII character
// are written raw, where encoding/json would escape some of them.
func writeString(out []byte, text string) []byte {
	out = append(out, '"')
	start := 0
	for i := 0; i < len(text); {
		char := text[i]
		if char < utf8.RuneSelf {
			if char >= 0x20 && char != '"' && char != '\\' {
				i++
				continue
			}
			out = append(out, text[start:i]...)
			switch char {
			case '"':
				out = append(out, '\\', '"')
			case '\\':
				out = append(out, '\\', '\\')
			case '\b':
				out = append(out, '\\', 'b')
			case '\f':
				out = append(out, '\\', 'f')
			case '\n':
				out = append(out, '\\', 'n')
			case '\r':
				out = append(out, '\\', 'r')
			case '\t':
				out = append(out, '\\', 't')
			default:
				out = append(out, '\\', 'u', '0', '0', hexDigits[char>>4], hexDigits[char&0xF])
			}
			i++
			start = i
			continue
		}

		if _, size := utf8.DecodeRuneInString(text[i:]); size > 1 {
			i += size
			continue
		}

		// Not valid UTF-8. A lone surrogate is written the way JSON.stringify
		// writes one, since the other implementations can carry it; anything
		// else becomes the replacement character.
		out = append(out, text[start:i]...)
		if surrogate, width := loneSurrogate(text[i:]); width > 0 {
			out = append(out, '\\', 'u',
				hexDigits[(surrogate>>12)&0xF], hexDigits[(surrogate>>8)&0xF],
				hexDigits[(surrogate>>4)&0xF], hexDigits[surrogate&0xF])
			i += width
		} else {
			out = append(out, replacementCharacter...)
			i++
		}
		start = i
	}
	out = append(out, text[start:]...)
	return append(out, '"')
}

// loneSurrogate reads a surrogate code point held in the three-byte form UTF-8
// forbids, which is the only way one reaches a Go string at all.
func loneSurrogate(text string) (rune, int) {
	if len(text) < 3 || text[0] != 0xED {
		return 0, 0
	}
	if text[1] < 0xA0 || text[1] > 0xBF || text[2] < 0x80 || text[2] > 0xBF {
		return 0, 0
	}
	return rune(text[0]&0x0F)<<12 | rune(text[1]&0x3F)<<6 | rune(text[2]&0x3F), 3
}

// formatFloat renders a number the way ECMAScript's Number::toString would.
//
// Go and JavaScript agree on the digits - both print the shortest decimal that
// reads back as the same double - and disagree only on where the decimal point
// goes and when to switch to an exponent. This applies the ECMAScript placement
// rules to Go's digits.
func formatFloat(value float64) string {
	// Non-finite values have no JSON spelling; JSON writers substitute null.
	if math.IsNaN(value) || math.IsInf(value, 0) {
		return "null"
	}
	if value == 0 {
		return "0" // including negative zero, which JSON cannot tell apart
	}

	sign := ""
	if value < 0 {
		sign = "-"
		value = -value
	}

	// Shortest round-trip digits, in the form d.ddde±dd, from which the digit
	// string and the decimal exponent fall out directly.
	text := strconv.FormatFloat(value, 'e', -1, 64)
	at := strings.IndexByte(text, 'e')
	exponent, err := strconv.Atoi(text[at+1:])
	if err != nil {
		return sign + text
	}
	digits := strings.Replace(text[:at], ".", "", 1)

	k := len(digits)
	n := exponent + 1

	switch {
	case k <= n && n <= 21:
		return sign + digits + strings.Repeat("0", n-k)
	case 0 < n && n <= 21:
		return sign + digits[:n] + "." + digits[n:]
	case -6 < n && n <= 0:
		return sign + "0." + strings.Repeat("0", -n) + digits
	}

	tail := "e+" + strconv.Itoa(n-1)
	if n-1 < 0 {
		tail = "e-" + strconv.Itoa(1-n)
	}
	if k == 1 {
		return sign + digits + tail
	}
	return sign + digits[:1] + "." + digits[1:] + tail
}

// utf16Length is the length of a string in UTF-16 code units.
//
// That is what String.length counts in JavaScript, and the encoder's cost model
// is calibrated against it - so an emoji weighs two, not one, and the same
// string is catalogued here and there or in neither.
func utf16Length(text string) int {
	length := 0
	for _, char := range text {
		length++
		if char > 0xFFFF {
			length++
		}
	}
	return length
}
