//! `segment_reduce_sum`  -  per-segment wrapping unsigned sum.
//!
//! Each work-group thread handles one segment.  The `segment_offsets`
//! buffer is CSR-style: `offsets[i]..offsets[i+1]` is the range of
//! `input` belonging to segment `i`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::segment_reduce_sum";

/// Build a Program: `output[seg] = Σ input[offsets[seg]..offsets[seg+1]]`.
///
/// Invalid segment counts lower to an explicit trap program.
#[must_use]
pub fn segment_reduce_sum(
    input: &str,
    segment_offsets: &str,
    output: &str,
    num_segments: u32,
) -> Program {
    if num_segments == 0 || num_segments > 256 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: segment_reduce_sum requires 0 < num_segments <= 256, got {num_segments}. For larger counts, tile the dispatch across multiple work-groups."),
        );
    }

    let lane = Expr::InvocationId { axis: 0 };

    let body = vec![
        Node::let_bind("start", Expr::load(segment_offsets, lane.clone())),
        Node::let_bind(
            "end",
            Expr::load(segment_offsets, Expr::add(lane.clone(), Expr::u32(1))),
        ),
        Node::let_bind("acc", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::var("start"),
            Expr::var("end"),
            vec![Node::assign(
                "acc",
                Expr::add(Expr::var("acc"), Expr::load(input, Expr::var("i"))),
            )],
        ),
        Node::store(output, lane.clone(), Expr::var("acc")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(segment_offsets, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(num_segments + 1),
            BufferDecl::storage(output, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_segments),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(lane.clone(), Expr::u32(num_segments)),
                body,
            )]),
        }],
    )
}

/// CPU reference.
///
/// Malformed segment bounds fail loudly; this oracle is only for parity tests
/// with valid CSR-style segment metadata and must not hide bad host fixtures.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32], segment_offsets: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
    match try_cpu_ref_into(input, segment_offsets, &mut out) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives segment_reduce_sum CPU reference failed: {error}"),
    }
}

/// CPU reference using a caller-owned output buffer.
///
/// Malformed segment bounds fail loudly so CPU parity cannot hide truncated or
/// non-monotonic segment metadata as an all-zero segment.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], segment_offsets: &[u32], out: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(input, segment_offsets, out) {
        panic!("vyre-primitives segment_reduce_sum CPU reference failed: {error}");
    }
}

