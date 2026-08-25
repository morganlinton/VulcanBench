//! Regression tests for VF-LOWER-003: the inliner must REJECT (hard Err) any
//! callee that references InvocationId / WorkgroupId / LocalId /
//! SubgroupLocalId / SubgroupSize rather than silently replacing them with 0.
//!
//! Before the fix:  every per-invocation built-in inside a callee body was
//!                  replaced with `Expr::u32(0)` and the call returned Ok — a
//!                  silent miscompile for every invocation other than lane 0.
//!
//! After the fix:   `inline_calls_with_resolver` returns
//!                  `Err(Error::Lowering { .. })` whose message names the
//!                  built-in and contains the string "Fix:".

use vyre_foundation::error::Error;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::inline::inline_calls_with_resolver;

/// Build a callee program whose store index uses `built_in_expr`.
fn callee_with_builtin(built_in_expr: Expr) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(64),
            BufferDecl::output("result", 1, DataType::U32).with_count(64),
        ],
        [64, 1, 1],
        vec![Node::Store {
            buffer: "result".into(),
            index: built_in_expr,
            value: Expr::u32(42),
        }],
    )
}

/// Build a caller that invokes `op_id` with a literal argument.
fn caller_for(op_id: &str) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("x", 0, BufferAccess::ReadOnly, DataType::U32).with_count(64),
            BufferDecl::output("out", 1, DataType::U32).with_count(64),
        ],
        [64, 1, 1],
        vec![Node::Store {
            buffer: "out".into(),
            index: Expr::u32(0),
            value: Expr::call(op_id, vec![Expr::load("x", Expr::u32(0))]),
        }],
    )
}

/// Non-capturing resolver mapping each test's op_id to a callee that references
/// the per-invocation built-in under test. `inline_calls_with_resolver` takes an
/// `OpResolver = fn(&str) -> Option<Program>` (a fn pointer), so the resolver
/// must NOT capture — it builds each callee from scratch instead.
fn builtin_resolver(id: &str) -> Option<Program> {
    let built_in = match id {
        "uses_gid" => Expr::InvocationId { axis: 0 },
        "uses_wgid" => Expr::WorkgroupId { axis: 0 },
        "uses_lid" => Expr::LocalId { axis: 0 },
        "uses_sgid" => Expr::SubgroupLocalId,
        "uses_sgsize" => Expr::SubgroupSize,
        "uses_gid_in_binop" => {
            // InvocationId nested inside a BinOp (index = InvocationId.y + 1):
            // exercises the recursive expr() traversal in primitive.rs.
            return Some(Program::wrapped(
                vec![
                    BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32)
                        .with_count(64),
                    BufferDecl::output("out2", 1, DataType::U32).with_count(64),
                ],
                [64, 1, 1],
                vec![Node::Store {
                    buffer: "out2".into(),
                    index: Expr::add(Expr::InvocationId { axis: 1 }, Expr::u32(1)),
                    value: Expr::u32(7),
                }],
            ));
        }
        _ => return None,
    };
    Some(callee_with_builtin(built_in))
}

fn assert_lowering_error_names_fix(result: Result<Program, Error>, builtin_name: &str) {
    match result {
        Err(Error::Lowering { message }) => {
            assert!(
                message.contains("Fix:"),
                "Fix: inline rejection for {builtin_name} must include 'Fix:' guidance, \
                 got message: {message:?}"
            );
            // The message must name the class of built-ins it rejected.
            let names_a_builtin = message.contains("InvocationId")
                || message.contains("WorkgroupId")
                || message.contains("LocalId")
                || message.contains("SubgroupLocalId")
                || message.contains("SubgroupSize");
            assert!(
                names_a_builtin,
                "Fix: inline rejection for {builtin_name} must name the offending \
                 built-in class in the error message, got: {message:?}"
            );
        }
        Ok(_) => panic!(
            "Fix: inlining a callee that uses {builtin_name} must return Err(Lowering), \
             not Ok — the silent replacement of per-invocation built-ins with 0 \
             was a miscompile."
        ),
        Err(other) => panic!(
            "Fix: inlining a callee that uses {builtin_name} must return \
             Err(Lowering), got unexpected error variant {other:?}"
        ),
    }
}

#[test]
fn inline_rejects_callee_with_invocation_id() {
    let caller = caller_for("uses_gid");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "InvocationId");
}

#[test]
fn inline_rejects_callee_with_workgroup_id() {
    let caller = caller_for("uses_wgid");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "WorkgroupId");
}

#[test]
fn inline_rejects_callee_with_local_id() {
    let caller = caller_for("uses_lid");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "LocalId");
}

#[test]
fn inline_rejects_callee_with_subgroup_local_id() {
    let caller = caller_for("uses_sgid");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "SubgroupLocalId");
}

#[test]
fn inline_rejects_callee_with_subgroup_size() {
    let caller = caller_for("uses_sgsize");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "SubgroupSize");
}

/// Callee with InvocationId nested inside a BinOp (not as a bare Store index)
/// must also be rejected — verifies the recursive expr() traversal in primitive.rs.
#[test]
fn inline_rejects_callee_with_invocation_id_in_binop() {
    // builtin_resolver maps "uses_gid_in_binop" to a callee whose store index is
    // `InvocationId.y + 1` — exercises the recursive expr() BinOp traversal.
    let caller = caller_for("uses_gid_in_binop");
    let result = inline_calls_with_resolver(&caller, builtin_resolver);
    assert_lowering_error_names_fix(result, "InvocationId (inside BinOp)");
}
