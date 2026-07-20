use crate::api::case::BenchError;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct QueueClosureLaneProfile {
    pub(crate) fixed_delta_source_slots: u64,
    pub(crate) profiled_delta_source_slots: u64,
    pub(crate) elided_delta_source_slots: u64,
    pub(crate) fixed_delta_lanes: u64,
    pub(crate) profiled_delta_lanes: u64,
    pub(crate) elided_delta_lanes: u64,
    pub(crate) delta_lane_elision_x1000: u64,
    pub(crate) launched_delta_lanes: u64,
    pub(crate) launch_elided_delta_lanes: u64,
    pub(crate) launch_lane_elision_x1000: u64,
}

impl QueueClosureLaneProfile {
    #[must_use]
    pub(crate) fn from_wave_lengths(
        queue_capacity: u32,
        wave_lengths: &[u32],
        lanes_per_source: u32,
    ) -> Self {
        let launch_lanes_per_wave =
            u64::from(queue_capacity).saturating_mul(u64::from(lanes_per_source.max(1)));
        Self::from_wave_lengths_with_launch_lanes(
            queue_capacity,
            wave_lengths,
            lanes_per_source,
            launch_lanes_per_wave,
        )
    }

    #[must_use]
    pub(crate) fn from_wave_lengths_with_launch_lanes(
        queue_capacity: u32,
        wave_lengths: &[u32],
        lanes_per_source: u32,
        launch_lanes_per_wave: u64,
    ) -> Self {
        let lanes_per_source = u128::from(lanes_per_source.max(1));
        let fixed_delta_source_slots =
            u128::from(queue_capacity).saturating_mul(wave_lengths.len() as u128);
        let profiled_delta_source_slots = wave_lengths
            .iter()
            .fold(0_u128, |total, &len| total.saturating_add(u128::from(len)));
        let elided_delta_source_slots =
            fixed_delta_source_slots.saturating_sub(profiled_delta_source_slots);
        let fixed_delta_lanes = fixed_delta_source_slots.saturating_mul(lanes_per_source);
        let profiled_delta_lanes = profiled_delta_source_slots.saturating_mul(lanes_per_source);
        let elided_delta_lanes = fixed_delta_lanes.saturating_sub(profiled_delta_lanes);
        let launched_delta_lanes =
            u128::from(launch_lanes_per_wave).saturating_mul(wave_lengths.len() as u128);
        let launch_elided_delta_lanes = fixed_delta_lanes.saturating_sub(launched_delta_lanes);
        let delta_lane_elision_x1000 = if fixed_delta_lanes == 0 {
            0
        } else {
            elided_delta_lanes.saturating_mul(1000) / fixed_delta_lanes
        };
        let launch_lane_elision_x1000 = if fixed_delta_lanes == 0 {
            0
        } else {
            launch_elided_delta_lanes.saturating_mul(1000) / fixed_delta_lanes
        };

        Self {
            fixed_delta_source_slots: u128_to_u64_saturating(fixed_delta_source_slots),
            profiled_delta_source_slots: u128_to_u64_saturating(profiled_delta_source_slots),
            elided_delta_source_slots: u128_to_u64_saturating(elided_delta_source_slots),
            fixed_delta_lanes: u128_to_u64_saturating(fixed_delta_lanes),
            profiled_delta_lanes: u128_to_u64_saturating(profiled_delta_lanes),
            elided_delta_lanes: u128_to_u64_saturating(elided_delta_lanes),
            delta_lane_elision_x1000: u128_to_u64_saturating(delta_lane_elision_x1000),
            launched_delta_lanes: u128_to_u64_saturating(launched_delta_lanes),
            launch_elided_delta_lanes: u128_to_u64_saturating(launch_elided_delta_lanes),
            launch_lane_elision_x1000: u128_to_u64_saturating(launch_lane_elision_x1000),
        }
    }
}

#[must_use]
pub(crate) fn queue_closure_launch_lanes_per_wave(
    dispatch_grid: [u32; 3],
    workgroup_size: [u32; 3],
) -> u64 {
    dispatch_grid
        .iter()
        .chain(workgroup_size.iter())
        .fold(1_u64, |lanes, &extent| {
            lanes.saturating_mul(u64::from(extent.max(1)))
        })
}

#[must_use]
#[cfg(test)]
pub(crate) fn queue_closure_delta_grid(
    active_queue_len: u32,
    lanes_per_source: u32,
    workgroup_size_x: u32,
) -> [u32; 3] {
    let total_lanes =
        u128::from(active_queue_len.max(1)).saturating_mul(u128::from(lanes_per_source.max(1)));
    let blocks = total_lanes.div_ceil(u128::from(workgroup_size_x.max(1)));
    [u128_to_u32_saturating(blocks).max(1), 1, 1]
}

