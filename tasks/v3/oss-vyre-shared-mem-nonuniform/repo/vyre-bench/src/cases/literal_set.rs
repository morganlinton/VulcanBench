use std::time::Instant;

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::resident::{transfer_accounting, u32_counter_reset_program, ResidentInputSet};
use crate::api::suite::SuiteKind;
use crate::cases::scan_ac_irregular::support::{build_irregular_haystack, encode_match_triples};
use crate::cases::scan_ac_irregular::PATTERNS;
use vyre_driver::{ResidentDispatchStep, ResidentReadRange};
use vyre_foundation::ir::Program;
use vyre_foundation::match_result::Match;
use vyre_libs::scan::classic_ac::{CLASSIC_AC_SUFFIX2_MASK_WORDS, CLASSIC_AC_SUFFIX3_BLOOM_WORDS};
use vyre_libs::scan::{
    GpuLiteralSet, LiteralSetPreparedCount, LiteralSetPreparedScan, LiteralSetScanScratch,
    LITERAL_SET_COUNT_RESET_RESOURCE_INDICES, LITERAL_SET_COUNT_RESOURCE_INDEX,
    LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES, LITERAL_SET_MATCHES_RESOURCE_INDEX,
    LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX, LITERAL_SET_RESET_RESOURCE_INDICES,
    LITERAL_SET_SCAN_RESOURCE_INDICES,
};

const HAYSTACK_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_LITERAL_SET_MATCH_CAP: u32 = 10_000;
const MATCH_TRIPLE_BYTES: u64 = 12;
const LITERAL_MICROBENCH_STRATIFICATION_SCHEMA_VERSION: u32 = 1;
const LITERAL_MICROBENCH_CHUNK_BYTES: usize = 32;
const STRATIFICATION_BASIS_POINTS: u32 = 10_000;
const FNV64_OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0100_0000_01B3;
const SUITES: &[SuiteKind] = &[
    SuiteKind::Smoke,
    SuiteKind::Release,
    SuiteKind::Gpu,
    SuiteKind::Deep,
    SuiteKind::Honest,
];

pub struct LiteralSetIrregularHotloop;
pub struct LiteralSetIrregularCountHotloop;

struct LiteralSetIrregularPrepared {
    engine: GpuLiteralSet,
    haystack: Vec<u8>,
    matches: Vec<Match>,
    scratch: LiteralSetScanScratch,
    prepared_scan: LiteralSetPreparedScan,
    reset_program: Program,
    resident: Option<ResidentInputSet>,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall_ns: u64,
    expected_matches: u32,
    max_matches: u32,
    planted_matches: u32,
    stratification: LiteralMicrobenchStratification,
    encoded_input_bytes: u64,
    output_bytes: u64,
}

