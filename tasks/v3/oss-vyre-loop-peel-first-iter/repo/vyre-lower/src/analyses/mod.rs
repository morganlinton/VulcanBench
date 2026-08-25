//! Substrate-aware-but-backend-agnostic analyses on `KernelDescriptor`.
//!
//! Source-of-truth: `SEPARATION_AUDIT_2026-05-01.md` section S3 +
//! `PERF_ROADMAP_2026-05-01.md` section B.3.
//!
//! Each analysis here operates on a `KernelDescriptor` post-lowering
//! and pre-emission. Any rewrite they produce is consumed by every
//! emitter, so analyses that work this layer pay off across all
//! substrates with one implementation.
//!
//! Substrate-specific emission patterns live in their respective
//! emitter crates instead.

pub mod access_kind;
pub mod alias_facts;
pub mod bank_conflict;
pub mod candidate_plan;
pub mod coalesce;
pub mod common_subexpr;
pub mod const_buffer_promote;
pub mod dead_op;
pub mod def_use;
pub mod layout_aos_to_soa;
pub(crate) mod load_counts;
pub mod op_histogram;
pub mod reaching_def_facts;
pub mod shared_mem_promote;
pub mod texture_promote;
pub mod value_range;
pub mod vec_pack;
pub mod weir_alias;
pub mod weir_reaching_def;
pub mod workgroup_uniform;

// Re-exports for the common case: a one-call combined audit.
pub use access_kind::AccessKind;
pub use bank_conflict::{analyze as analyze_bank_conflict, BankConflictReport};
pub use coalesce::{analyze as analyze_coalesce, CoalescenceReport};
pub use common_subexpr::{analyze as analyze_common_subexpr, CommonSubexprReport};
pub use const_buffer_promote::{analyze as analyze_const_buffer_promote, ConstBufferPlan};
pub use dead_op::{analyze as analyze_dead_op, DeadOpReport};
pub use def_use::{
    analyze as analyze_def_use, dead_by_no_use, DefUseReport, PerBodyChains, UseSite,
};
pub use layout_aos_to_soa::{analyze as analyze_layout_aos_to_soa, LayoutTransformPlan};
pub use op_histogram::{analyze as analyze_op_histogram, OpHistogram};
pub use reaching_def_facts::import_descriptor_reaching_defs;
pub use shared_mem_promote::{analyze as analyze_shared_mem_promote, PromotionPlan};
pub use texture_promote::{analyze as analyze_texture_promote, TexturePromotionPlan};
pub use value_range::{analyze as analyze_value_range, IntRange, ValueRangeReport};
pub use workgroup_uniform::{analyze as analyze_workgroup_uniform, WorkgroupUniformReport};
