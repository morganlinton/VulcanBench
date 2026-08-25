//! Fusion certificates  -  prove a fused kernel is equivalent to the unfused
//! chain on a declared witness set.
//!
//! Hook emitted after the fusion pass: captures `(pre_program_blake3,
//! post_program_blake3)` plus the witness set fingerprint used to verify the
//! fused kernel matches unfused on every boundary input. Consumers (conform
//! runner) attach the cert to the compiled kernel so `--unfuse` diagnostic
//! inversion is reversible: the cert carries enough context to rehydrate.

use crate::ir_inner::model::program::Program;

/// Certificate proving a fused kernel is equivalent to the unfused chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FusionCertificate {
    /// Stable fingerprint of the program BEFORE fusion.
    pub pre_blake3: [u8; 32],
    /// Stable fingerprint AFTER fusion.
    pub post_blake3: [u8; 32],
    /// Name of the witness set used to verify parity.
    pub witness_set: &'static str,
    /// `true` when every witness produced bit-identical output pre vs post.
    pub parity_holds: bool,
}

impl FusionCertificate {
    /// Build a certificate for a fusion transformation.
    ///
    /// Computes canonical blake3 fingerprints via the wire encoder; the
    /// caller supplies the witness set name and parity verdict from the
    /// conform-enforce pipeline.
    #[must_use]
    pub fn for_fusion(
        pre: &Program,
        post: &Program,
        witness_set: &'static str,
        parity_holds: bool,
    ) -> Self {
        Self {
            pre_blake3: blake3_program(pre),
            post_blake3: blake3_program(post),
            witness_set,
            parity_holds,
        }
    }

    /// True when this cert proves the fusion is safe (parity held).
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.parity_holds
    }
}

fn blake3_program(program: &Program) -> [u8; 32] {
    program.fingerprint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Node, Program};

    fn trivial_program() -> Program {
        Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![Node::let_bind("idx", crate::ir::Expr::u32(0))],
        )
    }

    #[test]
    fn cert_records_pre_post_fingerprints() {
        let pre = trivial_program();
        let post = trivial_program();
        let cert = FusionCertificate::for_fusion(&pre, &post, "u32-witness-v1", true);
        // Pre and post are identical by construction, so fingerprints match.
        assert_eq!(cert.pre_blake3, cert.post_blake3);
        assert!(cert.is_sound());
    }

    #[test]
    fn cert_flags_unsound_fusion() {
        let pre = trivial_program();
        let post = trivial_program();
        let cert = FusionCertificate::for_fusion(&pre, &post, "u32-witness-v1", false);
        assert!(!cert.is_sound());
    }
}
