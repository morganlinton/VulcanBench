use super::*;
use vyre::ir::BinOp;
use vyre_driver_cuda::cuda_factory;
use vyre_libs::parsing::c::lex::tokens::TOK_IDENTIFIER;
use vyre_reference::value::Value;

const GENERATED_BRACKET_CASES: u32 = 10_000;

fn bracket_pairs_gpu_reference(tokens: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if tokens.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let n = tokens.len() as u32;
    let program = c11_dual_bracket_match("tok_types", "paren_pairs", "brace_pairs", n);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(super::super::buffers::vec_u32_le_bytes(tokens))],
    )
    .expect("Fix: GPU bracket-pair program must execute under reference evaluator");
    assert_eq!(outputs.len(), 2);
    (
        read_u32_stream(&outputs[0].to_bytes(), tokens.len(), "paren test pairs")
            .expect("Fix: paren pairs must decode"),
        read_u32_stream(&outputs[1].to_bytes(), tokens.len(), "brace test pairs")
            .expect("Fix: brace pairs must decode"),
    )
}

fn bracket_pairs_cpu_oracle(tokens: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut paren = vec![MATCH_NONE; tokens.len()];
    let mut brace = vec![MATCH_NONE; tokens.len()];
    fill_pair_oracle(tokens, &mut paren, TOK_LPAREN, TOK_RPAREN);
    fill_pair_oracle(tokens, &mut brace, TOK_LBRACE, TOK_RBRACE);
    (paren, brace)
}

fn fill_pair_oracle(tokens: &[u32], pairs: &mut [u32], open_tok: u32, close_tok: u32) {
    let mut stack = Vec::new();
    for (idx, tok) in tokens.iter().copied().enumerate() {
        if tok == open_tok {
            stack.push(idx as u32);
        } else if tok == close_tok {
            if let Some(open_idx) = stack.pop() {
                pairs[open_idx as usize] = idx as u32;
                pairs[idx] = open_idx;
            }
        }
    }
}

fn next_u32(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(0xD134_2543_DE82_EF95)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    ((*state >> 32) as u32) ^ (*state as u32)
}

fn generated_bracket_case(case: u32) -> Vec<u32> {
    let mut state = u64::from(case) ^ 0xA076_1D64_78BD_642F;
    let len = (next_u32(&mut state) as usize) % 65;
    match case % 8 {
        0 => nested_case(TOK_LPAREN, TOK_RPAREN, len),
        1 => nested_case(TOK_LBRACE, TOK_RBRACE, len),
        2 => crossing_mixed_case(len),
        3 => close_heavy_case(len),
        4 => open_heavy_case(len),
        5 => chunked_balanced_case(len),
        _ => random_soup_case(&mut state, len),
    }
}

fn nested_case(open_tok: u32, close_tok: u32, len: usize) -> Vec<u32> {
    let depth = len / 2;
    let mut tokens = Vec::with_capacity(len);
    tokens.extend(std::iter::repeat(open_tok).take(depth));
    if len % 2 == 1 {
        tokens.push(TOK_IDENTIFIER);
    }
    tokens.extend(std::iter::repeat(close_tok).take(depth));
    tokens
}

fn crossing_mixed_case(len: usize) -> Vec<u32> {
    let pattern = [
        TOK_LPAREN,
        TOK_LBRACE,
        TOK_RPAREN,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_LPAREN,
        TOK_RBRACE,
        TOK_RPAREN,
    ];
    (0..len).map(|idx| pattern[idx % pattern.len()]).collect()
}

