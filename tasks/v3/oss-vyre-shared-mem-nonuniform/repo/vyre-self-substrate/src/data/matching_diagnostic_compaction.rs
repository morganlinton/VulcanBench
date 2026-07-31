//! Matching diagnostic compaction via `vyre-primitives::matching`.
//!
//! Self-substrate diagnostics and pass traces produce raw spans, brace-pair
//! links, and pattern-id regions. This module keeps that pipeline resident:
//! compile the DFA once, match brackets on-device, sort region triples, then
//! emit dedup survivor flags for stream compaction.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::hardware::scratch::reserve_vec_capacity;
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
use vyre_primitives::matching::bracket_match::{
    bracket_match, bracket_match_dispatch_grid, pack_u32, CLOSE_BRACE, MATCH_NONE, OPEN_BRACE,
    OTHER,
};
use vyre_primitives::matching::region::{
    dedup_regions_flag_program, region_dedup_dispatch_grid, region_sort_program, RegionTriple,
};
use vyre_primitives::matching::{
    dfa_compile, dfa_compile_with_budget, dfa_fingerprint, dfa_wire_bytes, nfa_to_dfa, CompiledDfa,
    DfaCompileError, DfaDedupBatch, DfaDedupResult, DfaDedupTable, NfaTables, NfaToDfaError,
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::matching::{
    bracket_match::cpu_ref as primitive_bracket_match,
    region::{dedup_regions_cpu, dedup_regions_inplace, sort_regions_cpu},
};

/// Caller-owned dispatch scratch for matching diagnostic compaction.
#[derive(Debug, Default)]
pub struct MatchingDiagnosticCompactionGpuScratch {
    inputs: Vec<Vec<u8>>,
    pids: Vec<u32>,
    starts: Vec<u32>,
    ends: Vec<u32>,
    decoded_pids: Vec<u32>,
    decoded_starts: Vec<u32>,
    decoded_ends: Vec<u32>,
    decoded_regions: Vec<RegionTriple>,
    match_pairs_seed: Vec<u32>,
}

/// Compile diagnostic patterns to a DFA using the default primitive budget.
#[must_use]
pub fn compile_diagnostic_dfa(patterns: &[&[u8]]) -> CompiledDfa {
    dfa_compile(patterns)
}

/// Compile diagnostic patterns to a DFA using an explicit transition-table budget.
///
/// # Errors
///
/// Returns [`DfaCompileError`] when the pattern set exceeds the caller budget.
pub fn compile_diagnostic_dfa_with_budget(
    patterns: &[&[u8]],
    budget_bytes: usize,
) -> Result<CompiledDfa, DfaCompileError> {
    dfa_compile_with_budget(patterns, budget_bytes)
}

/// Compile a diagnostic NFA table into the dense DFA used by the bounded scan path.
///
/// This is the self-substrate bridge for regex-style diagnostics: pattern sets
/// that remain within `max_dfa_states` can run on the dense one-load-per-byte
/// DFA kernel, while state-exploding sets report a structured error and stay on
/// the NFA path.
///
/// # Errors
///
/// Returns [`NfaToDfaError`] when the NFA table shape is malformed or subset
/// construction exceeds `max_dfa_states`.
pub fn compile_diagnostic_nfa_to_dfa(
    tables: &NfaTables<'_>,
    max_dfa_states: usize,
) -> Result<CompiledDfa, NfaToDfaError> {
    nfa_to_dfa(tables, max_dfa_states)
}

/// Stable content-addressed key for deduplicating diagnostic DFA plans.
#[must_use]
pub fn diagnostic_dfa_fingerprint(dfa: &CompiledDfa) -> u64 {
    dfa_fingerprint(dfa)
}

/// Wire-relevant byte size for diagnostic DFA reuse accounting.
#[must_use]
pub fn diagnostic_dfa_wire_bytes(dfa: &CompiledDfa) -> usize {
    dfa_wire_bytes(dfa)
}

/// Retained wire bytes across all canonical diagnostic DFA plans.
#[must_use]
pub fn diagnostic_dfa_canonical_wire_bytes(table: &DfaDedupTable) -> usize {
    table.canonical_wire_bytes()
}

/// Saved diagnostic DFA wire bytes as parts-per-million of submitted bytes.
#[must_use]
pub fn diagnostic_dfa_saved_wire_ppm(batch: &DfaDedupBatch) -> u32 {
    batch.saved_wire_ppm()
}

/// Deduplicate a diagnostic DFA plan into a caller-owned content-addressed table.
pub fn dedup_diagnostic_dfa_plan(table: &mut DfaDedupTable, dfa: CompiledDfa) -> DfaDedupResult {
    table.insert(dfa)
}

/// Deduplicate a batch of diagnostic DFA plans and retain input-order mappings.
pub fn dedup_diagnostic_dfa_plans<I>(table: &mut DfaDedupTable, dfas: I) -> DfaDedupBatch
where
    I: IntoIterator<Item = CompiledDfa>,
{
    table.insert_many(dfas)
}

/// Merge another diagnostic DFA table into this table without recompilation.
pub fn merge_diagnostic_dfa_tables(
    table: &mut DfaDedupTable,
    other: &DfaDedupTable,
) -> DfaDedupBatch {
    table.merge_from(other)
}

/// Return the little-endian u32 byte layout used for diagnostic fixture uploads.
#[must_use]
pub fn pack_diagnostic_u32(words: &[u32]) -> Vec<u8> {
    pack_u32(words)
}

/// Match diagnostic brace tokens through the bracket-match primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when `kinds.len()` or `max_depth` exceeds the
/// primitive index space, dispatch fails, or readback is malformed.
pub fn bracket_pairs_via(
    dispatcher: &dyn OptimizerDispatcher,
    kinds: &[u32],
    max_depth: u32,
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = MatchingDiagnosticCompactionGpuScratch::default();
    let mut out = Vec::new();
    bracket_pairs_via_with_scratch_into(dispatcher, kinds, max_depth, &mut scratch, &mut out)?;
    Ok(out)
}

/// Match diagnostic brace tokens through the bracket-match primitive using
/// caller-owned scratch.
///
/// # Errors
///
/// Returns [`DispatchError`] when validation, dispatch, or readback fails.
pub fn bracket_pairs_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    kinds: &[u32],
    max_depth: u32,
    scratch: &mut MatchingDiagnosticCompactionGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, matching_diagnostic_compaction_calls};
    bump(&matching_diagnostic_compaction_calls);

    let n = checked_len(kinds.len(), "bracket_pairs_via")?;
    let max_depth_usize = usize::try_from(max_depth).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: bracket_pairs_via max_depth={max_depth} does not fit usize scratch sizing."
        ))
    })?;
    let program = bracket_match("kinds", "stack", "match_pairs", n, max_depth);
    ensure_input_slots(&mut scratch.inputs, 3);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], kinds);
    write_zero_bytes(
        &mut scratch.inputs[1],
        max_depth_usize * std::mem::size_of::<u32>(),
    );
    scratch.match_pairs_seed.clear();
    scratch.match_pairs_seed.resize(kinds.len(), MATCH_NONE);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.match_pairs_seed);
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some(bracket_match_dispatch_grid(n, max_depth)),
    )?;
    decode_first_output(&outputs, kinds.len(), "bracket_pairs_via", out)
}

