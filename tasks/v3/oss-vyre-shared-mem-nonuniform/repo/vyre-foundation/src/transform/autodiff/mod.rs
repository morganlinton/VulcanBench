//! RFC-0002  -  Reverse-mode automatic differentiation as an IR transform.
//!
//! Consumes a forward `Program` + a set of output buffer names + a set of
//! input buffer names, and emits a new `Program` that computes the gradient
//! of the outputs with respect to the inputs via reverse-mode accumulation.
//!
//! # Design
//!
//! The transform walks the forward IR backwards, applying the chain rule to
//! every differentiable expression. For each forward `Node::Store { buf, idx, val }`,
//! it emits an adjoint load from the gradient-of-buf buffer, then propagates
//! that adjoint through `val`'s expression tree.
//!
//! Gradient buffers are named `grad_<original>` and declared as `ReadWrite`
//! output buffers (accumulated via `+=`).
//!
//! # Differentiable coverage
//!
//! - `BinOp::{Add, Sub, Mul, Div, Min, Max}`  -  standard rules
//! - `UnOp::{Negate, Exp, Log, Sqrt, Tanh, Sin, Cos, Abs, Sinh, Cosh}`  -  standard rules
//! - `Expr::Select`  -  pushes adjoint to the selected branch
//! - `Expr::Fma { a, b, c }`  -  `d/da = b`, `d/db = a`, `d/dc = 1`
//! - `Expr::Load`  -  routes adjoint to the loaded buffer's gradient
//! - Integer / bitwise / comparison ops → `AutodiffError::NotDifferentiable`

pub mod error;
pub mod grad;
pub mod rules;

pub use error::AutodiffError;
pub use grad::{grad, grad_with_pullback, PullbackMap};
