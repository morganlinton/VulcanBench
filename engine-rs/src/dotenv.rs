//! Minimal `.env` loader mirroring `python-dotenv`'s `load_dotenv()` as the
//! CLI uses it: find the nearest `.env` walking up from the engine's own
//! directory, and set each variable only when the shell has not already set it.

use std::env;
use std::path::{Path, PathBuf};

/// Finds `.env` in `start` or any ancestor.
pub fn find_dotenv(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(".env"))
        .find(|candidate| candidate.is_file())
}

/// Parses `.env` text into key/value pairs in file order.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        vars.push((key.to_owned(), unquote(value.trim())));
    }
    vars
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        let inner = &value[1..value.len() - 1];
        return inner
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].to_owned();
    }
    // Unquoted values end at an inline comment introduced by " #".
    match value.find(" #") {
        Some(idx) => value[..idx].trim_end().to_owned(),
        None => value.to_owned(),
    }
}

/// Loads the nearest `.env` into the process environment without overriding
/// variables that are already set. Returns the file that was loaded, if any.
pub fn load(start: &Path) -> Option<PathBuf> {
    let path = find_dotenv(start)?;
    let text = std::fs::read_to_string(&path).ok()?;
    for (key, value) in parse(&text) {
        if env::var_os(&key).is_none() {
            env::set_var(key, value);
        }
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_example_file_shapes() {
        let text = "# comment\n\nPOSTGRES_PASSWORD=change_me\nexport TOKEN=\"a b\\n\"\nQUOTED='x # not a comment'\nPLAIN=value # trailing comment\n# KEY=commented\nBAD LINE\n=novalue\n";
        let vars = parse(text);
        assert_eq!(
            vars,
            vec![
                ("POSTGRES_PASSWORD".to_owned(), "change_me".to_owned()),
                ("TOKEN".to_owned(), "a b\n".to_owned()),
                ("QUOTED".to_owned(), "x # not a comment".to_owned()),
                ("PLAIN".to_owned(), "value".to_owned()),
            ]
        );
    }
}
