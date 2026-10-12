//! Const-fold tests  -  split per audit cleanup A13 (2026-04-30) so no
//! single test file exceeds the 1000-LOC hygiene cap.

use super::super::*;
use crate::ir::Expr;

// ──── binop_identities: comparison self ────────────────────

mod binop_identity_part1 {

    include!("__split/binop_identity_part1.rs");
}
mod binop_identity_part2 {
    include!("__split/binop_identity_part2.rs");
}
