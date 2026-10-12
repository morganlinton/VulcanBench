//! Regression: the inliner must alpha-rename callee-local variables in EVERY
//! expression position, including a `Node::Trap` address and the offset/size of
//! `Node::AsyncLoad` / `Node::AsyncStore`.
//!
//! Before the fix: `expand_callee` rewrote callee-local names (`let x` ->
//!                 `_vyre_inlN_x`) in normal positions, but cloned `Node::Trap`
//!                 verbatim and rebuilt AsyncLoad/AsyncStore with
//!                 `(**offset).clone()` / `(**size).clone()` — so a callee-local
//!                 referenced in a trap address or an async offset/size kept its
//!                 ORIGINAL name and dangled against the renamed declaration
//!                 (`_vyre_inlN_x`), producing a reference to an undeclared
//!                 variable in the inlined program.
//!
//! After the fix: those positions route through `self.expr(...)`, so the
//!                offset / address Var is renamed to match its declaration.

use std::collections::HashSet;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::transform::inline::inline_calls_with_resolver;

/// Callee whose `Trap` address is a callee-local variable (`off`). The trailing
/// `Store` to the output buffer is what makes the callee inlinable (the expander
/// requires it to observe a write to the output).
fn callee_with_trap_local() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("src", 0, BufferAccess::ReadOnly, DataType::U32).with_count(64),
            BufferDecl::output("result", 1, DataType::U32).with_count(64),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("off", Expr::u32(7)),
            Node::Trap {
                address: Box::new(Expr::var("off")),
                tag: "trap_tag".into(),
            },
            Node::Store {
                buffer: "result".into(),
                index: Expr::u32(0),
                value: Expr::var("off"),
            },
        ],
    )
}

fn caller_calling(op_id: &str) -> Program {
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

fn trap_resolver(id: &str) -> Option<Program> {
    if id == "trap_op" {
        Some(callee_with_trap_local())
    } else {
        None
    }
}

/// Collect every name a `let` / loop introduces, descending all node bodies.
fn collect_declared(nodes: &[Node], out: &mut HashSet<String>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } => {
                out.insert(name.to_string());
            }
            Node::Loop { var, body, .. } => {
                out.insert(var.to_string());
                collect_declared(body, out);
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_declared(then, out);
                collect_declared(otherwise, out);
            }
            Node::Block(body) => collect_declared(body, out),
            Node::Region { body, .. } => collect_declared(body, out),
            _ => {}
        }
    }
}

/// Find the first `Node::Trap` whose address is a bare `Var` and return its name.
fn find_trap_address_var(nodes: &[Node]) -> Option<String> {
    for node in nodes {
        match node {
            Node::Trap { address, .. } => {
                if let Expr::Var(name) = address.as_ref() {
                    return Some(name.to_string());
                }
            }
            Node::If {
                then, otherwise, ..
            } => {
                if let Some(found) = find_trap_address_var(then).or_else(|| find_trap_address_var(otherwise)) {
                    return Some(found);
                }
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                if let Some(found) = find_trap_address_var(body) {
                    return Some(found);
                }
            }
            Node::Region { body, .. } => {
                if let Some(found) = find_trap_address_var(body) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn inliner_renames_callee_local_in_trap_address() {
    let inlined = inline_calls_with_resolver(&caller_calling("trap_op"), trap_resolver)
        .expect("inlining a callee with a trap must succeed");

    let mut declared = HashSet::new();
    collect_declared(inlined.entry(), &mut declared);

    let addr_var = find_trap_address_var(inlined.entry())
        .expect("the callee's Trap node must survive inlining");

    // The callee declared `off`; after inlining it is renamed (e.g.
    // `_vyre_inl0_off`). The trap address must reference that SAME renamed
    // name, i.e. a name that is actually declared in the inlined body. The
    // pre-fix inliner left the address as the bare `off`, which no longer
    // exists once `let off` became `_vyre_inl0_off` — a dangling reference.
    assert_ne!(
        addr_var, "off",
        "the callee-local trap address must be alpha-renamed, not left bare as `off`"
    );
    assert!(
        declared.contains(&addr_var),
        "inlined Trap address references `{addr_var}`, which is not declared in the \
         inlined body (declared names: {declared:?}). The inliner failed to alpha-rename \
         the trap address, leaving a dangling reference to a callee-local variable."
    );
}
