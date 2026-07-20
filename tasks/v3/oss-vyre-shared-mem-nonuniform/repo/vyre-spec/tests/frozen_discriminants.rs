//! Failure-oriented tests for frozen discriminant contracts.
//!
//! Every enum that participates in the wire format or conformance catalog
//! must reject invalid discriminants, preserve high-bit invariants for
//! extension ids, and return conservative sentinels for unbounded types.

use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionTernaryOpId,
    ExtensionUnOpId,
};
use vyre_spec::{
    BufferAccess, CapabilityId, Category, CostHint, DataType, DeterminismClass, EngineInvariant,
    FloatType, InvariantCategory, OpSignature, OperationContract, PgNodeKind, SideEffectClass,
    Verification,
};

// ------------------------------------------------------------------
// PgNodeKind  -  invalid u32 discriminants must return None
// ------------------------------------------------------------------

#[test]
fn pg_node_kind_from_u32_rejects_zero() {
    assert!(
        PgNodeKind::from_u32(0).is_none(),
        "0 is not a valid PgNodeKind discriminant"
    );
}

#[test]
fn pg_node_kind_from_u32_rejects_out_of_range() {
    assert!(
        PgNodeKind::from_u32(21).is_none(),
        "21 is beyond the last defined PgNodeKind"
    );
}

#[test]
fn pg_node_kind_from_u32_rejects_max_u32() {
    assert!(
        PgNodeKind::from_u32(u32::MAX).is_none(),
        "u32::MAX must not silently map to a fake variant"
    );
}

#[test]
fn pg_node_kind_from_u32_accepts_boundary_values() {
    assert_eq!(PgNodeKind::from_u32(1), Some(PgNodeKind::VariableDecl));
    assert_eq!(PgNodeKind::from_u32(20), Some(PgNodeKind::LiteralFloat));
}

