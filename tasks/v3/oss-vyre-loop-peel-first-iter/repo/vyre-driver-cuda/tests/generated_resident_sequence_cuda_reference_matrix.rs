//! Generated live CUDA-resident sequence/reference differential matrix.

mod common;
#[path = "generated_resident_sequence_cuda_reference_matrix/basic_sequence_contracts.rs"]
mod basic_sequence_contracts;
#[path = "generated_resident_sequence_cuda_reference_matrix/repeated_sequence_contracts.rs"]
mod repeated_sequence_contracts;

use common::{
    assert_compact_ranges_match, assert_f32_output_lanes, assert_u32_output_lanes, bool_bytes,
    compact_word_ranges, f32_bytes, generated_mixed_bool_values as generated_bool_values,
    generated_mixed_u32_values as generated_u32_values, live_backend, overlapping_word_ranges,
    reference_outputs, u32_bytes, GENERATED_LANE_COUNT as LANE_COUNT,
    GENERATED_WORKGROUP_SIZE_X as WORKGROUP_SIZE_X,
};
use vyre_driver::backend::{ResidentDispatchStep, ResidentReadRange};
use vyre_driver::VyreBackend;
use vyre_driver_cuda::CudaBackendRegistration;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

const OUTPUT_BYTES: usize = LANE_COUNT * std::mem::size_of::<u32>();
const MAX_F32_ULP: u32 = 1;

fn dispatch_two_step_sequence(
    backend: &CudaBackendRegistration,
    first: &Program,
    second: &Program,
    input_bytes: &[u8],
    ty: DataType,
    case_name: &str,
) -> Vec<u8> {
    let mut outputs = dispatch_two_step_sequence_read_ranges(
        backend,
        first,
        second,
        input_bytes,
        &[(0, OUTPUT_BYTES)],
        ty,
        case_name,
    );
    outputs.remove(0)
}

fn dispatch_two_step_sequence_read_ranges(
    backend: &CudaBackendRegistration,
    first: &Program,
    second: &Program,
    input_bytes: &[u8],
    ranges: &[(usize, usize)],
    ty: DataType,
    case_name: &str,
) -> Vec<Vec<u8>> {
    let input = VyreBackend::allocate_resident(backend, OUTPUT_BYTES)
        .unwrap_or_else(|error| panic!("Fix: {case_name} input allocation failed: {error}"));
    let tmp = VyreBackend::allocate_resident(backend, OUTPUT_BYTES)
        .unwrap_or_else(|error| panic!("Fix: {case_name} temporary allocation failed: {error}"));
    let output = VyreBackend::allocate_resident(backend, OUTPUT_BYTES)
        .unwrap_or_else(|error| panic!("Fix: {case_name} output allocation failed: {error}"));
    let result = (|| {
        VyreBackend::upload_resident(backend, &input, input_bytes)
            .map_err(|error| format!("Fix: {case_name} input upload failed: {error}"))?;
        let first_resources = [input.clone(), tmp.clone()];
        let second_resources = [tmp.clone(), output.clone()];
        let steps = [
            ResidentDispatchStep {
                program: first,
                resources: &first_resources,
                grid_override: None,
                workgroup_override: None,
            },
            ResidentDispatchStep {
                program: second,
                resources: &second_resources,
                grid_override: None,
                workgroup_override: None,
            },
        ];
        let read_ranges: Vec<_> = ranges
            .iter()
            .map(|(byte_offset, byte_len)| ResidentReadRange {
                resource: &output,
                byte_offset: *byte_offset,
                byte_len: *byte_len,
            })
            .collect();
        let mut outputs: Vec<Vec<u8>> = ranges.iter().map(|_| Vec::new()).collect();
        {
            let mut output_refs: Vec<&mut Vec<u8>> = outputs.iter_mut().collect();
            VyreBackend::dispatch_resident_sequence_read_ranges_into(
                backend,
                &steps,
                &read_ranges,
                &mut output_refs,
            )
            .map_err(|error| {
                format!("Fix: {case_name} resident sequence dispatch failed: {error}")
            })?;
        }
        for (index, (output, (_, expected_len))) in outputs.iter().zip(ranges.iter()).enumerate() {
            if output.len() != *expected_len {
                return Err(format!(
                    "Fix: {case_name} range {index} returned {} byte(s) for {:?}, expected {}.",
                    output.len(),
                    ty,
                    expected_len
                ));
            }
        }
        Ok(outputs)
    })();
    let free_input = VyreBackend::free_resident(backend, input);
    let free_tmp = VyreBackend::free_resident(backend, tmp);
    let free_output = VyreBackend::free_resident(backend, output);
    if let Err(error) = free_input {
        panic!("Fix: {case_name} input cleanup failed: {error}");
    }
    if let Err(error) = free_tmp {
        panic!("Fix: {case_name} temporary cleanup failed: {error}");
    }
    if let Err(error) = free_output {
        panic!("Fix: {case_name} output cleanup failed: {error}");
    }
    result.unwrap_or_else(|error| panic!("{error}"))
}