pub(crate) fn validate_queue_closure_wave_profile(
    context: &str,
    wave_lengths: &[u32],
    iterations: u32,
    total_queue_pops: u64,
    max_wave_queue_len: u32,
    queue_capacity: u32,
) -> Result<(), BenchError> {
    let iteration_count = usize::try_from(iterations).map_err(|error| {
        BenchError::EnvironmentInvalid(format!(
            "{context} queue closure iteration count {iterations} does not fit usize: {error}. Fix: split closure wave profiling."
        ))
    })?;
    if wave_lengths.len() != iteration_count {
        return Err(BenchError::EnvironmentInvalid(format!(
            "{context} queue closure wave profile recorded {} wave length(s) for {iterations} iteration(s). Fix: rebuild the closure oracle from the same queue loop.",
            wave_lengths.len()
        )));
    }

    let mut computed_total = 0_u64;
    let mut computed_max = 0_u32;
    for (wave, &len) in wave_lengths.iter().enumerate() {
        if len == 0 {
            return Err(BenchError::EnvironmentInvalid(format!(
                "{context} queue closure wave {wave} recorded zero active rows inside a live iteration. Fix: only profile non-empty queue waves."
            )));
        }
        if len > queue_capacity {
            return Err(BenchError::EnvironmentInvalid(format!(
                "{context} queue closure wave {wave} length {len} exceeds queue_capacity={queue_capacity}. Fix: size queues from max_wave_queue_len before profiling grids."
            )));
        }
        computed_total = computed_total.saturating_add(u64::from(len));
        computed_max = computed_max.max(len);
    }

    if computed_total != total_queue_pops {
        return Err(BenchError::EnvironmentInvalid(format!(
            "{context} queue closure wave profile sums to {computed_total} queue pop(s), but oracle recorded {total_queue_pops}. Fix: derive total_queue_pops from the profiled wave lengths."
        )));
    }
    if computed_max != max_wave_queue_len {
        return Err(BenchError::EnvironmentInvalid(format!(
            "{context} queue closure wave profile max is {computed_max}, but oracle recorded {max_wave_queue_len}. Fix: derive max_wave_queue_len from the profiled wave lengths."
        )));
    }

    Ok(())
}

const fn u128_to_u64_saturating(value: u128) -> u64 {
    if value > u64::MAX as u128 {
        u64::MAX
    } else {
        value as u64
    }
}

