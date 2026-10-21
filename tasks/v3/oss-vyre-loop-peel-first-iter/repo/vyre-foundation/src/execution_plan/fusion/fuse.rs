//! Core `fuse_programs` family + multi-program implementation.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::execution_plan::SchedulingPolicy;
use crate::ir::{BufferAccess, BufferDecl, Ident, Node, Program};

use super::alpha_rename::{
    multiply_declared_names, push_alpha_renamed_arm_entry_node, ArmRenamer,
};
use super::collectors::collect_buffer_targets;
use super::divergence::{
    has_divergent_invocation_gated_store, has_launch_geometry_dependent_write,
};
use super::helpers::{fallback_composition_key, upgrade_buffer_access};
use super::{FusionError, FusionOverDispatchError, FusionSelfAliasingError};

/// Combine `programs` into one fused [`Program`]. Returns the input verbatim
/// for 0 or 1 program; multi-program runs go through the full hazard tracker.
///
/// # Errors
///
/// Returns [`FusionError`] when the batch contains conflicting buffer aliases,
/// non-composable self-fusion, or over-dispatches the shared launch geometry.
pub fn fuse_programs(programs: &[Program]) -> Result<Program, FusionError> {
    match programs.len() {
        0 => Ok(Program::empty()),
        1 => Ok(programs[0].clone()),
        _ => fuse_programs_multi(programs),
    }
}

/// Fuse `programs` when the caller already owns a `Vec`.
///
/// For a single program this returns that value directly (no deep clone).
/// Multi-arm batches delegate to the same implementation as [`fuse_programs`].
///
/// # Errors
///
/// Returns [`FusionError`] under the same conditions as [`fuse_programs`].
#[inline]
#[must_use]
pub fn fuse_programs_vec(mut programs: Vec<Program>) -> Result<Program, FusionError> {
    match programs.len() {
        0 => Ok(Program::empty()),
        1 => {
            let Some(program) = programs.pop() else {
                return Ok(Program::empty());
            };
            Ok(program)
        }
        _ => fuse_programs_multi(programs.as_slice()),
    }
}

/// How a fused arm's local names and scope relate to the other arms.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArmNamespace {
    /// Arms are **independent** programs (inter-rule batch fusion, the
    /// megakernel builder). Each arm allocates temps from its own counter,
    /// so two arms can reuse the same temp name for different values. The
    /// fuser alpha-renames every arm-local name with the arm index and wraps
    /// each arm body in its own `Block` scope, so the reused names cannot
    /// collide in the combined program.
    Isolated,
    /// Arms are **sub-programs of one rule** that share a single global temp
    /// namespace (one monotonic `temp_counter` per `LowerCtx`; recursion is
    /// handled by the fixpoint operator, not by re-instantiating bodies, so
    /// no name is ever reused for two values). Renaming such names is not
    /// only unnecessary, it is actively wrong: a value produced in one arm
    /// (`let __cmp_N = load(__quant_flag_…)`) and consumed in another arm
    /// (`Var(__cmp_N)`) must keep ONE consistent name and live in ONE shared
    /// scope, or the consumer references an undeclared variable. Shared arms
    /// are therefore spliced flat — no per-arm rename, no per-arm `Block` —
    /// preserving decl→use linkage across the merge boundary.
    Shared,
}

/// Merge sub-programs that share one rule's global temp namespace into a
/// single program, preserving decl→use linkage across the merge boundary.
///
/// Same hazard analysis, buffer union, binding renumbering, and barrier
/// insertion as [`fuse_programs`]; the only difference is that arm-local
/// names and scopes are **shared**, not isolated (see [`ArmNamespace`]).
/// This is the correct primitive for shared-scope composition where the merged
/// arms must reference each other's local names — alpha-renaming would desync a
/// flag/readback from its in-program consumer.
///
/// # Errors
///
/// Returns [`FusionError`] under the same conditions as [`fuse_programs`].
pub fn merge_programs_shared(programs: &[Program]) -> Result<Program, FusionError> {
    match programs.len() {
        0 => Ok(Program::empty()),
        1 => Ok(programs[0].clone()),
        _ => fuse_programs_multi_with(programs, ArmNamespace::Shared),
    }
}