fn u32_sequence_first_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("tmp", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "tmp",
                Expr::gid_x(),
                Expr::bitxor(
                    Expr::mul(
                        Expr::add(Expr::load("input", Expr::gid_x()), Expr::u32(7)),
                        Expr::u32(3),
                    ),
                    Expr::u32(0xa5a5_5a5a),
                ),
            )],
        )],
    )
}

fn u32_sequence_second_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("tmp", 0, DataType::U32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::U32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::reverse_bits(Expr::shr(
                    Expr::load("tmp", Expr::gid_x()),
                    Expr::add(Expr::gid_x(), Expr::u32(33)),
                )),
            )],
        )],
    )
}

fn bool_sequence_first_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("tmp", 1, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "tmp",
                Expr::gid_x(),
                Expr::not(Expr::load("input", Expr::gid_x())),
            )],
        )],
    )
}

fn bool_sequence_second_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("tmp", 0, DataType::Bool).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::Bool).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::select(
                    Expr::eq(Expr::bitand(Expr::gid_x(), Expr::u32(1)), Expr::u32(0)),
                    Expr::load("tmp", Expr::gid_x()),
                    Expr::not(Expr::load("tmp", Expr::gid_x())),
                ),
            )],
        )],
    )
}

fn f32_sequence_first_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("input", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("tmp", 1, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "tmp",
                Expr::gid_x(),
                Expr::fma(
                    Expr::load("input", Expr::gid_x()),
                    Expr::f32(0.5),
                    Expr::f32(1.25),
                ),
            )],
        )],
    )
}

fn f32_sequence_second_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("tmp", 0, DataType::F32).with_count(LANE_COUNT as u32),
            BufferDecl::output("out", 1, DataType::F32).with_count(LANE_COUNT as u32),
        ],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "out",
                Expr::gid_x(),
                Expr::sqrt(Expr::abs(Expr::load("tmp", Expr::gid_x()))),
            )],
        )],
    )
}

fn generated_f32_values(salt: u32) -> Vec<f32> {
    (0..LANE_COUNT)
        .map(|lane| {
            let lane = lane as u32;
            let bits = match lane % 14 {
                0 => 0x0000_0000,
                1 => 0x8000_0000,
                2 => 0x3f80_0000,
                3 => 0xbf80_0000,
                4 => 0x4000_0000,
                5 => 0xc000_0000,
                6 => 0x3f00_0000,
                7 => 0xbf00_0000,
                8 => 0x0080_0000,
                9 => 0x8080_0000,
                10 => 0x7f7f_ffff,
                11 => 0xff7f_ffff,
                _ => (lane.wrapping_mul(0x0101_0101) ^ salt).rotate_left(lane & 15) & 0x7f7f_ffff,
            };
            f32::from_bits(bits)
        })
        .collect()
}


fn repeated_reference_outputs(
    prefix: &Program,
    repeated: &Program,
    input: &[u8],
    repeat_count: u32,
    label: &str,
) -> Vec<Vec<u8>> {
    let mut state = reference_outputs(
        prefix,
        &[input.to_vec()],
        &format!("repeated_resident_{label}_prefix"),
    );
    for step in 0..repeat_count {
        state = reference_outputs(
            repeated,
            &state,
            &format!("repeated_resident_{label}_step_{step}"),
        );
    }
    state
}

fn dispatch_repeated_in_place_sequence(
    backend: &CudaBackendRegistration,
    prefix: &Program,
    repeated: &Program,
    input_bytes: &[u8],
    repeat_count: u32,
    ty: DataType,
    case_name: &str,
) -> Vec<u8> {
    let mut outputs = dispatch_repeated_in_place_sequence_read_ranges(
        backend,
        prefix,
        repeated,
        input_bytes,
        repeat_count,
        &[(0, OUTPUT_BYTES)],
        ty,
        case_name,
    );
    outputs.remove(0)
}

