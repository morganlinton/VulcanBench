/// Canonical op id.
pub const OP_ID: &str = "vyre-libs::parsing::c::preprocess::gpu_if_expression";
/// Canonical binding for the input per-token byte-offset buffer.
pub const BINDING_TOK_STARTS: u32 = 0;
/// Canonical binding for the input per-token byte-length buffer.
pub const BINDING_TOK_LENS: u32 = 1;
/// Canonical binding for the input directive-kinds buffer.
pub const BINDING_DIRECTIVE_KINDS: u32 = 2;
/// Canonical binding for the input source bytes.
pub const BINDING_SOURCE: u32 = 3;
/// Canonical binding for the input packed defined-macro names.
pub const BINDING_MACRO_NAMES_PACKED: u32 = 4;
/// Canonical binding for the input macro-offset table.
pub const BINDING_MACRO_OFFSETS: u32 = 5;
/// Canonical binding for the input object-like macro integer values.
pub const BINDING_MACRO_VALUES: u32 = 6;
/// Canonical binding for the output `directive_values` buffer.
pub const BINDING_DIRECTIVE_VALUES: u32 = 7;

/// Per-thread stack depth.
pub const STACK_DEPTH: u32 = 16;
/// Maximum payload bytes scanned per directive.
pub const MAX_PAYLOAD_BYTES: u32 = 512;
/// Maximum identifier length scanned for macro lookups.
pub const MAX_IDENT_LEN: u32 = 64;

/// Operator-stack token for `(`.
pub const OP_LPAREN: u32 = 1;
/// Operator-stack token for the ternary `?` marker.
pub const OP_TERNARY_Q: u32 = 2;
/// Operator-stack token for logical-or.
pub const OP_LOR: u32 = 3;
/// Operator-stack token for logical-and.
pub const OP_LAND: u32 = 4;
/// Operator-stack token for bitwise-or.
pub const OP_BOR: u32 = 5;
/// Operator-stack token for bitwise-xor.
pub const OP_BXOR: u32 = 6;
/// Operator-stack token for bitwise-and.
pub const OP_BAND: u32 = 7;
/// Operator-stack token for equality comparison.
pub const OP_EQ: u32 = 8;
/// Operator-stack token for inequality comparison.
pub const OP_NE: u32 = 9;
/// Operator-stack token for less-than comparison.
pub const OP_LT: u32 = 10;
/// Operator-stack token for less-than-or-equal comparison.
pub const OP_LE: u32 = 11;
/// Operator-stack token for greater-than comparison.
pub const OP_GT: u32 = 12;
/// Operator-stack token for greater-than-or-equal comparison.
pub const OP_GE: u32 = 13;
/// Operator-stack token for left shift.
pub const OP_SHL: u32 = 14;
/// Operator-stack token for right shift.
pub const OP_SHR: u32 = 15;
/// Operator-stack token for addition.
pub const OP_ADD: u32 = 16;
/// Operator-stack token for subtraction.
pub const OP_SUB: u32 = 17;
/// Operator-stack token for multiplication.
pub const OP_MUL: u32 = 18;
/// Operator-stack token for division.
pub const OP_DIV: u32 = 19;
/// Operator-stack token for remainder.
pub const OP_MOD: u32 = 20;
/// Operator-stack token for unary logical-not.
pub const OP_UN_NOT: u32 = 101;
/// Operator-stack token for unary bitwise-not.
pub const OP_UN_BNOT: u32 = 102;
/// Operator-stack token for unary negation.
pub const OP_UN_NEG: u32 = 103;
/// Operator-stack token for unary plus.
pub const OP_UN_PLUS: u32 = 104;
/// Sentinel written to directive values for malformed `#if` arithmetic.
pub const INVALID_EXPR_VALUE: u32 = u32::MAX;

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::{BufferAccess, BufferDecl, DataType};

    #[test]
    fn source_and_macro_buffers_are_runtime_sized() {
        let buffers = [
            BufferDecl::storage(
                "source",
                BINDING_SOURCE,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(0),
            BufferDecl::storage(
                "macro_names_packed",
                BINDING_MACRO_NAMES_PACKED,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(0),
            BufferDecl::storage(
                "macro_offsets",
                BINDING_MACRO_OFFSETS,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(0),
            BufferDecl::storage(
                "macro_values",
                BINDING_MACRO_VALUES,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(0),
        ];
        for name in [
            "source",
            "macro_names_packed",
            "macro_offsets",
            "macro_values",
        ] {
            let buffer = buffers
                .iter()
                .find(|buffer| buffer.name() == name)
                .expect("Fix: buffer must exist");
            assert_eq!(
                buffer.count, 0,
                "{name} must be runtime-sized so one #if expression program serves all TU and macro-table sizes"
            );
        }
    }
}
