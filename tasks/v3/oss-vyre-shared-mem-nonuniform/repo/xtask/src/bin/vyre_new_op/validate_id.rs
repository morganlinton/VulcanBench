#![allow(missing_docs)]
use super::is_maintainer_allowed::is_maintainer_allowed;
use super::max_id_len::MAX_ID_LEN;
use super::reserved_id_env::RESERVED_ID_ENV;

pub(crate) fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > MAX_ID_LEN {
        return Err(format!(
            "Fix: op id `{id}` must be ASCII and between 1 and {MAX_ID_LEN} chars."
        ));
    }
    if !id
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.'))
    {
        return Err(format!(
            "Fix: change op id `{id}` to match ^[a-z0-9_.]+$ with ASCII characters only."
        ));
    }
    if (id.starts_with("internal.") || id.starts_with("test.")) && !is_maintainer_allowed() {
        return Err(format!(
            "Fix: op id `{id}` is reserved. Set {RESERVED_ID_ENV}=1 to generate reserved ops."
        ));
    }
    Ok(())
}
