package jsoncompress

import (
	"fmt"
	"strconv"
	"unicode/utf16"
	"unicode/utf8"
)

// Reading JSON text.
//
// This is a direct scan of the document, rather than a walk over
// encoding/json's token stream, for one reason: the token stream boxes every
// string and every number into an interface, which costs an allocation apiece
// and turns a large record set into millions of them. Reading the bytes here
// instead is around ten times quicker on the documents this format is for, and
// it is the same grammar - what encoding/json accepts, this accepts, and what
// it refuses, this refuses, down to how an unpaired surrogate is folded to
// U+FFFD.

// maxParseDepth is the nesting a document may reach before it is refused. The
// limit exists so that a hostile document exhausts a counter rather than the
// stack; encoding/json draws its own line in the same place.
const maxParseDepth = 10000

// ParseJSON reads JSON text into the value model: nil, bool, float64, string,
// []any and [*Object].
//
// It is JSON.parse, with objects that remember their key order. Numbers become
// float64, as they do in every other implementation of this format and in
// encoding/json.
func ParseJSON(data []byte) (any, error) {
	reader := &parser{data: data}

	value, err := reader.value(0)
	if err != nil {
		return nil, err
	}

	// A JSON document is one value. Anything after it is a second document, and
	// ignoring it silently would hide a truncated write.
	reader.skipSpace()
	if reader.at < len(reader.data) {
		return nil, reader.failAt(reader.at, "unexpected data after the end of the document")
	}
	return value, nil
}

// ParseJSONString is [ParseJSON] for a string.
func ParseJSONString(text string) (any, error) {
	return ParseJSON([]byte(text))
}

type parser struct {
	data []byte
	at   int
}

func (p *parser) fail(format string, args ...any) error {
	return p.failAt(p.at, format, args...)
}

func (p *parser) failAt(at int, format string, args ...any) error {
	if at >= len(p.data) {
		return fmt.Errorf("json-compress: the document ends before it is complete")
	}
	return fmt.Errorf("json-compress: "+format+", at byte %d", append(args, at)...)
}

func (p *parser) skipSpace() {
	for p.at < len(p.data) {
		switch p.data[p.at] {
		case ' ', '\t', '\n', '\r':
			p.at++
		default:
			return
		}
	}
}

func (p *parser) value(depth int) (any, error) {
	if depth > maxParseDepth {
		return nil, p.fail("the document is nested too deeply to read")
	}

	p.skipSpace()
	if p.at >= len(p.data) {
		return nil, p.fail("expected a value")
	}

	switch char := p.data[p.at]; char {
	case '{':
		return p.object(depth)
	case '[':
		return p.array(depth)
	case '"':
		return p.text()
	case 't':
		return true, p.literal("true")
	case 'f':
		return false, p.literal("false")
	case 'n':
		return nil, p.literal("null")
	default:
		if char == '-' || (char >= '0' && char <= '9') {
			return p.number()
		}
		return nil, p.fail("unexpected character %q", rune(char))
	}
}

func (p *parser) literal(want string) error {
	if p.at+len(want) > len(p.data) || string(p.data[p.at:p.at+len(want)]) != want {
		return p.fail("expected %s", want)
	}
	p.at += len(want)
	return nil
}

func (p *parser) object(depth int) (any, error) {
	p.at++ // the opening brace
	out := NewObject()

	p.skipSpace()
	if p.at < len(p.data) && p.data[p.at] == '}' {
		p.at++
		return out, nil
	}

	for {
		p.skipSpace()
		if p.at >= len(p.data) || p.data[p.at] != '"' {
			return nil, p.fail("expected a quoted property name")
		}
		key, err := p.text()
		if err != nil {
			return nil, err
		}

		p.skipSpace()
		if p.at >= len(p.data) || p.data[p.at] != ':' {
			return nil, p.fail("expected a colon after the property name")
		}
		p.at++

		value, err := p.value(depth + 1)
		if err != nil {
			return nil, err
		}
		out.Set(key, value)

		p.skipSpace()
		if p.at >= len(p.data) {
			return nil, p.fail("this object is never closed")
		}
		switch p.data[p.at] {
		case ',':
			p.at++
		case '}':
			p.at++
			return out, nil
		default:
			return nil, p.fail("expected a comma or a closing brace")
		}
	}
}

