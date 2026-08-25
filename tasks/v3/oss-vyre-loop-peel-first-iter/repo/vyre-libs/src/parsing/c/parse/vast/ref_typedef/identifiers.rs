use super::*;
pub(super) use vyre_primitives::hash::fnv1a::fnv1a32;

pub(super) fn identifier_lexeme<'a>(
    vast_nodes: &[u32],
    node_idx: usize,
    haystack: &'a [u8],
) -> Option<&'a [u8]> {
    if kind_at(vast_nodes, node_idx) != TOK_IDENTIFIER {
        return None;
    }
    let start = vast_field_at(vast_nodes, node_idx, 5) as usize;
    let len = vast_field_at(vast_nodes, node_idx, 6) as usize;
    haystack.get(start..start.saturating_add(len))
}

pub(super) fn is_gnu_typeof_hash_raw(hash: u32) -> bool {
    C_GNU_TYPEOF_HASHES.contains(&hash)
}

pub(super) fn is_gnu_auto_type_hash_raw(hash: u32) -> bool {
    hash == C_GNU_AUTO_TYPE_HASH
}

pub(super) fn symbol_hash_at(vast_nodes: &[u32], node_idx: usize) -> u32 {
    vast_field_at(vast_nodes, node_idx, VAST_TYPEDEF_SYMBOL_FIELD as usize)
}

pub(super) fn is_typeof_operator_raw(kind: u32, symbol_hash: u32) -> bool {
    matches!(kind, TOK_GNU_TYPEOF | TOK_GNU_TYPEOF_UNQUAL)
        || (kind == TOK_IDENTIFIER && is_gnu_typeof_hash_raw(symbol_hash))
}

pub(super) fn is_decl_prefix_at(vast_nodes: &[u32], node_idx: usize) -> bool {
    let kind = kind_at(vast_nodes, node_idx);
    let symbol_hash = symbol_hash_at(vast_nodes, node_idx);
    is_decl_prefix_raw(kind)
        || is_typeof_operator_raw(kind, symbol_hash)
        || (kind == TOK_IDENTIFIER && is_gnu_auto_type_hash_raw(symbol_hash))
}
