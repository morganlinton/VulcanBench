//! Same-width integer store-coercion parity on the live GPU.
//!
//! A value whose static type is a 32-bit integer can be stored into a buffer
//! whose element is the OTHER 32-bit-integer signedness (U32<->I32): the
//! validator now permits it (`same_width_int_reinterpret`) and the naga emitter
//! coerces the store value to the element type via `As{Sint/Uint, 4}` — a
//! bit-exact reinterpret. This test stores a value of the "wrong" signedness into
//! a buffer and reads back the raw word, proving the coercion preserves every bit
//! on real hardware.
//!
//! The chosen ops (subtract, bitwise-and) are bit-identical for signed and
//! unsigned operands (two's-complement), so this test isolates the STORE
//! coercion from any operation-level signedness question.

mod common;
use common::u32_bytes;

use vyre_driver::{DispatchConfig, VyreBackend};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_driver_wgpu::WgpuBackend;

/// `out[i] = a[i] - b[i]` where a/b are I32 and out is U32 — an I32-typed value
/// (Sub goes through Frame::Bin, preserving the I32 operand type) stored into a
/// U32 buffer. Subtraction is two's-complement so the bits are signedness-
/// independent; the store coercion (I32 -> As{Uint,4} -> array<u32>) must keep
/// every bit, so a negative difference reads back as its u32 bit pattern.
fn sub_i32_into_u32_program(n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::sub(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::I32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::I32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

/// `out[i] = a[i] & b[i]` where a/b are U32 and out is I32 — a U32-typed value
/// (BitAnd defaults to U32) stored into an I32 buffer. The store coercion
/// (U32 -> As{Sint,4} -> array<i32>) must keep every bit.
fn and_u32_into_i32_program(n: u32) -> Program {
    let mut body = Vec::new();
    for i in 0..n {
        body.push(Node::store(
            "out",
            Expr::u32(i),
            Expr::bitand(Expr::load("a", Expr::u32(i)), Expr::load("b", Expr::u32(i))),
        ));
    }
    Program::wrapped(
        vec![
            BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::I32).with_count(n),
            BufferDecl::storage("a", 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage("b", 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
        ],
        [1, 1, 1],
        body,
    )
}

fn dispatch_words(backend: &WgpuBackend, program: &Program, a: &[u32], b: &[u32]) -> Vec<u32> {
    let out_init = u32_bytes(&vec![0u32; a.len()]);
    let outputs = backend
        .dispatch_borrowed(
            program,
            &[out_init.as_slice(), u32_bytes(a).as_slice(), u32_bytes(b).as_slice()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU must dispatch the same-width store contract.");
    outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[test]
fn i32_value_stored_into_u32_buffer_preserves_bits_on_gpu() {
    let backend = WgpuBackend::acquire()
        .expect("Fix: same-width store parity requires a live GPU backend.");
    // a - b for (5,8)=-3, (10,2)=8, (0,1)=-1, (-100,-100)=0, (i32::MIN,1)=wrap.
    let a: Vec<i32> = vec![5, 10, 0, -100, i32::MIN];
    let b: Vec<i32> = vec![8, 2, 1, -100, 1];
    let a_u: Vec<u32> = a.iter().map(|&x| x as u32).collect();
    let b_u: Vec<u32> = b.iter().map(|&x| x as u32).collect();
    let n = a.len() as u32;
    let gpu = dispatch_words(&backend, &sub_i32_into_u32_program(n), &a_u, &b_u);
    let expected: Vec<u32> = a
        .iter()
        .zip(&b)
        .map(|(&x, &y)| x.wrapping_sub(y) as u32)
        .collect();
    assert_eq!(
        expected,
        vec![(-3i32) as u32, 8, (-1i32) as u32, 0, i32::MIN.wrapping_sub(1) as u32],
        "reference two's-complement subtraction drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU store of an i32-typed value into a u32 buffer dropped/changed bits.\n  \
         expected: {expected:?}\n  gpu: {gpu:?}"
    );
    // The first slot reinterprets to -3, confirming the negative value survived.
    assert_eq!(gpu[0] as i32, -3, "5 - 8 must read back as -3 from the u32 buffer");
}

#[test]
fn u32_value_stored_into_i32_buffer_preserves_bits_on_gpu() {
    let backend = WgpuBackend::acquire()
        .expect("Fix: same-width store parity requires a live GPU backend.");
    let a: Vec<u32> = vec![0xFFFF_FFFF, 0x1234_5678, 0x8000_0000, 0x0000_FF00];
    let b: Vec<u32> = vec![0x0000_FF00, 0xFFFF_0000, 0xFFFF_FFFF, 0x00FF_FF00];
    let n = a.len() as u32;
    let gpu = dispatch_words(&backend, &and_u32_into_i32_program(n), &a, &b);
    let expected: Vec<u32> = a.iter().zip(&b).map(|(&x, &y)| x & y).collect();
    assert_eq!(
        expected,
        vec![0x0000_FF00, 0x1234_0000, 0x8000_0000, 0x0000_FF00],
        "reference bitwise-and drifted"
    );
    assert_eq!(
        gpu, expected,
        "GPU store of a u32-typed value into an i32 buffer dropped/changed bits.\n  \
         expected: {expected:?}\n  gpu: {gpu:?}"
    );
    // The high-bit-set word reinterprets to a negative i32, surviving the coercion.
    assert_eq!(gpu[2] as i32, i32::MIN, "0x80000000 must survive as i32::MIN");
}
