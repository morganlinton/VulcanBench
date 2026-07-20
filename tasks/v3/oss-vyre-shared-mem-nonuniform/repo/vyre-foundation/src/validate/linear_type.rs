//! Linear-type discipline checker (P-1.0-V2.2).
//!
//! Walks every `Node` and `Expr` in a `Program`, counts reads and
//! writes per buffer, and reports violations against each
//! `BufferDecl::linear_type()` declaration:
//!
//! * `Linear`      -  exactly one use; reject if `uses == 0` or `uses > 1`.
//! * `Affine`      -  at most one use; reject if `uses > 1`.
//! * `Relevant`    -  at least one use; reject if `uses == 0`.
//! * `Unrestricted`  -  anything (default).
//!
//! "Use" here means any reference: `Expr::Load`, `Expr::BufLen`,
//! `Expr::Atomic`, `Node::Store`, `Node::AsyncLoad`, `Node::AsyncStore`,
//! `Node::IndirectDispatch`. The checker is conservative: it counts
//! every occurrence in source order, so a buffer appearing inside an
//! `If::then` *and* `If::otherwise` is two uses even though only one
//! path runs at dispatch time.
//!
//! Wired into `validate::validate` so backends never see a program
//! that violates declared discipline.

use crate::ir_inner::model::program::{BufferDecl, LinearType, Program};
use crate::validate::{err, ValidationError};

/// Walk `program` and return a list of validation errors describing
/// every buffer whose declared `linear_type` is violated.
#[must_use]
pub fn check_linear_types(program: &Program) -> Vec<ValidationError> {
    // Skip the fact derivation when no buffer has a non-default linear type
    // (the common case  -  most kernels do not opt into the linearity gate).
    if program
        .buffers()
        .iter()
        .all(|b| b.linear_type() == LinearType::Unrestricted)
    {
        return Vec::new();
    }

    // ProgramFacts records every buffer touch (Read / Write / Atomic /
    // AsyncSource / AsyncDestination / IndirectCount) in one cached
    // single-pass walk. `buffer_refs_of(name).len()` is the same use-count
    // the dedicated NodeVisitor + ExprVisitor pair built up  -  no second
    // full traversal needed.
    let facts = crate::optimizer::program_soa::ProgramFacts::build_cached(program);
    let mut errors = Vec::new();
    for buffer in program.buffers() {
        let lt = buffer.linear_type();
        if lt == LinearType::Unrestricted {
            continue;
        }
        let uses = u32::try_from(facts.buffer_refs_of(buffer.name()).len()).unwrap_or(u32::MAX);
        if let Some(message) = violation_message(buffer, lt, uses) {
            errors.push(err(message));
        }
    }
    errors
}

fn violation_message(buffer: &BufferDecl, lt: LinearType, uses: u32) -> Option<String> {
    match lt {
        LinearType::Linear => {
            if uses == 1 {
                None
            } else {
                Some(format!(
                    "buffer `{}` declared `LinearType::Linear` must be used exactly once but was used {uses} time(s). Fix: ensure the program reads or writes this buffer exactly once on every path, or change the discipline to Affine / Relevant / Unrestricted.",
                    buffer.name()
                ))
            }
        }
        LinearType::Affine => {
            if uses > 1 {
                Some(format!(
                    "buffer `{}` declared `LinearType::Affine` must be used at most once but was used {uses} time(s). Fix: drop the redundant references, or change the discipline to Relevant / Unrestricted to allow re-use.",
                    buffer.name()
                ))
            } else {
                None
            }
        }
        LinearType::Relevant => {
            if uses == 0 {
                Some(format!(
                    "buffer `{}` declared `LinearType::Relevant` must be used at least once but was unused. Fix: add a read or write of this buffer, or change the discipline to Affine / Unrestricted.",
                    buffer.name()
                ))
            } else {
                None
            }
        }
        LinearType::Unrestricted => None,
    }
}
