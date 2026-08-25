use std::sync::Arc;
use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op-id under which this VFS resolver registers itself in the
/// inventory.
pub const VFS_RESOLVE_OP_ID: &str = "vyre-primitives::vfs::resolve";

/// GPU-Native Virtual File System (VFS) Asynchronous DMA Resolver
///
/// Resolves `#include` directive string identifiers into asynchronous
/// block loads from High-Bandwidth Memory / Persistent Storage directly into the L1 Warp-Arena.
#[must_use]
pub fn vfs_resolve_dma(
    include_hashes: &str,
    out_file_buffers: &str,
    num_requests: Expr,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind("file_hash", Expr::load(include_hashes, t.clone())),
        // Async transfers now pair on stable stream tags instead of transient
        // handles, so both nodes use the same non-empty identifier.
        Node::AsyncLoad {
            source: Ident::from("global_dma_pool"),
            destination: Ident::from(out_file_buffers),
            offset: Box::new(Expr::var("file_hash")),
            size: Box::new(Expr::u32(4096)),
            tag: Ident::from("vfs_req"),
        },
        Node::AsyncWait {
            tag: Ident::from("vfs_req"),
        },
    ];

    let body = vec![Node::Region {
        generator: Ident::from(VFS_RESOLVE_OP_ID),
        source_region: None,
        body: Arc::new(vec![Node::if_then(
            Expr::lt(t.clone(), num_requests.clone()),
            loop_body,
        )]),
    }];

    let n = match &num_requests {
        Expr::LitU32(v) => *v,
        _ => 1,
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(include_hashes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(out_file_buffers, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n),
            BufferDecl::storage("global_dma_pool", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(1024),
        ],
        [256, 1, 1], // Warp aligned
        body,
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        VFS_RESOLVE_OP_ID,
        || vfs_resolve_dma("include_hashes", "out_file_buffers", Expr::u32(1)),
        Some(|| {
            let mut dma_pool = vec![0u8; 4096];
            dma_pool[..4].copy_from_slice(&[1, 2, 3, 4]);
            vec![vec![
                0u32.to_le_bytes().to_vec(),
                vec![0u8; 4],
                dma_pool,
            ]]
        }),
        Some(|| {
            vec![vec![
                vec![1, 2, 3, 4],
            ]]
        }),
    )
}
