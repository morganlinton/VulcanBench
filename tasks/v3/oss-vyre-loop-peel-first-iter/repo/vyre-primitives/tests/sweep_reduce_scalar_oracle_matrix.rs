//! Handwritten oracle matrix for scalar reducers (any/all/sum/min/max).
//!
//! Compares production `cpu_ref` against independent fold/oracle reducers
//! across thousands of generated hostile u32 slices.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

type ScalarReduce = fn(&[u32]) -> u32;

#[test]
fn scalar_reducers_match_independent_oracle_matrix() {
    assert_scalar(
        "reduce_any",
        vyre_primitives::reduce::any::cpu_ref,
        |input| u32::from(input.iter().any(|value| *value != 0)),
    );
    assert_scalar(
        "reduce_all",
        vyre_primitives::reduce::all::cpu_ref,
        |input| u32::from(input.iter().all(|value| *value != 0)),
    );
    assert_scalar(
        "reduce_sum",
        vyre_primitives::reduce::sum::cpu_ref,
        |input| input.iter().copied().fold(0u32, u32::wrapping_add),
    );
    assert_scalar(
        "reduce_min",
        vyre_primitives::reduce::min::cpu_ref,
        |input| input.iter().copied().min().unwrap_or(u32::MAX),
    );
    assert_scalar(
        "reduce_max",
        vyre_primitives::reduce::max::cpu_ref,
        |input| input.iter().copied().max().unwrap_or(0),
    );
}

fn assert_scalar(name: &str, actual: ScalarReduce, expected: ScalarReduce) {
    for (case_idx, input) in scalar_inputs().enumerate() {
        assert_eq!(
            actual(&input),
            expected(&input),
            "Fix: {name} scalar oracle case {case_idx} len={} must match the independent oracle.",
            input.len()
        );
    }
}

fn scalar_inputs() -> impl Iterator<Item = Vec<u32>> {
    let fixed_lengths = [0usize, 1, 2, 31, 32, 33, 64, 65, 127, 128, 255, 256, 1024];
    let fixed = fixed_lengths.into_iter().flat_map(|len| {
        [
            vec![0u32; len],
            vec![u32::MAX; len],
            ramp(len, 0x1357_9BDF),
            alternating(len, 0x5555_5555, 0xAAAA_AAAA),
        ]
        .into_iter()
    });
    let generated = (0..16384usize).map(|case| {
        let len = match case % 20 {
            0 => 0,
            1 => 1,
            2 => 32,
            3 => 257,
            _ => 1 + (case % 130),
        };
        lcg(case as u32 ^ 0xAED0_CE00, len)
    });
    fixed.chain(generated)
}

fn ramp(len: usize, start: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| start.wrapping_add((idx as u32).wrapping_mul(0x9E37_79B9)))
        .collect()
}

fn alternating(len: usize, even: u32, odd: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| if idx % 2 == 0 { even } else { odd })
        .collect()
}

fn lcg(seed: u32, len: usize) -> Vec<u32> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx % 31) as u32);
            state ^ (idx as u32).wrapping_mul(0x85EB_CA6B)
        })
        .collect()
}
