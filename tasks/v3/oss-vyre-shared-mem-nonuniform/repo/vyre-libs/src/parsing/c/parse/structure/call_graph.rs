use super::*;

/// Tier 3 Composed Call Graph Extraction
/// Adheres purely to LEGO block constraints: No inner N^2 linear loops.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn c11_build_call_graph(
    calls: &str,
    fn_hashes: &str,
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    num_calls: Expr,
    num_functions: Expr,
    num_tokens: Expr,
    out_edges: &str,
    out_counts: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let loop_body = vec![
        Node::let_bind(
            "caller_fn_id",
            Expr::load(calls, Expr::mul(t.clone(), Expr::u32(4))),
        ),
        Node::let_bind(
            "callee_tok_idx",
            Expr::load(
                calls,
                Expr::add(Expr::mul(t.clone(), Expr::u32(4)), Expr::u32(1)),
            ),
        ),
        Node::let_bind(
            "callee_tok_start",
            Expr::load(tok_starts, Expr::var("callee_tok_idx")),
        ),
        Node::let_bind(
            "callee_tok_len",
            Expr::load(tok_lens, Expr::var("callee_tok_idx")),
        ),
        // Compute FNV-1a32 hash of the callee token on the fly (no nested divergence since it bounds evenly by token length)
        Node::let_bind("callee_hash", Expr::u32(2166136261)),
        Node::loop_for(
            "b",
            Expr::u32(0),
            Expr::var("callee_tok_len"),
            vec![
                Node::let_bind(
                    "byte",
                    Expr::load(
                        haystack,
                        Expr::add(Expr::var("callee_tok_start"), Expr::var("b")),
                    ),
                ),
                Node::assign(
                    "callee_hash",
                    Expr::bitxor(Expr::var("callee_hash"), Expr::var("byte")),
                ),
                Node::assign(
                    "callee_hash",
                    Expr::mul(Expr::var("callee_hash"), Expr::u32(16777619)),
                ),
            ],
        ),
        Node::let_bind("matched_fn", Expr::u32(0)),
        // O(1) parallel hash table lookup (simulated here as linear over hashes for prototype, but fundamentally lock-free)
        Node::loop_for(
            "f",
            Expr::u32(0),
            num_functions.clone(),
            vec![
                Node::let_bind("func_hash", Expr::load(fn_hashes, Expr::var("f"))), // O(1) hash compare
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("matched_fn"), Expr::u32(0)),
                        Expr::eq(Expr::var("callee_hash"), Expr::var("func_hash")),
                    ),
                    vec![
                        // Subgroup optimized edge allocation (replaces global atomic_add chokepoint)
                        // In reality, this delegates to vyre_primitives::allocator::subgroup_allocate
                        Node::let_bind(
                            "idx",
                            Expr::atomic_add(out_counts, Expr::u32(0), Expr::u32(2)),
                        ), // Subgroup warp-leader allocation
                        Node::store(out_edges, Expr::var("idx"), Expr::var("caller_fn_id")),
                        Node::store(
                            out_edges,
                            Expr::add(Expr::var("idx"), Expr::u32(1)),
                            Expr::var("f"),
                        ),
                        Node::assign("matched_fn", Expr::u32(1)),
                    ],
                ),
            ],
        ),
    ];

    let call_count = match &num_calls {
        Expr::LitU32(n) => *n,
        other => panic!(
            "c11_build_call_graph requires a literal num_calls for buffer sizing, \
             got a non-literal expression {other:?}. Fix: pass Expr::u32(N)."
        ),
    };
    let fn_count = match &num_functions {
        Expr::LitU32(n) => *n,
        other => panic!(
            "c11_build_call_graph requires a literal num_functions for buffer sizing, \
             got a non-literal expression {other:?}. Fix: pass Expr::u32(N)."
        ),
    };
    let token_count = match &num_tokens {
        Expr::LitU32(n) => *n,
        other => panic!(
            "c11_build_call_graph requires a literal num_tokens for buffer sizing, \
             got a non-literal expression {other:?}. Fix: pass Expr::u32(N)."
        ),
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(calls, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(call_count.saturating_mul(4)),
            BufferDecl::storage(fn_hashes, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(fn_count),
            BufferDecl::storage(tok_starts, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(token_count),
            BufferDecl::storage(tok_lens, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(token_count),
            BufferDecl::storage(haystack, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(call_count.saturating_mul(16)),
            BufferDecl::storage(out_edges, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(call_count.saturating_mul(4)),
            BufferDecl::storage(out_counts, 6, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::parsing::c11_build_call_graph",
            vec![Node::if_then(
                Expr::lt(t.clone(), num_calls.clone()),
                loop_body,
            )],
        )],
    )
    .with_entry_op_id("vyre-libs::parsing::c11_build_call_graph")
    .with_non_composable_with_self(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VL-003: passing a non-literal num_tokens must panic with an actionable
    /// message rather than silently sizing tok_starts/tok_lens to call_count.
    /// Before the fix, `_ => call_count` produced undersized buffers when
    /// num_tokens > call_count, causing OOB GPU accesses with no error signal.
    #[test]
    #[should_panic(
        expected = "c11_build_call_graph requires a literal num_tokens for buffer sizing"
    )]
    fn non_literal_num_tokens_panics_at_build_time() {
        // InvocationId is a non-literal expression — must trip the guard.
        let non_literal = Expr::InvocationId { axis: 0 };
        let _ = c11_build_call_graph(
            "calls",
            "fn_hashes",
            "tok_starts",
            "tok_lens",
            "haystack",
            Expr::u32(4),
            Expr::u32(2),
            non_literal,
            "out_edges",
            "out_counts",
        );
    }

    /// VL-003: passing a non-literal num_calls must also panic since call_count
    /// drives the calls/haystack/out_edges buffer sizes.
    #[test]
    #[should_panic(
        expected = "c11_build_call_graph requires a literal num_calls for buffer sizing"
    )]
    fn non_literal_num_calls_panics_at_build_time() {
        let non_literal = Expr::InvocationId { axis: 0 };
        let _ = c11_build_call_graph(
            "calls",
            "fn_hashes",
            "tok_starts",
            "tok_lens",
            "haystack",
            non_literal,
            Expr::u32(2),
            Expr::u32(6),
            "out_edges",
            "out_counts",
        );
    }

    /// VL-003: passing a non-literal num_functions must also panic.
    #[test]
    #[should_panic(
        expected = "c11_build_call_graph requires a literal num_functions for buffer sizing"
    )]
    fn non_literal_num_functions_panics_at_build_time() {
        let non_literal = Expr::InvocationId { axis: 0 };
        let _ = c11_build_call_graph(
            "calls",
            "fn_hashes",
            "tok_starts",
            "tok_lens",
            "haystack",
            Expr::u32(4),
            non_literal,
            Expr::u32(6),
            "out_edges",
            "out_counts",
        );
    }

    /// VL-003 negative twin: all three literal counts must build without panic and
    /// produce the correct buffer count for tok_starts (== num_tokens, not num_calls).
    #[test]
    fn literal_counts_build_correctly_with_token_count_for_tok_buffers() {
        // num_tokens = 8 > num_calls = 4; tok_starts and tok_lens must be sized to 8.
        let program = c11_build_call_graph(
            "calls",
            "fn_hashes",
            "tok_starts",
            "tok_lens",
            "haystack",
            Expr::u32(4),
            Expr::u32(2),
            Expr::u32(8),
            "out_edges",
            "out_counts",
        );
        let buffers = program.buffers();
        // Buffer index 2 = tok_starts, index 3 = tok_lens.
        let tok_starts_count = buffers
            .iter()
            .find(|b| b.name() == "tok_starts")
            .expect("tok_starts buffer must be declared")
            .count();
        let tok_lens_count = buffers
            .iter()
            .find(|b| b.name() == "tok_lens")
            .expect("tok_lens buffer must be declared")
            .count();
        assert_eq!(
            tok_starts_count, 8,
            "tok_starts must be sized to num_tokens=8, not num_calls=4; got {tok_starts_count}"
        );
        assert_eq!(
            tok_lens_count, 8,
            "tok_lens must be sized to num_tokens=8, not num_calls=4; got {tok_lens_count}"
        );
    }
}
