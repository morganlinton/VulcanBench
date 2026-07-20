use super::*;

pub(super) fn reference_c11_annotate_typedef_names_from_words(
    raw_vast_nodes: Vec<u32>,
    haystack: &[u8],
) -> Vec<u8> {
    let node_count = raw_vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let mut annotated = raw_vast_nodes.clone();

    for node_idx in 0..node_count {
        let base = node_idx * VAST_NODE_STRIDE_U32 as usize;
        let raw_kind = kind_at(&raw_vast_nodes, node_idx);
        let name = identifier_lexeme(&raw_vast_nodes, node_idx, haystack);
        let scope_open = scope_open_before(&raw_vast_nodes, node_idx);
        let mut flags = 0u32;
        let decl_kind = declaration_kind_at(&raw_vast_nodes, node_idx, haystack);

        if raw_kind == TOK_IDENTIFIER && decl_kind == 0 {
            if let Some(name) = name {
                let visible_kind =
                    visible_declaration_kind(&raw_vast_nodes, node_idx, haystack, name);
                if visible_kind == 1 {
                    flags |= C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME;
                }
            }
        }

        match decl_kind {
            1 => flags |= C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR,
            2 => flags |= C_TYPEDEF_FLAG_ORDINARY_DECLARATOR,
            _ => {}
        }

        annotated[base + VAST_TYPEDEF_FLAGS_FIELD as usize] = flags;
        annotated[base + VAST_TYPEDEF_SCOPE_FIELD as usize] = scope_open;
        annotated[base + VAST_TYPEDEF_SYMBOL_FIELD as usize] = name.map(fnv1a32).unwrap_or(0);
    }

    u32_words_to_bytes(&annotated)
}
