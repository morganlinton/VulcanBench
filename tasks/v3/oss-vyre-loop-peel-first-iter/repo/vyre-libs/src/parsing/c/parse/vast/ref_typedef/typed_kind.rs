use super::*;

pub(super) fn reference_typed_kind(vast_nodes: &[u32], node_idx: usize) -> u32 {
    let node_count = vast_nodes.len() / VAST_NODE_STRIDE_U32 as usize;
    let raw_kind = kind_at(vast_nodes, node_idx);
    let cur_parent = parent_at(vast_nodes, node_idx);
    let symbol = symbol_hash_at(vast_nodes, node_idx);
    let current_is_typeof_operator = is_typeof_operator_raw(raw_kind, symbol);
    let first_child_kind = child_kind(vast_nodes, node_idx);
    let first_child_flags = child_flags(vast_nodes, node_idx);
    let first_child_symbol = child_symbol_hash(vast_nodes, node_idx);
    let raw_next_kind = if node_idx + 1 < node_count {
        kind_at(vast_nodes, node_idx + 1)
    } else {
        0
    };
    let raw_next_flags = if node_idx + 1 < node_count {
        flags_at(vast_nodes, node_idx + 1)
    } else {
        0
    };
    let raw_after_next_kind = if node_idx + 2 < node_count {
        kind_at(vast_nodes, node_idx + 2)
    } else {
        0
    };
    let raw_after_after_kind = if node_idx + 3 < node_count {
        kind_at(vast_nodes, node_idx + 3)
    } else {
        0
    };
    let next_idx = next_sibling_at(vast_nodes, node_idx);
    let next_valid = usize::try_from(next_idx)
        .ok()
        .is_some_and(|idx| idx < node_count);

    let next_kind = if next_valid {
        kind_at(vast_nodes, next_idx as usize)
    } else {
        0
    };
    let after_param_idx = if next_valid {
        next_sibling_at(vast_nodes, next_idx as usize)
    } else {
        SENTINEL
    };
    let decl_context = decl_context_before(vast_nodes, node_idx, cur_parent);
    let prev = previous_sibling_context(vast_nodes, node_idx, cur_parent);
    let has_prior_typedef = prior_typedef_seen(vast_nodes, node_idx);
    let has_typedef_annotations = has_any_typedef_annotations(vast_nodes);
    let has_prior_parenthesized_identifier_statement =
        prior_parenthesized_identifier_statement_seen(vast_nodes, node_idx);
    let ambiguous_parenthesized_identifier_multiply = raw_kind == TOK_LPAREN
        && next_kind == TOK_STAR
        && has_prior_parenthesized_identifier_statement;
    let fallback_has_prior_typedef = !has_typedef_annotations
        && has_prior_typedef
        && !prior_ordinary_decl_seen(vast_nodes, node_idx)
        && !prior_raw_ordinary_decl_seen(vast_nodes, node_idx)
        && !ambiguous_parenthesized_identifier_multiply;
    let raw_lparen = raw_kind == TOK_LPAREN;
    let suffix_start_idx = if raw_lparen {
        next_idx
    } else {
        after_param_idx
    };
    let suffix_boundary_kind = suffix_function_boundary_kind(vast_nodes, suffix_start_idx, 16);
    let function_boundary = suffix_boundary_kind != SENTINEL;
    let identifier_type_name_paren = raw_lparen
        && first_child_kind == TOK_IDENTIFIER
        && matches!(
            next_kind,
            TOK_LBRACE
                | TOK_LPAREN
                | TOK_IDENTIFIER
                | TOK_INTEGER
                | TOK_FLOAT
                | TOK_STRING
                | TOK_CHAR
                | TOK_STAR
                | TOK_AMP
                | TOK_PLUS
                | TOK_MINUS
                | TOK_BANG
                | TOK_TILDE
                | TOK_INC
                | TOK_DEC
        );
    let flat_identifier_type_name_paren = raw_lparen
        && raw_next_kind == TOK_IDENTIFIER
        && is_reference_typedef_name(raw_next_flags, fallback_has_prior_typedef)
        && raw_after_next_kind == TOK_RPAREN
        && matches!(
            raw_after_after_kind,
            TOK_LBRACE
                | TOK_LPAREN
                | TOK_IDENTIFIER
                | TOK_INTEGER
                | TOK_FLOAT
                | TOK_STRING
                | TOK_CHAR
                | TOK_STAR
                | TOK_AMP
                | TOK_PLUS
                | TOK_MINUS
                | TOK_BANG
                | TOK_TILDE
                | TOK_INC
                | TOK_DEC
        );
    let type_name_paren = raw_lparen
        && !matches!(prev.kind, TOK_SIZEOF | TOK_ALIGNOF | TOK_ATOMIC)
        && !is_typeof_operator_raw(prev.kind, prev.symbol_hash)
        && (is_type_name_start_raw(first_child_kind)
            || is_typeof_operator_raw(first_child_kind, first_child_symbol)
            || (is_reference_typedef_name(first_child_flags, fallback_has_prior_typedef)
                && identifier_type_name_paren)
            || flat_identifier_type_name_paren);
    let parent = parent_context(vast_nodes, cur_parent);
    let inherited_decl_prefix = parenthesized_declarator_context(vast_nodes, cur_parent);
    let effective_has_decl_prefix = decl_context.has_prefix || inherited_decl_prefix;

    let identifier_then_paren = raw_kind == TOK_IDENTIFIER && next_valid && next_kind == TOK_LPAREN;
    let is_function_declarator = raw_lparen
        && (function_boundary
            || ((type_name_paren || is_type_name_start_raw(first_child_kind))
                && matches!(prev.kind, TOK_LPAREN | TOK_RPAREN)))
        && matches!(prev.kind, TOK_IDENTIFIER | TOK_LPAREN | TOK_RPAREN)
        && (effective_has_decl_prefix || prev.kind == TOK_RPAREN);
    let is_return_function_suffix = raw_lparen
        && type_name_paren
        && function_boundary
        && effective_has_decl_prefix
        && prev.kind == TOK_LPAREN;
    let typedef_pointer_decl = raw_kind == TOK_STAR
        && is_reference_typedef_name(prev.flags, fallback_has_prior_typedef)
        && prev.kind == TOK_IDENTIFIER
        && next_kind == TOK_IDENTIFIER
        && matches!(
            prev.prev_kind,
            SENTINEL | TOK_LBRACE | TOK_LPAREN | TOK_SEMICOLON | TOK_COMMA
        );
    let is_pointer_decl =
        raw_kind == TOK_STAR && (effective_has_decl_prefix || typedef_pointer_decl);
    let parenthesized_declarator_suffix = prev.kind == TOK_LPAREN
        && matches!(
            child_kind(vast_nodes, prev.idx as usize),
            TOK_STAR | TOK_IDENTIFIER | TOK_LPAREN
        );
    let is_array_decl = raw_kind == TOK_LBRACKET
        && (prev.kind == TOK_IDENTIFIER || parenthesized_declarator_suffix)
        && effective_has_decl_prefix;
    let is_array_designator_expr = raw_kind == TOK_LBRACKET && next_kind == TOK_ASSIGN;
    let is_array_declaration_initializer_assign = raw_kind == TOK_ASSIGN
        && prev.kind == TOK_LBRACKET
        && effective_has_decl_prefix
        && !enclosing_brace_is_initializer_list(vast_nodes, cur_parent)
        && next_kind == TOK_STRING;
    let is_compound_literal = raw_lparen && type_name_paren && next_kind == TOK_LBRACE;
    let is_cast_expr =
        raw_lparen && type_name_paren && !is_function_declarator && !is_compound_literal;
    let brace_after_compound_literal_type = prev.kind == TOK_LPAREN
        && (matches!(
            child_kind(vast_nodes, prev.idx as usize),
            TOK_VOID
                | TOK_BOOL
                | TOK_CHAR_KW
                | TOK_SHORT
                | TOK_INT
                | TOK_LONG
                | TOK_FLOAT_KW
                | TOK_DOUBLE
                | TOK_SIGNED
                | TOK_UNSIGNED
                | TOK_STRUCT
                | TOK_UNION
                | TOK_ENUM
                | TOK_CONST
                | TOK_RESTRICT
                | TOK_VOLATILE
                | TOK_ATOMIC
        ) || (child_kind(vast_nodes, prev.idx as usize) == TOK_IDENTIFIER
            && is_reference_typedef_name(
                child_flags(vast_nodes, prev.idx as usize),
                fallback_has_prior_typedef,
            ))
            || is_typeof_operator_raw(
                child_kind(vast_nodes, prev.idx as usize),
                child_symbol_hash(vast_nodes, prev.idx as usize),
            ))
        && matches!(
            prev.prev_kind,
            TOK_ASSIGN | TOK_RETURN | TOK_COMMA | TOK_LPAREN
        );
    let is_initializer_list = raw_kind == TOK_LBRACE
        && (prev.kind == TOK_ASSIGN
            || brace_after_compound_literal_type
            || (matches!(prev.kind, SENTINEL | TOK_LBRACE | TOK_COMMA)
                && enclosing_brace_is_initializer_list(vast_nodes, cur_parent)));
    let is_field_decl = raw_kind == TOK_IDENTIFIER
        && parent.is_record_body
        && decl_context.has_prefix
        && matches!(
            next_kind,
            TOK_SEMICOLON | TOK_COMMA | TOK_ASSIGN | TOK_LBRACKET | TOK_COLON
        );
    let is_anonymous_bit_field_decl = raw_kind == TOK_COLON
        && parent.is_record_body
        && decl_context.has_prefix
        && prev.kind != TOK_IDENTIFIER;
    let is_enumerator_decl = raw_kind == TOK_IDENTIFIER
        && parent.is_enum_body
        && matches!(prev.kind, SENTINEL | TOK_COMMA)
        && matches!(next_kind, TOK_COMMA | TOK_ASSIGN | TOK_RBRACE);
    let is_label_stmt = raw_kind == TOK_IDENTIFIER
        && next_kind == TOK_COLON
        && !parent.is_record_body
        && !parent.is_enum_body;
    let is_gnu_statement_expr = raw_kind == TOK_LPAREN && first_child_kind == TOK_LBRACE;
    let is_gnu_label_address_expr =
        raw_kind == TOK_AND && reference_c_unary_context(prev.kind) && next_kind == TOK_IDENTIFIER;
    let is_asm_goto_qualifier =
        raw_kind == TOK_GOTO && asm_prefix_before(vast_nodes, node_idx, cur_parent);
    let is_asm_volatile_qualifier =
        raw_kind == TOK_VOLATILE && asm_prefix_before(vast_nodes, node_idx, cur_parent);
    let asm_kind = reference_c_asm_context_kind(vast_nodes, node_idx, raw_kind, cur_parent);
    let attribute_kind = reference_c_attribute_kind(vast_nodes, node_idx, raw_kind, cur_parent)
        .or_else(|| reference_c_direct_attribute_kind(vast_nodes, raw_kind, cur_parent, symbol));
    let cur_parent_parent_kind = usize::try_from(cur_parent)
        .ok()
        .filter(|parent_idx| *parent_idx < node_count)
        .map(|parent_idx| parent_at(vast_nodes, parent_idx))
        .and_then(|parent_parent| usize::try_from(parent_parent).ok())
        .filter(|parent_parent_idx| *parent_parent_idx < node_count)
        .map(|parent_parent_idx| kind_at(vast_nodes, parent_parent_idx))
        .unwrap_or(0);
    let inside_gnu_statement_expr_body = usize::try_from(cur_parent)
        .ok()
        .filter(|parent_idx| *parent_idx < node_count)
        .is_some_and(|parent_idx| {
            kind_at(vast_nodes, parent_idx) == TOK_LBRACE && cur_parent_parent_kind == TOK_LPAREN
        });
    let builtin_kind = reference_c_builtin_expression_kind(raw_kind)
        .or_else(|| reference_c_builtin_identifier_expression_kind(raw_kind, symbol, next_kind));
    let c99_for_init_statement_assign =
        c99_for_init_statement_assign(vast_nodes, raw_kind, cur_parent, effective_has_decl_prefix);
    let is_declaration_initializer_assign = raw_kind == TOK_ASSIGN
        && (effective_has_decl_prefix
            || declaration_initializer_prefix_before(vast_nodes, node_idx, cur_parent)
            || c99_for_init_statement_assign)
        && !inside_gnu_statement_expr_body
        && !is_array_declaration_initializer_assign;
    let expression_kind = if is_declaration_initializer_assign {
        None
    } else {
        reference_c_expression_operator_kind(raw_kind, prev.kind, prev.prev_kind)
    };
    let star_after_parenthesized_identifier_expr = raw_kind == TOK_STAR
        && prev.kind == TOK_LPAREN
        && child_kind(vast_nodes, prev.idx as usize) == TOK_IDENTIFIER
        && if has_typedef_annotations {
            !is_reference_typedef_name(child_flags(vast_nodes, prev.idx as usize), false)
        } else {
            !has_prior_typedef
                || prior_ordinary_decl_seen(vast_nodes, node_idx)
                || prior_raw_ordinary_decl_seen(vast_nodes, node_idx)
                || has_prior_parenthesized_identifier_statement
        };

    if identifier_then_paren
        && function_boundary
        && effective_has_decl_prefix
        && prev.kind != TOK_LPAREN
        && suffix_boundary_kind == TOK_LBRACE
    {
        C_AST_KIND_FUNCTION_DEFINITION
    } else if matches!(raw_kind, TOK_STRUCT | TOK_UNION | TOK_ENUM) {
        match raw_kind {
            TOK_STRUCT => C_AST_KIND_STRUCT_DECL,
            TOK_UNION => C_AST_KIND_UNION_DECL,
            TOK_ENUM => C_AST_KIND_ENUM_DECL,
            _ => 0,
        }
    } else if raw_kind == TOK_TYPEDEF {
        C_AST_KIND_TYPEDEF_DECL
    } else if raw_kind == TOK_STATIC_ASSERT {
        C_AST_KIND_STATIC_ASSERT_DECL
    } else if raw_kind == TOK_GNU_LABEL {
        C_AST_KIND_GNU_LOCAL_LABEL_DECL
    } else if let Some(kind) = attribute_kind {
        kind
    } else if (is_field_decl && next_kind == TOK_COLON) || is_anonymous_bit_field_decl {
        C_AST_KIND_BIT_FIELD_DECL
    } else if let Some(kind) = builtin_kind {
        kind
    } else if current_is_typeof_operator {
        C_AST_KIND_SIZEOF_EXPR
    } else if identifier_then_paren
        && function_boundary
        && effective_has_decl_prefix
        && prev.kind != TOK_LPAREN
    {
        node_kind::FUNCTION_DECL
    } else if is_function_declarator || is_return_function_suffix {
        C_AST_KIND_FUNCTION_DECLARATOR
    } else if is_pointer_decl {
        C_AST_KIND_POINTER_DECL
    } else if is_array_decl {
        C_AST_KIND_ARRAY_DECL
    } else if is_array_designator_expr {
        C_AST_KIND_ARRAY_SUBSCRIPT_EXPR
    } else if is_cast_expr {
        C_AST_KIND_CAST_EXPR
    } else if is_compound_literal {
        C_AST_KIND_COMPOUND_LITERAL_EXPR
    } else if is_initializer_list {
        C_AST_KIND_INITIALIZER_LIST
    } else if is_field_decl {
        C_AST_KIND_FIELD_DECL
    } else if is_enumerator_decl {
        C_AST_KIND_ENUMERATOR_DECL
    } else if is_label_stmt {
        C_AST_KIND_LABEL_STMT
    } else if is_gnu_statement_expr {
        C_AST_KIND_GNU_STATEMENT_EXPR
    } else if identifier_then_paren {
        node_kind::CALL
    } else if raw_kind == TOK_LBRACE {
        node_kind::BASIC_BLOCK
    } else if is_asm_goto_qualifier || is_asm_volatile_qualifier {
        C_AST_KIND_ASM_QUALIFIER
    } else if let Some(kind) = reference_c_statement_kind(raw_kind) {
        kind
    } else if star_after_parenthesized_identifier_expr {
        node_kind::BINARY
    } else if is_gnu_label_address_expr {
        C_AST_KIND_GNU_LABEL_ADDRESS_EXPR
    } else if let Some(kind) = asm_kind {
        kind
    } else if let Some(kind) = expression_kind {
        kind
    } else if raw_kind == TOK_GNU_ASM {
        C_AST_KIND_INLINE_ASM
    } else if raw_kind == TOK_GNU_ATTRIBUTE {
        C_AST_KIND_GNU_ATTRIBUTE
    } else if matches!(raw_kind, TOK_INTEGER | TOK_FLOAT | TOK_STRING | TOK_CHAR) {
        node_kind::LITERAL
    } else if raw_kind == TOK_IDENTIFIER && !is_gnu_auto_type_hash_raw(symbol) {
        node_kind::VARIABLE
    } else {
        0
    }
}
