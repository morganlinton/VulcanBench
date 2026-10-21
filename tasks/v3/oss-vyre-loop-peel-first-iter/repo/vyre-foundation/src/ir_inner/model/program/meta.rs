use std::hash::{Hash, Hasher as _};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rustc_hash::FxHasher;
use vyre_spec::bin_op::OpIntensity;

use crate::ir::{Expr, Node};
use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::types::BufferAccess;
use crate::transform::visit::{walk_nodes_and_exprs, ExprVisitor, NodeVisitor};

use super::Program;

/// Provenance for mutations that invalidate Program validation/cache state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProgramMutationProvenance {
    /// Program has not been mutated since construction or successful validation.
    Clean = 0,
    /// The non-composable dispatch flag changed.
    NonComposableFlag = 1,
    /// The target workgroup dimensions changed.
    WorkgroupSize = 2,
    /// The substrate-neutral parallel-region dimensions changed.
    ParallelRegionSize = 3,
    /// A caller borrowed the mutable entry vector.
    EntryMutation = 4,
    /// Internal builder or decode path rewrote Program shape.
    InternalShapeMutation = 5,
    /// Mutation provenance is unknown, so validation must fail closed.
    Unknown = 255,
}

impl ProgramMutationProvenance {
    #[inline]
    const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Clean,
            1 => Self::NonComposableFlag,
            2 => Self::WorkgroupSize,
            3 => Self::ParallelRegionSize,
            4 => Self::EntryMutation,
            5 => Self::InternalShapeMutation,
            _ => Self::Unknown,
        }
    }
}

fn mix_wire_fallback_hashable<T: Hash>(hasher: &mut blake3::Hasher, value: &T) {
    let mut state = FxHasher::default();
    value.hash(&mut state);
    hasher.update(&state.finish().to_le_bytes());
}

/// Bounded IR structure digest for wire-hash fallback (never formats full IR via `Debug`).
struct FallbackWireHasher<'a>(&'a mut blake3::Hasher);

impl NodeVisitor for FallbackWireHasher<'_> {
    fn visit_node(&mut self, node: &Node) {
        let h = &mut *self.0;
        match node {
            Node::Let { name, .. } => {
                h.update(b"n:Let\0");
                h.update(name.as_bytes());
            }
            Node::Assign { name, .. } => {
                h.update(b"n:Assign\0");
                h.update(name.as_bytes());
            }
            Node::Store { buffer, .. } => {
                h.update(b"n:Store\0");
                h.update(buffer.as_bytes());
            }
            Node::If { .. } => {
                h.update(b"n:If\0");
            }
            Node::Loop { var, .. } => {
                h.update(b"n:Loop\0");
                h.update(var.as_bytes());
            }
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => {
                h.update(b"n:IndirectDispatch\0");
                h.update(count_buffer.as_bytes());
                h.update(&count_offset.to_le_bytes());
            }
            Node::AsyncLoad {
                source,
                destination,
                tag,
                ..
            } => {
                h.update(b"n:AsyncLoad\0");
                h.update(source.as_bytes());
                h.update(destination.as_bytes());
                h.update(tag.as_bytes());
            }
            Node::AsyncStore {
                source,
                destination,
                tag,
                ..
            } => {
                h.update(b"n:AsyncStore\0");
                h.update(source.as_bytes());
                h.update(destination.as_bytes());
                h.update(tag.as_bytes());
            }
            Node::AsyncWait { tag } => {
                h.update(b"n:AsyncWait\0");
                h.update(tag.as_bytes());
            }
            Node::Trap { tag, .. } => {
                h.update(b"n:Trap\0");
                h.update(tag.as_bytes());
            }
            Node::Resume { tag } => {
                h.update(b"n:Resume\0");
                h.update(tag.as_bytes());
            }
            Node::AllReduce { buffer, op, group } => {
                h.update(b"n:AllReduce\0");
                h.update(buffer.as_bytes());
                h.update(&op.builtin_wire_tag().to_le_bytes());
                h.update(&group.as_u32().to_le_bytes());
            }
            Node::AllGather {
                input,
                output,
                group,
            } => {
                h.update(b"n:AllGather\0");
                h.update(input.as_bytes());
                h.update(output.as_bytes());
                h.update(&group.as_u32().to_le_bytes());
            }
            Node::ReduceScatter {
                input,
                output,
                op,
                group,
            } => {
                h.update(b"n:ReduceScatter\0");
                h.update(input.as_bytes());
                h.update(output.as_bytes());
                h.update(&op.builtin_wire_tag().to_le_bytes());
                h.update(&group.as_u32().to_le_bytes());
            }
            Node::Broadcast {
                buffer,
                root,
                group,
            } => {
                h.update(b"n:Broadcast\0");
                h.update(buffer.as_bytes());
                h.update(&root.to_le_bytes());
                h.update(&group.as_u32().to_le_bytes());
            }
            Node::Return => {
                h.update(b"n:Return\0");
            }
            Node::Barrier { ordering } => {
                h.update(b"n:Barrier\0");
                mix_wire_fallback_hashable(h, ordering);
            }
            Node::Block(_) => {
                h.update(b"n:Block\0");
            }
            Node::Region {
                generator,
                source_region,
                ..
            } => {
                h.update(b"n:Region\0");
                h.update(generator.as_bytes());
                if let Some(source_gen) = source_region {
                    h.update(source_gen.name.as_bytes());
                }
            }
            Node::Opaque(ext) => {
                h.update(b"n:Opaque\0");
                h.update(ext.extension_kind().as_bytes());
            }
        }
    }
}

