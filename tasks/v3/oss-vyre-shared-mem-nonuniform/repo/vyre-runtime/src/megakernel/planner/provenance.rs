//! Runtime-owned megakernel provenance planning.

use std::time::Duration;

use super::MegakernelWorkItem;
use vyre_driver::{BackendError, DispatchConfig, VyreBackend};
use vyre_foundation::ir::Program;

/// Build per-region lineage bitsets through a caller-supplied provenance kernel.
pub fn build_scallop_lineage_with_program_and_scratch(
    backend: &dyn VyreBackend,
    program: &Program,
    work_items: &[MegakernelWorkItem],
    exchange_adj: &[u32],
    n_items: usize,
    state: &mut Vec<u32>,
    next: &mut Vec<u32>,
    changed: &mut [u32; 1],
    timeout: Duration,
) -> Result<Vec<u32>, BackendError> {
    if n_items == 0 {
        return Ok(Vec::new());
    }
    let cell_count = n_items.checked_mul(n_items).ok_or_else(|| {
        BackendError::new(
            "megakernel lineage matrix size overflowed usize. Fix: shard the work queue before provenance closure.",
        )
    })?;
    if exchange_adj.len() < cell_count {
        return Err(BackendError::new(format!(
            "megakernel lineage adjacency has {} cells but needs {cell_count}. Fix: keep scheduler exchange adjacency sized to item_count^2.",
            exchange_adj.len()
        )));
    }
    u32::try_from(n_items).map_err(|_| {
        BackendError::new(
            "megakernel lineage item count exceeds u32::MAX. Fix: shard the work queue before provenance closure.",
        )
    })?;
    state.clear();
    reserve_u32_staging(state, cell_count, "provenance state")?;
    state.resize(cell_count, 0);
    for (i, item) in work_items.iter().enumerate().take(n_items) {
        state[i * n_items + i] = 1u32 << (item.op_handle % 32);
    }
    next.clear();
    reserve_u32_staging(next, cell_count, "provenance next-state")?;
    next.resize(cell_count, 0);
    changed[0] = 0;

    let mut dispatch_config = DispatchConfig::default();
    dispatch_config.timeout = Some(timeout);
    let outputs = backend.dispatch_borrowed(
        program,
        &[
            bytemuck::cast_slice(state),
            bytemuck::cast_slice(next),
            bytemuck::cast_slice(changed),
            bytemuck::cast_slice(&exchange_adj[..cell_count]),
        ],
        &dispatch_config,
    )?;
    let closure = outputs.first().ok_or_else(|| {
        BackendError::new(
            "megakernel lineage closure returned no state output. Fix: keep provenance_state as the first read-write buffer.",
        )
    })?;
    let expected_bytes = cell_count.checked_mul(4).ok_or_else(|| {
        BackendError::new(
            "megakernel lineage byte size overflowed usize. Fix: shard the work queue before provenance closure.",
        )
    })?;
    if closure.len() < expected_bytes {
        return Err(BackendError::new(format!(
            "megakernel lineage closure returned {} bytes but needs {expected_bytes}. Fix: verify provenance_state readback sizing.",
            closure.len()
        )));
    }
    let mut lineage = Vec::new();
    reserve_u32_staging(&mut lineage, n_items, "provenance lineage")?;
    for i in 0..n_items {
        let byte_offset = (i * n_items + i) * 4;
        lineage.push(u32::from_le_bytes(
            closure[byte_offset..byte_offset + 4]
                .try_into()
                .map_err(|_| {
                    BackendError::new(
                        "megakernel lineage output was not aligned to u32 cells. Fix: keep provenance_state encoded as little-endian u32 words.",
                    )
                })?,
        ));
    }
    let nonempty = lineage.iter().filter(|&&v| v != 0).count();
    let nonempty_fraction = if lineage.is_empty() {
        0.0
    } else {
        nonempty as f64 / lineage.len() as f64
    };
    record_provenance(nonempty_fraction);
    Ok(lineage)
}

fn reserve_u32_staging(
    values: &mut Vec<u32>,
    capacity: usize,
    label: &'static str,
) -> Result<(), BackendError> {
    vyre_foundation::allocation::try_reserve_vec_to_capacity(values, capacity).map_err(|source| {
        BackendError::new(format!(
            "megakernel {label} reservation failed for {capacity} u32 cell(s): {source}. Fix: shard the work queue before provenance closure."
        ))
    })
}

#[cfg(feature = "self-substrate-adapters")]
fn record_provenance(nonempty_fraction: f64) {
    vyre_self_substrate::decision_telemetry::record_provenance(nonempty_fraction);
}

#[cfg(not(feature = "self-substrate-adapters"))]
fn record_provenance(_nonempty_fraction: f64) {}

/// Build per-region lineage bitsets through the optional self-substrate adapter.
#[cfg(feature = "self-substrate-adapters")]
pub fn build_scallop_lineage_with_scratch(
    backend: &dyn VyreBackend,
    work_items: &[MegakernelWorkItem],
    exchange_adj: &[u32],
    n_items: usize,
    state: &mut Vec<u32>,
    next: &mut Vec<u32>,
    changed: &mut [u32; 1],
    timeout: Duration,
) -> Result<Vec<u32>, BackendError> {
    const MAX_ITERS: u32 = 16;
    let n_u32 = u32::try_from(n_items).map_err(|_| {
        BackendError::new(
            "megakernel lineage item count exceeds u32::MAX. Fix: shard the work queue before provenance closure.",
        )
    })?;
    let program =
        vyre_self_substrate::scallop_provenance::build_provenance_program(n_u32, MAX_ITERS);
    build_scallop_lineage_with_program_and_scratch(
        backend,
        &program,
        work_items,
        exchange_adj,
        n_items,
        state,
        next,
        changed,
        timeout,
    )
}
