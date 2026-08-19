//! Agreement with the other implementations.
//!
//! `SPEC.md` asks only that an implementation restore what a conforming encoder
//! wrote. This asserts the stronger claim the released implementations already
//! meet: that they all write the same envelope for the same document, byte for
//! byte.
//!
//! The corpus is 144 source/envelope pairs produced by the JavaScript encoder
//! and checked in at the path below, so this runs with no other toolchain
//! installed. It is shared with the .NET, Python, PHP and Go suites rather than
//! duplicated, because two copies of a conformance corpus is one copy too many.

mod common;

use std::path::{Path, PathBuf};

use json_compress::{decompress, format_json, parse, to_string};

use common::{make_value, Random};

struct Case {
    name: String,
    source: String,
    compressed: String,
    seed: Option<u32>,
}

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dotnet")
        .join("tests")
        .join("TechStyle.JsonCompress.Tests")
        .join("fixtures")
        .join("conformance.json")
}

/// The corpus is read with `serde_json` rather than with this crate's own
/// parser, so a fault in the reader cannot quietly excuse itself.
///
/// It lives outside the published crate, in the repository the six
/// implementations share, so a checkout of the crate on its own has nothing to
/// run these against and they stand aside rather than fail.
fn load_corpus() -> Option<Vec<Case>> {
    let path = corpus_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            eprintln!(
                "the conformance corpus is not checked out at {} - skipping",
                path.display()
            );
            return None;
        }
    };
    let document: serde_json::Value =
        serde_json::from_str(&text).expect("the conformance corpus does not parse");

    Some(
        document["cases"]
            .as_array()
            .expect("the conformance corpus has no cases")
            .iter()
            .map(|case| Case {
                name: case["name"].as_str().unwrap().to_owned(),
                source: case["source"].as_str().unwrap().to_owned(),
                compressed: case["compressed"].as_str().unwrap().to_owned(),
                seed: case["seed"].as_u64().map(|seed| seed as u32),
            })
            .collect(),
    )
}

/// Every test here walks the corpus, and every one of them stands aside when it
/// is not there.
macro_rules! corpus {
    () => {
        match load_corpus() {
            Some(cases) => cases,
            None => return,
        }
    };
}

#[test]
fn the_corpus_is_the_size_it_should_be() {
    assert!(
        corpus!().len() >= 144,
        "expected at least 144 cases in the conformance corpus"
    );
}

/// Clause 3 of the specification: what a conforming encoder wrote, we read.
#[test]
fn every_envelope_the_reference_encoder_wrote_is_restored() {
    for case in corpus!() {
        let envelope = parse(&case.compressed)
            .unwrap_or_else(|error| panic!("{}: the envelope does not parse: {error}", case.name));
        let restored =
            decompress(&envelope).unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            format_json(&restored),
            case.source,
            "{} did not restore to the document it was written from",
            case.name
        );
    }
}

/// Stronger than the specification asks: the same bytes, not merely the same
/// value.
#[test]
fn every_envelope_is_written_the_same_way() {
    for case in corpus!() {
        let source = parse(&case.source)
            .unwrap_or_else(|error| panic!("{}: the source does not parse: {error}", case.name));
        let ours = to_string(&source).unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            ours, case.compressed,
            "{} was not written the way the reference encoder wrote it",
            case.name
        );
    }
}

/// The generator is shared, so a seed names the same document everywhere.
#[test]
fn the_seeded_cases_are_reproduced_from_their_seeds() {
    let mut checked = 0;
    for case in corpus!() {
        let seed = match case.seed {
            Some(seed) => seed,
            None => continue,
        };
        let value = make_value(&mut Random::new(seed), 5);
        assert_eq!(
            format_json(&value),
            case.source,
            "seed {seed} does not name the document it names elsewhere"
        );
        checked += 1;
    }
    assert!(checked > 0, "the corpus carries no seeded cases");
}
