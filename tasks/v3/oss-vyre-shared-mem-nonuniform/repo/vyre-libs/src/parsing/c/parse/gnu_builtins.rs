use crate::parsing::c::parse::vast_kinds::{
    C_AST_KIND_BUILTIN_CHOOSE_EXPR, C_AST_KIND_BUILTIN_EXPECT_EXPR,
    C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR, C_AST_KIND_BUILTIN_OFFSETOF_EXPR,
    C_AST_KIND_BUILTIN_PREFETCH_EXPR, C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
};
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Compatibility opcode for front-end streams that tag `__builtin_expect`.
pub const GNU_BUILTIN_EXPECT_OPCODE: u32 = 0x4558_5043;
/// Compatibility opcode for front-end streams that tag `__builtin_offsetof`.
pub const GNU_BUILTIN_OFFSETOF_OPCODE: u32 = 0x4F46_5354;
/// Compatibility opcode for front-end streams that tag `__builtin_object_size`.
pub const GNU_BUILTIN_OBJECT_SIZE_OPCODE: u32 = 0x4F42_4A53;
/// Compatibility opcode for front-end streams that tag `__builtin_prefetch`.
pub const GNU_BUILTIN_PREFETCH_OPCODE: u32 = 0x5052_4546;
/// Compatibility opcode for front-end streams that tag `__builtin_unreachable`.
pub const GNU_BUILTIN_UNREACHABLE_OPCODE: u32 = 0x554E_5243;
/// Reserved opcode prefix for unsupported GNU builtin front-end tags.
pub const GNU_BUILTIN_RESERVED_PREFIX: u32 = 0x474E_5500;

/// Fail-loud GNU builtin classifier error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnuBuiltinError {
    /// Identifier byte length at the failure site.
    pub len: usize,
    /// Actionable diagnostic.
    pub message: &'static str,
}

impl core::fmt::Display for GnuBuiltinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} for {} bytes", self.message, self.len)
    }
}

impl std::error::Error for GnuBuiltinError {}

/// Classify GNU builtin identifier bytes into parser-local VAST kinds.
///
/// Ordinary identifiers return `Ok(None)`. Unknown `__builtin_*` names return
/// an error because silently treating compiler intrinsics as ordinary calls
/// loses semantics needed by the C frontend.
///
/// # Errors
///
/// Returns an actionable error for unsupported GNU builtin names.
pub fn try_classify_gnu_builtin_name(name: &[u8]) -> Result<Option<u32>, GnuBuiltinError> {
    if let Some(kind) = super::gnu_builtin_catalog::classify_gnu_builtin_name(name) {
        return Ok(Some(kind));
    }
    if name.starts_with(b"__builtin_") {
        return Err(GnuBuiltinError {
            len: name.len(),
            message: "Fix: add explicit GNU builtin semantics before accepting this intrinsic",
        });
    }
    Ok(None)
}

/// Multiplicative seed for the GPU `__has_builtin` perfect-hash table.
pub const GPU_BUILTIN_HASH_TABLE_SEED: u32 = 0x27b4;
/// Entry count for the GPU `__has_builtin` perfect-hash table.
pub const GPU_BUILTIN_HASH_TABLE_SIZE: usize = 5003;

/// Return the canonical GPU perfect-hash table for `__has_builtin` lookup.
#[must_use]
pub fn gpu_builtin_hash_table_words() -> Vec<u32> {
    let mut table = vec![0u32; GPU_BUILTIN_HASH_TABLE_SIZE];
    for entry in super::gnu_builtin_catalog::GNU_BUILTIN_NAME_KINDS {
        let slot = gpu_builtin_hash_slot(entry.hash);
        assert_eq!(
            table[slot], 0,
            "Fix: GPU builtin hash table seed must keep catalog hashes collision-free"
        );
        table[slot] = entry.hash;
    }
    table
}

fn gpu_builtin_hash_slot(hash: u32) -> usize {
    (hash.wrapping_mul(GPU_BUILTIN_HASH_TABLE_SEED) % GPU_BUILTIN_HASH_TABLE_SIZE as u32) as usize
}

