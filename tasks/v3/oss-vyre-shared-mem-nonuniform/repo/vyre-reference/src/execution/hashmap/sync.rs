//! Barrier and uniform-control-flow checks for the HashMap interpreter.
//!
//! The executor calls these helpers between round-robin steps to preserve the
//! reference interpreter's workgroup-wide barrier semantics.

use super::state::HashmapInvocation;
use smallvec::SmallVec;
use vyre::ir::BufferDecl;
use vyre::Error;

pub(crate) use crate::execution::node_tree::{contains_barrier, node_id};

pub(crate) fn release_barrier_if_ready(invocations: &mut [HashmapInvocation<'_>]) -> bool {
    let active = invocations.iter().filter(|inv| !inv.done()).count();
    let waiting = live_waiting_count(invocations);
    if active > 0 && active == waiting {
        for inv in invocations {
            inv.waiting_at_barrier = false;
        }
        true
    } else {
        false
    }
}

pub(crate) fn live_waiting_count(invocations: &[HashmapInvocation<'_>]) -> usize {
    invocations
        .iter()
        .filter(|inv| !inv.done() && inv.waiting_at_barrier)
        .count()
}

pub(crate) fn verify_uniform_control_flow(
    invocations: &[HashmapInvocation<'_>],
) -> Result<(), Error> {
    let mut observed = SmallVec::<[(usize, bool); 8]>::new();
    for invocation in invocations.iter().filter(|inv| !inv.done()) {
        for (id, value) in &invocation.uniform_checks {
            if let Some((_, previous)) = observed.iter().find(|(seen_id, _)| seen_id == id) {
                if previous != value {
                    return Err(Error::interp(
                        "program violates uniform-control-flow rule: Barrier appears inside an If whose condition differs across the workgroup. Fix: make the condition uniform or move Barrier outside the branch.",
                    ));
                }
            } else {
                observed.push((*id, *value));
            }
        }
    }
    Ok(())
}

pub(crate) fn element_count(decl: &BufferDecl, byte_len: usize) -> Result<u32, Error> {
    if let Some(bits) = decl.element().bit_width() {
        let total_bits = byte_len.checked_mul(8).ok_or_else(|| {
            Error::interp(format!(
                "buffer `{}` has {} bytes and overflows host bit counting. Fix: shrink declaration footprint or split work.",
                decl.name(),
                byte_len,
            ))
        })?;
        let elements = total_bits / bits;
        return u32::try_from(elements).map_err(|_| {
            Error::interp(format!(
                "buffer `{}` has {} bytes for {}-bit elements and overflows u32 elements. Fix: shrink declaration footprint or split work.",
                decl.name(),
                byte_len,
                bits,
            ))
        });
    }
    let Some(stride) = decl.element().size_bytes() else {
        return Err(Error::interp(format!(
            "buffer `{}` has unsized element type {}. Fix: provide a fixed-width buffer element type before invoking the reference interpreter.",
            decl.name(),
            decl.element()
        )));
    };
    if stride == 0 {
        return u32 :: try_from (byte_len) . map_err (| _ | { Error :: interp (format ! ("buffer `{}` has {} bytes and cannot be indexed within u32 address space. Fix: shrink or split the invocation." , decl . name () , byte_len ,)) }) ;
    }
    let elements = byte_len / stride;
    u32 :: try_from (elements) . map_err (| _ | { Error :: interp (format ! ("buffer `{}` has {} bytes for stride {} and overflows u32 elements. Fix: shrink declaration footprint or split work." , decl . name () , byte_len , stride ,)) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::DataType;

    #[test]
    fn element_count_uses_bit_width_for_packed_i4_buffers() {
        let decl = BufferDecl::read("packed", 0, DataType::I4).with_count(8);

        assert_eq!(
            element_count(&decl, 4).expect("Fix: packed I4 count must be computable."),
            8,
            "Fix: four bytes of I4 storage contain eight logical elements."
        );
        assert_eq!(
            element_count(&decl, 3).expect("Fix: packed I4 count must be computable."),
            6,
            "Fix: partial packed I4 buffers must report logical element count from bits, not bytes."
        );
    }
}
