//! Security / taint compositions for program-analysis pipelines.
//!
//! Each op registers via `inventory::submit!(OpEntry { … })` and
//! exports a `fn(...) -> Program`. Program-analysis lowerers emit
//! against these stable paths directly.
//!
//! All security ops compose GPU-parallel graph algorithms over the
//! vyre IR: forward / backward reachability, dominator walks, and
//! taint propagation with sanitizer masking.
//!
//! ## Module re-export rule
//!
//! Every `pub mod foo` in this file re-exports its primary entry
//! point as `pub use foo::foo;` at parent, alphabetized below.
//! Callers reach a primitive by `vyre_libs::security::foo(...)`
//! without learning the file layout. The single intentional
//! exception is `topology::match_order`  -  per
//! AUDIT_CLAUDE_2026-04-24 F7, the `match_order` symbol must be
//! imported from `vyre_libs::range_ordering::match_order`; the
//! `#[deprecated]` shim in `topology.rs` is a soft-landing for
//! out-of-tree callers and is intentionally NOT re-exported here
//! so its deprecation warning fires.
//!
//! `flow_composition` is `pub(crate)` because its helpers
//! (`fuse_security_flow`, `dataflow_hit_program`,
//! `sanitized_dataflow_hit_program`) are internal building blocks
//! the public primitives compose; consumers should reach them only
//! through a stable public op.