struct LiteralSetIrregularCountPrepared {
    engine: GpuLiteralSet,
    haystack: Vec<u8>,
    scratch: LiteralSetScanScratch,
    prepared_count: LiteralSetPreparedCount,
    reset_program: Program,
    resident: Option<ResidentInputSet>,
    baseline_output: Vec<u8>,
    baseline_wall_ns: u64,
    expected_matches: u32,
    planted_matches: u32,
    stratification: LiteralMicrobenchStratification,
    encoded_input_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LiteralMicrobenchStratification {
    pub schema_version: u32,
    pub pattern_count: u32,
    pub min_needle_len: u32,
    pub max_needle_len: u32,
    pub rare_byte: u8,
    pub rare_byte_position: u32,
    pub alphabet_size: u32,
    pub haystack_entropy_bps: u32,
    pub overlap_density_bps: u32,
    pub chunk_boundary_bytes: u32,
    pub chunk_boundary_crossing_matches: u32,
    pub pattern_digest: u64,
    pub match_digest: u64,
}

impl BenchCase for LiteralSetIrregularHotloop {
    fn id(&self) -> BenchId {
        BenchId("scan.literal_set.irregular_hotloop.4m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "GpuLiteralSet Irregular Hot Loop 4M".to_string(),
            description: "Public GpuLiteralSet prepared-dispatch hot loop over unaligned security/parser literals with resident input reuse when supported".to_string(),
            tags: vec![
                "scan".to_string(),
                "pattern".to_string(),
                "dfa".to_string(),
                "literal-set".to_string(),
                "irregular".to_string(),
                "hot-loop".to_string(),
                "resident".to_string(),
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
                "literal-set".to_string(),
                "public-api-hot-loop".to_string(),
                "resident-prepared-dispatch".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "GpuLiteralSet irregular public scan",
            "vyre-libs",
            "vyre-libs DFA reference_scan",
            1.0,
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<LiteralSetIrregularPrepared>()
            .map(|prepared| (prepared.encoded_input_bytes, prepared.output_bytes))
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let (haystack, planted_matches) = build_irregular_haystack(HAYSTACK_BYTES);
        let engine = GpuLiteralSet::try_compile(PATTERNS).map_err(|error| {
            BenchError::EnvironmentInvalid(format!(
                "literal-set irregular fixture failed to compile: {error}"
            ))
        })?;

        let baseline_start = Instant::now();
        let baseline_matches = engine.reference_scan(&haystack);
        let baseline_wall_ns = baseline_start
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        let expected_matches = u32::try_from(baseline_matches.len()).map_err(|_| {
            BenchError::EnvironmentInvalid(format!(
                "literal-set irregular fixture produced {} matches, above u32 capacity. Fix: lower fixture density or shard the scan.",
                baseline_matches.len()
            ))
        })?;
        let max_matches = expected_matches.max(1);
        let encoded_matches = encode_match_triples(&baseline_matches);
        let output_bytes = 4_u64.saturating_add(encoded_matches.len() as u64);
        let baseline_outputs = vec![expected_matches.to_le_bytes().to_vec(), encoded_matches];
        let stratification =
            literal_microbench_stratification(&haystack, &baseline_matches, planted_matches);
        validate_literal_microbench_stratification(&stratification)?;
        let mut scratch = LiteralSetScanScratch::default();
        engine
            .prepare_literal_scratch(max_matches, &mut scratch)
            .map_err(|error| {
                BenchError::ExecutionFailed(format!(
                    "literal-set irregular hot-loop scratch preparation failed: {error}"
                ))
            })?;
        let prepared_scan = engine
            .prepare_scan_dispatch(&haystack, max_matches)
            .map_err(|error| {
                BenchError::ExecutionFailed(format!(
                    "literal-set irregular prepared dispatch failed: {error}"
                ))
            })?;
        let reset_program = u32_counter_reset_program("match_count");
        let resident = ResidentInputSet::upload_with_zeroed_outputs_optional(
            ctx,
            &prepared_scan.inputs,
            &[prepared_scan.matches_output_bytes],
            "literal-set irregular hot-loop",
        )?;
        let encoded_input_bytes = prepared_scan.encoded_input_bytes;

        Ok(Box::new(LiteralSetIrregularPrepared {
            engine,
            haystack,
            matches: Vec::with_capacity(expected_matches as usize),
            scratch,
            prepared_scan,
            reset_program,
            resident,
            baseline_outputs,
            baseline_wall_ns,
            expected_matches,
            max_matches,
            planted_matches,
            stratification,
            encoded_input_bytes,
            output_bytes,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<LiteralSetIrregularPrepared>()
            .map(|prepared| &prepared.prepared_scan.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_mut::<LiteralSetIrregularPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared literal-set irregular payload had the wrong type".to_string(),
                )
            })?;

        let (outputs, wall_ns, resident_used, device_reset_sequence) =
            if let Some(resident) = prepared.resident.as_ref() {
                let started = Instant::now();
                let sequence = dispatch_literal_set_resident_sequence(ctx, prepared, resident)?;
                prepared
                    .prepared_scan
                    .decode_outputs_into(&sequence.outputs, &mut prepared.matches)
                    .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
                let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                let encoded_matches = encode_match_triples(&prepared.matches);
                (
                    vec![
                        (prepared.matches.len() as u32).to_le_bytes().to_vec(),
                        encoded_matches,
                    ],
                    wall_ns,
                    true,
                    true,
                )
            } else {
                let started = Instant::now();
                prepared
                    .engine
                    .scan_into_with_literal_scratch(
                        ctx.preferred_backend.as_ref(),
                        &prepared.haystack,
                        prepared.max_matches,
                        &mut prepared.matches,
                        &mut prepared.scratch,
                    )
                    .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
                let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                let encoded_matches = encode_match_triples(&prepared.matches);
                (
                    vec![
                        (prepared.matches.len() as u32).to_le_bytes().to_vec(),
                        encoded_matches,
                    ],
                    wall_ns,
                    false,
                    false,
                )
            };
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting =
            transfer_accounting(prepared.encoded_input_bytes, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns: None,
                input_bytes: Some(prepared.encoded_input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: literal_set_metric_points(
                    prepared,
                    wall_ns,
                    resident_used,
                    device_reset_sequence,
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.haystack.len() as u64),
                output_bytes: Some(prepared.output_bytes),
                custom: vec![metric(
                    "literal_set_irregular_reference_matches",
                    u64::from(prepared.expected_matches),
                )],
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(prepared.baseline_outputs.clone()),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

impl BenchCase for LiteralSetIrregularCountHotloop {
    fn id(&self) -> BenchId {
        BenchId("scan.literal_set.irregular_count_hotloop.4m".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "GpuLiteralSet Irregular Count Hot Loop 4M".to_string(),
            description: "Public GpuLiteralSet count-only prepared dispatch over unaligned security/parser literals with resident input reuse when supported".to_string(),
            tags: vec![
                "scan".to_string(),
                "pattern".to_string(),
                "dfa".to_string(),
                "literal-set".to_string(),
                "count-only".to_string(),
                "irregular".to_string(),
                "hot-loop".to_string(),
                "resident".to_string(),
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
                "literal-set".to_string(),
                "count-only".to_string(),
                "public-api-hot-loop".to_string(),
                "resident-prepared-dispatch".to_string(),
            ],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_min_speedup(
            "GpuLiteralSet irregular count-only public scan",
            "vyre-libs",
            "vyre-libs DFA reference_scan count",
            1.0,
        ))
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<LiteralSetIrregularCountPrepared>()
            .map(|prepared| {
                (
                    prepared.encoded_input_bytes,
                    prepared.baseline_output.len() as u64,
                )
            })
            .unwrap_or((0, 0))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let (haystack, planted_matches) = build_irregular_haystack(HAYSTACK_BYTES);
        let engine = GpuLiteralSet::try_compile(PATTERNS).map_err(|error| {
            BenchError::EnvironmentInvalid(format!(
                "literal-set irregular count fixture failed to compile: {error}"
            ))
        })?;

        let baseline_start = Instant::now();
        let baseline_matches = engine.reference_scan(&haystack);
        let expected_matches = u32::try_from(baseline_matches.len()).map_err(|_| {
            BenchError::EnvironmentInvalid(format!(
                "literal-set irregular count fixture produced {} matches, above u32 capacity. Fix: lower fixture density or shard the count scan.",
                baseline_matches.len()
            ))
        })?;
        let stratification =
            literal_microbench_stratification(&haystack, &baseline_matches, planted_matches);
        validate_literal_microbench_stratification(&stratification)?;
        let baseline_wall_ns = baseline_start
            .elapsed()
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        let prepared_count = engine.prepare_count_dispatch(&haystack).map_err(|error| {
            BenchError::ExecutionFailed(format!(
                "literal-set irregular prepared count dispatch failed: {error}"
            ))
        })?;
        let reset_program = u32_counter_reset_program("match_count");
        let resident = ResidentInputSet::upload_optional(
            ctx,
            &prepared_count.inputs,
            "literal-set irregular count hot-loop",
        )?;
        let encoded_input_bytes = prepared_count.encoded_input_bytes;

        Ok(Box::new(LiteralSetIrregularCountPrepared {
            engine,
            haystack,
            scratch: LiteralSetScanScratch::default(),
            prepared_count,
            reset_program,
            resident,
            baseline_output: expected_matches.to_le_bytes().to_vec(),
            baseline_wall_ns,
            expected_matches,
            planted_matches,
            stratification,
            encoded_input_bytes,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<LiteralSetIrregularCountPrepared>()
            .map(|prepared| &prepared.prepared_count.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_mut::<LiteralSetIrregularCountPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "prepared literal-set irregular count payload had the wrong type".to_string(),
                )
            })?;

        let (outputs, wall_ns, resident_used, device_reset_sequence) = if let Some(resident) =
            prepared.resident.as_ref()
        {
            let sequence = dispatch_literal_set_count_resident_sequence(ctx, prepared, resident)?;
            (sequence.outputs, sequence.wall_ns, true, true)
        } else {
            let started = Instant::now();
            let count = prepared
                .engine
                .count_with_literal_scratch(
                    ctx.preferred_backend.as_ref(),
                    &prepared.haystack,
                    &mut prepared.scratch,
                )
                .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
            let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
            (vec![count.to_le_bytes().to_vec()], wall_ns, false, false)
        };

        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting =
            transfer_accounting(prepared.encoded_input_bytes, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(wall_ns),
                dispatch_ns: None,
                input_bytes: Some(prepared.encoded_input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                custom: literal_set_count_metric_points(
                    prepared,
                    wall_ns,
                    resident_used,
                    device_reset_sequence,
                ),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(prepared.haystack.len() as u64),
                output_bytes: Some(prepared.baseline_output.len() as u64),
                custom: vec![metric(
                    "literal_set_irregular_count_reference_matches",
                    u64::from(prepared.expected_matches),
                )],
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![prepared.baseline_output.clone()]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

fn literal_microbench_stratification(
    haystack: &[u8],
    matches: &[Match],
    planted_matches: u32,
) -> LiteralMicrobenchStratification {
    let mut pattern_byte_counts = [0_u32; 256];
    let mut min_needle_len = u32::MAX;
    let mut max_needle_len = 0_u32;
    for pattern in PATTERNS {
        let len = pattern.len() as u32;
        min_needle_len = min_needle_len.min(len);
        max_needle_len = max_needle_len.max(len);
        for byte in *pattern {
            pattern_byte_counts[usize::from(*byte)] += 1;
        }
    }

    let mut rare_byte = 0_u8;
    let mut rare_byte_count = u32::MAX;
    for (byte, count) in pattern_byte_counts.iter().enumerate() {
        if *count > 0
            && (*count < rare_byte_count
                || (*count == rare_byte_count && byte < usize::from(rare_byte)))
        {
            rare_byte = byte as u8;
            rare_byte_count = *count;
        }
    }

    let expected_matches = u32::try_from(matches.len()).unwrap_or(u32::MAX);
    let overlap_matches = expected_matches.saturating_sub(planted_matches);
    let overlap_density_bps = if expected_matches == 0 {
        0
    } else {
        ((u64::from(overlap_matches) * u64::from(STRATIFICATION_BASIS_POINTS))
            / u64::from(expected_matches))
        .min(u64::from(STRATIFICATION_BASIS_POINTS)) as u32
    };
    let chunk_boundary_crossing_matches = u32::try_from(
        matches
            .iter()
            .filter(|hit| crosses_chunk_boundary(hit.start, hit.end))
            .count(),
    )
    .unwrap_or(u32::MAX);
    let pattern_digest = literal_pattern_digest();

    LiteralMicrobenchStratification {
        schema_version: LITERAL_MICROBENCH_STRATIFICATION_SCHEMA_VERSION,
        pattern_count: PATTERNS.len() as u32,
        min_needle_len: if min_needle_len == u32::MAX {
            0
        } else {
            min_needle_len
        },
        max_needle_len,
        rare_byte,
        rare_byte_position: first_needle_position(rare_byte),
        alphabet_size: pattern_byte_counts
            .iter()
            .filter(|count| **count > 0)
            .count() as u32,
        haystack_entropy_bps: haystack_entropy_bps(haystack),
        overlap_density_bps,
        chunk_boundary_bytes: LITERAL_MICROBENCH_CHUNK_BYTES as u32,
        chunk_boundary_crossing_matches,
        pattern_digest,
        match_digest: literal_match_digest(matches, pattern_digest),
    }
}

fn validate_literal_microbench_stratification(
    metadata: &LiteralMicrobenchStratification,
) -> Result<(), BenchError> {
    if metadata.schema_version != LITERAL_MICROBENCH_STRATIFICATION_SCHEMA_VERSION {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification schema version {} did not match expected {}. Fix: regenerate the literal microbench metadata with the current schema.",
            metadata.schema_version, LITERAL_MICROBENCH_STRATIFICATION_SCHEMA_VERSION
        )));
    }
    if metadata.pattern_count != PATTERNS.len() as u32 {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded {} patterns but fixture owns {}. Fix: rebuild metadata from PATTERNS in the benchmark fixture.",
            metadata.pattern_count,
            PATTERNS.len()
        )));
    }
    if metadata.min_needle_len == 0 || metadata.max_needle_len < metadata.min_needle_len {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded invalid needle lengths min={} max={}. Fix: record min/max from the literal fixture before dispatch.",
            metadata.min_needle_len, metadata.max_needle_len
        )));
    }
    if metadata.alphabet_size == 0 || metadata.alphabet_size > 256 {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded invalid alphabet size {}. Fix: derive alphabet coverage from literal bytes.",
            metadata.alphabet_size
        )));
    }
    if metadata.haystack_entropy_bps == 0
        || metadata.haystack_entropy_bps > STRATIFICATION_BASIS_POINTS
    {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded invalid haystack entropy {} bps. Fix: derive entropy from the generated haystack bytes.",
            metadata.haystack_entropy_bps
        )));
    }
    if metadata.overlap_density_bps > STRATIFICATION_BASIS_POINTS {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded invalid overlap density {} bps. Fix: bound overlap density to the basis-point scale.",
            metadata.overlap_density_bps
        )));
    }
    if metadata.chunk_boundary_bytes != LITERAL_MICROBENCH_CHUNK_BYTES as u32
        || metadata.chunk_boundary_crossing_matches == 0
    {
        return Err(BenchError::EnvironmentInvalid(format!(
            "literal microbench stratification recorded chunk_bytes={} crossing_matches={}. Fix: record boundary-crossing matches for the configured chunk size.",
            metadata.chunk_boundary_bytes, metadata.chunk_boundary_crossing_matches
        )));
    }
    if metadata.pattern_digest == 0 || metadata.match_digest == 0 {
        return Err(BenchError::EnvironmentInvalid(
            "literal microbench stratification omitted pattern or match digest. Fix: hash the literal metadata and exact match triples before reporting metrics."
                .to_string(),
        ));
    }
    Ok(())
}

