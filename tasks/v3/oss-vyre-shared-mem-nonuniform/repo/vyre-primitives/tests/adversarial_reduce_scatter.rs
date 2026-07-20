//! Generated adversarial contract tests for vyre-primitives.

// Adversarial tests for reduce::scatter

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

use vyre_primitives::reduce::scatter::*;

fn cpu_ref(src: &[u32], indices: &[u32], dst_len: usize) -> Vec<u32> {
    let mut dst = vec![0; dst_len];
    for (index, target) in indices.iter().copied().enumerate() {
        let target = target as usize;
        if target < dst.len() {
            if let Some(value) = src.get(index).copied() {
                dst[target] = value;
            }
        }
    }
    dst
}

adversarial_binary_vec_usize_cases! {
    test_reduce_scatter_adv_0: vec![0u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-0: Exact bit output mismatch";
    test_reduce_scatter_adv_1: vec![0u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-1: Exact bit output mismatch";
    test_reduce_scatter_adv_2: vec![0u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-2: Exact bit output mismatch";
    test_reduce_scatter_adv_3: vec![0u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-3: Exact bit output mismatch";
    test_reduce_scatter_adv_4: vec![0u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-4: Exact bit output mismatch";
    test_reduce_scatter_adv_5: vec![0u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-5: Exact bit output mismatch";
    test_reduce_scatter_adv_6: vec![0u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-6: Exact bit output mismatch";
    test_reduce_scatter_adv_7: vec![0u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-7: Exact bit output mismatch";
    test_reduce_scatter_adv_8: vec![0u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-8: Exact bit output mismatch";
    test_reduce_scatter_adv_9: vec![0u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-9: Exact bit output mismatch";
    test_reduce_scatter_adv_10: vec![0u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-10: Exact bit output mismatch";
    test_reduce_scatter_adv_11: vec![0u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-11: Exact bit output mismatch";
    test_reduce_scatter_adv_12: vec![0u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-12: Exact bit output mismatch";
    test_reduce_scatter_adv_13: vec![0u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-13: Exact bit output mismatch";
    test_reduce_scatter_adv_14: vec![0u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-14: Exact bit output mismatch";
    test_reduce_scatter_adv_15: vec![0u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-15: Exact bit output mismatch";
    test_reduce_scatter_adv_16: vec![0u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-16: Exact bit output mismatch";
    test_reduce_scatter_adv_17: vec![0u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-17: Exact bit output mismatch";
    test_reduce_scatter_adv_18: vec![0u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-18: Exact bit output mismatch";
    test_reduce_scatter_adv_19: vec![0u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-19: Exact bit output mismatch";
    test_reduce_scatter_adv_20: vec![0u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-20: Exact bit output mismatch";
    test_reduce_scatter_adv_21: vec![0u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-21: Exact bit output mismatch";
    test_reduce_scatter_adv_22: vec![0u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-22: Exact bit output mismatch";
    test_reduce_scatter_adv_23: vec![0u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-23: Exact bit output mismatch";
    test_reduce_scatter_adv_24: vec![0u32; 0], vec![2143289344u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-24: Exact bit output mismatch";
    test_reduce_scatter_adv_25: vec![0u32; 0], vec![2143289344u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-25: Exact bit output mismatch";
    test_reduce_scatter_adv_26: vec![0u32; 0], vec![2143289344u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-26: Exact bit output mismatch";
    test_reduce_scatter_adv_27: vec![0u32; 0], vec![0u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-27: Exact bit output mismatch";
    test_reduce_scatter_adv_28: vec![0u32; 0], vec![0u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-28: Exact bit output mismatch";
    test_reduce_scatter_adv_29: vec![0u32; 0], vec![0u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-29: Exact bit output mismatch";
    test_reduce_scatter_adv_30: vec![0u32; 0], vec![4294967295u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-30: Exact bit output mismatch";
    test_reduce_scatter_adv_31: vec![0u32; 0], vec![4294967295u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-31: Exact bit output mismatch";
    test_reduce_scatter_adv_32: vec![0u32; 0], vec![4294967295u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-32: Exact bit output mismatch";
    test_reduce_scatter_adv_33: vec![0u32; 0], vec![2143289344u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-33: Exact bit output mismatch";
    test_reduce_scatter_adv_34: vec![0u32; 0], vec![2143289344u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-34: Exact bit output mismatch";
    test_reduce_scatter_adv_35: vec![0u32; 0], vec![2143289344u32; 32], 32usize => vec![0; 32], "FINDING-ADV-REDUCE-SCATTER-35: Exact bit output mismatch";
    test_reduce_scatter_adv_36: vec![0u32; 0], vec![0u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-36: Exact bit output mismatch";
    test_reduce_scatter_adv_37: vec![0u32; 0], vec![0u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-37: Exact bit output mismatch";
    test_reduce_scatter_adv_38: vec![0u32; 0], vec![0u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-38: Exact bit output mismatch";
    test_reduce_scatter_adv_39: vec![0u32; 0], vec![4294967295u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-39: Exact bit output mismatch";
    test_reduce_scatter_adv_40: vec![0u32; 0], vec![4294967295u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-40: Exact bit output mismatch";
    test_reduce_scatter_adv_41: vec![0u32; 0], vec![4294967295u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-41: Exact bit output mismatch";
    test_reduce_scatter_adv_42: vec![0u32; 0], vec![2143289344u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-42: Exact bit output mismatch";
    test_reduce_scatter_adv_43: vec![0u32; 0], vec![2143289344u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-43: Exact bit output mismatch";
    test_reduce_scatter_adv_44: vec![0u32; 0], vec![2143289344u32; 1024], 1024usize => vec![0; 1024], "FINDING-ADV-REDUCE-SCATTER-44: Exact bit output mismatch";
    test_reduce_scatter_adv_45: vec![1u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-45: Exact bit output mismatch";
    test_reduce_scatter_adv_46: vec![1u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-46: Exact bit output mismatch";
    test_reduce_scatter_adv_47: vec![1u32; 0], vec![0u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-47: Exact bit output mismatch";
    test_reduce_scatter_adv_48: vec![1u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-48: Exact bit output mismatch";
    test_reduce_scatter_adv_49: vec![1u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-49: Exact bit output mismatch";
    test_reduce_scatter_adv_50: vec![1u32; 0], vec![4294967295u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-50: Exact bit output mismatch";
    test_reduce_scatter_adv_51: vec![1u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-51: Exact bit output mismatch";
    test_reduce_scatter_adv_52: vec![1u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-52: Exact bit output mismatch";
    test_reduce_scatter_adv_53: vec![1u32; 0], vec![2143289344u32; 0], 0usize => vec![], "FINDING-ADV-REDUCE-SCATTER-53: Exact bit output mismatch";
    test_reduce_scatter_adv_54: vec![1u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-54: Exact bit output mismatch";
    test_reduce_scatter_adv_55: vec![1u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-55: Exact bit output mismatch";
    test_reduce_scatter_adv_56: vec![1u32; 0], vec![0u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-56: Exact bit output mismatch";
    test_reduce_scatter_adv_57: vec![1u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-57: Exact bit output mismatch";
    test_reduce_scatter_adv_58: vec![1u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-58: Exact bit output mismatch";
    test_reduce_scatter_adv_59: vec![1u32; 0], vec![4294967295u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-59: Exact bit output mismatch";
    test_reduce_scatter_adv_60: vec![1u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-60: Exact bit output mismatch";
    test_reduce_scatter_adv_61: vec![1u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-61: Exact bit output mismatch";
    test_reduce_scatter_adv_62: vec![1u32; 0], vec![2143289344u32; 1], 1usize => vec![0], "FINDING-ADV-REDUCE-SCATTER-62: Exact bit output mismatch";
    test_reduce_scatter_adv_63: vec![1u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-63: Exact bit output mismatch";
    test_reduce_scatter_adv_64: vec![1u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-64: Exact bit output mismatch";
    test_reduce_scatter_adv_65: vec![1u32; 0], vec![0u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-65: Exact bit output mismatch";
    test_reduce_scatter_adv_66: vec![1u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-66: Exact bit output mismatch";
    test_reduce_scatter_adv_67: vec![1u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-67: Exact bit output mismatch";
    test_reduce_scatter_adv_68: vec![1u32; 0], vec![4294967295u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-68: Exact bit output mismatch";
    test_reduce_scatter_adv_69: vec![1u32; 0], vec![2143289344u32; 31], 31usize => vec![0; 31], "FINDING-ADV-REDUCE-SCATTER-69: Exact bit output mismatch";
}
