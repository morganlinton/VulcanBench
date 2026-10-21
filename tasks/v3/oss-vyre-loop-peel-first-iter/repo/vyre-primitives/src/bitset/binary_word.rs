//! Shared per-word binary bitset Program builders.
//!
//! The public `and`, `or`, `xor`, and in-place variants keep distinct op ids
//! and buffer contracts, but the IR shape is intentionally centralized here so
//! new bitset binary ops do not fork the same load/op/store kernel body.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Supported per-word bitwise binary operators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitwiseBinaryOp {
    /// `lhs & rhs`.
    And,
    /// `lhs | rhs`.
    Or,
    /// `lhs ^ rhs`.
    Xor,
    /// `lhs & !rhs`.
    AndNot,
}

impl BitwiseBinaryOp {
    fn apply(self, lhs: Expr, rhs: Expr) -> Expr {
        match self {
            Self::And => Expr::bitand(lhs, rhs),
            Self::Or => Expr::bitor(lhs, rhs),
            Self::Xor => Expr::bitxor(lhs, rhs),
            Self::AndNot => Expr::bitand(lhs, Expr::bitnot(rhs)),
        }
    }
}

/// Build `out[w] = lhs[w] <op> rhs[w]` over packed u32 bitset words.
#[must_use]
pub(crate) fn binary_word_program(
    op_id: &'static str,
    lhs: &str,
    rhs: &str,
    out: &str,
    words: u32,
    op: BitwiseBinaryOp,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let value = op.apply(Expr::load(lhs, t.clone()), Expr::load(rhs, t.clone()));
    let body = vec![Node::store(out, t.clone(), value)];
    Program::wrapped(
        vec![
            BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

/// Build `target[w] = target[w] <op> operand[w]` over packed u32 words.
#[must_use]
pub(crate) fn in_place_binary_word_program(
    op_id: &'static str,
    target: &str,
    operand: &str,
    words: u32,
    op: BitwiseBinaryOp,
) -> Program {
    target_operand_word_program(
        op_id,
        target,
        operand,
        words,
        |target_value, operand_value| op.apply(target_value, operand_value),
    )
}

/// Build `target[w] = source[w]` over packed u32 words.
#[must_use]
pub(crate) fn copy_word_program(
    op_id: &'static str,
    target: &str,
    source: &str,
    words: u32,
) -> Program {
    target_operand_word_program(op_id, target, source, words, |_, source_value| source_value)
}

fn target_operand_word_program<F>(
    op_id: &'static str,
    target: &str,
    operand: &str,
    words: u32,
    value: F,
) -> Program
where
    F: Fn(Expr, Expr) -> Expr,
{
    let t = Expr::InvocationId { axis: 0 };
    let value = value(
        Expr::load(target, t.clone()),
        Expr::load(operand, t.clone()),
    );
    let body = vec![Node::store(target, t.clone(), value)];
    Program::wrapped(
        vec![
            BufferDecl::storage(target, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::storage(operand, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

macro_rules! define_bitwise_binary_op {
    (
        op_id: $op_id:literal,
        fn_name: $fn_name:ident,
        op_kind: $op_kind:ident,
        combine: $combine:expr,
        inventory_words: $inventory_words:expr,
        inventory_lhs: [$($inventory_lhs:expr),* $(,)?],
        inventory_rhs: [$($inventory_rhs:expr),* $(,)?],
        inventory_expected: [$($inventory_expected:expr),* $(,)?],
        single_lhs: [$($single_lhs:expr),* $(,)?],
        single_rhs: [$($single_rhs:expr),* $(,)?],
        single_expected: [$($single_expected:expr),* $(,)?],
        boundary_lhs: [$($boundary_lhs:expr),* $(,)?],
        boundary_rhs: [$($boundary_rhs:expr),* $(,)?],
        boundary_expected: [$($boundary_expected:expr),* $(,)?]
    ) => {
        use super::binary_word::{binary_word_program, BitwiseBinaryOp};
        use vyre_foundation::ir::Program;

        /// Canonical op id.
        pub const OP_ID: &str = $op_id;

        #[doc = concat!("Build a Program for `", stringify!($fn_name), "`.")]
        #[must_use]
        pub fn $fn_name(lhs: &str, rhs: &str, out: &str, words: u32) -> Program {
            binary_word_program(OP_ID, lhs, rhs, out, words, BitwiseBinaryOp::$op_kind)
        }

        /// CPU reference.
        #[must_use]
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref(lhs: &[u32], rhs: &[u32]) -> Vec<u32> {
            let mut out = Vec::new();
            match try_cpu_ref_into(lhs, rhs, &mut out) {
                Ok(()) => out,
                // A parity oracle that returns empty on failure makes the
                // GPU-vs-CPU assertion pass on empty==empty, silently masking a
                // divergence (Law 10 / Law 6). Fail loud; use try_cpu_ref_into.
                Err(error) => panic!("vyre-primitives {OP_ID} cpu_ref failed: {error}"),
            }
        }

        /// CPU reference into caller-owned storage.
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref_into(lhs: &[u32], rhs: &[u32], out: &mut Vec<u32>) {
            if let Err(error) = try_cpu_ref_into(lhs, rhs, out) {
                panic!("vyre-primitives {OP_ID} cpu_ref_into failed: {error}");
            }
        }

        /// Fallible CPU reference into caller-owned storage.
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn try_cpu_ref_into(
            lhs: &[u32],
            rhs: &[u32],
            out: &mut Vec<u32>,
        ) -> Result<(), String> {
            let combine = $combine;
            let len = lhs.len().min(rhs.len());
            out.clear();
            if len > out.capacity() {
                out.try_reserve(len - out.capacity()).map_err(|err| {
                    format!(
                        "bitwise binary CPU reference could not reserve {len} output words: {err}"
                    )
                })?;
            }
            out.extend(lhs.iter().zip(rhs.iter()).map(|(a, b)| combine(*a, *b)));
            Ok(())
        }

        #[cfg(feature = "inventory-registry")]
        inventory::submit! {
            crate::harness::OpEntry::new(
                OP_ID,
                || $fn_name("lhs", "rhs", "out", $inventory_words),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![
                        to_bytes(&[$($inventory_lhs),*]),
                        to_bytes(&[$($inventory_rhs),*]),
                        to_bytes(&[0; $inventory_words as usize]),
                    ]]
                }),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&[$($inventory_expected),*])]]
                }),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn sample_matches_expected() {
                assert_eq!(
                    cpu_ref(&[$($inventory_lhs),*], &[$($inventory_rhs),*]),
                    vec![$($inventory_expected),*]
                );
            }

            #[test]
            fn empty_bitset() {
                assert_eq!(cpu_ref(&[], &[]), Vec::<u32>::new());
            }

            #[test]
            fn single_word_case() {
                assert_eq!(
                    cpu_ref(&[$($single_lhs),*], &[$($single_rhs),*]),
                    vec![$($single_expected),*]
                );
            }

            #[test]
            fn cross_word_boundary() {
                assert_eq!(
                    cpu_ref(&[$($boundary_lhs),*], &[$($boundary_rhs),*]),
                    vec![$($boundary_expected),*]
                );
            }

            #[test]
            fn cpu_ref_into_replaces_existing_output() {
                let mut out = vec![0xDEAD_BEEF, 0xCAFE_BABE, 0x1234_5678];
                cpu_ref_into(&[$($inventory_lhs),*], &[$($inventory_rhs),*], &mut out);
                assert_eq!(out, vec![$($inventory_expected),*]);
            }

            #[test]
            fn cpu_ref_truncates_to_shorter_input() {
                let mut lhs = vec![$($boundary_lhs),*];
                lhs.push(0xFFFF_FFFF);
                assert_eq!(
                    cpu_ref(&lhs, &[$($boundary_rhs),*]),
                    vec![$($boundary_expected),*]
                );
            }

            #[test]
            fn try_cpu_ref_into_clears_stale_tail_without_reallocating() {
                let lhs = [
                    0x0123_4567,
                    0x89ab_cdef,
                    0xfedc_ba98,
                    0x7654_3210,
                ];
                let rhs = [0xffff_0000, 0x1357_9bdf, 0x2468_ace0];
                let mut out = Vec::with_capacity(9);
                out.extend_from_slice(&[0xffff_ffff; 9]);
                let cap = out.capacity();

                try_cpu_ref_into(&lhs, &rhs, &mut out).unwrap();

                let combine = $combine;
                assert_eq!(
                    out,
                    lhs.iter()
                        .zip(rhs.iter())
                        .map(|(a, b)| combine(*a, *b))
                        .collect::<Vec<_>>()
                );
                assert_eq!(out.len(), rhs.len());
                assert_eq!(out.capacity(), cap);
            }

            #[test]
            fn compatibility_wrappers_match_fallible_reference() {
                let lhs = [
                    0x0123_4567,
                    0x89ab_cdef,
                    0xfedc_ba98,
                    0x7654_3210,
                ];
                let rhs = [0xffff_0000, 0x1357_9bdf, 0x2468_ace0];
                let mut compat = Vec::with_capacity(8);
                let mut fallible = Vec::with_capacity(8);

                cpu_ref_into(&lhs, &rhs, &mut compat);
                try_cpu_ref_into(&lhs, &rhs, &mut fallible)
                    .expect("Fix: small bitwise binary CPU oracle must reserve");

                assert_eq!(cpu_ref(&lhs, &rhs), fallible);
                assert_eq!(compat, fallible);
            }

            #[test]
            fn production_cpu_ref_wrappers_have_no_raw_panic_path() {
                let production = include_str!("binary_word.rs")
                    .split("#[cfg(test)]")
                    .next()
                    .expect("Fix: binary_word.rs must contain production section");

                assert!(
                    !production.contains(".expect(") && !production.contains(".unwrap("),
                    "Fix: shared bitwise binary CPU parity wrappers must not panic in production."
                );
            }

            #[test]
            fn generated_cpu_ref_matches_scalar_reference_matrix() {
                let combine = $combine;
                let mut seed = 0x243f_6a88_u32;
                let mut lhs = Vec::new();
                let mut rhs = Vec::new();
                for case in 0..257_u32 {
                    seed = seed
                        .wrapping_mul(0x9e37_79b9)
                        .rotate_left((case % 31) + 1)
                        ^ case.wrapping_mul(0x85eb_ca6b);
                    lhs.push(seed ^ case.rotate_left((case % 17) + 1));
                    if case % 5 != 0 {
                        rhs.push(seed.rotate_right((case % 19) + 1));
                    }
                }

                let mut out = Vec::new();
                try_cpu_ref_into(&lhs, &rhs, &mut out).unwrap();

                let expected = lhs
                    .iter()
                    .zip(rhs.iter())
                    .map(|(a, b)| combine(*a, *b))
                    .collect::<Vec<_>>();
                assert_eq!(out, expected);
                assert_eq!(cpu_ref(&lhs, &rhs), expected);
            }
        }
    };
}

pub(crate) use define_bitwise_binary_op;

macro_rules! define_bitwise_in_place_op {
    (
        op_id: $op_id:literal,
        fn_name: $fn_name:ident,
        op_kind: $op_kind:ident,
        combine: $combine:expr,
        inventory_words: $inventory_words:expr,
        inventory_target: [$($inventory_target:expr),* $(,)?],
        inventory_operand: [$($inventory_operand:expr),* $(,)?],
        inventory_expected: [$($inventory_expected:expr),* $(,)?],
        cases: {
            $(
                $case_name:ident: {
                    target: [$($case_target:expr),* $(,)?],
                    operand: [$($case_operand:expr),* $(,)?],
                    expected: [$($case_expected:expr),* $(,)?]
                }
            ),* $(,)?
        }
    ) => {
        use super::binary_word::{in_place_binary_word_program, BitwiseBinaryOp};
        use vyre_foundation::ir::Program;

        /// Canonical op id.
        pub const OP_ID: &str = $op_id;

        #[doc = concat!("Build an in-place bitset Program for `", stringify!($fn_name), "`.")]
        #[must_use]
        pub fn $fn_name(target: &str, operand: &str, words: u32) -> Program {
            in_place_binary_word_program(OP_ID, target, operand, words, BitwiseBinaryOp::$op_kind)
        }

        /// CPU reference. Mutates `target` in place.
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref(target: &mut [u32], operand: &[u32]) {
            let combine = $combine;
            let n = target.len().min(operand.len());
            for i in 0..n {
                target[i] = combine(target[i], operand[i]);
            }
        }

        #[cfg(feature = "inventory-registry")]
        inventory::submit! {
            crate::harness::OpEntry::new(
                OP_ID,
                || $fn_name("target", "operand", $inventory_words),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![
                        to_bytes(&[$($inventory_target),*]),
                        to_bytes(&[$($inventory_operand),*]),
                    ]]
                }),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&[$($inventory_expected),*])]]
                }),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn inventory_case_matches_cpu_reference() {
                let mut target = vec![$($inventory_target),*];
                cpu_ref(&mut target, &[$($inventory_operand),*]);
                assert_eq!(target, vec![$($inventory_expected),*]);
            }

            $(
                #[test]
                fn $case_name() {
                    let mut target = vec![$($case_target),*];
                    cpu_ref(&mut target, &[$($case_operand),*]);
                    assert_eq!(target, vec![$($case_expected),*]);
                }
            )*

            #[test]
            fn empty_operand_leaves_target_unchanged() {
                let mut target = vec![$($inventory_target),*];
                let expected = target.clone();
                cpu_ref(&mut target, &[]);
                assert_eq!(target, expected);
            }

            #[test]
            fn empty_target_is_noop() {
                let mut target = Vec::<u32>::new();
                cpu_ref(&mut target, &[$($inventory_operand),*]);
                assert!(target.is_empty());
            }

            #[test]
            fn shorter_operand_preserves_tail() {
                let mut target = vec![$($inventory_target),*, 0xCAFE_BABE];
                cpu_ref(&mut target, &[$($inventory_operand),*]);
                assert_eq!(target, vec![$($inventory_expected),*, 0xCAFE_BABE]);
            }

            #[test]
            fn generated_in_place_matches_scalar_reference_matrix() {
                let combine = $combine;
                let mut seed = 0x6a09_e667_u32;
                let mut target = Vec::new();
                let mut operand = Vec::new();
                for case in 0..193_u32 {
                    seed = seed
                        .wrapping_mul(0x85eb_ca6b)
                        .rotate_left((case % 29) + 1)
                        ^ case.wrapping_mul(0xc2b2_ae35);
                    target.push(seed ^ case.rotate_left((case % 23) + 1));
                    if case % 7 != 0 {
                        operand.push(seed.rotate_right((case % 17) + 1));
                    }
                }
                let mut expected = target.clone();
                for (value, rhs) in expected.iter_mut().zip(operand.iter()) {
                    *value = combine(*value, *rhs);
                }

                cpu_ref(&mut target, &operand);

                assert_eq!(target, expected);
            }
        }
    };
}

pub(crate) use define_bitwise_in_place_op;
