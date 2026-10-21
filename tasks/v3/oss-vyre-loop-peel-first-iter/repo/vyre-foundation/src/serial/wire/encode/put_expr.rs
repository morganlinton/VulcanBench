//! Expression encoder for the stable IR wire format.

use crate::serial::wire::encode::WireEncodeErr;
use crate::serial::wire::framing::{put_len_u32, put_string, put_u32, put_u8};
use crate::serial::wire::tags::{atomic_op_tag, bin_op_tag, put_data_type, un_op_tag};
use crate::serial::wire::{Expr, MAX_OPAQUE_PAYLOAD_LEN};

/// Append the wire-format tag and payload for one [`Expr`] to `out`.
///
/// # Role
///
/// This is the leaf encoder of the IR wire format. Every expression
/// variant is mapped to a single-byte discriminant followed by a
/// variant-specific payload. The discriminant table is the contract
/// between encoder and decoder; changing it is a breaking schema
/// change (audit L.1.47).
///
/// # Invariants
///
/// * `out` is appended to only; no bytes are removed or reordered.
/// * Recursive calls for nested expressions (`Load`, `BinOp`, `UnOp`,
///   `Call`, `Select`, `Cast`, `Fma`, `Atomic`) preserve this
///   invariant.
///
/// # Pre-conditions
///
/// `expr` must use only enum variants that have a registered stable
/// wire tag. Variants added to `Expr` without an assigned tag
/// will fail encoding (audit L.1.27 / I4).
///
/// # Return semantics
///
/// * `Ok(())` – the expression was fully appended to `out`.
/// * `Err(String)` – an actionable diagnostic starting with `Fix:`
///   describing the unsupported variant or oversized payload.
///
/// # Failure modes
///
/// * **Unmapped variant** – `bin_op_tag`, `un_op_tag`, or
///   `atomic_op_tag` returns `Err` when the op has no wire tag.
/// * **String overflow** – `put_string` rejects names longer than
///   [`crate::serial::wire::MAX_STRING_LEN`].
/// * **Length overflow** – `put_len_u32` rejects argument counts
///   larger than `u32::MAX`.
///
/// # Errors
///
/// Returns [`WireEncodeErr`] when an expression contains an unmapped operation
/// variant, an oversized string/payload, or a child expression that cannot be
/// represented by the stable wire format.
#[inline]
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "wire discriminant table is an ABI contract and must remain auditable in one encoder"
)]
pub fn put_expr(out: &mut Vec<u8>, expr: &Expr) -> Result<(), WireEncodeErr> {
    // Iterative explicit-stack encoder. Recursion-on-Expr blew the
    // 2 MiB default test-thread stack at modest depths (Select chains
    // produced by lowered C control flow, BinOp arithmetic chains).
    // Each variant writes its header bytes synchronously then queues
    // its child Expr references onto the work stack in REVERSE order
    // so they pop in declared order. Atomic interleaves a single
    // discriminator byte between `index` and `expected`/`value`, so
    // the work stack also accepts a literal-byte action.
    enum Step<'e> {
        Encode(&'e Expr),
        WriteByte(u8),
    }
    let mut stack: Vec<Step<'_>> = Vec::with_capacity(16);
    stack.push(Step::Encode(expr));
    while let Some(step) = stack.pop() {
        match step {
            Step::WriteByte(b) => put_u8(out, b),
            Step::Encode(expr) => match expr {
                Expr::LitU32(value) => {
                    put_u8(out, 0);
                    put_u32(out, *value);
                }
                Expr::LitI32(value) => {
                    put_u8(out, 1);
                    put_u32(out, u32::from_le_bytes(value.to_le_bytes()));
                }
                Expr::LitBool(value) => {
                    put_u8(out, 2);
                    put_u8(out, u8::from(*value));
                }
                Expr::LitF32(value) => {
                    put_u8(out, 15);
                    put_u32(out, canonical_f32_bits(*value));
                }
                Expr::Var(name) => {
                    put_u8(out, 3);
                    put_string(out, name)?;
                }
                Expr::Load { buffer, index } => {
                    put_u8(out, 4);
                    put_string(out, buffer)?;
                    stack.push(Step::Encode(index));
                }
                Expr::BufLen { buffer } => {
                    put_u8(out, 5);
                    put_string(out, buffer)?;
                }
                Expr::InvocationId { axis } => {
                    put_u8(out, 6);
                    put_u8(out, *axis);
                }
                Expr::WorkgroupId { axis } => {
                    put_u8(out, 7);
                    put_u8(out, *axis);
                }
                Expr::LocalId { axis } => {
                    put_u8(out, 8);
                    put_u8(out, *axis);
                }
                Expr::BinOp { op, left, right } => {
                    put_u8(out, 9);
                    if let crate::ir::BinOp::Opaque(id) = op {
                        put_u8(out, 0x80);
                        put_u32(out, id.as_u32());
                    } else {
                        put_u8(out, bin_op_tag(*op)?);
                    }
                    // Push right then left so left pops first (declared order).
                    stack.push(Step::Encode(right));
                    stack.push(Step::Encode(left));
                }
                Expr::UnOp { op, operand } => {
                    put_u8(out, 10);
                    if let crate::ir::UnOp::Opaque(id) = op {
                        put_u8(out, 0x80);
                        put_u32(out, id.as_u32());
                    } else {
                        put_u8(out, un_op_tag(op)?);
                    }
                    stack.push(Step::Encode(operand));
                }
                Expr::Call { op_id, args } => {
                    put_u8(out, 11);
                    put_string(out, op_id.as_str())?;
                    put_len_u32(out, args.len(), "call argument count")?;
                    // Push args in reverse so they pop in argument order.
                    for arg in args.iter().rev() {
                        stack.push(Step::Encode(arg));
                    }
                }
                Expr::Select {
                    cond,
                    true_val,
                    false_val,
                } => {
                    put_u8(out, 12);
                    stack.push(Step::Encode(false_val));
                    stack.push(Step::Encode(true_val));
                    stack.push(Step::Encode(cond));
                }
                Expr::Cast { target, value } => {
                    put_u8(out, 13);
                    put_data_type(out, target)?;
                    stack.push(Step::Encode(value));
                }
                Expr::Fma { a, b, c } => {
                    put_u8(out, 16);
                    stack.push(Step::Encode(c));
                    stack.push(Step::Encode(b));
                    stack.push(Step::Encode(a));
                }
                Expr::Atomic {
                    op,
                    buffer,
                    index,
                    expected,
                    value,
                    ordering,
                } => {
                    put_u8(out, 14);
                    if let crate::ir::AtomicOp::Opaque(id) = op {
                        put_u8(out, 0x80);
                        put_u32(out, id.as_u32());
                    } else {
                        put_u8(out, atomic_op_tag(*op)?);
                    }
                    put_u8(out, ordering.wire_tag());
                    put_string(out, buffer)?;
                    // Wire order: index, expected_present_byte, expected?, value.
                    // Push value last (pops last), then the expected branch,
                    // then index (pops first).
                    stack.push(Step::Encode(value));
                    match expected {
                        Some(expected_expr) => {
                            stack.push(Step::Encode(expected_expr));
                            stack.push(Step::WriteByte(1));
                        }
                        None => {
                            stack.push(Step::WriteByte(0));
                        }
                    }
                    stack.push(Step::Encode(index));
                }
                Expr::SubgroupReduce { op, value } => {
                    put_u8(out, 17);
                    put_u8(out, op.builtin_wire_tag());
                    stack.push(Step::Encode(value));
                }
                Expr::SubgroupShuffle { value, lane } => {
                    put_u8(out, 18);
                    stack.push(Step::Encode(lane));
                    stack.push(Step::Encode(value));
                }
                Expr::SubgroupBallot { cond } => {
                    put_u8(out, 19);
                    stack.push(Step::Encode(cond));
                }
                Expr::SubgroupLocalId => {
                    put_u8(out, 20);
                }
                Expr::SubgroupSize => {
                    put_u8(out, 21);
                }
                Expr::Opaque(extension) => {
                    put_u8(out, 0x80);
                    put_string(out, extension.extension_kind())?;
                    let payload = extension.wire_payload();
                    if payload.len() > MAX_OPAQUE_PAYLOAD_LEN {
                        return Err(WireEncodeErr::fmt_usize(
                            "opaque expression payload",
                            payload.len(),
                            &format!(" exceeds {MAX_OPAQUE_PAYLOAD_LEN}. Fix: split the payload across multiple opaque expressions or reduce the extension data size."),
                        ));
                    }
                    put_len_u32(out, payload.len(), "opaque expression payload length")?;
                    out.extend_from_slice(&payload);
                }
            },
        }
    }
    Ok(())
}

#[inline]
fn canonical_f32_bits(value: f32) -> u32 {
    if value.is_nan() {
        return 0x7FC0_0000;
    }
    if value.is_subnormal() {
        return 0.0f32.to_bits();
    }
    let bits = value.to_bits();
    if bits == (-0.0f32).to_bits() {
        0.0f32.to_bits()
    } else {
        bits
    }
}
