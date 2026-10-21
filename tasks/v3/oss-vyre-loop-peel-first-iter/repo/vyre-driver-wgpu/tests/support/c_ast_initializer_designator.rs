use vyre::ir::Expr;
use vyre_libs::parsing::c::lower::c_lower_ast_to_pg_nodes;
use vyre_reference::value::Value;

pub(crate) const VAST_STRIDE_U32: usize = 10;
pub(crate) const PG_STRIDE_U32: usize = 6;

pub(crate) fn word_at(buf: &[u8], word: usize) -> u32 {
    let off = word * 4;
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

pub(crate) fn node_count_from_vast(vast: &[u8]) -> u32 {
    u32::try_from(vast.len() / (VAST_STRIDE_U32 * 4)).unwrap_or_default()
}

pub(crate) fn typed_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    rows.chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

pub(crate) fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
}

pub(crate) fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

pub(crate) fn run_reference_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    let num_nodes = node_count_from_vast(typed_vast);
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "pg_nodes");
    let output_len = num_nodes.saturating_mul(PG_STRIDE_U32 as u32).max(1) as usize * 4;
    let values = [
        Value::from(typed_vast.to_vec()),
        Value::from(vec![0; output_len]),
    ];
    let outputs = vyre_reference::reference_eval(&program, &values)
        .unwrap_or_else(|error| panic!("Fix: C AST PG lowerer must execute on CPU: {error}"));
    assert_eq!(outputs.len(), 1, "Fix: PG lowerer must emit one buffer");
    outputs[0].to_bytes()
}

pub(crate) fn assert_pg_preserves_row(
    typed_vast: &[u8],
    pg: &[u8],
    tok_starts: &[u32],
    tok_lens: &[u32],
    idx: usize,
    expected_kind: u32,
) {
    assert_eq!(
        pg_word_at(pg, idx, 0),
        expected_kind,
        "PG kind mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 1),
        tok_starts[idx],
        "PG span_start mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 2),
        tok_starts[idx] + tok_lens[idx],
        "PG span_end mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 3),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 1),
        "PG parent mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 4),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 2),
        "PG first_child mismatch at row {idx}"
    );
    assert_eq!(
        pg_word_at(pg, idx, 5),
        word_at(typed_vast, idx * VAST_STRIDE_U32 + 3),
        "PG next_sibling mismatch at row {idx}"
    );
}

pub(crate) fn assert_pg_preserves_kind_span_and_links(
    typed_vast: &[u8],
    pg: &[u8],
    tok_starts: &[u32],
    tok_lens: &[u32],
    idx: usize,
    expected_kind: u32,
) {
    let pg_kind = pg_word_at(pg, idx, 0);
    let pg_start = pg_word_at(pg, idx, 1);
    let pg_end = pg_word_at(pg, idx, 2);
    let pg_parent = pg_word_at(pg, idx, 3);
    let pg_first_child = pg_word_at(pg, idx, 4);
    let pg_next_sibling = pg_word_at(pg, idx, 5);

    let vast_kind = word_at(typed_vast, idx * VAST_STRIDE_U32);
    let vast_parent = word_at(typed_vast, idx * VAST_STRIDE_U32 + 1);
    let vast_first_child = word_at(typed_vast, idx * VAST_STRIDE_U32 + 2);
    let vast_next_sibling = word_at(typed_vast, idx * VAST_STRIDE_U32 + 3);

    assert_eq!(pg_kind, expected_kind, "PG kind mismatch at row {idx}");
    assert_eq!(pg_kind, vast_kind, "PG/VAST kind drift at row {idx}");
    assert_eq!(
        pg_start, tok_starts[idx],
        "PG span_start mismatch at row {idx}"
    );
    assert_eq!(
        pg_end,
        tok_starts[idx] + tok_lens[idx],
        "PG span_end mismatch at row {idx}"
    );
    assert_eq!(pg_parent, vast_parent, "PG parent drift at row {idx}");
    assert_eq!(
        pg_first_child, vast_first_child,
        "PG first_child drift at row {idx}"
    );
    assert_eq!(
        pg_next_sibling, vast_next_sibling,
        "PG next_sibling drift at row {idx}"
    );
}