impl ExprVisitor for FallbackWireHasher<'_> {
    fn visit_expr(&mut self, expr: &Expr) {
        let h = &mut *self.0;
        match expr {
            Expr::LitU32(v) => {
                h.update(b"e:LitU32\0");
                h.update(&v.to_le_bytes());
            }
            Expr::LitI32(v) => {
                h.update(b"e:LitI32\0");
                h.update(&v.to_le_bytes());
            }
            Expr::LitF32(v) => {
                h.update(b"e:LitF32\0");
                h.update(&v.to_le_bytes());
            }
            Expr::LitBool(v) => {
                h.update(b"e:LitBool\0");
                h.update(&[u8::from(*v)]);
            }
            Expr::Var(name) => {
                h.update(b"e:Var\0");
                h.update(name.as_bytes());
            }
            Expr::Load { buffer, .. } => {
                h.update(b"e:Load\0");
                h.update(buffer.as_bytes());
            }
            Expr::BufLen { buffer } => {
                h.update(b"e:BufLen\0");
                h.update(buffer.as_bytes());
            }
            Expr::InvocationId { axis } => {
                h.update(b"e:InvocationId\0");
                h.update(&[*axis]);
            }
            Expr::WorkgroupId { axis } => {
                h.update(b"e:WorkgroupId\0");
                h.update(&[*axis]);
            }
            Expr::LocalId { axis } => {
                h.update(b"e:LocalId\0");
                h.update(&[*axis]);
            }
            Expr::BinOp { op, .. } => {
                h.update(b"e:BinOp\0");
                mix_wire_fallback_hashable(h, op);
            }
            Expr::UnOp { op, .. } => {
                h.update(b"e:UnOp\0");
                mix_wire_fallback_hashable(h, op);
            }
            Expr::Call { op_id, .. } => {
                h.update(b"e:Call\0");
                h.update(op_id.as_bytes());
            }
            Expr::Select { .. } => {
                h.update(b"e:Select\0");
            }
            Expr::Cast { target, .. } => {
                h.update(b"e:Cast\0");
                mix_wire_fallback_hashable(h, target);
            }
            Expr::Fma { .. } => {
                h.update(b"e:Fma\0");
            }
            Expr::Atomic {
                op,
                buffer,
                ordering,
                ..
            } => {
                h.update(b"e:Atomic\0");
                mix_wire_fallback_hashable(h, op);
                h.update(buffer.as_bytes());
                mix_wire_fallback_hashable(h, ordering);
            }
            Expr::SubgroupBallot { .. } => {
                h.update(b"e:SubgroupBallot\0");
            }
            Expr::SubgroupShuffle { .. } => {
                h.update(b"e:SubgroupShuffle\0");
            }
            Expr::SubgroupReduce { op, .. } => {
                h.update(b"e:SubgroupReduce\0");
                h.update(&[op.builtin_wire_tag()]);
            }
            Expr::SubgroupLocalId => {
                h.update(b"e:SubgroupLocalId\0");
            }
            Expr::SubgroupSize => {
                h.update(b"e:SubgroupSize\0");
            }
            Expr::Opaque(ext) => {
                h.update(b"e:Opaque\0");
                h.update(ext.extension_kind().as_bytes());
            }
        }
    }
}

