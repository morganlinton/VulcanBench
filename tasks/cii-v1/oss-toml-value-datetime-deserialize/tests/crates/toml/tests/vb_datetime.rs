//! Hidden fail-to-pass tests: typed deserialization of TOML datetimes from
//! `toml::Value` / `toml::Table`. Public API only; no error-text assertions.

use serde::Deserialize;
use toml::value::Datetime;
use toml::{Table, Value};

#[test]
fn datetime_from_value_typed() {
    let table: Table = "ts = 1979-05-27T07:32:00Z".parse().unwrap();
    let value = table["ts"].clone();
    let datetime: Datetime = value.try_into().unwrap();
    assert_eq!(datetime.to_string(), "1979-05-27T07:32:00Z");
}

#[test]
fn datetime_variants_from_value_typed() {
    // Offset datetime, local datetime, local date, local time.
    for input in [
        "1979-05-27T00:32:00-07:00",
        "1979-05-27T07:32:00",
        "1979-05-27",
        "07:32:00",
    ] {
        let table: Table = format!("ts = {input}").parse().unwrap();
        let datetime: Datetime = table["ts"].clone().try_into().unwrap();
        assert_eq!(datetime.to_string(), input);
    }
}

#[test]
fn datetime_struct_field_from_table() {
    // The `Table` deserializer must take the same path as `Value`.
    #[derive(Deserialize)]
    struct Doc {
        ts: Datetime,
        name: String,
    }
    let table: Table = "ts = 1979-05-27T07:32:00Z\nname = \"canonical\""
        .parse()
        .unwrap();
    let doc: Doc = table.try_into().unwrap();
    assert_eq!(doc.ts.to_string(), "1979-05-27T07:32:00Z");
    assert_eq!(doc.name, "canonical");
}

#[test]
fn datetime_struct_field_from_value() {
    #[derive(Deserialize)]
    struct Doc {
        ts: Datetime,
    }
    let table: Table = "ts = 1979-05-27T07:32:00Z".parse().unwrap();
    let doc: Doc = Value::Table(table).try_into().unwrap();
    assert_eq!(doc.ts.to_string(), "1979-05-27T07:32:00Z");
}
