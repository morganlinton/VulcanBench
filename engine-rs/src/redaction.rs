//! Secret redaction and field capping before anything touches disk, a port of
//! `harness/redaction.py` without a regex engine: every credential pattern is
//! a literal prefix followed by a character class with a minimum run length.

use crate::json::Value;
use crate::pyfmt::{char_len, char_prefix};

pub const REDACTED: &str = "[REDACTED]";
pub const MAX_FIELD_CHARS: usize = 20_000;

/// Env vars whose literal values are scrubbed wherever they appear.
const SECRET_ENV: [&str; 16] = [
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "ZAI_API_KEY",
    "MOONSHOT_API_KEY",
    "DASHSCOPE_API_KEY",
    "DEEPSEEK_API_KEY",
    "MODEL_API_KEY",
    "META_MUSE_SPARK_API",
    "OPENROUTER_API_KEY",
    "XAI_API_KEY",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "AWS_SECRET_ACCESS_KEY",
    "HF_TOKEN",
    "VULCANBENCH_API_TOKEN",
    "VULCANBENCH_API_KEY",
];

struct Pattern {
    prefix: &'static str,
    /// Alternatives for one character right after the prefix (`gh[opsru]_`).
    prefix_class: Option<(&'static str, &'static str)>,
    class: fn(u8) -> bool,
    min: usize,
    max: Option<usize>,
}

