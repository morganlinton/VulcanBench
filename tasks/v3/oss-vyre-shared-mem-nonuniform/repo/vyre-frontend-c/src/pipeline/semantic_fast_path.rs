use super::*;
pub(super) fn semantic_control_edges_required(token_types: &[u32]) -> bool {
    token_types.iter().any(|&token| {
        matches!(
            token,
            TOK_CASE | TOK_DEFAULT | TOK_GNU_LABEL | TOK_GOTO | TOK_SWITCH
        )
    })
}

pub(super) fn conditional_expression_shapes_required(token_types: &[u32]) -> bool {
    token_types.iter().any(|&token| token == TOK_QUESTION)
}

pub(super) fn c_global_typedef_fast_hashes(
    source: &[u8],
    token_types: &[u32],
    starts: &[u32],
    lens: &[u32],
) -> Option<Vec<u32>> {
    if token_types.len() != starts.len() || token_types.len() != lens.len() {
        return None;
    }
    let mut typedefs = Vec::<(u32, &[u8])>::new();
    let mut brace_depth = 0u32;
    let mut in_global_typedef = false;
    let mut current_typedef = None::<(u32, &[u8])>;
    for (idx, &token) in token_types.iter().enumerate() {
        if brace_depth > 0 && token == TOK_TYPEDEF {
            return None;
        }
        if brace_depth == 0 && token == TOK_TYPEDEF {
            in_global_typedef = true;
            current_typedef = None;
        } else if in_global_typedef && token == TOK_IDENTIFIER {
            let next = token_types.get(idx + 1).copied().unwrap_or(TOK_EOF);
            if c_possible_declarator_follower(next) {
                if current_typedef.is_some() {
                    return None;
                }
                let name = source_span_bytes(source, starts[idx], lens[idx])?;
                current_typedef = Some((fnv1a32_bytes(name), name));
            }
        } else if in_global_typedef && token == TOK_SEMICOLON {
            let typedef = current_typedef.take()?;
            if typedefs
                .iter()
                .any(|(hash, name)| *hash == typedef.0 && *name != typedef.1)
            {
                return None;
            }
            if !typedefs
                .iter()
                .any(|(hash, name)| *hash == typedef.0 && *name == typedef.1)
            {
                typedefs.push(typedef);
            }
            in_global_typedef = false;
            current_typedef = None;
        } else if in_global_typedef && matches!(token, TOK_LBRACE | TOK_RBRACE) {
            return None;
        }
        match token {
            TOK_LBRACE => brace_depth = brace_depth.saturating_add(1),
            TOK_RBRACE => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
    }
    if in_global_typedef || typedefs.is_empty() {
        return None;
    }
    for (idx, &token) in token_types.iter().enumerate() {
        if token != TOK_IDENTIFIER {
            continue;
        }
        let name = source_span_bytes(source, starts[idx], lens[idx])?;
        let hash = fnv1a32_bytes(name);
        if typedefs
            .iter()
            .any(|(typedef_hash, typedef_name)| *typedef_hash == hash && *typedef_name != name)
        {
            return None;
        }
    }
    let mut hashes = typedefs
        .iter()
        .map(|(hash, _name)| *hash)
        .collect::<Vec<_>>();
    hashes.sort_unstable();
    brace_depth = 0;
    for (idx, &token) in token_types.iter().enumerate() {
        match token {
            TOK_LBRACE => {
                brace_depth = brace_depth.saturating_add(1);
                continue;
            }
            TOK_RBRACE => {
                brace_depth = brace_depth.saturating_sub(1);
                continue;
            }
            _ => {}
        }
        if brace_depth == 0 || token != TOK_IDENTIFIER {
            continue;
        }
        let next = token_types.get(idx + 1).copied().unwrap_or(TOK_EOF);
        if c_possible_declarator_follower(next) {
            let hash = fnv1a32_source_span(source, starts[idx], lens[idx])?;
            if hashes.binary_search(&hash).is_ok() {
                return None;
            }
        }
    }
    Some(hashes)
}

pub(super) fn c_possible_declarator_follower(token: u32) -> bool {
    matches!(
        token,
        TOK_SEMICOLON
            | TOK_COMMA
            | TOK_ASSIGN
            | TOK_LPAREN
            | TOK_LBRACKET
            | TOK_COLON
            | TOK_RPAREN
            | TOK_RBRACKET
    )
}

pub(super) fn fnv1a32_source_span(source: &[u8], start: u32, len: u32) -> Option<u32> {
    source_span_bytes(source, start, len).map(fnv1a32_bytes)
}

pub(super) fn source_span_bytes(source: &[u8], start: u32, len: u32) -> Option<&[u8]> {
    let start = start as usize;
    let end = start.checked_add(len as usize)?;
    source.get(start..end)
}

pub(super) fn fnv1a32_bytes(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for &byte in bytes {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

#[cfg(test)]
mod typedef_fast_path_tests {
    use super::{
        c_global_typedef_fast_hashes, fnv1a32_bytes, TOK_IDENTIFIER, TOK_LBRACE, TOK_RBRACE,
        TOK_SEMICOLON, TOK_TYPEDEF,
    };

    fn span(source: &str, needle: &str) -> (u32, u32) {
        let start = source.find(needle).unwrap() as u32;
        (start, needle.len() as u32)
    }

    #[test]
    fn global_typedef_fast_path_rejects_distinct_identifier_hash_collision() {
        let source = "typedef int ynO;\nvoid f(){ int Wgca; }\n";
        assert_eq!(fnv1a32_bytes(b"ynO"), fnv1a32_bytes(b"Wgca"));
        let (typedef_start, typedef_len) = span(source, "ynO");
        let (collider_start, collider_len) = span(source, "Wgca");
        let tokens = [
            TOK_TYPEDEF,
            TOK_IDENTIFIER,
            TOK_SEMICOLON,
            TOK_LBRACE,
            TOK_IDENTIFIER,
            TOK_SEMICOLON,
            TOK_RBRACE,
        ];
        let starts = [0, typedef_start, 0, 0, collider_start, 0, 0];
        let lens = [0, typedef_len, 0, 0, collider_len, 0, 0];
        assert!(
            c_global_typedef_fast_hashes(source.as_bytes(), &tokens, &starts, &lens).is_none(),
            "global typedef hash fast path must reject distinct identifier collisions before GPU annotation"
        );
    }

    #[test]
    fn global_typedef_fast_path_accepts_unique_global_typedef_name() {
        let source = "typedef int UniqueType;\n";
        let (typedef_start, typedef_len) = span(source, "UniqueType");
        let tokens = [TOK_TYPEDEF, TOK_IDENTIFIER, TOK_SEMICOLON];
        let starts = [0, typedef_start, 0];
        let lens = [0, typedef_len, 0];
        let hashes = c_global_typedef_fast_hashes(source.as_bytes(), &tokens, &starts, &lens)
            .expect("Fix: unique global typedef should keep the fast path available");
        assert_eq!(hashes, vec![fnv1a32_bytes(b"UniqueType")]);
    }
}