/// Sort diagnostic region triples by `(pid, start, end)` through the primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when the region count is zero or too large,
/// dispatch fails, or readback is malformed.
pub fn sort_regions_via(
    dispatcher: &dyn OptimizerDispatcher,
    regions: &[RegionTriple],
) -> Result<Vec<RegionTriple>, DispatchError> {
    let mut scratch = MatchingDiagnosticCompactionGpuScratch::default();
    let mut out = Vec::new();
    sort_regions_via_with_scratch_into(dispatcher, regions, &mut scratch, &mut out)?;
    Ok(out)
}

/// Sort diagnostic region triples through the primitive using caller-owned
/// staging and output storage.
///
/// # Errors
///
/// Returns [`DispatchError`] when the region count is zero or too large,
/// dispatch fails, or readback is malformed.
pub fn sort_regions_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    regions: &[RegionTriple],
    scratch: &mut MatchingDiagnosticCompactionGpuScratch,
    out: &mut Vec<RegionTriple>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, matching_diagnostic_compaction_calls};
    bump(&matching_diagnostic_compaction_calls);

    let count = checked_nonzero_len(regions.len(), "sort_regions_via")?;
    split_regions_into(
        regions,
        &mut scratch.pids,
        &mut scratch.starts,
        &mut scratch.ends,
    )?;
    let program = region_sort_program(
        "pids",
        "starts",
        "ends",
        "pids_out",
        "starts_out",
        "ends_out",
        count,
    );
    ensure_input_slots(&mut scratch.inputs, 6);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &scratch.pids);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &scratch.starts);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.ends);
    for slot in 3..=5 {
        write_zero_bytes(
            &mut scratch.inputs[slot],
            regions.len() * std::mem::size_of::<u32>(),
        );
    }
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some([ceil_div_u32(count, 256), 1, 1]),
    )?;
    decode_region_outputs_into(&outputs, regions.len(), "sort_regions_via", scratch, out)
}