const fn u128_to_u32_saturating(value: u128) -> u32 {
    if value > u32::MAX as u128 {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_queue_closure_lane_profiles_account_for_every_elided_lane() {
        const CASES: u32 = 20_000;
        let mut eliding_cases = 0_u32;
        let mut exact_cases = 0_u32;

        for case in 0..CASES {
            let capacity = (mix32(case ^ 0x51A7_E001) % 65_536).max(1);
            let wave_count = mix32(case ^ 0x51A7_E002) % 129;
            let lanes_per_source = if case & 1 == 0 { 1 } else { 32 };
            let mut waves = Vec::with_capacity(wave_count as usize);
            for wave in 0..wave_count {
                let raw = mix32(case ^ wave.wrapping_mul(0x9E37_79B9));
                let len = 1 + (raw % capacity);
                waves.push(len);
            }

            let profile =
                QueueClosureLaneProfile::from_wave_lengths(capacity, &waves, lanes_per_source);
            let capped_launch_sources = capacity.min(16_384);
            let capped_launch_lanes =
                u64::from(capped_launch_sources.max(1)).saturating_mul(u64::from(lanes_per_source));
            let capped_profile = QueueClosureLaneProfile::from_wave_lengths_with_launch_lanes(
                capacity,
                &waves,
                lanes_per_source,
                capped_launch_lanes,
            );
            let profiled_sources = waves.iter().map(|&len| u64::from(len)).sum::<u64>();
            let fixed_sources = u64::from(capacity).saturating_mul(u64::from(wave_count));
            let elided_sources = fixed_sources.saturating_sub(profiled_sources);
            let fixed_lanes = fixed_sources.saturating_mul(u64::from(lanes_per_source));
            let launched_lanes = capped_launch_lanes.saturating_mul(u64::from(wave_count));

            assert_eq!(
                profile.fixed_delta_source_slots, fixed_sources,
                "fixed source slots case {case}"
            );
            assert_eq!(
                profile.profiled_delta_source_slots, profiled_sources,
                "profiled source slots case {case}"
            );
            assert_eq!(
                profile.elided_delta_source_slots, elided_sources,
                "elided source slots case {case}"
            );
            assert_eq!(
                profile.fixed_delta_lanes, fixed_lanes,
                "fixed lanes case {case}"
            );
            assert_eq!(
                profile.profiled_delta_lanes,
                profiled_sources.saturating_mul(u64::from(lanes_per_source)),
                "profiled lanes case {case}"
            );
            assert_eq!(
                profile.elided_delta_lanes,
                elided_sources.saturating_mul(u64::from(lanes_per_source)),
                "elided lanes case {case}"
            );
            assert_eq!(
                capped_profile.launched_delta_lanes, launched_lanes,
                "launch lanes case {case}"
            );
            assert_eq!(
                capped_profile.launch_elided_delta_lanes,
                fixed_lanes.saturating_sub(launched_lanes),
                "launch-elided lanes case {case}"
            );
            if fixed_sources == 0 {
                assert_eq!(profile.delta_lane_elision_x1000, 0, "empty case {case}");
                exact_cases += 1;
            } else if elided_sources == 0 {
                exact_cases += 1;
            } else {
                assert!(
                    profile.delta_lane_elision_x1000 > 0,
                    "elision scale case {case}"
                );
                eliding_cases += 1;
            }

            for &wave_len in &waves {
                let grid = queue_closure_delta_grid(wave_len, lanes_per_source, 256);
                let launched_lanes = u64::from(grid[0]) * 256;
                assert!(
                    launched_lanes >= u64::from(wave_len) * u64::from(lanes_per_source),
                    "grid underlaunch case {case}"
                );
                assert!(
                    launched_lanes < u64::from(wave_len) * u64::from(lanes_per_source) + 256,
                    "grid overlaunch case {case}"
                );
            }
        }

        assert!(eliding_cases > CASES * 9 / 10);
        assert!(exact_cases > 0);
    }

    #[test]
    fn generated_queue_closure_launch_lane_product_is_saturating() {
        const CASES: u32 = 10_000;
        let mut saturated_cases = 0_u32;

        for case in 0..CASES {
            let grid = [
                mix32(case ^ 0xA171_0001),
                mix32(case ^ 0xA171_0002),
                1 + (mix32(case ^ 0xA171_0003) % 9),
            ];
            let workgroup = [1 + (mix32(case ^ 0xA171_0004) % 1024), 1, 1];
            let lanes = queue_closure_launch_lanes_per_wave(grid, workgroup);
            let expected = grid
                .iter()
                .chain(workgroup.iter())
                .fold(1_u128, |total, &extent| {
                    total.saturating_mul(u128::from(extent.max(1)))
                });
            assert_eq!(
                lanes,
                u128_to_u64_saturating(expected),
                "launch lane product case {case}"
            );
            saturated_cases += u32::from(expected > u128::from(u64::MAX));
        }

        assert!(saturated_cases > 0);
    }

    #[test]
    fn generated_queue_closure_wave_profile_validation_rejects_bad_shapes() {
        const CASES: u32 = 10_000;
        let mut rejected_zero = 0_u32;
        let mut rejected_capacity = 0_u32;
        let mut rejected_sum = 0_u32;

        for case in 0..CASES {
            let capacity = 1 + (mix32(case ^ 0xB16B_0001) % 4096);
            let wave_count = 1 + (mix32(case ^ 0xB16B_0002) % 64);
            let waves = (0..wave_count)
                .map(|wave| 1 + (mix32(case ^ wave.wrapping_mul(17)) % capacity))
                .collect::<Vec<_>>();
            let total = waves.iter().map(|&len| u64::from(len)).sum::<u64>();
            let max_wave = waves.iter().copied().max().unwrap_or(0);

            validate_queue_closure_wave_profile(
                "generated valid",
                &waves,
                wave_count,
                total,
                max_wave,
                capacity,
            )
            .unwrap_or_else(|error| panic!("valid generated profile case {case} failed: {error}"));

            let mutation_index = (case as usize) % waves.len();
            let mut zero_wave = waves.clone();
            zero_wave[mutation_index] = 0;
            rejected_zero += u32::from(
                validate_queue_closure_wave_profile(
                    "generated zero",
                    &zero_wave,
                    wave_count,
                    total,
                    max_wave,
                    capacity,
                )
                .is_err(),
            );

            let mut over_capacity = waves.clone();
            over_capacity[mutation_index] = capacity.saturating_add(1);
            rejected_capacity += u32::from(
                validate_queue_closure_wave_profile(
                    "generated capacity",
                    &over_capacity,
                    wave_count,
                    total,
                    max_wave,
                    capacity,
                )
                .is_err(),
            );

            rejected_sum += u32::from(
                validate_queue_closure_wave_profile(
                    "generated sum",
                    &waves,
                    wave_count,
                    total.saturating_add(1),
                    max_wave,
                    capacity,
                )
                .is_err(),
            );
        }

        assert_eq!(rejected_zero, CASES);
        assert_eq!(rejected_capacity, CASES);
        assert_eq!(rejected_sum, CASES);
    }

    const fn mix32(mut value: u32) -> u32 {
        value ^= value >> 16;
        value = value.wrapping_mul(0x7FEB_352D);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846C_A68B);
        value ^ (value >> 16)
    }
}
