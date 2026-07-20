//! Decode → scan fusion optimizer pass (G5).
//!
//! # Idea
//!
//! When a single Program already contains both a decoder and a
//! scanner  -  the decoder writes some `ReadWrite` storage handoff
//! buffer, the scanner then reads from it  -  the decoded bytes
//! don't need to round-trip through DRAM. Promoting the handoff
//! to workgroup memory keeps the bytes in the SM's shared
//! scratchpad and lets the scanner hit L1 instead of HBM.
//!
//! The companion library API in
//! `vyre_libs::decode::streaming::fuse_decode_scan` does the
//! same transform for a *pair* of Programs (separately-owned
//! decoder + scanner); this pass handles the pre-fused case that
//! already lives in one `Program`.
//!
//! # Transform
//!
//! For every buffer `b` where:
//!   * `b.access() == BufferAccess::ReadWrite` (written then read),
//!   * `b.count() > 0` (static size known  -  workgroup memory
//!     requires a compile-time count), and
//!   * `b` is not marked `pipeline_live_out` (a workgroup buffer
//!     cannot be observed outside the dispatch),
//!
//! the pass rewrites `b` in-place to
//! `BufferDecl::workgroup(name, count, element)`  -  the access mode
//! flips to `Workgroup`, the memory tier flips to `Shared`, and
//! the binding slot is dropped (workgroup buffers do not hold a
//! `@binding`). Entry-body node ops reference buffers by name, so
//! no body rewriting is required.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{BufferAccess, BufferDecl, DataType, Ident, Program};
use crate::optimizer::{fingerprint_program, vyre_pass, PassAnalysis, PassResult};

/// Conservative ceiling on workgroup-promoted buffer size.
///
/// vyre-driver's `DeviceCaps::wgpu_like_default` reports 16 KiB of
/// shared memory on the wgpu fallback path; CUDA/SPIR-V get 48 KiB+.
/// Without a target backend at this stage, we use the wgpu floor so a
/// program that compiles after this pass on any reachable backend.
const MAX_WORKGROUP_PROMOTION_BYTES: u64 = 16 * 1024;

/// Bytes-per-element for the destination workgroup buffer. Delegates
/// to the canonical [`DataType::size_bytes`] table so every variant
/// (U8/I8/Bool/Bytes = 1, U16/I16/F16/BF16 = 2, U32/I32/F32 = 4,
/// U64/I64/F64/Vec2U32 = 8, `Vec4U32` = 16, Vec/Array follow the
/// element/lane math, F8/F4/I4/NF4 = 1) is sized correctly.
///
/// `size_bytes` returns None for dynamically-sized variants (Tensor,
/// `TensorShaped`, SparseCsr/SparseCoo/SparseBsr, Opaque). Those cannot
/// be promoted to fixed-size workgroup storage because any guessed size
/// can understate shared-memory pressure and corrupt dispatch layout.
fn element_bytes(element: &DataType) -> Option<u64> {
    element.size_bytes().map(|bytes| bytes as u64)
}

fn fits_workgroup_budget(buf: &BufferDecl) -> bool {
    let Some(element_bytes) = element_bytes(&buf.element()) else {
        return false;
    };
    let Some(bytes) = u64::from(buf.count()).checked_mul(element_bytes) else {
        return false;
    };
    bytes > 0 && bytes <= MAX_WORKGROUP_PROMOTION_BYTES
}

/// Built-in optimizer pass for in-program decode/scan handoff fusion.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "decode_scan_fuse",
    requires = [],
    invalidates = ["buffer_layout", "fusion"]
)]
pub struct DecodeScanFuse;

impl DecodeScanFuse {
    /// Run only when a program has at least one promotable handoff buffer.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if count_opportunities(program) == 0 {
            PassAnalysis::SKIP
        } else {
            PassAnalysis::RUN
        }
    }

    /// Promote storage handoff buffers to workgroup memory.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let before = fingerprint_program(&program);
        let optimized = run(program);
        PassResult {
            changed: fingerprint_program(&optimized) != before,
            program: optimized,
        }
    }
}

