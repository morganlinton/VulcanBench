//! Generated descriptor-diff matrix for debug triage invariants.
//!
//! Descriptor diffs are used when narrowing lowering/rewrite failures. This
//! test drives generated root-level descriptor mutations to pin the expected
//! op-count delta and root-shape classification.

use vyre_debug::descriptor_diff::{diff_descriptors, DescriptorDiff};
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn seed_program(seed: u32) -> Program {
    let out_count = 1 + (seed % 32);
    Program::wrapped(
        vec![
            BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(out_count),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_count),
        ],
        [64, 1, 1],
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(out_count)),
                vec![Node::store(
                    "out",
                    Expr::var("idx"),
                    Expr::add(
                        Expr::load("a", Expr::var("idx")),
                        Expr::u32(seed.rotate_left(seed & 31)),
                    ),
                )],
            ),
        ],
    )
}

fn append_literal_ops(desc: &mut vyre_lower::KernelDescriptor, count: usize) {
    for offset in 0..count {
        desc.body.ops.push(vyre_lower::KernelOp {
            result: Some(100_000 + offset as u32),
            kind: vyre_lower::KernelOpKind::Literal,
            operands: vec![offset as u32],
        });
    }
}

fn root_delta(diff: &DescriptorDiff) -> i64 {
    diff.op_count_delta
        .get(&Vec::<usize>::new())
        .copied()
        .unwrap_or(0)
}

#[test]
fn generated_root_op_count_deltas_match_controlled_mutations() {
    for seed in 0..512u32 {
        let program = seed_program(seed);
        let before = vyre_lower::lower(&program).expect("Fix: seed program must lower.");
        let mut after = before.clone();
        let appended = 1 + (seed as usize % 11);
        append_literal_ops(&mut after, appended);

        let diff = diff_descriptors(&before, &after);
        assert_eq!(
            root_delta(&diff),
            appended as i64,
            "seed {seed} appended {appended} root ops"
        );
        assert!(
            diff.root_shape_changed,
            "seed {seed}: appending root ops must change root shape"
        );
        assert!(diff.bindings_added.is_empty());
        assert!(diff.bindings_dropped.is_empty());
    }
}

#[test]
fn generated_descriptor_diffs_round_trip_through_json() {
    for seed in 0..256u32 {
        let program = seed_program(seed ^ 0x5eed_5eed);
        let before = vyre_lower::lower(&program).expect("Fix: seed program must lower.");
        let mut after = before.clone();
        let appended = 1 + (seed as usize % 7);
        append_literal_ops(&mut after, appended);
        let diff = diff_descriptors(&before, &after);

        let json =
            serde_json::to_vec(&diff).expect("Fix: DescriptorDiff must serialize for debug logs.");
        let restored: DescriptorDiff = serde_json::from_slice(&json)
            .expect("Fix: DescriptorDiff must deserialize from debug logs.");
        assert_eq!(restored.bindings_added, diff.bindings_added);
        assert_eq!(restored.bindings_dropped, diff.bindings_dropped);
        assert_eq!(restored.op_count_delta, diff.op_count_delta);
        assert_eq!(restored.root_shape_changed, diff.root_shape_changed);
    }
}
