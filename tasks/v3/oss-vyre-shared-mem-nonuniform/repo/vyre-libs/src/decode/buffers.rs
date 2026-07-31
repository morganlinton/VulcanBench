//! Decode-family buffer scoping helpers.
//!
//! Decode compositions keep public builder names ergonomic (`input`, `output`,
//! `decoded`) while internally isolating fixture/default buffer names by
//! family. Centralizing that rule prevents every decoder from carrying its own
//! alias policy.

use crate::buffer_names::scoped_generic_name;

/// Scope a decode input buffer while preserving explicit caller names.
#[must_use]
pub(crate) fn scoped_decode_input_buffer(family_prefix: &str, name: &str) -> String {
    scoped_generic_name(family_prefix, "input", name, &["input"])
}

/// Scope a decode output-like buffer with caller-provided generic aliases.
#[must_use]
pub(crate) fn scoped_decode_output_buffer(
    family_prefix: &str,
    default: &str,
    name: &str,
    aliases: &[&str],
) -> String {
    scoped_generic_name(family_prefix, default, name, aliases)
}

/// Scope the common decoded-output buffer used by byte decoders.
#[must_use]
pub(crate) fn scoped_decoded_output_buffer(family_prefix: &str, name: &str) -> String {
    scoped_decode_output_buffer(family_prefix, "decoded", name, &["decoded", "output"])
}
