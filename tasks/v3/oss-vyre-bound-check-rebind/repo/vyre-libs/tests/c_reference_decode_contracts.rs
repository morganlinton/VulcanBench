//! C parser Reference-oracle byte decoders must reject malformed VAST input.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]
use vyre_libs::parsing::c::lower::try_reference_ast_to_pg_nodes;
use vyre_libs::parsing::c::parse::vast::{
    try_reference_c11_annotate_typedef_names, try_reference_c11_build_expression_shape_nodes,
    try_reference_c11_classify_vast_node_kinds, CReferenceDecodeError,
};

#[test]
fn c_vast_reference_oracles_reject_non_word_aligned_bytes() {
    let err = try_reference_c11_classify_vast_node_kinds(&[0, 1, 2])
        .expect_err("classifier must reject trailing partial u32 bytes");
    assert!(
        matches!(err, CReferenceDecodeError::MisalignedBytes { len: 3 }),
        "misaligned classifier input must be rejected, got {err:?}"
    );

    let err = try_reference_c11_annotate_typedef_names(&[0, 1, 2], b"")
        .expect_err("typedef annotation must reject trailing partial u32 bytes");
    assert!(
        matches!(err, CReferenceDecodeError::MisalignedBytes { len: 3 }),
        "misaligned annotation input must be rejected, got {err:?}"
    );
}

#[test]
fn c_vast_reference_oracles_reject_partial_rows() {
    let partial_row = 0_u32.to_le_bytes();
    let err = try_reference_c11_classify_vast_node_kinds(&partial_row)
        .expect_err("classifier must reject partial VAST rows");
    assert!(
        matches!(
            err,
            CReferenceDecodeError::PartialVastRow {
                words: 1,
                stride: 10
            }
        ),
        "partial classifier row must be rejected, got {err:?}"
    );

    let err = try_reference_c11_build_expression_shape_nodes(&partial_row, &partial_row)
        .expect_err("expression-shape oracle must reject partial VAST rows");
    assert!(
        matches!(
            err,
            CReferenceDecodeError::PartialVastRow {
                words: 1,
                stride: 10
            }
        ),
        "partial expression-shape row must be rejected, got {err:?}"
    );
}

#[test]
fn ast_to_pg_reference_oracle_rejects_malformed_vast_bytes() {
    let err = try_reference_ast_to_pg_nodes(&[0, 1, 2])
        .expect_err("AST-to-PG oracle must reject trailing partial u32 bytes");
    assert!(
        err.to_string().contains("4-byte aligned"),
        "misaligned AST-to-PG error must be actionable, got {err:?}"
    );

    let partial_row = 0_u32.to_le_bytes();
    let err = try_reference_ast_to_pg_nodes(&partial_row)
        .expect_err("AST-to-PG oracle must reject partial VAST rows");
    assert!(
        err.to_string().contains("row stride"),
        "partial-row AST-to-PG error must be actionable, got {err:?}"
    );
}
