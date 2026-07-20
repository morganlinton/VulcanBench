//! Test: memory vector.
use super::*;

fn dynamic_reassociated_vector_load_kernel(seed: u32) -> KernelDescriptor {
    let stride = seed.wrapping_mul(13).wrapping_add(1) << 2;
    two_slot_u32_kernel(
        "dynamic_reassociated_vec_load",
        vec![
            KernelOp {
                kind: KernelOpKind::LocalInvocationId,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![0, 1],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![3],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 2],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 3],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 7],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 4],
                result: Some(9),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 9],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 5],
                result: Some(11),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 11],
                result: Some(12),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![4],
                result: Some(13),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 13, 12],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(stride),
            LiteralValue::U32(1),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
            LiteralValue::U32(0),
        ],
    )
}

fn dynamic_reassociated_vector_store_kernel(seed: u32) -> KernelDescriptor {
    let stride = seed.wrapping_mul(17).wrapping_add(2) << 2;
    let value_base = 0x1000_0000_u32.wrapping_add(seed.rotate_left(seed % 31));
    two_slot_u32_kernel(
        "dynamic_reassociated_vec_store",
        vec![
            KernelOp {
                kind: KernelOpKind::LocalInvocationId,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![0, 1],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![3],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![4],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 6],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![5],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 8],
                result: Some(9),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![6],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 10],
                result: Some(11),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![7],
                result: Some(12),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![0, 12],
                result: Some(13),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 2, 7],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 3],
                result: Some(14),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 14, 9],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 4],
                result: Some(15),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 15, 11],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![2, 5],
                result: Some(16),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 16, 13],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(stride),
            LiteralValue::U32(1),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
            LiteralValue::U32(value_base),
            LiteralValue::U32(value_base.wrapping_add(1)),
            LiteralValue::U32(value_base.wrapping_add(2)),
            LiteralValue::U32(value_base.wrapping_add(3)),
        ],
    )
}

fn dynamic_misaligned_gather_to_vector_store_kernel() -> KernelDescriptor {
    two_slot_u32_kernel(
        "dynamic_misaligned_gather_to_vec_store",
        vec![
            KernelOp {
                kind: KernelOpKind::LocalInvocationId,
                operands: vec![0],
                result: Some(0),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(1),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![1],
                result: Some(2),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![0, 1],
                result: Some(3),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![2],
                result: Some(4),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Mul),
                operands: vec![0, 4],
                result: Some(5),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 3],
                result: Some(6),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![3, 2],
                result: Some(7),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 7],
                result: Some(8),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![3],
                result: Some(9),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![3, 9],
                result: Some(10),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 10],
                result: Some(11),
            },
            KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![4],
                result: Some(12),
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![3, 12],
                result: Some(13),
            },
            KernelOp {
                kind: KernelOpKind::LoadGlobal,
                operands: vec![0, 13],
                result: Some(14),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 5, 6],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 2],
                result: Some(15),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 15, 8],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 9],
                result: Some(16),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 16, 11],
                result: None,
            },
            KernelOp {
                kind: KernelOpKind::BinOpKind(BinOp::Add),
                operands: vec![5, 12],
                result: Some(17),
            },
            KernelOp {
                kind: KernelOpKind::StoreGlobal,
                operands: vec![1, 17, 14],
                result: None,
            },
        ],
        vec![
            LiteralValue::U32(5),
            LiteralValue::U32(1),
            LiteralValue::U32(4),
            LiteralValue::U32(2),
            LiteralValue::U32(3),
        ],
    )
}

#[path = "memory_vector/load_vector_contracts.rs"]
mod load_vector_contracts;
#[path = "memory_vector/load_cache_hoist_contracts.rs"]
mod load_cache_hoist_contracts;
#[path = "memory_vector/store_vector_contracts.rs"]
mod store_vector_contracts;
#[path = "memory_vector/store_pruning_contracts.rs"]
mod store_pruning_contracts;
