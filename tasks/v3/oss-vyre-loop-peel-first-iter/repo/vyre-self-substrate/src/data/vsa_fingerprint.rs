//! VSA-based op cache key via #13 hypervector primitives (#29).
//!
//! Fingerprints a Program by binding op-kind, buffer-signature, and
//! region-shape into one 10K-dim hypervector. Approximate-match cache
//! returns the same fingerprint for two semantically-equivalent
//! Region trees with reordered children  -  beats byte-equal hashing.

use crate::dispatch_buffers::{
    ceil_div_u32, decode_u32_output_exact, ensure_input_slots, write_u32_slice_le_bytes,
    write_zero_bytes,
};
use crate::optimizer::dispatcher::{DispatchError, OptimizerDispatcher};
#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::hash::hypervector::xor_bind_cpu;
use vyre_primitives::hash::hypervector::{hamming_similarity, hypervector_xor_bind};

/// Caller-owned GPU dispatch scratch for VSA fingerprint XOR binding.
#[derive(Debug, Default)]
pub struct VsaFingerprintGpuScratch {
    inputs: Vec<Vec<u8>>,
    bound1: Vec<u32>,
}

/// Build a stable VSA cache fingerprint directly from a vyre Program.
///
/// The canonical program fingerprint is 32 bytes; this converts it into
/// eight little-endian `u32` hypervector lanes so approximate lookup can
/// share the same cache representation as manually supplied component
/// fingerprints.
#[must_use]
pub fn vsa_fingerprint(program: &vyre_foundation::ir::Program) -> Vec<u32> {
    vsa_fingerprint_words(program).to_vec()
}

/// Build the stable eight-lane VSA cache fingerprint without heap allocation.
#[must_use]
pub fn vsa_fingerprint_words(program: &vyre_foundation::ir::Program) -> [u32; 8] {
    use crate::observability::{bump, vsa_fingerprint_calls};
    bump(&vsa_fingerprint_calls);
    let fingerprint = program.fingerprint();
    vyre_primitives::wire::decode_u32x8_le_bytes(&fingerprint)
}

/// Fingerprint a Program from a (kind, signature, region) triple.
/// Caller supplies pre-computed hypervectors for each component.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_fingerprint(kind_hv: &[u32], signature_hv: &[u32], region_hv: &[u32]) -> Vec<u32> {
    let bound1 = xor_bind_cpu(kind_hv, signature_hv);
    xor_bind_cpu(&bound1, region_hv)
}

/// Fingerprint a Program component triple through GPU-dispatchable XOR binding primitives.
///
/// Unlike [`reference_fingerprint`], this production dispatch path rejects mismatched dimensions instead of
/// silently truncating. Cache fingerprints must have one unambiguous dimensionality across all
/// components.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when dimensions are zero or mismatched, dispatch fails, or
/// a backend returns a truncated output buffer.
pub fn fingerprint_via(
    dispatcher: &dyn OptimizerDispatcher,
    kind_hv: &[u32],
    signature_hv: &[u32],
    region_hv: &[u32],
) -> Result<Vec<u32>, DispatchError> {
    let mut out = Vec::new();
    fingerprint_via_into(dispatcher, kind_hv, signature_hv, region_hv, &mut out)?;
    Ok(out)
}

/// Fingerprint a Program component triple through GPU-dispatchable XOR binding
/// primitives into caller-owned output storage.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when dimensions are zero or mismatched,
/// dispatch fails, or a backend returns malformed output.
pub fn fingerprint_via_into(
    dispatcher: &dyn OptimizerDispatcher,
    kind_hv: &[u32],
    signature_hv: &[u32],
    region_hv: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let mut scratch = VsaFingerprintGpuScratch::default();
    fingerprint_via_with_scratch_into(
        dispatcher,
        kind_hv,
        signature_hv,
        region_hv,
        &mut scratch,
        out,
    )
}

/// Fingerprint a Program component triple through GPU-dispatchable XOR binding
/// primitives into caller-owned dispatch and output storage.
///
/// # Errors
///
/// Returns [`DispatchError::BadInputs`] when dimensions are zero or mismatched,
/// dispatch fails, or a backend returns malformed output.
pub fn fingerprint_via_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    kind_hv: &[u32],
    signature_hv: &[u32],
    region_hv: &[u32],
    scratch: &mut VsaFingerprintGpuScratch,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let dim_words = validate_fingerprint_dims(kind_hv, signature_hv, region_hv)?;
    dispatch_xor_bind_with_scratch_into(
        dispatcher,
        kind_hv,
        signature_hv,
        dim_words,
        &mut scratch.inputs,
        &mut scratch.bound1,
    )?;
    dispatch_xor_bind_with_scratch_into(
        dispatcher,
        &scratch.bound1,
        region_hv,
        dim_words,
        &mut scratch.inputs,
        out,
    )
}

