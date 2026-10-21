//! Grid-sync kernel splitting.
//!
//! Op id: `vyre-driver::grid_sync`. Soundness: `Exact` over the
//! cross-grid barrier contract.
//!
//! ## Why this lives in vyre-driver, not the backend
//!
//! Every backend that lacks a native cooperative whole-grid launch
//! needs the same kernel-split semantics for
//! `Node::Barrier { ordering: GridSync }`: split the program at the
//! barrier, dispatch each segment as its own kernel launch, and
//! re-feed the prior segment's outputs as inputs to the next. The
//! kernel-launch boundary itself is the grid-level fence  -  every
//! prior write becomes globally visible before the next launch reads.
//!
//! Backends route through [`crate::grid_sync::dispatch_with_grid_sync_split`] when
//! [`VyreBackend::supports_grid_sync`] is `false` and the program
//! contains any `Node::Barrier { ordering: GridSync }`. Backends that
//! return `true` emit one kernel and satisfy the barrier device-side.
//!
//! ## Algorithm
//!
//! 1. Walk the program's top-level entry sequence.
//! 2. Each prefix-suffix split at a `Node::Barrier { GridSync }`
//!    becomes one segment.
//! 3. For each segment, build a `Program` with a segment-local buffer
//!    table: buffers read or written by that segment plus passthrough
//!    read-write buffers that must preserve caller-visible storage.
//! 4. Dispatch segments in order, threading live buffers by buffer name
//!    rather than positional output slot. Segment read-only inputs are
//!    assembled from the caller's original bytes or prior segment
//!    outputs; final host-visible output slots are reassembled in the
//!    original program's output declaration order.
//!
//! ## Device-resident variant
//!
//! [`dispatch_with_grid_sync_split_into`] round-trips every live buffer
//! host↔device between each segment and on every fixpoint pass. For a fused
//! multi-rule program whose shared output accumulator is hundreds of MiB and
//! which splits into hundreds of segments, that transfer — not launch
//! latency — dominates wall time. [`dispatch_resident_grid_sync_fixpoint_into`]
//! is the device-resident counterpart: it uploads inputs into backend-resident
//! resources once, keeps them bound across every segment and fixpoint pass (so
//! the accumulator threads in place on-device, since resident dispatch never
//! clears a bound buffer between launches), and reads back only the final
//! outputs. It requires [`VyreBackend::supports_resident_dispatch`]; callers
//! route to it on resident-capable backends and to the host split otherwise.
//! Both paths are recall- and proof-identical (proven by a host/resident
//! differential gate); the choice is purely a host↔device-traffic optimization.
//!
//! ## Soundness
//!
//! - Atomicity preserved: every `atomic_or` that fired in segment N
//!   has flushed to global memory by the time segment N+1 launches  -
//!   backend launch APIs issue an implicit grid-level fence at
//!   submission boundaries.
//! - Ordering preserved: the original program's host-visible output
//!   is byte-identical to the un-split version, modulo timing.
//! - No re-validation surprise: each split segment validates against
//!   the same backend supported-ops set as the original.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use smallvec::SmallVec;
use vyre_foundation::ir::{BufferAccess, BufferDecl, Expr, Ident, MemoryKind, Node, Program};
use vyre_foundation::memory_model::MemoryOrdering;

use crate::backend::{
    BackendError, DispatchConfig, OutputBuffers, ResidentDispatchStep, ResidentReadRange, Resource,
    TimedDispatchResult, VyreBackend,
};
use crate::binding::{Binding, BindingPlan, BindingRole};

/// Walk past `Program::wrapped`'s synthetic outer Region. Real
/// programs are constructed via `wrapped`, which inserts a single
/// outer Region around the user's entry sequence; the split logic
/// must operate on the inner sequence so a `GridSync` barrier inside
/// the wrapper actually splits the program. Programs constructed
/// via `Program::new` use the entry sequence directly  -  in that
/// case we just return it unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
enum EntryWrapper {
    Region { generator: Ident },
    Block,
}

struct PlannedGridSyncSegment {
    program: Program,
    input_names: Vec<Ident>,
    output_names: Vec<Ident>,
}

fn peel_entry_wrappers(program: &Program) -> (Vec<EntryWrapper>, &[Node]) {
    let mut wrappers = Vec::new();
    let mut entry = program.entry();
    loop {
        if entry.len() == 1 {
            match &entry[0] {
                Node::Region {
                    generator, body, ..
                } => {
                    wrappers.push(EntryWrapper::Region {
                        generator: generator.clone(),
                    });
                    entry = body.as_slice();
                    continue;
                }
                Node::Block(body) => {
                    wrappers.push(EntryWrapper::Block);
                    entry = body.as_slice();
                    continue;
                }
                _ => {}
            }
        }
        break;
    }
    (wrappers, entry)
}

fn entry_sequence(program: &Program) -> &[Node] {
    peel_entry_wrappers(program).1
}

/// Whether `program` contains any `Node::Barrier { ordering: GridSync }`
/// in its dispatch-level entry sequence (peeled past any synthetic
/// outer Region).
///
/// The check is intentionally shallow: nested grid-sync barriers
/// inside `Node::Loop` or inner `Node::Region` bodies are a contract
/// violation (`validate::barrier` rejects them) and never reach this
/// path. The split operates at the dispatch-level granularity.
#[must_use]
pub fn contains_grid_sync(program: &Program) -> bool {
    // O(1) negative gate: if the cached ProgramStats bitset records no
    // Barrier of any kind in the entire tree, there is definitely no
    // top-level GridSync barrier either. Skip the entry-sequence walk
    // (which itself is shallow but still pays a buffers/buffer_index
    // dispatch on every backend dispatch path).
    if !program.stats().has_node_barrier() {
        return false;
    }
    node_slice_contains_grid_sync(entry_sequence(program))
}

fn node_slice_contains_grid_sync(nodes: &[Node]) -> bool {
    nodes.iter().any(node_contains_grid_sync)
}

fn node_contains_grid_sync(node: &Node) -> bool {
    match node {
        Node::Barrier {
            ordering: MemoryOrdering::GridSync,
            ..
        } => true,
        Node::If {
            then, otherwise, ..
        } => node_slice_contains_grid_sync(then) || node_slice_contains_grid_sync(otherwise),
        Node::Loop { body, .. } | Node::Block(body) => node_slice_contains_grid_sync(body),
        Node::Region { body, .. } => node_slice_contains_grid_sync(body),
        _ => false,
    }
}

/// Split `program` at every top-level `Node::Barrier { GridSync }`.
///
/// Returns a vector of segments in execution order. The barrier nodes
/// themselves are dropped from the segments  -  the kernel-launch
/// boundary between segments takes their place.
///
/// Each returned segment is a complete `Program` that shares the
/// original's buffer table, workgroup size, and metadata; only the
/// entry sequence changes. Segments without any executable nodes are
/// preserved (an empty segment between two adjacent barriers becomes
/// a no-op kernel that completes with byte-identical inputs and
/// outputs).
#[must_use]
pub fn split_on_grid_sync(program: &Program) -> Vec<Program> {
    match try_split_on_grid_sync(program) {
        Ok(segments) => segments,
        Err(_error) => Vec::new(),
    }
}

/// Fallible variant of [`split_on_grid_sync`] for production dispatch paths.
///
/// # Errors
/// Returns an actionable [`BackendError`] if segment storage cannot be
/// reserved or if split accounting overflows.
fn hoist_grid_sync_barriers(nodes: &[Node]) -> Vec<Node> {
    let mut new_nodes = Vec::new();
    for node in nodes {
        match node {
            Node::Block(body) => {
                let new_body = hoist_grid_sync_barriers(body);
                let has_barrier = new_body.iter().any(|n| {
                    matches!(
                        n,
                        Node::Barrier {
                            ordering: MemoryOrdering::GridSync,
                            ..
                        }
                    )
                });
                if has_barrier {
                    let mut current_segment = Vec::new();
                    for b_node in new_body {
                        if matches!(
                            b_node,
                            Node::Barrier {
                                ordering: MemoryOrdering::GridSync,
                                ..
                            }
                        ) {
                            new_nodes.push(Node::Block(std::mem::take(&mut current_segment)));
                            new_nodes.push(b_node);
                        } else {
                            current_segment.push(b_node);
                        }
                    }
                    new_nodes.push(Node::Block(current_segment));
                } else {
                    new_nodes.push(Node::Block(new_body));
                }
            }
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let new_body = hoist_grid_sync_barriers(body);
                let has_barrier = new_body.iter().any(|n| {
                    matches!(
                        n,
                        Node::Barrier {
                            ordering: MemoryOrdering::GridSync,
                            ..
                        }
                    )
                });
                if has_barrier {
                    let mut current_segment = Vec::new();
                    for b_node in new_body {
                        if matches!(
                            b_node,
                            Node::Barrier {
                                ordering: MemoryOrdering::GridSync,
                                ..
                            }
                        ) {
                            new_nodes.push(Node::Region {
                                generator: generator.clone(),
                                source_region: source_region.clone(),
                                body: Arc::new(std::mem::take(&mut current_segment)),
                            });
                            new_nodes.push(b_node);
                        } else {
                            current_segment.push(b_node);
                        }
                    }
                    new_nodes.push(Node::Region {
                        generator: generator.clone(),
                        source_region: source_region.clone(),
                        body: Arc::new(current_segment),
                    });
                } else {
                    new_nodes.push(Node::Region {
                        generator: generator.clone(),
                        source_region: source_region.clone(),
                        body: Arc::new(new_body),
                    });
                }
            }
            other => {
                new_nodes.push(other.clone());
            }
        }
    }
    new_nodes
}

fn collect_global_let_bindings(nodes: &[Node], map: &mut std::collections::HashMap<String, Node>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => {
                map.insert(name.as_str().to_string(), node.clone());
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_global_let_bindings(then, map);
                collect_global_let_bindings(otherwise, map);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_global_let_bindings(body, map);
            }
            Node::Region { body, .. } => {
                collect_global_let_bindings(&body[..], map);
            }
            _ => {}
        }
    }
}

fn collect_locally_defined_vars(nodes: &[Node], vars: &mut std::collections::HashSet<String>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => {
                vars.insert(name.as_str().to_string());
            }
            Node::Loop { var, body, .. } => {
                vars.insert(var.as_str().to_string());
                collect_locally_defined_vars(body, vars);
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_locally_defined_vars(then, vars);
                collect_locally_defined_vars(otherwise, vars);
            }
            Node::Block(body) => {
                collect_locally_defined_vars(body, vars);
            }
            Node::Region { body, .. } => {
                collect_locally_defined_vars(&body[..], vars);
            }
            _ => {}
        }
    }
}

fn collect_referenced_vars(expr: &Expr, vars: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            vars.insert(name.as_str().to_string());
        }
        Expr::Load { index, .. } => {
            collect_referenced_vars(index, vars);
        }
        Expr::BinOp { left, right, .. } => {
            collect_referenced_vars(left, vars);
            collect_referenced_vars(right, vars);
        }
        Expr::UnOp { operand, .. } => {
            collect_referenced_vars(operand, vars);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_referenced_vars(arg, vars);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_referenced_vars(cond, vars);
            collect_referenced_vars(true_val, vars);
            collect_referenced_vars(false_val, vars);
        }
        Expr::Cast { value, .. } => {
            collect_referenced_vars(value, vars);
        }
        Expr::Fma { a, b, c } => {
            collect_referenced_vars(a, vars);
            collect_referenced_vars(b, vars);
            collect_referenced_vars(c, vars);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            collect_referenced_vars(index, vars);
            if let Some(expected) = expected {
                collect_referenced_vars(expected, vars);
            }
            collect_referenced_vars(value, vars);
        }
        Expr::SubgroupBallot { cond } => {
            collect_referenced_vars(cond, vars);
        }
        Expr::SubgroupShuffle { value, lane } => {
            collect_referenced_vars(value, vars);
            collect_referenced_vars(lane, vars);
        }
        Expr::SubgroupReduce { value, .. } => {
            collect_referenced_vars(value, vars);
        }
        _ => {}
    }
}

