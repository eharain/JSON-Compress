//! The serde bridge: any `Serialize` type into the model, any `Deserialize`
//! type back out, and the model itself as a serde type.

#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use json_compress::{
    compress, decompress, from_str, from_value, to_string, to_value, Object, Value,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    role: Role,
    active: bool,
    score: f64,
    manager: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Admin,
    Editor,
    Viewer,
}

fn users() -> Vec<User> {
    vec![
        User {
            id: 1,
            name: "Ada".to_owned(),
            role: Role::Admin,
            active: true,
            score: 1.5,
            manager: None,
            tags: vec!["founder".to_owned()],
        },
        User {
            id: 2,
            name: "Bob".to_owned(),
            role: Role::Viewer,
            active: false,
            score: -0.25,
            manager: Some(1),
            tags: Vec::new(),
        },
    ]
}

#[test]
fn a_struct_goes_in_and_comes_back() {
    let value = to_value(&users()).unwrap();
    let small = to_string(&value).unwrap();
    let back: Vec<User> = from_value(from_str(&small).unwrap()).unwrap();

    assert_eq!(back, users());
}

#[test]
fn struct_fields_keep_the_order_they_are_declared_in() {
    let value = to_value(&users()[0]).unwrap();
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["id", "name", "role", "active", "score", "manager", "tags"]
    );

    // And the order survives the round trip, which is the whole promise.
    let restored = decompress(&compress(&value).unwrap()).unwrap();
    assert_eq!(
        restored.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["id", "name", "role", "active", "score", "manager", "tags"]
    );
}

#[test]
fn the_shapes_serde_has_that_json_does_not_are_written_the_way_serde_json_writes_them() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Newtype(u8);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Tuple(u8, &'static str);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Shape {
        Unit,
        Wrapped(u8),
        Pair(u8, u8),
        Named { width: u8 },
    }

    assert_eq!(to_value(&Newtype(7)).unwrap().to_json(), "7");
    assert_eq!(to_value(&Tuple(7, "x")).unwrap().to_json(), r#"[7,"x"]"#);
    assert_eq!(to_value(&Shape::Unit).unwrap().to_json(), r#""Unit""#);
    assert_eq!(
        to_value(&Shape::Wrapped(1)).unwrap().to_json(),
        r#"{"Wrapped":1}"#
    );
    assert_eq!(
        to_value(&Shape::Pair(1, 2)).unwrap().to_json(),
        r#"{"Pair":[1,2]}"#
    );
    assert_eq!(
        to_value(&Shape::Named { width: 3 }).unwrap().to_json(),
        r#"{"Named":{"width":3}}"#
    );

    for shape in [
        Shape::Unit,
        Shape::Wrapped(1),
        Shape::Pair(1, 2),
        Shape::Named { width: 3 },
    ] {
        let value = to_value(&shape).unwrap();
        let back: Shape = from_value(from_str(&to_string(&value).unwrap()).unwrap()).unwrap();
        assert_eq!(back, shape);
    }
}

#[test]
fn a_map_with_numeric_keys_is_written_the_way_json_has_to_write_one() {
    let mut counts: BTreeMap<u32, &str> = BTreeMap::new();
    counts.insert(2, "two");
    counts.insert(10, "ten");

    assert_eq!(
        to_value(&counts).unwrap().to_json(),
        r#"{"2":"two","10":"ten"}"#
    );

    let back: BTreeMap<u32, String> = from_value(to_value(&counts).unwrap()).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[&10], "ten");
}

#[test]
fn an_object_key_that_json_cannot_write_is_refused() {
    let mut impossible: BTreeMap<Vec<u8>, u8> = BTreeMap::new();
    impossible.insert(vec![1, 2], 3);
    assert!(to_value(&impossible).is_err());
}

/// A number read from JSON text is a double, because that is what the format
/// carries. An integer field has to be filled from one.
#[test]
fn an_integer_field_is_filled_from_the_double_the_reader_produced() {
    let text = to_string(&to_value(&users()).unwrap()).unwrap();
    let restored = from_str(&text).unwrap();

    assert!(matches!(
        restored.as_array().unwrap()[0].get("id"),
        Some(Value::Float(_))
    ));

    let back: Vec<User> = from_value(restored).unwrap();
    assert_eq!(back[0].id, 1);

    // And a number that is not a whole one is refused rather than truncated.
    #[derive(Deserialize)]
    struct Whole {
        #[allow(dead_code)]
        n: u8,
    }
    assert!(from_value::<Whole>(json_compress::parse(r#"{"n":1.5}"#).unwrap()).is_err());
    assert!(from_value::<Whole>(json_compress::parse(r#"{"n":300}"#).unwrap()).is_err());
}

#[test]
fn a_non_finite_number_becomes_null_as_json_requires() {
    assert_eq!(to_value(&f64::NAN).unwrap(), Value::Null);
    assert_eq!(to_value(&f64::INFINITY).unwrap(), Value::Null);
    assert_eq!(to_value(&-0.0f64).unwrap().to_json(), "0");
}

#[test]
fn the_model_itself_is_a_serde_type() {
    let value = json_compress::parse(r#"{"b":1,"a":[true,null,"x"]}"#).unwrap();

    // Out through serde_json, and the key order survives because this crate
    // hands its members over in order.
    assert_eq!(
        serde_json::to_string(&value).unwrap(),
        r#"{"b":1.0,"a":[true,null,"x"]}"#
    );

    // And back in.
    let back: Value = serde_json::from_str(r#"{"b":1,"a":[true,null,"x"]}"#).unwrap();
    assert_eq!(
        back.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["b", "a"]
    );
    assert_eq!(back.get("b"), Some(&Value::Uint(1)));

    let object: Object = serde_json::from_str(r#"{"a":1}"#).unwrap();
    assert_eq!(object.len(), 1);
    assert!(serde_json::from_str::<Object>("[1]").is_err());
}

#[test]
fn an_envelope_travels_as_an_ordinary_serde_value() {
    // The point of the format: the envelope is JSON, so anything that already
    // speaks serde can carry it.
    let envelope = compress(&to_value(&users()).unwrap()).unwrap();
    let over_the_wire = serde_json::to_string(&envelope).unwrap();
    let arrived: Value = serde_json::from_str(&over_the_wire).unwrap();

    let back: Vec<User> = from_value(decompress(&arrived).unwrap()).unwrap();
    assert_eq!(back, users());
}
