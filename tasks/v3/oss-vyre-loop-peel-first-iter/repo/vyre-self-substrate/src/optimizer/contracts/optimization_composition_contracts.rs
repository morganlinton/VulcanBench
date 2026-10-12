//! Composition contracts for optimization passes.

use std::collections::BTreeSet;
use std::fmt::Debug;

use super::optimization_registry::OptimizationRegistry;

/// Mathematical law an optimization pass must satisfy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptimizationCompositionLaw {
    /// Running a pass twice must produce the same result as running it once.
    Idempotent {
        /// Registered pass id.
        pass_id: &'static str,
        /// Invariant that makes idempotence valid.
        invariant: &'static str,
        /// Test or benchmark that owns the law.
        benchmark: &'static str,
    },
    /// Two passes may be reordered without changing the result.
    Commutative {
        /// First registered pass id.
        left_id: &'static str,
        /// Second registered pass id.
        right_id: &'static str,
        /// Invariant that makes commutativity valid.
        invariant: &'static str,
        /// Test or benchmark that owns the law.
        benchmark: &'static str,
    },
}

/// Release-path composition laws that must remain discoverable and validated.
pub const RELEASE_COMPOSITION_LAWS: &[OptimizationCompositionLaw] = &[
    OptimizationCompositionLaw::Idempotent {
        pass_id: "self.dce-resident",
        invariant: "dead code elimination reaches a fixed point",
        benchmark: "self_optimizer_e2e",
    },
    OptimizationCompositionLaw::Idempotent {
        pass_id: "self.const-fold",
        invariant: "constant expression folding reaches a fixed point",
        benchmark: "self_optimizer_const_fold_extended",
    },
    OptimizationCompositionLaw::Idempotent {
        pass_id: "dataflow.graph-normalization",
        invariant: "canonical graph layout normalizes equivalent edge order once",
        benchmark: "fixed_point_graph",
    },
    OptimizationCompositionLaw::Idempotent {
        pass_id: "cuda.compact-read-ranges",
        invariant: "read range compaction is canonical for a fixed output layout",
        benchmark: "resident_dispatch_contracts",
    },
    OptimizationCompositionLaw::Commutative {
        left_id: "cuda.compact-read-ranges",
        right_id: "cuda.borrowed-output-slots",
        invariant: "readback range choice is independent of caller-owned slot allocation",
        benchmark: "resident_dispatch_contracts",
    },
    OptimizationCompositionLaw::Commutative {
        left_id: "dataflow.bitset-tail-validation",
        right_id: "primitive.bitset-and",
        invariant: "tail-bit clearing commutes with masked bitset intersection",
        benchmark: "bitset_primitives_gpu_parity",
    },
];

/// Validate release composition laws against the optimization registry.
pub fn validate_release_composition_laws(registry: &OptimizationRegistry) -> Result<(), String> {
    let mut seen = BTreeSet::new();

    for law in RELEASE_COMPOSITION_LAWS {
        validate_law_metadata(*law)?;
        match *law {
            OptimizationCompositionLaw::Idempotent { pass_id, .. } => {
                require_registered(registry, pass_id)?;
                let key = format!("idempotent:{pass_id}");
                if !seen.insert(key) {
                    return Err(format!(
                        "duplicate idempotence law for `{pass_id}`. Fix: keep one owning contract per law."
                    ));
                }
            }
            OptimizationCompositionLaw::Commutative {
                left_id, right_id, ..
            } => {
                require_registered(registry, left_id)?;
                require_registered(registry, right_id)?;
                if left_id == right_id {
                    return Err(format!(
                        "commutativity law repeats `{left_id}` on both sides. Fix: use an idempotence law instead."
                    ));
                }
                let (a, b) = if left_id <= right_id {
                    (left_id, right_id)
                } else {
                    (right_id, left_id)
                };
                let key = format!("commutative:{a}:{b}");
                if !seen.insert(key) {
                    return Err(format!(
                        "duplicate commutativity law for `{left_id}` and `{right_id}`. Fix: keep one owning contract per pair."
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Verify idempotence for a concrete pass function and input.
pub fn verify_idempotent<T, F>(input: T, mut pass: F) -> Result<(), String>
where
    T: Clone + Debug + Eq,
    F: FnMut(T) -> T,
{
    let once = pass(input);
    let twice = pass(once.clone());
    if once != twice {
        return Err(format!(
            "idempotence contract failed. once={once:?}, twice={twice:?}"
        ));
    }
    Ok(())
}

/// Verify commutativity for two concrete pass functions and input.
pub fn verify_commutative<T, F, G>(input: T, mut left: F, mut right: G) -> Result<(), String>
where
    T: Clone + Debug + Eq,
    F: FnMut(T) -> T,
    G: FnMut(T) -> T,
{
    let left_then_right = right(left(input.clone()));
    let right_then_left = left(right(input));
    if left_then_right != right_then_left {
        return Err(format!(
            "commutativity contract failed. left_then_right={left_then_right:?}, right_then_left={right_then_left:?}"
        ));
    }
    Ok(())
}

fn validate_law_metadata(law: OptimizationCompositionLaw) -> Result<(), String> {
    let fields: &[(&str, &str)] = match law {
        OptimizationCompositionLaw::Idempotent {
            pass_id,
            invariant,
            benchmark,
        } => &[
            ("pass_id", pass_id),
            ("invariant", invariant),
            ("benchmark", benchmark),
        ],
        OptimizationCompositionLaw::Commutative {
            left_id,
            right_id,
            invariant,
            benchmark,
        } => &[
            ("left_id", left_id),
            ("right_id", right_id),
            ("invariant", invariant),
            ("benchmark", benchmark),
        ],
    };

    for (field, value) in fields {
        if value.trim().is_empty() {
            return Err(format!(
                "composition law has empty {field}. Fix: every law needs registered pass ids, invariant, and benchmark."
            ));
        }
    }
    Ok(())
}

fn require_registered(registry: &OptimizationRegistry, pass_id: &str) -> Result<(), String> {
    registry.get(pass_id).ok_or_else(|| {
        format!(
            "composition law references unknown pass `{pass_id}`. Fix: register the pass before adding composition laws."
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_composition_laws_reference_registered_passes() {
        let registry = OptimizationRegistry::with_release_builtins();

        validate_release_composition_laws(&registry)
            .expect("Fix: release composition laws must reference registered passes");
    }

    #[test]
    fn idempotence_harness_accepts_fixed_point_pass() {
        verify_idempotent(vec![3, 1, 3, 2], |mut values| {
            values.sort_unstable();
            values.dedup();
            values
        })
        .expect("Fix: sort and dedup reaches fixed point");
    }

    #[test]
    fn idempotence_harness_rejects_non_fixed_point_pass() {
        let err =
            verify_idempotent(1_u32, |value| value + 1).expect_err("increment is not idempotent");

        assert!(err.contains("idempotence contract failed"), "{err}");
    }

    #[test]
    fn commutativity_harness_accepts_independent_bit_masks() {
        verify_commutative(0b1111_u8, |value| value & 0b1101, |value| value & 0b1011)
            .expect("Fix: independent bit masks commute");
    }

    #[test]
    fn commutativity_harness_rejects_order_dependent_passes() {
        let err = verify_commutative(2_u32, |value| value * 2, |value| value + 1)
            .expect_err("multiply and add are order-dependent");

        assert!(err.contains("commutativity contract failed"), "{err}");
    }
}