fn collect_node_referenced_vars(node: &Node, vars: &mut std::collections::HashSet<String>) {
    match node {
        Node::Let { value, .. } => {
            collect_referenced_vars(value, vars);
        }
        Node::Assign { value, .. } => {
            collect_referenced_vars(value, vars);
        }
        Node::Store { index, value, .. } => {
            collect_referenced_vars(index, vars);
            collect_referenced_vars(value, vars);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            collect_referenced_vars(cond, vars);
            for n in then {
                collect_node_referenced_vars(n, vars);
            }
            for n in otherwise {
                collect_node_referenced_vars(n, vars);
            }
        }
        Node::Loop { from, to, body, .. } => {
            collect_referenced_vars(from, vars);
            collect_referenced_vars(to, vars);
            for n in body {
                collect_node_referenced_vars(n, vars);
            }
        }
        Node::Block(body) => {
            for n in body {
                collect_node_referenced_vars(n, vars);
            }
        }
        Node::Region { body, .. } => {
            for n in body.as_ref() {
                collect_node_referenced_vars(n, vars);
            }
        }
        Node::AsyncLoad { offset, size, .. } => {
            collect_referenced_vars(offset, vars);
            collect_referenced_vars(size, vars);
        }
        Node::AsyncStore { offset, size, .. } => {
            collect_referenced_vars(offset, vars);
            collect_referenced_vars(size, vars);
        }
        Node::Trap { address, .. } => {
            collect_referenced_vars(address, vars);
        }
        _ => {}
    }
}

fn resolve_dependencies(
    name: &str,
    global_lets: &std::collections::HashMap<String, Node>,
    resolved_names: &mut std::collections::HashSet<String>,
    resolved_lets: &mut Vec<Node>,
) {
    if resolved_names.contains(name) {
        return;
    }
    if let Some(let_node) = global_lets.get(name) {
        resolved_names.insert(name.to_string());
        let mut deps = std::collections::HashSet::new();
        collect_node_referenced_vars(let_node, &mut deps);
        for dep in deps {
            resolve_dependencies(&dep, global_lets, resolved_names, resolved_lets);
        }
        resolved_lets.push(let_node.clone());
    }
}

fn propagate_let_bindings(segments: &mut [Vec<Node>], hoisted_inner: &[Node]) {
    let mut global_lets = std::collections::HashMap::new();
    collect_global_let_bindings(hoisted_inner, &mut global_lets);

    for segment_nodes in segments {
        let mut locally_defined = std::collections::HashSet::new();
        collect_locally_defined_vars(segment_nodes, &mut locally_defined);

        let mut referenced = std::collections::HashSet::new();
        for node in segment_nodes.iter() {
            collect_node_referenced_vars(node, &mut referenced);
        }

        let mut free_vars = Vec::new();
        for name in referenced {
            if !locally_defined.contains(&name) {
                free_vars.push(name);
            }
        }

        let mut resolved_lets = Vec::new();
        let mut resolved_names = std::collections::HashSet::new();
        for name in free_vars {
            resolve_dependencies(&name, &global_lets, &mut resolved_names, &mut resolved_lets);
        }

        if !resolved_lets.is_empty() {
            resolved_lets.extend(std::mem::take(segment_nodes));
            *segment_nodes = resolved_lets;
        }
    }
}

/// Fallible variant of [`split_on_grid_sync`] for production dispatch paths.
///
/// # Errors
/// Returns an actionable [`BackendError`] if segment storage cannot be
/// reserved or if split accounting overflows.

pub fn try_split_on_grid_sync(program: &Program) -> Result<Vec<Program>, BackendError> {
    let (wrappers, inner) = peel_entry_wrappers(program);
    let hoisted_inner = hoist_grid_sync_barriers(inner);
    let split_count = hoisted_inner
        .iter()
        .filter(|node| {
            matches!(
                node,
                Node::Barrier {
                    ordering: MemoryOrdering::GridSync,
                    ..
                }
            )
        })
        .count();
    if split_count == 0 {
        let mut segments = Vec::new();
        reserve_grid_sync_vec(&mut segments, 1, "grid-sync no-op segment")?;
        segments.push(program.clone());
        return Ok(segments);
    }

    let segment_count = split_count + 1;
    let executable_nodes = hoisted_inner.len().checked_sub(split_count).ok_or_else(|| {
        BackendError::InvalidProgram {
            fix: format!(
            "grid-sync split_count {split_count} exceeded entry node count {}. Fix: split_on_grid_sync must count barriers from the same entry sequence it segments.",
            hoisted_inner.len()
            ),
        }
    })?;
    let segment_capacity = executable_nodes.div_ceil(segment_count);

    let mut raw_segments = Vec::new();
    let mut current = Vec::new();
    reserve_grid_sync_vec(&mut current, segment_capacity, "grid-sync current segment")?;
    for node in &hoisted_inner {
        match node {
            Node::Barrier {
                ordering: MemoryOrdering::GridSync,
                ..
            } => {
                let mut next = Vec::new();
                reserve_grid_sync_vec(&mut next, segment_capacity, "grid-sync next segment")?;
                let entry = std::mem::replace(&mut current, next);
                raw_segments.push(entry);
            }
            other => {
                current.push(other.clone());
            }
        }
    }
    raw_segments.push(current);

    propagate_let_bindings(&mut raw_segments, &hoisted_inner);

    let mut segments = Vec::new();
    reserve_grid_sync_vec(
        &mut segments,
        raw_segments.len(),
        "grid-sync split segments",
    )?;
    for entry in raw_segments {
        segments.push(wrap_split_segment(program, &wrappers, entry));
    }
    Ok(segments)
}

fn wrap_split_segment(program: &Program, wrappers: &[EntryWrapper], entry: Vec<Node>) -> Program {
    // Re-wrap each segment in the same wrapper stack the source had,
    // so tagged/fused programs keep provenance and structure while the
    // executable body is split at launch boundaries.
    let mut wrapped_entry = entry;
    for wrapper in wrappers.iter().rev() {
        match wrapper {
            EntryWrapper::Region { generator } => {
                wrapped_entry = vec![Node::Region {
                    generator: generator.clone(),
                    source_region: None,
                    body: Arc::new(wrapped_entry),
                }];
            }
            EntryWrapper::Block => {
                wrapped_entry = vec![Node::Block(wrapped_entry)];
            }
        }
    }
    program.with_rewritten_entry(wrapped_entry)
}

/// Diagnostics: the host-split segment **programs** (post buffer-rewrite) that
/// the host-split dispatch path (`dispatch_with_grid_sync_split*`) validates and
/// launches when the backend lacks native grid-sync. Exposed so tooling and
/// tests can inspect or validate each segment without a live backend — the
/// raw [`try_split_on_grid_sync`] output omits the per-segment buffer
/// access/role rewrite, so it is not what the backend actually sees.
///
/// # Errors
/// Propagates any [`BackendError`] from splitting or buffer rewriting.
pub fn plan_host_grid_sync_segment_programs(
    program: &Program,
) -> Result<Vec<Program>, BackendError> {
    Ok(plan_host_grid_sync_segments(program)?
        .into_iter()
        .map(|segment| segment.program)
        .collect())
}

fn plan_host_grid_sync_segments(
    program: &Program,
) -> Result<Vec<PlannedGridSyncSegment>, BackendError> {
    let split = try_split_on_grid_sync(program)?;
    let first_writer = first_writer_segment_per_buffer(&split, program)?;
    let mut planned = Vec::new();
    reserve_grid_sync_vec(&mut planned, split.len(), "grid-sync planned host segments")?;
    for (segment_idx, segment) in split.into_iter().enumerate() {
        let rewritten = rewrite_segment_buffers_for_host_split(
            program,
            &segment,
            segment_idx,
            &first_writer,
        )?;
        let input_names = segment_input_names(&rewritten)?;
        let output_names = segment_output_names(&rewritten)?;
        planned.push(PlannedGridSyncSegment {
            program: rewritten,
            input_names,
            output_names,
        });
    }
    Ok(planned)
}

/// For each buffer name, the index of the FIRST split segment that writes it.
///
/// A source-output buffer written by more than one segment is an
/// **accumulator**: each segment writes only its own slots (e.g. a fused
/// multi-rule `results_packed`, where every rule's result-store lands in a
/// different grid-sync segment). A LATER writer must therefore read+merge the
/// value forwarded from earlier segments via `current_inputs`, never overwrite
/// it with a fresh WriteOnly buffer — which would silently zero every earlier
/// segment's slots (recall=0 for every rule whose store is not in the final
/// segment). `rewrite_segment_buffers_for_host_split` uses this map to keep an
/// already-produced output buffer as a `ReadWrite` accumulator in later
/// segments instead of a write-only output.
fn first_writer_segment_per_buffer(
    split: &[Program],
    program: &Program,
) -> Result<HashMap<Ident, usize>, BackendError> {
    let mut first_writer: HashMap<Ident, usize> = HashMap::new();
    reserve_grid_sync_hash_map(
        &mut first_writer,
        program.buffers().len(),
        "grid-sync first-writer map",
    )?;
    for (segment_idx, segment) in split.iter().enumerate() {
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();
        reserve_grid_sync_hash_set(
            &mut reads,
            program.buffers().len(),
            "grid-sync first-writer read scan",
        )?;
        reserve_grid_sync_hash_set(
            &mut writes,
            program.buffers().len(),
            "grid-sync first-writer write scan",
        )?;
        for node in entry_sequence(segment) {
            collect_segment_buffer_targets(node, &mut reads, &mut writes);
        }
        for name in writes {
            first_writer.entry(name).or_insert(segment_idx);
        }
    }
    Ok(first_writer)
}

fn rewrite_segment_buffers_for_host_split(
    source: &Program,
    segment: &Program,
    segment_idx: usize,
    first_writer: &HashMap<Ident, usize>,
) -> Result<Program, BackendError> {
    let mut reads = HashSet::new();
    let mut writes = HashSet::new();
    reserve_grid_sync_hash_set(
        &mut reads,
        source.buffers().len(),
        "grid-sync segment read set",
    )?;
    reserve_grid_sync_hash_set(
        &mut writes,
        source.buffers().len(),
        "grid-sync segment write set",
    )?;
    for node in entry_sequence(segment) {
        collect_segment_buffer_targets(node, &mut reads, &mut writes);
    }

    let mut buffers = Vec::new();
    reserve_grid_sync_vec(
        &mut buffers,
        source.buffers().len(),
        "grid-sync segment buffers",
    )?;
    for buffer in source.buffers() {
        let name = Ident::from(buffer.name());
        let reads_this = reads.contains(&name);
        let writes_this = writes.contains(&name);
        let readwrite_passthrough = matches!(buffer.access(), BufferAccess::ReadWrite)
            && !buffer.is_output()
            && !buffer.is_pipeline_live_out()
            && !reads_this
            && !writes_this;

        if !reads_this && !writes_this && !readwrite_passthrough {
            continue;
        }

        let mut rewritten = buffer.clone();
        if matches!(rewritten.access(), BufferAccess::Workgroup) {
            buffers.push(rewritten);
            continue;
        }

        // A source-output buffer that an EARLIER segment already wrote is an
        // accumulator across the split: this segment must read the value
        // forwarded via `current_inputs` and merge its own slots, never
        // overwrite it with a fresh WriteOnly buffer (which zeroes the earlier
        // segments' slots — the silent recall=0 mode for any fused rule whose
        // result-store does not land in the final segment).
        let is_source_output = buffer.is_output() || buffer.is_pipeline_live_out();
        let earlier_segment_wrote_output = is_source_output
            && first_writer
                .get(&name)
                .is_some_and(|&first| first < segment_idx);

        let access = if readwrite_passthrough {
            BufferAccess::ReadWrite
        } else if earlier_segment_wrote_output && writes_this {
            // Later writer of a multi-segment output accumulator: read the
            // accumulated prior value (uploaded as input) and merge this
            // segment's slots in place.
            BufferAccess::ReadWrite
        } else {
            match (reads_this, writes_this) {
                (true, true) => BufferAccess::ReadWrite,
                (true, false) => BufferAccess::ReadOnly,
                (false, true) => BufferAccess::WriteOnly,
                (false, false) => BufferAccess::ReadWrite,
            }
        };
        rewrite_segment_buffer_access(&mut rewritten, access);
        // Never mark a split segment's buffer as the program output: a
        // multi-segment output accumulator must CONSUME its forwarded prior
        // value as input in later segments, and `segment_buffer_consumes_input`
        // refuses any `is_output` buffer. Each writing segment still produces
        // the buffer (WriteOnly/ReadWrite both produce output), so its bytes
        // are captured into `current_inputs`; the final host-visible values are
        // reassembled by name from the SOURCE program's output set in
        // `collect_final_named_outputs`, independent of any per-segment flag.
        rewritten.is_output = false;
        rewritten.pipeline_live_out = false;
        buffers.push(rewritten);
    }

    Ok(segment.with_rewritten_buffers(buffers))
}

