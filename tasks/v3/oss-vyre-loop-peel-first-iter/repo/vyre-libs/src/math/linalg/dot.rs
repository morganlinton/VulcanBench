//! Dot product  -  element-wise multiply + sum-reduce.
//!
//! Category A composition: reads two equally-sized u32 buffers,
//! multiplies element-wise, and reduces through workgroup scratch.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::{
    builder::{check_tensors, strided_accumulate_child, BuildOptions},
    region::wrap,
    tensor_ref::{TensorRef, TensorRefError},
};
use vyre_primitives::reduce::workgroup_tree::{self, WorkgroupReductionScope};

const OP_ID: &str = "vyre-libs::math::dot";
#[cfg(test)]
const DOT_REFERENCE_OP_ID: &str = "vyre-libs::math::dot_reference";
const DOT_TILE: u32 = 256;

/// Typed Cat-A builder for [`dot`].
#[derive(Debug, Clone)]
pub struct Dot {
    lhs: TensorRef,
    rhs: TensorRef,
    out: TensorRef,
    options: BuildOptions,
}

impl Dot {
    /// Create a dot-product builder from two input vectors and one scalar output.
    #[must_use]
    pub fn new(lhs: TensorRef, rhs: TensorRef, out: TensorRef) -> Self {
        Self {
            lhs,
            rhs,
            out,
            options: BuildOptions::default(),
        }
    }

    /// Validate tensor metadata and materialize the dot-product Program.
    ///
    /// # Errors
    ///
    /// Returns [`TensorRefError`] when dtypes, names, shapes, or vector length
    /// violate the dot-product contract.
    pub fn build(self) -> Result<Program, TensorRefError> {
        check_tensors(
            OP_ID,
            &[
                (&self.lhs, DataType::U32),
                (&self.rhs, DataType::U32),
                (&self.out, DataType::U32),
            ],
        )?;
        if self.lhs.shape != self.rhs.shape {
            return Err(TensorRefError::ShapeMismatch {
                name: self.rhs.name_str().to_string(),
                found: self.rhs.shape.to_vec(),
                expected: self.lhs.shape.to_vec(),
                op: OP_ID,
            });
        }
        if self.out.shape.as_ref() != [1] {
            return Err(TensorRefError::ShapeMismatch {
                name: self.out.name_str().to_string(),
                found: self.out.shape.to_vec(),
                expected: vec![1],
                op: OP_ID,
            });
        }
        let n = self
            .lhs
            .element_count()
            .ok_or_else(|| TensorRefError::ElementCountOverflow {
                name: self.lhs.name_str().to_string(),
                shape: self.lhs.shape.to_vec(),
            })?;
        if n == 0 {
            return Err(TensorRefError::ShapeMismatch {
                name: self.lhs.name_str().to_string(),
                found: self.lhs.shape.to_vec(),
                expected: vec![1],
                op: OP_ID,
            });
        }
        let lhs = self.lhs.name_str();
        let rhs = self.rhs.name_str();
        let out = self.out.name_str();
        let workgroup = self.options.workgroup_size.unwrap_or([DOT_TILE, 1, 1]);
        let tile = workgroup[0].max(1);
        let region = wrap(
            self.options.region_generator.unwrap_or(OP_ID),
            dot_tiled_body(lhs, rhs, out, n, tile),
            None,
        );
        Ok(Program::wrapped(
            vec![
                BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
                BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
                BufferDecl::workgroup("dot_scratch", tile, DataType::U32),
                BufferDecl::output(out, 2, DataType::U32).with_count(1),
            ],
            workgroup,
            vec![region],
        ))
    }
}

crate::builder::impl_cat_a_builder_options!(Dot);

/// Build a Program that computes the dot product of `lhs` and `rhs`
/// (both length `n`) into `out[0]`.
///
/// Buffers:
/// - `lhs` (u32, read-only, n elems)
/// - `rhs` (u32, read-only, n elems)
/// - `out` (u32, output, 1 elem)
///
/// Workgroup size `[256, 1, 1]` by default: each lane accumulates a
/// strided slice and the workgroup reduces into `out[0]`.
///
/// # Errors
/// Returns `Err` when `n == 0`  -  empty reductions are rejected
/// (FINDING-V7-TEST-009-DOT).
pub fn dot(lhs: &str, rhs: &str, out: &str, n: u32) -> Result<Program, String> {
    Dot::new(
        TensorRef::u32_1d(lhs, n),
        TensorRef::u32_1d(rhs, n),
        TensorRef::u32_1d(out, 1),
    )
    .build()
    .map_err(|error| format!("Fix: {OP_ID} build failed: {error}"))
}

