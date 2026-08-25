//! Compile-time expansion of `Expr::Call` composition nodes.
//!
//! Calls are resolved against Category A operation programs and expanded into
//! ordinary IR before backend lowering. No runtime dispatch or GPU-side
//! interpreter is introduced by this pass.

use crate::error::{Error, Result};
use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::{BufferDecl, Program};
use crate::ir_inner::model::types::{BufferAccess, DataType};
use rustc_hash::FxHashMap as HashMap;

/// Resolve an operation id to the canonical IR program for that operation.
pub type OpResolver = fn(&str) -> Option<Program>;

/// Inline all `Expr::Call` nodes in a program using the built-in operation set.
///
/// # Errors
///
/// Returns [`Error::InlineUnknownOp`] when a call cannot be resolved,
/// [`Error::InlineNonInlinable`] when a registered operation must dispatch as a
/// separate kernel, and [`Error::InlineCycle`] when recursive operation
/// composition is detected.
#[inline]
#[must_use]
pub fn inline_calls(program: &Program) -> Result<Program> {
    inline_calls_with_resolver(program, default_resolver)
}

/// Inline all `Expr::Call` nodes with a caller-supplied operation resolver.
///
/// This entry point exists for tests and embedders that provide their own
/// operation registry. The resolver must return complete Category A programs;
/// intrinsic-only operations are not valid inline targets.
///
/// # Errors
///
/// Returns [`Error::InlineUnknownOp`] when a call cannot be resolved,
/// [`Error::InlineNonInlinable`] when a registered operation must dispatch as a
/// separate kernel, and [`Error::InlineCycle`] when recursive operation
/// composition is detected.
#[inline]
#[must_use]
pub fn inline_calls_with_resolver(program: &Program, resolver: OpResolver) -> Result<Program> {
    let mut ctx = InlineCtx::new(resolver);
    let entry = ctx.inline_nodes(program.entry())?;
    // Reuse the buffer Arc + buffer_index from the source program instead
    // of re-cloning + re-interning via Program::wrapped.
    Ok(program.with_rewritten_wrapped_entry(entry))
}

/// Resolve inline calls against the foundation-level empty registry.
///
/// Foundation does not host a dialect registry; the driver layer plugs its
/// `vyre_driver::registry::DialectRegistry` into call sites via
/// [`inline_calls_with_resolver`]. The default resolver therefore returns
/// `None` so that a direct call to [`inline_calls`] inside tests or
/// foundation-only consumers fails with [`Error::InlineUnknownOp`] on any
/// `Expr::Call`.
#[inline]
#[must_use]
pub fn default_resolver(_op_id: &str) -> Option<Program> {
    None
}

/// Mutable state for one inline expansion pass.
pub struct InlineCtx {
    /// Operation resolver used for `Expr::Call` targets.
    resolver: OpResolver,
    /// Active expansion stack used to reject recursive composition.
    stack: Vec<String>,
    /// Monotonic suffix for generated temporary names.
    next_call_id: usize,
}

mod expand;
mod impl_inlinectx;

/// Map a callee's input buffers to the argument expressions from a call site.
#[inline]
pub(crate) fn input_arg_map(callee: &Program, args: Vec<Expr>) -> HashMap<Ident, Expr> {
    let mut inputs = input_buffers(callee);
    inputs.sort_by_key(|buf| buf.binding());
    inputs
        .into_iter()
        .zip(args)
        .map(|(buf, arg)| (Ident::from(buf.name()), arg))
        .collect()
}

/// Return read-only and uniform buffers that receive call arguments.
#[must_use]
#[inline]
pub(crate) fn input_buffers(callee: &Program) -> Vec<&BufferDecl> {
    callee
        .buffers()
        .iter()
        .filter(|buf| matches!(buf.access(), BufferAccess::ReadOnly | BufferAccess::Uniform))
        .collect()
}

/// Return the single output buffer required for an inlineable callee.
///
/// # Errors
///
/// Returns an inline error when the callee has no output buffer or more than
/// one output buffer.
#[inline]
#[must_use]
pub fn output_buffer<'a>(op_id: &str, program: &'a Program) -> Result<&'a BufferDecl> {
    let outputs: Vec<&BufferDecl> = program
        .buffers()
        .iter()
        .filter(|buf| buf.is_output())
        .collect();
    match outputs.as_slice() {
        [output] => Ok(output),
        [] => Err(Error::InlineNoOutput {
            op_id: op_id.to_string(),
        }),
        outputs => Err(Error::InlineOutputCountMismatch {
            op_id: op_id.to_string(),
            got: outputs.len(),
        }),
    }
}

/// Construct the zero literal used when an inline target needs a default value.
#[inline]
#[must_use]
pub fn zero_value(ty: &DataType) -> Expr {
    match ty {
        DataType::I32 => Expr::i32(0),
        DataType::Bool => Expr::LitBool(false),
        DataType::F32 | DataType::F16 | DataType::BF16 | DataType::F64 => Expr::f32(0.0),
        _ => Expr::u32(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir_inner::model::expr::Expr;
    use crate::ir_inner::model::node::Node;
    use crate::ir_inner::model::program::BufferDecl;

    fn make_caller() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("A", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::Call {
                    op_id: "add_one".into(),
                    args: vec![Expr::Load {
                        buffer: "A".into(),
                        index: Box::new(Expr::u32(0)),
                    }],
                },
            )],
        )
    }

    fn make_callee() -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(1),
                BufferDecl::output("result", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store(
                "result",
                Expr::u32(0),
                Expr::add(
                    Expr::Load {
                        buffer: "x".into(),
                        index: Box::new(Expr::u32(0)),
                    },
                    Expr::u32(1),
                ),
            )],
        )
    }

    fn test_resolver(op_id: &str) -> Option<Program> {
        if op_id == "add_one" {
            Some(make_callee())
        } else {
            None
        }
    }

    #[test]
    fn test_inline_call_success() {
        let caller = make_caller();
        let inlined = inline_calls_with_resolver(&caller, test_resolver).unwrap();

        // The call should be gone
        let nodes = inlined.entry();
        // Since we inline a store, we expect an internal let for the argument or a direct replacement
        // Just verify we don't have Expr::Call anymore
        let mut has_call = false;
        let dump = format!("{nodes:?}");
        if dump.contains("Call {") {
            has_call = true;
        }
        assert!(!has_call, "Expr::Call should be inlined out: {dump}");
    }

    #[test]
    fn test_inline_unknown_op() {
        let caller = make_caller();
        // default_resolver always returns None
        let err = inline_calls(&caller).unwrap_err();
        assert!(matches!(err, Error::InlineUnknownOp { .. }));
    }

    #[test]
    fn test_zero_value() {
        assert_eq!(zero_value(&DataType::I32), Expr::i32(0));
        assert_eq!(zero_value(&DataType::F32), Expr::f32(0.0));
        assert_eq!(zero_value(&DataType::Bool), Expr::LitBool(false));
        assert_eq!(zero_value(&DataType::U32), Expr::u32(0));
    }
}
