//! [`PipelineFingerprint`]  -  the content-addressed key for cache
//! lookups. Wrapped in its own module so the field-allowlist invariant
//! and hashing helper sit next to the public type.

use vyre_foundation::ir::Program;

/// Program-intrinsic fields that are permitted to contribute to
/// [`PipelineFingerprint`].
///
/// The key is intentionally narrow:
/// - canonical IR node graph
/// - declared buffer layout (names, bindings, access, dtypes, counts)
/// - the `Program`'s declared workgroup size
/// - canonical wire-format framing emitted by `Program::to_wire()`
///
/// The key intentionally excludes every dispatch-time concern:
/// - input buffer count or byte contents
/// - `DispatchConfig` labels, profiles, timeout, and ULP budget
/// - runtime workgroup overrides or launch geometry
///
/// The compile-time assertion below pins `PipelineFingerprint::of` to
/// `fn(&Program) -> PipelineFingerprint`, so no per-dispatch structure can
/// accidentally enter the key without changing the public signature.
const PIPELINE_FINGERPRINT_ALLOWED_FIELDS: &[&str] = &[
    "canonical_ir_graph",
    "buffer_layout",
    "declared_workgroup_size",
    "canonical_wire_framing",
];

/// The blake3 fingerprint of a canonicalized Program. 32 bytes so
/// collisions are cryptographically impossible for our scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineFingerprint(pub [u8; 32]);

const _: fn(&Program) -> PipelineFingerprint = PipelineFingerprint::of;

impl PipelineFingerprint {
    /// Derive a fingerprint from a Program. Runs
    /// `vyre_foundation::optimizer::passes::algebraic::canonicalize_engine::run`
    /// first so semantically-equal Programs share a fingerprint.
    ///
    /// Only program-intrinsic state is allowed into this hash. The
    /// fingerprint must stay stable across different dispatch inputs and
    /// execution-time knobs so the cache remains content-addressed rather
    /// than dispatch-addressed.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_foundation::ir::Program;
    /// use vyre_runtime::PipelineFingerprint;
    ///
    /// let a = Program::empty();
    /// let b = Program::empty();
    ///
    /// assert_eq!(PipelineFingerprint::of(&a), PipelineFingerprint::of(&b));
    /// ```
    #[must_use]
    pub fn of(program: &Program) -> Self {
        Self(hash_pipeline_fingerprint(program))
    }

    /// Hex-encode the fingerprint for human display + path-safe
    /// storage. Lowercase, no separators, 64 chars.
    #[must_use]
    pub fn hex(&self) -> String {
        let mut out = String::with_capacity(64);
        self.push_hex(&mut out);
        out
    }

    pub(super) fn push_hex(&self, out: &mut String) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for &byte in &self.0 {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
}

fn hash_pipeline_fingerprint(program: &Program) -> [u8; 32] {
    debug_assert_eq!(
        PIPELINE_FINGERPRINT_ALLOWED_FIELDS.len(),
        4,
        "Fix: update PIPELINE_FINGERPRINT_ALLOWED_FIELDS whenever the fingerprint contract changes."
    );
    // Audit P0 #26: routes through the shared
    // `vyre_foundation::optimizer::pipeline_fingerprint_bytes` so
    // AOT-emitted artifacts and runtime-cache blobs cannot drift apart.
    vyre_foundation::optimizer::pipeline_fingerprint_bytes(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_cache::test_helpers::tiny_program;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

    #[test]
    fn fingerprint_is_deterministic() {
        let a = PipelineFingerprint::of(&tiny_program());
        let b = PipelineFingerprint::of(&tiny_program());
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_hex_is_64_chars() {
        let fp = PipelineFingerprint::of(&tiny_program());
        assert_eq!(fp.hex().len(), 64);
    }

    #[test]
    fn canonically_equal_programs_share_fingerprint() {
        // `a + 1` and `1 + a` canonicalize to the same IR → same fingerprint.
        let p1 = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::add(Expr::var("a"), Expr::u32(1)),
            )],
        );
        let p2 = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::add(Expr::u32(1), Expr::var("a")),
            )],
        );
        let fp1 = PipelineFingerprint::of(&p1);
        let fp2 = PipelineFingerprint::of(&p2);
        assert_eq!(
            fp1, fp2,
            "canonicalize makes `a+1` and `1+a` share a fingerprint"
        );
    }

    #[test]
    fn fingerprint_changes_when_declared_program_shape_changes() {
        let base = tiny_program();
        let widened = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
            [64, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
        );

        assert_ne!(
            PipelineFingerprint::of(&base),
            PipelineFingerprint::of(&widened),
            "declared workgroup size is program-intrinsic and must change the fingerprint"
        );
    }
}
