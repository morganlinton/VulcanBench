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

use crate::ir::{BufferAccess, Expr, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use rustc_hash::FxHashSet;
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
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|node| hoist_prefix(node, &read_only, &mut changed))
                .collect()
        });
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

fn hoist_prefix(
    node: Node,
    read_only: &FxHashSet<crate::ir::Ident>,
    changed: &mut bool,
) -> Vec<Node> {
    let recursed = recurse_children(node, read_only, changed);
    if let Node::If {
        cond,
        then,
        otherwise,
    } = recursed
    {
        let (prefix, new_then, new_otherwise) = extract_common_prefix(then, otherwise, read_only);
        if !prefix.is_empty() {
            *changed = true;
            let mut out = prefix;
            out.push(Node::If {
                cond,
                then: new_then,
                otherwise: new_otherwise,
            });
            return out;
        }
        return vec![Node::If {
            cond,
            then: new_then,
            otherwise: new_otherwise,
        }];
    }
    vec![recursed]
}

fn recurse_children(
    node: Node,
    read_only: &FxHashSet<crate::ir::Ident>,
    changed: &mut bool,
) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond,
            then: then
                .into_iter()
                .flat_map(|n| hoist_prefix(n, read_only, changed))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .flat_map(|n| hoist_prefix(n, read_only, changed))
                .collect(),
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
            body: body
                .into_iter()
                .flat_map(|n| hoist_prefix(n, read_only, changed))
                .collect(),
        },
        Node::Block(body) => Node::Block(
            body.into_iter()
                .flat_map(|n| hoist_prefix(n, read_only, changed))
                .collect(),
        ),
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
                body: Arc::new(
                    body_vec
                        .into_iter()
                        .flat_map(|n| hoist_prefix(n, read_only, changed))
                        .collect(),
                ),
            }
        }
        other => other,
    }
}

fn extract_common_prefix(
    mut then: Vec<Node>,
    mut otherwise: Vec<Node>,
    read_only: &FxHashSet<crate::ir::Ident>,
) -> (Vec<Node>, Vec<Node>, Vec<Node>) {
    let prefix_len = then
        .iter()
        .zip(otherwise.iter())
        .take_while(|(t, o)| is_hoistable_pair(t, o, read_only))
        .count();
    if prefix_len == 0 {
        return (Vec::new(), then, otherwise);
    }
    let prefix = then.drain(..prefix_len).collect();
    otherwise.drain(..prefix_len);
    (prefix, then, otherwise)
}

fn is_hoistable_pair(a: &Node, b: &Node, read_only: &FxHashSet<crate::ir::Ident>) -> bool {
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
                is_hoistable_pair(t, o, read_only)
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
}