#[test]
fn pg_node_kind_full_discriminant_map_is_frozen_and_serde_stable() {
    let cases = [
        (1, PgNodeKind::VariableDecl, r#""VariableDecl""#),
        (2, PgNodeKind::VariableUse, r#""VariableUse""#),
        (3, PgNodeKind::Assignment, r#""Assignment""#),
        (4, PgNodeKind::Binary, r#""Binary""#),
        (5, PgNodeKind::Comparison, r#""Comparison""#),
        (6, PgNodeKind::FunctionCall, r#""FunctionCall""#),
        (7, PgNodeKind::FunctionDef, r#""FunctionDef""#),
        (8, PgNodeKind::IfStmt, r#""IfStmt""#),
        (9, PgNodeKind::ForStmt, r#""ForStmt""#),
        (10, PgNodeKind::WhileStmt, r#""WhileStmt""#),
        (11, PgNodeKind::ReturnStmt, r#""ReturnStmt""#),
        (12, PgNodeKind::Deref, r#""Deref""#),
        (13, PgNodeKind::AddrOf, r#""AddrOf""#),
        (14, PgNodeKind::Cast, r#""Cast""#),
        (15, PgNodeKind::MemberAccess, r#""MemberAccess""#),
        (16, PgNodeKind::ArrayAccess, r#""ArrayAccess""#),
        (17, PgNodeKind::StructDecl, r#""StructDecl""#),
        (18, PgNodeKind::LiteralInt, r#""LiteralInt""#),
        (19, PgNodeKind::LiteralStr, r#""LiteralStr""#),
        (20, PgNodeKind::LiteralFloat, r#""LiteralFloat""#),
    ];
    for (raw, expected, json) in cases {
        assert_eq!(
            PgNodeKind::from_u32(raw),
            Some(expected),
            "Fix: PgNodeKind discriminant {raw} changed; this is a frozen cross-tool graph ABI."
        );
        assert_eq!(
            serde_json::to_string(&expected).expect("Fix: PgNodeKind must serialize."),
            json,
            "Fix: PgNodeKind JSON spelling changed for discriminant {raw}."
        );
        assert_eq!(
            serde_json::from_str::<PgNodeKind>(json)
                .expect("Fix: PgNodeKind must deserialize frozen JSON spelling."),
            expected,
            "Fix: PgNodeKind JSON round-trip changed for discriminant {raw}."
        );
    }
    for raw in 21..=256 {
        assert!(
            PgNodeKind::from_u32(raw).is_none(),
            "Fix: PgNodeKind discriminant {raw} became valid without updating the frozen ABI table."
        );
    }
}

// ------------------------------------------------------------------
// Extension ids  -  high bit must always be set
// ------------------------------------------------------------------

#[test]
fn extension_data_type_id_always_has_high_bit() {
    let id = ExtensionDataTypeId::from_name("anything.at.all");
    assert!(
        id.is_extension(),
        "extension id must have high bit set: {:#010x}",
        id.as_u32()
    );
    assert_ne!(id.as_u32() & 0x8000_0000, 0);
}

#[test]
fn extension_binop_id_always_has_high_bit() {
    let id = ExtensionBinOpId::from_name("test.op");
    assert_ne!(id.as_u32() & ExtensionBinOpId::EXTENSION_RANGE_MASK, 0);
}

#[test]
fn extension_unop_id_always_has_high_bit() {
    let id = ExtensionUnOpId::from_name("test.op");
    assert_ne!(id.as_u32() & ExtensionUnOpId::EXTENSION_RANGE_MASK, 0);
}

#[test]
fn extension_atomic_op_id_always_has_high_bit() {
    let id = ExtensionAtomicOpId::from_name("test.op");
    assert_ne!(id.as_u32() & ExtensionAtomicOpId::EXTENSION_RANGE_MASK, 0);
}

#[test]
fn extension_ternary_op_id_always_has_high_bit() {
    let id = ExtensionTernaryOpId::from_name("test.op");
    assert_ne!(id.as_u32() & ExtensionTernaryOpId::EXTENSION_RANGE_MASK, 0);
}

// ------------------------------------------------------------------
// DataType  -  conservative sentinels for unbounded / opaque types
// ------------------------------------------------------------------

#[test]
fn data_type_max_bytes_returns_none_for_unbounded_variants() {
    assert!(DataType::TensorShaped {
        element: Box::new(DataType::F32),
        shape: [1, 2].as_slice().into()
    }
    .max_bytes()
    .is_none());
    assert!(DataType::SparseCsr {
        element: Box::new(DataType::F32)
    }
    .max_bytes()
    .is_none());
    assert!(DataType::SparseCoo {
        element: Box::new(DataType::F32)
    }
    .max_bytes()
    .is_none());
    assert!(DataType::SparseBsr {
        element: Box::new(DataType::F32),
        block_rows: 2,
        block_cols: 2
    }
    .max_bytes()
    .is_none());
}

#[test]
fn data_type_size_bytes_returns_none_for_variable_types() {
    assert!(DataType::Tensor.size_bytes().is_none());
    assert!(DataType::TensorShaped {
        element: Box::new(DataType::F32),
        shape: [1].as_slice().into()
    }
    .size_bytes()
    .is_none());
    assert!(DataType::SparseCsr {
        element: Box::new(DataType::F32)
    }
    .size_bytes()
    .is_none());
}

#[test]
fn data_type_min_bytes_is_zero_for_unbounded() {
    assert_eq!(DataType::Tensor.min_bytes(), 0);
    assert_eq!(DataType::Bytes.min_bytes(), 0);
    assert_eq!(
        DataType::DeviceMesh {
            axes: [2, 2].as_slice().into()
        }
        .min_bytes(),
        0
    );
}

#[test]
fn data_type_is_float_family_recognizes_all_floats() {
    assert!(DataType::F16.is_float_family());
    assert!(DataType::BF16.is_float_family());
    assert!(DataType::F32.is_float_family());
    assert!(DataType::F64.is_float_family());
    assert!(DataType::F8E4M3.is_float_family());
    assert!(DataType::F8E5M2.is_float_family());
    assert!(DataType::FP4.is_float_family());
    assert!(DataType::NF4.is_float_family());
}

#[test]
fn data_type_is_float_family_rejects_integers() {
    assert!(!DataType::U32.is_float_family());
    assert!(!DataType::I32.is_float_family());
    assert!(!DataType::U64.is_float_family());
    assert!(!DataType::Bool.is_float_family());
}

#[test]
fn data_type_is_float_family_propagates_through_vec() {
    let vec_f32 = DataType::Vec {
        element: Box::new(DataType::F32),
        count: 4,
    };
    assert!(vec_f32.is_float_family());
    let vec_u32 = DataType::Vec {
        element: Box::new(DataType::U32),
        count: 4,
    };
    assert!(!vec_u32.is_float_family());
}

// ------------------------------------------------------------------
// Verification  -  witness_count contract
// ------------------------------------------------------------------

#[test]
fn verification_witness_count_is_none_for_exhaustive_variants() {
    assert!(Verification::ExhaustiveU8.witness_count().is_none());
    assert!(Verification::ExhaustiveU16.witness_count().is_none());
    assert!(Verification::ExhaustiveFloat {
        typ: FloatType::F32,
    }
    .witness_count()
    .is_none());
}

#[test]
fn verification_witness_count_matches_for_witnessed_u32() {
    let v = Verification::WitnessedU32 {
        seed: 7,
        count: 1024,
    };
    assert_eq!(v.witness_count(), Some(1024));
}

// ------------------------------------------------------------------
// OperationContract  -  empty/none contract invariants
// ------------------------------------------------------------------

#[test]
fn operation_contract_none_has_all_none_fields() {
    let c = OperationContract::none();
    assert!(c.capability_requirements.is_none());
    assert!(c.determinism.is_none());
    assert!(c.side_effect.is_none());
    assert!(c.cost_hint.is_none());
}

#[test]
fn operation_contract_default_equals_none() {
    let c: OperationContract = Default::default();
    assert!(c.capability_requirements.is_none());
    assert!(c.determinism.is_none());
    assert!(c.side_effect.is_none());
    assert!(c.cost_hint.is_none());
}

#[test]
fn capability_id_as_str_roundtrips() {
    let c = CapabilityId::new("subgroup_ops");
    assert_eq!(c.as_str(), "subgroup_ops");
}

// ------------------------------------------------------------------
// OpSignature  -  edge cases
// ------------------------------------------------------------------

#[test]
fn op_signature_min_input_bytes_with_empty_inputs_is_zero() {
    let sig = OpSignature {
        inputs: vec![],
        output: DataType::U32,
        input_params: None,
        output_params: None,
        contract: None,
    };
    assert_eq!(sig.min_input_bytes(), 0);
}

// ------------------------------------------------------------------
// EngineInvariant  -  ordinal coverage
// ------------------------------------------------------------------

#[test]
fn every_engine_invariant_ordinal_matches_name() {
    for inv in EngineInvariant::iter() {
        let expected = match inv {
            EngineInvariant::I1 => 1,
            EngineInvariant::I2 => 2,
            EngineInvariant::I3 => 3,
            EngineInvariant::I4 => 4,
            EngineInvariant::I5 => 5,
            EngineInvariant::I6 => 6,
            EngineInvariant::I7 => 7,
            EngineInvariant::I8 => 8,
            EngineInvariant::I9 => 9,
            EngineInvariant::I10 => 10,
            EngineInvariant::I11 => 11,
            EngineInvariant::I12 => 12,
            EngineInvariant::I13 => 13,
            EngineInvariant::I14 => 14,
            EngineInvariant::I15 => 15,
            _ => panic!("unexpected EngineInvariant variant"),
        };
        assert_eq!(inv.ordinal(), expected, "ordinal mismatch for {:?}", inv);
    }
}

#[test]
fn engine_invariant_display_matches_ordinal() {
    assert_eq!(EngineInvariant::I4.to_string(), "I4");
    assert_eq!(EngineInvariant::I15.to_string(), "I15");
}

// ------------------------------------------------------------------
// Category  -  PartialEq ignores backend availability function pointer
// ------------------------------------------------------------------

#[test]
fn category_c_equality_ignores_availability_fn() {
    let c1 = Category::C {
        hardware: "tensorcore",
        backend_availability: vyre_spec::BackendAvailabilityPredicate::new(|_| true),
    };
    let c2 = Category::C {
        hardware: "tensorcore",
        backend_availability: vyre_spec::BackendAvailabilityPredicate::new(|_| false),
    };
    assert_eq!(
        c1, c2,
        "Category C equality must depend only on hardware string"
    );
}

#[test]
fn category_c_inequality_for_different_hardware() {
    let c1 = Category::C {
        hardware: "tensorcore",
        backend_availability: vyre_spec::BackendAvailabilityPredicate::new(|_| true),
    };
    let c2 = Category::C {
        hardware: "raytracing",
        backend_availability: vyre_spec::BackendAvailabilityPredicate::new(|_| true),
    };
    assert_ne!(c1, c2);
}

#[test]
fn category_unclassified_is_not_equal_to_populated_a() {
    let empty = Category::unclassified();
    let populated = Category::A {
        composition_of: vec!["vyre::add"],
    };
    assert_ne!(empty, populated);
}

// ------------------------------------------------------------------
// FloatType  -  basic variant distinctness
// ------------------------------------------------------------------

#[test]
fn float_type_variants_are_distinct() {
    assert_ne!(FloatType::F16, FloatType::F32);
    assert_ne!(FloatType::BF16, FloatType::F32);
    assert_ne!(FloatType::F16, FloatType::BF16);
}

// ------------------------------------------------------------------
// BufferAccess  -  variant distinctness
// ------------------------------------------------------------------

#[test]
fn buffer_access_variants_are_distinct() {
    assert_ne!(BufferAccess::ReadOnly, BufferAccess::ReadWrite);
    assert_ne!(BufferAccess::Uniform, BufferAccess::Workgroup);
    assert_ne!(BufferAccess::WriteOnly, BufferAccess::ReadOnly);
}

// ------------------------------------------------------------------
// InvariantCategory  -  variant distinctness
// ------------------------------------------------------------------

#[test]
fn invariant_category_variants_are_distinct() {
    assert_ne!(InvariantCategory::Execution, InvariantCategory::Algebra);
    assert_ne!(InvariantCategory::Resource, InvariantCategory::Stability);
}

// ------------------------------------------------------------------
// DeterminismClass / SideEffectClass / CostHint  -  variant distinctness
// ------------------------------------------------------------------

#[test]
fn determinism_class_variants_are_distinct() {
    assert_ne!(
        DeterminismClass::Deterministic,
        DeterminismClass::NonDeterministic
    );
    assert_ne!(
        DeterminismClass::DeterministicModuloRounding,
        DeterminismClass::Deterministic
    );
}

#[test]
fn side_effect_class_variants_are_distinct() {
    assert_ne!(SideEffectClass::Pure, SideEffectClass::WritesMemory);
    assert_ne!(SideEffectClass::Atomic, SideEffectClass::Synchronizing);
}

#[test]
fn cost_hint_variants_are_distinct() {
    assert_ne!(CostHint::Cheap, CostHint::Expensive);
    assert_ne!(CostHint::Medium, CostHint::Unknown);
}
