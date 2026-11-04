//! Hand-rolled JSON: an ordered value type, a parser and a Python-compatible writer.
//!
//! Objects keep insertion order so `summary.json` and `trace.jsonl` carry the
//! same key order as the Python engine's `json.dumps`. The writer reproduces
//! CPython's defaults: `", "` and `": "` separators in compact mode,
//! `indent=2` layout in pretty mode, ASCII-only output with `\uXXXX` escapes,
//! and `repr(float)` spelling for numbers (`1.0`, `0.9824`, `1e-05`, `1e+16`).

use std::fmt;

/// A JSON value. Integers and floats are kept apart because Python prints
/// `1` and `1.0` differently.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Builds an ordered JSON object: `obj! { "key" => value, ... }`.
macro_rules! obj {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut object = $crate::json::Value::object();
        $( object.insert($key, $value); )*
        object
    }};
}
pub(crate) use obj;

impl Value {
    /// An empty object.
    pub fn object() -> Self {
        Value::Object(Vec::new())
    }

    /// Looks a key up in an object; `None` for other kinds or missing keys.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The integer payload; booleans are not integers here (unlike Python).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Any number as a float.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Sets a key on an object, replacing an existing entry in place (Python
    /// dict semantics: the key keeps its original position).
    ///
    /// # Panics
    ///
    /// Panics when called on a non-object, which is a programming error.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> &mut Self {
        let key = key.into();
        let value = value.into();
        match self {
            Value::Object(entries) => {
                if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
                    slot.1 = value;
                } else {
                    entries.push((key, value));
                }
            }
            other => panic!("insert on a non-object JSON value: {other:?}"),
        }
        self
    }

    /// Appends every entry of `other` (an object) onto this object, like `dict.update`.
    pub fn extend(&mut self, other: Value) -> &mut Self {
        if let Value::Object(entries) = other {
            for (k, v) in entries {
                self.insert(k, v);
            }
        }
        self
    }

    /// Compact text with Python's default separators.
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        write_compact(self, &mut out);
        out
    }

    /// Pretty text matching `json.dumps(value, indent=2)`.
    pub fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        write_pretty(self, &mut out, 0);
        out
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Value::Str(s.clone())
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int(i64::from(i))
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Value::Int(i64::try_from(n).unwrap_or(i64::MAX))
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Self {
        Value::Array(items)
    }
}

