use super::*;
pub(super) fn validate_security_cross_sections(index: &CObjectSecurityIndex) -> Result<(), String> {
    let token_count = u32::try_from(index.lex.tokens.len()).map_err(|_| {
        format!(
            "vyre-frontend-c security index has {} tokens, exceeding u32 token-id space. Fix: shard the translation unit.",
            index.lex.tokens.len()
        )
    })?;
    for (row, function) in index.structure.functions.iter().enumerate() {
        require_token_index(
            function.name_token,
            token_count,
            "Functions",
            row,
            "name_token",
        )?;
        require_token_index(
            function.body_start_token,
            token_count,
            "Functions",
            row,
            "body_start_token",
        )?;
        require_token_index(
            function.body_end_token,
            token_count,
            "Functions",
            row,
            "body_end_token",
        )?;
    }
    for (row, call) in index.structure.calls.iter().enumerate() {
        require_token_index(call.callee_token, token_count, "Calls", row, "callee_token")?;
        require_token_index(
            call.args_start_token,
            token_count,
            "Calls",
            row,
            "args_start_token",
        )?;
        require_token_index(
            call.args_end_token,
            token_count,
            "Calls",
            row,
            "args_end_token",
        )?;
    }
    for (row, symbol) in index.sema_scope.symbols().enumerate() {
        if index.symbol_token(symbol).is_none() {
            let end = symbol.token_start.checked_add(symbol.token_len).ok_or_else(|| {
                format!(
                    "vyre-frontend-c semantic scope symbol row {row} byte span overflows u32. Fix: regenerate the object with bounded semantic scope spans."
                )
            })?;
            return Err(format!(
                "vyre-frontend-c semantic scope symbol row {row} at byte span [{}..{}) does not resolve to a Lex token. Fix: regenerate the object; semantic scope rows must reference emitted lexical tokens exactly.",
                symbol.token_start,
                end
            ));
        }
    }
    Ok(())
}

fn require_token_index(
    token: u32,
    token_count: u32,
    section: &str,
    row: usize,
    field: &str,
) -> Result<(), String> {
    if token < token_count {
        return Ok(());
    }
    Err(format!(
        "vyre-frontend-c {section} row {row} field {field} references token {token}, but Lex contains {token_count} tokens. Fix: regenerate the object with cross-section-consistent token ids."
    ))
}
