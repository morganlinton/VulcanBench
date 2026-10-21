use super::*;

pub(crate) fn row_indices(rows: &[u8], kind: u32) -> Vec<usize> {
    row_indices_by_stride(rows, VAST_STRIDE_U32, kind)
}

pub(crate) fn row_indices_by_stride(rows: &[u8], stride_words: usize, kind: u32) -> Vec<usize> {
    rows.chunks_exact(stride_words * core::mem::size_of::<u32>())
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == kind).then_some(idx)
        })
        .collect()
}

pub(crate) fn lexeme_indices(fix: &Fixture, lexeme: &str) -> Vec<usize> {
    fix.tok_starts
        .iter()
        .zip(&fix.tok_lens)
        .enumerate()
        .filter_map(|(idx, (start, len))| {
            let start = *start as usize;
            let end = start.saturating_add(*len as usize);
            (fix.source.as_bytes().get(start..end) == Some(lexeme.as_bytes())).then_some(idx)
        })
        .collect()
}

pub(crate) fn token_indices_containing(fix: &Fixture, needle: &str) -> Vec<usize> {
    fix.tok_starts
        .iter()
        .zip(&fix.tok_lens)
        .enumerate()
        .filter_map(|(idx, (start, len))| {
            let start = *start as usize;
            let end = start.saturating_add(*len as usize);
            let token = fix.source.as_bytes().get(start..end)?;
            token
                .windows(needle.len())
                .any(|window| window == needle.as_bytes())
                .then_some(idx)
        })
        .collect()
}

pub(crate) fn starts_for_lens(lens: &[u32]) -> Vec<u32> {
    let mut cursor = 0u32;
    lens.iter()
        .map(|len| {
            let start = cursor;
            cursor = cursor.saturating_add(*len).saturating_add(1);
            start
        })
        .collect()
}

pub(crate) fn pg_word_at(buf: &[u8], idx: usize, field: usize) -> u32 {
    word_at(buf, idx * PG_STRIDE_U32 + field)
}

pub(crate) fn kind_at(rows: &[u8], idx: usize) -> u32 {
    word_at(rows, idx * VAST_STRIDE_U32)
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

pub(crate) fn assert_words_eq(actual: &[u8], expected: &[u8], context: &str) {
    if actual == expected {
        return;
    }
    let limit = (actual.len() / 4).min(expected.len() / 4);
    for w in 0..limit {
        let a = word_at(actual, w);
        let e = word_at(expected, w);
        if a != e {
            let row = w / VAST_STRIDE_U32;
            let actual_row: Vec<u32> = (0..VAST_STRIDE_U32)
                .map(|field| word_at(actual, row * VAST_STRIDE_U32 + field))
                .collect();
            let expected_row: Vec<u32> = (0..VAST_STRIDE_U32)
                .map(|field| word_at(expected, row * VAST_STRIDE_U32 + field))
                .collect();
            let nearby_start = row.saturating_sub(3);
            let nearby_end = (row + 4).min(limit / VAST_STRIDE_U32);
            let nearby_actual: Vec<Vec<u32>> = (nearby_start..nearby_end)
                .map(|nearby_row| {
                    (0..VAST_STRIDE_U32)
                        .map(|field| word_at(actual, nearby_row * VAST_STRIDE_U32 + field))
                        .collect()
                })
                .collect();
            panic!(
                "{context}: word {w} differs (row={row}, field={}): actual={a}, expected={e}; actual_row={actual_row:?}; expected_row={expected_row:?}; nearby_actual_start={nearby_start}; nearby_actual={nearby_actual:?}",
                w % VAST_STRIDE_U32
            );
        }
    }
    panic!(
        "{context}: byte lengths differ: actual={}, expected={}",
        actual.len(),
        expected.len()
    );
}
