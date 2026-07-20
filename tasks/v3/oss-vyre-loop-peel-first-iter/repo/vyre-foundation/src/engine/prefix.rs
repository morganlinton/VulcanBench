//! Host-side prefix array builder.
//!
//! NOTE: This is NOT part of the vyre IR. It runs on the CPU once before a
//! GPU dispatch needs the prefix arrays as input buffers. It does not produce
//! a Program, does not go through validate or lower, and is not registered in
//! the op registry. Callers compose it manually with their dispatch flow.
//!
//! The IR-side equivalents are in `vyre_spec::string::prefix_brace` (and
//! related ops in the `string` and `match_ops` domains) -- those are real
//! Category A compositions that lower to GPU code.

use crate::error::{Error, Result};

/// Maximum input bytes accepted by CPU prefix builders.
///
/// I10: prefix builders allocate one `u32` per input byte, plus one sentinel
/// for newline prefix sums. The 64 MiB bound keeps each prefix array bounded
/// before `Vec::with_capacity` and structurally prevents `bytes.len() + 1`
/// overflow in newline prefix construction. Prevents OOM on the host.
pub const MAX_PREFIX_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Compute brace `{`/`}` depth at each byte offset.
///
/// `result[i]` = nesting depth BEFORE byte `i`.
/// Already existed as `compute_file_brace_depths`  -  moved here for reuse.
///
/// # Errors
///
/// Returns `Error::Prefix` if the input exceeds `MAX_PREFIX_INPUT_BYTES`.
///
/// # Examples
///
/// ```
/// use vyre_foundation::engine::prefix::brace_depth_prefix;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let input = b"{a{b}c}";
/// let depths = brace_depth_prefix(input)?;
/// assert_eq!(depths, vec![0, 1, 1, 2, 2, 1, 1]);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn brace_depth_prefix(bytes: &[u8]) -> Result<Vec<u32>> {
    validate_prefix_input(bytes.len())?;
    let mut depths = Vec::with_capacity(bytes.len().max(1));
    let mut depth = 0u32;
    for &byte in bytes {
        depths.push(depth);
        match byte {
            b'{' => depth = depth.saturating_add(1),
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depths.is_empty() {
        depths.push(0);
    }
    Ok(depths)
}

/// Compute combined brace `{`/`}` and paren `(`/`)` nesting depth at each byte offset.
///
/// `result[i]` = combined nesting depth BEFORE byte `i`.
/// Replaces the O(n) `nested_depth()` shader function.
///
/// # Errors
///
/// Returns `Error::Prefix` if the input exceeds `MAX_PREFIX_INPUT_BYTES`.
///
/// # Examples
///
/// ```
/// use vyre_foundation::engine::prefix::nested_depth_prefix;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let input = b"f({x})";
/// let depths = nested_depth_prefix(input)?;
/// assert_eq!(depths, vec![0, 0, 1, 2, 2, 1]);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn nested_depth_prefix(bytes: &[u8]) -> Result<Vec<u32>> {
    validate_prefix_input(bytes.len())?;
    let mut depths = Vec::with_capacity(bytes.len().max(1));
    let mut depth = 0u32;
    for &byte in bytes {
        depths.push(depth);
        match byte {
            b'{' | b'(' => depth = depth.saturating_add(1),
            b'}' | b')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if depths.is_empty() {
        depths.push(0);
    }
    Ok(depths)
}

/// Compute newline prefix sum for O(1) line-distance queries.
///
/// `result[i]` = number of `\n` bytes in `bytes[0..i]`.
/// `newline_count_between(a, b) = result[b] - result[a]`  -  O(1).
///
/// # Errors
///
/// Returns `Error::Prefix` if the input exceeds `MAX_PREFIX_INPUT_BYTES`
/// or if the prefix capacity overflows `usize`.
///
/// # Examples
///
/// ```
/// use vyre_foundation::engine::prefix::newline_prefix_sum;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let input = b"a\nb\nc";
/// let sums = newline_prefix_sum(input)?;
/// assert_eq!(sums, vec![0, 0, 1, 1, 2, 2]);
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn newline_prefix_sum(bytes: &[u8]) -> Result<Vec<u32>> {
    validate_prefix_input(bytes.len())?;
    let capacity = bytes.len().checked_add(1).ok_or_else(|| Error::Prefix {
        message: "newline prefix capacity overflowed usize. Fix: split the input before prefixing."
            .to_string(),
    })?;
    let mut sums = Vec::with_capacity(capacity);
    let mut count = 0u32;
    sums.push(0);
    for &byte in bytes {
        if byte == b'\n' {
            count = count.saturating_add(1);
        }
        sums.push(count);
    }
    Ok(sums)
}

/// Validate the byte length accepted by prefix construction.
///
/// # Errors
///
/// Returns [`Error::Prefix`] when `byte_len` exceeds
/// [`MAX_PREFIX_INPUT_BYTES`].
#[must_use]
pub fn validate_prefix_input(byte_len: usize) -> Result<()> {
    if byte_len > MAX_PREFIX_INPUT_BYTES {
        return Err(Error::Prefix {
            message: format!(
                "prefix input is {byte_len} bytes, exceeding {MAX_PREFIX_INPUT_BYTES}. Fix: split the file before prefix construction."
            ),
        });
    }
    Ok(())
}