macro_rules! define_bitset_and_security_op {
    (
        $module:ident,
        $function:ident,
        $marker:ident,
        $op_id:literal,
        $left:ident,
        $right:ident,
        $doc:literal,
        tests { $($test_name:ident: ($lhs:expr, $rhs:expr) => $expected:expr;)+ }
    ) => {
        #[doc = $doc]
        pub mod $module {
            use vyre::ir::Program;
            use vyre_primitives::bitset::and::bitset_and;
            use vyre_primitives::graph::csr_forward_traverse::bitset_words;

            pub(crate) const OP_ID: &str = $op_id;

            /// Build the canonical security bitset-intersection program.
            #[must_use]
            pub fn $function(
                node_count: u32,
                $left: &str,
                $right: &str,
                out: &str,
            ) -> Program {
                let words = bitset_words(node_count);
                crate::region::tag_program(OP_ID, bitset_and($left, $right, out, words))
            }

            /// CPU oracle for this security bitset-intersection predicate.
            #[must_use]
            #[cfg(test)]
            pub(crate) fn cpu_ref($left: &[u32], $right: &[u32]) -> Vec<u32> {
                vyre_primitives::bitset::and::cpu_ref($left, $right)
            }

            #[doc = concat!("Soundness marker for [`", stringify!($function), "`].")]
            pub struct $marker;

            impl vyre::soundness::SoundnessTagged for $marker {
                fn soundness(&self) -> vyre::soundness::Soundness {
                    vyre::soundness::Soundness::Exact
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                $(
                    #[test]
                    fn $test_name() {
                        assert_eq!(cpu_ref($lhs, $rhs), $expected);
                    }
                )+
            }
        }
    };
}

macro_rules! define_bitset_and_not_security_op {
    (
        $module:ident,
        $function:ident,
        $marker:ident,
        $op_id:literal,
        $left:ident,
        $right:ident,
        $doc:literal,
        tests { $($test_name:ident: ($lhs:expr, $rhs:expr) => $expected:expr;)+ }
    ) => {
        #[doc = $doc]
        pub mod $module {
            use vyre::ir::Program;
            use vyre_primitives::bitset::and_not::bitset_and_not;
            use vyre_primitives::graph::csr_forward_traverse::bitset_words;

            pub(crate) const OP_ID: &str = $op_id;

            /// Build the canonical security bitset-subtraction program.
            #[must_use]
            pub fn $function(
                node_count: u32,
                $left: &str,
                $right: &str,
                out: &str,
            ) -> Program {
                let words = bitset_words(node_count);
                crate::region::tag_program(OP_ID, bitset_and_not($left, $right, out, words))
            }

            /// CPU oracle for this security bitset-subtraction predicate.
            #[must_use]
            #[cfg(test)]
            pub(crate) fn cpu_ref($left: &[u32], $right: &[u32]) -> Vec<u32> {
                vyre_primitives::bitset::and_not::cpu_ref($left, $right)
            }

            #[doc = concat!("Soundness marker for [`", stringify!($function), "`].")]
            pub struct $marker;

            impl vyre::soundness::SoundnessTagged for $marker {
                fn soundness(&self) -> vyre::soundness::Soundness {
                    vyre::soundness::Soundness::Exact
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                $(
                    #[test]
                    fn $test_name() {
                        assert_eq!(cpu_ref($lhs, $rhs), $expected);
                    }
                )+
            }
        }
    };
}

pub mod aliases_dataflow;
define_bitset_and_security_op!(
    auth_check_dominates,
    auth_check_dominates,
    AuthCheckDominates,
    "vyre-libs::security::auth_check_dominates",
    auth_doms,
    sensitive_op_set,
    "`auth_check_dominates` - authorization check dominates sensitive operation.",
    tests {
        protected_op_returns_set: (&[0b1100], &[0b0100]) => vec![0b0100];
        unprotected_op_returns_empty: (&[0b0001], &[0b1110]) => vec![0];
        no_sensitive_ops: (&[0xFFFF], &[0]) => vec![0];
        no_auth_checks: (&[0], &[0xFFFF]) => vec![0];
    }
);
pub mod bounded_by_comparison;
define_bitset_and_security_op!(
    buffer_size_check,
    buffer_size_check,
    BufferSizeCheck,
    "vyre-libs::security::buffer_size_check",
    size_compared,
    user_input_set,
    "`buffer_size_check` - buffer size is compared to user input.",
    tests {
        checked_size_returns_set: (&[0b1010], &[0b1100]) => vec![0b1000];
        unchecked_size_returns_empty: (&[0b0001], &[0b1110]) => vec![0];
        no_user_input_yields_empty: (&[0xFFFF], &[0]) => vec![0];
        full_overlap: (&[0xDEAD], &[0xDEAD]) => vec![0xDEAD];
    }
);
mod catalog;
pub mod dominator_tree;
pub mod facts;
pub(crate) mod flow_composition;
pub mod flows_to;
pub mod flows_to_to_sink;
pub mod flows_to_with_sanitizer;
// `weir_ifds` is an INCOMPLETE integration: it `use`s a crate
// `external_dataflow_engine` that is wired into no Cargo.toml and exists nowhere
// on the tree, so it does not compile under `--features security` and broke every
// downstream consumer (e.g. surgec) the moment a cache invalidation forced a
// vyre-libs rebuild. Gated behind the non-existent feature `weir_ifds_external_engine`
// so the workspace builds again WITHOUT deleting the WIP. To finish the
// integration: add the `external_dataflow_engine` crate to the workspace + this
// crate's deps, then restore these guards to `#[cfg(feature = "security")]`.
#[cfg(feature = "weir_ifds_external_engine")]
pub mod weir_ifds;
define_bitset_and_not_security_op!(
    format_string_check,
    format_string_check,
    FormatStringCheck,
    "vyre-libs::security::format_string_check",
    format_arg_pts,
    non_literal_set,
    "`format_string_check` - format argument is reachable only from literals.",
    tests {
        literal_only_returns_full: (&[0xFFFF], &[0]) => vec![0xFFFF];
        user_input_present_subtracts: (&[0xFFFF], &[0xFF00]) => vec![0x00FF];
        fully_user_input_returns_empty: (&[0xDEAD], &[0xFFFF]) => vec![0];
        distributes: (&[0xFFFF, 0x0F0F], &[0xFF00, 0x0000]) => vec![0x00FF, 0x0F0F];
    }
);
pub mod integer_overflow_arith;
pub mod label_by_family;
define_bitset_and_security_op!(
    lock_dominates,
    lock_dominates,
    LockDominates,
    "vyre-libs::security::lock_dominates",
    lock_doms,
    shared_access_set,
    "`lock_dominates` - lock acquisition dominates shared-state access.",
    tests {
        locked_access: (&[0b1110], &[0b0010]) => vec![0b0010];
        unlocked_access: (&[0b0001], &[0b0010]) => vec![0];
        no_accesses: (&[0xFFFF], &[0]) => vec![0];
        empty_lock_set: (&[0], &[0xFFFF]) => vec![0];
    }
);
define_bitset_and_security_op!(
    path_canonical,
    path_canonical,
    PathCanonical,
    "vyre-libs::security::path_canonical",
    canonicalizer_dominates,
    fs_op_set,
    "`path_canonical` - path string was canonicalized before a filesystem operation.",
    tests {
        canonicalized_op: (&[0b1110], &[0b0010]) => vec![0b0010];
        uncanonicalized_op: (&[0b0001], &[0b0010]) => vec![0];
        no_fs_ops: (&[0xFFFF], &[0]) => vec![0];
        distributes: (&[0xFF00, 0x00FF], &[0xFFFF, 0xFFFF]) => vec![0xFF00, 0x00FF];
    }
);
pub mod path_reconstruct;
pub mod predicate_catalog;
pub mod relation_analyzer;
pub mod reporter;
pub mod sanitized_by;
define_bitset_and_security_op!(
    sanitizer_dominates,
    sanitizer_dominates,
    SanitizerDominates,
    "vyre-libs::security::sanitizer_dominates",
    sanitizer_doms,
    sink_set,
    "`sanitizer_dominates` - sanitizer dominates the queried sink.",
    tests {
        dominated_sink_returns_set: (&[0b1111], &[0b0010]) => vec![0b0010];
        non_dominated_sink_returns_empty: (&[0b0001], &[0b0010]) => vec![0];
        no_sinks_returns_empty: (&[0xFFFF], &[0]) => vec![0];
        distributes_per_word: (&[0xFF00, 0x00FF], &[0x0FF0, 0x0FF0]) => vec![0x0F00, 0x00F0];
    }
);
pub mod sink_intersection;
define_bitset_and_security_op!(
    sql_param_bound,
    sql_param_bound,
    SqlParamBound,
    "vyre-libs::security::sql_param_bound",
    param_binding_set,
    sql_query_set,
    "`sql_param_bound` - SQL query is built through parameter binding.",
    tests {
        parameterized_query: (&[0b1100], &[0b0100]) => vec![0b0100];
        raw_concat_query: (&[0b0001], &[0b0010]) => vec![0];
        no_queries: (&[0xFFFF], &[0]) => vec![0];
        distributes: (&[0xFF00, 0xF0F0], &[0x0FF0, 0x0F0F]) => vec![0x0F00, 0x0000];
    }
);
pub mod taint_flow;
pub mod taint_kill;
pub mod taint_pollution;
pub mod topology;
define_bitset_and_not_security_op!(
    unchecked_return,
    unchecked_return,
    UncheckedReturn,
    "vyre-libs::security::unchecked_return",
    use_set,
    check_dominates,
    "`unchecked_return` - sensitive return-value use lacks a dominating check.",
    tests {
        use_without_check_returns_set: (&[0b1100], &[0b0001]) => vec![0b1100];
        use_with_dominating_check_returns_empty: (&[0b0010], &[0b0010]) => vec![0];
        no_uses_returns_empty: (&[0], &[0xFFFF]) => vec![0];
        distributes: (&[0xFFFF, 0x0F0F], &[0x00FF, 0xF000]) => vec![0xFF00, 0x0F0F];
    }
);
define_bitset_and_security_op!(
    xss_escape,
    xss_escape,
    XssEscape,
    "vyre-libs::security::xss_escape",
    escape_dominates,
    render_set,
    "`xss_escape` - HTML output escaping dominates render sites.",
    tests {
        escaped_render: (&[0b1100], &[0b0100]) => vec![0b0100];
        unescaped_render: (&[0b0001], &[0b0010]) => vec![0];
        no_renders: (&[0xFFFF], &[0]) => vec![0];
        no_escape_dominators: (&[0], &[0xFFFF]) => vec![0];
    }
);

pub use aliases_dataflow::{aliases_dataflow, try_aliases_dataflow};
pub use auth_check_dominates::auth_check_dominates;
pub use bounded_by_comparison::bounded_by_comparison;
pub use buffer_size_check::buffer_size_check;
pub use dominator_tree::dominator_tree;
pub use facts::{
    AnalysisFact, AnalysisFactColumns, AnalysisFactError, AnalysisFactTable, AnalysisSourceSpan,
    FactId, FactKind, FindingProofBundle, FindingProofStep,
};
pub use flows_to::flows_to;
pub use flows_to_to_sink::flows_to_to_sink;
pub use flows_to_with_sanitizer::flows_to_with_sanitizer;
// Gated off with `weir_ifds` above (incomplete integration; missing the
// `external_dataflow_engine` crate). Restore to `#[cfg(feature = "security")]`
// once that crate is wired into the workspace.
#[cfg(feature = "weir_ifds_external_engine")]
pub use weir_ifds::{
    route_security_taint_through_weir_ifds, security_witness_path_from_weir,
    SecurityFindingWitnessPath, SecurityWitnessPathError, SecurityWitnessStatement,
    WeirIfdsSecurityBuffers, WeirIfdsSecurityDispatch, WeirIfdsSecurityRouteError,
    WEIR_IFDS_SECURITY_BACKEND_ID,
};
pub use format_string_check::format_string_check;
pub use integer_overflow_arith::integer_overflow_arith;
pub use label_by_family::label_by_family;
pub use lock_dominates::lock_dominates;
pub use path_canonical::path_canonical;
pub use path_reconstruct::path_reconstruct;
pub use reporter::{
    render_security_reporter_output, SecurityReporterError, SecurityReporterFinding,
    SecurityReporterOutputBytes, SecurityReporterPlannerPath, SecurityReporterSourceFile,
    SECURITY_REPORTER_SCHEMA_VERSION,
};
pub use predicate_catalog::{
    security_predicate_row_by_op_id, security_predicate_rows, try_security_predicate_rows,
    SecurityPredicateOperation, SecurityPredicateRow,
};
pub use relation_analyzer::{
    generated_relation_finding_fact_ids, run_generated_security_relation_analyzer,
    GeneratedSecurityRelationAnalyzerEvidence, GeneratedSecurityRelationAnalyzerReport,
    GeneratedSecurityRelationAnalyzerRunStats, GeneratedSecurityRelationAnalyzerSpec,
    SecurityRelationAnalyzerError, SecurityRelationQueryFamily,
    SECURITY_RELATION_ANALYZER_SCHEMA_VERSION,
};
pub use sanitized_by::sanitized_by;
pub use sanitizer_dominates::sanitizer_dominates;
pub use sink_intersection::sink_intersection;
pub use sql_param_bound::sql_param_bound;
pub use taint_flow::taint_flow;
pub use taint_kill::taint_kill;
pub use taint_pollution::taint_pollution;
pub use unchecked_return::unchecked_return;
pub use xss_escape::xss_escape;

/// Validate that a security composition's input shape + buffer names
/// are non-degenerate. Panics with a `Fix:` message on violation so
/// downstream substrate errors don't surface as cryptic OOB indices.
///
/// The contract is: every security op rejects degenerate input rather
/// than building a Program that traps inside the reference interpreter
/// (or worse, runs to completion and emits silently-wrong taint sets).
pub(crate) fn assert_security_inputs(op: &str, node_count: u32, buffers: &[(&str, &str)]) {
    assert!(
        node_count > 0,
        "Fix: {op} node_count must be positive; got 0. \
         A taint analysis over an empty program graph has no meaningful \
         result  -  callers must skip empty translation units before lowering."
    );
    for (role, name) in buffers {
        assert!(
            !name.is_empty(),
            "Fix: {op} requires non-empty buffer name for {role}. \
             Empty buffer names alias to the zero-length lookup key in the \
             validator and produce silent miscompiles. Pass a stable \
             non-empty buffer identifier."
        );
    }
}
