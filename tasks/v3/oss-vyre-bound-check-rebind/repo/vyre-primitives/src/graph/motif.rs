//! `motif`  -  intersect edge witnesses for a small graph pattern.
//!
//! Each motif edge is checked independently against the canonical
//! ProgramGraph CSR. If every requested motif edge exists, every
//! endpoint participating in the motif is marked in the final witness.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::graph::program_graph::{
    ProgramGraphShape, BINDING_PRIMITIVE_START, NAME_EDGE_KIND_MASK, NAME_EDGE_OFFSETS,
    NAME_EDGE_TARGETS,
};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::graph::motif";
/// Canonical binding index for motif scratch hits.
pub const MOTIF_HITS_BUFFER: u32 = BINDING_PRIMITIVE_START;
/// Canonical binding index for the public witness output.
pub const MOTIF_WITNESS_OUT_BUFFER: u32 = BINDING_PRIMITIVE_START + 1;
/// Motif matching is serial over the small pattern by construction.
pub const MOTIF_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];
/// Canonical motif dispatch grid.
pub const MOTIF_DISPATCH_GRID: [u32; 3] = [1, 1, 1];

/// Validated motif dispatch layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotifLayout {
    /// Number of graph nodes and output words.
    pub node_count: u32,
    /// Number of graph nodes and output words, widened for host buffer sizing.
    pub output_words: usize,
    /// Number of physical CSR edges.
    pub edge_count: u32,
    /// Number of u32 words required by physical edge buffers after padding.
    pub edge_storage_words: usize,
    /// Number of requested motif edges.
    pub motif_edge_count: u32,
}

/// One directed motif edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotifEdge {
    /// Source node id.
    pub from: u32,
    /// Edge-kind mask that must match.
    pub kind_mask: u32,
    /// Destination node id.
    pub to: u32,
}

/// Primitive-owned cache identity for motif Programs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MotifProgramCacheKey {
    /// Number of graph nodes baked into the generated Program.
    pub node_count: u32,
    /// Number of physical CSR edges baked into the generated Program shape.
    pub edge_count: u32,
    /// Motif edges lowered as Program constants.
    pub motif_edges: Vec<MotifEdge>,
    /// Witness output buffer name baked into the Program.
    pub witness_out: String,
}

/// Primitive-owned identity for reusable motif static graph inputs.
///
/// Motif edges are compiled into the generated Program and are therefore part
/// of [`MotifProgramCacheKey`]. This key only tracks staged CSR graph inputs so
/// dispatch wrappers can reuse static graph buffers across motif-program
/// changes without forking fingerprint rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotifStaticInputKey {
    /// Number of graph nodes and witness words.
    pub node_count: u32,
    /// Number of u32 witness/scratch words staged by the wrapper.
    pub output_words: usize,
    /// Number of u32 words staged for edge targets and kind masks.
    pub edge_storage_words: usize,
    /// Stable fingerprint of the CSR offsets upload.
    pub edge_offsets_hash: u64,
    /// Stable fingerprint of the padded target upload.
    pub edge_targets_hash: u64,
    /// Stable fingerprint of the padded kind-mask upload.
    pub edge_kind_mask_hash: u64,
}

/// Validated motif launch plan without eager Program materialization.
pub struct MotifLaunchPlan {
    layout: MotifLayout,
    cache_key: MotifProgramCacheKey,
}

impl MotifLaunchPlan {
    /// Validated motif graph/pattern layout.
    #[must_use]
    pub const fn layout(&self) -> MotifLayout {
        self.layout
    }

    /// Stable cache identity for the generated Program.
    #[must_use]
    pub fn cache_key(&self) -> &MotifProgramCacheKey {
        &self.cache_key
    }

    /// Number of u32 words in motif scratch and witness outputs.
    #[must_use]
    pub const fn output_words(&self) -> usize {
        self.layout.output_words
    }

    /// Number of u32 words required by physical edge buffers after padding.
    #[must_use]
    pub const fn edge_storage_words(&self) -> usize {
        self.layout.edge_storage_words
    }

    /// Canonical one-workgroup dispatch grid.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        MOTIF_DISPATCH_GRID
    }

    /// Materialize the canonical primitive Program for this launch plan.
    #[must_use]
    pub fn program(&self) -> Program {
        motif(
            ProgramGraphShape::new(self.layout.node_count, self.layout.edge_count.max(1)),
            &self.cache_key.motif_edges,
            &self.cache_key.witness_out,
        )
    }

    /// Return the primitive-owned cache identity for static CSR graph inputs.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the supplied CSR slices no longer
    /// match the validated launch-plan shape.
    pub fn static_input_key(
        &self,
        edge_offsets: &[u32],
        edge_targets: &[u32],
        edge_kind_mask: &[u32],
    ) -> Result<MotifStaticInputKey, String> {
        if edge_offsets.len() != self.layout.node_count as usize + 1 {
            return Err(format!(
                "Fix: motif static key expected {} offset words, got {}.",
                self.layout.node_count as usize + 1,
                edge_offsets.len()
            ));
        }
        if edge_targets.len() != self.layout.edge_count as usize {
            return Err(format!(
                "Fix: motif static key expected {} target word(s), got {}.",
                self.layout.edge_count,
                edge_targets.len()
            ));
        }
        if edge_kind_mask.len() != self.layout.edge_count as usize {
            return Err(format!(
                "Fix: motif static key expected {} kind-mask word(s), got {}.",
                self.layout.edge_count,
                edge_kind_mask.len()
            ));
        }
        Ok(MotifStaticInputKey {
            node_count: self.layout.node_count,
            output_words: self.layout.output_words,
            edge_storage_words: self.layout.edge_storage_words,
            edge_offsets_hash: motif_padded_slice_fingerprint(edge_offsets, edge_offsets.len()),
            edge_targets_hash: motif_padded_slice_fingerprint(
                edge_targets,
                self.layout.edge_storage_words,
            ),
            edge_kind_mask_hash: motif_padded_slice_fingerprint(
                edge_kind_mask,
                self.layout.edge_storage_words,
            ),
        })
    }
}

