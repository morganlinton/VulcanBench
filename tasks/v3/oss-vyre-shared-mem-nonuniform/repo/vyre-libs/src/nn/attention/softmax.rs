//! Softmax  -  `softmax(x)_i = exp(x_i - max(x)) / sum(exp(x_j - max(x)))`.
//!
//! Category-A composition over `BinOp::Sub/Div`, `UnOp::Exp`, and
//! `Expr::max`. The numerically-stable formulation subtracts the max
//! before exponentiating so the sum stays in `[1.0, n]` regardless of
//! the input magnitude.
//!
//! Shape: single-workgroup tiled reduction (max, sum-of-exp, divide)
//! using workgroup scratch. [`softmax_reference`] keeps the scalar
//! correctness oracle available for parity tests and conservative
//! callers.
//!
//! ## API surface
//!
//! - [`Softmax`]  -  typed builder. Accepts [`TensorRef`]s, checks dtype +
//!   shape + name-uniqueness at [`Softmax::build`] time, returns
//!   [`TensorRefError`] on contract violation.
//! - [`softmax`]  -  back-compat free function. Calls the builder with
//!   default options and lowers invalid inputs to an explicit trap.
//!
//! Both paths produce the same IR. New code should prefer the builder.

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_primitives::reduce::workgroup_tree::{self, WorkgroupReductionScope};

use crate::builder::{
    check_tensors, strided_accumulate_child, strided_writeback_child, BuildOptions,
};
use crate::region::wrap;
use crate::tensor_ref::{TensorRef, TensorRefError};

/// Canonical op id; matches the region generator name so conformance
/// certificates stay self-describing.
const OP_ID: &str = "vyre-libs::nn::softmax";
const REFERENCE_OP_ID: &str = "vyre-libs::nn::softmax_reference";
const SOFTMAX_TILE: u32 = 256;

/// Typed Cat-A builder for [`softmax`]. Future knobs (workgroup size,
/// region generator override, tenant id) land as [`BuildOptions`]
/// chains without changing the builder's method surface.
#[derive(Debug, Clone)]
pub struct Softmax {
    input: TensorRef,
    output: TensorRef,
    options: BuildOptions,
}

impl Softmax {
    /// Start a builder with the two required tensors. Use chaining
    /// methods for optional overrides.
    #[must_use]
    pub fn new(input: TensorRef, output: TensorRef) -> Self {
        Self {
            input,
            output,
            options: BuildOptions::default(),
        }
    }

    /// Validate + materialize the Program.
    ///
    /// # Errors
    ///
    /// - [`TensorRefError::DtypeMismatch`] if either tensor isn't `F32`.
    /// - [`TensorRefError::ShapeMismatch`] if `input` and `output`
    ///   shapes diverge (both must be 1-D with matching length).
    /// - [`TensorRefError::NameCollision`] if input and output
    ///   resolve to the same buffer name.
    /// - [`TensorRefError::ElementCountOverflow`] on pathological
    ///   shapes whose product exceeds `u32::MAX`.
    pub fn build(self) -> Result<Program, TensorRefError> {
        check_tensors(
            OP_ID,
            &[(&self.input, DataType::F32), (&self.output, DataType::F32)],
        )?;
        if self.input.shape != self.output.shape {
            return Err(TensorRefError::ShapeMismatch {
                name: self.output.name.as_str().to_string(),
                found: self.output.shape.to_vec(),
                expected: self.input.shape.to_vec(),
                op: OP_ID,
            });
        }
        let n = self
            .input
            .element_count()
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: self.input.name_str().to_string(),
                shape: self.input.shape.to_vec(),
            })?;
        // V7-CORR-012: reject n=0 so the first Expr::load(input, 0)
        // sentinel is not an out-of-bounds read. softmax(∅) is
        // undefined; the builder surfaces the error explicitly.
        if n == 0 {
            return Err(TensorRefError::ShapeMismatch {
                name: self.input.name.as_str().to_string(),
                found: self.input.shape.to_vec(),
                expected: vec![1],
                op: OP_ID,
            });
        }

        let generator = self.options.region_generator.unwrap_or(OP_ID);
        Ok(softmax_tiled_program(
            self.input.name_str(),
            self.output.name_str(),
            n,
            self.options.workgroup_size.unwrap_or([SOFTMAX_TILE, 1, 1]),
            generator,
        ))
    }
}

crate::builder::impl_cat_a_builder_options!(Softmax);

