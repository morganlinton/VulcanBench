//! Linear builder + the canonical `linear()` Cat-A constructor.

use vyre::ir::{DataType, Program};

use crate::{
    builder::{check_tensors, BuildOptions},
    region::tag_program,
    tensor_ref::{TensorRef, TensorRefError},
    MatmulBias,
};

use super::tiled::{linear_tiled, LINEAR_TILED_MIN_WORK, LINEAR_TILED_TILE};

pub(super) const LINEAR_OP_ID: &str = "vyre-libs::nn::linear";
/// Typed Cat-A builder for [`linear`].
#[derive(Debug, Clone)]
pub struct Linear {
    x: TensorRef,
    w: TensorRef,
    b: TensorRef,
    out: TensorRef,
    options: BuildOptions,
}

impl Linear {
    /// Create a builder for `out[i] = sum_k x[k] * w[k, i] + b[i]`.
    #[must_use]
    pub fn new(x: TensorRef, w: TensorRef, b: TensorRef, out: TensorRef) -> Self {
        Self {
            x,
            w,
            b,
            out,
            options: BuildOptions::default(),
        }
    }

    /// Validate tensor metadata and materialize the linear Program.
    ///
    /// # Errors
    ///
    /// Returns [`TensorRefError`] when dtypes, names, shapes, or dimensions
    /// violate the linear-layer contract.
    pub fn build(self) -> Result<Program, TensorRefError> {
        check_tensors(
            LINEAR_OP_ID,
            &[
                (&self.x, DataType::U32),
                (&self.w, DataType::U32),
                (&self.b, DataType::U32),
                (&self.out, DataType::U32),
            ],
        )?;
        let x_shape = self.x.shape.as_ref();
        let w_shape = self.w.shape.as_ref();
        let b_shape = self.b.shape.as_ref();
        let out_shape = self.out.shape.as_ref();
        let expected_w = match x_shape {
            [in_dim] => match out_shape {
                [out_dim] => vec![*in_dim, *out_dim],
                _ => vec![],
            },
            _ => vec![],
        };
        if w_shape != expected_w.as_slice() {
            return Err(TensorRefError::ShapeMismatch {
                name: self.w.name_str().to_string(),
                found: self.w.shape.to_vec(),
                expected: expected_w,
                op: LINEAR_OP_ID,
            });
        }
        if b_shape != out_shape {
            return Err(TensorRefError::ShapeMismatch {
                name: self.b.name_str().to_string(),
                found: self.b.shape.to_vec(),
                expected: self.out.shape.to_vec(),
                op: LINEAR_OP_ID,
            });
        }
        let &[in_dim] = x_shape else {
            return Err(TensorRefError::ShapeMismatch {
                name: self.x.name_str().to_string(),
                found: self.x.shape.to_vec(),
                expected: vec![1],
                op: LINEAR_OP_ID,
            });
        };
        let &[out_dim] = out_shape else {
            return Err(TensorRefError::ShapeMismatch {
                name: self.out.name_str().to_string(),
                found: self.out.shape.to_vec(),
                expected: vec![1],
                op: LINEAR_OP_ID,
            });
        };
        if in_dim == 0 {
            return Err(TensorRefError::ShapeMismatch {
                name: self.x.name_str().to_string(),
                found: self.x.shape.to_vec(),
                expected: vec![1],
                op: LINEAR_OP_ID,
            });
        }
        if out_dim == 0 {
            return Err(TensorRefError::ShapeMismatch {
                name: self.out.name_str().to_string(),
                found: self.out.shape.to_vec(),
                expected: vec![1],
                op: LINEAR_OP_ID,
            });
        }
        build_linear_program(
            self.x.name_str(),
            self.w.name_str(),
            self.b.name_str(),
            self.out.name_str(),
            in_dim,
            out_dim,
            self.options,
        )
        .map_err(|_| TensorRefError::ElementCountOverflow {
            name: self.w.name_str().to_string(),
            shape: self.w.shape.to_vec(),
        })
    }
}

crate::builder::impl_cat_a_builder_options!(Linear);

/// Build a Program that computes `out[i] = sum_k x[k] * w[k, i] + b[i]`.
///
/// Shapes: `x: [in_dim]`, `w: [in_dim, out_dim]`, `b: [out_dim]`,
/// `out: [out_dim]`. Workgroup `[64, 1, 1]`  -  each invocation handles
/// one output index.
///
/// # Errors
/// Returns `Err` when `in_dim == 0` (FINDING-V7-TEST-010-LINEAR).
pub fn linear(
    x: &str,
    w: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
) -> Result<Program, String> {
    if in_dim
        .checked_mul(out_dim)
        .is_some_and(|work| work >= LINEAR_TILED_MIN_WORK)
    {
        return linear_tiled(x, w, b, out, in_dim, out_dim, LINEAR_TILED_TILE);
    }

    Linear::new(
        TensorRef::u32_1d(x, in_dim),
        TensorRef::u32_2d(w, in_dim, out_dim),
        TensorRef::u32_1d(b, out_dim),
        TensorRef::u32_1d(out, out_dim),
    )
    .build()
    .map_err(|error| format!("Fix: {LINEAR_OP_ID} build failed: {error}"))
}

fn build_linear_program(
    x: &str,
    w: &str,
    b: &str,
    out: &str,
    in_dim: u32,
    out_dim: u32,
    options: BuildOptions,
) -> Result<Program, String> {
    in_dim.checked_mul(out_dim).ok_or_else(|| {
        "Fix: linear in_dim*out_dim overflows u32; reduce dimensions.".to_string()
    })?;
    let mut builder = MatmulBias::new(
        TensorRef::u32_2d(x, 1, in_dim),
        TensorRef::u32_2d(w, in_dim, out_dim),
        TensorRef::u32_1d(b, out_dim),
        TensorRef::u32_2d(out, 1, out_dim),
    );
    if let Some(workgroup_size) = options.workgroup_size {
        builder = builder.with_workgroup_size(workgroup_size);
    }
    let program = builder
        .build()
        .map_err(|error| format!("Fix: linear matmul_bias build failed: {error}"))?;
    Ok(tag_program(
        options.region_generator.unwrap_or(LINEAR_OP_ID),
        program,
    ))
}