func (p *parser) array(depth int) (any, error) {
	p.at++ // the opening bracket
	out := []any{}

	p.skipSpace()
	if p.at < len(p.data) && p.data[p.at] == ']' {
		p.at++
		return out, nil
	}

	for {
		value, err := p.value(depth + 1)
		if err != nil {
			return nil, err
		}
		out = append(out, value)

		p.skipSpace()
		if p.at >= len(p.data) {
			return nil, p.fail("this array is never closed")
		}
		switch p.data[p.at] {
		case ',':
			p.at++
		case ']':
			p.at++
			return out, nil
		default:
			return nil, p.fail("expected a comma or a closing bracket")
		}
	}
}

// text reads a string. The common case - no escapes, valid UTF-8 - is one
// allocation for the string itself and nothing else.
func (p *parser) text() (string, error) {
	start := p.at
	p.at++ // the opening quote

	for at := p.at; at < len(p.data); at++ {
		char := p.data[at]
		if char == '"' {
			plain := p.data[p.at:at]
			p.at = at + 1
			if utf8.Valid(plain) {
				return string(plain), nil
			}
			return coerceUTF8(plain), nil
		}
		if char == '\\' {
			return p.escapedText(start)
		}
		if char < 0x20 {
			return "", p.failAt(at, "a raw control character inside a string")
		}
	}

	return "", p.failAt(start, "this string is never closed")
}

// escapedText finishes a string that turned out to hold an escape sequence.
func (p *parser) escapedText(start int) (string, error) {
	// Unescaping only ever shortens, so the distance to the closing quote is an
	// upper bound on the result and the buffer is sized once. Sizing it from
	// what is left of the document instead would allocate a megabyte per string
	// in a document full of escapes.
	out := make([]byte, 0, p.textExtent())

	for p.at < len(p.data) {
		char := p.data[p.at]

		switch {
		case char == '"':
			p.at++
			return string(out), nil

		case char == '\\':
			p.at++
			if p.at >= len(p.data) {
				return "", p.failAt(start, "this string is never closed")
			}
			switch escape := p.data[p.at]; escape {
			case '"', '\\', '/':
				out = append(out, escape)
				p.at++
			case 'b':
				out = append(out, '\b')
				p.at++
			case 'f':
				out = append(out, '\f')
				p.at++
			case 'n':
				out = append(out, '\n')
				p.at++
			case 'r':
				out = append(out, '\r')
				p.at++
			case 't':
				out = append(out, '\t')
				p.at++
			case 'u':
				character, err := p.escapedRune()
				if err != nil {
					return "", err
				}
				out = utf8.AppendRune(out, character)
			default:
				return "", p.failAt(p.at-1, "%q is not a valid escape sequence", "\\"+string(rune(escape)))
			}

		case char < 0x20:
			return "", p.fail("a raw control character inside a string")

		case char < utf8.RuneSelf:
			out = append(out, char)
			p.at++

		default:
			character, size := utf8.DecodeRune(p.data[p.at:])
			if character == utf8.RuneError && size == 1 {
				out = append(out, replacementCharacter...)
			} else {
				out = append(out, p.data[p.at:p.at+size]...)
			}
			p.at += size
		}
	}

	return "", p.failAt(start, "this string is never closed")
}

// textExtent is how many bytes are left before this string's closing quote, or
// what is left of the document if it never closes. Escaped quotes are stepped
// over; a wrong answer here only costs a resize, never a correct result.
func (p *parser) textExtent() int {
	for at := p.at; at < len(p.data); {
		switch p.data[at] {
		case '\\':
			at += 2
		case '"':
			return at - p.at
		default:
			at++
		}
	}
	return len(p.data) - p.at
}