/// GNU builtin front-end normalization pass.
///
/// The pass preserves already-classified VAST builtin kinds and maps legacy
/// front-end builtin opcodes onto the same stable kind IDs. Reserved GNU
/// builtin opcodes trap instead of passing through as ordinary calls.
#[must_use]
pub fn c11_gnu_builtins_pass(
    ast_opcodes: &str,
    out_ast_opcodes: &str,
    num_ast_nodes: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("opcode", Expr::load(ast_opcodes, t.clone())),
        Node::let_bind("normalized", Expr::var("opcode")),
        Node::if_then(
            Expr::eq(Expr::var("opcode"), Expr::u32(GNU_BUILTIN_EXPECT_OPCODE)),
            vec![Node::assign(
                "normalized",
                Expr::u32(C_AST_KIND_BUILTIN_EXPECT_EXPR),
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("opcode"), Expr::u32(GNU_BUILTIN_OFFSETOF_OPCODE)),
            vec![Node::assign(
                "normalized",
                Expr::u32(C_AST_KIND_BUILTIN_OFFSETOF_EXPR),
            )],
        ),
        Node::if_then(
            Expr::eq(
                Expr::var("opcode"),
                Expr::u32(GNU_BUILTIN_OBJECT_SIZE_OPCODE),
            ),
            vec![Node::assign(
                "normalized",
                Expr::u32(C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR),
            )],
        ),
        Node::if_then(
            Expr::eq(Expr::var("opcode"), Expr::u32(GNU_BUILTIN_PREFETCH_OPCODE)),
            vec![Node::assign(
                "normalized",
                Expr::u32(C_AST_KIND_BUILTIN_PREFETCH_EXPR),
            )],
        ),
        Node::if_then(
            Expr::eq(
                Expr::var("opcode"),
                Expr::u32(GNU_BUILTIN_UNREACHABLE_OPCODE),
            ),
            vec![Node::assign(
                "normalized",
                Expr::u32(C_AST_KIND_BUILTIN_UNREACHABLE_STMT),
            )],
        ),
        Node::if_then(
            Expr::eq(
                Expr::bitand(Expr::var("opcode"), Expr::u32(0xFFFF_FF00)),
                Expr::u32(GNU_BUILTIN_RESERVED_PREFIX),
            ),
            vec![Node::trap(
                Expr::var("opcode"),
                "unsupported-gnu-builtin-opcode",
            )],
        ),
        Node::store(out_ast_opcodes, t.clone(), Expr::var("normalized")),
    ];

    let ast_count = match &num_ast_nodes {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(ast_opcodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(ast_count),
            BufferDecl::storage(out_ast_opcodes, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(ast_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_gnu_builtins_pass",
            vec![Node::if_then(Expr::lt(t.clone(), num_ast_nodes), loop_body)],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_gnu_builtins_pass")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c11_gnu_builtins_pass",
        build: || c11_gnu_builtins_pass("ast", "out_ast", Expr::u32(4)),
        test_inputs: Some(|| {
            let ast = [
                0x11u32,
                GNU_BUILTIN_EXPECT_OPCODE,
                GNU_BUILTIN_OBJECT_SIZE_OPCODE,
                C_AST_KIND_BUILTIN_CHOOSE_EXPR,
            ];
            let ast_bytes = vyre_primitives::wire::pack_u32_slice(&ast);
            vec![vec![ast_bytes, vec![0u8; 4 * 4]]]
        }),
        expected_output: Some(|| {
            let out = [
                0x11u32,
                C_AST_KIND_BUILTIN_EXPECT_EXPR,
                C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
                C_AST_KIND_BUILTIN_CHOOSE_EXPR,
            ];
            let out_bytes = vyre_primitives::wire::pack_u32_slice(&out);
            vec![vec![out_bytes]]
        }),
        category: Some("parsing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::c::parse::vast_kinds::{
        C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR, C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
        C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR, C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
        C_AST_KIND_BUILTIN_VA_INTRIN_EXPR,
    };

    #[test]
    fn classifier_accepts_common_real_header_gnu_builtins() {
        let cases: &[(&[u8], u32)] = &[
            (b"__builtin_memchr", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
            (b"__builtin_strnlen", C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR),
            (
                b"__builtin___memcpy_chk",
                C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
            ),
            (
                b"__builtin___strcpy_chk",
                C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
            ),
            (b"__builtin_ms_va_start", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
            (b"__builtin_next_arg", C_AST_KIND_BUILTIN_VA_INTRIN_EXPR),
            (
                b"__builtin_frob_return_addr",
                C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
            ),
            (
                b"__builtin_unwind_init",
                C_AST_KIND_BUILTIN_FRAME_INTRIN_EXPR,
            ),
            (
                b"__builtin_speculation_safe_value",
                C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
            ),
            (
                b"__builtin_is_aligned",
                C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
            ),
            (b"__builtin_align_up", C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR),
            (
                b"__builtin_align_down",
                C_AST_KIND_BUILTIN_ASSUME_INTRIN_EXPR,
            ),
            (
                b"__builtin_preserve_access_index",
                C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
            ),
            (
                b"__builtin_btf_type_id",
                C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR,
            ),
        ];

        for (name, expected) in cases {
            assert_eq!(
                try_classify_gnu_builtin_name(name).unwrap(),
                Some(*expected),
                "{}",
                String::from_utf8_lossy(name)
            );
        }
    }

    #[test]
    fn classifier_still_rejects_unknown_builtin_names() {
        let error = try_classify_gnu_builtin_name(b"__builtin_vyre_unknown")
            .expect_err("unknown compiler builtins must not become ordinary calls");
        assert_eq!(error.len, b"__builtin_vyre_unknown".len());
    }

    #[test]
    fn gpu_hash_table_is_exact_for_catalog_hashes() {
        let table = gpu_builtin_hash_table_words();
        assert!(
            table.len() == GPU_BUILTIN_HASH_TABLE_SIZE,
            "Fix: GPU __has_builtin table size must match its shader slot mask"
        );
        for entry in super::super::gnu_builtin_catalog::GNU_BUILTIN_NAME_KINDS {
            assert_eq!(table[gpu_builtin_hash_slot(entry.hash)], entry.hash);
        }
    }
}
