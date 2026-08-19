//! The public surface: what each entry point promises, and what it refuses.

use json_compress::{
    beautify, compress, compress_with, decompress, decompress_if_needed, format_json, from_str,
    is_compressed, locate, measure, measure_text, parse, to_string, to_string_pretty, validate,
    Codec, Error, Object, Options, Value, FORMAT_VERSION, VERSION,
};

fn document() -> Value {
    parse(
        r#"[{"id":1,"role":"admin","country":"UK"},
            {"id":2,"role":"admin","country":"UK"},
            {"id":3,"role":"user","country":"UK"}]"#,
    )
    .unwrap()
}

#[test]
fn the_versions_are_what_they_say_they_are() {
    assert_eq!(FORMAT_VERSION, 1);
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn an_envelope_carries_the_members_the_format_defines() {
    let envelope = compress(&document()).unwrap();
    let object = envelope.as_object().unwrap();

    assert_eq!(object.get("jc"), Some(&Value::Int(1)));
    assert_eq!(
        object.get("k").and_then(Value::as_array).unwrap().len(),
        3,
        "three distinct keys"
    );
    assert!(object.contains_key("d"));
    // Nothing repeats often enough to earn a string catalogue here.
    assert!(!object.contains_key("s"));
    // The members come out in the order the format lists them.
    assert_eq!(object.keys().collect::<Vec<_>>(), ["jc", "k", "d"]);
}

#[test]
fn is_compressed_tells_an_envelope_from_a_document() {
    assert!(is_compressed(&compress(&document()).unwrap()));
    assert!(!is_compressed(&document()));
    assert!(!is_compressed(&Value::Null));
    assert!(!is_compressed(
        &parse(r#"{"jc":2,"k":[],"d":null}"#).unwrap()
    ));
    assert!(!is_compressed(&parse(r#"{"jc":1,"d":null}"#).unwrap()));
}

#[test]
fn decompress_if_needed_carries_a_plain_document_straight_through() {
    let plain = document();
    assert_eq!(decompress_if_needed(&plain).unwrap(), plain);

    let envelope = compress(&plain).unwrap();
    assert_eq!(decompress_if_needed(&envelope).unwrap(), plain);
}

#[test]
fn to_string_pretty_indents_the_envelope_and_still_reads_back() {
    let text = to_string_pretty(&document(), 2).unwrap();
    assert!(text.contains("\n  \"jc\": 1"), "{text}");
    assert_eq!(from_str(&text).unwrap(), document());
}

#[test]
fn measure_reports_against_the_minified_form() {
    let report = measure(&document()).unwrap();

    assert_eq!(report.minified, format_json(&document()).len());
    assert_eq!(report.compressed, to_string(&document()).unwrap().len());
    assert_eq!(
        report.saved,
        report.minified as i64 - report.compressed as i64
    );
    assert!((report.ratio - report.compressed as f64 / report.minified as f64).abs() < 1e-12);

    #[cfg(feature = "gzip")]
    {
        assert!(report.gzip_minified.unwrap() > 0);
        assert!(report.gzip_compressed.unwrap() > 0);
    }
    #[cfg(not(feature = "gzip"))]
    {
        assert_eq!(report.gzip_minified, None);
    }
}

#[test]
fn measure_text_reports_the_bytes_that_really_arrived() {
    let text = "{\n  \"a\": [1, 2, 3]\n}";
    let report = measure_text(text).unwrap();

    assert_eq!(report.original, text.len());
    assert!(report.minified < report.original, "whitespace was counted");
    assert!(measure_text("{oops}").is_err());
}

#[test]
fn a_codec_binds_its_options_once() {
    let options = Options {
        min_rows: 10,
        ..Options::default()
    };
    let codec = Codec::new(options).unwrap();
    assert_eq!(codec.options().min_rows, 10);

    let text = codec.to_string(&document()).unwrap();
    assert!(
        !text.contains("~t"),
        "three rows is under the codec's floor"
    );
    assert_eq!(codec.from_str(&text).unwrap(), document());
    assert_eq!(
        codec
            .decompress(&codec.compress(&document()).unwrap())
            .unwrap(),
        document()
    );
    assert!(codec.measure(&document()).unwrap().minified > 0);

    assert!(Codec::new(Options {
        min_rows: 0,
        ..Options::default()
    })
    .is_err());
}

#[test]
fn nonsense_options_are_refused_rather_than_quietly_obeyed() {
    for options in [
        Options {
            min_rows: 0,
            ..Options::default()
        },
        Options {
            max_hole_ratio: 1.5,
            ..Options::default()
        },
        Options {
            max_hole_ratio: f64::NAN,
            ..Options::default()
        },
        Options {
            min_string_count: 1,
            ..Options::default()
        },
    ] {
        assert!(options.validate().is_err(), "{options:?} should be refused");
        assert!(matches!(
            compress_with(&document(), &options),
            Err(Error::Options(_))
        ));
    }
}

#[track_caller]
fn refuses(envelope: &str, why: &str) {
    let value = parse(envelope).unwrap_or_else(|error| panic!("{why}: {error}"));
    assert!(
        matches!(decompress(&value), Err(Error::Decode(_))),
        "{why}: {envelope} should have been refused"
    );
}

#[test]
fn a_document_that_cannot_be_restored_is_refused_rather_than_half_read() {
    refuses(
        r#"{"jc":2,"k":[],"d":null}"#,
        "a version this build has never heard of",
    );
    refuses(r#"{"k":[],"d":null}"#, "no version at all");
    refuses(r#"{"jc":1,"d":null}"#, "no key catalogue");
    refuses(
        r#"{"jc":1,"k":{},"d":null}"#,
        "a key catalogue that is not an array",
    );
    refuses(
        r#"{"jc":1,"k":[1],"d":null}"#,
        "a key catalogue holding a number",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"B":1}}"#,
        "a key token past the end of the catalogue",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"0":1}}"#,
        "a key token that is not one",
    );
    refuses(
        r#"{"jc":1,"k":[],"d":"~0"}"#,
        "a string reference with no catalogue",
    );
    refuses(
        r#"{"jc":1,"k":[],"s":["a"],"d":"~5"}"#,
        "a string reference past the end",
    );
    refuses(
        r#"{"jc":1,"k":[],"d":"~"}"#,
        "a row hole marker outside a table row",
    );
    refuses(
        r#"{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~e":[0],"~r":[[1,2]]}}"#,
        "an encoding list that does not match the columns",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"~t":[7],"~r":[[1]]}}"#,
        "a column index past the end of the catalogue",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"~t":[0],"~r":[1]}}"#,
        "a row that is not an array",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"s":["x"],"d":{"~t":[0],"~e":[1],"~r":[[9]]}}"#,
        "a dictionary index past the end of the catalogue",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"~t":[0],"~e":[2],"~n":2}}"#,
        "a constant column with no constant",
    );
    refuses(
        r#"{"jc":1,"k":["a"],"d":{"~t":[0],"~e":[2],"~cv":[1],"~n":999999999}}"#,
        "a row count larger than this build will read",
    );

    assert!(matches!(decompress(&Value::Null), Err(Error::Decode(_))));
}

