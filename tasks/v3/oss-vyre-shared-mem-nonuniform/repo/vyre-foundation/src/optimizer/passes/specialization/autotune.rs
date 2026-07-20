use crate::execution_plan::SchedulingPolicy;
use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::program_shape_facts::ProgramShapeFacts;
use crate::optimizer::program_soa::ProgramFacts;
use crate::optimizer::AdapterCaps;
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use rustc_hash::FxHashSet;

/// Dynamically adjust dispatch dimensions and workgroup bounds.
#[derive(Debug, Default)]
#[vyre_pass(name = "autotune", requires = [], invalidates = [])]
pub struct Autotune;

impl Autotune {
    /// O(1) gates: autotune only adjusts 1-D workgroup kernels with at least
    /// one buffer (it derives shape facts from buffers and may emit a
    /// `buf_len` bounds guard). Multi-dimensional workgroups have intentional
    /// spatial structure (e.g. 2-D matmul tile) and skip the tuner anyway  -
    /// pre-gating here avoids deriving `ProgramShapeFacts` for those programs.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        let wg = program.workgroup_size();
        if wg[1] != 1 || wg[2] != 1 {
            return PassAnalysis::SKIP;
        }
        if program.buffers().is_empty() {
            return PassAnalysis::SKIP;
        }
        PassAnalysis::RUN
    }

    /// Autotune invocation scales without introducing partial-wave OOB accesses.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        Self::transform_for_adapter(program, &AdapterCaps::conservative())
    }

    /// Autotune against concrete device capabilities.
    #[must_use]
    pub fn transform_for_adapter(program: Program, caps: &AdapterCaps) -> PassResult {
        let current = program.workgroup_size();
        let shape_facts = ProgramShapeFacts::derive_cached(&program);
        let tuned = tuned_workgroup_size_for(
            current,
            infer_problem_size_from_facts(&program, &shape_facts),
            caps,
        );
        let size_changed = tuned != current;

        if !size_changed {
            // Missing bounds-guard is not a compiler-wide crash condition  -
            // it is exactly what this pass would inject if it were running
            // a tuning step. Return unchanged so a later pass / the backend
            // validator surfaces the issue with an actionable diagnostic.
            // VYRE_IR_HOTSPOTS CRIT: cloning + comparing the whole Program
            // just to prove changed=false is O(N) pure overhead; use the
            // fast-path PassResult::unchanged.
            let _divisibility = check_even_divisible_without_guard(&program, current);
            return PassResult::unchanged(program);
        }

        let Some(bound_buffer) = inferred_guard_bound_buffer(&program) else {
            return PassResult::unchanged(program);
        };
        let bound = Expr::buf_len(bound_buffer.name());

        let has_guard = program_has_gid_x_bounds_check(&program);
        let shape_proves_even_divisible = shape_facts
            .get(&Ident::from(bound_buffer.name()))
            .is_some_and(|facts| facts.vectorizable_at(tuned[0]));
        let scaffold = program.with_rewritten_workgroup_size_and_entry(tuned, Vec::new());
        let entry_body = program.into_entry_vec();
        let entry = if has_guard || shape_proves_even_divisible {
            entry_body
        } else {
            vec![Node::if_then(Expr::lt(Expr::gid_x(), bound), entry_body)]
        };

        let optimized = scaffold.with_rewritten_entry(entry);
        PassResult {
            program: optimized,
            changed: true,
        }
    }
}

fn tuned_workgroup_size_for(
    current: [u32; 3],
    problem_size: Option<u32>,
    caps: &AdapterCaps,
) -> [u32; 3] {
    // Only tune 1D kernels  -  multi-dimensional workgroups have intentional
    // spatial structure (e.g. 2D tile for matmul) that we must not disturb.
    if current[1] != 1 || current[2] != 1 {
        return current;
    }

    [
        SchedulingPolicy::standard().select_workgroup_x(current[0], problem_size, caps),
        1,
        1,
    ]
}

fn program_has_gid_x_bounds_check(program: &Program) -> bool {
    program.entry().iter().any(node_has_gid_x_bounds_check)
}

