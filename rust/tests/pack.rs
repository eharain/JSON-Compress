//! The columnar transform on its own, with no envelope and no catalogue.

use json_compress::{
    is_packed, pack, pack_all, parse, text_to_array, unpack, unpack_all, Object, TextShape, Value,
};

#[test]
fn records_flatten_into_fields_and_rows() {
    let records = parse(r#"[{"id":1,"name":"Ada"},{"id":2,"name":"Bob"}]"#).unwrap();
    let table = pack(&records).unwrap();

    assert_eq!(table.fields, ["id", "name"]);
    assert_eq!(table.count, 2);
    assert_eq!(
        table.to_value().to_json(),
        r#"{"fields":["id","name"],"data":[[1,"Ada"],[2,"Bob"]],"count":2}"#
    );
    assert_eq!(Value::Array(table.unpack()), records);
}

#[test]
fn fields_are_the_union_of_every_record_in_first_seen_order() {
    let records = parse(r#"[{"b":1},{"a":2,"c":3},{"b":4,"a":5}]"#).unwrap();
    let table = pack(&records).unwrap();

    assert_eq!(table.fields, ["b", "a", "c"]);
    // A record missing a field contributes null in that column, which is the one
    // thing the plain pack gives up next to the full format.
    assert_eq!(
        table.to_value().to_json(),
        r#"{"fields":["b","a","c"],"data":[[1,null,null],[null,2,3],[4,5,null]],"count":3}"#
    );
}

#[test]
fn an_empty_record_set_packs_to_an_empty_table() {
    let table = pack(&Value::array()).unwrap();
    assert!(table.fields.is_empty());
    assert_eq!(table.count, 0);
    assert_eq!(
        table.to_value().to_json(),
        r#"{"fields":[],"data":[],"count":0}"#
    );
}

#[test]
fn anything_that_is_not_an_array_of_objects_is_refused_rather_than_mangled() {
    assert!(pack(&Value::Null).is_none());
    assert!(pack(&parse(r#"{"a":1}"#).unwrap()).is_none());
    assert!(pack(&parse("[1,2,3]").unwrap()).is_none());
    assert!(pack(&parse(r#"[{"a":1},2]"#).unwrap()).is_none());
}

#[test]
fn unpack_reads_a_table_another_implementation_wrote() {
    let packed = parse(r#"{"fields":["id","name"],"data":[[1,"Ada"],[2]],"count":2}"#).unwrap();

    assert!(is_packed(&packed));
    assert_eq!(
        Value::Array(unpack(&packed).unwrap()).to_json(),
        r#"[{"id":1,"name":"Ada"},{"id":2,"name":null}]"#
    );

    assert!(!is_packed(&Value::Null));
    assert!(!is_packed(&parse(r#"{"fields":["a"]}"#).unwrap()));
    assert!(unpack(&parse("[1]").unwrap()).is_none());
}

#[test]
fn several_result_sets_pack_and_unpack_together() {
    let sets = parse(r#"[[{"a":1},{"a":2}],[{"b":3}],"not a result set"]"#).unwrap();
    let packed = Value::Array(pack_all(&sets).unwrap());

    assert_eq!(
        packed.to_json(),
        r#"[{"fields":["a"],"data":[[1],[2]],"count":2},{"fields":["b"],"data":[[3]],"count":1},"not a result set"]"#
    );
    assert_eq!(Value::Array(unpack_all(&packed).unwrap()), sets);
}

#[test]
fn a_delimited_column_reads_back_as_numbers() {
    let mut record = Object::new().with("ids", "[[[1,2,3]]]");
    text_to_array(&mut record, "ids", &TextShape::default());
    assert_eq!(record.get("ids").unwrap().to_json(), "[1,2,3]");

    let mut record = Object::new().with("ids", "{1;2}");
    text_to_array(
        &mut record,
        "ids",
        &TextShape {
            open: 1,
            close: 1,
            separator: ";".to_owned(),
        },
    );
    assert_eq!(record.get("ids").unwrap().to_json(), "[1,2]");

    // A null column, an empty one, and a value that is not a number each give
    // something a caller can carry on with rather than an error.
    let mut record = Object::new().with("ids", Value::Null).with("other", 1);
    text_to_array(&mut record, "ids", &TextShape::default());
    text_to_array(&mut record, "missing", &TextShape::default());
    assert_eq!(record.get("ids").unwrap().to_json(), "[]");
    assert!(!record.contains_key("missing"));

    let mut record = Object::new().with("ids", "[[[a,2]]]");
    text_to_array(&mut record, "ids", &TextShape::default());
    assert_eq!(record.get("ids").unwrap().to_json(), "[null,2]");

    let mut record = Object::new().with("ids", "[[[]]]");
    text_to_array(&mut record, "ids", &TextShape::default());
    assert_eq!(record.get("ids").unwrap().to_json(), "[]");
}
