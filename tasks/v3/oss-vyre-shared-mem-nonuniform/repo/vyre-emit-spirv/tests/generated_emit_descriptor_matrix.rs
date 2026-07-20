//! Generated descriptor matrix for SPIR-V emission invariants.
//!
//! The adversarial corpus covers hostile shapes. This test covers generated
//! ordinary kernels with varied dispatch geometry and arithmetic chain depth,
//! pinning the raw/optimized word and byte emission contracts.

use vyre_foundation::ir::{BinOp, DataType};
use vyre_lower::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass,
};

fn rw_slot(name: &str) -> BindingSlot {
    BindingSlot {
        slot: 0,
        element_type: DataType::U32,
        element_count: None,
        memory_class: MemoryClass::Global,
        visibility: BindingVisibility::ReadWrite,
        name: name.into(),
    }
}

fn generated_descriptor(seed: u32) -> KernelDescriptor {
    let chain_len = 1 + (seed as usize % 12);
    let mut literals = vec![LiteralValue::U32(0)];
    let mut ops = vec![KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![0],
        result: Some(0),
    }];
    let mut accumulator = 0u32;

    for idx in 0..chain_len {
        let literal_idx = literals.len() as u32;
        let literal_value = seed
            .wrapping_mul(0x9e37_79b9)
            .rotate_left((idx as u32) & 31)
            .wrapping_add(idx as u32);
        literals.push(LiteralValue::U32(literal_value));

        let literal_result = ops.len() as u32;
        ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![literal_idx],
            result: Some(literal_result),
        });

        let binop_result = ops.len() as u32;
        let op = match idx % 4 {
            0 => BinOp::Add,
            1 => BinOp::BitXor,
            2 => BinOp::BitOr,
            _ => BinOp::BitAnd,
        };
        ops.push(KernelOp {
            kind: KernelOpKind::BinOpKind(op),
            operands: vec![accumulator, literal_result],
            result: Some(binop_result),
        });
        accumulator = binop_result;
    }

    ops.push(KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![0, 0, accumulator],
        result: None,
    });

    KernelDescriptor {
        id: format!("generated_spirv_{seed:08x}"),
        bindings: BindingLayout {
            slots: vec![rw_slot("out")],
        },
        dispatch: Dispatch::new(
            1 + (seed & 255),
            1 + ((seed >> 8) & 7),
            1 + ((seed >> 16) & 3),
        ),
        body: KernelBody {
            ops,
            child_bodies: vec![],
            literals,
        },
    }
}

fn words_from_le_bytes(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("exact 4-byte chunk")))
        .collect()
}

#[test]
fn generated_descriptors_emit_valid_raw_and_optimized_spirv() {
    for seed in 0..256u32 {
        let desc = generated_descriptor(seed.wrapping_mul(0x45d9_f3b));
        let raw = vyre_emit_spirv::emit(&desc)
            .unwrap_or_else(|err| panic!("raw SPIR-V emit failed for {}: {err:?}", desc.id));
        let optimized = vyre_emit_spirv::emit_optimized(&desc)
            .unwrap_or_else(|err| panic!("optimized SPIR-V emit failed for {}: {err:?}", desc.id));

        assert_eq!(raw[0], vyre_emit_spirv::SPIRV_MAGIC, "{}", desc.id);
        assert_eq!(
            optimized[0],
            vyre_emit_spirv::SPIRV_MAGIC,
            "{} optimized magic",
            desc.id
        );
        assert!(raw.len() > 16, "{} raw kernel too small", desc.id);
        assert!(
            optimized.len() > 16,
            "{} optimized kernel too small",
            desc.id
        );
    }
}

#[test]
fn generated_descriptors_bytes_match_word_emission() {
    for seed in 0..128u32 {
        let desc = generated_descriptor(seed ^ 0xa501_7b1d);
        let words = vyre_emit_spirv::emit_optimized(&desc)
            .unwrap_or_else(|err| panic!("optimized SPIR-V emit failed for {}: {err:?}", desc.id));
        let bytes = vyre_emit_spirv::emit_optimized_bytes(&desc)
            .unwrap_or_else(|err| panic!("optimized byte emit failed for {}: {err:?}", desc.id));

        assert_eq!(bytes.len(), words.len() * 4, "{}", desc.id);
        assert_eq!(words_from_le_bytes(&bytes), words, "{}", desc.id);
    }
}

#[test]
fn generated_descriptors_return_optimization_stats() {
    for seed in 0..128u32 {
        let desc = generated_descriptor(seed.rotate_left(7));
        let (bytes, stats) = vyre_emit_spirv::emit_optimized_bytes_with_stats(&desc)
            .unwrap_or_else(|err| panic!("stats byte emit failed for {}: {err:?}", desc.id));
        assert!(bytes.len() >= 4, "{}", desc.id);
        assert_eq!(
            u32::from_le_bytes(bytes[0..4].try_into().expect("SPIR-V header word")),
            vyre_emit_spirv::SPIRV_MAGIC,
            "{}",
            desc.id
        );
        assert!(
            stats.iterations >= 1,
            "{} stats must record at least one pass",
            desc.id
        );
    }
}
