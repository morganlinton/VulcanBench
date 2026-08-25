use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use vyre_libs::parsing::rust::lex::lexer::core::lex as lex_cpu;
use vyre_libs::parsing::rust::lex::lexer::plan::rust_lexer_batch;

struct RustLexerBatchGpuPipeline;

const RUST_LEXER_BATCH_SOURCES: usize = 2048;
const WORKGROUP_SIZE: u32 = 256;

struct RustLexerBatchPrepared {
    program: vyre::ir::Program,
    inputs: Vec<Vec<u8>>,
    source_bytes: usize,
    source_count: u32,
    token_stride: usize,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall_ns: u64,
    token_count: usize,
}

impl BenchCase for RustLexerBatchGpuPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.rust.lexer.batch_ir_execute".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Batched Rust GPU Lexer IR Execute".to_string(),
            description:
                "Many small Rust nano-subset sources packed into one GPU lexer dispatch with exact per-source CPU lexer column parity"
                    .to_string(),
            tags: vec![
                "frontend-rust".to_string(),
                "gpu-lexer".to_string(),
                "lexer".to_string(),
                "batch".to_string(),
                "many-source".to_string(),
                "tokenization".to_string(),
                "ir-lexer".to_string(),
                "release".to_string(),
            ],
            layer: BenchLayer::Libs,
            workload: WorkloadClass::Macro,
            determinism: DeterminismClass::Deterministic,
            owner_crate: "vyre-libs".to_string(),
        }
    }

    fn suites(&self) -> &'static [SuiteKind] {
        &[
            SuiteKind::Release,
            SuiteKind::Gpu,
            SuiteKind::Deep,
            SuiteKind::Honest,
        ]
    }

    fn requirements(&self) -> BenchRequirements {
        BenchRequirements {
            needs_gpu: true,
            needs_network: false,
            min_vram_bytes: None,
            min_input_bytes: Some((RUST_LEXER_BATCH_SOURCES * 192) as u64),
            feature_set: vec![
                "rust-parser".to_string(),
                "gpu-lexer".to_string(),
                "batched-lexer".to_string(),
                "ir-lexer".to_string(),
            ],
        }
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let sources = rust_lexer_batch_sources();
        let layout = RustLexerBatchLayout::from_sources(&sources)?;
        let source_count = u32::try_from(sources.len()).map_err(|_| {
            BenchError::ExecutionFailed(
                "Rust lexer batch source count exceeds u32-addressable plan limit".to_string(),
            )
        })?;
        let haystack_len = u32::try_from(layout.packed_source.len()).map_err(|_| {
            BenchError::ExecutionFailed(
                "Rust lexer batch source bytes exceed u32-addressable plan limit".to_string(),
            )
        })?;
        let token_stride = u32::try_from(layout.token_stride).map_err(|_| {
            BenchError::ExecutionFailed(
                "Rust lexer batch token stride exceeds u32-addressable plan limit".to_string(),
            )
        })?;
        let program = rust_lexer_batch(
            "haystack",
            "source_offsets",
            "source_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            haystack_len,
            source_count,
            token_stride,
        );
        let inputs = layout.inputs();

        let baseline_start = std::time::Instant::now();
        let (baseline_outputs, token_count) =
            rust_lexer_batch_baseline_outputs(&sources, layout.token_stride)?;
        let baseline_wall_ns = baseline_start.elapsed().as_nanos() as u64;

        Ok(Box::new(RustLexerBatchPrepared {
            program,
            inputs,
            source_bytes: layout.packed_source.len(),
            source_count,
            token_stride: layout.token_stride,
            baseline_outputs,
            baseline_wall_ns,
            token_count,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        prepared
            .downcast_ref::<RustLexerBatchPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<RustLexerBatchPrepared>()
            .map(|prepared| {
                let read_bytes = prepared.inputs.iter().map(Vec::len).sum::<usize>() as u64;
                let write_bytes = prepared
                    .baseline_outputs
                    .iter()
                    .map(Vec::len)
                    .sum::<usize>() as u64;
                (read_bytes, write_bytes)
            })
            .unwrap_or((0, 0))
    }

    fn run(
        &self,
        ctx: &mut BenchContext,
        prepared: &mut PreparedCase,
    ) -> Result<BenchRun, BenchError> {
        let prepared = prepared
            .downcast_ref::<RustLexerBatchPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed(
                    "Rust lexer batch prepared payload type mismatch".to_string(),
                )
            })?;

        let mut dispatch_config = ctx.dispatch_config.clone();
        dispatch_config.grid_override =
            Some([prepared.source_count.div_ceil(WORKGROUP_SIZE).max(1), 1, 1]);
        let timed = ctx
            .dispatch_timed(&prepared.program, &prepared.inputs, &dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        if timed.outputs.len() != 4 {
            return Err(BenchError::BackendFailed(format!(
                "Rust batch lexer IR must return 4 live-out columns [types, starts, lens, counts], got {}",
                timed.outputs.len()
            )));
        }

        let input_bytes = prepared.inputs.iter().map(Vec::len).sum::<usize>() as u64;
        let output_bytes = timed.outputs.iter().map(Vec::len).sum::<usize>() as u64;

        Ok(BenchRun {
            metrics: BenchMetrics {
                wall_ns: Some(timed.wall_ns),
                dispatch_ns: Some(timed.wall_ns),
                kernel_execute_ns: timed.device_ns.filter(|ns| *ns > 0),
                input_bytes: Some(input_bytes),
                output_bytes: Some(output_bytes),
                bytes_read: Some(input_bytes),
                bytes_written: Some(output_bytes),
                wire_bytes: Some(prepared.source_bytes as u64),
                custom: vec![
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_batch_speedup_x1000".to_string(),
                        value: super::speedup_x1000(prepared.baseline_wall_ns, timed.wall_ns),
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_batch_tokens".to_string(),
                        value: prepared.token_count as u64,
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_batch_source_bytes".to_string(),
                        value: prepared.source_bytes as u64,
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_batch_sources".to_string(),
                        value: u64::from(prepared.source_count),
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_batch_token_stride".to_string(),
                        value: prepared.token_stride as u64,
                    },
                ],
                ..Default::default()
            },
            baseline_metrics: Some(BenchMetrics {
                wall_ns: Some(prepared.baseline_wall_ns),
                input_bytes: Some(input_bytes),
                output_bytes: Some(
                    prepared
                        .baseline_outputs
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>() as u64,
                ),
                bytes_read: Some(prepared.source_bytes as u64),
                bytes_written: Some(
                    prepared
                        .baseline_outputs
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>() as u64,
                ),
                wire_bytes: Some(prepared.source_bytes as u64),
                ..Default::default()
            }),
            outputs: timed.outputs,
            baseline_outputs: ctx
                .include_baseline_outputs
                .then(|| prepared.baseline_outputs.clone()),
        })
    }

    fn verify(&self, _ctx: &mut BenchContext, run: &BenchRun) -> Result<Correctness, BenchError> {
        run.verify_exact_outputs()
    }
}

