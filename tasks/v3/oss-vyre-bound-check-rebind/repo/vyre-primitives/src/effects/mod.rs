//! Effects-typed pipeline primitives (P-1.0-V1.x).
//!
//! Effects rows track which side-effect kinds (memory writes, atomic
//! ops, host I/O, GPU dispatch) a Region produces. Handlers consume
//! a row and produce a residual row representing what's left after
//! the handler has discharged its effects.
//!
//! The substrate is pure-data: an `EffectRow` is a u32 bitmask
//! indexed by `EffectKind`. `handler_apply` removes the handled
//! effects from a row; `handler_compose` (V1.2) builds a single
//! handler from two.
//!
//! No IR-builder dependencies  -  this layer is consumable from
//! anywhere in the workspace.

pub mod handler_apply;
pub mod handler_compose;
pub mod type_checker;
pub use handler_apply::{handler_apply, EffectKind, EffectRow, Handler};
pub use handler_compose::handler_compose;
pub use type_checker::{check_effect_row, fits_signature, EffectTypeError};
