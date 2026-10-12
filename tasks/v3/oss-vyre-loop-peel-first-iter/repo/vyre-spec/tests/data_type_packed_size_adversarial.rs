//! Adversarial packed-size contracts for recursive fixed-width data types.
//!
//! `packed_size_bytes` is a release-path sizing helper for CUDA buffers and
//! wire codecs. It must distinguish three cases precisely: fixed size,
//! genuinely variable size, and arithmetic overflow. Treating overflow as
//! `None` would make a malformed fixed-width contract look like a legal
//! variable-width contract.

use vyre_spec::{DataType, QuantizationScale, QuantizationZeroPoint};

#[test]
fn nested_fixed_width_vectors_report_overflow_instead_of_variable_width() {
    let ty = nested_vec(DataType::U64, 16, u8::MAX);

    assert_eq!(
        ty.bit_width(),
        None,
        "Fix: public bit_width sentinel should remain conservative after recursive overflow."
    );
    let err = ty
        .packed_size_bytes(1)
        .expect_err("Fix: recursive fixed-width overflow must be reported as an error.");
    assert!(
        err.contains("overflowed nested bit width"),
        "Fix: nested vector overflow diagnostic must point at bit-width arithmetic: {err}"
    );
}

#[test]
fn nested_fixed_byte_arrays_report_overflow_instead_of_variable_width() {
    let ty = DataType::Vec {
        element: Box::new(DataType::Array {
            element_size: usize::MAX,
        }),
        count: 2,
    };

    assert_eq!(
        ty.size_bytes(),
        None,
        "Fix: public size_bytes sentinel should remain conservative after recursive overflow."
    );
    let err = ty
        .packed_size_bytes(1)
        .expect_err("Fix: recursive fixed byte-width overflow must be reported as an error.");
    assert!(
        err.contains("overflowed nested byte width"),
        "Fix: nested array overflow diagnostic must point at byte-width arithmetic: {err}"
    );
}

#[test]
fn generated_packed_size_matrix_matches_checked_oracle_for_12288_cases() {
    let mut fixed = 0usize;
    let mut variable = 0usize;
    let mut overflow = 0usize;

    for seed in 0u64..12_288 {
        let ty = generated_type(seed);
        let elements = generated_element_count(seed);
        let actual = ty.packed_size_bytes(elements);
        let expected = oracle_packed_size(&ty, elements);

        match (&actual, &expected) {
            (Ok(Some(_)), Ok(Some(_))) => fixed += 1,
            (Ok(None), Ok(None)) => variable += 1,
            (Err(_), Err(_)) => overflow += 1,
            _ => {}
        }

        assert_eq!(
            actual.map_err(|err| err.contains("overflowed")),
            expected.map_err(|err| err.contains("overflowed")),
            "Fix: generated packed-size case {seed} drifted for {ty} with {elements} logical elements."
        );
    }

    assert!(
        fixed > 6_000,
        "Fix: packed-size matrix must retain thousands of fixed-width cases; got {fixed}."
    );
    assert!(
        variable > 1_000,
        "Fix: packed-size matrix must retain variable-width sentinels; got {variable}."
    );
    assert!(
        overflow > 1_000,
        "Fix: packed-size matrix must retain overflow adversarial cases; got {overflow}."
    );
}

fn generated_type(seed: u64) -> DataType {
    match seed % 12 {
        0 => DataType::I4,
        1 => DataType::FP4,
        2 => DataType::NF4,
        3 => DataType::U8,
        4 => DataType::U32,
        5 => DataType::Vec4U32,
        6 => nested_vec(
            DataType::I4,
            ((seed >> 8) % 4) as usize + 1,
            generated_count(seed),
        ),
        7 => DataType::Quantized {
            storage: Box::new(match (seed >> 12) % 5 {
                0 => DataType::I4,
                1 => DataType::FP4,
                2 => DataType::NF4,
                3 => DataType::U8,
                _ => DataType::I8,
            }),
            scale: QuantizationScale::PerGroup {
                group_size: ((seed >> 16) as u32 % 512) + 1,
            },
            zero_point: QuantizationZeroPoint::Absent,
        },
        8 => DataType::Tensor,
        9 => DataType::SparseCsr {
            element: Box::new(DataType::F32),
        },
        10 => nested_vec(DataType::U64, 16, u8::MAX),
        _ => DataType::Vec {
            element: Box::new(DataType::Array {
                element_size: usize::MAX,
            }),
            count: 2,
        },
    }
}

