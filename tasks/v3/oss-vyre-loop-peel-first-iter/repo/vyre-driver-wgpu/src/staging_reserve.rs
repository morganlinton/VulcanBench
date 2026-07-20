//! Shared fallible staging reservation helpers for WGPU hot paths.
//!
//! Dispatch, readback, and multi-GPU orchestration all grow short-lived
//! staging collections under caller-controlled batch sizes. Centralizing the
//! reservation shape keeps the release path fallible, keeps error messages
//! actionable, and prevents each dispatch lane from inventing a slightly
//! different allocation policy.

use smallvec::{Array, SmallVec};
use vyre_driver::{reservation_policy::ReservationPolicy, BackendError};

pub(crate) fn reserve_vec<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    ReservationPolicy::new(context, fix).reserve_vec_additional(vec, additional, item)
}

pub(crate) fn reserve_vec_exact_for_len<T>(
    vec: &mut Vec<T>,
    target_len: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError> {
    ReservationPolicy::new(context, fix).reserve_vec_exact_for_len(vec, target_len, item)
}

pub(crate) fn reserve_smallvec<A>(
    vec: &mut SmallVec<A>,
    additional: usize,
    context: &'static str,
    item: &'static str,
    fix: &'static str,
) -> Result<(), BackendError>
where
    A: Array,
{
    ReservationPolicy::new(context, fix).reserve_smallvec_additional(vec, additional, item)
}

/// Backend-domain wrapper: reserve Vec capacity for dispatch recordings.
pub(crate) fn reserve_backend_vec<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    reserve_vec(
        vec,
        additional,
        context,
        "slot",
        "split the dispatch batch or reduce parallelism",
    )
}

/// Pipeline-domain wrapper: reserve Vec capacity for compiled output buffers.
pub(crate) fn reserve_pipeline_vec<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    reserve_vec(
        vec,
        additional,
        context,
        "buffer",
        "reduce output binding count or pipeline batch size",
    )
}

/// Multi-GPU domain wrapper: reserve Vec capacity for executor devices/outputs.
pub(crate) fn reserve_multi_gpu_vec<T>(
    vec: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    reserve_vec(
        vec,
        additional,
        context,
        "item",
        "reduce active GPU count or shard the workload",
    )
}

#[cfg(test)]
mod tests {
    use super::{reserve_smallvec, reserve_vec, reserve_vec_exact_for_len};
    use smallvec::SmallVec;

    #[test]
    fn generated_staging_reserve_helpers_grow_without_mutating_lengths() {
        let mut vec = vec![1u8, 2, 3];
        reserve_vec(
            &mut vec,
            17,
            "generated WGPU reservation test",
            "byte",
            "split the generated batch",
        )
        .expect("Fix: generated Vec reservation should succeed");
        assert_eq!(vec, vec![1u8, 2, 3]);
        assert!(vec.capacity() >= 20);

        let mut exact = vec![9u8];
        reserve_vec_exact_for_len(
            &mut exact,
            33,
            "generated WGPU exact reservation test",
            "slot",
            "split the generated batch",
        )
        .expect("Fix: generated exact Vec reservation should succeed");
        assert_eq!(exact, vec![9u8]);
        assert!(exact.capacity() >= 33);

        let mut small = SmallVec::<[u8; 2]>::new();
        small.push(7);
        reserve_smallvec(
            &mut small,
            19,
            "generated WGPU SmallVec reservation test",
            "item",
            "split the generated batch",
        )
        .expect("Fix: generated SmallVec reservation should succeed");
        assert_eq!(small.as_slice(), &[7]);
        assert!(small.capacity() >= 20);
    }
}
