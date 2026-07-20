//! ROADMAP A15  -  buffer aliasing facts into load elision.
//!
//! Read-only-buffer slice shipped here. When both arms of an
//! `Node::If` begin with a `Let(name, Load(buf, idx))` whose
//! `buf` is declared `BufferAccess::ReadOnly` AND the same name +
//! same index, the Load is hoisted before the If. The ReadOnly
//! declaration is the alias proof: a ReadOnly buffer is fully
//! initialised by the host before kernel launch, so the Load is
//! observably-safe to execute on the unconditional path  -  there
//! is no observable difference between "load was already issued"
//! and "load was about to be issued in one arm only".
//!
//! Op id: `vyre-foundation::optimizer::passes::read_only_load_hoist`.
//! Soundness: `Exact`. The ReadOnly access mode is enforced by the
//! buffer table; any pass that mutates a ReadOnly buffer is a
//! validation error caught by `Program::validate()`. Therefore the
//! Load result is invariant under the If's two execution paths,
//! and hoisting the Load to the unconditional path produces the
//! same value at every read site.
//!
//! Cost direction: monotone-down on `node_count` (one fewer Let
//! per fired hoist) and monotone-down on per-arm dispatch overhead
//! (the Load is issued once instead of once per branch).
//!
//! Preserves: every analysis. Invalidates: nothing  -  the hoisted
//! Load is the alias-proof-licensed counterpart of A18's
//! observably-free prefix hoist for non-Load values.
//!
//! ## Pattern
//!
//! ```text
//! If(cond,
//!    [Let(x, Load(ro_buf, idx)), then_rest...],
//!    [Let(x, Load(ro_buf, idx)), other_rest...])
//!     where program.buffer(ro_buf).access() == BufferAccess::ReadOnly
//!     AND idx is observably-free
//! → Let(x, Load(ro_buf, idx)); If(cond, [then_rest...], [other_rest...])
//! ```
//!
//! Idx must be observably-free because the index expression also
//! becomes unconditional after the hoist.
//!
//! ## Why this is A15
//!
//! A15 says "buffer aliasing facts into load elision". The full
//! alias substrate (proving two arbitrary buffers don't alias) is
//! a downstream alias analysis. ReadOnly is the trivial alias proof: a buffer
//! that nobody writes cannot alias with any write target, so its
//! Loads are invariant across control flow. Shipping the trivial
//! slice here gives the hot path the same code-size win that the
//! full aliasing substrate would deliver, while the fact-driven
//! variant lands beside the downstream alias pass.

use crate::ir::{BufferAccess, Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::bound_names::count_bound_names;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Hoist Loads on declared-ReadOnly buffers out of common
/// branch prefixes.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "read_only_load_hoist",
    requires = [],
    invalidates = [],
    phase = "memory",
    boundary_class = "abi_preserving",
    cost_model_family = "memory"
)]
pub struct ReadOnlyLoadHoistPass;

impl ReadOnlyLoadHoistPass {
    /// Skip programs with no candidate `If`.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // The hoist needs an If with two arms that both load from a
        // ReadOnly buffer. Without an If, no candidate is possible.
        if !program.stats().has_node_if() {
            return PassAnalysis::SKIP;
        }
        let read_only = read_only_buffer_set(program);
        if read_only.is_empty() {
            return PassAnalysis::SKIP;
        }
        let mut found = false;
        for node in program.entry() {
            if has_candidate(node, &read_only) {
                found = true;
                break;
            }
        }
        if found {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and hoist common Read-Only-Load prefixes.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let read_only = read_only_buffer_set(&program);
        if read_only.is_empty() {
            return PassResult {
                program,
                changed: false,
            };
        }
        let mut changed = false;
        let program = program.map_entry(|entry| hoist_in_body(entry, &read_only, &mut changed));
        PassResult { program, changed }
    }
}

fn read_only_buffer_set(program: &Program) -> FxHashSet<crate::ir::Ident> {
    program
        .buffers()
        .iter()
        .filter(|b| matches!(b.access(), BufferAccess::ReadOnly))
        .map(|b| crate::ir::Ident::from(b.name.as_ref()))
        .collect()
}

