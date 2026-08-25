//! Loop-invariant code motion (LICM) over the post-prop Program.
//!
//! Walks the IR; for each `Node::Loop { var: i, body, .. }`, scans
//! the body for `Node::Let { name, value }` Lets whose `value` Expr
//! is **loop-invariant**  -  it doesn't reference `Var(i)`. Such Lets
//! get hoisted to a fresh sibling immediately before the Loop in
//! the parent scope, so the body re-using `Var(name)` sees the
//! same binding evaluated once instead of every iteration.
//!
//! Conservative V1 rules:
//!  - Only LIFT Lets whose value Expr is built from literals,
//!    Vars (other than the iter var), BinOp, UnOp, Select, Fma.
//!    Loads are skipped because the underlying buffer may be
//!    written by some other Node and the read isn't trivially
//!    invariant.
//!  - Only the iter var of the IMMEDIATELY enclosing Loop is
//!    considered. A Let inside a nested Loop only hoists out of
//!    the inner Loop when invariant w.r.t. the inner iter var.
//!  - Don't reorder relative to side-effecting Nodes (Store,
//!    Assign, Trap, Async*, Barrier, IndirectDispatch). If any
//!    such Node precedes the Let in the body, hoisting could
//!    change observable behaviour, so we leave the Let in place.
//!  - Don't hoist past `Assign`s to the same name (reassignments
//!    are tracked).

use std::sync::Arc;

use rustc_hash::FxHashSet;
use vyre_foundation::ir::{BufferAccess, Expr, Ident, Node, Program};

/// Apply LICM. Returns a new Program with loop-invariant Lets
/// hoisted to sibling positions immediately before their enclosing
/// Loop.
pub fn apply_licm(program: &Program) -> Program {
    // Names of buffers declared `ReadOnly`  -  Loads from these can
    // be hoisted because no Store inside the program writes to them.
    let read_only: FxHashSet<Ident> = program
        .buffers()
        .iter()
        .filter(|b| matches!(b.access, BufferAccess::ReadOnly))
        .map(|b| Ident::new(b.name.clone()))
        .collect();

    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };
    let new_body = rewrite_scope(&body, &read_only);

    let new_entry = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            ..
        }] => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(new_body),
        }],
        _ => new_body,
    };
    program.with_rewritten_entry(new_entry)
}

fn rewrite_scope(body: &[Node], read_only: &FxHashSet<Ident>) -> Vec<Node> {
    let prefix_len = super::encode::reachable_prefix_len(body);
    let mut out: Vec<Node> = Vec::with_capacity(prefix_len);
    for node in &body[..prefix_len] {
        match node {
            Node::Loop {
                var,
                from,
                to,
                body: loop_body,
            } => {
                // Recursively LICM the body's nested scopes first.
                let inner = rewrite_scope(loop_body, read_only);
                let (hoisted, kept) = split_invariants(var, &inner, read_only);
                // Emit hoisted Lets BEFORE the Loop; the Loop now
                // has only the kept Nodes.
                out.extend(hoisted);
                out.push(Node::loop_for(var.clone(), from.clone(), to.clone(), kept));
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                out.push(Node::if_then_else(
                    cond.clone(),
                    rewrite_scope(then, read_only),
                    rewrite_scope(otherwise, read_only),
                ));
            }
            Node::Block(b) => out.push(Node::Block(rewrite_scope(b, read_only))),
            Node::Region {
                generator,
                source_region,
                body,
            } => out.push(Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(rewrite_scope(body.as_slice(), read_only)),
            }),
            other => out.push(other.clone()),
        }
    }
    out
}

