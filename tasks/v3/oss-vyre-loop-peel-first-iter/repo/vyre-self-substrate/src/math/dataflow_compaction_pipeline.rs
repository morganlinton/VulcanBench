//! Self-substrate wrappers for dataflow convergence, scan, and compaction kernels.
//!
//! This is the glue layer for small but central GPU building blocks: fixpoint
//! flags, prefix scans, stream compaction, stochastic bitsets, interval merges,
//! sparse recovery, DP clipping, differentiable selection, and attention dot
//! partials. The primitive crate owns the executable semantics.

use vyre_foundation::ir::{Expr, Node, Program};
use vyre_primitives::{
    bitset::stochastic_compute::{
        decode_bitstream, encode_bitstream, encode_bitstream_into, stochastic_and_mul,
    },
    fixpoint::bitset_fixpoint::{bitset_fixpoint, bitset_fixpoint_warm_start},
    math::{
        differentiable::softmax_step,
        dot_partial::{dot_partial, dot_partial_program},
        dp_clip::dp_clip_per_sample,
        interval::{interval_merge_body, interval_merge_program},
        prefix_scan::{
            prefix_scan, prefix_scan_large, prefix_scan_large_with_op_id, prefix_scan_with_op_id,
            ScanKind,
        },
        sparse_recovery::iht_threshold,
        stream_compact::stream_compact,
    },
};

#[cfg(any(test, feature = "cpu-parity"))]
use vyre_primitives::{
    fixpoint::bitset_fixpoint::reference_eval_warm_start,
    math::{
        differentiable::{differentiable_argmax_cpu, softmax_cpu},
        dp_clip::dp_clip_per_sample_cpu,
        interval::cpu_interval_merge,
        sparse_recovery::iht_top_k_cpu,
        stream_compact::cpu_ref as stream_compact_cpu,
    },
};

/// Build a cold bitset fixpoint convergence-flag dispatch.
#[must_use]
pub fn dispatch_bitset_fixpoint(current: &str, next: &str, changed: &str, words: u32) -> Program {
    bitset_fixpoint(current, next, changed, words)
}

/// Build a warm-start bitset fixpoint convergence dispatch.
#[must_use]
pub fn dispatch_bitset_fixpoint_warm_start(
    current: &str,
    next: &str,
    changed: &str,
    seed: &str,
    words: u32,
) -> Program {
    bitset_fixpoint_warm_start(current, next, changed, seed, words)
}

/// Reference warm-start convergence result.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_bitset_fixpoint_warm_start(
    current: &[u32],
    next: &[u32],
    seed: &[u32],
) -> (Vec<u32>, u32) {
    reference_eval_warm_start(current, next, seed)
}

/// Build a single-workgroup prefix scan dispatch.
#[must_use]
pub fn dispatch_prefix_scan(in_buf: &str, out_buf: &str, n: u32, kind: ScanKind) -> Program {
    prefix_scan(in_buf, out_buf, n, kind)
}

/// Build a single-workgroup prefix scan dispatch with an explicit op id.
#[must_use]
pub fn dispatch_prefix_scan_with_op_id(
    in_buf: &str,
    out_buf: &str,
    n: u32,
    kind: ScanKind,
    op_id: &'static str,
) -> Program {
    prefix_scan_with_op_id(in_buf, out_buf, n, kind, op_id)
}

/// Build a large parallel prefix scan dispatch.
#[must_use]
pub fn dispatch_prefix_scan_large(in_buf: &str, out_buf: &str, n: u32) -> Program {
    prefix_scan_large(in_buf, out_buf, n)
}

/// Build a large parallel prefix scan dispatch with an explicit op id.
#[must_use]
pub fn dispatch_prefix_scan_large_with_op_id(
    in_buf: &str,
    out_buf: &str,
    n: u32,
    op_id: &'static str,
) -> Program {
    prefix_scan_large_with_op_id(in_buf, out_buf, n, op_id)
}

/// Build a stream-compaction dispatch consuming exclusive prefix offsets.
#[must_use]
pub fn dispatch_stream_compact(
    payloads: &str,
    flags: &str,
    offsets: &str,
    compacted: &str,
    live_count: &str,
    count: u32,
) -> Program {
    stream_compact(payloads, flags, offsets, compacted, live_count, count)
}

/// CPU stream-compaction reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_stream_compact(payloads: &[u32], flags: &[u32]) -> (Vec<u32>, u32) {
    stream_compact_cpu(payloads, flags)
}

/// Emit a composable interval merge body.
#[must_use]
pub fn interval_body(
    mins_a: &str,
    maxs_a: &str,
    mins_b: &str,
    maxs_b: &str,
    mins_out: &str,
    maxs_out: &str,
    lane_count: u32,
) -> Vec<Node> {
    interval_merge_body(
        mins_a, maxs_a, mins_b, maxs_b, mins_out, maxs_out, lane_count,
    )
}