fn fuse_programs_multi(programs: &[Program]) -> Result<Program, FusionError> {
    fuse_programs_multi_with(programs, ArmNamespace::Isolated)
}

fn fuse_programs_multi_with(
    programs: &[Program],
    namespace: ArmNamespace,
) -> Result<Program, FusionError> {
    reject_non_composable_self_fusion(programs)?;

    // ------------------------------------------------------------------
    // Single pass over programs: collect entries, atomics, buffers,
    // hazards, and workgroup size in one go.
    // ------------------------------------------------------------------
    let mut merged_buffers: Vec<BufferDecl> = Vec::new();
    let mut name_to_index: FxHashMap<Ident, usize> = FxHashMap::default();
    let mut next_binding = 0_u32;

    let mut read_arms_per_buffer: FxHashMap<Ident, Vec<usize>> = FxHashMap::default();
    // Track write-arm history per buffer so a later READER can force
    // a barrier after the earlier writer. Without this, the fused
    // kernel runs writer + reader in the same launch with no
    // synchronization, and the reader sees stale data from threads
    // that haven't completed the writer's body yet  -  the exact
    // "stack_overflow_gets misses node 39" mode.
    let mut write_arms_per_buffer: FxHashMap<Ident, Vec<usize>> = FxHashMap::default();
    let mut barrier_after_arm: FxHashSet<usize> = FxHashSet::default();
    // Arms whose writes are derived from launch geometry need a grid-level
    // fence before later arms read them. A workgroup barrier waits only for
    // the current block, so it cannot order "block 0 writes offsets, block 1
    // reads offsets" shapes inside a fused launch.
    let mut grid_sync_writer_arms: FxHashSet<usize> = FxHashSet::default();

    let mut fused_workgroup = [1u32, 1, 1];
    let mut max_arm_threads: u64 = 1;

    let mut arm_entries: Vec<Vec<Node>> = Vec::with_capacity(programs.len());

    // Shared-namespace merge prefixes ONLY names declared in ≥2 arms (genuine
    // collisions, e.g. a primitive's internal `acc`). A name declared in
    // exactly one arm — including a value produced in one arm and consumed in
    // another (`let __cmp_N = …` / `Var(__cmp_N)`) — is globally unique and
    // stays unrenamed so the decl→use link survives. Isolated fusion renames
    // every name (the set is unused for that mode).
    let multiply_declared: FxHashSet<Ident> = match namespace {
        ArmNamespace::Isolated => FxHashSet::default(),
        ArmNamespace::Shared => {
            let entries: Vec<&[Node]> = programs.iter().map(Program::entry).collect();
            multiply_declared_names(&entries)
        }
    };

    for (arm_idx, prog) in programs.iter().enumerate() {
        // Walk entry nodes once: clone into segment and collect both
        // atomic targets (writes) and Load targets (reads). Buffers
        // referenced inside the body but NOT declared in the arm's
        // own `buffers()` table  -  produced by an earlier arm  -  only
        // surface here. Without this, RAW hazards across arms that
        // read shared scalars (e.g. broadcast reading the scalar
        // written by a single-thread `bitset_any`) get no barrier
        // and silently produce stale reads on threads that haven't
        // observed the writer's flush.
        let entry = prog.entry();
        let mut segment = Vec::with_capacity(entry.len());
        let mut atomic_targets: FxHashSet<Ident> = FxHashSet::default();
        let mut load_targets: FxHashSet<Ident> = FxHashSet::default();
        let mut store_targets: FxHashSet<Ident> = FxHashSet::default();
        let mut divergent_store_seen = false;
        for node in entry {
            match namespace {
                ArmNamespace::Isolated => {
                    push_alpha_renamed_arm_entry_node(&mut segment, node, arm_idx);
                }
                ArmNamespace::Shared => {
                    ArmRenamer::shared(arm_idx, &multiply_declared)
                        .push_entry_node(&mut segment, node);
                }
            }
            collect_buffer_targets(
                node,
                &mut load_targets,
                &mut store_targets,
                &mut atomic_targets,
            );
            if has_divergent_invocation_gated_store(node, false) {
                divergent_store_seen = true;
            }
        }
        if divergent_store_seen || has_launch_geometry_dependent_write(prog.entry()) {
            grid_sync_writer_arms.insert(arm_idx);
        }
        arm_entries.push(segment);

        let mut arm_reads: FxHashSet<Ident> = FxHashSet::default();
        let mut arm_explicit_writes: FxHashSet<Ident> = FxHashSet::default();
        classify_and_merge_arm_buffers(
            prog,
            &mut arm_reads,
            &mut arm_explicit_writes,
            &mut merged_buffers,
            &mut name_to_index,
            &mut next_binding,
        );

        // Body-level reads from buffers declared by EARLIER arms.
        // The arm's own buffers().iter() loop already populated
        // `arm_reads` for declared ReadOnly inputs; this adds any
        // additional reads inferred from `Expr::Load` references.
        for target in &load_targets {
            arm_reads.insert(target.clone());
        }
        // Body-level stores to buffers declared by earlier arms.
        for target in &store_targets {
            arm_explicit_writes.insert(target.clone());
        }

        // Atomic writes count only for buffers not already read or explicitly written.
        let mut arm_writes = arm_explicit_writes.clone();
        for target in &atomic_targets {
            if !arm_reads.contains(target) && !arm_explicit_writes.contains(target) {
                arm_writes.insert(target.clone());
            }
        }

        // F-IR-22: WAR hazard  -  for each buffer this arm writes, if
        // any previous arm read it, mark a barrier after every such
        // earlier read arm so the new write can't clobber the read.
        for write_buf in &arm_writes {
            if let Some(read_arms) = read_arms_per_buffer.get(write_buf) {
                for &read_arm in read_arms {
                    barrier_after_arm.insert(read_arm);
                }
            }
        }

        // RAW hazard  -  for each buffer this arm reads, if any
        // previous arm wrote it, the writer's results must be
        // visible before this read. Insert a barrier after every
        // such earlier writer arm. Required because the fused
        // kernel runs as one backend launch; without a barrier,
        // threads in this arm may execute the load before the
        // writer arm's threads have completed their store, yielding
        // stale data and silently dropping rule findings (recall=0
        // mode previously observed on `stack_overflow_gets` for
        // node ids past the warp boundary).
        for read_buf in &arm_reads {
            if let Some(write_arms) = write_arms_per_buffer.get(read_buf) {
                for &write_arm in write_arms {
                    barrier_after_arm.insert(write_arm);
                }
            }
        }

        // Update read tracking for later arms.
        for read_buf in &arm_reads {
            read_arms_per_buffer
                .entry(read_buf.clone())
                .or_default()
                .push(arm_idx);
        }
        // Update write tracking for later RAW detection.
        for write_buf in &arm_writes {
            write_arms_per_buffer
                .entry(write_buf.clone())
                .or_default()
                .push(arm_idx);
        }

        // Workgroup size tracking.
        let wg = prog.workgroup_size();
        fused_workgroup[0] = fused_workgroup[0].max(wg[0]);
        fused_workgroup[1] = fused_workgroup[1].max(wg[1]);
        fused_workgroup[2] = fused_workgroup[2].max(wg[2]);
        let arm_threads = u64::from(wg[0]) * u64::from(wg[1]) * u64::from(wg[2]);
        max_arm_threads = max_arm_threads.max(arm_threads);
    }

    let combined_entry = flatten_arm_entries(
        arm_entries,
        &barrier_after_arm,
        &grid_sync_writer_arms,
        programs.len(),
        namespace,
    );
    reject_overdispatch(fused_workgroup, max_arm_threads)?;
    Ok(Program::wrapped(
        merged_buffers,
        fused_workgroup,
        combined_entry,
    ))
}