struct RustLexerBatchLayout {
    packed_source: Vec<u8>,
    offsets: Vec<u32>,
    lens: Vec<u32>,
    token_stride: usize,
}

impl RustLexerBatchLayout {
    fn from_sources(sources: &[Vec<u8>]) -> Result<Self, BenchError> {
        let mut packed_source = Vec::new();
        let mut offsets = Vec::with_capacity(sources.len());
        let mut lens = Vec::with_capacity(sources.len());
        let mut token_stride = 1usize;
        for (source_idx, source) in sources.iter().enumerate() {
            offsets.push(u32::try_from(packed_source.len()).map_err(|_| {
                BenchError::ExecutionFailed(format!(
                    "Rust lexer batch byte offset for source {source_idx} exceeds u32"
                ))
            })?);
            lens.push(u32::try_from(source.len()).map_err(|_| {
                BenchError::ExecutionFailed(format!(
                    "Rust lexer batch source {source_idx} length exceeds u32"
                ))
            })?);
            token_stride = token_stride.max(source.len().saturating_add(1).max(1));
            packed_source.extend_from_slice(source);
        }
        Ok(Self {
            packed_source,
            offsets,
            lens,
            token_stride,
        })
    }

    fn inputs(&self) -> Vec<Vec<u8>> {
        let token_slots = self.offsets.len().max(1) * self.token_stride;
        let zero_tokens = vec![0u8; token_slots * std::mem::size_of::<u32>()];
        vec![
            u32s_to_bytes(&rust_source_words(&self.packed_source)),
            u32s_to_bytes(&self.offsets),
            u32s_to_bytes(&self.lens),
            zero_tokens.clone(),
            zero_tokens.clone(),
            zero_tokens,
            vec![0u8; self.offsets.len().max(1) * std::mem::size_of::<u32>()],
        ]
    }
}

fn rust_lexer_batch_sources() -> Vec<Vec<u8>> {
    let mut sources = Vec::with_capacity(RUST_LEXER_BATCH_SOURCES);
    for idx in 0..RUST_LEXER_BATCH_SOURCES {
        sources.push(
            format!(
                "fn stress_file_{idx}(n: i32, flag: bool) -> i32 {{
    let mut acc: i32 = {};
    /* per-file block comment to force bounded scanning */
    for i in -{}..n {{
        // line comment with branch-heavy token stream
        if i <= {} && flag != false {{
            acc += i * {};
        }} else {{
            acc -= i % {};
        }};
    }}
    return acc;
}}
",
                idx % 31,
                (idx % 7) + 1,
                (idx % 13) + 1,
                (idx % 5) + 2,
                (idx % 11) + 2
            )
            .into_bytes(),
        );
    }
    sources
}

