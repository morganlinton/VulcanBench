use std::time::Instant;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, u32_counter_reset_program,
    ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use vyre_driver::{ResidentDispatchStep, ResidentReadRange};
use vyre_foundation::ir::Program;
use vyre_libs::scan::classic_ac::{
    classic_ac_candidate_end_byte_mask_words, classic_ac_candidate_suffix2_mask_words,
    classic_ac_candidate_suffix3_bloom_words, classic_ac_compile,
    try_build_ac_bounded_ranges_suffix3_prefilter_program_ext, ClassicAcAutomaton,
    CLASSIC_AC_SUFFIX2_MASK_WORDS,
};
use vyre_libs::scan::{pack_haystack_u32, pack_u32_slice};

mod baseline;
mod count;
mod metrics;
pub(crate) mod support;

use baseline::cpu_aho_overlapping_matches;
#[cfg(test)]
use baseline::cpu_bounded_range_matches;
use metrics::{scan_ac_baseline_metric_points, scan_ac_bounded_ranges_metric_points, ScanAcStats};
use support::{
    build_irregular_haystack, decode_scan_outputs, encode_match_triples,
    match_triples_output_bytes, match_triples_readback_bytes, pattern_lengths,
    selected_scan_output_bytes, with_matches_readback_range,
};

#[cfg(test)]
mod tests;

