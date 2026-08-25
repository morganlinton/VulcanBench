//! GPU-resident e-graph substrate.
//!
//! The CPU-side `eqsat::EGraph` materialises rewrite candidates,
//! union-find merges, and cost-based extraction in a single
//! sequential walker. For wide rewrite families (algebraic
//! identities, peephole tables, pattern-match-heavy primitives)
//! the per-iteration cost grows with the e-graph size, and the
//! hash-cons table becomes the bottleneck. This module ships the
//! GPU-resident representation: a flattened, columnar mirror of
//! the EGraph that can be uploaded to a GPU buffer and walked in
//! parallel by warp-cooperative passes.
//!
//! The mirror is additive: CPU passes keep using `EGraph::saturate`,
//! while GPU-aware passes use `GpuEGraphSnapshot::from_egraph_with`
//! to materialise the columnar arrays and merge discovered equivalences
//! back through `apply_equivalences_to_egraph`.
//!
//! Soundness: the snapshot is read-only. Any equivalence the GPU
//! discovers is merged through the same `EGraph::merge` API the
//! CPU uses, so the EGraph's saturation invariants hold by
//! construction.
//!
//! ## Why the columnar layout
//!
//! Each row of the snapshot is `(eclass_id, language_op_id,
//! children_offset, children_len)`. The children indices live in
//! a separate `children: Vec<u32>` column. This layout fits a
//! GPU's coalesced-memory access pattern: a warp reading 32
//! consecutive rows touches one cache line per column (4 columns
//! × 4 bytes × 32 lanes = 512 bytes per warp).

use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use super::eqsat::{
    try_extract_best_with_budget, EClassId, EGraph, EGraphError, ENodeLang,
    DEFAULT_EXTRACTION_ITER_BUDGET,
};

/// GPU-resident snapshot row: one entry per node in the e-graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SnapshotRow {
    /// E-class id this node belongs to (post-canonicalisation).
    pub eclass_id: u32,
    /// Stable language-op id (e.g. `BinOp::Add` → 1, `Load` → 2).
    /// The `OpIdRegistry` maintains the assignment.
    pub language_op_id: u32,
    /// Offset into the snapshot's `children` column where this
    /// node's child eclass ids start.
    pub children_offset: u32,
    /// Number of children (consecutive in the `children` column).
    pub children_len: u32,
}

/// One discovered equivalence (e-class merge candidate) produced by a
/// saturation pass. The CPU merges these back into the `EGraph`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Equivalence {
    /// Left e-class id.
    pub left: u32,
    /// Right e-class id (to be merged with left).
    pub right: u32,
}

/// Columnar GPU-uploadable mirror of an e-graph.
#[derive(Clone, Debug, Default)]
pub struct GpuEGraphSnapshot {
    /// Per-node rows in `(eclass_id, language_op_id, offset, len)` form.
    pub rows: Vec<SnapshotRow>,
    /// Flat children column. `rows[i]` references children at
    /// `children[rows[i].children_offset..rows[i].children_offset + rows[i].children_len]`.
    pub children: Vec<u32>,
    /// Op-id assignment used by `language_op_id`. Stable for the
    /// life of the snapshot.
    pub op_ids: OpIdRegistry,
}

/// Error returned when a CPU e-graph cannot be represented by the current
/// 32-bit GPU snapshot ABI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuEGraphSnapshotError {
    context: &'static str,
    value: usize,
}

impl GpuEGraphSnapshotError {
    fn new(context: &'static str, value: usize) -> Self {
        Self { context, value }
    }

    /// Human-readable conversion context.
    #[must_use]
    pub const fn context(&self) -> &'static str {
        self.context
    }

    /// Host-side value that could not fit the GPU snapshot ABI.
    #[must_use]
    pub const fn value(&self) -> usize {
        self.value
    }
}

impl fmt::Display for GpuEGraphSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GPU e-graph snapshot {} value {} exceeds the u32 column ABI. Fix: shard the e-graph snapshot or widen the GPU snapshot ABI before upload.",
            self.context, self.value
        )
    }
}

impl std::error::Error for GpuEGraphSnapshotError {}

/// Error returned when a GPU e-graph snapshot is structurally malformed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuEGraphSnapshotIntegrityError {
    context: &'static str,
    row: usize,
    value: u32,
}

impl GpuEGraphSnapshotIntegrityError {
    fn new(context: &'static str, row: usize, value: u32) -> Self {
        Self {
            context,
            row,
            value,
        }
    }

    /// Human-readable validation context.
    #[must_use]
    pub const fn context(&self) -> &'static str {
        self.context
    }

    /// Snapshot row that failed validation.
    #[must_use]
    pub const fn row(&self) -> usize {
        self.row
    }

    /// Row-local value that failed validation.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.value
    }
}

impl fmt::Display for GpuEGraphSnapshotIntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GPU e-graph snapshot integrity error at row {}: {} value {} is invalid. Fix: rebuild the snapshot from canonical e-graph rows before upload.",
            self.row, self.context, self.value
        )
    }
}

impl std::error::Error for GpuEGraphSnapshotIntegrityError {}

/// Error returned when a GPU e-graph snapshot cannot be packed for upload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuEGraphDeviceImageError {
    /// Snapshot rows are structurally malformed.
    Integrity(GpuEGraphSnapshotIntegrityError),
    /// Snapshot or derived index columns exceed the current u32 device ABI.
    Layout(GpuEGraphSnapshotError),
}

impl fmt::Display for GpuEGraphDeviceImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integrity(error) => error.fmt(f),
            Self::Layout(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for GpuEGraphDeviceImageError {}

impl From<GpuEGraphSnapshotIntegrityError> for GpuEGraphDeviceImageError {
    fn from(error: GpuEGraphSnapshotIntegrityError) -> Self {
        Self::Integrity(error)
    }
}

impl From<GpuEGraphSnapshotError> for GpuEGraphDeviceImageError {
    fn from(error: GpuEGraphSnapshotError) -> Self {
        Self::Layout(error)
    }
}

/// Error returned by the measured CPU/GPU e-graph bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuEGraphBridgeError {
    /// CPU e-graph snapshot construction failed.
    Snapshot(GpuEGraphSnapshotError),
    /// Snapshot packing into the uploadable device image failed.
    DeviceImage(GpuEGraphDeviceImageError),
    /// CPU e-graph extraction failed during parity proof.
    EGraph(EGraphError),
}