/// Build a softmax Program from raw buffer names. Back-compat wrapper
/// around [`Softmax`]; panics on contract violation. New code should
/// prefer the builder.
#[must_use]
pub fn softmax(input: &str, output: &str, n: u32) -> Program {
    Softmax::new(TensorRef::f32_1d(input, n), TensorRef::f32_1d(output, n))
        .build()
        .unwrap_or_else(|err| {
            crate::builder::invalid_output_program(
                OP_ID,
                output,
                DataType::F32,
                format!("Fix: softmax build failed: {err}"),
            )
        })
}

/// Build the scalar three-pass softmax correctness reference.
#[must_use]
pub fn softmax_reference(input: &str, output: &str, n: u32) -> Program {
    if n == 0 {
        return crate::builder::invalid_output_program(
            REFERENCE_OP_ID,
            output,
            DataType::F32,
            "Fix: softmax_reference requires n > 0, got 0.".to_string(),
        );
    }
    softmax_reference_program(input, output, n, [1, 1, 1], REFERENCE_OP_ID)
}

fn softmax_tiled_program(
    input: &str,
    output: &str,
    n: u32,
    workgroup: [u32; 3],
    generator: &'static str,
) -> Program {
    let tile = workgroup[0].max(1);
    let chunks = n.div_ceil(tile);
    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        strided_accumulate_child(
            OP_ID,
            tile,
            chunks,
            n,
            "local_max",
            Expr::f32(f32::MIN),
            "softmax_scratch",
            |idx, acc| {
                let loaded = Expr::load(input, idx);
                Expr::select(
                    Expr::BinOp {
                        op: BinOp::Gt,
                        left: Box::new(loaded.clone()),
                        right: Box::new(acc.clone()),
                    },
                    loaded,
                    acc,
                )
            },
        ),
        Node::barrier(),
    ];
    body.push(workgroup_tree::max_f32_child(
        OP_ID,
        tile,
        "softmax_scratch",
        WorkgroupReductionScope::FirstWorkgroup,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            Expr::eq(Expr::var("local"), Expr::u32(0)),
        ),
        vec![Node::Store {
            buffer: "softmax_max".into(),
            index: Expr::u32(0),
            value: Expr::load("softmax_scratch", Expr::u32(0)),
        }],
    ));
    body.push(Node::barrier());
    body.extend(vec![
        strided_accumulate_child(
            OP_ID,
            tile,
            chunks,
            n,
            "local_sum",
            Expr::f32(0.0),
            "softmax_scratch",
            |idx, acc| {
                Expr::add(
                    acc,
                    Expr::UnOp {
                        op: UnOp::Exp,
                        operand: Box::new(Expr::BinOp {
                            op: BinOp::Sub,
                            left: Box::new(Expr::load(input, idx)),
                            right: Box::new(Expr::load("softmax_max", Expr::u32(0))),
                        }),
                    },
                )
            },
        ),
        Node::barrier(),
    ]);
    body.push(workgroup_tree::sum_f32_child(
        OP_ID,
        tile,
        "softmax_scratch",
        WorkgroupReductionScope::FirstWorkgroup,
    ));
    body.push(strided_writeback_child(
        OP_ID,
        tile,
        chunks,
        n,
        output,
        vec![
            Node::let_bind("sum_val", Expr::load("softmax_scratch", Expr::u32(0))),
            Node::let_bind("max_val", Expr::load("softmax_max", Expr::u32(0))),
        ],
        |idx| Expr::BinOp {
            op: BinOp::Div,
            left: Box::new(Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::BinOp {
                    op: BinOp::Sub,
                    left: Box::new(Expr::load(input, idx)),
                    right: Box::new(Expr::var("max_val")),
                }),
            }),
            right: Box::new(Expr::var("sum_val")),
        },
    ));
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::workgroup("softmax_scratch", tile, DataType::F32),
            BufferDecl::workgroup("softmax_max", 1, DataType::F32),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        workgroup,
        vec![wrap(generator, body, None)],
    )
}

