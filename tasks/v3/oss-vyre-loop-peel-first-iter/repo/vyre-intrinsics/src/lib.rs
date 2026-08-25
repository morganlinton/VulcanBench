#![forbid(unsafe_code)]
#![allow(
    clippy::ptr_arg,
    clippy::should_implement_trait,
    clippy::module_inception
)]
//! # vyre-intrinsics  -  Cat-C hardware intrinsics
//!
//! This crate holds every vyre op that requires dedicated backend
//! and reference-interpreter support  -  the ones that *cannot* be
//! written as `fn(...) -> Program` over existing `vyre::ir::*`
//! primitives. If an op can be expressed purely as a composition of
//! existing `Expr`/`Node` variants, it belongs in `vyre-libs` or a
//! user package, NOT here.
//!
//! See `docs/migration-vyre-ops-to-intrinsics.md` for the
//! classification rule that drove the split.
//!
//! ## Current surface (9 intrinsics)
//!
//! - `subgroup_add`, `subgroup_ballot`, `subgroup_shuffle`  -  wave-level
//!   ops backed by target builder 25+ subgroup lowering.
//! - `workgroup_barrier`, `storage_barrier`  -  concurrency fences.
//! - `bit_reverse_u32`, `popcount_u32`  -  bit intrinsics mapping 1:1
//!   to hardware instructions (`reverseBits`, `countOneBits`).
//! - `fma_f32`  -  fused multiply-add (byte-identical to `f32::mul_add`).
//! - `inverse_sqrt_f32`  -  maps to hardware `inverseSqrt()` via target builder.
//!
//! Everything else that used to live here (atomics, lzcnt/tzcnt,
//! clamp_u32, hashes) moved to `vyre-libs` in Migration 2–3.

/// Region builder  -  the composition-chain wrap helper mandatory at
/// every tier. Spec: `docs/region-chain.md`.
pub mod region;

/// Inventory-backed OpEntry registry used by the intrinsic-differential
/// harness.
#[doc(hidden)]
pub mod harness;

/// CPU reference evaluation traits re-exported from `vyre-foundation`.
#[allow(deprecated)]
pub use vyre_foundation::cpu_op::{self, structured_intrinsic_cpu, CategoryAOp, CpuOp};
/// Spec types every intrinsic registers against: algebraic laws, backend identifiers, intrinsic descriptors.
pub use vyre_spec::{AlgebraicLaw, Backend, BackendId, CpuFn, IntrinsicDescriptor};

/// Category-classification consistency gate (F-IR-34).
pub mod category_check;

/// Category C hardware intrinsics  -  subgroup collectives, barriers, bit intrinsics, FMA, inverseSqrt.
#[cfg(feature = "hardware")]
pub mod hardware;
