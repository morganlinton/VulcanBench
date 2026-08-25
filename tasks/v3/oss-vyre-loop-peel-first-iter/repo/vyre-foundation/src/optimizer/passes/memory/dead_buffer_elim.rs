use crate::ir::{Ident, Node, Program};
use crate::optimizer::fact_substrate::{FactSubstrate, UseFacts};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use rustc_hash::FxHashSet;
use std::sync::Arc;

/// Remove buffers whose contents cannot contribute to observable output.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "dead_buffer_elim",
    requires = ["fusion"],
    invalidates = ["buffer_layout"],
    phase = "memory",
    boundary_class = "abi_preserving",
    cost_model_family = "memory"
)]
pub struct DeadBufferElim;

impl DeadBufferElim {
    /// Decide whether this pass should run.
    #[must_use]
    #[inline]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if live_buffers(program).len() == program.buffers().len() {
            PassAnalysis::SKIP
        } else {
            PassAnalysis::RUN
        }
    }

    /// Remove dead buffer declarations and stores to dead buffers.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let live = live_buffers(&program);
        if live.len() == program.buffers().len() {
            return PassResult::unchanged(program);
        }
        // `live.len()` is the exact post-filter buffer count; pre-size
        // so collect doesn't grow-by-doubling on programs with many
        // buffers (the launch shapes carry 60+).
        let mut buffers: Vec<_> = Vec::with_capacity(live.len());
        buffers.extend(
            program
                .buffers()
                .iter()
                .filter(|buffer| live.contains(buffer.name.as_ref()))
                .cloned(),
        );
        let entry = filter_nodes(program.entry(), &live);

        let optimized = Program::wrapped(buffers, program.workgroup_size(), entry)
            .with_optional_entry_op_id(program.entry_op_id().map(ToOwned::to_owned))
            .with_non_composable_with_self(program.is_non_composable_with_self());
        PassResult {
            program: optimized,
            changed: true,
        }
    }
}

type LiveBufferSet<'a> = FxHashSet<&'a str>;

fn live_buffers(program: &Program) -> LiveBufferSet<'_> {
    let live = cached_live_buffer_idents(program);
    program
        .buffers()
        .iter()
        .filter_map(|buffer| {
            live.contains(buffer.name.as_ref())
                .then_some(buffer.name.as_ref())
        })
        .collect()
}

fn cached_live_buffer_idents(program: &Program) -> FxHashSet<Ident> {
    let substrate = FactSubstrate::derive_use_only_cached(program);
    let use_facts = substrate.use_facts().unwrap_or_else(|| {
        unreachable!("derive_use_only_cached contract: use_facts is always populated")
    });
    compute_live_buffer_idents(program, use_facts)
}

fn compute_live_buffer_idents(program: &Program, use_facts: &UseFacts) -> FxHashSet<Ident> {
    if use_facts.has_opaque {
        return program
            .buffers()
            .iter()
            .map(|buffer| Ident::new(Arc::clone(&buffer.name)))
            .collect();
    }

    let mut live = program
        .buffers()
        .iter()
        .filter(|buffer| buffer.is_output() || buffer.is_pipeline_live_out())
        .map(|buffer| Ident::new(Arc::clone(&buffer.name)))
        .collect::<FxHashSet<_>>();
    let mut worklist = Vec::with_capacity(live.len() + use_facts.indirect_dispatch_buffers.len());
    worklist.extend(live.iter().cloned());

    for buffer in &use_facts.indirect_dispatch_buffers {
        let buffer = buffer.clone();
        if live.insert(buffer.clone()) {
            worklist.push(buffer);
        }
    }

    while let Some(buffer) = worklist.pop() {
        let Some(deps) = use_facts.buffer_write_deps.get(&buffer) else {
            continue;
        };
        for dep in deps {
            let dep = dep.clone();
            if live.insert(dep.clone()) {
                worklist.push(dep);
            }
        }
    }

    live
}

fn filter_nodes(nodes: &[Node], live: &LiveBufferSet<'_>) -> Vec<Node> {
    let mut out = Vec::with_capacity(nodes.len());
    out.extend(nodes.iter().filter_map(|node| filter_node(node, live)));
    out
}

