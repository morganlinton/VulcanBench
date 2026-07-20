use crate::api::case::{
    BenchCase, BenchContext, BenchError, BenchId, BenchLayer, BenchMetadata, BenchRequirements,
    BenchRun, Correctness, DeterminismClass, PreparedCase, WorkloadClass,
};
use crate::api::metric::{BenchMetrics, MetricPoint};
use crate::api::suite::SuiteKind;
use vyre_libs::parsing::rust::lex::lexer::core::lex as lex_cpu;
use vyre_libs::parsing::rust::lex::lexer::plan::rust_lexer;

struct RustLexerGpuPipeline;

const RUST_LEXER_REPEATS: usize = 32;

struct RustLexerPrepared {
    program: vyre::ir::Program,
    inputs: Vec<Vec<u8>>,
    source_bytes: Vec<u8>,
    baseline_outputs: Vec<Vec<u8>>,
    baseline_wall_ns: u64,
    token_count: usize,
}

impl BenchCase for RustLexerGpuPipeline {
    fn id(&self) -> BenchId {
        BenchId("frontend.rust.lexer.ir_execute".to_string())
    }

    fn metadata(&self) -> BenchMetadata {
        BenchMetadata {
            id: self.id(),
            name: "Rust GPU Lexer IR Execute".to_string(),
            description:
                "Rust nano-subset source tokenized by the Vyre IR lexer on GPU with exact CPU lexer column parity"
                    .to_string(),
            tags: vec![
                "frontend-rust".to_string(),
                "gpu-lexer".to_string(),
                "lexer".to_string(),
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
            min_input_bytes: Some((RUST_LEXER_REPEATS * 512) as u64),
            feature_set: vec![
                "rust-parser".to_string(),
                "gpu-lexer".to_string(),
                "ir-lexer".to_string(),
            ],
        }
    }

    fn prepare(&self, _ctx: &mut BenchContext) -> Result<PreparedCase, BenchError> {
        let source_bytes = rust_lexer_source();
        let haystack_len = u32::try_from(source_bytes.len()).map_err(|_| {
            BenchError::ExecutionFailed(
                "Rust lexer benchmark source exceeds u32-addressable plan limit".to_string(),
            )
        })?;
        let program = rust_lexer(
            "haystack",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            haystack_len,
        );
        let inputs = rust_lexer_inputs(&source_bytes);

        let baseline_start = std::time::Instant::now();
        let (baseline_outputs, token_count) = rust_lexer_baseline_outputs(&source_bytes)?;
        let baseline_wall_ns = baseline_start.elapsed().as_nanos() as u64;

        Ok(Box::new(RustLexerPrepared {
            program,
            inputs,
            source_bytes,
            baseline_outputs,
            baseline_wall_ns,
            token_count,
        }))
    }

    fn program<'a>(&self, prepared: &'a PreparedCase) -> Option<&'a vyre::ir::Program> {
        prepared
            .downcast_ref::<RustLexerPrepared>()
            .map(|prepared| &prepared.program)
    }

    fn bytes_touched(&self, prepared: &PreparedCase) -> (u64, u64) {
        prepared
            .downcast_ref::<RustLexerPrepared>()
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
            .downcast_ref::<RustLexerPrepared>()
            .ok_or_else(|| {
                BenchError::ExecutionFailed("Rust lexer prepared payload type mismatch".to_string())
            })?;

        let timed = ctx
            .dispatch_timed(&prepared.program, &prepared.inputs, &ctx.dispatch_config)
            .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
        if timed.outputs.len() != 4 {
            return Err(BenchError::BackendFailed(format!(
                "Rust lexer IR must return 4 live-out columns [types, starts, lens, count], got {}",
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
                wire_bytes: Some(prepared.source_bytes.len() as u64),
                custom: vec![
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_speedup_x1000".to_string(),
                        value: super::speedup_x1000(prepared.baseline_wall_ns, timed.wall_ns),
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_tokens".to_string(),
                        value: prepared.token_count as u64,
                    },
                    MetricPoint {
                        name: "rust_frontend_gpu_lexer_source_bytes".to_string(),
                        value: prepared.source_bytes.len() as u64,
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
                bytes_read: Some(prepared.source_bytes.len() as u64),
                bytes_written: Some(
                    prepared
                        .baseline_outputs
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>() as u64,
                ),
                wire_bytes: Some(prepared.source_bytes.len() as u64),
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

fn rust_lexer_source() -> Vec<u8> {
    let mut source = String::new();
    for idx in 0..RUST_LEXER_REPEATS {
        source.push_str(&format!(
            "fn stress_{idx}(n: i32, flag: bool) -> i32 {{
    let mut acc: i32 = {idx};
    // branchy token stream with comments, booleans, ranges, and compound ops
    for i in -3..n {{
        if i <= 0 && flag != false {{
            acc += i * 2;
        }} else {{
            acc -= i % 3;
        }};
    }}
    return acc;
}}
"
        ));
    }
    source.into_bytes()
}

fn rust_lexer_inputs(source: &[u8]) -> Vec<Vec<u8>> {
    let token_capacity = token_capacity(source);
    let zero_tokens = vec![0u8; token_capacity * std::mem::size_of::<u32>()];
    vec![
        u32s_to_bytes(&rust_source_words(source)),
        zero_tokens.clone(),
        zero_tokens.clone(),
        zero_tokens,
        u32s_to_bytes(&[0]),
    ]
}

fn rust_lexer_baseline_outputs(source: &[u8]) -> Result<(Vec<Vec<u8>>, usize), BenchError> {
    let tokens = lex_cpu(source).map_err(|offset| {
        BenchError::ExecutionFailed(format!(
            "Rust lexer benchmark CPU baseline rejected source at byte {offset}"
        ))
    })?;
    let token_capacity = token_capacity(source);
    if tokens.len() > token_capacity {
        return Err(BenchError::ExecutionFailed(format!(
            "Rust lexer baseline emitted {} tokens for capacity {token_capacity}",
            tokens.len()
        )));
    }

    let mut kinds = vec![0u32; token_capacity];
    let mut starts = vec![0u32; token_capacity];
    let mut lens = vec![0u32; token_capacity];
    for (idx, token) in tokens.iter().enumerate() {
        kinds[idx] = u32::from(token.kind);
        starts[idx] = token.start;
        lens[idx] = u32::from(token.len);
    }
    let count = u32::try_from(tokens.len()).map_err(|_| {
        BenchError::ExecutionFailed(
            "Rust lexer benchmark token count exceeds u32 output count".to_string(),
        )
    })?;

    Ok((
        vec![
            u32s_to_bytes(&kinds),
            u32s_to_bytes(&starts),
            u32s_to_bytes(&lens),
            u32s_to_bytes(&[count]),
        ],
        tokens.len(),
    ))
}

fn token_capacity(source: &[u8]) -> usize {
    source.len().saturating_add(1).max(1)
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
    &RustLexerGpuPipeline as &'static dyn BenchCase
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::BufferAccess;
    use vyre_libs::parsing::rust::lex::lexer::core::Token;
    use vyre_libs::parsing::rust::lex::tokens::{EOF, KW_FN};

    fn decode_u32_words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(std::mem::size_of::<u32>())
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
            .collect()
    }

    #[test]
    fn rust_lexer_baseline_pads_live_out_columns_to_program_shape() {
        let source = rust_lexer_source();
        let (outputs, token_count) =
            rust_lexer_baseline_outputs(&source).expect("benchmark source lexes");
        let token_capacity = token_capacity(&source);

        assert_eq!(outputs.len(), 4);
        assert_eq!(
            outputs[0].len(),
            token_capacity * std::mem::size_of::<u32>()
        );
        assert_eq!(
            outputs[1].len(),
            token_capacity * std::mem::size_of::<u32>()
        );
        assert_eq!(
            outputs[2].len(),
            token_capacity * std::mem::size_of::<u32>()
        );
        assert_eq!(outputs[3].len(), std::mem::size_of::<u32>());
        assert!(token_count > RUST_LEXER_REPEATS);

        let kinds = decode_u32_words(&outputs[0]);
        let count = decode_u32_words(&outputs[3])[0] as usize;
        assert_eq!(count, token_count);
        assert_eq!(kinds[0], u32::from(KW_FN));
        assert_eq!(kinds[count - 1], u32::from(EOF));
    }

    #[test]
    fn rust_lexer_program_declares_haystack_plus_four_live_out_columns() {
        let source = rust_lexer_source();
        let program = rust_lexer(
            "haystack",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            source.len() as u32,
        );
        let buffers = program.buffers();
        assert_eq!(buffers.len(), 5);
        assert_eq!(buffers[0].access(), BufferAccess::ReadOnly);
        assert!(
            buffers[1..]
                .iter()
                .all(|buffer| buffer.access() == BufferAccess::ReadWrite),
            "token columns must be read-write live-outs so CUDA returns all lexer columns"
        );
    }

    #[test]
    fn rust_lexer_source_stays_inside_cpu_subset() {
        let source = rust_lexer_source();
        let tokens = lex_cpu(&source).expect("benchmark source must stay in lexer subset");
        assert_eq!(tokens.last().map(|token: &Token| token.kind), Some(EOF));
    }
}
