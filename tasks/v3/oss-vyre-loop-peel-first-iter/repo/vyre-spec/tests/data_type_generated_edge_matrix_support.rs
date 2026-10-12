//! Data type generated edge matrix support test suite.

use vyre_spec::extension::ExtensionDataTypeId;
use vyre_spec::{DataType, QuantizationScale, QuantizationZeroPoint, TypeId};

/// Assert the size/bit-width/tag layout invariants hold for one generated type.
pub(crate) fn assert_layout_invariants(idx: usize, ty: &DataType) {
    let display = ty.to_string();
    assert!(
        !display.is_empty(),
        "Fix: generated data type #{idx} must have non-empty Display."
    );
    assert!(
        !display.contains("weir")
            && !display.contains("surgec")
            && !display.contains("gossan")
            && !display.contains("keyhog"),
        "Fix: platform spec Display leaked a consumer name for generated type #{idx}: {display}."
    );

    if let Some(max_bytes) = ty.max_bytes() {
        assert!(
            ty.min_bytes() <= max_bytes,
            "Fix: min_bytes exceeded max_bytes for generated type #{idx} ({display}): min={} max={max_bytes}.",
            ty.min_bytes()
        );
    }

    if let Some(size_bytes) = ty.size_bytes() {
        assert!(
            ty.min_bytes() <= size_bytes || ty.min_bytes() == 0,
            "Fix: fixed size must cover the minimum layout for generated type #{idx} ({display})."
        );
    }

    if let (Some(bit_width), Some(size_bytes)) = (ty.bit_width(), ty.size_bytes()) {
        assert!(
            size_bytes.saturating_mul(8) >= bit_width,
            "Fix: byte size cannot hold bit width for generated type #{idx} ({display}): size={size_bytes}, bits={bit_width}."
        );
    }

    if matches!(ty, DataType::Opaque(_)) {
        assert_eq!(
            ty.builtin_wire_tag(),
            None,
            "Fix: generated opaque type #{idx} must remain outside the builtin tag space."
        );
    } else if let Some(tag) = ty.builtin_wire_tag() {
        assert!(
            (0x01..=0x1F).contains(&tag),
            "Fix: generated builtin type #{idx} ({display}) has invalid tag {tag:#04x}."
        );
    }
}

/// Assert a generated type survives a JSON serialize/deserialize round trip.
pub(crate) fn assert_serde_round_trip(idx: usize, ty: &DataType) {
    let json = serde_json::to_string(ty)
        .unwrap_or_else(|err| panic!("Fix: generated data type #{idx} failed to serialize: {err}"));
    let decoded: DataType = serde_json::from_str(&json).unwrap_or_else(|err| {
        panic!("Fix: generated data type #{idx} failed to deserialize from `{json}`: {err}")
    });

    assert_eq!(
        decoded, *ty,
        "Fix: generated data type #{idx} must survive serde round trip."
    );
}

/// Build the full matrix of generated edge-case data types (leaves, vectors,
/// tensors, sparse, quantized, and one level of nesting).
pub(crate) fn generated_edge_types() -> Vec<DataType> {
    let leaves = leaf_edge_types();
    let mut cases = leaves.clone();

    for leaf in &leaves {
        for count in [0u8, 1, 2, 3, 8, 16, 64, u8::MAX] {
            cases.push(DataType::Vec {
                element: Box::new(leaf.clone()),
                count,
            });
        }

        for shape in [
            Vec::new(),
            vec![0],
            vec![1],
            vec![2, 3],
            vec![1, 1, 1, 1],
            vec![u32::MAX],
        ] {
            cases.push(DataType::TensorShaped {
                element: Box::new(leaf.clone()),
                shape: shape.as_slice().into(),
            });
        }

        cases.push(DataType::SparseCsr {
            element: Box::new(leaf.clone()),
        });
        cases.push(DataType::SparseCoo {
            element: Box::new(leaf.clone()),
        });

        for block_rows in [0u32, 1, 2, 8, u32::MAX] {
            for block_cols in [0u32, 1, 3, u32::MAX] {
                cases.push(DataType::SparseBsr {
                    element: Box::new(leaf.clone()),
                    block_rows,
                    block_cols,
                });
            }
        }
    }

    for storage in quantized_storage_types() {
        for scale in quantization_scales() {
            for zero_point in quantization_zero_points() {
                cases.push(DataType::Quantized {
                    storage: Box::new(storage.clone()),
                    scale: scale.clone(),
                    zero_point: zero_point.clone(),
                });
            }
        }
    }

    let first_order = cases.clone();
    for ty in first_order {
        cases.push(DataType::Vec {
            element: Box::new(ty.clone()),
            count: 2,
        });
        cases.push(DataType::SparseCsr {
            element: Box::new(ty),
        });
    }

    cases
}

