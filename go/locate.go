package jsoncompress

import (
	"fmt"
	"strings"
	"unicode/utf8"
)

// Finding the exact place a JSON document goes wrong.
//
// A parser reports a message, and what that message contains depends on the
// parser: sometimes a byte offset, sometimes a line and column, increasingly
// often just a fragment of the text with no position at all. None of that is
// enough to put a cursor on the offending character.
//
// So this scans the document itself, against the grammar in RFC 8259, and
// reports the first thing that does not belong - with an offset, a line, a
// column and a description written for whoever has to fix it.

// Location is what a scan of a JSON document found.
type Location struct {
	// Valid is true when the document is well-formed JSON, in which case every
	// other field is zero.
	Valid bool
	// Error says what is wrong, in plain words.
	Error string
	// Position is the byte offset of the problem, so text[Position:] starts at
	// it.
	Position int
	// Line is the line number, counted from one.
	Line int
	// Column is the column number, counted from one in characters - so a column
	// means what it means in an editor even when the line above holds an emoji.
	Column int
	// Excerpt is the line the problem is on.
	Excerpt string
}

// String is a one-line summary, ready to log.
func (l Location) String() string {
	if l.Valid {
		return "valid JSON"
	}
	if l.Line == 0 {
		return "invalid JSON: " + l.Error
	}
	return fmt.Sprintf("invalid JSON at line %d, column %d: %s", l.Line, l.Column, l.Error)
}

// scanError is somewhere to unwind to, carrying the offset the problem was found
// at.
type scanError struct {
	message string
	at      int
}

func (e *scanError) Error() string {
	return e.message
}

// Locate checks a JSON document and, if it is broken, says exactly where.
//
// It is only worth running after a parse has already failed, so the cost of a
// second pass falls on documents that were not going to work anyway.
func Locate(text string) Location {
	scanner := &scanner{text: text}

	err := scanner.scanValue(0)
	if err == nil {
		scanner.skipWhitespace()
		if scanner.at < len(text) {
			err = &scanError{"unexpected " + describe(text, scanner.at) + " after the end of the document", scanner.at}
		}
	}
	if err == nil {
		return Location{Valid: true}
	}

	failure := err.(*scanError)
	at := failure.at
	if at < 0 {
		at = 0
	}
	if at > len(text) {
		at = len(text)
	}

	line, column, excerpt := positionOf(text, at)
	return Location{
		Valid:    false,
		Error:    failure.message,
		Position: at,
		Line:     line,
		Column:   column,
		Excerpt:  excerpt,
	}
}

// positionOf turns a byte offset into a line and column, both counted from one,
// and the line the offset falls on.
func positionOf(text string, at int) (int, int, string) {
	before := text[:at]
	line := strings.Count(before, "\n") + 1
	start := strings.LastIndexByte(before, '\n') + 1
	column := utf8.RuneCountInString(before[start:]) + 1

	end := strings.IndexByte(text[start:], '\n')
	if end < 0 {
		return line, column, text[start:]
	}
	return line, column, text[start : start+end]
}

func describe(text string, at int) string {
	if at >= len(text) {
		return "the document ends here"
	}
	char := text[at]
	if char < 0x20 {
		return fmt.Sprintf("an unescaped control character (0x%02x)", char)
	}
	character, _ := utf8.DecodeRuneInString(text[at:])
	return `"` + string(character) + `"`
}

type scanner struct {
	text string
	at   int
}

func (s *scanner) skipWhitespace() {
	for s.at < len(s.text) {
		switch s.text[s.at] {
		case ' ', '\t', '\n', '\r':
			s.at++
		default:
			return
		}
	}
}

func (s *scanner) expect(char byte, what string) error {
	if s.at >= len(s.text) || s.text[s.at] != char {
		return &scanError{"expected " + what + ", found " + describe(s.text, s.at), s.at}
	}
	s.at++
	return nil
}

func (s *scanner) starts(prefix string) bool {
	return strings.HasPrefix(s.text[s.at:], prefix)
}

func (s *scanner) scanValue(depth int) error {
	if depth > 5000 {
		return &scanError{"the document is nested too deeply to check", s.at}
	}

	s.skipWhitespace()
	if s.at >= len(s.text) {
		return &scanError{"expected a value, but the document ends here", s.at}
	}

	switch char := s.text[s.at]; {
	case char == '{':
		return s.scanObject(depth)
	case char == '[':
		return s.scanArray(depth)
	case char == '"':
		return s.scanString()
	case char == '-' || (char >= '0' && char <= '9'):
		return s.scanNumber()
	}

	if s.starts("true") {
		s.at += 4
		return nil
	}
	if s.starts("false") {
		s.at += 5
		return nil
	}
	if s.starts("null") {
		s.at += 4
		return nil
	}

	// The three mistakes that account for most hand-edited JSON.
	if s.text[s.at] == '\'' {
		return &scanError{"strings must use double quotes, not single quotes", s.at}
	}
	if s.starts("True") || s.starts("False") {
		return &scanError{"true and false must be lower case in JSON", s.at}
	}
	if s.starts("None") || s.starts("undefined") {
		return &scanError{"JSON has no None or undefined - use null", s.at}
	}

	return &scanError{"expected a value, found " + describe(s.text, s.at), s.at}
}

