use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashMap;

use crate::ir_inner::model::node::Node;

use super::BufferDecl;

/// A complete vyre program.
///
/// Contains everything needed to execute a GPU compute dispatch:
/// buffer declarations, workgroup configuration, and the entry point body.
///
/// # Example
///
/// A program that XORs two input buffers element-wise:
///
/// ```rust
/// use vyre::ir::{Program, BufferDecl, BufferAccess, DataType, Node, Expr, BinOp};
///
/// let program = Program::wrapped(
///     vec![
///         BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32),
///         BufferDecl::storage("b", 1, BufferAccess::ReadOnly, DataType::U32),
///         BufferDecl::storage("out", 2, BufferAccess::ReadWrite, DataType::U32),
///     ],
///     [64, 1, 1],
///     vec![
///         Node::let_bind("idx", Expr::gid_x()),
///         Node::if_then(
///             Expr::lt(Expr::var("idx"), Expr::buf_len("out")),
///             vec![
///                 Node::store("out", Expr::var("idx"),
///                     Expr::bitxor(
///                         Expr::load("a", Expr::var("idx")),
///                         Expr::load("b", Expr::var("idx")),
///                     ),
///                 ),
///             ],
///         ),
///     ],
/// );
/// assert_eq!(program.buffers().len(), 3);
/// ```
#[derive(Debug)]
pub struct Program {
    /// Stable ID of the certified operation this program implements.
    ///
    /// Runtime lowering must reject programs without an ID because anonymous IR
    /// cannot be tied back to a conform registry entry.
    pub entry_op_id: Option<String>,
    /// Buffer declarations. Each declares a named, typed, bound memory region.
    pub buffers: Arc<[BufferDecl]>,
    /// Sidecar index for O(1) buffer lookup by name.
    pub(crate) buffer_index: Arc<FxHashMap<Arc<str>, usize>>,
    /// Workgroup size: `[x, y, z]`. Controls `@workgroup_size` in target-text.
    pub workgroup_size: [u32; 3],
    /// Entry point body. Executes once per invocation.
    pub entry: Arc<Vec<Node>>,
    /// Cached blake3 hash of the program for fast equality and cache lookups.
    pub(crate) hash: OnceLock<blake3::Hash>,
    /// Per-backend validation cache (lazily initialized  -  most intermediate
    /// programs created during fixpoint iteration are never validated, so the
    /// sharded DashSet is only allocated on first `mark_validated_on` call).
    #[doc(hidden)]
    pub(crate) validation_set: OnceLock<Arc<dashmap::DashSet<Arc<str>>>>,
    pub(crate) structural_validated: AtomicBool,
    pub(crate) structural_validation_fingerprint: AtomicU64,
    pub(crate) mutation_provenance: AtomicU8,
    pub(crate) fingerprint: OnceLock<[u8; 32]>,
    // VYRE_IR_HOTSPOTS HIGH (core.rs:100-117): both caches were
    // plain values, so `Program::clone` copied the whole Vec / whole
    // ProgramStats by value. Wrapping them in Arc turns the clone
    // into a refcount bump, keeping Program::clone O(1) on every
    // field.
    pub(crate) output_buffer_index: OnceLock<Arc<Vec<u32>>>,
    pub(crate) has_indirect_dispatch: OnceLock<bool>,
    /// Cached statistics computed from a single walk of the program.
    ///
    /// This is a transient cache: it is not serialized to wire format and is
    /// invalidated whenever the program shape mutates.
    pub(crate) stats: OnceLock<Arc<super::ProgramStats>>,
    /// When true, this program must not be fused with another copy of itself
    /// in the same megakernel. Parser programs that use workgroup-local scratch
    /// buffers set this to avoid state corruption when two invocations share
    /// the same workgroup memory.
    pub non_composable_with_self: bool,
}

impl Default for Program {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl Clone for Program {
    fn clone(&self) -> Self {
        let cloned = Self {
            entry_op_id: self.entry_op_id.clone(),
            buffers: Arc::clone(&self.buffers),
            buffer_index: Arc::clone(&self.buffer_index),
            workgroup_size: self.workgroup_size,
            entry: Arc::clone(&self.entry),
            hash: OnceLock::new(),
            validation_set: {
                let cell = OnceLock::new();
                if let Some(set) = self.validation_set.get() {
                    let _ = cell.set(Arc::clone(set));
                }
                cell
            },
            structural_validated: AtomicBool::new(self.is_structurally_validated()),
            structural_validation_fingerprint: AtomicU64::new(
                self.structural_validation_fingerprint
                    .load(Ordering::Acquire),
            ),
            mutation_provenance: AtomicU8::new(self.mutation_provenance.load(Ordering::Acquire)),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self: self.non_composable_with_self,
        };
        // Each OnceLock above was just initialised with `OnceLock::new()`,
        // so the matching `.set(...)` is infallible by construction  -
        // `let _ = ...set(...)` matches the same pattern used for the
        // validation_set initialiser above and avoids the raw-expect
        // CI gate without introducing fake error paths.
        if let Some(hash) = self.hash.get() {
            let _ = cloned.hash.set(*hash);
        }
        if let Some(fingerprint) = self.fingerprint.get() {
            let _ = cloned.fingerprint.set(*fingerprint);
        }
        if let Some(output_buffer_index) = self.output_buffer_index.get() {
            // Arc::clone = refcount bump, no Vec<u32> copy.
            let _ = cloned
                .output_buffer_index
                .set(Arc::clone(output_buffer_index));
        }
        if let Some(has_indirect_dispatch) = self.has_indirect_dispatch.get() {
            let _ = cloned.has_indirect_dispatch.set(*has_indirect_dispatch);
        }
        if let Some(stats) = self.stats.get() {
            // Arc::clone = refcount bump, no ProgramStats copy.
            let _ = cloned.stats.set(Arc::clone(stats));
        }
        cloned
    }
}

impl PartialEq for Program {
    fn eq(&self, other: &Self) -> bool {
        self.structural_eq(other)
    }
}

impl Eq for Program {}