/// Emit dedup survivor flags for sorted region triples through the primitive.
///
/// # Errors
///
/// Returns [`DispatchError`] when the region count is too large, dispatch
/// fails, or readback is malformed.
pub fn dedup_region_survivor_flags_via(
    dispatcher: &dyn OptimizerDispatcher,
    sorted_regions: &[RegionTriple],
) -> Result<Vec<u32>, DispatchError> {
    let mut scratch = MatchingDiagnosticCompactionGpuScratch::default();
    let mut out = Vec::new();
    dedup_region_survivor_flags_via_with_scratch_into(
        dispatcher,
        sorted_regions,
        &mut scratch,
        &mut out,
    )?;
    Ok(out)
}

/// Emit dedup survivor flags through the primitive using caller-owned staging.
///
/// # Errors
///
/// Returns [`DispatchError`] when the region count is too large, dispatch
/// fails, or readback is malformed.
pub fn dedup_region_survivor_flags_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    sorted_regions: &[RegionTriple],
    scratch: &mut MatchingDiagnosticCompactionGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    use crate::observability::{bump, matching_diagnostic_compaction_calls};
    bump(&matching_diagnostic_compaction_calls);

    if sorted_regions.is_empty() {
        out.clear();
        return Ok(());
    }
    let count = checked_len(sorted_regions.len(), "dedup_region_survivor_flags_via")?;
    split_regions_into(
        sorted_regions,
        &mut scratch.pids,
        &mut scratch.starts,
        &mut scratch.ends,
    )?;
    let program = dedup_regions_flag_program("pids", "starts", "ends", "survivors", count);
    ensure_input_slots(&mut scratch.inputs, 4);
    write_u32_slice_le_bytes(&mut scratch.inputs[0], &scratch.pids);
    write_u32_slice_le_bytes(&mut scratch.inputs[1], &scratch.starts);
    write_u32_slice_le_bytes(&mut scratch.inputs[2], &scratch.ends);
    write_zero_bytes(
        &mut scratch.inputs[3],
        sorted_regions.len() * std::mem::size_of::<u32>(),
    );
    let outputs = dispatcher.dispatch(
        &program,
        &scratch.inputs,
        Some(region_dedup_dispatch_grid(count)),
    )?;
    decode_first_output(
        &outputs,
        sorted_regions.len(),
        "dedup_region_survivor_flags_via",
        out,
    )
}

/// Sort and dedup diagnostic regions on the CPU parity path.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_dedup_regions(regions: Vec<RegionTriple>) -> Vec<RegionTriple> {
    dedup_regions_cpu(regions)
}

/// Sort diagnostic regions on the CPU parity path.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_sort_regions(mut regions: Vec<RegionTriple>) -> Vec<RegionTriple> {
    sort_regions_cpu(&mut regions);
    regions
}

