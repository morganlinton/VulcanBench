//! Packed AST (VAST)  -  see `vyre-foundation::vast` and workspace
//! `docs/parsing-and-frontends.md`.

pub use vyre_foundation::vast::{
    pack_spine_vast, validate_vast, walk_postorder_indices, walk_preorder_indices, VastError,
    VastHeader, VastNode, HEADER_LEN, NODE_STRIDE_U32, SENTINEL, VAST_MAGIC, VAST_VERSION,
};
