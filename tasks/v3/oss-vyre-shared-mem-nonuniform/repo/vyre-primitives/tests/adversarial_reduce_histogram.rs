//! Generated adversarial contract tests for vyre-primitives.

// Adversarial tests for reduce::histogram

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

use vyre_primitives::reduce::histogram::*;

fn cpu_ref(input: &[u32], num_bins: u32) -> Vec<u32> {
    let mut out = vec![0u32; num_bins as usize];
    for bin in input.iter().copied() {
        if let Some(slot) = out.get_mut(bin as usize) {
            *slot = (*slot).wrapping_add(1);
        }
    }
    out
}

adversarial_vec_u32_cases! {
    test_reduce_histogram_adv_0: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-0: Exact bit output mismatch";
    test_reduce_histogram_adv_1: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-1: Exact bit output mismatch";
    test_reduce_histogram_adv_2: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-2: Exact bit output mismatch";
    test_reduce_histogram_adv_3: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-3: Exact bit output mismatch";
    test_reduce_histogram_adv_4: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-4: Exact bit output mismatch";
    test_reduce_histogram_adv_5: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-5: Exact bit output mismatch";
    test_reduce_histogram_adv_6: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-6: Exact bit output mismatch";
    test_reduce_histogram_adv_7: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-7: Exact bit output mismatch";
    test_reduce_histogram_adv_8: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-8: Exact bit output mismatch";
    test_reduce_histogram_adv_9: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-9: Exact bit output mismatch";
    test_reduce_histogram_adv_10: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-10: Exact bit output mismatch";
    test_reduce_histogram_adv_11: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-11: Exact bit output mismatch";
    test_reduce_histogram_adv_12: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-12: Exact bit output mismatch";
    test_reduce_histogram_adv_13: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-13: Exact bit output mismatch";
    test_reduce_histogram_adv_14: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-14: Exact bit output mismatch";
    test_reduce_histogram_adv_15: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-15: Exact bit output mismatch";
    test_reduce_histogram_adv_16: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-16: Exact bit output mismatch";
    test_reduce_histogram_adv_17: vec![0u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-17: Exact bit output mismatch";
    test_reduce_histogram_adv_18: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-18: Exact bit output mismatch";
    test_reduce_histogram_adv_19: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-19: Exact bit output mismatch";
    test_reduce_histogram_adv_20: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-20: Exact bit output mismatch";
    test_reduce_histogram_adv_21: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-21: Exact bit output mismatch";
    test_reduce_histogram_adv_22: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-22: Exact bit output mismatch";
    test_reduce_histogram_adv_23: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-23: Exact bit output mismatch";
    test_reduce_histogram_adv_24: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-24: Exact bit output mismatch";
    test_reduce_histogram_adv_25: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-25: Exact bit output mismatch";
    test_reduce_histogram_adv_26: vec![0u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-26: Exact bit output mismatch";
    test_reduce_histogram_adv_27: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-27: Exact bit output mismatch";
    test_reduce_histogram_adv_28: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-28: Exact bit output mismatch";
    test_reduce_histogram_adv_29: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-29: Exact bit output mismatch";
    test_reduce_histogram_adv_30: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-30: Exact bit output mismatch";
    test_reduce_histogram_adv_31: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-31: Exact bit output mismatch";
    test_reduce_histogram_adv_32: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-32: Exact bit output mismatch";
    test_reduce_histogram_adv_33: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-33: Exact bit output mismatch";
    test_reduce_histogram_adv_34: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-34: Exact bit output mismatch";
    test_reduce_histogram_adv_35: vec![0u32; 0], 32u32 => vec![0; 32], "FINDING-ADV-REDUCE-HISTOGRAM-35: Exact bit output mismatch";
    test_reduce_histogram_adv_36: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-36: Exact bit output mismatch";
    test_reduce_histogram_adv_37: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-37: Exact bit output mismatch";
    test_reduce_histogram_adv_38: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-38: Exact bit output mismatch";
    test_reduce_histogram_adv_39: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-39: Exact bit output mismatch";
    test_reduce_histogram_adv_40: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-40: Exact bit output mismatch";
    test_reduce_histogram_adv_41: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-41: Exact bit output mismatch";
    test_reduce_histogram_adv_42: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-42: Exact bit output mismatch";
    test_reduce_histogram_adv_43: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-43: Exact bit output mismatch";
    test_reduce_histogram_adv_44: vec![0u32; 0], 1024u32 => vec![0; 1024], "FINDING-ADV-REDUCE-HISTOGRAM-44: Exact bit output mismatch";
    test_reduce_histogram_adv_45: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-45: Exact bit output mismatch";
    test_reduce_histogram_adv_46: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-46: Exact bit output mismatch";
    test_reduce_histogram_adv_47: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-47: Exact bit output mismatch";
    test_reduce_histogram_adv_48: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-48: Exact bit output mismatch";
    test_reduce_histogram_adv_49: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-49: Exact bit output mismatch";
    test_reduce_histogram_adv_50: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-50: Exact bit output mismatch";
    test_reduce_histogram_adv_51: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-51: Exact bit output mismatch";
    test_reduce_histogram_adv_52: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-52: Exact bit output mismatch";
    test_reduce_histogram_adv_53: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-53: Exact bit output mismatch";
    test_reduce_histogram_adv_54: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-54: Exact bit output mismatch";
    test_reduce_histogram_adv_55: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-55: Exact bit output mismatch";
    test_reduce_histogram_adv_56: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-56: Exact bit output mismatch";
    test_reduce_histogram_adv_57: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-57: Exact bit output mismatch";
    test_reduce_histogram_adv_58: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-58: Exact bit output mismatch";
    test_reduce_histogram_adv_59: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-59: Exact bit output mismatch";
    test_reduce_histogram_adv_60: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-60: Exact bit output mismatch";
    test_reduce_histogram_adv_61: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-61: Exact bit output mismatch";
    test_reduce_histogram_adv_62: vec![1u32; 0], 1u32 => vec![0], "FINDING-ADV-REDUCE-HISTOGRAM-62: Exact bit output mismatch";
    test_reduce_histogram_adv_63: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-63: Exact bit output mismatch";
    test_reduce_histogram_adv_64: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-64: Exact bit output mismatch";
    test_reduce_histogram_adv_65: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-65: Exact bit output mismatch";
    test_reduce_histogram_adv_66: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-66: Exact bit output mismatch";
    test_reduce_histogram_adv_67: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-67: Exact bit output mismatch";
    test_reduce_histogram_adv_68: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-68: Exact bit output mismatch";
    test_reduce_histogram_adv_69: vec![1u32; 0], 31u32 => vec![0; 31], "FINDING-ADV-REDUCE-HISTOGRAM-69: Exact bit output mismatch";
}
