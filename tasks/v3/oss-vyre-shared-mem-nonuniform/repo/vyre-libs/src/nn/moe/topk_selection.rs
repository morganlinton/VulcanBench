use vyre::ir::{Expr, Node};

pub(super) const BEST_VALS: &str = "best_vals";
pub(super) const BEST_IDXS: &str = "best_idxs";

pub(super) fn init_top_k_slots(k: u32) -> Vec<Node> {
    let mut body = Vec::with_capacity(k as usize * 2);
    for slot in 0..k {
        body.push(Node::Store {
            buffer: BEST_VALS.into(),
            index: Expr::u32(slot),
            value: Expr::f32(f32::NEG_INFINITY),
        });
        body.push(Node::Store {
            buffer: BEST_IDXS.into(),
            index: Expr::u32(slot),
            value: Expr::u32(0),
        });
    }
    body
}

pub(super) fn insert_top_k_candidate(
    k: u32,
    candidate_value: Expr,
    candidate_index: Expr,
) -> Vec<Node> {
    if k == 0 {
        return Vec::new();
    }
    vec![
        Node::let_bind("insert_pos", Expr::u32(k)),
        Node::loop_for(
            "j",
            Expr::u32(0),
            Expr::u32(k),
            vec![Node::if_then(
                Expr::and(
                    Expr::eq(Expr::var("insert_pos"), Expr::u32(k)),
                    Expr::gt(
                        candidate_value.clone(),
                        Expr::load(BEST_VALS, Expr::var("j")),
                    ),
                ),
                vec![Node::assign("insert_pos", Expr::var("j"))],
            )],
        ),
        Node::if_then(
            Expr::lt(Expr::var("insert_pos"), Expr::u32(k)),
            vec![
                Node::loop_for(
                    "shift_j",
                    Expr::u32(0),
                    Expr::u32(k),
                    vec![
                        Node::let_bind("rev", Expr::sub(Expr::u32(k - 1), Expr::var("shift_j"))),
                        Node::if_then(
                            Expr::and(
                                Expr::ge(Expr::var("rev"), Expr::var("insert_pos")),
                                Expr::lt(Expr::var("rev"), Expr::u32(k - 1)),
                            ),
                            vec![
                                Node::Store {
                                    buffer: BEST_VALS.into(),
                                    index: Expr::add(Expr::var("rev"), Expr::u32(1)),
                                    value: Expr::load(BEST_VALS, Expr::var("rev")),
                                },
                                Node::Store {
                                    buffer: BEST_IDXS.into(),
                                    index: Expr::add(Expr::var("rev"), Expr::u32(1)),
                                    value: Expr::load(BEST_IDXS, Expr::var("rev")),
                                },
                            ],
                        ),
                    ],
                ),
                Node::Store {
                    buffer: BEST_VALS.into(),
                    index: Expr::var("insert_pos"),
                    value: candidate_value,
                },
                Node::Store {
                    buffer: BEST_IDXS.into(),
                    index: Expr::var("insert_pos"),
                    value: candidate_index,
                },
            ],
        ),
    ]
}

pub(super) fn copy_top_k_indices(output_indices: &str, k: u32) -> Vec<Node> {
    (0..k)
        .map(|slot| Node::Store {
            buffer: output_indices.into(),
            index: Expr::u32(slot),
            value: Expr::load(BEST_IDXS, Expr::u32(slot)),
        })
        .collect()
}

pub(super) fn copy_top_k_indices_and_normalized_weights(
    out_indices: &str,
    out_weights: &str,
    k: u32,
    denominator: Expr,
) -> Vec<Node> {
    let mut body = Vec::with_capacity(k as usize * 2);
    for slot in 0..k {
        body.push(Node::Store {
            buffer: out_weights.into(),
            index: Expr::u32(slot),
            value: Expr::div(Expr::load(BEST_VALS, Expr::u32(slot)), denominator.clone()),
        });
        body.push(Node::Store {
            buffer: out_indices.into(),
            index: Expr::u32(slot),
            value: Expr::load(BEST_IDXS, Expr::u32(slot)),
        });
    }
    body
}

#[cfg(test)]
mod tests {
    use super::{
        copy_top_k_indices, copy_top_k_indices_and_normalized_weights, init_top_k_slots,
        insert_top_k_candidate,
    };
    use vyre::ir::Expr;

    #[test]
    fn generated_top_k_scaffold_sizes_are_stable() {
        let mut checked = 0_u32;
        for k in 0..=2048 {
            assert_eq!(init_top_k_slots(k).len(), k as usize * 2);
            assert_eq!(copy_top_k_indices("idx", k).len(), k as usize);
            assert_eq!(
                copy_top_k_indices_and_normalized_weights("idx", "weight", k, Expr::var("sum"))
                    .len(),
                k as usize * 2
            );
            assert_eq!(
                insert_top_k_candidate(k, Expr::var("value"), Expr::var("index")).len(),
                if k == 0 { 0 } else { 3 }
            );
            checked += 1;
        }
        assert_eq!(checked, 2_049);
    }
}