fn dispatch_repeated_in_place_sequence_read_ranges(
    backend: &CudaBackendRegistration,
    prefix: &Program,
    repeated: &Program,
    input_bytes: &[u8],
    repeat_count: u32,
    ranges: &[(usize, usize)],
    ty: DataType,
    case_name: &str,
) -> Vec<Vec<u8>> {
    let state = VyreBackend::allocate_resident(backend, OUTPUT_BYTES)
        .unwrap_or_else(|error| panic!("Fix: {case_name} state allocation failed: {error}"));
    let result = (|| {
        VyreBackend::upload_resident(backend, &state, input_bytes)
            .map_err(|error| format!("Fix: {case_name} state upload failed: {error}"))?;
        let prefix_resources = [state.clone()];
        let repeated_resources = [state.clone()];
        let prefix_steps = [ResidentDispatchStep {
            program: prefix,
            resources: &prefix_resources,
            grid_override: None,
            workgroup_override: None,
        }];
        let repeated_steps = [ResidentDispatchStep {
            program: repeated,
            resources: &repeated_resources,
            grid_override: None,
            workgroup_override: None,
        }];
        let read_ranges: Vec<_> = ranges
            .iter()
            .map(|(byte_offset, byte_len)| ResidentReadRange {
                resource: &state,
                byte_offset: *byte_offset,
                byte_len: *byte_len,
            })
            .collect();
        let mut outputs: Vec<Vec<u8>> = ranges.iter().map(|_| Vec::new()).collect();
        {
            let mut output_refs: Vec<&mut Vec<u8>> = outputs.iter_mut().collect();
            VyreBackend::dispatch_resident_repeated_sequence_read_ranges_into(
                backend,
                &prefix_steps,
                &repeated_steps,
                repeat_count,
                &read_ranges,
                &mut output_refs,
            )
            .map_err(|error| {
                format!("Fix: {case_name} repeated resident sequence dispatch failed: {error}")
            })?;
        }
        for (index, (output, (_, expected_len))) in outputs.iter().zip(ranges.iter()).enumerate() {
            if output.len() != *expected_len {
                return Err(format!(
                    "Fix: {case_name} range {index} returned {} byte(s) for {:?}, expected {}.",
                    output.len(),
                    ty,
                    expected_len
                ));
            }
        }
        Ok(outputs)
    })();
    if let Err(error) = VyreBackend::free_resident(backend, state) {
        panic!("Fix: {case_name} state cleanup failed: {error}");
    }
    result.unwrap_or_else(|error| panic!("{error}"))
}

fn repeated_u32_prefix_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::bitxor(
                    Expr::add(Expr::load("state", Expr::gid_x()), Expr::u32(11)),
                    Expr::mul(Expr::gid_x(), Expr::u32(0x0101_0101)),
                ),
            )],
        )],
    )
}

fn repeated_u32_step_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::U32).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::reverse_bits(Expr::shl(
                    Expr::bitxor(Expr::load("state", Expr::gid_x()), Expr::u32(0x9e37_79b9)),
                    Expr::add(Expr::gid_x(), Expr::u32(65)),
                )),
            )],
        )],
    )
}

fn repeated_bool_prefix_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::Bool).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::not(Expr::load("state", Expr::gid_x())),
            )],
        )],
    )
}

fn repeated_bool_step_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::Bool).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::select(
                    Expr::eq(Expr::bitand(Expr::gid_x(), Expr::u32(3)), Expr::u32(0)),
                    Expr::not(Expr::load("state", Expr::gid_x())),
                    Expr::load("state", Expr::gid_x()),
                ),
            )],
        )],
    )
}

fn repeated_f32_prefix_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::F32).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::abs(Expr::fma(
                    Expr::load("state", Expr::gid_x()),
                    Expr::f32(0.25),
                    Expr::f32(2.0),
                )),
            )],
        )],
    )
}

fn repeated_f32_step_program() -> Program {
    Program::wrapped(
        vec![BufferDecl::read_write("state", 0, DataType::F32).with_count(LANE_COUNT as u32)],
        [WORKGROUP_SIZE_X, 1, 1],
        vec![Node::if_then(
            Expr::lt(Expr::gid_x(), Expr::u32(LANE_COUNT as u32)),
            vec![Node::store(
                "state",
                Expr::gid_x(),
                Expr::sqrt(Expr::add(
                    Expr::abs(Expr::load("state", Expr::gid_x())),
                    Expr::f32(0.5),
                )),
            )],
        )],
    )
}