impl From<Vec<String>> for Value {
    fn from(items: Vec<String>) -> Self {
        Value::Array(items.into_iter().map(Value::Str).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(opt: Option<T>) -> Self {
        opt.map_or(Value::Null, Into::into)
    }
}

impl From<&Value> for Value {
    fn from(v: &Value) -> Self {
        v.clone()
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// `repr(float)` as CPython spells it.
pub fn format_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    // Rust's Debug form is the shortest round-trip spelling with the same
    // fixed/exponent thresholds as CPython; only the exponent syntax differs.
    let text = format!("{f:?}");
    match text.split_once('e') {
        None => text,
        Some((mantissa, exponent)) => {
            let exp: i32 = exponent.parse().unwrap_or(0);
            let sign = if exp < 0 { '-' } else { '+' };
            format!("{mantissa}e{sign}{:02}", exp.abs())
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ' '..='~' => out.push(ch),
            _ => {
                let mut units = [0u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    out.push('"');
}

fn write_scalar(value: &Value, out: &mut String) -> bool {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&format_float(*f)),
        Value::Str(s) => write_string(s, out),
        Value::Array(_) | Value::Object(_) => return false,
    }
    true
}

fn write_compact(value: &Value, out: &mut String) {
    if write_scalar(value, out) {
        return;
    }
    match value {
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_string(k, out);
                out.push_str(": ");
                write_compact(v, out);
            }
            out.push('}');
        }
        _ => {}
    }
}

fn write_pretty(value: &Value, out: &mut String, level: usize) {
    if write_scalar(value, out) {
        return;
    }
    let inner = "  ".repeat(level + 1);
    let outer = "  ".repeat(level);
    match value {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(entries) if entries.is_empty() => out.push_str("{}"),
        Value::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&inner);
                write_pretty(item, out, level + 1);
            }
            out.push('\n');
            out.push_str(&outer);
            out.push(']');
        }
        Value::Object(entries) => {
            out.push_str("{\n");
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&inner);
                write_string(k, out);
                out.push_str(": ");
                write_pretty(v, out, level + 1);
            }
            out.push('\n');
            out.push_str(&outer);
            out.push('}');
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// A JSON syntax error with the byte offset it was found at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub offset: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at offset {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// Parses one JSON document. Trailing whitespace is allowed, anything else is not.
///
/// # Errors
///
/// Returns a [`ParseError`] for malformed input.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("trailing characters"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_owned(),
            offset: self.pos,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), ParseError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", byte as char)))
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            None => Err(self.error("unexpected end of input")),
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::Str),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, ParseError> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("invalid literal"))
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let int_start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == int_start {
            return Err(self.error("expected a digit"));
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            let frac_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == frac_start {
                return Err(self.error("expected a fraction digit"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let exp_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == exp_start {
                return Err(self.error("expected an exponent digit"));
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.error("invalid number"))?;
        if !is_float {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Value::Int(i));
            }
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.error("invalid number"))
    }

    fn hex4(&mut self) -> Result<u16, ParseError> {
        let end = self.pos + 4;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| self.error("truncated \\u escape"))?;
        let text = std::str::from_utf8(slice).map_err(|_| self.error("invalid \\u escape"))?;
        let unit = u16::from_str_radix(text, 16).map_err(|_| self.error("invalid \\u escape"))?;
        self.pos = end;
        Ok(unit)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.pos])
                    .map_err(|_| self.error("invalid UTF-8"))?,
            );
            match self.peek() {
                None => return Err(self.error("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let esc = self.peek().ok_or_else(|| self.error("truncated escape"))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&unit)
                                && self.bytes[self.pos..].starts_with(b"\\u")
                            {
                                self.pos += 2;
                                let low = self.hex4()?;
                                if (0xDC00..0xE000).contains(&low) {
                                    0x10000
                                        + ((u32::from(unit) - 0xD800) << 10)
                                        + (u32::from(low) - 0xDC00)
                                } else {
                                    u32::from(unit)
                                }
                            } else {
                                u32::from(unit)
                            };
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(self.error("invalid escape")),
                    }
                }
                Some(_) => return Err(self.error("control character in string")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.expect(b'{')?;
        let mut object = Value::object();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(object);
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.value()?;
            object.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(object);
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_summary_shapes_in_order() {
        let text = r#"{"b": 1, "a": [1.0, 2.5, null, true], "c": {"z": "x", "y": {}}, "d": []}"#;
        let value = parse(text).expect("parses");
        assert_eq!(value.to_json(), text);
        let keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(keys, ["b", "a", "c", "d"]);
    }

    #[test]
    fn pretty_matches_python_indent_2() {
        let value = obj! {
            "run_id" => "x",
            "scores" => obj! { "functional" => 1.0, "human_like" => Value::Null },
            "empty" => Value::object(),
            "list" => vec![Value::Int(1), Value::Int(2)],
            "none" => Vec::<Value>::new(),
        };
        let expected = "{\n  \"run_id\": \"x\",\n  \"scores\": {\n    \"functional\": 1.0,\n    \"human_like\": null\n  },\n  \"empty\": {},\n  \"list\": [\n    1,\n    2\n  ],\n  \"none\": []\n}";
        assert_eq!(value.to_json_pretty(), expected);
    }

    #[test]
    fn floats_spell_like_python_repr() {
        assert_eq!(format_float(1.0), "1.0");
        assert_eq!(format_float(0.9824), "0.9824");
        assert_eq!(format_float(100.0), "100.0");
        assert_eq!(format_float(0.0), "0.0");
        assert_eq!(format_float(-0.0), "-0.0");
        assert_eq!(format_float(1e16), "1e+16");
        assert_eq!(format_float(1.5e-7), "1.5e-07");
        assert_eq!(format_float(1e15), "1000000000000000.0");
        assert_eq!(format_float(0.0001), "0.0001");
    }

    #[test]
    fn strings_escape_like_ensure_ascii() {
        let value = Value::Str("a\"b\\c\n\t\u{7f}é😀/".to_owned());
        assert_eq!(
            value.to_json(),
            "\"a\\\"b\\\\c\\n\\t\\u007f\\u00e9\\ud83d\\ude00/\""
        );
        let back = parse(&value.to_json()).expect("parses");
        assert_eq!(back, value);
    }

    #[test]
    fn ints_and_floats_stay_distinct() {
        assert_eq!(parse("1").expect("int"), Value::Int(1));
        assert_eq!(parse("1.0").expect("float"), Value::Float(1.0));
        assert_eq!(parse("-2e3").expect("float"), Value::Float(-2000.0));
        assert_eq!(Value::Int(7).to_json(), "7");
    }

    #[test]
    fn insert_replaces_in_place() {
        let mut value = obj! { "a" => 1, "b" => 2 };
        value.insert("a", 3);
        assert_eq!(value.to_json(), r#"{"a": 3, "b": 2}"#);
    }

    #[test]
    fn rejects_trailing_garbage_and_bad_escapes() {
        assert!(parse("{} x").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("\"\\q\"").is_err());
        assert!(parse("").is_err());
    }
}
