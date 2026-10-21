use super::*;

#[test]
fn conditional_stack_open_increments_depth_and_sets_active_bit() {
    // #if 1 → depth 1, active_bits bit 0 set.
    let tok_types = &[TOK_PREPROC, TOK_IDENTIFIER, TOK_PREPROC];
    let directive_kinds = &[TOK_PP_IF, 0, TOK_PP_ENDIF];
    let directive_values = &[1, 0, 0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("open/close must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1, 1, 1]);
}

#[test]
fn conditional_stack_false_open_masks_body() {
    // #if 0 → body token must be masked out.
    let tok_types = &[TOK_PREPROC, TOK_IDENTIFIER, TOK_PREPROC];
    let directive_kinds = &[TOK_PP_IF, 0, TOK_PP_ENDIF];
    let directive_values = &[0, 0, 0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("false branch must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(mask, vec![1, 0, 1]);
}

#[test]
fn conditional_stack_elif_selects_next_branch() {
    let tok_types = &[
        TOK_PREPROC,
        TOK_IDENTIFIER,
        TOK_PREPROC,
        TOK_IDENTIFIER,
        TOK_PREPROC,
    ];
    let directive_kinds = &[TOK_PP_IF, 0, TOK_PP_ELIF, 0, TOK_PP_ENDIF];
    let directive_values = &[0, 0, 1, 0, 0];
    let outputs =
        run_conditional_mask_with_directives(tok_types, directive_kinds, directive_values)
            .expect("elif branch must succeed");
    let mask = decode_u32_words(&outputs[0].to_bytes());
    // directives stay live; first body dead; elif body live; second body dead
    assert_eq!(mask, vec![1, 0, 1, 1, 1]);
}