fn motif_padded_slice_fingerprint(values: &[u32], padded_words: usize) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for byte in (padded_words as u64).to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    for index in 0..padded_words {
        let value = values.get(index).copied().unwrap_or(0);
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Primitive-owned motif dispatch plan.
pub struct MotifDispatchPlan {
    layout: MotifLayout,
    program: Program,
}

impl MotifDispatchPlan {
    /// Validated motif graph/pattern layout.
    #[must_use]
    pub const fn layout(&self) -> MotifLayout {
        self.layout
    }

    /// Canonical primitive program for this motif.
    #[must_use]
    pub const fn program(&self) -> &Program {
        &self.program
    }

    /// Number of u32 words in motif scratch and witness outputs.
    #[must_use]
    pub const fn output_words(&self) -> usize {
        self.layout.output_words
    }

    /// Number of u32 words required by physical edge buffers after padding.
    #[must_use]
    pub const fn edge_storage_words(&self) -> usize {
        self.layout.edge_storage_words
    }

    /// Canonical one-workgroup dispatch grid.
    #[must_use]
    pub const fn dispatch_grid(&self) -> [u32; 3] {
        MOTIF_DISPATCH_GRID
    }
}

/// Validate motif inputs and build the canonical dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic for malformed CSR inputs, mismatched edge
/// masks, out-of-range destinations, or motif patterns too large for GPU
/// metadata.
pub fn plan_motif_launch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
    witness_out: &str,
) -> Result<MotifLaunchPlan, String> {
    let layout = validate_motif_inputs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )?;

    Ok(MotifLaunchPlan {
        layout,
        cache_key: MotifProgramCacheKey {
            node_count: layout.node_count,
            edge_count: layout.edge_count,
            motif_edges: motif_edges.to_vec(),
            witness_out: witness_out.to_string(),
        },
    })
}

/// Validate motif inputs and build the canonical dispatch plan.
///
/// # Errors
///
/// Returns an actionable diagnostic for malformed CSR inputs, mismatched edge
/// masks, out-of-range destinations, or motif patterns too large for GPU
/// metadata.
pub fn plan_motif_dispatch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
    witness_out: &str,
) -> Result<MotifDispatchPlan, String> {
    let launch = plan_motif_launch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
        witness_out,
    )?;
    let layout = launch.layout();
    let program = launch.program();
    Ok(MotifDispatchPlan { layout, program })
}

