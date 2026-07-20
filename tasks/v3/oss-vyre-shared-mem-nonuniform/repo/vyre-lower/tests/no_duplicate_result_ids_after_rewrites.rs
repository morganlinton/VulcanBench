//! Pin-test: every shipped rewrite combination must preserve the
//! "every result_id is unique across the body tree" invariant.
//!
//! emit-naga's `bind_result` uses last-write-wins on a single
//! `values: BTreeMap<u32, Handle>`  -  when two ops in different bodies
//! produce the same result_id, the second overwrites the first and any
//! cross-block read of either binding dangles in the WGSL output
//! (naga's parser rejects with `no definition in scope for identifier
//! _eN`). The licm-shared-allocator regression that produced 32
//! duplicate ids on P-6 (`semantic_pg`) only surfaced after the lex
//! fixes landed and the pipeline reached P-6  -  it had been silently
//! corrupting earlier IRs too. This test would have caught it at
//! commit time.
//!
//! Strategy: build a small kernel that exercises every shape known
//! to trip an id-allocation discipline failure (nested loops, hoist-
//! eligible invariants in inner loops, multiple if-then branches
//! contributing merge-Selects, structured-block scopes), run
//! `vyre_lower::rewrites::run_all_with_stats` on it, and walk the
//! resulting body asserting that every result_id appears exactly
//! once.

use std::collections::BTreeMap;

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    rewrites::run_all_with_stats, BindingLayout, BindingSlot, BindingVisibility, Dispatch,
    KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn count_result_ids(body: &KernelBody) -> BTreeMap<u32, u32> {
    fn walk(body: &KernelBody, out: &mut BTreeMap<u32, u32>) {
        for op in &body.ops {
            if let Some(r) = op.result {
                *out.entry(r).or_insert(0) += 1;
            }
        }
        for child in &body.child_bodies {
            walk(child, out);
        }
    }
    let mut out = BTreeMap::new();
    walk(body, &mut out);
    out
}

fn assert_no_duplicates(body: &KernelBody, label: &str) {
    let counts = count_result_ids(body);
    let dups: Vec<_> = counts.iter().filter(|(_, &n)| n > 1).collect();
    assert!(
        dups.is_empty(),
        "{label}: {} duplicate result_ids after rewrites: {:?}",
        dups.len(),
        dups.iter().take(8).collect::<Vec<_>>(),
    );
}

fn nested_loop_with_hoistable_invariant_descriptor() -> KernelDescriptor {
    // Outer loop 0..N, body contains:
    //   - an inner loop 0..M whose body has a Literal op (hoist
    //     candidate at depth-2)
    //   - an if-then branch that conditionally rebinds a value
    //     (forces merge-Select emission in the parent body)
    // Plus a top-level loop and an outer-scope hoist candidate.
    // This is the shape that previously produced duplicate ids when
    // licm's per-recursion subtree-only id-max scan missed sibling
    // hoists.
    let bindings = vec![BindingSlot {
        slot: 0,
        element_type: DataType::U32,
        element_count: Some(64),
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: "out".into(),
    }];

    // SSA id allocation (manual, must match a plausible lower output)
    // 0..3: pre-loop literals + GlobalInvocationId
    // 10..14: inner loop body computations
    // 20..24: outer loop body computations
    let ops = vec![
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![0],
            result: Some(0),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![1],
            result: Some(1),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![2],
            result: Some(2),
        },
        KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![3],
            result: Some(3),
        },
        KernelOp {
            kind: KernelOpKind::GlobalInvocationId,
            operands: vec![0],
            result: Some(4),
        },
        KernelOp {
            kind: KernelOpKind::StructuredForLoop {
                loop_var: "i".into(),
            },
            operands: vec![0, 1, 0],
            result: None,
        },
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 4, 24],
            result: None,
        },
    ];
    let inner_body = KernelBody {
        ops: vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![10, 2],
                result: Some(11),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![11, 3],
                result: Some(12),
            },
        ],
        child_bodies: vec![],
        literals: vec![LiteralValue::U32(7)],
    };
    let outer_body = KernelBody {
        ops: vec![
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(20),
            },
            KernelOp {
                kind: KernelOpKind::StructuredForLoop {
                    loop_var: "j".into(),
                },
                operands: vec![2, 3, 0],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![20, 12],
                result: Some(24),
            },
        ],
        child_bodies: vec![inner_body],
        literals: vec![LiteralValue::U32(5)],
    };
    KernelDescriptor {
        id: "nested_loop_hoist".into(),
        bindings: BindingLayout { slots: bindings },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![outer_body],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(8),
                LiteralValue::U32(0),
                LiteralValue::U32(4),
            ],
        },
    }
}

#[test]
fn input_descriptor_starts_with_unique_ids() {
    let desc = nested_loop_with_hoistable_invariant_descriptor();
    assert_no_duplicates(&desc.body, "input descriptor");
}

#[test]
fn nested_loop_hoist_produces_no_duplicate_result_ids() {
    let desc = nested_loop_with_hoistable_invariant_descriptor();
    let (out, _stats) = run_all_with_stats(&desc);
    assert_no_duplicates(&out.body, "post-rewrites");
}

#[test]
fn idempotent_rewrites_preserve_unique_ids() {
    // Run the rewrite suite TWICE  -  id allocation must remain stable
    // across re-rewrite. Previously a stale `next_free_id` could
    // collide with ids freshly allocated by the previous iteration.
    let desc = nested_loop_with_hoistable_invariant_descriptor();
    let (once, _) = run_all_with_stats(&desc);
    assert_no_duplicates(&once.body, "post-rewrites (1st pass)");
    let (twice, _) = run_all_with_stats(&once);
    assert_no_duplicates(&twice.body, "post-rewrites (2nd pass)");
}
