//! Round-trip tests.
//!
//! The contract the whole library rests on is that `decompress(compress(x))`
//! gives back `x` - not something equivalent, the same document, keys in the
//! same order. Everything here checks that, on shapes chosen to poke at the
//! places where the format could plausibly lose information.

mod common;

use json_compress::{
    compress, compress_with, decompress, format_json, from_str, to_string, to_string_with, Object,
    Options, Value,
};

use common::{make_value, Random};

/// Builds an object, which reads closer to the JSON it stands for than a series
/// of `insert` calls would.
macro_rules! obj {
    ($($key:expr => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut out = Object::new();
        $( out.insert($key, $value); )*
        Value::Object(out)
    }};
}

/// Builds an array.
macro_rules! arr {
    ($($value:expr),* $(,)?) => {
        Value::Array(vec![$($value),*])
    };
}

/// Insists that a value survives compression exactly - the same values and the
/// same key order, which comparing the JSON text checks in one go.
#[track_caller]
fn round_trips(value: &Value, note: &str) {
    let envelope = compress(value).unwrap_or_else(|error| panic!("{note}: compressing: {error}"));
    let restored =
        decompress(&envelope).unwrap_or_else(|error| panic!("{note}: restoring: {error}"));
    assert_eq!(
        format_json(&restored),
        format_json(value),
        "{note} did not round-trip"
    );
}

#[track_caller]
fn restore(value: &Value) -> Vec<Value> {
    let envelope = compress(value).unwrap();
    match decompress(&envelope).unwrap() {
        Value::Array(items) => items,
        other => panic!("expected an array, got {other}"),
    }
}

#[test]
fn scalars_survive_intact() {
    for value in [
        Value::Null,
        Value::Bool(true),
        Value::Bool(false),
        Value::Float(0.0),
        Value::Float(-1.0),
        Value::Float(1.5),
        Value::Float(1e21),
        Value::Float(-1e-7),
        Value::Int(i64::MIN),
        Value::Uint(u64::MAX),
        Value::String(String::new()),
        Value::String("plain".to_owned()),
    ] {
        let note = format_json(&value);
        round_trips(&value, &note);
    }
}

#[test]
fn the_tilde_escape_is_reversible() {
    for value in ["~", "~~", "~~~", "~0", "~z", "~ref", "~t", "~n", "a~b"] {
        round_trips(&Value::from(value), value);
    }
    round_trips(
        &arr![
            "~".into(),
            "~".into(),
            "~".into(),
            "~0".into(),
            "~0".into(),
            "~0".into()
        ],
        "repeated tilde strings",
    );
    round_trips(&obj! { "~key" => "~value" }, "tilde in both key and value");
}

#[test]
fn empty_containers_survive() {
    round_trips(&Value::array(), "empty array");
    round_trips(&Value::object(), "empty object");
    round_trips(
        &arr![
            Value::array(),
            Value::object(),
            arr![Value::array()],
            arr![Value::object()]
        ],
        "nested empties",
    );
    round_trips(
        &arr![Value::object(), Value::object(), Value::object()],
        "array of empty objects",
    );
}

#[test]
fn arrays_of_like_shaped_objects_become_tables_and_come_back() {
    let rows = arr![
        obj! { "id" => 1.0, "name" => "Ada", "role" => "admin" },
        obj! { "id" => 2.0, "name" => "Bob", "role" => "admin" },
        obj! { "id" => 3.0, "name" => "Cy", "role" => "user" },
    ];
    assert!(
        to_string(&rows).unwrap().contains("~t"),
        "expected uniform rows to be packed as a table"
    );
    round_trips(&rows, "uniform rows");
}

#[test]
fn rows_with_different_key_sets_survive() {
    round_trips(
        &arr![
            obj! { "a" => 1.0 },
            obj! { "b" => 2.0 },
            obj! { "a" => 3.0, "b" => 4.0 }
        ],
        "disjoint keys",
    );
    round_trips(
        &arr![
            obj! { "a" => 1.0, "b" => 2.0, "c" => 3.0 },
            obj! { "a" => 4.0 },
            obj! { "a" => 5.0, "c" => 6.0 }
        ],
        "ragged rows",
    );
}