/// Partition `body` (a Loop's inner scope) into:
///  - Lets hoisted out: invariant w.r.t. `iter_var` and pure.
///  - Lets / other Nodes that stay inside the Loop.
///
/// The hoisted Lets are emitted in the same order they appeared,
/// so dependencies between hoistable Lets are preserved.
fn split_invariants(
    iter_var: &Ident,
    body: &[Node],
    read_only: &FxHashSet<Ident>,
) -> (Vec<Node>, Vec<Node>) {
    let mut hoisted: Vec<Node> = Vec::new();
    let mut kept: Vec<Node> = Vec::new();
    let mut hoisted_names: FxHashSet<Ident> = FxHashSet::default();
    // Names bound inside this loop body that we did NOT hoist.
    // A Let referencing one of these is not invariant  -  hoisting it
    // would put it above the loop where the dependency is undefined.
    let mut local_unhoisted: FxHashSet<Ident> = FxHashSet::default();
    // `false` once we encounter a side-effecting Node  -  anything
    // after that point cannot be reordered above the Loop without
    // changing observable behaviour.
    let mut still_safe = true;
    for node in body {
        if !still_safe {
            kept.push(node.clone());
            continue;
        }
        match node {
            Node::Let { name, value } => {
                if name == iter_var {
                    // Shadowing the iter var  -  keep in place.
                    kept.push(node.clone());
                    local_unhoisted.insert(name.clone());
                    continue;
                }
                if expr_is_invariant(value, iter_var, &hoisted_names, &local_unhoisted, read_only) {
                    hoisted.push(Node::let_bind(name.clone(), value.clone()));
                    hoisted_names.insert(name.clone());
                } else {
                    kept.push(node.clone());
                    local_unhoisted.insert(name.clone());
                }
            }
            // Side-effecting Nodes block further hoisting in this
            // scope  -  they could observe state that a hoisted Let
            // would otherwise read after.
            Node::Store { .. }
            | Node::Assign { .. }
            | Node::Trap { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Barrier { .. }
            | Node::IndirectDispatch { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {
                still_safe = false;
                kept.push(node.clone());
            }
            // Control flow: don't try to hoist out of nested If /
            // Loop scopes here (the recursion in `rewrite_scope`
            // already handled them). Just keep them in place.
            _ => kept.push(node.clone()),
        }
    }
    (hoisted, kept)
}

/// Whether `expr` is loop-invariant w.r.t. `iter_var`. Returns
/// `false` for any `Var(iter_var)` reference, any `Load` (we don't
/// reason about buffer aliasing here), or unsupported variants.
/// `hoisted_names` covers Lets we've already hoisted in this
/// scope: a Var reference to one of those is invariant since the
/// Let lives outside the Loop now.
#[allow(clippy::only_used_in_recursion)]
fn expr_is_invariant(
    expr: &Expr,
    iter_var: &Ident,
    hoisted: &FxHashSet<Ident>,
    local_unhoisted: &FxHashSet<Ident>,
    read_only: &FxHashSet<Ident>,
) -> bool {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::BufLen { .. } => true,
        // A Var is invariant when it doesn't reference the iter var
        // AND it doesn't reference a name bound (and not hoisted) in
        // this loop body. Names bound outside the loop or already
        // hoisted satisfy this trivially.
        Expr::Var(name) => name != iter_var && !local_unhoisted.contains(name),
        Expr::BinOp { left, right, .. } => {
            expr_is_invariant(left, iter_var, hoisted, local_unhoisted, read_only)
                && expr_is_invariant(right, iter_var, hoisted, local_unhoisted, read_only)
        }
        Expr::UnOp { operand, .. } => {
            expr_is_invariant(operand, iter_var, hoisted, local_unhoisted, read_only)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_is_invariant(cond, iter_var, hoisted, local_unhoisted, read_only)
                && expr_is_invariant(true_val, iter_var, hoisted, local_unhoisted, read_only)
                && expr_is_invariant(false_val, iter_var, hoisted, local_unhoisted, read_only)
        }
        Expr::Fma { a, b, c } => {
            expr_is_invariant(a, iter_var, hoisted, local_unhoisted, read_only)
                && expr_is_invariant(b, iter_var, hoisted, local_unhoisted, read_only)
                && expr_is_invariant(c, iter_var, hoisted, local_unhoisted, read_only)
        }
        // Loads from `ReadOnly` buffers are invariant (the substrate
        // disallows writes to them, so the value never changes).
        // Loads from RW buffers might alias a Store in the loop and
        // are kept conservative.
        Expr::Load { buffer, index } => {
            read_only.contains(buffer)
                && expr_is_invariant(index, iter_var, hoisted, local_unhoisted, read_only)
        }
        // Unsupported variants  -  be conservative.
        _ => false,
    }
}
