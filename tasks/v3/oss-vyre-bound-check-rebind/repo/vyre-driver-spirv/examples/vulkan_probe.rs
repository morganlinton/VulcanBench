//! Minimal Vulkan probe example.

#![allow(unsafe_code)]

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

fn main() {
    println!("Probing Vulkan dispatch...");
    let backend = vyre_driver_spirv::SpirvBackendRegistration::acquire()
        .expect("Fix: Failed to acquire backend");

    println!("Building program...");
    let program = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
            BufferDecl::output("out", 2, DataType::U32).with_count(4),
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
    );

    let a = vec![1u8, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
    let b = vec![10u8, 0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 0, 40, 0, 0, 0];

    println!("Lowering to SPIR-V...");
    let spv = vyre_driver_spirv::SpirvBackend::program_to_spv(&program)
        .expect("Fix: SPIR-V lowering failed");
    println!("SPIR-V: {} words", spv.len());

    println!("Dispatching...");
    match backend.dispatch(&program, &vec![a, b], &DispatchConfig::default()) {
        Ok(outputs) => {
            println!("Dispatch succeeded! {} output buffers", outputs.len());
            for (i, out) in outputs.iter().enumerate() {
                let vals: Vec<u32> = out
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                println!("  output[{i}]: {vals:?}");
            }
        }
        Err(e) => println!("Dispatch failed: {e}"),
    }
}
