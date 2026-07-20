//! Streaming decode → scan adapter (G5).
//!
//! # What this does
//!
//! `decode::base64 / hex / inflate / lz4` each used to produce a
//! storage-buffer output that `matching::dfa / nfa` then re-read
//! from DRAM. Two kernels, two DMA round-trips, one pipeline
//! barrier  -  cheap on a 1 MiB corpus, ruinous on a 100 GiB corpus
//! scan where the decoded-bytes footprint dominates the DRAM budget.
//!
//! The fused path hands bytes from decoder → scanner through
//! **workgroup-shared memory** on the same dispatch. No DRAM
//! round-trip, no pipeline barrier  -  the scanner's loads hit L1 on
//! the SM that decoded the bytes.
//!
//! # Contract
//!
//! The caller supplies a `decoder` Program that *writes* a named
//! handoff buffer and a `scanner` Program that *reads* the same
//! named buffer. `fuse_decode_scan` merges them via the existing
//! [`vyre_foundation::execution_plan::fusion::fuse_programs`] kernel
//! fuser, then rewrites the handoff buffer's declaration so it lives
//! in workgroup memory instead of storage.
//!
//! # Why this module lives in vyre-libs
//!
//! The fusion transformation itself is a foundation-layer pass
//! (`optimizer::passes::decode_scan_fuse`). This module is the
//! library-level API that consumers reach for directly. Both
//! paths land on the same fused Program  -  the foundation pass is
//! the canonical transformation, this is a thin convenience layer.

use vyre_foundation::execution_plan::fusion::fuse_programs;
use vyre_foundation::ir::{BufferDecl, DataType, Program};

/// Error states surfaced by [`fuse_decode_scan`].
#[derive(Debug, thiserror::Error)]
pub enum DecodeScanFuseError {
    /// The caller supplied `&decoder` and `&scanner` but neither
    /// declares the named handoff buffer. Fusing would produce a
    /// Program with no shared byte-flow path  -  the caller's intent
    /// cannot be honoured.
    #[error(
        "Fix: handoff buffer {handoff:?} does not appear in the decoder or scanner Program's \
         buffer list. Add a `BufferDecl::storage({handoff:?}, ..., DataType::U32)` to both \
         Programs before calling `fuse_decode_scan`."
    )]
    HandoffBufferMissing {
        /// Name of the missing decoder/scanner handoff buffer.
        handoff: String,
    },
    /// The caller passed `handoff_byte_count = 0`. Workgroup
    /// allocations must be strictly positive or the fused Program
    /// declares a zero-sized shared buffer that every driver
    /// rejects. Returning an error instead of asserting (per
    /// PHASE2_DECODE HIGH / LAW 5) keeps adversarial callers from
    /// crashing the driver process.
    #[error(
        "Fix: fuse_decode_scan(handoff_byte_count = 0) is rejected on buffer {handoff:?}. \
         Pass the decoder's peak output-bytes-per-workgroup."
    )]
    ZeroHandoff {
        /// Name of the handoff buffer whose capacity was zero.
        handoff: String,
    },
    /// `fuse_programs` rejected the pair (self-aliasing or
    /// workgroup-size mismatch). The inner error is the original.
    #[error(
        "Fix: kernel-level fusion failed  -  run the autotune pass to normalise workgroup \
         sizes and rename any self-aliasing buffers before calling `fuse_decode_scan`. \
         Inner: {0}"
    )]
    Fusion(#[from] vyre_foundation::execution_plan::fusion::FusionError),
}

/// Fuse a decoder Program with a scanner Program into a single
/// dispatch. The decoder writes `handoff_buf`; the scanner reads
/// it. In the fused Program the handoff buffer is promoted to
/// workgroup memory so its bytes never touch DRAM.
///
/// `handoff_byte_count` is the capacity the fused Program reserves
/// for the workgroup handoff  -  typically the decoder's max output
/// bytes per workgroup. Must be strictly positive.
pub fn fuse_decode_scan(
    decoder: Program,
    scanner: Program,
    handoff_buf: &str,
    handoff_byte_count: u32,
) -> Result<Program, DecodeScanFuseError> {
    if handoff_byte_count == 0 {
        return Err(DecodeScanFuseError::ZeroHandoff {
            handoff: handoff_buf.to_string(),
        });
    }
    let decoder_has = decoder.buffers.iter().any(|b| b.name() == handoff_buf);
    let scanner_has = scanner.buffers.iter().any(|b| b.name() == handoff_buf);
    if !decoder_has && !scanner_has {
        return Err(DecodeScanFuseError::HandoffBufferMissing {
            handoff: handoff_buf.to_string(),
        });
    }

    let fused = fuse_programs(&[decoder, scanner])?;
    Ok(promote_to_workgroup(fused, handoff_buf, handoff_byte_count))
}

fn promote_to_workgroup(program: Program, handoff_buf: &str, count: u32) -> Program {
    // `BufferDecl::workgroup` already sets `access: Workgroup` and
    // `kind: Shared`  -  no extra `.with_kind()` required.
    let mut new_buffers: Vec<BufferDecl> = program
        .buffers
        .iter()
        .filter(|b| b.name() != handoff_buf)
        .cloned()
        .collect();
    new_buffers.push(BufferDecl::workgroup(handoff_buf, count, DataType::U32));
    // Avoid a full `Vec<Node>` deep-clone when we own the Arc.
    let entry = std::sync::Arc::try_unwrap(program.entry).unwrap_or_else(|arc| (*arc).clone());
    Program::wrapped(new_buffers, program.workgroup_size, entry)
}