impl Program {
    /// Re-apply the same top-level `Node::Region` contract as
    /// [`Program::wrapped`].
    ///
    /// The [`region_inline_engine`](crate::optimizer::passes::cleanup::region_inline_engine)
    /// pass flattens small Category-A regions so CSE/DCE can see a single
    /// function-shaped body, which can leave a statement-shaped entry list. The
    /// standard optimizer run ends with this helper so the program remains in
    /// a runnable, validator/reference-interpreter–compatible form while
    /// still benefiting from the inline pass.
    #[must_use]
    pub fn reconcile_runnable_top_level(self) -> Self {
        if self.is_top_level_region_wrapped() {
            return self;
        }
        // Move the entry Vec out via map_entry's Arc-aware path; one
        // Program rebuild instead of two scaffold rebuilds.
        self.map_entry(Self::wrap_entry)
    }

    /// Look up a buffer declaration by name.
    #[must_use]
    #[inline]
    pub fn buffer(&self, name: &str) -> Option<&super::BufferDecl> {
        self.buffer_index
            .get(name)
            .and_then(|&index| self.buffers.get(index))
    }

    /// Declared buffers.
    #[must_use]
    #[inline]
    pub fn buffers(&self) -> &[super::BufferDecl] {
        self.buffers.as_ref()
    }

    /// Access the buffer declaration Arc directly for identity checks.
    #[must_use]
    #[inline]
    #[cfg(test)]
    pub(crate) fn buffers_arc(&self) -> &Arc<[super::BufferDecl]> {
        &self.buffers
    }

    /// Compare two programs by observable IR structure.
    ///
    /// This walk intentionally ignores buffer declaration order and never
    /// consults arena-local allocation identity. Two programs are structurally
    /// equal when they declare the same buffers, workgroup size, optional entry
    /// op id, and entry body semantics.
    #[must_use]
    #[inline]
    pub fn structural_eq(&self, other: &Self) -> bool {
        // Identity short-circuit: Program::clone shares all the
        // inner Arcs, so comparing a cloned program against its
        // source (the common optimizer-pipeline pattern) is pure
        // refcount comparison.
        if std::ptr::eq(self, other)
            || (Arc::ptr_eq(&self.buffers, &other.buffers)
                && Arc::ptr_eq(&self.entry, &other.entry)
                && self.entry_op_id == other.entry_op_id
                && self.non_composable_with_self == other.non_composable_with_self
                && self.workgroup_size == other.workgroup_size)
        {
            return true;
        }
        self.entry_op_id == other.entry_op_id
            && self.non_composable_with_self == other.non_composable_with_self
            && buffers_equal_ignoring_declaration_order(&self.buffers, &other.buffers)
            && self.workgroup_size == other.workgroup_size
            && self.entry == other.entry
    }

    /// Workgroup dimensions.
    #[must_use]
    #[inline]
    pub fn workgroup_size(&self) -> [u32; 3] {
        self.workgroup_size
    }

    /// Substrate-neutral alias for [`workgroup_size`](Self::workgroup_size).
    ///
    /// Naming: "parallel region" avoids picking a single target substrate's
    /// word for one dispatch invocation grouping.
    #[must_use]
    #[inline]
    pub fn parallel_region_size(&self) -> [u32; 3] {
        self.workgroup_size
    }

    /// Return true when this program must not be fused with another copy
    /// of itself in the same megakernel.
    #[must_use]
    #[inline]
    pub fn is_non_composable_with_self(&self) -> bool {
        self.non_composable_with_self
    }

    /// Mark this program as non-composable with itself.
    #[must_use]
    #[inline]
    pub fn with_non_composable_with_self(mut self, flag: bool) -> Self {
        self.non_composable_with_self = flag;
        self.invalidate_caches_for(ProgramMutationProvenance::NonComposableFlag);
        self
    }

    /// Set the workgroup dimensions in place. Used by harnesses that
    /// need to clone-and-rewrite a program's workgroup size for fallback
    /// dispatch  -  the alternative was to reconstruct the entire Program,
    /// which is unnecessarily expensive when only one field changes.
    #[inline]
    pub fn set_workgroup_size(&mut self, workgroup_size: [u32; 3]) {
        self.workgroup_size = workgroup_size;
        self.invalidate_caches_for(ProgramMutationProvenance::WorkgroupSize);
    }

    /// Substrate-neutral alias for [`set_workgroup_size`](Self::set_workgroup_size).
    #[inline]
    pub fn set_parallel_region_size(&mut self, parallel_region_size: [u32; 3]) {
        self.workgroup_size = parallel_region_size;
        self.invalidate_caches_for(ProgramMutationProvenance::ParallelRegionSize);
    }

    /// Entry-point nodes.
    #[must_use]
    #[inline]
    pub fn entry(&self) -> &[Node] {
        self.entry.as_ref().as_slice()
    }