fn rewrite_segment_buffer_access(buffer: &mut BufferDecl, access: BufferAccess) {
    buffer.kind = match &access {
        BufferAccess::ReadOnly => MemoryKind::Readonly,
        BufferAccess::Uniform => MemoryKind::Uniform,
        BufferAccess::Workgroup => MemoryKind::Shared,
        _ => MemoryKind::Global,
    };
    buffer.access = access;
}

fn segment_input_names(segment: &Program) -> Result<Vec<Ident>, BackendError> {
    let mut names = Vec::new();
    reserve_grid_sync_vec(
        &mut names,
        segment.buffers().len(),
        "grid-sync segment input names",
    )?;
    for buffer in segment.buffers() {
        if matches!(buffer.access(), BufferAccess::Workgroup) {
            continue;
        }
        if segment_buffer_consumes_input(buffer) {
            names.push(Ident::from(buffer.name()));
        }
    }
    Ok(names)
}

fn segment_output_names(segment: &Program) -> Result<Vec<Ident>, BackendError> {
    let mut names = Vec::new();
    reserve_grid_sync_vec(
        &mut names,
        segment.buffers().len(),
        "grid-sync segment output names",
    )?;
    for buffer in segment.buffers() {
        if matches!(buffer.access(), BufferAccess::Workgroup) {
            continue;
        }
        if segment_buffer_produces_output(buffer) {
            names.push(Ident::from(buffer.name()));
        }
    }
    Ok(names)
}

fn original_input_names(program: &Program) -> Result<Vec<Ident>, BackendError> {
    segment_input_names(program)
}

fn original_output_names(program: &Program) -> Result<Vec<Ident>, BackendError> {
    segment_output_names(program)
}

fn segment_buffer_consumes_input(buffer: &BufferDecl) -> bool {
    if buffer.is_output() || buffer.is_pipeline_live_out() {
        return false;
    }
    matches!(
        buffer.access(),
        BufferAccess::ReadOnly | BufferAccess::ReadWrite | BufferAccess::Uniform
    )
}

fn segment_buffer_produces_output(buffer: &BufferDecl) -> bool {
    buffer.is_output()
        || buffer.is_pipeline_live_out()
        || matches!(
            buffer.access(),
            BufferAccess::ReadWrite | BufferAccess::WriteOnly
        )
}

fn collect_segment_buffer_targets(
    node: &Node,
    reads: &mut HashSet<Ident>,
    writes: &mut HashSet<Ident>,
) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            collect_segment_expr_targets(value, reads, writes);
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            writes.insert(Ident::from(buffer));
            collect_segment_expr_targets(index, reads, writes);
            collect_segment_expr_targets(value, reads, writes);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            collect_segment_expr_targets(cond, reads, writes);
            for child in then.iter().chain(otherwise.iter()) {
                collect_segment_buffer_targets(child, reads, writes);
            }
        }
        Node::Loop { from, to, body, .. } => {
            collect_segment_expr_targets(from, reads, writes);
            collect_segment_expr_targets(to, reads, writes);
            for child in body {
                collect_segment_buffer_targets(child, reads, writes);
            }
        }
        Node::Block(body) => {
            for child in body {
                collect_segment_buffer_targets(child, reads, writes);
            }
        }
        Node::Region { body, .. } => {
            for child in body.iter() {
                collect_segment_buffer_targets(child, reads, writes);
            }
        }
        Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
            reads.insert(buffer.clone());
            writes.insert(buffer.clone());
        }
        Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
            reads.insert(input.clone());
            writes.insert(output.clone());
        }
        Node::IndirectDispatch { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => {}
        _ => {}
    }
}

fn collect_segment_expr_targets(
    expr: &Expr,
    reads: &mut HashSet<Ident>,
    writes: &mut HashSet<Ident>,
) {
    match expr {
        Expr::Load { buffer, index } => {
            reads.insert(Ident::from(buffer));
            collect_segment_expr_targets(index, reads, writes);
        }
        Expr::Atomic {
            buffer,
            index,
            expected,
            value,
            ..
        } => {
            let name = Ident::from(buffer);
            reads.insert(name.clone());
            writes.insert(name);
            collect_segment_expr_targets(index, reads, writes);
            if let Some(expected) = expected {
                collect_segment_expr_targets(expected, reads, writes);
            }
            collect_segment_expr_targets(value, reads, writes);
        }
        Expr::BinOp { left, right, .. } => {
            collect_segment_expr_targets(left, reads, writes);
            collect_segment_expr_targets(right, reads, writes);
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            collect_segment_expr_targets(operand, reads, writes);
        }
        Expr::Fma { a, b, c } => {
            collect_segment_expr_targets(a, reads, writes);
            collect_segment_expr_targets(b, reads, writes);
            collect_segment_expr_targets(c, reads, writes);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_segment_expr_targets(arg, reads, writes);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_segment_expr_targets(cond, reads, writes);
            collect_segment_expr_targets(true_val, reads, writes);
            collect_segment_expr_targets(false_val, reads, writes);
        }
        Expr::SubgroupBallot { cond } => collect_segment_expr_targets(cond, reads, writes),
        Expr::SubgroupShuffle { value, lane } => {
            collect_segment_expr_targets(value, reads, writes);
            collect_segment_expr_targets(lane, reads, writes);
        }
        Expr::SubgroupReduce { value, .. } => collect_segment_expr_targets(value, reads, writes),
        _ => {}
    }
}

/// Universal dispatch helper that satisfies `Node::Barrier { ordering:
/// GridSync }` on any backend by splitting at the barrier and running
/// each segment as its own kernel launch.
///
/// Backends with native cooperative-launch grid sync (advertised via
/// [`VyreBackend::supports_grid_sync`]) bypass the split  -  the
/// program is dispatched once. Backends without it route here so the
/// kernel-launch boundary becomes the grid-level fence: every prior
/// write is globally visible to subsequent launches.
///
/// # Inputs
/// `inputs` matches the input slice the caller would have passed to
/// `dispatch_borrowed`. After each segment, the helper refreshes
/// every ReadWrite buffer's slot from the segment's readback so the
/// next segment sees the prior writes.
///
/// # Errors
/// Propagates any `BackendError` raised by `dispatch_borrowed` on a
/// segment, prefixed with the segment index for diagnosability.
pub fn dispatch_with_grid_sync_split(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
) -> Result<Vec<Vec<u8>>, BackendError> {
    let mut outputs = Vec::new();
    reserve_grid_sync_vec(
        &mut outputs,
        program.output_buffer_indices().len().max(1),
        "grid-sync final outputs",
    )?;
    dispatch_with_grid_sync_split_into(backend, program, inputs, config, &mut outputs)?;
    Ok(outputs)
}

/// Timed variant of [`dispatch_with_grid_sync_split`].
///
/// # Errors
/// Propagates any [`BackendError`] raised by a segment dispatch.
pub fn dispatch_with_grid_sync_split_timed(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
) -> Result<TimedDispatchResult, BackendError> {
    let started = std::time::Instant::now();
    let outputs = dispatch_with_grid_sync_split(backend, program, inputs, config)?;
    Ok(TimedDispatchResult {
        outputs,
        wall_ns: elapsed_wall_ns(started)?,
        device_ns: None,
        enqueue_ns: None,
        wait_ns: None,
    })
}

/// Resident-resource variant of [`dispatch_with_grid_sync_split_timed`].
///
/// This keeps the same resource handles bound for every segment. Read-write
/// buffers therefore refresh in place on the backend's device-resident storage
/// between segment launches instead of downloading bytes to the host and
/// re-uploading them as the next segment's inputs.
///
/// # Errors
/// Propagates any [`BackendError`] raised by a segment resident dispatch.
pub fn dispatch_resident_with_grid_sync_split_timed(
    backend: &dyn VyreBackend,
    program: &Program,
    resources: &[Resource],
    config: &DispatchConfig,
) -> Result<TimedDispatchResult, BackendError> {
    // These are the explicit non-native grid-sync routes (host split /
    // resident fixpoint). They split unconditionally when the program carries a
    // grid-sync barrier: native cooperative launch has a residency ceiling, so
    // `supports_grid_sync()` no longer implies "this program runs natively".
    // The orchestrator (or the registry's `should_split_grid_sync`) decides
    // native-vs-split per program; once here, always split.
    if !contains_grid_sync(program) {
        return backend.dispatch_resident_timed(program, resources, config);
    }
    let segments = try_split_on_grid_sync(program)?;
    if segments.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: program contains GridSync barrier but split_on_grid_sync produced 0 \
                  segments. This is a grid_sync invariant bug  -  split_on_grid_sync must \
                  always return at least one segment."
                .to_string(),
        });
    }
    let started = std::time::Instant::now();
    let mut final_outputs = Vec::new();
    let mut device_ns = Some(0_u64);
    let mut enqueue_ns = Some(0_u64);
    let mut wait_ns = Some(0_u64);
    for (segment_idx, segment) in segments.iter().enumerate() {
        let timed = backend
            .dispatch_resident_timed(segment, resources, config)
            .map_err(|error| grid_sync_segment_error(error, segment_idx, segments.len()))?;
        if segment_idx + 1 == segments.len() {
            final_outputs = timed.outputs;
        }
        device_ns = sum_optional_timing(device_ns, timed.device_ns, "device timing")?;
        enqueue_ns = sum_optional_timing(enqueue_ns, timed.enqueue_ns, "enqueue timing")?;
        wait_ns = sum_optional_timing(wait_ns, timed.wait_ns, "wait timing")?;
    }
    Ok(TimedDispatchResult {
        outputs: final_outputs,
        wall_ns: elapsed_wall_ns(started)?,
        device_ns,
        enqueue_ns,
        wait_ns,
    })
}

fn elapsed_wall_ns(started: std::time::Instant) -> Result<u64, BackendError> {
    u64::try_from(started.elapsed().as_nanos()).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: grid-sync segmented wall timing cannot fit u64 nanoseconds: {error}. Split telemetry windows or report per-segment timing."
        ),
    })
}

fn sum_optional_timing(
    accumulator: Option<u64>,
    next: Option<u64>,
    field: &'static str,
) -> Result<Option<u64>, BackendError> {
    match (accumulator, next) {
        (Some(left), Some(right)) => Ok(Some(left.checked_add(right).ok_or_else(|| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: grid-sync segmented {field} overflowed u64 nanoseconds. Split telemetry windows or report per-segment timing instead of silently clamping."
                ),
            }
        })?)),
        _ => Ok(None),
    }
}