fn rust_lexer_batch_baseline_outputs(
    sources: &[Vec<u8>],
    token_stride: usize,
) -> Result<(Vec<Vec<u8>>, usize), BenchError> {
    let token_slots = sources.len().max(1) * token_stride;
    let mut kinds = vec![0u32; token_slots];
    let mut starts = vec![0u32; token_slots];
    let mut lens = vec![0u32; token_slots];
    let mut counts = vec![0u32; sources.len().max(1)];
    let mut total_tokens = 0usize;

    for (source_idx, source) in sources.iter().enumerate() {
        let tokens = lex_cpu(source).map_err(|offset| {
            BenchError::ExecutionFailed(format!(
                "Rust lexer batch CPU baseline rejected source {source_idx} at byte {offset}"
            ))
        })?;
        if tokens.len() > token_stride {
            return Err(BenchError::ExecutionFailed(format!(
                "Rust lexer batch source {source_idx} emitted {} tokens for stride {token_stride}",
                tokens.len()
            )));
        }
        counts[source_idx] = u32::try_from(tokens.len()).map_err(|_| {
            BenchError::ExecutionFailed(format!(
                "Rust lexer batch source {source_idx} token count exceeds u32"
            ))
        })?;
        let base = source_idx * token_stride;
        for (token_idx, token) in tokens.iter().enumerate() {
            let out_idx = base + token_idx;
            kinds[out_idx] = u32::from(token.kind);
            starts[out_idx] = token.start;
            lens[out_idx] = u32::from(token.len);
        }
        total_tokens += tokens.len();
    }

    Ok((
        vec![
            u32s_to_bytes(&kinds),
            u32s_to_bytes(&starts),
            u32s_to_bytes(&lens),
            u32s_to_bytes(&counts),
        ],
        total_tokens,
    ))
}

fn rust_source_words(source: &[u8]) -> Vec<u32> {
    let mut words = source
        .iter()
        .map(|byte| u32::from(*byte))
        .collect::<Vec<_>>();
    if words.is_empty() {
        words.push(0);
    }
    words
}

fn u32s_to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

inventory::submit! {
    &RustLexerBatchGpuPipeline as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::BufferAccess;
    use vyre_libs::parsing::rust::lex::tokens::{EOF, KW_FN};

    fn decode_u32_words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(std::mem::size_of::<u32>())
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
            .collect()
    }

    #[test]
    fn rust_lexer_batch_baseline_pads_each_source_window_to_program_shape() {
        let sources = rust_lexer_batch_sources();
        let layout = RustLexerBatchLayout::from_sources(&sources).expect("layout builds");
        let (outputs, token_count) =
            rust_lexer_batch_baseline_outputs(&sources, layout.token_stride)
                .expect("benchmark sources lex");
        let token_slots = sources.len() * layout.token_stride;

        assert_eq!(outputs.len(), 4);
        assert_eq!(outputs[0].len(), token_slots * std::mem::size_of::<u32>());
        assert_eq!(outputs[1].len(), token_slots * std::mem::size_of::<u32>());
        assert_eq!(outputs[2].len(), token_slots * std::mem::size_of::<u32>());
        assert_eq!(outputs[3].len(), sources.len() * std::mem::size_of::<u32>());
        assert!(token_count > sources.len());

        let kinds = decode_u32_words(&outputs[0]);
        let counts = decode_u32_words(&outputs[3]);
        assert_eq!(kinds[0], u32::from(KW_FN));
        for (source_idx, count) in counts.iter().copied().enumerate() {
            assert!(count as usize <= layout.token_stride);
            let last_idx = source_idx * layout.token_stride + count as usize - 1;
            assert_eq!(kinds[last_idx], u32::from(EOF));
        }
    }

    #[test]
    fn rust_lexer_batch_program_declares_packed_inputs_and_four_live_out_columns() {
        let sources = rust_lexer_batch_sources();
        let layout = RustLexerBatchLayout::from_sources(&sources).expect("layout builds");
        let program = rust_lexer_batch(
            "haystack",
            "source_offsets",
            "source_lens",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            layout.packed_source.len() as u32,
            sources.len() as u32,
            layout.token_stride as u32,
        );
        let buffers = program.buffers();
        assert_eq!(buffers.len(), 7);
        assert_eq!(buffers[0].access(), BufferAccess::ReadOnly);
        assert_eq!(buffers[1].access(), BufferAccess::ReadOnly);
        assert_eq!(buffers[2].access(), BufferAccess::ReadOnly);
        assert!(
            buffers[3..]
                .iter()
                .all(|buffer| buffer.access() == BufferAccess::ReadWrite),
            "token columns and per-source counts must be live-outs"
        );
    }

    #[test]
    fn rust_lexer_batch_sources_stay_inside_cpu_subset() {
        let sources = rust_lexer_batch_sources();
        for (source_idx, source) in sources.iter().enumerate().take(32) {
            let tokens = lex_cpu(source).unwrap_or_else(|offset| {
                panic!("benchmark source {source_idx} must lex, rejected at byte {offset}")
            });
            assert_eq!(tokens.last().map(|token| token.kind), Some(EOF));
        }
    }
}
