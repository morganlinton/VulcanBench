//! C VAST construction, classification, expression-shape, and typedef passes.
//!
//! Public builders live in the shortest module that owns their ABI. Reference
//! helpers are explicit oracle surfaces for parity and corpus checks; production
//! parsing should use the dispatchable builders exported from this module.

mod build;
mod build_declaration_kind_inner;
mod classify;
mod decl_context_common;
mod expr_shape;
mod helpers;
#[cfg(any(test, feature = "cpu-parity"))]
mod ref_classify;
#[cfg(any(test, feature = "cpu-parity"))]
mod ref_decode_err;
#[cfg(any(test, feature = "cpu-parity"))]
mod ref_expr_shape;
#[cfg(any(test, feature = "cpu-parity"))]
mod ref_typedef;
mod typedef_ann;

pub use build::{c11_build_vast_nodes, c11_build_vast_nodes_uses_global_last_child};
pub use classify::{
    c11_classify_annotated_vast_node_kinds_precomputed_context, c11_classify_vast_node_kinds,
    c11_classify_vast_node_kinds_precomputed_context,
};
pub use expr_shape::{
    c11_build_expression_shape_nodes, c11_build_expression_shape_nodes_no_conditional,
};
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use ref_classify::{
    reference_c11_classify_vast_node_kinds, try_reference_c11_classify_vast_node_kinds,
};
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use ref_decode_err::{reference_c11_build_vast_nodes, CReferenceDecodeError};
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use ref_expr_shape::{
    reference_c11_build_expression_shape_nodes, try_reference_c11_build_expression_shape_nodes,
};
#[allow(deprecated)]
#[cfg(any(test, feature = "cpu-parity"))]
pub use ref_typedef::{
    reference_c11_annotate_typedef_names, try_reference_c11_annotate_typedef_names,
};
pub use typedef_ann::{
    c11_annotate_global_typedef_names_fast, c11_annotate_typedef_names,
    c11_annotate_typedef_names_packed_haystack, c11_annotate_typedef_names_precomputed_context,
    c11_annotate_typedef_names_precomputed_context_packed_haystack,
    c11_annotate_typedef_names_precomputed_scope,
    c11_annotate_typedef_names_precomputed_scope_packed_haystack, c11_link_vast_typedef_symbols,
    c11_precompute_vast_decl_contexts, c11_precompute_vast_decl_prefix_starts,
    c11_precompute_vast_scopes, c11_precompute_vast_scopes_uses_global_stack,
    c11_prehash_vast_identifiers, c11_prehash_vast_identifiers_packed_haystack,
};

// Sibling re-exports keep each active pass focused on its own program builder
// while sharing one explicit helper surface. If a helper becomes specific to a
// single pass, move it into that pass instead of growing this shared prelude.

#[cfg(any(test, feature = "cpu-parity"))]
use crate::harness::OpEntry;
use vyre_primitives::predicate::node_kind;