fn close_heavy_case(len: usize) -> Vec<u32> {
    let pattern = [
        TOK_RPAREN,
        TOK_RBRACE,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    (0..len).map(|idx| pattern[idx % pattern.len()]).collect()
}

fn open_heavy_case(len: usize) -> Vec<u32> {
    let pattern = [
        TOK_LPAREN,
        TOK_LBRACE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_LBRACE,
        TOK_RPAREN,
    ];
    (0..len).map(|idx| pattern[idx % pattern.len()]).collect()
}

fn chunked_balanced_case(len: usize) -> Vec<u32> {
    let pattern = [
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_RBRACE,
    ];
    (0..len).map(|idx| pattern[idx % pattern.len()]).collect()
}

fn random_soup_case(state: &mut u64, len: usize) -> Vec<u32> {
    (0..len)
        .map(|_| match next_u32(state) % 96 {
            0..=15 => TOK_LPAREN,
            16..=31 => TOK_RPAREN,
            32..=47 => TOK_LBRACE,
            48..=63 => TOK_RBRACE,
            _ => TOK_IDENTIFIER,
        })
        .collect()
}

fn expr_has_invocation_zero_eq(expr: &Expr) -> bool {
    match expr {
        Expr::BinOp { op, left, right } => {
            (*op == BinOp::Eq && is_invocation_x_zero_pair(left, right))
                || expr_has_invocation_zero_eq(left)
                || expr_has_invocation_zero_eq(right)
        }
        Expr::Load { index, .. } => expr_has_invocation_zero_eq(index),
        Expr::UnOp { operand, .. } => expr_has_invocation_zero_eq(operand),
        Expr::Call { args, .. } => args.iter().any(expr_has_invocation_zero_eq),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_has_invocation_zero_eq(cond)
                || expr_has_invocation_zero_eq(true_val)
                || expr_has_invocation_zero_eq(false_val)
        }
        Expr::Cast { value, .. } => expr_has_invocation_zero_eq(value),
        Expr::Fma { a, b, c } => {
            expr_has_invocation_zero_eq(a)
                || expr_has_invocation_zero_eq(b)
                || expr_has_invocation_zero_eq(c)
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_has_invocation_zero_eq(index)
                || expected
                    .as_deref()
                    .map(expr_has_invocation_zero_eq)
                    .unwrap_or(false)
                || expr_has_invocation_zero_eq(value)
        }
        Expr::SubgroupBallot { cond } => expr_has_invocation_zero_eq(cond),
        Expr::SubgroupShuffle { value, lane } => {
            expr_has_invocation_zero_eq(value) || expr_has_invocation_zero_eq(lane)
        }
        Expr::SubgroupReduce { value, .. } => expr_has_invocation_zero_eq(value),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => false,
        _ => false,
    }
}

fn is_invocation_x_zero_pair(left: &Expr, right: &Expr) -> bool {
    (matches!(left, Expr::InvocationId { axis: 0 }) && matches!(right, Expr::LitU32(0)))
        || (matches!(right, Expr::InvocationId { axis: 0 }) && matches!(left, Expr::LitU32(0)))
}

fn nodes_have_single_lane_guard(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => expr_has_invocation_zero_eq(value),
        Node::Store { index, value, .. } => {
            expr_has_invocation_zero_eq(index) || expr_has_invocation_zero_eq(value)
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            expr_has_invocation_zero_eq(cond)
                || nodes_have_single_lane_guard(then)
                || nodes_have_single_lane_guard(otherwise)
        }
        Node::Loop { from, to, body, .. } => {
            expr_has_invocation_zero_eq(from)
                || expr_has_invocation_zero_eq(to)
                || nodes_have_single_lane_guard(body)
        }
        Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
            expr_has_invocation_zero_eq(offset) || expr_has_invocation_zero_eq(size)
        }
        Node::Trap { address, .. } => expr_has_invocation_zero_eq(address),
        Node::Block(body) => nodes_have_single_lane_guard(body),
        Node::Region { body, .. } => nodes_have_single_lane_guard(body.as_ref()),
        Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::AllReduce { .. }
        | Node::AllGather { .. }
        | Node::ReduceScatter { .. }
        | Node::Broadcast { .. }
        | Node::Return
        | Node::Barrier { .. }
        | Node::Opaque(_) => false,
        _ => false,
    })
}

