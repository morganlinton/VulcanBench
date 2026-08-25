//! Node encoder for the stable IR wire format.

use super::{put_expr, put_nodes};
use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::framing::{put_len_u32, put_string, put_u32, put_u8};
use crate::serial::wire::{Node, MAX_OPAQUE_PAYLOAD_LEN};

/// Append the wire-format tag and payload for one [`Node`] to `out`.
///
/// # Role
///
/// Encodes statement-level IR: bindings, assignments, stores,
/// control flow, barriers, and async operations. Every variant maps
/// to a discriminant byte followed by a payload whose shape mirrors
/// the in-memory `Node` layout.
///
/// # Invariants
///
/// * `out` is appended to only; no bytes are removed or reordered.
/// * Nested expression payloads delegate to [`put_expr()`].
/// * Nested node lists (`If` branches, `Loop` body, `Block`) delegate
///   to [`put_nodes()`], which preserves the append-only invariant.
///
/// # Pre-conditions
///
/// All expressions embedded in the node must satisfy the
/// pre-conditions of [`put_expr()`] (stable wire tags, bounded string
/// lengths).
///
/// # Return semantics
///
/// * `Ok(())` – the node was fully appended to `out`.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`.
///
/// # Failure modes
///
/// Same as [`put_expr()`] for expression sub-payloads, plus
/// [`put_nodes()`] failures for nested statement lists (e.g. node
/// count exceeds `u32::MAX`).
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when a node contains an unmapped/oversized nested
/// payload or a nested statement list that cannot fit the stable wire format.
#[inline]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "wire discriminant table is an ABI contract and must remain auditable in one encoder"
)]
pub fn put_node(out: &mut Vec<u8>, node: &Node) -> Result<(), WireEncodeErr> {
    match node {
        Node::Let { name, value } => {
            put_u8(out, 0);
            put_string(out, name.as_str())?;
            put_expr(out, value)?;
        }
        Node::Assign { name, value } => {
            put_u8(out, 1);
            put_string(out, name.as_str())?;
            put_expr(out, value)?;
        }
        Node::Store {
            buffer,
            index,
            value,
        } => {
            put_u8(out, 2);
            put_string(out, buffer.as_str())?;
            put_expr(out, index)?;
            put_expr(out, value)?;
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            put_u8(out, 3);
            put_expr(out, cond)?;
            put_nodes(out, then)?;
            put_nodes(out, otherwise)?;
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            put_u8(out, 4);
            put_string(out, var.as_str())?;
            put_expr(out, from)?;
            put_expr(out, to)?;
            put_nodes(out, body)?;
        }
        Node::Return => put_u8(out, 5),
        Node::Block(nodes) => {
            put_u8(out, 6);
            put_nodes(out, nodes)?;
        }
        Node::Barrier { ordering } => {
            put_u8(out, 7);
            put_u8(out, ordering.wire_tag());
        }
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => {
            put_u8(out, 8);
            put_string(out, count_buffer.as_str())?;
            out.extend_from_slice(&count_offset.to_le_bytes());
        }
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => {
            put_u8(out, 9);
            put_string(out, source.as_str())?;
            put_string(out, destination.as_str())?;
            put_expr(out, offset)?;
            put_expr(out, size)?;
            put_string(out, tag.as_str())?;
        }
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => {
            put_u8(out, 12);
            put_string(out, source.as_str())?;
            put_string(out, destination.as_str())?;
            put_expr(out, offset)?;
            put_expr(out, size)?;
            put_string(out, tag.as_str())?;
        }
        Node::Trap { address, tag } => {
            put_u8(out, 13);
            put_expr(out, address)?;
            put_string(out, tag.as_str())?;
        }
        Node::Resume { tag } => {
            put_u8(out, 14);
            put_string(out, tag.as_str())?;
        }
        Node::AllReduce { buffer, op, group } => {
            put_u8(out, 15);
            put_string(out, buffer.as_str())?;
            put_u8(out, op.builtin_wire_tag());
            put_u32(out, group.as_u32());
        }
        Node::AllGather {
            input,
            output,
            group,
        } => {
            put_u8(out, 16);
            put_string(out, input.as_str())?;
            put_string(out, output.as_str())?;
            put_u32(out, group.as_u32());
        }
        Node::ReduceScatter {
            input,
            output,
            op,
            group,
        } => {
            put_u8(out, 17);
            put_string(out, input.as_str())?;
            put_string(out, output.as_str())?;
            put_u8(out, op.builtin_wire_tag());
            put_u32(out, group.as_u32());
        }
        Node::Broadcast {
            buffer,
            root,
            group,
        } => {
            put_u8(out, 18);
            put_string(out, buffer.as_str())?;
            put_u32(out, *root);
            put_u32(out, group.as_u32());
        }
        Node::AsyncWait { tag } => {
            put_u8(out, 10);
            put_string(out, tag.as_str())?;
        }
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            put_u8(out, 11);
            put_string(out, generator.as_str())?;
            match source_region {
                Some(region) => {
                    put_u8(out, 1);
                    put_string(out, region.name.as_str())?;
                }
                None => put_u8(out, 0),
            }
            put_nodes(out, body)?;
        }
        Node::Opaque(extension) => {
            put_u8(out, 0x80);
            put_string(out, extension.extension_kind())?;
            let payload = extension.wire_payload();
            if payload.len() > MAX_OPAQUE_PAYLOAD_LEN {
                return Err(WireEncodeErr::fmt_usize(
                    "opaque node payload",
                    payload.len(),
                    &format!(" exceeds {MAX_OPAQUE_PAYLOAD_LEN}. Fix: split the payload across multiple opaque nodes or reduce the extension data size."),
                ));
            }
            put_len_u32(out, payload.len(), "opaque node payload length")?;
            out.extend_from_slice(&payload);
        }
    }
    Ok(())
}
