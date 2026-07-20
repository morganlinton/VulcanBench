use super::*;

#[test]
fn table_conditional_mask_all_directive_shapes() {
    struct Case {
        kinds: &'static [u32],
        values: &'static [u32],
        expected: &'static [u32],
    }
    let cases: &[Case] = &[
        // if-true body endif
        Case {
            kinds: &[TOK_PP_IF, 0, TOK_PP_ENDIF],
            values: &[1, 0, 0],
            expected: &[1, 1, 1],
        },
        // if-false body endif
        Case {
            kinds: &[TOK_PP_IF, 0, TOK_PP_ENDIF],
            values: &[0, 0, 0],
            expected: &[1, 0, 1],
        },
        // if-false elif-true body endif
        Case {
            kinds: &[TOK_PP_IF, 0, TOK_PP_ELIF, 0, TOK_PP_ENDIF],
            values: &[0, 0, 1, 0, 0],
            expected: &[1, 0, 1, 1, 1],
        },
        // if-false else body endif
        Case {
            kinds: &[TOK_PP_IF, 0, TOK_PP_ELSE, 0, TOK_PP_ENDIF],
            values: &[0, 0, 0, 0, 0],
            expected: &[1, 0, 1, 1, 1],
        },
        // if-true elif-true body endif (elif dead because already taken)
        Case {
            kinds: &[TOK_PP_IF, 0, TOK_PP_ELIF, 0, TOK_PP_ENDIF],
            values: &[1, 0, 1, 0, 0],
            expected: &[1, 1, 1, 0, 1],
        },
        // ifdef defined
        Case {
            kinds: &[TOK_PP_IFDEF, 0, TOK_PP_ENDIF],
            values: &[1, 0, 0],
            expected: &[1, 1, 1],
        },
        // ifndef undefined
        Case {
            kinds: &[TOK_PP_IFNDEF, 0, TOK_PP_ENDIF],
            values: &[1, 0, 0],
            expected: &[1, 1, 1],
        },
    ];

    for (idx, case) in cases.iter().enumerate() {
        let tok_types = vec![TOK_PREPROC; case.kinds.len()];
        let outputs = run_conditional_mask_with_directives(&tok_types, case.kinds, case.values)
            .unwrap_or_else(|e| panic!("case {} failed: {}", idx, e));
        let mask = decode_u32_words(&outputs[0].to_bytes());
        assert_eq!(
            &mask[..case.expected.len()],
            case.expected,
            "case {} mask mismatch",
            idx
        );
    }
}

#[test]
fn table_macro_expansion_shapes() {
    struct DynCase {
        input: &'static [u32],
        replacement: &'static [u32],
        expected: &'static [u32],
    }
    let cases: &[DynCase] = &[
        // single-token replacement
        DynCase {
            input: &[TOK_IDENTIFIER],
            replacement: &[TOK_INTEGER],
            expected: &[TOK_INTEGER],
        },
        // multi-token replacement
        DynCase {
            input: &[TOK_IDENTIFIER],
            replacement: &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER],
            expected: &[TOK_INTEGER, TOK_PLUS, TOK_INTEGER],
        },
        // zero-token replacement
        DynCase {
            input: &[TOK_IDENTIFIER, TOK_SEMICOLON],
            replacement: &[],
            expected: &[TOK_SEMICOLON],
        },
        // interleaved passthrough
        DynCase {
            input: &[TOK_INT, TOK_IDENTIFIER, TOK_SEMICOLON],
            replacement: &[TOK_VOID],
            expected: &[TOK_INT, TOK_VOID, TOK_SEMICOLON],
        },
    ];

    for (idx, case) in cases.iter().enumerate() {
        let mut fixture = DynamicFixture::empty();
        fixture.insert(TOK_IDENTIFIER, 512, case.replacement);
        let outputs = run_dynamic(case.input, &fixture, 16)
            .unwrap_or_else(|e| panic!("case {} failed: {}", idx, e));
        let out = decode_u32_words(&outputs[0].to_bytes());
        let count = decode_u32_words(&outputs[1].to_bytes());
        assert_eq!(
            count[0] as usize,
            case.expected.len(),
            "case {} count mismatch",
            idx
        );
        assert_eq!(
            &out[..case.expected.len()],
            case.expected,
            "case {} output mismatch",
            idx
        );
    }
}