/// Build an interval merge dispatch.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn dispatch_interval_merge(
    mins_a: &str,
    maxs_a: &str,
    mins_b: &str,
    maxs_b: &str,
    mins_out: &str,
    maxs_out: &str,
    lane_count: u32,
) -> Program {
    interval_merge_program(
        mins_a, maxs_a, mins_b, maxs_b, mins_out, maxs_out, lane_count,
    )
}

/// CPU interval merge reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_interval_merge(
    mins_a: &[u32],
    maxs_a: &[u32],
    mins_b: &[u32],
    maxs_b: &[u32],
) -> (Vec<u32>, Vec<u32>) {
    cpu_interval_merge(mins_a, maxs_a, mins_b, maxs_b)
}

/// Build an iterative-hard-thresholding dispatch.
#[must_use]
pub fn dispatch_iht_threshold(z: &str, threshold: &str, out: &str, n: u32) -> Program {
    iht_threshold(z, threshold, out, n)
}

/// CPU top-k hard-thresholding reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_iht_top_k(z: &[f64], k: usize) -> (Vec<f64>, f64) {
    iht_top_k_cpu(z, k)
}

/// Build a per-sample DP clipping dispatch.
#[must_use]
pub fn dispatch_dp_clip_per_sample(
    grads: &str,
    norms: &str,
    clip_norm: &str,
    clipped: &str,
    b: u32,
    d: u32,
) -> Program {
    dp_clip_per_sample(grads, norms, clip_norm, clipped, b, d)
}

/// CPU per-sample DP clipping reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_dp_clip_per_sample(
    grads: &[f64],
    norms: &[f64],
    clip_norm: f64,
    b: u32,
    d: u32,
) -> Vec<f64> {
    dp_clip_per_sample_cpu(grads, norms, clip_norm, b, d)
}

/// Build a softmax normalization dispatch over precomputed exponentials.
#[must_use]
pub fn dispatch_softmax(pre_exp: &str, out: &str, n: u32) -> Program {
    softmax_step(pre_exp, out, n)
}

/// CPU softmax reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_softmax(x: &[f64]) -> Vec<f64> {
    softmax_cpu(x)
}

/// CPU differentiable argmax reference.
#[cfg(any(test, feature = "cpu-parity"))]
#[must_use]
pub fn reference_differentiable_argmax(x: &[f64], temperature: f64) -> Vec<f64> {
    differentiable_argmax_cpu(x, temperature)
}

/// Emit a composable dot-partial accumulator node.
#[must_use]
pub fn dot_partial_accumulate(
    q_buffer: &str,
    k_buffer: &str,
    accum_var: &str,
    q_base: Expr,
    k_base: Expr,
    d: u32,
) -> Node {
    dot_partial(q_buffer, k_buffer, accum_var, q_base, k_base, d)
}

/// Build a standalone dot-partial dispatch.
#[must_use]
pub fn dispatch_dot_partial(q_buffer: &str, k_buffer: &str, out: &str, d: u32) -> Program {
    dot_partial_program(q_buffer, k_buffer, out, d)
}

/// Build a stochastic bitstream multiply dispatch.
#[must_use]
pub fn dispatch_stochastic_and_mul(a: &str, b: &str, out: &str, n_words: u32) -> Program {
    stochastic_and_mul(a, b, out, n_words)
}

/// Encode a probability as a deterministic stochastic bitstream.
#[must_use]
pub fn stochastic_encode(p: f64, len_bits: usize, seed: u32) -> Vec<u32> {
    encode_bitstream(p, len_bits, seed)
}

/// Encode a probability into caller-owned stochastic bitstream storage.
pub fn stochastic_encode_into(p: f64, len_bits: usize, seed: u32, out: &mut Vec<u32>) {
    encode_bitstream_into(p, len_bits, seed, out);
}

