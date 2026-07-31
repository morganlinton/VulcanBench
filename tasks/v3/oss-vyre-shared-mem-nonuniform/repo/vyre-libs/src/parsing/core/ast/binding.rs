use crate::parsing::c::lex::tokens::*;
use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// SIMT Binding Strength Pass (Innovation 1: Divergence-Free Shunting Yard)
///
/// Replaces the serial expression stack with a purely mathematical map-reduce.
/// Calculates the structural depth of every token in parallel, then applies
/// intrinsic operator precedence to generate an absolute "Binding Strength"
/// for every token.
///
/// `out_depths` must be pre-populated by `parsing::common::parallel_prefix_scan`.
#[must_use]
pub fn ast_binding_strength(
    tok_types: &str,
    out_depths: &str,
    out_strengths: &str,
    num_tokens: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("tok", Expr::load(tok_types, t.clone())),
        Node::let_bind("depth", Expr::load(out_depths, t.clone())),
        // Base strength determined by parenthesis depth (e.g. nested deeply = high strength)
        Node::let_bind(
            "base_strength",
            Expr::mul(Expr::var("depth"), Expr::u32(100)),
        ),
        Node::let_bind("precedence", Expr::u32(0)),
        // Map operator precedences (C11 rules)
        Node::if_then(
            Expr::or(
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_STAR)),
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_SLASH)),
            ),
            vec![Node::assign("precedence", Expr::u32(40))],
        ),
        Node::if_then(
            Expr::or(
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_PLUS)),
                Expr::eq(Expr::var("tok"), Expr::u32(TOK_MINUS)),
            ),
            vec![Node::assign("precedence", Expr::u32(30))],
        ),
        Node::if_then(
            Expr::eq(Expr::var("tok"), Expr::u32(0x3D)), // Assignment has lowest precedence
            vec![Node::assign("precedence", Expr::u32(10))],
        ),
        // Assign final mathematical binding strength
        Node::store(
            out_strengths,
            t.clone(),
            Expr::add(Expr::var("base_strength"), Expr::var("precedence")),
        ),
    ];

    let tok_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count),
            BufferDecl::storage(out_depths, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count),
            BufferDecl::storage(out_strengths, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::ast_binding_strength",
            vec![Node::if_then(Expr::lt(t.clone(), num_tokens), loop_body)],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::ast_binding_strength")
    .with_non_composable_with_self(true)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::ast_binding_strength",
        // Use a small 4-token fixture so the witness is trivially
        // checkable: tok_types = [STAR, PLUS, '=', 0], depths = [1, 1, 0, 0].
        // Expected strengths = depth*100 + precedence: [1*100+40=140,
        // 1*100+30=130, 0*100+10=10, 0*100+0=0].
        build: || ast_binding_strength("tok_types", "out_depths", "out_strengths", Expr::u32(4)),
        test_inputs: Some(|| {
            let tokens: [u32; 4] = [TOK_STAR, TOK_PLUS, 0x3D, 0];
            let depths: [u32; 4] = [1, 1, 0, 0];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&tokens), to_bytes(&depths), vec![0u8; 4 * 4]]]
        }),
        expected_output: Some(|| {
            let strengths: [u32; 4] = [140, 130, 10, 0];
            let bytes = vyre_primitives::wire::pack_u32_slice(&strengths);
            vec![vec![bytes]]
        }),
        category: Some("parsing"),
    }
}
