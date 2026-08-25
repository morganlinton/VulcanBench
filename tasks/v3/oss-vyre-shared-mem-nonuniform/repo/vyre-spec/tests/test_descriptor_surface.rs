//! Surface tests for `TestDescriptor` struct construction and field access.

use vyre_spec::{InvariantId, TestDescriptor};

#[test]
fn test_descriptor_new_roundtrips_fields() {
    let td = TestDescriptor {
        name: "my_test",
        purpose: "check something",
        invariant: InvariantId::I4,
    };
    assert_eq!(td.name, "my_test");
    assert_eq!(td.purpose, "check something");
    assert_eq!(td.invariant.ordinal(), 4);
}

#[test]
fn test_descriptor_unicode_reason() {
    let td = TestDescriptor {
        name: "unicode_テスト",
        purpose: "Unicode reason: 🚀",
        invariant: InvariantId::I7,
    };
    assert_eq!(td.name, "unicode_テスト");
    assert_eq!(td.purpose, "Unicode reason: 🚀");
    assert_eq!(td.invariant.ordinal(), 7);
}

#[test]
fn test_descriptor_name_is_static_str() {
    let td = TestDescriptor {
        name: "static_name",
        purpose: "p",
        invariant: InvariantId::I1,
    };
    assert_eq!(td.name, "static_name");
}

#[test]
fn test_descriptor_invariant_i1_is_valid() {
    let td = TestDescriptor {
        name: "i1_invariant",
        purpose: "p",
        invariant: InvariantId::I1,
    };
    assert_eq!(td.invariant.ordinal(), 1);
}

#[test]
fn test_descriptor_purpose_is_empty_allowed() {
    let td = TestDescriptor {
        name: "empty_purpose",
        purpose: "",
        invariant: InvariantId::I11,
    };
    assert_eq!(td.purpose, "");
    assert_eq!(td.invariant.ordinal(), 11);
}

#[test]
fn test_descriptor_invariant_display() {
    let td = TestDescriptor {
        name: "display",
        purpose: "p",
        invariant: InvariantId::I5,
    };
    assert_eq!(format!("{}", td.invariant), "I5");
}