/// Variant of [`dispatch_with_grid_sync_split`] that writes final outputs into
/// caller-owned storage.
///
/// # Errors
/// Propagates any `BackendError` raised by a segment dispatch.
pub fn dispatch_with_grid_sync_split_into(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    // These are the explicit non-native grid-sync routes (host split /
    // resident fixpoint). They split unconditionally when the program carries a
    // grid-sync barrier: native cooperative launch has a residency ceiling, so
    // `supports_grid_sync()` no longer implies "this program runs natively".
    // The orchestrator (or the registry's `should_split_grid_sync`) decides
    // native-vs-split per program; once here, always split.
    if !contains_grid_sync(program) {
        return backend.dispatch_borrowed_into(program, inputs, config, outputs);
    }
    let segments = plan_host_grid_sync_segments(program)?;
    if segments.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: program contains GridSync barrier but split_on_grid_sync produced 0 \
                  segments. This is a grid_sync invariant bug  -  split_on_grid_sync must \
                  always return at least one segment."
                .to_string(),
        });
    }
    crate::observability::record_grid_sync_split(segments.len());
    // Build a mutable input set we rotate between segments. ReadOnly
    // inputs stay borrowed from the caller for the whole split; only
    // ReadWrite buffers become owned after a segment produces updated
    // bytes. The previous implementation cloned every input before
    // the first launch, which turned large read-only buffers into a
    // host-memory copy on the slow path.
    let initial_input_names = original_input_names(program)?;
    if inputs.len() != initial_input_names.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: grid-sync split expected {} initial input buffer(s) but received {}. Rebuild the dispatch inputs from the Program buffer declarations before splitting.",
                initial_input_names.len(),
                inputs.len()
            ),
        });
    }
    let mut current_inputs: HashMap<Ident, GridSyncInput<'_>> = HashMap::new();
    reserve_grid_sync_hash_map(
        &mut current_inputs,
        program.buffers().len(),
        "grid-sync rotating input map",
    )?;
    for (name, bytes) in initial_input_names.into_iter().zip(inputs.iter().copied()) {
        current_inputs.insert(name, GridSyncInput::Borrowed(bytes));
    }
    let mut segment_outputs = Vec::new();
    reserve_grid_sync_vec(
        &mut segment_outputs,
        outputs.capacity().max(1),
        "grid-sync intermediate outputs",
    )?;
    let final_output_names = original_output_names(program)?;

    // Honor the program's fixpoint contract across the split. The
    // non-split dispatch path (`dispatch_borrowed`) re-runs the WHOLE
    // program `fixpoint_iterations` times with persistent ReadWrite
    // buffers, so a program authored as a fixpoint closure converges —
    // a multi-hop reachability/dataflow closure is exactly this shape: a
    // `seed (acc |= source) → hop (acc' = step(acc)) → merge (acc |= acc')`
    // body whose accumulator grows by ONE dataflow hop per whole-program
    // pass, relying on the dispatcher to iterate it to a fixpoint.
    //
    // GridSync barriers split that body across segments, so ONE pass over
    // the segment sequence advances the accumulator by exactly one hop.
    // Re-running an individual SEGMENT N times (the previous behavior:
    // `config` with its fixpoint count reached each segment) does NOT
    // converge — re-launching the isolated `hop` segment recomputes the
    // same frontier from an unchanged `acc`. The whole SEQUENCE must be
    // looped instead, with each segment run once per pass. Net device work
    // is identical (sequence_len × iterations launches either way); only
    // the nesting order changes, which is what makes the closure converge.
    // A flow that needs k hops through k-1 intermediate variables (the
    // dominant launch-rule shape: `q = src; sink(q)`) silently returned an
    // empty frontier under the old single-pass split — recall=0.
    let iterations = crate::fixpoint_iterations::resolve_fixpoint_iterations(
        config,
        "grid-sync split",
    )?;
    let mut segment_config = config.clone();
    segment_config.fixpoint_iterations = Some(1);

    // Adaptive convergence: `iterations` is an UPPER bound (the worst-case hop
    // depth, one hop per whole-sequence pass). The segment sequence is a
    // deterministic function of its live buffers, so once a full pass leaves
    // every evolving (Owned) accumulator unchanged the closure has reached a
    // fixpoint — every remaining pass would re-dispatch the entire segment
    // sequence (hundreds of launches on a large fused program) for zero new
    // dataflow. Stop as soon as two consecutive passes produce the same state.
    let mut prev_fingerprint: Option<u64> = None;
    for _ in 0..iterations {
        for (segment_idx, segment) in segments.iter().enumerate() {
            let borrowed = borrowed_grid_sync_inputs_by_name(segment, &current_inputs)?;
            backend
                .dispatch_borrowed_into(
                    &segment.program,
                    borrowed.as_slice(),
                    &segment_config,
                    &mut segment_outputs,
                )
                .map_err(|error| grid_sync_segment_error(error, segment_idx, segments.len()))?;
            drop(borrowed);
            refresh_named_outputs(segment, &mut segment_outputs, &mut current_inputs)?;
        }
        let fingerprint = owned_accumulator_fingerprint(&current_inputs);
        if prev_fingerprint == Some(fingerprint) {
            break;
        }
        prev_fingerprint = Some(fingerprint);
    }
    collect_final_named_outputs(&final_output_names, &mut current_inputs, outputs)?;
    Ok(())
}

/// Device-resident counterpart of [`dispatch_with_grid_sync_split_into`].
///
/// The host-split path round-trips every live buffer host↔device between each
/// split segment AND on every fixpoint pass. A fused multi-rule
/// `results_packed` accumulator is hundreds of MiB, so a program that splits
/// into hundreds of segments moves tens of GiB across PCIe per dispatch — that
/// transfer, not launch latency, is the host-split wall.
///
/// This variant uploads the program's inputs into backend-resident resources
/// ONCE, keeps them bound across every segment and every fixpoint pass — so a
/// multi-rule accumulator threads IN PLACE on device storage with no host copy
/// and no clobber — and reads back only the final output ranges a single time.
/// Net host↔device traffic drops from `O(segments × passes × live_bytes)` to
/// `O(inputs + outputs)`.
///
/// Every split segment from [`try_split_on_grid_sync`] carries the full program
/// buffer table (only the executable entry sequence differs), so one resident
/// resource slice binds to every segment. Resident dispatch never clears a
/// bound buffer between launches, so each rule's result-store accumulates into
/// the shared device `results_packed` exactly as the un-split program would.
///
/// `outputs` is shaped byte-identically to
/// [`dispatch_with_grid_sync_split_into`]: one `Vec<u8>` per original output
/// buffer, in declaration order, so a caller can swap paths without changing
/// readback.
///
/// Requires a backend implementing the resident half of the [`VyreBackend`]
/// contract (`allocate_resident` / `upload_resident` /
/// `dispatch_resident_repeated_sequence_read_ranges_into` / `free_resident`).
/// A backend without residency fails loudly with `UnsupportedFeature` at the
/// first resident call; callers route those to
/// [`dispatch_with_grid_sync_split_into`].
///
/// # Errors
/// Propagates any [`BackendError`] from splitting, resident allocation, upload,
/// segment dispatch, or readback. Resident resources allocated by this call are
/// always freed before returning, on success and on error.
pub fn dispatch_resident_grid_sync_fixpoint_into(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    // These are the explicit non-native grid-sync routes (host split /
    // resident fixpoint). They split unconditionally when the program carries a
    // grid-sync barrier: native cooperative launch has a residency ceiling, so
    // `supports_grid_sync()` no longer implies "this program runs natively".
    // The orchestrator (or the registry's `should_split_grid_sync`) decides
    // native-vs-split per program; once here, always split.
    if !contains_grid_sync(program) {
        return backend.dispatch_borrowed_into(program, inputs, config, outputs);
    }
    let segments = try_split_on_grid_sync(program)?;
    if segments.is_empty() {
        return Err(BackendError::InvalidProgram {
            fix: "Fix: program contains GridSync barrier but split_on_grid_sync produced 0 \
                  segments. This is a grid_sync invariant bug  -  split_on_grid_sync must \
                  always return at least one segment."
                .to_string(),
        });
    }
    crate::observability::record_grid_sync_split(segments.len());

    // Allocate one resident resource per non-shared binding (caller inputs
    // uploaded; output/scratch buffers zeroed so an accumulator's unfired
    // slots stay 0), then run the fixpoint and read back final outputs.
    let resident = allocate_resident_program_resources(backend, program, inputs)?;
    let result =
        run_resident_grid_sync_fixpoint(backend, program, &segments, &resident, config, outputs);
    // Free every resident resource before returning, success or error.
    let free_result = free_resident_program_resources(backend, resident);
    result.and(free_result)
}

/// Resident resources backing one [`dispatch_resident_grid_sync_fixpoint_into`]
/// call: the binding-ordered slice every segment dispatches against, plus a
/// name → (handle, byte-len) map for output readback.
struct ResidentProgramResources {
    /// One resource per non-shared binding, in [`BindingPlan`] order — the
    /// slice the backend's resident dispatch binds positionally.
    ordered: Vec<Resource>,
    /// Buffer-name → (resident handle clone, byte length) for output readback
    /// by name. The handle is a cheap id clone; freeing `ordered` frees it.
    by_name: HashMap<Ident, (Resource, usize)>,
}

/// Allocate + initialize one resident resource per non-shared program binding.
///
/// Inputs are uploaded from the caller slice; output / write-only / scratch
/// buffers that consume no input are zeroed, mirroring the borrowed path's
/// memset of input-less buffers so a fused accumulator's unfired slots read 0.
fn allocate_resident_program_resources(
    backend: &dyn VyreBackend,
    program: &Program,
    inputs: &[&[u8]],
) -> Result<ResidentProgramResources, BackendError> {
    let plan = BindingPlan::from_borrowed_inputs(program, inputs)?;
    let mut ordered = Vec::new();
    reserve_grid_sync_vec(&mut ordered, plan.bindings.len(), "resident grid-sync resources")?;
    let mut by_name = HashMap::new();
    reserve_grid_sync_hash_map(
        &mut by_name,
        plan.bindings.len(),
        "resident grid-sync resource name map",
    )?;
    for binding in &plan.bindings {
        if binding.role == BindingRole::Shared {
            continue;
        }
        // Logical length is the caller input slice length (input bindings) or
        // the buffer's static size (outputs/scratch). The host path binds the
        // unused standard scanner buffers (counts/offsets/lengths/metadata) as
        // zero-length `&[]`; resident allocation rejects 0 bytes, so allocate
        // one element (element-aligned, so the backend's element-size
        // validation holds) for those — the kernel never reads a 0/1-element
        // unused buffer, so the placeholder is bound but inert (proven equal to
        // the host path by the resident/host differential gate).
        let byte_len = resident_binding_byte_len(binding, inputs)?;
        let alloc_len = byte_len.max(binding.element_size.max(1));
        let resource = backend.allocate_resident(alloc_len)?;
        // Upload exactly `alloc_len` bytes so the backend's full-buffer upload
        // contract holds: the caller input when it is non-empty, else zeros
        // (output/scratch buffers, and the inert zero-length standard inputs).
        match binding.input_index {
            Some(index) if !inputs.get(index).copied().unwrap_or(&[]).is_empty() => {
                let bytes = inputs[index];
                backend.upload_resident(&resource, bytes)?;
            }
            _ => {
                let zeros = zeroed_upload_buffer(alloc_len)?;
                backend.upload_resident(&resource, &zeros)?;
            }
        }
        by_name.insert(
            Ident::from(binding.name.as_ref()),
            (resource.clone(), byte_len),
        );
        ordered.push(resource);
    }
    Ok(ResidentProgramResources { ordered, by_name })
}

/// Byte length to allocate for a binding's resident resource: the caller input
/// slice length for input-consuming bindings, else the buffer's static size.
fn resident_binding_byte_len(
    binding: &Binding,
    inputs: &[&[u8]],
) -> Result<usize, BackendError> {
    if let Some(index) = binding.input_index {
        if let Some(bytes) = inputs.get(index) {
            return Ok(bytes.len());
        }
    }
    binding.static_byte_len.ok_or_else(|| BackendError::InvalidProgram {
        fix: format!(
            "Fix: resident grid-sync output buffer `{}` has no static byte length; dynamic-sized outputs are not supported on the resident grid-sync path. Declare a fixed `count` on the buffer or route this program through dispatch_with_grid_sync_split_into.",
            binding.name
        ),
    })
}

