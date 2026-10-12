//! Type-discipline primitives (P-PRIM-14, P-PRIM-15, …).
//!
//! Pure-CPU type-checker primitives the optimizer / validate pipeline
//! consumes to reject ill-typed programs before lowering. Each
//! primitive is a single function with no IR-builder dependency so
//! it can run inside any layer of the workspace.

pub mod linear_check;
pub mod shape_smt;

pub use linear_check::{check_linear_use, LinearDiscipline, LinearTypeError};
pub use shape_smt::{evaluate as evaluate_shape, ShapeFormula};