/// Decode a stochastic bitstream back into a probability estimate.
#[must_use]
pub fn stochastic_decode(bs: &[u32], len_bits: usize) -> f64 {
    decode_bitstream(bs, len_bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::math::prefix_scan::OP_ID_INCLUSIVE_SUM;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-8 * (1.0 + a.abs() + b.abs())
    }

    fn program_generator(program: &Program) -> &str {
        let Some(Node::Region { generator, .. }) = program.entry.first() else {
            panic!("Fix: dataflow compaction Program must start with a Region.");
        };
        generator.as_str()
    }

    #[test]
    fn program_builders_emit_expected_primitives() {
        assert_eq!(
            program_generator(&dispatch_bitset_fixpoint("cur", "next", "changed", 4)),
            "vyre-primitives::fixpoint::bitset_fixpoint"
        );
        assert_eq!(
            program_generator(&dispatch_bitset_fixpoint_warm_start(
                "cur", "next", "changed", "seed", 4
            )),
            "vyre-primitives::fixpoint::bitset_fixpoint_warm_start"
        );
        assert_eq!(
            program_generator(&dispatch_prefix_scan(
                "input",
                "output",
                8,
                ScanKind::InclusiveSum
            )),
            "vyre-primitives::math::prefix_scan_inclusive_sum"
        );
        assert_eq!(
            program_generator(&dispatch_prefix_scan_with_op_id(
                "input",
                "output",
                8,
                ScanKind::InclusiveSum,
                OP_ID_INCLUSIVE_SUM
            )),
            "vyre-primitives::math::prefix_scan_inclusive_sum"
        );
        assert_eq!(
            program_generator(&dispatch_prefix_scan_large("input", "output", 2048)),
            "vyre-primitives::math::prefix_scan_inclusive_sum"
        );
        assert_eq!(
            program_generator(&dispatch_prefix_scan_large_with_op_id(
                "input",
                "output",
                2048,
                OP_ID_INCLUSIVE_SUM
            )),
            "vyre-primitives::math::prefix_scan_inclusive_sum"
        );
        assert_eq!(
            program_generator(&dispatch_stream_compact(
                "payloads", "flags", "offsets", "out", "live", 8
            )),
            "vyre-primitives::math::stream_compact"
        );
        assert_eq!(
            program_generator(&dispatch_interval_merge(
                "amin", "amax", "bmin", "bmax", "omin", "omax", 8
            )),
            "vyre-primitives::math::interval_merge"
        );
        assert_eq!(
            program_generator(&dispatch_iht_threshold("z", "threshold", "out", 8)),
            "vyre-primitives::math::iht_threshold"
        );
        assert_eq!(
            program_generator(&dispatch_dp_clip_per_sample("g", "n", "c", "out", 2, 2)),
            "vyre-primitives::math::dp_clip_per_sample"
        );
        assert_eq!(
            program_generator(&dispatch_dot_partial("q", "k", "out", 4)),
            "vyre-primitives::math::dot_partial"
        );
        assert_eq!(
            program_generator(&dispatch_stochastic_and_mul("a", "b", "out", 4)),
            "vyre-primitives::bitset::stochastic_and_mul"
        );
    }

    #[test]
    fn body_builders_emit_composable_ir() {
        assert!(!interval_body("amin", "amax", "bmin", "bmax", "omin", "omax", 4).is_empty());
        let node = dot_partial_accumulate("q", "k", "acc", Expr::u32(0), Expr::u32(0), 4);
        assert!(matches!(node, Node::Block(_) | Node::Loop { .. }));
    }

    #[test]
    fn cpu_references_match_contracts() {
        assert_eq!(
            reference_bitset_fixpoint_warm_start(&[0b001], &[0b011], &[0b100]),
            (vec![0b101], 1)
        );
        assert_eq!(
            reference_stream_compact(&[10, 20, 30, 40], &[0, 1, 1, 0]),
            (vec![20, 30], 2)
        );
        assert_eq!(
            reference_interval_merge(&[10, 0], &[20, 3], &[4, 2], &[18, 5]),
            (vec![4, 0], vec![20, 5])
        );

        let (iht, threshold) = reference_iht_top_k(&[0.1, -2.0, 3.0], 2);
        assert!(approx_eq(threshold, 2.0));
        assert_eq!(iht, vec![0.0, -2.0, 3.0]);

        let clipped = reference_dp_clip_per_sample(&[3.0, 4.0], &[5.0], 1.0, 1, 2);
        assert!(approx_eq(clipped[0], 0.6));
        assert!(approx_eq(clipped[1], 0.8));

        let softmax = reference_softmax(&[1.0, 1.0, 1.0, 1.0]);
        assert!(softmax.iter().all(|value| approx_eq(*value, 0.25)));

        let argmax = reference_differentiable_argmax(&[0.0, 10.0], 0.5);
        assert!(argmax[1] > 0.999);
    }

    #[test]
    fn stochastic_bitstream_wrappers_roundtrip_probability() {
        let bitstream = stochastic_encode(0.25, 1024, 42);
        let decoded = stochastic_decode(&bitstream, 1024);
        assert!((decoded - 0.25).abs() < 0.05);

        let mut into = Vec::with_capacity(bitstream.len());
        stochastic_encode_into(0.25, 1024, 42, &mut into);
        assert_eq!(into, bitstream);
    }
}
