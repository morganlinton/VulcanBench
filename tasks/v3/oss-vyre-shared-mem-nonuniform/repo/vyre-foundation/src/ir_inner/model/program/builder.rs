use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashMap;

use crate::ir_inner::model::arena::{ArenaProgram, ExprArena};
use crate::ir_inner::model::node::Node;

use super::{BufferDecl, Program};

impl Program {
    /// Synthetic generator id used when callers submit a raw top-level body
    /// instead of an explicit `Node::Region`.
    pub const ROOT_REGION_GENERATOR: &'static str = "vyre.program.root";

    /// Create a complete program from buffer declarations, workgroup size, and
    /// entry-point nodes, auto-wrapping the top-level body in a root Region
    /// when necessary.
    ///
    /// This is the default construction path for runnable Programs.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferAccess, BufferDecl, DataType, Node, Program};
    ///
    /// let program = Program::wrapped(
    ///     vec![BufferDecl::storage(
    ///         "output",
    ///         0,
    ///         BufferAccess::ReadWrite,
    ///         DataType::U32,
    ///     )],
    ///     [64, 1, 1],
    ///     Vec::new(),
    /// );
    ///
    /// assert_eq!(program.workgroup_size(), [64, 1, 1]);
    /// assert_eq!(program.buffers().len(), 1);
    /// assert!(matches!(program.entry(), [Node::Region { .. }]));
    /// ```
    #[must_use]
    #[inline]
    pub fn wrapped(buffers: Vec<BufferDecl>, workgroup_size: [u32; 3], entry: Vec<Node>) -> Self {
        Self::new_raw(buffers, workgroup_size, Self::wrap_entry(entry))
    }

