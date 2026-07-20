//! Shared per-word unary bitset kernel builder.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

pub(crate) fn bitset_unary_word_program(
    op_id: &'static str,
    input: &str,
    output: &str,
    words: u32,
    op: UnOp,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![Node::store(
        output,
        t.clone(),
        Expr::UnOp {
            op,
            operand: Box::new(Expr::load(input, t.clone())),
        },
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_unary_word_program_lengths_are_declared_exactly() {
        let mut cases = 0usize;
        for words in 0..=2048 {
            for op in [UnOp::BitNot, UnOp::Popcount] {
                let program = bitset_unary_word_program(
                    "vyre-primitives::bitset::test",
                    "in",
                    "out",
                    words,
                    op,
                );
                assert_eq!(program.buffers().len(), 2);
                let output = program
                    .buffers()
                    .iter()
                    .find(|buffer| buffer.name() == "out")
                    .expect("Fix: bitset unary program must declare output buffer.");
                assert_eq!(output.count(), words);
                cases += 1;
            }
        }
        assert_eq!(cases, 4_098);
    }
}
