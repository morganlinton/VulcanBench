//! Declarative Rewrite DSL for algebraic simplifications.
//!
//! This module provides the `rewrite_rules!` macro, which allows passes to define
//! algebraic rewrites and peephole optimizations in a declarative, readable format
//! rather than writing massive imperative match statements.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::ir::{BinOp, Expr};
//! use crate::optimizer::dsl::rewrite_rules;
//!
//! let mut rules = rewrite_rules! {
//!     // Eliminate `X + 0`
//!     add_zero: Expr::BinOp { op: BinOp::Add, left, right } if matches!(**right, Expr::LitU32(0)) => {
//!         left.as_ref().clone()
//!     },
//!
//!     // Eliminate `X * 1`
//!     mul_one: Expr::BinOp { op: BinOp::Mul, left, right } if matches!(**right, Expr::LitU32(1)) => {
//!         left.as_ref().clone()
//!     },
//! };
//!
//! // Pass the closure directly into `rewrite_program` or `rewrite_expr`
//! // let (program, changed) = rewrite_program(program, &mut rules);
//! ```

/// Defines a set of declarative rewrite rules that compile into a `|&Expr| -> Option<Expr>` closure.
#[macro_export]
macro_rules! rewrite_rules {
    (
        $(
            $name:ident: $pattern:pat $(if $guard:expr)? => $replacement:expr
        ),* $(,)?
    ) => {
        |expr: &$crate::ir::Expr| match expr {
            $(
                $pattern $(if $guard)? => Some($replacement),
            )*
            _ => None,
        }
    }
}