fn first_needle_position(byte: u8) -> u32 {
    for pattern in PATTERNS {
        if let Some(position) = pattern.iter().position(|candidate| *candidate == byte) {
            return position as u32;
        }
    }
    0
}

fn haystack_entropy_bps(haystack: &[u8]) -> u32 {
    if haystack.is_empty() {
        return 0;
    }

    let mut counts = [0_u64; 256];
    for byte in haystack {
        counts[usize::from(*byte)] += 1;
    }
    let len = haystack.len() as f64;
    let mut entropy_bits = 0.0_f64;
    for count in counts {
        if count > 0 {
            let probability = count as f64 / len;
            entropy_bits -= probability * probability.log2();
        }
    }
    ((entropy_bits / 8.0) * f64::from(STRATIFICATION_BASIS_POINTS))
        .round()
        .clamp(0.0, f64::from(STRATIFICATION_BASIS_POINTS)) as u32
}

fn crosses_chunk_boundary(start: u32, end: u32) -> bool {
    if end <= start {
        return false;
    }
    let start_chunk = start as usize / LITERAL_MICROBENCH_CHUNK_BYTES;
    let last_chunk = end.saturating_sub(1) as usize / LITERAL_MICROBENCH_CHUNK_BYTES;
    start_chunk != last_chunk
}