fn dot_tiled_body(lhs: &str, rhs: &str, out: &str, n: u32, tile: u32) -> Vec<Node> {
    let chunks = n.div_ceil(tile);
    let local = Expr::var("local");
    let mut body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        strided_accumulate_child(
            OP_ID,
            tile,
            chunks,
            n,
            "local_acc",
            Expr::u32(0),
            "dot_scratch",
            |idx, acc| {
                Expr::add(
                    acc,
                    Expr::mul(Expr::load(lhs, idx.clone()), Expr::load(rhs, idx)),
                )
            },
        ),
        Node::barrier(),
    ];
    body.push(workgroup_tree::sum_u32_child(
        OP_ID,
        tile,
        "dot_scratch",
        WorkgroupReductionScope::FirstWorkgroup,
    ));
    body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::WorkgroupId { axis: 0 }, Expr::u32(0)),
            Expr::eq(local, Expr::u32(0)),
        ),
        vec![Node::Store {
            buffer: out.into(),
            index: Expr::u32(0),
            value: Expr::load("dot_scratch", Expr::u32(0)),
        }],
    ));
    body
}

#[cfg(test)]
fn dot_reference_body(lhs: &str, rhs: &str, out: &str, n: u32) -> Vec<Node> {
    vec![
        Node::let_bind("acc", Expr::u32(0)),
        Node::loop_for(
            "dk",
            Expr::u32(0),
            Expr::u32(n),
            vec![Node::assign(
                "acc",
                Expr::add(
                    Expr::var("acc"),
                    Expr::mul(
                        Expr::load(lhs, Expr::var("dk")),
                        Expr::load(rhs, Expr::var("dk")),
                    ),
                ),
            )],
        ),
        Node::Store {
            buffer: out.into(),
            index: Expr::u32(0),
            value: Expr::var("acc"),
        },
    ]
}

#[cfg(test)]
fn dot_reference(lhs: &str, rhs: &str, out: &str, n: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 2, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![wrap(
            DOT_REFERENCE_OP_ID,
            dot_reference_body(lhs, rhs, out, n),
            None,
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || dot("lhs", "rhs", "out", 256).unwrap_or_else(|error| crate::invalid_program(OP_ID, format!("Fix: dot fixture must build: {error}"))),
        test_inputs: Some(|| vec![vec![
            vec![0u8; 256 * 4],
            vec![0u8; 256 * 4],
        ]]),
        expected_output: Some(|| vec![vec![
            0u32.to_le_bytes().to_vec(),
        ]]),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::decode_u32_one as decode_one;
    use vyre_reference::value::Value;

    #[test]
    fn tiled_dot_matches_scalar_reference_across_multiple_tiles() {
        let n = 777_u32;
        let lhs = (0..n)
            .map(|i| i.wrapping_mul(17).wrapping_add(3))
            .collect::<Vec<_>>();
        let rhs = (0..n)
            .map(|i| i.wrapping_mul(29).wrapping_add(11))
            .collect::<Vec<_>>();
        let run = |program: Program| {
            let outputs = vyre_reference::reference_eval(
                &program,
                &[
                    Value::from(crate::test_support::byte_pack::u32_bytes(&lhs)),
                    Value::from(crate::test_support::byte_pack::u32_bytes(&rhs)),
                    Value::from(vec![0u8; core::mem::size_of::<u32>()]),
                ],
            )
            .expect("Fix: dot program must execute in the reference interpreter.");
            decode_one(&outputs[0].to_bytes())
        };
        let actual = run(dot("lhs", "rhs", "out", n).expect("Fix: dot dimensions are valid"));
        let expected = run(dot_reference("lhs", "rhs", "out", n));
        assert_eq!(
            actual, expected,
            "tiled dot must preserve wrapping u32 scalar semantics"
        );
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures exposing real gaps
    // ------------------------------------------------------------------

    #[test]
    fn dot_single_element() {
        let lhs = vec![7u32];
        let rhs = vec![3u32];
        let program = dot("lhs", "rhs", "out", 1).expect("Fix: dot n=1 must build");
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::test_support::byte_pack::u32_bytes(&lhs)),
                Value::from(crate::test_support::byte_pack::u32_bytes(&rhs)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: dot n=1 must execute");
        let actual = decode_one(&outputs[0].to_bytes());
        assert_eq!(actual, 21u32, "dot of [7]·[3] = 21");
    }

    #[test]
    fn dot_empty_rejected() {
        let err = dot("lhs", "rhs", "out", 0).expect_err("dot n=0 must be rejected");
        assert!(
            err.contains("dot") || err.contains("ShapeMismatch"),
            "dot n=0 error must name the op or shape failure: {err}"
        );
    }

    #[test]
    fn dot_large_n_tile_boundary_matches_reference() {
        let n = 1025_u32; // Just above DOT_TILE=256, needs multiple tiles
        let lhs: Vec<u32> = (0..n).map(|i| i.wrapping_add(1)).collect();
        let rhs: Vec<u32> = (0..n).map(|i| i.wrapping_add(2)).collect();
        let program = dot("lhs", "rhs", "out", n).expect("Fix: dot n=1025 must build");
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(crate::test_support::byte_pack::u32_bytes(&lhs)),
                Value::from(crate::test_support::byte_pack::u32_bytes(&rhs)),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: dot n=1025 must execute");
        let actual = decode_one(&outputs[0].to_bytes());
        let expected: u32 = lhs
            .iter()
            .zip(rhs.iter())
            .map(|(a, b)| a.wrapping_mul(*b))
            .fold(0u32, |acc, x| acc.wrapping_add(x));
        assert_eq!(actual, expected, "dot n=1025 mismatch");
    }
}
