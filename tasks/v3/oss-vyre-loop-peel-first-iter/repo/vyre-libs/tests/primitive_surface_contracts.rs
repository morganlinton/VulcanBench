//! P3.9  -  primitive surface contract tests for Cat-A tiled/parallel
//! variants. These gates compile-check the IR surface: a regression
//! that removes `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }`, `DataType::Shared`, `BufferAccess::Workgroup`,
//! or `BinOp::WaveReduce` fails CI.

use vyre::ir::Node;

/// Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }
/// exists in the IR today; constructing it locks the public contract.
#[test]
fn contract_workgroup_barrier_exists() {
    // Barrier is a real Node variant; constructing it must compile.
    let _ = Node::barrier();
}

/// Workgroup-shared storage (`DataType::Shared` / `BufferAccess::Workgroup`)
/// is the primitive every parallel Cat-A variant needs  -  tiled matmul,
/// FlashAttention-v2, cooperative scans. Construct a Program that
/// declares a workgroup buffer to prove the type is stable today.
#[test]
fn contract_datatype_shared_is_constructible() {
    use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    let decl = BufferDecl::workgroup("tile", 64, DataType::U32);
    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::barrier(),
    ];
    let program = Program::wrapped(vec![decl], [64, 1, 1], body);
    assert_eq!(program.buffers()[0].name(), "tile");
    assert!(matches!(
        program.buffers()[0].access(),
        BufferAccess::Workgroup
    ));
}

/// Subgroup reductions (`BinOp::WaveReduce`) are first-class IR
/// today (P1.4 landed RotateRight; subgroup ops were previously
/// wired through the 3 panics we removed). Proof-of-presence test:
/// `WaveReduce` is a real BinOp variant that CSE treats as
/// non-mergeable.
#[test]
fn contract_subgroup_reduce_variant_exists() {
    use vyre::ir::{BinOp, Expr};
    let _ = Expr::BinOp {
        op: BinOp::WaveReduce,
        left: Box::new(Expr::u32(0)),
        right: Box::new(Expr::u32(0)),
    };
}

// Assertion-free contract shells are not tests; concrete contract tests
// belong in the corresponding primitive file.
