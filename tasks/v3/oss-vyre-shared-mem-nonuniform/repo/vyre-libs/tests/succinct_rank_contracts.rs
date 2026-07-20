//! Contract tests for succinct bitvector rank metadata.
//!
//! These tests exercise the public Cat-A builders through the reference
//! interpreter so the rank/select substrate has an executable oracle before
//! parser and graph code depend on it.

#![cfg(feature = "math-succinct")]
#![allow(deprecated)]
mod common;
use common::{decode_u32_words, u32_bytes};
use vyre_reference::value::Value;

#[test]
fn rank_superblocks_store_zero_prefix_and_total_sentinel() {
    let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0];
    let program = vyre_libs::math::rank1_superblocks("bits", "superblocks", bits.len() as u32, 2);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&bits)),
            Value::from(vec![0u8; 3 * core::mem::size_of::<u32>()]),
        ],
    )
    .expect("rank1_superblocks must execute");

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0, 4, 20],
        "superblocks must be prefix counts plus a total-popcount sentinel"
    );
}

#[test]
fn rank_queries_count_bits_strictly_before_each_offset() {
    let bits = [0b1011u32, 0x8000_0000, 0xFFFF_0000, 0];
    let superblocks = [0u32, 4, 20];
    let queries = [0u32, 1, 4, 63, 64, 80, 112, 127];
    let program = vyre_libs::math::rank1_query(
        "bits",
        "superblocks",
        "queries",
        "out",
        bits.len() as u32,
        queries.len() as u32,
        2,
    );
    let outputs = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&bits)),
            Value::from(u32_bytes(&superblocks)),
            Value::from(u32_bytes(&queries)),
            Value::from(vec![0u8; queries.len() * core::mem::size_of::<u32>()]),
        ],
    )
    .expect("rank1_query must execute");

    assert_eq!(
        decode_u32_words(&outputs[0].to_bytes()),
        vec![0, 1, 3, 3, 4, 4, 20, 20],
        "rank is exclusive of the queried bit offset"
    );
}

#[test]
fn rank_builders_reject_zero_word_superblocks() {
    let err = vyre_libs::math::try_rank1_superblocks("bits", "superblocks", 1, 0)
        .expect_err("zero-sized superblocks must be rejected");
    assert_eq!(
        err.to_string(),
        "Fix: rank superblock size must be at least one u32 word"
    );

    let err = vyre_libs::math::try_rank1_query("bits", "superblocks", "queries", "out", 1, 1, 0)
        .expect_err("zero-sized query superblocks must be rejected");
    assert_eq!(
        err.to_string(),
        "Fix: rank superblock size must be at least one u32 word"
    );
}

#[test]
fn rank_query_traps_out_of_bounds_offsets() {
    let program = vyre_libs::math::rank1_query("bits", "superblocks", "queries", "out", 1, 1, 1);
    let result = vyre_reference::reference_eval(
        &program,
        &[
            Value::from(u32_bytes(&[0u32])),
            Value::from(u32_bytes(&[0u32, 0])),
            Value::from(u32_bytes(&[32u32])),
            Value::from(vec![0u8; core::mem::size_of::<u32>()]),
        ],
    );

    let err =
        result.expect_err("rank1_query must fail loudly when a query addresses a missing word");
    assert!(
        err.to_string().contains("rank-query-out-of-bounds"),
        "unexpected error: {err}"
    );
}