fn key_class(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn alnum_underscore(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn upper_digit(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit()
}

fn alnum_dash(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-'
}

/// Credential shapes, most specific prefix first (`sk-ant-` before `sk-`).
const PATTERNS: [Pattern; 7] = [
    Pattern {
        prefix: "sk-ant-",
        prefix_class: None,
        class: key_class,
        min: 16,
        max: None,
    },
    Pattern {
        prefix: "sk-",
        prefix_class: None,
        class: key_class,
        min: 16,
        max: None,
    },
    Pattern {
        prefix: "gh",
        prefix_class: Some(("opsru", "_")),
        class: alnum,
        min: 16,
        max: None,
    },
    Pattern {
        prefix: "github_pat_",
        prefix_class: None,
        class: alnum_underscore,
        min: 20,
        max: None,
    },
    Pattern {
        prefix: "AKIA",
        prefix_class: None,
        class: upper_digit,
        min: 16,
        max: Some(16),
    },
    Pattern {
        prefix: "xox",
        prefix_class: Some(("baprs", "-")),
        class: alnum_dash,
        min: 10,
        max: None,
    },
    Pattern {
        prefix: "AIza",
        prefix_class: None,
        class: key_class,
        min: 30,
        max: None,
    },
];

/// Length of a match of `pattern` starting at byte `at`, if any.
fn match_at(bytes: &[u8], at: usize, pattern: &Pattern) -> Option<usize> {
    let rest = &bytes[at..];
    if !rest.starts_with(pattern.prefix.as_bytes()) {
        return None;
    }
    let mut pos = pattern.prefix.len();
    if let Some((alternatives, tail)) = pattern.prefix_class {
        let next = *rest.get(pos)?;
        if !alternatives.as_bytes().contains(&next) {
            return None;
        }
        pos += 1;
        if !rest[pos..].starts_with(tail.as_bytes()) {
            return None;
        }
        pos += tail.len();
    }
    let run = rest[pos..].iter().take_while(|b| (pattern.class)(**b)).count();
    if run < pattern.min {
        return None;
    }
    let taken = pattern.max.map_or(run, |max| run.min(max));
    Some(pos + taken)
}

/// `re.sub(pattern, REDACTED, text)`: leftmost, non-overlapping matches.
fn substitute(text: &str, pattern: &Pattern) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i < bytes.len() {
        if let Some(len) = match_at(bytes, i, pattern) {
            out.push_str(&text[copied..i]);
            out.push_str(REDACTED);
            i += len;
            copied = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[copied..]);
    out
}

fn bearer_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
}

/// `(?i)(bearer\s+)[A-Za-z0-9._-]{16,}` with the prefix kept.
fn substitute_bearer(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.len() != text.len() || !lower.contains("bearer") {
        // A case-changing character that alters byte length would desync the
        // offsets; such text cannot contain an ASCII "bearer" prefix anyway.
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut search_from = 0;
    while let Some(found) = lower[search_from..].find("bearer") {
        let start = search_from + found;
        let after = start + "bearer".len();
        let ws_len: usize = text[after..]
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(char::len_utf8)
            .sum();
        let token_start = after + ws_len;
        let token_len: usize = text[token_start..]
            .chars()
            .take_while(|c| bearer_token_char(*c))
            .map(char::len_utf8)
            .sum();
        if ws_len > 0 && token_len >= 16 {
            out.push_str(&text[copied..token_start]);
            out.push_str(REDACTED);
            copied = token_start + token_len;
            search_from = copied;
        } else {
            search_from = after;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Masks credential-shaped substrings and known secret env values.
pub fn redact(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = text.to_owned();
    for var in SECRET_ENV {
        if let Ok(value) = std::env::var(var) {
            if value.len() >= 8 {
                out = out.replace(&value, REDACTED);
            }
        }
    }
    out = substitute_bearer(&out);
    for pattern in &PATTERNS {
        out = substitute(&out, pattern);
    }
    out
}

fn cap(text: String, max_chars: usize) -> String {
    let len = char_len(&text);
    if len <= max_chars {
        return text;
    }
    let omitted = len - max_chars;
    format!(
        "{}...[truncated {omitted} chars]",
        char_prefix(&text, max_chars)
    )
}

/// Recursively redacts secrets and caps string length in a JSON-able value.
pub fn sanitize(value: &Value) -> Value {
    sanitize_with(value, MAX_FIELD_CHARS)
}

fn sanitize_with(value: &Value, max_chars: usize) -> Value {
    match value {
        Value::Str(s) => Value::Str(cap(redact(s), max_chars)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| sanitize_with(v, max_chars)).collect())
        }
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), sanitize_with(v, max_chars)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_credential_shapes() {
        let key = "sk-ant-".to_owned() + &"a".repeat(20);
        assert_eq!(redact(&format!("key={key} end")), "key=[REDACTED] end");
        let openai = "sk-".to_owned() + &"b".repeat(16);
        assert_eq!(redact(&openai), REDACTED);
        assert_eq!(redact("sk-short"), "sk-short");
        let gh = "ghp_".to_owned() + &"c".repeat(16);
        assert_eq!(redact(&format!("token {gh}!")), "token [REDACTED]!");
        assert_eq!(redact("ghx_cccccccccccccccc"), "ghx_cccccccccccccccc");
        assert_eq!(redact("AKIAABCDEFGHIJKLMNOPQRS"), "[REDACTED]QRS");
        let slack = "xoxb-".to_owned() + &"1".repeat(10);
        assert_eq!(redact(&slack), REDACTED);
        let google = "AIza".to_owned() + &"z".repeat(30);
        assert_eq!(redact(&google), REDACTED);
        let pat = "github_pat_".to_owned() + &"d".repeat(20);
        assert_eq!(redact(&pat), REDACTED);
    }

    #[test]
    fn bearer_keeps_its_prefix() {
        let token = "A".repeat(16);
        assert_eq!(
            redact(&format!("Authorization: Bearer {token}")),
            "Authorization: Bearer [REDACTED]"
        );
        assert_eq!(redact("bearer\tshort"), "bearer\tshort");
        assert_eq!(redact("bearerNoSpaceAAAAAAAAAAAAAAAA"), "bearerNoSpaceAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn sanitize_caps_long_strings() {
        let long = "x".repeat(MAX_FIELD_CHARS + 5);
        let value = Value::Array(vec![Value::Str(long), Value::Int(1)]);
        let Value::Array(items) = sanitize(&value) else {
            panic!("array expected");
        };
        let capped = items[0].as_str().expect("string");
        assert!(capped.ends_with("...[truncated 5 chars]"));
        assert_eq!(char_len(capped), MAX_FIELD_CHARS + "...[truncated 5 chars]".len());
        assert_eq!(items[1], Value::Int(1));
    }
}
