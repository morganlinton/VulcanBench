#![allow(
    missing_docs,
    dead_code,
    unused_imports,
    unused_variables,
    unreachable_patterns,
    clippy::all
)]
use crate::quick::QuickOp;
use crate::quick_cache::{eval_and, eval_or, QuickMutation};

pub(crate) fn mutations_for(op: &QuickOp) -> Vec<QuickMutation> {
    if op.id.contains(".bitwise.") {
        vec![
            QuickMutation {
                id: "bit_op_swap_xor_to_and",
                from: "^",
                eval: Some(eval_and),
                laws: None,
            },
            QuickMutation {
                id: "bit_op_swap_xor_to_or",
                from: "^",
                eval: Some(eval_or),
                laws: None,
            },
            QuickMutation {
                id: "law_delete",
                from: "AlgebraicLaw",
                eval: None,
                laws: Some(&[]),
            },
        ]
    } else {
        vec![QuickMutation {
            id: "law_delete",
            from: "AlgebraicLaw",
            eval: None,
            laws: Some(&[]),
        }]
    }
}