/// Dedup diagnostic regions in place on the CPU parity path.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_dedup_regions_inplace(regions: &mut Vec<RegionTriple>) {
    dedup_regions_inplace(regions);
}

/// Match diagnostic brace tokens on the CPU parity path.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_bracket_pairs(kinds: &[u32], max_depth: u32) -> Vec<u32> {
    primitive_bracket_match(kinds, max_depth)
}

/// Build a compact fixture token stream for one nested diagnostic block.
#[must_use]
pub fn nested_diagnostic_brace_fixture() -> Vec<u32> {
    vec![OPEN_BRACE, OTHER, OPEN_BRACE, CLOSE_BRACE, CLOSE_BRACE]
}

#[cfg(test)]
fn split_regions(regions: &[RegionTriple]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut pids = Vec::with_capacity(regions.len());
    let mut starts = Vec::with_capacity(regions.len());
    let mut ends = Vec::with_capacity(regions.len());
    split_regions_into(regions, &mut pids, &mut starts, &mut ends)
        .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - test fixture region split should reserve output columns");
    (pids, starts, ends)
}

fn split_regions_into(
    regions: &[RegionTriple],
    pids: &mut Vec<u32>,
    starts: &mut Vec<u32>,
    ends: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    pids.clear();
    starts.clear();
    ends.clear();
    reserve_vec_capacity(pids, regions.len(), "diagnostic region pids")?;
    reserve_vec_capacity(starts, regions.len(), "diagnostic region starts")?;
    reserve_vec_capacity(ends, regions.len(), "diagnostic region ends")?;
    for region in regions {
        pids.push(region.pid);
        starts.push(region.start);
        ends.push(region.end);
    }
    Ok(())
}

fn checked_len(len: usize, context: &'static str) -> Result<u32, DispatchError> {
    u32::try_from(len).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: {context} received {len} items, which exceeds the u32 GPU index space."
        ))
    })
}

fn checked_nonzero_len(len: usize, context: &'static str) -> Result<u32, DispatchError> {
    let count = checked_len(len, context)?;
    if count == 0 {
        return Err(DispatchError::BadInputs(format!(
            "Fix: {context} requires at least one region."
        )));
    }
    Ok(count)
}

fn decode_region_outputs_into(
    outputs: &[Vec<u8>],
    count: usize,
    context: &'static str,
    scratch: &mut MatchingDiagnosticCompactionGpuScratch,
    out: &mut Vec<RegionTriple>,
) -> Result<(), DispatchError> {
    if outputs.len() < 3 {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected three output buffers, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], count, context, &mut scratch.decoded_pids)?;
    decode_u32_output_exact(&outputs[1], count, context, &mut scratch.decoded_starts)?;
    decode_u32_output_exact(&outputs[2], count, context, &mut scratch.decoded_ends)?;
    scratch.decoded_regions.clear();
    reserve_vec_capacity(&mut scratch.decoded_regions, count, context)?;
    for index in 0..count {
        scratch.decoded_regions.push(RegionTriple::new(
            scratch.decoded_pids[index],
            scratch.decoded_starts[index],
            scratch.decoded_ends[index],
        ));
    }
    out.clear();
    out.extend_from_slice(&scratch.decoded_regions);
    Ok(())
}