fn literal_pattern_digest() -> u64 {
    let mut digest = fnv64_u64(FNV64_OFFSET_BASIS, PATTERNS.len() as u64);
    for (index, pattern) in PATTERNS.iter().enumerate() {
        digest = fnv64_u64(digest, index as u64);
        digest = fnv64_u64(digest, pattern.len() as u64);
        digest = fnv64_bytes(digest, pattern);
    }
    digest
}

fn literal_match_digest(matches: &[Match], pattern_digest: u64) -> u64 {
    let mut digest = fnv64_u64(FNV64_OFFSET_BASIS, pattern_digest);
    digest = fnv64_u64(digest, matches.len() as u64);
    for hit in matches {
        digest = fnv64_u64(digest, u64::from(hit.pattern_id));
        digest = fnv64_u64(digest, u64::from(hit.start));
        digest = fnv64_u64(digest, u64::from(hit.end));
    }
    digest
}

fn fnv64_u64(digest: u64, value: u64) -> u64 {
    fnv64_bytes(digest, &value.to_le_bytes())
}

fn fnv64_bytes(mut digest: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(FNV64_PRIME);
    }
    digest
}

fn extend_literal_stratification_metrics(
    metrics: &mut Vec<MetricPoint>,
    prefix: &str,
    metadata: LiteralMicrobenchStratification,
) {
    metrics.extend([
        prefixed_metric(
            prefix,
            "stratification_schema_version",
            u64::from(metadata.schema_version),
        ),
        prefixed_metric(
            prefix,
            "stratified_pattern_count",
            u64::from(metadata.pattern_count),
        ),
        prefixed_metric(prefix, "min_needle_len", u64::from(metadata.min_needle_len)),
        prefixed_metric(prefix, "max_needle_len", u64::from(metadata.max_needle_len)),
        prefixed_metric(prefix, "rare_byte", u64::from(metadata.rare_byte)),
        prefixed_metric(
            prefix,
            "rare_byte_position",
            u64::from(metadata.rare_byte_position),
        ),
        prefixed_metric(prefix, "alphabet_size", u64::from(metadata.alphabet_size)),
        prefixed_metric(
            prefix,
            "haystack_entropy_bps",
            u64::from(metadata.haystack_entropy_bps),
        ),
        prefixed_metric(
            prefix,
            "overlap_density_bps",
            u64::from(metadata.overlap_density_bps),
        ),
        prefixed_metric(
            prefix,
            "chunk_boundary_bytes",
            u64::from(metadata.chunk_boundary_bytes),
        ),
        prefixed_metric(
            prefix,
            "chunk_boundary_crossing_matches",
            u64::from(metadata.chunk_boundary_crossing_matches),
        ),
        prefixed_metric(prefix, "pattern_digest", metadata.pattern_digest),
        prefixed_metric(prefix, "match_digest", metadata.match_digest),
    ]);
}

