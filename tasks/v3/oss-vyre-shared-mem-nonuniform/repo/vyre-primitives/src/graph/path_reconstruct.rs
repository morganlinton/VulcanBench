//! `path_reconstruct`  -  walk a parent-pointer array back from a
//! target node, emitting the materialized path into an output
//! buffer.
//!
//! Given:
//! - `parent`: u32 buffer where `parent[v] == u` means `u → v` is
//!   the chosen predecessor edge (and `parent[root] == root` marks
//!   termination).
//! - `target`: u32 buffer whose slot 0 names the node to walk back
//!   from.
//!
//! Emits `path_out[0..len]` = `[target, parent[target], parent[parent[target]], …, root]`
//! and writes the path length into `path_len[0]`. Bounded by
//! `max_depth` so a corrupt parent array cannot hang the GPU.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::path_reconstruct";

/// Canonical batched op id.
pub const BATCHED_OP_ID: &str = "vyre-primitives::graph::batched_path_reconstruct";

/// Workgroup size used by the batched path-reconstruction primitive.
pub const BATCHED_WORKGROUP_SIZE: u32 = 256;
/// Canonical parent input buffer.
pub const PATH_PARENT_BUFFER: &str = "path_reconstruct parent";
/// Canonical single-target input buffer.
pub const PATH_TARGET_BUFFER: &str = "path_reconstruct target";
/// Canonical single-path output buffer.
pub const PATH_OUT_BUFFER: &str = "path_reconstruct path_out";
/// Canonical single-path length output buffer.
pub const PATH_LEN_BUFFER: &str = "path_reconstruct path_len";
/// Canonical batched-target input buffer.
pub const BATCHED_PATH_TARGETS_BUFFER: &str = "batched_path_reconstruct targets";
/// Canonical batched-path output buffer.
pub const BATCHED_PATHS_BUFFER: &str = "batched_path_reconstruct paths";
/// Canonical batched-length output buffer.
pub const BATCHED_LENS_BUFFER: &str = "batched_path_reconstruct lens";
/// Single-lane path-reconstruction dispatch grid.
pub const PATH_RECONSTRUCT_DISPATCH_GRID: [u32; 3] = [1, 1, 1];

/// Validated batched path-reconstruction buffer layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchedPathReconstructLayout {
    /// Number of target nodes in the batch, narrowed for GPU dispatch lanes.
    pub target_count: u32,
    /// Total number of u32 path output words.
    pub path_words: usize,
    /// Total number of u32 path output words, narrowed for primitive buffer metadata.
    pub path_words_u32: u32,
}

/// Primitive-owned dispatch plan for one path reconstruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathReconstructDispatchPlan {
    /// Number of parent entries supplied by the caller.
    pub parent_words: usize,
    /// Number of words in the target buffer.
    pub target_words: usize,
    /// Number of words in the padded path output.
    pub path_words: usize,
    /// Number of words in the length output.
    pub len_words: usize,
    /// Maximum path depth accepted by this dispatch.
    pub max_depth: u32,
    /// Dispatch grid override.
    pub grid: [u32; 3],
}

impl PathReconstructDispatchPlan {
    /// Build the single-target path-reconstruction program for this plan.
    #[must_use]
    pub fn program(&self) -> Program {
        path_reconstruct(
            PATH_PARENT_BUFFER,
            PATH_TARGET_BUFFER,
            PATH_OUT_BUFFER,
            PATH_LEN_BUFFER,
            self.max_depth,
        )
    }

    /// Return the primitive-owned cache identity for this plan's static parent input.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the parent slice no longer matches
    /// the validated single-target dispatch plan.
    pub fn static_input_key(
        &self,
        parent: &[u32],
    ) -> Result<PathReconstructStaticInputKey, String> {
        if parent.len() != self.parent_words {
            return Err(format!(
                "Fix: path_reconstruct static key expected {} parent word(s), got {}.",
                self.parent_words,
                parent.len()
            ));
        }
        Ok(PathReconstructStaticInputKey {
            parent_words: self.parent_words,
            parent_hash: path_reconstruct_u32_slice_fingerprint(parent),
            target_count: 1,
            max_depth: self.max_depth,
            batched: false,
        })
    }
}