#[test]
fn rows_whose_key_orders_disagree_are_left_as_objects() {
    let rows = arr![
        obj! { "a" => 1.0, "b" => 2.0 },
        obj! { "b" => 3.0, "a" => 4.0 },
    ];
    assert!(
        !to_string(&rows).unwrap().contains("~t"),
        "no column order can satisfy both rows, so no table should be built"
    );
    round_trips(&rows, "rows whose key orders disagree");
}

#[test]
fn a_missing_key_is_not_the_same_as_a_null_one() {
    let value = arr![
        obj! { "a" => 1.0, "b" => Value::Null },
        obj! { "a" => 2.0 },
        obj! { "a" => 3.0, "b" => Value::Null },
        obj! { "a" => 4.0, "b" => 5.0 },
    ];
    let restored = restore(&value);

    let first = restored[0].as_object().unwrap();
    assert!(first.contains_key("b"), "an explicit null should be kept");
    assert_eq!(first.get("b"), Some(&Value::Null));
    assert!(
        !restored[1].as_object().unwrap().contains_key("b"),
        "an absent key should stay absent"
    );
    round_trips(&value, "nulls against holes");
}

#[test]
fn a_missing_key_is_not_the_same_as_a_null_one_in_a_dictionary_column() {
    let value = arr![
        obj! { "k" => "x", "n" => 1.0 },
        obj! { "k" => Value::Null, "n" => 2.0 },
        obj! { "n" => 3.0 },
        obj! { "k" => "x", "n" => 4.0 },
        obj! { "k" => "y", "n" => 5.0 },
        obj! { "k" => "x", "n" => 6.0 },
    ];
    let restored = restore(&value);

    assert_eq!(
        restored[1].as_object().unwrap().get("k"),
        Some(&Value::Null),
        "an explicit null should be kept"
    );
    assert!(
        !restored[2].as_object().unwrap().contains_key("k"),
        "an absent key should stay absent"
    );
    round_trips(&value, "nulls against holes in a dictionary column");
}

/// `-1` is what a dictionary column writes for an absent key. A raw column that
/// happens to hold that number must not be trimmed away as though the key were
/// missing - the two look alike on the wire and are not the same thing.
#[test]
fn a_raw_column_may_hold_the_number_a_dictionary_column_uses_for_a_hole() {
    round_trips(
        &arr![
            obj! { "a" => 1.0, "b" => -1.0 },
            obj! { "a" => 2.0, "b" => 5.0 }
        ],
        "minus one in a trailing raw cell",
    );
    round_trips(
        &arr![
            obj! { "a" => 1.0, "b" => -1.0 },
            obj! { "a" => 2.0, "b" => -1.0 },
            obj! { "a" => 3.0, "b" => 7.0 }
        ],
        "minus one on several rows",
    );
    round_trips(
        &arr![
            obj! { "a" => -1.0 },
            obj! { "a" => -1.0 },
            obj! { "a" => 2.0 }
        ],
        "minus one in the only column",
    );
    round_trips(
        &arr![obj! { "a" => 1.0 }, obj! { "a" => 2.0, "b" => -1.0 }],
        "a real hole beside a real minus one",
    );

    let value = arr![
        obj! { "a" => 1.0, "b" => -1.0 },
        obj! { "a" => 2.0, "b" => 5.0 }
    ];
    assert_eq!(
        to_string(&value).unwrap(),
        r#"{"jc":1,"k":["a","b"],"d":{"~t":[0,1],"~r":[[1,-1],[2,5]]}}"#,
        "the cell should be written, not trimmed"
    );
}

#[test]
fn constant_columns_are_restored_on_every_row() {
    let value = arr![
        obj! { "type" => "order", "id" => 1.0 },
        obj! { "type" => "order", "id" => 2.0 },
        obj! { "type" => "order", "id" => 3.0 },
    ];
    let envelope = to_string(&value).unwrap();
    assert!(envelope.contains("~cv"), "expected a constant column");
    assert_eq!(
        envelope.matches("order").count(),
        1,
        "the constant should be written once"
    );
    round_trips(&value, "constant columns");
}

#[test]
fn a_table_where_every_column_is_constant_keeps_its_row_count() {
    let row = || obj! { "a" => 1.0, "b" => "x" };
    let value = arr![row(), row(), row(), row()];
    assert!(
        to_string(&value).unwrap().contains("~n"),
        "expected a row count"
    );
    round_trips(&value, "every column constant");
}