fn prefixed_metric(prefix: &str, suffix: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: format!("{prefix}_{suffix}"),
        value,
    }
}

fn literal_set_metric_points(
    prepared: &LiteralSetIrregularPrepared,
    wall_ns: u64,
    resident_used: bool,
    device_reset_sequence: bool,
) -> Vec<MetricPoint> {
    let avoided_default_matches =
        DEFAULT_LITERAL_SET_MATCH_CAP.saturating_sub(prepared.max_matches);
    let mut metrics = vec![
        metric(
            "literal_set_irregular_haystack_bytes",
            prepared.haystack.len() as u64,
        ),
        metric("literal_set_irregular_patterns", PATTERNS.len() as u64),
        metric(
            "literal_set_irregular_pattern_bytes",
            prepared.engine.pattern_bytes.len() as u64,
        ),
        metric(
            "literal_set_irregular_dfa_states",
            u64::from(prepared.engine.dfa.state_count),
        ),
        metric(
            "literal_set_irregular_dfa_table_bytes",
            ((prepared.engine.dfa.transitions.len()
                + prepared.engine.dfa.output_offsets.len()
                + prepared.engine.dfa.output_records.len()) as u64)
                .saturating_mul(4),
        ),
        metric(
            "literal_set_irregular_dfa_output_records",
            prepared.engine.dfa.output_records.len() as u64,
        ),
        metric(
            "literal_set_irregular_prefilter_mask_bytes",
            ((8 + CLASSIC_AC_SUFFIX2_MASK_WORDS + CLASSIC_AC_SUFFIX3_BLOOM_WORDS) as u64)
                .saturating_mul(4),
        ),
        metric(
            "literal_set_irregular_resident_used",
            u64::from(resident_used),
        ),
        metric(
            "literal_set_irregular_device_reset_sequence",
            u64::from(device_reset_sequence),
        ),
        metric(
            "literal_set_irregular_expected_matches",
            u64::from(prepared.expected_matches),
        ),
        metric(
            "literal_set_irregular_max_matches",
            u64::from(prepared.max_matches),
        ),
        metric(
            "literal_set_irregular_planted_matches",
            u64::from(prepared.planted_matches),
        ),
        metric(
            "literal_set_irregular_cap_specific_scratch_program_cache",
            u64::from(prepared.max_matches != DEFAULT_LITERAL_SET_MATCH_CAP),
        ),
        metric(
            "literal_set_irregular_avoided_default_readback_bytes",
            u64::from(avoided_default_matches).saturating_mul(MATCH_TRIPLE_BYTES),
        ),
    ];
    extend_literal_stratification_metrics(
        &mut metrics,
        "literal_set_irregular",
        prepared.stratification,
    );
    if wall_ns > 0 {
        metrics.push(metric(
            "literal_set_irregular_speedup_x1000",
            (u128::from(prepared.baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        ));
    }
    metrics
}

fn literal_set_count_metric_points(
    prepared: &LiteralSetIrregularCountPrepared,
    wall_ns: u64,
    resident_used: bool,
    device_reset_sequence: bool,
) -> Vec<MetricPoint> {
    let mut metrics = vec![
        metric(
            "literal_set_irregular_count_haystack_bytes",
            prepared.haystack.len() as u64,
        ),
        metric(
            "literal_set_irregular_count_patterns",
            PATTERNS.len() as u64,
        ),
        metric(
            "literal_set_irregular_count_pattern_bytes",
            prepared.engine.pattern_bytes.len() as u64,
        ),
        metric(
            "literal_set_irregular_count_dfa_states",
            u64::from(prepared.engine.dfa.state_count),
        ),
        metric(
            "literal_set_irregular_count_dfa_table_bytes",
            ((prepared.engine.dfa.transitions.len() + prepared.engine.dfa.output_offsets.len())
                as u64)
                .saturating_mul(4),
        ),
        metric(
            "literal_set_irregular_count_prefilter_mask_bytes",
            ((8 + CLASSIC_AC_SUFFIX2_MASK_WORDS + CLASSIC_AC_SUFFIX3_BLOOM_WORDS) as u64)
                .saturating_mul(4),
        ),
        metric(
            "literal_set_irregular_count_resident_used",
            u64::from(resident_used),
        ),
        metric(
            "literal_set_irregular_count_device_reset_sequence",
            u64::from(device_reset_sequence),
        ),
        metric(
            "literal_set_irregular_count_expected_matches",
            u64::from(prepared.expected_matches),
        ),
        metric(
            "literal_set_irregular_count_planted_matches",
            u64::from(prepared.planted_matches),
        ),
    ];
    extend_literal_stratification_metrics(
        &mut metrics,
        "literal_set_irregular_count",
        prepared.stratification,
    );
    if wall_ns > 0 {
        metrics.push(metric(
            "literal_set_irregular_count_speedup_x1000",
            (u128::from(prepared.baseline_wall_ns) * 1000 / u128::from(wall_ns))
                .min(u128::from(u64::MAX)) as u64,
        ));
    }
    metrics
}

fn metric(name: &str, value: u64) -> MetricPoint {
    MetricPoint {
        name: name.to_string(),
        value,
    }
}

struct LiteralSetResidentSequenceRun {
    outputs: Vec<Vec<u8>>,
}

struct LiteralSetCountResidentSequenceRun {
    outputs: Vec<Vec<u8>>,
    wall_ns: u64,
}

fn dispatch_literal_set_resident_sequence(
    ctx: &BenchContext,
    prepared: &LiteralSetIrregularPrepared,
    resident: &ResidentInputSet,
) -> Result<LiteralSetResidentSequenceRun, BenchError> {
    let program_workgroup = prepared.prepared_scan.program.workgroup_size();
    if let Some(override_workgroup) = ctx.dispatch_config.workgroup_override {
        if override_workgroup != program_workgroup {
            return Err(BenchError::ExecutionFailed(format!(
                "literal-set irregular resident sequence uses program workgroup {:?}, but received override {:?}. Fix: run the resident scan sequence without a workgroup override or rebuild the prepared dispatch program.",
                program_workgroup, override_workgroup
            )));
        }
    }

    let reset_resources = resident.resources_for_indices(
        &LITERAL_SET_RESET_RESOURCE_INDICES,
        "literal-set irregular reset sequence",
    )?;
    let scan_resources = resident.resources_for_indices(
        &LITERAL_SET_SCAN_RESOURCE_INDICES,
        "literal-set irregular scan sequence",
    )?;
    let reset_step = ResidentDispatchStep {
        program: &prepared.reset_program,
        resources: &reset_resources,
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let scan_step = ResidentDispatchStep {
        program: &prepared.prepared_scan.program,
        resources: &scan_resources,
        grid_override: prepared.prepared_scan.dispatch_config.grid_override,
        workgroup_override: None,
    };
    let match_output_bytes = prepared
        .prepared_scan
        .match_triples_readback_bytes(prepared.expected_matches)
        .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
    let read_ranges = [
        ResidentReadRange {
            resource: &scan_resources[LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX],
            byte_offset: 0,
            byte_len: prepared.prepared_scan.match_count_readback_bytes(),
        },
        ResidentReadRange {
            resource: &scan_resources[LITERAL_SET_MATCHES_RESOURCE_INDEX],
            byte_offset: 0,
            byte_len: match_output_bytes,
        },
    ];

    let mut count_output = Vec::with_capacity(prepared.prepared_scan.match_count_readback_bytes());
    let mut matches_output = Vec::with_capacity(match_output_bytes);
    ctx.preferred_backend
        .dispatch_resident_sequence_read_ranges_into(
            &[reset_step, scan_step],
            &read_ranges,
            &mut [&mut count_output, &mut matches_output],
        )
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;

    Ok(LiteralSetResidentSequenceRun {
        outputs: vec![count_output, matches_output],
    })
}

fn dispatch_literal_set_count_resident_sequence(
    ctx: &BenchContext,
    prepared: &LiteralSetIrregularCountPrepared,
    resident: &ResidentInputSet,
) -> Result<LiteralSetCountResidentSequenceRun, BenchError> {
    let program_workgroup = prepared.prepared_count.program.workgroup_size();
    if let Some(override_workgroup) = ctx.dispatch_config.workgroup_override {
        if override_workgroup != program_workgroup {
            return Err(BenchError::ExecutionFailed(format!(
                "literal-set irregular count resident sequence uses program workgroup {:?}, but received override {:?}. Fix: run the resident count sequence without a workgroup override or rebuild the prepared dispatch program.",
                program_workgroup, override_workgroup
            )));
        }
    }

    let reset_resources = resident.resources_for_indices(
        &LITERAL_SET_COUNT_RESET_RESOURCE_INDICES,
        "literal-set irregular count reset sequence",
    )?;
    let scan_resources = resident.resources_for_indices(
        &LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES,
        "literal-set irregular count scan sequence",
    )?;
    let reset_step = ResidentDispatchStep {
        program: &prepared.reset_program,
        resources: &reset_resources,
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let scan_step = ResidentDispatchStep {
        program: &prepared.prepared_count.program,
        resources: &scan_resources,
        grid_override: prepared.prepared_count.dispatch_config.grid_override,
        workgroup_override: None,
    };
    let read_ranges = [ResidentReadRange {
        resource: &scan_resources[LITERAL_SET_COUNT_RESOURCE_INDEX],
        byte_offset: 0,
        byte_len: prepared.prepared_count.count_readback_bytes(),
    }];

    let mut count_output = Vec::with_capacity(prepared.prepared_count.count_readback_bytes());
    let started = Instant::now();
    ctx.preferred_backend
        .dispatch_resident_sequence_read_ranges_into(
            &[reset_step, scan_step],
            &read_ranges,
            &mut [&mut count_output],
        )
        .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
    let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

    Ok(LiteralSetCountResidentSequenceRun {
        outputs: vec![count_output],
        wall_ns,
    })
}

inventory::submit! {
    &LiteralSetIrregularHotloop as &'static dyn BenchCase
}

inventory::submit! {
    &LiteralSetIrregularCountHotloop as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_matches() -> [Match; 2] {
        [Match::new(0, 31, 35), Match::new(1, 64, 68)]
    }

    #[test]
    fn literal_microbench_stratification_records_pattern_metadata_and_digest() {
        let haystack = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let matches = sample_matches();
        let metadata = literal_microbench_stratification(haystack, &matches, 1);

        assert_eq!(
            metadata.pattern_count,
            u32::try_from(PATTERNS.len()).unwrap()
        );
        assert_eq!(
            metadata.min_needle_len,
            PATTERNS.iter().map(|pattern| pattern.len()).min().unwrap() as u32
        );
        assert_eq!(
            metadata.max_needle_len,
            PATTERNS.iter().map(|pattern| pattern.len()).max().unwrap() as u32
        );
        assert!(metadata.alphabet_size > 0);
        assert!(metadata.pattern_digest != 0);
        assert!(metadata.match_digest != 0);
        assert_eq!(metadata.chunk_boundary_crossing_matches, 1);
        validate_literal_microbench_stratification(&metadata).unwrap();
    }

    #[test]
    fn literal_microbench_metric_fanout_covers_scan_and_count_cases() {
        let matches = sample_matches();
        let metadata = literal_microbench_stratification(b"abcdefghijklmnopqrstuvwxyz", &matches, 1);
        let mut metrics = Vec::new();

        extend_literal_stratification_metrics(&mut metrics, "literal_set_irregular", metadata);
        extend_literal_stratification_metrics(
            &mut metrics,
            "literal_set_irregular_count",
            metadata,
        );

        assert!(metrics.iter().any(|metric| {
            metric.name == "literal_set_irregular_min_needle_len"
                && metric.value == u64::from(metadata.min_needle_len)
        }));
        assert!(metrics.iter().any(|metric| {
            metric.name == "literal_set_irregular_match_digest"
                && metric.value == metadata.match_digest
        }));
        assert!(metrics.iter().any(|metric| {
            metric.name == "literal_set_irregular_count_haystack_entropy_bps"
                && metric.value == u64::from(metadata.haystack_entropy_bps)
        }));
        assert!(metrics.iter().any(|metric| {
            metric.name == "literal_set_irregular_count_chunk_boundary_crossing_matches"
                && metric.value == u64::from(metadata.chunk_boundary_crossing_matches)
        }));
    }

    #[test]
    fn literal_microbench_stratification_rejects_missing_digest() {
        let matches = sample_matches();
        let mut metadata = literal_microbench_stratification(b"abcdefghijklmnopqrstuvwxyz", &matches, 1);
        metadata.match_digest = 0;

        let error = validate_literal_microbench_stratification(&metadata).unwrap_err();
        assert!(error.to_string().contains("digest"));
    }
}
