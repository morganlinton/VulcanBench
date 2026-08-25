use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

/// Wrap a Tier-2.5 bitset primitive as a Tier-3 logical composition.
#[must_use]
pub(crate) fn wrap_bitset_binary(
    op_id: &'static str,
    primitive_op_id: &'static str,
    a: &str,
    b: &str,
    out: &str,
    size: u32,
    primitive: Program,
) -> Program {
    let parent = GeneratorRef {
        name: op_id.to_string(),
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(size),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32).with_count(size),
            BufferDecl::output(out, 2, DataType::U32).with_count(size),
        ],
        primitive.workgroup_size(),
        vec![crate::region::wrap_anonymous(
            op_id,
            vec![crate::region::wrap_child(
                primitive_op_id,
                parent,
                primitive.into_entry_vec(),
            )],
        )],
    )
}

/// Build a Tier-3 elementwise u32 logical binary op.
#[must_use]
pub(crate) fn build_logical_binary<F>(
    op_id: &'static str,
    a: &str,
    b: &str,
    out: &str,
    size: u32,
    op: F,
) -> Program
where
    F: Fn(Expr, Expr) -> Expr,
{
    crate::math::elementwise::u32_elementwise_binary(op_id, a, b, out, size, op)
}
