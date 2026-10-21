use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

#[must_use]
/// Build the GPU program that compacts sparse lexer rows into dense token columns.
pub fn c11_compact_sparse_tokens(
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    inclusive_offsets: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    count: u32,
) -> Program {
    c11_compact_sparse_tokens_impl(
        sparse_types,
        sparse_starts,
        sparse_lens,
        inclusive_offsets,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        count,
        false,
    )
}

#[must_use]
/// Build the GPU program that compacts sparse lexer rows and marks dense token columns as outputs.
pub fn c11_compact_sparse_tokens_output(
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    inclusive_offsets: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    count: u32,
) -> Program {
    c11_compact_sparse_tokens_impl(
        sparse_types,
        sparse_starts,
        sparse_lens,
        inclusive_offsets,
        out_tok_types,
        out_tok_starts,
        out_tok_lens,
        out_counts,
        count,
        true,
    )
}

fn c11_compact_sparse_tokens_impl(
    sparse_types: &str,
    sparse_starts: &str,
    sparse_lens: &str,
    inclusive_offsets: &str,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
    count: u32,
    explicit_outputs: bool,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("tok_type", Expr::load(sparse_types, t.clone())),
        Node::let_bind("offset", Expr::load(inclusive_offsets, t.clone())),
        Node::if_then(
            Expr::ne(Expr::var("tok_type"), Expr::u32(0)),
            vec![
                Node::let_bind("dst", Expr::sub(Expr::var("offset"), Expr::u32(1))),
                Node::store(out_tok_types, Expr::var("dst"), Expr::var("tok_type")),
                Node::store(
                    out_tok_starts,
                    Expr::var("dst"),
                    Expr::load(sparse_starts, t.clone()),
                ),
                Node::store(
                    out_tok_lens,
                    Expr::var("dst"),
                    Expr::load(sparse_lens, t.clone()),
                ),
            ],
        ),
        Node::if_then(
            Expr::eq(
                Expr::add(t.clone(), Expr::u32(1)),
                Expr::buf_len(inclusive_offsets),
            ),
            vec![Node::store(out_counts, Expr::u32(0), Expr::var("offset"))],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(sparse_types, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(sparse_starts, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(sparse_lens, 2, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(inclusive_offsets, 3, BufferAccess::ReadOnly, DataType::U32),
            compact_output_decl(out_tok_types, 4, count.max(1), explicit_outputs),
            compact_output_decl(out_tok_starts, 5, count.max(1), explicit_outputs),
            compact_output_decl(out_tok_lens, 6, count.max(1), explicit_outputs),
            compact_output_decl_count(out_counts, 7, 1, explicit_outputs),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_compact_sparse_tokens",
            vec![Node::if_then(
                Expr::lt(t, Expr::buf_len(inclusive_offsets)),
                body,
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_compact_sparse_tokens")
    .with_non_composable_with_self(true)
}

fn compact_output_decl(name: &str, binding: u32, count: u32, explicit_output: bool) -> BufferDecl {
    let access = if explicit_output {
        BufferAccess::WriteOnly
    } else {
        BufferAccess::ReadWrite
    };
    BufferDecl::storage(name, binding, access, DataType::U32)
        .with_count(count)
        .with_pipeline_live_out(explicit_output)
}

fn compact_output_decl_count(
    name: &str,
    binding: u32,
    count: u32,
    explicit_output: bool,
) -> BufferDecl {
    let access = if explicit_output {
        BufferAccess::WriteOnly
    } else {
        BufferAccess::ReadWrite
    };
    BufferDecl::storage(name, binding, access, DataType::U32)
        .with_count(count)
        .with_pipeline_live_out(explicit_output)
}

#[cfg(test)]
mod tests {
    use super::{c11_compact_sparse_tokens, c11_compact_sparse_tokens_output};
    use vyre::ir::BufferAccess;

    #[test]
    fn compact_output_variant_marks_final_streams_live_out_without_result_outputs() {
        let program = c11_compact_sparse_tokens_output(
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "offsets",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            64,
        );
        let result_output_names = program
            .buffers()
            .iter()
            .filter(|buffer| buffer.is_output())
            .map(|buffer| buffer.name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            result_output_names,
            Vec::<String>::new(),
            "compact emits multiple live buffers, so it must not use BufferDecl::output, which is reserved for a single result buffer"
        );
        let live_out_names = program
            .buffers()
            .iter()
            .filter(|buffer| buffer.is_pipeline_live_out())
            .map(|buffer| buffer.name().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            live_out_names,
            vec![
                "out_tok_types".to_string(),
                "out_tok_starts".to_string(),
                "out_tok_lens".to_string(),
                "out_counts".to_string(),
            ]
        );
        assert!(
            program
                .buffers()
                .iter()
                .filter(|buffer| buffer.is_pipeline_live_out())
                .all(|buffer| buffer.access() == BufferAccess::WriteOnly),
            "explicit compact live-out buffers are written from scratch and must not require host input bytes"
        );
    }

    #[test]
    fn compact_default_variant_preserves_readwrite_contract() {
        let program = c11_compact_sparse_tokens(
            "sparse_types",
            "sparse_starts",
            "sparse_lens",
            "offsets",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            64,
        );
        assert!(
            program.buffers().iter().all(|buffer| !buffer.is_output()),
            "default compact builder must preserve the historical read-write buffer contract"
        );
    }
}
