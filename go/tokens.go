package jsoncompress

// Short tokens for catalogue indices.
//
// Two alphabets, because the two places a token appears have different rules.
//
// String references sit in value position, where nothing constrains them, so
// they use plain base-62: the first 62 entries cost one character.
//
// Object keys cannot. A JavaScript object - and every JSON parser that builds
// one - reorders keys that look like array indices to the front, in numeric
// order, so a key of "10" would jump ahead of a key of "2". Encoding a key as
// "7" would therefore quietly shuffle the document. Key tokens instead start
// with a letter and continue in base-62, which is never index-like: 52 keys
// still cost one character, the next 3,224 cost two, and the original key order
// survives the round trip exactly.

const (
	digitAlphabet  = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
	letterAlphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
)

var (
	digitValues  [256]int8
	letterValues [256]int8
)

func init() {
	for i := range digitValues {
		digitValues[i] = -1
		letterValues[i] = -1
	}
	for i := 0; i < len(digitAlphabet); i++ {
		digitValues[digitAlphabet[i]] = int8(i)
	}
	for i := 0; i < len(letterAlphabet); i++ {
		letterValues[letterAlphabet[i]] = int8(i)
	}
}

/* ------------------------------------------------------------------ *
 * Base-62, for string references
 * ------------------------------------------------------------------ */

// toBase62 writes a non-negative integer as a base-62 token.
func toBase62(n int) string {
	if n < 62 {
		return digitAlphabet[n : n+1]
	}
	var buffer [11]byte
	at := len(buffer)
	for n > 0 {
		at--
		buffer[at] = digitAlphabet[n%62]
		n /= 62
	}
	return string(buffer[at:])
}

// fromBase62 reads a base-62 token, or returns -1 if it is not one.
func fromBase62(token string) int {
	if len(token) == 0 || len(token) > 10 {
		return -1
	}
	n := 0
	for i := 0; i < len(token); i++ {
		digit := digitValues[token[i]]
		if digit < 0 {
			return -1
		}
		n = n*62 + int(digit)
		if n < 0 {
			return -1 // an absurdly long token, rather than a wrapped one
		}
	}
	return n
}

// base62Length is the length of the base-62 token for n, without building it.
func base62Length(n int) int {
	switch {
	case n < 62:
		return 1
	case n < 3844:
		return 2
	case n < 238328:
		return 3
	}
	length := 0
	for n > 0 {
		length++
		n /= 62
	}
	return length
}

/* ------------------------------------------------------------------ *
 * Letter-first tokens, for object keys
 * ------------------------------------------------------------------ */

// toKeyToken writes a non-negative integer as a key token: a letter, then
// base-62 digits.
//
// The numbering is bijective - each token length covers a contiguous block of
// indices and no index has two spellings - so fromKeyToken inverts it exactly.
func toKeyToken(n int) string {
	if n < 52 {
		return letterAlphabet[n : n+1]
	}

	// Find the block this index falls in: 52 one-character tokens, then
	// 52 * 62 two-character ones, and so on.
	start, size, length := 0, 52, 1
	for n >= start+size {
		start += size
		size *= 62
		length++
	}

	value := n - start
	out := make([]byte, length)
	for i := length - 1; i >= 1; i-- {
		out[i] = digitAlphabet[value%62]
		value /= 62
	}
	out[0] = letterAlphabet[value]
	return string(out)
}

// fromKeyToken reads a key token, or returns -1 if it is not one.
func fromKeyToken(token string) int {
	if len(token) == 0 || len(token) > 10 {
		return -1
	}

	letter := letterValues[token[0]]
	if letter < 0 {
		return -1
	}

	n := int(letter)
	for i := 1; i < len(token); i++ {
		digit := digitValues[token[i]]
		if digit < 0 {
			return -1
		}
		n = n*62 + int(digit)
		if n < 0 {
			return -1
		}
	}

	start, size := 0, 52
	for i := 1; i < len(token); i++ {
		start += size
		size *= 62
	}
	return start + n
}
