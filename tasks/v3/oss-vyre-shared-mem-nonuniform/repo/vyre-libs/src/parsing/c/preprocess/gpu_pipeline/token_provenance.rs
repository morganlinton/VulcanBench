//! Token-level spelling/expansion provenance emitted by the GPU preprocessor.

use std::sync::{Mutex, OnceLock};

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use smallvec::SmallVec;

use super::macro_events::{stable_macro_symbol_id, MacroEvent, MacroEventKind};
use super::tokenization::gpu_tokenize_without_directive_metadata;
use super::{ClassifiedTokens, GpuDispatcher, MacroDef};

type MacroBucket<'a> = SmallVec<[&'a MacroDef; 2]>;

#[path = "token_provenance/anchor_match.rs"]
mod anchor_match;
#[path = "token_provenance/checked.rs"]
mod checked;
#[path = "token_provenance/direct.rs"]
mod direct;
#[path = "token_provenance/invocation.rs"]
mod invocation;
#[path = "token_provenance/macro_record.rs"]
mod macro_record;
#[path = "token_provenance/missing_invocation.rs"]
mod missing_invocation;
#[path = "token_provenance/model.rs"]
mod model;
#[path = "token_provenance/object_backfill.rs"]
mod object_backfill;
#[path = "token_provenance/parameter_substitution.rs"]
mod parameter_substitution;
#[path = "token_provenance/params.rs"]
mod params;
#[path = "token_provenance/replacement_cache.rs"]
mod replacement_cache;
#[path = "token_provenance/replacement_tokens.rs"]
mod replacement_tokens;
#[path = "token_provenance/span_dedupe.rs"]
mod span_dedupe;
#[path = "token_provenance/spelling_origin.rs"]
mod spelling_origin;
#[path = "token_provenance/token_columns.rs"]
mod token_columns;

pub(super) use direct::record_direct_token_provenance;
pub(super) use macro_record::record_macro_token_provenance;
pub use model::TokenProvenanceEvent;

use anchor_match::*;
use checked::*;
use invocation::*;
use missing_invocation::record_missing_invocation_provenance;
use model::{ReplacementTokenCacheKey, REPLACEMENT_TOKEN_CACHE_MAX_ENTRIES};
use object_backfill::record_missing_object_replacement_provenance;
use parameter_substitution::record_missing_parameter_substitution_provenance;
use params::*;
use replacement_cache::cached_replacement_tokens;
use replacement_tokens::*;
use span_dedupe::SpanDedupe;
use spelling_origin::macro_spelling_origin;
use token_columns::{token_len, token_start};

fn reserve_token_provenance_events(
    token_provenance_events: &mut Vec<TokenProvenanceEvent>,
    additional: usize,
    label: &'static str,
) -> Result<(), String> {
    token_provenance_events
        .try_reserve_exact(additional)
        .map_err(|error| {
            format!(
                "vyre-libs::gpu_pipeline: could not reserve {additional} token provenance events for {label}: {error:?}. Fix: shard preprocessing before provenance export."
            )
        })
}
