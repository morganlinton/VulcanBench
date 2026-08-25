//! End-to-end test: GPU CSE on real CUDA hardware.
//!
//! Builds a Program with structurally identical sub-expressions,
//! dispatches the two CSE kernels (structural-hash + canonical-id)
//! through `CudaOptimizerDispatcher`, and verifies the canonical
//! buffer assigns equal canonicals to syntactically equal Exprs.

#![cfg(test)]

mod common;

use common::live_backend;
use vyre::ir::{Expr, Node, Program};
use vyre_driver_cuda::CudaOptimizerDispatcher;
use vyre_self_substrate::optimizer::cse_via_encoded::gpu_cse_canonicals;

#[test]
fn cuda_cse_finds_canonicals_for_equal_literal_pairs() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // Two identical literal pairs in different lets:
    //   let a = 5         (LitU32 5 -> hash H_5)
    //   let b = 5         (LitU32 5 -> hash H_5)
    //   let c = 7         (LitU32 7 -> hash H_7)
    //   store buf 0 0
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::u32(5)),
            Node::let_bind("b", Expr::u32(5)),
            Node::let_bind("c", Expr::u32(7)),
            Node::store("buf", Expr::u32(0), Expr::u32(0)),
        ],
    );

    let (arena, canonical) =
        gpu_cse_canonicals(&p, &dispatcher).expect("gpu_cse_canonicals must succeed");
    assert_eq!(canonical.len(), arena.expr_count as usize);

    // Find the LitU32(5) Expr ids  -  they should share a canonical.
    let mut lit5_ids: Vec<u32> = Vec::new();
    let mut lit7_ids: Vec<u32> = Vec::new();
    for (i, &kind) in arena.kinds.iter().enumerate() {
        if kind == vyre_self_substrate::optimizer::expr_arena::expr_kind::LIT_U32 {
            match arena.arg0[i] {
                5 => lit5_ids.push(i as u32),
                7 => lit7_ids.push(i as u32),
                _ => {}
            }
        }
    }
    assert!(
        lit5_ids.len() >= 2,
        "Program must encode at least two LitU32(5) entries; got {lit5_ids:?}"
    );
    assert!(
        !lit7_ids.is_empty(),
        "Program must encode at least one LitU32(7) entry"
    );

    // First LitU32(5) is its own canonical.
    let canon5 = canonical[lit5_ids[0] as usize];
    assert_eq!(
        canon5, lit5_ids[0],
        "the first LitU32(5) must be its own canonical"
    );
    // All subsequent LitU32(5) entries point to the first.
    for &id in &lit5_ids[1..] {
        assert_eq!(
            canonical[id as usize], lit5_ids[0],
            "duplicate LitU32(5) at id {id} must share canonical with id {}",
            lit5_ids[0]
        );
    }
    // LitU32(7) is its own canonical and distinct from LitU32(5).
    let lit7 = lit7_ids[0];
    assert_eq!(canonical[lit7 as usize], lit7);
    assert_ne!(canonical[lit7 as usize], lit5_ids[0]);
}

#[test]
fn cuda_cse_finds_canonicals_for_equal_binops() {
    let backend = live_backend();
    let dispatcher = CudaOptimizerDispatcher::new(&backend);

    // Two structurally identical binops:
    //   let a = 1 + 2
    //   let b = 1 + 2     // same shape, same literals
    //   let c = 3 + 4     // different
    //   store buf 0 0
    let p = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("b", Expr::add(Expr::u32(1), Expr::u32(2))),
            Node::let_bind("c", Expr::add(Expr::u32(3), Expr::u32(4))),
            Node::store("buf", Expr::u32(0), Expr::u32(0)),
        ],
    );

    let (arena, canonical) =
        gpu_cse_canonicals(&p, &dispatcher).expect("gpu_cse_canonicals must succeed");

    // Identify the BIN_OP entries with LitU32(1)+LitU32(2) children
    // vs LitU32(3)+LitU32(4) children.
    let bin_op_kind = vyre_self_substrate::optimizer::expr_arena::expr_kind::BIN_OP;
    let lit_kind = vyre_self_substrate::optimizer::expr_arena::expr_kind::LIT_U32;
    let mut bin_one_two: Vec<u32> = Vec::new();
    let mut bin_three_four: Vec<u32> = Vec::new();
    for (i, &kind) in arena.kinds.iter().enumerate() {
        if kind != bin_op_kind {
            continue;
        }
        let l = arena.arg1[i] as usize;
        let r = arena.arg2[i] as usize;
        if l >= arena.kinds.len() || r >= arena.kinds.len() {
            continue;
        }
        if arena.kinds[l] != lit_kind || arena.kinds[r] != lit_kind {
            continue;
        }
        match (arena.arg0[l], arena.arg0[r]) {
            (1, 2) => bin_one_two.push(i as u32),
            (3, 4) => bin_three_four.push(i as u32),
            _ => {}
        }
    }
    assert!(
        bin_one_two.len() >= 2,
        "Program must encode at least two `1+2` BinOps; got {bin_one_two:?}"
    );
    assert!(!bin_three_four.is_empty());

    // Both `1+2` BinOps share a canonical (the smaller id).
    let first = bin_one_two[0];
    assert_eq!(canonical[first as usize], first);
    for &id in &bin_one_two[1..] {
        assert_eq!(
            canonical[id as usize], first,
            "duplicate `1+2` BinOp at id {id} must share canonical with id {first}"
        );
    }
    // `3+4` is distinct.
    let other = bin_three_four[0];
    assert_eq!(canonical[other as usize], other);
    assert_ne!(canonical[other as usize], first);
}
