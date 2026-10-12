//! Frozen layout matrix for every public `DataType` family.
//!
//! These are certificate-facing contracts. A backend may depend on
//! `vyre-spec` alone, so byte width, bit width, display spelling, and
//! conservative bounds must move only through explicit spec updates.

use vyre_spec::extension::ExtensionDataTypeId;
use vyre_spec::{DataType, TypeId};

#[derive(Clone)]
struct LayoutCase {
    ty: DataType,
    display: &'static str,
    min_bytes: usize,
    max_bytes: Option<usize>,
    size_bytes: Option<usize>,
    element_size: Option<usize>,
    bit_width: Option<usize>,
    is_float: bool,
}

#[test]
fn builtin_data_type_layout_matrix_is_frozen() {
    let cases = [
        LayoutCase::fixed(DataType::U8, "u8", 1, 8, false),
        LayoutCase::fixed(DataType::U16, "u16", 2, 16, false),
        LayoutCase::fixed(DataType::U32, "u32", 4, 32, false),
        LayoutCase::fixed(DataType::U64, "u64", 8, 64, false),
        LayoutCase::fixed(DataType::I8, "i8", 1, 8, false),
        LayoutCase::fixed(DataType::I16, "i16", 2, 16, false),
        LayoutCase::fixed(DataType::I32, "i32", 4, 32, false),
        LayoutCase::fixed(DataType::I64, "i64", 8, 64, false),
        LayoutCase::fixed(DataType::Bool, "bool", 4, 32, false),
        LayoutCase::fixed(DataType::F16, "f16", 2, 16, true),
        LayoutCase::fixed(DataType::BF16, "bf16", 2, 16, true),
        LayoutCase::fixed(DataType::F32, "f32", 4, 32, true),
        LayoutCase::fixed(DataType::F64, "f64", 8, 64, true),
        LayoutCase::fixed(DataType::F8E4M3, "f8e4m3", 1, 8, true),
        LayoutCase::fixed(DataType::F8E5M2, "f8e5m2", 1, 8, true),
        LayoutCase::fixed(DataType::I4, "i4", 1, 4, false),
        LayoutCase::fixed(DataType::FP4, "fp4", 1, 4, true),
        LayoutCase::fixed(DataType::NF4, "nf4", 1, 4, true),
        LayoutCase::fixed(DataType::Vec2U32, "vec2<u32>", 8, 64, false),
        LayoutCase::fixed(DataType::Vec4U32, "vec4<u32>", 16, 128, false),
        LayoutCase::fixed(
            DataType::Handle(TypeId(0xABCD)),
            "handle<0x0000abcd>",
            4,
            32,
            false,
        ),
        LayoutCase {
            ty: DataType::Bytes,
            display: "bytes",
            min_bytes: 0,
            max_bytes: Some(64 * 1024 * 1024),
            size_bytes: Some(1),
            element_size: None,
            bit_width: Some(8),
            is_float: false,
        },
        LayoutCase {
            ty: DataType::Array { element_size: 7 },
            display: "array<7B>",
            min_bytes: 0,
            max_bytes: Some(256 * 1024 * 1024),
            size_bytes: Some(7),
            element_size: Some(7),
            bit_width: None,
            is_float: false,
        },
        LayoutCase {
            ty: DataType::Tensor,
            display: "tensor",
            min_bytes: 0,
            max_bytes: Some(256 * 1024 * 1024),
            size_bytes: None,
            element_size: None,
            bit_width: None,
            is_float: false,
        },
        LayoutCase {
            ty: DataType::Vec {
                element: Box::new(DataType::U16),
                count: 3,
            },
            display: "vec<u16;3>",
            min_bytes: 6,
            max_bytes: Some(6),
            size_bytes: Some(6),
            element_size: Some(2),
            bit_width: Some(48),
            is_float: false,
        },
        LayoutCase {
            ty: DataType::Vec {
                element: Box::new(DataType::F32),
                count: 4,
            },
            display: "vec<f32;4>",
            min_bytes: 16,
            max_bytes: Some(16),
            size_bytes: Some(16),
            element_size: Some(4),
            bit_width: Some(128),
            is_float: true,
        },
        LayoutCase {
            ty: DataType::TensorShaped {
                element: Box::new(DataType::F32),
                shape: [2, 3].as_slice().into(),
            },
            display: "tensor<f32;2x3>",
            min_bytes: 0,
            max_bytes: None,
            size_bytes: None,
            element_size: Some(4),
            bit_width: None,
            is_float: true,
        },
        LayoutCase {
            ty: DataType::SparseCsr {
                element: Box::new(DataType::F32),
            },
            display: "sparse_csr<f32>",
            min_bytes: 0,
            max_bytes: None,
            size_bytes: None,
            element_size: Some(4),
            bit_width: None,
            is_float: true,
        },
        LayoutCase {
            ty: DataType::SparseCoo {
                element: Box::new(DataType::I32),
            },
            display: "sparse_coo<i32>",
            min_bytes: 0,
            max_bytes: None,
            size_bytes: None,
            element_size: Some(4),
            bit_width: None,
            is_float: false,
        },
        LayoutCase {
            ty: DataType::SparseBsr {
                element: Box::new(DataType::F16),
                block_rows: 2,
                block_cols: 4,
            },
            display: "sparse_bsr<f16;2x4>",
            min_bytes: 0,
            max_bytes: None,
            size_bytes: None,
            element_size: Some(2),
            bit_width: None,
            is_float: true,
        },
        LayoutCase {
            ty: DataType::DeviceMesh {
                axes: [2, 4, 8].as_slice().into(),
            },
            display: "device_mesh<2x4x8>",
            min_bytes: 0,
            max_bytes: Some(4),
            size_bytes: Some(4),
            element_size: None,
            bit_width: Some(32),
            is_float: false,
        },
    ];

    for case in cases {
        assert_layout(case);
    }
}

