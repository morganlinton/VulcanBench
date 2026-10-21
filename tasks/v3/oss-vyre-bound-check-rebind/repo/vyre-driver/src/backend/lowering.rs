//! Decentralized trait-owned lowering interfaces.
//!
//! This module defines the core trait boundaries for operations to emit
//! themselves into backend-owned target IRs directly. This decentralizes
//! the lowering monolith and ensures operations own their compilation rules.

use vyre_foundation::ir::Program;

/// Represents context provided to an operation during target expression generation.
pub trait TargetGenCtx {
    /// Register a target expression for the op being lowered. The
    /// `format` string is opaque to the trait  -  backends interpret it
    /// per their own emit conventions.
    fn register_expression(&mut self, format: &str) -> Result<(), ()>;
}

/// A target-agnostic context payload bounds ops that can be lowered.
pub trait LowerableOp: Send + Sync + 'static {
    /// Lower the operation into the target expression context.
    fn lower_expression(&self, ctx: &mut dyn TargetGenCtx, program: &Program)
        -> Result<(), String>;

    /// Lower the operation into a target binary context.
    fn lower_binary(&self, ctx: &mut (), program: &Program) -> Result<(), String>;
}