#[test]
fn a_trimmed_row_and_an_untrimmed_one_mean_the_same_thing() {
    // Section 6.4: encoders should trim trailing holes, decoders must accept
    // both.
    let trimmed = r#"{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~r":[[1],[2,3]]}}"#;
    let untrimmed = r#"{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~r":[[1,"~"],[2,3]]}}"#;
    assert_eq!(from_str(trimmed).unwrap(), from_str(untrimmed).unwrap());
    assert_eq!(
        format_json(&from_str(trimmed).unwrap()),
        r#"[{"a":1},{"a":2,"b":3}]"#
    );
}

#[test]
fn a_reader_ignores_envelope_members_it_does_not_know() {
    // Section 2: a later version can add a member without breaking older
    // readers on the parts they do understand.
    let envelope = r#"{"jc":1,"k":["a"],"x":"from the future","d":{"A":1}}"#;
    assert_eq!(format_json(&from_str(envelope).unwrap()), r#"{"a":1}"#);
}

#[test]
fn locate_and_validate_put_a_cursor_on_the_problem() {
    let found = locate("{\n  \"a\": 1,\n  \"b\": oops\n}");
    assert!(!found.valid);
    assert_eq!(found.error, "expected a value, found \"o\"");
    assert_eq!(found.position, 19);
    assert_eq!(found.line, 3);
    assert_eq!(found.column, 8);
    assert_eq!(found.excerpt, "  \"b\": oops");
    assert_eq!(
        found.to_string(),
        "invalid JSON at line 3, column 8: expected a value, found \"o\""
    );

    assert!(validate(r#"{"a": 1}"#).valid);
    assert_eq!(validate(r#"{"a": 1}"#).to_string(), "valid JSON");
}

#[test]
fn beautify_writes_line_feeds_whatever_the_platform() {
    let out = beautify(r#"{"a":1}"#, 2).unwrap();
    assert!(!out.contains('\r'), "{out:?}");
    assert_eq!(out, "{\n  \"a\": 1\n}");
}

#[test]
fn an_object_keeps_its_order_and_tells_absent_from_null() {
    let mut object = Object::new().with("b", 1).with("a", Value::Null);

    assert_eq!(object.keys().collect::<Vec<_>>(), ["b", "a"]);
    assert!(object.contains_key("a"));
    assert_eq!(object.get("a"), Some(&Value::Null));
    assert_eq!(object.get("missing"), None);
    assert!(!object.contains_key("missing"));

    // A repeated key keeps the position it first had, as JSON.parse does.
    object.insert("b", 2);
    assert_eq!(object.keys().collect::<Vec<_>>(), ["b", "a"]);
    assert_eq!(object.get("b"), Some(&Value::Int(2)));

    assert_eq!(object.remove("b"), Some(Value::Int(2)));
    assert_eq!(object.keys().collect::<Vec<_>>(), ["a"]);
    assert_eq!(object.remove("b"), None);

    // Past the point where an index is built, everything still holds.
    let mut large = Object::new();
    for at in 0..50 {
        large.insert(format!("key{at}"), at);
    }
    assert_eq!(large.len(), 50);
    assert_eq!(large.get("key49"), Some(&Value::Int(49)));
    assert_eq!(large.remove("key0"), Some(Value::Int(0)));
    assert_eq!(large.get("key49"), Some(&Value::Int(49)));
    assert_eq!(large.at(0).unwrap().0, "key1");
}

#[test]
fn the_error_messages_name_the_library() {
    let error = decompress(&Value::Null).unwrap_err();
    assert!(error.to_string().starts_with("json-compress: "), "{error}");

    let error = parse("{").unwrap_err();
    assert!(error.to_string().starts_with("json-compress: "), "{error}");

    // A parse failure carries the byte it gave up at, so a caller can slice with
    // it.
    match parse("[1, oops]") {
        Err(Error::Parse {
            position: Some(at), ..
        }) => assert_eq!(at, 4),
        other => panic!("expected a parse error with a position, got {other:?}"),
    }
}
