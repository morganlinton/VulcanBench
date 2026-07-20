use rustc_hash::FxHashSet as HashSet;
use vyre::ir::{BufferAccess, Expr, Program};

use crate::parsing::c::preprocess::expansion::opt_named_macro_expansion_materialized;

use super::buffers::{
    bucket_pow2, checked_gpu_u32, pack_u32_words_into, read_u32_scalar_exact, read_u32_word,
};
use super::expansion_events::record_macro_expansions;
use super::macro_table;
use super::segments::{
    classified_segment, has_live_macro_for_segment_excluding, live_macro_defs_for_segment,
    macro_segment_shard_ranges, macro_use_statement_ranges, LiveMacroLookup,
};
use super::token_provenance::{
    record_direct_token_provenance, record_macro_token_provenance, TokenProvenanceEvent,
};
use super::{ClassifiedTokens, GpuDispatcher, MacroDef};
use super::{MacroEvent, MacroExpansionEvent};

#[path = "macro_expansion/cache_key.rs"]
mod cache_key;
#[path = "macro_expansion/decode_outputs.rs"]
mod decode_outputs;
#[path = "macro_expansion/flush.rs"]
mod flush;
#[path = "macro_expansion/gpu_buffers.rs"]
mod gpu_buffers;
#[path = "macro_expansion/model.rs"]
mod model;
#[path = "macro_expansion/prescan.rs"]
mod prescan;
#[path = "macro_expansion/rescan.rs"]
mod rescan;
#[path = "macro_expansion/segment_ranges.rs"]
mod segment_ranges;

pub(super) use flush::flush_active_macro_segment;
pub(super) use model::MacroExpansionCache;

use cache_key::*;
use decode_outputs::expanded_classified_from_materialized_outputs;
use gpu_buffers::*;
use model::*;
use prescan::*;
use rescan::disabled_self_recursive_macro_names;
use segment_ranges::flush_macro_segment_ranges;
