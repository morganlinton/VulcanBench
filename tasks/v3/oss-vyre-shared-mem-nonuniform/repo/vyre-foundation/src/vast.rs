//! **VAST**  -  packed AST wire layout (host validator + tree walks).
//!
//! Matches the buffer contract in `docs/parsing-and-frontends.md` in the
//! vyre workspace (magic `VAST`, fixed `Node` rows). GPU `ast_walk_*`
//! compositions target the same logical layout.

mod error;
mod edit_corpus;
mod header;
mod layout;
mod node;
mod pack;
mod validate;
mod walk;

pub use error::VastError;
pub use edit_corpus::{
    apply_vast_edit_script, changed_ranges_from_vast_edits, vast_edit_corpus_evidence,
    vast_edit_digest, VastChangedRange, VastEdit, VastEditCorpusCase, VastEditCorpusError,
    VastEditCorpusEvidence, VastEditDigest, VAST_EDIT_CORPUS_SCHEMA_VERSION,
};
pub use header::{VastHeader, HEADER_LEN, VAST_MAGIC, VAST_VERSION};
pub use node::{VastFile, VastNode, NODE_STRIDE_U32, SENTINEL};
pub use pack::pack_spine_vast;
pub use validate::validate_vast;
pub use walk::{walk_postorder_indices, walk_preorder_indices};
