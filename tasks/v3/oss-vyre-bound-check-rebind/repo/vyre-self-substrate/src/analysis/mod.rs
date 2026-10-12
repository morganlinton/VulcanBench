//! Static-analysis, fixpoint, diagnostics, and verification substrate modules.

pub mod cost_model;
pub mod dataflow_fixpoint;
pub mod decision_telemetry;
pub mod diagnostic_aggregation;
pub mod diagnostic_comparison;
pub mod effect_signature_check;
pub mod incremental_invalidation;
pub mod knowledge_compile_pass_precondition;
pub mod linear_type_check;
pub mod persistent_fixpoint_program;
pub mod shape_smt_check;