/// How many bytes of DRAM traffic one dispatch saves by fusing a
/// decoder+scanner pair with an N-byte handoff. Used by the G12
/// benchmark harness to verify the fusion is paying off.
#[must_use]
pub fn dram_bytes_saved(handoff_byte_count: u32, invocations: u32) -> u64 {
    // Decoder would have written `handoff_byte_count` bytes to DRAM
    // per invocation, scanner would have read the same. 2×.
    2_u64 * u64::from(handoff_byte_count) * u64::from(invocations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, Expr, MemoryKind, Node};

    fn decoder_with_handoff(handoff: &str) -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(64),
                BufferDecl::storage(handoff, 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(64),
            ],
            [64, 1, 1],
            vec![Node::store(
                handoff,
                Expr::InvocationId { axis: 0 },
                Expr::u32(0xAA),
            )],
        )
    }

    fn scanner_with_handoff(handoff: &str) -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage(handoff, 1, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(64),
                BufferDecl::storage("matches", 2, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(64),
            ],
            [64, 1, 1],
            vec![Node::let_bind(
                "byte",
                Expr::load(handoff, Expr::InvocationId { axis: 0 }),
            )],
        )
    }

    #[test]
    fn missing_handoff_buffer_errors_with_actionable_fix() {
        let decoder = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
            ],
            [64, 1, 1],
            vec![],
        );
        let scanner = Program::wrapped(
            vec![
                BufferDecl::storage("matches", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(1),
            ],
            [64, 1, 1],
            vec![],
        );
        let err = fuse_decode_scan(decoder, scanner, "decoded", 64).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Fix:"));
        assert!(msg.contains("decoded"));
    }

    #[test]
    fn zero_handoff_byte_count_returns_structured_error() {
        let decoder = decoder_with_handoff("decoded");
        let scanner = scanner_with_handoff("decoded");
        let err = fuse_decode_scan(decoder, scanner, "decoded", 0).unwrap_err();
        assert!(matches!(err, DecodeScanFuseError::ZeroHandoff { .. }));
        assert!(err.to_string().contains("Fix:"));
    }

    #[test]
    fn fused_program_promotes_handoff_to_workgroup_memory() {
        let decoder = decoder_with_handoff("decoded");
        let scanner = scanner_with_handoff("decoded");
        let fused = fuse_decode_scan(decoder, scanner, "decoded", 128).unwrap();
        let handoff = fused.buffers.iter().find(|b| b.name() == "decoded").expect(
            "Fix: handoff buffer survives fusion; restore this invariant before continuing.",
        );
        assert_eq!(handoff.access(), BufferAccess::Workgroup);
        assert_eq!(handoff.kind(), MemoryKind::Shared);
        assert_eq!(handoff.count(), 128);
    }

    #[test]
    fn non_handoff_buffers_stay_as_declared() {
        let decoder = decoder_with_handoff("decoded");
        let scanner = scanner_with_handoff("decoded");
        let fused = fuse_decode_scan(decoder, scanner, "decoded", 128).unwrap();
        let input = fused.buffers.iter().find(|b| b.name() == "input").unwrap();
        assert_eq!(input.access(), BufferAccess::ReadOnly);
        let matches = fused
            .buffers
            .iter()
            .find(|b| b.name() == "matches")
            .unwrap();
        assert_eq!(matches.access(), BufferAccess::ReadWrite);
    }

    #[test]
    fn fused_body_contains_both_decoder_and_scanner_nodes() {
        let decoder = decoder_with_handoff("decoded");
        let scanner = scanner_with_handoff("decoded");
        let fused = fuse_decode_scan(decoder, scanner, "decoded", 64).unwrap();
        // `Program::wrapped` always normalizes a multi-node entry into a
        // single root `Node::Region` that owns the per-arm body; check
        // that body holds the two arms (plus any inserted barrier).
        assert_eq!(
            fused.entry.len(),
            1,
            "wrapped entry must be a single root Region"
        );
        let body = match &fused.entry[0] {
            vyre::ir::Node::Region { body, .. } => body.as_ref(),
            other => panic!("Fix: fused entry root must be a Region, got {other:?}"),
        };
        assert!(
            body.len() >= 2,
            "fused root region body should contain both arms, got {} nodes",
            body.len()
        );
    }

    #[test]
    fn dram_bytes_saved_scales_with_invocations() {
        assert_eq!(dram_bytes_saved(0, 1_000_000), 0);
        assert_eq!(dram_bytes_saved(1024, 1000), 2 * 1024 * 1000);
        assert_eq!(dram_bytes_saved(1, u32::MAX), 2 * u64::from(u32::MAX));
    }

    #[test]
    fn handoff_present_in_only_scanner_still_fuses() {
        let decoder = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
            ],
            [64, 1, 1],
            vec![],
        );
        let scanner = scanner_with_handoff("decoded");
        let fused = fuse_decode_scan(decoder, scanner, "decoded", 64).unwrap();
        let handoff = fused
            .buffers
            .iter()
            .find(|b| b.name() == "decoded")
            .unwrap();
        assert_eq!(handoff.access(), BufferAccess::Workgroup);
    }
}