/// Primitive-owned dispatch plan for batched path reconstruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchedPathReconstructDispatchPlan {
    /// Validated batched output layout.
    pub layout: BatchedPathReconstructLayout,
    /// Number of parent entries supplied by the caller.
    pub parent_words: usize,
    /// Number of words in the targets input.
    pub target_words: usize,
    /// Number of words in the padded paths output.
    pub path_words: usize,
    /// Number of words in the lengths output.
    pub len_words: usize,
    /// Maximum path depth accepted by this dispatch.
    pub max_depth: u32,
    /// Dispatch grid override.
    pub grid: [u32; 3],
}

/// Primitive-owned identity for reusable path-reconstruction static inputs.
///
/// Dispatch wrappers use this key to decide whether the parent-pointer array
/// can remain staged across calls. Keeping the fingerprint beside the primitive
/// plan prevents every wrapper from forking parent-vector cache semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PathReconstructStaticInputKey {
    /// Number of parent entries supplied by the caller.
    pub parent_words: usize,
    /// Stable content fingerprint for the parent-pointer array.
    pub parent_hash: u64,
    /// Number of target nodes in this dispatch.
    pub target_count: u32,
    /// Maximum path depth accepted by this dispatch.
    pub max_depth: u32,
    /// True when this key describes a batched dispatch.
    pub batched: bool,
}

impl BatchedPathReconstructDispatchPlan {
    /// Build the batched path-reconstruction program for this plan.
    #[must_use]
    pub fn program(&self) -> Program {
        batched_path_reconstruct(self.layout.target_count, self.max_depth)
    }

    /// Return the primitive-owned cache identity for this plan's static parent input.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the parent slice no longer matches
    /// the validated batched dispatch plan.
    pub fn static_input_key(
        &self,
        parent: &[u32],
    ) -> Result<PathReconstructStaticInputKey, String> {
        if parent.len() != self.parent_words {
            return Err(format!(
                "Fix: batched_path_reconstruct static key expected {} parent word(s), got {}.",
                self.parent_words,
                parent.len()
            ));
        }
        Ok(PathReconstructStaticInputKey {
            parent_words: self.parent_words,
            parent_hash: path_reconstruct_u32_slice_fingerprint(parent),
            target_count: self.layout.target_count,
            max_depth: self.max_depth,
            batched: true,
        })
    }
}

