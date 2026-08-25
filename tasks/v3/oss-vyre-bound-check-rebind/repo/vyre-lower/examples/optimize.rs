//! Build a deliberately-inefficient kernel, run the full vyre-lower
//! optimization pipeline, and print before/after stats.
//!
//! Run with: `cargo run --example optimize -p vyre-lower`

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    rewrites::run_all_with_stats, verify, BindingLayout, BindingSlot, BindingVisibility, Dispatch,
    KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn main() {
    // Three bindings  -  `output` is used; `scratch_a` and `scratch_b`
    // are declared but never touched. drop_unused_bindings will strip
    // them.
    let bindings = vec![
        BindingSlot {
            slot: 0,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "output".into(),
        },
        BindingSlot {
            slot: 1,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "scratch_a".into(),
        },
        BindingSlot {
            slot: 2,
            element_type: DataType::U32,
            element_count: None,
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: "scratch_b".into(),
        },
    ];

    // Body: literal arithmetic, dead expressions, redundant store-load,
    // identity ops, dead arithmetic. Every shape exercises a different
    // pass.
    let ops = vec![
        // r0 = Lit(0), r1 = Lit(1), r2 = Lit(7), r3 = Lit(8) (pow-of-2)
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
        // r4 = Add(r2, r0)  -  identity_elim → r2
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Add),
            operands: vec![2, 0],
            result: Some(4),
        },
        // r5 = Mul(r2, r1)  -  identity_elim → r2
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 1],
            result: Some(5),
        },
        // r6 = Mul(r2, r0)  -  absorbing zero → r0
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 0],
            result: Some(6),
        },
        // r7 = Mul(r2, r3)  -  strength_reduce → Shl(r2, 3) → const_fold → Lit(56)
        KernelOp {
            kind: KernelOpKind::BinOpKind(BinOp::Mul),
            operands: vec![2, 3],
            result: Some(7),
        },
        // Store to slot 0  -  overwrites the next store at the same idx
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 0, 4], // slot 0, idx r0, val r4 (=r2 after identity_elim)
            result: None,
        },
        // Reload  -  load_forwarding will turn this into a ref to the
        // just-stored val, then DCE drops it
        KernelOp {
            kind: KernelOpKind::LoadGlobal,
            operands: vec![0, 0],
            result: Some(8),
        },
        // Final store  -  same slot, same idx  -  dead_store will drop the
        // earlier store
        KernelOp {
            kind: KernelOpKind::StoreGlobal,
            operands: vec![0, 0, 7],
            result: None,
        },
    ];

    let desc = KernelDescriptor {
        id: "kitchen_sink".into(),
        bindings: BindingLayout { slots: bindings },
        dispatch: Dispatch::new(64, 1, 1),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals: vec![
                LiteralValue::U32(0),
                LiteralValue::U32(1),
                LiteralValue::U32(7),
                LiteralValue::U32(8),
            ],
        },
    };

    println!("=== before ===");
    println!("ops:           {}", desc.body.ops.len());
    println!("bindings:      {}", desc.bindings.slots.len());
    println!("literals:      {}", desc.body.literals.len());

    let (optimized, stats) = run_all_with_stats(&desc);

    println!();
    println!("=== after run_all_with_stats ===");
    println!(
        "ops:           {} ({} eliminated)",
        stats.ops_after,
        stats.ops_eliminated()
    );
    println!(
        "bindings:      {} ({} dropped)",
        stats.bindings_after,
        stats.bindings_dropped()
    );
    println!(
        "literals:      {} -> {}",
        stats.literals_before, stats.literals_after
    );
    println!("iterations:    {}", stats.iterations);
    println!("converged:     {}", stats.converged);

    println!();
    println!("=== verify ===");
    match verify(&optimized) {
        Ok(()) => println!("OK"),
        Err(errs) => {
            println!("FAIL ({} errors)", errs.len());
            for e in &errs {
                println!("  {e:?}");
            }
            std::process::exit(1);
        }
    }

    println!();
    println!("=== surviving ops ===");
    for (i, op) in optimized.body.ops.iter().enumerate() {
        println!(
            "  [{i:2}] {:?} operands={:?} -> {:?}",
            op.kind, op.operands, op.result
        );
    }
    println!();
    println!("=== surviving bindings ===");
    for s in &optimized.bindings.slots {
        println!("  slot={} name={}", s.slot, s.name);
    }
    println!();
    println!("=== surviving literals ===");
    for (i, l) in optimized.body.literals.iter().enumerate() {
        println!("  pool[{i}] = {l:?}");
    }
}
