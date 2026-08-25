//! Public cooperative tiled matmul builders and Cat-A wrappers.

use vyre::ir::{DataType, Program};

use crate::builder::{check_tensors, BuildOptions};
use crate::tensor_ref::{TensorRef, TensorRefError};

use super::program::{build_matmul_tiled_program, MatmulTiledProgramSpec};
use super::mma_fragment::MmaCapabilityRecord;
use super::shape::MatrixShape;
use super::tensor_core_policy::{
    plan_matmul_kernel, F32MatmulMode, MatmulKernelCapabilities, MatmulKernelPath,
};

const OP_ID: &str = "vyre-libs::math::matmul_tiled";
const OP_ID_BIAS: &str = "vyre-libs::math::matmul_bias_tiled";

#[derive(Debug, Clone)]
struct MatmulTiledCore {
    op_id: &'static str,
    a: TensorRef,
    b: TensorRef,
    bias: Option<TensorRef>,
    out: TensorRef,
    tile: u32,
    options: BuildOptions,
}

impl MatmulTiledCore {
    fn plain(a: TensorRef, b: TensorRef, out: TensorRef, tile: u32) -> Self {
        Self {
            op_id: OP_ID,
            a,
            b,
            bias: None,
            out,
            tile,
            options: BuildOptions::default(),
        }
    }

    fn bias(a: TensorRef, b: TensorRef, bias: TensorRef, out: TensorRef, tile: u32) -> Self {
        Self {
            op_id: OP_ID_BIAS,
            a,
            b,
            bias: Some(bias),
            out,
            tile,
            options: BuildOptions::default(),
        }
    }

    fn plain_auto(a: TensorRef, b: TensorRef, out: TensorRef) -> Self {
        let tile = auto_tile_for(&a, &b);
        Self::plain(a, b, out, tile)
    }

    fn bias_auto(a: TensorRef, b: TensorRef, bias: TensorRef, out: TensorRef) -> Self {
        let tile = auto_tile_for(&a, &b);
        Self::bias(a, b, bias, out, tile)
    }

    fn with_workgroup_size(mut self, size: [u32; 3]) -> Self {
        self.options = self.options.with_workgroup_size(size);
        self
    }

    fn with_region_generator(mut self, name: &'static str) -> Self {
        self.options = self.options.with_region_generator(name);
        self
    }

    fn with_tenant_id(mut self, tenant_id: u32) -> Self {
        self.options = self.options.with_tenant_id(tenant_id);
        self
    }

    fn build(self) -> Result<Program, TensorRefError> {
        let dtype = self.a.dtype.clone();
        let tensors = if let Some(bias) = self.bias.as_ref() {
            vec![
                (&self.a, dtype.clone()),
                (&self.b, dtype.clone()),
                (bias, dtype.clone()),
                (&self.out, dtype.clone()),
            ]
        } else {
            vec![
                (&self.a, dtype.clone()),
                (&self.b, dtype.clone()),
                (&self.out, dtype.clone()),
            ]
        };
        check_tensors(self.op_id, &tensors)?;

        if self.tile == 0 {
            return Err(TensorRefError::ShapeMismatch {
                name: "tile".into(),
                found: vec![0],
                expected: vec![1],
                op: self.op_id,
            });
        }

        let shape_name = if self.bias.is_some() {
            "a/b/bias/out"
        } else {
            "a/b/out"
        };
        let bias_shape_is_valid = self
            .bias
            .as_ref()
            .map_or(true, |bias| bias.shape.len() == 1);
        if self.a.shape.len() != 2
            || self.b.shape.len() != 2
            || !bias_shape_is_valid
            || self.out.shape.len() != 2
        {
            return Err(TensorRefError::ShapeMismatch {
                name: shape_name.into(),
                found: vec![],
                expected: vec![0, 0],
                op: self.op_id,
            });
        }

        let m = self.a.shape[0];
        let k = self.a.shape[1];
        let n = self.b.shape[1];
        if self.b.shape[0] != k {
            return Err(TensorRefError::ShapeMismatch {
                name: self.b.name.as_str().to_string(),
                found: self.b.shape.to_vec(),
                expected: vec![k, n],
                op: self.op_id,
            });
        }
        if let Some(bias) = self.bias.as_ref() {
            if bias.shape[0] != n {
                return Err(TensorRefError::ShapeMismatch {
                    name: bias.name.as_str().to_string(),
                    found: bias.shape.to_vec(),
                    expected: vec![n],
                    op: self.op_id,
                });
            }
        }
        if self.out.shape.as_ref() != [m, n] {
            return Err(TensorRefError::ShapeMismatch {
                name: self.out.name.as_str().to_string(),
                found: self.out.shape.to_vec(),
                expected: vec![m, n],
                op: self.op_id,
            });
        }

        build_matmul_tiled_program(MatmulTiledProgramSpec {
            op_id: self.op_id,
            a: self.a.name_str(),
            b: self.b.name_str(),
            bias: self.bias.as_ref().map(TensorRef::name_str),
            out: self.out.name_str(),
            m,
            k,
            n,
            tile: self.tile,
            workgroup: self.options.workgroup_size.unwrap_or([16, 16, 1]),
            generator: self.options.region_generator.unwrap_or(self.op_id),
            dtype,
            a_tile_name: "matmul_a_tile",
            b_tile_name: "matmul_b_tile",
            mma_capabilities: MmaCapabilityRecord::ptx_sm80(),
        })
    }
}

