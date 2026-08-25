//! Decomposable-NNF (d-DNNF) compiler primitive (P-PRIM-6).
//!
//! d-DNNF is the canonical knowledge-compilation target  -  every
//! Boolean formula is rewritten into a directed acyclic graph of
//! AND/OR gates over literals such that:
//!
//! * **Decomposability** (D): every AND gate's children share no
//!   variables.
//! * **Determinism** (d): every OR gate's children are pairwise
//!   inconsistent (logically disjoint).
//!
//! On a d-DNNF, model counting and weighted model counting are
//! linear in the gate count. The optimizer's
//! `knowledge_compile_pass_precondition` substrate consumer uses
//! a d-DNNF to evaluate which optimizer-pass-precondition
//! formulae a given Program satisfies  -  pass-precondition
//! evaluation drops from exponential (per pass × per Program)
//! to linear (per pass × per d-DNNF gate).
//!
//! The substrate ships the host-side compiler and the GPU-shaped
//! bottom-up evaluator. Compilation takes a CNF formula (vector of
//! clauses) and emits a d-DNNF DAG via the textbook "Shannon
//! decomposition + hash-cons" algorithm; evaluation composes the
//! compiled DAG buffers with the graph wave scheduler.

pub mod compile;

#[cfg(any(test, feature = "cpu-parity"))]
pub use crate::graph::knowledge_compile::ddnnf_evaluate_cpu;
pub use crate::graph::knowledge_compile::{
    ddnnf_evaluate, AND_NODE, LITERAL_FALSE, LITERAL_TRUE, OR_NODE,
};
pub use compile::{compile_dnnf, model_count, DnnfDag, DnnfGate};
