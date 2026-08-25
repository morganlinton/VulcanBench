use super::*;

pub(in crate::pipeline) struct DecodedC11Tokens {
    pub(in crate::pipeline) tok_types: Vec<u32>,
    pub(in crate::pipeline) start_words: Vec<u32>,
    pub(in crate::pipeline) len_words: Vec<u32>,
    pub(in crate::pipeline) starts_logical: Vec<u8>,
    pub(in crate::pipeline) lens_logical: Vec<u8>,
    pub(in crate::pipeline) types_logical: Vec<u8>,
    pub(in crate::pipeline) n_tokens: u32,
    pub(in crate::pipeline) nt: u32,
}

pub(in crate::pipeline) fn decode_c11_tokens(
    path: &Path,
    source: &str,
    types: &[u8],
    starts: &[u8],
    lens: &[u8],
    n_tokens: u32,
    mut log: impl FnMut(&str),
) -> Result<DecodedC11Tokens, String> {
    let n_tokens = reconcile_compacted_token_count(types, starts, lens, n_tokens)?;
    let mut tok_types = token_types_from_lex(types, n_tokens)?;
    let mut start_words = read_u32_stream(starts, n_tokens as usize, "token starts")?;
    let mut len_words = read_u32_stream(lens, n_tokens as usize, "token lengths")?;
    trim_decoded_zero_tail(&mut tok_types, &mut start_words, &mut len_words)?;
    let n_tokens = u32::try_from(tok_types.len()).map_err(|error| {
        format!(
            "decoded token count {} does not fit u32 after zero-tail trim: {error}.",
            tok_types.len()
        )
    })?;
    repair_token_spans_from_source(source, &tok_types, &mut start_words, &mut len_words)?;
    log("host token decode/repair/diagnostics");
    let starts_logical = vec_u32_le_bytes_min_words(&start_words, n_tokens.max(1))?;
    let lens_logical = vec_u32_le_bytes_min_words(&len_words, n_tokens.max(1))?;
    reject_c11_lexer_diagnostics(path, &tok_types, &starts_logical, &lens_logical)?;
    let types_logical = logical_type_bytes_from_lex_buffer(types, n_tokens)?;
    Ok(DecodedC11Tokens {
        tok_types,
        start_words,
        len_words,
        starts_logical,
        lens_logical,
        types_logical,
        n_tokens,
        nt: n_tokens.max(1),
    })
}

fn logical_type_bytes_from_lex_buffer(types: &[u8], n_tokens: u32) -> Result<Vec<u8>, String> {
    if n_tokens == 0 {
        return Ok(vec![0; 4]);
    }
    let logical_bytes = usize::try_from(n_tokens)
        .ok()
        .and_then(|tokens| tokens.checked_mul(4))
        .ok_or_else(|| {
            format!(
                "token type logical byte length overflows host indexing for n_tokens={n_tokens}. Fix: shard the token stream before materialization."
            )
        })?;
    if types.len() < logical_bytes {
        return Err(format!(
            "token type buffer has {} bytes, need {logical_bytes} for {n_tokens} logical token(s). Fix: keep lexer output buffers aligned before token materialization.",
            types.len()
        ));
    }
    Ok(types[..logical_bytes].to_vec())
}

fn reconcile_compacted_token_count(
    types: &[u8],
    starts: &[u8],
    lens: &[u8],
    n_tokens: u32,
) -> Result<u32, String> {
    let mut effective = usize::try_from(n_tokens).map_err(|error| {
        format!(
            "token count {n_tokens} does not fit host indexing: {error}. Fix: shard token materialization."
        )
    })?;
    while effective > 0 {
        let row = effective - 1;
        let ty = read_u32_at(types, row * 4)?;
        let start = read_u32_at(starts, row * 4)?;
        let len = read_u32_at(lens, row * 4)?;
        if ty != 0 {
            break;
        }
        if start != 0 || len != 0 {
            return Err(format!(
                "token {row} has zero type but nonzero span start={start} len={len}. Fix: the GPU C lexer must keep token type/span columns aligned."
            ));
        }
        effective -= 1;
    }
    for row in 0..effective {
        let ty = read_u32_at(types, row * 4)?;
        if ty == 0 {
            let start = read_u32_at(starts, row * 4)?;
            let len = read_u32_at(lens, row * 4)?;
            return Err(format!(
                "token {row} has invalid interior zero type start={start} len={len}. Fix: sparse compaction must not leave holes inside the declared token prefix."
            ));
        }
    }
    u32::try_from(effective).map_err(|error| {
        format!(
            "effective token count {effective} does not fit u32 after compaction reconciliation: {error}."
        )
    })
}

