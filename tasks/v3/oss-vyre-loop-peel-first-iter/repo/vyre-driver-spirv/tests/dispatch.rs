//! Integration tests for SPIR-V backend runtime dispatch.
//!
//! Tests build simple programs, dispatch them through the Vulkan-backed
//! SPIR-V backend, and compare results against the CPU reference.
//! When Vulkan probing fails, tests fail with an actionable configuration error.

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_driver_spirv::SpirvBackendRegistration;
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_reference::value::Value;

fn require_vulkan_backend() -> SpirvBackendRegistration {
    SpirvBackendRegistration::acquire().unwrap_or_else(|error| {
        panic!(
            "Fix: SPIR-V dispatch tests require a live Vulkan compute GPU. \
             Missing Vulkan is a driver/probe configuration failure, not a skipped test. \
             Probe error: {error}"
        )
    })
}

/// Build an element-wise add program: out[i] = a[i] + b[i].
fn elementwise_add_program(count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(count),
            BufferDecl::read("b", 1, DataType::U32).with_count(count),
            BufferDecl::output("out", 2, DataType::U32).with_count(count),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(
                Expr::load("a", Expr::gid_x()),
                Expr::load("b", Expr::gid_x()),
            ),
        )],
    )
}

/// Same computation with output at binding 0 to exercise BindingPlan input/output ordering.
fn output_first_elementwise_add_program(count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(count),
            BufferDecl::read("a", 1, DataType::U32).with_count(count),
            BufferDecl::read("b", 2, DataType::U32).with_count(count),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(
                Expr::load("a", Expr::gid_x()),
                Expr::load("b", Expr::gid_x()),
            ),
        )],
    )
}

/// Build a program that computes out[i] = a[i] * 2 + 1.
fn elementwise_fma_program(count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(count),
            BufferDecl::output("out", 1, DataType::U32).with_count(count),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::gid_x(),
            Expr::add(
                Expr::mul(Expr::load("a", Expr::gid_x()), Expr::u32(2)),
                Expr::u32(1),
            ),
        )],
    )
}

fn u32_values_to_bytes(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn bytes_to_u32_values(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Run the CPU reference interpreter and return output bytes.
fn reference_outputs(program: &Program, inputs: &[Value]) -> Vec<Vec<u8>> {
    vyre_reference::reference_eval(program, inputs)
        .expect("Fix: reference evaluation must succeed for valid test programs")
        .iter()
        .map(|v| v.to_bytes())
        .collect()
}

#[test]
fn spirv_output_first_binding_matches_reference() {
    let backend = require_vulkan_backend();

    let count = 128u32;
    let program = output_first_elementwise_add_program(count);

    let a: Vec<u32> = (0..count).map(|i| i.wrapping_mul(5)).collect();
    let b: Vec<u32> = (0..count).map(|i| i.wrapping_mul(7)).collect();

    let inputs_bytes = vec![u32_values_to_bytes(&a), u32_values_to_bytes(&b)];
    let spirv_outputs = backend
        .dispatch(&program, &inputs_bytes, &DispatchConfig::default())
        .expect("Fix: SPIR-V dispatch must bind output-first programs through BindingPlan.");

    let ref_inputs = vec![
        Value::Bytes(u32_values_to_bytes(&a).into()),
        Value::Bytes(u32_values_to_bytes(&b).into()),
    ];
    let ref_outputs = reference_outputs(&program, &ref_inputs);

    assert_eq!(
        spirv_outputs.len(),
        ref_outputs.len(),
        "Fix: SPIR-V output-first binding must produce the same output count as reference."
    );
    assert_eq!(
        bytes_to_u32_values(&spirv_outputs[0]),
        bytes_to_u32_values(&ref_outputs[0]),
        "Fix: SPIR-V must not consume input buffers by raw binding order when an output has binding 0."
    );
}

#[test]
fn spirv_elementwise_add_matches_reference() {
    let backend = require_vulkan_backend();

    let count = 256u32;
    let program = elementwise_add_program(count);

    let a: Vec<u32> = (0..count).collect();
    let b: Vec<u32> = (0..count).map(|i| i.wrapping_mul(3)).collect();

    let inputs_bytes = vec![u32_values_to_bytes(&a), u32_values_to_bytes(&b)];

    let spirv_outputs = backend
        .dispatch(&program, &inputs_bytes, &DispatchConfig::default())
        .expect("Fix: SPIR-V dispatch must succeed");

    let ref_inputs = vec![
        Value::Bytes(
            a.iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
                .into(),
        ),
        Value::Bytes(
            b.iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
                .into(),
        ),
    ];
    let ref_outputs = reference_outputs(&program, &ref_inputs);

    assert_eq!(
        spirv_outputs.len(),
        ref_outputs.len(),
        "Fix: SPIR-V and reference must produce the same number of output buffers"
    );
    for (idx, (spirv, reference)) in spirv_outputs.iter().zip(ref_outputs.iter()).enumerate() {
        let spirv_u32 = bytes_to_u32_values(spirv);
        let ref_u32 = bytes_to_u32_values(reference);
        assert_eq!(
            spirv_u32, ref_u32,
            "Fix: SPIR-V output buffer {idx} does not match reference"
        );
    }
}

#[test]
fn spirv_elementwise_fma_matches_reference() {
    let backend = require_vulkan_backend();

    let count = 128u32;
    let program = elementwise_fma_program(count);

    let a: Vec<u32> = (1..=count).collect();
    let inputs_bytes = vec![u32_values_to_bytes(&a)];

    let spirv_outputs = backend
        .dispatch(&program, &inputs_bytes, &DispatchConfig::default())
        .expect("Fix: SPIR-V dispatch must succeed");

    let ref_inputs = vec![Value::Bytes(
        a.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    )];
    let ref_outputs = reference_outputs(&program, &ref_inputs);

    assert_eq!(
        spirv_outputs.len(),
        ref_outputs.len(),
        "Fix: SPIR-V and reference must produce the same number of output buffers"
    );
    for (idx, (spirv, reference)) in spirv_outputs.iter().zip(ref_outputs.iter()).enumerate() {
        let spirv_u32 = bytes_to_u32_values(spirv);
        let ref_u32 = bytes_to_u32_values(reference);
        assert_eq!(
            spirv_u32, ref_u32,
            "Fix: SPIR-V output buffer {idx} does not match reference"
        );
    }
}

#[test]
fn spirv_backend_factory_reports_backend_identity() {
    let backend = vyre_driver_spirv::spirv_factory()
        .expect("Fix: SPIR-V factory must return a backend handle");
    assert_eq!(backend.id(), vyre_driver_spirv::SPIRV_BACKEND_ID);
}

#[test]
fn spirv_device_buffer_api_rejects_host_shim_fallback() {
    let backend = require_vulkan_backend();
    let err = backend
        .allocate_device_buffer(16)
        .expect_err("Fix: SPIR-V must not allocate HostShimBuffer as a fake resident buffer");
    let msg = format!("{err}");
    assert!(
        msg.contains("DeviceBuffer") && msg.contains("HostShimBuffer dispatch is forbidden"),
        "Fix: SPIR-V DeviceBuffer rejection must name the forbidden host-shim fallback: {msg}"
    );
}

#[test]
fn spirv_backend_id_is_stable() {
    assert_eq!(vyre_driver_spirv::SPIRV_BACKEND_ID, "spirv");
}