/// Fallible CPU reference using a caller-owned output buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    input: &[u32],
    segment_offsets: &[u32],
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let num_segments = segment_offsets.len().checked_sub(1).ok_or_else(|| {
        "segment_reduce_sum CPU oracle received empty segment_offsets. Fix: pass at least one CSR-style offset.".to_string()
    })?;
    if num_segments > out.capacity() {
        out.try_reserve_exact(num_segments - out.capacity())
            .map_err(|err| {
                format!(
                    "segment_reduce_sum CPU oracle could not reserve {num_segments} output segments: {err}"
                )
            })?;
    }
    for seg in 0..num_segments {
        let start = usize::try_from(segment_offsets[seg]).map_err(|_| {
            format!("segment_reduce_sum CPU oracle segment {seg} start does not fit host usize.")
        })?;
        let end = usize::try_from(segment_offsets[seg + 1]).map_err(|_| {
            format!("segment_reduce_sum CPU oracle segment {seg} end does not fit host usize.")
        })?;
        if start > end || end > input.len() {
            return Err(format!(
                "segment_reduce_sum CPU oracle received malformed segment {seg}: start={start}, end={end}, input_len={}. Fix: rebuild monotonic in-bounds segment offsets before parity comparison.",
                input.len()
            ));
        }
    }

    out.clear();
    for seg in 0..num_segments {
        let start = usize::try_from(segment_offsets[seg]).map_err(|_| {
            format!("segment_reduce_sum CPU oracle segment {seg} start does not fit host usize.")
        })?;
        let end = usize::try_from(segment_offsets[seg + 1]).map_err(|_| {
            format!("segment_reduce_sum CPU oracle segment {seg} end does not fit host usize.")
        })?;
        let sum = input[start..end]
            .iter()
            .copied()
            .fold(0u32, u32::wrapping_add);
        out.push(sum);
    }
    Ok(())
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || segment_reduce_sum("input", "segment_offsets", "output", 2),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[1, 2, 3, 4, 5]),
                to_bytes(&[0, 2, 5]),
                to_bytes(&[0, 0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[3, 12])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_segments() {
        assert_eq!(cpu_ref(&[1, 2, 3, 4, 5], &[0, 2, 5]), vec![3, 12]);
    }

    #[test]
    fn single_segment() {
        assert_eq!(cpu_ref(&[10, 20, 30], &[0, 3]), vec![60]);
    }

    #[test]
    fn empty_segment() {
        assert_eq!(cpu_ref(&[1, 2, 3], &[0, 0, 3]), vec![0, 6]);
    }

    #[test]
    fn wraps_on_overflow() {
        assert_eq!(cpu_ref(&[u32::MAX, 1, 2], &[0, 2, 3]), vec![0, 2]);
    }

    #[test]
    fn cpu_ref_into_reuses_output_buffer() {
        let mut out = Vec::with_capacity(8);
        let ptr = out.as_ptr();
        cpu_ref_into(&[1, 2, 3, 4, 5], &[0, 2, 5], &mut out);
        assert_eq!(out, vec![3, 12]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn try_cpu_ref_into_clears_stale_tail_without_reallocating() {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[u32::MAX; 8]);
        let ptr = out.as_ptr();

        try_cpu_ref_into(&[1, 2, 3, 4, 5], &[0, 2, 5], &mut out).unwrap();

        assert_eq!(out, vec![3, 12]);
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn try_cpu_ref_into_rejects_bad_offsets_without_mutating_output() {
        let mut out = vec![0xDEAD_BEEF, 0xCAFE_BABE];
        let before = out.clone();

        let err = try_cpu_ref_into(&[1, 2, 3], &[0, 4], &mut out)
            .expect_err("out-of-bounds segment must be rejected");

        assert!(err.contains("malformed segment"));
        assert_eq!(out, before);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let input = &[1, 2, 3, 4, 5];
        let offsets = &[0, 2, 5];
        let mut compat = Vec::with_capacity(8);
        let mut fallible = Vec::with_capacity(8);

        cpu_ref_into(input, offsets, &mut compat);
        try_cpu_ref_into(input, offsets, &mut fallible)
            .expect("Fix: small segment_reduce_sum CPU reference must reserve");

        assert_eq!(cpu_ref(input, offsets), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_wrappers_have_no_raw_panic_path() {
        let production = include_str!("segment_reduce.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: segment_reduce.rs must contain production section");

        // No LAZY panics (no fix hint); an explicit panic!() fail-loud IS the
        // blessed Law-10 fix for an infallible parity wrapper.
        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: segment_reduce_sum production wrappers must not use bare .unwrap()/.expect() — use an explicit panic!() with the error."
        );
        // No SILENT fallback: returning empty on failure masks a parity divergence (Law 10/6).
        assert!(
            !production.contains(concat!("eprintln", "!(\"vyre-primitives segment_reduce_sum")),
            "Fix: segment_reduce_sum CPU oracle must not log-and-return empty on error — fail loud via panic!() so callers use the try_ variant."
        );
        assert!(
            production.contains("panic!("),
            "Fix: segment_reduce_sum CPU oracle must panic!() when it cannot compute the reference, never return an empty vec."
        );
    }

    #[test]
    fn emitted_program_has_expected_buffers() {
        let p = segment_reduce_sum("input", "segment_offsets", "output", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["input", "segment_offsets", "output"]);
    }

    #[test]
    fn zero_segments_traps() {
        let p = segment_reduce_sum("input", "segment_offsets", "output", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn over_limit_segments_traps() {
        let p = segment_reduce_sum("input", "segment_offsets", "output", 257);
        assert!(p.stats().trap());
    }
}
