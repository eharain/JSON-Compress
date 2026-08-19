//! What happens when the input is not what it should be.
//!
//! The reader and the decoder both face bytes they did not write, so neither may
//! panic, hang or hand back something nearly right. These throw truncated,
//! mutated and outright random documents at both and insist on the only two
//! outcomes either is allowed to have: a value, or an error saying why not.

mod common;

use json_compress::{decompress, format_json, is_compressed, parse, to_string, Value};

use common::{make_value, Random};

/// A character set with every shape that means something to the reader in it,
/// so a random string is a plausible broken document rather than noise.
const ALPHABET: [char; 34] = [
    '{', '}', '[', ']', '"', ':', ',', '\\', '/', '~', '-', '+', '.', 'e', 'E', '0', '1', '9', 'a',
    'z', 'n', 't', 'f', 'u', ' ', '\t', '\n', '\r', '\u{0}', '\u{7f}', '\u{80}', '\u{2028}', '😀',
    'é',
];

fn random_text(random: &mut Random, length: usize) -> String {
    (0..length)
        .map(|_| ALPHABET[(random.next() * ALPHABET.len() as f64) as usize])
        .collect()
}

/// Whatever the reader accepts, it has to accept its own writing of it, and read
/// the same thing out again. That is the property a broken reader breaks.
#[track_caller]
fn reads_or_refuses(text: &str) {
    let value = match parse(text) {
        Ok(value) => value,
        Err(_) => return,
    };
    let written = format_json(&value);
    let again = parse(&written)
        .unwrap_or_else(|error| panic!("we wrote {written} and could not read it back: {error}"));
    assert_eq!(
        format_json(&again),
        written,
        "reading our own writing of {text} changed it"
    );
}

#[test]
fn random_text_is_read_or_refused_and_never_anything_else() {
    let mut random = Random::new(1);
    for length in 1..40 {
        for _ in 0..200 {
            reads_or_refuses(&random_text(&mut random, length));
        }
    }
}

#[test]
fn every_prefix_of_a_real_document_is_read_or_refused() {
    for seed in 0..100u32 {
        let text = to_string(&make_value(&mut Random::new(seed), 4)).unwrap();
        for end in 0..text.len() {
            if !text.is_char_boundary(end) {
                continue;
            }
            reads_or_refuses(&text[..end]);
        }
    }
}

/// A mutated envelope is a corrupt one. The decoder may restore it - a flipped
/// digit is often still a readable document - but it may not panic, and it may
/// not return something it could not have been given.
#[test]
fn a_mutated_envelope_is_restored_or_refused_and_never_anything_else() {
    let mut random = Random::new(7);

    for seed in 0..200u32 {
        let text = to_string(&make_value(&mut Random::new(seed), 4)).unwrap();
        let characters: Vec<char> = text.chars().collect();

        for _ in 0..20 {
            let at = (random.next() * characters.len() as f64) as usize;
            if at >= characters.len() {
                continue;
            }

            let mut mutated = characters.clone();
            mutated[at] = ALPHABET[(random.next() * ALPHABET.len() as f64) as usize];
            let mutated: String = mutated.into_iter().collect();

            let envelope = match parse(&mutated) {
                Ok(envelope) => envelope,
                Err(_) => continue,
            };
            match decompress(&envelope) {
                Ok(restored) => {
                    // Whatever came back has to be a document in its own right.
                    let written = format_json(&restored);
                    assert!(
                        parse(&written).is_ok(),
                        "restoring {mutated} gave something that is not JSON: {written}"
                    );
                }
                Err(error) => {
                    let message = error.to_string();
                    assert!(message.starts_with("json-compress: "), "{message}");
                }
            }
        }
    }
}

#[test]
fn a_document_that_is_not_an_envelope_is_refused_rather_than_guessed_at() {
    for seed in 0..200u32 {
        let value = make_value(&mut Random::new(seed), 4);
        if is_compressed(&value) {
            continue; // vanishingly unlikely, but the generator can write anything
        }
        assert!(
            decompress(&value).is_err(),
            "{} is not an envelope and should have been refused",
            format_json(&value)
        );
    }
}

#[test]
fn the_decoder_survives_the_shapes_a_hand_written_envelope_can_take() {
    for data in [
        "null",
        "0",
        "[]",
        "{}",
        r#"{"~t":[]}"#,
        r#"{"~t":[0],"~r":[]}"#,
        r#"{"~t":[0],"~r":[[]]}"#,
        r#"{"~t":[0],"~n":0}"#,
        r#"{"~t":[0],"~cv":[]}"#,
        r#"{"~t":[0],"~e":[0],"~r":[[null]]}"#,
        r#"{"~t":[0],"~e":[9],"~r":[[1]]}"#,
        r#"[{"~t":[0],"~r":[[1]]}]"#,
    ] {
        let envelope = parse(&format!(r#"{{"jc":1,"k":["a"],"d":{data}}}"#)).unwrap();
        match decompress(&envelope) {
            Ok(restored) => assert!(parse(&format_json(&restored)).is_ok()),
            Err(error) => assert!(error.to_string().starts_with("json-compress: ")),
        }
    }
}

#[test]
fn a_catalogue_index_is_bounded_however_it_is_written() {
    // The three places an index reaches into a catalogue, each given something
    // outside it.
    for envelope in [
        r#"{"jc":1,"k":["a"],"d":{"B":1}}"#,
        r#"{"jc":1,"k":["a"],"d":{"~t":[1],"~r":[[1]]}}"#,
        r#"{"jc":1,"k":["a"],"s":[],"d":{"~t":[0],"~e":[1],"~r":[[0]]}}"#,
        r#"{"jc":1,"k":["a"],"s":[],"d":"~0"}"#,
        r#"{"jc":1,"k":["a"],"d":{"~t":[-1],"~r":[[1]]}}"#,
        r#"{"jc":1,"k":["a"],"s":["x"],"d":{"~t":[0],"~e":[1],"~r":[[-2]]}}"#,
    ] {
        let value = parse(envelope).unwrap();
        assert!(
            decompress(&value).is_err(),
            "{envelope} should have been refused"
        );
    }
}

#[test]
fn a_deeply_nested_envelope_is_refused_rather_than_walked_off_the_stack() {
    let deep = format!(
        r#"{{"jc":1,"k":["a"],"d":{}{}}}"#,
        "[".repeat(4000),
        "]".repeat(4000)
    );
    match parse(&deep) {
        Ok(value) => assert!(decompress(&value).is_err()),
        Err(error) => assert!(error.to_string().contains("nested too deeply"), "{error}"),
    }
    assert_eq!(
        format_json(&Value::Null),
        "null",
        "and the process is still here to say so"
    );
}
