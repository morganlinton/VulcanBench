//! A TOML subset: enough for `vulcanbench.toml` (tables, bare and quoted
//! keys, strings, integers, floats, booleans, arrays, comments).
//!
//! Not supported: inline tables, arrays of tables, dates, multi-line strings.
//! The parsed document is an ordered [`Value`] object so the settings reader
//! can share the JSON value type.

use std::fmt;

use crate::json::Value;

/// A TOML syntax error, or a construct outside the supported subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlError {
    pub message: String,
    pub line: usize,
}

impl fmt::Display for TomlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (line {})", self.message, self.line)
    }
}

impl std::error::Error for TomlError {}

/// Parses a TOML document into a nested object.
///
/// # Errors
///
/// Returns [`TomlError`] for malformed input or unsupported syntax.
pub fn parse(text: &str) -> Result<Value, TomlError> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
    };
    let mut root = Value::object();
    let mut current: Vec<String> = Vec::new();
    loop {
        parser.skip_blank();
        match parser.peek() {
            None => return Ok(root),
            Some('[') => {
                parser.pos += 1;
                if parser.peek() == Some('[') {
                    return Err(parser.error("arrays of tables are not supported"));
                }
                current = parser.key_path()?;
                parser.skip_spaces();
                parser.expect(']')?;
                ensure_table(&mut root, &current);
                parser.end_of_line()?;
            }
            Some(_) => {
                let path = parser.key_path()?;
                parser.skip_spaces();
                parser.expect('=')?;
                parser.skip_spaces();
                let value = parser.value()?;
                parser.end_of_line()?;
                let (last, parents) = path.split_last().ok_or_else(|| parser.error("empty key"))?;
                let full: Vec<String> = current.iter().chain(parents.iter()).cloned().collect();
                let table = ensure_table(&mut root, &full);
                if table.get(last).is_some() {
                    return Err(parser.error(&format!("duplicate key {last:?}")));
                }
                table.insert(last.clone(), value);
            }
        }
    }
}

