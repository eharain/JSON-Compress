//! Short tokens for catalogue indices.
//!
//! Two alphabets, because the two places a token appears have different rules.
//!
//! String references sit in value position, where nothing constrains them, so
//! they use plain base-62: the first 62 entries cost one character.
//!
//! Object keys cannot. A JavaScript object - and every JSON parser that builds
//! one - reorders keys that look like array indices to the front, in numeric
//! order, so a key of `"10"` would jump ahead of a key of `"2"`. Encoding a key
//! as `"7"` would therefore quietly shuffle the document. Key tokens instead
//! start with a letter and continue in base-62, which is never index-like: 52
//! keys still cost one character, the next 3,224 cost two, and the original key
//! order survives the round trip exactly.

const DIGIT_ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const LETTER_ALPHABET: &[u8; 52] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// A token longer than this is not one this build wrote, and folding it in
/// would only risk an overflow.
const MAX_TOKEN_LENGTH: usize = 10;

const fn value_table(alphabet: &[u8], length: usize) -> [i8; 256] {
    let mut out = [-1i8; 256];
    let mut at = 0;
    while at < length {
        out[alphabet[at] as usize] = at as i8;
        at += 1;
    }
    out
}

static DIGIT_VALUES: [i8; 256] = value_table(DIGIT_ALPHABET, 62);
static LETTER_VALUES: [i8; 256] = value_table(LETTER_ALPHABET, 52);

/* ------------------------------------------------------------------ *
 * Base-62, for string references
 * ------------------------------------------------------------------ */

/// Writes an index as a base-62 token.
pub(crate) fn to_base62(mut index: usize) -> String {
    if index < 62 {
        return (DIGIT_ALPHABET[index] as char).to_string();
    }

    let mut buffer = [0u8; 16];
    let mut at = buffer.len();
    while index > 0 {
        at -= 1;
        buffer[at] = DIGIT_ALPHABET[index % 62];
        index /= 62;
    }
    String::from_utf8_lossy(&buffer[at..]).into_owned()
}

/// Reads a base-62 token, or `None` if it is not one.
pub(crate) fn from_base62(token: &str) -> Option<usize> {
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_TOKEN_LENGTH {
        return None;
    }

    let mut index: usize = 0;
    for byte in bytes {
        let digit = DIGIT_VALUES[*byte as usize];
        if digit < 0 {
            return None;
        }
        index = index.checked_mul(62)?.checked_add(digit as usize)?;
    }
    Some(index)
}

/// The length of the base-62 token for an index, without building it.
pub(crate) fn base62_length(index: usize) -> usize {
    match index {
        0..=61 => 1,
        62..=3843 => 2,
        3844..=238327 => 3,
        _ => {
            let mut length = 0;
            let mut left = index;
            while left > 0 {
                length += 1;
                left /= 62;
            }
            length
        }
    }
}

/* ------------------------------------------------------------------ *
 * Letter-first tokens, for object keys
 * ------------------------------------------------------------------ */

/// Writes an index as a key token: a letter, then base-62 digits.
///
/// The numbering is bijective - each token length covers a contiguous block of
/// indices and no index has two spellings - so [`from_key_token`] inverts it
/// exactly.
pub(crate) fn to_key_token(index: usize) -> String {
    if index < 52 {
        return (LETTER_ALPHABET[index] as char).to_string();
    }

    // Find the block this index falls in: 52 one-character tokens, then
    // 52 * 62 two-character ones, and so on.
    let (mut start, mut size, mut length) = (0usize, 52usize, 1usize);
    while index >= start + size {
        start += size;
        size *= 62;
        length += 1;
    }

    let mut value = index - start;
    let mut out = vec![0u8; length];
    for at in (1..length).rev() {
        out[at] = DIGIT_ALPHABET[value % 62];
        value /= 62;
    }
    out[0] = LETTER_ALPHABET[value];
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads a key token, or `None` if it is not one.
pub(crate) fn from_key_token(token: &str) -> Option<usize> {
    let bytes = token.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_TOKEN_LENGTH {
        return None;
    }

    let letter = LETTER_VALUES[bytes[0] as usize];
    if letter < 0 {
        return None;
    }

    let mut index = letter as usize;
    for byte in &bytes[1..] {
        let digit = DIGIT_VALUES[*byte as usize];
        if digit < 0 {
            return None;
        }
        index = index.checked_mul(62)?.checked_add(digit as usize)?;
    }

    let (mut start, mut size) = (0usize, 52usize);
    for _ in 1..bytes.len() {
        start += size;
        size = size.checked_mul(62)?;
    }
    start.checked_add(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base62_round_trips() {
        for index in [0usize, 1, 61, 62, 63, 3843, 3844, 238327, 1_000_000] {
            let token = to_base62(index);
            assert_eq!(from_base62(&token), Some(index), "token {token}");
            assert_eq!(base62_length(index), token.len(), "token {token}");
        }
        assert_eq!(to_base62(0), "0");
        assert_eq!(to_base62(61), "z");
        assert_eq!(to_base62(62), "10");
    }

    #[test]
    fn key_tokens_round_trip_and_never_look_like_numbers() {
        for index in 0..5000usize {
            let token = to_key_token(index);
            assert_eq!(from_key_token(&token), Some(index), "token {token}");
            assert!(token.as_bytes()[0].is_ascii_alphabetic(), "token {token}");
        }
        assert_eq!(to_key_token(0), "A");
        assert_eq!(to_key_token(51), "z");
        assert_eq!(to_key_token(52), "A0");
        assert_eq!(to_key_token(3275), "zz");
        assert_eq!(to_key_token(3276), "A00");
    }

    #[test]
    fn nonsense_is_refused_rather_than_folded() {
        assert_eq!(from_key_token(""), None);
        assert_eq!(from_key_token("0"), None);
        assert_eq!(from_key_token("A-"), None);
        assert_eq!(from_key_token("AAAAAAAAAAAAAAAA"), None);
        assert_eq!(from_base62("~"), None);
        assert_eq!(from_base62(""), None);
    }
}
