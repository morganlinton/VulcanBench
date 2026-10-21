//! Compile-time constant-buffer folding and shader monomorphization support.
//!
//! This pass-level utility replaces loads from known immutable buffers with
//! literal immediates. Lowering then emits immediate expressions instead of
//! storage-buffer reads, which lets a backend monomorphize shaders for static
//! LUTs without carrying a runtime binding.

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{fingerprint_program, PassResult};

/// Compile-time-known u32 buffer contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstBuffer {
    /// Buffer name referenced by `Expr::Load`.
    pub name: Ident,
    /// Immutable u32 values available at compile time.
    pub values: Vec<u32>,
}

/// Inline literal loads from a compile-time-known u32 buffer.
#[must_use]
pub fn fold_const_buffer(program: &Program, constant: &ConstBuffer) -> PassResult {
    let before_fp = fingerprint_program(program);
    let entry = program
        .entry()
        .iter()
        .map(|node| fold_node(node, constant))
        .collect();
    let optimized = program.with_rewritten_entry(entry);
    let changed = fingerprint_program(&optimized) != before_fp;
    PassResult {
        program: optimized,
        changed,
    }
}

fn fold_node(node: &Node, constant: &ConstBuffer) -> Node {
    match node {
        Node::Let { name, value } => Node::let_bind(name.clone(), fold_expr(value, constant)),
        Node::Assign { name, value } => Node::Assign {
            name: name.clone(),
            value: fold_expr(value, constant),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::store(
            buffer,
            fold_expr(index, constant),
            fold_expr(value, constant),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            fold_expr(cond, constant),
            then.iter().map(|node| fold_node(node, constant)).collect(),
            otherwise
                .iter()
                .map(|node| fold_node(node, constant))
                .collect(),
        ),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::loop_for(
            var,
            fold_expr(from, constant),
            fold_expr(to, constant),
            body.iter().map(|node| fold_node(node, constant)).collect(),
        ),
        Node::Block(nodes) => {
            Node::block(nodes.iter().map(|node| fold_node(node, constant)).collect())
        }
        Node::Return => Node::Return,
        Node::Barrier { ordering } => Node::barrier_with_ordering(*ordering),
        Node::IndirectDispatch {
            count_buffer,
            count_offset,
        } => Node::indirect_dispatch(count_buffer.clone(), *count_offset),
        Node::AsyncLoad {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::async_load_ext(
            source.clone(),
            destination.clone(),
            (**offset).clone(),
            (**size).clone(),
            tag.clone(),
        ),
        Node::AsyncStore {
            source,
            destination,
            offset,
            size,
            tag,
        } => Node::async_store(
            source.clone(),
            destination.clone(),
            (**offset).clone(),
            (**size).clone(),
            tag.clone(),
        ),
        Node::AsyncWait { tag } => Node::async_wait(tag.clone()),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: std::sync::Arc::clone(body),
        },
        Node::Trap { .. }
        | Node::Resume { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. } => node.clone(),
        Node::Opaque(ext) => Node::Opaque(ext.clone()),
    }
}

fn fold_expr(expr: &Expr, constant: &ConstBuffer) -> Expr {
    match expr {
        Expr::Load { buffer, index } if buffer == &constant.name => {
            let index = fold_expr(index, constant);
            if let Expr::LitU32(i) = index {
                if let Some(value) = constant.values.get(i as usize) {
                    return Expr::u32(*value);
                }
            }
            Expr::Load {
                buffer: buffer.clone(),
                index: Box::new(index),
            }
        }
        Expr::Load { buffer, index } => Expr::Load {
            buffer: buffer.clone(),
            index: Box::new(fold_expr(index, constant)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op: *op,
            left: Box::new(fold_expr(left, constant)),
            right: Box::new(fold_expr(right, constant)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op: op.clone(),
            operand: Box::new(fold_expr(operand, constant)),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(fold_expr(cond, constant)),
            true_val: Box::new(fold_expr(true_val, constant)),
            false_val: Box::new(fold_expr(false_val, constant)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target: target.clone(),
            value: Box::new(fold_expr(value, constant)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(fold_expr(a, constant)),
            b: Box::new(fold_expr(b, constant)),
            c: Box::new(fold_expr(c, constant)),
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op: *op,
            buffer: buffer.clone(),
            index: Box::new(fold_expr(index, constant)),
            expected: expected
                .as_ref()
                .map(|expected| Box::new(fold_expr(expected, constant))),
            value: Box::new(fold_expr(value, constant)),
            ordering: *ordering,
        },
        Expr::Call { op_id, args } => Expr::call(
            op_id,
            args.iter().map(|arg| fold_expr(arg, constant)).collect(),
        ),
        _ => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType};

    #[test]
    fn const_buffer_inlined_when_compile_time_known() {
        let program =
            crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
                vec![
                    BufferDecl::read("lut", 0, DataType::U32).with_count(256),
                    BufferDecl::output("out", 1, DataType::U32).with_count(1),
                ],
                [1, 1, 1],
                vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("lut", Expr::u32(7)),
                )],
            ));
        let result = fold_const_buffer(
            &program,
            &ConstBuffer {
                name: "lut".into(),
                values: (0..256).map(|value| value * 3).collect(),
            },
        );

        assert!(result.changed);
        let body = crate::test_util::region_body(&result.program);
        assert!(matches!(
            &body[0],
            Node::Store {
                value: Expr::LitU32(21),
                ..
            }
        ));
    }

    #[test]
    fn out_of_range_index_stays_as_load() {
        let program =
            crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
                vec![
                    BufferDecl::read("lut", 0, DataType::U32).with_count(4),
                    BufferDecl::output("out", 1, DataType::U32).with_count(1),
                ],
                [1, 1, 1],
                vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("lut", Expr::u32(999)),
                )],
            ));
        let result = fold_const_buffer(
            &program,
            &ConstBuffer {
                name: "lut".into(),
                values: vec![10, 20, 30, 40],
            },
        );
        assert!(!result.changed);
    }

    #[test]
    fn different_buffer_not_folded() {
        let program =
            crate::optimizer::passes::cleanup::region_inline_engine::run(Program::wrapped(
                vec![
                    BufferDecl::read("data", 0, DataType::U32).with_count(4),
                    BufferDecl::output("out", 1, DataType::U32).with_count(1),
                ],
                [1, 1, 1],
                vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("data", Expr::u32(0)),
                )],
            ));
        let result = fold_const_buffer(
            &program,
            &ConstBuffer {
                name: "lut".into(),
                values: vec![42],
            },
        );
        assert!(!result.changed);
    }

    #[test]
    fn const_buffer_struct_eq() {
        let a = ConstBuffer {
            name: "x".into(),
            values: vec![1, 2, 3],
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