impl fmt::Display for GpuEGraphBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot(error) => write!(f, "GPU e-graph bridge snapshot failed: {error}"),
            Self::DeviceImage(error) => {
                write!(f, "GPU e-graph bridge device image failed: {error}")
            }
            Self::EGraph(error) => write!(f, "GPU e-graph bridge extraction failed: {error}"),
        }
    }
}

impl std::error::Error for GpuEGraphBridgeError {}

impl From<GpuEGraphSnapshotError> for GpuEGraphBridgeError {
    fn from(error: GpuEGraphSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}

impl From<GpuEGraphDeviceImageError> for GpuEGraphBridgeError {
    fn from(error: GpuEGraphDeviceImageError) -> Self {
        Self::DeviceImage(error)
    }
}

impl From<EGraphError> for GpuEGraphBridgeError {
    fn from(error: EGraphError) -> Self {
        Self::EGraph(error)
    }
}

/// Contiguous span inside a packed GPU e-graph device image.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GpuEGraphDeviceSpan {
    offset: usize,
    len: usize,
}

impl GpuEGraphDeviceSpan {
    const fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }

    /// Word offset of the span inside the packed u32 slab.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Number of u32 words in the span.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` iff the span contains no words.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn slice<'a>(&self, words: &'a [u32]) -> &'a [u32] {
        &words[self.offset..self.offset + self.len]
    }
}

/// Segment table for a packed GPU e-graph device image.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GpuEGraphDeviceLayout {
    row_count: usize,
    child_count: usize,
    eclass_group_count: usize,
    row_eclass_ids: GpuEGraphDeviceSpan,
    row_language_op_ids: GpuEGraphDeviceSpan,
    row_children_offsets: GpuEGraphDeviceSpan,
    row_children_lens: GpuEGraphDeviceSpan,
    row_signatures: GpuEGraphDeviceSpan,
    children: GpuEGraphDeviceSpan,
    group_eclass_ids: GpuEGraphDeviceSpan,
    group_offsets: GpuEGraphDeviceSpan,
    group_rows: GpuEGraphDeviceSpan,
}

impl GpuEGraphDeviceLayout {
    /// Number of snapshot rows packed into the device image.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Number of child references packed into the device image.
    #[must_use]
    pub const fn child_count(&self) -> usize {
        self.child_count
    }

    /// Number of e-class row groups in the device image.
    #[must_use]
    pub const fn eclass_group_count(&self) -> usize {
        self.eclass_group_count
    }

    /// Span containing one e-class id per snapshot row.
    #[must_use]
    pub const fn row_eclass_ids(&self) -> GpuEGraphDeviceSpan {
        self.row_eclass_ids
    }

    /// Span containing one language op id per snapshot row.
    #[must_use]
    pub const fn row_language_op_ids(&self) -> GpuEGraphDeviceSpan {
        self.row_language_op_ids
    }

    /// Span containing one child-column offset per snapshot row.
    #[must_use]
    pub const fn row_children_offsets(&self) -> GpuEGraphDeviceSpan {
        self.row_children_offsets
    }

    /// Span containing one child count per snapshot row.
    #[must_use]
    pub const fn row_children_lens(&self) -> GpuEGraphDeviceSpan {
        self.row_children_lens
    }

    /// Span containing one structural row signature per snapshot row.
    #[must_use]
    pub const fn row_signatures(&self) -> GpuEGraphDeviceSpan {
        self.row_signatures
    }

    /// Span containing the flat child e-class column.
    #[must_use]
    pub const fn children(&self) -> GpuEGraphDeviceSpan {
        self.children
    }

    /// Span containing sorted e-class ids for row groups.
    #[must_use]
    pub const fn group_eclass_ids(&self) -> GpuEGraphDeviceSpan {
        self.group_eclass_ids
    }

    /// Span containing prefix offsets into [`Self::group_rows`].
    #[must_use]
    pub const fn group_offsets(&self) -> GpuEGraphDeviceSpan {
        self.group_offsets
    }

    /// Span containing row indices grouped by e-class.
    #[must_use]
    pub const fn group_rows(&self) -> GpuEGraphDeviceSpan {
        self.group_rows
    }
}

/// Validated, single-slab u32 image ready for backend upload.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GpuEGraphDeviceImage {
    words: Vec<u32>,
    layout: GpuEGraphDeviceLayout,
}

impl GpuEGraphDeviceImage {
    /// Packed u32 words. Backends can upload this slab with one host-to-device copy.
    #[must_use]
    pub fn words(&self) -> &[u32] {
        &self.words
    }

    /// Segment table describing the packed word slab.
    #[must_use]
    pub const fn layout(&self) -> GpuEGraphDeviceLayout {
        self.layout
    }

    /// One e-class id per snapshot row.
    #[must_use]
    pub fn row_eclass_ids(&self) -> &[u32] {
        self.layout.row_eclass_ids.slice(&self.words)
    }

    /// One language op id per snapshot row.
    #[must_use]
    pub fn row_language_op_ids(&self) -> &[u32] {
        self.layout.row_language_op_ids.slice(&self.words)
    }

    /// One child-column offset per snapshot row.
    #[must_use]
    pub fn row_children_offsets(&self) -> &[u32] {
        self.layout.row_children_offsets.slice(&self.words)
    }

    /// One child count per snapshot row.
    #[must_use]
    pub fn row_children_lens(&self) -> &[u32] {
        self.layout.row_children_lens.slice(&self.words)
    }

    /// One structural signature per snapshot row.
    #[must_use]
    pub fn row_signatures(&self) -> &[u32] {
        self.layout.row_signatures.slice(&self.words)
    }

    /// Flat child e-class column.
    #[must_use]
    pub fn children(&self) -> &[u32] {
        self.layout.children.slice(&self.words)
    }

    /// Sorted e-class ids for row groups.
    #[must_use]
    pub fn group_eclass_ids(&self) -> &[u32] {
        self.layout.group_eclass_ids.slice(&self.words)
    }

    /// Prefix offsets into [`Self::group_rows`].
    #[must_use]
    pub fn group_offsets(&self) -> &[u32] {
        self.layout.group_offsets.slice(&self.words)
    }

    /// Row indices grouped by e-class.
    #[must_use]
    pub fn group_rows(&self) -> &[u32] {
        self.layout.group_rows.slice(&self.words)
    }
}

/// Stable language-op id assignment used inside snapshot rows.
#[derive(Clone, Debug, Default)]
pub struct OpIdRegistry {
    by_name: FxHashMap<Arc<str>, u32>,
    names: Vec<Arc<str>>,
}

