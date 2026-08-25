//! Lexer dispatch for the Rust frontend pipeline.

mod batch;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::CompiledPipeline;
use vyre_libs::parsing::rust::lex::lexer::core::{lex as lex_cpu, Token};
use vyre_libs::parsing::rust::lex::lexer::plan::RustLexerPlan;
use vyre_libs::parsing::rust::lex::tokens::{EOF, ERROR};

#[cfg(not(target_os = "macos"))]
use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;

use crate::pipeline::RustPipelineConfig;
use crate::RustFrontendError;

const RUST_GPU_LEXER_WORKGROUP_SIZE: u32 = 256;

/// Lex source bytes, preferring GPU if configured and available.
pub fn lex(
    source: &[u8],
    config: &RustPipelineConfig,
    plan: &RustLexerPlan,
) -> Result<Vec<Token>, RustFrontendError> {
    if config.gpu_lex {
        return lex_gpu(source, plan);
    }

    lex_cpu(source).map_err(RustFrontendError::Lex)
}

/// Lex many source buffers, sharing one GPU dispatch if configured.
pub fn lex_batch(
    sources: &[&[u8]],
    config: &RustPipelineConfig,
    plan: &RustLexerPlan,
) -> Result<Vec<Result<Vec<Token>, RustFrontendError>>, RustFrontendError> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    if config.gpu_lex {
        return batch::lex_gpu_batch(sources, plan);
    }

    Ok(sources
        .iter()
        .map(|source| lex_cpu(source).map_err(RustFrontendError::Lex))
        .collect())
}

fn lex_gpu(source: &[u8], plan: &RustLexerPlan) -> Result<Vec<Token>, RustFrontendError> {
    std::str::from_utf8(source).map_err(|error| RustFrontendError::Lex(error.valid_up_to()))?;
    let haystack_len =
        u32::try_from(source.len()).map_err(|_| RustFrontendError::Lex(u32::MAX as usize))?;
    let program = plan.build_for_len(haystack_len);
    let inputs = lexer_inputs(source);
    let input_refs = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let backend = shared_gpu_lexer_backend()?;
    let outputs = dispatch_gpu_lexer_cached(
        backend.as_ref(),
        &program,
        &input_refs,
        [1, 1, 1],
        "rust_frontend_gpu_lexer",
    )?;
    decode_gpu_tokens(&outputs)
}

fn shared_gpu_lexer_backend() -> Result<Arc<dyn VyreBackend>, RustFrontendError> {
    static BACKEND: OnceLock<Arc<dyn VyreBackend>> = OnceLock::new();
    if let Some(backend) = BACKEND.get() {
        return Ok(Arc::clone(backend));
    }

    let backend = if let Ok(requested) = std::env::var("VYRE_BACKEND") {
        if requested == "preferred" {
            return Err(RustFrontendError::Backend(
                "Rust GPU lexer backend override VYRE_BACKEND=preferred is recursive; unset it or set `cuda`/`wgpu` explicitly"
                    .to_string(),
            ));
        }
        dispatch_backend_by_id(&requested).map_err(|error| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer backend override VYRE_BACKEND={requested} failed: {error}"
            ))
        })?
    } else {
        match dispatch_backend_by_id("cuda") {
            Ok(cuda) => cuda,
            Err(cuda_error) => dispatch_backend_by_id("wgpu").map_err(|wgpu_error| {
                RustFrontendError::Backend(format!(
                    "Rust GPU lexer backend unavailable. CUDA-first acquisition failed: {cuda_error}; secondary WGPU acquisition failed: {wgpu_error}"
                ))
            })?,
        }
    };

    let _ = BACKEND.set(Arc::clone(&backend));
    Ok(BACKEND.get().map_or(backend, Arc::clone))
}

fn dispatch_backend_by_id(id: &str) -> Result<Arc<dyn VyreBackend>, String> {
    match id {
        "cuda" | "wgpu" => {}
        other => {
            return Err(format!(
                "`{other}` is not a GPU lexer backend; use `cuda` or `wgpu`"
            ));
        }
    }
    let backend = vyre_driver::backend::acquire(id)
        .map_err(|error| format!("dispatch backend `{id}` unavailable: {error}"))?;
    Ok(Arc::from(backend))
}

fn dispatch_gpu_lexer_cached(
    backend: &dyn VyreBackend,
    program: &vyre::ir::Program,
    inputs: &[&[u8]],
    grid: [u32; 3],
    label: &str,
) -> Result<Vec<Vec<u8>>, RustFrontendError> {
    static PIPELINES: OnceLock<Mutex<HashMap<(String, [u8; 32]), Arc<dyn CompiledPipeline>>>> =
        OnceLock::new();

    let mut dispatch_config = DispatchConfig::default();
    dispatch_config.grid_override = Some(grid);
    dispatch_config.label = Some(label.to_string());

    let key = (backend.id().to_string(), program.fingerprint());
    let cache = PIPELINES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(pipeline) = cache
        .lock()
        .map_err(|error| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer pipeline cache lock poisoned: {error}"
            ))
        })?
        .get(&key)
        .cloned()
    {
        let mut outputs = Vec::new();
        pipeline
            .dispatch_borrowed_into(inputs, &dispatch_config, &mut outputs)
            .map_err(|error| {
                RustFrontendError::Backend(format!(
                    "Rust GPU lexer cached dispatch failed on `{}`: {error}",
                    backend.id()
                ))
            })?;
        return Ok(outputs);
    }

    let Some(pipeline) = backend
        .compile_native(program, &dispatch_config)
        .map_err(|error| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer native compilation failed on `{}`: {error}",
                backend.id()
            ))
        })?
    else {
        return backend
            .dispatch_borrowed(program, inputs, &dispatch_config)
            .map_err(|error| {
                RustFrontendError::Backend(format!(
                    "Rust GPU lexer direct dispatch failed on `{}`: {error}",
                    backend.id()
                ))
            });
    };

    let mut outputs = Vec::new();
    pipeline
        .dispatch_borrowed_into(inputs, &dispatch_config, &mut outputs)
        .map_err(|error| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer dispatch failed on `{}`: {error}",
                backend.id()
            ))
        })?;
    cache
        .lock()
        .map_err(|error| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer pipeline cache lock poisoned while inserting: {error}"
            ))
        })?
        .insert(key, pipeline);
    Ok(outputs)
}

