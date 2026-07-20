//! Generic delimiter-depth scan for token streams.
//!
//! This is parsing substrate, not a language-specific op: every parser with
//! paired delimiters can reuse the same inclusive prefix-depth scan.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable Tier 2.5 op id.
pub const OP_ID: &str = "vyre-primitives::parsing::core_delimiter_match";

/// Build a delimiter-depth Program for arbitrary token streams.
///
/// `tok_types[t] == open_tok_id` increments depth and `close_tok_id`
/// decrements depth. The inclusive prefix depth is written to
/// `tok_depths[t]`.
#[must_use]
pub fn core_delimiter_match(
    tok_types: &str,
    tok_depths: &str,
    tok_count: u32,
    open_tok_id: u32,
    close_tok_id: u32,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let transform_logic = vec![
        Node::let_bind("running_depth", Expr::u32(0)),
        Node::loop_for(
            "k",
            Expr::u32(0),
            Expr::add(t.clone(), Expr::u32(1)),
            vec![
                Node::let_bind("kth_tok", Expr::load(tok_types, Expr::var("k"))),
                Node::if_then(
                    Expr::eq(Expr::var("kth_tok"), Expr::u32(open_tok_id)),
                    vec![Node::assign(
                        "running_depth",
                        Expr::add(Expr::var("running_depth"), Expr::u32(1)),
                    )],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("kth_tok"), Expr::u32(close_tok_id)),
                    vec![Node::assign(
                        "running_depth",
                        Expr::sub(Expr::var("running_depth"), Expr::u32(1)),
                    )],
                ),
            ],
        ),
        Node::store(tok_depths, t.clone(), Expr::var("running_depth")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(tok_count),
            BufferDecl::storage(tok_depths, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(tok_count),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t, Expr::u32(tok_count)),
                transform_logic,
            )]),
        }],
    )
    .with_entry_op_id(OP_ID)
    .with_non_composable_with_self(true)
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || core_delimiter_match("tok_types", "tok_depths", 8, 12, 13),
        Some(|| {
            let tokens: [u32; 8] = [12, 12, 0, 0, 0, 13, 13, 0];
            let bytes = crate::wire::pack_u32_slice(&tokens);
            vec![vec![bytes, vec![0u8; 4 * 8]]]
        }),
        Some(|| {
            let depths: [u32; 8] = [1, 2, 2, 2, 2, 1, 0, 0];
            let bytes = crate::wire::pack_u32_slice(&depths);
            vec![vec![bytes]]
        }),
    )
}
