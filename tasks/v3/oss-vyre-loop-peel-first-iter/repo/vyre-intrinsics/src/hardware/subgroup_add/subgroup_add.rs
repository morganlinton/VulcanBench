//! Cat-C `subgroup_add`  -  per-lane sum reduction broadcast to every lane.
//! Maps to hardware `subgroupAdd()`.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::hardware::pack_u32;
use crate::hardware::MAP_WORKGROUP;

/// Build a Program whose per-lane output is the sum of all active subgroup
/// lanes.
#[must_use]
pub fn subgroup_add(values: &str, out: &str, n: u32) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        "vyre-intrinsics::hardware::subgroup_add",
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![
                    Node::let_bind(
                        "group_base",
                        Expr::mul(Expr::div(Expr::var("idx"), Expr::u32(32)), Expr::u32(32)),
                    ),
                    Node::let_bind("sum", Expr::u32(0)),
                    Node::loop_for(
                        "lane",
                        Expr::u32(0),
                        Expr::u32(32),
                        vec![
                            Node::let_bind(
                                "peer",
                                Expr::add(Expr::var("group_base"), Expr::var("lane")),
                            ),
                            Node::if_then(
                                Expr::lt(Expr::var("peer"), Expr::buf_len(values)),
                                vec![Node::assign(
                                    "sum",
                                    Expr::add(
                                        Expr::var("sum"),
                                        Expr::load(values, Expr::var("peer")),
                                    ),
                                )],
                            ),
                        ],
                    ),
                    Node::store(out, Expr::var("idx"), Expr::var("sum")),
                ],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(values, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 1, DataType::U32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

fn cpu_ref(values: &[u32]) -> Vec<u8> {
    const SUBGROUP_WIDTH: usize = 32;
    let mut out = Vec::with_capacity(values.len() * 4);
    for chunk in values.chunks(SUBGROUP_WIDTH) {
        let sum = chunk.iter().copied().fold(0u32, u32::wrapping_add);
        for _ in 0..chunk.len() {
            out.extend_from_slice(&sum.to_le_bytes());
        }
    }
    out
}

fn test_inputs() -> Vec<Vec<Vec<u8>>> {
    let values = vec![1u32, 2, 3, 4];
    let len = values.len() * 4;
    vec![vec![pack_u32(&values), vec![0u8; len]]]
}

fn expected_output() -> Vec<Vec<Vec<u8>>> {
    let values = vec![1u32, 2, 3, 4];
    vec![vec![cpu_ref(&values)]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-intrinsics::hardware::subgroup_add",
        build: || subgroup_add("values", "out", 4),
        test_inputs: Some(test_inputs),
        expected_output: Some(expected_output),
        category: Some("hardware"),
        shape: Some(crate::harness::OpShape::new(
            1,
            1,
            4,
            crate::harness::HardwareSemantic::SubgroupAddU32,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{lcg_u32, run_program};

    fn assert_case(values: &[u32]) {
        let n = values.len() as u32;
        let program = subgroup_add("values", "out", n.max(1));
        let outputs = run_program(
            &program,
            vec![pack_u32(values), vec![0u8; (n.max(1) * 4) as usize]],
        );
        assert_eq!(outputs, vec![cpu_ref(values)]);
    }

    #[test]
    fn one_element() {
        assert_case(&[42]);
    }
    #[test]
    fn max_value() {
        assert_case(&[u32::MAX]);
    }
    #[test]
    fn random_sixty_four() {
        assert_case(&lcg_u32(0xC100_0033, 64));
    }
}
