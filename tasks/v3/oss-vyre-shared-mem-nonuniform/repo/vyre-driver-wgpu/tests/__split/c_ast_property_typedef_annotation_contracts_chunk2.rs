// ---------------------------------------------------------------------------
// Proptest
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ArbAtom {
    Tok(u32),
    Ident(String),
}

fn arb_atom() -> impl Strategy<Value = ArbAtom> {
    let fixed = prop::sample::select(vec![
        TOK_TYPEDEF,
        TOK_INT,
        TOK_VOID,
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_RBRACE,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_COMMA,
        TOK_STAR,
        TOK_ASSIGN,
    ]);
    let ident = prop::sample::select(vec!["a", "b", "c", "T", "S", "foo", "bar", "baz"])
        .prop_map(|s| ArbAtom::Ident(s.to_string()));
    prop_oneof![1 => fixed.prop_map(ArbAtom::Tok), 3 => ident]
}

fn pack_arb_atoms(atoms: &[ArbAtom]) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u8>) {
    let mut tok_types = Vec::new();
    let mut tok_lens = Vec::new();
    let mut source = String::new();
    for atom in atoms {
        if !source.is_empty() {
            source.push(' ');
        }
        match atom {
            ArbAtom::Tok(t) => {
                tok_types.push(*t);
                let len = match *t {
                    TOK_EQ | TOK_NE | TOK_LE | TOK_GE | TOK_AND | TOK_OR | TOK_LSHIFT
                    | TOK_RSHIFT | TOK_INC | TOK_DEC | TOK_PLUS_EQ | TOK_MINUS_EQ | TOK_STAR_EQ
                    | TOK_SLASH_EQ | TOK_ARROW => 2,
                    TOK_ELLIPSIS => 3,
                    _ => 1,
                };
                tok_lens.push(len);
                for _ in 0..len {
                    source.push('x');
                }
            }
            ArbAtom::Ident(name) => {
                tok_types.push(TOK_IDENTIFIER);
                tok_lens.push(name.len() as u32);
                source.push_str(name);
            }
        }
    }
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens, source.into_bytes())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn proptest_random_atoms_annotation_invariants(atoms in prop::collection::vec(arb_atom(), 4..32)) {
        let (tok_types, tok_starts, tok_lens, haystack) = pack_arb_atoms(&atoms);
        let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
        let annotated = reference_c11_annotate_typedef_names(&raw, &haystack);
        assert_annotation_invariants(&annotated, &haystack);
    }
}
