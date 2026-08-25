#![allow(
    clippy::doc_lazy_continuation,
    clippy::double_must_use,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::redundant_closure,
    clippy::too_many_arguments,
    clippy::nonminimal_bool,
    clippy::derivable_impls,
    clippy::single_char_add_str,
    clippy::type_complexity,
    clippy::map_entry,
    clippy::only_used_in_recursion,
    clippy::manual_flatten,
    clippy::explicit_counter_loop
)]
pub mod carriers;
pub mod dangling;
pub mod descriptor_diff;
pub mod descriptor_dump;
pub mod fixtures;
pub mod naga_dump;
pub mod naga_trace;
pub(crate) mod path_map_serde;
pub mod scan_explain;
pub mod source_walker;
pub mod wgsl;

pub use carriers::{carrier_summary, find_uncarriered_assigns, CarrierSummary, UncarrieredAssign};
pub use dangling::{find_dangling_refs, DanglingRef};
pub use descriptor_diff::{bisect_rewrites, diff_descriptors, DescriptorDiff, RewriteBisectResult};
pub use descriptor_dump::{dump_descriptor, DescriptorDump, DescriptorDumpOptions};
pub use naga_dump::{dump_naga_module, NagaDump};
pub use naga_trace::{
    failure_trace, failure_trace_wgsl, load_bind_result_log, BindResultLogError, FailureTrace,
};
pub use scan_explain::{
    scan_explain_report, ScanExplainEngine, ScanExplainError,
    ScanExplainExactnessClass, ScanExplainFactor, ScanExplainRejectedEngine,
    ScanExplainReport, ScanExplainRouteEvidence, ScanExplainVerifierFragment, ScanFactorRole,
    SCAN_EXPLAIN_REPORT_SCHEMA_VERSION,
};
pub use wgsl::{dump_wgsl, dump_wgsl_with_lines, WgslDump};