pub use super::vast_kinds::{
    C_AST_KIND_ALIGNOF_EXPR, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
    C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND,
    C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_KIND_ASM_QUALIFIER, C_AST_KIND_ASM_TEMPLATE,
    C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIAS, C_AST_KIND_ATTRIBUTE_ALIGNED,
    C_AST_KIND_ATTRIBUTE_ALLOC_ALIGN, C_AST_KIND_ATTRIBUTE_ALLOC_SIZE,
    C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE, C_AST_KIND_ATTRIBUTE_ASSUME_ALIGNED,
    C_AST_KIND_ATTRIBUTE_CLEANUP, C_AST_KIND_ATTRIBUTE_COLD, C_AST_KIND_ATTRIBUTE_CONST,
    C_AST_KIND_ATTRIBUTE_CONSTRUCTOR, C_AST_KIND_ATTRIBUTE_DEPRECATED,
    C_AST_KIND_ATTRIBUTE_DESTRUCTOR, C_AST_KIND_ATTRIBUTE_DLLEXPORT,
    C_AST_KIND_ATTRIBUTE_DLLIMPORT, C_AST_KIND_ATTRIBUTE_FALLTHROUGH, C_AST_KIND_ATTRIBUTE_FLATTEN,
    C_AST_KIND_ATTRIBUTE_FORMAT, C_AST_KIND_ATTRIBUTE_FORMAT_ARG, C_AST_KIND_ATTRIBUTE_GNU_INLINE,
    C_AST_KIND_ATTRIBUTE_HOT, C_AST_KIND_ATTRIBUTE_IFUNC, C_AST_KIND_ATTRIBUTE_INTERRUPT,
    C_AST_KIND_ATTRIBUTE_LEAF, C_AST_KIND_ATTRIBUTE_MALLOC, C_AST_KIND_ATTRIBUTE_MODE,
    C_AST_KIND_ATTRIBUTE_MS_ABI, C_AST_KIND_ATTRIBUTE_NAKED, C_AST_KIND_ATTRIBUTE_NOINLINE,
    C_AST_KIND_ATTRIBUTE_NONNULL, C_AST_KIND_ATTRIBUTE_NORETURN, C_AST_KIND_ATTRIBUTE_NOTHROW,
    C_AST_KIND_ATTRIBUTE_NO_INSTRUMENT_FUNCTION, C_AST_KIND_ATTRIBUTE_NO_SANITIZE,
    C_AST_KIND_ATTRIBUTE_PACKED, C_AST_KIND_ATTRIBUTE_PURE, C_AST_KIND_ATTRIBUTE_RETURNS_NONNULL,
    C_AST_KIND_ATTRIBUTE_RETURNS_TWICE, C_AST_KIND_ATTRIBUTE_SECTION,
    C_AST_KIND_ATTRIBUTE_SELECTANY, C_AST_KIND_ATTRIBUTE_SENTINEL, C_AST_KIND_ATTRIBUTE_SYSV_ABI,
    C_AST_KIND_ATTRIBUTE_TARGET, C_AST_KIND_ATTRIBUTE_TLS_MODEL, C_AST_KIND_ATTRIBUTE_UNUSED,
    C_AST_KIND_ATTRIBUTE_USED, C_AST_KIND_ATTRIBUTE_VECTOR_SIZE, C_AST_KIND_ATTRIBUTE_VISIBILITY,
    C_AST_KIND_ATTRIBUTE_WARN_UNUSED_RESULT, C_AST_KIND_ATTRIBUTE_WEAK,
    C_AST_KIND_ATTRIBUTE_WEAKREF, C_AST_KIND_BIT_FIELD_DECL, C_AST_KIND_BREAK_STMT,
    C_AST_KIND_BUILTIN_BPF_CORE_INTRIN_EXPR, C_AST_KIND_BUILTIN_CHOOSE_EXPR,
    C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
    C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_BUILTIN_LIBC_INTRIN_EXPR,
    C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR, C_AST_KIND_BUILTIN_OFFSETOF_EXPR,
    C_AST_KIND_BUILTIN_OVERFLOW_EXPR, C_AST_KIND_BUILTIN_PREFETCH_EXPR,
    C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR, C_AST_KIND_BUILTIN_UNREACHABLE_STMT,
    C_AST_KIND_CASE_STMT, C_AST_KIND_CAST_EXPR, C_AST_KIND_COMPOUND_LITERAL_EXPR,
    C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_CONTINUE_STMT, C_AST_KIND_DEFAULT_STMT,
    C_AST_KIND_DO_STMT, C_AST_KIND_ELSE_STMT, C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_ENUM_DECL,
    C_AST_KIND_FIELD_DECL, C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR,
    C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GENERIC_SELECTION_EXPR, C_AST_KIND_GNU_ATTRIBUTE,
    C_AST_KIND_GNU_LABEL_ADDRESS_EXPR, C_AST_KIND_GNU_LOCAL_LABEL_DECL,
    C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
    C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM, C_AST_KIND_LABEL_STMT,
    C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RANGE_DESIGNATOR_EXPR,
    C_AST_KIND_RETURN_STMT, C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_STATIC_ASSERT_DECL,
    C_AST_KIND_STRUCT_DECL, C_AST_KIND_SWITCH_STMT, C_AST_KIND_TYPEDEF_DECL, C_AST_KIND_UNARY_EXPR,
    C_AST_KIND_UNION_DECL, C_AST_KIND_WHILE_STMT, C_EXPR_ASSOC_LEFT, C_EXPR_ASSOC_NONE,
    C_EXPR_ASSOC_RIGHT, C_EXPR_SHAPE_BINARY, C_EXPR_SHAPE_CONDITIONAL, C_EXPR_SHAPE_NONE,
    C_EXPR_SHAPE_STRIDE_U32,
};

