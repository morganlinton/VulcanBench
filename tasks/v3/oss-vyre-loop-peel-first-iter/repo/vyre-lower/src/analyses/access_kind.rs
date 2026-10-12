//! Shared memory-access direction used by substrate-neutral analyses and emit patterns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessKind {
    Load,
    Store,
}