fn decode_first_output(
    outputs: &[Vec<u8>],
    words: usize,
    context: &'static str,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: {context} expected at least one output buffer, got 0."
        )));
    }
    decode_u32_output_exact(&outputs[0], words, context, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;
    use vyre_foundation::ir::Program;

    struct MatchingDispatcher;

    impl OptimizerDispatcher for MatchingDispatcher {
        fn dispatch(
            &self,
            program: &Program,
            inputs: &[Vec<u8>],
            grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let op_id = program
                .entry
                .iter()
                .find_map(|node| match node {
                    vyre_foundation::ir::Node::Region { generator, .. } => Some(generator.as_str()),
                    _ => None,
                })
                .expect("Fix: matching primitive should expose a region generator");
            match op_id {
                vyre_primitives::matching::bracket_match::OP_ID => {
                    let kinds = crate::hardware::dispatch_buffers::read_u32s(&inputs[0]);
                    let depth_words = inputs[1].len() / std::mem::size_of::<u32>();
                    assert_eq!(
                        grid_override,
                        Some(bracket_match_dispatch_grid(kinds.len() as u32, depth_words as u32)),
                        "Fix: bracket_pairs_via must dispatch the primitive with enough workgroups for its selected bracket matcher."
                    );
                    Ok(vec![u32_slice_to_le_bytes(&primitive_bracket_match(
                        &kinds,
                        depth_words as u32,
                    ))])
                }
                "vyre-primitives::matching::region::region_sort" => {
                    let regions = join_regions(
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[0]),
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[1]),
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[2]),
                    );
                    assert_eq!(
                        grid_override,
                        Some([ceil_div_u32(regions.len() as u32, 256), 1, 1]),
                        "Fix: sort_regions_via must dispatch one lane per region triple."
                    );
                    let sorted = reference_sort_regions(regions);
                    let (pids, starts, ends) = split_regions(&sorted);
                    Ok(vec![
                        u32_slice_to_le_bytes(&pids),
                        u32_slice_to_le_bytes(&starts),
                        u32_slice_to_le_bytes(&ends),
                    ])
                }
                "vyre-primitives::matching::region::dedup_regions_flag" => {
                    let regions = join_regions(
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[0]),
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[1]),
                        &crate::hardware::dispatch_buffers::read_u32s(&inputs[2]),
                    );
                    assert_eq!(
                        grid_override,
                        Some(region_dedup_dispatch_grid(regions.len() as u32)),
                        "Fix: dedup_region_survivor_flags_via must use the primitive's 256-lane region-dedup grid."
                    );
                    let flags = survivor_flags(&regions);
                    Ok(vec![u32_slice_to_le_bytes(&flags)])
                }
                other => panic!("unexpected matching primitive op id {other}"),
            }
        }
    }

    fn join_regions(pids: &[u32], starts: &[u32], ends: &[u32]) -> Vec<RegionTriple> {
        pids.iter()
            .zip(starts.iter())
            .zip(ends.iter())
            .map(|((pid, start), end)| RegionTriple::new(*pid, *start, *end))
            .collect()
    }

    fn survivor_flags(sorted_regions: &[RegionTriple]) -> Vec<u32> {
        let mut flags = Vec::with_capacity(sorted_regions.len());
        for (index, current) in sorted_regions.iter().enumerate() {
            let has_prev_overlap = sorted_regions[..index]
                .iter()
                .any(|prior| prior.pid == current.pid && prior.end >= current.start);
            flags.push(u32::from(!has_prev_overlap));
        }
        flags
    }

    #[test]
    fn dfa_compile_wrappers_use_primitive_compiler() {
        let patterns: &[&[u8]] = &[b"error", b"warning"];
        let default = compile_diagnostic_dfa(patterns);
        let budgeted = compile_diagnostic_dfa_with_budget(patterns, 1 << 20).unwrap();
        assert_eq!(default.state_count, budgeted.state_count);
        assert_eq!(default.max_pattern_len, 7);
    }

    #[test]
    fn bracket_pairs_dispatch_through_primitive() {
        let fixture = nested_diagnostic_brace_fixture();
        assert_eq!(
            bracket_pairs_via(&MatchingDispatcher, &fixture, 8).unwrap(),
            reference_bracket_pairs(&fixture, 8)
        );
        assert_eq!(
            pack_diagnostic_u32(&[OPEN_BRACE, CLOSE_BRACE]),
            pack_u32(&[OPEN_BRACE, CLOSE_BRACE])
        );
    }

    #[test]
    fn bracket_pairs_uncapped_large_stream_dispatches_all_parallel_workgroups() {
        let mut kinds = vec![OTHER; 513];
        kinds[0] = OPEN_BRACE;
        kinds[255] = OPEN_BRACE;
        kinds[256] = CLOSE_BRACE;
        kinds[512] = CLOSE_BRACE;

        assert_eq!(
            bracket_pairs_via(&MatchingDispatcher, &kinds, kinds.len() as u32).unwrap(),
            reference_bracket_pairs(&kinds, kinds.len() as u32)
        );
    }

    #[test]
    fn bracket_pairs_depth_capped_stream_keeps_single_workgroup_fallback() {
        let mut kinds = vec![OTHER; 513];
        kinds[0] = OPEN_BRACE;
        kinds[64] = OPEN_BRACE;
        kinds[65] = CLOSE_BRACE;

        assert_eq!(
            bracket_pairs_via(&MatchingDispatcher, &kinds, 64).unwrap(),
            reference_bracket_pairs(&kinds, 64)
        );
    }

    #[test]
    fn bracket_pairs_generated_dispatch_grids_cover_4096_large_streams() {
        for case in 0..4096u32 {
            let len = 257 + (case.wrapping_mul(37) % 768) as usize;
            let max_depth = if case % 2 == 0 {
                len as u32
            } else {
                1 + case.wrapping_mul(19) % 192
            };
            let mut state = 0x8BAD_F00Du32 ^ case.wrapping_mul(0x9E37_79B9);
            let mut kinds = Vec::with_capacity(len);
            for index in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let kind = match (state.wrapping_add(index as u32)) % 7 {
                    0 | 1 => OPEN_BRACE,
                    2 | 3 => CLOSE_BRACE,
                    _ => OTHER,
                };
                kinds.push(kind);
            }

            assert_eq!(
                bracket_pairs_via(&MatchingDispatcher, &kinds, max_depth).unwrap(),
                reference_bracket_pairs(&kinds, max_depth),
                "case {case}: diagnostic bracket dispatch must match primitive CPU oracle"
            );
        }
    }

    #[test]
    fn dedup_survivor_flags_nested_cluster_uses_prior_merged_span() {
        let sorted = vec![
            RegionTriple::new(7, 0, 10),
            RegionTriple::new(7, 2, 3),
            RegionTriple::new(7, 9, 12),
            RegionTriple::new(7, 20, 25),
        ];

        assert_eq!(
            dedup_region_survivor_flags_via(&MatchingDispatcher, &sorted).unwrap(),
            vec![1, 0, 0, 1]
        );
    }

    #[test]
    fn dedup_survivor_flags_large_stream_dispatches_region_grid() {
        let sorted = (0..513u32)
            .map(|index| RegionTriple::new(index / 171, index * 3, index * 3 + 1))
            .collect::<Vec<_>>();

        assert_eq!(
            dedup_region_survivor_flags_via(&MatchingDispatcher, &sorted).unwrap(),
            vec![1; sorted.len()]
        );
    }

    #[test]
    fn dedup_survivor_flags_generated_regions_cover_4096_large_streams() {
        for case in 0..4096u32 {
            let count = 257 + (case.wrapping_mul(29) % 768) as usize;
            let mut state = 0xD1CE_C0DEu32 ^ case.wrapping_mul(0x85EB_CA6B);
            let mut regions = Vec::with_capacity(count);
            for index in 0..count {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let pid = state % 7;
                state = state.rotate_left(3).wrapping_add(index as u32);
                let start = state % 4096;
                state = state.rotate_left(9) ^ case;
                let width = state % 64;
                regions.push(RegionTriple::new(pid, start, start.saturating_add(width)));
            }

            let mut sorted = regions;
            sort_regions_cpu(&mut sorted);
            let flags = dedup_region_survivor_flags_via(&MatchingDispatcher, &sorted).unwrap();
            let actual_cluster_starts = sorted
                .iter()
                .zip(flags.iter())
                .filter_map(|(region, flag)| (*flag != 0).then_some((region.pid, region.start)))
                .collect::<Vec<_>>();
            let expected_cluster_starts = reference_dedup_regions(sorted.clone())
                .into_iter()
                .map(|region| (region.pid, region.start))
                .collect::<Vec<_>>();

            assert_eq!(
                actual_cluster_starts, expected_cluster_starts,
                "case {case}: survivor flags must mark the same cluster starts as CPU dedup"
            );
        }
    }

    #[test]
    fn region_cpu_wrappers_match_primitives_exactly() {
        let regions = vec![
            RegionTriple::new(0, 7, 10),
            RegionTriple::new(0, 5, 8),
            RegionTriple::new(1, 5, 8),
        ];
        assert_eq!(
            reference_dedup_regions(regions.clone()),
            dedup_regions_cpu(regions.clone())
        );
        let mut in_place = regions.clone();
        reference_dedup_regions_inplace(&mut in_place);
        assert_eq!(in_place, reference_dedup_regions(regions));
    }

    #[test]
    fn region_sort_dispatches_primitive_shape() {
        let regions = vec![
            RegionTriple::new(2, 0, 1),
            RegionTriple::new(0, 5, 10),
            RegionTriple::new(0, 5, 8),
        ];
        assert_eq!(
            sort_regions_via(&MatchingDispatcher, &regions).unwrap(),
            reference_sort_regions(regions)
        );
    }

    #[test]
    fn region_sort_reuses_caller_owned_split_and_decode_capacity() {
        let large = (0..128)
            .map(|idx| RegionTriple::new(idx % 7, 128 - idx, 128 - idx + 3))
            .collect::<Vec<_>>();
        let small = vec![RegionTriple::new(1, 2, 3), RegionTriple::new(0, 1, 4)];
        let mut scratch = MatchingDiagnosticCompactionGpuScratch::default();
        let mut out = Vec::new();

        sort_regions_via_with_scratch_into(&MatchingDispatcher, &large, &mut scratch, &mut out)
            .expect("Fix: large diagnostic region sort should dispatch");
        let pids_capacity = scratch.pids.capacity();
        let decoded_capacity = scratch.decoded_regions.capacity();

        sort_regions_via_with_scratch_into(&MatchingDispatcher, &small, &mut scratch, &mut out)
            .expect("Fix: small diagnostic region sort should reuse scratch");

        assert_eq!(scratch.pids.capacity(), pids_capacity);
        assert_eq!(scratch.decoded_regions.capacity(), decoded_capacity);
        assert_eq!(out, reference_sort_regions(small));
    }

    #[test]
    fn dedup_flags_dispatches_primitive_shape() {
        let sorted = vec![
            RegionTriple::new(0, 5, 8),
            RegionTriple::new(0, 7, 10),
            RegionTriple::new(1, 7, 10),
        ];
        assert_eq!(
            dedup_region_survivor_flags_via(&MatchingDispatcher, &sorted).unwrap(),
            vec![1, 0, 1]
        );
    }

    #[test]
    fn dedup_flags_reuses_caller_owned_split_capacity() {
        let large = (0..63)
            .map(|idx| RegionTriple::new(idx % 11, idx, idx + 2))
            .collect::<Vec<_>>();
        let small = vec![
            RegionTriple::new(0, 0, 2),
            RegionTriple::new(0, 1, 3),
            RegionTriple::new(1, 1, 3),
        ];
        let mut scratch = MatchingDiagnosticCompactionGpuScratch::default();
        let mut flags = Vec::new();

        dedup_region_survivor_flags_via_with_scratch_into(
            &MatchingDispatcher,
            &large,
            &mut scratch,
            &mut flags,
        )
        .expect("Fix: large diagnostic dedup should dispatch");
        let pids_capacity = scratch.pids.capacity();

        dedup_region_survivor_flags_via_with_scratch_into(
            &MatchingDispatcher,
            &small,
            &mut scratch,
            &mut flags,
        )
        .expect("Fix: small diagnostic dedup should reuse scratch");

        assert_eq!(scratch.pids.capacity(), pids_capacity);
        assert_eq!(flags, vec![1, 0, 1]);
    }

    #[test]
    fn empty_region_sort_error_is_actionable() {
        let err = sort_regions_via(&MatchingDispatcher, &[]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Fix: sort_regions_via requires at least one region"));
    }
}
