//! WGPU integer arithmetic contract tests.

mod common;
use common::u32_bytes;

use vyre_driver::VyreBackend;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[test]
fn u32_mul_accumulate_wraps_to_low_32_bits() {
    let backend = vyre_driver_wgpu::WgpuBackend::acquire()
        .expect("Fix: WGPU arithmetic contract requires a live GPU backend.");
    let program = Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::U32).with_count(2),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(
                Expr::mul(Expr::load("a", Expr::u32(0)), Expr::load("b", Expr::u32(0))),
                Expr::mul(Expr::load("a", Expr::u32(1)), Expr::load("b", Expr::u32(1))),
            ),
        )],
    );

    let out_init = u32_bytes(&[999]);
    let a = u32_bytes(&[65_536, 65_536]);
    let b = u32_bytes(&[65_536, 65_536]);
    let outputs = backend
        .dispatch_borrowed(
            &program,
            &[out_init.as_slice(), a.as_slice(), b.as_slice()],
            &vyre_driver::DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the u32 wrapping arithmetic contract.");
    assert_eq!(outputs, vec![u32_bytes(&[0])]);
}
