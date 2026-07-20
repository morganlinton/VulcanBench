//! Atomic LRU update: safely update access timestamps/priority in a shared buffer.
//!
//! Category-B composition over `AtomicOp::Max`.

use crate::region::wrap_anonymous;
use vyre::ir::{AtomicOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::memory_model::MemoryOrdering;

/// Build a Program that atomically updates an LRU slot.
#[must_use]
pub fn atomic_lru_update_u32(buffer: &str, index: Expr, timestamp: Expr) -> Program {
    let body = vec![
        Node::let_bind("idx", index),
        Node::let_bind("ts", timestamp),
        Node::let_bind(
            "_prev",
            Expr::Atomic {
                op: AtomicOp::Max,
                buffer: buffer.into(),
                index: Box::new(Expr::var("idx")),
                expected: None,
                value: Box::new(Expr::var("ts")),
                ordering: MemoryOrdering::SeqCst,
            },
        ),
    ];

    Program::wrapped(
        vec![BufferDecl::storage(buffer, 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::math::atomic::lru_update_u32",
            body,
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::atomic::lru_update_u32",
        build: || atomic_lru_update_u32("buffer", Expr::u32(0), Expr::u32(12345)),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[0u32]), // buffer (single slot, initial value 0)
            ]]
        }),
        expected_output: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            // Single lane writes timestamp 12345 into slot 0.
            vec![vec![to_bytes(&[12345u32])]]
        }),
        category: Some("math"),
    }
}

::inventory::submit! {
    ::vyre_driver::registry::dialect::OpDefRegistration::new(|| ::vyre_driver::registry::OpDef {
        id: "vyre-libs::math::atomic::lru_update_u32",
        dialect: "vyre-libs.math.atomic",
        category: ::vyre_driver::registry::Category::Intrinsic,
        signature: ::vyre_driver::registry::Signature {
            inputs: &[
                ::vyre_driver::registry::TypedParam { name: "buffer", ty: "buffer<u32>" },
                ::vyre_driver::registry::TypedParam { name: "index", ty: "u32" },
                ::vyre_driver::registry::TypedParam { name: "timestamp", ty: "u32" },
            ],
            outputs: &[],
            attrs: &[],
            bytes_extraction: false,
        },
        lowerings: ::vyre_foundation::dialect_lookup::LoweringTable::empty(),
        laws: &[],
        compose: Some(|| atomic_lru_update_u32("buffer", Expr::u32(0), Expr::u32(12345))),
    })
}