fn classify_and_merge_arm_buffers(
    prog: &Program,
    arm_reads: &mut FxHashSet<Ident>,
    arm_explicit_writes: &mut FxHashSet<Ident>,
    merged_buffers: &mut Vec<BufferDecl>,
    name_to_index: &mut FxHashMap<Ident, usize>,
    next_binding: &mut u32,
) {
    for buf in prog.buffers() {
        let name = Ident::from(buf.name());
        match buf.access() {
            BufferAccess::ReadOnly | BufferAccess::Uniform => {
                arm_reads.insert(name.clone());
            }
            BufferAccess::ReadWrite => {
                arm_explicit_writes.insert(name.clone());
            }
            _ => {}
        }
        if let Some(&idx) = name_to_index.get(&name) {
            let existing = &mut merged_buffers[idx];
            let access = buf.access();
            upgrade_buffer_access(existing, &access);
            if buf.count > existing.count {
                existing.count = buf.count;
            }
            if buf.is_output() {
                existing.is_output = true;
                existing.pipeline_live_out = true;
            }
        } else {
            let mut merged = buf.clone();
            if merged.access() != BufferAccess::Workgroup {
                merged.binding = *next_binding;
                *next_binding += 1;
            }
            name_to_index.insert(Ident::from(merged.name()), merged_buffers.len());
            merged_buffers.push(merged);
        }
    }
}

