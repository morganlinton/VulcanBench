//! Cross-entropy loss: `loss[t] = -log(softmax(logits[t])[target[t]])`.
//!
//! Category A composition. One workgroup owns one token row and cooperatively
//! reduces the vocabulary dimension with log-sum-exp stabilization.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_primitives::reduce::workgroup_tree::{self, WorkgroupReductionScope};

use crate::region::wrap_anonymous;
use crate::tensor_ref::TensorRefError;

const OP_ID: &str = "vyre-libs::nn::cross_entropy";
const CROSS_ENTROPY_TILE: u32 = 256;

/// Build a Program computing per-token cross-entropy loss.
///
/// `logits[n * vocab_size]` (F32), `targets[n]` (U32 token indices),
/// `loss_out[n]` (F32).
///
/// For each token `t`:
///   `target_logit = logits[t * vocab + targets[t]]`
///   `lse = log(sum_v(exp(logits[t * vocab + v] - max_logit)))`
///   `loss[t] = -target_logit + max_logit + lse`
#[must_use]
pub fn cross_entropy(
    logits: &str,
    targets: &str,
    loss_out: &str,
    n: u32,
    vocab_size: u32,
) -> Program {
    try_cross_entropy(logits, targets, loss_out, n, vocab_size).unwrap_or_else(|error| {
        crate::builder::invalid_output_program(
            OP_ID,
            loss_out,
            DataType::F32,
            format!("Fix: cross_entropy build failed: {error}"),
        )
    })
}

