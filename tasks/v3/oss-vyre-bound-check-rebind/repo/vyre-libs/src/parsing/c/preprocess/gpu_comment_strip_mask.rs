//! GPU comment-strip mask for C source.
//!
//! Phase 17b.5: per-byte mask `0 = code, 1 = drop comment byte,
//! 2 = C-required replacement space`. Composes
//! with `gpu_line_splice_classify` (multiply masks element-wise) and
//! `stream_compact` to produce a comment-and-splice-free byte stream
//! before lexing.
//!
//! ## GPU formulation
//!
//! Comment state is sequential by nature: a `/*` opens until the next
//! `*/`; a `//` opens until the next `\n`. Pure parallel formulations
//! exist (segmented scans, balanced-paren style) but each adds enough
//! complexity and register pressure. The shipped kernel uses exactly
//! one GPU lane per dispatch, byte-at-a-time, so production code stays
//! GPU-resident without launching idle helper lanes or routing comment
//! semantics through a CPU helper.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `bytes_in`  -  compatibility [`gpu_comment_strip_mask`] expects
//!     packed little-endian `DataType::U32` words, four source bytes per
//!     word. [`gpu_comment_strip_mask_u8`] expects one `DataType::U8`
//!     element per source byte.
//!
//! Outputs:
//!   - `comment_mask_out` (U32)  -  one entry per byte. `0` for ordinary
//!     source, `1` for comment bytes to drop, `2` for the first comment
//!     byte that must materialize as one replacement space.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

mod abi;
mod builder;
#[cfg(any(test, feature = "cpu-parity"))]
mod reference;
#[cfg(test)]
mod tests;

pub use abi::{BINDING_BYTES_IN, BINDING_COMMENT_MASK_OUT, OP_ID};
pub use builder::{gpu_comment_strip_mask, gpu_comment_strip_mask_u8};
#[cfg(any(test, feature = "cpu-parity"))]
pub use reference::reference_gpu_comment_strip_mask;
