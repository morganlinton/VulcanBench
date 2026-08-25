//! Compatibility metadata shared with the wgpu trap readback path.

use std::sync::Arc;

pub const TRAP_SIDECAR_NAME: &str = "__vyre_naga_trap_sidecar";
pub const TRAP_SIDECAR_WORDS: u32 = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrapTag {
    pub code: u32,
    pub tag: Arc<str>,
}
