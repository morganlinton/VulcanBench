//! `bitset_zero` - per-word device clear (`target[w] = 0`).
//!
//! Resident graph pipelines use this to clear scratch/output bitsets on device
//! instead of uploading zero-filled host buffers every iteration.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::bitset::zero";

/// Build a Program: `target[w] = 0` for `w` in `0..words`.
#[must_use]
pub fn bitset_zero(target: &str, words: u32) -> Program {
    let w = Expr::InvocationId { axis: 0 };
    Program::wrapped(
        vec![
            BufferDecl::storage(target, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(w.clone(), Expr::u32(words)),
                vec![Node::store(target, w, Expr::u32(0))],
            )]),
        }],
    )
}

/// CPU reference. Clears every target word.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(target: &mut [u32]) {
    target.fill(0);
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || bitset_zero("target", 3),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1, 0xDEAD_BEEF, u32::MAX])]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[0, 0, 0])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_clears_all_words() {
        let mut words = vec![1u32, 0xDEAD_BEEF, u32::MAX];
        cpu_ref(&mut words);
        assert_eq!(words, vec![0, 0, 0]);
    }

    #[test]
    fn emitted_program_has_one_rw_target_buffer() {
        let program = bitset_zero("target", 17);
        assert_eq!(program.workgroup_size, [256, 1, 1]);
        assert_eq!(program.buffers.len(), 1);
        assert_eq!(program.buffers[0].count, 17);
    }
}