    /// Shared entry-point body Arc for identity checks.
    #[must_use]
    #[inline]
    pub fn entry_arc(&self) -> &Arc<Vec<Node>> {
        &self.entry
    }

    /// Return true when this Program is the canonical no-op shape produced by
    /// [`Program::empty`]: no buffers and a single empty root Region.
    #[must_use]
    #[inline]
    pub fn is_explicit_noop(&self) -> bool {
        self.buffers().is_empty()
            && matches!(self.entry(), [Node::Region { body, .. }] if body.is_empty())
    }

    /// Return true when the program satisfies the top-level region-chain
    /// invariant: at least one top-level node, and every top-level node is a
    /// `Node::Region`.
    #[must_use]
    #[inline]
    pub fn is_top_level_region_wrapped(&self) -> bool {
        !self.entry.is_empty()
            && self
                .entry()
                .iter()
                .all(|node| matches!(node, Node::Region { .. }))
    }

    /// Actionable error text describing why the top-level region invariant
    /// failed, or `None` when the entry is valid.
    #[must_use]
    pub fn top_level_region_violation(&self) -> Option<String> {
        if self.entry().is_empty() {
            return Some(
                "program entry has no top-level Region. Fix: construct runnable programs with Program::wrapped(...) or wrap the body in Node::Region before validation, interpretation, or dispatch."
                    .to_string(),
            );
        }

        self.entry()
            .iter()
            .enumerate()
            .find(|(_, node)| !matches!(node, Node::Region { .. }))
            .map(|(index, node)| {
                format!(
                    "program entry node {index} is `{}` instead of `Node::Region`. Fix: construct runnable programs with Program::wrapped(...) or wrap the top-level body in Node::Region; raw Program::new is reserved for wire decode and negative tests.",
                    Self::top_level_node_name(node)
                )
            })
    }

    /// Mutable entry-point nodes for transformation passes.
    #[must_use]
    #[inline]
    pub fn entry_mut(&mut self) -> &mut Vec<Node> {
        self.invalidate_caches_for(ProgramMutationProvenance::EntryMutation);
        Arc::make_mut(&mut self.entry)
    }

    /// Stable BLAKE3 fingerprint of the canonical wire-format bytes.
    #[must_use]
    #[inline]
    pub fn fingerprint(&self) -> [u8; 32] {
        *self.fingerprint.get_or_init(|| {
            let hash = self.compute_wire_hash();
            let _ = self.hash.set(hash);
            *hash.as_bytes()
        })
    }

    /// VSA-style hypervector fingerprint of the canonical wire-format
    /// bytes. Each `u32` lane is one segment of the program's blake3
    /// hash; together they form an 8-lane hypervector suitable for
    /// approximate similarity search via hamming distance.
    ///
    /// Use as the canonical cache key for approximate-match caches
    /// (e.g. validation cache, AOT artifact dedup); use
    /// [`Self::fingerprint`] for exact-match lookups.
    ///
    /// Wires the substrate's #29 hypervector primitive into Program
    /// itself  -  every Program now carries its own VSA fingerprint
    /// without callers having to reach into the substrate explicitly.
    #[must_use]
    pub fn vsa_fingerprint(&self) -> Vec<u32> {
        self.fingerprint()
            .chunks_exact(core::mem::size_of::<u32>())
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    }

    /// Indices of writable storage outputs in `buffers()` order.
    #[must_use]
    #[inline]
    pub fn output_buffer_indices(&self) -> &[u32] {
        self.output_buffer_index
            .get_or_init(|| {
                Arc::new(
                    self.buffers()
                        .iter()
                        .enumerate()
                        .filter_map(|(index, buffer)| {
                            matches!(
                                buffer.access(),
                                BufferAccess::ReadWrite | BufferAccess::WriteOnly
                            )
                            .then(|| u32::try_from(index).ok())
                            .flatten()
                        })
                        .collect(),
                )
            })
            .as_slice()
    }