/// Run the decode→scan fusion over a Program.
///
/// Promotes every handoff-looking `ReadWrite` storage buffer to
/// workgroup memory. Returns the rewritten Program. Caller-visible
/// buffers (`pipeline_live_out = true`) are preserved as-is.
#[must_use]
pub fn run(program: Program) -> Program {
    let promotable: FxHashSet<Ident> = program
        .buffers
        .iter()
        .filter(|b| {
            // Promotability criteria: `ReadWrite` (written then read within
            // one dispatch), static count (workgroup allocs must be compile-
            // time sized), and not externally observed (workgroup buffers
            // don't survive past dispatch end).
            b.access() == BufferAccess::ReadWrite
                && b.count() > 0
                && !b.is_pipeline_live_out()
                && fits_workgroup_budget(b)
        })
        .map(|b| Ident::from(b.name()))
        .collect();

    if promotable.is_empty() {
        return program;
    }

    let new_buffers: Vec<BufferDecl> = program
        .buffers
        .iter()
        .map(|b| {
            if promotable.contains(&Ident::from(b.name())) {
                BufferDecl::workgroup(b.name(), b.count(), b.element())
            } else {
                b.clone()
            }
        })
        .collect();

    // VYRE_IR_HOTSPOTS audit: avoid the deep-clone of the entry
    // Vec<Node>. When the Arc is unique (the common case  -  we own
    // the only reference after `run()` returns) `try_unwrap` hands
    // back the Vec<Node> directly. Only fall back to cloning when
    // another Arc is still outstanding.
    let entry = std::sync::Arc::try_unwrap(program.entry).unwrap_or_else(|arc| (*arc).clone());
    Program::wrapped(new_buffers, program.workgroup_size, entry)
}

/// Count decode-handoff candidate buffers in `program`  -  the
/// buffers `run` would promote. Identical filter to `run`.
#[must_use]
pub fn count_opportunities(program: &Program) -> usize {
    program
        .buffers
        .iter()
        .filter(|b| {
            // Same promotability criteria as `run`.
            b.access() == BufferAccess::ReadWrite
                && b.count() > 0
                && !b.is_pipeline_live_out()
                && fits_workgroup_budget(b)
        })
        .count()
}

