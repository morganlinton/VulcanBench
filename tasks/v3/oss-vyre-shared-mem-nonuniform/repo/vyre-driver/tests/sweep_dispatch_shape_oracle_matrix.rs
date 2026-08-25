//! Handwritten oracle matrix for dispatch-shape helpers and binding-plan invariants.
//!
//! Compares `dispatch_shape` byte-shape predicates and `binding` ABI layout
//! contracts against independent reference oracles over hostile corpora.

#![forbid(unsafe_code)]

use vyre_driver::binding::{
    binding_plans_share_layout, BindingPlan, BindingRole, BindingSetFingerprint,
};
use vyre_driver::dispatch_shape::{
    borrowed_input_batch_shapes_match, borrowed_input_shapes_match,
    dispatch_configs_share_launch_shape,
};
use vyre_driver::fixpoint_iterations::resolve_fixpoint_iterations;
use vyre_driver::DispatchConfig;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Program};

const SHAPE_CASES: usize = 256;
const CONFIG_CASES: usize = 256;
const BINDING_CASES: usize = 256;

#[test]
fn borrowed_input_shape_oracle_matches_independent_length_contract() {
    let mut assertions = 0usize;
    for case in 0..SHAPE_CASES {
        let left = hostile_input_batch(case, 0);
        let right_match = hostile_input_batch(case ^ 0x5A, 0);
        let right_mismatch = hostile_input_batch(case, 1);

        let left_refs: Vec<&[u8]> = left.iter().map(|v| v.as_slice()).collect();
        let match_refs: Vec<&[u8]> = right_match.iter().map(|v| v.as_slice()).collect();
        let mismatch_refs: Vec<&[u8]> = right_mismatch.iter().map(|v| v.as_slice()).collect();

        assert_eq!(
            borrowed_input_shapes_match(&left_refs, &match_refs),
            oracle_borrowed_input_shapes_match(&left_refs, &match_refs),
            "Fix: borrowed_input_shapes_match case {case} must follow the independent length oracle."
        );
        assertions += 1;

        assert_eq!(
            borrowed_input_shapes_match(&left_refs, &mismatch_refs),
            oracle_borrowed_input_shapes_match(&left_refs, &mismatch_refs),
            "Fix: borrowed_input_shapes_match mismatch case {case} must follow the independent length oracle."
        );
        assertions += 1;

        let batches_match: Vec<&[&[u8]]> = vec![&left_refs, &match_refs];
        let batches_mismatch: Vec<&[&[u8]]> = vec![&left_refs, &mismatch_refs];
        assert_eq!(
            borrowed_input_batch_shapes_match(&batches_match),
            oracle_borrowed_input_batch_shapes_match(&batches_match),
            "Fix: borrowed_input_batch_shapes_match uniform case {case} must follow the independent oracle."
        );
        assertions += 1;
        assert_eq!(
            borrowed_input_batch_shapes_match(&batches_mismatch),
            oracle_borrowed_input_batch_shapes_match(&batches_mismatch),
            "Fix: borrowed_input_batch_shapes_match divergent case {case} must follow the independent oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, SHAPE_CASES * 4);
}

#[test]
fn dispatch_config_launch_shape_oracle_matches_independent_policy() {
    let mut assertions = 0usize;
    for case in 0..CONFIG_CASES {
        let compiled = hostile_dispatch_config(case as u32);
        let runtime_match = compiled.clone();
        let runtime_grid = {
            let mut cfg = compiled.clone();
            cfg.grid_override = Some([(case as u32 % 17) + 1, 1, 1]);
            cfg
        };
        let runtime_fixpoint = {
            let mut cfg = compiled.clone();
            cfg.fixpoint_iterations = Some(1 + (case as u32 % 4));
            cfg
        };

        assert_eq!(
            dispatch_configs_share_launch_shape(&compiled, &runtime_match),
            oracle_dispatch_configs_share_launch_shape(&compiled, &runtime_match),
            "Fix: dispatch_configs_share_launch_shape identical case {case} must follow the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            dispatch_configs_share_launch_shape(&compiled, &runtime_grid),
            oracle_dispatch_configs_share_launch_shape(&compiled, &runtime_grid),
            "Fix: dispatch_configs_share_launch_shape grid case {case} must follow the independent oracle."
        );
        assertions += 1;

        assert_eq!(
            dispatch_configs_share_launch_shape(&compiled, &runtime_fixpoint),
            oracle_dispatch_configs_share_launch_shape(&compiled, &runtime_fixpoint),
            "Fix: dispatch_configs_share_launch_shape fixpoint case {case} must follow the independent oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, CONFIG_CASES * 3);
}

#[test]
fn binding_plan_layout_oracle_matches_independent_fingerprint_contract() {
    let mut assertions = 0usize;
    for case in 0..BINDING_CASES {
        let program_a = hostile_binding_program(case as u32, false);
        let program_b = hostile_binding_program(case as u32, true);
        let plan_a = BindingPlan::build(&program_a)
            .unwrap_or_else(|error| panic!("Fix: binding plan case {case} must build: {error}"));
        let plan_b = BindingPlan::build(&program_b).unwrap_or_else(|error| {
            panic!("Fix: binding plan variant case {case} must build: {error}")
        });

        assert_eq!(
            oracle_binding_slots_sorted(&plan_a),
            true,
            "Fix: binding plan case {case} must keep bindings sorted by binding index."
        );
        assertions += 1;

        assert_eq!(
            plan_a.input_indices.len(),
            oracle_input_role_count(&plan_a),
            "Fix: binding plan case {case} input_indices must match input-role cardinality."
        );
        assertions += 1;

        assert_eq!(
            binding_plans_share_layout(&plan_a, &plan_b),
            oracle_binding_plans_share_layout(&plan_a, &plan_b),
            "Fix: binding_plans_share_layout case {case} must follow the independent fingerprint oracle."
        );
        assertions += 1;

        assert_eq!(
            BindingSetFingerprint::from_plan(&plan_a),
            oracle_binding_set_fingerprint(&plan_a),
            "Fix: BindingSetFingerprint case {case} must match the independent slot oracle."
        );
        assertions += 1;
    }
    assert_eq!(assertions, BINDING_CASES * 4);
}

fn oracle_borrowed_input_shapes_match(left: &[&[u8]], right: &[&[u8]]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(lhs, rhs)| lhs.len() == rhs.len())
}

fn oracle_borrowed_input_batch_shapes_match(batches: &[&[&[u8]]]) -> bool {
    let Some((first, rest)) = batches.split_first() else {
        return true;
    };
    rest.iter()
        .all(|batch| oracle_borrowed_input_shapes_match(first, batch))
}

fn oracle_dispatch_configs_share_launch_shape(
    compiled: &DispatchConfig,
    runtime: &DispatchConfig,
) -> bool {
    compiled.profile == runtime.profile
        && compiled.ulp_budget.unwrap_or(0) == runtime.ulp_budget.unwrap_or(0)
        && compiled.max_output_bytes == runtime.max_output_bytes
        && compiled.workgroup_override == runtime.workgroup_override
        && compiled.grid_override == runtime.grid_override
        && oracle_fixpoint_iterations_share_launch_shape(compiled, runtime)
        && compiled.speculation == runtime.speculation
        && compiled.persistent_thread == runtime.persistent_thread
        && compiled.cooperative == runtime.cooperative
}

fn oracle_fixpoint_iterations_share_launch_shape(
    compiled: &DispatchConfig,
    runtime: &DispatchConfig,
) -> bool {
    let Ok(compiled_iterations) = resolve_fixpoint_iterations(compiled, "oracle-dispatch-shape")
    else {
        return false;
    };
    let Ok(runtime_iterations) = resolve_fixpoint_iterations(runtime, "oracle-dispatch-shape")
    else {
        return false;
    };
    compiled_iterations == runtime_iterations
}

fn oracle_binding_slots_sorted(plan: &BindingPlan) -> bool {
    plan.bindings
        .windows(2)
        .all(|pair| pair[0].binding <= pair[1].binding)
}

fn oracle_input_role_count(plan: &BindingPlan) -> usize {
    plan.bindings
        .iter()
        .filter(|binding| {
            matches!(
                binding.role,
                BindingRole::Input | BindingRole::InputOutput | BindingRole::Uniform
            )
        })
        .count()
}

fn oracle_binding_set_fingerprint(plan: &BindingPlan) -> BindingSetFingerprint {
    let mut slots: Vec<(u32, BindingRole, usize)> = plan
        .bindings
        .iter()
        .map(|binding| (binding.binding, binding.role, binding.element_size))
        .collect();
    slots.sort_by_key(|(binding, _, _)| *binding);
    BindingSetFingerprint { slots }
}

fn oracle_binding_plans_share_layout(a: &BindingPlan, b: &BindingPlan) -> bool {
    oracle_binding_set_fingerprint(a) == oracle_binding_set_fingerprint(b)
}

fn hostile_input_batch(seed: usize, variant: usize) -> Vec<Vec<u8>> {
    let arity = 1 + (seed % 5);
    (0..arity)
        .map(|slot| {
            let len = ((seed.wrapping_mul(17 + slot).wrapping_add(variant)) % 128) as usize;
            lcg_bytes(seed as u32 ^ slot as u32 ^ variant as u32, len)
        })
        .collect()
}

fn hostile_dispatch_config(seed: u32) -> DispatchConfig {
    let mut cfg = DispatchConfig::default();
    if seed & 1 == 0 {
        cfg.profile = Some(format!("profile-{}", seed % 7));
    }
    if seed & 2 == 0 {
        cfg.ulp_budget = Some((seed % 4) as u8);
    }
    if seed & 4 == 0 {
        cfg.max_output_bytes = Some(((seed % 19) + 1) as usize * 64);
    }
    if seed & 8 == 0 {
        cfg.workgroup_override = Some([64 + (seed % 3) * 64, 1, 1]);
    }
    if seed & 16 == 0 {
        cfg.grid_override = Some([(seed % 11) + 1, 1, 1]);
    }
    if seed & 32 == 0 {
        cfg.fixpoint_iterations = Some(1 + (seed % 5));
    }
    if seed & 64 == 0 {
        cfg.cooperative = seed & 128 != 0;
    }
    cfg
}

fn hostile_binding_program(seed: u32, bump_counts: bool) -> Program {
    let count_a = 4 + (seed % 16);
    let count_b = if bump_counts { count_a + 8 } else { count_a };
    let include_uniform = seed & 1 == 0;
    let include_io = seed & 2 == 0;
    let mut buffers = vec![
        BufferDecl::storage("input_a", 0, BufferAccess::ReadOnly, DataType::U32)
            .with_count(count_a),
        BufferDecl::output("output", 1, DataType::U32).with_count(count_b),
    ];
    if include_uniform {
        buffers.push(
            BufferDecl::storage("params", 2, BufferAccess::Uniform, DataType::U32).with_count(2),
        );
    }
    if include_io {
        buffers.push(
            BufferDecl::storage("scratch", 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count_a),
        );
    }
    Program::wrapped(buffers, [64, 1, 1], Vec::new())
}

fn lcg_bytes(seed: u32, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx as u32) % 13);
            (state ^ (idx as u32).wrapping_mul(0x85EB_CA6B)) as u8
        })
        .collect()
}
