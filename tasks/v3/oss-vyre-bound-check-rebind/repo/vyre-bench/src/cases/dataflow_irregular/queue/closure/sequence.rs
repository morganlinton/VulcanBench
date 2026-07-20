use std::time::Instant;

use crate::api::case::{BenchContext, BenchError};
use crate::api::resident::ResidentInputSet;
use vyre_driver::{ResidentDispatchStep, ResidentReadRange};

use super::{
    DataflowIfdsSkewedQueueClosurePrepared, QUEUE_CLOSURE_ACCUMULATOR_INDEX,
    QUEUE_CLOSURE_EDGE_KIND_INDEX, QUEUE_CLOSURE_EDGE_OFFSETS_INDEX,
    QUEUE_CLOSURE_EDGE_TARGETS_INDEX, QUEUE_CLOSURE_LEN_A_INDEX, QUEUE_CLOSURE_LEN_B_INDEX,
    QUEUE_CLOSURE_QUEUE_A_INDEX, QUEUE_CLOSURE_QUEUE_B_INDEX, QUEUE_CLOSURE_SEED_FRONTIER_INDEX,
    QUEUE_CLOSURE_SEED_LEN_INDEX, QUEUE_CLOSURE_SEED_QUEUE_INDEX, QUEUE_CLOSURE_WORKGROUP_SIZE,
};

const QUEUE_CLOSURE_RESET_ACCUMULATOR_RESOURCE: usize = 4;
const QUEUE_CLOSURE_RESET_RESOURCE_INDICES: [usize; 7] = [
    QUEUE_CLOSURE_SEED_FRONTIER_INDEX,
    QUEUE_CLOSURE_SEED_QUEUE_INDEX,
    QUEUE_CLOSURE_SEED_LEN_INDEX,
    QUEUE_CLOSURE_QUEUE_A_INDEX,
    QUEUE_CLOSURE_ACCUMULATOR_INDEX,
    QUEUE_CLOSURE_LEN_A_INDEX,
    QUEUE_CLOSURE_LEN_B_INDEX,
];
const QUEUE_CLOSURE_CLEAR_A_RESOURCE_INDICES: [usize; 1] = [QUEUE_CLOSURE_LEN_A_INDEX];
const QUEUE_CLOSURE_CLEAR_B_RESOURCE_INDICES: [usize; 1] = [QUEUE_CLOSURE_LEN_B_INDEX];
const QUEUE_CLOSURE_DELTA_A_TO_B_RESOURCE_INDICES: [usize; 8] = [
    QUEUE_CLOSURE_QUEUE_A_INDEX,
    QUEUE_CLOSURE_LEN_A_INDEX,
    QUEUE_CLOSURE_EDGE_OFFSETS_INDEX,
    QUEUE_CLOSURE_EDGE_TARGETS_INDEX,
    QUEUE_CLOSURE_EDGE_KIND_INDEX,
    QUEUE_CLOSURE_ACCUMULATOR_INDEX,
    QUEUE_CLOSURE_QUEUE_B_INDEX,
    QUEUE_CLOSURE_LEN_B_INDEX,
];
const QUEUE_CLOSURE_DELTA_B_TO_A_RESOURCE_INDICES: [usize; 8] = [
    QUEUE_CLOSURE_QUEUE_B_INDEX,
    QUEUE_CLOSURE_LEN_B_INDEX,
    QUEUE_CLOSURE_EDGE_OFFSETS_INDEX,
    QUEUE_CLOSURE_EDGE_TARGETS_INDEX,
    QUEUE_CLOSURE_EDGE_KIND_INDEX,
    QUEUE_CLOSURE_ACCUMULATOR_INDEX,
    QUEUE_CLOSURE_QUEUE_A_INDEX,
    QUEUE_CLOSURE_LEN_A_INDEX,
];

pub(super) struct QueueClosureSequenceRun {
    pub(super) outputs: Vec<Vec<u8>>,
    pub(super) wall_ns: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueueClosureRepeatedPlan {
    leading_a_to_b_half_wave: bool,
    repeated_pair_count: u32,
}

impl QueueClosureRepeatedPlan {
    const fn total_half_waves(self) -> u32 {
        self.repeated_pair_count
            .saturating_mul(2)
            .saturating_add(self.leading_a_to_b_half_wave as u32)
    }

