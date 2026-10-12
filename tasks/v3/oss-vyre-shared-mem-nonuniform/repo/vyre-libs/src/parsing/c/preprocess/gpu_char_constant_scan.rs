//! GPU char-constant scanner.
//!
//! Phase 17b.3: parse C char constants like `'A'`, `'\n'`, `L'X'`,
//! `'\\\''`, etc. Returns `(value, bytes_consumed, ok)`.
//!
//! ## Pass split
//!
//! - **17b.3a (this commit):** prefix tolerance (`L`, `u`, `U`, `u8`),
//!   single-char constants, and the simple escape table
//!   (`\\ \' \" \? \a \b \f \n \r \t \v \0` and `\<otherbyte> → otherbyte`).
//! - **17b.3b (follow-up):** numeric escapes  -  octal (`\012`), hex
//!   (`\xff`), and universal-character escapes (`A`, `\U00000041`).
//!   Land in the same kernel by extending the escape branch.
//!
//! ## Limitation
//!
//! `value` is `u32` with wrapping arithmetic, mirroring the
//! `gpu_int_literal_scan` contract. Multi-char concatenation
//! (`'ABCD'`) is supported via `value = (value << 8) | (byte & 0xff)`
//!  -  this matches the CPU `consume_char_constant`'s `wrapping_shl(8)`
//! semantics on a u64 truncated to u32.
//!
//! ## Wire layout
//!
//! Inputs:
//!   - `source` (U8)
//!   - `start_pos` (U32, single element).
//!
//! Outputs:
//!   - `value_out` (U32, single element).
//!   - `bytes_consumed_out` (U32, single element).
//!   - `ok_out` (U32, single element). `1` if a valid char constant
//!     was scanned; `0` if no constant at this position OR it was
//!     malformed (unterminated, embedded newline, empty `''`).

use vyre::ir::{Expr, Node, Program};

mod abi;
mod builder;
#[cfg(test)]
mod tests;

pub use abi::{
    BINDING_BYTES_CONSUMED_OUT, BINDING_OK_OUT, BINDING_SOURCE, BINDING_START_POS,
    BINDING_VALUE_OUT, MAX_CONTENT_BYTES, OP_ID,
};
pub use builder::gpu_char_constant_scan;
