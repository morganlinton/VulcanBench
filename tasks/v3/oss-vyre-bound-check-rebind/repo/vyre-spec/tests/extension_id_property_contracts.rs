//! Generated property coverage for extension-id determinism and reserved range.

use proptest::prelude::*;
use vyre_spec::extension::{
    ExtensionAtomicOpId, ExtensionBinOpId, ExtensionDataTypeId, ExtensionTernaryOpId,
    ExtensionUnOpId,
};

fn extension_name_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_.-]{0,63}"
}

proptest! {
    #[test]
    fn generated_extension_data_type_ids_are_deterministic_and_reserved(name in extension_name_strategy()) {
        let first = ExtensionDataTypeId::from_name(&name);
        let second = ExtensionDataTypeId::from_name(&name);

        prop_assert_eq!(first, second);
        prop_assert!(first.is_extension());
        prop_assert_ne!(first.as_u32() & ExtensionDataTypeId::EXTENSION_RANGE_MASK, 0);
    }

    #[test]
    fn generated_extension_operator_ids_are_deterministic_and_reserved(name in extension_name_strategy()) {
        let bin = ExtensionBinOpId::from_name(&name);
        let un = ExtensionUnOpId::from_name(&name);
        let atomic = ExtensionAtomicOpId::from_name(&name);
        let ternary = ExtensionTernaryOpId::from_name(&name);

        prop_assert_eq!(bin, ExtensionBinOpId::from_name(&name));
        prop_assert_eq!(un, ExtensionUnOpId::from_name(&name));
        prop_assert_eq!(atomic, ExtensionAtomicOpId::from_name(&name));
        prop_assert_eq!(ternary, ExtensionTernaryOpId::from_name(&name));

        prop_assert!(bin.is_extension());
        prop_assert!(un.is_extension());
        prop_assert!(atomic.is_extension());
        prop_assert!(ternary.is_extension());
    }
}