/// Allocate a zero-filled host staging buffer of `byte_len` for initializing a
/// resident output/scratch resource.
fn zeroed_upload_buffer(byte_len: usize) -> Result<Vec<u8>, BackendError> {
    let mut zeros = Vec::new();
    crate::allocation::try_reserve_vec_to_capacity(&mut zeros, byte_len).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve a {byte_len}-byte zero-init staging buffer for a resident grid-sync output: {error}. Shard the program into smaller buffers."
            ),
        }
    })?;
    zeros.resize(byte_len, 0);
    Ok(zeros)
}

/// Run the fixpoint sequence resident: every segment dispatched against the
/// shared resident resource slice, the whole sequence repeated to the program's
/// fixpoint bound, then the final outputs read back by name into `outputs`.
fn run_resident_grid_sync_fixpoint(
    backend: &dyn VyreBackend,
    program: &Program,
    segments: &[Program],
    resident: &ResidentProgramResources,
    config: &DispatchConfig,
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    let iterations =
        crate::fixpoint_iterations::resolve_fixpoint_iterations(config, "resident grid-sync split")?;
    let repeat_count = u32::try_from(iterations).map_err(|error| BackendError::InvalidProgram {
        fix: format!(
            "Fix: resident grid-sync fixpoint iteration count {iterations} does not fit u32: {error}."
        ),
    })?;

    // Every split segment shares the full program buffer layout, so the same
    // resident resource slice binds positionally to each one.
    let mut steps = Vec::new();
    reserve_grid_sync_vec(&mut steps, segments.len(), "resident grid-sync steps")?;
    for segment in segments {
        steps.push(ResidentDispatchStep {
            program: segment,
            resources: resident.ordered.as_slice(),
            grid_override: config.grid_override,
            // Carry the workgroup too: `grid_override` is sized for this
            // workgroup, so dropping it would launch a grid that under-covers
            // the work and silently drops findings.
            workgroup_override: config.workgroup_override,
        });
    }

    // Read back each original output buffer (declaration order) so the output
    // shape is byte-identical to the host-split path.
    let output_names = original_output_names(program)?;
    let mut read_ranges = Vec::new();
    reserve_grid_sync_vec(&mut read_ranges, output_names.len(), "resident grid-sync read ranges")?;
    for name in &output_names {
        let (resource, byte_len) =
            resident.by_name.get(name).ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident grid-sync final output `{name}` has no resident resource; it was not declared as a non-shared program buffer."
                ),
            })?;
        read_ranges.push(ResidentReadRange {
            resource,
            byte_offset: 0,
            byte_len: *byte_len,
        });
    }

    // Size `outputs` to one slot per output buffer, reusing existing
    // allocations, then hand the readback mutable references in order.
    while outputs.len() < output_names.len() {
        outputs.push(Vec::new());
    }
    outputs.truncate(output_names.len());
    for slot in outputs.iter_mut() {
        slot.clear();
    }
    let mut output_refs: Vec<&mut Vec<u8>> = outputs.iter_mut().collect();

    backend.dispatch_resident_repeated_sequence_read_ranges_into(
        &[],
        &steps,
        repeat_count,
        &read_ranges,
        output_refs.as_mut_slice(),
    )
}

/// Free every resident resource allocated for a
/// [`dispatch_resident_grid_sync_fixpoint_into`] call. Attempts every free even
/// if one fails, returning the first error so a leak is surfaced loudly.
fn free_resident_program_resources(
    backend: &dyn VyreBackend,
    resident: ResidentProgramResources,
) -> Result<(), BackendError> {
    let ResidentProgramResources { ordered, by_name } = resident;
    // `by_name` holds handle clones of the same resources in `ordered`; drop
    // it first so each underlying handle is freed exactly once via `ordered`.
    drop(by_name);
    let mut first_error: Option<BackendError> = None;
    for resource in ordered {
        if let Err(error) = backend.free_resident(resource) {
            if first_error.is_none() {
                first_error = Some(error);
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn reserve_grid_sync_vec<T>(
    vec: &mut Vec<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError> {
    crate::allocation::try_reserve_vec_to_capacity(vec, capacity).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {capacity} entries during grid-sync dispatch splitting: {error}. Split the program into fewer grid-sync segments or run on a backend with native grid sync."
            ),
        }
    })
}

fn reserve_grid_sync_hash_map<K, V>(
    map: &mut HashMap<K, V>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError>
where
    K: Eq + std::hash::Hash,
{
    map.try_reserve(capacity)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {capacity} entries during grid-sync dispatch splitting: {error}. Split the program into fewer grid-sync segments or run on a backend with native grid sync."
            ),
        })
}

fn reserve_grid_sync_hash_set<T>(
    set: &mut HashSet<T>,
    capacity: usize,
    field: &'static str,
) -> Result<(), BackendError>
where
    T: Eq + std::hash::Hash,
{
    set.try_reserve(capacity)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve {field} for {capacity} entries during grid-sync dispatch splitting: {error}. Split the program into fewer grid-sync segments or run on a backend with native grid sync."
            ),
        })
}

fn borrowed_grid_sync_inputs<'a>(
    inputs: &'a [GridSyncInput<'a>],
) -> Result<SmallVec<[&'a [u8]; 8]>, BackendError> {
    let mut borrowed = SmallVec::<[&[u8]; 8]>::new();
    borrowed.try_reserve(inputs.len()).map_err(|error| {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve grid-sync borrowed input slices for {} input(s): {error}. Split the program into fewer grid-sync live buffers or run on a backend with native grid sync.",
                inputs.len()
            ),
        }
    })?;
    borrowed.extend(inputs.iter().map(GridSyncInput::as_slice));
    Ok(borrowed)
}

fn borrowed_grid_sync_inputs_by_name<'a>(
    segment: &PlannedGridSyncSegment,
    inputs: &'a HashMap<Ident, GridSyncInput<'a>>,
) -> Result<SmallVec<[&'a [u8]; 8]>, BackendError> {
    let mut borrowed = SmallVec::<[&[u8]; 8]>::new();
    borrowed
        .try_reserve(segment.input_names.len())
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: failed to reserve grid-sync borrowed input slices for {} segment input(s): {error}. Split the program into fewer grid-sync live buffers or run on a backend with native grid sync.",
                segment.input_names.len()
            ),
        })?;
    for name in &segment.input_names {
        let input = inputs.get(name).ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: grid-sync segment input `{name}` has no bytes from caller input or a prior segment output. Ensure every cross-segment read is written before the GridSync barrier."
            ),
        })?;
        borrowed.push(input.as_slice());
    }
    Ok(borrowed)
}

/// Order-independent fingerprint of the EVOLVING accumulator state threaded
/// between grid-sync segments.
///
/// Only `Owned` entries are hashed: a `Borrowed` entry is a caller input that
/// is never written by any segment (constant for the whole split), so it cannot
/// change between passes and excluding it keeps the fingerprint cheap. Each
/// owned buffer mixes its NAME and its bytes (FNV-1a) so a value moving between
/// buffers is observed, and the per-buffer hashes are XOR-combined so map
/// iteration order does not affect the result. Two consecutive passes with an
/// identical fingerprint prove the deterministic segment sequence reached a
/// fixpoint (used to early-exit the outer iteration loop).
fn owned_accumulator_fingerprint(inputs: &HashMap<Ident, GridSyncInput<'_>>) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut combined: u64 = 0;
    for (name, input) in inputs {
        let GridSyncInput::Owned(bytes) = input else {
            continue;
        };
        let mut hash = FNV_OFFSET;
        for byte in name.as_str().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        // Separator so `name`+`bytes` cannot alias a different split.
        hash ^= 0xff;
        hash = hash.wrapping_mul(FNV_PRIME);
        for byte in bytes.iter() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        combined ^= hash;
    }
    combined
}

fn grid_sync_segment_error(
    error: BackendError,
    segment_idx: usize,
    segment_count: usize,
) -> BackendError {
    match error {
        BackendError::InvalidProgram { fix } => BackendError::InvalidProgram {
            fix: format!(
                "Fix: grid-sync split segment {segment_idx} of {segment_count} dispatch failed: {fix}"
            ),
        },
        other => other,
    }
}

enum GridSyncInput<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

impl GridSyncInput<'_> {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(bytes) => bytes.as_slice(),
        }
    }

    fn refresh_from_output(&mut self, bytes: &mut Vec<u8>) -> Result<(), BackendError> {
        match self {
            Self::Borrowed(_) => {
                let mut owned = Vec::new();
                reserve_grid_sync_vec(&mut owned, bytes.len(), "grid-sync readwrite input")?;
                owned.extend_from_slice(bytes);
                *self = Self::Owned(owned);
            }
            Self::Owned(owned) => {
                std::mem::swap(owned, bytes);
            }
        }
        Ok(())
    }
}

fn refresh_named_outputs<'a>(
    segment: &PlannedGridSyncSegment,
    outputs: &mut Vec<Vec<u8>>,
    inputs: &mut HashMap<Ident, GridSyncInput<'a>>,
) -> Result<(), BackendError> {
    if outputs.len() != segment.output_names.len() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: grid-sync split segment produced {} output slot(s) but the planned buffer map expected {}. Preserve segment output declaration order when dispatching split kernels.",
                outputs.len(),
                segment.output_names.len()
            ),
        });
    }
    for (name, bytes) in segment.output_names.iter().cloned().zip(outputs.iter_mut()) {
        match inputs.get_mut(&name) {
            Some(slot) => slot.refresh_from_output(bytes)?,
            None => {
                let mut owned = GridSyncInput::Owned(Vec::new());
                owned.refresh_from_output(bytes)?;
                inputs.insert(name, owned);
            }
        }
    }
    for output in outputs {
        output.clear();
    }
    Ok(())
}

fn collect_final_named_outputs<'a>(
    final_output_names: &[Ident],
    inputs: &mut HashMap<Ident, GridSyncInput<'a>>,
    outputs: &mut OutputBuffers,
) -> Result<(), BackendError> {
    let mut final_outputs = Vec::new();
    reserve_grid_sync_vec(
        &mut final_outputs,
        final_output_names.len(),
        "grid-sync final named outputs",
    )?;
    for name in final_output_names {
        let output = inputs
            .remove(name)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: grid-sync final output `{name}` was not produced by any split segment."
                ),
            })?;
        match output {
            GridSyncInput::Owned(bytes) => final_outputs.push(bytes),
            GridSyncInput::Borrowed(bytes) => {
                let mut owned = Vec::new();
                reserve_grid_sync_vec(&mut owned, bytes.len(), "grid-sync borrowed final output")?;
                owned.extend_from_slice(bytes);
                final_outputs.push(owned);
            }
        }
    }
    crate::replace_output_buffers_preserving_slots(final_outputs, outputs);
    Ok(())
}

