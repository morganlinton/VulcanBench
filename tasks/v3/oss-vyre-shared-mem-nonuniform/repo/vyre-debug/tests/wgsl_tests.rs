//! Test: wgsl tests.
use vyre_debug::wgsl::{dump_wgsl, dump_wgsl_with_lines};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

fn minimal_program() -> Program {
    let buffer =
        BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16);
    Program::wrapped(
        vec![buffer],
        [64, 1, 1],
        vec![Node::Store {
            buffer: Ident::from("out"),
            index: Expr::InvocationId { axis: 0 },
            value: Expr::LitU32(7),
        }],
    )
}

#[test]
fn dump_wgsl_minimal_program_returns_compute_entry() {
    let p = minimal_program();
    let dump = dump_wgsl(&p).unwrap();
    assert!(dump.text.contains("@compute @workgroup_size"));
    assert!(dump.text.contains("fn main"));
}

#[test]
fn dump_wgsl_with_lines_prefixes_each_line() {
    let p = minimal_program();
    let dump = dump_wgsl_with_lines(&p).unwrap();
    for line in dump.text.lines() {
        if !line.trim().is_empty() {
            // Regex: ^\s*\d+ \|
            let trimmed = line.trim_start();
            let mut parts = trimmed.splitn(2, " | ");
            let num = parts.next().unwrap();
            assert!(num.parse::<usize>().is_ok());
            assert!(parts.next().is_some());
        }
    }
}

#[test]
fn dump_wgsl_propagates_naga_validation_failure() {
    // To trigger a naga validation failure but pass lowering, we can create an invalid construct
    // e.g., mismatched types that vyre-lower accepts but naga rejects.
    // However, vyre_lower has type verification.
    // Let's create an invalid assignment type mismatch by bypassing `Expr` helpers.
    // Or just manually corrupt a Lowered descriptor? No, the API takes Program.
    // Let's use `vyre_lower::lower_for_emit` and intercept, or wait, we just use a Program that vyre_lower doesn't catch.
    let buffer =
        BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(16);
    let p = Program::wrapped(
        vec![buffer],
        [0, 1, 1], // Workgroup size 0 is valid in vyre but invalid in Naga
        vec![Node::Store {
            buffer: Ident::from("out"),
            index: Expr::InvocationId { axis: 0 },
            value: Expr::u32(7),
        }],
    );
    let err = match dump_wgsl(&p) {
        Err(e) => e,
        Ok(_) => panic!("Expected error"),
    };
    assert!(
        err.to_lowercase().contains("failed") || err.to_lowercase().contains("error"),
        "Expected error message to contain 'failed' or 'error', got: {}",
        err
    );
}
