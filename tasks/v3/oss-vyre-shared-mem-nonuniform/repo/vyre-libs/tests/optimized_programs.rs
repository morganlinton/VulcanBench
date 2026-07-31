//! Shape tests for optimized Cat-A programs.

#[cfg(feature = "nn-attention")]
use vyre::ir::Expr;
use vyre::ir::{MemoryKind, Node};

#[cfg(feature = "nn-linear")]
#[test]
fn linear_tiled_uses_tiled_matmul_kernel_shape() {
    let program = vyre_libs::nn::linear_tiled("x", "w", "b", "out", 37, 65, 16)
        .expect("Fix: optimized linear_tiled must build for positive dimensions.");

    assert_eq!(program.workgroup_size(), [256, 1, 1]);
    assert!(
        program
            .buffers()
            .iter()
            .any(|buffer| buffer.name.as_ref() == "w" && buffer.count == 37 * 65),
        "Fix: linear_tiled must preserve the matmul weight buffer contract."
    );
    assert_region_generator(&program, "vyre-libs::nn::linear_tiled");
}

#[cfg(feature = "nn-attention")]
#[test]
fn attention_default_is_query_row_parallel() {
    let program = vyre_libs::nn::attention("q", "k", "v", "out", 8, 4);

    assert_eq!(program.workgroup_size(), [256, 1, 1]);
    let body = root_region_body(&program);
    assert!(
        body.iter().any(|node| matches!(
            node,
            Node::Let {
                name,
                value: Expr::InvocationId { axis: 0 },
            } if name.as_str() == "i"
        )),
        "Fix: attention must bind one invocation to one query row."
    );
    assert_eq!(
        count_invocation_id_lets(body, "idx"),
        0,
        "Fix: attention must not use the old one-invocation-per-output-element idx kernel."
    );
}

#[cfg(feature = "nn-attention")]
#[test]
fn softmax_default_uses_tiled_workgroup_scratch() {
    let program = vyre_libs::nn::softmax("input", "output", 513);

    assert_eq!(program.workgroup_size(), [256, 1, 1]);
    assert!(
        program.buffers().iter().any(|buffer| {
            buffer.name.as_ref() == "softmax_scratch"
                && buffer.kind == MemoryKind::Shared
                && buffer.count == 256
        }),
        "Fix: optimized softmax must keep its tiled workgroup scratch buffer."
    );
}

#[cfg(feature = "nn-norm")]
#[test]
fn rms_norm_default_uses_tiled_workgroup_scratch() {
    let program = vyre_libs::nn::rms_norm("input", "output", 777, 1.0e-5);

    assert_eq!(program.workgroup_size(), [256, 1, 1]);
    assert!(
        program.buffers().iter().any(|buffer| {
            buffer.name.as_ref() == "rms_scratch"
                && buffer.kind == MemoryKind::Shared
                && buffer.count == 256
        }),
        "Fix: optimized rms_norm must keep its tiled workgroup scratch buffer."
    );
}

fn assert_region_generator(program: &vyre::ir::Program, expected: &str) {
    match &program.entry()[0] {
        Node::Region { generator, .. } => assert_eq!(generator.as_str(), expected),
        other => panic!("Fix: expected optimized root Region, got {other:?}"),
    }
}

#[cfg(feature = "nn-attention")]
fn root_region_body(program: &vyre::ir::Program) -> &[Node] {
    match &program.entry()[0] {
        Node::Region { body, .. } => body.as_ref(),
        other => panic!("Fix: expected optimized root Region, got {other:?}"),
    }
}

#[cfg(feature = "nn-attention")]
fn count_invocation_id_lets(nodes: &[Node], name: &str) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::Let {
                name: let_name,
                value: Expr::InvocationId { .. },
            } if let_name.as_str() == name => 1,
            Node::If {
                then, otherwise, ..
            } => count_invocation_id_lets(then, name) + count_invocation_id_lets(otherwise, name),
            Node::Loop { body, .. } | Node::Block(body) => count_invocation_id_lets(body, name),
            Node::Region { body, .. } => count_invocation_id_lets(body, name),
            _ => 0,
        })
        .sum()
}