/// Fallible cross-entropy builder.
///
/// # Errors
///
/// Returns [`TensorRefError`] when dimensions are zero or buffer counts
/// overflow `u32`.
pub fn try_cross_entropy(
    logits: &str,
    targets: &str,
    loss_out: &str,
    n: u32,
    vocab_size: u32,
) -> Result<Program, TensorRefError> {
    if n == 0 || vocab_size == 0 {
        return Err(TensorRefError::ShapeMismatch {
            name: logits.to_string(),
            found: vec![n, vocab_size],
            expected: vec![1, vocab_size.max(1)],
            op: OP_ID,
        });
    }
    let logits_count =
        n.checked_mul(vocab_size)
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: logits.to_string(),
                shape: vec![n, vocab_size],
            })?;
    let padded_output_count =
        n.checked_mul(CROSS_ENTROPY_TILE)
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: loss_out.to_string(),
                shape: vec![n, CROSS_ENTROPY_TILE],
            })?;

    let body = cross_entropy_body(logits, targets, loss_out, n, vocab_size);

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(logits, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(logits_count),
            BufferDecl::storage(targets, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::workgroup("ce_scratch", CROSS_ENTROPY_TILE, DataType::F32),
            BufferDecl::workgroup("ce_max_logit", 1, DataType::F32),
            BufferDecl::workgroup("ce_target_logit", 1, DataType::F32),
            BufferDecl::output(loss_out, 2, DataType::F32)
                .with_count(padded_output_count)
                .with_output_byte_range(0..((n as usize) * core::mem::size_of::<f32>())),
        ],
        [CROSS_ENTROPY_TILE, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

fn cross_entropy_body(
    logits: &str,
    targets: &str,
    loss_out: &str,
    n: u32,
    vocab_size: u32,
) -> Vec<Node> {
    let tile = CROSS_ENTROPY_TILE;
    let chunks = vocab_size.div_ceil(tile);
    let local = Expr::var("local");
    let token = Expr::var("token");
    let vocab_idx = Expr::var("vocab_idx");
    let base = Expr::var("base");
    let max_logit = Expr::load("ce_max_logit", Expr::u32(0));
    let sum_exp = Expr::load("ce_scratch", Expr::u32(0));

    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        Node::let_bind("token", Expr::WorkgroupId { axis: 0 }),
        Node::let_bind("base", Expr::mul(token.clone(), Expr::u32(vocab_size))),
        Node::if_then(
            Expr::lt(token.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("local_max", Expr::f32(f32::MIN)),
                Node::loop_for(
                    "chunk",
                    Expr::u32(0),
                    Expr::u32(chunks),
                    vec![
                        Node::let_bind(
                            "vocab_idx",
                            Expr::add(
                                Expr::mul(Expr::var("chunk"), Expr::u32(tile)),
                                local.clone(),
                            ),
                        ),
                        Node::if_then(
                            Expr::lt(vocab_idx.clone(), Expr::u32(vocab_size)),
                            vec![Node::assign(
                                "local_max",
                                Expr::max(
                                    Expr::var("local_max"),
                                    Expr::load(logits, Expr::add(base.clone(), vocab_idx.clone())),
                                ),
                            )],
                        ),
                    ],
                ),
                Node::Store {
                    buffer: "ce_scratch".into(),
                    index: local.clone(),
                    value: Expr::var("local_max"),
                },
            ],
        ),
        Node::barrier(),
    ];
    body.push(workgroup_tree::max_f32_child(
        OP_ID,
        tile,
        "ce_scratch",
        WorkgroupReductionScope::EveryWorkgroup,
    ));
    body.extend(vec![
        Node::if_then(
            Expr::and(
                Expr::lt(token.clone(), Expr::u32(n)),
                Expr::eq(local.clone(), Expr::u32(0)),
            ),
            vec![Node::Store {
                buffer: "ce_max_logit".into(),
                index: Expr::u32(0),
                value: Expr::load("ce_scratch", Expr::u32(0)),
            }],
        ),
        Node::if_then(
            Expr::and(
                Expr::lt(token.clone(), Expr::u32(n)),
                Expr::eq(local.clone(), Expr::u32(0)),
            ),
            vec![Node::Store {
                buffer: "ce_target_logit".into(),
                index: Expr::u32(0),
                value: Expr::load(
                    logits,
                    Expr::add(base.clone(), Expr::load(targets, token.clone())),
                ),
            }],
        ),
        Node::barrier(),
        Node::if_then(
            Expr::lt(token.clone(), Expr::u32(n)),
            vec![
                Node::let_bind("max_logit", max_logit),
                Node::let_bind("local_sum", Expr::f32(0.0)),
                Node::loop_for(
                    "chunk",
                    Expr::u32(0),
                    Expr::u32(chunks),
                    vec![
                        Node::let_bind(
                            "vocab_idx",
                            Expr::add(
                                Expr::mul(Expr::var("chunk"), Expr::u32(tile)),
                                local.clone(),
                            ),
                        ),
                        Node::if_then(
                            Expr::lt(vocab_idx, Expr::u32(vocab_size)),
                            vec![Node::assign(
                                "local_sum",
                                Expr::add(
                                    Expr::var("local_sum"),
                                    Expr::UnOp {
                                        op: UnOp::Exp,
                                        operand: Box::new(Expr::sub(
                                            Expr::load(
                                                logits,
                                                Expr::add(base, Expr::var("vocab_idx")),
                                            ),
                                            Expr::var("max_logit"),
                                        )),
                                    },
                                ),
                            )],
                        ),
                    ],
                ),
                Node::Store {
                    buffer: "ce_scratch".into(),
                    index: local.clone(),
                    value: Expr::var("local_sum"),
                },
            ],
        ),
        Node::barrier(),
    ]);
    body.push(workgroup_tree::sum_f32_child(
        OP_ID,
        tile,
        "ce_scratch",
        WorkgroupReductionScope::EveryWorkgroup,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::lt(token.clone(), Expr::u32(n)),
            Expr::eq(local, Expr::u32(0)),
        ),
        vec![Node::Store {
            buffer: loss_out.into(),
            index: token,
            value: Expr::sub(
                Expr::add(
                    Expr::load("ce_max_logit", Expr::u32(0)),
                    Expr::UnOp {
                        op: UnOp::Log,
                        operand: Box::new(sum_exp),
                    },
                ),
                Expr::load("ce_target_logit", Expr::u32(0)),
            ),
        }],
    ));
    body
}

