#![allow(missing_docs)]
use super::allowed_archetypes::ALLOWED_ARCHETYPES;

pub(crate) fn validate_archetype(archetype: &str) -> Result<(), String> {
    if ALLOWED_ARCHETYPES.contains(&archetype) {
        return Ok(());
    }
    Err(format!(
        "Fix: archetype `{}` is unknown. Valid archetypes: {}",
        archetype,
        ALLOWED_ARCHETYPES.join(", ")
    ))
}
