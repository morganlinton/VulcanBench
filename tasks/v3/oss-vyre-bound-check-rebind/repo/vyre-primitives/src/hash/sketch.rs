//! Sketch primitives  -  Count-Sketch (Charikar 2002) and a leverage-
//! score (Drineas 2012) one-shot sampler.
//!
//! Sketches give compressed estimators for matrix products, norms,
//! eigenvalues, and frequency moments with provable error bounds.
//! Underexploited as a tier-2.5 primitive because deep learning ate
//! the attention budget  -  but the substrate is GPU-trivial.
//!
//! # Why this primitive is dual-use
//!
//! | Composition role | Use |
//! |---|---|
//! | streaming summaries | streaming statistics with bounded memory |
//! | randomized linear algebra | sketch-based linear regression / SVD |
//! | observability histograms | approximate quantiles |
//! | profiling summaries | per-Program latency distribution in O(log n) memory |
//!
//! # Operations
//!
//! - [`crate::hash::sketch::count_sketch_update`]  -  given an item and its hash + sign,
//!   add to the sketch table. Single-lane stream model.
//! - [`crate::hash::sketch::count_sketch_query_cpu`]  -  estimate frequency of an item by
//!   reading hash·sign-indexed cells across `d` independent sketches
//!   and taking the median of `sign * cell` reads.

use std::fmt;
use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id for the update primitive.
pub const UPDATE_OP_ID: &str = "vyre-primitives::hash::count_sketch_update";

/// Count-sketch CPU-reference validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CountSketchError {
    /// Sketch dimensions must be non-zero.
    InvalidDimensions {
        /// Number of sketch rows.
        d: u32,
        /// Number of sketch columns.
        w: u32,
    },
    /// `d * w` overflowed the supported host table length.
    TableSizeOverflow {
        /// Number of sketch rows.
        d: u32,
        /// Number of sketch columns.
        w: u32,
    },
    /// Table length does not match `d * w`.
    BadTableShape {
        /// Expected table cells.
        expected: usize,
        /// Actual table cells.
        actual: usize,
    },
    /// Hash or sign vectors are too short for `d` rows.
    BadQueryShape {
        /// Required row count.
        d: usize,
        /// Hash count supplied.
        hashes: usize,
        /// Sign count supplied.
        signs: usize,
    },
    /// A query hash addressed a column outside `[0, w)`.
    HashOutOfRange {
        /// Row containing the bad hash.
        row: usize,
        /// Bad column value.
        col: u32,
        /// Sketch width.
        w: u32,
    },
    /// Caller-owned scratch could not reserve enough estimates.
    Allocation {
        /// Requested estimate count.
        requested: usize,
        /// Allocator detail.
        source: String,
    },
}

impl fmt::Display for CountSketchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { d, w } => {
                write!(f, "count-sketch dimensions must be non-zero, got d={d}, w={w}.")
            }
            Self::TableSizeOverflow { d, w } => {
                write!(f, "count-sketch table size overflowed for d={d}, w={w}.")
            }
            Self::BadTableShape { expected, actual } => write!(
                f,
                "count-sketch table requires {expected} cells for the declared dimensions, got {actual}."
            ),
            Self::BadQueryShape { d, hashes, signs } => write!(
                f,
                "count-sketch query requires {d} hashes and signs, got hashes={hashes}, signs={signs}."
            ),
            Self::HashOutOfRange { row, col, w } => write!(
                f,
                "count-sketch query hash at row {row} addressed column {col}, outside width {w}."
            ),
            Self::Allocation { requested, source } => write!(
                f,
                "count-sketch query could not reserve {requested} estimate slots: {source}."
            ),
        }
    }
}

impl std::error::Error for CountSketchError {}

