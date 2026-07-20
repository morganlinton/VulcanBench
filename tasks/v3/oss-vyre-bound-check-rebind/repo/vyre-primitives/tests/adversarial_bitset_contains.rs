//! Generated adversarial contract tests for vyre-primitives.

// Adversarial tests for bitset::contains

#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_macros,
    clippy::identity_op,
    clippy::assertions_on_constants
)]

#[macro_use]
mod common;

use vyre_primitives::bitset::contains::*;

fn cpu_ref(input: &[u32], index: u32) -> u32 {
    let word = (index / 32) as usize;
    let bit = index % 32;
    input.get(word).map_or(0, |value| (value >> bit) & 1)
}

adversarial_vec_u32_cases! {
    test_bitset_contains_adv_0: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-0: Exact bit output mismatch";
    test_bitset_contains_adv_1: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-1: Exact bit output mismatch";
    test_bitset_contains_adv_2: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-2: Exact bit output mismatch";
    test_bitset_contains_adv_3: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-3: Exact bit output mismatch";
    test_bitset_contains_adv_4: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-4: Exact bit output mismatch";
    test_bitset_contains_adv_5: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-5: Exact bit output mismatch";
    test_bitset_contains_adv_6: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-6: Exact bit output mismatch";
    test_bitset_contains_adv_7: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-7: Exact bit output mismatch";
    test_bitset_contains_adv_8: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-8: Exact bit output mismatch";
    test_bitset_contains_adv_9: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-9: Exact bit output mismatch";
    test_bitset_contains_adv_10: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-10: Exact bit output mismatch";
    test_bitset_contains_adv_11: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-11: Exact bit output mismatch";
    test_bitset_contains_adv_12: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-12: Exact bit output mismatch";
    test_bitset_contains_adv_13: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-13: Exact bit output mismatch";
    test_bitset_contains_adv_14: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-14: Exact bit output mismatch";
    test_bitset_contains_adv_15: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-15: Exact bit output mismatch";
    test_bitset_contains_adv_16: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-16: Exact bit output mismatch";
    test_bitset_contains_adv_17: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-17: Exact bit output mismatch";
    test_bitset_contains_adv_18: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-18: Exact bit output mismatch";
    test_bitset_contains_adv_19: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-19: Exact bit output mismatch";
    test_bitset_contains_adv_20: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-20: Exact bit output mismatch";
    test_bitset_contains_adv_21: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-21: Exact bit output mismatch";
    test_bitset_contains_adv_22: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-22: Exact bit output mismatch";
    test_bitset_contains_adv_23: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-23: Exact bit output mismatch";
    test_bitset_contains_adv_24: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-24: Exact bit output mismatch";
    test_bitset_contains_adv_25: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-25: Exact bit output mismatch";
    test_bitset_contains_adv_26: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-26: Exact bit output mismatch";
    test_bitset_contains_adv_27: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-27: Exact bit output mismatch";
    test_bitset_contains_adv_28: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-28: Exact bit output mismatch";
    test_bitset_contains_adv_29: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-29: Exact bit output mismatch";
    test_bitset_contains_adv_30: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-30: Exact bit output mismatch";
    test_bitset_contains_adv_31: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-31: Exact bit output mismatch";
    test_bitset_contains_adv_32: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-32: Exact bit output mismatch";
    test_bitset_contains_adv_33: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-33: Exact bit output mismatch";
    test_bitset_contains_adv_34: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-34: Exact bit output mismatch";
    test_bitset_contains_adv_35: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-35: Exact bit output mismatch";
    test_bitset_contains_adv_36: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-36: Exact bit output mismatch";
    test_bitset_contains_adv_37: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-37: Exact bit output mismatch";
    test_bitset_contains_adv_38: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-38: Exact bit output mismatch";
    test_bitset_contains_adv_39: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-39: Exact bit output mismatch";
    test_bitset_contains_adv_40: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-40: Exact bit output mismatch";
    test_bitset_contains_adv_41: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-41: Exact bit output mismatch";
    test_bitset_contains_adv_42: vec![0u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-42: Exact bit output mismatch";
    test_bitset_contains_adv_43: vec![0u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-43: Exact bit output mismatch";
    test_bitset_contains_adv_44: vec![0u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-44: Exact bit output mismatch";
    test_bitset_contains_adv_45: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-45: Exact bit output mismatch";
    test_bitset_contains_adv_46: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-46: Exact bit output mismatch";
    test_bitset_contains_adv_47: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-47: Exact bit output mismatch";
    test_bitset_contains_adv_48: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-48: Exact bit output mismatch";
    test_bitset_contains_adv_49: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-49: Exact bit output mismatch";
    test_bitset_contains_adv_50: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-50: Exact bit output mismatch";
    test_bitset_contains_adv_51: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-51: Exact bit output mismatch";
    test_bitset_contains_adv_52: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-52: Exact bit output mismatch";
    test_bitset_contains_adv_53: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-53: Exact bit output mismatch";
    test_bitset_contains_adv_54: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-54: Exact bit output mismatch";
    test_bitset_contains_adv_55: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-55: Exact bit output mismatch";
    test_bitset_contains_adv_56: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-56: Exact bit output mismatch";
    test_bitset_contains_adv_57: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-57: Exact bit output mismatch";
    test_bitset_contains_adv_58: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-58: Exact bit output mismatch";
    test_bitset_contains_adv_59: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-59: Exact bit output mismatch";
    test_bitset_contains_adv_60: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-60: Exact bit output mismatch";
    test_bitset_contains_adv_61: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-61: Exact bit output mismatch";
    test_bitset_contains_adv_62: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-62: Exact bit output mismatch";
    test_bitset_contains_adv_63: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-63: Exact bit output mismatch";
    test_bitset_contains_adv_64: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-64: Exact bit output mismatch";
    test_bitset_contains_adv_65: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-65: Exact bit output mismatch";
    test_bitset_contains_adv_66: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-66: Exact bit output mismatch";
    test_bitset_contains_adv_67: vec![1u32; 0], 4294967295u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-67: Exact bit output mismatch";
    test_bitset_contains_adv_68: vec![1u32; 0], 31u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-68: Exact bit output mismatch";
    test_bitset_contains_adv_69: vec![1u32; 0], 0u32 => 0u32, "FINDING-ADV-BITSET-CONTAINS-69: Exact bit output mismatch";
}