/// Map from candidate handoff buffer name to its declared element
/// count. Parallel to [`count_opportunities`] with names exposed.
#[must_use]
pub fn candidate_handoffs(program: &Program) -> FxHashMap<Ident, u32> {
    let mut out = FxHashMap::default();
    for buf in program.buffers.iter() {
        // Same promotability criteria as `run` and `count_opportunities`.
        if buf.access() == BufferAccess::ReadWrite && buf.count() > 0 && !buf.is_pipeline_live_out()
        {
            out.insert(Ident::from(buf.name()), buf.count());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Program};

    fn decoder_like() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(64),
                BufferDecl::storage("decoded", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(128),
            ],
            [64, 1, 1],
            vec![],
        )
    }

    #[test]
    fn run_promotes_readwrite_handoff_to_workgroup() {
        let p = decoder_like();
        let before_bufs = p.buffers.len();
        let after = run(p);
        assert_eq!(after.buffers.len(), before_bufs);
        let decoded = after
            .buffers
            .iter()
            .find(|b| b.name() == "decoded")
            .unwrap();
        assert_eq!(decoded.access(), BufferAccess::Workgroup);
    }

    #[test]
    fn run_leaves_read_only_buffers_alone() {
        let p = decoder_like();
        let after = run(p);
        let input = after.buffers.iter().find(|b| b.name() == "input").unwrap();
        assert_eq!(input.access(), BufferAccess::ReadOnly);
    }

    #[test]
    fn run_preserves_pipeline_live_out_buffer() {
        // A ReadWrite buffer that is live-out must NOT be demoted
        // to workgroup memory  -  callers expect to read it back.
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("result", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(16)
                    .with_pipeline_live_out(true),
            ],
            [64, 1, 1],
            vec![],
        );
        let after = run(p);
        let r = after.buffers.iter().find(|b| b.name() == "result").unwrap();
        assert_eq!(r.access(), BufferAccess::ReadWrite);
        assert!(r.is_pipeline_live_out());
    }

    #[test]
    fn run_is_identity_when_no_candidates() {
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
            ],
            [64, 1, 1],
            vec![],
        );
        let after = run(p);
        assert_eq!(after.buffers.len(), 1);
        assert_eq!(after.buffers[0].access(), BufferAccess::ReadOnly);
    }

    #[test]
    fn run_skips_runtime_sized_buffers() {
        // count=0 means runtime-sized (no `with_count`); workgroup
        // allocations must be static so we can't promote those.
        let p = Program::wrapped(
            vec![BufferDecl::storage(
                "dynamic",
                0,
                BufferAccess::ReadWrite,
                DataType::U32,
            )],
            [64, 1, 1],
            vec![],
        );
        let after = run(p);
        let b = after
            .buffers
            .iter()
            .find(|b| b.name() == "dynamic")
            .unwrap();
        assert_eq!(b.access(), BufferAccess::ReadWrite);
    }

    #[test]
    fn count_opportunities_finds_one_candidate() {
        assert_eq!(count_opportunities(&decoder_like()), 1);
    }

    /// A ReadWrite handoff that exceeds 16 KiB stays in storage memory
    ///  -  wgpu's shared-memory floor would reject the workgroup decl on
    /// the fallback path. 4097 u32 elements = 16388 bytes, just above
    /// the 16384-byte budget.
    #[test]
    fn run_leaves_oversize_handoff_in_storage() {
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(64),
                BufferDecl::storage("decoded", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(4097),
            ],
            [64, 1, 1],
            vec![],
        );
        assert_eq!(count_opportunities(&p), 0);
        let after = run(p);
        let decoded = after
            .buffers
            .iter()
            .find(|b| b.name() == "decoded")
            .unwrap();
        assert_eq!(
            decoded.access(),
            BufferAccess::ReadWrite,
            "oversize handoff must not be promoted; would exceed 16 KiB shared-memory floor"
        );
    }

    /// Twin of the above: a 4096-element buffer (exactly at 16 KiB) is
    /// still promotable.
    #[test]
    fn run_promotes_at_workgroup_byte_ceiling() {
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("decoded", 1, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(4096),
            ],
            [64, 1, 1],
            vec![],
        );
        let after = run(p);
        let decoded = after
            .buffers
            .iter()
            .find(|b| b.name() == "decoded")
            .unwrap();
        assert_eq!(decoded.access(), BufferAccess::Workgroup);
    }

    #[test]
    fn count_opportunities_zero_on_read_only_program() {
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(1),
            ],
            [64, 1, 1],
            vec![],
        );
        assert_eq!(count_opportunities(&p), 0);
    }

    #[test]
    fn candidate_handoffs_exposes_name_and_count() {
        let p = decoder_like();
        let cands = candidate_handoffs(&p);
        assert_eq!(cands.get(&Ident::from("decoded")).copied(), Some(128));
        assert!(!cands.contains_key(&Ident::from("input")));
    }

    #[test]
    fn multiple_candidates_all_surface() {
        let p = Program::wrapped(
            vec![
                BufferDecl::storage("a", 0, BufferAccess::ReadWrite, DataType::U32).with_count(32),
                BufferDecl::storage("b", 1, BufferAccess::ReadWrite, DataType::U32).with_count(64),
                BufferDecl::storage("c", 2, BufferAccess::ReadOnly, DataType::U32).with_count(16),
            ],
            [64, 1, 1],
            vec![],
        );
        let cands = candidate_handoffs(&p);
        assert_eq!(cands.len(), 2);
        assert_eq!(cands.get(&Ident::from("a")).copied(), Some(32));
        assert_eq!(cands.get(&Ident::from("b")).copied(), Some(64));
    }
}
