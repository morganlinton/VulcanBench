//! ROADMAP A29  -  strip-mine large literal loops into tiled outer and
//! fixed-size inner loops.
//!
//! Soundness: `Exact`. For an original `i in from..to`, the rewrite
//! computes `i = from + tile_index * TILE + lane` and guards every
//! tiled body with `i < to`, so the transformed loop executes each
//! original iteration exactly once and executes no observable body for
//! padded tail lanes. Cost direction: exposes a small fixed-trip inner
//! loop to unroll/vectorization and tiled-memory rewrites while keeping
//! code size bounded.

use super::substitution::{body_writes_loop_var, substitute_nodes};
use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;

/// Generic strip-mining tile. Eight lanes stays below the existing
/// unroll budget while still exposing a regular inner loop.
pub const DEFAULT_STRIP_MINE_TILE: u32 = 8;

/// Convert large literal loops into tiled outer + fixed inner loops.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_strip_mine",
    requires = ["const_fold"],
    invalidates = ["loop_unroll", "vectorization"],
    phase = "loop",
    boundary_class = "abi_preserving",
    cost_model_family = "loop"
)]
pub struct LoopStripMine;

impl LoopStripMine {
    /// Skip programs without a strip-mining-eligible loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // O(1) fast-path: no Loop in the cached stats bitset → no
        // strip-mining work possible. Avoids the per-call recursive
        // tree walk on programs that contain no loops at all.
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|node| node_map::any_descendant(node, &mut is_strip_mine_eligible))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Rewrite every eligible loop.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| {
            // Every name bound or referenced anywhere in the program. Synthesized
            // tile/lane loop variables must avoid ALL of them -- not just the
            // names in the loop body -- so a generated `<var>_tile` cannot shadow
            // an (otherwise unused) enclosing binding of the same name (V008).
            let scope_names = names_in_nodes(&entry);
            entry
                .into_iter()
                .map(|node| rewrite_node(node, &scope_names, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

fn rewrite_node(node: Node, scope_names: &[Ident], changed: &mut bool) -> Node {
    let recursed =
        node_map::map_children(node, &mut |child| rewrite_node(child, scope_names, changed));
    let recursed = node_map::map_body(recursed, &mut |body| {
        body.into_iter()
            .map(|child| rewrite_node(child, scope_names, changed))
            .collect()
    });
    strip_mine_if_eligible(recursed, scope_names, changed)
}

fn strip_mine_if_eligible(node: Node, scope_names: &[Ident], changed: &mut bool) -> Node {
    let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    else {
        return node;
    };
    let Some((from_lit, to_lit)) = literal_bounds(&from, &to) else {
        return Node::Loop {
            var,
            from,
            to,
            body,
        };
    };
    let Some(trip_count) = to_lit.checked_sub(from_lit) else {
        return Node::Loop {
            var,
            from,
            to,
            body,
        };
    };
    if trip_count < DEFAULT_STRIP_MINE_TILE.saturating_mul(2) || body_writes_loop_var(&body, &var) {
        return Node::Loop {
            var,
            from,
            to,
            body,
        };
    }

    // Avoid every name in scope (enclosing bindings + this body), not just the
    // body's names, so the synthesized tile/lane vars cannot shadow an outer
    // binding (V008). `scope_names` already includes this loop's body names.
    let outer_var = fresh_ident(&var, "tile", scope_names);
    let lane_var = fresh_ident(&var, "lane", scope_names);
    let tile_count = trip_count.div_ceil(DEFAULT_STRIP_MINE_TILE);
    let tile_offset = Expr::add(
        Expr::mul(
            Expr::var(outer_var.as_str()),
            Expr::u32(DEFAULT_STRIP_MINE_TILE),
        ),
        Expr::var(lane_var.as_str()),
    );
    let original_index = Expr::add(Expr::u32(from_lit), tile_offset.clone());
    let tiled_body = substitute_nodes(&body, &var, &original_index);
    let guarded_body = vec![Node::if_then(
        Expr::lt(tile_offset, Expr::u32(trip_count)),
        tiled_body,
    )];

    *changed = true;
    Node::loop_for(
        outer_var,
        Expr::u32(0),
        Expr::u32(tile_count),
        vec![Node::loop_for(
            lane_var,
            Expr::u32(0),
            Expr::u32(DEFAULT_STRIP_MINE_TILE),
            guarded_body,
        )],
    )
}

fn literal_bounds(from: &Expr, to: &Expr) -> Option<(u32, u32)> {
    let from = literal_u32(from)?;
    let to = literal_u32(to)?;
    Some((from, to))
}

fn literal_u32(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::LitU32(value) => Some(*value),
        Expr::LitI32(value) => u32::try_from(*value).ok(),
        _ => None,
    }
}

fn is_strip_mine_eligible(node: &Node) -> bool {
    let Node::Loop {
        var,
        from,
        to,
        body,
    } = node
    else {
        return false;
    };
    let Some((from, to)) = literal_bounds(from, to) else {
        return false;
    };
    matches!(to.checked_sub(from), Some(n) if n >= DEFAULT_STRIP_MINE_TILE * 2)
        && !body_writes_loop_var(body, var)
}

fn fresh_ident(base: &Ident, suffix: &str, used: &[Ident]) -> Ident {
    for ordinal in 0.. {
        let candidate = if ordinal == 0 {
            format!("{}_{}", base.as_str(), suffix)
        } else {
            format!("{}_{}_{}", base.as_str(), suffix, ordinal)
        };
        if used.iter().all(|name| name.as_str() != candidate) {
            return Ident::from(candidate.as_str());
        }
    }
    unreachable!("unbounded ordinal search must return before overflow")
}

fn names_in_nodes(nodes: &[Node]) -> Vec<Ident> {
    // Lower bound: every Let/Assign at this level pushes one name;
    // pre-size to the sibling count so the typical small body avoids
    // grow-by-doubling. collect_names recurses and may push more.
    let mut out = Vec::with_capacity(nodes.len());
    collect_names(nodes, &mut out);
    out
}

fn collect_names(nodes: &[Node], out: &mut Vec<Ident>) {
    for node in nodes {
        match node {
            Node::Let { name, value } | Node::Assign { name, value } => {
                out.push(name.clone());
                collect_names_in_expr(value, out);
            }
            Node::Store { index, value, .. } => {
                collect_names_in_expr(index, out);
                collect_names_in_expr(value, out);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_names_in_expr(cond, out);
                collect_names(then, out);
                collect_names(otherwise, out);
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                out.push(var.clone());
                collect_names_in_expr(from, out);
                collect_names_in_expr(to, out);
                collect_names(body, out);
            }
            Node::Block(body) => collect_names(body, out),
            Node::Region { body, .. } => collect_names(body, out),
            Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                collect_names_in_expr(offset, out);
                collect_names_in_expr(size, out);
            }
            Node::Trap { address, .. } => collect_names_in_expr(address, out),
            Node::IndirectDispatch { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::AsyncWait { .. }
            | Node::Resume { .. }
            | Node::Return
            | Node::Barrier { .. }
            | Node::Opaque(_) => {}
        }
    }
}

fn collect_names_in_expr(expr: &Expr, out: &mut Vec<Ident>) {
    match expr {
        Expr::Var(name) => out.push(name.clone()),
        Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => {
            collect_names_in_expr(index, out);
        }
        Expr::BinOp { left, right, .. } => {
            collect_names_in_expr(left, out);
            collect_names_in_expr(right, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_names_in_expr(arg, out);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_names_in_expr(cond, out);
            collect_names_in_expr(true_val, out);
            collect_names_in_expr(false_val, out);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            collect_names_in_expr(value, out);
        }
        Expr::Fma { a, b, c } => {
            collect_names_in_expr(a, out);
            collect_names_in_expr(b, out);
            collect_names_in_expr(c, out);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            collect_names_in_expr(index, out);
            if let Some(expected) = expected {
                collect_names_in_expr(expected, out);
            }
            collect_names_in_expr(value, out);
        }
        Expr::SubgroupBallot { cond } => collect_names_in_expr(cond, out),
        Expr::SubgroupShuffle { value, lane } => {
            collect_names_in_expr(value, out);
            collect_names_in_expr(lane, out);
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
    }
}

// `body_writes_loop_var` lives in `super::substitution` (one canonical copy
// shared by every loop pass that reasons about induction-variable stability).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType};

    fn buf() -> BufferDecl {
        BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(64)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    #[test]
    fn strip_mines_large_literal_loop() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(32),
            vec![Node::store(
                "out",
                Expr::var("i"),
                Expr::add(Expr::var("i"), Expr::u32(1)),
            )],
        )]));
        assert!(result.changed);
        let entry = crate::test_util::region_body(&result.program);
        let Node::Loop {
            var: outer,
            from,
            to,
            body,
        } = &entry[0]
        else {
            panic!("expected outer loop");
        };
        assert_eq!(outer.as_str(), "i_tile");
        assert_eq!(from, &Expr::u32(0));
        assert_eq!(to, &Expr::u32(4));
        let Node::Loop {
            var: lane,
            from: lane_from,
            to: lane_to,
            body: lane_body,
        } = &body[0]
        else {
            panic!("expected inner lane loop");
        };
        assert_eq!(lane.as_str(), "i_lane");
        assert_eq!(lane_from, &Expr::u32(0));
        assert_eq!(lane_to, &Expr::u32(DEFAULT_STRIP_MINE_TILE));
        assert!(matches!(&lane_body[0], Node::If { .. }));
    }

    #[test]
    fn tail_guard_uses_trip_count_to_prevent_wrapped_tail_lanes() {
        let from = u32::MAX - 17;
        let to = u32::MAX;
        let trip_count = to - from;
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(from),
            Expr::u32(to),
            vec![Node::store("out", Expr::var("i"), Expr::var("i"))],
        )]));
        assert!(result.changed);
        let entry = crate::test_util::region_body(&result.program);
        let Node::Loop { body, .. } = &entry[0] else {
            panic!("expected outer loop");
        };
        let Node::Loop { body: inner, .. } = &body[0] else {
            panic!("expected inner loop");
        };
        let Node::If { cond, .. } = &inner[0] else {
            panic!("expected tail guard");
        };
        let expected_cond = Expr::lt(
            Expr::add(
                Expr::mul(Expr::var("i_tile"), Expr::u32(DEFAULT_STRIP_MINE_TILE)),
                Expr::var("i_lane"),
            ),
            Expr::u32(trip_count),
        );
        assert_eq!(cond, &expected_cond);
    }

    #[test]
    fn preserves_non_zero_lower_bound_in_index_expression() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(16),
            Expr::u32(48),
            vec![Node::store("out", Expr::var("i"), Expr::var("i"))],
        )]));
        assert!(result.changed);
        let entry = crate::test_util::region_body(&result.program);
        let Node::Loop { body, .. } = &entry[0] else {
            panic!("expected outer loop");
        };
        let Node::Loop { body: inner, .. } = &body[0] else {
            panic!("expected inner loop");
        };
        let Node::If { then, .. } = &inner[0] else {
            panic!("expected tail guard");
        };
        let Node::Store { index, value, .. } = &then[0] else {
            panic!("expected store in guarded body");
        };
        let expected = Expr::add(
            Expr::u32(16),
            Expr::add(
                Expr::mul(Expr::var("i_tile"), Expr::u32(DEFAULT_STRIP_MINE_TILE)),
                Expr::var("i_lane"),
            ),
        );
        assert_eq!(index, &expected);
        assert_eq!(value, &expected);
    }

    #[test]
    fn skips_small_loops_that_unroll_directly() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(DEFAULT_STRIP_MINE_TILE),
            vec![Node::store("out", Expr::var("i"), Expr::u32(1))],
        )]));
        assert!(!result.changed);
    }

    #[test]
    fn skips_runtime_bounds() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::buf_len("out"),
            vec![Node::store("out", Expr::var("i"), Expr::u32(1))],
        )]));
        assert!(!result.changed);
    }

    #[test]
    fn skips_body_that_rebinds_loop_var() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(32),
            vec![Node::let_bind("i", Expr::u32(7))],
        )]));
        assert!(!result.changed);
    }

    #[test]
    fn freshens_generated_names_when_body_already_uses_defaults() {
        let result = LoopStripMine::transform(program(vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(32),
            vec![
                Node::let_bind("i_tile", Expr::u32(0)),
                Node::let_bind("i_lane", Expr::u32(0)),
                Node::store("out", Expr::var("i"), Expr::u32(1)),
            ],
        )]));
        assert!(result.changed);
        let entry = crate::test_util::region_body(&result.program);
        let Node::Loop { var, body, .. } = &entry[0] else {
            panic!("expected outer loop");
        };
        assert_eq!(var.as_str(), "i_tile_1");
        let Node::Loop { var: lane, .. } = &body[0] else {
            panic!("expected inner loop");
        };
        assert_eq!(lane.as_str(), "i_lane_1");
    }

    #[test]
    fn analyze_skips_without_large_loop_and_runs_with_large_loop() {
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(
                &LoopStripMine,
                &program(vec![Node::store("out", Expr::u32(0), Expr::u32(1))])
            ),
            PassAnalysis::SKIP
        );
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(
                &LoopStripMine,
                &program(vec![Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(32),
                    vec![Node::store("out", Expr::var("i"), Expr::u32(1))],
                )])
            ),
            PassAnalysis::RUN
        );
    }
}