fn validate_fingerprint_dims(
    kind_hv: &[u32],
    signature_hv: &[u32],
    region_hv: &[u32],
) -> Result<u32, DispatchError> {
    if kind_hv.is_empty() {
        return Err(DispatchError::BadInputs(
            "Fix: fingerprint_via requires non-empty hypervectors.".to_string(),
        ));
    }
    if kind_hv.len() != signature_hv.len() || kind_hv.len() != region_hv.len() {
        return Err(DispatchError::BadInputs(format!(
            "Fix: fingerprint_via requires equal hypervector lengths; got kind={}, signature={}, region={}.",
            kind_hv.len(),
            signature_hv.len(),
            region_hv.len()
        )));
    }
    u32::try_from(kind_hv.len()).map_err(|_| {
        DispatchError::BadInputs(format!(
            "Fix: fingerprint_via hypervector length {} exceeds u32::MAX.",
            kind_hv.len()
        ))
    })
}

fn dispatch_xor_bind_with_scratch_into(
    dispatcher: &dyn OptimizerDispatcher,
    a: &[u32],
    b: &[u32],
    dim_words: u32,
    inputs: &mut Vec<Vec<u8>>,
    out: &mut Vec<u32>,
) -> Result<(), DispatchError> {
    let program = hypervector_xor_bind("a", "b", "out", dim_words);
    let out_len = dim_words as usize;
    ensure_input_slots(inputs, 3);
    write_u32_slice_le_bytes(&mut inputs[0], a);
    write_u32_slice_le_bytes(&mut inputs[1], b);
    write_zero_bytes(&mut inputs[2], out_len * std::mem::size_of::<u32>());
    let grid_x = ceil_div_u32(dim_words, 256);
    let outputs = dispatcher.dispatch(&program, inputs, Some([grid_x, 1, 1]))?;
    if outputs.is_empty() {
        return Err(DispatchError::BackendError(format!(
            "Fix: fingerprint_via XOR expected at least one output buffer, got {}.",
            outputs.len()
        )));
    }
    decode_u32_output_exact(&outputs[0], out_len, "fingerprint_via XOR", out)
}