#[test]
fn nested_tables_survive() {
    round_trips(
        &obj! {
            "groups" => arr![
                obj! { "name" => "a", "items" => arr![obj! { "q" => 1.0, "p" => 2.0 }, obj! { "q" => 3.0, "p" => 4.0 }] },
                obj! { "name" => "b", "items" => arr![obj! { "q" => 5.0, "p" => 6.0 }, obj! { "q" => 7.0, "p" => 8.0 }] },
            ]
        },
        "tables inside tables",
    );
}

#[test]
fn arrays_of_arrays_are_left_alone() {
    round_trips(
        &arr![
            arr![1.0.into(), 2.0.into()],
            arr![3.0.into(), 4.0.into()],
            arr![5.0.into(), 6.0.into()]
        ],
        "matrix",
    );
    round_trips(
        &arr![
            arr![obj! { "a" => 1.0 }, obj! { "a" => 2.0 }],
            arr![obj! { "a" => 3.0 }, obj! { "a" => 4.0 }]
        ],
        "arrays of tables",
    );
}

#[test]
fn mixed_arrays_are_left_alone() {
    round_trips(
        &arr![
            1.0.into(),
            "two".into(),
            Value::Null,
            obj! { "three" => 3.0 },
            arr![4.0.into()]
        ],
        "mixed",
    );
    round_trips(
        &arr![obj! { "a" => 1.0 }, 2.0.into(), obj! { "a" => 3.0 }],
        "objects interrupted by a scalar",
    );
}

#[test]
fn deep_nesting_survives() {
    let mut value = obj! { "leaf" => true };
    for level in 0..200 {
        value = obj! { format!("level{level}") => value };
    }
    round_trips(&value, "200 levels");
}

#[test]
fn a_document_nested_past_the_limit_is_refused_rather_than_walked() {
    let mut value = Value::Bool(true);
    for _ in 0..json_compress::MAX_COMPRESS_DEPTH + 2 {
        value = Value::Array(vec![value]);
    }
    assert!(compress(&value).is_err());
}

#[test]
fn repeated_strings_are_catalogued_and_restored() {
    let value = Value::Array(
        (0..50)
            .map(|at| {
                let country = if at % 2 == 1 {
                    "United Kingdom"
                } else {
                    "Republic of Ireland"
                };
                obj! {
                    "id" => at as f64,
                    "country" => country,
                    "note" => "a reasonably long repeated note value",
                }
            })
            .collect::<Vec<_>>(),
    );

    let envelope = compress(&value).unwrap();
    let catalogue = envelope.get("s").and_then(Value::as_array).unwrap();
    assert!(
        catalogue.contains(&Value::String("United Kingdom".to_owned())),
        "the repeated country should have been catalogued"
    );
    round_trips(&value, "50 records with repeated strings");

    // The note never changes, so it is hoisted out of the rows entirely and
    // written down exactly once.
    assert_eq!(
        to_string(&value)
            .unwrap()
            .matches("a reasonably long repeated note value")
            .count(),
        1
    );
}

#[test]
fn unicode_and_escapes_survive() {
    round_trips(
        &obj! {
            "emoji" => "compressed 📦 nicely",
            "quote" => "she said \"hello\"",
            "backslash" => "C:\\Users\\path",
            "newline" => "one\ntwo\tthree",
            "surrogate" => "😀😁",
            "arabic" => "مرحبا بالعالم",
            "nul" => "a\u{0}b",
        },
        "awkward text",
    );
}

#[test]
fn keys_that_look_like_tokens_are_not_confused_with_tokens() {
    round_trips(
        &obj! {
            "0" => 1.0, "1" => 2.0, "a" => 3.0, "z" => 4.0,
            "10" => 5.0, "~t" => 6.0, "~r" => 7.0,
        },
        "token-shaped keys",
    );
}

#[test]
fn a_key_named_proto_is_data() {
    round_trips(
        &obj! { "__proto__" => obj! { "polluted" => true }, "constructor" => 1.0 },
        "__proto__ as a key",
    );
    round_trips(
        &arr![
            obj! { "__proto__" => 1.0, "a" => 2.0 },
            obj! { "__proto__" => 3.0, "a" => 4.0 }
        ],
        "__proto__ as a column",
    );
}

#[test]
fn a_large_key_set_pushes_tokens_past_one_character() {
    let mut value = Object::new();
    for at in 0..5000 {
        value.insert(format!("key_number_{at}"), at as f64);
    }
    round_trips(&Value::Object(value), "5000 keys");
}