/// The leaf (non-composite) data types used as building blocks for the matrix.
pub(crate) fn leaf_edge_types() -> Vec<DataType> {
    vec![
        DataType::U8,
        DataType::U16,
        DataType::U32,
        DataType::U64,
        DataType::I8,
        DataType::I16,
        DataType::I32,
        DataType::I64,
        DataType::Bool,
        DataType::F16,
        DataType::BF16,
        DataType::F32,
        DataType::F64,
        DataType::F8E4M3,
        DataType::F8E5M2,
        DataType::I4,
        DataType::FP4,
        DataType::NF4,
        DataType::Vec2U32,
        DataType::Vec4U32,
        DataType::Bytes,
        DataType::Tensor,
        DataType::Array { element_size: 0 },
        DataType::Array { element_size: 1 },
        DataType::Array {
            element_size: usize::MAX / 8,
        },
        DataType::Handle(TypeId(0)),
        DataType::Handle(TypeId(u32::MAX)),
        DataType::DeviceMesh {
            axes: Vec::<u32>::new().as_slice().into(),
        },
        DataType::DeviceMesh {
            axes: [1, 2, 4].as_slice().into(),
        },
        DataType::DeviceMesh {
            axes: [u32::MAX].as_slice().into(),
        },
        DataType::Opaque(ExtensionDataTypeId::from_name("edge_matrix.dtype")),
    ]
}

/// Storage element types that are valid backings for a quantized type.
pub(crate) fn quantized_storage_types() -> Vec<DataType> {
    vec![
        DataType::I4,
        DataType::I8,
        DataType::I16,
        DataType::U8,
        DataType::U16,
        DataType::F8E4M3,
        DataType::F8E5M2,
        DataType::FP4,
        DataType::NF4,
    ]
}

/// Representative quantization scale modes (per-tensor, per-channel, per-group).
pub(crate) fn quantization_scales() -> Vec<QuantizationScale> {
    vec![
        QuantizationScale::PerTensor,
        QuantizationScale::PerChannel { axis: 0 },
        QuantizationScale::PerChannel { axis: 1 },
        QuantizationScale::PerChannel { axis: 31 },
        QuantizationScale::PerChannel { axis: u32::MAX },
        QuantizationScale::PerGroup { group_size: 1 },
        QuantizationScale::PerGroup { group_size: 2 },
        QuantizationScale::PerGroup { group_size: 128 },
        QuantizationScale::PerGroup {
            group_size: u32::MAX,
        },
    ]
}

/// Representative quantization zero-point modes (absent, per-tensor/channel/group).
pub(crate) fn quantization_zero_points() -> Vec<QuantizationZeroPoint> {
    vec![
        QuantizationZeroPoint::Absent,
        QuantizationZeroPoint::PerTensor,
        QuantizationZeroPoint::PerChannel { axis: 0 },
        QuantizationZeroPoint::PerChannel { axis: 1 },
        QuantizationZeroPoint::PerChannel { axis: 31 },
        QuantizationZeroPoint::PerChannel { axis: u32::MAX },
        QuantizationZeroPoint::PerGroup { group_size: 1 },
        QuantizationZeroPoint::PerGroup { group_size: 2 },
        QuantizationZeroPoint::PerGroup { group_size: 128 },
        QuantizationZeroPoint::PerGroup {
            group_size: u32::MAX,
        },
    ]
}

/// One representative type per builtin wire tag, to exercise tag coverage.
pub(crate) fn builtin_wire_tag_representatives() -> Vec<DataType> {
    vec![
        DataType::U32,
        DataType::I32,
        DataType::U64,
        DataType::Vec2U32,
        DataType::Vec4U32,
        DataType::Bool,
        DataType::Bytes,
        DataType::Array { element_size: 4 },
        DataType::F16,
        DataType::BF16,
        DataType::F32,
        DataType::F64,
        DataType::Tensor,
        DataType::U8,
        DataType::U16,
        DataType::I8,
        DataType::I16,
        DataType::I64,
        DataType::Handle(TypeId(7)),
        DataType::Vec {
            element: Box::new(DataType::U32),
            count: 4,
        },
        DataType::TensorShaped {
            element: Box::new(DataType::F32),
            shape: [2, 3].as_slice().into(),
        },
        DataType::SparseCsr {
            element: Box::new(DataType::F32),
        },
        DataType::SparseCoo {
            element: Box::new(DataType::F32),
        },
        DataType::SparseBsr {
            element: Box::new(DataType::F32),
            block_rows: 2,
            block_cols: 4,
        },
        DataType::F8E4M3,
        DataType::F8E5M2,
        DataType::I4,
        DataType::FP4,
        DataType::NF4,
        DataType::DeviceMesh {
            axes: [2, 4].as_slice().into(),
        },
        DataType::Quantized {
            storage: Box::new(DataType::I4),
            scale: QuantizationScale::PerTensor,
            zero_point: QuantizationZeroPoint::Absent,
        },
    ]
}

/// Deterministically derive a unique extension name for the given index.
pub(crate) fn generated_extension_name(idx: u32) -> String {
    format!(
        "generated.extension.family.{:04x}.{:08x}",
        idx,
        idx.wrapping_mul(0x045D_9F3B).rotate_left(idx % 17)
    )
}
