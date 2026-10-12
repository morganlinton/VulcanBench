//! Generated payload-invariance coverage for frozen `DataType` builtin tags.
//!
//! Payload-bearing variants carry sizes, shapes, extension ids, quantization
//! sidecars, and nested element types. Their builtin wire tags must identify
//! the variant family only; changing a payload must never allocate a new core
//! tag or enter the high-bit extension-id space.

use std::collections::{BTreeMap, BTreeSet};

use vyre_spec::extension::ExtensionDataTypeId;
use vyre_spec::{DataType, QuantizationScale, QuantizationZeroPoint, TypeId};

#[test]
fn generated_payload_variants_keep_family_tags_for_24576_cases() {
    let mut expected_by_family = BTreeMap::<&'static str, u8>::new();
    let mut assigned_tags = BTreeSet::<u8>::new();
    let mut checked = 0usize;
    let mut opaque_checked = 0usize;

    for seed in 0u32..24_576 {
        for (family, ty) in generated_payload_cases(seed) {
            match ty.builtin_wire_tag() {
                Some(tag) => {
                    assert!(
                        (0x01..=0x7F).contains(&tag),
                        "Fix: DataType::{family} builtin tag {tag:#04x} must stay below extension-id space."
                    );
                    match expected_by_family.insert(family, tag) {
                        Some(previous) => assert_eq!(
                            previous, tag,
                            "Fix: DataType::{family} builtin tag changed across payload variants at seed {seed}."
                        ),
                        None => {
                            assigned_tags.insert(tag);
                        }
                    }
                    checked += 1;
                }
                None => {
                    assert_eq!(
                        family, "Opaque",
                        "Fix: only DataType::Opaque may omit a builtin wire tag; got {family} at seed {seed}."
                    );
                    opaque_checked += 1;
                }
            }
        }
    }

    assert_eq!(
        expected_by_family.get("Array"),
        Some(&0x08),
        "Fix: DataType::Array tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("Handle"),
        Some(&0x13),
        "Fix: DataType::Handle tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("Vec"),
        Some(&0x14),
        "Fix: DataType::Vec tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("TensorShaped"),
        Some(&0x15),
        "Fix: DataType::TensorShaped tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("SparseBsr"),
        Some(&0x18),
        "Fix: DataType::SparseBsr tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("DeviceMesh"),
        Some(&0x1E),
        "Fix: DataType::DeviceMesh tag drifted from the frozen wire contract."
    );
    assert_eq!(
        expected_by_family.get("Quantized"),
        Some(&0x1F),
        "Fix: DataType::Quantized tag drifted from the frozen wire contract."
    );
    assert_eq!(
        checked,
        24_576 * 9,
        "Fix: generated DataType payload-invariance matrix must keep every builtin case active."
    );
    assert_eq!(
        opaque_checked, 24_576,
        "Fix: generated DataType payload-invariance matrix must keep opaque extension cases active."
    );
    assert_eq!(
        assigned_tags.len(),
        expected_by_family.len(),
        "Fix: generated payload-bearing DataType families must not collide on builtin tags."
    );
}

fn generated_payload_cases(seed: u32) -> [(&'static str, DataType); 10] {
    let leaf = generated_leaf(seed);
    [
        (
            "Opaque",
            DataType::Opaque(ExtensionDataTypeId::from_name(&format!(
                "test.dtype.payload.{}",
                mix32(seed ^ 0xABCD_EF01)
            ))),
        ),
        (
            "Array",
            DataType::Array {
                element_size: generated_nonzero_usize(seed),
            },
        ),
        ("Handle", DataType::Handle(TypeId(mix32(seed)))),
        (
            "Vec",
            DataType::Vec {
                element: Box::new(leaf.clone()),
                count: generated_count(seed),
            },
        ),
        (
            "TensorShaped",
            DataType::TensorShaped {
                element: Box::new(leaf.clone()),
                shape: generated_shape(seed).as_slice().into(),
            },
        ),
        (
            "SparseCsr",
            DataType::SparseCsr {
                element: Box::new(leaf.clone()),
            },
        ),
        (
            "SparseCoo",
            DataType::SparseCoo {
                element: Box::new(leaf.clone()),
            },
        ),
        (
            "SparseBsr",
            DataType::SparseBsr {
                element: Box::new(leaf),
                block_rows: generated_block_dim(seed ^ 0xA5A5_5A5A),
                block_cols: generated_block_dim(seed ^ 0x5A5A_A5A5),
            },
        ),
        (
            "DeviceMesh",
            DataType::DeviceMesh {
                axes: generated_shape(seed.rotate_left(7)).as_slice().into(),
            },
        ),
        (
            "Quantized",
            DataType::Quantized {
                storage: Box::new(generated_quantized_storage(seed)),
                scale: generated_scale(seed),
                zero_point: generated_zero_point(seed),
            },
        ),
    ]
}

fn generated_leaf(seed: u32) -> DataType {
    match mix32(seed) % 12 {
        0 => DataType::U8,
        1 => DataType::U16,
        2 => DataType::U32,
        3 => DataType::U64,
        4 => DataType::I8,
        5 => DataType::I16,
        6 => DataType::I32,
        7 => DataType::I64,
        8 => DataType::Bool,
        9 => DataType::F32,
        10 => DataType::I4,
        _ => DataType::Opaque(ExtensionDataTypeId::from_name(&format!(
            "test.dtype.{}",
            mix32(seed)
        ))),
    }
}

fn generated_quantized_storage(seed: u32) -> DataType {
    match mix32(seed ^ 0x5151_2424) % 9 {
        0 => DataType::I4,
        1 => DataType::I8,
        2 => DataType::I16,
        3 => DataType::U8,
        4 => DataType::U16,
        5 => DataType::F8E4M3,
        6 => DataType::F8E5M2,
        7 => DataType::FP4,
        _ => DataType::NF4,
    }
}

fn generated_scale(seed: u32) -> QuantizationScale {
    match mix32(seed ^ 0xC001_CAFE) % 3 {
        0 => QuantizationScale::PerTensor,
        1 => QuantizationScale::PerChannel {
            axis: mix32(seed) % 8,
        },
        _ => QuantizationScale::PerGroup {
            group_size: generated_block_dim(seed),
        },
    }
}

fn generated_zero_point(seed: u32) -> QuantizationZeroPoint {
    match mix32(seed ^ 0xFACE_FEED) % 4 {
        0 => QuantizationZeroPoint::Absent,
        1 => QuantizationZeroPoint::PerTensor,
        2 => QuantizationZeroPoint::PerChannel {
            axis: mix32(seed) % 8,
        },
        _ => QuantizationZeroPoint::PerGroup {
            group_size: generated_block_dim(seed),
        },
    }
}

fn generated_shape(seed: u32) -> Vec<u32> {
    let len = (mix32(seed) % 5) as usize;
    (0..len)
        .map(|axis| (mix32(seed.wrapping_add(axis as u32)) % 1024) + 1)
        .collect()
}

fn generated_count(seed: u32) -> u8 {
    ((mix32(seed ^ 0xDEAD_BEEF) % 16) + 1) as u8
}

fn generated_block_dim(seed: u32) -> u32 {
    (mix32(seed ^ 0x9E37_79B9) % 512) + 1
}

fn generated_nonzero_usize(seed: u32) -> usize {
    match mix32(seed) % 8 {
        0 => 1,
        1 => 2,
        2 => 3,
        3 => 4,
        4 => 8,
        5 => 16,
        6 => 255,
        _ => usize::MAX,
    }
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}
