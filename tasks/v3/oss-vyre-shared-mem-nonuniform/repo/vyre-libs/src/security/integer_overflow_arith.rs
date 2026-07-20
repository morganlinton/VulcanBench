//! `integer_overflow_arith`  -  does this binary op overflow on
//! attacker input? CWE-190 supporting predicate.
//!
//! Per node `n`, write 1 iff `n` is a binary arithmetic node
//! (mul / add / shl) AND at least one operand is reachable from
//! `@http_input_family` AND there is no dominating overflow check.

use std::sync::Arc;

use vyre::ir::model::expr::Ident;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::graph::csr_forward_traverse::bitset_words;

pub(crate) const OP_ID: &str = "vyre-libs::security::integer_overflow_arith";

/// Build an overflow-check Program: arith_set AND attacker_reach
/// AND NOT overflow_check_dominates.
#[must_use]
pub fn integer_overflow_arith(
    node_count: u32,
    arith_set: &str,
    attacker_reach: &str,
    overflow_check_dominates: &str,
    intermediate: &str,
    out: &str,
) -> Program {
    let words = bitset_words(node_count);
    let t = Expr::InvocationId { axis: 0 };
    let attacker_arith = Expr::bitand(
        Expr::load(arith_set, t.clone()),
        Expr::load(attacker_reach, t.clone()),
    );
    let body = vec![
        Node::let_bind("attacker_arith", attacker_arith),
        Node::store(intermediate, t.clone(), Expr::var("attacker_arith")),
        Node::store(
            out,
            t.clone(),
            Expr::bitand(
                Expr::var("attacker_arith"),
                Expr::bitnot(Expr::load(overflow_check_dominates, t.clone())),
            ),
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(arith_set, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(attacker_reach, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(
                overflow_check_dominates,
                2,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(words),
            BufferDecl::storage(intermediate, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::output(out, 4, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

/// CPU oracle.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(
    arith_set: &[u32],
    attacker_reach: &[u32],
    overflow_check_dominates: &[u32],
) -> Vec<u32> {
    let inter = vyre_primitives::bitset::and::cpu_ref(arith_set, attacker_reach);
    vyre_primitives::bitset::and_not::cpu_ref(&inter, overflow_check_dominates)
}

/// Soundness marker for [`integer_overflow_arith`].
pub struct IntegerOverflowArith;
impl vyre::soundness::SoundnessTagged for IntegerOverflowArith {
    fn soundness(&self) -> vyre::soundness::Soundness {
        vyre::soundness::Soundness::Exact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unguarded_attacker_arith_fires() {
        // arith {0,1,2,3}, attacker {1,2}, no checks.
        assert_eq!(cpu_ref(&[0b1111], &[0b0110], &[0]), vec![0b0110]);
    }

    #[test]
    fn guarded_does_not_fire() {
        assert_eq!(cpu_ref(&[0b1111], &[0b0110], &[0b0010]), vec![0b0100]);
    }

    #[test]
    fn no_attacker_means_no_finding() {
        assert_eq!(cpu_ref(&[0b1111], &[0], &[0]), vec![0]);
    }

    #[test]
    fn no_arith_means_no_finding() {
        assert_eq!(cpu_ref(&[0], &[0xFFFF], &[0]), vec![0]);
    }
}