    const fn dispatch_count(self) -> u32 {
        1_u32.saturating_add(self.total_half_waves().saturating_mul(2))
    }
}

const fn queue_closure_repeated_plan(closure_iterations: u32) -> QueueClosureRepeatedPlan {
    QueueClosureRepeatedPlan {
        leading_a_to_b_half_wave: closure_iterations & 1 == 1,
        repeated_pair_count: closure_iterations / 2,
    }
}

pub(super) fn dispatch_resident_queue_closure_sequence(
    ctx: &BenchContext,
    prepared: &DataflowIfdsSkewedQueueClosurePrepared,
    resident: &ResidentInputSet,
) -> Result<QueueClosureSequenceRun, BenchError> {
    let mut resource_sets = Vec::new();
    resource_sets.push(resident.resources_for_indices(
        &QUEUE_CLOSURE_RESET_RESOURCE_INDICES,
        "IFDS queue closure reset",
    )?);
    resource_sets.push(resident.resources_for_indices(
        &QUEUE_CLOSURE_CLEAR_A_RESOURCE_INDICES,
        "IFDS queue closure clear queue A length",
    )?);
    resource_sets.push(resident.resources_for_indices(
        &QUEUE_CLOSURE_CLEAR_B_RESOURCE_INDICES,
        "IFDS queue closure clear queue B length",
    )?);
    resource_sets.push(resident.resources_for_indices(
        &QUEUE_CLOSURE_DELTA_A_TO_B_RESOURCE_INDICES,
        "IFDS queue closure delta A to B",
    )?);
    resource_sets.push(resident.resources_for_indices(
        &QUEUE_CLOSURE_DELTA_B_TO_A_RESOURCE_INDICES,
        "IFDS queue closure delta B to A",
    )?);

    let reset_grid = [
        prepared
            .stats
            .frontier_words
            .max(prepared.seed_queue_len)
            .div_ceil(QUEUE_CLOSURE_WORKGROUP_SIZE[0])
            .max(1),
        1,
        1,
    ];
    let reset_step = ResidentDispatchStep {
        program: &prepared.reset_program,
        resources: &resource_sets[0],
        grid_override: Some(reset_grid),
        workgroup_override: None,
    };
    let read_ranges = [ResidentReadRange {
        resource: &resource_sets[0][QUEUE_CLOSURE_RESET_ACCUMULATOR_RESOURCE],
        byte_offset: 0,
        byte_len: prepared.baseline_output.len(),
    }];

    let mut accumulator_output = Vec::with_capacity(prepared.baseline_output.len());
    let started = Instant::now();
    let plan = queue_closure_repeated_plan(prepared.closure_iterations);
    debug_assert_eq!(
        plan.dispatch_count(),
        1 + prepared.closure_iterations.saturating_mul(2)
    );
    let clear_a_step = || ResidentDispatchStep {
        program: &prepared.clear_len_program,
        resources: &resource_sets[1],
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let clear_b_step = || ResidentDispatchStep {
        program: &prepared.clear_len_program,
        resources: &resource_sets[2],
        grid_override: Some([1, 1, 1]),
        workgroup_override: None,
    };
    let delta_a_to_b_step = || ResidentDispatchStep {
        program: &prepared.delta_program,
        resources: &resource_sets[3],
        grid_override: Some(prepared.delta_grid),
        workgroup_override: None,
    };
    let delta_b_to_a_step = || ResidentDispatchStep {
        program: &prepared.delta_program,
        resources: &resource_sets[4],
        grid_override: Some(prepared.delta_grid),
        workgroup_override: None,
    };

    if plan.leading_a_to_b_half_wave {
        let prefix_steps = [reset_step, clear_b_step(), delta_a_to_b_step()];
        let repeated_steps = [
            clear_a_step(),
            delta_b_to_a_step(),
            clear_b_step(),
            delta_a_to_b_step(),
        ];
        ctx.preferred_backend
            .dispatch_resident_repeated_sequence_read_ranges_into(
                &prefix_steps,
                &repeated_steps,
                plan.repeated_pair_count,
                &read_ranges,
                &mut [&mut accumulator_output],
            )
    } else {
        let prefix_steps = [reset_step];
        let repeated_steps = [
            clear_b_step(),
            delta_a_to_b_step(),
            clear_a_step(),
            delta_b_to_a_step(),
        ];
        ctx.preferred_backend
            .dispatch_resident_repeated_sequence_read_ranges_into(
                &prefix_steps,
                &repeated_steps,
                plan.repeated_pair_count,
                &read_ranges,
                &mut [&mut accumulator_output],
            )
    }
    .map_err(|error| BenchError::BackendFailed(error.to_string()))?;
    let wall_ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

    Ok(QueueClosureSequenceRun {
        outputs: vec![accumulator_output],
        wall_ns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_repeated_plan_preserves_every_queue_closure_wave() {
        const CASES: u32 = 10_000;
        let mut odd_cases = 0_u32;
        let mut repeated_pairs = 0_u64;

        for case in 0..CASES {
            let iterations = mix32(case ^ 0xC105_E7E5) % 16_385;
            let plan = queue_closure_repeated_plan(iterations);

            assert_eq!(plan.total_half_waves(), iterations, "case {case}");
            assert_eq!(
                plan.dispatch_count(),
                1 + iterations.saturating_mul(2),
                "dispatch count case {case}"
            );
            assert_eq!(
                plan.leading_a_to_b_half_wave,
                iterations & 1 == 1,
                "leading wave parity case {case}"
            );
            assert_eq!(
                plan.repeated_pair_count,
                iterations / 2,
                "pair count case {case}"
            );
            assert_repeated_plan_expands_to_alternating_half_waves(case, iterations, plan);

            odd_cases += u32::from(plan.leading_a_to_b_half_wave);
            repeated_pairs += u64::from(plan.repeated_pair_count);
        }

        assert!(odd_cases > CASES / 3);
        assert!(repeated_pairs > u64::from(CASES) * 1_000);
    }

    fn assert_repeated_plan_expands_to_alternating_half_waves(
        case: u32,
        iterations: u32,
        plan: QueueClosureRepeatedPlan,
    ) {
        let mut half_wave = 0_u32;
        if plan.leading_a_to_b_half_wave {
            assert_half_wave(case, half_wave, true);
            half_wave += 1;
        }

        for _ in 0..plan.repeated_pair_count {
            if plan.leading_a_to_b_half_wave {
                assert_half_wave(case, half_wave, false);
                half_wave += 1;
                assert_half_wave(case, half_wave, true);
            } else {
                assert_half_wave(case, half_wave, true);
                half_wave += 1;
                assert_half_wave(case, half_wave, false);
            }
            half_wave += 1;
        }

        assert_eq!(half_wave, iterations, "expanded wave count case {case}");
    }

    fn assert_half_wave(case: u32, half_wave: u32, a_to_b: bool) {
        assert_eq!(
            a_to_b,
            half_wave & 1 == 0,
            "half-wave direction case {case} wave {half_wave}"
        );
    }

    const fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7FEB_352D);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846C_A68B);
        value ^ (value >> 16)
    }
}
