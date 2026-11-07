//! Small helpers that reproduce CPython spellings the artifacts depend on:
//! `repr()` of scalars, `round(x, n)`, ISO timestamps, `str.splitlines()`
//! and universal-newline text decoding.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::json::{format_float, Value};

/// `round(x, ndigits)` with CPython's correctly rounded, half-to-even result.
/// Rust's fixed-precision formatting is also correctly rounded, so formatting
/// and parsing back yields the same double.
pub fn py_round(x: f64, ndigits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.ndigits$}").parse().unwrap_or(x)
}

/// The Python type name of a JSON value, for error messages.
pub fn py_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

/// `repr(value)` for the JSON-shaped values tool arguments can carry.
pub fn py_repr(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format_float(*f),
        Value::Str(s) => py_repr_str(s),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Object(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", py_repr_str(k), py_repr(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
    }
}

/// `repr(str)`: single quotes unless the text has a single quote and no double quote.
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// `str(value)` for scalars as the CLI prints them (`None`, `1.0`, `0.998`).
pub fn py_str(value: &Value) -> String {
    match value {
        Value::Str(s) => s.clone(),
        other => py_repr(other),
    }
}

/// `datetime.now(UTC).isoformat()`: `YYYY-MM-DDTHH:MM:SS.ffffff+00:00`,
/// with the fraction omitted when the microsecond field is zero.
pub fn utc_now_isoformat() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let micros = now.subsec_micros();
    let (year, month, day) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let fraction = if micros == 0 {
        String::new()
    } else {
        format!(".{micros:06}")
    };
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{fraction}+00:00")
}

/// Proleptic Gregorian date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (
        year,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// Text decoding with Python's universal newlines: `\r\n` and `\r` become `\n`.
pub fn universal_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_owned();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// `str.splitlines(keepends)` with Python's full set of line boundaries.
pub fn py_splitlines(text: &str, keepends: bool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        let boundary = matches!(
            ch,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !boundary {
            current.push(ch);
            continue;
        }
        let mut ending = String::from(ch);
        if ch == '\r' && chars.peek() == Some(&'\n') {
            ending.push('\n');
            chars.next();
        }
        if keepends {
            current.push_str(&ending);
        }
        lines.push(std::mem::take(&mut current));
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// `len(str)`: the number of code points, which is what every Python cap counts.
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// `s[:n]` by code points.
pub fn char_prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_matches_python() {
        assert_eq!(py_round(0.998_044_4, 4), 0.998);
        assert_eq!(py_round(2.675, 2), 2.67);
        assert_eq!(py_round(1.0, 4), 1.0);
        assert_eq!(py_round(0.982_4, 4), 0.9824);
    }

    #[test]
    fn repr_spellings() {
        assert_eq!(py_repr(&Value::Str("abc".to_owned())), "'abc'");
        assert_eq!(py_repr(&Value::Str("it's".to_owned())), "\"it's\"");
        assert_eq!(py_repr(&Value::Str("a\nb".to_owned())), "'a\\nb'");
        assert_eq!(py_repr(&Value::Bool(true)), "True");
        assert_eq!(py_repr(&Value::Null), "None");
        assert_eq!(py_repr(&Value::Float(5.0)), "5.0");
        let list = Value::Array(vec![Value::Int(1), Value::Str("x".to_owned())]);
        assert_eq!(py_repr(&list), "[1, 'x']");
        let mut map = Value::object();
        map.insert("k", 2);
        assert_eq!(py_repr(&map), "{'k': 2}");
    }

    #[test]
    fn dates_and_lines() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_749), (2026, 10, 23));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(py_splitlines("a\nb\r\nc\rd", false), ["a", "b", "c", "d"]);
        assert_eq!(py_splitlines("a\nb\n", true), ["a\n", "b\n"]);
        assert_eq!(py_splitlines("", false).len(), 0);
        assert_eq!(universal_newlines("x\r\ny\rz"), "x\ny\nz");
        assert_eq!(char_prefix("héllo", 2), "hé");
        assert_eq!(char_len("héllo"), 5);
    }
}
