//! Deterministic buffer naming for generic Cat-A builder aliases.
//!
//! Some vyre-libs builders historically accepted generic names such as
//! `"input"`, `"output"`, `"decoded"`, or `"out"`. When multiple Programs were
//! fused into one rule kernel, those aliases could collide with unrelated
//! buffers across families. This module rewrites only the generic aliases into
//! stable family-scoped names while preserving caller-specified explicit names.

/// Stable internal buffer name for a family-local role.
#[must_use]
pub(crate) fn fixed_name(family_prefix: &str, role: &str) -> String {
    format!("__vyre_{family_prefix}_{role}")
}

/// Rewrite a generic buffer name into a stable family-local name.
///
/// Explicit caller-provided names are preserved so intentional composition can
/// still route buffers by name.
#[must_use]
pub fn scoped_generic_name(
    family_prefix: &str,
    role: &str,
    requested: &str,
    generic_aliases: &[&str],
) -> String {
    if generic_aliases.iter().any(|alias| *alias == requested) {
        fixed_name(family_prefix, role)
    } else {
        requested.to_string()
    }
}