/// Build a Program: one invocation checks every motif edge, records
/// participating endpoint bits only for matched edges, and publishes
/// the participant union if the whole motif matched.
///
/// Invalid motif sizes lower to an explicit trap program. Prior code
/// silently truncated `edges.len() as u32`; this path keeps the failure
/// executable without crashing the host process.
#[must_use]
pub fn motif(shape: ProgramGraphShape, edges: &[MotifEdge], witness_out: &str) -> Program {
    let Ok(edge_count) = u32::try_from(edges.len()) else {
        return crate::invalid_output_program(
            OP_ID,
            witness_out,
            DataType::U32,
            "Fix: motif edges.len() exceeds u32::MAX; split the motif or redesign the caller."
                .to_string(),
        );
    };
    let mut buffers = shape.read_only_buffers();
    buffers.push(
        BufferDecl::storage(
            "motif_hits",
            MOTIF_HITS_BUFFER,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(shape.node_count.max(1)),
    );
    buffers.push(
        BufferDecl::storage(
            witness_out,
            MOTIF_WITNESS_OUT_BUFFER,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(shape.node_count.max(1)),
    );

    let clear_outputs = vec![
        Node::store("motif_hits", Expr::var("node"), Expr::u32(0)),
        Node::store(witness_out, Expr::var("node"), Expr::u32(0)),
    ];
    // Motif edges are compile-time operands of this generated program, not
    // runtime graph data. Lowering them as constants removes three input
    // buffers and prevents loop-carried scratch state from making a partial
    // motif look like a complete match.
    let Some(scan_capacity) = edges.len().checked_mul(5) else {
        return crate::invalid_output_program(
            OP_ID,
            witness_out,
            DataType::U32,
            "Fix: motif scan node count overflows usize; split the motif before lowering."
                .to_string(),
        );
    };
    let Some(mark_capacity) = edges.len().checked_mul(2) else {
        return crate::invalid_output_program(
            OP_ID,
            witness_out,
            DataType::U32,
            "Fix: motif witness mark count overflows usize; split the motif before lowering."
                .to_string(),
        );
    };
    let mut scan_edges: Vec<Node> = Vec::new();
    if let Err(error) = scan_edges.try_reserve(scan_capacity) {
        return crate::invalid_output_program(
            OP_ID,
            witness_out,
            DataType::U32,
            format!("Fix: motif lowering could not reserve {scan_capacity} scan nodes: {error}"),
        );
    }
    let mut mark_hits: Vec<Node> = Vec::new();
    if let Err(error) = mark_hits.try_reserve(mark_capacity) {
        return crate::invalid_output_program(
            OP_ID,
            witness_out,
            DataType::U32,
            format!("Fix: motif lowering could not reserve {mark_capacity} mark nodes: {error}"),
        );
    }
    for (idx, edge) in edges.iter().enumerate() {
        let edge_found = format!("edge_found_{idx}");
        let edge_start = format!("edge_start_{idx}");
        let edge_end = format!("edge_end_{idx}");
        let edge_index = format!("e_{idx}");
        let actual_dst = format!("actual_dst_{idx}");
        let actual_kind = format!("actual_kind_{idx}");
        scan_edges.push(Node::let_bind(&edge_found, Expr::u32(0)));
        if edge.from < shape.node_count {
            scan_edges.push(Node::let_bind(
                &edge_start,
                Expr::load(NAME_EDGE_OFFSETS, Expr::u32(edge.from)),
            ));
            scan_edges.push(Node::let_bind(
                &edge_end,
                Expr::load(NAME_EDGE_OFFSETS, Expr::u32(edge.from.saturating_add(1))),
            ));
            scan_edges.push(Node::loop_for(
                &edge_index,
                Expr::var(&edge_start),
                Expr::var(&edge_end),
                vec![
                    Node::let_bind(
                        &actual_dst,
                        Expr::load(NAME_EDGE_TARGETS, Expr::var(&edge_index)),
                    ),
                    Node::let_bind(
                        &actual_kind,
                        Expr::load(NAME_EDGE_KIND_MASK, Expr::var(&edge_index)),
                    ),
                    Node::if_then(
                        Expr::and(
                            Expr::eq(Expr::var(&actual_dst), Expr::u32(edge.to)),
                            Expr::ne(
                                Expr::bitand(Expr::var(&actual_kind), Expr::u32(edge.kind_mask)),
                                Expr::u32(0),
                            ),
                        ),
                        vec![Node::assign(&edge_found, Expr::u32(1))],
                    ),
                ],
            ));
        }
        scan_edges.push(Node::if_then(
            Expr::ne(Expr::var(&edge_found), Expr::u32(0)),
            vec![Node::assign(
                "matched_edges",
                Expr::add(Expr::var("matched_edges"), Expr::u32(1)),
            )],
        ));
        if edge.from < shape.node_count {
            mark_hits.push(Node::store(
                "motif_hits",
                Expr::u32(edge.from),
                Expr::u32(1),
            ));
        }
        if edge.to < shape.node_count {
            mark_hits.push(Node::store("motif_hits", Expr::u32(edge.to), Expr::u32(1)));
        }
    }
    let materialize = vec![Node::store(
        witness_out,
        Expr::var("node"),
        Expr::load("motif_hits", Expr::var("node")),
    )];
    let mut publish_full_match = mark_hits;
    publish_full_match.push(Node::loop_for(
        "node",
        Expr::u32(0),
        Expr::u32(shape.node_count),
        materialize,
    ));

    // PHASE7_GRAPH C2: motif is fundamentally serial  -  one thread loops
    // over every motif edge in order and accumulates `matched_edges`.
    // Using a [256,1,1] workgroup with a `gid_x() == 0` gate burns 255
    // idle lanes per workgroup. Dispatch a single 1-lane workgroup
    // instead so the wasted parallelism is gone, and drop the redundant
    // gate.
    Program::wrapped(
        buffers,
        MOTIF_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![
                Node::loop_for(
                    "node",
                    Expr::u32(0),
                    Expr::u32(shape.node_count),
                    clear_outputs,
                ),
                Node::let_bind("matched_edges", Expr::u32(0)),
                Node::Block(scan_edges),
                Node::if_then(
                    Expr::eq(Expr::var("matched_edges"), Expr::u32(edge_count)),
                    publish_full_match,
                ),
            ]),
        }],
    )
}

/// CPU reference: return one byte-per-node witness set where `1`
/// means the node participates in a complete motif match.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]

pub fn cpu_ref(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Vec<u32> {
    let mut participants = Vec::new();
    try_cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
        &mut participants,
    )
    .unwrap_or_else(|err| panic!("motif CPU oracle received malformed input. {err}"));
    participants
}

/// Fallible CPU reference into caller-owned witness storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
    participants: &mut Vec<u32>,
) -> Result<(), String> {
    let layout = validate_motif_inputs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )?;
    crate::graph::scratch::reserve_graph_items(
        participants,
        layout.output_words,
        "motif CPU oracle",
        "motif witness output",
    )?;
    participants.clear();
    participants.resize(layout.output_words, 0);
    if !motif_all_edges_present(edge_offsets, edge_targets, edge_kind_mask, motif_edges) {
        return Ok(());
    }
    for motif_edge in motif_edges {
        if let Some(hit) = participants.get_mut(motif_edge.from as usize) {
            *hit = 1;
        }
        if let Some(hit) = participants.get_mut(motif_edge.to as usize) {
            *hit = 1;
        }
    }
    Ok(())
}

