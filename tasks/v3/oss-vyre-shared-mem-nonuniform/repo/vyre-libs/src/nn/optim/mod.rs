//! Optimizer sub-dialect for Parameter Golf recipe (all F32).
//!
//! MuonEq-R, AdamW, EMA, Newton-Schulz orthogonalization.
pub mod adamw_step;
pub mod ema_apply;
pub(crate) mod muon_core;
pub mod muon_update;
pub mod muoneq_r;
pub mod newton_schulz;

pub use adamw_step::adamw_step;
pub use ema_apply::ema_apply;
pub use muon_update::muon_update;
pub use muoneq_r::muoneq_r;
pub use newton_schulz::newton_schulz_5step;
