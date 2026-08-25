//! `regex.backtracking.adversarial`  -  Catastrophic backtracking regex.
//!
//! Pattern `(a+)+b` against hostile inputs of repeated 'a's. CPU regex engines
//! with backtracking go superlinear (O(2^n)). GPU parallelism should dominate
//! by evaluating all NFA states simultaneously.

use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PerformanceContract, PreparedCase, WorkloadClass,
};
use crate::api::metric::BenchMetrics;
use crate::api::resident::{
    dispatch_program_timed, input_bytes_total, transfer_accounting, ResidentInputSet,
};
use crate::api::suite::SuiteKind;
use pcre2::bytes::{Regex, RegexBuilder};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Input size: 1024 bytes of 'a' per instance, 4096 instances.
/// Each GPU thread evaluates whether the pattern matches its input slice.
const INPUT_LEN: u32 = 256; // words (1024 bytes)
const INSTANCE_COUNT: u32 = 4096;
const TOTAL_WORDS: u32 = INPUT_LEN * INSTANCE_COUNT;

const HONEST_SUITES: &[SuiteKind] = &[
    SuiteKind::Honest,
    SuiteKind::Deep,
    SuiteKind::Release,
    SuiteKind::Smoke,
];

pub struct RegexBacktracking;

struct RegexBacktrackingPrepared {
    program: Program,
    regex: Regex,
    input_bytes: Vec<u8>,
    inputs: Vec<Vec<u8>>,
    input_bytes_total: u64,
    resident: Option<ResidentInputSet>,
}

impl BenchCase for RegexBacktracking {
    fn id(&self) -> BenchId {
        BenchId("regex.backtracking.adversarial".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Regex Backtracking Adversarial".to_string(),
            description: "Catastrophic backtracking: (a+)+b pattern on hostile 'aaaa...' input"
                .to_string(),
            tags: vec![
                "honest".to_string(),
                "regex".to_string(),
                "adversarial".to_string(),
            ],
            layer: BenchLayer::Honest,
            workload: WorkloadClass::Honest,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-bench".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        HONEST_SUITES
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: Some((TOTAL_WORDS as u64 + INSTANCE_COUNT as u64) * 4),
            min_input_bytes: None,
            feature_set: vec![],
        }
    }

    fn performance_contract(&self) -> Option<PerformanceContract> {
        Some(PerformanceContract::cpu_sota_3x(
            "Catastrophic backtracking regex",
            "pcre2",
            "PCRE2 10.44 (backtracking engine)",
        ))
    }

