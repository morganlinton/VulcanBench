//! Backend-neutral pipeline-cache invalidation helpers.
//!
//! Backends provide their cache keys and lineage cells; this module owns
//! the shared causal-impact/provenance walk so the backend crates do not
//! depend on self-substrate implementation modules directly.

#[cfg(feature = "self-substrate-adapters")]
use vyre_self_substrate::do_calculus_change_impact::{
    predict_impact_via_into, DoCalculusImpactScratch,
};
#[cfg(feature = "self-substrate-adapters")]
use vyre_self_substrate::optimizer::dispatcher::{
    DispatchError as SelfSubstrateDispatchError, OptimizerDispatcher,
};
#[cfg(feature = "self-substrate-adapters")]
use vyre_self_substrate::scallop_provenance::provenance_closure_via_into;

/// Error raised by GPU-resident cache invalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheInvalidationError {
    message: String,
}

impl CacheInvalidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CacheInvalidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CacheInvalidationError {}

#[cfg(feature = "self-substrate-adapters")]
impl From<SelfSubstrateDispatchError> for CacheInvalidationError {
    fn from(error: SelfSubstrateDispatchError) -> Self {
        Self::new(error.to_string())
    }
}

/// Reusable scratch for shared pipeline-cache invalidation.
#[derive(Debug, Default)]
pub struct CacheInvalidationScratch {
    #[cfg(feature = "self-substrate-adapters")]
    impact: DoCalculusImpactScratch,
    #[cfg(feature = "self-substrate-adapters")]
    closure: Vec<u32>,
}

/// Compute a 0/1 impact mask for cache entries.
///
/// Production builds use the self-substrate implementation. Builds that
/// explicitly disable `self-substrate-adapters` fail loudly instead of running
/// a hidden reference cache-invalidation path.
pub fn impacted_entries_into(
    #[cfg(feature = "self-substrate-adapters")] dispatcher: &dyn OptimizerDispatcher,
    intervention_mask: &[u32],
    rule_adj: &[u32],
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    lineage_cells: &[u32],
    out: &mut Vec<u32>,
    _scratch: &mut CacheInvalidationScratch,
) -> Result<(), CacheInvalidationError> {
    out.clear();
    reserve_impact_mask(out, lineage_cells.len())?;
    out.resize(lineage_cells.len(), 0);

    #[cfg(not(feature = "self-substrate-adapters"))]
    {
        let _ = (
            intervention_mask,
            rule_adj,
            state,
            join_rules,
            n,
            max_iterations,
            lineage_cells,
            _scratch,
        );
        return Err(CacheInvalidationError::new(
            "vyre-driver cache invalidation requires the `self-substrate-adapters` feature. Fix: enable the feature; production builds must not run the reference cache-invalidation oracle.",
        ));
    }

    #[cfg(feature = "self-substrate-adapters")]
    {
        let n_us = n as usize;
        let Some(matrix_len) = n_us.checked_mul(n_us) else {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation n*n overflows usize for n={n}."
            )));
        };
        if intervention_mask.len() != n_us {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation requires intervention_mask.len() == n ({n_us}), got {}.",
                intervention_mask.len()
            )));
        }
        if rule_adj.len() != matrix_len {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation requires rule_adj.len() == n*n ({matrix_len}), got {}.",
                rule_adj.len()
            )));
        }
        if state.len() != matrix_len {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation requires state.len() == n*n ({matrix_len}), got {}.",
                state.len()
            )));
        }
        if join_rules.len() != matrix_len {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation requires join_rules.len() == n*n ({matrix_len}), got {}.",
                join_rules.len()
            )));
        }

        predict_impact_via_into(
            dispatcher,
            rule_adj,
            intervention_mask,
            n,
            &mut _scratch.impact,
        )
        .map_err(CacheInvalidationError::from)?;
        provenance_closure_via_into(
            dispatcher,
            state,
            join_rules,
            n,
            max_iterations,
            &mut _scratch.closure,
        )
        .map_err(CacheInvalidationError::from)?;

        let impacted_rules = _scratch.impact.impact_mask();
        let closure = &_scratch.closure;
        if impacted_rules.len() < n_us || closure.len() < matrix_len {
            return Err(CacheInvalidationError::new(format!(
                "Fix: cache invalidation GPU outputs were undersized: impact_mask={}, closure={}, required n={n_us}, matrix={matrix_len}.",
                impacted_rules.len(),
                closure.len()
            )));
        }

        for (entry_idx, &cell) in lineage_cells.iter().enumerate() {
            let cell = cell as usize;
            if cell >= n_us {
                continue;
            }
            let row_start = cell * n_us;
            let row = &closure[row_start..row_start + n_us];
            // A cell is impacted if any
            // provenance edge from it lands on an impacted node OR
            // if the cell itself is in the (transitively) impacted
            // set. Without the second clause, intervention seeds and
            // their direct rule-adjacency closure stay unmarked when
            // they have no outgoing provenance edges.
            let directly_impacted = impacted_rules.get(cell).is_some_and(|&v| v != 0);
            if directly_impacted
                || row
                    .iter()
                    .zip(impacted_rules.iter())
                    .any(|(&bitset, &impacted)| bitset != 0 && impacted != 0)
            {
                out[entry_idx] = 1;
            }
        }
        Ok(())
    }
}

