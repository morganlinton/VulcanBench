//! ROADMAP A31  -  software pipelining.
//!
//! 2-stage Load-then-Store narrow slice shipped here. Detect the
//! tight pattern:
//!
//! ```text
//! Loop(i, LitU32(lo), LitU32(hi), [
//!   Let(x, Load(buf_in, Var(i))),
//!   Store(buf_out, Var(i), expr_using_x),
//! ])
//!     where lo, hi are literals AND hi - lo >= 2
//!     AND `buf_in` != `buf_out` (distinct named buffers  -  A12 alias proof)
//!     AND `expr_using_x` reads `Var(x)` (so the Store depends on the Load)
//!     AND `expr_using_x` is observably free of side effects beyond `Var(x)`
//! ```
//!
//! Rewrite to overlap the load of iteration `i+1` with the
//! compute / store of iteration `i`:
//!
//! ```text
//! Let(__sp_x_pipe, Load(buf_in, LitU32(lo)));   // prologue: load 0th iteration
//! Loop(i, lo, hi - 1, [
//!   Let(__sp_x_next, Load(buf_in, Var(i) + LitU32(1))),  // prefetch next
//!   Store(buf_out, Var(i), expr_using_x[x := __sp_x_pipe]),  // process current
//!   Assign(__sp_x_pipe, Var(__sp_x_next)),  // shuffle
//! ]);
//! Store(buf_out, LitU32(hi - 1),
//!       expr_using_x[x := __sp_x_pipe]);  // epilogue: last iteration
//! ```
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_software_pipeline`.
//! Soundness: `Exact`. Each iteration `i ∈ [lo, hi)` produces the
//! same (load, compute, store) triple in both forms; the rewrite
//! shifts the load forward by one iteration without changing what
//! gets stored or in what order. The A12 alias proof on
//! `buf_in != buf_out` guarantees the prefetched load doesn't read
//! a value the previous iteration's store invalidated.
//!
//! Cost direction: monotone-down on per-iteration latency (the
//! load latency is hidden behind the previous iteration's compute);
//! `node_count` rises by ~3 (prologue Let + Assign + epilogue
//! Store) but the per-iteration body shrinks by zero  -  net
//! constant overhead amortised over `hi - lo` iterations.
//!
//! ## Conservatism
//!
//! - Both bounds must be `Expr::LitU32` literals; trip count must
//!   be `>= 2` (otherwise prologue/epilogue degenerate).
//! - Index expression in both Load and Store must be exactly
//!   `Var(loop_var)`. Affine indexing (e.g. `Var(i) * stride`) is
//!   the next refinement and lands beside this row.
//! - The Store value expression must read `Var(name)` where `name`
//!   is the Let-bound symbol from the Load. No other reads of
//!   `name` in the body, no reassignment, no nested capture.
//! - `buf_in` and `buf_out` must be provably-distinct
//!   (`ProgramFacts::buffers_provably_distinct` from A12). If they
//!   alias, the prefetch could read a value the previous iteration
//!   just overwrote.
//! - The Store value expression must be observably free apart from
//!   the `Var(name)` read  -  no Load (other than via name), no
//!   Atomic, no Call, no Opaque, no Subgroup.

use crate::ir::{BinOp, Expr, Ident, Node, Program};
use crate::optimizer::program_soa::ProgramFacts;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// 2-stage Load-then-Store software pipeline pass.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_software_pipeline",
    requires = ["const_fold"],
    invalidates = ["loop_unroll", "loop_strip_mine"],
    phase = "loop",
    boundary_class = "abi_preserving",
    cost_model_family = "loop"
)]
pub struct LoopSoftwarePipeline;

impl LoopSoftwarePipeline {
    /// Skip programs without a candidate loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Pipelining requires a Loop. Without one, the ProgramFacts
        // build (full SoA walk) and the recursive pipelinable check
        // would both come up empty.
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        let facts = ProgramFacts::build_cached(program);
        if program
            .entry()
            .iter()
            .any(|n| node_has_pipelinable_loop(n, &facts))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree; rewrite every pipelinable Loop into
    /// prologue + steady-state + epilogue.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let facts = ProgramFacts::build_cached(&program);
        let mut changed = false;
        let program = program.map_entry(|entry| {
            entry
                .into_iter()
                .flat_map(|n| rewrite_node(n, &facts, &mut changed))
                .collect()
        });
        PassResult { program, changed }
    }
}

fn rewrite_node(node: Node, facts: &ProgramFacts, changed: &mut bool) -> Vec<Node> {
    match node {
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            if let Some(plan) = analyse_pipelinable(&var, &from, &to, &body, facts) {
                *changed = true;
                return apply_pipeline(&plan);
            }
            vec![Node::Loop {
                var,
                from,
                to,
                body: body
                    .into_iter()
                    .flat_map(|n| rewrite_node(n, facts, changed))
                    .collect(),
            }]
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => vec![Node::If {
            cond,
            then: then
                .into_iter()
                .flat_map(|n| rewrite_node(n, facts, changed))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .flat_map(|n| rewrite_node(n, facts, changed))
                .collect(),
        }],
        Node::Block(body) => vec![Node::Block(
            body.into_iter()
                .flat_map(|n| rewrite_node(n, facts, changed))
                .collect(),
        )],
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            vec![Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(
                    body_vec
                        .into_iter()
                        .flat_map(|n| rewrite_node(n, facts, changed))
                        .collect(),
                ),
            }]
        }
        other => vec![other],
    }
}

struct PipelinePlan {
    loop_var: Ident,
    lo: u32,
    hi: u32,
    pipe_name: Ident,
    next_name: Ident,
    let_name: Ident,
    buf_in: Ident,
    buf_out: Ident,
    store_value_template: Expr,
}

fn analyse_pipelinable(
    var: &Ident,
    from: &Expr,
    to: &Expr,
    body: &[Node],
    facts: &ProgramFacts,
) -> Option<PipelinePlan> {
    let (lo, hi) = match (from, to) {
        (Expr::LitU32(lo), Expr::LitU32(hi)) if hi.checked_sub(*lo).is_some_and(|n| n >= 2) => {
            (*lo, *hi)
        }
        _ => return None,
    };
    if body.len() != 2 {
        return None;
    }
    let (let_name, buf_in) = match &body[0] {
        Node::Let {
            name,
            value: Expr::Load { buffer, index },
        } => match index.as_ref() {
            Expr::Var(idx_var) if idx_var == var => (name.clone(), buffer.clone()),
            _ => return None,
        },
        _ => return None,
    };
    let (buf_out, _store_index, store_value) = match &body[1] {
        Node::Store {
            buffer,
            index,
            value,
        } => match index {
            Expr::Var(idx_var) if idx_var == var => (buffer.clone(), index.clone(), value.clone()),
            _ => return None,
        },
        _ => return None,
    };
    if buf_in == buf_out {
        return None;
    }
    if !facts.buffers_provably_distinct(buf_in.as_str(), buf_out.as_str()) {
        return None;
    }
    if !expr_reads_only(&store_value, &let_name) {
        return None;
    }
    let pipe_name = Ident::from(format!("__sp_{}_pipe", let_name.as_str()));
    let next_name = Ident::from(format!("__sp_{}_next", let_name.as_str()));
    Some(PipelinePlan {
        loop_var: var.clone(),
        lo,
        hi,
        pipe_name,
        next_name,
        let_name,
        buf_in,
        buf_out,
        store_value_template: store_value,
    })
}

fn apply_pipeline(plan: &PipelinePlan) -> Vec<Node> {
    let prologue = Node::let_bind(
        plan.pipe_name.clone(),
        Expr::Load {
            buffer: plan.buf_in.clone(),
            index: Box::new(Expr::u32(plan.lo)),
        },
    );
    let prefetch = Node::let_bind(
        plan.next_name.clone(),
        Expr::Load {
            buffer: plan.buf_in.clone(),
            index: Box::new(Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(Expr::Var(plan.loop_var.clone())),
                right: Box::new(Expr::u32(1)),
            }),
        },
    );
    let pipe_value = substitute_var(
        plan.store_value_template.clone(),
        &plan.let_name,
        &plan.pipe_name,
    );
    let store_current = Node::Store {
        buffer: plan.buf_out.clone(),
        index: Expr::Var(plan.loop_var.clone()),
        value: pipe_value.clone(),
    };
    let shuffle = Node::Assign {
        name: plan.pipe_name.clone(),
        value: Expr::Var(plan.next_name.clone()),
    };
    let steady = Node::Loop {
        var: plan.loop_var.clone(),
        from: Expr::u32(plan.lo),
        to: Expr::u32(plan.hi - 1),
        body: vec![prefetch, store_current, shuffle],
    };
    let epilogue = Node::Store {
        buffer: plan.buf_out.clone(),
        index: Expr::u32(plan.hi - 1),
        value: pipe_value,
    };
    vec![prologue, steady, epilogue]
}

/// True iff `expr` reads `Var(name)` at least once AND has no
/// observable side effect beyond that read (no Load except via
/// `name`, no Atomic, no Call, no Opaque, no Subgroup).
fn expr_reads_only(expr: &Expr, name: &Ident) -> bool {
    let mut reads_name = false;
    let observable = expr_visit_check(expr, name, &mut reads_name);
    observable && reads_name
}

fn expr_visit_check(expr: &Expr, name: &Ident, reads_name: &mut bool) -> bool {
    match expr {
        Expr::Var(n) => {
            if n == name {
                *reads_name = true;
            }
            true
        }
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => true,
        Expr::BinOp { left, right, .. } => {
            expr_visit_check(left, name, reads_name) && expr_visit_check(right, name, reads_name)
        }
        Expr::UnOp { operand, .. } => expr_visit_check(operand, name, reads_name),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_visit_check(cond, name, reads_name)
                && expr_visit_check(true_val, name, reads_name)
                && expr_visit_check(false_val, name, reads_name)
        }
        Expr::Cast { value, .. } => expr_visit_check(value, name, reads_name),
        Expr::Fma { a, b, c } => {
            expr_visit_check(a, name, reads_name)
                && expr_visit_check(b, name, reads_name)
                && expr_visit_check(c, name, reads_name)
        }
        Expr::Load { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
    }
}

fn substitute_var(expr: Expr, from: &Ident, to: &Ident) -> Expr {
    match expr {
        Expr::Var(ref n) if n == from => Expr::Var(to.clone()),
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(substitute_var(*index, from, to)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op,
            left: Box::new(substitute_var(*left, from, to)),
            right: Box::new(substitute_var(*right, from, to)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(substitute_var(*operand, from, to)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|a| substitute_var(a, from, to))
                .collect(),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(substitute_var(*cond, from, to)),
            true_val: Box::new(substitute_var(*true_val, from, to)),
            false_val: Box::new(substitute_var(*false_val, from, to)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(substitute_var(*value, from, to)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(substitute_var(*a, from, to)),
            b: Box::new(substitute_var(*b, from, to)),
            c: Box::new(substitute_var(*c, from, to)),
        },
        other => other,
    }
}

fn node_has_pipelinable_loop(node: &Node, facts: &ProgramFacts) -> bool {
    match node {
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            analyse_pipelinable(var, from, to, body, facts).is_some()
                || body.iter().any(|n| node_has_pipelinable_loop(n, facts))
        }
        Node::If {
            then, otherwise, ..
        } => {
            then.iter().any(|n| node_has_pipelinable_loop(n, facts))
                || otherwise
                    .iter()
                    .any(|n| node_has_pipelinable_loop(n, facts))
        }
        Node::Block(body) => body.iter().any(|n| node_has_pipelinable_loop(n, facts)),
        Node::Region { body, .. } => body.iter().any(|n| node_has_pipelinable_loop(n, facts)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node};

    fn ro(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadOnly, DataType::U32).with_count(16)
    }

    fn rw(name: &str, binding: u32) -> BufferDecl {
        BufferDecl::storage(name, binding, BufferAccess::ReadWrite, DataType::U32).with_count(16)
    }

    fn program(buffers: Vec<BufferDecl>, entry: Vec<Node>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], entry)
    }

    fn pipelinable_loop(lo: u32, hi: u32) -> Vec<Node> {
        vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(lo),
            to: Expr::u32(hi),

            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store(
                    "rw",
                    Expr::var("i"),
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("x")),
                        right: Box::new(Expr::u32(1)),
                    },
                ),
            ],
        }]
    }

    fn count_loops_and_stores(nodes: &[Node]) -> (usize, usize) {
        let mut loops = 0;
        let mut stores = 0;
        for n in nodes {
            match n {
                Node::Loop { body, .. } => {
                    loops += 1;
                    let (l, s) = count_loops_and_stores(body);
                    loops += l;
                    stores += s;
                }
                Node::Store { .. } => stores += 1,
                Node::Block(body) => {
                    let (l, s) = count_loops_and_stores(body);
                    loops += l;
                    stores += s;
                }
                Node::Region { body, .. } => {
                    let (l, s) = count_loops_and_stores(body.as_ref());
                    loops += l;
                    stores += s;
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    let (l, s) = count_loops_and_stores(then);
                    loops += l;
                    stores += s;
                    let (l2, s2) = count_loops_and_stores(otherwise);
                    loops += l2;
                    stores += s2;
                }
                _ => {}
            }
        }
        (loops, stores)
    }

    /// Positive: pipelinable Load-then-Store loop becomes prologue +
    /// steady-state + epilogue.
    #[test]
    fn pipelines_simple_load_store_loop() {
        let entry = pipelinable_loop(0, 8);
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(result.changed, "Load-then-Store loop must pipeline");
        let (loops, stores) = count_loops_and_stores(result.program.entry());
        assert_eq!(loops, 1, "exactly one steady-state Loop after pipelining");
        // 1 store inside the loop body + 1 epilogue store
        assert_eq!(stores, 2);
    }

    /// Negative: trip count < 2  -  the prologue/epilogue degenerate.
    #[test]
    fn keeps_loop_with_trip_count_one() {
        let entry = pipelinable_loop(0, 1);
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// Negative: same buffer in Load and Store  -  alias proof fails.
    #[test]
    fn keeps_loop_when_buffers_alias() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("rw"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store(
                    "rw",
                    Expr::var("i"),
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("x")),
                        right: Box::new(Expr::u32(1)),
                    },
                ),
            ],
        }];
        let prog = program(vec![rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(
            !result.changed,
            "self-aliasing Load+Store must not pipeline"
        );
    }

    /// Negative: index expression isn't `Var(i)`  -  affine indexing
    /// is the next refinement.
    #[test]
    fn keeps_loop_with_non_var_index() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::BinOp {
                            op: BinOp::Add,
                            left: Box::new(Expr::var("i")),
                            right: Box::new(Expr::u32(1)),
                        }),
                    },
                ),
                Node::store("rw", Expr::var("i"), Expr::var("x")),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// Negative: body has a third statement  -  pattern doesn't match.
    #[test]
    fn keeps_loop_with_three_body_stmts() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::let_bind("y", Expr::u32(1)),
                Node::store("rw", Expr::var("i"), Expr::var("x")),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// Negative: store value doesn't read the Let-bound name  -
    /// no Load → Store dataflow to pipeline.
    #[test]
    fn keeps_loop_when_store_does_not_use_load() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store("rw", Expr::var("i"), Expr::u32(99)),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// Negative: store value contains another Load  -  observably-free
    /// gate fails.
    #[test]
    fn keeps_loop_when_store_value_has_other_load() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store(
                    "rw",
                    Expr::var("i"),
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("x")),
                        right: Box::new(Expr::Load {
                            buffer: Ident::from("ro"),
                            index: Box::new(Expr::u32(0)),
                        }),
                    },
                ),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// Negative: runtime bounds skip  -  needs literal bounds for
    /// prologue / epilogue construction.
    #[test]
    fn keeps_loop_with_runtime_bounds() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::var("n"),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store("rw", Expr::var("i"), Expr::var("x")),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }

    /// `analyze` short-circuits when no candidate exists.
    #[test]
    fn analyze_skips_program_without_pipelinable_loop() {
        let entry = vec![Node::store("rw", Expr::u32(0), Expr::u32(1))];
        let prog = program(vec![rw("rw", 1)], entry);
        match crate::optimizer::ProgramPass::analyze(&LoopSoftwarePipeline, &prog) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP, got {other:?}"),
        }
    }

    /// Loop bounds with `lo + 2 > u32::MAX` previously overflowed in the
    /// pipelinable-loop guard. The replacement uses `checked_sub` so the
    /// pass cleanly declines instead of panicking on overflow under
    /// debug-assertions.
    #[test]
    fn keeps_loop_when_pipelinability_check_would_overflow() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(u32::MAX),
            to: Expr::u32(u32::MAX),
            body: vec![
                Node::let_bind(
                    "x",
                    Expr::Load {
                        buffer: Ident::from("ro"),
                        index: Box::new(Expr::var("i")),
                    },
                ),
                Node::store(
                    "rw",
                    Expr::var("i"),
                    Expr::BinOp {
                        op: BinOp::Add,
                        left: Box::new(Expr::var("x")),
                        right: Box::new(Expr::u32(1)),
                    },
                ),
            ],
        }];
        let prog = program(vec![ro("ro"), rw("rw", 1)], entry);
        let result = LoopSoftwarePipeline::transform(prog);
        assert!(!result.changed);
    }
}