    /// True when the entry walk discovers any indirect dispatch node.
    #[must_use]
    #[inline]
    pub fn has_indirect_dispatch(&self) -> bool {
        *self.has_indirect_dispatch.get_or_init(|| {
            // Fast-path: ProgramStats records every node kind seen during
            // its single-pass walk. If the IndirectDispatch bit is unset,
            // the tree definitely contains no IndirectDispatch nodes and
            // the explicit traversal below would redundantly visit every
            // node only to return false. Reading the bit is O(1).
            if !self
                .stats()
                .has_any_node_kind(super::stats::NODE_KIND_INDIRECT_DISPATCH)
            {
                return false;
            }
            let mut stack: smallvec::SmallVec<[&Node; 32]> = self.entry().iter().rev().collect();
            while let Some(node) = stack.pop() {
                match node {
                    Node::IndirectDispatch { .. } => return true,
                    Node::If {
                        then, otherwise, ..
                    } => {
                        stack.extend(otherwise.iter().rev());
                        stack.extend(then.iter().rev());
                    }
                    Node::Loop { body, .. } | Node::Block(body) => {
                        stack.extend(body.iter().rev());
                    }
                    Node::Region { body, .. } => {
                        stack.extend(body.iter().rev());
                    }
                    Node::Let { .. }
                    | Node::Assign { .. }
                    | Node::Store { .. }
                    | Node::AllReduce { .. }
                    | Node::AllGather { .. }
                    | Node::ReduceScatter { .. }
                    | Node::Broadcast { .. }
                    | Node::Return
                    | Node::Barrier { .. }
                    | Node::AsyncLoad { .. }
                    | Node::AsyncStore { .. }
                    | Node::AsyncWait { .. }
                    | Node::Trap { .. }
                    | Node::Resume { .. }
                    | Node::Opaque(_) => {}
                }
            }
            false
        })
    }

    /// Check whether a named buffer exists.
    #[must_use]
    #[inline]
    pub fn has_buffer(&self, name: &str) -> bool {
        self.buffer_index.contains_key(name)
    }

    /// Number of declared buffers.
    #[must_use]
    #[inline]
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    #[inline]
    pub(super) fn build_buffer_index(
        buffers: &[super::BufferDecl],
    ) -> rustc_hash::FxHashMap<Arc<str>, usize> {
        let mut index = rustc_hash::FxHashMap::default();
        index.reserve(buffers.len());
        for (buffer_index, buffer) in buffers.iter().enumerate() {
            index
                .entry(Arc::clone(&buffer.name))
                .or_insert(buffer_index);
        }
        index
    }

    /// Mark this program as successfully validated structurally.
    #[inline]
    pub fn mark_structurally_validated(&self) {
        self.structural_validation_fingerprint.store(
            self.current_validation_fingerprint_token(),
            Ordering::Release,
        );
        self.mutation_provenance
            .store(ProgramMutationProvenance::Clean as u8, Ordering::Release);
        self.structural_validated.store(true, Ordering::Release);
    }

    /// Return true once structural validation has succeeded for this program shape.
    #[must_use]
    #[inline]
    pub fn is_structurally_validated(&self) -> bool {
        if !self.structural_validated.load(Ordering::Acquire) {
            return false;
        }
        if self.validation_mutation_provenance() == ProgramMutationProvenance::Unknown {
            self.structural_validated.store(false, Ordering::Release);
            return false;
        }
        let recorded = self
            .structural_validation_fingerprint
            .load(Ordering::Acquire);
        if recorded == 0 || recorded != self.current_validation_fingerprint_token() {
            self.structural_validated.store(false, Ordering::Release);
            return false;
        }
        true
    }

    /// Last mutation provenance recorded for validation/cache invalidation.
    #[must_use]
    #[inline]
    pub fn validation_mutation_provenance(&self) -> ProgramMutationProvenance {
        ProgramMutationProvenance::from_code(self.mutation_provenance.load(Ordering::Acquire))
    }

    /// Mark the Program as having been mutated by a boundary that cannot name a
    /// concrete provenance. Validation fails closed until the Program is rebuilt
    /// through a known constructor or known mutation API.
    #[inline]
    pub fn mark_unknown_mutation_provenance(&mut self) {
        self.invalidate_caches_for(ProgramMutationProvenance::Unknown);
    }

    /// Mark this program as successfully validated for a specific backend.
    #[inline]
    pub fn mark_validated_on(&self, backend_id: &str) {
        if self.validation_mutation_provenance() == ProgramMutationProvenance::Unknown {
            return;
        }
        self.validation_set
            .get_or_init(|| Arc::new(dashmap::DashSet::new()))
            .insert(Arc::from(self.validation_cache_key(backend_id)));
    }

    /// Return true if this program has been validated for the given backend.
    #[must_use]
    #[inline]
    pub fn is_validated_on(&self, backend_id: &str) -> bool {
        self.validation_set
            .get()
            .is_some_and(|set| set.contains(self.validation_cache_key(backend_id).as_str()))
    }

    /// Deprecated: use `is_structurally_validated` or `is_validated_on`.
    #[deprecated(note = "use is_structurally_validated or is_validated_on")]
    #[must_use]
    #[inline]
    pub fn is_validated(&self) -> bool {
        self.is_structurally_validated()
    }