#[test]
fn c11_bracket_program_is_lane_parallel_without_workgroup_stacks() {
    let program = c11_dual_bracket_match("tok_types", "paren_pairs", "brace_pairs", 513);
    assert_eq!(program.workgroup_size(), [BRACKET_MATCH_WORKGROUP, 1, 1]);
    assert_eq!(program.buffers().len(), 3);
    assert!(
        program
            .buffers()
            .iter()
            .all(|decl| decl.access != BufferAccess::Workgroup),
        "Fix: bracket pairing must not allocate a fixed-depth workgroup stack."
    );
    let paren = program
        .buffer("paren_pairs")
        .expect("Fix: paren output buffer must be declared");
    assert_eq!(paren.binding, 1);
    assert!(paren.is_output);
    assert!(paren.pipeline_live_out);
    let brace = program
        .buffer("brace_pairs")
        .expect("Fix: brace output buffer must be declared");
    assert_eq!(brace.binding, 2);
    assert!(brace.pipeline_live_out);
    assert!(
        !nodes_have_single_lane_guard(program.entry()),
        "Fix: bracket pairing must dispatch across token lanes, not gate all work on global invocation 0."
    );
}

#[test]
fn generated_bracket_pairs_match_independent_cpu_oracle_for_10000_cases() {
    for case in 0..GENERATED_BRACKET_CASES {
        let tokens = generated_bracket_case(case);
        let expected = bracket_pairs_cpu_oracle(&tokens);
        let actual = bracket_pairs_gpu_reference(&tokens);
        assert_eq!(
            actual.0, expected.0,
            "Fix: paren pair mismatch in generated bracket case {case}: {tokens:?}"
        );
        assert_eq!(
            actual.1, expected.1,
            "Fix: brace pair mismatch in generated bracket case {case}: {tokens:?}"
        );
    }
}

#[test]
fn matches_balanced_parens_and_braces() {
    let tokens = vec![
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_IDENTIFIER,
        TOK_RBRACE,
    ];
    let (paren, brace) = bracket_pairs_gpu_reference(&tokens);
    assert_eq!(paren[2], 4);
    assert_eq!(paren[4], 2);
    assert_eq!(brace[5], 9);
    assert_eq!(brace[9], 5);
    assert_eq!(paren[0], MATCH_NONE);
    assert_eq!(brace[2], MATCH_NONE);
}

#[test]
fn long_non_delimiter_stream_stays_unmatched() {
    let tokens = vec![TOK_IDENTIFIER; 4097];
    let (paren, brace) = bracket_pairs_gpu_reference(&tokens);
    assert_eq!(paren.len(), tokens.len());
    assert_eq!(brace.len(), tokens.len());
    assert!(paren.iter().all(|word| *word == MATCH_NONE));
    assert!(brace.iter().all(|word| *word == MATCH_NONE));
}

#[test]
fn cuda_nested_parens_cross_legacy_fixed_depth_cap() {
    let depth = 4097;
    let mut tokens = Vec::with_capacity(depth * 2);
    tokens.extend(std::iter::repeat(TOK_LPAREN).take(depth));
    tokens.extend(std::iter::repeat(TOK_RPAREN).take(depth));
    let expected = bracket_pairs_cpu_oracle(&tokens);
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let actual = dispatch_c11_bracket_pairs(
        backend.as_ref(),
        &tokens,
        "cuda nested C11 bracket depth regression",
    )
    .expect("Fix: CUDA bracket-pair dispatch must handle nesting beyond the old stack cap.");
    assert_eq!(actual, expected);
    assert_eq!(actual.0[0], (depth * 2 - 1) as u32);
    assert_eq!(actual.0[depth - 1], depth as u32);
}

#[test]
fn nested_brackets_pair_innermost_first() {
    let tokens = vec![TOK_LPAREN, TOK_LPAREN, TOK_RPAREN, TOK_RPAREN];
    let (paren, _) = bracket_pairs_gpu_reference(&tokens);
    assert_eq!(paren[1], 2);
    assert_eq!(paren[2], 1);
    assert_eq!(paren[0], 3);
    assert_eq!(paren[3], 0);
}

#[test]
fn unmatched_close_is_ignored_not_panic() {
    let tokens = vec![TOK_RPAREN, TOK_LPAREN, TOK_RPAREN];
    let (paren, _) = bracket_pairs_gpu_reference(&tokens);
    assert_eq!(paren[0], MATCH_NONE);
    assert_eq!(paren[1], 2);
    assert_eq!(paren[2], 1);
}

#[test]
fn empty_token_stream_returns_empty_pairs() {
    let (paren, brace) = bracket_pairs_gpu_reference(&[]);
    assert!(paren.is_empty());
    assert!(brace.is_empty());
}
