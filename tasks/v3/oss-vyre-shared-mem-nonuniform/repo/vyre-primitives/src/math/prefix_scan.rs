//! Subgroup prefix-sum (inclusive / exclusive scan)  -  core 1000×
//! primitive for variable-length compaction.
//!
//! # Use cases
//!
//! * **Hit-buffer compaction:** each lane produces 0 or 1 live
//!   flag; an exclusive scan over the flag vector gives the
//!   destination slot for each live hit. One dispatch provides the
//!   parallel compaction primitive used by PHASE9_EMIT.
//! * **Histogram prefix:** turn a bin-count vector into the CDF
//!   lookup used by the radix-sort primitive.
//! * **Segmented-reduce baseline:** classical parallel-scan is
//!   the inner kernel of a `(segment_offsets, values)` pair.
//!
//! # Algorithm
//!
//! Hillis-Steele scan over `N` elements, O(N log N) work,
//! `log2(N)` rounds. One invocation per output lane. Round `k`:
//!
//! ```text
//!   if lane >= 2^k:
//!       out[lane] = in[lane - 2^k] op in[lane]
//!   else:
//!       out[lane] = in[lane]
//! ```
//!
//! `op` is `+` for sum-scan; the emitted Program ping-pongs through
//! two workgroup-local scratch buffers with a barrier after every
//! round. The public builder accepts any `N` in `1..=1024` and pads
//! the workgroup to the next power of two internally.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::reduce::multi_block_prefix_scan::multi_block_prefix_scan_sum_u32;

/// Canonical op id for inclusive sum-scan.
pub const OP_ID_INCLUSIVE_SUM: &str = "vyre-primitives::math::prefix_scan_inclusive_sum";
/// Canonical op id for exclusive sum-scan.
pub const OP_ID_EXCLUSIVE_SUM: &str = "vyre-primitives::math::prefix_scan_exclusive_sum";

/// Which scan variant to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    /// `out[i] = sum(in[0..=i])`.
    InclusiveSum,
    /// `out[i] = sum(in[0..i])`  -  identity element (`0`) at slot 0.
    ExclusiveSum,
}

/// Emit a Hillis-Steele prefix-sum Program.
///
/// `n` is the number of input slots. The emitted workgroup size is
/// `n.next_power_of_two()` so non-power-of-two lengths execute with
/// inactive padded lanes.
#[must_use]
pub fn prefix_scan(in_buf: &str, out_buf: &str, n: u32, kind: ScanKind) -> Program {
    let op_id = match kind {
        ScanKind::InclusiveSum => OP_ID_INCLUSIVE_SUM,
        ScanKind::ExclusiveSum => OP_ID_EXCLUSIVE_SUM,
    };
    prefix_scan_with_op_id(in_buf, out_buf, n, kind, op_id)
}