    /// Create a complete program from buffer declarations, workgroup size, and
    /// entry-point nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferAccess, BufferDecl, DataType, Node, Program};
    ///
    /// let program = Program::wrapped(
    ///     vec![BufferDecl::storage(
    ///         "output",
    ///         0,
    ///         BufferAccess::ReadWrite,
    ///         DataType::U32,
    ///     )],
    ///     [64, 1, 1],
    ///     Vec::new(),
    /// );
    ///
    /// assert_eq!(program.workgroup_size(), [64, 1, 1]);
    /// assert_eq!(program.buffers().len(), 1);
    /// assert!(matches!(program.entry(), [Node::Region { .. }]));
    /// ```
    #[deprecated(
        note = "Program::new preserves raw top-level entry nodes. Use Program::wrapped for runnable programs; reserve Program::new for wire decode and negative tests."
    )]
    #[must_use]
    #[inline]
    pub fn new(buffers: Vec<BufferDecl>, workgroup_size: [u32; 3], entry: Vec<Node>) -> Self {
        Self::new_raw(buffers, workgroup_size, entry)
    }

    #[must_use]
    #[inline]
    pub(crate) fn new_raw(
        buffers: Vec<BufferDecl>,
        workgroup_size: [u32; 3],
        entry: Vec<Node>,
    ) -> Self {
        let mut interner = FxHashMap::<Arc<str>, Arc<str>>::default();
        interner.reserve(buffers.len());
        let buffers: Vec<BufferDecl> = buffers
            .into_iter()
            .map(|mut b| {
                let arc = interner
                    .entry(Arc::clone(&b.name))
                    .or_insert_with(|| Arc::clone(&b.name))
                    .clone();
                b.name = arc;
                b
            })
            .collect();
        let buffer_index = Self::build_buffer_index(&buffers);
        Self {
            entry_op_id: None,
            buffers: Arc::from(buffers),
            buffer_index: Arc::new(buffer_index),
            workgroup_size,
            entry: Arc::new(entry),
            hash: OnceLock::new(),
            validation_set: OnceLock::new(),
            structural_validated: std::sync::atomic::AtomicBool::new(false),
            structural_validation_fingerprint: std::sync::atomic::AtomicU64::new(0),
            mutation_provenance: std::sync::atomic::AtomicU8::new(0),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self: false,
        }
    }

    /// Same as [`Self::with_rewritten_entry`] but wraps the entry first via
    /// the runnable-Region root contract (matches [`Self::wrapped`]). Use
    /// from passes that produce a fully fresh entry body but want to reuse
    /// the existing buffer Arc instead of paying for a full
    /// [`Self::wrapped`] (which deep-clones buffers, re-interns names, and
    /// rebuilds the buffer index).
    #[must_use]
    #[inline]
    pub fn with_rewritten_wrapped_entry(&self, entry: Vec<Node>) -> Self {
        self.with_rewritten_entry(Self::wrap_entry(entry))
    }

    /// Consume this program and rebuild it with `f` applied to the owned
    /// entry vec. Reuses the entry Arc when uniquely owned (the common
    /// case under the optimizer fixpoint)  -  no deep clone of the entry
    /// body and no scaffold allocation. Equivalent to:
    ///
    /// ```ignore
    /// let scaffold = program.with_rewritten_entry(Vec::new());
    /// let entry = f(program.into_entry_vec());
    /// scaffold.with_rewritten_entry(entry)
    /// ```
    ///
    /// but produces only one new `Program` value instead of two.
    #[must_use]
    #[inline]
    pub fn map_entry<F: FnOnce(Vec<Node>) -> Vec<Node>>(self, f: F) -> Self {
        let entry_op_id = self.entry_op_id.clone();
        let buffers = Arc::clone(&self.buffers);
        let buffer_index = Arc::clone(&self.buffer_index);
        let workgroup_size = self.workgroup_size;
        let non_composable_with_self = self.non_composable_with_self;
        let entry = f(self.into_entry_vec());
        Self {
            entry_op_id,
            buffers,
            buffer_index,
            workgroup_size,
            entry: Arc::new(entry),
            hash: OnceLock::new(),
            validation_set: OnceLock::new(),
            structural_validated: std::sync::atomic::AtomicBool::new(false),
            structural_validation_fingerprint: std::sync::atomic::AtomicU64::new(0),
            mutation_provenance: std::sync::atomic::AtomicU8::new(0),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self,
        }
    }

    /// Clone this program with a replacement entry body while preserving the
    /// existing buffer table, workgroup size, and optional certified op id.
    #[must_use]
    #[inline]
    pub fn with_rewritten_entry(&self, entry: Vec<Node>) -> Self {
        Self {
            entry_op_id: self.entry_op_id.clone(),
            buffers: Arc::clone(&self.buffers),
            buffer_index: Arc::clone(&self.buffer_index),
            workgroup_size: self.workgroup_size,
            entry: Arc::new(entry),
            hash: OnceLock::new(),
            validation_set: OnceLock::new(),
            structural_validated: std::sync::atomic::AtomicBool::new(false),
            structural_validation_fingerprint: std::sync::atomic::AtomicU64::new(0),
            mutation_provenance: std::sync::atomic::AtomicU8::new(0),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self: self.non_composable_with_self,
        }
    }

    /// Clone this program with replacement buffer declarations while
    /// preserving the entry body, workgroup size, and metadata flags.
    #[must_use]
    #[inline]
    pub fn with_rewritten_buffers(&self, buffers: Vec<BufferDecl>) -> Self {
        let buffer_index = Self::build_buffer_index(&buffers);
        Self {
            entry_op_id: self.entry_op_id.clone(),
            buffers: Arc::from(buffers),
            buffer_index: Arc::new(buffer_index),
            workgroup_size: self.workgroup_size,
            entry: Arc::clone(&self.entry),
            hash: OnceLock::new(),
            validation_set: OnceLock::new(),
            structural_validated: std::sync::atomic::AtomicBool::new(false),
            structural_validation_fingerprint: std::sync::atomic::AtomicU64::new(0),
            mutation_provenance: std::sync::atomic::AtomicU8::new(0),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self: self.non_composable_with_self,
        }
    }

    /// Clone this program with replacement dispatch dimensions and entry body
    /// while preserving the existing buffer table, indexes, and metadata flags.
    #[must_use]
    #[inline]
    pub fn with_rewritten_workgroup_size_and_entry(
        &self,
        workgroup_size: [u32; 3],
        entry: Vec<Node>,
    ) -> Self {
        Self {
            entry_op_id: self.entry_op_id.clone(),
            buffers: Arc::clone(&self.buffers),
            buffer_index: Arc::clone(&self.buffer_index),
            workgroup_size,
            entry: Arc::new(entry),
            hash: OnceLock::new(),
            validation_set: OnceLock::new(),
            structural_validated: std::sync::atomic::AtomicBool::new(false),
            structural_validation_fingerprint: std::sync::atomic::AtomicU64::new(0),
            mutation_provenance: std::sync::atomic::AtomicU8::new(0),
            fingerprint: OnceLock::new(),
            output_buffer_index: OnceLock::new(),
            has_indirect_dispatch: OnceLock::new(),
            stats: OnceLock::new(),
            non_composable_with_self: self.non_composable_with_self,
        }
    }

    /// Consume the program and return its entry nodes, reusing the
    /// backing vector when this program owns the entry body uniquely.
    #[must_use]
    #[inline]
    pub fn into_entry_vec(self) -> Vec<Node> {
        Arc::try_unwrap(self.entry).unwrap_or_else(|entry| entry.as_ref().clone())
    }

    /// Create an arena-backed program scaffold.
    ///
    /// This constructor is the opt-in migration path for builders that want
    /// [`ExprRef`](crate::ir_inner::model::arena::ExprRef) handles instead of boxed
    /// expression trees. [`Program::new`] remains the boxed-tree constructor.
    #[must_use]
    #[inline]
    pub fn with_arena(
        arena: &ExprArena,
        buffers: Vec<BufferDecl>,
        workgroup_size: [u32; 3],
    ) -> ArenaProgram<'_> {
        ArenaProgram::new(arena, buffers, workgroup_size)
    }

    /// Create a minimal program with no buffers and an empty body.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Program;
    ///
    /// let program = Program::empty();
    ///
    /// assert!(program.buffers().is_empty());
    /// assert_eq!(program.workgroup_size(), [1, 1, 1]);
    /// assert!(program.is_explicit_noop());
    /// ```
    #[must_use]
    #[inline]
    pub fn empty() -> Self {
        Self::wrapped(Vec::new(), [1, 1, 1], Vec::new())
    }

    /// Attach the stable operation ID whose conform registry entry certifies
    /// this program for runtime lowering.
    #[must_use]
    #[inline]
    pub fn with_entry_op_id(mut self, op_id: impl Into<String>) -> Self {
        self.entry_op_id = Some(op_id.into());
        self.invalidate_caches();
        self
    }

    /// Stable operation ID required by the conform gate.
    #[must_use]
    #[inline]
    pub fn entry_op_id(&self) -> Option<&str> {
        self.entry_op_id.as_deref()
    }

    /// Attach an optional operation ID while preserving anonymous test IR.
    #[must_use]
    #[inline]
    pub(crate) fn with_optional_entry_op_id(mut self, op_id: Option<String>) -> Self {
        self.entry_op_id = op_id;
        self.invalidate_caches();
        self
    }
}