#[test]
fn opaque_data_type_uses_conservative_spec_level_sentinels() {
    let ty = DataType::Opaque(ExtensionDataTypeId::from_name("vendor.dtype"));

    assert_eq!(ty.builtin_wire_tag(), None);
    assert_eq!(ty.min_bytes(), 0);
    assert_eq!(ty.max_bytes(), None);
    assert_eq!(ty.size_bytes(), None);
    assert_eq!(ty.element_size(), None);
    assert_eq!(ty.bit_width(), None);
    assert!(!ty.is_float_family());
    assert!(
        ty.to_string().starts_with("opaque<0x"),
        "Fix: opaque display must expose the stable extension id, got `{ty}`."
    );
}

impl LayoutCase {
    fn fixed(
        ty: DataType,
        display: &'static str,
        bytes: usize,
        bits: usize,
        is_float: bool,
    ) -> Self {
        Self {
            ty,
            display,
            min_bytes: bytes,
            max_bytes: Some(bytes),
            size_bytes: Some(bytes),
            element_size: None,
            bit_width: Some(bits),
            is_float,
        }
    }
}

fn assert_layout(case: LayoutCase) {
    assert_eq!(case.ty.to_string(), case.display);
    assert_eq!(
        case.ty.min_bytes(),
        case.min_bytes,
        "Fix: min_bytes drifted for {}.",
        case.display
    );
    assert_eq!(
        case.ty.max_bytes(),
        case.max_bytes,
        "Fix: max_bytes drifted for {}.",
        case.display
    );
    assert_eq!(
        case.ty.size_bytes(),
        case.size_bytes,
        "Fix: size_bytes drifted for {}.",
        case.display
    );
    assert_eq!(
        case.ty.element_size(),
        case.element_size,
        "Fix: element_size drifted for {}.",
        case.display
    );
    assert_eq!(
        case.ty.bit_width(),
        case.bit_width,
        "Fix: bit_width drifted for {}.",
        case.display
    );
    assert_eq!(
        case.ty.is_float_family(),
        case.is_float,
        "Fix: float-family classification drifted for {}.",
        case.display
    );
}