/// Emit a Hillis-Steele prefix-sum Program with an explicit region generator id.
#[must_use]
pub fn prefix_scan_with_op_id(
    in_buf: &str,
    out_buf: &str,
    n: u32,
    kind: ScanKind,
    op_id: &'static str,
) -> Program {
    if n == 0 || n > 1024 {
        return crate::invalid_output_program(
            op_id,
            out_buf,
            DataType::U32,
            format!("Fix: prefix_scan requires n in 1..=1024, got {n}."),
        );
    }

    let lanes = n.next_power_of_two();
    let lane = Expr::InvocationId { axis: 0 };
    let scratch_a = format!("__{out_buf}_scan_a");
    let scratch_b = format!("__{out_buf}_scan_b");

    let mut body: Vec<Node> = Vec::new();
    body.push(Node::store(&scratch_a, lane.clone(), Expr::u32(0)));
    match kind {
        ScanKind::InclusiveSum => body.push(Node::if_then(
            Expr::lt(lane.clone(), Expr::u32(n)),
            vec![Node::store(
                &scratch_a,
                lane.clone(),
                Expr::load(in_buf, lane.clone()),
            )],
        )),
        ScanKind::ExclusiveSum => body.push(Node::if_then(
            Expr::and(
                Expr::lt(Expr::u32(0), lane.clone()),
                Expr::lt(lane.clone(), Expr::u32(n)),
            ),
            vec![Node::store(
                &scratch_a,
                lane.clone(),
                Expr::load(in_buf, Expr::add(lane.clone(), Expr::u32(u32::MAX))),
            )],
        )),
    }
    body.push(Node::Barrier {
        ordering: vyre_foundation::MemoryOrdering::SeqCst,
    });

    let mut stride = 1_u32;
    while stride < lanes {
        let previous_lane = Expr::add(lane.clone(), Expr::u32(u32::MAX.wrapping_sub(stride - 1)));
        body.push(Node::store(
            &scratch_b,
            lane.clone(),
            Expr::load(&scratch_a, lane.clone()),
        ));
        body.push(Node::if_then(
            Expr::lt(Expr::u32(stride - 1), lane.clone()),
            vec![Node::store(
                &scratch_b,
                lane.clone(),
                Expr::add(
                    Expr::load(&scratch_a, lane.clone()),
                    Expr::load(&scratch_a, previous_lane),
                ),
            )],
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        body.push(Node::store(
            &scratch_a,
            lane.clone(),
            Expr::load(&scratch_b, lane.clone()),
        ));
        body.push(Node::Barrier {
            ordering: vyre_foundation::MemoryOrdering::SeqCst,
        });
        stride *= 2;
    }

    body.push(Node::if_then(
        Expr::lt(lane.clone(), Expr::u32(n)),
        vec![Node::store(
            out_buf,
            lane.clone(),
            Expr::load(&scratch_a, lane.clone()),
        )],
    ));

    let output_bytes = usize::try_from(n)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);
    let buffers = vec![
        BufferDecl::storage(in_buf, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
        BufferDecl::output(out_buf, 1, DataType::U32)
            .with_count(n)
            .with_output_byte_range(0..output_bytes),
        BufferDecl::workgroup(&scratch_a, lanes, DataType::U32),
        BufferDecl::workgroup(&scratch_b, lanes, DataType::U32),
    ];

    Program::wrapped(
        buffers,
        [lanes, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Emit a parallel inclusive scan for inputs too large for one workgroup.
///
/// The returned program uses the reduce-domain multi-block scan and wraps it
/// with the math-domain op id so existing callers keep a stable builder
/// identity while large buffers execute through the GPU prefix-scan chain.
#[must_use]
pub fn prefix_scan_large(in_buf: &str, out_buf: &str, n: u32) -> Program {
    prefix_scan_large_with_op_id(in_buf, out_buf, n, OP_ID_INCLUSIVE_SUM)
}

/// Emit a parallel inclusive scan with an explicit region generator id.
#[must_use]
pub fn prefix_scan_large_with_op_id(
    in_buf: &str,
    out_buf: &str,
    n: u32,
    op_id: &'static str,
) -> Program {
    if n == 0 {
        return empty_large_scan_program(in_buf, out_buf, op_id);
    }
    if n <= 1024 {
        return prefix_scan_with_op_id(in_buf, out_buf, n, ScanKind::InclusiveSum, op_id);
    }

    wrap_large_scan_program(multi_block_prefix_scan_sum_u32(in_buf, out_buf, n), op_id)
}

fn empty_large_scan_program(in_buf: &str, out_buf: &str, op_id: &'static str) -> Program {
    let input_decl = BufferDecl::storage(in_buf, 0, BufferAccess::ReadOnly, DataType::U32);
    let output_decl = BufferDecl::output(out_buf, 1, DataType::U32)
        .with_count(1)
        .with_output_byte_range(0..0);

    Program::wrapped(
        vec![input_decl, output_decl],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(Vec::new()),
        }],
    )
}

fn wrap_large_scan_program(program: Program, op_id: &'static str) -> Program {
    Program::wrapped(
        program.buffers().to_vec(),
        program.workgroup_size(),
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(program.entry().to_vec()),
        }],
    )
}

/// CPU-reference prefix scan. Conformance tests verify the GPU
/// Program produces the same output for every input.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32], kind: ScanKind) -> Vec<u32> {
    let mut out = Vec::new();
    try_cpu_ref_into(input, kind, &mut out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - prefix_scan cpu_ref failed: output allocation failed");
    out
}

/// Fallible CPU-reference prefix scan.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref(input: &[u32], kind: ScanKind) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    try_cpu_ref_into(input, kind, &mut out)?;
    Ok(out)
}

/// CPU-reference prefix scan using a caller-owned output buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], kind: ScanKind, out: &mut Vec<u32>) {
    try_cpu_ref_into(input, kind, out)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - prefix_scan cpu_ref_into failed: output allocation failed");
}