const BUILD_VAST_OP_ID: &str = "vyre-libs::parsing::c11_build_vast_nodes_v2";
const PREHASH_VAST_IDENTIFIERS_OP_ID: &str = "vyre-libs::parsing::c11_prehash_vast_identifiers";
const PRECOMPUTE_VAST_SCOPES_OP_ID: &str = "vyre-libs::parsing::c11_precompute_vast_scopes";
const LINK_VAST_TYPEDEF_SYMBOLS_OP_ID: &str = "vyre-libs::parsing::c11_link_vast_typedef_symbols";
const PRECOMPUTE_VAST_DECL_CONTEXTS_OP_ID: &str =
    "vyre-libs::parsing::c11_precompute_vast_decl_contexts";
const PRECOMPUTE_VAST_DECL_PREFIX_STARTS_OP_ID: &str =
    "vyre-libs::parsing::c11_precompute_vast_decl_prefix_starts";
const CLASSIFY_VAST_OP_ID: &str = "vyre-libs::parsing::c11_classify_vast_node_kinds";
const ANNOTATE_TYPEDEF_OP_ID: &str = "vyre-libs::parsing::c11_annotate_typedef_names";
const EXPR_SHAPE_OP_ID: &str = "vyre-libs::parsing::c11_build_expression_shape_nodes";
const VAST_NODE_STRIDE_U32: u32 = 10;
const VAST_DECL_CONTEXT_STRIDE_U32: u32 = 4;
const VAST_DECL_CONTEXT_PREFIX_START_FIELD: u32 = 0;
const VAST_DECL_CONTEXT_PREV_BUCKET_LINK_FIELD: u32 = 1;
const VAST_DECL_CONTEXT_PREV_DECL_LINK_FIELD: u32 = 2;
const VAST_DECL_CONTEXT_PREV_DECL_CHAIN_LEN_FIELD: u32 = 3;
const SENTINEL: u32 = u32::MAX;
const VAST_SRC_FILE_FIELD: u32 = 4;
const VAST_TYPEDEF_FLAGS_FIELD: u32 = 7;
const VAST_TYPEDEF_SCOPE_FIELD: u32 = 8;
const VAST_TYPEDEF_SYMBOL_FIELD: u32 = 9;
const VAST_PREVIOUS_SIBLING_FIELD: u32 = VAST_SRC_FILE_FIELD;
const C_TYPEDEF_FLAG_VISIBLE_TYPEDEF_NAME: u32 = 1;
const C_TYPEDEF_FLAG_TYPEDEF_DECLARATOR: u32 = 1 << 1;
const C_TYPEDEF_FLAG_ORDINARY_DECLARATOR: u32 = 1 << 2;

#[inline]
pub(crate) fn c_vast_word_at(vast_nodes: &[u32], node_idx: usize, field_idx: usize) -> u32 {
    node_idx
        .checked_mul(VAST_NODE_STRIDE_U32 as usize)
        .and_then(|base| base.checked_add(field_idx))
        .and_then(|word_idx| vast_nodes.get(word_idx))
        .copied()
        .unwrap_or(0)
}

const C_GNU_TYPEOF_HASHES: &[u32] = &[
    0x9a90_a8a0, // typeof
    0xff65_c714, // __typeof__
    0xee15_bd69, // typeof_unqual
    0x812b_41f1, // __typeof_unqual__
];
const C_GNU_AUTO_TYPE_HASH: u32 = 0x572b_7b0d;