fn auto_tile_for(a: &TensorRef, b: &TensorRef) -> u32 {
    if a.shape.len() != 2 || b.shape.len() != 2 {
        return 1;
    }

    let m = a.shape[0];
    let k = a.shape[1];
    let n = b.shape[1];
    if plan_matmul_kernel(
        &a.dtype,
        MatrixShape { m, k, n },
        16,
        1,
        F32MatmulMode::StrictF32,
        MatmulKernelCapabilities::current_codegen(),
    )
    .selected_path
        == MatmulKernelPath::TensorCoreF16M16N8K16
    {
        return 16;
    }

    let bounded_k = k.clamp(1, 32);
    bounded_k.next_power_of_two() >> u32::from(!bounded_k.is_power_of_two())
}

macro_rules! impl_common_builder_controls {
    ($builder:ident) => {
        /// Override workgroup size.
        #[must_use]
        pub fn with_workgroup_size(mut self, size: [u32; 3]) -> Self {
            self.core = self.core.with_workgroup_size(size);
            self
        }

        /// Override region generator name.
        #[must_use]
        pub fn with_region_generator(mut self, name: &'static str) -> Self {
            self.core = self.core.with_region_generator(name);
            self
        }

        /// Stamp tenant id.
        #[must_use]
        pub fn with_tenant_id(mut self, tenant_id: u32) -> Self {
            self.core = self.core.with_tenant_id(tenant_id);
            self
        }

        /// Validate + materialize.
        ///
        /// # Errors
        ///
        /// Returns tensor shape, dtype, name, tile, or element-count contract errors.
        pub fn build(self) -> Result<Program, TensorRefError> {
            self.core.build()
        }
    };
}

/// Typed Cat-A builder for [`matmul_tiled`].
#[derive(Debug, Clone)]
pub struct MatmulTiled {
    core: MatmulTiledCore,
}

impl MatmulTiled {
    /// Start a builder. `tile` splits the k axis for register-reuse.
    #[must_use]
    pub fn new(a: TensorRef, b: TensorRef, out: TensorRef, tile: u32) -> Self {
        Self {
            core: MatmulTiledCore::plain(a, b, out, tile),
        }
    }

    /// Start a builder with a shape-aware tile selected for the fastest known path.
    #[must_use]
    pub fn auto(a: TensorRef, b: TensorRef, out: TensorRef) -> Self {
        Self {
            core: MatmulTiledCore::plain_auto(a, b, out),
        }
    }

    impl_common_builder_controls!(MatmulTiled);
}

/// Typed Cat-A builder for [`matmul_bias_tiled`].
#[derive(Debug, Clone)]
pub struct MatmulBiasTiled {
    core: MatmulTiledCore,
}

impl MatmulBiasTiled {
    /// Start a builder. `tile` splits the k axis for register-reuse.
    #[must_use]
    pub fn new(a: TensorRef, b: TensorRef, bias: TensorRef, out: TensorRef, tile: u32) -> Self {
        Self {
            core: MatmulTiledCore::bias(a, b, bias, out, tile),
        }
    }

    /// Start a builder with a shape-aware tile selected for the fastest known path.
    #[must_use]
    pub fn auto(a: TensorRef, b: TensorRef, bias: TensorRef, out: TensorRef) -> Self {
        Self {
            core: MatmulTiledCore::bias_auto(a, b, bias, out),
        }
    }

