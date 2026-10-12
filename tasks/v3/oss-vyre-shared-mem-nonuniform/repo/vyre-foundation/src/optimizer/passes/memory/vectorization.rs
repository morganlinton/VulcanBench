use crate::ir::{CacheLocality, MemoryHints, Program};
use crate::optimizer::fact_substrate::{FactSubstrate, UseFacts};
use crate::optimizer::program_shape_facts::ProgramShapeFacts;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};

/// Promote proven-safe vector/coalescing layout hints from buffer shape facts.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "vectorization",
    requires = [],
    invalidates = ["buffer_layout"],
    phase = "memory",
    boundary_class = "abi_preserving",
    cost_model_family = "memory"
)]
pub struct Vectorization;

impl Vectorization {
    /// Decide whether this pass should run.
    #[must_use]
    #[inline]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if program.buffers().is_empty() {
            return PassAnalysis::SKIP;
        }
        // Vectorization rewrites apply where the program actually
        // touches buffer memory. memory_op_count is a cached counter
        // of every Load / Store / async copy in the program; zero
        // means there is no buffer access to vectorize.
        if program.stats().memory_op_count == 0 {
            return PassAnalysis::SKIP;
        }
        PassAnalysis::RUN
    }

    /// Rewrite buffer hints when shape facts prove tail-free vector lanes.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let substrate = FactSubstrate::derive_shape_and_use_cached(&program);
        let shapes = substrate.shape.as_deref().unwrap_or_else(|| {
            unreachable!("derive_shape_and_use_cached contract: shape always populated")
        });
        let use_facts = substrate.use_facts.as_deref().unwrap_or_else(|| {
            unreachable!("derive_shape_and_use_cached contract: use_facts always populated")
        });
        let rewritten_buffers = {
            let buffers = program.buffers();
            let mut rewritten_buffers = None::<Vec<_>>;
            for (index, buffer) in buffers.iter().enumerate() {
                let rewritten = vectorized_buffer(buffer, shapes, use_facts);
                match (rewritten_buffers.as_mut(), rewritten) {
                    (None, None) => {}
                    (Some(out), None) => out.push(buffer.clone()),
                    (None, Some(rewritten)) => {
                        let mut out = Vec::with_capacity(buffers.len());
                        out.extend_from_slice(&buffers[..index]);
                        out.push(rewritten);
                        rewritten_buffers = Some(out);
                    }
                    (Some(out), Some(rewritten)) => out.push(rewritten),
                }
            }
            rewritten_buffers
        };

        if let Some(buffers) = rewritten_buffers {
            PassResult {
                program: program.with_rewritten_buffers(buffers),
                changed: true,
            }
        } else {
            PassResult::unchanged(program)
        }
    }
}