fn path_reconstruct_u32_slice_fingerprint(values: &[u32]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in (values.len() as u64).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for value in values {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Build the IR `Program` for path reconstruction.
#[must_use]
///
pub fn path_reconstruct(
    parent: &str,
    target: &str,
    path_out: &str,
    path_len: &str,
    max_depth: u32,
) -> Program {
    if max_depth == 0 {
        return crate::invalid_output_program(
            OP_ID,
            path_out,
            DataType::U32,
            "Fix: path_reconstruct max_depth must be >= 1.".to_string(),
        );
    }
    // Single-threaded walk (invocation 0 owns the chain). The work
    // is O(path_length) which is typically small (stack trace length,
    // tiny CFG path), so parallelism is not meaningful here.
    //
    // AUDIT_2026-04-24 F-PR-01: two divergences from cpu_ref fixed
    // here.
    //   (1) Prior code overloaded `len` as both the path-length
    //       counter and the loop-termination signal (setting
    //       `len = max_depth` on root-hit), so the stored
    //       `path_len[0]` reported `max_depth` instead of the true
    //       path length whenever a root was reached before the cap.
    //       Now uses a separate `done` flag; `len` stays truthful.
    //   (2) Prior code left `path_out[len..max_depth]` uninitialized
    //       while cpu_ref explicitly pads that tail with zeros, so
    //       harness byte-compare diverged unless the dispatcher
    //       zeroed the buffer between runs. IR now writes 0 into
    //       the unused tail slots on early termination.
    let body = vec![
        Node::let_bind("current", Expr::load(target, Expr::u32(0))),
        Node::let_bind("len", Expr::u32(0)),
        Node::let_bind("done", Expr::u32(0)),
        Node::loop_for(
            "step",
            Expr::u32(0),
            Expr::u32(max_depth),
            vec![Node::if_then(
                Expr::eq(Expr::var("done"), Expr::u32(0)),
                vec![
                    Node::store(path_out, Expr::var("len"), Expr::var("current")),
                    Node::assign("len", Expr::add(Expr::var("len"), Expr::u32(1))),
                    Node::let_bind(
                        "next",
                        Expr::select(
                            Expr::lt(Expr::var("current"), Expr::buf_len(parent)),
                            Expr::load(parent, Expr::var("current")),
                            Expr::var("current"),
                        ),
                    ),
                    Node::if_then(
                        Expr::eq(Expr::var("next"), Expr::var("current")),
                        vec![Node::assign("done", Expr::u32(1))],
                    ),
                    Node::assign("current", Expr::var("next")),
                ],
            )],
        ),
        // Zero-fill path_out[len..max_depth] so harness byte-compare
        // matches cpu_ref tail-padding convention.
        Node::loop_for(
            "pad",
            Expr::var("len"),
            Expr::u32(max_depth),
            vec![Node::store(path_out, Expr::var("pad"), Expr::u32(0))],
        ),
        Node::store(path_len, Expr::u32(0), Expr::var("len")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(parent, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(target, 1, BufferAccess::ReadOnly, DataType::U32).with_count(1),
            BufferDecl::storage(path_out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(max_depth),
            BufferDecl::storage(path_len, 3, BufferAccess::ReadWrite, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                body,
            )]),
        }],
    )
}

/// Build the IR `Program` for batched path reconstruction.
///
/// Each invocation reconstructs one target's parent chain and writes
/// a `max_depth`-padded slice at `paths[target_index * max_depth..]`.
/// `lens[target_index]` receives the valid path length before padding.
///
/// # Contract
///
/// The caller supplies:
/// - `parent`: parent-pointer array.
/// - `targets`: `target_count` target nodes.
/// - `paths`: `target_count * max_depth` u32 slots.
/// - `lens`: `target_count` u32 slots.
///
/// `max_depth == 0` or `target_count * max_depth` overflow produces a trap
/// program rather than silently emitting malformed buffer metadata.
#[must_use]
pub fn batched_path_reconstruct(target_count: u32, max_depth: u32) -> Program {
    let layout = match validate_batched_path_reconstruct_layout(target_count as usize, max_depth) {
        Ok(layout) => layout,
        Err(error) => {
            return crate::invalid_output_program(BATCHED_OP_ID, "paths", DataType::U32, error);
        }
    };
    let path_words = layout.path_words_u32;

    let body = vec![
        Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::u32(target_count)),
            vec![
                Node::let_bind("base", Expr::mul(Expr::var("idx"), Expr::u32(max_depth))),
                Node::let_bind("current", Expr::load("targets", Expr::var("idx"))),
                Node::let_bind("len", Expr::u32(0)),
                Node::let_bind("done", Expr::u32(0)),
                Node::loop_for(
                    "step",
                    Expr::u32(0),
                    Expr::u32(max_depth),
                    vec![Node::if_then(
                        Expr::eq(Expr::var("done"), Expr::u32(0)),
                        vec![
                            Node::store(
                                "paths",
                                Expr::add(Expr::var("base"), Expr::var("len")),
                                Expr::var("current"),
                            ),
                            Node::assign("len", Expr::add(Expr::var("len"), Expr::u32(1))),
                            Node::let_bind(
                                "next",
                                Expr::select(
                                    Expr::lt(Expr::var("current"), Expr::buf_len("parent")),
                                    Expr::load("parent", Expr::var("current")),
                                    Expr::var("current"),
                                ),
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("next"), Expr::var("current")),
                                vec![Node::assign("done", Expr::u32(1))],
                            ),
                            Node::assign("current", Expr::var("next")),
                        ],
                    )],
                ),
                Node::loop_for(
                    "pad",
                    Expr::var("len"),
                    Expr::u32(max_depth),
                    vec![Node::store(
                        "paths",
                        Expr::add(Expr::var("base"), Expr::var("pad")),
                        Expr::u32(0),
                    )],
                ),
                Node::store("lens", Expr::var("idx"), Expr::var("len")),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage("parent", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage("targets", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(target_count),
            BufferDecl::storage("paths", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(path_words),
            BufferDecl::storage("lens", 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(target_count),
        ],
        [BATCHED_WORKGROUP_SIZE, 1, 1],
        vec![Node::Region {
            generator: Ident::from(BATCHED_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Validate batched path-reconstruction output layout.
///
/// # Errors
///
/// Returns an actionable diagnostic when `max_depth` is zero, target count
/// exceeds the primitive's u32 lane space, or `target_count * max_depth`
/// cannot be represented by primitive buffer metadata.
pub fn validate_batched_path_reconstruct_layout(
    target_len: usize,
    max_depth: u32,
) -> Result<BatchedPathReconstructLayout, String> {
    if max_depth == 0 {
        return Err("Fix: batched_path_reconstruct max_depth must be >= 1.".to_string());
    }
    let target_count = u32::try_from(target_len).map_err(|_| {
        format!(
            "Fix: batched_path_reconstruct target count {target_len} exceeds the primitive u32 lane limit."
        )
    })?;
    let path_words_u32 = target_count.checked_mul(max_depth).ok_or_else(|| {
        format!(
            "Fix: batched_path_reconstruct target_count*max_depth overflows u32 for target_count={target_count}, max_depth={max_depth}."
        )
    })?;
    let path_words = usize::try_from(path_words_u32).map_err(|_| {
        format!("Fix: batched_path_reconstruct path word count {path_words_u32} exceeds usize.")
    })?;
    Ok(BatchedPathReconstructLayout {
        target_count,
        path_words,
        path_words_u32,
    })
}

/// Validate a single-target reconstruction dispatch and return its buffer plan.
pub fn plan_path_reconstruct_dispatch(
    parent_len: usize,
    max_depth: u32,
) -> Result<PathReconstructDispatchPlan, String> {
    if max_depth == 0 {
        return Err("Fix: path_reconstruct max_depth must be >= 1.".to_string());
    }
    Ok(PathReconstructDispatchPlan {
        parent_words: parent_len,
        target_words: 1,
        path_words: max_depth as usize,
        len_words: 1,
        max_depth,
        grid: PATH_RECONSTRUCT_DISPATCH_GRID,
    })
}

/// Validate a batched path-reconstruction dispatch and return its buffer plan.

pub fn plan_batched_path_reconstruct_dispatch(
    parent_len: usize,
    target_len: usize,
    max_depth: u32,
) -> Result<BatchedPathReconstructDispatchPlan, String> {
    let layout = validate_batched_path_reconstruct_layout(target_len, max_depth)?;
    Ok(BatchedPathReconstructDispatchPlan {
        parent_words: parent_len,
        target_words: target_len,
        path_words: layout.path_words,
        len_words: target_len,
        max_depth,
        grid: [
            ceil_div_u32(layout.target_count, BATCHED_WORKGROUP_SIZE),
            1,
            1,
        ],
        layout,
    })
}

fn ceil_div_u32(value: u32, divisor: u32) -> u32 {
    if value == 0 {
        0
    } else {
        ((value - 1) / divisor) + 1
    }
}

/// Validate the length word returned by a single-target path-reconstruction dispatch.
///
/// Backend wrappers use this after readback so malformed GPU output is checked
/// by the primitive owner rather than by each consumer.
pub fn validate_path_reconstruct_readback(
    plan: &PathReconstructDispatchPlan,
    len: u32,
) -> Result<usize, String> {
    let len_usize = usize::try_from(len).map_err(|_| {
        format!("Fix: path_reconstruct returned length {len}, which cannot fit this host usize.")
    })?;
    if len_usize > plan.path_words {
        return Err(format!(
            "Fix: path_reconstruct returned length {len}, exceeding max_depth {}. Treat this as malformed GPU readback or a backend bug.",
            plan.max_depth
        ));
    }
    Ok(len_usize)
}

/// Validate output-buffer shapes and length words returned by a batched
/// path-reconstruction dispatch.
///
/// This is deliberately primitive-owned: dispatch wrappers may stage buffers
/// differently, but the post-dispatch correctness envelope is part of the
/// graph primitive contract.
pub fn validate_batched_path_reconstruct_readback(
    plan: &BatchedPathReconstructDispatchPlan,
    path_words: usize,
    len_words: usize,
    lens: &[u32],
) -> Result<(), String> {
    if path_words != plan.path_words || len_words != plan.len_words {
        return Err(format!(
            "Fix: batched_path_reconstruct returned {path_words} path word(s) and {len_words} len word(s), expected {} and {}.",
            plan.path_words, plan.len_words
        ));
    }
    for (target_index, &len) in lens.iter().enumerate() {
        let len_usize = usize::try_from(len).map_err(|_| {
            format!(
                "Fix: batched_path_reconstruct target {target_index} returned length {len}, which cannot fit this host usize."
            )
        })?;
        if len_usize > plan.max_depth as usize {
            return Err(format!(
                "Fix: batched_path_reconstruct target {target_index} returned length {len}, exceeding max_depth {}. Treat this as malformed GPU readback or a backend bug.",
                plan.max_depth
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod dispatch_plan_tests {
    use super::*;

    #[test]
    fn single_path_dispatch_plan_owns_outputs_and_grid() {
        let plan = plan_path_reconstruct_dispatch(4, 8)
            .expect("Fix: nonzero max_depth should plan single reconstruction");

        assert_eq!(plan.parent_words, 4);
        assert_eq!(plan.target_words, 1);
        assert_eq!(plan.path_words, 8);
        assert_eq!(plan.len_words, 1);
        assert_eq!(plan.grid, PATH_RECONSTRUCT_DISPATCH_GRID);
    }

    #[test]
    fn single_path_dispatch_plan_rejects_zero_depth() {
        let err = plan_path_reconstruct_dispatch(4, 0).unwrap_err();
        assert!(err.contains("max_depth"));
    }

    #[test]
    fn batched_path_dispatch_plan_owns_layout_and_grid() {
        let plan = plan_batched_path_reconstruct_dispatch(4, 513, 3)
            .expect("Fix: valid batched reconstruction should plan");

        assert_eq!(plan.parent_words, 4);
        assert_eq!(plan.target_words, 513);
        assert_eq!(plan.path_words, 1539);
        assert_eq!(plan.len_words, 513);
        assert_eq!(plan.grid, [3, 1, 1]);
        assert_eq!(plan.layout.target_count, 513);
    }

    #[test]
    fn static_input_key_tracks_parent_content_and_dispatch_shape() {
        let single = plan_path_reconstruct_dispatch(4, 8)
            .expect("Fix: nonzero max_depth should plan single reconstruction");
        let batched = plan_batched_path_reconstruct_dispatch(4, 2, 8)
            .expect("Fix: valid batched reconstruction should plan");

        let first = single
            .static_input_key(&[0, 0, 1, 2])
            .expect("Fix: matching parent slice should key");
        let same = single
            .static_input_key(&[0, 0, 1, 2])
            .expect("Fix: matching parent slice should key");
        let changed = single
            .static_input_key(&[0, 0, 0, 2])
            .expect("Fix: same-shape parent content should key");
        let batched_key = batched
            .static_input_key(&[0, 0, 1, 2])
            .expect("Fix: matching batched parent slice should key");

        assert_eq!(first, same);
        assert_ne!(first, changed);
        assert_ne!(first, batched_key);
        assert_eq!(first.parent_words, 4);
        assert_eq!(first.target_count, 1);
        assert!(!first.batched);
        assert_eq!(batched_key.target_count, 2);
        assert!(batched_key.batched);
    }

    #[test]
    fn static_input_key_rejects_parent_length_drift() {
        let single = plan_path_reconstruct_dispatch(4, 8)
            .expect("Fix: nonzero max_depth should plan single reconstruction");
        let batched = plan_batched_path_reconstruct_dispatch(4, 2, 8)
            .expect("Fix: valid batched reconstruction should plan");

        let err = single.static_input_key(&[0, 0, 1]).unwrap_err();
        assert!(err.contains("expected 4 parent word"));

        let err = batched.static_input_key(&[0, 0, 1]).unwrap_err();
        assert!(err.contains("expected 4 parent word"));
    }

    #[test]
    fn single_path_readback_validation_rejects_impossible_len() {
        let plan = plan_path_reconstruct_dispatch(4, 4)
            .expect("Fix: nonzero max_depth should plan single reconstruction");

        assert_eq!(validate_path_reconstruct_readback(&plan, 4), Ok(4));

        let err = validate_path_reconstruct_readback(&plan, 5).unwrap_err();
        assert!(err.contains("exceeding max_depth 4"));
    }

    #[test]
    fn batched_path_readback_validation_rejects_shape_and_len_drift() {
        let plan = plan_batched_path_reconstruct_dispatch(4, 2, 4)
            .expect("Fix: valid batched reconstruction should plan");

        validate_batched_path_reconstruct_readback(&plan, 8, 2, &[4, 1]).unwrap();

        let err = validate_batched_path_reconstruct_readback(&plan, 7, 2, &[4, 1]).unwrap_err();
        assert!(err.contains("expected 8 and 2"));

        let err = validate_batched_path_reconstruct_readback(&plan, 8, 2, &[4, 5]).unwrap_err();
        assert!(err.contains("target 1"));
        assert!(err.contains("exceeding max_depth 4"));
    }
}

/// CPU reference: walks parent pointers up to `max_depth`, writing
/// the materialized path into `scratch` and returning its length.
/// Early-terminates when a node's parent points at itself (root
/// convention).
///
/// # Performance
///
/// Callers doing many reconstructions (e.g. one per node in a deep
/// call graph) should reuse one scratch vector across calls to avoid
/// an allocation per walk.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(parent: &[u32], target: u32, max_depth: u32, scratch: &mut Vec<u32>) -> u32 {
    scratch.clear();
    let mut current = target;
    let mut len = 0u32;
    let cap = max_depth as usize;
    while (len as usize) < cap {
        scratch.push(current);
        len += 1;
        let next = parent.get(current as usize).copied().unwrap_or(current);
        if next == current {
            break;
        }
        current = next;
    }
    while scratch.len() < cap {
        scratch.push(0);
    }
    len
}

/// Fallible CPU reference for the batched path-reconstruction contract.
///
/// This is the allocation-safe entry point for wrappers that run CPU parity on
/// hostile batch dimensions. The legacy `cpu_ref_batched` wrapper delegates here
/// to preserve the existing call surface while avoiding infallible reservations.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_batched(
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
    paths: &mut Vec<u32>,
    lens: &mut Vec<u32>,
) -> Result<(), String> {
    let mut scratch = Vec::new();
    try_cpu_ref_batched_with_scratch(parent, targets, max_depth, paths, lens, &mut scratch)
}

/// Fallible batched CPU reference using caller-owned output and per-target scratch storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_batched_with_scratch(
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
    paths: &mut Vec<u32>,
    lens: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
) -> Result<(), String> {
    let layout = validate_batched_path_reconstruct_layout(targets.len(), max_depth)?;
    let depth = max_depth as usize;
    scratch.clear();
    crate::graph::scratch::reserve_graph_items(
        paths,
        layout.path_words,
        "path reconstruction CPU oracle",
        "batched path output",
    )?;
    crate::graph::scratch::reserve_graph_items(
        lens,
        layout.target_count as usize,
        "path reconstruction CPU oracle",
        "batched length output",
    )?;
    crate::graph::scratch::reserve_graph_items(
        scratch,
        depth,
        "path reconstruction CPU oracle",
        "per-target path scratch",
    )?;
    paths.clear();
    lens.clear();
    for &target in targets {
        let len = cpu_ref(parent, target, max_depth, scratch);
        paths.extend_from_slice(&scratch);
        lens.push(len);
    }
    Ok(())
}

/// CPU reference for the batched path-reconstruction contract.
///
/// `paths` is rewritten to `targets.len() * max_depth` entries, where each
/// target owns one zero-padded segment. `lens` is rewritten to one valid
/// length per target.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_batched(
    parent: &[u32],
    targets: &[u32],
    max_depth: u32,
    paths: &mut Vec<u32>,
    lens: &mut Vec<u32>,
) {
    try_cpu_ref_batched(parent, targets, max_depth, paths, lens)
        .expect("Fix: batched path reconstruction CPU oracle allocation failed");
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || path_reconstruct("parent", "target", "path_out", "path_len", 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            // parent: [0, 0, 1, 2]  (0 is root; 1→0, 2→1, 3→2)
            // target = 3
            // expected path = [3, 2, 1, 0], len = 4
            vec![vec![
                to_bytes(&[0, 0, 1, 2]),
                to_bytes(&[3]),
                to_bytes(&[0, 0, 0, 0]),
                to_bytes(&[0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[3, 2, 1, 0]),
                to_bytes(&[4]),
            ]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_parent_chain_to_root() {
        let mut scratch = Vec::with_capacity(4);
        let len = cpu_ref(&[0, 0, 1, 2], 3, 4, &mut scratch);
        assert_eq!(len, 4);
        assert_eq!(&scratch[0..4], &[3, 2, 1, 0]);
    }

    #[test]
    fn terminates_on_max_depth() {
        // Cycle: 0 → 1 → 0. Without max_depth we'd loop forever.
        // AUDIT_2026-04-24 F-PR-02: also assert path contents so a
        // silent buffer corruption cannot slip past the test.
        let mut scratch = Vec::with_capacity(8);
        let len = cpu_ref(&[1, 0], 0, 8, &mut scratch);
        assert_eq!(len, 8);
        assert_eq!(&scratch[..], &[0, 1, 0, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn tail_is_zero_padded_when_root_reached_before_cap() {
        // AUDIT_2026-04-24 F-PR-01: cpu_ref must zero-fill the tail
        // beyond the materialized path so harness byte-compare with
        // the IR builder stays stable.
        let mut scratch = Vec::with_capacity(8);
        let len = cpu_ref(&[0, 0, 1, 2], 3, 8, &mut scratch);
        assert_eq!(len, 4);
        assert_eq!(&scratch[..4], &[3, 2, 1, 0]);
        assert_eq!(&scratch[4..], &[0, 0, 0, 0]);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures  -  self-loops, deep chain, OOB target, max_depth.
    // ------------------------------------------------------------------

    #[test]
    fn parent_self_loops_terminate_immediately() {
        // parent[0]=0, parent[1]=1. target=1 → path [1].
        let mut scratch = Vec::with_capacity(4);
        let len = cpu_ref(&[0, 1], 1, 4, &mut scratch);
        assert_eq!(len, 1);
        assert_eq!(scratch[0], 1);
        assert_eq!(&scratch[1..], &[0, 0, 0]);
    }

    #[test]
    fn deep_chain_within_max_depth() {
        // 0 ← 1 ← 2 ← 3 ← 4
        let parent = &[0, 0, 1, 2, 3];
        let mut scratch = Vec::with_capacity(8);
        let len = cpu_ref(parent, 4, 8, &mut scratch);
        assert_eq!(len, 5);
        assert_eq!(&scratch[..5], &[4, 3, 2, 1, 0]);
        assert_eq!(&scratch[5..], &[0, 0, 0]);
    }

    #[test]
    fn target_not_in_parent_array_terminates_at_target() {
        // parent has 3 entries. target=5 is OOB.
        let mut scratch = Vec::with_capacity(4);
        let len = cpu_ref(&[0, 0, 1], 5, 4, &mut scratch);
        assert_eq!(len, 1);
        assert_eq!(scratch[0], 5);
        assert_eq!(&scratch[1..], &[0, 0, 0]);
    }

    #[test]
    fn max_depth_zero_returns_empty_path() {
        let mut scratch = Vec::with_capacity(4);
        let len = cpu_ref(&[0, 0, 1, 2], 3, 0, &mut scratch);
        assert_eq!(len, 0);
        assert!(scratch.is_empty());
    }

    #[test]
    fn max_depth_one_returns_only_target() {
        let mut scratch = Vec::with_capacity(4);
        let len = cpu_ref(&[0, 0, 1, 2], 3, 1, &mut scratch);
        assert_eq!(len, 1);
        assert_eq!(scratch[0], 3);
        assert_eq!(scratch.len(), 1, "cap == max_depth == 1, no padding needed");
    }

    #[test]
    fn program_builder_max_depth_zero_emits_trap() {
        let p = path_reconstruct("parent", "target", "out", "len", 0);
        // Trap programs contain a Node::Trap in the entry region body.
        let entry = p.entry();
        let has_trap = entry.iter().any(|n| {
            if let Node::Region { body, .. } = n {
                body.iter().any(|inner| matches!(inner, Node::Trap { .. }))
            } else {
                matches!(n, Node::Trap { .. })
            }
        });
        assert!(
            has_trap,
            "max_depth == 0 must produce a trap program, not panic"
        );
    }

    #[test]
    fn batched_program_has_expected_buffers_and_workgroup() {
        let p = batched_path_reconstruct(3, 4);
        assert_eq!(p.workgroup_size, [BATCHED_WORKGROUP_SIZE, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["parent", "targets", "paths", "lens"]);
        assert_eq!(p.buffers[1].count(), 3);
        assert_eq!(p.buffers[2].count(), 12);
        assert_eq!(p.buffers[3].count(), 3);
    }

    #[test]
    fn batched_layout_validator_accepts_empty_and_canonical_batches() {
        assert_eq!(
            validate_batched_path_reconstruct_layout(0, 4).unwrap(),
            BatchedPathReconstructLayout {
                target_count: 0,
                path_words: 0,
                path_words_u32: 0,
            }
        );
        assert_eq!(
            validate_batched_path_reconstruct_layout(3, 4).unwrap(),
            BatchedPathReconstructLayout {
                target_count: 3,
                path_words: 12,
                path_words_u32: 12,
            }
        );
    }

    #[test]
    fn batched_layout_validator_rejects_zero_depth_and_overflow() {
        let err = validate_batched_path_reconstruct_layout(3, 0).unwrap_err();
        assert!(err.contains("max_depth must be >= 1"));

        let err = validate_batched_path_reconstruct_layout(u32::MAX as usize + 1, 1).unwrap_err();
        assert!(err.contains("target count"));

        let err = validate_batched_path_reconstruct_layout(u32::MAX as usize, 2).unwrap_err();
        assert!(err.contains("target_count*max_depth"));
    }

    #[test]
    fn batched_cpu_ref_matches_single_target_segments() {
        let mut paths = Vec::new();
        let mut lens = Vec::new();
        cpu_ref_batched(&[0, 0, 1, 2], &[3, 0, 2], 4, &mut paths, &mut lens);
        assert_eq!(lens, vec![4, 1, 3]);
        assert_eq!(&paths[0..4], &[3, 2, 1, 0]);
        assert_eq!(&paths[4..8], &[0, 0, 0, 0]);
        assert_eq!(&paths[8..12], &[2, 1, 0, 0]);
    }

    #[test]
    fn batched_cpu_ref_with_scratch_reuses_all_storage() {
        let mut paths = Vec::with_capacity(32);
        let mut lens = Vec::with_capacity(8);
        let mut scratch = Vec::with_capacity(8);
        paths.extend_from_slice(&[0xDEAD_BEEF; 5]);
        lens.extend_from_slice(&[0xCAFE_BABE; 3]);
        scratch.extend_from_slice(&[0xFEED_FACE; 6]);
        let paths_capacity = paths.capacity();
        let lens_capacity = lens.capacity();
        let scratch_capacity = scratch.capacity();

        try_cpu_ref_batched_with_scratch(
            &[0, 0, 1, 2],
            &[3, 0, 2],
            4,
            &mut paths,
            &mut lens,
            &mut scratch,
        )
        .expect("Fix: valid batched path reconstruction must evaluate.");

        assert_eq!(lens, vec![4, 1, 3]);
        assert_eq!(&paths[0..4], &[3, 2, 1, 0]);
        assert_eq!(&paths[4..8], &[0, 0, 0, 0]);
        assert_eq!(&paths[8..12], &[2, 1, 0, 0]);
        assert_eq!(scratch, vec![2, 1, 0, 0]);
        assert_eq!(paths.capacity(), paths_capacity);
        assert_eq!(lens.capacity(), lens_capacity);
        assert_eq!(scratch.capacity(), scratch_capacity);

        try_cpu_ref_batched_with_scratch(
            &[0, 0, 1, 2],
            &[1],
            2,
            &mut paths,
            &mut lens,
            &mut scratch,
        )
        .expect("Fix: second valid batch must reuse and truncate buffers.");

        assert_eq!(lens, vec![2]);
        assert_eq!(paths, vec![1, 0]);
        assert_eq!(scratch, vec![1, 0]);
        assert_eq!(paths.capacity(), paths_capacity);
        assert_eq!(lens.capacity(), lens_capacity);
        assert_eq!(scratch.capacity(), scratch_capacity);
    }

    #[test]
    fn batched_cpu_ref_rejects_zero_depth_like_dispatch_planner() {
        let mut paths = vec![0xDEAD_BEEF];
        let mut lens = vec![0xCAFE_BABE];
        let err = try_cpu_ref_batched(&[0], &[0], 0, &mut paths, &mut lens).unwrap_err();

        assert!(err.contains("max_depth must be >= 1"));
        assert_eq!(paths, vec![0xDEAD_BEEF]);
        assert_eq!(lens, vec![0xCAFE_BABE]);
    }

    #[test]
    fn generated_batched_cpu_ref_matches_single_target_oracle_shapes() {
        for target_count in 0usize..64 {
            for depth in 1u32..65 {
                let parent: Vec<u32> = (0..128u32)
                    .map(|node| if node == 0 { 0 } else { node - 1 })
                    .collect();
                let targets: Vec<u32> = (0..target_count)
                    .map(|index| ((index * 17 + depth as usize * 3) % parent.len()) as u32)
                    .collect();
                let mut paths = vec![0xDEAD_BEEFu32; 3];
                let mut lens = vec![0xCAFE_BABEu32; 2];
                try_cpu_ref_batched(&parent, &targets, depth, &mut paths, &mut lens).unwrap();
                assert_eq!(lens.len(), targets.len());
                assert_eq!(paths.len(), targets.len() * depth as usize);
                let mut single = Vec::new();
                for (target_index, &target) in targets.iter().enumerate() {
                    let expected_len = cpu_ref(&parent, target, depth, &mut single);
                    assert_eq!(lens[target_index], expected_len);
                    let start = target_index * depth as usize;
                    let end = start + depth as usize;
                    assert_eq!(&paths[start..end], &single[..]);
                }
            }
        }
    }

    #[test]
    fn batched_program_zero_depth_emits_trap() {
        let p = batched_path_reconstruct(3, 0);
        let entry = p.entry();
        let has_trap = entry.iter().any(|n| {
            if let Node::Region { body, .. } = n {
                body.iter().any(|inner| matches!(inner, Node::Trap { .. }))
            } else {
                matches!(n, Node::Trap { .. })
            }
        });
        assert!(has_trap, "zero-depth batched path reconstruction must trap");
    }
}
