//! **Host-only** translation-unit preparation (Tier outside `vyre-libs` ops).
//!
//! Production work here is orchestration around the GPU-resident C frontend:
//! bounded source/include file I/O, include-path resolution, forced-include
//! prefixing, cache keys, and dependency invalidation.
//!
//! CLI `-D`/`-U` state is passed to the GPU preprocessor as ordered macro
//! actions. The legacy host-side `#define` prefix helpers are compatibility
//! oracles only and are not part of production resident preprocessing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::api::{CliMacroAction, VyreCompileOptions};
use crate::hash::{
    blake3_128, blake3_128_from_hasher, blake3_128_update_len_prefixed, StableHash128,
};

/// GPU-resident preprocessor pipeline orchestrator. Lives in
/// `vyre-libs::parsing::c::preprocess::gpu_pipeline`; re-exported here
/// so the existing tu_host call sites can be migrated incrementally.
pub use vyre_libs::parsing::c::preprocess::gpu_pipeline;
mod include_loader;
mod include_loader_cache;
mod preprocess;
mod system_includes;
mod target_predefines;

use include_loader::ResidentIncludeLoader;
#[cfg(feature = "cpu-oracle")]
pub use preprocess::reference_expand_preprocessor_macros;
#[cfg(feature = "cpu-oracle")]
use preprocess::{eval_preprocessor_condition, parse_define, strip_directive_comments, MacroDef};
use system_includes::system_include_dirs;

#[cfg(feature = "cpu-oracle")]
const MAX_INCLUDE_DEPTH: u32 = 64;
const MAX_INCLUDE_BYTES: usize = 16 * 1024 * 1024;
const RESIDENT_PREP_CACHE_MAX_ENTRIES: usize = 4096;
const RESIDENT_PREP_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

mod cli_macros;
mod file_io;
mod include_expand;
mod include_search;
mod resident_backend;
mod resident_cache;
mod resident_prepare;
mod source_text;
#[cfg(test)]
mod tests;

pub use cli_macros::apply_cli_defines_prefix;
#[cfg(feature = "cpu-oracle")]
pub use include_expand::expand_local_includes;
#[cfg(feature = "cpu-oracle")]
pub use resident_prepare::reference_prepare_resident_translation_unit_source as reference_prepare_translation_unit_source;
pub use resident_prepare::{
    prepare_resident_translation_unit_source, prepare_resident_translation_unit_source_gpu,
};

#[cfg(feature = "cpu-oracle")]
use cli_macros::apply_cli_source_prefix;
use cli_macros::{
    apply_forced_include_prefix, cli_macro_actions, cli_macro_defs, reject_mixed_macro_transport,
};
use file_io::read_include_bounded;
#[cfg(feature = "cpu-oracle")]
use include_expand::expand_local_includes_with_search_dirs;
use include_search::{
    expanded_include_search_dirs, search_include_file, search_include_next_file,
    search_system_include_file,
};
use resident_backend::{resident_preprocessor_backend, CachedResidentDispatcher};
use resident_cache::{
    clone_resident_prep_cache_source, insert_resident_prep_cache, lookup_resident_prep_cache_deps,
    remove_stale_resident_prep_cache_entry, resident_dep_from_metadata, resident_prep_cache,
    resident_prep_deps_valid, resident_prep_key, stable_hash_bytes, ResidentPrepEntry,
};
#[cfg(test)]
use resident_cache::{
    insert_resident_prep_cache_with_limits, lookup_resident_prep_cache,
    resident_dep_metadata_identity_available, resident_prep_cache_bytes, ResidentPrepCache,
    ResidentPrepKey,
};
use source_text::splice_line_continuations;
#[cfg(feature = "cpu-oracle")]
use source_text::{parse_directive, parse_include_literal};