/// After each segment dispatch, overwrite every ReadWrite buffer's
/// slot in `inputs` with the freshly-read bytes from `outputs`. The
/// backend returns one Vec<u8> per ReadWrite buffer in declaration
/// order; this function locates each ReadWrite buffer's input-slot
/// index and overwrites it. ReadOnly buffers stay untouched between
/// segments.
fn refresh_readwrite_inputs(
    segment: &Program,
    outputs: &mut Vec<Vec<u8>>,
    inputs: &mut [GridSyncInput<'_>],
) -> Result<(), BackendError> {
    use vyre_foundation::ir::BufferAccess;
    // Walk the segment's buffer table twice in lockstep  -  once for the
    // input slice, once for the output readback. Both paths must
    // mirror the convention `dispatch_borrowed` uses: input position
    // skips Workgroup AND `is_output` buffers; output position emits
    // one slot per ReadWrite buffer (whether or not is_output).
    let mut input_idx = 0usize;
    let mut output_idx = 0usize;
    for buffer in segment.buffers() {
        if matches!(buffer.access(), BufferAccess::Workgroup) {
            continue;
        }
        let is_output_buffer = buffer.is_output();
        let is_readwrite = matches!(buffer.access(), BufferAccess::ReadWrite);

        // Refresh the input slot from the readback if this buffer
        // appears in BOTH input and output positions (i.e. ReadWrite
        // and NOT is_output  -  the rule scratch / `gets` case).
        if is_readwrite && !is_output_buffer {
            if let (Some(slot), Some(bytes)) =
                (inputs.get_mut(input_idx), outputs.get_mut(output_idx))
            {
                slot.refresh_from_output(bytes)?;
            }
        }

        // Advance the input cursor for every non-output buffer.
        if !is_output_buffer {
            input_idx += 1;
        }
        // Advance the output cursor for every ReadWrite buffer (output
        // or not  -  the backend includes them all in the readback).
        if is_readwrite {
            output_idx += 1;
        }
    }
    for output in outputs {
        output.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr};

    fn buffer() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn region(generator: &str, body: Vec<Node>) -> Node {
        Node::Region {
            generator: Ident::from(generator),
            source_region: None,
            body: Arc::new(body),
        }
    }

    #[test]
    fn grid_sync_release_paths_use_fallible_split_storage() {
        let source = include_str!("grid_sync.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: grid-sync production source must precede tests");

        assert!(
            production.contains("pub fn try_split_on_grid_sync")
                && production.contains("fn reserve_grid_sync_vec")
                && production.contains("try_reserve_vec_to_capacity"),
            "Fix: grid-sync splitting must expose fallible segment/input/output scratch reservation."
        );
        assert!(
            production.contains("let segments = try_split_on_grid_sync(program)?")
                && !production.contains("let segments = split_on_grid_sync(program);"),
            "Fix: production grid-sync dispatch paths must use fallible splitting, not the legacy infallible helper."
        );
        assert!(
            !production.contains("Vec::with_capacity"),
            "Fix: production grid-sync splitting must not allocate dispatch scratch infallibly."
        );
        assert!(
            !production.contains(".as_nanos() as u64")
                && !production.contains("segmented timing overflowed u64"),
            "Fix: production grid-sync timing telemetry must return typed errors instead of truncating or panicking."
        );
    }

    /// Get the inner-segment node count for a wrapped or unwrapped Program.
    fn inner_len(program: &Program) -> usize {
        entry_sequence(program).len()
    }

    #[test]
    fn no_grid_sync_returns_single_segment() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![region(
                "a",
                vec![Node::store("buf", Expr::u32(0), Expr::u32(1))],
            )],
        );
        assert!(!contains_grid_sync(&program));
        let segments = split_on_grid_sync(&program);
        assert_eq!(segments.len(), 1);
        // Original entry was [Region("a", ...)] so the inner sequence is 1.
        assert_eq!(inner_len(&segments[0]), 1);
    }

    #[test]
    fn one_grid_sync_splits_into_two() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::store("buf", Expr::u32(0), Expr::u32(1))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("buf", Expr::u32(1), Expr::u32(2))]),
            ],
        );
        assert!(contains_grid_sync(&program));
        let segments = split_on_grid_sync(&program);
        assert_eq!(segments.len(), 2);
        assert_eq!(inner_len(&segments[0]), 1);
        assert_eq!(inner_len(&segments[1]), 1);
    }

    #[test]
    fn block_nested_grid_sync_splits_into_two() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![Node::Block(vec![
                region("a", vec![Node::store("buf", Expr::u32(0), Expr::u32(1))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("buf", Expr::u32(1), Expr::u32(2))]),
            ])],
        );
        assert!(contains_grid_sync(&program));
        let segments = split_on_grid_sync(&program);
        assert_eq!(segments.len(), 2);
        assert_eq!(inner_len(&segments[0]), 1);
        assert_eq!(inner_len(&segments[1]), 1);
    }

    #[test]
    fn three_grid_syncs_split_into_four() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("c", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("d", vec![Node::Return]),
            ],
        );
        let segments = split_on_grid_sync(&program);
        assert_eq!(segments.len(), 4);
    }

    #[test]
    fn workgroup_barrier_does_not_split() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::SeqCst),
                region("b", vec![Node::Return]),
            ],
        );
        assert!(!contains_grid_sync(&program));
        let segments = split_on_grid_sync(&program);
        assert_eq!(segments.len(), 1);
        // Region("a"), Barrier(SeqCst), Region("b") = 3 inner nodes.
        assert_eq!(inner_len(&segments[0]), 3);
    }

    #[test]
    fn buffers_and_workgroup_size_propagate_to_each_segment() {
        let program = Program::wrapped(
            vec![buffer()],
            [256, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );
        let segments = split_on_grid_sync(&program);
        for seg in &segments {
            assert_eq!(seg.workgroup_size(), [256, 1, 1]);
            assert_eq!(seg.buffers().len(), 1);
            assert_eq!(seg.buffers()[0].name(), "buf");
        }
    }

    #[test]
    fn refresh_readwrite_inputs_swaps_owned_buffers_after_first_segment() {
        let segment = Program::wrapped(vec![buffer()], [1, 1, 1], vec![Node::Return]);
        let initial = [1u8, 0, 0, 0];
        let mut inputs = [GridSyncInput::Borrowed(initial.as_slice())];
        let mut outputs = vec![Vec::with_capacity(8)];
        let output_ptr = outputs[0].as_ptr() as usize;
        outputs[0].extend_from_slice(&[2, 0, 0, 0]);

        refresh_readwrite_inputs(&segment, &mut outputs, &mut inputs)
            .expect("Fix: test readwrite refresh should fit borrowed promotion storage");

        let first_owned_ptr = match &inputs[0] {
            GridSyncInput::Owned(bytes) => {
                assert_eq!(bytes, &[2, 0, 0, 0]);
                bytes.as_ptr() as usize
            }
            GridSyncInput::Borrowed(_) => panic!("ReadWrite input must become owned after refresh"),
        };
        assert_eq!(outputs[0].as_ptr() as usize, output_ptr);
        assert!(outputs[0].is_empty());

        outputs[0].extend_from_slice(&[3, 0, 0, 0]);
        let second_output_ptr = outputs[0].as_ptr() as usize;
        refresh_readwrite_inputs(&segment, &mut outputs, &mut inputs)
            .expect("Fix: test readwrite refresh should reuse owned storage");

        match &inputs[0] {
            GridSyncInput::Owned(bytes) => {
                assert_eq!(bytes, &[3, 0, 0, 0]);
                assert_eq!(
                    bytes.as_ptr() as usize,
                    second_output_ptr,
                    "owned ReadWrite input should take the backend output allocation instead of copying"
                );
            }
            GridSyncInput::Borrowed(_) => panic!("ReadWrite input must remain owned"),
        }
        assert_eq!(
            outputs[0].as_ptr() as usize,
            first_owned_ptr,
            "backend output slot should receive the previous owned input allocation for reuse"
        );
    }

    struct ReuseCheckingBackend {
        calls: AtomicUsize,
        final_outputs_addr: usize,
        final_slot_addr: usize,
    }

    impl crate::backend::private::Sealed for ReuseCheckingBackend {}

    impl VyreBackend for ReuseCheckingBackend {
        fn id(&self) -> &'static str {
            "grid-sync-reuse-checking"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 1 && self.final_outputs_addr != 0 {
                assert_eq!(outputs.as_ptr() as usize, self.final_outputs_addr);
                assert_eq!(outputs[0].as_ptr() as usize, self.final_slot_addr);
            }
            if outputs.is_empty() {
                outputs.push(Vec::new());
            }
            outputs[0].clear();
            outputs[0].extend_from_slice(inputs[0]);
            if call == 0 {
                outputs[0][0] = 7;
            } else {
                outputs[0][0] = outputs[0][0].saturating_add(1);
            }
            Ok(())
        }
    }

    #[test]
    fn split_into_preserves_caller_output_slot_after_named_output_collection() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );
        let mut outputs = vec![Vec::with_capacity(8)];
        let outputs_addr = outputs.as_ptr() as usize;
        let slot_addr = outputs[0].as_ptr() as usize;
        let backend = ReuseCheckingBackend {
            calls: AtomicUsize::new(0),
            final_outputs_addr: 0,
            final_slot_addr: 0,
        };
        let input = [0u8, 0, 0, 0];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[input.as_slice()],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("Fix: grid-sync split should write into caller-owned output storage");

        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
        assert_eq!(outputs, vec![vec![8, 0, 0, 0]]);
        assert_eq!(outputs.as_ptr() as usize, outputs_addr);
        assert_eq!(outputs[0].as_ptr() as usize, slot_addr);
    }

    /// Each `dispatch_borrowed_into` reads `inputs[0][0]`, writes `+1`. With the
    /// ReadWrite buffer rotating between segments, a single pass over a
    /// two-segment program advances the accumulator by 2. The multi-hop
    /// `flows_to` closure relies on the WHOLE sequence being re-run
    /// `fixpoint_iterations` times (one dataflow hop per pass); a single pass
    /// is one hop, which silently dropped every flow through an intermediate
    /// variable to recall=0.
    struct IncrementingBackend {
        calls: AtomicUsize,
    }

    impl crate::backend::private::Sealed for IncrementingBackend {}

    impl VyreBackend for IncrementingBackend {
        fn id(&self) -> &'static str {
            "grid-sync-incrementing"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Each segment must run exactly once per outer pass: the whole
            // sequence carries the fixpoint, not any single segment.
            assert_eq!(
                config.fixpoint_iterations,
                Some(1),
                "segment dispatch must receive fixpoint_iterations=1; the outer split loop owns the iteration count"
            );
            if outputs.is_empty() {
                outputs.push(Vec::new());
            }
            outputs[0].clear();
            outputs[0].extend_from_slice(inputs[0]);
            outputs[0][0] = outputs[0][0].saturating_add(1);
            Ok(())
        }
    }

    #[test]
    fn split_into_loops_whole_sequence_fixpoint_iterations_times() {
        // Two segments separated by a GridSync barrier.
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );

        // Single pass (default): 2 segment launches, accumulator = 2.
        let backend = IncrementingBackend {
            calls: AtomicUsize::new(0),
        };
        let mut outputs = vec![Vec::new()];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[[0u8, 0, 0, 0].as_slice()],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("single-pass split dispatch");
        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
        assert_eq!(outputs, vec![vec![2, 0, 0, 0]]);

        // Three fixpoint iterations: 3 passes × 2 segments = 6 launches, and
        // the accumulator advances one hop per pass to 6. This is the exact
        // property the multi-hop `flows_to` split depended on and the
        // single-pass implementation lacked.
        let backend = IncrementingBackend {
            calls: AtomicUsize::new(0),
        };
        let config = DispatchConfig {
            fixpoint_iterations: Some(3),
            ..DispatchConfig::default()
        };
        let mut outputs = vec![Vec::new()];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[[0u8, 0, 0, 0].as_slice()],
            &config,
            &mut outputs,
        )
        .expect("multi-pass split dispatch");
        assert_eq!(
            backend.calls.load(Ordering::SeqCst),
            6,
            "split must re-run the whole 2-segment sequence 3 times"
        );
        assert_eq!(
            outputs,
            vec![vec![6, 0, 0, 0]],
            "accumulator must advance one hop per fixpoint pass (2 segments × 3 passes)"
        );
    }

    struct OwnedFinalReserveBackend {
        calls: AtomicUsize,
    }

    impl crate::backend::private::Sealed for OwnedFinalReserveBackend {}

    impl VyreBackend for OwnedFinalReserveBackend {
        fn id(&self) -> &'static str {
            "grid-sync-owned-final-reserve"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 1 {
                assert!(
                    outputs.capacity() >= 1,
                    "owned grid-sync split wrapper must pre-reserve final output slots before the final segment dispatch"
                );
            }
            if outputs.is_empty() {
                outputs.push(Vec::new());
            }
            outputs[0].clear();
            outputs[0].extend_from_slice(inputs[0]);
            outputs[0][0] = outputs[0][0].saturating_add(1);
            Ok(())
        }
    }

    #[test]
    fn split_owned_wrapper_reserves_final_output_vector_before_final_segment() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );
        let backend = OwnedFinalReserveBackend {
            calls: AtomicUsize::new(0),
        };
        let input = [4u8, 0, 0, 0];

        let outputs = dispatch_with_grid_sync_split(
            &backend,
            &program,
            &[input.as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: owned grid-sync split should reserve and return final outputs");

        assert_eq!(backend.calls.load(Ordering::SeqCst), 2);
        assert_eq!(outputs, vec![vec![6, 0, 0, 0]]);
    }

    #[test]
    fn grid_sync_split_records_segment_telemetry() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("c", vec![Node::Return]),
            ],
        );
        let backend = ReuseCheckingBackend {
            calls: AtomicUsize::new(0),
            final_outputs_addr: 0,
            final_slot_addr: 0,
        };
        let before = crate::observability::snapshot_dispatch_telemetry();
        let input = [0u8, 0, 0, 0];
        let mut outputs = Vec::new();

        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[input.as_slice()],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("Fix: grid-sync split should dispatch every segment");

        let after = crate::observability::snapshot_dispatch_telemetry();
        assert_eq!(backend.calls.load(Ordering::SeqCst), 3);
        assert!(after.grid_sync_splits >= before.grid_sync_splits + 1);
        assert!(after.grid_sync_segments >= before.grid_sync_segments + 3);
        assert!(after.grid_sync_points >= before.grid_sync_points + 2);
    }

    struct IntermediateReuseBackend {
        calls: AtomicUsize,
        first_outputs_addr: AtomicUsize,
        first_slot_addr: AtomicUsize,
    }

    impl crate::backend::private::Sealed for IntermediateReuseBackend {}

    impl VyreBackend for IntermediateReuseBackend {
        fn id(&self) -> &'static str {
            "grid-sync-intermediate-reuse"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if outputs.is_empty() {
                outputs.push(Vec::with_capacity(8));
            }
            if call == 0 {
                self.first_outputs_addr
                    .store(outputs.as_ptr() as usize, Ordering::SeqCst);
                self.first_slot_addr
                    .store(outputs[0].as_ptr() as usize, Ordering::SeqCst);
            } else if call == 1 {
                assert_eq!(
                    outputs.as_ptr() as usize,
                    self.first_outputs_addr.load(Ordering::SeqCst)
                );
                assert_eq!(
                    outputs[0].as_ptr() as usize,
                    self.first_slot_addr.load(Ordering::SeqCst)
                );
            }
            outputs[0].clear();
            outputs[0].extend_from_slice(inputs[0]);
            outputs[0][0] = outputs[0][0].saturating_add(1);
            Ok(())
        }
    }

    #[test]
    fn split_reuses_intermediate_output_slot_between_segments() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("c", vec![Node::Return]),
            ],
        );
        let backend = IntermediateReuseBackend {
            calls: AtomicUsize::new(0),
            first_outputs_addr: AtomicUsize::new(0),
            first_slot_addr: AtomicUsize::new(0),
        };
        let input = [1u8, 0, 0, 0];
        let mut outputs = vec![Vec::with_capacity(8)];

        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[input.as_slice()],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("Fix: grid-sync split should reuse intermediate output scratch");

        assert_eq!(backend.calls.load(Ordering::SeqCst), 3);
        assert_eq!(outputs, vec![vec![4, 0, 0, 0]]);
    }

    #[test]
    fn split_keeps_multi_segment_output_as_readwrite_accumulator() {
        // An OUTPUT buffer whose slots are written by DIFFERENT grid-sync
        // segments (the fused multi-rule `results_packed` shape: each rule's
        // result-store lands in its own segment) must ACCUMULATE across the host
        // split. The first writer establishes it (WriteOnly); every LATER writer
        // must read the forwarded value and merge its own slots (ReadWrite)
        // instead of overwriting it with a fresh write-only buffer — which would
        // silently zero the earlier segments' slots (recall=0 for every rule
        // whose store is not in the final segment).
        let out = BufferDecl::output("out", 0, DataType::U32).with_count(4);
        let program = Program::wrapped(
            vec![out],
            [1, 1, 1],
            vec![
                region("a", vec![Node::store("out", Expr::u32(0), Expr::u32(0xAA))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("out", Expr::u32(2), Expr::u32(0xBB))]),
            ],
        );
        let segments =
            plan_host_grid_sync_segment_programs(&program).expect("plan host grid-sync segments");
        assert_eq!(segments.len(), 2, "one GridSync barrier -> two segments");

        let seg0_out = segments[0]
            .buffers()
            .iter()
            .find(|b| b.name() == "out")
            .expect("segment 0 must declare the output it writes");
        assert_eq!(
            seg0_out.access(),
            BufferAccess::WriteOnly,
            "the first writer establishes the accumulator as write-only"
        );
        assert!(
            !seg0_out.is_output() && !seg0_out.is_pipeline_live_out(),
            "split segment buffers must never be marked program-output; final values are reassembled by name"
        );

        let seg1_out = segments[1]
            .buffers()
            .iter()
            .find(|b| b.name() == "out")
            .expect("segment 1 must declare the output it writes");
        assert_eq!(
            seg1_out.access(),
            BufferAccess::ReadWrite,
            "a later writer of a multi-segment output must read+merge the accumulated value, not overwrite it"
        );
        assert!(
            !seg1_out.is_output() && !seg1_out.is_pipeline_live_out(),
            "the later writer must consume its forwarded prior value, which `segment_buffer_consumes_input` refuses for is_output buffers"
        );
        assert!(
            segment_input_names(&segments[1])
                .expect("segment 1 input names")
                .iter()
                .any(|n| n.as_str() == "out"),
            "the accumulated output must be forwarded as an input to the later writing segment"
        );
    }

    /// Emulates a backend that lacks native grid-sync: for the single output
    /// buffer `out`, it starts from the forwarded prior value (when the segment
    /// consumes it) or zeros, then applies that segment's literal `Store out[i]
    /// = v` writes — exactly the per-slot store shape a fused multi-rule program
    /// produces. Proves end-to-end that earlier segments' slots survive.
    struct SlotStoringBackend {
        calls: AtomicUsize,
    }

    impl crate::backend::private::Sealed for SlotStoringBackend {}

    impl VyreBackend for SlotStoringBackend {
        fn id(&self) -> &'static str {
            "grid-sync-slot-storing"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            // Locate `out`'s positional input/output slots using the SAME
            // role convention the host split planner uses.
            let mut in_pos = None;
            let mut cur_in = 0usize;
            let mut out_pos = None;
            let mut cur_out = 0usize;
            for buffer in program.buffers() {
                if matches!(buffer.access(), BufferAccess::Workgroup) {
                    continue;
                }
                let consumes = segment_buffer_consumes_input(buffer);
                let produces = segment_buffer_produces_output(buffer);
                if buffer.name() == "out" {
                    if consumes {
                        in_pos = Some(cur_in);
                    }
                    if produces {
                        out_pos = Some(cur_out);
                    }
                }
                if consumes {
                    cur_in += 1;
                }
                if produces {
                    cur_out += 1;
                }
            }
            let out_pos = out_pos.expect("every writing segment must produce `out`");
            let mut state = match in_pos {
                Some(i) => inputs[i].to_vec(),
                None => vec![0u8; 16],
            };

            fn apply(nodes: &[Node], state: &mut [u8]) {
                for node in nodes {
                    match node {
                        Node::Store {
                            buffer,
                            index: Expr::LitU32(i),
                            value: Expr::LitU32(v),
                        } if buffer.as_str() == "out" => {
                            let off = (*i as usize) * 4;
                            state[off] = (*v & 0xff) as u8;
                        }
                        Node::Region { body, .. } => apply(body, state),
                        Node::Block(body) => apply(body, state),
                        Node::If {
                            then, otherwise, ..
                        } => {
                            apply(then, state);
                            apply(otherwise, state);
                        }
                        Node::Loop { body, .. } => apply(body, state),
                        _ => {}
                    }
                }
            }
            apply(entry_sequence(program), &mut state);

            self.calls.fetch_add(1, Ordering::SeqCst);
            while outputs.len() <= out_pos {
                outputs.push(Vec::new());
            }
            outputs[out_pos].clear();
            outputs[out_pos].extend_from_slice(&state);
            Ok(())
        }
    }

    #[test]
    fn split_preserves_earlier_segment_output_slots_end_to_end() {
        // Regression: a fused multi-arm program where arm A's result-store is in
        // segment 0 (slot at element 0) and arm B's in the final segment (slot
        // at element 2). Before the accumulator fix the final segment's
        // write-only `out` zeroed element 0, dropping arm A entirely (a co-fused
        // rule whose result-store does not land in the final grid-sync segment
        // returned recall=0). Both slots must now survive.
        let out = BufferDecl::output("out", 0, DataType::U32).with_count(4);
        let program = Program::wrapped(
            vec![out],
            [1, 1, 1],
            vec![
                region("a", vec![Node::store("out", Expr::u32(0), Expr::u32(0xAA))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("out", Expr::u32(2), Expr::u32(0xBB))]),
            ],
        );
        let backend = SlotStoringBackend {
            calls: AtomicUsize::new(0),
        };
        let mut outputs = vec![Vec::new()];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("split dispatch");
        assert_eq!(
            backend.calls.load(Ordering::SeqCst),
            2,
            "two segments, single fixpoint pass"
        );
        assert_eq!(outputs.len(), 1);
        assert_eq!(
            outputs[0].len(),
            16,
            "output buffer is 4 × u32 = 16 bytes"
        );
        assert_eq!(
            outputs[0][0], 0xAA,
            "segment 0's slot (element 0) must survive the final segment's write"
        );
        assert_eq!(
            outputs[0][8], 0xBB,
            "the final segment's slot (element 2) is also present"
        );
    }

    /// Copies its input to its output and bumps byte 0 toward a saturation cap.
    /// Once the cap is reached the output equals the input, so a full pass over
    /// the split leaves the carried accumulator unchanged — a fixpoint.
    struct SaturatingBackend {
        calls: AtomicUsize,
        cap: u8,
    }

    impl crate::backend::private::Sealed for SaturatingBackend {}

    impl VyreBackend for SaturatingBackend {
        fn id(&self) -> &'static str {
            "grid-sync-saturating"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_borrowed_into")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            inputs: &[&[u8]],
            _config: &DispatchConfig,
            outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if outputs.is_empty() {
                outputs.push(Vec::new());
            }
            outputs[0].clear();
            outputs[0].extend_from_slice(inputs[0]);
            if outputs[0][0] < self.cap {
                outputs[0][0] += 1;
            }
            Ok(())
        }
    }

    #[test]
    fn split_outer_loop_early_exits_when_accumulator_reaches_fixpoint() {
        // Two segments (one GridSync barrier). With a generous iteration budget
        // of 10, byte 0 saturates at 3, after which a whole pass leaves the
        // accumulator unchanged. The outer loop must stop once two consecutive
        // passes match instead of burning all 10 iterations.
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );
        let backend = SaturatingBackend {
            calls: AtomicUsize::new(0),
            cap: 3,
        };
        let config = DispatchConfig {
            fixpoint_iterations: Some(10),
            ..DispatchConfig::default()
        };
        let mut outputs = vec![Vec::new()];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[[0u8, 0, 0, 0].as_slice()],
            &config,
            &mut outputs,
        )
        .expect("converging split dispatch");
        // pass0 -> 2, pass1 -> 3 (saturates mid-pass), pass2 -> 3 (unchanged) =>
        // break after pass2. 3 passes x 2 segments = 6 launches, NOT 10x2=20.
        assert_eq!(
            backend.calls.load(Ordering::SeqCst),
            6,
            "outer loop must early-exit one pass after the accumulator stops changing, not run all 10 iterations"
        );
        assert_eq!(
            outputs,
            vec![vec![3, 0, 0, 0]],
            "early-exit must return the converged fixpoint value, identical to running every iteration"
        );
    }

    #[test]
    fn split_non_converging_accumulator_runs_full_iteration_budget() {
        // The dual of the early-exit test: an accumulator that changes every
        // pass (never reaches a fixpoint within budget) must run all
        // iterations — early-exit must not fire on a still-advancing closure.
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
            ],
        );
        // cap=255 so it never saturates within 4 passes (8 increments).
        let backend = SaturatingBackend {
            calls: AtomicUsize::new(0),
            cap: 255,
        };
        let config = DispatchConfig {
            fixpoint_iterations: Some(4),
            ..DispatchConfig::default()
        };
        let mut outputs = vec![Vec::new()];
        dispatch_with_grid_sync_split_into(
            &backend,
            &program,
            &[[0u8, 0, 0, 0].as_slice()],
            &config,
            &mut outputs,
        )
        .expect("non-converging split dispatch");
        assert_eq!(
            backend.calls.load(Ordering::SeqCst),
            8,
            "a still-advancing accumulator must run the full 4 iterations x 2 segments"
        );
        assert_eq!(outputs, vec![vec![8, 0, 0, 0]]);
    }

    struct ResidentReuseBackend {
        calls: AtomicUsize,
    }

    impl crate::backend::private::Sealed for ResidentReuseBackend {}

    impl VyreBackend for ResidentReuseBackend {
        fn id(&self) -> &'static str {
            "grid-sync-resident-reuse"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("test uses dispatch_resident_timed")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            _inputs: &[&[u8]],
            _config: &DispatchConfig,
            _outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            unreachable!("resident grid-sync split must not refresh through host borrowed inputs")
        }

        fn dispatch_resident_timed(
            &self,
            _program: &Program,
            resources: &[Resource],
            _config: &DispatchConfig,
        ) -> Result<TimedDispatchResult, BackendError> {
            assert!(
                matches!(resources, [Resource::Resident(11), Resource::Resident(22)]),
                "Fix: resident grid-sync split must keep the original device handles bound across every segment."
            );
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(TimedDispatchResult {
                outputs: vec![vec![call as u8]],
                wall_ns: 10,
                device_ns: Some(2),
                enqueue_ns: Some(3),
                wait_ns: Some(4),
            })
        }
    }

    #[test]
    fn resident_split_reuses_same_device_resources_across_segments() {
        let program = Program::wrapped(
            vec![buffer()],
            [1, 1, 1],
            vec![
                region("a", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::Return]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("c", vec![Node::Return]),
            ],
        );
        let backend = ResidentReuseBackend {
            calls: AtomicUsize::new(0),
        };

        let timed = dispatch_resident_with_grid_sync_split_timed(
            &backend,
            &program,
            &[Resource::Resident(11), Resource::Resident(22)],
            &DispatchConfig::default(),
        )
        .expect("Fix: resident grid-sync split should run each segment on the same device handles");

        assert_eq!(backend.calls.load(Ordering::SeqCst), 3);
        assert_eq!(timed.outputs, vec![vec![2]]);
        assert_eq!(timed.device_ns, Some(6));
        assert_eq!(timed.enqueue_ns, Some(9));
        assert_eq!(timed.wait_ns, Some(12));
    }

    /// In-memory device for the resident fixpoint path: holds one byte vector
    /// per resident handle, applies a segment's `out` stores IN PLACE to the
    /// bound device buffer (no clear between launches), and reads ranges back.
    /// `allocate_resident` fills fresh buffers with 0xFF so a test can prove the
    /// zero-init upload actually ran.
    struct ResidentDeviceBackend {
        next_id: std::sync::atomic::AtomicU64,
        buffers: std::sync::Mutex<HashMap<u64, Vec<u8>>>,
        freed: std::sync::Mutex<Vec<u64>>,
        dispatches: AtomicUsize,
    }

    impl ResidentDeviceBackend {
        fn new() -> Self {
            Self {
                next_id: std::sync::atomic::AtomicU64::new(1),
                buffers: std::sync::Mutex::new(HashMap::new()),
                freed: std::sync::Mutex::new(Vec::new()),
                dispatches: AtomicUsize::new(0),
            }
        }

        fn resident_id(resource: &Resource) -> u64 {
            match resource {
                Resource::Resident(id) => *id,
                Resource::Borrowed(_) => {
                    panic!("Fix: resident grid-sync fixpoint must bind Resident handles, not Borrowed")
                }
            }
        }
    }

    impl crate::backend::private::Sealed for ResidentDeviceBackend {}

    impl VyreBackend for ResidentDeviceBackend {
        fn id(&self) -> &'static str {
            "grid-sync-resident-device"
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<Vec<Vec<u8>>, BackendError> {
            unreachable!("resident fixpoint test uses resident dispatch")
        }

        fn dispatch_borrowed_into(
            &self,
            _program: &Program,
            _inputs: &[&[u8]],
            _config: &DispatchConfig,
            _outputs: &mut OutputBuffers,
        ) -> Result<(), BackendError> {
            unreachable!("resident fixpoint must thread device handles, never host borrowed inputs")
        }

        fn allocate_resident(&self, byte_len: usize) -> Result<Resource, BackendError> {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);
            // Fresh device memory is garbage (0xFF here) so the zero-init upload
            // path is actually exercised by the test assertions.
            self.buffers.lock().unwrap().insert(id, vec![0xFFu8; byte_len]);
            Ok(Resource::Resident(id))
        }

        fn upload_resident(&self, resource: &Resource, bytes: &[u8]) -> Result<(), BackendError> {
            let id = Self::resident_id(resource);
            let mut buffers = self.buffers.lock().unwrap();
            let buf = buffers.get_mut(&id).expect("resident handle exists");
            assert!(
                bytes.len() <= buf.len(),
                "upload {} bytes into a {}-byte resident buffer",
                bytes.len(),
                buf.len()
            );
            buf[..bytes.len()].copy_from_slice(bytes);
            Ok(())
        }

        fn download_resident_range_into(
            &self,
            resource: &Resource,
            byte_offset: usize,
            byte_len: usize,
            output: &mut Vec<u8>,
        ) -> Result<(), BackendError> {
            let id = Self::resident_id(resource);
            let buffers = self.buffers.lock().unwrap();
            let buf = buffers.get(&id).expect("resident handle exists");
            output.clear();
            output.extend_from_slice(&buf[byte_offset..byte_offset + byte_len]);
            Ok(())
        }

        fn free_resident(&self, resource: Resource) -> Result<(), BackendError> {
            let id = Self::resident_id(&resource);
            self.buffers.lock().unwrap().remove(&id);
            self.freed.lock().unwrap().push(id);
            Ok(())
        }

        fn dispatch_resident_timed(
            &self,
            program: &Program,
            resources: &[Resource],
            _config: &DispatchConfig,
        ) -> Result<TimedDispatchResult, BackendError> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            // Find `out`'s index among the non-shared bindings  -  the same
            // order `allocate_resident_program_resources` builds `resources` in.
            let plan = BindingPlan::build(program)?;
            let mut out_slot = None;
            let mut pos = 0usize;
            for binding in &plan.bindings {
                if binding.role == BindingRole::Shared {
                    continue;
                }
                if binding.name.as_ref() == "out" {
                    out_slot = Some(pos);
                }
                pos += 1;
            }
            let out_slot = out_slot.expect("program declares `out`");
            let id = Self::resident_id(&resources[out_slot]);
            let mut buffers = self.buffers.lock().unwrap();
            let buf = buffers.get_mut(&id).expect("resident `out` handle exists");

            // Apply the segment's `out` stores IN PLACE  -  never clearing the
            // buffer, so earlier segments' slots persist (the accumulator).
            fn apply(nodes: &[Node], state: &mut [u8]) {
                for node in nodes {
                    match node {
                        Node::Store {
                            buffer,
                            index: Expr::LitU32(i),
                            value: Expr::LitU32(v),
                        } if buffer.as_str() == "out" => {
                            state[(*i as usize) * 4] = (*v & 0xff) as u8;
                        }
                        Node::Region { body, .. } => apply(body, state),
                        Node::Block(body) => apply(body, state),
                        Node::If { then, otherwise, .. } => {
                            apply(then, state);
                            apply(otherwise, state);
                        }
                        Node::Loop { body, .. } => apply(body, state),
                        _ => {}
                    }
                }
            }
            apply(entry_sequence(program), buf.as_mut_slice());

            Ok(TimedDispatchResult {
                outputs: Vec::new(),
                wall_ns: 1,
                device_ns: Some(1),
                enqueue_ns: Some(1),
                wait_ns: Some(1),
            })
        }
    }

    #[test]
    fn resident_fixpoint_accumulates_across_segments_zero_inits_and_frees() {
        // Same cross-anchor shape as the host-path regression: arm A stores slot
        // 0 in segment 0, arm B stores slot 2 in the final segment. The resident
        // path keeps ONE device `out` buffer bound across both segments, so both
        // slots must survive WITHOUT the host-path accumulator role-rewrite  -
        // the persistent device buffer is never cleared between launches.
        let out = BufferDecl::output("out", 0, DataType::U32).with_count(4);
        let program = Program::wrapped(
            vec![out],
            [1, 1, 1],
            vec![
                region("a", vec![Node::store("out", Expr::u32(0), Expr::u32(0xAA))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("out", Expr::u32(2), Expr::u32(0xBB))]),
            ],
        );
        let backend = ResidentDeviceBackend::new();
        let mut outputs = vec![Vec::new()];
        dispatch_resident_grid_sync_fixpoint_into(
            &backend,
            &program,
            &[],
            &DispatchConfig::default(),
            &mut outputs,
        )
        .expect("resident grid-sync fixpoint dispatch");

        assert_eq!(
            backend.dispatches.load(Ordering::SeqCst),
            2,
            "two segments, single fixpoint pass under the default config"
        );
        assert_eq!(outputs.len(), 1, "one output buffer (`out`)");
        assert_eq!(outputs[0].len(), 16, "4 × u32 = 16 bytes");
        assert_eq!(
            outputs[0][0], 0xAA,
            "segment 0's slot survives  -  resident accumulation, no clobber"
        );
        assert_eq!(outputs[0][8], 0xBB, "the final segment's slot is present");
        // Zero-init proof: every byte the kernel did not write is 0, not the
        // 0xFF garbage `allocate_resident` seeded  -  the output buffer was
        // zeroed before dispatch.
        assert_eq!(outputs[0][4], 0x00, "untouched slot 1 was zero-initialized");
        assert_eq!(outputs[0][12], 0x00, "untouched slot 3 was zero-initialized");
        // Every resident resource is freed exactly once.
        assert_eq!(
            backend.freed.lock().unwrap().len(),
            1,
            "the single `out` resident buffer is freed"
        );
        assert!(
            backend.buffers.lock().unwrap().is_empty(),
            "no resident buffer leaks after dispatch"
        );
    }

    #[test]
    fn resident_fixpoint_repeats_to_fixpoint_bound() {
        // With a fixpoint bound > 1, the whole segment sequence repeats that many
        // times against the same resident buffers (idempotent stores here, so the
        // result is unchanged, but the launch count proves the repeat wiring).
        let out = BufferDecl::output("out", 0, DataType::U32).with_count(4);
        let program = Program::wrapped(
            vec![out],
            [1, 1, 1],
            vec![
                region("a", vec![Node::store("out", Expr::u32(0), Expr::u32(0xAA))]),
                Node::barrier_with_ordering(MemoryOrdering::GridSync),
                region("b", vec![Node::store("out", Expr::u32(2), Expr::u32(0xBB))]),
            ],
        );
        let backend = ResidentDeviceBackend::new();
        let mut config = DispatchConfig::default();
        config.fixpoint_iterations = Some(3);
        let mut outputs = vec![Vec::new()];
        dispatch_resident_grid_sync_fixpoint_into(
            &backend,
            &program,
            &[],
            &config,
            &mut outputs,
        )
        .expect("resident grid-sync fixpoint dispatch");
        assert_eq!(
            backend.dispatches.load(Ordering::SeqCst),
            6,
            "2 segments × 3 fixpoint passes"
        );
        assert_eq!(outputs[0][0], 0xAA);
        assert_eq!(outputs[0][8], 0xBB);
    }
}
