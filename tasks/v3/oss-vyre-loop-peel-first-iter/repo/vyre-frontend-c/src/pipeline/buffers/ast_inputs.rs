use super::*;
#[derive(Debug, Default)]
pub(crate) struct AstOwnedInputBuffers {
    pub(crate) tok_b: Vec<u8>,
    pub(crate) out_ast: Vec<u8>,
    pub(crate) out_cnt: Vec<u8>,
    pub(crate) out_roots: Vec<u8>,
    pub(crate) scratch_v: Vec<u8>,
    pub(crate) scratch_o: Vec<u8>,
}

#[cfg(test)]
pub(crate) fn build_ast_owned_inputs_with_capacity(
    tok_types: &[u32],
    num_stmt: u32,
    token_capacity: u32,
) -> Result<AstOwnedInputBuffers, String> {
    let mut buffers = AstOwnedInputBuffers::default();
    build_ast_owned_inputs_with_capacity_into(tok_types, num_stmt, token_capacity, &mut buffers)?;
    Ok(buffers)
}

pub(crate) fn build_ast_owned_inputs_with_capacity_into(
    tok_types: &[u32],
    num_stmt: u32,
    token_capacity: u32,
    buffers: &mut AstOwnedInputBuffers,
) -> Result<(), String> {
    if token_capacity == 0 {
        return Err(
            "ast_shunting_yard token capacity is zero. Fix: pass a non-zero token window."
                .to_string(),
        );
    }
    if token_capacity > MAX_TOK_SCAN {
        return Err(format!(
            "ast_shunting_yard token capacity {token_capacity} exceeds MAX_TOK_SCAN {MAX_TOK_SCAN}. Fix: window the translation unit explicitly instead of clamping and truncating tokens."
        ));
    }
    if tok_types.len() > token_capacity as usize {
        return Err(format!(
            "ast_shunting_yard input has {} tokens but capacity is {token_capacity}. Fix: pass a matching token window; silent token truncation is forbidden.",
            tok_types.len()
        ));
    }
    let token_capacity_usize = usize::try_from(token_capacity).map_err(|error| {
        format!("ast_shunting_yard token capacity {token_capacity} does not fit usize: {error}")
    })?;
    let token_bytes = token_capacity_usize.checked_mul(4).ok_or_else(|| {
        format!("ast_shunting_yard token capacity {token_capacity} overflows byte size")
    })?;
    buffers.tok_b.clear();
    buffers.tok_b.resize(token_bytes, 0);
    let packed_len = tok_types.len().checked_mul(4).ok_or_else(|| {
        format!(
            "ast_shunting_yard token count {} overflows byte size",
            tok_types.len()
        )
    })?;
    #[cfg(target_endian = "little")]
    {
        buffers.tok_b[..packed_len].copy_from_slice(bytemuck::cast_slice(tok_types));
    }
    #[cfg(target_endian = "big")]
    for (index, token) in tok_types.iter().enumerate() {
        buffers.tok_b[index * 4..index * 4 + 4].copy_from_slice(&token.to_le_bytes());
    }
    let out_ast_bytes = token_capacity_usize
        .checked_mul(4)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!("ast_shunting_yard AST output for token capacity {token_capacity} overflows byte size")
        })?;
    buffers.out_ast.clear();
    buffers.out_ast.resize(out_ast_bytes, 0);
    buffers.out_cnt.clear();
    buffers.out_cnt.resize(4, 0);
    let roots_words = num_stmt.max(1);
    let roots_bytes = usize::try_from(roots_words)
        .ok()
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!("ast_shunting_yard root word count {roots_words} overflows byte size")
        })?;
    buffers.out_roots.clear();
    buffers.out_roots.resize(roots_bytes, 0);
    let scratch_words = num_stmt.checked_mul(64).ok_or_else(|| {
        format!("ast_shunting_yard num_stmt={num_stmt} overflows scratch word count. Fix: window the translation unit before AST lowering.")
    })?.max(64);
    let scratch_bytes = usize::try_from(scratch_words)
        .ok()
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            format!("ast_shunting_yard scratch word count {scratch_words} overflows byte size")
        })?;
    buffers.scratch_v.clear();
    buffers.scratch_v.resize(scratch_bytes, 0);
    buffers.scratch_o.clear();
    buffers.scratch_o.resize(scratch_bytes, 0);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ast_inputs_reject_token_stream_larger_than_capacity() {
        let err = build_ast_owned_inputs_with_capacity(&[1, 2], 1, 1)
            .expect_err("token truncation must fail");
        assert!(
            err.contains("silent token truncation is forbidden"),
            "{err}"
        );
    }

    #[test]
    fn ast_inputs_reject_capacity_above_fixed_ast_window() {
        let err = build_ast_owned_inputs_with_capacity(&[1], 1, MAX_TOK_SCAN + 1)
            .expect_err("implicit capacity clamp must fail");
        assert!(err.contains("exceeds MAX_TOK_SCAN"), "{err}");
        assert!(err.contains("instead of clamping"), "{err}");
    }
}