func (s *scanner) scanObject(depth int) error {
	s.at++ // the opening brace
	s.skipWhitespace()

	if s.at < len(s.text) && s.text[s.at] == '}' {
		s.at++
		return nil
	}

	for {
		s.skipWhitespace()
		if s.at < len(s.text) && s.text[s.at] != '"' {
			if s.text[s.at] == '\'' {
				return &scanError{"strings must use double quotes, not single quotes", s.at}
			}
			return &scanError{"expected a quoted property name, found " + describe(s.text, s.at), s.at}
		}
		if err := s.scanString(); err != nil {
			return err
		}

		s.skipWhitespace()
		if err := s.expect(':', "a colon after the property name"); err != nil {
			return err
		}

		if err := s.scanValue(depth + 1); err != nil {
			return err
		}
		s.skipWhitespace()

		if s.at >= len(s.text) {
			return &scanError{"this object is never closed", s.at}
		}
		switch s.text[s.at] {
		case ',':
			commaAt := s.at
			s.at++
			s.skipWhitespace()
			if s.at < len(s.text) && s.text[s.at] == '}' {
				return &scanError{"a trailing comma is not allowed in JSON", commaAt}
			}
		case '}':
			s.at++
			return nil
		default:
			return &scanError{"expected a comma or a closing brace, found " + describe(s.text, s.at), s.at}
		}
	}
}

func (s *scanner) scanArray(depth int) error {
	s.at++ // the opening bracket
	s.skipWhitespace()

	if s.at < len(s.text) && s.text[s.at] == ']' {
		s.at++
		return nil
	}

	for {
		if err := s.scanValue(depth + 1); err != nil {
			return err
		}
		s.skipWhitespace()

		if s.at >= len(s.text) {
			return &scanError{"this array is never closed", s.at}
		}
		switch s.text[s.at] {
		case ',':
			commaAt := s.at
			s.at++
			s.skipWhitespace()
			if s.at < len(s.text) && s.text[s.at] == ']' {
				return &scanError{"a trailing comma is not allowed in JSON", commaAt}
			}
		case ']':
			s.at++
			return nil
		default:
			return &scanError{"expected a comma or a closing bracket, found " + describe(s.text, s.at), s.at}
		}
	}
}

func (s *scanner) scanString() error {
	start := s.at
	s.at++ // the opening quote

	for s.at < len(s.text) {
		char := s.text[s.at]

		if char == '"' {
			s.at++
			return nil
		}

		if char == '\\' {
			s.at++
			if s.at >= len(s.text) {
				break
			}
			escape := s.text[s.at]
			if strings.IndexByte(`"\/bfnrt`, escape) >= 0 {
				s.at++
				continue
			}
			if escape == 'u' {
				for i := 1; i <= 4; i++ {
					if s.at+i >= len(s.text) || !isHex(s.text[s.at+i]) {
						return &scanError{`\u must be followed by four hexadecimal digits`, s.at - 1}
					}
				}
				s.at += 5
				continue
			}
			return &scanError{`"\` + string(escape) + `" is not a valid escape sequence`, s.at - 1}
		}

		if char < 0x20 {
			return &scanError{
				fmt.Sprintf("a raw control character inside a string - escape it as \\u%04x", char),
				s.at,
			}
		}

		s.at++
	}

	return &scanError{"this string is never closed", start}
}

func isHex(char byte) bool {
	return (char >= '0' && char <= '9') || (char >= 'a' && char <= 'f') || (char >= 'A' && char <= 'F')
}

func (s *scanner) scanNumber() error {
	start := s.at

	if s.text[s.at] == '-' {
		s.at++
	}

	if s.at >= len(s.text) {
		return &scanError{"this number has no digits", start}
	}

	if s.text[s.at] == '0' {
		s.at++
		if s.at < len(s.text) && isDigit(s.text[s.at]) {
			return &scanError{"a number may not have a leading zero", start}
		}
	} else {
		if !isDigit(s.text[s.at]) {
			return &scanError{"expected a digit, found " + describe(s.text, s.at), s.at}
		}
		for s.at < len(s.text) && isDigit(s.text[s.at]) {
			s.at++
		}
	}

	if s.at < len(s.text) && s.text[s.at] == '.' {
		s.at++
		digits := 0
		for s.at < len(s.text) && isDigit(s.text[s.at]) {
			s.at++
			digits++
		}
		if digits == 0 {
			return &scanError{"a decimal point must be followed by at least one digit", s.at - 1}
		}
	}

	if s.at < len(s.text) && (s.text[s.at] == 'e' || s.text[s.at] == 'E') {
		s.at++
		if s.at < len(s.text) && (s.text[s.at] == '+' || s.text[s.at] == '-') {
			s.at++
		}
		digits := 0
		for s.at < len(s.text) && isDigit(s.text[s.at]) {
			s.at++
			digits++
		}
		if digits == 0 {
			return &scanError{"an exponent must have at least one digit", s.at - 1}
		}
	}

	return nil
}

func isDigit(char byte) bool {
	return char >= '0' && char <= '9'
}