fn filter_node(node: &Node, live: &LiveBufferSet<'_>) -> Option<Node> {
    match node {
        Node::Store { buffer, .. } if !live.contains(buffer.as_str()) => None,
        Node::AsyncStore { destination, .. } if !live.contains(destination.as_str()) => None,
        Node::AsyncLoad { destination, .. } if !live.contains(destination.as_str()) => None,
        Node::Region {
            generator,
            source_region,
            body,
        } => Some(Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(filter_nodes(body, live)),
        }),
        Node::If {
            cond,
            then,
            otherwise,
        } => Some(Node::if_then_else(
            cond.clone(),
            filter_nodes(then, live),
            filter_nodes(otherwise, live),
        )),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Some(Node::loop_for(
            var,
            from.clone(),
            to.clone(),
            filter_nodes(body, live),
        )),
        Node::Block(nodes) => Some(Node::block(filter_nodes(nodes, live))),
        other => Some(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr};

    #[test]
    fn unread_buffer_removed() {
        let optimized = run(sample_program(false));
        assert!(optimized.buffer("scratch").is_none());
    }

    #[test]
    fn output_buffer_preserved() {
        let optimized = run(sample_program(false));
        assert!(optimized.buffer("out").is_some());
    }

    fn run(program: Program) -> Program {
        DeadBufferElim::transform(program).program
    }

    fn sample_program(read_scratch: bool) -> Program {
        Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(1),
                BufferDecl::read_write("scratch", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            if read_scratch {
                vec![
                    Node::store("scratch", Expr::u32(0), Expr::u32(999)),
                    Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
                ]
            } else {
                vec![
                    Node::store("scratch", Expr::u32(0), Expr::u32(999)),
                    Node::store("out", Expr::u32(0), Expr::u32(7)),
                ]
            },
        )
    }

    #[test]
    fn read_used_buffer_preserved() {
        // scratch IS read by output → must not be eliminated.
        let optimized = run(sample_program(true));
        assert!(
            optimized.buffer("scratch").is_some(),
            "scratch is read by out, must stay"
        );
        assert!(optimized.buffer("out").is_some());
    }

    #[test]
    fn let_mediated_buffer_read_preserves_source_buffer() {
        let program = Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(1),
                BufferDecl::read_write("scratch", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![
                Node::store("scratch", Expr::u32(0), Expr::u32(99)),
                Node::let_bind("x", Expr::load("scratch", Expr::u32(0))),
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ],
        );

        let optimized = run(program);
        assert!(
            optimized.buffer("scratch").is_some(),
            "scratch feeds the output through scalar binding `x`; removing it leaves a dangling load"
        );
    }

    #[test]
    fn pipeline_live_out_buffer_preserved() {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("pipeline_buf", 0, DataType::F32)
                .with_count(4)
                .with_pipeline_live_out(true)],
            [1, 1, 1],
            vec![], // no stores at all, but pipeline_live_out keeps it alive
        );
        let optimized = run(program);
        assert!(
            optimized.buffer("pipeline_buf").is_some(),
            "pipeline_live_out buffers must never be eliminated"
        );
    }

    #[test]
    fn transitive_liveness_through_chain() {
        // a → scratch → out: scratch feeds into out, a feeds into scratch.
        let program = Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(1),
                BufferDecl::read_write("scratch", 1, DataType::U32).with_count(1),
                BufferDecl::read_write("a", 2, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![
                Node::store("a", Expr::u32(0), Expr::u32(42)),
                Node::store("scratch", Expr::u32(0), Expr::load("a", Expr::u32(0))),
                Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
            ],
        );
        let optimized = run(program);
        assert!(
            optimized.buffer("a").is_some(),
            "a is transitively live via scratch→out"
        );
        assert!(optimized.buffer("scratch").is_some());
        assert!(optimized.buffer("out").is_some());
    }

    #[test]
    fn scalar_mediated_transitive_liveness_uses_shared_facts() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(1),
                BufferDecl::read_write("scratch", 1, DataType::U32).with_count(1),
                BufferDecl::read_write("dead", 2, DataType::U32).with_count(1),
                BufferDecl::output("out", 3, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::load("input", Expr::u32(0))),
                Node::store("scratch", Expr::u32(0), Expr::var("x")),
                Node::store("dead", Expr::u32(0), Expr::u32(99)),
                Node::store("out", Expr::u32(0), Expr::load("scratch", Expr::u32(0))),
            ],
        );

        let optimized = run(program);
        assert!(optimized.buffer("input").is_some());
        assert!(optimized.buffer("scratch").is_some());
        assert!(optimized.buffer("out").is_some());
        assert!(optimized.buffer("dead").is_none());
    }

    #[test]
    fn indirect_dispatch_count_buffer_is_live() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("counts", 0, DataType::U32).with_count(1),
                BufferDecl::read_write("dead", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![
                Node::store("dead", Expr::u32(0), Expr::u32(99)),
                Node::indirect_dispatch("counts", 0),
            ],
        );

        let optimized = run(program);
        assert!(optimized.buffer("counts").is_some());
        assert!(optimized.buffer("dead").is_none());
    }

    #[test]
    fn analyze_skips_when_all_buffers_live() {
        // Every buffer is either output or read by output → SKIP.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        );
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&DeadBufferElim, &program),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_dead_buffers_present() {
        let program = sample_program(false); // scratch is dead
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&DeadBufferElim, &program),
            PassAnalysis::RUN
        );
    }
}
