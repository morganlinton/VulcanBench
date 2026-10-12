#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick_cache::json_string_field;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

const MAX_CACHED_OUTCOME_BYTES: u64 = 1_048_576;

pub(crate) fn cached_outcome(path: &Path) -> Result<Option<String>, String> {
    match read_text_bounded(path) {
        Ok(content) => Ok(json_string_field(&content, "outcome")),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!(
            "could not read {}: {err}. Fix: remove corrupt cache file and rerun.",
            path.display()
        )),
    }
}

fn read_text_bounded(path: &Path) -> io::Result<String> {
    let mut reader = fs::File::open(path)?.take(MAX_CACHED_OUTCOME_BYTES.saturating_add(1));
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    if text.len() as u64 > MAX_CACHED_OUTCOME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds {MAX_CACHED_OUTCOME_BYTES} byte cached outcome read cap",
                path.display()
            ),
        ));
    }
    Ok(text)
}