fn reference_cross_entropy_bytes(logits: &[f32], targets: &[u32], vocab_size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(targets.len() * core::mem::size_of::<f32>());
    for (token, &target) in targets.iter().enumerate() {
        let row = &logits[token * vocab_size..(token + 1) * vocab_size];
        let max_logit = row
            .iter()
            .copied()
            .fold(f32::MIN, |acc, value| acc.max(value));
        let sum = row
            .iter()
            .copied()
            .map(|value| libm::expf(value - max_logit))
            .sum::<f32>();
        let target_logit = row.get(target as usize).copied().unwrap_or(0.0);
        let loss = max_logit + libm::logf(sum) - target_logit;
        vyre_primitives::wire::append_f32_slice_le_bytes(&[loss], &mut out);
    }
    out
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || cross_entropy("logits", "targets", "loss", 2, 4),
        test_inputs: Some(|| {
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![
                // logits: 2 tokens × 4 vocab
                to_f32(&[1.0, 2.0, 3.0, 0.5,  0.1, 0.2, 0.3, 0.4]),
                // targets: [2, 0]
                to_u32(&[2, 0]),
            ]]
        }),
        expected_output: Some(|| {
            let logits = [1.0_f32, 2.0, 3.0, 0.5, 0.1, 0.2, 0.3, 0.4];
            let targets = [2_u32, 0];
            vec![vec![reference_cross_entropy_bytes(&logits, &targets, 4)]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    #[test]
    fn cross_entropy_matches_logsumexp_reference() {
        let logits = [1.0_f32, 2.0, 3.0, 0.5, 0.1, 0.2, 0.3, 0.4];
        let targets = [2_u32, 0];
        let program = cross_entropy("logits", "targets", "loss", 2, 4);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&targets)),
            Value::from(vec![0u8; 4 * 2 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy must execute in the reference interpreter.");
        assert_eq!(
            outputs[0].to_bytes(),
            reference_cross_entropy_bytes(&logits, &targets, 4)
        );
    }

    #[test]
    fn try_cross_entropy_rejects_zero_and_overflow_dimensions() {
        assert!(matches!(
            try_cross_entropy("logits", "targets", "loss", 0, 4),
            Err(crate::tensor_ref::TensorRefError::ShapeMismatch { .. })
        ));
        assert!(matches!(
            try_cross_entropy("logits", "targets", "loss", u32::MAX, 2),
            Err(crate::tensor_ref::TensorRefError::ElementCountOverflow { .. })
        ));
    }

    #[test]
    fn cross_entropy_nan_in_logits_propagates_nan() {
        let logits = [f32::NAN, 1.0, 2.0, 0.5];
        let targets = [0u32];
        let program = cross_entropy("logits", "targets", "loss", 1, 4);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&targets)),
            Value::from(vec![0u8; 4 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy must not panic on NaN logits");
        let loss = f32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert!(
            loss.is_nan(),
            "cross_entropy with NaN logits must produce NaN loss, got {loss}"
        );
    }

    #[test]
    fn cross_entropy_inf_in_logits() {
        // +Inf in logits: the log-sum-exp formulation may produce NaN or Inf.
        // We just assert it does not panic and the result is not a finite wrong value.
        let logits = [f32::INFINITY, 1.0, 2.0, 0.5];
        let targets = [0u32];
        let program = cross_entropy("logits", "targets", "loss", 1, 4);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&targets)),
            Value::from(vec![0u8; 4 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy must not panic on +Inf logits");
        let loss = f32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert!(
            loss.is_nan() || loss.is_infinite(),
            "cross_entropy with +Inf logits must produce NaN or Inf, got {loss}"
        );
    }

    #[test]
    fn cross_entropy_all_zeros_is_log_vocab() {
        let logits = [0.0f32, 0.0, 0.0, 0.0];
        let targets = [0u32];
        let program = cross_entropy("logits", "targets", "loss", 1, 4);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&targets)),
            Value::from(vec![0u8; 4 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy all-zeros must execute");
        let loss = f32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        let expected = (4.0f32).ln();
        assert!(
            (loss - expected).abs() <= 1.0e-4,
            "cross_entropy all-zeros must equal ln(vocab_size) = {expected}, got {loss}"
        );
    }

    #[test]
    fn cross_entropy_all_ones_same_as_all_zeros() {
        // Uniform shift: adding 1.0 to every logit does not change softmax or loss.
        let logits = [1.0f32; 4];
        let _targets = [1u32];
        let program = cross_entropy("logits", "targets", "loss", 1, 4);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&[1u32])),
            Value::from(vec![0u8; 4 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy all-ones must execute");
        let loss = f32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        let expected = (4.0f32).ln();
        assert!(
            (loss - expected).abs() <= 1.0e-4,
            "cross_entropy all-ones must equal ln(vocab_size) = {expected}, got {loss}"
        );
    }

    #[test]
    fn cross_entropy_single_token_single_vocab() {
        let logits = [0.5f32];
        let targets = [0u32];
        let program = cross_entropy("logits", "targets", "loss", 1, 1);
        let inputs = vec![
            Value::from(vyre_primitives::wire::pack_f32_slice(&logits)),
            Value::from(vyre_primitives::wire::pack_u32_slice(&targets)),
            Value::from(vec![0u8; 4 * CROSS_ENTROPY_TILE as usize]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: cross_entropy single token must execute");
        let loss = f32::from_le_bytes(outputs[0].to_bytes()[0..4].try_into().unwrap());
        assert!(
            loss.abs() <= 1.0e-4,
            "cross_entropy single token single vocab must be 0.0, got {loss}"
        );
    }
}