fn lexer_inputs(source: &[u8]) -> Vec<Vec<u8>> {
    let token_capacity = token_capacity(source);
    let zero_tokens = vec![0u8; token_capacity * std::mem::size_of::<u32>()];
    vec![
        u32s_to_bytes(&source_words(source)),
        zero_tokens.clone(),
        zero_tokens.clone(),
        zero_tokens,
        u32s_to_bytes(&[0]),
    ]
}

fn decode_gpu_tokens(outputs: &[Vec<u8>]) -> Result<Vec<Token>, RustFrontendError> {
    if outputs.len() != 4 {
        return Err(RustFrontendError::Backend(format!(
            "Rust GPU lexer returned {} output buffers, expected 4 token columns [types, starts, lens, count]",
            outputs.len()
        )));
    }
    let kinds = decode_u32_words(&outputs[0], "token types")?;
    let starts = decode_u32_words(&outputs[1], "token starts")?;
    let lens = decode_u32_words(&outputs[2], "token lengths")?;
    let count = decode_u32_words(&outputs[3], "token count")?
        .first()
        .copied()
        .ok_or_else(|| {
            RustFrontendError::Backend(
                "Rust GPU lexer returned an empty token-count buffer".to_string(),
            )
        })? as usize;
    if count == 0 || count > kinds.len() || count > starts.len() || count > lens.len() {
        return Err(RustFrontendError::Backend(format!(
            "Rust GPU lexer emitted token count {count}, but column lengths are types={}, starts={}, lens={}",
            kinds.len(),
            starts.len(),
            lens.len()
        )));
    }

    decode_token_window(&kinds, &starts, &lens, 0, count)
}

fn decode_token_window(
    kinds: &[u32],
    starts: &[u32],
    lens: &[u32],
    base: usize,
    count: usize,
) -> Result<Vec<Token>, RustFrontendError> {
    let mut tokens = Vec::with_capacity(count);
    for idx in 0..count {
        let out_idx = base + idx;
        let start = starts[out_idx];
        if kinds[out_idx] == u32::from(ERROR) {
            return Err(RustFrontendError::Lex(start as usize));
        }
        let kind = u16::try_from(kinds[out_idx]).map_err(|_| {
            RustFrontendError::Backend(format!(
                "Rust GPU lexer emitted token kind {} at token {idx}, which cannot fit u16",
                kinds[out_idx]
            ))
        })?;
        let len =
            u16::try_from(lens[out_idx]).map_err(|_| RustFrontendError::Lex(start as usize))?;
        tokens.push(Token { kind, start, len });
    }
    if tokens.last().map(|token| token.kind) != Some(EOF) {
        return Err(RustFrontendError::Backend(
            "Rust GPU lexer did not terminate its token stream with EOF".to_string(),
        ));
    }
    Ok(tokens)
}

fn decode_u32_words(bytes: &[u8], label: &str) -> Result<Vec<u32>, RustFrontendError> {
    if bytes.len() % std::mem::size_of::<u32>() != 0 {
        return Err(RustFrontendError::Backend(format!(
            "Rust GPU lexer {label} buffer has {} bytes, not a multiple of 4",
            bytes.len()
        )));
    }
    Ok(bytes
        .chunks_exact(std::mem::size_of::<u32>())
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn token_capacity(source: &[u8]) -> usize {
    source.len().saturating_add(1).max(1)
}

fn source_words(source: &[u8]) -> Vec<u32> {
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
    // Canonical LE-pack: wire's bytemuck `cast_slice` fast-path is a single memcpy
    // on little-endian hosts vs the per-word `flat_map(to_le_bytes)` this replaced
    // (NO DUPLICATION; Law 7). Big-endian falls back to per-word inside wire.
    vyre_primitives::wire::pack_u32_slice(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_libs::parsing::rust::lex::tokens::{KW_FN, KW_I32, KW_LET};

    #[test]
    fn gpu_token_decoder_reconstructs_token_stream_columns() {
        let outputs = vec![
            u32s_to_bytes(&[
                u32::from(KW_FN),
                u32::from(KW_LET),
                u32::from(KW_I32),
                u32::from(EOF),
            ]),
            u32s_to_bytes(&[0, 8, 15, 18]),
            u32s_to_bytes(&[2, 3, 3, 0]),
            u32s_to_bytes(&[4]),
        ];
        let tokens = decode_gpu_tokens(&outputs).expect("valid GPU token columns decode");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, KW_FN);
        assert_eq!(tokens[2].start, 15);
        assert_eq!(tokens[3].kind, EOF);
    }

    #[test]
    fn gpu_token_decoder_maps_error_token_to_lex_error() {
        let outputs = vec![
            u32s_to_bytes(&[u32::from(ERROR), u32::from(EOF)]),
            u32s_to_bytes(&[7, 8]),
            u32s_to_bytes(&[1, 0]),
            u32s_to_bytes(&[2]),
        ];
        assert!(matches!(
            decode_gpu_tokens(&outputs),
            Err(RustFrontendError::Lex(7))
        ));
    }
}
