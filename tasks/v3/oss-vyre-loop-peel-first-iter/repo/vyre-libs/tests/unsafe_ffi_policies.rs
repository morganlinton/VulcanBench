//! Unsafe ffi policies test suite.

const UNSAFE_BLOCKS: &str =
    include_str!("../../docs/optimization/UNSAFE_BLOCK_SAFETY_CONTRACTS.toml");
const FFI: &str = include_str!("../../docs/optimization/FFI_ABI_BOUNDARY_CONTRACTS.toml");
const LAYOUT: &str =
    include_str!("../../docs/optimization/MEMORY_LAYOUT_ALIASING_ALIGNMENT.toml");

#[test]
fn unsafe_block_safety_contracts_record_capability_invariants_wrappers_and_diagnostics() {
    for required in [
        "unsafe_id",
        "owning_crate",
        "unsafe_capability",
        "safety_invariant",
        "preconditions",
        "postconditions",
        "safe_wrapper",
        "audit_owner",
        "diagnostic",
    ] {
        assert!(
            UNSAFE_BLOCKS.contains(required),
            "unsafe block safety contract must include {required}"
        );
    }
}

#[test]
fn ffi_abi_boundary_contracts_record_abi_pointer_string_ownership_thread_and_unwind_policies() {
    for required in [
        "boundary_id",
        "abi",
        "symbol_class",
        "pointer_policy",
        "string_policy",
        "ownership_policy",
        "thread_safety_policy",
        "unwind_policy",
        "C-unwind",
        "panic-cannot-cross-ffi-boundary",
    ] {
        assert!(
            FFI.contains(required),
            "FFI ABI boundary contract must include {required}"
        );
    }
}

#[test]
fn memory_layout_contracts_record_repr_size_alignment_aliasing_endianness_and_validity() {
    for required in [
        "layout_id",
        "type_class",
        "repr_policy",
        "size_policy",
        "alignment_policy",
        "aliasing_policy",
        "endianness_policy",
        "validity_policy",
        "diagnostic",
    ] {
        assert!(
            LAYOUT.contains(required),
            "memory layout aliasing alignment contract must include {required}"
        );
    }
}
