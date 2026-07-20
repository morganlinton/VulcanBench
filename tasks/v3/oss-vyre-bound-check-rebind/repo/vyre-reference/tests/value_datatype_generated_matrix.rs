//! Generated datatype-width matrix for the reference value oracle.
//!
//! The interpreter's `Value` boundary is where typed IR buffer elements become
//! host bytes. This matrix pins fixed-width scalar, vector, shaped, and
//! quantized storage so out-of-bounds loads and backend parity comparison never
//! silently collapse typed values into empty byte payloads.

use smallvec::smallvec;
use vyre::ir::DataType;
use vyre_reference::value::Value;
use vyre_spec::{QuantizationScale, QuantizationZeroPoint};

#[derive(Clone)]
struct WidthCase {
    name: &'static str,
    ty: DataType,
    width: usize,
}

fn fixed_width_cases() -> Vec<WidthCase> {
    vec![
        WidthCase {
            name: "u8",
            ty: DataType::U8,
            width: 1,
        },
        WidthCase {
            name: "i8",
            ty: DataType::I8,
            width: 1,
        },
        WidthCase {
            name: "u16",
            ty: DataType::U16,
            width: 2,
        },
        WidthCase {
            name: "i16",
            ty: DataType::I16,
            width: 2,
        },
        WidthCase {
            name: "f16",
            ty: DataType::F16,
            width: 2,
        },
        WidthCase {
            name: "bf16",
            ty: DataType::BF16,
            width: 2,
        },
        WidthCase {
            name: "i64",
            ty: DataType::I64,
            width: 8,
        },
        WidthCase {
            name: "f8e4m3",
            ty: DataType::F8E4M3,
            width: 1,
        },
        WidthCase {
            name: "f8e5m2",
            ty: DataType::F8E5M2,
            width: 1,
        },
        WidthCase {
            name: "i4",
            ty: DataType::I4,
            width: 1,
        },
        WidthCase {
            name: "fp4",
            ty: DataType::FP4,
            width: 1,
        },
        WidthCase {
            name: "nf4",
            ty: DataType::NF4,
            width: 1,
        },
        WidthCase {
            name: "vec2u32",
            ty: DataType::Vec2U32,
            width: 8,
        },
        WidthCase {
            name: "vec4u32",
            ty: DataType::Vec4U32,
            width: 16,
        },
        WidthCase {
            name: "array7",
            ty: DataType::Array { element_size: 7 },
            width: 7,
        },
        WidthCase {
            name: "vec_u16x3",
            ty: DataType::Vec {
                element: Box::new(DataType::U16),
                count: 3,
            },
            width: 6,
        },
        WidthCase {
            name: "device_mesh_handle",
            ty: DataType::DeviceMesh {
                axes: smallvec![2, 4],
            },
            width: 4,
        },
        WidthCase {
            name: "quantized_i4",
            ty: DataType::Quantized {
                storage: Box::new(DataType::I4),
                scale: QuantizationScale::PerGroup { group_size: 32 },
                zero_point: QuantizationZeroPoint::Absent,
            },
            width: 1,
        },
        WidthCase {
            name: "tensor_u8_2x3",
            ty: DataType::TensorShaped {
                element: Box::new(DataType::U8),
                shape: smallvec![2, 3],
            },
            width: 6,
        },
        WidthCase {
            name: "tensor_quantized_u8_2x3",
            ty: DataType::TensorShaped {
                element: Box::new(DataType::Quantized {
                    storage: Box::new(DataType::U8),
                    scale: QuantizationScale::PerTensor,
                    zero_point: QuantizationZeroPoint::PerTensor,
                }),
                shape: smallvec![2, 3],
            },
            width: 6,
        },
    ]
}