    /// Deprecated: use `mark_structurally_validated` or `mark_validated_on`.
    #[deprecated(note = "use mark_structurally_validated or mark_validated_on")]
    #[inline]
    pub fn mark_validated(&self) {
        self.mark_structurally_validated();
    }

    /// Validate the program and cache the successful result on the program.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::WireFormatValidation`] with every validation
    /// message joined when the structural validator rejects the program.
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.validation_mutation_provenance() == ProgramMutationProvenance::Unknown {
            return Err(crate::error::Error::WireFormatValidation {
                message: "program validation cache was invalidated by unknown mutation provenance. Fix: rebuild the Program through Program::wrapped/from_wire or use a named Program mutation API before validating.".into(),
            });
        }
        if self.is_structurally_validated() {
            return Ok(());
        }
        let errors = crate::validate::validate(self);
        if errors.is_empty() {
            self.mark_structurally_validated();
            return Ok(());
        }
        let mut message = String::new();
        for (index, error) in errors.into_iter().enumerate() {
            if index > 0 {
                message.push_str("; ");
            }
            message.push_str(error.message());
        }
        Err(crate::error::Error::WireFormatValidation { message })
    }

    #[inline]
    /// Estimate the peak VRAM byte size of this Program.
    ///
    /// Innovation I.11: Static VRAM Pressure Analysis.
    /// Returns the total bytes required by all storage and uniform buffers
    /// declared in the Program. Optimizer passes use this to automatically
    /// partition workloads if they would exceed a backend-specific safety
    /// margin.
    #[must_use]
    pub fn estimate_peak_vram_bytes(&self) -> u64 {
        self.buffers
            .iter()
            .map(|buffer| {
                let Some(element_size) = buffer.element.size_bytes() else {
                    return u64::MAX;
                };
                u64::from(buffer.count)
                    .saturating_mul(u64::try_from(element_size).unwrap_or(u64::MAX))
            })
            .fold(0u64, u64::saturating_add)
    }

    /// Return the peak computational intensity found in any instruction.
    #[must_use]
    pub fn peak_intensity(&self) -> OpIntensity {
        let mut peak = OpIntensity::Free;
        for node in self.entry() {
            peak = peak.max(Self::node_intensity(node));
        }
        peak
    }

    fn node_intensity(node: &crate::ir::Node) -> OpIntensity {
        use crate::ir::Node;
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => Self::expr_intensity(value),
            Node::Store { index, value, .. } => {
                Self::expr_intensity(index).max(Self::expr_intensity(value))
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let mut p = Self::expr_intensity(cond);
                for n in then {
                    p = p.max(Self::node_intensity(n));
                }
                for n in otherwise {
                    p = p.max(Self::node_intensity(n));
                }
                p
            }
            Node::Loop { from, to, body, .. } => {
                let mut p = Self::expr_intensity(from).max(Self::expr_intensity(to));
                for n in body {
                    p = p.max(Self::node_intensity(n));
                }
                p
            }
            Node::Block(nodes) => {
                let mut p = OpIntensity::Free;
                for n in nodes {
                    p = p.max(Self::node_intensity(n));
                }
                p
            }
            Node::Region { body, .. } => {
                let mut p = OpIntensity::Free;
                for n in body.iter() {
                    p = p.max(Self::node_intensity(n));
                }
                p
            }
            _ => OpIntensity::Free,
        }
    }

    fn expr_intensity(expr: &crate::ir::Expr) -> OpIntensity {
        use crate::ir::Expr;
        match expr {
            Expr::BinOp { op, left, right } => op
                .intensity()
                .max(Self::expr_intensity(left))
                .max(Self::expr_intensity(right)),
            Expr::UnOp { operand, .. } => Self::expr_intensity(operand),
            Expr::Load { index, .. } => Self::expr_intensity(index),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => Self::expr_intensity(cond)
                .max(Self::expr_intensity(true_val))
                .max(Self::expr_intensity(false_val)),
            Expr::Cast { value, .. } => Self::expr_intensity(value),
            Expr::Fma { a, b, c } => Self::expr_intensity(a)
                .max(Self::expr_intensity(b))
                .max(Self::expr_intensity(c)),
            Expr::Atomic {
                index,
                value,
                expected,
                ..
            } => {
                let mut p = Self::expr_intensity(index).max(Self::expr_intensity(value));
                if let Some(e) = expected {
                    p = p.max(Self::expr_intensity(e));
                }
                p.max(OpIntensity::Heavy)
            }
            Expr::SubgroupBallot { cond } => Self::expr_intensity(cond).max(OpIntensity::Heavy),
            Expr::SubgroupShuffle { value, lane } => Self::expr_intensity(value)
                .max(Self::expr_intensity(lane))
                .max(OpIntensity::Heavy),
            Expr::SubgroupReduce { value, .. } => Self::expr_intensity(value).max(OpIntensity::Heavy),
            _ => OpIntensity::Free,
        }
    }

    fn compute_wire_hash(&self) -> blake3::Hash {
        match self.canonical_wire_hash() {
            Ok(hash) => hash,
            Err(error) => {
                let structural = self.structural_fingerprint_fallback();
                let err_msg = error.to_string();
                let mut fallback = Vec::with_capacity(96 + err_msg.len() + structural.len());
                fallback.extend_from_slice(b"VYRE-PROGRAM-CANONICAL-WIRE-HASH-ERROR\0");
                fallback.extend_from_slice(err_msg.as_bytes());
                fallback.push(0);
                fallback.extend_from_slice(structural.as_bytes());
                blake3::hash(&fallback)
            }
        }
    }

    fn structural_fingerprint_fallback(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"VYRE-WIRE-FALLBACK-V4\0");
        if let Some(id) = self.entry_op_id.as_deref() {
            hasher.update(id.as_bytes());
        }
        hasher.update(b"\0");
        for axis in &self.workgroup_size {
            hasher.update(&axis.to_le_bytes());
        }
        hasher.update(&[u8::from(self.non_composable_with_self)]);
        let mut keys: Vec<Vec<u8>> = self
            .buffers()
            .iter()
            .map(buffer_decl_canonical_key)
            .collect();
        keys.sort_unstable();
        for key in keys {
            hasher.update(&key);
        }
        let mut visitor = FallbackWireHasher(&mut hasher);
        walk_nodes_and_exprs(self, &mut visitor);
        hasher.finalize().to_hex().to_string()
    }

    fn validation_cache_key(&self, backend_id: &str) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let fingerprint = self.current_validation_fingerprint();
        let mut key = String::with_capacity(backend_id.len() + 1 + 64);
        key.push_str(backend_id);
        key.push(':');
        for &byte in &fingerprint {
            key.push(HEX[(byte >> 4) as usize] as char);
            key.push(HEX[(byte & 0x0f) as usize] as char);
        }
        key
    }

    #[inline]
    pub(super) fn invalidate_caches(&mut self) {
        self.invalidate_caches_for(ProgramMutationProvenance::InternalShapeMutation);
    }

    #[inline]
    pub(super) fn invalidate_caches_for(&mut self, provenance: ProgramMutationProvenance) {
        self.structural_validated.store(false, Ordering::Release);
        self.structural_validation_fingerprint
            .store(0, Ordering::Release);
        self.mutation_provenance
            .store(provenance as u8, Ordering::Release);
        if let Some(set) = self.validation_set.get() {
            set.clear();
        }
        let _ = self.hash.take();
        let _ = self.fingerprint.take();
        drop(self.output_buffer_index.take());
        let _ = self.has_indirect_dispatch.take();
        drop(self.stats.take());
    }

    fn current_validation_fingerprint(&self) -> [u8; 32] {
        *self.compute_wire_hash().as_bytes()
    }

    fn current_validation_fingerprint_token(&self) -> u64 {
        let bytes = self.current_validation_fingerprint();
        let token = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        token.max(1)
    }

    #[inline]
    pub(super) fn wrap_entry(entry: Vec<Node>) -> Vec<Node> {
        if !Self::entry_needs_root_region(&entry) {
            return entry;
        }
        vec![Node::Region {
            generator: Ident::from(Self::ROOT_REGION_GENERATOR),
            source_region: None,
            body: Arc::new(entry),
        }]
    }

    #[inline]
    fn entry_needs_root_region(entry: &[Node]) -> bool {
        entry.is_empty()
            || entry
                .iter()
                .any(|node| !matches!(node, Node::Region { .. }))
    }

    #[inline]
    fn top_level_node_name(node: &Node) -> &'static str {
        match node {
            Node::Let { .. } => "Let",
            Node::Assign { .. } => "Assign",
            Node::Store { .. } => "Store",
            Node::If { .. } => "If",
            Node::Loop { .. } => "Loop",
            Node::Return => "Return",
            Node::Block(_) => "Block",
            Node::Barrier { .. } => "Barrier",
            Node::Region { .. } => "Region",
            Node::IndirectDispatch { .. } => "IndirectDispatch",
            Node::AsyncLoad { .. } => "AsyncLoad",
            Node::AsyncStore { .. } => "AsyncStore",
            Node::AsyncWait { .. } => "AsyncWait",
            Node::Trap { .. } => "Trap",
            Node::Resume { .. } => "Resume",
            Node::AllReduce { .. } => "AllReduce",
            Node::AllGather { .. } => "AllGather",
            Node::ReduceScatter { .. } => "ReduceScatter",
            Node::Broadcast { .. } => "Broadcast",
            Node::Opaque(_) => "Opaque",
        }
    }
}

