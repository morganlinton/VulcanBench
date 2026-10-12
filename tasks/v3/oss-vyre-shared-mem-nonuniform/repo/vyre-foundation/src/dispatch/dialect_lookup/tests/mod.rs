use super::*;

#[test]
fn intern_string_is_deterministic() {
    let a = intern_string("test::op::add");
    let b = intern_string("test::op::add");
    assert_eq!(a, b);
}

#[test]
fn intern_string_distinct_for_different_ops() {
    let a = intern_string("test::op::add");
    let b = intern_string("test::op::mul");
    assert_ne!(a, b);
}

#[test]
fn lowering_table_empty_has_no_native_builders() {
    let table = LoweringTable::empty();
    assert!(table.primary_text.is_none());
    assert!(table.primary_binary.is_none());
    assert!(table.secondary_text.is_none());
    assert!(table.native_module.is_none());
    assert!(table.extensions.is_empty());
}

#[test]
fn lowering_table_extension_lookup() {
    fn dummy_builder(_: &LoweringCtx<'_>) -> Result<Vec<u8>, String> {
        Ok(vec![1, 2, 3])
    }
    let table = LoweringTable::empty().with_extension("my-extension", dummy_builder);
    assert!(table.extension("my-extension").is_some());
    assert!(table.extension("nonexistent").is_none());
}

#[test]
fn opdef_default_has_empty_id() {
    let def = OpDef::default();
    assert_eq!(def.id(), "");
    assert!(def.program().is_none());
}

#[test]
fn signature_bytes_extractor_sets_flag() {
    let sig = Signature::bytes_extractor(&[], &[], &[]);
    assert!(sig.bytes_extraction);
}

#[test]
fn secondary_text_module_equality() {
    let a = TextModule {
        asm: ".version 7.0".into(),
        version: 70,
    };
    let b = TextModule {
        asm: ".version 7.0".into(),
        version: 70,
    };
    assert_eq!(a, b);
}

#[test]
fn native_module_module_equality() {
    let a = NativeModule {
        ast: vec![1, 2, 3],
        entry: "main".into(),
    };
    let b = NativeModule {
        ast: vec![1, 2, 3],
        entry: "main".into(),
    };
    assert_eq!(a, b);
}

#[test]
fn category_debug() {
    assert_eq!(format!("{:?}", Category::Composite), "Composite");
    assert_eq!(format!("{:?}", Category::Extension), "Extension");
    assert_eq!(format!("{:?}", Category::Intrinsic), "Intrinsic");
}