fn generated_element_count(seed: u64) -> usize {
    match (seed >> 20) % 8 {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 7,
        5 => 255,
        6 => 4096,
        _ => usize::MAX,
    }
}

fn generated_count(seed: u64) -> u8 {
    ((seed >> 28) as u8 % 16) + 1
}

fn nested_vec(mut ty: DataType, depth: usize, count: u8) -> DataType {
    for _ in 0..depth {
        ty = DataType::Vec {
            element: Box::new(ty),
            count,
        };
    }
    ty
}

fn oracle_packed_size(ty: &DataType, elements: usize) -> Result<Option<usize>, String> {
    if let Some(bits) = oracle_bit_width(ty)? {
        let total_bits = bits
            .checked_mul(elements)
            .ok_or_else(|| "overflowed bits".to_owned())?;
        return total_bits
            .checked_add(7)
            .map(|rounded| Some(rounded / 8))
            .ok_or_else(|| "overflowed rounding".to_owned());
    }
    if let Some(bytes) = oracle_size_bytes(ty)? {
        return bytes
            .checked_mul(elements)
            .map(Some)
            .ok_or_else(|| "overflowed bytes".to_owned());
    }
    Ok(None)
}

fn oracle_bit_width(ty: &DataType) -> Result<Option<usize>, String> {
    match ty {
        DataType::I4 | DataType::FP4 | DataType::NF4 => Ok(Some(4)),
        DataType::F8E4M3 | DataType::F8E5M2 | DataType::U8 | DataType::I8 => Ok(Some(8)),
        DataType::U16 | DataType::I16 | DataType::F16 | DataType::BF16 => Ok(Some(16)),
        DataType::Bool | DataType::U32 | DataType::I32 | DataType::F32 | DataType::Handle(_) => {
            Ok(Some(32))
        }
        DataType::I64 | DataType::U64 | DataType::F64 | DataType::Vec2U32 => Ok(Some(64)),
        DataType::Vec4U32 => Ok(Some(128)),
        DataType::DeviceMesh { .. } => Ok(Some(32)),
        DataType::Bytes => Ok(Some(8)),
        DataType::Quantized { storage, .. } => oracle_bit_width(storage),
        DataType::Vec { element, count } => {
            let Some(bits) = oracle_bit_width(element)? else {
                return Ok(None);
            };
            bits.checked_mul(*count as usize)
                .map(Some)
                .ok_or_else(|| "overflowed nested bits".to_owned())
        }
        DataType::Array { .. }
        | DataType::Tensor
        | DataType::TensorShaped { .. }
        | DataType::SparseCsr { .. }
        | DataType::SparseCoo { .. }
        | DataType::SparseBsr { .. }
        | DataType::Opaque(_) => Ok(None),
        _ => Ok(None),
    }
}

fn oracle_size_bytes(ty: &DataType) -> Result<Option<usize>, String> {
    match ty {
        DataType::U8 | DataType::I8 => Ok(Some(1)),
        DataType::U16 | DataType::I16 | DataType::F16 | DataType::BF16 => Ok(Some(2)),
        DataType::Bool | DataType::U32 | DataType::I32 | DataType::F32 => Ok(Some(4)),
        DataType::I64 | DataType::U64 | DataType::Vec2U32 | DataType::F64 => Ok(Some(8)),
        DataType::Vec4U32 => Ok(Some(16)),
        DataType::Handle(_) => Ok(Some(4)),
        DataType::Bytes => Ok(Some(1)),
        DataType::Array { element_size } => Ok(Some(*element_size)),
        DataType::Vec { element, count } => {
            let Some(bytes) = oracle_size_bytes(element)? else {
                return Ok(None);
            };
            bytes
                .checked_mul(*count as usize)
                .map(Some)
                .ok_or_else(|| "overflowed nested bytes".to_owned())
        }
        DataType::Tensor | DataType::TensorShaped { .. } => Ok(None),
        DataType::F8E4M3 | DataType::F8E5M2 => Ok(Some(1)),
        DataType::I4 | DataType::FP4 | DataType::NF4 => Ok(Some(1)),
        DataType::SparseCsr { .. } | DataType::SparseCoo { .. } | DataType::SparseBsr { .. } => {
            Ok(None)
        }
        DataType::DeviceMesh { .. } => Ok(Some(4)),
        DataType::Quantized { storage, .. } => oracle_size_bytes(storage),
        DataType::Opaque(_) => Ok(None),
        _ => Ok(None),
    }
}