fn ensure_table<'a>(root: &'a mut Value, path: &[String]) -> &'a mut Value {
    let mut node = root;
    for key in path {
        if node.get(key).is_none() {
            node.insert(key.clone(), Value::object());
        }
        let Value::Object(entries) = node else {
            unreachable!("tables are always objects");
        };
        node = &mut entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .expect("key was just inserted")
            .1;
        if !matches!(node, Value::Object(_)) {
            *node = Value::object();
        }
    }
    node
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Parser {
    fn error(&self, message: &str) -> TomlError {
        TomlError {
            message: message.to_owned(),
            line: self.line,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    fn expect(&mut self, ch: char) -> Result<(), TomlError> {
        if self.peek() == Some(ch) {
            self.bump();
            Ok(())
        } else {
            Err(self.error(&format!("expected {ch:?}")))
        }
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.pos += 1;
        }
    }

    fn skip_comment(&mut self) {
        if self.peek() == Some('#') {
            while !matches!(self.peek(), None | Some('\n')) {
                self.pos += 1;
            }
        }
    }

    /// Skips whitespace, newlines and comments.
    fn skip_blank(&mut self) {
        loop {
            self.skip_spaces();
            self.skip_comment();
            if matches!(self.peek(), Some('\n' | '\r')) {
                self.bump();
            } else {
                return;
            }
        }
    }

    fn end_of_line(&mut self) -> Result<(), TomlError> {
        self.skip_spaces();
        self.skip_comment();
        match self.peek() {
            None => Ok(()),
            Some('\n' | '\r') => {
                self.bump();
                Ok(())
            }
            Some(_) => Err(self.error("expected end of line")),
        }
    }

    fn key_path(&mut self) -> Result<Vec<String>, TomlError> {
        let mut path = Vec::new();
        loop {
            self.skip_spaces();
            let key = match self.peek() {
                Some('"') => self.basic_string()?,
                Some('\'') => self.literal_string()?,
                Some(c) if is_bare_key(c) => {
                    let start = self.pos;
                    while self.peek().is_some_and(is_bare_key) {
                        self.pos += 1;
                    }
                    self.chars[start..self.pos].iter().collect()
                }
                _ => return Err(self.error("expected a key")),
            };
            path.push(key);
            self.skip_spaces();
            if self.peek() == Some('.') {
                self.pos += 1;
            } else {
                return Ok(path);
            }
        }
    }

    fn value(&mut self) -> Result<Value, TomlError> {
        match self.peek() {
            Some('"') => self.basic_string().map(Value::Str),
            Some('\'') => self.literal_string().map(Value::Str),
            Some('[') => self.array(),
            Some('{') => Err(self.error("inline tables are not supported")),
            Some('t') if self.starts_with("true") => {
                self.pos += 4;
                Ok(Value::Bool(true))
            }
            Some('f') if self.starts_with("false") => {
                self.pos += 5;
                Ok(Value::Bool(false))
            }
            Some(c) if c == '+' || c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }

    fn starts_with(&self, word: &str) -> bool {
        word.chars()
            .enumerate()
            .all(|(i, c)| self.chars.get(self.pos + i) == Some(&c))
    }

    fn number(&mut self) -> Result<Value, TomlError> {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '_' | '.'))
        {
            self.pos += 1;
        }
        let raw: String = self.chars[start..self.pos]
            .iter()
            .filter(|c| **c != '_')
            .collect();
        if let Ok(i) = raw.parse::<i64>() {
            return Ok(Value::Int(i));
        }
        raw.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.error(&format!("invalid number {raw:?}")))
    }

    fn array(&mut self) -> Result<Value, TomlError> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            if self.peek() == Some(']') {
                self.bump();
                return Ok(Value::Array(items));
            }
            items.push(self.value()?);
            self.skip_blank();
            match self.peek() {
                Some(',') => {
                    self.bump();
                }
                Some(']') => {
                    self.bump();
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']' in array")),
            }
        }
    }

    fn basic_string(&mut self) -> Result<String, TomlError> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n') => return Err(self.error("unterminated string")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('u') => {
                        let hex: String = (0..4).filter_map(|_| self.bump()).collect();
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|_| self.error("invalid \\u escape"))?;
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    _ => return Err(self.error("invalid escape")),
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn literal_string(&mut self) -> Result<String, TomlError> {
        self.expect('\'')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n') => return Err(self.error("unterminated string")),
                Some('\'') => return Ok(out),
                Some(c) => out.push(c),
            }
        }
    }
}

fn is_bare_key(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_settings_file_shape() {
        let text = "# VulcanBench operator settings\n\n[effort]\n# blocked levels\nblocked = [\"ultra\"]  # trailing comment\n\n[sweep]\nconcurrency = 1\nratio = 0.5\nenabled = true\nname = 'plain'\n";
        let doc = parse(text).expect("parses");
        let blocked = doc.get("effort").and_then(|e| e.get("blocked")).expect("blocked");
        assert_eq!(blocked, &Value::Array(vec![Value::Str("ultra".to_owned())]));
        let sweep = doc.get("sweep").expect("sweep");
        assert_eq!(sweep.get("concurrency"), Some(&Value::Int(1)));
        assert_eq!(sweep.get("ratio"), Some(&Value::Float(0.5)));
        assert_eq!(sweep.get("enabled"), Some(&Value::Bool(true)));
        assert_eq!(sweep.get("name"), Some(&Value::Str("plain".to_owned())));
    }

    #[test]
    fn dotted_headers_and_multiline_arrays() {
        let text = "[a.b]\nxs = [\n  1,\n  2,\n]\n\"quoted key\" = \"v\\n\"\n";
        let doc = parse(text).expect("parses");
        let b = doc.get("a").and_then(|a| a.get("b")).expect("a.b");
        assert_eq!(b.get("xs"), Some(&Value::Array(vec![Value::Int(1), Value::Int(2)])));
        assert_eq!(b.get("quoted key"), Some(&Value::Str("v\n".to_owned())));
    }

    #[test]
    fn rejects_unsupported_and_malformed_input() {
        assert!(parse("x = {a = 1}").is_err());
        assert!(parse("[[t]]\n").is_err());
        assert!(parse("x = \n").is_err());
        assert!(parse("x = 1 y = 2\n").is_err());
        assert!(parse("x = 1\nx = 2\n").is_err());
    }
}