    fn prepare(&self, ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        // GPU kernel: NFA-style parallel state evaluation.
        // For pattern (a+)+b, the NFA has states:
        //   S0: start  -  on 'a' go to S1
        //   S1: in (a+)  -  on 'a' stay in S1, on 'b' go to S2 (accept)
        //   S2: accept
        //
        // Each thread scans its input slice byte-by-byte in a loop.
        // Since all inputs are 'aaa...' with no 'b', result is always 0 (no match).
        // The work is in the scanning, not the result.
        //
        // The GPU does this honestly: it walks every byte. The advantage is
        // parallelism across instances, not algorithm tricks.
        let prog = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(TOTAL_WORDS),
                BufferDecl::output("results", 1, DataType::U32).with_count(INSTANCE_COUNT),
            ],
            [256, 1, 1],
            vec![
                Node::let_bind("tid", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("tid"), Expr::u32(INSTANCE_COUNT)),
                    vec![
                        Node::let_bind("base", Expr::mul(Expr::var("tid"), Expr::u32(INPUT_LEN))),
                        // NFA state: 0=start, 1=in_a_group, 2=matched
                        Node::let_bind("state", Expr::u32(0)),
                        Node::let_bind("match_count", Expr::u32(0)),
                        // Scan each word
                        Node::Loop {
                            var: "i".into(),
                            from: Expr::u32(0),
                            to: Expr::u32(INPUT_LEN),
                            body: vec![
                                Node::let_bind(
                                    "word",
                                    Expr::load(
                                        "input",
                                        Expr::add(Expr::var("base"), Expr::var("i")),
                                    ),
                                ),
                                // Extract each byte from the word and process
                                // Byte 0
                                Node::let_bind(
                                    "b0",
                                    Expr::bitand(Expr::var("word"), Expr::u32(0xFF)),
                                ),
                                // 'a' = 0x61, 'b' = 0x62
                                Node::if_then(
                                    Expr::eq(Expr::var("b0"), Expr::u32(0x61)),
                                    vec![Node::assign("state", Expr::u32(1))],
                                ),
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("b0"), Expr::u32(0x62)),
                                        Expr::eq(Expr::var("state"), Expr::u32(1)),
                                    ),
                                    vec![
                                        Node::assign(
                                            "match_count",
                                            Expr::add(Expr::var("match_count"), Expr::u32(1)),
                                        ),
                                        Node::assign("state", Expr::u32(0)),
                                    ],
                                ),
                                // Byte 1
                                Node::let_bind(
                                    "b1",
                                    Expr::bitand(
                                        Expr::shr(Expr::var("word"), Expr::u32(8)),
                                        Expr::u32(0xFF),
                                    ),
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("b1"), Expr::u32(0x61)),
                                    vec![Node::assign("state", Expr::u32(1))],
                                ),
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("b1"), Expr::u32(0x62)),
                                        Expr::eq(Expr::var("state"), Expr::u32(1)),
                                    ),
                                    vec![
                                        Node::assign(
                                            "match_count",
                                            Expr::add(Expr::var("match_count"), Expr::u32(1)),
                                        ),
                                        Node::assign("state", Expr::u32(0)),
                                    ],
                                ),
                                // Byte 2
                                Node::let_bind(
                                    "b2",
                                    Expr::bitand(
                                        Expr::shr(Expr::var("word"), Expr::u32(16)),
                                        Expr::u32(0xFF),
                                    ),
                                ),
                                Node::if_then(
                                    Expr::eq(Expr::var("b2"), Expr::u32(0x61)),
                                    vec![Node::assign("state", Expr::u32(1))],
                                ),
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("b2"), Expr::u32(0x62)),
                                        Expr::eq(Expr::var("state"), Expr::u32(1)),
                                    ),
                                    vec![
                                        Node::assign(
                                            "match_count",
                                            Expr::add(Expr::var("match_count"), Expr::u32(1)),
                                        ),
                                        Node::assign("state", Expr::u32(0)),
                                    ],
                                ),
                                // Byte 3
                                Node::let_bind("b3", Expr::shr(Expr::var("word"), Expr::u32(24))),
                                Node::if_then(
                                    Expr::eq(Expr::var("b3"), Expr::u32(0x61)),
                                    vec![Node::assign("state", Expr::u32(1))],
                                ),
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("b3"), Expr::u32(0x62)),
                                        Expr::eq(Expr::var("state"), Expr::u32(1)),
                                    ),
                                    vec![
                                        Node::assign(
                                            "match_count",
                                            Expr::add(Expr::var("match_count"), Expr::u32(1)),
                                        ),
                                        Node::assign("state", Expr::u32(0)),
                                    ],
                                ),
                            ],
                        },
                        Node::store("results", Expr::var("tid"), Expr::var("match_count")),
                    ],
                ),
            ],
        );
        let regex = RegexBuilder::new()
            .jit(true)
            .build(r"(a+)+b")
            .map_err(|error| BenchError::ExecutionFailed(error.to_string()))?;
        let input_bytes = vec![0x61u8; TOTAL_WORDS as usize * 4];
        let inputs = vec![input_bytes.clone()];
        let input_bytes_total = input_bytes_total(&inputs);
        let resident = ResidentInputSet::upload_optional(ctx, &inputs, "regex backtracking bench")?;
        Ok(Box::new(RegexBacktrackingPrepared {
            program: prog,
            regex,
            input_bytes,
            inputs,
            input_bytes_total,
            resident,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a Program> {
        prepared
            .downcast_ref::<RegexBacktrackingPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<RegexBacktrackingPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "regex backtracking prepared payload type mismatch".to_string(),
                )
            })?;

        let dispatch = dispatch_program_timed(
            ctx,
            &prepared.program,
            prepared.resident.as_ref(),
            &prepared.inputs,
            &ctx.dispatch_config,
        )?;
        let resident_used = dispatch.resident_used;
        let timed = dispatch.timed;
        let outputs = timed.outputs;

        // CPU baseline: PCRE2 backtracking engine on the exact same hostile corpus.
        let start_ref = std::time::Instant::now();
        let cpu_results = cpu_pcre2_scan(
            &prepared.regex,
            &prepared.input_bytes,
            INSTANCE_COUNT as usize,
            INPUT_LEN as usize * 4,
        )?;
        let elapsed_ref = start_ref.elapsed().as_nanos() as u64;
        let output_bytes = outputs.iter().map(Vec::len).sum::<usize>() as u64;
        let accounting =
            transfer_accounting(prepared.input_bytes_total, output_bytes, resident_used);

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: timed.device_ns,
                input_bytes: Some(prepared.input_bytes_total),
                output_bytes: Some(output_bytes),
                bytes_read: Some(accounting.bytes_read),
                bytes_written: Some(accounting.bytes_written),
                bytes_touched: Some(accounting.bytes_touched),
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(elapsed_ref),
                input_bytes: Some(prepared.input_bytes.len() as u64),
                output_bytes: Some(cpu_results.len() as u64),
                ..Default::default()
            }),
            outputs,
            baseline_outputs: Some(vec![cpu_results]),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

/// CPU PCRE2 scan  -  the advertised backtracking baseline, not a custom NFA.
fn cpu_pcre2_scan(
    regex: &Regex,
    input: &[u8],
    instances: usize,
    bytes_per: usize,
) -> Result<Vec<u8>, BenchError> {
    let mut results = vec![0u32; instances];
    for instance in 0..instances {
        let base = instance * bytes_per;
        let haystack = &input[base..base + bytes_per];
        let mut matches = regex
            .find_iter(haystack)
            .map(|item| item.map(|_| 1u32))
            .try_fold(0u32, |count, item| {
                item.map(|matched| count + matched)
                    .map_err(|error| BenchError::ExecutionFailed(error.to_string()))
            })?;
        if matches > 0 {
            matches = 1;
        }
        results[instance] = matches;
    }
    Ok(vyre_primitives::wire::pack_u32_slice(&results))
}

inventory::submit! {
    &RegexBacktracking as &'static dyn BenchCase
}