fn trim_decoded_zero_tail(
    tok_types: &mut Vec<u32>,
    starts: &mut Vec<u32>,
    lens: &mut Vec<u32>,
) -> Result<(), String> {
    while tok_types.last() == Some(&0) {
        let row = tok_types.len() - 1;
        let start = starts.get(row).copied().unwrap_or(u32::MAX);
        let len = lens.get(row).copied().unwrap_or(u32::MAX);
        if start != 0 || len != 0 {
            return Err(format!(
                "token {row} has zero type but nonzero decoded span start={start} len={len}. Fix: the GPU C lexer must keep decoded token columns aligned."
            ));
        }
        tok_types.pop();
        starts.pop();
        lens.pop();
    }
    for (row, ty) in tok_types.iter().copied().enumerate() {
        if ty == 0 {
            return Err(format!(
                "token {row} has invalid interior zero type after decode. Fix: sparse compaction and keyword promotion must not leave holes inside the token prefix."
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        logical_type_bytes_from_lex_buffer, reconcile_compacted_token_count, trim_decoded_zero_tail,
    };

    #[test]
    fn logical_type_bytes_reuses_lex_prefix_without_repacking() {
        let bytes = vec![1, 0, 0, 0, 2, 0, 0, 0, 99, 99, 99, 99];
        assert_eq!(
            logical_type_bytes_from_lex_buffer(&bytes, 2).unwrap(),
            vec![1, 0, 0, 0, 2, 0, 0, 0]
        );
    }

    #[test]
    fn logical_type_bytes_zero_tokens_emit_single_zero_word() {
        assert_eq!(
            logical_type_bytes_from_lex_buffer(&[], 0).unwrap(),
            vec![0; 4]
        );
    }

    #[test]
    fn reconcile_compacted_token_count_trims_zero_tail_capacity() {
        let types = pack_words(&[10, 20, 0, 0]);
        let starts = pack_words(&[0, 5, 0, 0]);
        let lens = pack_words(&[3, 2, 0, 0]);

        assert_eq!(
            reconcile_compacted_token_count(&types, &starts, &lens, 4).unwrap(),
            2
        );
    }

    #[test]
    fn reconcile_compacted_token_count_rejects_interior_zero_holes() {
        let types = pack_words(&[10, 0, 30]);
        let starts = pack_words(&[0, 4, 8]);
        let lens = pack_words(&[1, 1, 1]);

        let error = reconcile_compacted_token_count(&types, &starts, &lens, 3)
            .expect_err("Fix: zero token holes inside compacted prefix must be rejected.");
        assert!(error.contains("interior zero type"));
    }

    #[test]
    fn trim_decoded_zero_tail_removes_zero_capacity_rows() {
        let mut types = vec![10, 20, 0, 0];
        let mut starts = vec![0, 5, 0, 0];
        let mut lens = vec![3, 2, 0, 0];

        trim_decoded_zero_tail(&mut types, &mut starts, &mut lens).unwrap();

        assert_eq!(types, vec![10, 20]);
        assert_eq!(starts, vec![0, 5]);
        assert_eq!(lens, vec![3, 2]);
    }

    // Canonical LE-pack — no hand-rolled reimplementation (the production path in
    // `api/lex_index.rs` aliases the same wire helper). wire's bytemuck fast-path
    // also beats the old `flat_map(to_le_bytes)` on little-endian hosts.
    use vyre_primitives::wire::pack_u32_slice as pack_words;
}
