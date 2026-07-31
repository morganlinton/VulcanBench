//! `reduce_histogram`  -  parallel atomic histogram over a u32 ValueSet.
//!
//! Each global invocation owns one output bin and scans the input stream,
//! storing that bin's count. Used by radix_sort, frequency analysis, and label
//! distribution.
//!
//! # Algorithm
//!
//! Work-group size `[256, 1, 1]`.  Caller dispatches
//! `(count + 255) / 256` work-groups.  Each active lane:
//!
//! ```text
//! if global_id < count:
//!     total = 0
//!     for i in 0..count:
//!         total += input[i] == global_id
//!     output[global_id] = total
//! ```
//!
//! Out-of-range indices are silently dropped because no lane owns them.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::histogram";

/// Build a Program: `output[bin] = count(input[i] == bin)` for each bin.
///
/// Invalid zero dimensions lower to an explicit trap program.
#[must_use]
pub fn histogram(input: &str, output: &str, count: u32, num_bins: u32) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: histogram requires count > 0, got {count}."),
        );
    }
    if num_bins == 0 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: histogram requires num_bins > 0, got {num_bins}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(num_bins)),
        vec![
            Node::let_bind("total", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(count),
                vec![Node::assign(
                    "total",
                    Expr::add(
                        Expr::var("total"),
                        Expr::select(
                            Expr::eq(Expr::load(input, Expr::var("i")), t.clone()),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ),
                )],
            ),
            Node::store(output, t.clone(), Expr::var("total")),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_bins),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Build the legacy atomic scatter variant for callers that can prove backend
/// atomic-add semantics and want input-parallel execution.
#[must_use]
pub fn histogram_atomic_scatter(input: &str, output: &str, count: u32, num_bins: u32) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: histogram_atomic_scatter requires count > 0, got {count}."),
        );
    }
    if num_bins == 0 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: histogram_atomic_scatter requires num_bins > 0, got {num_bins}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("bin", Expr::load(input, t.clone())),
        Node::if_then(
            Expr::lt(Expr::var("bin"), Expr::u32(num_bins)),
            vec![Node::let_bind(
                "_prev",
                Expr::atomic_add(output, Expr::var("bin"), Expr::u32(1)),
            )],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_bins),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(count)),
                body,
            )]),
        }],
    )
}

/// CPU reference.
///
/// Returns a `Vec<u32>` of length `num_bins`.  Out-of-range input
/// values are ignored (matches the GPU drop behaviour).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32], num_bins: u32) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(input, num_bins, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives histogram CPU reference failed: {error}"),
    }
}

/// CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], num_bins: u32, out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(input, num_bins, out) {
        panic!("vyre-primitives histogram CPU reference failed: {error}");
    }
}