/// CPU reference into caller-owned witness storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
    participants: &mut Vec<u32>,
) {
    try_cpu_ref_into(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
        participants,
    )
    .unwrap_or_else(|err| panic!("motif CPU oracle received malformed input. {err}"));
}

/// Return true iff the complete motif exists.
///
/// This avoids allocating a full witness vector for existence checks.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_matches(
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> bool {
    motif_all_edges_present(edge_offsets, edge_targets, edge_kind_mask, motif_edges)
}

/// Count distinct nodes participating in a complete motif match.
///
/// This avoids materializing the witness vector when callers only need a
/// scheduling signal.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_participation_count(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> u32 {
    try_cpu_ref_participation_count(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )
    .unwrap_or_else(|err| panic!("motif participation oracle received malformed input. {err}"))
}

/// Caller-owned workspace for motif CPU reference helpers.
#[cfg(any(test, feature = "cpu-parity"))]
#[derive(Debug, Default, Clone)]
pub struct MotifCpuScratch {
    /// Distinct endpoint scratch used by participation-count queries.
    pub endpoints: Vec<u32>,
}

#[cfg(any(test, feature = "cpu-parity"))]
impl MotifCpuScratch {
    /// Create an empty reusable motif workspace.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fallible count of distinct nodes participating in a complete motif match.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_participation_count(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Result<u32, String> {
    let mut scratch = MotifCpuScratch::default();
    try_cpu_ref_participation_count_with_scratch(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
        &mut scratch,
    )
}

/// Fallible participation count using caller-owned endpoint scratch.
///
/// Validation happens before the scratch vector is touched. For valid inputs,
/// the scratch vector is cleared and reused even when the complete motif is not
/// present, so stale endpoints cannot leak into later proof cases.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_participation_count_with_scratch(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
    scratch: &mut MotifCpuScratch,
) -> Result<u32, String> {
    validate_motif_inputs(
        node_count,
        edge_offsets,
        edge_targets,
        edge_kind_mask,
        motif_edges,
    )?;
    let endpoint_count = motif_edges
        .len()
        .checked_mul(2)
        .ok_or_else(|| "Fix: motif endpoint count overflows usize.".to_string())?;
    scratch
        .endpoints
        .try_reserve(endpoint_count)
        .map_err(|error| {
            format!(
            "Fix: motif participation oracle could not reserve {endpoint_count} endpoints: {error}"
        )
        })?;
    scratch.endpoints.clear();
    if !motif_all_edges_present(edge_offsets, edge_targets, edge_kind_mask, motif_edges) {
        return Ok(0);
    }
    for motif_edge in motif_edges {
        if motif_edge.from < node_count {
            scratch.endpoints.push(motif_edge.from);
        }
        if motif_edge.to < node_count {
            scratch.endpoints.push(motif_edge.to);
        }
    }
    scratch.endpoints.sort_unstable();
    scratch.endpoints.dedup();
    u32::try_from(scratch.endpoints.len()).map_err(|error| {
        format!("Fix: motif participation count does not fit u32 after deduplication: {error}")
    })
}

/// Validate the public CSR inputs consumed by the motif primitive.
///
/// Returns the exact edge count declared by `edge_offsets[node_count]`, so
/// dispatch wrappers can pad zero-edge buffers without duplicating CSR
/// validation logic.
///
/// # Errors
///
/// Returns an actionable diagnostic for malformed row offsets, edge arrays, or
/// out-of-range destinations.
pub fn validate_csr_inputs(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
) -> Result<MotifLayout, String> {
    validate_motif_inputs(node_count, edge_offsets, edge_targets, edge_kind_mask, &[])
}

