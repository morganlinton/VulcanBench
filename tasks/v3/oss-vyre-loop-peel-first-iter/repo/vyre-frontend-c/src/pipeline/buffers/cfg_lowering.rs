use super::*;

pub(crate) fn c_abi_type_table_bytes_into(tok_types: &[u32], bytes: &mut Vec<u8>) {
    abi::c_abi_type_table_bytes_from_tokens_into(tok_types, bytes);
}

pub(crate) fn cfg_ssa_words_from_vast(vast_blob: &[u8]) -> Result<Vec<u32>, String> {
    const VAST_NODE_STRIDE_U32: usize = 10;
    const IDX_KIND: usize = 0;
    const IDX_NEXT_SIBLING: usize = 3;
    const IDX_SYMBOL_HASH: usize = 9;
    const SSA_LABEL_OPCODE: u32 = 0x4C41_424C;
    const SSA_GOTO_OPCODE: u32 = 0x474F_544F;

    if vast_blob.len() % 4 != 0 {
        return Err(format!(
            "typed VAST blob length must be u32-aligned before CFG lowering: {} bytes",
            vast_blob.len()
        ));
    }
    let row_count = vast_blob.len() / (VAST_NODE_STRIDE_U32 * 4);
    let mut ssa = Vec::new();
    for row_index in 0..row_count {
        match packed_u32_at(vast_blob, row_index * VAST_NODE_STRIDE_U32 + IDX_KIND) {
            C_AST_KIND_LABEL_STMT => {
                let hash = packed_u32_at(
                    vast_blob,
                    row_index * VAST_NODE_STRIDE_U32 + IDX_SYMBOL_HASH,
                );
                if hash != 0 {
                    ssa.extend_from_slice(&[SSA_LABEL_OPCODE, hash]);
                }
            }
            C_AST_KIND_GOTO_STMT => {
                let target_idx = packed_u32_at(
                    vast_blob,
                    row_index * VAST_NODE_STRIDE_U32 + IDX_NEXT_SIBLING,
                ) as usize;
                let target_hash = if target_idx < row_count {
                    packed_u32_at(
                        vast_blob,
                        target_idx * VAST_NODE_STRIDE_U32 + IDX_SYMBOL_HASH,
                    )
                } else {
                    0
                };
                if target_hash != 0 {
                    ssa.extend_from_slice(&[SSA_GOTO_OPCODE, target_hash]);
                }
            }
            _ => {}
        }
    }
    if ssa.is_empty() {
        ssa.push(0);
    }
    Ok(ssa)
}

fn packed_u32_at(bytes: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