pub(crate) fn buffers_equal_ignoring_declaration_order(
    left: &[super::BufferDecl],
    right: &[super::BufferDecl],
) -> bool {
    if left.len() != right.len() {
        return false;
    }

    // VYRE_IR_HOTSPOTS HIGH (meta.rs:360-379): previous impl allocated
    // two Vec<Vec<u8>> then sorted on every equality call. Fast-path:
    // if the slices compare equal in-place (declaration orders match)
    // we skip the key-materialization entirely. This catches every
    // Program::clone(prog) == prog and every `Arc::clone`-equivalent
    // comparison, which dominate the call distribution.
    if left == right {
        return true;
    }

    let mut left_keys = Vec::with_capacity(left.len());
    left_keys.extend(left.iter().map(buffer_decl_canonical_key));
    let mut right_keys = Vec::with_capacity(right.len());
    right_keys.extend(right.iter().map(buffer_decl_canonical_key));
    left_keys.sort_unstable();
    right_keys.sort_unstable();
    left_keys == right_keys
}

pub(super) fn buffer_decl_canonical_key(buffer: &super::BufferDecl) -> Vec<u8> {
    use crate::serial::wire::framing::{put_len_u32, put_u32, put_u8};
    use crate::serial::wire::tags::put_data_type;

    let mut key = Vec::with_capacity(96);
    if let Err(error) = put_len_u32(&mut key, buffer.name.len(), "buffer name length") {
        key.extend_from_slice(b"\0name-length-error\0");
        key.extend_from_slice(error.as_bytes());
    }
    key.extend_from_slice(buffer.name.as_bytes());
    put_u32(&mut key, buffer.binding);
    match crate::serial::wire::tags::access_tag::access_tag(&buffer.access) {
        Ok(tag) => put_u8(&mut key, tag),
        Err(error) => {
            put_u8(&mut key, u8::MAX);
            key.extend_from_slice(error.as_bytes());
        }
    }
    put_u8(
        &mut key,
        match buffer.kind {
            super::MemoryKind::Global => 0,
            super::MemoryKind::Shared => 1,
            super::MemoryKind::Uniform => 2,
            super::MemoryKind::Local => 3,
            super::MemoryKind::Readonly => 4,
            super::MemoryKind::Persistent => 5,
            super::MemoryKind::Push => 6,
        },
    );
    if let Err(error) = put_data_type(&mut key, &buffer.element) {
        key.extend_from_slice(b"\0dtype-error\0");
        key.extend_from_slice(error.as_bytes());
    }
    put_u32(&mut key, buffer.count);
    put_u8(&mut key, u8::from(buffer.is_output));
    put_u8(&mut key, u8::from(buffer.pipeline_live_out));
    match &buffer.output_byte_range {
        Some(range) => {
            put_u8(&mut key, 1);
            match u32::try_from(range.start) {
                Ok(start) => put_u32(&mut key, start),
                Err(error) => {
                    put_u32(&mut key, u32::MAX);
                    key.extend_from_slice(error.to_string().as_bytes());
                }
            }
            match u32::try_from(range.end) {
                Ok(end) => put_u32(&mut key, end),
                Err(error) => {
                    put_u32(&mut key, u32::MAX);
                    key.extend_from_slice(error.to_string().as_bytes());
                }
            }
        }
        None => put_u8(&mut key, 0),
    }
    match buffer.hints.coalesce_axis {
        Some(axis) => {
            put_u8(&mut key, 1);
            put_u8(&mut key, axis);
        }
        None => put_u8(&mut key, 0),
    }
    put_u32(&mut key, buffer.hints.preferred_alignment);
    put_u8(
        &mut key,
        match buffer.hints.cache_locality {
            super::CacheLocality::Streaming => 0,
            super::CacheLocality::Temporal => 1,
            super::CacheLocality::Random => 2,
        },
    );
    put_u8(&mut key, u8::from(buffer.bytes_extraction));
    key
}