fn inferred_guard_bound_buffer(program: &Program) -> Option<&crate::ir::BufferDecl> {
    referenced_storage_buffers(program)
        .into_iter()
        // Skip zero-count buffers (uniforms, unbound temporaries) that carry
        // no meaningful problem-size information.
        .filter(|buffer| buffer.count() > 0)
        .max_by_key(|buffer| {
            (
                // Prefer output / pipeline-live-out buffers as the bounds
                // source because they define the result domain  -  input
                // buffers may be oversized padding or reused across calls.
                u8::from(buffer.is_output() || buffer.is_pipeline_live_out()),
                buffer.count(),
            )
        })
}

fn infer_problem_size(program: &Program) -> Option<u32> {
    referenced_storage_buffers(program)
        .into_iter()
        .map(crate::ir_inner::model::program::BufferDecl::count)
        // Zero-count buffers are uniforms or temporaries with no
        // meaningful element count  -  exclude them from problem-size
        // inference.
        .filter(|count| *count > 0)
        .min()
}

fn infer_problem_size_from_facts(program: &Program, facts: &ProgramShapeFacts) -> Option<u32> {
    referenced_storage_buffers(program)
        .into_iter()
        .filter_map(|buffer| {
            facts
                .get(&Ident::from(buffer.name()))
                .and_then(|fact| fact.max_count)
                .filter(|count| *count > 0)
                .or_else(|| (buffer.count() > 0).then_some(buffer.count()))
        })
        .min()
}

fn referenced_storage_buffers(program: &Program) -> Vec<&crate::ir::BufferDecl> {
    let facts = ProgramFacts::build_cached(program);
    let mut names = FxHashSet::<Ident>::default();
    for (_, name, _) in facts.buffer_refs() {
        names.insert(name.clone());
    }
    names
        .into_iter()
        .filter_map(|name| program.buffer(name.as_str()))
        .collect()
}

/// Returns `Ok(())` when the program has a bounds check OR the
/// workgroup size evenly divides the inferred problem size.
/// Returns `Err(msg)` when neither holds  -  the caller then decides
/// whether to emit a diagnostic, fall through without tuning, or
/// inject the missing guard.
///
/// Historical note: this used to be
/// `assert_even_divisible_without_guard` with an `assert_eq!` that
/// panicked on legal user IR (`VYRE_OPTIMIZER` audit CRIT-01:
/// optimizer crashing on valid input). Panicking the whole compiler
/// for a condition the very same pass is supposed to *fix* is the
/// exact wrong move. The caller now gets an actionable Result.
fn check_even_divisible_without_guard(
    program: &Program,
    workgroup_size: [u32; 3],
) -> Result<(), String> {
    if program_has_gid_x_bounds_check(program) {
        return Ok(());
    }
    if let Some(problem_size) = infer_problem_size(program) {
        if problem_size % workgroup_size[0] != 0 {
            return Err(format!(
                "Fix: inject a bounds check when workgroup_size.x={} does not evenly divide inferred problem size {}.",
                workgroup_size[0], problem_size,
            ));
        }
    }
    Ok(())
}