impl OpIdRegistry {
    /// Intern a language-op name and return its stable id.
    /// Repeated calls with the same name return the same id.
    ///
    /// Returns `u32::MAX` only when the registry has exceeded the current
    /// 32-bit GPU column ABI. Use [`Self::try_intern`] when the exact overflow
    /// reason must be surfaced.
    pub fn intern(&mut self, name: &str) -> u32 {
        match self.try_intern(name) {
            Ok(id) => id,
            Err(_) => u32::MAX,
        }
    }

    /// Fallible form of [`Self::intern`] for GPU snapshot builders that must
    /// reject over-wide op dictionaries instead of silently saturating ids.
    pub fn try_intern(&mut self, name: &str) -> Result<u32, GpuEGraphSnapshotError> {
        if let Some(&id) = self.by_name.get(name) {
            return Ok(id);
        }
        let id = u32_len(self.names.len(), "op-id registry")?;
        let name: Arc<str> = Arc::from(name);
        self.names.push(Arc::clone(&name));
        self.by_name.insert(name, id);
        Ok(id)
    }

    /// Resolve an op-id back to its name, or `None` if unknown.
    #[must_use]
    pub fn name_of(&self, id: u32) -> Option<&str> {
        self.names.get(id as usize).map(AsRef::as_ref)
    }

    /// Number of registered op names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// `true` iff zero op names registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl GpuEGraphSnapshot {
    /// Build a snapshot from a sequence of `(eclass_id, op_name,
    /// children: &[u32])` triples. Caller-driven construction so
    /// this module doesn't depend on the exact `eqsat::EGraph`
    /// internal shape; the `EGraph` crate's adapter calls this
    /// builder to materialise the GPU mirror.
    ///
    /// Returns an empty snapshot if the input exceeds the current 32-bit GPU
    /// column ABI. Use [`Self::try_build`] for actionable overflow diagnostics.
    #[must_use]
    pub fn build<'a, I>(rows: I) -> Self
    where
        I: IntoIterator<Item = (u32, &'a str, &'a [u32])>,
    {
        match Self::try_build(rows) {
            Ok(snapshot) => snapshot,
            Err(_) => Self::default(),
        }
    }

    /// Fallible form of [`Self::build`] that rejects snapshots too large for
    /// the current 32-bit GPU column ABI.
    pub fn try_build<'a, I>(rows: I) -> Result<Self, GpuEGraphSnapshotError>
    where
        I: IntoIterator<Item = (u32, &'a str, &'a [u32])>,
    {
        let mut snapshot = Self::default();
        let rows = rows.into_iter();
        let (lower_bound, _) = rows.size_hint();
        snapshot.rows.reserve(lower_bound);
        for (eclass_id, op_name, kids) in rows {
            let language_op_id = snapshot.op_ids.try_intern(op_name)?;
            let children_offset = u32_len(snapshot.children.len(), "GPU egraph children offset")?;
            let children_len = u32_len(kids.len(), "GPU egraph row child count")?;
            snapshot.children.extend_from_slice(kids);
            snapshot.rows.push(SnapshotRow {
                eclass_id,
                language_op_id,
                children_offset,
                children_len,
            });
        }
        Ok(snapshot)
    }

    /// Materialise a snapshot directly from the CPU `EGraph`.
    ///
    /// The caller supplies the stable operation-name projection because
    /// `ENodeLang` is intentionally domain-generic and does not require
    /// `Debug` or a string identity. Child ids are canonicalized during the
    /// copy so the GPU columns match the CPU graph's current union-find state.
    ///
    /// Returns an empty snapshot if the CPU e-graph exceeds the current 32-bit
    /// GPU column ABI. Use [`Self::try_from_egraph_with`] for actionable
    /// overflow diagnostics.
    #[must_use]
    pub fn from_egraph_with<L, F, S>(egraph: &EGraph<L>, mut op_name: F) -> Self
    where
        L: ENodeLang,
        F: FnMut(&L) -> S,
        S: AsRef<str>,
    {
        match Self::try_from_egraph_with(egraph, &mut op_name) {
            Ok(snapshot) => snapshot,
            Err(_) => Self::default(),
        }
    }

    /// Fallible form of [`Self::from_egraph_with`] that rejects CPU e-graphs
    /// whose node or child-column counts exceed the current 32-bit GPU ABI.
    pub fn try_from_egraph_with<L, F, S>(
        egraph: &EGraph<L>,
        mut op_name: F,
    ) -> Result<Self, GpuEGraphSnapshotError>
    where
        L: ENodeLang,
        F: FnMut(&L) -> S,
        S: AsRef<str>,
    {
        let mut snapshot = Self::default();
        snapshot.rows.reserve(egraph.class_count());
        for (eclass_id, node) in egraph.iter_nodes() {
            let language_op_id = snapshot.op_ids.try_intern(op_name(node).as_ref())?;
            let children = node.children();
            let children_offset = u32_len(snapshot.children.len(), "GPU egraph children offset")?;
            let children_len = u32_len(children.len(), "GPU egraph row child count")?;
            snapshot
                .children
                .extend(children.iter().map(|child| egraph.find_immut(*child).0));
            snapshot.rows.push(SnapshotRow {
                eclass_id: egraph.find_immut(eclass_id).0,
                language_op_id,
                children_offset,
                children_len,
            });
        }
        Ok(snapshot)
    }

