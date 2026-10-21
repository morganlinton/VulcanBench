use super::*;

/// Decode framed AST windows from a compiled `vyre-frontend-c` object.
pub fn decode_object_ast(object_bytes: &[u8]) -> Result<CObjectAst, String> {
    decode_embedded_object(object_bytes, decode_object_ast_from_container)
}

pub(crate) fn decode_object_ast_from_container(
    container: &Vyrecob2<'_>,
) -> Result<CObjectAst, String> {
    let ast_section = container.section(SectionTag::Ast).ok_or_else(|| {
        "vyre-frontend-c object is missing Ast. Fix: compile with AST emission enabled.".to_string()
    })?;
    let windows = decode_ast_windows(ast_section)?;
    let ast_node_count = windows.iter().try_fold(0u64, |acc, window| {
        let nodes = checked_count_u64(window.ast_words.len() / 4, "AST window node count")?;
        acc.checked_add(nodes).ok_or_else(|| {
            "vyre-frontend-c AST decoded node count overflows u64. Fix: shard AST object sections."
                .to_string()
        })
    })?;
    Ok(CObjectAst {
        vyrecob2_version: container.version,
        windows,
        ast_node_count,
    })
}

/// Read and decode framed AST windows from a compiled object path.
pub fn decode_object_ast_file(path: &Path) -> Result<CObjectAst, String> {
    read_object_file(path, decode_object_ast)
}

pub(super) fn decode_ast_windows(section: &[u8]) -> Result<Vec<CObjectAstWindow>, String> {
    const MAGIC: &[u8; 8] = b"VYRAST1\0";
    if section.len() < MAGIC.len() || &section[..MAGIC.len()] != MAGIC {
        return Err(
            "vyre-frontend-c AST section is not framed as VYRAST1. Fix: recompile with the v9 AST section writer."
                .to_string(),
        );
    }
    let mut offset = MAGIC.len();
    let mut windows = Vec::new();
    while offset < section.len() {
        let token_start = read_ast_u32(section, offset, "token_start")?;
        let token_count = read_ast_u32(section, offset + 4, "token_count")?;
        let ast_word_count = read_ast_u32(section, offset + 8, "ast_word_count")?;
        let root_count = read_ast_u32(section, offset + 12, "root_count")?;
        offset += 16;
        if ast_word_count % 4 != 0 {
            return Err(format!(
                "vyre-frontend-c AST window at token {token_start} has {ast_word_count} AST words, not a whole number of 4-word AST nodes. Fix: repair ast_shunting_yard count emission."
            ));
        }
        if ast_word_count == 0 {
            return Err(format!(
                "vyre-frontend-c AST window at token {token_start} is empty. Fix: regenerate the object; every AST window must carry at least one node."
            ));
        }
        if root_count == 0 {
            return Err(format!(
                "vyre-frontend-c AST window at token {token_start} has no root entries. Fix: regenerate the object; every AST window must carry root evidence."
            ));
        }
        let _token_end = token_start.checked_add(token_count).ok_or_else(|| {
            format!(
                "vyre-frontend-c AST window token range {token_start}+{token_count} overflows u32. Fix: regenerate the object with bounded token windows."
            )
        })?;
        let ast_byte_len = usize::try_from(ast_word_count)
            .ok()
            .and_then(|words| words.checked_mul(4))
            .ok_or_else(|| {
                "vyre-frontend-c AST window byte length overflowed usize. Fix: shard AST object sections."
                    .to_string()
            })?;
        let root_byte_len = usize::try_from(root_count)
            .ok()
            .and_then(|roots| roots.checked_mul(4))
            .ok_or_else(|| {
                "vyre-frontend-c AST root byte length overflowed usize. Fix: shard AST object sections."
                    .to_string()
            })?;
        let ast_end = offset.checked_add(ast_byte_len).ok_or_else(|| {
            "vyre-frontend-c AST window byte length overflowed usize. Fix: shard AST object sections."
                .to_string()
        })?;
        let root_end = ast_end.checked_add(root_byte_len).ok_or_else(|| {
            "vyre-frontend-c AST root byte length overflowed usize. Fix: shard AST object sections."
                .to_string()
        })?;
        if root_end > section.len() {
            return Err(format!(
                "vyre-frontend-c AST window at token {token_start} is truncated: need byte {root_end}, section has {}. Fix: regenerate the object.",
                section.len()
            ));
        }
        windows.push(CObjectAstWindow {
            token_start,
            token_count,
            ast_words: decode_u32_words(&section[offset..ast_end])?,
            root_words: decode_u32_words(&section[ast_end..root_end])?,
        });
        offset = root_end;
    }
    if windows.is_empty() {
        return Err(
            "vyre-frontend-c AST section contains no VYRAST1 windows. Fix: regenerate the object; AST emission must produce at least one window."
                .to_string(),
        );
    }
    Ok(windows)
}

pub(super) fn read_ast_u32(bytes: &[u8], offset: usize, label: &str) -> Result<u32, String> {
    let end = offset.checked_add(4).ok_or_else(|| {
        format!(
            "vyre-frontend-c AST section offset overflow while reading {label} at byte {offset}. Fix: regenerate the object."
        )
    })?;
    if end > bytes.len() {
        return Err(format!(
            "vyre-frontend-c AST section truncated while reading {label} at byte {offset}. Fix: regenerate the object."
        ));
    }
    Ok(u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]))
}
