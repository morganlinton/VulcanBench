use std::time::Instant;

use rustc_hash::FxHashMap as HashMap;

use super::include_guard_scan::{include_guard_ifndef_names, IncludeGuardIfndefNames};
use crate::parsing::c::preprocess::gpu_pipeline::conditional_stack::ConditionalFrame;
use crate::parsing::c::preprocess::gpu_pipeline::live_state::LiveMacroNameBuffers;
use crate::parsing::c::preprocess::gpu_pipeline::macro_expansion::MacroExpansionCache;
use crate::parsing::c::preprocess::gpu_pipeline::DirectivePayload;

pub(super) struct DirectiveWalkState {
    pub(super) conditionals: Vec<ConditionalFrame>,
    pub(super) last_emit_end: usize,
    pub(super) active_segment: Vec<u8>,
    pub(super) active_segment_start: Option<usize>,
    pub(super) macro_expansion_cache: MacroExpansionCache,
    pub(super) live_macro_buffers_cache: Option<LiveMacroNameBuffers>,
    pub(super) include_guard_ifndef_names: IncludeGuardIfndefNames,
    pub(super) ifdef_truth_cache: HashMap<usize, bool>,
    pub(super) walk_start: Instant,
    pub(super) gpu_ifdef: u32,
    pub(super) gpu_if: u32,
}

impl DirectiveWalkState {
    pub(super) fn new(payloads: &[DirectivePayload]) -> Self {
        Self {
            conditionals: Vec::new(),
            last_emit_end: 0,
            active_segment: Vec::new(),
            active_segment_start: None,
            macro_expansion_cache: MacroExpansionCache::default(),
            live_macro_buffers_cache: None,
            include_guard_ifndef_names: include_guard_ifndef_names(payloads),
            ifdef_truth_cache: HashMap::default(),
            walk_start: Instant::now(),
            gpu_ifdef: 0,
            gpu_if: 0,
        }
    }

    pub(super) fn invalidate_macro_dependent_caches(&mut self) {
        self.macro_expansion_cache.clear();
        self.live_macro_buffers_cache = None;
        self.ifdef_truth_cache.clear();
    }

    pub(super) fn invalidate_ifdef_truth_cache(&mut self) {
        self.ifdef_truth_cache.clear();
    }
}
