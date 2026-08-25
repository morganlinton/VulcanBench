use super::*;

pub(crate) fn expanded_classified_from_materialized_outputs(
    expanded: &[Vec<u8>],
    token_count: usize,
    source: &[u8],
) -> Result<ClassifiedTokens, String> {
    if expanded.len() < 6 {
        return Err(format!(
            "named macro expansion materialization: expected at least 6 output buffers before token-column decode, got {}. Fix: preserve the macro expansion ABI outputs.",
            expanded.len()
        ));
    }
    let mut directive_kinds = Vec::new();
    reserve_macro_decode_vec_capacity(
        &mut directive_kinds,
        token_count,
        "expanded directive kind defaults",
    )?;
    directive_kinds.resize(token_count, 0);
    Ok(ClassifiedTokens::from_parts(
        read_u32_words_exact(&expanded[0], token_count, "expanded token types")?,
        read_u32_words_exact(&expanded[1], token_count, "expanded token starts")?,
        read_u32_words_exact(&expanded[2], token_count, "expanded token lengths")?,
        directive_kinds,
        std::sync::Arc::from(source),
    ))
}

pub(crate) fn read_u32_words_exact(
    bytes: &[u8],
    count: usize,
    label: &str,
) -> Result<Vec<u32>, String> {
    let required = count.checked_mul(4).ok_or_else(|| {
        format!("named macro expansion {label} byte length overflow. Fix: shard macro expansion output before ABI decode.")
    })?;
    if bytes.len() < required {
        return Err(format!(
            "named macro expansion {label} buffer too short: need {required} bytes for {count} u32 words, got {}. Fix: backend must return the declared macro expansion token columns.",
            bytes.len()
        ));
    }
    let mut out = Vec::new();
    reserve_macro_decode_vec_capacity(&mut out, count, label)?;
    for idx in 0..count {
        out.push(read_u32_word(bytes, idx, label)?);
    }
    Ok(out)
}

fn reserve_macro_decode_vec_capacity<T>(
    vec: &mut Vec<T>,
    count: usize,
    label: &str,
) -> Result<(), String> {
    vec.try_reserve_exact(count).map_err(|error| {
        format!(
            "named macro expansion {label} allocation failed for {count} elements: {error}. Fix: shard macro expansion output before ABI decode."
        )
    })
}