#[test]
fn generated_fixed_width_datatypes_decode_only_the_declared_storage_window() {
    let mut checked = 0usize;
    for case in fixed_width_cases() {
        let payload = (0..case.width + 3)
            .map(|idx| (idx as u8).wrapping_mul(17).wrapping_add(3))
            .collect::<Vec<_>>();
        let value = Value::from_element_bytes(case.ty.clone(), &payload)
            .unwrap_or_else(|err| panic!("Fix: {} should decode: {err}", case.name));

        assert_eq!(
            value.to_bytes(),
            payload[..case.width],
            "Fix: {} must retain exactly its declared storage window.",
            case.name
        );

        let zero = Value::zero_for(case.ty.clone()).to_bytes();
        assert_eq!(
            zero.len(),
            case.width,
            "Fix: {} typed zero must preserve storage width.",
            case.name
        );
        assert!(
            zero.iter().all(|&byte| byte == 0),
            "Fix: {} typed zero must be byte-zeroed.",
            case.name
        );

        if case.width > 0 {
            let short = vec![0xA5; case.width - 1];
            assert!(
                Value::from_element_bytes(case.ty.clone(), &short).is_err(),
                "Fix: {} should reject short fixed-width payloads.",
                case.name
            );
        }
        checked += 1;
    }

    assert_eq!(checked, 20);
}

#[test]
fn generated_typed_scalar_datatypes_decode_value_semantics() {
    let u32_value = Value::from_element_bytes(DataType::U32, &0xAABB_CCDDu32.to_le_bytes())
        .expect("Fix: u32 scalar should decode.");
    assert_eq!(u32_value, Value::U32(0xAABB_CCDD));

    let i32_value = Value::from_element_bytes(DataType::I32, &(-12345i32).to_le_bytes())
        .expect("Fix: i32 scalar should decode.");
    assert_eq!(i32_value, Value::I32(-12345));

    let u64_value =
        Value::from_element_bytes(DataType::U64, &0x1122_3344_5566_7788u64.to_le_bytes())
            .expect("Fix: u64 scalar should decode.");
    assert_eq!(u64_value, Value::U64(0x1122_3344_5566_7788));

    let bool_value = Value::from_element_bytes(DataType::Bool, &[2, 0, 0, 0])
        .expect("Fix: bool scalar should decode.");
    assert_eq!(bool_value, Value::Bool(true));

    let f32_bits = 0x3F80_0000u32;
    let f32_value = Value::from_element_bytes(DataType::F32, &f32_bits.to_le_bytes())
        .expect("Fix: f32 scalar should decode.");
    assert_eq!(
        f32_value
            .try_as_f32()
            .expect("Fix: decoded f32 must expose f32 semantics.")
            .to_bits(),
        f32::from_bits(f32_bits).to_bits()
    );

    let f64_value = Value::from_element_bytes(DataType::F64, &(-0.0f64).to_le_bytes())
        .expect("Fix: f64 scalar should decode.");
    assert_eq!(f64_value, Value::Float(-0.0));
}

#[test]
fn generated_variable_width_datatypes_keep_full_payloads_and_empty_fallback_zeros() {
    let payload = [9, 8, 7, 6, 5, 4, 3];
    let cases = [
        DataType::Bytes,
        DataType::Tensor,
        DataType::SparseCsr {
            element: Box::new(DataType::U32),
        },
        DataType::SparseCoo {
            element: Box::new(DataType::U32),
        },
        DataType::SparseBsr {
            element: Box::new(DataType::U8),
            block_rows: 2,
            block_cols: 4,
        },
    ];

    for ty in cases {
        let value = Value::from_element_bytes(ty.clone(), &payload)
            .expect("Fix: variable-width datatypes should preserve host payload bytes.");
        assert_eq!(
            value.to_bytes(),
            payload,
            "Fix: {ty} should not invent a fixed element width."
        );
        assert!(
            Value::try_zero_for(ty.clone()).is_none() || ty == DataType::Bytes,
            "Fix: {ty} should not claim a typed zero unless it has fixed storage."
        );
    }
}