/// Hoist common Read-Only-Load prefixes out of every `If` in `body`, after
/// recursing into nested bodies.
///
/// Hoisting a prefix `let x = load(ro, idx)` to before the `If` moves `x` from
/// arm scope -- which the block-scoped IR pops at arm exit -- to THIS enclosing
/// scope, where `x` now lives across the `If` and the rest of `body`. That is
/// sound only if no other node in `body` binds `x`; otherwise the hoisted
/// binding collides with that other binder, which the validator rejects as a
/// duplicate sibling (V032) or a shadow (V008). A name bound at the front of
/// both arms is counted exactly twice over `body` (once per arm) iff this `If`
/// is its only binder, so `count_bound_names(body)[x] == 2` is the scope-safety
/// gate (see `extract_common_prefix`).
fn hoist_in_body(body: Vec<Node>, read_only: &FxHashSet<Ident>, changed: &mut bool) -> Vec<Node> {
    // Recurse first so nested `If`s hoist within their own bodies; any prefix a
    // nested `If` lifts up becomes a sibling here and is reflected in the counts.
    let recursed: Vec<Node> = body
        .into_iter()
        .map(|node| recurse_children(node, read_only, changed))
        .collect();

    let mut body_counts: FxHashMap<Ident, usize> = FxHashMap::default();
    count_bound_names(&recursed, &mut body_counts);

    let mut out = Vec::with_capacity(recursed.len());
    for node in recursed {
        if let Node::If {
            cond,
            then,
            otherwise,
        } = node
        {
            let (prefix, new_then, new_otherwise) =
                extract_common_prefix(then, otherwise, read_only, &body_counts);
            if !prefix.is_empty() {
                *changed = true;
                out.extend(prefix);
            }
            out.push(Node::If {
                cond,
                then: new_then,
                otherwise: new_otherwise,
            });
        } else {
            out.push(node);
        }
    }
    out
}

fn recurse_children(node: Node, read_only: &FxHashSet<Ident>, changed: &mut bool) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: hoist_in_body(then, read_only, changed),
            otherwise: hoist_in_body(otherwise, read_only, changed),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from,
            to,
            body: hoist_in_body(body, read_only, changed),
        },
        Node::Block(body) => Node::Block(hoist_in_body(body, read_only, changed)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: Arc::new(hoist_in_body(body_vec, read_only, changed)),
            }
        }
        other => other,
    }
}

fn extract_common_prefix(
    mut then: Vec<Node>,
    mut otherwise: Vec<Node>,
    read_only: &FxHashSet<Ident>,
    body_counts: &FxHashMap<Ident, usize>,
) -> (Vec<Node>, Vec<Node>, Vec<Node>) {
    let prefix_len = then
        .iter()
        .zip(otherwise.iter())
        .take_while(|(t, o)| {
            // Structurally hoistable AND scope-safe: bound only by this `If`'s
            // two arm prefixes (count == 2), so hoisting introduces no
            // colliding sibling/shadow binding in the enclosing scope.
            is_hoistable_load_pair(t, o, read_only)
                && matches!(t, Node::Let { name, .. } if body_counts.get(name).copied().unwrap_or(0) == 2)
        })
        .count();
    if prefix_len == 0 {
        return (Vec::new(), then, otherwise);
    }
    let prefix = then.drain(..prefix_len).collect();
    otherwise.drain(..prefix_len);
    (prefix, then, otherwise)
}

/// The structural half of the hoist test: both arms bind the SAME name to the
/// SAME read-only load with an observably-free index. Scope safety (the name
/// not being bound elsewhere in the enclosing body) is gated separately in
/// [`extract_common_prefix`] via `body_counts`; the analyze path uses only this
/// structural predicate, so it may over-approximate `RUN` (a no-op transform).
fn is_hoistable_load_pair(a: &Node, b: &Node, read_only: &FxHashSet<Ident>) -> bool {
    let Node::Let {
        name: name_a,
        value: value_a,
    } = a
    else {
        return false;
    };
    let Node::Let {
        name: name_b,
        value: value_b,
    } = b
    else {
        return false;
    };
    if name_a != name_b || value_a != value_b {
        return false;
    }
    matches!(value_a, Expr::Load { buffer, index } if read_only.contains(buffer) && index_is_observably_free(index))
}