    /// Number of nodes in the snapshot.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.rows.len()
    }

    /// `true` iff the snapshot contains no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Total number of children references across all rows.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Children of the row at `row_idx`, or `None` if the snapshot row
    /// references an invalid range.
    #[must_use]
    pub fn children_of(&self, row_idx: usize) -> Option<&[u32]> {
        let row = self.rows.get(row_idx)?;
        let start = row.children_offset as usize;
        let end = start.checked_add(row.children_len as usize)?;
        self.children.get(start..end)
    }

    /// Group rows by their `eclass_id`, returning a map of
    /// `eclass_id → Vec<row_idx>`. Useful for the GPU saturation
    /// kernel's per-eclass passes.
    #[must_use]
    pub fn rows_by_eclass(&self) -> FxHashMap<u32, Vec<usize>> {
        let mut out: FxHashMap<u32, Vec<usize>> =
            FxHashMap::with_capacity_and_hasher(self.rows.len(), Default::default());
        for (i, row) in self.rows.iter().enumerate() {
            out.entry(row.eclass_id).or_default().push(i);
        }
        out
    }

    /// Validate that the columnar snapshot is safe to upload to a GPU kernel.
    ///
    /// This checks every row's operation id, child-column range, and child
    /// e-class references. It is intentionally stricter than construction:
    /// callers may still build partial test fixtures, but production upload
    /// paths can require this gate before device execution.
    ///
    /// # Errors
    ///
    /// Returns [`GpuEGraphSnapshotIntegrityError`] when a row references an
    /// unknown op id, points outside the child column, or names a child e-class
    /// not present in the snapshot.
    pub fn validate_integrity(&self) -> Result<(), GpuEGraphSnapshotIntegrityError> {
        let mut eclasses: FxHashSet<u32> =
            FxHashSet::with_capacity_and_hasher(self.rows.len(), Default::default());
        for row in &self.rows {
            eclasses.insert(row.eclass_id);
        }
        for (row_idx, row) in self.rows.iter().enumerate() {
            if self.op_ids.name_of(row.language_op_id).is_none() {
                return Err(GpuEGraphSnapshotIntegrityError::new(
                    "unknown language_op_id",
                    row_idx,
                    row.language_op_id,
                ));
            }
            let start = row.children_offset as usize;
            let end = start
                .checked_add(row.children_len as usize)
                .ok_or_else(|| {
                    GpuEGraphSnapshotIntegrityError::new(
                        "children range overflow",
                        row_idx,
                        row.children_len,
                    )
                })?;
            if end > self.children.len() {
                return Err(GpuEGraphSnapshotIntegrityError::new(
                    "children range end",
                    row_idx,
                    row.children_len,
                ));
            }
            for &child in &self.children[start..end] {
                if !eclasses.contains(&child) {
                    return Err(GpuEGraphSnapshotIntegrityError::new(
                        "dangling child eclass",
                        row_idx,
                        child,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Pack the validated snapshot into one backend-uploadable u32 slab.
    ///
    /// The image contains row metadata columns, a structural row-signature
    /// column, the flat child column, and a deterministic e-class-to-row prefix
    /// index. CUDA and other backends can upload the returned
    /// [`GpuEGraphDeviceImage::words`] slice in one copy and pass the spans
    /// from [`GpuEGraphDeviceImage::layout`] as kernel parameters.
    ///
    /// # Errors
    ///
    /// Returns [`GpuEGraphDeviceImageError`] if the snapshot fails integrity
    /// validation or if a derived row/group index exceeds the u32 device ABI.
    pub fn try_pack_device_image(&self) -> Result<GpuEGraphDeviceImage, GpuEGraphDeviceImageError> {
        self.validate_integrity()?;

        let mut groups: FxHashMap<u32, Vec<u32>> =
            FxHashMap::with_capacity_and_hasher(self.rows.len(), Default::default());
        for (row_idx, row) in self.rows.iter().enumerate() {
            groups
                .entry(row.eclass_id)
                .or_default()
                .push(u32_len(row_idx, "GPU egraph grouped row index")?);
        }

        let mut group_eclass_ids = groups.keys().copied().collect::<Vec<_>>();
        group_eclass_ids.sort_unstable();

        let mut group_offsets = Vec::with_capacity(group_eclass_ids.len() + 1);
        let mut group_rows = Vec::with_capacity(self.rows.len());
        for eclass_id in &group_eclass_ids {
            group_offsets.push(u32_len(group_rows.len(), "GPU egraph group row offset")?);
            let Some(rows) = groups.get(eclass_id) else {
                return Err(GpuEGraphSnapshotIntegrityError::new(
                    "missing grouped eclass key",
                    0,
                    *eclass_id,
                )
                .into());
            };
            group_rows.extend_from_slice(rows);
        }
        group_offsets.push(u32_len(
            group_rows.len(),
            "GPU egraph group row terminal offset",
        )?);

        let row_signatures = self
            .rows
            .iter()
            .map(|row| {
                let start = row.children_offset as usize;
                let end = start + row.children_len as usize;
                egraph_row_signature(row, &self.children[start..end])
            })
            .collect::<Vec<_>>();
        let mut words = Vec::with_capacity(
            self.rows.len() * 5
                + self.children.len()
                + group_eclass_ids.len()
                + group_offsets.len()
                + group_rows.len(),
        );
        let row_eclass_ids = append_words(&mut words, self.rows.iter().map(|row| row.eclass_id));
        let row_language_op_ids =
            append_words(&mut words, self.rows.iter().map(|row| row.language_op_id));
        let row_children_offsets =
            append_words(&mut words, self.rows.iter().map(|row| row.children_offset));
        let row_children_lens =
            append_words(&mut words, self.rows.iter().map(|row| row.children_len));
        let row_signatures = append_words(&mut words, row_signatures);
        let children = append_words(&mut words, self.children.iter().copied());
        let group_eclass_ids_span = append_words(&mut words, group_eclass_ids);
        let group_offsets = append_words(&mut words, group_offsets);
        let group_rows = append_words(&mut words, group_rows);

        Ok(GpuEGraphDeviceImage {
            words,
            layout: GpuEGraphDeviceLayout {
                row_count: self.rows.len(),
                child_count: self.children.len(),
                eclass_group_count: groups.len(),
                row_eclass_ids,
                row_language_op_ids,
                row_children_offsets,
                row_children_lens,
                row_signatures,
                children,
                group_eclass_ids: group_eclass_ids_span,
                group_offsets,
                group_rows,
            },
        })
    }

    /// Compatibility wrapper for [`Self::try_pack_device_image`].
    ///
    /// Returns an empty image when the snapshot cannot be represented by the
    /// current 32-bit GPU column ABI. Call [`Self::try_pack_device_image`] when
    /// the exact overflow reason must be surfaced to the operator.
    #[must_use]
    pub fn pack_device_image(&self) -> GpuEGraphDeviceImage {
        match self.try_pack_device_image() {
            Ok(image) => image,
            Err(_) => GpuEGraphDeviceImage::default(),
        }
    }
}

/// Report returned after applying discovered equivalences to an `EGraph`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApplyEquivalencesReport {
    /// Input equivalence count.
    pub requested: usize,
    /// Equivalences whose e-class ids existed in the target `EGraph`.
    pub valid: usize,
    /// Direct union operations that changed the union-find root.
    pub merged: usize,
    /// Additional unions discovered during `EGraph::rebuild`.
    pub rebuild_unions: usize,
}

/// Measured bridge report for the CPU e-graph to GPU-columnar equivalence path.
///
/// The bridge accepts equivalences as if they were produced by a backend GPU
/// kernel, applies them through the canonical CPU merge API, and compares that
/// result against an independent CPU parity clone. This proves the snapshot,
/// packing, equivalence merge, and extraction seams without fabricating device
/// execution when no backend kernel was launched.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GpuEGraphBridgeReport {
    /// Number of rows in the compact GPU snapshot.
    pub snapshot_rows: usize,
    /// Number of child references in the compact GPU snapshot.
    pub snapshot_children: usize,
    /// Number of u32 words in the packed upload slab.
    pub device_words: usize,
    /// Number of e-class row groups in the packed upload slab.
    pub device_eclass_groups: usize,
    /// Input equivalence count.
    pub equivalences_requested: usize,
    /// Equivalences with valid e-class ids on the GPU-equivalence path.
    pub equivalences_valid: usize,
    /// State-changing merges on the GPU-equivalence path.
    pub equivalences_merged: usize,
    /// Additional rebuild unions on the GPU-equivalence path.
    pub rebuild_unions: usize,
    /// Equivalences with valid e-class ids on the CPU parity path.
    pub cpu_equivalences_valid: usize,
    /// State-changing merges on the CPU parity path.
    pub cpu_equivalences_merged: usize,
    /// Additional rebuild unions on the CPU parity path.
    pub cpu_rebuild_unions: usize,
    /// Snapshot construction time in nanoseconds.
    pub snapshot_ns: u64,
    /// Device-image packing time in nanoseconds.
    pub pack_ns: u64,
    /// CPU parity equivalence application time in nanoseconds.
    pub cpu_apply_ns: u64,
    /// GPU-equivalence application time in nanoseconds.
    pub gpu_apply_ns: u64,
    /// CPU parity extraction time in nanoseconds.
    pub cpu_extraction_ns: u64,
    /// GPU-equivalence extraction time in nanoseconds.
    pub gpu_extraction_ns: u64,
    /// Best extraction cost after the CPU parity apply.
    pub cpu_extraction_cost: Option<u64>,
    /// Best extraction cost after the GPU-equivalence apply.
    pub gpu_extraction_cost: Option<u64>,
    /// True when CPU parity and GPU-equivalence paths converge on the same
    /// apply report and extracted node/cost.
    pub recall_parity: bool,
    /// True when repeated snapshot packing produces identical class grouping
    /// columns for the same CPU e-graph.
    pub class_id_deterministic: bool,
}

/// Build a compact GPU e-graph image, apply backend-discovered equivalences
/// through the canonical CPU merge API, and prove extraction parity against an
/// independent CPU clone.
///
/// This function is the host-side bridge for GPU e-graph work: it measures the
/// CPU snapshot and packing cost, records the time to apply equivalences
/// returned by a backend, and checks that the resulting extraction matches a
/// CPU parity path that applied the same equivalences.
///
/// # Errors
///
/// Returns [`GpuEGraphBridgeError`] if snapshot construction, device packing, or
/// parity extraction fails.
pub fn bridge_equivalence_batch_with_report<L, F, S, C>(
    egraph: &mut EGraph<L>,
    root: EClassId,
    op_name: F,
    equivalences: &[Equivalence],
    cost_fn: C,
) -> Result<GpuEGraphBridgeReport, GpuEGraphBridgeError>
where
    L: ENodeLang,
    F: Fn(&L) -> S,
    S: AsRef<str>,
    C: Fn(&L) -> u64 + Copy,
{
    let snapshot_start = Instant::now();
    let snapshot = GpuEGraphSnapshot::try_from_egraph_with(egraph, |node| op_name(node))?;
    let snapshot_ns = elapsed_nonzero_ns(snapshot_start);

    let deterministic_snapshot =
        GpuEGraphSnapshot::try_from_egraph_with(egraph, |node| op_name(node))?;

    let pack_start = Instant::now();
    let image = snapshot.try_pack_device_image()?;
    let pack_ns = elapsed_nonzero_ns(pack_start);
    let deterministic_image = deterministic_snapshot.try_pack_device_image()?;
    let class_id_deterministic = image.group_eclass_ids() == deterministic_image.group_eclass_ids()
        && image.group_offsets() == deterministic_image.group_offsets()
        && image.group_rows() == deterministic_image.group_rows();

    let mut cpu_parity = egraph.clone();
    let cpu_apply_start = Instant::now();
    let cpu_apply = apply_equivalences_to_egraph(&mut cpu_parity, equivalences);
    let cpu_apply_ns = elapsed_nonzero_ns(cpu_apply_start);

    let gpu_apply_start = Instant::now();
    let gpu_apply = apply_equivalences_to_egraph(egraph, equivalences);
    let gpu_apply_ns = elapsed_nonzero_ns(gpu_apply_start);

    let cpu_extraction_start = Instant::now();
    let cpu_extraction = try_extract_best_with_budget(
        &cpu_parity,
        root,
        |node| cost_fn(node),
        DEFAULT_EXTRACTION_ITER_BUDGET,
    )?;
    let cpu_extraction_ns = elapsed_nonzero_ns(cpu_extraction_start);

    let gpu_extraction_start = Instant::now();
    let gpu_extraction = try_extract_best_with_budget(
        egraph,
        root,
        |node| cost_fn(node),
        DEFAULT_EXTRACTION_ITER_BUDGET,
    )?;
    let gpu_extraction_ns = elapsed_nonzero_ns(gpu_extraction_start);

    let cpu_extraction_cost = cpu_extraction.best.as_ref().map(|(_, cost)| *cost);
    let gpu_extraction_cost = gpu_extraction.best.as_ref().map(|(_, cost)| *cost);
    let recall_parity = cpu_apply == gpu_apply && cpu_extraction.best == gpu_extraction.best;

    Ok(GpuEGraphBridgeReport {
        snapshot_rows: snapshot.node_count(),
        snapshot_children: snapshot.child_count(),
        device_words: image.words().len(),
        device_eclass_groups: image.layout().eclass_group_count(),
        equivalences_requested: gpu_apply.requested,
        equivalences_valid: gpu_apply.valid,
        equivalences_merged: gpu_apply.merged,
        rebuild_unions: gpu_apply.rebuild_unions,
        cpu_equivalences_valid: cpu_apply.valid,
        cpu_equivalences_merged: cpu_apply.merged,
        cpu_rebuild_unions: cpu_apply.rebuild_unions,
        snapshot_ns,
        pack_ns,
        cpu_apply_ns,
        gpu_apply_ns,
        cpu_extraction_ns,
        gpu_extraction_ns,
        cpu_extraction_cost,
        gpu_extraction_cost,
        recall_parity,
        class_id_deterministic,
    })
}

/// Apply a batch of GPU-discovered equivalences to a CPU-side
/// merge sink. The `merger` closure receives `(left, right)` and
/// performs the canonical `EGraph` merge. Returns the number of
/// merges that actually changed the union-find state (the merger
/// returns `true` for a state-changing merge, `false` for a no-op
/// where left and right were already in the same e-class).
pub fn apply_equivalences<F>(equivalences: &[Equivalence], mut merger: F) -> usize
where
    F: FnMut(u32, u32) -> bool,
{
    let mut applied = 0usize;
    for eq in equivalences {
        if merger(eq.left, eq.right) {
            applied += 1;
        }
    }
    applied
}

/// Apply discovered equivalences to the CPU `EGraph` and rebuild it once.
///
/// Invalid e-class ids are counted as requested but not applied; user input
/// must not be able to panic the optimizer by returning an out-of-range merge.
pub fn apply_equivalences_to_egraph<L>(
    egraph: &mut EGraph<L>,
    equivalences: &[Equivalence],
) -> ApplyEquivalencesReport
where
    L: ENodeLang,
{
    let mut report = ApplyEquivalencesReport {
        requested: equivalences.len(),
        ..ApplyEquivalencesReport::default()
    };
    let Ok(class_count) = u32_len(egraph.class_count(), "CPU egraph class count") else {
        return report;
    };
    for eq in equivalences {
        if eq.left >= class_count || eq.right >= class_count {
            continue;
        }
        report.valid += 1;
        let left = EClassId(eq.left);
        let right = EClassId(eq.right);
        if egraph.find(left) != egraph.find(right) {
            egraph.union(left, right);
            report.merged += 1;
        }
    }
    report.rebuild_unions = egraph.rebuild();
    report
}

#[inline]
fn u32_len(value: usize, context: &'static str) -> Result<u32, GpuEGraphSnapshotError> {
    u32::try_from(value).map_err(|_| GpuEGraphSnapshotError::new(context, value))
}

fn append_words<I>(words: &mut Vec<u32>, values: I) -> GpuEGraphDeviceSpan
where
    I: IntoIterator<Item = u32>,
{
    let offset = words.len();
    words.extend(values);
    GpuEGraphDeviceSpan::new(offset, words.len() - offset)
}

fn elapsed_nonzero_ns(start: Instant) -> u64 {
    let ns = start.elapsed().as_nanos();
    u64::try_from(ns).unwrap_or(u64::MAX).max(1)
}

/// Structural row signature for packed GPU e-graph columns.
///
/// Matches the CUDA row-signature refresh kernel and initial image packing.
#[must_use]
pub fn gpu_egraph_row_signature(language_op_id: u32, children_len: u32, children: &[u32]) -> u32 {
    let mut hash = mix_egraph_signature(0xA24B_AED4, language_op_id);
    hash = mix_egraph_signature(hash, children_len);
    for &child in children {
        hash = mix_egraph_signature(hash, child);
    }
    hash
}

fn egraph_row_signature(row: &SnapshotRow, children: &[u32]) -> u32 {
    gpu_egraph_row_signature(row.language_op_id, row.children_len, children)
}

fn mix_egraph_signature(hash: u32, value: u32) -> u32 {
    let mixed = hash
        ^ value
            .wrapping_add(0x9E37_79B9)
            .wrapping_add(hash << 6)
            .wrapping_add(hash >> 2);
    mixed.rotate_left(13).wrapping_mul(0x85EB_CA6B)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    #[derive(Clone, Debug, Eq, Hash, PartialEq)]
    enum TinyLang {
        Lit(u32),
        Add(EClassId, EClassId),
    }

    impl ENodeLang for TinyLang {
        fn children(&self) -> super::super::eqsat::EChildren {
            match self {
                Self::Lit(_) => super::super::eqsat::EChildren::new(),
                Self::Add(left, right) => [*left, *right].into_iter().collect(),
            }
        }

        fn with_children(&self, children: &[EClassId]) -> Self {
            match self {
                Self::Lit(value) => Self::Lit(*value),
                Self::Add(_, _) => Self::Add(children[0], children[1]),
            }
        }
    }

    fn tiny_op_name(node: &TinyLang) -> &'static str {
        match node {
            TinyLang::Lit(_) => "lit",
            TinyLang::Add(_, _) => "add",
        }
    }

    fn tiny_cost(node: &TinyLang) -> u64 {
        match node {
            TinyLang::Lit(_) => 1,
            TinyLang::Add(_, _) => 4,
        }
    }

    /// Empty snapshot: zero rows, zero children, registry empty.
    #[test]
    fn empty_snapshot() {
        let snap = GpuEGraphSnapshot::default();
        assert!(snap.is_empty());
        assert_eq!(snap.node_count(), 0);
        assert_eq!(snap.child_count(), 0);
        assert!(snap.op_ids.is_empty());
    }

    /// Build a 3-node snapshot via the iterator builder; assert
    /// row layout + children column line up.
    #[test]
    fn build_three_node_snapshot() {
        let snap = GpuEGraphSnapshot::build([
            (0u32, "lit_u32", &[][..]),
            (1u32, "lit_u32", &[][..]),
            (2u32, "binop_add", &[0u32, 1u32][..]),
        ]);
        assert_eq!(snap.node_count(), 3);
        assert_eq!(snap.child_count(), 2);
        let empty: &[u32] = &[];
        assert_eq!(snap.children_of(0), Some(empty));
        assert_eq!(snap.children_of(1), Some(empty));
        assert_eq!(snap.children_of(2), Some(&[0, 1][..]));
        assert_eq!(snap.children_of(99), None);
    }

    /// `OpIdRegistry::intern` returns the same id for repeated
    /// names.
    #[test]
    fn op_id_intern_dedups() {
        let mut reg = OpIdRegistry::default();
        let a = reg.intern("foo");
        let b = reg.intern("bar");
        let c = reg.intern("foo");
        assert_eq!(a, c);
        assert_ne!(a, b);
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.name_of(a), Some("foo"));
        assert_eq!(reg.name_of(b), Some("bar"));
        assert_eq!(reg.name_of(99), None);
    }

    #[test]
    fn gpu_snapshot_u32_layout_conversion_rejects_overflow() {
        let error = u32_len(u32::MAX as usize + 1, "test overflow")
            .expect_err("Fix: GPU e-graph snapshot must not silently saturate oversized columns");

        assert_eq!(error.context(), "test overflow");
        assert_eq!(error.value(), u32::MAX as usize + 1);
        assert!(
            error.to_string().contains("shard the e-graph snapshot")
                && error.to_string().contains("widen the GPU snapshot ABI"),
            "oversized GPU snapshot errors must explain both viable fixes"
        );
    }

    #[test]
    fn gpu_snapshot_builders_use_fallible_u32_conversion_not_saturation() {
        let source = include_str!("eqsat_gpu.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: production eqsat_gpu section must exist");
        assert!(
            source.contains("pub fn try_build")
                && source.contains("pub fn try_from_egraph_with")
                && source.contains("snapshot.op_ids.try_intern")
                && source.contains("u32::try_from(value).map_err")
                && !source.contains(concat!("unwrap_or", "(u32::MAX)")),
            "Fix: GPU e-graph snapshots must reject oversized u32 ABI fields instead of saturating them to u32::MAX."
        );
        assert!(
            !production.contains(".expect("),
            "Fix: GPU e-graph snapshot production paths must return structured errors instead of panicking."
        );
    }

    /// `rows_by_eclass` groups multi-row e-classes.
    #[test]
    fn rows_by_eclass_groups_correctly() {
        let snap = GpuEGraphSnapshot::build([
            (0u32, "lit_u32", &[][..]),
            (0u32, "var", &[][..]),
            (1u32, "binop_add", &[0u32][..]),
        ]);
        let groups = snap.rows_by_eclass();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get(&0).unwrap().len(), 2);
        assert_eq!(groups.get(&1).unwrap().len(), 1);
    }

    #[test]
    fn generated_snapshot_integrity_accepts_pack_boundaries_and_forward_children() {
        for node_count in [1_usize, 2, 7, 8, 9, 16, 17, 31, 32, 33, 65, 128] {
            let mut rows = Vec::with_capacity(node_count);
            let mut child_storage = Vec::new();
            for row in 0..node_count {
                let start = child_storage.len();
                if row > 0 {
                    child_storage.push((row - 1) as u32);
                }
                if row > 1 && row % 3 == 0 {
                    child_storage.push((row / 2) as u32);
                }
                rows.push((
                    row as u32,
                    if row % 2 == 0 { "lit" } else { "add" },
                    start,
                    child_storage.len() - start,
                ));
            }
            let build_rows = rows
                .iter()
                .map(|&(class, op, start, len)| (class, op, &child_storage[start..start + len]))
                .collect::<Vec<_>>();
            let snapshot = GpuEGraphSnapshot::build(build_rows);

            snapshot
                .validate_integrity()
                .unwrap_or_else(|error| panic!("node_count={node_count}: {error}"));
        }
    }

    #[test]
    fn snapshot_integrity_rejects_unknown_op_id() {
        let mut snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..])]);
        snapshot.rows[0].language_op_id = 99;

        let error = snapshot
            .validate_integrity()
            .expect_err("Fix: malformed GPU snapshot op ids must be rejected before upload.");

        assert_eq!(error.context(), "unknown language_op_id");
        assert_eq!(error.row(), 0);
        assert_eq!(error.value(), 99);
    }

    #[test]
    fn snapshot_integrity_rejects_out_of_bounds_child_range() {
        let mut snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..])]);
        snapshot.rows[0].children_offset = 1;
        snapshot.rows[0].children_len = 1;

        let error = snapshot
            .validate_integrity()
            .expect_err("Fix: malformed GPU snapshot child ranges must be rejected before upload.");

        assert_eq!(error.context(), "children range end");
        assert_eq!(error.row(), 0);
    }

    #[test]
    fn snapshot_integrity_rejects_dangling_child_eclass() {
        let snapshot =
            GpuEGraphSnapshot::build([(0u32, "lit", &[][..]), (1u32, "add", &[0u32, 99u32][..])]);

        let error = snapshot.validate_integrity().expect_err(
            "Fix: malformed GPU snapshot child eclasses must be rejected before upload.",
        );

        assert_eq!(error.context(), "dangling child eclass");
        assert_eq!(error.row(), 1);
        assert_eq!(error.value(), 99);
    }

    #[test]
    fn device_image_packs_single_upload_slab_with_sorted_group_index() {
        let snapshot = GpuEGraphSnapshot::build([
            (2u32, "lit", &[][..]),
            (1u32, "lit", &[][..]),
            (2u32, "add", &[1u32, 2u32][..]),
        ]);

        let image = snapshot
            .try_pack_device_image()
            .expect("Fix: valid GPU e-graph snapshot must pack into a device image");
        let layout = image.layout();

        assert_eq!(layout.row_count(), 3);
        assert_eq!(layout.child_count(), 2);
        assert_eq!(layout.eclass_group_count(), 2);
        assert_eq!(image.row_eclass_ids(), &[2, 1, 2]);
        assert_eq!(image.row_language_op_ids(), &[0, 0, 1]);
        assert_eq!(image.row_children_offsets(), &[0, 0, 0]);
        assert_eq!(image.row_children_lens(), &[0, 0, 2]);
        assert_eq!(image.row_signatures().len(), 3);
        assert_ne!(image.row_signatures()[0], image.row_signatures()[2]);
        assert_eq!(image.children(), &[1, 2]);
        assert_eq!(image.group_eclass_ids(), &[1, 2]);
        assert_eq!(image.group_offsets(), &[0, 1, 3]);
        assert_eq!(image.group_rows(), &[1, 0, 2]);
        assert_eq!(
            image.words().len(),
            layout.row_eclass_ids().len()
                + layout.row_language_op_ids().len()
                + layout.row_children_offsets().len()
                + layout.row_children_lens().len()
                + layout.row_signatures().len()
                + layout.children().len()
                + layout.group_eclass_ids().len()
                + layout.group_offsets().len()
                + layout.group_rows().len()
        );
    }

    #[test]
    fn generated_device_image_pack_accepts_empty_and_power_boundaries() {
        for node_count in [0_usize, 1, 2, 7, 8, 9, 31, 32, 33, 127, 128, 129] {
            let mut rows = Vec::with_capacity(node_count);
            let mut child_storage = Vec::new();
            for row in 0..node_count {
                let start = child_storage.len();
                if row > 0 {
                    child_storage.push((row - 1) as u32);
                }
                rows.push((
                    row as u32,
                    if row & 1 == 0 { "lit" } else { "neg" },
                    start,
                    child_storage.len() - start,
                ));
            }
            let build_rows = rows
                .iter()
                .map(|&(class, op, start, len)| (class, op, &child_storage[start..start + len]))
                .collect::<Vec<_>>();
            let snapshot = GpuEGraphSnapshot::build(build_rows);

            let image = snapshot
                .try_pack_device_image()
                .unwrap_or_else(|error| panic!("node_count={node_count}: {error}"));

            assert_eq!(image.layout().row_count(), node_count);
            assert_eq!(image.row_eclass_ids().len(), node_count);
            assert_eq!(image.row_language_op_ids().len(), node_count);
            assert_eq!(image.row_signatures().len(), node_count);
            assert_eq!(image.group_rows().len(), node_count);
            assert_eq!(image.group_offsets().len(), node_count + 1);
        }
    }

    #[test]
    fn row_signatures_group_structural_duplicates_without_eclass_identity() {
        let snapshot = GpuEGraphSnapshot::build([
            (1u32, "lit", &[][..]),
            (2u32, "lit", &[][..]),
            (10u32, "add", &[1u32, 2u32][..]),
            (11u32, "add", &[1u32, 2u32][..]),
            (12u32, "add", &[2u32, 1u32][..]),
            (13u32, "mul", &[1u32, 2u32][..]),
        ]);

        let image = snapshot
            .try_pack_device_image()
            .expect("Fix: valid duplicate-signature snapshot must pack");

        assert_eq!(image.row_signatures()[2], image.row_signatures()[3]);
        assert_ne!(image.row_signatures()[2], image.row_signatures()[4]);
        assert_ne!(image.row_signatures()[2], image.row_signatures()[5]);
    }

    #[test]
    fn device_image_rejects_malformed_snapshot_before_pack() {
        let mut snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..])]);
        snapshot.rows[0].language_op_id = 42;

        let error = snapshot
            .try_pack_device_image()
            .expect_err("Fix: device image packing must reject malformed snapshots");

        match error {
            GpuEGraphDeviceImageError::Integrity(error) => {
                assert_eq!(error.context(), "unknown language_op_id");
                assert_eq!(error.row(), 0);
                assert_eq!(error.value(), 42);
            }
            GpuEGraphDeviceImageError::Layout(error) => {
                panic!("expected integrity error, got layout error: {error}")
            }
        }
    }

    /// Snapshot directly from the CPU EGraph canonicalizes children and
    /// assigns stable operation ids.
    #[test]
    fn snapshot_from_egraph_uses_canonical_children() {
        let mut egraph = EGraph::new();
        let a = egraph.add(TinyLang::Lit(1));
        let b = egraph.add(TinyLang::Lit(2));
        let add = egraph.add(TinyLang::Add(a, b));
        assert_eq!(add.0, 2);

        let snap = GpuEGraphSnapshot::from_egraph_with(&egraph, |node| match node {
            TinyLang::Lit(_) => "lit",
            TinyLang::Add(_, _) => "add",
        });

        assert_eq!(snap.node_count(), 3);
        assert_eq!(snap.child_count(), 2);
        assert_eq!(snap.op_ids.name_of(0), Some("lit"));
        assert_eq!(snap.op_ids.name_of(1), Some("add"));
        assert_eq!(snap.children_of(2), Some(&[0, 1][..]));
    }

    /// `apply_equivalences` calls the merger for each equivalence
    /// and counts state-changing merges.
    #[test]
    fn apply_equivalences_counts_state_changes() {
        let equivalences = vec![
            Equivalence { left: 0, right: 1 },
            Equivalence { left: 1, right: 0 }, // no-op (already merged)
            Equivalence { left: 2, right: 3 },
        ];
        let mut canonical: FxHashMap<u32, u32> = FxHashMap::default();
        let applied = apply_equivalences(&equivalences, |a, b| {
            let canon_a = *canonical.get(&a).unwrap_or(&a);
            let canon_b = *canonical.get(&b).unwrap_or(&b);
            if canon_a == canon_b {
                false
            } else {
                let (lo, hi) = if canon_a < canon_b {
                    (canon_a, canon_b)
                } else {
                    (canon_b, canon_a)
                };
                canonical.insert(hi, lo);
                canonical.insert(a, lo);
                canonical.insert(b, lo);
                true
            }
        });
        assert_eq!(applied, 2);
    }

    /// Empty equivalence batch is a no-op.
    #[test]
    fn apply_equivalences_empty_batch() {
        let applied = apply_equivalences(&[], |_, _| true);
        assert_eq!(applied, 0);
    }

    /// EGraph merge bridge ignores invalid ids and rebuilds after valid
    /// merges.
    #[test]
    fn apply_equivalences_to_egraph_merges_valid_ids() {
        let mut egraph = EGraph::new();
        let a = egraph.add(TinyLang::Lit(1));
        let b = egraph.add(TinyLang::Lit(2));
        let c = egraph.add(TinyLang::Lit(3));
        let report = apply_equivalences_to_egraph(
            &mut egraph,
            &[
                Equivalence {
                    left: a.0,
                    right: b.0,
                },
                Equivalence {
                    left: c.0,
                    right: 99,
                },
            ],
        );
        assert_eq!(
            report,
            ApplyEquivalencesReport {
                requested: 2,
                valid: 1,
                merged: 1,
                rebuild_unions: 0,
            }
        );
        assert_eq!(egraph.find(a), egraph.find(b));
        assert_ne!(egraph.find(a), egraph.find(c));
    }

    #[test]
    fn gpu_egraph_bridge_reports_compact_columns_apply_and_extraction_parity() {
        let mut egraph = EGraph::new();
        let one = egraph.add(TinyLang::Lit(1));
        let two = egraph.add(TinyLang::Lit(2));
        let add = egraph.add(TinyLang::Add(one, two));
        let folded = egraph.add(TinyLang::Lit(3));

        let report = bridge_equivalence_batch_with_report(
            &mut egraph,
            add,
            tiny_op_name,
            &[Equivalence {
                left: add.0,
                right: folded.0,
            }],
            tiny_cost,
        )
        .expect("Fix: valid GPU e-graph bridge probe must produce a parity report");

        assert_eq!(report.snapshot_rows, 4);
        assert_eq!(report.snapshot_children, 2);
        assert!(report.device_words > report.snapshot_rows);
        assert_eq!(report.device_eclass_groups, 4);
        assert_eq!(report.equivalences_requested, 1);
        assert_eq!(report.equivalences_valid, 1);
        assert_eq!(report.equivalences_merged, 1);
        assert_eq!(report.cpu_equivalences_valid, 1);
        assert_eq!(report.cpu_equivalences_merged, 1);
        assert_eq!(report.cpu_extraction_cost, Some(1));
        assert_eq!(report.gpu_extraction_cost, Some(1));
        assert!(report.snapshot_ns > 0);
        assert!(report.pack_ns > 0);
        assert!(report.cpu_apply_ns > 0);
        assert!(report.gpu_apply_ns > 0);
        assert!(report.cpu_extraction_ns > 0);
        assert!(report.gpu_extraction_ns > 0);
        assert!(report.recall_parity);
        assert!(report.class_id_deterministic);
    }
}