fn reject_non_composable_self_fusion(programs: &[Program]) -> Result<(), FusionError> {
    let mut seen_op_ids: FxHashMap<String, bool> = FxHashMap::default();
    for prog in programs {
        let key = prog
            .entry_op_id()
            .map_or_else(|| fallback_composition_key(prog), ToString::to_string);
        let is_non_comp = prog.is_non_composable_with_self();
        match seen_op_ids.get_mut(&key) {
            Some(has_non_comp) if *has_non_comp || is_non_comp => {
                return Err(FusionError::SelfAliasing(FusionSelfAliasingError {
                    op_id: key,
                    fix: "rename the second parser's workgroup buffer or split into two separate dispatches",
                }));
            }
            Some(_) => {}
            None => {
                seen_op_ids.insert(key, is_non_comp);
            }
        }
    }
    Ok(())
}

fn flatten_arm_entries(
    arm_entries: Vec<Vec<Node>>,
    barrier_after_arm: &FxHashSet<usize>,
    grid_sync_writer_arms: &FxHashSet<usize>,
    program_count: usize,
    namespace: ArmNamespace,
) -> Vec<Node> {
    let total_nodes: usize = arm_entries.iter().map(Vec::len).sum();
    let mut combined_entry = Vec::with_capacity(total_nodes + program_count);
    for (arm_idx, segment) in arm_entries.into_iter().enumerate() {
        match namespace {
            // Isolated arms each get their own `Block` scope so reused
            // arm-local names cannot collide across arms.
            ArmNamespace::Isolated => combined_entry.push(Node::Block(segment)),
            // Shared arms splice flat into the one rule-wide scope, so a
            // `let` in an earlier arm stays visible to a later arm's use.
            ArmNamespace::Shared => combined_entry.extend(segment),
        }
        if barrier_after_arm.contains(&arm_idx) {
            // Workgroup `SeqCst` (`bar.sync 0`) is sufficient only when the
            // prior write is uniform across the launch. Launch-geometry
            // dependent writes must become a top-level `GridSync`, where the
            // runtime split pass can lower the fused program into globally
            // ordered dispatch segments.
            let ordering = if grid_sync_writer_arms.contains(&arm_idx) {
                crate::memory_model::MemoryOrdering::GridSync
            } else {
                crate::memory_model::MemoryOrdering::SeqCst
            };
            combined_entry.push(Node::barrier_with_ordering(ordering));
        }
    }
    combined_entry
}

fn reject_overdispatch(fused_workgroup: [u32; 3], max_arm_threads: u64) -> Result<(), FusionError> {
    let fused_threads = u64::from(fused_workgroup[0])
        * u64::from(fused_workgroup[1])
        * u64::from(fused_workgroup[2]);
    let policy = SchedulingPolicy::standard();
    if policy.allow_fused_threads(fused_threads, max_arm_threads) {
        return Ok(());
    }
    Err(FusionError::OverDispatch(FusionOverDispatchError {
        max_arm_threads,
        fused_threads,
        fix: "split the batch or use per-arm dispatch; axis-wise max exceeds the shared over-dispatch policy",
    }))
}