fn node_has_gid_x_bounds_check(node: &Node) -> bool {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            is_gid_x_bounds_cond(cond)
                || then.iter().any(node_has_gid_x_bounds_check)
                || otherwise.iter().any(node_has_gid_x_bounds_check)
        }
        Node::Loop { body, .. } | Node::Block(body) => body.iter().any(node_has_gid_x_bounds_check),
        Node::Region { body, .. } => body.iter().any(node_has_gid_x_bounds_check),
        Node::Let { .. }
        | Node::Assign { .. }
        | Node::Store { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncLoad { .. }
        | Node::AsyncStore { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::AsyncWait { .. }
        | Node::Trap { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => false,
    }
}

fn is_gid_x_bounds_cond(cond: &Expr) -> bool {
    matches!(
        cond,
        Expr::BinOp { left, right, .. }
            if matches!(left.as_ref(), Expr::InvocationId { axis: 0 })
                && matches!(right.as_ref(), Expr::BufLen { .. } | Expr::LitU32(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, ShapePredicate};

    #[test]
    fn analyze_skips_program_with_no_buffers() {
        let program = Program::wrapped(Vec::new(), [64, 1, 1], vec![Node::Return]);
        match crate::optimizer::ProgramPass::analyze(&Autotune, &program) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP for buffer-less program, got {other:?}"),
        }
    }

    #[test]
    fn analyze_skips_multidimensional_workgroup() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(64)],
            [8, 8, 1],
            vec![Node::Return],
        );
        match crate::optimizer::ProgramPass::analyze(&Autotune, &program) {
            PassAnalysis::SKIP => {}
            other => panic!("expected SKIP for multi-dim workgroup, got {other:?}"),
        }
    }

    #[test]
    fn injects_gid_x_bounds_check_when_clamping_oversized_workgroup() {
        // 512 exceeds the conservative portable cap (256) and is clamped.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1000)],
            [512, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(
            optimized.workgroup_size(),
            [
                SchedulingPolicy::standard()
                    .legal_workgroup_x_ceiling(&AdapterCaps::conservative()),
                1,
                1
            ]
        );
        assert!(program_has_gid_x_bounds_check(&optimized));
    }

    #[test]
    fn preserves_valid_power_of_two_workgroup() {
        // 256 is valid: power of two, within range.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(256)],
            [256, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(optimized.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn rounds_non_power_of_two_down() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1000)],
            [100, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(optimized.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn promotes_trivial_workgroup_to_default() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1000)],
            [1, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(optimized.workgroup_size(), [256, 1, 1]);
    }

    #[test]
    fn shape_predicate_multiple_of_avoids_redundant_guard() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32)
                .with_count(1024)
                .with_shape_predicate(ShapePredicate::MultipleOf(256))],
            [1, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(optimized.workgroup_size(), [256, 1, 1]);
        assert!(
            !program_has_gid_x_bounds_check(&optimized),
            "Fix: shape facts proving divisibility must prevent redundant guard injection"
        );
    }

    #[test]
    fn preserves_multidimensional_workgroup() {
        // 2D workgroup  -  never tuned.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1000)],
            [8, 8, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform(program).program;
        assert_eq!(optimized.workgroup_size(), [8, 8, 1]);
    }

    #[test]
    fn referenced_buffers_come_from_program_facts_async_edges() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("src", 0, DataType::U32).with_count(1024),
                BufferDecl::read_write("dst", 1, DataType::U32).with_count(1024),
            ],
            [1, 1, 1],
            vec![Node::AsyncLoad {
                source: Ident::from("src"),
                destination: Ident::from("dst"),
                offset: Box::new(Expr::u32(0)),
                size: Box::new(Expr::u32(128)),
                tag: Ident::from("copy"),
            }],
        );

        let mut names: Vec<&str> = referenced_storage_buffers(&program)
            .into_iter()
            .map(|buffer| buffer.name())
            .collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["dst", "src"],
            "autotune must consume ProgramFacts buffer_refs, including async source/destination edges"
        );
    }

    #[test]
    fn adapter_caps_allow_wider_occupancy_shape() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [1, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform_for_adapter(program, &AdapterCaps::high_end()).program;
        assert_eq!(optimized.workgroup_size(), [256, 1, 1]);
        assert!(!program_has_gid_x_bounds_check(&optimized));
    }

    #[test]
    fn adapter_caps_clamp_to_small_device_limit() {
        let caps = AdapterCaps {
            max_workgroup_size: [128, 1, 1],
            max_invocations_per_workgroup: 128,
            subgroup_size: 32,
            ..AdapterCaps::conservative()
        };
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [512, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );

        let optimized = Autotune::transform_for_adapter(program, &caps).program;
        assert_eq!(optimized.workgroup_size(), [128, 1, 1]);
    }

    #[test]
    fn device_signature_tile_bias_changes_transformed_workgroup() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [1, 1, 1],
            vec![Node::store("out", Expr::gid_x(), Expr::u32(1))],
        );
        let compact = AdapterCaps {
            max_workgroup_size: [256, 256, 64],
            max_invocations_per_workgroup: 256,
            subgroup_size: 32,
            ideal_workgroup_tile: [8, 8, 1],
            ..AdapterCaps::conservative()
        };
        let wide = AdapterCaps {
            ideal_workgroup_tile: [16, 16, 1],
            ..compact
        };

        let compact_program =
            Autotune::transform_for_adapter(Clone::clone(&program), &compact).program;
        let wide_program = Autotune::transform_for_adapter(program, &wide).program;

        assert_eq!(compact_program.workgroup_size(), [64, 1, 1]);
        assert_eq!(wide_program.workgroup_size(), [256, 1, 1]);
    }
}
