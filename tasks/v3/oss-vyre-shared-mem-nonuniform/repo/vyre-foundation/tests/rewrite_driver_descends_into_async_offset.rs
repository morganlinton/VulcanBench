//! Regression: the canonical expression-rewrite driver `rewrite_node_cow`
//! (behind `rewrite_program`) must descend into the `offset` and `size`
//! expressions of `AsyncLoad` / `AsyncStore`.
//!
//! `rewrite_program` is documented as "run an expression-rewrite closure over
//! EVERY node in the program". It descends into Let/Assign/Store/If/Loop/
//! Block/Region AND `Node::Trap`'s address expression -- but it lumped
//! `AsyncLoad` / `AsyncStore` into the no-rewrite `Cow::Borrowed(node)` arm
//! alongside genuinely expr-free nodes (Return/Barrier/AllReduce/...), even
//! though async copies carry `offset: Box<Expr>` and `size: Box<Expr>`. So any
//! pass routed through `rewrite_program` (const_fold, strength_reduce) silently
//! skipped those two expression positions.
//!
//! For const_fold/strength_reduce this is a missed optimization (a constant
//! offset is left unfolded but still evaluates correctly); the deeper hazard is
//! that the canonical driver is the one every pass is meant to route through,
//! so a CORRECTNESS rewrite (e.g. copy-propagation that drops the source
//! binding) routed through it would leave a dangling reference in an async
//! offset. Either way the driver's "every node" contract is violated.
//!
//! Proof vehicle: `ConstFold::transform` (a public pass that routes through
//! `rewrite_program`). A constant `2 + 3` in an async offset must fold to `5`.

use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::optimizer::passes::const_fold::ConstFold;

/// An `AsyncLoad` and an `AsyncStore` whose offset/size are literal BinOps that
/// const-fold to known values.
fn program_with_const_in_async_offset_and_size() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::read("src", 0, DataType::U32).with_count(8),
            BufferDecl::read_write("dst", 1, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![
            Node::async_load_ext(
                "src",
                "dst",
                Expr::add(Expr::u32(2), Expr::u32(3)), // -> 5
                Expr::add(Expr::u32(4), Expr::u32(4)), // -> 8
                "tl",
            ),
            Node::async_store(
                "dst",
                "dst",
                Expr::add(Expr::u32(10), Expr::u32(1)), // -> 11
                Expr::add(Expr::u32(6), Expr::u32(6)),  // -> 12
                "ts",
            ),
        ],
    )
}

/// Recursively assert every AsyncLoad/AsyncStore offset/size is the folded
/// literal. `Program::wrapped` nests the entry in a root `Region`, so the async
/// nodes sit one level down -- descend into every body-bearing node.
fn check_async_offsets(nodes: &[Node], checked: &mut usize) {
    for node in nodes {
        match node {
            Node::AsyncLoad { offset, size, .. } => {
                assert_eq!(
                    **offset,
                    Expr::u32(5),
                    "async_load offset must fold 2+3 -> 5, got {offset:?}"
                );
                assert_eq!(
                    **size,
                    Expr::u32(8),
                    "async_load size must fold 4+4 -> 8, got {size:?}"
                );
                *checked += 1;
            }
            Node::AsyncStore { offset, size, .. } => {
                assert_eq!(
                    **offset,
                    Expr::u32(11),
                    "async_store offset must fold 10+1 -> 11, got {offset:?}"
                );
                assert_eq!(
                    **size,
                    Expr::u32(12),
                    "async_store size must fold 6+6 -> 12, got {size:?}"
                );
                *checked += 1;
            }
            Node::If {
                then, otherwise, ..
            } => {
                check_async_offsets(then, checked);
                check_async_offsets(otherwise, checked);
            }
            Node::Loop { body, .. } => check_async_offsets(body, checked),
            Node::Block(body) => check_async_offsets(body, checked),
            Node::Region { body, .. } => check_async_offsets(body, checked),
            _ => {}
        }
    }
}

#[test]
fn const_fold_folds_constants_inside_async_offset_and_size() {
    let program = program_with_const_in_async_offset_and_size();
    let folded = ConstFold::transform(program).program;

    // The canonical rewrite driver must have descended into both async nodes'
    // offset/size. Pre-fix it skipped them, so the BinOps survived unfolded
    // (offset is `BinOp(Add, 2, 3)`, NOT `LitU32(5)`).
    let mut checked = 0;
    check_async_offsets(folded.entry(), &mut checked);
    assert_eq!(
        checked, 2,
        "both async nodes must survive const_fold and be checked (found {checked})"
    );
}
