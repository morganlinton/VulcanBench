//! `taint_kill`  -  one-Region set-difference dataflow primitive.
//!
//! Given a `frontier_in` bitset of currently-tainted nodes and a
//! `kill_set` bitset of nodes whose taint must be dropped (e.g.
//! sanitizer-tagged nodes, or nodes covered by an explicit guard),
//! emit a Program that writes `frontier_out = frontier_in & !kill_set`.
//!
//! This is the symmetric counterpart to `bitset_or_into`'s "merge
//! reach" pattern: where `bitset_or_into` accumulates positives,
//! `taint_kill` removes negatives. Downstream analyzer's `sanitized_by` uses this
//! as its first stage; the iterated taint-fixpoint loop applies it
//! after each step to guarantee sanitizer nodes never re-enter the
//! frontier.
//!
//! Soundness: ``Exact``. The set difference
//! is bit-precise on every word; no over- or under-approximation.

use vyre::ir::Program;
use vyre_primitives::bitset::and_not::bitset_and_not;
use vyre_primitives::graph::csr_forward_traverse::bitset_words;

pub(crate) const OP_ID: &str = "vyre-libs::security::taint_kill";

/// Emit `frontier_out = frontier_in & !kill_set`.
///
/// `node_count` defines how many bits the bitset covers; the emitted
/// Program iterates one thread per bit (rounded up to the next 32-bit
/// word boundary). Backends that prefer word-level dispatch can rely
/// on `bitset_words(node_count)` to size their workgroup grid.
#[must_use]
pub fn taint_kill(
    node_count: u32,
    frontier_in: &str,
    kill_set: &str,
    frontier_out: &str,
) -> Program {
    let words = bitset_words(node_count);
    let primitive = bitset_and_not(frontier_in, kill_set, frontier_out, words);
    Program::wrapped(
        primitive.buffers().to_vec(),
        primitive.workgroup_size(),
        crate::region::reparent_program_children(&primitive, OP_ID),
    )
}

/// CPU oracle. Mirrors the per-word semantic exactly.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(frontier_in: &[u32], kill_set: &[u32]) -> Vec<u32> {
    vyre_primitives::bitset::and_not::cpu_ref(frontier_in, kill_set)
}

/// Marker type for the taint_kill dataflow primitive.
pub struct TaintKill;

impl vyre::soundness::SoundnessTagged for TaintKill {
    fn soundness(&self) -> vyre::soundness::Soundness {
        vyre::soundness::Soundness::Exact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_kill_set_passes_frontier_through() {
        assert_eq!(cpu_ref(&[0xFFFF_FFFF], &[0]), vec![0xFFFF_FFFF]);
    }

    #[test]
    fn full_kill_set_zeros_frontier() {
        assert_eq!(cpu_ref(&[0xFFFF_FFFF], &[0xFFFF_FFFF]), vec![0]);
    }

    #[test]
    fn partial_kill_set_drops_specific_bits() {
        // Frontier covers bits 0-15; kill set covers bits 8-15.
        // Result: only bits 0-7 remain.
        let frontier = [0x0000_FFFFu32];
        let kill = [0x0000_FF00u32];
        assert_eq!(cpu_ref(&frontier, &kill), vec![0x0000_00FF]);
    }

    #[test]
    fn idempotent_under_repeated_application() {
        let frontier = [0xDEAD_BEEFu32];
        let kill = [0xF0F0_F0F0u32];
        let after_one = cpu_ref(&frontier, &kill);
        let after_two = cpu_ref(&after_one, &kill);
        assert_eq!(after_one, after_two);
    }

    #[test]
    fn taint_kill_program_emits_and_not_region() {
        let program = taint_kill(64, "fin", "kill", "fout");
        let buffer_names: Vec<&str> = program.buffers().iter().map(|b| b.name()).collect();
        assert!(buffer_names.contains(&"fin"));
        assert!(buffer_names.contains(&"kill"));
        assert!(buffer_names.contains(&"fout"));
    }
}
