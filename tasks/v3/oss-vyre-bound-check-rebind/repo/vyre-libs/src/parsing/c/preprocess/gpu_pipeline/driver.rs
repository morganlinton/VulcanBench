//! Translation-unit orchestration for the GPU preprocessor.
//!
//! This module owns run lifecycle only. File preparation, directive walking,
//! macro mutation, include recursion, and conditional stack updates live in
//! duty-specific files under `gpu_pipeline/driver/`.

#[path = "driver/active_segments.rs"]
mod active_segments;
#[path = "driver/conditional_directives.rs"]
mod conditional_directives;
#[path = "driver/directive_diagnostics.rs"]
mod directive_diagnostics;
#[path = "driver/directive_emission.rs"]
mod directive_emission;
#[path = "driver/directive_state.rs"]
mod directive_state;
#[path = "driver/directive_walk.rs"]
mod directive_walk;
#[path = "driver/file_inputs.rs"]
mod file_inputs;
#[path = "driver/ifdef_truth_batch.rs"]
mod ifdef_truth_batch;
#[path = "driver/include_directives.rs"]
mod include_directives;
#[path = "driver/include_file_cache.rs"]
mod include_file_cache;
#[path = "driver/include_guard_scan.rs"]
mod include_guard_scan;
#[path = "driver/macro_directives.rs"]
mod macro_directives;
#[path = "driver/stage_trace.rs"]
mod stage_trace;

use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap as HashMap;

use super::directives::DirectiveExtractionScratch;
use super::gpu_pipeline_filter::FilterScratch;
use super::header_reuse;
use super::include_acceleration::IncludeAccelerationState;
use super::live_state::{replace_live_macro_indexed, LiveConditionalScratch};
use super::tokenization::TokenizationScratch;
use super::{
    ConditionalEvent, GpuDispatcher, HeaderReuseEvent, IncludeAccelerationEvent, IncludeEvent,
    IncludeLoader, MacroDef, MacroEvent, MacroExpansionEvent, PreprocessedSource,
    TokenProvenanceEvent, MAX_INCLUDE_DEPTH,
};

pub(super) fn preprocess_translation_unit(
    dispatcher: &dyn GpuDispatcher,
    loader: &dyn IncludeLoader,
    tu_path: &Path,
    source: &[u8],
    cli_macros: &[MacroDef],
) -> Result<PreprocessedSource, String> {
    let mut run = PreprocessRun::try_new(dispatcher, loader, cli_macros)?;
    run.preprocess_one_file(tu_path, source, 0)?;
    Ok(run.finish())
}

struct PreprocessRun<'a> {
    dispatcher: &'a dyn GpuDispatcher,
    loader: &'a dyn IncludeLoader,
    macros: Vec<MacroDef>,
    macro_index: HashMap<Vec<u8>, usize>,
    macro_generation: u64,
    defines_hash_cache: Option<(u64, [u8; 16])>,
    output: Vec<u8>,
    stack: Vec<PathBuf>,
    include_events: Vec<IncludeEvent>,
    conditional_events: Vec<ConditionalEvent>,
    macro_events: Vec<MacroEvent>,
    macro_expansion_events: Vec<MacroExpansionEvent>,
    token_provenance_events: Vec<TokenProvenanceEvent>,
    include_acceleration_state: IncludeAccelerationState,
    include_acceleration_events: Vec<IncludeAccelerationEvent>,
    header_reuse_events: Vec<HeaderReuseEvent>,
    include_file_cache: include_file_cache::IncludeFileCache,
    filter_scratch: FilterScratch,
    directive_extraction_scratch: DirectiveExtractionScratch,
    tokenization_scratch: TokenizationScratch,
    live_conditional_scratch: LiveConditionalScratch,
    ifdef_truth_batch_scratch: ifdef_truth_batch::IfdefTruthBatchScratch,
}

impl<'a> PreprocessRun<'a> {
    fn try_new(
        dispatcher: &'a dyn GpuDispatcher,
        loader: &'a dyn IncludeLoader,
        cli_macros: &[MacroDef],
    ) -> Result<Self, String> {
        let mut macros = Vec::new();
        macros.try_reserve_exact(cli_macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} CLI macro definitions: {error:?}. Fix: shard or reject oversized CLI macro configuration before GPU preprocessing.",
                cli_macros.len()
            )
        })?;
        let mut macro_index = HashMap::default();
        macro_index.try_reserve(cli_macros.len()).map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {} CLI macro index entries: {error:?}. Fix: shard or reject oversized CLI macro configuration before GPU preprocessing.",
                cli_macros.len()
            )
        })?;
        for mac in cli_macros {
            replace_live_macro_indexed(&mut macros, &mut macro_index, mac.clone());
        }
        Ok(Self {
            dispatcher,
            loader,
            macros,
            macro_index,
            macro_generation: 0,
            defines_hash_cache: None,
            output: Vec::new(),
            stack: Vec::new(),
            include_events: Vec::new(),
            conditional_events: Vec::new(),
            macro_events: Vec::new(),
            macro_expansion_events: Vec::new(),
            token_provenance_events: Vec::new(),
            include_acceleration_state: IncludeAccelerationState::default(),
            include_acceleration_events: Vec::new(),
            header_reuse_events: Vec::new(),
            include_file_cache: include_file_cache::IncludeFileCache::default(),
            filter_scratch: FilterScratch::default(),
            directive_extraction_scratch: DirectiveExtractionScratch::default(),
            tokenization_scratch: TokenizationScratch::default(),
            live_conditional_scratch: LiveConditionalScratch::default(),
            ifdef_truth_batch_scratch: ifdef_truth_batch::IfdefTruthBatchScratch::default(),
        })
    }

    fn finish(self) -> PreprocessedSource {
        PreprocessedSource {
            bytes: self.output,
            macros: self.macros,
            include_byte_cache_stats: self.include_file_cache.stats(),
            include_events: self.include_events,
            conditional_events: self.conditional_events,
            macro_events: self.macro_events,
            macro_expansion_events: self.macro_expansion_events,
            token_provenance_events: self.token_provenance_events,
            include_acceleration_events: self.include_acceleration_events,
            header_reuse_events: self.header_reuse_events,
        }
    }

    fn live_defines_hash(&mut self) -> [u8; 16] {
        if let Some((generation, hash)) = self.defines_hash_cache {
            if generation == self.macro_generation {
                return hash;
            }
        }
        let hash = header_reuse::hash_defines(&self.macros);
        self.defines_hash_cache = Some((self.macro_generation, hash));
        hash
    }

    fn invalidate_defines_hash(&mut self) {
        self.macro_generation = self.macro_generation.saturating_add(1);
        self.defines_hash_cache = None;
    }

    fn preprocess_one_file(
        &mut self,
        file_path: &Path,
        source: &[u8],
        depth: u32,
    ) -> Result<(), String> {
        let mut trace = stage_trace::StageTrace::new(depth, file_path, source.len());
        trace.log("enter");
        if depth > MAX_INCLUDE_DEPTH {
            return Err(format!(
                "vyre-libs::gpu_pipeline: include depth exceeded {MAX_INCLUDE_DEPTH}"
            ));
        }
        let prepared =
            file_inputs::prepare_file_inputs(self, file_path, source, depth, &mut trace)?;
        self.include_acceleration_state.observe_file(
            file_path,
            &prepared.classified,
            &prepared.payloads,
            &mut self.include_acceleration_events,
        );
        directive_walk::walk_directives(self, file_path, source, depth, prepared, &mut trace)
    }
}
