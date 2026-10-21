use crate::support::object::{u32_words_from_bytes, CompiledObject, SECTION_VAST, VAST_STRIDE_U32};

pub(crate) fn vyrec_user_kinds(object: &CompiledObject) -> Vec<String> {
    let bytes = object.section(SECTION_VAST);
    let words = u32_words_from_bytes(bytes);
    let stride = VAST_STRIDE_U32;
    let mut kinds = Vec::new();
    for chunk in words.chunks_exact(stride) {
        let kind = chunk[0];
        if kind == 0 {
            continue;
        }
        kinds.push(vast_kind_label(kind).to_string());
    }
    kinds
}

/// Stable string label for every public C VAST kind. Kept in lock-step with
/// `vyre-libs/src/parsing/c/parse/vast_kinds.rs`. Unknown kinds (e.g. raw
/// token kinds before classification, or new constants Kimi may add) fall
/// through to `Other(<hex>)` so the harness never silently drops information.
pub(crate) fn vast_kind_label(kind: u32) -> String {
    use vyre_libs::parsing::c::parse::vast::{
        C_AST_KIND_ALIGNOF_EXPR, C_AST_KIND_ARRAY_DECL, C_AST_KIND_ARRAY_SUBSCRIPT_EXPR,
        C_AST_KIND_ASM_CLOBBERS_LIST, C_AST_KIND_ASM_GOTO_LABELS, C_AST_KIND_ASM_INPUT_OPERAND,
        C_AST_KIND_ASM_OUTPUT_OPERAND, C_AST_KIND_ASM_QUALIFIER, C_AST_KIND_ASM_TEMPLATE,
        C_AST_KIND_ASSIGN_EXPR, C_AST_KIND_ATTRIBUTE_ALIAS, C_AST_KIND_ATTRIBUTE_ALIGNED,
        C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE, C_AST_KIND_ATTRIBUTE_CLEANUP,
        C_AST_KIND_ATTRIBUTE_COLD, C_AST_KIND_ATTRIBUTE_CONST, C_AST_KIND_ATTRIBUTE_CONSTRUCTOR,
        C_AST_KIND_ATTRIBUTE_DEPRECATED, C_AST_KIND_ATTRIBUTE_DESTRUCTOR,
        C_AST_KIND_ATTRIBUTE_FALLTHROUGH, C_AST_KIND_ATTRIBUTE_FORMAT, C_AST_KIND_ATTRIBUTE_HOT,
        C_AST_KIND_ATTRIBUTE_MODE, C_AST_KIND_ATTRIBUTE_NAKED, C_AST_KIND_ATTRIBUTE_NOINLINE,
        C_AST_KIND_ATTRIBUTE_NORETURN, C_AST_KIND_ATTRIBUTE_PACKED, C_AST_KIND_ATTRIBUTE_PURE,
        C_AST_KIND_ATTRIBUTE_SECTION, C_AST_KIND_ATTRIBUTE_UNUSED, C_AST_KIND_ATTRIBUTE_USED,
        C_AST_KIND_ATTRIBUTE_VISIBILITY, C_AST_KIND_ATTRIBUTE_WEAK, C_AST_KIND_BIT_FIELD_DECL,
        C_AST_KIND_BREAK_STMT, C_AST_KIND_BUILTIN_CHOOSE_EXPR,
        C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR, C_AST_KIND_BUILTIN_CONSTANT_P_EXPR,
        C_AST_KIND_BUILTIN_EXPECT_EXPR, C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR,
        C_AST_KIND_BUILTIN_OFFSETOF_EXPR, C_AST_KIND_BUILTIN_OVERFLOW_EXPR,
        C_AST_KIND_BUILTIN_PREFETCH_EXPR, C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR,
        C_AST_KIND_BUILTIN_UNREACHABLE_STMT, C_AST_KIND_CASE_STMT, C_AST_KIND_CAST_EXPR,
        C_AST_KIND_COMPOUND_LITERAL_EXPR, C_AST_KIND_CONDITIONAL_EXPR, C_AST_KIND_CONTINUE_STMT,
        C_AST_KIND_DEFAULT_STMT, C_AST_KIND_DO_STMT, C_AST_KIND_ELSE_STMT,
        C_AST_KIND_ENUMERATOR_DECL, C_AST_KIND_ENUM_DECL, C_AST_KIND_FIELD_DECL,
        C_AST_KIND_FOR_STMT, C_AST_KIND_FUNCTION_DECLARATOR, C_AST_KIND_FUNCTION_DEFINITION,
        C_AST_KIND_GENERIC_SELECTION_EXPR, C_AST_KIND_GNU_ATTRIBUTE,
        C_AST_KIND_GNU_LABEL_ADDRESS_EXPR, C_AST_KIND_GNU_LOCAL_LABEL_DECL,
        C_AST_KIND_GNU_STATEMENT_EXPR, C_AST_KIND_GOTO_STMT, C_AST_KIND_IF_STMT,
        C_AST_KIND_INITIALIZER_LIST, C_AST_KIND_INLINE_ASM, C_AST_KIND_LABEL_STMT,
        C_AST_KIND_MEMBER_ACCESS_EXPR, C_AST_KIND_POINTER_DECL, C_AST_KIND_RANGE_DESIGNATOR_EXPR,
        C_AST_KIND_RETURN_STMT, C_AST_KIND_SIZEOF_EXPR, C_AST_KIND_STATIC_ASSERT_DECL,
        C_AST_KIND_STRUCT_DECL, C_AST_KIND_SWITCH_STMT, C_AST_KIND_TYPEDEF_DECL,
        C_AST_KIND_UNARY_EXPR, C_AST_KIND_UNION_DECL, C_AST_KIND_WHILE_STMT,
    };
    let s = match kind {
        // Statements
        C_AST_KIND_IF_STMT => "IfStmt",
        C_AST_KIND_ELSE_STMT => "ElseStmt",
        C_AST_KIND_SWITCH_STMT => "SwitchStmt",
        C_AST_KIND_CASE_STMT => "CaseStmt",
        C_AST_KIND_DEFAULT_STMT => "DefaultStmt",
        C_AST_KIND_FOR_STMT => "ForStmt",
        C_AST_KIND_WHILE_STMT => "WhileStmt",
        C_AST_KIND_DO_STMT => "DoStmt",
        C_AST_KIND_RETURN_STMT => "ReturnStmt",
        C_AST_KIND_BREAK_STMT => "BreakStmt",
        C_AST_KIND_CONTINUE_STMT => "ContinueStmt",
        C_AST_KIND_GOTO_STMT => "GotoStmt",
        C_AST_KIND_LABEL_STMT => "LabelStmt",
        C_AST_KIND_BUILTIN_UNREACHABLE_STMT => "BuiltinUnreachableStmt",
        // Declarations
        C_AST_KIND_STRUCT_DECL => "StructDecl",
        C_AST_KIND_UNION_DECL => "UnionDecl",
        C_AST_KIND_ENUM_DECL => "EnumDecl",
        C_AST_KIND_TYPEDEF_DECL => "TypedefDecl",
        C_AST_KIND_FUNCTION_DEFINITION => "FunctionDefinition",
        C_AST_KIND_FIELD_DECL => "FieldDecl",
        C_AST_KIND_ENUMERATOR_DECL => "EnumeratorDecl",
        C_AST_KIND_BIT_FIELD_DECL => "BitFieldDecl",
        C_AST_KIND_STATIC_ASSERT_DECL => "StaticAssertDecl",
        C_AST_KIND_GNU_LOCAL_LABEL_DECL => "GnuLocalLabelDecl",
        // Declarators
        C_AST_KIND_POINTER_DECL => "PointerDecl",
        C_AST_KIND_ARRAY_DECL => "ArrayDecl",
        C_AST_KIND_FUNCTION_DECLARATOR => "FunctionDeclarator",
        // Expressions
        C_AST_KIND_ASSIGN_EXPR => "AssignExpr",
        C_AST_KIND_MEMBER_ACCESS_EXPR => "MemberAccessExpr",
        C_AST_KIND_SIZEOF_EXPR => "SizeofExpr",
        C_AST_KIND_ALIGNOF_EXPR => "AlignofExpr",
        C_AST_KIND_CONDITIONAL_EXPR => "ConditionalExpr",
        C_AST_KIND_UNARY_EXPR => "UnaryExpr",
        C_AST_KIND_ARRAY_SUBSCRIPT_EXPR => "ArraySubscriptExpr",
        C_AST_KIND_GENERIC_SELECTION_EXPR => "GenericSelectionExpr",
        C_AST_KIND_RANGE_DESIGNATOR_EXPR => "RangeDesignatorExpr",
        C_AST_KIND_CAST_EXPR => "CastExpr",
        C_AST_KIND_COMPOUND_LITERAL_EXPR => "CompoundLiteralExpr",
        C_AST_KIND_INITIALIZER_LIST => "InitializerList",
        C_AST_KIND_GNU_STATEMENT_EXPR => "GnuStatementExpr",
        C_AST_KIND_GNU_LABEL_ADDRESS_EXPR => "GnuLabelAddressExpr",
        // GNU builtins
        C_AST_KIND_BUILTIN_CONSTANT_P_EXPR => "BuiltinConstantPExpr",
        C_AST_KIND_BUILTIN_CHOOSE_EXPR => "BuiltinChooseExpr",
        C_AST_KIND_BUILTIN_TYPES_COMPATIBLE_P_EXPR => "BuiltinTypesCompatiblePExpr",
        C_AST_KIND_BUILTIN_EXPECT_EXPR => "BuiltinExpectExpr",
        C_AST_KIND_BUILTIN_OFFSETOF_EXPR => "BuiltinOffsetofExpr",
        C_AST_KIND_BUILTIN_OBJECT_SIZE_EXPR => "BuiltinObjectSizeExpr",
        C_AST_KIND_BUILTIN_PREFETCH_EXPR => "BuiltinPrefetchExpr",
        C_AST_KIND_BUILTIN_OVERFLOW_EXPR => "BuiltinOverflowExpr",
        C_AST_KIND_BUILTIN_CLASSIFY_TYPE_EXPR => "BuiltinClassifyTypeExpr",
        // Inline asm
        C_AST_KIND_INLINE_ASM => "InlineAsm",
        C_AST_KIND_ASM_TEMPLATE => "AsmTemplate",
        C_AST_KIND_ASM_OUTPUT_OPERAND => "AsmOutputOperand",
        C_AST_KIND_ASM_INPUT_OPERAND => "AsmInputOperand",
        C_AST_KIND_ASM_CLOBBERS_LIST => "AsmClobbersList",
        C_AST_KIND_ASM_GOTO_LABELS => "AsmGotoLabels",
        C_AST_KIND_ASM_QUALIFIER => "AsmQualifier",
        // GNU attributes
        C_AST_KIND_GNU_ATTRIBUTE => "GnuAttribute",
        C_AST_KIND_ATTRIBUTE_SECTION => "AttributeSection",
        C_AST_KIND_ATTRIBUTE_WEAK => "AttributeWeak",
        C_AST_KIND_ATTRIBUTE_ALIAS => "AttributeAlias",
        C_AST_KIND_ATTRIBUTE_ALIGNED => "AttributeAligned",
        C_AST_KIND_ATTRIBUTE_USED => "AttributeUsed",
        C_AST_KIND_ATTRIBUTE_UNUSED => "AttributeUnused",
        C_AST_KIND_ATTRIBUTE_NAKED => "AttributeNaked",
        C_AST_KIND_ATTRIBUTE_VISIBILITY => "AttributeVisibility",
        C_AST_KIND_ATTRIBUTE_PACKED => "AttributePacked",
        C_AST_KIND_ATTRIBUTE_CLEANUP => "AttributeCleanup",
        C_AST_KIND_ATTRIBUTE_CONSTRUCTOR => "AttributeConstructor",
        C_AST_KIND_ATTRIBUTE_DESTRUCTOR => "AttributeDestructor",
        C_AST_KIND_ATTRIBUTE_MODE => "AttributeMode",
        C_AST_KIND_ATTRIBUTE_NOINLINE => "AttributeNoinline",
        C_AST_KIND_ATTRIBUTE_ALWAYS_INLINE => "AttributeAlwaysInline",
        C_AST_KIND_ATTRIBUTE_COLD => "AttributeCold",
        C_AST_KIND_ATTRIBUTE_HOT => "AttributeHot",
        C_AST_KIND_ATTRIBUTE_PURE => "AttributePure",
        C_AST_KIND_ATTRIBUTE_CONST => "AttributeConst",
        C_AST_KIND_ATTRIBUTE_FORMAT => "AttributeFormat",
        C_AST_KIND_ATTRIBUTE_FALLTHROUGH => "AttributeFallthrough",
        C_AST_KIND_ATTRIBUTE_NORETURN => "AttributeNoreturn",
        C_AST_KIND_ATTRIBUTE_DEPRECATED => "AttributeDeprecated",
        _ => return format!("Other(0x{kind:08X})"),
    };
    s.to_string()
}

