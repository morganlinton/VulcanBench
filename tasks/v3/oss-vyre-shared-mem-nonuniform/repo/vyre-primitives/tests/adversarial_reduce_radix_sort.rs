//! Generated adversarial contract tests for vyre-primitives.

// Adversarial tests for reduce::radix_sort

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

use vyre_primitives::reduce::radix_sort::*;

fn cpu_ref(input: &[u32], bits: u32) -> Vec<u32> {
    let bits = bits.min(32);
    let mut out = input.to_vec();
    if bits == 0 {
        return out;
    }
    let mask = if bits == 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    };
    out.sort_by_key(|value| *value & mask);
    out
}

adversarial_vec_u32_cases! {
    test_reduce_radix_sort_adv_0: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-0: Exact bit output mismatch";
    test_reduce_radix_sort_adv_1: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-1: Exact bit output mismatch";
    test_reduce_radix_sort_adv_2: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-2: Exact bit output mismatch";
    test_reduce_radix_sort_adv_3: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-3: Exact bit output mismatch";
    test_reduce_radix_sort_adv_4: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-4: Exact bit output mismatch";
    test_reduce_radix_sort_adv_5: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-5: Exact bit output mismatch";
    test_reduce_radix_sort_adv_6: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-6: Exact bit output mismatch";
    test_reduce_radix_sort_adv_7: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-7: Exact bit output mismatch";
    test_reduce_radix_sort_adv_8: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-8: Exact bit output mismatch";
    test_reduce_radix_sort_adv_9: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-9: Exact bit output mismatch";
    test_reduce_radix_sort_adv_10: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-10: Exact bit output mismatch";
    test_reduce_radix_sort_adv_11: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-11: Exact bit output mismatch";
    test_reduce_radix_sort_adv_12: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-12: Exact bit output mismatch";
    test_reduce_radix_sort_adv_13: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-13: Exact bit output mismatch";
    test_reduce_radix_sort_adv_14: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-14: Exact bit output mismatch";
    test_reduce_radix_sort_adv_15: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-15: Exact bit output mismatch";
    test_reduce_radix_sort_adv_16: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-16: Exact bit output mismatch";
    test_reduce_radix_sort_adv_17: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-17: Exact bit output mismatch";
    test_reduce_radix_sort_adv_18: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-18: Exact bit output mismatch";
    test_reduce_radix_sort_adv_19: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-19: Exact bit output mismatch";
    test_reduce_radix_sort_adv_20: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-20: Exact bit output mismatch";
    test_reduce_radix_sort_adv_21: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-21: Exact bit output mismatch";
    test_reduce_radix_sort_adv_22: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-22: Exact bit output mismatch";
    test_reduce_radix_sort_adv_23: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-23: Exact bit output mismatch";
    test_reduce_radix_sort_adv_24: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-24: Exact bit output mismatch";
    test_reduce_radix_sort_adv_25: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-25: Exact bit output mismatch";
    test_reduce_radix_sort_adv_26: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-26: Exact bit output mismatch";
    test_reduce_radix_sort_adv_27: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-27: Exact bit output mismatch";
    test_reduce_radix_sort_adv_28: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-28: Exact bit output mismatch";
    test_reduce_radix_sort_adv_29: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-29: Exact bit output mismatch";
    test_reduce_radix_sort_adv_30: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-30: Exact bit output mismatch";
    test_reduce_radix_sort_adv_31: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-31: Exact bit output mismatch";
    test_reduce_radix_sort_adv_32: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-32: Exact bit output mismatch";
    test_reduce_radix_sort_adv_33: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-33: Exact bit output mismatch";
    test_reduce_radix_sort_adv_34: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-34: Exact bit output mismatch";
    test_reduce_radix_sort_adv_35: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-35: Exact bit output mismatch";
    test_reduce_radix_sort_adv_36: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-36: Exact bit output mismatch";
    test_reduce_radix_sort_adv_37: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-37: Exact bit output mismatch";
    test_reduce_radix_sort_adv_38: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-38: Exact bit output mismatch";
    test_reduce_radix_sort_adv_39: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-39: Exact bit output mismatch";
    test_reduce_radix_sort_adv_40: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-40: Exact bit output mismatch";
    test_reduce_radix_sort_adv_41: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-41: Exact bit output mismatch";
    test_reduce_radix_sort_adv_42: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-42: Exact bit output mismatch";
    test_reduce_radix_sort_adv_43: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-43: Exact bit output mismatch";
    test_reduce_radix_sort_adv_44: vec![0u32; 0], 0u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-44: Exact bit output mismatch";
    test_reduce_radix_sort_adv_45: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-45: Exact bit output mismatch";
    test_reduce_radix_sort_adv_46: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-46: Exact bit output mismatch";
    test_reduce_radix_sort_adv_47: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-47: Exact bit output mismatch";
    test_reduce_radix_sort_adv_48: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-48: Exact bit output mismatch";
    test_reduce_radix_sort_adv_49: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-49: Exact bit output mismatch";
    test_reduce_radix_sort_adv_50: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-50: Exact bit output mismatch";
    test_reduce_radix_sort_adv_51: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-51: Exact bit output mismatch";
    test_reduce_radix_sort_adv_52: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-52: Exact bit output mismatch";
    test_reduce_radix_sort_adv_53: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-53: Exact bit output mismatch";
    test_reduce_radix_sort_adv_54: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-54: Exact bit output mismatch";
    test_reduce_radix_sort_adv_55: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-55: Exact bit output mismatch";
    test_reduce_radix_sort_adv_56: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-56: Exact bit output mismatch";
    test_reduce_radix_sort_adv_57: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-57: Exact bit output mismatch";
    test_reduce_radix_sort_adv_58: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-58: Exact bit output mismatch";
    test_reduce_radix_sort_adv_59: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-59: Exact bit output mismatch";
    test_reduce_radix_sort_adv_60: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-60: Exact bit output mismatch";
    test_reduce_radix_sort_adv_61: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-61: Exact bit output mismatch";
    test_reduce_radix_sort_adv_62: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-62: Exact bit output mismatch";
    test_reduce_radix_sort_adv_63: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-63: Exact bit output mismatch";
    test_reduce_radix_sort_adv_64: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-64: Exact bit output mismatch";
    test_reduce_radix_sort_adv_65: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-65: Exact bit output mismatch";
    test_reduce_radix_sort_adv_66: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-66: Exact bit output mismatch";
    test_reduce_radix_sort_adv_67: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-67: Exact bit output mismatch";
    test_reduce_radix_sort_adv_68: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-68: Exact bit output mismatch";
    test_reduce_radix_sort_adv_69: vec![1u32; 0], 1u32 => vec![], "FINDING-ADV-REDUCE-RADIX_SORT-69: Exact bit output mismatch";
}
