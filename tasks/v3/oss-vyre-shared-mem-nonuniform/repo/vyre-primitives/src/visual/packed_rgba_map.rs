//! Packed-RGBA per-pixel map skeleton.
//!
//! Higher-level visual ops specialize the pixel expression, but they all
//! share the same execution shape: one invocation reads or derives one
//! packed `u32` RGBA pixel and writes one packed `u32` pixel.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable Tier 2.5 op id.
pub const OP_ID: &str = "vyre-primitives::visual::packed_rgba_map";

/// Emit a generic identity packed-RGBA map node.
#[must_use]
pub fn packed_rgba_map_node(input: &str, output: &str, count: u32) -> Node {
    Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(count)),
                vec![
                    Node::let_bind("pixel", Expr::load(input, Expr::var("idx"))),
                    Node::store(output, Expr::var("idx"), Expr::var("pixel")),
                ],
            ),
        ]),
    }
}

/// Standalone identity packed-RGBA map Program.
#[must_use]
pub fn packed_rgba_map(input: &str, output: &str, count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
        ],
        [256, 1, 1],
        vec![packed_rgba_map_node(input, output, count)],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || packed_rgba_map("in", "out", 4),
        Some(|| {
            let pixels = [0xFF00_0000u32, 0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000];
            let bytes = crate::wire::pack_u32_slice(&pixels);
            vec![vec![bytes, vec![0; 16]]]
        }),
        Some(|| {
            let pixels = [0xFF00_0000u32, 0xFF00_00FF, 0xFF00_FF00, 0xFFFF_0000];
            let bytes = crate::wire::pack_u32_slice(&pixels);
            vec![vec![bytes]]
        }),
    )
}
