//! Generated CPU-reference matrix for public u32 hardware intrinsic builders.
//!
//! These intrinsics are Cat-C because backends lower them to dedicated hardware
//! instructions or barriers. The CPU reference path is still the conformance
//! oracle, so the public builders must stay byte-exact over edge-heavy and
//! generated lanes, including dispatch extents larger than one workgroup.

use vyre_foundation::ir::Program;
use vyre_reference::value::Value;

struct U32Case {
    name: &'static str,
    build: fn(&str, &str, u32) -> Program,
    expected: fn(u32) -> u32,
}

const CASES: &[U32Case] = &[
    U32Case {
        name: "bit_reverse_u32",
        build: vyre_intrinsics::hardware::bit_reverse_u32::bit_reverse_u32::bit_reverse_u32,
        expected: u32::reverse_bits,
    },
    U32Case {
        name: "popcount_u32",
        build: vyre_intrinsics::hardware::popcount_u32::popcount_u32::popcount_u32,
        expected: u32::count_ones,
    },
    U32Case {
        name: "storage_barrier",
        build: vyre_intrinsics::hardware::storage_barrier::storage_barrier::storage_barrier,
        expected: |value| value,
    },
    U32Case {
        name: "workgroup_barrier",
        build: vyre_intrinsics::hardware::workgroup_barrier::workgroup_barrier::workgroup_barrier,
        expected: |value| value,
    },
];

fn generated_input(len: usize, seed: u32) -> Vec<u32> {
    let edge = [
        0,
        1,
        2,
        3,
        31,
        32,
        63,
        64,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_fffe,
        u32::MAX,
    ];
    let mut state = seed;
    (0..len)
        .map(|idx| {
            if idx < edge.len() {
                edge[idx]
            } else {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                state.rotate_left((idx as u32) & 31) ^ ((idx as u32).wrapping_mul(0x9e37_79b9))
            }
        })
        .collect()
}

fn run(program: &Program, input: &[u32]) -> Vec<u8> {
    let input_bytes = vyre_primitives::wire::pack_u32_slice(input);
    let output_bytes = vec![0u8; input.len().max(1) * 4];
    let values = [
        Value::Bytes(input_bytes.into()),
        Value::Bytes(output_bytes.into()),
    ];
    let outputs = vyre_reference::reference_eval(program, &values)
        .expect("Fix: u32 hardware intrinsic builder must execute on the CPU oracle.");
    assert_eq!(
        outputs.len(),
        1,
        "Fix: each u32 intrinsic emits one output buffer."
    );
    outputs[0].to_bytes()
}

#[test]
fn generated_u32_hardware_intrinsics_match_host_semantics() {
    let lengths = [
        1usize, 2, 3, 4, 31, 32, 63, 64, 65, 127, 128, 257, 1024, 4096,
    ];
    let mut checked_lanes = 0usize;

    for case in CASES {
        for &len in &lengths {
            let input = generated_input(len, case.name.len() as u32 ^ len as u32);
            let program = (case.build)("input", "out", len as u32);
            let got = run(&program, &input);
            let expected_words: Vec<u32> = input.iter().copied().map(case.expected).collect();
            let expected = vyre_primitives::wire::pack_u32_slice(&expected_words);
            assert_eq!(got, expected, "{} failed for len {len}", case.name);
            checked_lanes += len;
        }
    }

    assert_eq!(checked_lanes, CASES.len() * lengths.iter().sum::<usize>());
}