const HAYSTACK_BYTES: usize = 4 * 1024 * 1024;
const MAX_MATCHES: u32 = 65_536;
const MATCH_COUNT_INPUT_INDEX: usize = 6;
const CANDIDATE_END_MASK_INPUT_INDEX: usize = 7;
const CANDIDATE_SUFFIX2_MASK_INPUT_INDEX: usize = 8;
const CANDIDATE_SUFFIX3_BLOOM_INPUT_INDEX: usize = 9;
const MATCHES_RESOURCE_INDEX: usize = 10;
const RESET_RESOURCE_INDICES: [usize; 1] = [MATCH_COUNT_INPUT_INDEX];
const SCAN_RESOURCE_INDICES: [usize; 11] = [
    0,
    1,
    2,
    3,
    4,
    5,
    MATCH_COUNT_INPUT_INDEX,
    CANDIDATE_END_MASK_INPUT_INDEX,
    CANDIDATE_SUFFIX2_MASK_INPUT_INDEX,
    CANDIDATE_SUFFIX3_BLOOM_INPUT_INDEX,
    MATCHES_RESOURCE_INDEX,
];
const MATCH_TRIPLE_WORDS: usize = 3;
const SUITES: &[SuiteKind] = &[
    SuiteKind::Smoke,
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

pub(crate) const PATTERNS: &[&[u8]] = &[
    b"AKIA",
    b"ghp_",
    b"Authorization: Bearer ",
    b"password=",
    b"api_key",
    b"secret=",
    b"BEGIN RSA PRIVATE KEY",
    b"BEGIN OPENSSH PRIVATE KEY",
    b"eval(",
    b"strcpy(",
    b"memcpy(",
    b"TODO:",
    b"unsafe {",
    b"__attribute__((",
    b"container_of(",
    b"ioread32(",
];

pub struct ScanAcIrregularLiterals;

struct ScanAcIrregularPrepared {
    program: Program,
    reset_program: Program,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall_ns: u64,
    stats: ScanAcStats,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for ScanAcIrregularLiterals {
    fn id(&self) -> BenchId {
        BenchId("scan.ac.irregular_literals.4m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Aho-Corasick Irregular Literal Scan 4M".to_string(),
            description: "Packed-byte AC bounded-ranges scan over unaligned, varied-length security/parser literals in a noisy 4 MiB haystack".to_string(),
            tags: vec![
                "scan".to_string(),
                "pattern".to_string(),
                "dfa".to_string(),
                "aho-corasick".to_string(),
                "packed-byte".to_string(),
                "irregular".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-libs".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some(32 * 1024 * 1024),
            min_input_bytes: Some(HAYSTACK_BYTES as u64),
            feature_set: vec![
                "matching-dfa".to_string(),
                "packed-byte".to_string(),
                "aho-corasick".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "Packed-byte Aho-Corasick irregular literal scan",
            "vyre-libs",
            "aho-corasick 1.1 overlapping CPU automaton",
            1.0,
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<ScanAcIrregularPrepared>()
            .map(|prepared| {
                let output_bytes = selected_scan_output_bytes(prepared.stats);
                (prepared.input_bytes_total, output_bytes)
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        Ok(Box::new(prepare_scan_ac_irregular(Some(ctx))?))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<ScanAcIrregularPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<ScanAcIrregularPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared irregular AC scan payload had the wrong type".to_string(),
                )
            })?;

        let mut dispatch_config = ctx.dispatch_config.clone();
        let program_workgroup = prepared.program.workgroup_size();
        let workgroup = dispatch_config
            .workgroup_override
            .unwrap_or(program_workgroup);
        if workgroup.contains(&0) {
            return Err(BenchError::ExecutionFailed(format!(
                "irregular AC scan received invalid workgroup {:?}. Fix: use positive dispatch dimensions.",
                workgroup
            )));
        }
        dispatch_config.grid_override.get_or_insert([
            prepared.stats.haystack_bytes.div_ceil(workgroup[0]).max(1),
            1,
            1,
        ]);

        let (outputs, wall_ns, dispatch_ns, resident_used, device_reset_sequence) =
            if let Some(resident) = prepared.resident.as_ref() {
                let sequence =
                    dispatch_resident_scan_sequence(ctx, prepared, resident, program_workgroup)?;
                (sequence.outputs, sequence.wall_ns, None, true, true)
            } else {
                let dispatch = dispatch_program_timed(
                    ctx,
                    &prepared.program,
                    None,
                    &prepared.inputs,
                    &dispatch_config,
                )?;
                let timed = dispatch.timed;
                (
                    timed.outputs,
                    timed.wall_ns,
                    timed.device_ns,
                    dispatch.resident_used,
                    false,
                )
            };

        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let resident_reset_bytes = 0;
        let accounting =
            transfer_accounting(prepared.input_bytes_total, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: scan_ac_bounded_ranges_metric_points(
                    prepared.stats,
                    prepared.baseline_wall_ns,
                    wall_ns,
                    resident_used,
                    resident_reset_bytes,
                    device_reset_sequence,
                    workgroup[0],
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(
                    prepared
                        .baseline_outputs
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>() as u64,
                ),
                custom: scan_ac_baseline_metric_points(prepared.stats),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(prepared.baseline_outputs.clone()),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        let baseline = run.baseline_outputs.as_ref().ok_or_else(|| {
            BenchError::CorrectnessViolation(
                "irregular AC scan did not capture baseline outputs".to_string(),
            )
        })?;
        let expected = decode_scan_outputs(baseline, "baseline irregular AC scan")?;
        let actual = decode_scan_outputs(&run.outputs, "GPU irregular AC scan")?;
        if actual != expected {
            return Err(BenchError::CorrectnessViolation(format!(
                "irregular AC scan decoded match mismatch: expected {} matches, got {}",
                expected.len(),
                actual.len()
            )));
        }
        Ok(Correctness::Certificate {
            digest: *blake3::hash(&encode_match_triples(&actual)).as_bytes(),
        })
    }
}

fn prepare_scan_ac_irregular(
    ctx: Option<&BenchContext>,
) -> Result<ScanAcIrregularPrepared, BenchError> {
    let (haystack, planted_matches) = build_irregular_haystack(HAYSTACK_BYTES);
    let ac = classic_ac_compile(PATTERNS);
    let pattern_lengths = pattern_lengths()?;
    let candidate_end_mask = classic_ac_candidate_end_byte_mask_words(&ac.dfa);
    let candidate_suffix2_mask = classic_ac_candidate_suffix2_mask_words(&ac.dfa);
    let candidate_suffix3_bloom = classic_ac_candidate_suffix3_bloom_words(PATTERNS);
    let reset_program = u32_counter_reset_program("match_count");

    let baseline_start = std::time::Instant::now();
    let expected_matches = cpu_aho_overlapping_matches(PATTERNS, &haystack)?;
    let baseline_wall_ns = baseline_start
        .elapsed()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64;
    if expected_matches.len() > MAX_MATCHES as usize {
        return Err(BenchError::EnvironmentInvalid(format!(
            "irregular AC scan fixture produced {} matches, above MAX_MATCHES={MAX_MATCHES}. Fix: lower fixture density or raise output capacity.",
            expected_matches.len()
        )));
    }
    let expected_match_count = expected_matches.len() as u32;
    let program = try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(
        &ac.dfa,
        pattern_lengths.len() as u32,
        MAX_MATCHES,
        false,
    )
    .map_err(BenchError::ExecutionFailed)
    .and_then(|program| with_matches_readback_range(program, expected_match_count))?;

    let inputs = scan_ac_inputs(
        &ac,
        &pattern_lengths,
        &haystack,
        &candidate_end_mask,
        &candidate_suffix2_mask,
        &candidate_suffix3_bloom,
    );
    let input_bytes_total = input_bytes_total(&inputs);
    let resident_output_sizes = [match_triples_output_bytes(MAX_MATCHES)?];
    let resident = ctx
        .map(|ctx| {
            ResidentInputSet::upload_with_zeroed_outputs_optional(
                ctx,
                &inputs,
                &resident_output_sizes,
                "irregular AC scan",
            )
        })
        .transpose()?
        .flatten();
    let baseline_outputs = vec![
        pack_u32_slice(&[expected_match_count]),
        encode_match_triples(&expected_matches),
    ];
    let stats = ScanAcStats {
        haystack_bytes: HAYSTACK_BYTES as u32,
        packed_haystack_words: HAYSTACK_BYTES.div_ceil(4) as u32,
        patterns: PATTERNS.len() as u32,
        dfa_states: ac.dfa.state_count,
        max_pattern_len: ac.dfa.max_pattern_len,
        output_records: ac.dfa.output_records.len() as u32,
        expected_matches: expected_match_count,
        max_matches: MAX_MATCHES,
        planted_matches,
        candidate_end_bytes: count::candidate_end_byte_count(&candidate_end_mask),
        candidate_end_lanes: count::candidate_end_lane_count(&haystack, &candidate_end_mask),
        candidate_suffix2_lanes: count::candidate_suffix2_lane_count(
            &haystack,
            &candidate_end_mask,
            &candidate_suffix2_mask,
        ),
        candidate_suffix3_lanes: count::candidate_suffix3_lane_count(
            &haystack,
            &candidate_end_mask,
            &candidate_suffix2_mask,
            &candidate_suffix3_bloom,
        ),
    };

    Ok(ScanAcIrregularPrepared {
        program,
        reset_program,
        inputs,
        input_bytes_total,
        baseline_outputs,
        baseline_wall_ns,
        stats,
        resident,
    })
}

fn scan_ac_inputs(
    ac: &ClassicAcAutomaton,
    pattern_lengths: &[u32],
    haystack: &[u8],
    candidate_end_mask: &[u32; 8],
    candidate_suffix2_mask: &[u32; CLASSIC_AC_SUFFIX2_MASK_WORDS],
    candidate_suffix3_bloom: &[u32],
) -> Vec<Vec<u8>> {
    vec![
        pack_haystack_u32(haystack),
        pack_u32_slice(&ac.dfa.transitions),
        pack_u32_slice(&ac.dfa.output_offsets),
        pack_u32_slice(&ac.dfa.output_records),
        pack_u32_slice(pattern_lengths),
        pack_u32_slice(&[haystack.len() as u32]),
        pack_u32_slice(&[0]),
        pack_u32_slice(candidate_end_mask),
        pack_u32_slice(candidate_suffix2_mask),
        pack_u32_slice(candidate_suffix3_bloom),
    ]
}

struct ScanResidentSequenceRun {
    outputs: Vec<Vec<u8>>,
    wall_ns: u64,
}

fn dispatch_resident_scan_sequence(
    ctx: &BenchContext,
    prepared: &ScanAcIrregularPrepared,
    resident: &ResidentInputSet,
    workgroup: [u32; 3],
) -> Result<ScanResidentSequenceRun, BenchError> {
    if let Some(override_workgroup) = ctx.dispatch_config.workgroup_override {
        if override_workgroup != workgroup {
            return Err(BenchError::ExecutionFailed(format!(
                "irregular AC scan resident sequence uses program workgroup {:?}, but received override {:?}. Fix: run the resident scan sequence without a workgroup override or rebuild the resident sequence program.",
                workgroup, override_workgroup
            )));
        }
    }

    let reset_resources = resident
        .resources_for_indices(&RESET_RESOURCE_INDICES, "irregular AC scan reset sequence")?;
    let scan_resources =
        resident.resources_for_indices(&SCAN_RESOURCE_INDICES, "irregular AC scan sequence")?;
    let reset_step = ResidentDispatchStep {
        program: &prepared.reset_program,
        resources: &reset_resources,
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let scan_step = ResidentDispatchStep {
        program: &prepared.program,
        resources: &scan_resources,
        grid_override: Some([
            prepared.stats.haystack_bytes.div_ceil(workgroup[0]).max(1),
            1,
            1,
        ]),
        workgroup_override: None,
    };
    let match_output_bytes = match_triples_readback_bytes(prepared.stats.expected_matches)?;
    let read_ranges = [
        ResidentReadRange {
            resource: &scan_resources[MATCH_COUNT_INPUT_INDEX],
            byte_offset: 0,
            byte_len: prepared.baseline_outputs[0].len(),
        },
        ResidentReadRange {
            resource: &scan_resources[MATCHES_RESOURCE_INDEX],
            byte_offset: 0,
            byte_len: match_output_bytes,
        },
    ];

    let mut count_output = Vec::with_capacity(prepared.baseline_outputs[0].len());
    let mut matches_output = Vec::with_capacity(match_output_bytes);
    let started = Instant::now();
    ctx.preferred_backend
        .dispatch_resident_sequence_read_ranges_into(
            &[reset_step, scan_step],
            &read_ranges,
            &mut [&mut count_output, &mut matches_output],
        )
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
    let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

    Ok(ScanResidentSequenceRun {
        outputs: vec![count_output, matches_output],
        wall_ns,
    })
}

inventory::submit! {
    &ScanAcIrregularLiterals as &'static dyn BenchCase
}
