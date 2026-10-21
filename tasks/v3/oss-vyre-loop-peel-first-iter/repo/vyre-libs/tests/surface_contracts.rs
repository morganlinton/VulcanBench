//! Surface-presence tests for vyre-libs items that graduated from the
//! gap ledger.
//! Each `#[test]` asserts the named function exists; the test fails at
//! compile time if a migrated item disappears.

#![allow(deprecated)]
#![cfg(all(
    feature = "math-linalg",
    feature = "nn-norm",
    feature = "nn-attention",
    feature = "matching-substring",
    feature = "matching-dfa",
    feature = "crypto-blake3",
))]

// Gaps L-1..L-6 and L-8 CLOSED  -  the Cat-A compositions ship. These
// tests now assert their presence as structural smoke tests. Byte-level
// oracle checks live in `tests/cat_a_conform.rs`.

#[test]
fn contract_nn_softmax_exists() {
    use vyre_libs::nn::softmax;
    let p = softmax("x", "y", 64);
    assert_eq!(p.buffers().len(), 4);
}

#[test]
fn contract_nn_layer_norm_exists() {
    use vyre_libs::nn::layer_norm;
    let p = layer_norm("x", "out", 64, 1e-5);
    assert_eq!(p.buffers().len(), 2);
}

#[test]
fn contract_nn_attention_exists() {
    use vyre_libs::nn::attention;
    let p = attention("q", "k", "v", "out", 64, 8);
    assert_eq!(p.buffers().len(), 4);
}

#[test]
fn contract_matching_dfa_compile_exists() {
    use vyre_libs::scan::dfa_compile;
    let dfa = dfa_compile(&[b"foo", b"bar"]);
    assert!(dfa.state_count >= 1);
    assert_eq!(dfa.transitions.len(), (dfa.state_count as usize) * 256);
}

#[test]
fn contract_matching_aho_corasick_exists() {
    use vyre_libs::scan::aho_corasick;
    let p = aho_corasick("haystack", "transitions", "accept", "matches", 16, 8);
    assert_eq!(p.buffers().len(), 4);
}

#[test]
fn contract_crypto_blake3_exists() {
    use vyre_libs::hash::blake3_compress;
    let p = blake3_compress("chaining_in", "message", "params", "chaining_out");
    assert_eq!(p.buffers().len(), 4);
}

// Substring search: substring_search performs a real byte-by-byte
// equality via `load(haystack, i+k) == load(needle, k)`. A regressed
// constant-true predicate would make every byte position match; this
// test combines (a) a structural assertion that the Load/Load Eq
// pair exists in the inner k-loop with (b) an end-to-end execution
// through the reference interpreter to prove the bitmap is correct
// on a canonical input. Region execution is wired into the
// interpreter so the full path is exercised.
#[test]
fn contract_substring_real_byte_compare() {
    use vyre::ir::{Expr, Node};
    use vyre_libs::scan::substring_search;

    let program = substring_search("haystack", "needle", "matches", 5, 2);

    fn contains_load_load_eq(nodes: &[Node]) -> bool {
        nodes.iter().any(node_contains)
    }
    fn node_contains(node: &Node) -> bool {
        match node {
            Node::Block(children) | Node::Loop { body: children, .. } => {
                contains_load_load_eq(children)
            }
            Node::If {
                then,
                otherwise,
                cond,
            } => {
                expr_contains(cond)
                    || contains_load_load_eq(then)
                    || contains_load_load_eq(otherwise)
            }
            Node::Let { value, .. } | Node::Assign { value, .. } => expr_contains(value),
            Node::Region { body, .. } => contains_load_load_eq(body),
            _ => false,
        }
    }
    fn expr_contains(expr: &Expr) -> bool {
        use Expr::*;
        match expr {
            BinOp { op, left, right } => {
                matches!(op, vyre::ir::BinOp::Eq)
                    && matches!(left.as_ref(), Load { .. })
                    && matches!(right.as_ref(), Load { .. })
                    || expr_contains(left)
                    || expr_contains(right)
            }
            Select {
                cond,
                true_val,
                false_val,
            } => expr_contains(cond) || expr_contains(true_val) || expr_contains(false_val),
            _ => false,
        }
    }

    assert!(
        contains_load_load_eq(program.entry()),
        "substring_search must contain a Load-vs-Load equality inside its k-loop; \
         if this regresses, the inner compare has been replaced with a constant predicate (LAW 1)"
    );

    // End-to-end execution: "hello" contains "lo" at byte offset 3.
    use vyre_reference::value::Value;
    let haystack_bytes: Vec<u8> = "hello"
        .bytes()
        .flat_map(|b| u32::from(b).to_le_bytes())
        .chain(std::iter::repeat(0u8).take(12))
        .collect();
    let needle_bytes: Vec<u8> = "lo"
        .bytes()
        .flat_map(|b| u32::from(b).to_le_bytes())
        .collect();
    let matches_bytes = vec![0u8; 5 * 4];
    let inputs = [
        Value::from(haystack_bytes),
        Value::from(needle_bytes),
        Value::from(matches_bytes),
    ];
    let outputs =
        vyre_reference::reference_eval(&program, &inputs).expect("execute substring_search");
    assert_eq!(outputs.len(), 1, "only matches buffer is ReadWrite");
    let raw = outputs[0].to_bytes();
    let words: Vec<u32> = raw
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(words.len(), 5, "matches buffer has 5 u32 slots");
    assert_eq!(words[3], 1, "match at byte offset 3");
    for (i, w) in words.iter().enumerate() {
        if i == 3 {
            continue;
        }
        assert_eq!(*w, 0, "no match at byte offset {i}, got {w}");
    }
}

#[test]
fn contract_math_matmul_tiled_exists() {
    use vyre_libs::math::matmul_tiled;
    let p = matmul_tiled("a", "b", "c", 64, 64, 64, 16);
    assert_eq!(p.buffers().len(), 3);
    // matmul_tiled flattens the 2D workgroup into a 1D dispatch so that
    // InvocationId { axis: 0 } is a unique linear index.
    assert_eq!(p.workgroup_size(), [256, 1, 1]);
}
