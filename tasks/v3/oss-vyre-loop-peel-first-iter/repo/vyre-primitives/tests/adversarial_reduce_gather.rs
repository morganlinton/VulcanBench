//! Generated adversarial contract tests for vyre-primitives.

// Adversarial tests for reduce::gather

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

use vyre_primitives::reduce::gather::*;

fn cpu_ref(src: &[u32], indices: &[u32]) -> Vec<u32> {
    indices
        .iter()
        .map(|index| src.get(*index as usize).copied().unwrap_or(0))
        .collect()
}

adversarial_binary_vec_cases! {
    test_reduce_gather_adv_0: vec![0u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-0: Exact bit output mismatch";
    test_reduce_gather_adv_1: vec![0u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-1: Exact bit output mismatch";
    test_reduce_gather_adv_2: vec![0u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-2: Exact bit output mismatch";
    test_reduce_gather_adv_3: vec![0u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-3: Exact bit output mismatch";
    test_reduce_gather_adv_4: vec![0u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-4: Exact bit output mismatch";
    test_reduce_gather_adv_5: vec![0u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-5: Exact bit output mismatch";
    test_reduce_gather_adv_6: vec![0u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-6: Exact bit output mismatch";
    test_reduce_gather_adv_7: vec![0u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-7: Exact bit output mismatch";
    test_reduce_gather_adv_8: vec![0u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-8: Exact bit output mismatch";
    test_reduce_gather_adv_9: vec![0u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-9: Exact bit output mismatch";
    test_reduce_gather_adv_10: vec![0u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-10: Exact bit output mismatch";
    test_reduce_gather_adv_11: vec![0u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-11: Exact bit output mismatch";
    test_reduce_gather_adv_12: vec![0u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-12: Exact bit output mismatch";
    test_reduce_gather_adv_13: vec![0u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-13: Exact bit output mismatch";
    test_reduce_gather_adv_14: vec![0u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-14: Exact bit output mismatch";
    test_reduce_gather_adv_15: vec![0u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-15: Exact bit output mismatch";
    test_reduce_gather_adv_16: vec![0u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-16: Exact bit output mismatch";
    test_reduce_gather_adv_17: vec![0u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-17: Exact bit output mismatch";
    test_reduce_gather_adv_18: vec![0u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-18: Exact bit output mismatch";
    test_reduce_gather_adv_19: vec![0u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-19: Exact bit output mismatch";
    test_reduce_gather_adv_20: vec![0u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-20: Exact bit output mismatch";
    test_reduce_gather_adv_21: vec![0u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-21: Exact bit output mismatch";
    test_reduce_gather_adv_22: vec![0u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-22: Exact bit output mismatch";
    test_reduce_gather_adv_23: vec![0u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-23: Exact bit output mismatch";
    test_reduce_gather_adv_24: vec![0u32; 0], vec![2143289344u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-24: Exact bit output mismatch";
    test_reduce_gather_adv_25: vec![0u32; 0], vec![2143289344u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-25: Exact bit output mismatch";
    test_reduce_gather_adv_26: vec![0u32; 0], vec![2143289344u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-26: Exact bit output mismatch";
    test_reduce_gather_adv_27: vec![0u32; 0], vec![0u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-27: Exact bit output mismatch";
    test_reduce_gather_adv_28: vec![0u32; 0], vec![0u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-28: Exact bit output mismatch";
    test_reduce_gather_adv_29: vec![0u32; 0], vec![0u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-29: Exact bit output mismatch";
    test_reduce_gather_adv_30: vec![0u32; 0], vec![4294967295u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-30: Exact bit output mismatch";
    test_reduce_gather_adv_31: vec![0u32; 0], vec![4294967295u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-31: Exact bit output mismatch";
    test_reduce_gather_adv_32: vec![0u32; 0], vec![4294967295u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-32: Exact bit output mismatch";
    test_reduce_gather_adv_33: vec![0u32; 0], vec![2143289344u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-33: Exact bit output mismatch";
    test_reduce_gather_adv_34: vec![0u32; 0], vec![2143289344u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-34: Exact bit output mismatch";
    test_reduce_gather_adv_35: vec![0u32; 0], vec![2143289344u32; 32] => vec![0; 32], "FINDING-ADV-REDUCE-GATHER-35: Exact bit output mismatch";
    test_reduce_gather_adv_36: vec![0u32; 0], vec![0u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-36: Exact bit output mismatch";
    test_reduce_gather_adv_37: vec![0u32; 0], vec![0u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-37: Exact bit output mismatch";
    test_reduce_gather_adv_38: vec![0u32; 0], vec![0u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-38: Exact bit output mismatch";
    test_reduce_gather_adv_39: vec![0u32; 0], vec![4294967295u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-39: Exact bit output mismatch";
    test_reduce_gather_adv_40: vec![0u32; 0], vec![4294967295u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-40: Exact bit output mismatch";
    test_reduce_gather_adv_41: vec![0u32; 0], vec![4294967295u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-41: Exact bit output mismatch";
    test_reduce_gather_adv_42: vec![0u32; 0], vec![2143289344u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-42: Exact bit output mismatch";
    test_reduce_gather_adv_43: vec![0u32; 0], vec![2143289344u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-43: Exact bit output mismatch";
    test_reduce_gather_adv_44: vec![0u32; 0], vec![2143289344u32; 1024] => vec![0; 1024], "FINDING-ADV-REDUCE-GATHER-44: Exact bit output mismatch";
    test_reduce_gather_adv_45: vec![1u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-45: Exact bit output mismatch";
    test_reduce_gather_adv_46: vec![1u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-46: Exact bit output mismatch";
    test_reduce_gather_adv_47: vec![1u32; 0], vec![0u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-47: Exact bit output mismatch";
    test_reduce_gather_adv_48: vec![1u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-48: Exact bit output mismatch";
    test_reduce_gather_adv_49: vec![1u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-49: Exact bit output mismatch";
    test_reduce_gather_adv_50: vec![1u32; 0], vec![4294967295u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-50: Exact bit output mismatch";
    test_reduce_gather_adv_51: vec![1u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-51: Exact bit output mismatch";
    test_reduce_gather_adv_52: vec![1u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-52: Exact bit output mismatch";
    test_reduce_gather_adv_53: vec![1u32; 0], vec![2143289344u32; 0] => vec![], "FINDING-ADV-REDUCE-GATHER-53: Exact bit output mismatch";
    test_reduce_gather_adv_54: vec![1u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-54: Exact bit output mismatch";
    test_reduce_gather_adv_55: vec![1u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-55: Exact bit output mismatch";
    test_reduce_gather_adv_56: vec![1u32; 0], vec![0u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-56: Exact bit output mismatch";
    test_reduce_gather_adv_57: vec![1u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-57: Exact bit output mismatch";
    test_reduce_gather_adv_58: vec![1u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-58: Exact bit output mismatch";
    test_reduce_gather_adv_59: vec![1u32; 0], vec![4294967295u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-59: Exact bit output mismatch";
    test_reduce_gather_adv_60: vec![1u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-60: Exact bit output mismatch";
    test_reduce_gather_adv_61: vec![1u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-61: Exact bit output mismatch";
    test_reduce_gather_adv_62: vec![1u32; 0], vec![2143289344u32; 1] => vec![0], "FINDING-ADV-REDUCE-GATHER-62: Exact bit output mismatch";
    test_reduce_gather_adv_63: vec![1u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-63: Exact bit output mismatch";
    test_reduce_gather_adv_64: vec![1u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-64: Exact bit output mismatch";
    test_reduce_gather_adv_65: vec![1u32; 0], vec![0u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-65: Exact bit output mismatch";
    test_reduce_gather_adv_66: vec![1u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-66: Exact bit output mismatch";
    test_reduce_gather_adv_67: vec![1u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-67: Exact bit output mismatch";
    test_reduce_gather_adv_68: vec![1u32; 0], vec![4294967295u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-68: Exact bit output mismatch";
    test_reduce_gather_adv_69: vec![1u32; 0], vec![2143289344u32; 31] => vec![0; 31], "FINDING-ADV-REDUCE-GATHER-69: Exact bit output mismatch";
}
