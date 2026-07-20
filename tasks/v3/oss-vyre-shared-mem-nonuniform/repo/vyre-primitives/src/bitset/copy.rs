//! `bitset_copy`  -  per-word bitwise copy (`target = source`).
//!
//! Replaces the `bitset_or_into` "OR-into-zero" idiom that external analyzer was
//! using as a structural copy. Explicit primitive: doc-clear,
//! semantics obvious, kernel one assignment per word. Downstream analyzer's
//! lower_expr's BindingRef arm (and any other "structural copy
//! between two same-shape bitset buffers") consumes this directly.

use vyre_foundation::ir::Program;

use super::binary_word::copy_word_program;

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::copy";

/// Build a Program: `target[w] = source[w]` for `w` in `0..words`.
#[must_use]
pub fn bitset_copy(target: &str, source: &str, words: u32) -> Program {
    copy_word_program(OP_ID, target, source, words)
}

/// CPU reference. Copies `source` into `target` word-for-word.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(target: &mut [u32], source: &[u32]) {
    let n = target.len().min(source.len());
    target[..n].copy_from_slice(&source[..n]);
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_copy("target", "source", 2),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0]),
                to_bytes(&[0xDEAD, 0xBEEF]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0xDEAD, 0xBEEF])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_copies_word_for_word() {
        let src = vec![0x1234_5678, 0xDEAD_BEEF, 0x0000_FFFF, 0xFFFF_0000];
        let mut dst = vec![0u32; src.len()];
        cpu_ref(&mut dst, &src);
        assert_eq!(dst, src);
    }

    #[test]
    fn cpu_ref_stops_at_shorter_source() {
        let src = vec![1u32, 2, 3];
        let mut dst = vec![10u32, 20, 30, 40, 50];
        cpu_ref(&mut dst, &src);
        assert_eq!(dst, vec![1u32, 2, 3, 40, 50]);
    }
}