#[test]
fn table_named_macro_function_like_shapes() {
    struct NamedCase {
        source: &'static [u8],
        types: &'static [u32],
        starts: &'static [u32],
        lens: &'static [u32],
        param_count: u32,
        replacement: &'static [(u32, u32)],
        expected: &'static [u32],
    }
    let cases: &[NamedCase] = &[
        // zero-arg function-like with empty body
        NamedCase {
            source: b"F()",
            types: &[TOK_IDENTIFIER, TOK_LPAREN, TOK_RPAREN],
            starts: &[0, 1, 2],
            lens: &[1, 1, 1],
            param_count: 0,
            replacement: &[],
            expected: &[],
        },
        // one-arg passthrough
        NamedCase {
            source: b"F(x)",
            types: &[TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
            starts: &[0, 1, 2, 3],
            lens: &[1, 1, 1, 1],
            param_count: 1,
            replacement: &[(0, 0)],
            expected: &[TOK_IDENTIFIER],
        },
        // two-arg with literal separator
        NamedCase {
            source: b"ADD(a,b)",
            types: &[
                TOK_IDENTIFIER,
                TOK_LPAREN,
                TOK_IDENTIFIER,
                TOK_COMMA,
                TOK_IDENTIFIER,
                TOK_RPAREN,
            ],
            starts: &[0, 3, 4, 5, 6, 7],
            lens: &[3, 1, 1, 1, 1, 1],
            param_count: 2,
            replacement: &[(0, 0), (TOK_PLUS, C_MACRO_REPLACEMENT_LITERAL), (0, 1)],
            expected: &[TOK_IDENTIFIER, TOK_PLUS, TOK_IDENTIFIER],
        },
        // object-like ignores following lparen
        NamedCase {
            source: b"OBJ(x)",
            types: &[TOK_IDENTIFIER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
            starts: &[0, 3, 4, 5],
            lens: &[3, 1, 1, 1],
            param_count: 0,
            replacement: &[(TOK_INTEGER, C_MACRO_REPLACEMENT_LITERAL)],
            expected: &[TOK_INTEGER, TOK_LPAREN, TOK_IDENTIFIER, TOK_RPAREN],
        },
    ];

    for (idx, case) in cases.iter().enumerate() {
        let stream = TokenStream {
            source: case.source,
            types: case.types.to_vec(),
            starts: case.starts.to_vec(),
            lens: case.lens.to_vec(),
        };
        let mut fixture = NamedFixture::empty();
        let (name, kind): (&[u8], u32) = if case.source.starts_with(b"ADD") {
            (b"ADD", C_MACRO_KIND_FUNCTION_LIKE)
        } else if case.source.starts_with(b"OBJ") {
            (b"OBJ", C_MACRO_KIND_OBJECT_LIKE)
        } else {
            (b"F", C_MACRO_KIND_FUNCTION_LIKE)
        };
        fixture.insert(name, 512, kind, case.param_count, case.replacement);
        let outputs = run_named(&stream, &fixture, 16)
            .unwrap_or_else(|e| panic!("case {} failed: {}", idx, e));
        let out = decode_u32_words(&outputs[0].to_bytes());
        let count = decode_u32_words(&outputs[1].to_bytes());
        assert_eq!(
            count[0] as usize,
            case.expected.len(),
            "case {} count mismatch",
            idx
        );
        assert_eq!(
            &out[..case.expected.len()],
            case.expected,
            "case {} output mismatch",
            idx
        );
    }
}