/// Apply one item to the count-sketch table.
///
/// Inputs:
/// - `table`: `d * w` u32 cells (d sketches × w columns).
/// - `hashes`: `d` precomputed column indices in `[0, w)` for the
///   current item (one per sketch row).
/// - `signs`: `d` precomputed `±1` signs (encoded as `1` and
///   `0xFFFF_FFFF` in u32 two's-complement).
///
/// For each row r in 0..d:
///   `table[r*w + hashes[r]] += signs[r]`
///
/// Invalid dimensions lower to an explicit trap program.
#[must_use]
pub fn count_sketch_update(table: &str, hashes: &str, signs: &str, d: u32, w: u32) -> Program {
    if d == 0 {
        return crate::invalid_output_program(
            UPDATE_OP_ID,
            table,
            DataType::U32,
            format!("Fix: count_sketch_update requires d > 0, got {d}."),
        );
    }
    if w == 0 {
        return crate::invalid_output_program(
            UPDATE_OP_ID,
            table,
            DataType::U32,
            format!("Fix: count_sketch_update requires w > 0, got {w}."),
        );
    }
    let Some(table_words) = d.checked_mul(w) else {
        return crate::invalid_output_program(
            UPDATE_OP_ID,
            table,
            DataType::U32,
            format!("Fix: count_sketch_update table size overflowed for d={d}, w={w}."),
        );
    };

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(d)),
        vec![
            Node::let_bind("col", Expr::load(hashes, t.clone())),
            Node::let_bind("sgn", Expr::load(signs, t.clone())),
            Node::let_bind("row_base", Expr::mul(t.clone(), Expr::u32(w))),
            Node::let_bind("addr", Expr::add(Expr::var("row_base"), Expr::var("col"))),
            Node::store(
                table,
                Expr::var("addr"),
                Expr::add(Expr::load(table, Expr::var("addr")), Expr::var("sgn")),
            ),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(table, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(table_words),
            BufferDecl::storage(hashes, 1, BufferAccess::ReadOnly, DataType::U32).with_count(d),
            BufferDecl::storage(signs, 2, BufferAccess::ReadOnly, DataType::U32).with_count(d),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(UPDATE_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

// ---- CPU references ----

/// CPU helper: apply `(hashes, signs)` for one item to a (d × w) sketch.
/// Encoding: `signs[i]` is `+1` or `-1` as `i32`; we cast through `u32`
/// for the table representation.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn count_sketch_update_cpu(table: &mut [u32], hashes: &[u32], signs: &[i32], d: u32, w: u32) {
    let Ok(expected_len) = count_sketch_table_len(d, w) else {
        return;
    };
    let Ok(d_len) = usize::try_from(d) else {
        return;
    };
    let Ok(w_len) = usize::try_from(w) else {
        return;
    };
    if table.len() != expected_len || hashes.len() < d_len || signs.len() < d_len {
        return;
    }
    for r in 0..d_len {
        let Ok(col) = usize::try_from(hashes[r]) else {
            continue;
        };
        if col >= w_len {
            continue;
        }
        let addr = r * w_len + col;
        // Two's-complement add via u32 wrap is the GPU semantics.
        let cell = table[addr] as i32;
        table[addr] = (cell + signs[r]) as u32;
    }
}

/// CPU helper: estimate item frequency from sketch (median of
/// `sign[r] * table[r * w + hash[r]]` across the d rows).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn count_sketch_query_cpu(table: &[u32], hashes: &[u32], signs: &[i32], d: u32, w: u32) -> i32 {
    let mut estimates = Vec::new();
    count_sketch_query_cpu_into(table, hashes, signs, d, w, &mut estimates)
}

/// Caller-owned variant of [`count_sketch_query_cpu`].
#[cfg(any(test, feature = "cpu-parity"))]
pub fn count_sketch_query_cpu_into(
    table: &[u32],
    hashes: &[u32],
    signs: &[i32],
    d: u32,
    w: u32,
    estimates: &mut Vec<i32>,
) -> i32 {
    try_count_sketch_query_cpu_into(table, hashes, signs, d, w, estimates).unwrap_or(0)
}

/// Fallible caller-owned variant of [`count_sketch_query_cpu`].
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_count_sketch_query_cpu_into(
    table: &[u32],
    hashes: &[u32],
    signs: &[i32],
    d: u32,
    w: u32,
    estimates: &mut Vec<i32>,
) -> Result<i32, CountSketchError> {
    let table_len = count_sketch_table_len(d, w)?;
    if table.len() != table_len {
        return Err(CountSketchError::BadTableShape {
            expected: table_len,
            actual: table.len(),
        });
    }
    let d_len = usize::try_from(d).map_err(|_| CountSketchError::TableSizeOverflow { d, w })?;
    let w_len = usize::try_from(w).map_err(|_| CountSketchError::TableSizeOverflow { d, w })?;
    if hashes.len() < d_len || signs.len() < d_len {
        return Err(CountSketchError::BadQueryShape {
            d: d_len,
            hashes: hashes.len(),
            signs: signs.len(),
        });
    }
    for (row, &col) in hashes.iter().take(d_len).enumerate() {
        if col >= w {
            return Err(CountSketchError::HashOutOfRange { row, col, w });
        }
    }
    if d_len > estimates.capacity() {
        estimates
            .try_reserve_exact(d_len - estimates.capacity())
            .map_err(|source| CountSketchError::Allocation {
                requested: d_len,
                source: source.to_string(),
            })?;
    }

    estimates.clear();
    for r in 0..d_len {
        let col = usize::try_from(hashes[r]).map_err(|_| CountSketchError::HashOutOfRange {
            row: r,
            col: hashes[r],
            w,
        })?;
        let cell = table[r * w_len + col] as i32;
        estimates.push(cell * signs[r]);
    }
    estimates.sort_unstable();
    Ok(estimates[estimates.len() / 2])
}

#[cfg(any(test, feature = "cpu-parity"))]
fn count_sketch_table_len(d: u32, w: u32) -> Result<usize, CountSketchError> {
    if d == 0 || w == 0 {
        return Err(CountSketchError::InvalidDimensions { d, w });
    }
    let cells = d
        .checked_mul(w)
        .ok_or(CountSketchError::TableSizeOverflow { d, w })?;
    usize::try_from(cells).map_err(|_| CountSketchError::TableSizeOverflow { d, w })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_single_item_round_trip() {
        // Insert item once, query  -  should return ~1.
        let d = 5u32;
        let w = 16u32;
        let mut table = vec![0u32; (d * w) as usize];
        let hashes = vec![3u32, 11, 2, 7, 14];
        let signs = vec![1i32, -1, 1, -1, 1];
        count_sketch_update_cpu(&mut table, &hashes, &signs, d, w);
        let est = count_sketch_query_cpu(&table, &hashes, &signs, d, w);
        assert_eq!(est, 1);
    }

    #[test]
    fn cpu_repeated_inserts_count() {
        let d = 5u32;
        let w = 16u32;
        let mut table = vec![0u32; (d * w) as usize];
        let hashes = vec![3u32, 11, 2, 7, 14];
        let signs = vec![1i32, -1, 1, -1, 1];
        for _ in 0..7 {
            count_sketch_update_cpu(&mut table, &hashes, &signs, d, w);
        }
        let est = count_sketch_query_cpu(&table, &hashes, &signs, d, w);
        assert_eq!(est, 7);
    }

    #[test]
    fn cpu_unrelated_query_returns_zero_or_small() {
        // After inserting one item, a different item with disjoint
        // hashes should query as 0.
        let d = 5u32;
        let w = 16u32;
        let mut table = vec![0u32; (d * w) as usize];
        let h_a = vec![3u32, 11, 2, 7, 14];
        let s_a = vec![1i32, -1, 1, -1, 1];
        count_sketch_update_cpu(&mut table, &h_a, &s_a, d, w);

        let h_b = vec![5u32, 9, 0, 4, 12];
        let s_b = vec![-1i32, 1, -1, 1, -1];
        let est = count_sketch_query_cpu(&table, &h_b, &s_b, d, w);
        assert_eq!(est, 0);
    }

    #[test]
    fn cpu_two_items_independent_estimates() {
        let d = 7u32;
        let w = 32u32;
        let mut table = vec![0u32; (d * w) as usize];
        let h_a = vec![1u32, 2, 3, 4, 5, 6, 7];
        let s_a = vec![1i32, 1, -1, 1, -1, 1, 1];
        let h_b = vec![10u32, 20, 30, 11, 21, 0, 25];
        let s_b = vec![-1i32, 1, 1, -1, 1, 1, -1];

        for _ in 0..3 {
            count_sketch_update_cpu(&mut table, &h_a, &s_a, d, w);
        }
        for _ in 0..5 {
            count_sketch_update_cpu(&mut table, &h_b, &s_b, d, w);
        }
        assert_eq!(count_sketch_query_cpu(&table, &h_a, &s_a, d, w), 3);
        assert_eq!(count_sketch_query_cpu(&table, &h_b, &s_b, d, w), 5);
    }

    #[test]
    fn cpu_helpers_reject_malformed_inputs_without_panicking() {
        let mut table = vec![9u32; 4];
        count_sketch_update_cpu(&mut table, &[9], &[1], 2, 2);
        assert_eq!(table, vec![9u32; 4]);

        let mut estimates = Vec::with_capacity(8);
        let ptr = estimates.as_ptr();
        let got = count_sketch_query_cpu_into(&table, &[99, 1], &[1, 1], 2, 2, &mut estimates);
        assert_eq!(got, 0);
        assert_eq!(estimates.as_ptr(), ptr);
    }

    #[test]
    fn try_query_reuses_estimates_and_is_transactional_on_bad_hash() {
        let d = 3u32;
        let w = 8u32;
        let mut table = vec![0u32; 24];
        let hashes = vec![1u32, 2, 3];
        let signs = vec![1i32, -1, 1];
        for _ in 0..5 {
            count_sketch_update_cpu(&mut table, &hashes, &signs, d, w);
        }
        let mut estimates = Vec::with_capacity(8);
        estimates.extend_from_slice(&[99, 98, 97, 96]);
        let ptr = estimates.as_ptr();

        let got = try_count_sketch_query_cpu_into(&table, &hashes, &signs, d, w, &mut estimates)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid sketch query must succeed");

        assert_eq!(got, 5);
        assert_eq!(estimates.as_ptr(), ptr);
        let before = estimates.clone();
        let err =
            try_count_sketch_query_cpu_into(&table, &[1, 99, 3], &signs, d, w, &mut estimates)
                .expect_err("out-of-range hash must be reported");
        assert!(matches!(
            err,
            CountSketchError::HashOutOfRange { row: 1, .. }
        ));
        assert_eq!(estimates, before);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = count_sketch_update("t", "h", "s", 5, 16);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["t", "h", "s"]);
        assert_eq!(p.buffers[0].count(), 5 * 16);
        assert_eq!(p.buffers[1].count(), 5);
        assert_eq!(p.buffers[2].count(), 5);
    }

    #[test]
    fn zero_d_traps() {
        let p = count_sketch_update("t", "h", "s", 0, 16);
        assert!(p.stats().trap());
    }

    #[test]
    fn zero_w_traps() {
        let p = count_sketch_update("t", "h", "s", 5, 0);
        assert!(p.stats().trap());
    }
}