#[test]
fn a_large_string_catalogue_pushes_references_past_one_character() {
    let mut value: Vec<Value> = (0..500)
        .map(|at| Value::String(format!("a distinctive value number {at}")))
        .collect();
    value.extend(value.clone());
    round_trips(&Value::Array(value), "500 repeated strings");
}

#[test]
fn to_string_and_from_str_agree_with_compress_and_decompress() {
    let value = obj! {
        "users" => arr![obj! { "id" => 1.0, "name" => "Ada" }, obj! { "id" => 2.0, "name" => "Bob" }]
    };

    let text = to_string(&value).unwrap();
    assert_eq!(format_json(&from_str(&text).unwrap()), format_json(&value));
    assert_eq!(format_json(&compress(&value).unwrap()), text);
}

fn every_combination() -> Vec<Options> {
    let mut out = Vec::new();
    for strings in [true, false] {
        for tables in [true, false] {
            for constants in [true, false] {
                for dict_columns in [true, false] {
                    out.push(Options {
                        strings,
                        tables,
                        constants,
                        dict_columns,
                        ..Options::default()
                    });
                }
            }
        }
    }
    out
}

#[test]
fn every_combination_of_options_round_trips() {
    let rows: Vec<Value> = (0..20)
        .map(|at| {
            let mut row = Object::new()
                .with("id", at as f64)
                .with("status", if at % 2 == 1 { "open" } else { "closed" })
                .with("fixed", "always the same");
            if at % 5 != 0 {
                row.insert("note", format!("note {}", at % 3));
            }
            Value::Object(row)
        })
        .collect();
    let value = obj! { "rows" => Value::Array(rows) };
    let expected = format_json(&value);

    for options in every_combination() {
        let envelope = compress_with(&value, &options).unwrap();
        let restored = decompress(&envelope).unwrap();
        assert_eq!(
            format_json(&restored),
            expected,
            "{options:?} did not round-trip"
        );
    }
}

#[test]
fn min_rows_controls_when_a_table_is_built() {
    let two = arr![obj! { "a" => 1.0 }, obj! { "a" => 2.0 }];

    let packing = Options {
        min_rows: 2,
        ..Options::default()
    };
    assert!(to_string_with(&two, &packing).unwrap().contains("~t"));

    let leaving = Options {
        min_rows: 3,
        ..Options::default()
    };
    assert!(!to_string_with(&two, &leaving).unwrap().contains("~t"));

    let envelope = compress_with(&two, &leaving).unwrap();
    assert_eq!(
        format_json(&decompress(&envelope).unwrap()),
        format_json(&two)
    );
}

#[test]
fn max_hole_ratio_keeps_very_sparse_arrays_out_of_tables() {
    let sparse = arr![
        obj! { "a" => 1.0 },
        obj! { "b" => 2.0 },
        obj! { "c" => 3.0 },
        obj! { "d" => 4.0 },
        obj! { "e" => 5.0 },
    ];
    assert!(
        !to_string(&sparse).unwrap().contains("~t"),
        "a mostly empty table should not be built"
    );
    round_trips(&sparse, "sparse rows");

    let packing = Options {
        max_hole_ratio: 1.0,
        ..Options::default()
    };
    let envelope = compress_with(&sparse, &packing).unwrap();
    assert_eq!(
        format_json(&decompress(&envelope).unwrap()),
        format_json(&sparse)
    );
}

/// The shared seeded generator, run over two thousand documents. A failure names
/// the seed, and that seed builds the same document in every other
/// implementation.
#[test]
fn generated_documents_survive_the_round_trip() {
    for seed in 0..2000u32 {
        let value = make_value(&mut Random::new(seed), 5);
        let envelope = compress(&value).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        let restored = decompress(&envelope).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(
            format_json(&restored),
            format_json(&value),
            "seed {seed} did not round-trip"
        );
    }
}

/// The same documents through the text-level pair, which also exercises the
/// reader and the writer.
#[test]
fn generated_documents_survive_the_round_trip_as_text() {
    for seed in 0..2000u32 {
        let value = make_value(&mut Random::new(seed), 5);
        let expected = format_json(&value);
        let text = to_string(&value).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        let restored = from_str(&text).unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(format_json(&restored), expected, "seed {seed}");
    }
}
