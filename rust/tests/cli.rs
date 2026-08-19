//! The command line, driven the way a shell drives it.

use std::io::Write;
use std::process::{Command, Stdio};

const BINARY: &str = env!("CARGO_BIN_EXE_json-compress");

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn run(arguments: &[&str], input: &str) -> Run {
    let mut child = Command::new(BINARY)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the command line should be built before this test runs");

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let finished = child.wait_with_output().unwrap();
    Run {
        code: finished.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&finished.stdout)
            .trim_end()
            .to_owned(),
        err: String::from_utf8_lossy(&finished.stderr)
            .trim_end()
            .to_owned(),
    }
}

const RECORDS: &str = r#"[{"id":1,"role":"admin"},{"id":2,"role":"admin"}]"#;
const ENVELOPE: &str =
    r#"{"jc":1,"k":["id","role"],"d":{"~t":[0,1],"~e":[0,2],"~cv":["admin"],"~r":[[1],[2]]}}"#;

#[test]
fn compress_and_decompress_are_a_round_trip_through_the_shell() {
    let compressed = run(&["compress"], RECORDS);
    assert_eq!(compressed.code, 0, "{}", compressed.err);
    assert_eq!(compressed.out, ENVELOPE);

    let restored = run(&["decompress"], &compressed.out);
    assert_eq!(restored.code, 0, "{}", restored.err);
    assert_eq!(restored.out, RECORDS);
}

#[test]
fn pretty_indents_the_output() {
    let pretty = run(&["compress", "--pretty"], RECORDS);
    assert_eq!(pretty.code, 0);
    assert!(pretty.out.starts_with("{\n  \"jc\": 1,"), "{}", pretty.out);

    let wider = run(&["compress", "-p", "-i", "4"], RECORDS);
    assert!(wider.out.starts_with("{\n    \"jc\": 1,"), "{}", wider.out);
}

#[test]
fn the_encoder_switches_are_honoured() {
    let plain = run(&["compress", "--no-tables"], RECORDS);
    assert_eq!(plain.code, 0);
    assert!(!plain.out.contains("~t"), "{}", plain.out);

    let floor = run(&["compress", "--min-rows", "3"], RECORDS);
    assert!(!floor.out.contains("~t"), "{}", floor.out);

    let broken = run(&["compress", "--min-rows", "x"], RECORDS);
    assert_eq!(broken.code, 1);
    assert!(broken.err.contains("must be a number"), "{}", broken.err);
}

#[test]
fn minify_and_beautify_touch_nothing_but_whitespace() {
    let minified = run(&["minify"], "{\n  \"a\" : [ 1, 2 ]\n}");
    assert_eq!(minified.out, r#"{"a":[1,2]}"#);

    let beautified = run(&["beautify"], r#"{"a":[1]}"#);
    assert_eq!(beautified.out, "{\n  \"a\": [\n    1\n  ]\n}");
}

#[test]
fn validate_says_where_a_document_breaks() {
    let good = run(&["validate"], r#"{"a":1}"#);
    assert_eq!(good.code, 0);
    assert_eq!(good.out, "valid JSON");

    let bad = run(&["validate"], "{\"a\":1,}");
    assert_eq!(bad.code, 1);
    assert!(bad.err.contains("line 1, column 7"), "{}", bad.err);
    assert!(bad.err.contains("trailing comma"), "{}", bad.err);
}

#[test]
fn stats_changes_nothing_and_reports_what_it_would_save() {
    let stats = run(&["stats"], RECORDS);
    assert_eq!(stats.code, 0);
    assert!(stats.out.starts_with("stdin"), "{}", stats.out);
    assert!(stats.out.contains("minified"), "{}", stats.out);
    assert!(stats.out.contains("compressed"), "{}", stats.out);
}

#[test]
fn the_usual_mistakes_are_answered_rather_than_crashed_on() {
    let unknown = run(&["explode"], RECORDS);
    assert_eq!(unknown.code, 1);
    assert!(unknown.err.contains("unknown command"), "{}", unknown.err);

    let option = run(&["compress", "--nope"], RECORDS);
    assert_eq!(option.code, 1);
    assert!(option.err.contains("unknown option"), "{}", option.err);

    let broken = run(&["compress"], "{oops}");
    assert_eq!(broken.code, 1);
    assert!(broken.err.contains("not valid JSON"), "{}", broken.err);

    let plain = run(&["decompress"], RECORDS);
    assert_eq!(plain.code, 1);
    assert!(
        plain.err.contains("not a compressed document"),
        "{}",
        plain.err
    );
}

#[test]
fn help_and_version_answer_without_reading_anything() {
    let help = run(&["--help"], "");
    assert_eq!(help.code, 0);
    assert!(help.out.contains("Usage"), "{}", help.out);

    let version = run(&["--version"], "");
    assert_eq!(version.code, 0);
    assert_eq!(version.out, env!("CARGO_PKG_VERSION"));

    let nothing = run(&[], "");
    assert_eq!(nothing.code, 0);
    assert!(nothing.out.contains("Usage"), "{}", nothing.out);
}

#[test]
fn a_byte_order_mark_is_dropped_rather_than_refused() {
    let marked = run(&["minify"], "\u{feff}{\"a\":1}");
    assert_eq!(marked.code, 0, "{}", marked.err);
    assert_eq!(marked.out, r#"{"a":1}"#);
}

#[test]
fn a_file_can_stand_in_for_stdin_and_for_stdout() {
    let directory = std::env::temp_dir().join(format!("json-compress-cli-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("records.json");
    let target = directory.join("records.jc.json");
    std::fs::write(&source, RECORDS).unwrap();

    let compressed = run(
        &[
            "compress",
            source.to_str().unwrap(),
            "-o",
            target.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(compressed.code, 0, "{}", compressed.err);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), ENVELOPE);

    let named = run(&["stats", source.to_str().unwrap()], "");
    assert!(named.out.starts_with("records.json"), "{}", named.out);

    let missing = run(&["compress", "no-such-file.json"], "");
    assert_eq!(missing.code, 1);
    assert!(missing.err.contains("cannot read"), "{}", missing.err);

    std::fs::remove_dir_all(&directory).ok();
}
