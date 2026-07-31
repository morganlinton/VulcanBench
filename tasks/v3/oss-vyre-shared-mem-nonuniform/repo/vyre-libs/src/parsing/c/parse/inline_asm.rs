use crate::compiler::atomic_collect::atomic_collect_u32;
use crate::parsing::c::lex::tokens::{
    TOK_COLON, TOK_GNU_ASM, TOK_GOTO, TOK_LPAREN, TOK_RPAREN, TOK_STRING, TOK_VOLATILE,
};
use vyre::ir::{Expr, Program};

/// Front-end opcode for a GNU inline-asm AST row.
pub const GNU_INLINE_ASM_OPCODE: u32 = 0x4153_4D00;
const OP_ID: &str = "vyre-libs::parsing::c11_gnu_inline_asm_pass";

/// Token-level summary for a GNU inline assembly statement or declaration alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnuInlineAsmSummary {
    /// Token index containing `asm`, `__asm`, or `__asm__`.
    pub asm_token: usize,
    /// Whether `volatile` / `__volatile__` was present before the operand list.
    pub is_volatile: bool,
    /// Whether `goto` was present before the operand list.
    pub is_goto: bool,
    /// Token index of the template string.
    pub template_token: usize,
    /// One-past-last token index of the asm construct.
    pub end_token: usize,
    /// Number of top-level colon separators in the operand list.
    pub top_level_colons: u32,
}

/// Fail-loud inline-asm parser error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnuInlineAsmError {
    /// Token index where parsing failed.
    pub token_index: usize,
    /// Actionable diagnostic.
    pub message: &'static str,
}

impl core::fmt::Display for GnuInlineAsmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} at token {}", self.message, self.token_index)
    }
}

impl std::error::Error for GnuInlineAsmError {}

/// Parse the token envelope of a GNU inline-asm construct.
///
/// The parser validates the GNU shape `asm [volatile] [goto] ( "template" ... )`
/// without interpreting architecture-specific template text. That keeps asm
/// payloads opaque for ABI lowering while still giving the C frontend stable
/// spans and fail-loud malformed-stream behavior.
///
/// # Errors
///
/// Returns an actionable error for a malformed or truncated asm envelope.
pub fn try_classify_gnu_inline_asm_tokens(
    tok_types: &[u32],
    asm_token: usize,
) -> Result<GnuInlineAsmSummary, GnuInlineAsmError> {
    if tok_types.get(asm_token).copied() != Some(TOK_GNU_ASM) {
        return Err(GnuInlineAsmError {
            token_index: asm_token,
            message: "Fix: GNU inline asm parser must start at TOK_GNU_ASM",
        });
    }

    let mut cursor = asm_token + 1;
    let mut is_volatile = false;
    let mut is_goto = false;
    while let Some(kind) = tok_types.get(cursor).copied() {
        match kind {
            TOK_VOLATILE => {
                is_volatile = true;
                cursor += 1;
            }
            TOK_GOTO => {
                is_goto = true;
                cursor += 1;
            }
            _ => break,
        }
    }

    if tok_types.get(cursor).copied() != Some(TOK_LPAREN) {
        return Err(GnuInlineAsmError {
            token_index: cursor,
            message: "Fix: GNU inline asm requires an opening parenthesis",
        });
    }

    let template_token = cursor + 1;
    if tok_types.get(template_token).copied() != Some(TOK_STRING) {
        return Err(GnuInlineAsmError {
            token_index: template_token,
            message: "Fix: GNU inline asm requires a string template as the first operand",
        });
    }

    let mut depth = 1u32;
    let mut top_level_colons = 0u32;
    cursor += 1;
    while cursor + 1 < tok_types.len() {
        cursor += 1;
        match tok_types[cursor] {
            TOK_LPAREN => depth = depth.saturating_add(1),
            TOK_RPAREN => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(GnuInlineAsmSummary {
                        asm_token,
                        is_volatile,
                        is_goto,
                        template_token,
                        end_token: cursor + 1,
                        top_level_colons,
                    });
                }
            }
            TOK_COLON if depth == 1 => top_level_colons = top_level_colons.saturating_add(1),
            _ => {}
        }
    }

    Err(GnuInlineAsmError {
        token_index: tok_types.len(),
        message: "Fix: GNU inline asm operand list is missing its closing parenthesis",
    })
}

/// GNU Compiler Extensions: Inline Assembly Parser
///
/// GNU-C code often uses `asm volatile(...)` blocks for architecture-specific
/// hardware control. This module isolates inline assembly tokens and passes
/// the raw strings to an architecture-specific assembler block during ABI
/// lowering, preventing the C semantic analyzer from treating assembler text
/// as ordinary C expressions.
#[must_use]
pub fn c11_gnu_inline_asm_pass(
    ast_opcodes: &str,
    out_asm_blocks: &str,
    num_ast_nodes: Expr,
) -> Program {
    atomic_collect_u32(
        OP_ID,
        ast_opcodes,
        out_asm_blocks,
        "out_asm_counts",
        num_ast_nodes,
        1,
        Some("inline-asm-registry-overflow"),
        |opcode, _t| Expr::eq(opcode, Expr::u32(GNU_INLINE_ASM_OPCODE)),
        |_t, asm_id| asm_id,
        |t, _asm_id| t,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || c11_gnu_inline_asm_pass("ast", "out_asm", Expr::u32(4)),
        // ast: 4 u32 opcodes including one ASM tag (0x41534D00) at
        // index 2. out_asm: 4 u32 slots. out_asm_counts: 1 u32 slot
        // for the atomic counter. The pass writes t=2 into
        // out_asm[0] and leaves non-ASM slots untouched.
        test_inputs: Some(|| {
            let ast = [0u32, 1, GNU_INLINE_ASM_OPCODE, 3];
            let ast_bytes = vyre_primitives::wire::pack_u32_slice(&ast);
            vec![vec![ast_bytes, vec![0u8; 4 * 4], vec![0u8; 4]]]
        }),
        expected_output: Some(|| {
            // t=2 sees the ASM tag, atomic_add claims slot 0, and
            // we store `t=2` into out_asm_blocks[0]. All other
            // slots stay zero. The counter records the single asm block.
            let mut out = vec![0u8; 4 * 4];
            out[0..4].copy_from_slice(&2u32.to_le_bytes());
            let mut count = vec![0u8; 4];
            count.copy_from_slice(&1u32.to_le_bytes());
            vec![vec![out, count]]
        }),
        category: Some("parsing"),
    }
}
