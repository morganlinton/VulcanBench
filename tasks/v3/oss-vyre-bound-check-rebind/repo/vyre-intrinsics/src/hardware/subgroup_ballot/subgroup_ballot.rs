//! Cat-C `subgroup_ballot`  -  popcount of per-lane bool into u32 bitmask.
//!
//! Maps to the target-native subgroup ballot intrinsic via a concrete
//! driver emitter arm.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::hardware::pack_u32;
use crate::hardware::MAP_WORKGROUP;

/// Build a Program that collects the per-lane boolean predicate into a u32
/// bitmask broadcast to every lane.
#[must_use]
pub fn subgroup_ballot(cond_input: &str, out: &str, n: u32) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        "vyre-intrinsics::hardware::subgroup_ballot",
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![Node::store(
                    out,
                    Expr::var("idx"),
                    Expr::SubgroupBallot {
                        cond: Box::new(Expr::eq(
                            Expr::load(cond_input, Expr::var("idx")),
                            Expr::u32(1),
                        )),
                    },
                )],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(cond_input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

fn cpu_ref(cond: &[u32]) -> Vec<u8> {
    const SUBGROUP_WIDTH: usize = 32;
    let n = cond.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let subgroup_start = (i / SUBGROUP_WIDTH) * SUBGROUP_WIDTH;
        let subgroup_end = (subgroup_start + SUBGROUP_WIDTH).min(n);
        let mut mask = 0u32;
        for (lane, c) in cond[subgroup_start..subgroup_end].iter().enumerate() {
            if *c == 1 {
                mask |= 1u32 << lane;
            }
        }
        out.push(mask);
    }
    pack_u32(&out)
}

fn test_inputs() -> Vec<Vec<Vec<u8>>> {
    let cond = vec![0u32, 1, 0, 1];
    let len = cond.len() * 4;
    vec![vec![pack_u32(&cond), vec![0u8; len]]]
}

fn expected_output() -> Vec<Vec<Vec<u8>>> {
    let cond = vec![0u32, 1, 0, 1];
    vec![vec![cpu_ref(&cond)]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-intrinsics::hardware::subgroup_ballot",
        build: || subgroup_ballot("cond", "out", 4),
        test_inputs: Some(test_inputs),
        expected_output: Some(expected_output),
        category: Some("hardware"),
        shape: Some(crate::harness::OpShape::new(
            1,
            1,
            4,
            crate::harness::HardwareSemantic::SubgroupBallotU32,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::run_program;

    fn assert_case(cond: &[u32]) {
        let n = cond.len() as u32;
        let program = subgroup_ballot("cond", "out", n.max(1));
        let outputs = run_program(
            &program,
            vec![pack_u32(cond), vec![0u8; (n.max(1) * 4) as usize]],
        );
        assert_eq!(outputs, vec![cpu_ref(cond)]);
    }

    #[test]
    fn one_element_true() {
        assert_case(&[1]);
    }
    #[test]
    fn one_element_false() {
        assert_case(&[0]);
    }
    #[test]
    fn mixed() {
        assert_case(&[0, 1, 0, 1, 1, 1, 0, 0]);
    }
}