/// Approximate cache lookup: return the index of the cached entry
/// whose fingerprint is most similar to the query, or `None` if all
/// similarities are below `threshold`.
#[must_use]
pub fn lookup_approximate(query: &[u32], cached: &[Vec<u32>], threshold: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, c) in cached.iter().enumerate() {
        let sim = hamming_similarity(query, c);
        if sim >= threshold {
            match best {
                None => best = Some((i, sim)),
                Some((_, best_sim)) if sim > best_sim => best = Some((i, sim)),
                _ => {}
            }
        }
    }
    best.map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch_buffers::u32_slice_to_le_bytes;

    struct XorDispatcher;

    impl OptimizerDispatcher for XorDispatcher {
        fn dispatch(
            &self,
            _program: &vyre_foundation::ir::Program,
            inputs: &[Vec<u8>],
            _grid_override: Option<[u32; 3]>,
        ) -> Result<Vec<Vec<u8>>, DispatchError> {
            let [a_bytes, b_bytes, out_bytes] = inputs else {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: XOR test dispatcher expected 3 buffers, got {}.",
                    inputs.len()
                )));
            };
            let a = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                a_bytes,
                "XOR test dispatcher",
            )?;
            let b = crate::hardware::dispatch_buffers::decode_u32_input_aligned(
                b_bytes,
                "XOR test dispatcher",
            )?;
            let out_len = out_bytes.len() / 4;
            if a.len() < out_len || b.len() < out_len {
                return Err(DispatchError::BadInputs(format!(
                    "Fix: XOR test dispatcher input too short for out_len={out_len}; got a={}, b={}.",
                    a.len(),
                    b.len()
                )));
            }
            let out = a
                .iter()
                .zip(b.iter())
                .take(out_len)
                .map(|(&left, &right)| left ^ right)
                .collect::<Vec<_>>();
            Ok(vec![u32_slice_to_le_bytes(&out)])
        }
    }

    #[test]
    fn fingerprint_self_lookup_returns_match() {
        let kind = vec![0xDEAD_BEEFu32; 8];
        let sig = vec![0x1234_5678u32; 8];
        let region = vec![0x9ABC_DEF0u32; 8];
        let fp = reference_fingerprint(&kind, &sig, &region);
        let cache = vec![fp.clone()];
        let hit = lookup_approximate(&fp, &cache, 0.99);
        assert_eq!(hit, Some(0));
    }

    #[test]
    fn fingerprint_high_threshold_excludes_distant() {
        let kind1 = vec![0u32; 8];
        let sig1 = vec![0u32; 8];
        let region1 = vec![0u32; 8];
        let fp1 = reference_fingerprint(&kind1, &sig1, &region1);

        let kind2 = vec![u32::MAX; 8];
        let sig2 = vec![u32::MAX; 8];
        let region2 = vec![u32::MAX; 8];
        let fp2 = reference_fingerprint(&kind2, &sig2, &region2);

        let cache = vec![fp1];
        let hit = lookup_approximate(&fp2, &cache, 0.99);
        assert_eq!(hit, None); // far below threshold
    }

    #[test]
    fn fingerprint_low_threshold_finds_partial_match() {
        let kind1 = vec![0u32; 8];
        let sig1 = vec![0u32; 8];
        let region1 = vec![0u32; 8];
        let fp1 = reference_fingerprint(&kind1, &sig1, &region1);
        let cache = vec![fp1.clone()];
        let hit = lookup_approximate(&fp1, &cache, -1.0); // any
        assert_eq!(hit, Some(0));
    }

    #[test]
    fn fingerprint_via_matches_reference_for_equal_dimensions() {
        let dispatcher = XorDispatcher;
        let kind = vec![0xDEAD_BEEFu32; 8];
        let sig = vec![0x1234_5678u32; 8];
        let region = vec![0x9ABC_DEF0u32; 8];
        let got =
            fingerprint_via(&dispatcher, &kind, &sig, &region).expect("Fix: dispatch succeeds");
        assert_eq!(got, reference_fingerprint(&kind, &sig, &region));
    }

    #[test]
    fn release_fingerprint_via_path_does_not_call_cpu_or_reference_helpers() {
        let source = include_str!("vsa_fingerprint.rs");
        let start = source
            .find("pub fn fingerprint_via")
            .expect("Fix: via path marker must exist");
        let end = source
            .find("\n/// Approximate cache lookup")
            .expect("Fix: lookup marker must exist");
        let release_path = &source[start..end];
        assert!(!release_path.contains("_cpu"));
        assert!(!release_path.contains("reference_"));
    }

    #[test]
    fn fingerprint_via_into_reuses_output() {
        let dispatcher = XorDispatcher;
        let kind = vec![0xDEAD_BEEFu32; 8];
        let sig = vec![0x1234_5678u32; 8];
        let region = vec![0x9ABC_DEF0u32; 8];
        let mut out = Vec::with_capacity(16);
        let ptr = out.as_ptr();

        fingerprint_via_into(&dispatcher, &kind, &sig, &region, &mut out).unwrap();

        assert_eq!(out.as_ptr(), ptr);
        assert_eq!(out, reference_fingerprint(&kind, &sig, &region));
    }

    #[test]
    fn fingerprint_via_with_scratch_reuses_dispatch_intermediate_and_output_storage() {
        let dispatcher = XorDispatcher;
        let kind = vec![0xDEAD_BEEFu32; 8];
        let sig = vec![0x1234_5678u32; 8];
        let region = vec![0x9ABC_DEF0u32; 8];
        let mut scratch = VsaFingerprintGpuScratch::default();
        let mut out = Vec::with_capacity(8);

        fingerprint_via_with_scratch_into(
            &dispatcher,
            &kind,
            &sig,
            &region,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        let input_capacities = scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>();
        let bound1_capacity = scratch.bound1.capacity();
        let out_capacity = out.capacity();

        fingerprint_via_with_scratch_into(
            &dispatcher,
            &region,
            &kind,
            &sig,
            &mut scratch,
            &mut out,
        )
        .unwrap();

        assert_eq!(
            scratch.inputs.iter().map(Vec::capacity).collect::<Vec<_>>(),
            input_capacities
        );
        assert_eq!(scratch.bound1.capacity(), bound1_capacity);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(out, reference_fingerprint(&region, &kind, &sig));
    }

    #[test]
    fn fingerprint_via_rejects_mismatched_dimensions() {
        let dispatcher = XorDispatcher;
        let error = fingerprint_via(&dispatcher, &[1, 2], &[1], &[1, 2])
            .expect_err("mismatched dimensions must be rejected");
        assert!(
            error.to_string().contains("equal hypervector lengths"),
            "expected dimension error, got {error}"
        );
    }
}