fn vectorized_buffer(
    buffer: &crate::ir::BufferDecl,
    shapes: &ProgramShapeFacts,
    use_facts: &UseFacts,
) -> Option<crate::ir::BufferDecl> {
    let name = fact_name(buffer.name());
    let fact = shapes.get(&name)?;
    let plan = vector_plan(fact, buffer.hints(), use_facts)?;

    let mut hints = buffer.hints();
    if hints.coalesce_axis.is_none() {
        hints.coalesce_axis = Some(plan.coalesce_axis);
    }
    if hints.preferred_alignment < plan.alignment_bytes {
        hints.preferred_alignment = plan.alignment_bytes;
    }
    if hints == buffer.hints() {
        return None;
    }
    let mut rewritten = buffer.clone();
    rewritten.hints = hints;
    Some(rewritten)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VectorPlan {
    coalesce_axis: u8,
    alignment_bytes: u32,
}

fn vector_plan(
    facts: &crate::optimizer::program_shape_facts::BufferShapeFacts,
    hints: MemoryHints,
    use_facts: &UseFacts,
) -> Option<VectorPlan> {
    if hints.cache_locality == CacheLocality::Random
        && use_facts.dominant_index_axis(&facts.name).is_none()
    {
        return None;
    }
    let element_size = u32::try_from(facts.element_size_bytes?).ok()?.max(1);
    let max_lanes = 16u32.checked_div(element_size)?.max(1);
    let coalesce_axis = hints
        .coalesce_axis
        .or_else(|| use_facts.dominant_index_axis(&facts.name))
        .unwrap_or(0);
    for lanes in [16, 8, 4, 2] {
        if lanes <= max_lanes && facts.vectorizable_at(lanes) {
            let alignment_bytes = lanes.checked_mul(element_size)?;
            if facts
                .max_bytes
                .is_some_and(|bytes| bytes < u64::from(alignment_bytes))
            {
                continue;
            }
            return Some(VectorPlan {
                coalesce_axis,
                alignment_bytes,
            });
        }
    }
    None
}

fn fact_name(name: &str) -> crate::ir::Ident {
    crate::ir::Ident::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, CacheLocality, DataType, Expr, MemoryHints, Node, ShapePredicate};

    #[test]
    fn vectorization_sets_coalesce_axis_and_alignment_from_fixed_count() {
        let program = Program::wrapped(
            vec![BufferDecl::read("input", 0, DataType::U32).with_count(64)],
            [64, 1, 1],
            vec![Node::return_()],
        );

        let optimized = Vectorization::transform(program).program;
        let hints = optimized.buffer("input").unwrap().hints();
        assert_eq!(hints.coalesce_axis, Some(0));
        assert_eq!(hints.preferred_alignment, 16);
    }

    #[test]
    fn vectorization_preserves_author_coalesce_axis() {
        let hints = MemoryHints {
            coalesce_axis: Some(1),
            preferred_alignment: 4,
            cache_locality: CacheLocality::Streaming,
        };
        let program = Program::wrapped(
            vec![BufferDecl::read("input", 0, DataType::U32)
                .with_count(64)
                .with_hints(hints)],
            [64, 1, 1],
            vec![Node::return_()],
        );

        let optimized = Vectorization::transform(program).program;
        let hints = optimized.buffer("input").unwrap().hints();
        assert_eq!(hints.coalesce_axis, Some(1));
        assert_eq!(hints.preferred_alignment, 16);
        assert_eq!(hints.cache_locality, CacheLocality::Streaming);
    }

    #[test]
    fn vectorization_uses_shape_predicate_for_runtime_sized_buffer() {
        let program = Program::wrapped(
            vec![BufferDecl::read("bytes", 0, DataType::Bytes)
                .with_shape_predicate(ShapePredicate::MultipleOf(16))],
            [64, 1, 1],
            vec![Node::return_()],
        );

        let optimized = Vectorization::transform(program).program;
        let hints = optimized.buffer("bytes").unwrap().hints();
        assert_eq!(hints.coalesce_axis, Some(0));
        assert_eq!(hints.preferred_alignment, 16);
    }

    #[test]
    fn vectorization_prefers_observed_y_axis_indexing() {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("input", 0, DataType::U32).with_count(64)],
            [8, 8, 1],
            vec![Node::store(
                "input",
                Expr::add(Expr::gid_y(), Expr::u32(1)),
                Expr::u32(7),
            )],
        );

        let optimized = Vectorization::transform(program).program;
        let hints = optimized.buffer("input").unwrap().hints();
        assert_eq!(hints.coalesce_axis, Some(1));
        assert_eq!(hints.preferred_alignment, 16);
    }

    #[test]
    fn vectorization_avoids_random_buffers_without_proven_axis() {
        let hints = MemoryHints {
            coalesce_axis: None,
            preferred_alignment: 0,
            cache_locality: CacheLocality::Random,
        };
        let program = Program::wrapped(
            vec![BufferDecl::read("input", 0, DataType::U32)
                .with_count(64)
                .with_hints(hints)],
            [64, 1, 1],
            vec![Node::return_()],
        );

        let result = Vectorization::transform(program);
        assert!(!result.changed);
    }

    #[test]
    fn vectorization_leaves_unproven_shape_unchanged() {
        let program = Program::wrapped(
            vec![BufferDecl::read("input", 0, DataType::U32)
                .with_shape_predicate(ShapePredicate::AtLeast(64))],
            [64, 1, 1],
            vec![Node::return_()],
        );

        let result = Vectorization::transform(program);
        assert!(!result.changed);
        let hints = result.program.buffer("input").unwrap().hints();
        assert_eq!(hints.coalesce_axis, None);
        assert_eq!(hints.preferred_alignment, 0);
    }

    /// `analyze_skips_program_with_no_buffers` exercises the SKIP arm.
    /// A buffer-less program has no candidates for the coalescing /
    /// alignment hint promotion, so the pass must not run. The H10d
    /// audit (`tests/analyze_skip_audit.rs`) requires every pass with
    /// a `PassAnalysis::SKIP` branch to ship at least one test that
    /// hits it.
    #[test]
    fn analyze_skips_program_with_no_buffers() {
        let program = Program::wrapped(Vec::new(), [1, 1, 1], vec![Node::return_()]);
        match crate::optimizer::ProgramPass::analyze(&Vectorization, &program) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP for zero-buffer program, got {other:?}"),
        }
    }
}
