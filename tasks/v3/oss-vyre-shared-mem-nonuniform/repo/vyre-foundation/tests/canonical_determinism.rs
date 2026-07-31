//! Property-based and deterministic tests for canonicalization determinism.

use proptest::prelude::*;
use vyre_foundation::ir::{BinOp, BufferDecl, DataType, Expr, Node, Program};

// ---------------------------------------------------------------------------
// 1. Buffer order independence
// ---------------------------------------------------------------------------
#[test]
fn buffer_order_independence() {
    let body = vec![Node::store("out", Expr::u32(0), Expr::u32(42))];
    let program_abc = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body.clone(),
    );
    let program_cba = Program::wrapped(
        vec![
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        body,
    );

    assert_eq!(
        program_abc.canonical_wire_hash().unwrap(),
        program_cba.canonical_wire_hash().unwrap(),
        "Fix: canonical_wire_hash must be independent of buffer declaration order"
    );
}

// ---------------------------------------------------------------------------
// 2. Commutative operand normalization
// ---------------------------------------------------------------------------
#[test]
fn commutative_operand_normalization() {
    let ops = [
        BinOp::Add,
        BinOp::Mul,
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Min,
        BinOp::Max,
    ];

    for op in ops {
        let forward = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::BinOp {
                    op,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(2)),
                },
            )],
        );
        let reversed = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::BinOp {
                    op,
                    left: Box::new(Expr::u32(2)),
                    right: Box::new(Expr::u32(1)),
                },
            )],
        );

        assert_eq!(
            forward.canonical_wire_hash().unwrap(),
            reversed.canonical_wire_hash().unwrap(),
            "Fix: canonical_wire_hash must normalize commutative operand order for {op:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Nested block flattening
// ---------------------------------------------------------------------------
fn assert_no_blocks(nodes: &[Node]) {
    for node in nodes {
        assert!(
            !matches!(node, Node::Block(_)),
            "Fix: canonicalization must flatten binding-free nested Block wrappers"
        );
        match node {
            Node::Region { body, .. } => assert_no_blocks(body),
            Node::If {
                then, otherwise, ..
            } => {
                assert_no_blocks(then);
                assert_no_blocks(otherwise);
            }
            Node::Loop { body, .. } => assert_no_blocks(body),
            _ => {}
        }
    }
}

#[test]
fn nested_block_flattening() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::block(vec![Node::block(vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::u32(1),
        )])])],
    );
    let canonical = program.canonicalized();
    assert_no_blocks(canonical.entry());
}

// ---------------------------------------------------------------------------
// 4. Region wrapper preservation
// ---------------------------------------------------------------------------
#[test]
fn region_wrapper_preservation() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );
    let canonical = program.canonicalized();

    assert!(
        matches!(canonical.entry(), [Node::Region { .. }]),
        "Fix: canonicalization must preserve the root Region wrapper"
    );
}

// ---------------------------------------------------------------------------
// 5. Fingerprint stability (property-based)
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn fingerprint_stability(seed in any::<u32>()) {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::add(Expr::u32(seed), Expr::var("x")),
            )],
        );

        let once = program.canonicalized();
        let twice = once.canonicalized();

        prop_assert_eq!(
            once.canonical_wire_bytes().unwrap(),
            twice.canonical_wire_bytes().unwrap(),
            "Fix: double canonicalization must produce identical wire bytes"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Non-canonical vs canonical hash
// ---------------------------------------------------------------------------
#[test]
fn non_canonical_vs_canonical_hash() {
    let body = vec![Node::store("out", Expr::u32(0), Expr::u32(42))];
    let program_a = Program::wrapped(
        vec![
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        body.clone(),
    );
    let program_b = Program::wrapped(
        vec![
            BufferDecl::output("out", 2, DataType::U32).with_count(1),
            BufferDecl::read("a", 0, DataType::U32).with_count(4),
            BufferDecl::read("b", 1, DataType::U32).with_count(4),
        ],
        [1, 1, 1],
        body,
    );

    let raw_a = blake3::hash(&program_a.to_wire().expect("Fix: program_a must encode"));
    let raw_b = blake3::hash(&program_b.to_wire().expect("Fix: program_b must encode"));
    let canonical_a = program_a
        .canonical_wire_hash()
        .expect("Fix: program_a must canonicalize");
    let canonical_b = program_b
        .canonical_wire_hash()
        .expect("Fix: program_b must canonicalize");

    assert_ne!(
        raw_a, raw_b,
        "Fix: raw wire hash must differ for programs with different buffer orders"
    );
    assert_eq!(
        canonical_a, canonical_b,
        "Fix: canonical_wire_hash must match for semantically identical programs"
    );
}