// escapedRune reads a \u escape, and the second half of a surrogate pair when
// there is one. An unpaired surrogate becomes U+FFFD, which is what
// encoding/json makes of it and the only thing a Go string can hold.
func (p *parser) escapedRune() (rune, error) {
	first, err := p.hex4()
	if err != nil {
		return 0, err
	}

	if !utf16.IsSurrogate(first) {
		return first, nil
	}

	// A high surrogate may be followed by its low half, written as a second
	// escape; anything else leaves it unpaired.
	if p.at+1 < len(p.data) && p.data[p.at] == '\\' && p.data[p.at+1] == 'u' {
		mark := p.at
		p.at++ // hex4 reads from the u, not from the backslash
		second, err := p.hex4()
		if err != nil {
			return 0, err
		}
		if paired := utf16.DecodeRune(first, second); paired != utf8.RuneError {
			return paired, nil
		}
		p.at = mark // the second escape stands on its own
	}

	return utf8.RuneError, nil
}

// hex4 reads the four hexadecimal digits of a \u escape, having already read
// the backslash and the u.
func (p *parser) hex4() (rune, error) {
	at := p.at + 1 // the u
	if at+4 > len(p.data) {
		return 0, p.failAt(p.at-1, `\u must be followed by four hexadecimal digits`)
	}

	value := rune(0)
	for i := 0; i < 4; i++ {
		digit := p.data[at+i]
		switch {
		case digit >= '0' && digit <= '9':
			value = value*16 + rune(digit-'0')
		case digit >= 'a' && digit <= 'f':
			value = value*16 + rune(digit-'a'+10)
		case digit >= 'A' && digit <= 'F':
			value = value*16 + rune(digit-'A'+10)
		default:
			return 0, p.failAt(p.at-1, `\u must be followed by four hexadecimal digits`)
		}
	}

	p.at = at + 4
	return value, nil
}

// number reads a JSON number, which is stricter than strconv is: no leading
// plus, no leading zero, no bare decimal point, no hexadecimal.
func (p *parser) number() (any, error) {
	start := p.at

	if p.data[p.at] == '-' {
		p.at++
		if p.at >= len(p.data) {
			return nil, p.failAt(start, "this number has no digits")
		}
	}

	switch char := p.data[p.at]; {
	case char == '0':
		p.at++
	case char >= '1' && char <= '9':
		for p.at < len(p.data) && isDigit(p.data[p.at]) {
			p.at++
		}
	default:
		return nil, p.failAt(start, "this number has no digits")
	}

	if p.at < len(p.data) && p.data[p.at] == '.' {
		p.at++
		if p.at >= len(p.data) || !isDigit(p.data[p.at]) {
			return nil, p.failAt(start, "a decimal point must be followed by at least one digit")
		}
		for p.at < len(p.data) && isDigit(p.data[p.at]) {
			p.at++
		}
	}

	if p.at < len(p.data) && (p.data[p.at] == 'e' || p.data[p.at] == 'E') {
		p.at++
		if p.at < len(p.data) && (p.data[p.at] == '+' || p.data[p.at] == '-') {
			p.at++
		}
		if p.at >= len(p.data) || !isDigit(p.data[p.at]) {
			return nil, p.failAt(start, "an exponent must have at least one digit")
		}
		for p.at < len(p.data) && isDigit(p.data[p.at]) {
			p.at++
		}
	}

	text := string(p.data[start:p.at])
	value, err := strconv.ParseFloat(text, 64)
	if err != nil {
		// Only a magnitude a double cannot hold gets this far.
		return nil, fmt.Errorf("json-compress: %s cannot be read as a number", text)
	}
	return value, nil
}

// coerceUTF8 replaces the bytes that are not part of any character, the way
// encoding/json replaces them, so that what comes out of the parser is always
// something a Go string can hold and write back.
func coerceUTF8(data []byte) string {
	out := make([]byte, 0, len(data))
	for at := 0; at < len(data); {
		if data[at] < utf8.RuneSelf {
			out = append(out, data[at])
			at++
			continue
		}
		character, size := utf8.DecodeRune(data[at:])
		if character == utf8.RuneError && size == 1 {
			out = append(out, replacementCharacter...)
		} else {
			out = append(out, data[at:at+size]...)
		}
		at += size
	}
	return string(out)
}