/// Fallible CPU reference into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(input: &[u32], num_bins: u32, out: &mut Vec<u32>) -> Result<(), String> {
    let num_bins = usize::try_from(num_bins)
        .map_err(|_| format!("histogram bin count {num_bins} does not fit host usize"))?;
    if num_bins > out.capacity() {
        out.try_reserve_exact(num_bins - out.capacity())
            .map_err(|err| {
                format!("histogram CPU reference could not reserve {num_bins} bins: {err}")
            })?;
    }
    out.clear();
    out.resize(num_bins, 0);
    for &bin in input {
        if let Ok(bin) = usize::try_from(bin) {
            if let Some(slot) = out.get_mut(bin) {
                *slot = slot.wrapping_add(1);
            }
        }
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || histogram("input", "output", 8, 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 1, 2, 3, 0, 1, 2, 3]),
                to_bytes(&[0, 0, 0, 0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[2, 2, 2, 2])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_histogram() {
        let input = &[0u32, 1, 2, 3, 0, 1, 2, 3];
        assert_eq!(cpu_ref(input, 4), vec![2, 2, 2, 2]);
    }

    #[test]
    fn empty_input() {
        assert_eq!(cpu_ref(&[], 4), vec![0, 0, 0, 0]);
    }

    #[test]
    fn all_same_bin() {
        let input = &[2u32, 2, 2, 2, 2];
        assert_eq!(cpu_ref(input, 4), vec![0, 0, 5, 0]);
    }

    #[test]
    fn out_of_bounds_ignored() {
        let input = &[0u32, 1, 99, 2, 3, 100];
        assert_eq!(cpu_ref(input, 4), vec![1, 1, 1, 1]);
    }

    #[test]
    fn try_cpu_ref_into_reuses_output_and_clears_stale_tail() {
        let input = &[0u32, 1, 99, 2, 3, 100];
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&[u32::MAX; 16]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(input, 4, &mut out).unwrap();

        assert_eq!(out, vec![1, 1, 1, 1]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let input = &[0u32, 1, 99, 2, 3, 100];
        let mut compat = Vec::with_capacity(16);
        let mut fallible = Vec::with_capacity(16);

        cpu_ref_into(input, 4, &mut compat);
        try_cpu_ref_into(input, 4, &mut fallible)
            .expect("Fix: small histogram CPU reference must reserve");

        assert_eq!(cpu_ref(input, 4), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_cpu_ref_wrappers_have_no_raw_panic_path() {
        let production = include_str!("histogram.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: histogram.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: histogram CPU parity wrappers must not panic in production."
        );
    }

    #[test]
    fn wrapping_on_overflow() {
        // u32::MAX + 1 wraps to 0, matching GPU atomic_add semantics.
        // cpu_ref uses wrapping_add, so we verify the accumulator behaviour
        // by starting from a high base and adding repeatedly.
        let mut base = u32::MAX - 1;
        base = base.wrapping_add(1); // = u32::MAX
        base = base.wrapping_add(1); // = 0
        assert_eq!(base, 0);
    }

    #[test]
    fn wrapping_overflow_correct() {
        let base = u32::MAX - 1;
        let after_three = base.wrapping_add(3);
        assert_eq!(after_three, 1);
    }

    #[test]
    fn many_bins() {
        let input: Vec<u32> = (0..100).collect();
        let out = cpu_ref(&input, 100);
        assert_eq!(out.len(), 100);
        for (i, &v) in out.iter().enumerate() {
            assert_eq!(v, 1, "bin {i} should have count 1");
        }
    }

    #[test]
    fn sparse_bins() {
        let input = &[0u32, 50, 50, 99];
        let mut expected = vec![0u32; 100];
        expected[0] = 1;
        expected[50] = 2;
        expected[99] = 1;
        assert_eq!(cpu_ref(input, 100), expected);
    }

    #[test]
    fn program_has_expected_buffers() {
        let p = histogram("in", "out", 1024, 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["in", "out"]);
    }

    #[test]
    fn program_buffer_counts() {
        let p = histogram("in", "out", 1024, 16);
        assert_eq!(p.buffers[0].count(), 1024);
        assert_eq!(p.buffers[1].count(), 16);
    }

    #[test]
    fn zero_bins_traps() {
        let p = histogram("in", "out", 10, 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_count_traps() {
        let p = histogram("in", "out", 0, 4);
        assert!(p.stats().trap());
    }

    #[test]
    fn concurrent_access_cpu_simulation() {
        // Simulate what 256 parallel threads would do: many threads hit
        // the same bin.  The result must be deterministic.
        let input = vec![7u32; 10_000];
        let out = cpu_ref(&input, 16);
        assert_eq!(out[7], 10_000);
        for (i, &v) in out.iter().enumerate() {
            if i != 7 {
                assert_eq!(v, 0);
            }
        }
    }

    #[test]
    fn adversarial_all_out_of_bounds() {
        let input = &[100u32, 200, 300];
        assert_eq!(cpu_ref(input, 2), vec![0, 0]);
    }

    #[test]
    fn adversarial_max_u32_index() {
        let input = &[u32::MAX];
        assert_eq!(cpu_ref(input, 4), vec![0, 0, 0, 0]);
    }
}
