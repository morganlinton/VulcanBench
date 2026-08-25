//! Linear-algebra sub-dialect: dot product, matmul, tiled matmul,
//! Strassen base case.
mod dot;
mod matmul;
mod matmul_strassen;
pub(crate) mod matmul_tiled;

pub use dot::{dot, Dot};
pub use matmul::{matmul, matmul_bias, Matmul, MatmulBias};
pub use matmul_strassen::{matmul_strassen_2x2, matmul_strassen_one_level};

// H1 Strassen base case re-export at the math root for parity with
// existing dot/matmul exports.
pub use matmul_tiled::{matmul_bias_tiled, matmul_tiled, MatmulBiasTiled, MatmulTiled};
#[cfg(feature = "nn-linear-4bit")]
pub(crate) use matmul_tiled::{
    plan_matmul_kernel, F32MatmulMode, MatmulFallbackReason, MatmulKernelCapabilities,
    MatmulKernelPath, MatmulKernelPlan, MatrixShape,
};