/// Validate the public CSR and motif inputs consumed by the motif primitive.
///
/// # Errors
///
/// Returns an actionable diagnostic for malformed row offsets, edge arrays,
/// out-of-range destinations, or motif edge counts that exceed u32 dispatch
/// metadata.
pub fn validate_motif_inputs(
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> Result<MotifLayout, String> {
    let expected_offsets = (node_count as usize).checked_add(1).ok_or_else(|| {
        format!("Fix: motif node_count + 1 overflows usize for node_count={node_count}.")
    })?;
    if edge_offsets.len() != expected_offsets {
        return Err(format!(
            "Fix: motif requires edge_offsets.len() == node_count + 1, got len={}, node_count={node_count}.",
            edge_offsets.len()
        ));
    }
    if edge_targets.len() != edge_kind_mask.len() {
        return Err(format!(
            "Fix: motif requires edge_targets.len() == edge_kind_mask.len(), got {} vs {}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    if let Some(&first) = edge_offsets.first() {
        if first != 0 {
            return Err(format!(
                "Fix: motif requires edge_offsets[0] == 0, got {first}."
            ));
        }
    }
    for (index, pair) in edge_offsets.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(format!(
                "Fix: motif offsets must be monotonic; offsets[{index}]={} > offsets[{}]={}.",
                pair[0],
                index + 1,
                pair[1]
            ));
        }
    }
    let edge_count = edge_offsets[expected_offsets - 1] as usize;
    if edge_targets.len() != edge_count {
        return Err(format!(
            "Fix: motif final offset declares edge_count={edge_count}, but targets_len={} and kind_mask_len={}.",
            edge_targets.len(),
            edge_kind_mask.len()
        ));
    }
    for (index, &target) in edge_targets.iter().enumerate() {
        if target >= node_count {
            return Err(format!(
                "Fix: motif edge_targets[{index}]={target} is outside node_count {node_count}."
            ));
        }
    }
    for (index, motif_edge) in motif_edges.iter().enumerate() {
        if motif_edge.from >= node_count {
            return Err(format!(
                "Fix: motif_edges[{index}].from={} is outside node_count {node_count}.",
                motif_edge.from
            ));
        }
        if motif_edge.to >= node_count {
            return Err(format!(
                "Fix: motif_edges[{index}].to={} is outside node_count {node_count}.",
                motif_edge.to
            ));
        }
    }
    let edge_count = u32::try_from(edge_count)
        .map_err(|_| format!("Fix: motif edge count {edge_count} exceeds u32 index space."))?;
    let motif_edge_count = u32::try_from(motif_edges.len()).map_err(|_| {
        format!(
            "Fix: motif edge pattern count {} exceeds u32 index space.",
            motif_edges.len()
        )
    })?;
    Ok(MotifLayout {
        node_count,
        output_words: node_count as usize,
        edge_count,
        edge_storage_words: edge_targets.len().max(1),
        motif_edge_count,
    })
}

/// Count nonzero witness entries using the primitive's u32 result contract.
///
/// # Errors
///
/// Returns an actionable diagnostic if the witness vector is too large to
/// report with the primitive's u32 count metadata.
pub fn count_witness_participants(witness: &[u32]) -> Result<u32, String> {
    let count = witness.iter().filter(|&&value| value != 0).count();
    u32::try_from(count)
        .map_err(|_| format!("Fix: motif witness participant count {count} exceeds u32::MAX."))
}

/// Validate the primitive's u32 witness output contract.
///
/// # Errors
///
/// Returns an actionable diagnostic if the backend returns the wrong number of
/// witness words or any non-boolean witness entry.
pub fn validate_motif_witness(layout: MotifLayout, witness: &[u32]) -> Result<(), String> {
    if witness.len() != layout.output_words {
        return Err(format!(
            "Fix: motif witness expected {} word(s), got {}.",
            layout.output_words,
            witness.len()
        ));
    }
    for (index, &value) in witness.iter().enumerate() {
        if value > 1 {
            return Err(format!(
                "Fix: motif witness[{index}]={value} is not boolean; expected 0 or 1."
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod dispatch_contract_tests {
    use super::*;

    #[test]
    fn static_input_key_tracks_graph_content_not_motif_program() {
        let first_motif = [MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        }];
        let second_motif = [MotifEdge {
            from: 1,
            kind_mask: 1,
            to: 2,
        }];
        let first = plan_motif_launch(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &first_motif, "w")
            .expect("Fix: first motif launch should plan");
        let second = plan_motif_launch(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &second_motif, "w")
            .expect("Fix: second motif launch should plan");

        assert_ne!(first.cache_key(), second.cache_key());
        assert_eq!(
            first
                .static_input_key(&[0, 1, 2, 2], &[1, 2], &[1, 1])
                .expect("Fix: first motif static key should build"),
            second
                .static_input_key(&[0, 1, 2, 2], &[1, 2], &[1, 1])
                .expect("Fix: second motif static key should build")
        );
    }

    #[test]
    fn static_input_key_refreshes_on_same_shape_graph_content_change() {
        let motif = [MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        }];
        let plan = plan_motif_launch(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &motif, "w")
            .expect("Fix: motif launch should plan");
        let first = plan
            .static_input_key(&[0, 1, 2, 2], &[1, 2], &[1, 1])
            .expect("Fix: first static key should build");
        let changed = plan
            .static_input_key(&[0, 1, 2, 2], &[2, 2], &[1, 1])
            .expect("Fix: same-shape changed graph should key");

        assert_eq!(first.edge_offsets_hash, changed.edge_offsets_hash);
        assert_eq!(first.edge_kind_mask_hash, changed.edge_kind_mask_hash);
        assert_ne!(first.edge_targets_hash, changed.edge_targets_hash);
        assert_ne!(first, changed);
    }

    #[test]
    fn static_input_key_rejects_shape_drift() {
        let motif = [MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        }];
        let plan = plan_motif_launch(2, &[0, 1, 1], &[1], &[1], &motif, "w")
            .expect("Fix: motif launch should plan");
        let err = plan
            .static_input_key(&[0, 1, 1], &[], &[])
            .expect_err("Fix: stale motif plan must reject edge-array drift");

        assert!(err.contains("expected 1 target"));
    }

    #[test]
    fn witness_validation_rejects_non_boolean_backend_output() {
        let layout = validate_motif_inputs(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &[])
            .expect("Fix: valid graph should validate");

        validate_motif_witness(layout, &[0, 1, 0]).expect("Fix: boolean witness is valid");
        let err = validate_motif_witness(layout, &[0, 2, 0])
            .expect_err("Fix: non-boolean witness must be rejected");

        assert!(err.contains("witness[1]=2 is not boolean"));
    }
}

#[cfg(any(test, feature = "cpu-parity"))]
fn motif_all_edges_present(
    edge_offsets: &[u32],
    edge_targets: &[u32],
    edge_kind_mask: &[u32],
    motif_edges: &[MotifEdge],
) -> bool {
    for motif_edge in motif_edges {
        let Some(start) = edge_offsets.get(motif_edge.from as usize).copied() else {
            return false;
        };
        let Some(end) = edge_offsets.get(motif_edge.from as usize + 1).copied() else {
            return false;
        };
        let start = start as usize;
        let end = end as usize;
        let mut found = false;
        for edge_idx in start..end {
            let Some(dst) = edge_targets.get(edge_idx).copied() else {
                break;
            };
            let Some(kind) = edge_kind_mask.get(edge_idx).copied() else {
                break;
            };
            if dst == motif_edge.to && (kind & motif_edge.kind_mask) != 0 {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }
    true
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || motif(ProgramGraphShape::new(4, 4), &[MotifEdge { from: 0, to: 1, kind_mask: 1 }], "witness"),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0, 0]),          // pg_nodes
                to_bytes(&[0, 2, 3, 4, 4]),       // pg_edge_offsets
                to_bytes(&[1, 2, 3, 3]),          // pg_edge_targets
                to_bytes(&[1, 1, 1, 1]),          // pg_edge_kind_mask
                to_bytes(&[0, 0, 0, 0]),          // pg_node_tags
                to_bytes(&[0, 0, 0, 0]),          // motif_hits
                to_bytes(&[0, 0, 0, 0]),          // witness
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1, 1, 0, 0]),          // expected motif_hits
                to_bytes(&[1, 1, 0, 0]),          // expected witness
            ]]
        }),
    )
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn try_cpu_ref_into_rejects_bad_motif_endpoint_without_clobbering_witness() {
        let mut witness = vec![9, 8, 7];
        let motif = [MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 3,
        }];

        let err = try_cpu_ref_into(3, &[0, 1, 1, 1], &[1], &[1], &motif, &mut witness)
            .expect_err("motif endpoint beyond node_count must fail validation");

        assert!(
            err.contains("motif_edges[0].to=3 is outside node_count 3"),
            "Fix: motif endpoint errors must identify the bad endpoint, got: {err}"
        );
        assert_eq!(
            witness,
            vec![9, 8, 7],
            "failed motif preflight must preserve the previous witness vector"
        );
    }

    #[test]
    fn try_participation_count_rejects_bad_motif_endpoint() {
        let motif = [MotifEdge {
            from: 4,
            kind_mask: 1,
            to: 0,
        }];

        let err = try_cpu_ref_participation_count(3, &[0, 0, 0, 0], &[], &[], &motif)
            .expect_err("motif participation count must validate pattern endpoints");

        assert!(
            err.contains("motif_edges[0].from=4 is outside node_count 3"),
            "Fix: motif participation count must surface endpoint shape errors, got: {err}"
        );
    }

    #[test]
    fn try_participation_count_with_scratch_reuses_endpoint_storage() {
        let mut endpoints = Vec::with_capacity(8);
        endpoints.extend_from_slice(&[99, 98, 97]);
        let mut scratch = MotifCpuScratch { endpoints };
        let capacity = scratch.endpoints.capacity();
        let motif = [
            MotifEdge {
                from: 0,
                kind_mask: 1,
                to: 1,
            },
            MotifEdge {
                from: 1,
                kind_mask: 1,
                to: 2,
            },
        ];

        let count = try_cpu_ref_participation_count_with_scratch(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &motif,
            &mut scratch,
        )
        .expect("Fix: valid motif count must run with reusable endpoint scratch.");

        assert_eq!(count, 3);
        assert_eq!(scratch.endpoints.capacity(), capacity);
        assert_eq!(
            scratch.endpoints,
            vec![0, 1, 2],
            "Fix: endpoint scratch must be sorted and deduplicated for the live motif."
        );

        let count = try_cpu_ref_participation_count_with_scratch(
            3,
            &[0, 1, 1, 1],
            &[1],
            &[1],
            &motif,
            &mut scratch,
        )
        .expect("Fix: valid graph with missing motif must return zero without stale endpoints.");

        assert_eq!(count, 0);
        assert_eq!(scratch.endpoints.capacity(), capacity);
        assert!(
            scratch.endpoints.is_empty(),
            "Fix: missing motif must clear stale endpoint scratch."
        );
    }

    #[test]
    fn try_participation_count_with_scratch_validates_before_mutating_storage() {
        let mut scratch = MotifCpuScratch {
            endpoints: vec![0xCAFE_BABE, 0xDEAD_BEEF],
        };
        let motif = [MotifEdge {
            from: 4,
            kind_mask: 1,
            to: 0,
        }];

        let err = try_cpu_ref_participation_count_with_scratch(
            3,
            &[0, 0, 0, 0],
            &[],
            &[],
            &motif,
            &mut scratch,
        )
        .expect_err("Fix: motif endpoint validation must run before scratch reuse.");

        assert!(
            err.contains("motif_edges[0].from=4 is outside node_count 3"),
            "Fix: motif participation count must surface endpoint shape errors, got: {err}"
        );
        assert_eq!(
            scratch.endpoints,
            vec![0xCAFE_BABE, 0xDEAD_BEEF],
            "Fix: validation failure must not clear reusable endpoint scratch."
        );
    }

    #[test]
    fn generated_participation_count_matches_witness_count() {
        for node_count in 2u32..=7 {
            let mut offsets = Vec::with_capacity(node_count as usize + 1);
            let mut targets = Vec::new();
            let mut masks = Vec::new();
            offsets.push(0);
            for node in 0..node_count {
                targets.push((node + 1) % node_count);
                masks.push(1);
                offsets.push(targets.len() as u32);
            }
            let motif = [MotifEdge {
                from: 0,
                kind_mask: 1,
                to: 1,
            }];
            let witness = cpu_ref(node_count, &offsets, &targets, &masks, &motif);
            let witness_count =
                count_witness_participants(&witness).expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated witness count must fit u32");
            let count =
                try_cpu_ref_participation_count(node_count, &offsets, &targets, &masks, &motif)
                    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated motif participation count must pass validation");

            assert_eq!(
                count, witness_count,
                "participation count diverged from witness count at node_count={node_count}"
            );
        }
    }

    #[test]
    fn three_node_chain_motif_marks_every_participant() {
        let witness = cpu_ref(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &[
                MotifEdge {
                    from: 0,
                    kind_mask: 1,
                    to: 1,
                },
                MotifEdge {
                    from: 1,
                    kind_mask: 1,
                    to: 2,
                },
            ],
        );
        assert_eq!(witness, vec![1, 1, 1]);
    }

    #[test]
    fn missing_motif_edge_clears_all_participants() {
        let witness = cpu_ref(
            3,
            &[0, 1, 1, 1],
            &[1],
            &[1],
            &[
                MotifEdge {
                    from: 0,
                    kind_mask: 1,
                    to: 1,
                },
                MotifEdge {
                    from: 1,
                    kind_mask: 1,
                    to: 2,
                },
            ],
        );
        assert_eq!(witness, vec![0, 0, 0]);
    }

    #[test]
    fn cpu_ref_into_reuses_witness_storage() {
        let mut witness = Vec::with_capacity(8);
        cpu_ref_into(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &[
                MotifEdge {
                    from: 0,
                    kind_mask: 1,
                    to: 1,
                },
                MotifEdge {
                    from: 1,
                    kind_mask: 1,
                    to: 2,
                },
            ],
            &mut witness,
        );
        let capacity = witness.capacity();
        assert_eq!(witness, vec![1, 1, 1]);

        cpu_ref_into(
            3,
            &[0, 1, 1, 1],
            &[1],
            &[1],
            &[MotifEdge {
                from: 1,
                kind_mask: 1,
                to: 2,
            }],
            &mut witness,
        );
        assert_eq!(witness.capacity(), capacity);
        assert_eq!(witness, vec![0, 0, 0]);
    }

    #[test]
    fn cpu_ref_into_validates_before_clearing_witness_storage() {
        let mut witness = vec![0xCAFE_BABEu32, 0xDEAD_BEEF];
        let ptr = witness.as_ptr();
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cpu_ref_into(
                2,
                &[0, 1, 1],
                &[1],
                &[],
                &[MotifEdge {
                    from: 0,
                    kind_mask: 1,
                    to: 1,
                }],
                &mut witness,
            );
        }));
        std::panic::set_hook(previous_hook);

        assert!(err.is_err(), "mismatched CSR edge arrays must be rejected");
        assert_eq!(
            witness,
            vec![0xCAFE_BABEu32, 0xDEAD_BEEF],
            "Fix: motif CPU oracle must validate before clearing caller witness storage."
        );
        assert_eq!(witness.as_ptr(), ptr);
    }

    #[test]
    fn generated_try_cpu_ref_into_and_count_match_witness() {
        for node_count in 1u32..=64 {
            let mut scratch = MotifCpuScratch::new();
            let edge_offsets: Vec<u32> = (0..=node_count).collect();
            let edge_targets: Vec<u32> = (0..node_count)
                .map(|node| (node + 1) % node_count)
                .collect();
            let edge_kind_mask = vec![1u32; node_count as usize];
            for motif_len in 0usize..64 {
                let motif_edges: Vec<MotifEdge> = (0..motif_len)
                    .map(|index| {
                        let from = (index as u32) % node_count;
                        MotifEdge {
                            from,
                            kind_mask: 1,
                            to: (from + 1) % node_count,
                        }
                    })
                    .collect();
                let mut witness = vec![0xCAFE_BABEu32; 3];
                try_cpu_ref_into(
                    node_count,
                    &edge_offsets,
                    &edge_targets,
                    &edge_kind_mask,
                    &motif_edges,
                    &mut witness,
                )
                .unwrap();
                let count = try_cpu_ref_participation_count_with_scratch(
                    node_count,
                    &edge_offsets,
                    &edge_targets,
                    &edge_kind_mask,
                    &motif_edges,
                    &mut scratch,
                )
                .unwrap();
                assert_eq!(witness.len(), node_count as usize);
                assert_eq!(
                    count,
                    witness.iter().filter(|&&value| value != 0).count() as u32
                );
            }
        }
    }

    #[test]
    fn allocation_free_predicates_match_witness_contract() {
        let motif = [
            MotifEdge {
                from: 0,
                kind_mask: 1,
                to: 1,
            },
            MotifEdge {
                from: 1,
                kind_mask: 1,
                to: 2,
            },
        ];
        assert!(cpu_ref_matches(&[0, 1, 2, 2], &[1, 2], &[1, 1], &motif));
        assert_eq!(
            cpu_ref_participation_count(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &motif),
            3
        );
        assert!(!cpu_ref_matches(&[0, 1, 1, 1], &[1], &[1], &motif));
        assert_eq!(
            cpu_ref_participation_count(3, &[0, 1, 1, 1], &[1], &[1], &motif),
            0
        );
        assert!(
            cpu_ref_matches(&[0, 1, 2, 2], &[1, 2], &[1, 1], &[]),
            "empty motif has no missing edges"
        );
        assert_eq!(
            cpu_ref_participation_count(3, &[0, 1, 2, 2], &[1, 2], &[1, 1], &[]),
            0,
            "empty motif has no participating nodes"
        );
    }

    #[test]
    fn validate_csr_inputs_accepts_empty_and_canonical_graphs() {
        assert_eq!(
            validate_motif_inputs(0, &[0], &[], &[], &[]).unwrap(),
            MotifLayout {
                node_count: 0,
                output_words: 0,
                edge_count: 0,
                edge_storage_words: 1,
                motif_edge_count: 0,
            }
        );
        assert_eq!(
            validate_motif_inputs(
                3,
                &[0, 1, 2, 2],
                &[1, 2],
                &[1, 1],
                &[MotifEdge {
                    from: 0,
                    kind_mask: 1,
                    to: 1,
                }],
            )
            .unwrap(),
            MotifLayout {
                node_count: 3,
                output_words: 3,
                edge_count: 2,
                edge_storage_words: 2,
                motif_edge_count: 1,
            }
        );
    }

    #[test]
    fn dispatch_plan_owns_shape_grid_buffers_and_readback_words() {
        let motif_edges = [MotifEdge {
            from: 0,
            kind_mask: 1,
            to: 1,
        }];
        let launch = plan_motif_launch(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &motif_edges,
            "witness_out",
        )
        .expect("Fix: canonical motif launch plan should validate without materializing a Program");
        assert_eq!(launch.layout().node_count, 3);
        assert_eq!(launch.output_words(), 3);
        assert_eq!(launch.edge_storage_words(), 2);
        assert_eq!(launch.dispatch_grid(), MOTIF_DISPATCH_GRID);
        assert_eq!(
            launch.cache_key(),
            &MotifProgramCacheKey {
                node_count: 3,
                edge_count: 2,
                motif_edges: motif_edges.to_vec(),
                witness_out: "witness_out".to_string(),
            }
        );

        let plan = plan_motif_dispatch(
            3,
            &[0, 1, 2, 2],
            &[1, 2],
            &[1, 1],
            &motif_edges,
            "witness_out",
        )
        .expect("Fix: canonical motif dispatch plan should validate");

        assert_eq!(plan.layout().node_count, 3);
        assert_eq!(plan.layout().edge_count, 2);
        assert_eq!(plan.layout().motif_edge_count, 1);
        assert_eq!(plan.output_words(), 3);
        assert_eq!(plan.edge_storage_words(), 2);
        assert_eq!(plan.dispatch_grid(), MOTIF_DISPATCH_GRID);
        assert_eq!(plan.program().workgroup_size, MOTIF_WORKGROUP_SIZE);
        let bindings = plan
            .program()
            .buffers
            .iter()
            .map(|buffer| buffer.binding)
            .collect::<Vec<_>>();
        assert!(bindings.contains(&MOTIF_HITS_BUFFER));
        assert!(bindings.contains(&MOTIF_WITNESS_OUT_BUFFER));

        let empty_edge_plan = plan_motif_dispatch(1, &[0, 0], &[], &[], &[], "witness_out")
            .expect("Fix: zero-edge motif graph should still have padded edge storage");
        assert_eq!(empty_edge_plan.layout().edge_count, 0);
        assert_eq!(empty_edge_plan.edge_storage_words(), 1);
    }

    #[test]
    fn witness_participant_count_uses_primitive_contract() {
        assert_eq!(count_witness_participants(&[1, 0, 2, 0]).unwrap(), 2);
    }

    #[test]
    fn validate_csr_inputs_rejects_malformed_csr() {
        let err = validate_csr_inputs(2, &[0, 1, 1], &[1], &[]).unwrap_err();
        assert!(err.contains("edge_targets.len() == edge_kind_mask.len()"));

        let err = validate_csr_inputs(2, &[0, 2, 1], &[1], &[1]).unwrap_err();
        assert!(err.contains("offsets must be monotonic"));

        let err = validate_csr_inputs(2, &[0, 1, 1], &[5], &[1]).unwrap_err();
        assert!(err.contains("outside node_count"));
    }
}
