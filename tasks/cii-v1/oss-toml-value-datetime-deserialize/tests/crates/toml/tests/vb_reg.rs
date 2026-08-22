//! Hidden pass-to-pass guards: generic (`deserialize_any`) conversions and the
//! parser path must not change.

use serde::Deserialize;
use toml::value::Datetime;
use toml::Table;

#[test]
fn generic_json_conversion_still_string() {
    // Cross-format conversion goes through `deserialize_any`, where a TOML
    // datetime is represented as a string — that must not change.
    let table: Table = "ts = 1979-05-27T07:32:00Z".parse().unwrap();
    let json: serde_json::Value = table["ts"].clone().try_into().unwrap();
    assert_eq!(
        json,
        serde_json::Value::String("1979-05-27T07:32:00Z".to_owned())
    );
}

#[test]
fn from_str_datetime_field_works() {
    // Deserializing straight from the document (parser path) already worked.
    #[derive(Deserialize)]
    struct Doc {
        ts: Datetime,
    }
    let doc: Doc = toml::from_str("ts = 1979-05-27T07:32:00Z").unwrap();
    assert_eq!(doc.ts.to_string(), "1979-05-27T07:32:00Z");
}

#[test]
fn plain_typed_table_fields_work() {
    #[derive(Deserialize)]
    struct Doc {
        name: String,
        count: i64,
    }
    let table: Table = "name = \"x\"\ncount = 3".parse().unwrap();
    let doc: Doc = table.try_into().unwrap();
    assert_eq!(doc.name, "x");
    assert_eq!(doc.count, 3);
}
