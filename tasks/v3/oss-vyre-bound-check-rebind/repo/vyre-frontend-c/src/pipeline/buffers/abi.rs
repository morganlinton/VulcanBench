use vyre_libs::compiler::types_layout::{C_ABI_CHAR, C_ABI_DOUBLE, C_ABI_LONG, C_ABI_POINTER};
use vyre_libs::parsing::c::lex::tokens::{
    TOK_CHAR_KW, TOK_DOUBLE, TOK_FLOAT_KW, TOK_INT, TOK_LONG, TOK_SHORT, TOK_STAR, TOK_VOID,
};

#[inline]
fn c_abi_type_kind(tok: u32) -> Option<u32> {
    match tok {
        TOK_CHAR_KW => Some(C_ABI_CHAR),
        TOK_STAR => Some(C_ABI_POINTER),
        TOK_LONG => Some(C_ABI_LONG),
        TOK_DOUBLE => Some(C_ABI_DOUBLE),
        TOK_INT | TOK_SHORT | TOK_FLOAT_KW | TOK_VOID => Some(0),
        _ => None,
    }
}

pub(super) fn c_abi_type_table_bytes_from_tokens_into(tok_types: &[u32], bytes: &mut Vec<u8>) {
    bytes.clear();
    bytes.reserve(tok_types.len().max(1).saturating_mul(4));
    let mut emitted = false;
    for tok in tok_types.iter().copied() {
        if let Some(kind) = c_abi_type_kind(tok) {
            bytes.extend_from_slice(&kind.to_le_bytes());
            emitted = true;
        }
    }
    if !emitted {
        bytes.extend_from_slice(&0u32.to_le_bytes());
    }
}