/// Compute a 0/1 impact mask using temporary scratch.
#[must_use]
pub fn impacted_entries(
    #[cfg(feature = "self-substrate-adapters")] dispatcher: &dyn OptimizerDispatcher,
    intervention_mask: &[u32],
    rule_adj: &[u32],
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    max_iterations: u32,
    lineage_cells: &[u32],
) -> Result<Vec<u32>, CacheInvalidationError> {
    let mut out = reserved_impact_mask(lineage_cells.len())?;
    let mut scratch = CacheInvalidationScratch::default();
    impacted_entries_into(
        #[cfg(feature = "self-substrate-adapters")]
        dispatcher,
        intervention_mask,
        rule_adj,
        state,
        join_rules,
        n,
        max_iterations,
        lineage_cells,
        &mut out,
        &mut scratch,
    )?;
    Ok(out)
}

fn reserve_impact_mask(out: &mut Vec<u32>, len: usize) -> Result<(), CacheInvalidationError> {
    crate::allocation::try_reserve_vec_to_capacity(out, len).map_err(|error| {
        CacheInvalidationError::new(format!(
            "pipeline cache invalidation could not reserve {len} impact-mask slot(s): {error}. Fix: split lineage cells across smaller cache-invalidation shards."
        ))
    })
}

fn reserved_impact_mask(len: usize) -> Result<Vec<u32>, CacheInvalidationError> {
    let mut out = Vec::new();
    reserve_impact_mask(&mut out, len)?;
    Ok(out)
}

#[cfg(all(test, feature = "self-substrate-adapters"))]
mod tests {
    use super::*;
    use vyre_foundation::ir::Program;

    struct EchoStateDispatcher;

    impl OptimizerDispatcher for EchoStateDispatcher {
        fn dispatch(
            &self,
            _program: &Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, SelfSubstrateDispatchError> {
            Ok(vec![inputs.first().cloned().unwrap_or_default()])
        }
    }

    #[test]
    fn impact_mask_marks_lineage_intersection() {
        let dispatcher = EchoStateDispatcher;
        let n = 3;
        let mut rule_adj = vec![0u32; 9];
        rule_adj[0 * 3 + 1] = 1;
        let intervention_mask = vec![1, 0, 0];

        let mut state = vec![0u32; 9];
        state[1 * 3] = 1;
        let join_rules = vec![0u32; 9];
        let mask = impacted_entries(
            &dispatcher,
            &intervention_mask,
            &rule_adj,
            &state,
            &join_rules,
            n,
            16,
            &[1, 2],
        )
        .expect("Fix: test dispatcher must return one state output");
        assert_eq!(mask, vec![1, 0]);
    }

    #[test]
    fn malformed_dimensions_do_not_panic() {
        let dispatcher = EchoStateDispatcher;
        let err = impacted_entries(&dispatcher, &[1], &[], &[], &[], 32, 16, &[0, 1])
            .expect_err("malformed dimensions must fail loudly");
        assert!(
            err.to_string().contains("Fix:"),
            "cache invalidation dimension errors must be actionable"
        );
    }
}