fn softmax_reference_program(
    input: &str,
    output: &str,
    n: u32,
    workgroup: [u32; 3],
    generator: &'static str,
) -> Program {
    let n_expr = Expr::u32(n);
    let max_loop = Node::loop_for(
        "i",
        Expr::u32(1),
        n_expr.clone(),
        vec![Node::assign(
            "max_val",
            Expr::select(
                Expr::gt(Expr::load(input, Expr::var("i")), Expr::var("max_val")),
                Expr::load(input, Expr::var("i")),
                Expr::var("max_val"),
            ),
        )],
    );
    let sum_loop = Node::loop_for(
        "i",
        Expr::u32(0),
        n_expr.clone(),
        vec![Node::assign(
            "sum_val",
            Expr::add(
                Expr::var("sum_val"),
                Expr::UnOp {
                    op: UnOp::Exp,
                    operand: Box::new(Expr::BinOp {
                        op: BinOp::Sub,
                        left: Box::new(Expr::load(input, Expr::var("i"))),
                        right: Box::new(Expr::var("max_val")),
                    }),
                },
            ),
        )],
    );
    let write_loop = Node::loop_for(
        "i",
        Expr::u32(0),
        n_expr,
        vec![Node::Store {
            buffer: output.into(),
            index: Expr::var("i"),
            value: Expr::BinOp {
                op: BinOp::Div,
                left: Box::new(Expr::UnOp {
                    op: UnOp::Exp,
                    operand: Box::new(Expr::BinOp {
                        op: BinOp::Sub,
                        left: Box::new(Expr::load(input, Expr::var("i"))),
                        right: Box::new(Expr::var("max_val")),
                    }),
                }),
                right: Box::new(Expr::var("sum_val")),
            },
        }],
    );
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(output, 1, DataType::F32).with_count(n),
        ],
        workgroup,
        vec![wrap(
            generator,
            vec![
                Node::let_bind("max_val", Expr::load(input, Expr::u32(0))),
                max_loop,
                Node::let_bind("sum_val", Expr::f32(0.0)),
                sum_loop,
                write_loop,
            ],
            None,
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::nn::softmax",
        build: || softmax("input", "output", 4),
        test_inputs: Some(|| {
            let input = [0.5f32, -1.0, 1.5, 0.25];
            vec![vec![
                vyre_primitives::wire::pack_f32_slice(&input),
                vec![0u8; input.len() * core::mem::size_of::<f32>()],
            ]]
        }),
        expected_output: Some(|| vec![
            vec![
                vec![0x7b, 0xf0, 0x58, 0x3e, 0x74, 0x9f, 0x41, 0x3d, 0xf3, 0x6c, 0x13, 0x3f, 0xdb, 0xf3, 0x28, 0x3e, ],
            ],
        ]),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn softmax_lowers_and_emits_sm80_kernel() {
        let program = softmax("input", "output", 4);
        let descriptor = vyre_lower::lower(&program).unwrap();
        let ptx = vyre_emit_ptx::emit_with_options(
            &descriptor,
            vyre_emit_ptx::PtxEmitOptions {
                target: vyre_emit_ptx::ComputeCapability::SM_80,
                subgroup_size: 32,
                ulp_budget: Some(128),
                cooperative_grid_sync: false,
            },
        )
        .unwrap();
        assert!(ptx.contains(".target sm_80"), "{ptx}");
        assert!(ptx.contains(".visible .entry main("), "{ptx}");
        assert!(ptx.contains("region: vyre-libs::nn::softmax"), "{ptx}");
    }

    #[test]
    fn builder_rejects_dtype_mismatch() {
        let err = Softmax::new(TensorRef::u32_1d("in", 4), TensorRef::f32_1d("out", 4))
            .build()
            .unwrap_err();
        assert!(matches!(err, TensorRefError::DtypeMismatch { .. }));
    }

    #[test]
    fn builder_rejects_shape_mismatch() {
        let err = Softmax::new(TensorRef::f32_1d("in", 4), TensorRef::f32_1d("out", 8))
            .build()
            .unwrap_err();
        assert!(matches!(err, TensorRefError::ShapeMismatch { .. }));
    }

    #[test]
    fn builder_rejects_name_collision() {
        let err = Softmax::new(TensorRef::f32_1d("x", 4), TensorRef::f32_1d("x", 4))
            .build()
            .unwrap_err();
        assert!(matches!(err, TensorRefError::NameCollision { .. }));
    }

    #[test]
    fn builder_workgroup_override_lands_in_program() {
        let program = Softmax::new(TensorRef::f32_1d("in", 4), TensorRef::f32_1d("out", 4))
            .with_workgroup_size([64, 1, 1])
            .build()
            .unwrap();
        assert_eq!(program.workgroup_size(), [64, 1, 1]);
    }

    #[test]
    fn builder_region_generator_override_lands_in_program() {
        let program = Softmax::new(TensorRef::f32_1d("in", 4), TensorRef::f32_1d("out", 4))
            .with_region_generator("custom::softmax")
            .build()
            .unwrap();
        match &program.entry()[0] {
            Node::Region { generator, .. } => {
                assert_eq!(generator.as_str(), "custom::softmax");
            }
            other => panic!("expected Region, got {other:?}"),
        }
    }

    #[test]
    fn free_function_and_builder_produce_equal_programs_by_default() {
        let free = softmax("in", "out", 4);
        let built = Softmax::new(TensorRef::f32_1d("in", 4), TensorRef::f32_1d("out", 4))
            .build()
            .unwrap();
        // to_wire is the canonical byte-identity gate  -  a divergence
        // between the two paths is a refactor regression.
        let free_bytes = free.to_wire().unwrap();
        let built_bytes = built.to_wire().unwrap();
        assert_eq!(
            free_bytes, built_bytes,
            "free `softmax` and builder `Softmax::build` must yield byte-identical wire output"
        );
    }

    #[test]
    fn tiled_softmax_matches_scalar_reference_across_multiple_tiles() {
        let n = 513_u32;
        let input = (0..n)
            .map(|i| ((i as f32) * 0.03125).sin() * 4.0 - ((i % 7) as f32))
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; n as usize * 4]),
                ],
            )
            .expect("Fix: softmax program must execute in the reference interpreter.");

            decode_f32(&outputs[0].to_bytes())
        };
        let actual = run(softmax("input", "output", n));
        let expected = run(softmax_reference("input", "output", n));
        for (idx, (lhs, rhs)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (lhs - rhs).abs() <= 1.0e-6,
                "softmax mismatch at lane {idx}: tiled={lhs:?} reference={rhs:?}"
            );
        }
    }

    #[test]
    fn generated_softmax_matches_reference_for_2048_lanes() {
        let n = 2048_u32;
        let input = (0..n)
            .map(|i| {
                let wave = ((i as f32) * 0.019_531_25).cos() * 6.0;
                let saw = ((i % 53) as f32 - 26.0) * 0.0625;
                wave - saw
            })
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(f32_bytes(&input)),
                    Value::from(vec![0u8; n as usize * 4]),
                ],
            )
            .expect("Fix: generated softmax program must execute in the reference interpreter.");
            decode_f32(&outputs[0].to_bytes())
        };
        let actual = run(softmax("input", "output", n));
        let expected = run(softmax_reference("input", "output", n));
        for (idx, (lhs, rhs)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (lhs - rhs).abs() <= 2.0e-5,
                "generated softmax mismatch at lane {idx}: tiled={lhs:?} reference={rhs:?}"
            );
        }
    }

    #[test]
    fn softmax_all_very_large_values_does_not_overflow() {
        // All equal large values: max = 88, exp(0) = 1, sum = n, output = 1/n.
        // This tests that the max-subtraction stabilizer works even at scale.
        let input = [88.0f32; 8];
        let program = softmax("input", "output", 8);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 32])],
        )
        .expect("Fix: softmax must not panic on all-large values");
        let out = decode_f32(&outputs[0].to_bytes());
        let expected = 1.0 / 8.0;
        for (i, &v) in out.iter().enumerate() {
            assert!(
                (v - expected).abs() <= 1.0e-5,
                "softmax all-large mismatch at {i}: {v} != {expected}"
            );
        }
    }

    #[test]
    fn softmax_zero_sequence_length_rejected() {
        let err = Softmax::new(TensorRef::f32_1d("in", 0), TensorRef::f32_1d("out", 0))
            .build()
            .unwrap_err();
        assert!(matches!(err, TensorRefError::ShapeMismatch { .. }));
    }

    #[test]
    fn softmax_single_token() {
        let input = [2.5f32];
        let program = softmax("input", "output", 1);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 4])],
        )
        .expect("Fix: softmax single token must execute");
        let out = decode_f32(&outputs[0].to_bytes());
        assert_eq!(out[0], 1.0, "softmax of a single element must be 1.0");
    }

    #[test]
    fn softmax_nan_in_input_propagates() {
        let input = [f32::NAN, 1.0, 2.0, 3.0];
        let program = softmax("input", "output", 4);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; 16])],
        )
        .expect("Fix: softmax must not panic on NaN input");
        let out = decode_f32(&outputs[0].to_bytes());
        // At least one output lane must be NaN because NaN poisons the sum.
        assert!(
            out.iter().any(|v| v.is_nan()),
            "softmax with NaN input must produce at least one NaN output, got {:?}",
            out
        );
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn softmax_sums_to_one(input in prop::collection::vec(prop::num::f32::NORMAL, 1..16)) {
            let n = input.len() as u32;
            let program = softmax("input", "output", n);
            let outputs = vyre_reference::reference_eval(
                &program,
                &[Value::from(f32_bytes(&input)), Value::from(vec![0u8; input.len() * 4])],
            )
            .expect("Fix: softmax must execute");
            let out = decode_f32(&outputs[0].to_bytes());
            let sum = out.iter().sum::<f32>();
            prop_assert!(
                (sum - 1.0).abs() <= 1.0e-4,
                "softmax must sum to 1.0, got {sum}"
            );
        }
    }
}