    impl_common_builder_controls!(MatmulBiasTiled);
}

/// Back-compat wrapper; returns an invalid-output program on contract violation.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> Program {
    MatmulTiled::new(
        TensorRef::u32_2d(a, m, k),
        TensorRef::u32_2d(b, k, n),
        TensorRef::u32_2d(out, m, n),
        tile,
    )
    .build()
    .unwrap_or_else(|err| {
        crate::builder::invalid_output_program(OP_ID, out, DataType::U32, format!("Fix: {err}"))
    })
}

/// Back-compat wrapper; returns an invalid-output program on contract violation.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn matmul_bias_tiled(
    a: &str,
    b: &str,
    bias: &str,
    out: &str,
    m: u32,
    k: u32,
    n: u32,
    tile: u32,
) -> Program {
    MatmulBiasTiled::new(
        TensorRef::u32_2d(a, m, k),
        TensorRef::u32_2d(b, k, n),
        TensorRef::u32_1d(bias, n),
        TensorRef::u32_2d(out, m, n),
        tile,
    )
    .build()
    .unwrap_or_else(|err| {
        crate::builder::invalid_output_program(
            OP_ID_BIAS,
            out,
            DataType::U32,
            format!("Fix: {err}"),
        )
    })
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::matmul_tiled",
        build: || matmul_tiled("a", "b", "out", 2, 2, 2, 2),
        test_inputs: Some(|| {
            vec![vec![
                crate::test_support::byte_pack::u32_bytes(&[1, 2, 3, 4]),
                crate::test_support::byte_pack::u32_bytes(&[5, 6, 7, 8]),
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![crate::test_support::byte_pack::u32_bytes(&[19, 22, 43, 50])]]
        }),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::math::matmul_bias_tiled",
        build: || matmul_bias_tiled("a", "b", "bias", "out", 2, 2, 2, 2),
        test_inputs: Some(|| {
            vec![vec![
                crate::test_support::byte_pack::u32_bytes(&[1, 2, 3, 4]),
                crate::test_support::byte_pack::u32_bytes(&[5, 6, 7, 8]),
                crate::test_support::byte_pack::u32_bytes(&[10, 20]),
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![crate::test_support::byte_pack::u32_bytes(&[29, 42, 53, 70])]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::bytes_to_u32 as decode_u32_words;
    use vyre_reference::value::Value;

    fn output_zero_bytes(program: &Program) -> Vec<u8> {
        let output = program
            .buffers()
            .iter()
            .find(|buffer| buffer.is_output())
            .expect("Fix: tiled matmul test program must declare an output buffer.");
        vec![0u8; (output.count() as usize) * core::mem::size_of::<u32>()]
    }

    fn run_program(program: &Program, inputs: Vec<Vec<u8>>) -> Vec<u32> {
        let values = inputs.into_iter().map(Value::from).collect::<Vec<_>>();
        let outputs = vyre_reference::reference_eval(program, &values)
            .expect("Fix: tiled matmul must execute in the reference interpreter.");
        decode_u32_words(&outputs[0].to_bytes())
    }

    fn expected_matmul(
        a: &[u32],
        b: &[u32],
        bias: Option<&[u32]>,
        m: u32,
        k: u32,
        n: u32,
    ) -> Vec<u32> {
        let mut out = Vec::with_capacity((m * n) as usize);
        for row in 0..m {
            for col in 0..n {
                let mut acc = bias.map_or(0, |values| values[col as usize]);
                for kk in 0..k {
                    let av = a[(row * k + kk) as usize];
                    let bv = b[(kk * n + col) as usize];
                    acc = acc.wrapping_add(av.wrapping_mul(bv));
                }
                out.push(acc);
            }
        }
        out
    }

    fn pseudo_random_words(count: usize, seed: &mut u32) -> Vec<u32> {
        (0..count)
            .map(|_| {
                *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *seed
            })
            .collect()
    }

    fn assert_mma_coordinates(program: &Program, label: &str) {
        let debug = format!("{:?}", program.entry());
        assert!(
            debug.contains("tile_row_base"),
            "{label} MMA tiled matmul must include workgroup row offset in output coordinates"
        );
        assert!(
            debug.contains("tile_col_base"),
            "{label} MMA tiled matmul must include workgroup column offset in output coordinates"
        );
        assert_eq!(
            program.workgroup_size(),
            [32, 1, 1],
            "{label} MMA tiled matmul must force the 32-lane M16N8K16 workgroup"
        );
    }

    fn assert_not_mma_path(program: &Program, label: &str) {
        let debug = format!("{:?}", program.entry());
        assert!(
            !debug.contains("mma_c0"),
            "{label} must stay on cooperative tiled matmul when K is not M16N8K16-aligned"
        );
        assert_ne!(
            program.workgroup_size(),
            [32, 1, 1],
            "{label} must not force the tensor-core workgroup on a non-K16 shape"
        );
    }

    #[test]
    fn matmul_tiled_rejects_zero_tile_without_panic() {
        let error = MatmulTiled::new(
            TensorRef::u32_2d("a", 2, 2),
            TensorRef::u32_2d("b", 2, 2),
            TensorRef::u32_2d("out", 2, 2),
            0,
        )
        .build()
        .expect_err("zero tile must be a build error");

        assert!(
            error.to_string().contains("tile"),
            "zero-tile error must identify the invalid tile dimension: {error}"
        );
    }

    #[test]
    fn matmul_bias_tiled_rejects_zero_tile_without_panic() {
        let error = MatmulBiasTiled::new(
            TensorRef::u32_2d("a", 2, 2),
            TensorRef::u32_2d("b", 2, 2),
            TensorRef::u32_1d("bias", 2),
            TensorRef::u32_2d("out", 2, 2),
            0,
        )
        .build()
        .expect_err("zero tile must be a build error");

        assert!(
            error.to_string().contains("tile"),
            "zero-tile bias error must identify the invalid tile dimension: {error}"
        );
    }

    #[test]
    fn cooperative_matmul_tiled_matches_reference_on_edge_tiles() {
        let (m, k, n, tile) = (17_u32, 19_u32, 13_u32, 8_u32);
        let mut seed = 0xA5A5_0131;
        let a = pseudo_random_words((m * k) as usize, &mut seed);
        let b = pseudo_random_words((k * n) as usize, &mut seed);
        let program = MatmulTiled::new(
            TensorRef::u32_2d("a", m, k),
            TensorRef::u32_2d("b", k, n),
            TensorRef::u32_2d("out", m, n),
            tile,
        )
        .with_workgroup_size([8, 8, 1])
        .build()
        .expect("Fix: edge-tiled matmul dimensions are valid.");

        let actual = run_program(
            &program,
            vec![
                crate::test_support::byte_pack::u32_bytes(&a),
                crate::test_support::byte_pack::u32_bytes(&b),
                output_zero_bytes(&program),
            ],
        );
        let expected = expected_matmul(&a, &b, None, m, k, n);
        assert_eq!(&actual[..expected.len()], expected.as_slice());
    }

    #[test]
    fn cooperative_matmul_bias_tiled_matches_reference_on_edge_tiles() {
        let (m, k, n, tile) = (17_u32, 19_u32, 13_u32, 8_u32);
        let mut seed = 0x5A5A_0717;
        let a = pseudo_random_words((m * k) as usize, &mut seed);
        let b = pseudo_random_words((k * n) as usize, &mut seed);
        let bias = pseudo_random_words(n as usize, &mut seed);
        let program = MatmulBiasTiled::new(
            TensorRef::u32_2d("a", m, k),
            TensorRef::u32_2d("b", k, n),
            TensorRef::u32_1d("bias", n),
            TensorRef::u32_2d("out", m, n),
            tile,
        )
        .with_workgroup_size([8, 8, 1])
        .build()
        .expect("Fix: edge-tiled matmul+bias dimensions are valid.");

        let actual = run_program(
            &program,
            vec![
                crate::test_support::byte_pack::u32_bytes(&a),
                crate::test_support::byte_pack::u32_bytes(&b),
                crate::test_support::byte_pack::u32_bytes(&bias),
                output_zero_bytes(&program),
            ],
        );
        let expected = expected_matmul(&a, &b, Some(&bias), m, k, n);
        assert_eq!(&actual[..expected.len()], expected.as_slice());
    }

    #[test]
    fn matmul_tiled_mma_path_uses_workgroup_tile_coordinates() {
        let program = MatmulTiled::new(
            TensorRef::f16_2d("a", 32, 16),
            TensorRef::f16_2d("b", 16, 16),
            TensorRef::f16_2d("out", 32, 16),
            16,
        )
        .build()
        .expect("Fix: F16 M16N8K16 tiled matmul dimensions are valid.");

        assert_mma_coordinates(&program, "plain");
    }

    #[test]
    fn matmul_bias_tiled_mma_path_uses_workgroup_tile_coordinates() {
        let program = MatmulBiasTiled::new(
            TensorRef::f16_2d("a", 32, 16),
            TensorRef::f16_2d("b", 16, 16),
            TensorRef::f16_1d("bias", 16),
            TensorRef::f16_2d("out", 32, 16),
            16,
        )
        .build()
        .expect("Fix: F16 M16N8K16 bias tiled matmul dimensions are valid.");

        assert_mma_coordinates(&program, "bias");
    }

    #[test]
    fn auto_tile_selects_mma_path_when_shape_can_use_tensor_cores() {
        let program = MatmulTiled::auto(
            TensorRef::f16_2d("a", 32, 16),
            TensorRef::f16_2d("b", 16, 16),
            TensorRef::f16_2d("out", 32, 16),
        )
        .build()
        .expect("Fix: auto-tiled F16 tensor-core shape is valid.");

        assert_mma_coordinates(&program, "auto");
    }

    #[test]
    fn auto_tile_does_not_promote_f16_non_k16_shapes_to_tensor_cores() {
        let program = MatmulTiled::auto(
            TensorRef::f16_2d("a", 32, 17),
            TensorRef::f16_2d("b", 17, 16),
            TensorRef::f16_2d("out", 32, 16),
        )
        .build()
        .expect("Fix: non-K16 F16 shape is valid but must not use M16N8K16 tensor cores.");

        assert_not_mma_path(&program, "auto non-K16");
    }

    #[test]
    fn generated_tiled_matmul_shape_tile_matrix_builds_consistently() {
        let mut cases = 0usize;
        for m in [1, 2, 3, 7, 16, 17, 31, 32] {
            for k in [1, 2, 5, 8, 16, 19, 31] {
                for n in [1, 2, 7, 8, 13, 16, 17] {
                    for tile in [1, 2, 4, 8, 16] {
                        let plain = MatmulTiled::new(
                            TensorRef::u32_2d("a", m, k),
                            TensorRef::u32_2d("b", k, n),
                            TensorRef::u32_2d("out", m, n),
                            tile,
                        )
                        .build()
                        .expect("Fix: generated plain tiled matmul shape must build.");
                        assert!(
                            plain.buffers().iter().any(|buffer| buffer.is_output()),
                            "plain generated case must declare an output buffer"
                        );
                        cases += 1;

                        let bias = MatmulBiasTiled::new(
                            TensorRef::u32_2d("a", m, k),
                            TensorRef::u32_2d("b", k, n),
                            TensorRef::u32_1d("bias", n),
                            TensorRef::u32_2d("out", m, n),
                            tile,
                        )
                        .build()
                        .expect("Fix: generated bias tiled matmul shape must build.");
                        assert!(
                            bias.buffers().iter().any(|buffer| buffer.is_output()),
                            "bias generated case must declare an output buffer"
                        );
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 3_920);
    }

    #[test]
    fn generated_tiled_matmul_invalid_shapes_reject_precisely() {
        for n in [1, 2, 8, 16] {
            let bad_b = MatmulTiled::new(
                TensorRef::u32_2d("a", 4, 3),
                TensorRef::u32_2d("b", 4, n),
                TensorRef::u32_2d("out", 4, n),
                2,
            )
            .build()
            .expect_err("mismatched k dimension must be rejected");
            assert!(bad_b.to_string().contains("b"), "{bad_b}");

            let bad_bias = MatmulBiasTiled::new(
                TensorRef::u32_2d("a", 4, 3),
                TensorRef::u32_2d("b", 3, n),
                TensorRef::u32_1d("bias", n + 1),
                TensorRef::u32_2d("out", 4, n),
                2,
            )
            .build()
            .expect_err("mismatched bias dimension must be rejected");
            assert!(bad_bias.to_string().contains("bias"), "{bad_bias}");

            let bad_out = MatmulBiasTiled::new(
                TensorRef::u32_2d("a", 4, 3),
                TensorRef::u32_2d("b", 3, n),
                TensorRef::u32_1d("bias", n),
                TensorRef::u32_2d("out", 5, n),
                2,
            )
            .build()
            .expect_err("mismatched output dimension must be rejected");
            assert!(bad_out.to_string().contains("out"), "{bad_out}");
        }
    }
}
