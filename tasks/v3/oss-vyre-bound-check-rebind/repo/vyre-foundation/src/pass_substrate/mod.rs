#![allow(missing_docs)]
pub mod adjustment_set_pass_dependency;
pub mod dataflow_fixpoint;
pub mod functorial_pass_composition;
// `matroid_megakernel_scheduler` + `megakernel_schedule` relocated to
// `optimizer::megakernel::{matroid_subset, schedule_oracle}` in audit
// cleanup A9 (2026-04-30)  -  megakernel-fusion scheduler concept now
// lives in one place under the optimizer.
pub mod multigrid_matroid_solver;
pub mod polyhedral_fusion;
pub mod string_diagram_ir_rewrite;
pub mod tensor_network_fusion_order;
