//! Sequence encoder for IR node lists.

use super::put_node;
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::framing::put_len_u32;
use crate::serial::wire::Node;

/// Append a length-prefixed sequence of IR nodes to `out`.
///
/// # Role
///
/// This is the canonical encoder for statement lists: entry bodies,
/// `If`/`Loop`/`Block` children, and any other `[Node]` slice. The
/// wire format represents every list as a little-endian `u32` count
/// followed by that many encoded nodes.
///
/// # Invariants
///
/// * `out` is appended to only.
/// * The length field written equals `nodes.len()`; truncation or
///   padding would corrupt the decoder's bounds checks (I10).
///
/// # Pre-conditions
///
/// `nodes.len()` must fit in `u32`; otherwise the wire format cannot
/// represent the sequence.
///
/// # Return semantics
///
/// * `Ok(())` – count and all nodes were appended.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// * **Length overflow** – `put_len_u32` rejects `nodes.len()` >
///   `u32::MAX` with a `Fix:` error advising the caller to split
///   the program.
/// * **Node encoding failure** – any error from [`put_node`] is
///   propagated upward unchanged.
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when the node count cannot fit the wire format or
/// any contained node fails to encode.
#[inline]
#[must_use]
pub fn put_nodes(out: &mut Vec<u8>, nodes: &[Node]) -> Result<(), WireEncodeErr> {
    put_len_u32(out, nodes.len(), "node count")?;
    for node in nodes {
        put_node(out, node)?;
    }
    Ok(())
}