/// Hard assertion: every kind in `wanted` must appear at least once in `kinds`.
/// Failure message names the missing kind plus the first ten kinds that *did*
/// appear, to make per-feature regressions diagnosable from CI logs.
#[track_caller]
pub(crate) fn assert_kinds_contain(kinds: &[String], wanted: &[&str]) {
    for w in wanted {
        if !kinds.iter().any(|k| k == w) {
            let preview: Vec<&str> = kinds.iter().take(20).map(String::as_str).collect();
            panic!(
                "ast_oracle: expected kind `{w}` not found in {} kinds. First 20: {:?}",
                kinds.len(),
                preview,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vast_kind_labels_match_known_constants() {
        use vyre_libs::parsing::c::parse::vast::{
            C_AST_KIND_FUNCTION_DEFINITION, C_AST_KIND_GNU_ATTRIBUTE, C_AST_KIND_IF_STMT,
        };
        assert_eq!(vast_kind_label(C_AST_KIND_IF_STMT), "IfStmt");
        assert_eq!(
            vast_kind_label(C_AST_KIND_FUNCTION_DEFINITION),
            "FunctionDefinition"
        );
        assert_eq!(vast_kind_label(C_AST_KIND_GNU_ATTRIBUTE), "GnuAttribute");
        assert_eq!(vast_kind_label(0xDEAD_BEEF), "Other(0xDEADBEEF)");
    }
}