const C_ATTRIBUTE_KIND_HASHES: &[(u32, u32)] = &[
    (0xfcdd_0ccc, C_AST_KIND_ATTRIBUTE_SECTION),
    (0x2a13_825c, C_AST_KIND_ATTRIBUTE_SECTION),
    (0xedbc_2ec9, C_AST_KIND_ATTRIBUTE_WEAK),
    (0xa67d_9bad, C_AST_KIND_ATTRIBUTE_WEAK),
    (0x7d26_8157, C_AST_KIND_ATTRIBUTE_ALIAS),
    (0xa79d_c33b, C_AST_KIND_ATTRIBUTE_ALIAS),
    (0xc731_74df, C_AST_KIND_ATTRIBUTE_ALIGNED),
    (0x45b0_1e27, C_AST_KIND_ATTRIBUTE_ALIGNED),
    (0x6a78_6eb0, C_AST_KIND_ATTRIBUTE_USED),
    (0xbc04_7928, C_AST_KIND_ATTRIBUTE_USED),
    (0x85cf_281b, C_AST_KIND_ATTRIBUTE_UNUSED),
    (0xc6de_fd0f, C_AST_KIND_ATTRIBUTE_UNUSED),
    (0x06ca_5a98, C_AST_KIND_ATTRIBUTE_NAKED),
    (0x7d09_0c10, C_AST_KIND_ATTRIBUTE_NAKED),
    (0x7f37_f5e5, C_AST_KIND_ATTRIBUTE_VISIBILITY),
    (0x643d_c155, C_AST_KIND_ATTRIBUTE_VISIBILITY),
    (0x7d7f_64e1, C_AST_KIND_ATTRIBUTE_PACKED),
    (0x2c44_2d6d, C_AST_KIND_ATTRIBUTE_PACKED),
    (0xd95d_f1b3, C_AST_KIND_ATTRIBUTE_CLEANUP),
    (0xac5f_fe13, C_AST_KIND_ATTRIBUTE_CLEANUP),
    (0xf25d_9f4f, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR),
    (0x963c_e7ef, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR),
    (0xb856_15de, C_AST_KIND_ATTRIBUTE_DESTRUCTOR),
    (0xee92_8ba6, C_AST_KIND_ATTRIBUTE_DESTRUCTOR),
    (0xec6e_e012, C_AST_KIND_ATTRIBUTE_MODE),
    (0x1cd7_9962, C_AST_KIND_ATTRIBUTE_MODE),
    (0xb0a7_e467, C_AST_KIND_ATTRIBUTE_NOINLINE),
    (0x268f_f2d3, C_AST_KIND_ATTRIBUTE_NOINLINE),
    (0xe368_4d30, C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE),
    (0x9190_71f4, C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE),
    (0xea44_dd0f, C_AST_KIND_ATTRIBUTE_COLD),
    (0x057f_7b43, C_AST_KIND_ATTRIBUTE_COLD),
    (0xfec3_a7d4, C_AST_KIND_ATTRIBUTE_HOT),
    (0x9b27_4c90, C_AST_KIND_ATTRIBUTE_HOT),
    (0x966d_d8e3, C_AST_KIND_ATTRIBUTE_PURE),
    (0x4edb_a0f3, C_AST_KIND_ATTRIBUTE_PURE),
    (0x664f_d1d4, C_AST_KIND_ATTRIBUTE_CONST),
    (0xc53a_deb4, C_AST_KIND_ATTRIBUTE_CONST),
    (0xb99d_8552, C_AST_KIND_ATTRIBUTE_FORMAT),
    (0x5299_0142, C_AST_KIND_ATTRIBUTE_FORMAT),
    (0x8034_7b09, C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
    (0xc373_7bd1, C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
    (0xb478_da94, C_AST_KIND_ATTRIBUTE_NORETURN),
    (0x700e_0da4, C_AST_KIND_ATTRIBUTE_DEPRECATED),
    // FNV-1a32 hashes for additional GCC/clang/MSVC attribute names.
    // Each name appears in both `name` and `__name__` form because real
    // C headers spell attributes both ways (libc uses the underscored
    // form to avoid clashing with macros named `aligned`, `malloc`, etc).
    (0x2c91_51f9, C_AST_KIND_ATTRIBUTE_NONNULL),
    (0x87d0_1b61, C_AST_KIND_ATTRIBUTE_NONNULL),
    (0x32be_2a85, C_AST_KIND_ATTRIBUTE_RETURNS_NONNULL),
    (0xcde4_3ad9, C_AST_KIND_ATTRIBUTE_RETURNS_NONNULL),
    (0x558c_274d, C_AST_KIND_ATTRIBUTE_MALLOC),
    (0x8752_e709, C_AST_KIND_ATTRIBUTE_MALLOC),
    (0xb75e_ffb8, C_AST_KIND_ATTRIBUTE_WARN_UNUSED_RESULT),
    (0x3ca3_f710, C_AST_KIND_ATTRIBUTE_WARN_UNUSED_RESULT),
    (0x83a0_c19a, C_AST_KIND_ATTRIBUTE_NOTHROW),
    (0x74eb_42e6, C_AST_KIND_ATTRIBUTE_NOTHROW),
    (0xc5f0_cec4, C_AST_KIND_ATTRIBUTE_ASSUME_ALIGNED),
    (0x673b_a19c, C_AST_KIND_ATTRIBUTE_ASSUME_ALIGNED),
    (0x756d_ac4a, C_AST_KIND_ATTRIBUTE_ALLOC_SIZE),
    (0xc731_ed82, C_AST_KIND_ATTRIBUTE_ALLOC_SIZE),
    (0xc78c_a4f0, C_AST_KIND_ATTRIBUTE_ALLOC_ALIGN),
    (0x102a_dab4, C_AST_KIND_ATTRIBUTE_ALLOC_ALIGN),
    (0x8fe9_c41e, C_AST_KIND_ATTRIBUTE_WEAKREF),
    (0x7d46_7226, C_AST_KIND_ATTRIBUTE_WEAKREF),
    (0x167c_332d, C_AST_KIND_ATTRIBUTE_SENTINEL),
    (0xb028_89a5, C_AST_KIND_ATTRIBUTE_SENTINEL),
    (0x2648_cd55, C_AST_KIND_ATTRIBUTE_LEAF),
    (0xce02_9465, C_AST_KIND_ATTRIBUTE_LEAF),
    (0x21c3_98af, C_AST_KIND_ATTRIBUTE_RETURNS_TWICE),
    (0x34c0_17b3, C_AST_KIND_ATTRIBUTE_RETURNS_TWICE),
    (0xa3a5_0ba8, C_AST_KIND_ATTRIBUTE_NO_SANITIZE),
    (0xa6c4_9b28, C_AST_KIND_ATTRIBUTE_NO_SANITIZE),
    (0xc5f6_3fc9, C_AST_KIND_ATTRIBUTE_FLATTEN),
    (0x967d_b3c1, C_AST_KIND_ATTRIBUTE_FLATTEN),
    (0x3260_8848, C_AST_KIND_ATTRIBUTE_TARGET),
    (0x7756_48ac, C_AST_KIND_ATTRIBUTE_TARGET),
    (0x42b3_5373, C_AST_KIND_ATTRIBUTE_TARGET),
    (0x8dd1_87b7, C_AST_KIND_ATTRIBUTE_TARGET),
    (0xb074_1550, C_AST_KIND_ATTRIBUTE_INTERRUPT),
    (0x9dc6_1004, C_AST_KIND_ATTRIBUTE_INTERRUPT),
    (0xbd7b_ba09, C_AST_KIND_ATTRIBUTE_INTERRUPT),
    (0x7ef1_dbb1, C_AST_KIND_ATTRIBUTE_INTERRUPT),
    (0x8afb_247e, C_AST_KIND_ATTRIBUTE_VECTOR_SIZE),
    (0x9957_3a9a, C_AST_KIND_ATTRIBUTE_VECTOR_SIZE),
    (0x63fd_eb32, C_AST_KIND_ATTRIBUTE_IFUNC),
    (0x3ca6_584e, C_AST_KIND_ATTRIBUTE_IFUNC),
    (0xc745_e84c, C_AST_KIND_ATTRIBUTE_TLS_MODEL),
    (0x083c_7c20, C_AST_KIND_ATTRIBUTE_TLS_MODEL),
    (0xb5ad_4ca5, C_AST_KIND_ATTRIBUTE_GNU_INLINE),
    (0x400d_b34d, C_AST_KIND_ATTRIBUTE_GNU_INLINE),
    (0x2a0c_9a2a, C_AST_KIND_ATTRIBUTE_DLLIMPORT),
    (0xbddd_336a, C_AST_KIND_ATTRIBUTE_DLLIMPORT),
    (0xb9bf_5b49, C_AST_KIND_ATTRIBUTE_DLLEXPORT),
    (0x9dc5_dd21, C_AST_KIND_ATTRIBUTE_DLLEXPORT),
    (0x9f20_8c85, C_AST_KIND_ATTRIBUTE_SELECTANY),
    (0xb266_f531, C_AST_KIND_ATTRIBUTE_SELECTANY),
    (0x506c_b880, C_AST_KIND_ATTRIBUTE_MS_ABI),
    (0x7d92_da38, C_AST_KIND_ATTRIBUTE_MS_ABI),
    (0x376d_0281, C_AST_KIND_ATTRIBUTE_SYSV_ABI),
    (0x2168_9f21, C_AST_KIND_ATTRIBUTE_SYSV_ABI),
    (0x003d_0c87, C_AST_KIND_ATTRIBUTE_NO_INSTRUMENT_FUNCTION),
    (0x010a_5aaf, C_AST_KIND_ATTRIBUTE_NO_INSTRUMENT_FUNCTION),
    (0xab77_8c35, C_AST_KIND_ATTRIBUTE_FORMAT_ARG),
    (0x98f8_ab09, C_AST_KIND_ATTRIBUTE_FORMAT_ARG),
];

#[cfg(test)]
mod attribute_hash_tests {
    use super::*;
    use crate::parsing::c::lex::keyword::fnv1a32;

    /// Every attribute name in `expected_pairs` must be present in
    /// `C_ATTRIBUTE_KIND_HASHES` mapped to the expected kind, in BOTH
    /// `name` and `__name__` spellings. Real C headers (libc, kernel,
    /// OpenSSL, sqlite) use both forms  -  missing either spelling silently
    /// drops attribute classification on millions of lines of source.
    #[test]
    fn attribute_table_recognises_both_spellings_for_every_supported_name() {
        // Format: (canonical name, expected VAST kind).
        let expected_pairs: &[(&str, u32)] = &[
            // Original (pre-2026-05-09) attribute coverage.
            ("section", C_AST_KIND_ATTRIBUTE_SECTION),
            ("weak", C_AST_KIND_ATTRIBUTE_WEAK),
            ("alias", C_AST_KIND_ATTRIBUTE_ALIAS),
            ("aligned", C_AST_KIND_ATTRIBUTE_ALIGNED),
            ("used", C_AST_KIND_ATTRIBUTE_USED),
            ("unused", C_AST_KIND_ATTRIBUTE_UNUSED),
            ("naked", C_AST_KIND_ATTRIBUTE_NAKED),
            ("visibility", C_AST_KIND_ATTRIBUTE_VISIBILITY),
            ("packed", C_AST_KIND_ATTRIBUTE_PACKED),
            ("cleanup", C_AST_KIND_ATTRIBUTE_CLEANUP),
            ("constructor", C_AST_KIND_ATTRIBUTE_CONSTRUCTOR),
            ("destructor", C_AST_KIND_ATTRIBUTE_DESTRUCTOR),
            ("mode", C_AST_KIND_ATTRIBUTE_MODE),
            ("noinline", C_AST_KIND_ATTRIBUTE_NOINLINE),
            ("always_inline", C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE),
            ("cold", C_AST_KIND_ATTRIBUTE_COLD),
            ("hot", C_AST_KIND_ATTRIBUTE_HOT),
            ("pure", C_AST_KIND_ATTRIBUTE_PURE),
            ("format", C_AST_KIND_ATTRIBUTE_FORMAT),
            ("fallthrough", C_AST_KIND_ATTRIBUTE_FALLTHROUGH),
            // Newly added (2026-05-09): clang-parity sweep.
            ("nonnull", C_AST_KIND_ATTRIBUTE_NONNULL),
            ("returns_nonnull", C_AST_KIND_ATTRIBUTE_RETURNS_NONNULL),
            ("malloc", C_AST_KIND_ATTRIBUTE_MALLOC),
            (
                "warn_unused_result",
                C_AST_KIND_ATTRIBUTE_WARN_UNUSED_RESULT,
            ),
            ("nothrow", C_AST_KIND_ATTRIBUTE_NOTHROW),
            ("assume_aligned", C_AST_KIND_ATTRIBUTE_ASSUME_ALIGNED),
            ("alloc_size", C_AST_KIND_ATTRIBUTE_ALLOC_SIZE),
            ("alloc_align", C_AST_KIND_ATTRIBUTE_ALLOC_ALIGN),
            ("weakref", C_AST_KIND_ATTRIBUTE_WEAKREF),
            ("sentinel", C_AST_KIND_ATTRIBUTE_SENTINEL),
            ("leaf", C_AST_KIND_ATTRIBUTE_LEAF),
            ("returns_twice", C_AST_KIND_ATTRIBUTE_RETURNS_TWICE),
            ("no_sanitize", C_AST_KIND_ATTRIBUTE_NO_SANITIZE),
            ("flatten", C_AST_KIND_ATTRIBUTE_FLATTEN),
            ("target", C_AST_KIND_ATTRIBUTE_TARGET),
            ("target_clones", C_AST_KIND_ATTRIBUTE_TARGET),
            ("interrupt", C_AST_KIND_ATTRIBUTE_INTERRUPT),
            ("signal", C_AST_KIND_ATTRIBUTE_INTERRUPT),
            ("vector_size", C_AST_KIND_ATTRIBUTE_VECTOR_SIZE),
            ("ifunc", C_AST_KIND_ATTRIBUTE_IFUNC),
            ("tls_model", C_AST_KIND_ATTRIBUTE_TLS_MODEL),
            ("gnu_inline", C_AST_KIND_ATTRIBUTE_GNU_INLINE),
            ("dllimport", C_AST_KIND_ATTRIBUTE_DLLIMPORT),
            ("dllexport", C_AST_KIND_ATTRIBUTE_DLLEXPORT),
            ("selectany", C_AST_KIND_ATTRIBUTE_SELECTANY),
            ("ms_abi", C_AST_KIND_ATTRIBUTE_MS_ABI),
            ("sysv_abi", C_AST_KIND_ATTRIBUTE_SYSV_ABI),
            (
                "no_instrument_function",
                C_AST_KIND_ATTRIBUTE_NO_INSTRUMENT_FUNCTION,
            ),
            ("format_arg", C_AST_KIND_ATTRIBUTE_FORMAT_ARG),
        ];
        for (name, expected_kind) in expected_pairs {
            let bare_hash = fnv1a32(name.as_bytes());
            let underscored = format!("__{name}__");
            let underscored_hash = fnv1a32(underscored.as_bytes());
            let bare = C_ATTRIBUTE_KIND_HASHES
                .iter()
                .find_map(|(h, k)| (*h == bare_hash).then_some(*k));
            let under = C_ATTRIBUTE_KIND_HASHES
                .iter()
                .find_map(|(h, k)| (*h == underscored_hash).then_some(*k));
            assert_eq!(
                bare,
                Some(*expected_kind),
                "attribute `{name}` (hash {bare_hash:#010x}) must classify as {expected_kind:#010x}"
            );
            assert_eq!(
                under,
                Some(*expected_kind),
                "attribute `__{name}__` (hash {underscored_hash:#010x}) must classify as {expected_kind:#010x}"
            );
        }
    }

    /// No two distinct attribute names in the table may share a hash. A
    /// collision here would mean reference_c_attribute_kind silently
    /// classifies one attribute as the other (the find_map returns the
    /// first match). Catching this at unit-test time prevents shipping a
    /// hash duplicate.
    #[test]
    fn attribute_hash_table_has_no_intra_kind_hash_collisions() {
        let mut by_hash: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for (hash, kind) in C_ATTRIBUTE_KIND_HASHES {
            if let Some(prev) = by_hash.insert(*hash, *kind) {
                assert_eq!(
                    prev, *kind,
                    "hash {hash:#010x} maps to two distinct attribute kinds: \
                     {prev:#010x} and {kind:#010x}  -  rename one or change the hash"
                );
            }
        }
    }
}