fn index_is_observably_free(expr: &Expr) -> bool {
    match expr {
        Expr::Load { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => true,
        Expr::BinOp { left, right, .. } => {
            index_is_observably_free(left) && index_is_observably_free(right)
        }
        Expr::UnOp { operand, .. } => index_is_observably_free(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            index_is_observably_free(cond)
                && index_is_observably_free(true_val)
                && index_is_observably_free(false_val)
        }
        Expr::Cast { value, .. } => index_is_observably_free(value),
        Expr::Fma { a, b, c } => {
            index_is_observably_free(a)
                && index_is_observably_free(b)
                && index_is_observably_free(c)
        }
    }
}

fn has_candidate(node: &Node, read_only: &FxHashSet<crate::ir::Ident>) -> bool {
    match node {
        Node::If {
            then, otherwise, ..
        } => match (then.first(), otherwise.first()) {
            (Some(t), Some(o)) => {
                is_hoistable_load_pair(t, o, read_only)
                    || then.iter().any(|n| has_candidate(n, read_only))
                    || otherwise.iter().any(|n| has_candidate(n, read_only))
            }
            _ => {
                then.iter().any(|n| has_candidate(n, read_only))
                    || otherwise.iter().any(|n| has_candidate(n, read_only))
            }
        },
        Node::Loop { body, .. } => body.iter().any(|n| has_candidate(n, read_only)),
        Node::Block(body) => body.iter().any(|n| has_candidate(n, read_only)),
        Node::Region { body, .. } => body.iter().any(|n| has_candidate(n, read_only)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Ident, Node};

    fn ro_buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadOnly, DataType::U32).with_count(8)
    }

    fn rw_buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 1, BufferAccess::ReadWrite, DataType::U32).with_count(8)
    }

    fn program(buffers: Vec<BufferDecl>, entry: Vec<Node>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], entry)
    }

    fn find_siblings(nodes: &[Node]) -> Option<&[Node]> {
        if nodes
            .iter()
            .any(|n| matches!(n, Node::Let { .. } | Node::If { .. }))
        {
            return Some(nodes);
        }
        for n in nodes {
            let body = match n {
                Node::Block(body) => body.as_slice(),
                Node::Region { body, .. } => body.as_ref().as_slice(),
                _ => continue,
            };
            if let Some(found) = find_siblings(body) {
                return Some(found);
            }
        }
        None
    }

    /// Positive: Load on a ReadOnly buffer at the start of both arms
    /// hoists out before the If.
    #[test]
    fn hoists_read_only_load_prefix() {
        let load = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(0)),
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::let_bind("x", load.clone()),
                Node::store("rw", Expr::u32(0), Expr::var("x")),
            ],
            otherwise: vec![
                Node::let_bind("x", load),
                Node::store("rw", Expr::u32(1), Expr::var("x")),
            ],
        }];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(result.changed, "ReadOnly Load prefix must hoist");
        let siblings =
            find_siblings(result.program.entry()).expect("Fix: hoisted Let + If present");
        assert!(matches!(&siblings[0], Node::Let { name, value }
            if name.as_str() == "x" && matches!(value, Expr::Load { .. })));
        assert!(matches!(&siblings[1], Node::If { .. }));
    }

    /// Negative: Load on a ReadWrite buffer must NOT hoist (alias
    /// proof unavailable; another arm could write between the If and
    /// the post-If sequencing).
    #[test]
    fn keeps_read_write_load() {
        let load = Expr::Load {
            buffer: Ident::from("rw"),
            index: Box::new(Expr::u32(0)),
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", load.clone())],
            otherwise: vec![Node::let_bind("x", load)],
        }];
        let prog = program(vec![rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(!result.changed, "ReadWrite Load must not hoist");
    }

    /// Negative: differing names block the hoist.
    #[test]
    fn keeps_when_names_differ() {
        let load = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(0)),
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", load.clone())],
            otherwise: vec![Node::let_bind("y", load)],
        }];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(!result.changed, "differing names must not hoist");
    }

    /// Negative: differing indices block the hoist.
    #[test]
    fn keeps_when_indices_differ() {
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind(
                "x",
                Expr::Load {
                    buffer: Ident::from("ro"),
                    index: Box::new(Expr::u32(0)),
                },
            )],
            otherwise: vec![Node::let_bind(
                "x",
                Expr::Load {
                    buffer: Ident::from("ro"),
                    index: Box::new(Expr::u32(1)),
                },
            )],
        }];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);

        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(!result.changed, "differing indices must not hoist");
    }

    /// Negative: an index expression that itself contains a Load
    /// blocks the hoist (the index Load could observe state that
    /// the unconditional path shouldn't trigger).
    #[test]
    fn keeps_when_index_reads_memory() {
        let load = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::Load {
                buffer: Ident::from("rw"),
                index: Box::new(Expr::u32(0)),
            }),
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![Node::let_bind("x", load.clone())],
            otherwise: vec![Node::let_bind("x", load)],
        }];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(!result.changed, "index that reads memory must block hoist");
    }

    /// `analyze` short-circuits when the program declares no
    /// ReadOnly buffer.
    #[test]
    fn analyze_skips_program_with_no_read_only_buffer() {
        let entry = vec![Node::store("rw", Expr::u32(0), Expr::u32(1))];
        let prog = program(vec![rw_buf("rw")], entry);
        match crate::optimizer::ProgramPass::analyze(&ReadOnlyLoadHoistPass, &prog) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }

    /// Positive end-to-end smoke: chain of two ReadOnly Loads with
    /// different indices in the prefix hoists both.
    #[test]
    fn hoists_chain_of_read_only_loads() {
        let load_a = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(0)),
        };
        let load_b = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(1)),
        };
        let entry = vec![Node::If {
            cond: Expr::var("c"),
            then: vec![
                Node::let_bind("a", load_a.clone()),
                Node::let_bind("b", load_b.clone()),
                Node::store("rw", Expr::u32(0), Expr::var("a")),
            ],
            otherwise: vec![
                Node::let_bind("a", load_a),
                Node::let_bind("b", load_b),
                Node::store("rw", Expr::u32(1), Expr::var("b")),
            ],
        }];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(result.changed, "chain of ReadOnly Loads must hoist");
        let siblings =
            find_siblings(result.program.entry()).expect("Fix: hoisted Lets + If present");
        assert!(siblings.len() >= 3);
        assert!(matches!(&siblings[0], Node::Let { name, .. } if name.as_str() == "a"));
        assert!(matches!(&siblings[1], Node::Let { name, .. } if name.as_str() == "b"));
    }

    /// Negative (scope extension): the hoisted name `x` is rebound by a later
    /// sibling. Hoisting moves `x` from arm scope to the enclosing scope, where
    /// it lives across the If and collides with the trailing `let x` -- a
    /// duplicate sibling binding the validator rejects (V032). The pass must
    /// decline. (Oracle-differential proof: tests/read_only_load_hoist_scope.rs.)
    #[test]
    fn keeps_when_hoisted_name_rebound_by_later_sibling() {
        let load = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(0)),
        };
        let entry = vec![
            Node::If {
                cond: Expr::var("c"),
                then: vec![
                    Node::let_bind("x", load.clone()),
                    Node::store("rw", Expr::u32(0), Expr::var("x")),
                ],
                otherwise: vec![
                    Node::let_bind("x", load),
                    Node::store("rw", Expr::u32(1), Expr::var("x")),
                ],
            },
            Node::let_bind("x", Expr::u32(7)), // rebinds `x` after the If
            Node::store("rw", Expr::u32(2), Expr::var("x")),
        ];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(
            !result.changed,
            "hoisting `x` would collide with the later `let x`; pass must decline"
        );
    }

    /// Positive (no over-block): a later sibling that binds a DIFFERENT name
    /// must not block the hoist -- the scope-safety gate keys on the hoisted
    /// name only.
    #[test]
    fn hoists_when_later_sibling_binds_a_different_name() {
        let load = Expr::Load {
            buffer: Ident::from("ro"),
            index: Box::new(Expr::u32(0)),
        };
        let entry = vec![
            Node::If {
                cond: Expr::var("c"),
                then: vec![
                    Node::let_bind("x", load.clone()),
                    Node::store("rw", Expr::u32(0), Expr::var("x")),
                ],
                otherwise: vec![
                    Node::let_bind("x", load),
                    Node::store("rw", Expr::u32(1), Expr::var("x")),
                ],
            },
            Node::let_bind("y", Expr::u32(7)), // different name; no collision
            Node::store("rw", Expr::u32(2), Expr::var("y")),
        ];
        let prog = program(vec![ro_buf("ro"), rw_buf("rw")], entry);
        let result = ReadOnlyLoadHoistPass::transform(prog);
        assert!(
            result.changed,
            "a later sibling binding a different name must not block the hoist"
        );
        let siblings = find_siblings(result.program.entry()).expect("hoisted Let + If present");
        assert!(matches!(&siblings[0], Node::Let { name, .. } if name.as_str() == "x"));
    }
}