/// Fallible CPU-reference prefix scan using a caller-owned output buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(input: &[u32], kind: ScanKind, out: &mut Vec<u32>) -> Result<(), String> {
    if input.len() > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            input.len() - out.len(),
            "prefix scan CPU oracle",
            "scan output",
        )?;
    }
    out.clear();
    let mut acc = 0_u32;
    match kind {
        ScanKind::InclusiveSum => {
            for &x in input {
                acc = acc.wrapping_add(x);
                out.push(acc);
            }
        }
        ScanKind::ExclusiveSum => {
            for &x in input {
                out.push(acc);
                acc = acc.wrapping_add(x);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_cpu_ref_matches_textbook() {
        assert_eq!(
            cpu_ref(&[1, 2, 3, 4], ScanKind::InclusiveSum),
            vec![1, 3, 6, 10],
        );
    }

    #[test]
    fn exclusive_cpu_ref_matches_textbook() {
        assert_eq!(
            cpu_ref(&[1, 2, 3, 4], ScanKind::ExclusiveSum),
            vec![0, 1, 3, 6],
        );
    }

    #[test]
    fn empty_cpu_ref_returns_empty() {
        assert_eq!(cpu_ref(&[], ScanKind::InclusiveSum), Vec::<u32>::new());
        assert_eq!(cpu_ref(&[], ScanKind::ExclusiveSum), Vec::<u32>::new());
    }

    #[test]
    fn wrap_on_overflow() {
        // Overflow check: wrapping_add semantics.
        assert_eq!(
            cpu_ref(&[u32::MAX, 1], ScanKind::InclusiveSum),
            vec![u32::MAX, 0],
        );
    }

    #[test]
    fn cpu_ref_into_reuses_output_buffer() {
        let mut out = Vec::with_capacity(16);
        let ptr = out.as_ptr();
        cpu_ref_into(&[1, 2, 3, 4], ScanKind::ExclusiveSum, &mut out);
        assert_eq!(out, vec![0, 1, 3, 6]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn cpu_ref_into_truncates_stale_tail_without_reallocating() {
        let mut out = Vec::with_capacity(16);
        out.extend([99u32; 16]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&[1, 2, 3, 4], ScanKind::InclusiveSum, &mut out).unwrap();

        assert_eq!(out, vec![1, 3, 6, 10]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn generated_cpu_ref_matches_independent_wrapping_scan() {
        for len in 0..128usize {
            let input: Vec<u32> = (0..len)
                .map(|idx| {
                    (idx as u32)
                        .wrapping_mul(0x9E37_79B9)
                        .wrapping_add(len as u32)
                })
                .collect();
            for kind in [ScanKind::InclusiveSum, ScanKind::ExclusiveSum] {
                let mut out = Vec::with_capacity(len + 3);
                try_cpu_ref_into(&input, kind, &mut out).unwrap();
                let mut expected = Vec::with_capacity(len);
                let mut acc = 0u32;
                for &value in &input {
                    match kind {
                        ScanKind::InclusiveSum => {
                            acc = acc.wrapping_add(value);
                            expected.push(acc);
                        }
                        ScanKind::ExclusiveSum => {
                            expected.push(acc);
                            acc = acc.wrapping_add(value);
                        }
                    }
                }
                assert_eq!(
                    out, expected,
                    "generated prefix scan len={len} kind={kind:?}"
                );
            }
        }
    }

    #[test]
    fn emitted_inclusive_program_has_expected_buffers() {
        let p = prefix_scan("in", "out", 32, ScanKind::InclusiveSum);
        assert_eq!(p.workgroup_size, [32, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["in", "out", "__out_scan_a", "__out_scan_b"]);
    }

    #[test]
    fn emitted_exclusive_program_has_expected_buffers() {
        let p = prefix_scan("in", "out", 64, ScanKind::ExclusiveSum);
        assert_eq!(p.workgroup_size, [64, 1, 1]);
    }

    #[test]
    fn non_power_of_two_n_pads_to_next_power_of_two() {
        let p = prefix_scan("in", "out", 5, ScanKind::InclusiveSum);
        assert_eq!(p.workgroup_size, [8, 1, 1]);
    }

    #[test]
    fn zero_n_traps() {
        let p = prefix_scan("in", "out", 0, ScanKind::InclusiveSum);
        assert!(p.stats().trap());
    }

    #[test]
    fn over_limit_n_traps() {
        let p = prefix_scan("in", "out", 2048, ScanKind::InclusiveSum);
        assert!(p.stats().trap());
    }

    #[test]
    fn binary_power_of_two_sizes_accepted() {
        for n in &[1_u32, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024] {
            let program = prefix_scan("in", "out", *n, ScanKind::InclusiveSum);
            let names: Vec<&str> = program.buffers().iter().map(|b| b.name()).collect();
            assert!(
                names.contains(&"in"),
                "prefix_scan must declare in for n={n}"
            );
            assert!(
                names.contains(&"out"),
                "prefix_scan must declare out for n={n}"
            );
        }
    }
}
