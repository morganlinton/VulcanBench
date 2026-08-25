use crate::validate::{err, ValidationError};

/// Default maximum nested operation-call depth accepted by validation.
pub const DEFAULT_MAX_CALL_DEPTH: usize = 32;

/// Default maximum `If`/`Loop`/`Block` nesting accepted by validation.
pub const DEFAULT_MAX_NESTING_DEPTH: usize = 64;

/// Default maximum statement node count accepted by validation.
///
/// This is a megakernel substrate: the launch path fuses every rule in a
/// class into ONE program (see surge's `fuse::dispatch::build`), so the
/// statement-node ceiling must accommodate a fully fused bundle, not a
/// single hand-written kernel. The value mirrors the GPU-native limit
/// validator in `vyre-self-substrate`'s `optimizer::validate_via_encoded`
/// (`DEFAULT_MAX_NODE_COUNT = 100_000`), which is exercised at that scale by
/// the substrate's corpus-parity integration evidence. The two MUST agree:
/// a program the encoded validator blesses must not be rejected here, or
/// fused bundles between the two ceilings pass GPU validation and fail CPU
/// validation. Above this ceiling, V019's advice still holds — split into
/// smaller kernels or run an optimization pass before lowering.
pub const DEFAULT_MAX_NODE_COUNT: usize = 100_000;

/// Default maximum expression nesting accepted by validation.
pub const DEFAULT_MAX_EXPR_DEPTH: usize = 128;

/// Mutable state used while checking program size and nesting limits.
#[derive(Debug, Default)]
pub struct LimitState {
    /// Number of statement nodes visited so far.
    pub node_count: usize,
    /// Whether the nesting depth error has already been reported.
    pub nesting_reported: bool,
    /// Whether the node count error has already been reported.
    pub node_count_reported: bool,
}

/// Increment `limits` and emit errors if depth or node count exceeds defaults.
#[inline]
pub fn check_limits(limits: &mut LimitState, depth: usize, errors: &mut Vec<ValidationError>) {
    limits.node_count = limits.node_count.saturating_add(1);
    if limits.node_count > DEFAULT_MAX_NODE_COUNT && !limits.node_count_reported {
        limits.node_count_reported = true;
        errors.push(err(format!(
            "V019: program node count exceeds limit {DEFAULT_MAX_NODE_COUNT}. Fix: split the program into smaller kernels or run an optimization pass before lowering."
        )));
    }
    if depth > DEFAULT_MAX_NESTING_DEPTH && !limits.nesting_reported {
        limits.nesting_reported = true;
        errors.push(err(format!(
            "V018: program nesting depth {depth} exceeds max {DEFAULT_MAX_NESTING_DEPTH}. Fix: flatten nested If/Loop/Block structures or split the program before lowering."
        )));
    }
}

/// Return true when the expression nesting depth is still within bounds.
#[inline]
#[must_use]
pub fn check_expr_depth(depth: usize, errors: &mut Vec<ValidationError>) -> bool {
    if depth > DEFAULT_MAX_EXPR_DEPTH {
        errors.push(err(format!(
            "V033: expression nesting depth {depth} exceeds max {DEFAULT_MAX_EXPR_DEPTH}. Fix: split the expression into intermediate let-bindings before lowering."
        )));
        return false;
    }
    true
}

/// Compute the maximum call depth reachable from `op_id`.
///
/// Returns `Ok(max_depth)` when within [`DEFAULT_MAX_CALL_DEPTH`], or
/// `Err(depth)` if the limit is exceeded.
///
/// # Errors
///
/// Returns the offending `depth` when it exceeds [`DEFAULT_MAX_CALL_DEPTH`].
#[inline]
#[must_use]
pub fn max_call_depth(_op_id: &str, depth: usize) -> Result<usize, usize> {
    if depth > DEFAULT_MAX_CALL_DEPTH {
        return Err(depth);
    }
    // Foundation does not own the dialect registry, so it cannot walk an
    // operation's callee graph on its own. Driver-level callers either pass
    // an already-inlined program (no Expr::Call nodes remain, so this
    // function is never invoked) or run their own registry-aware traversal
    // before validation. See `vyre-driver::pipeline::compile` for the full
    // call-depth walk that uses the DialectRegistry.
    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_limits_within_bounds_no_error() {
        let mut limits = LimitState::default();
        let mut errors = Vec::new();
        check_limits(&mut limits, 5, &mut errors);
        assert!(errors.is_empty());
        assert_eq!(limits.node_count, 1);
    }

    #[test]
    fn check_limits_nesting_depth_overflow() {
        let mut limits = LimitState::default();
        let mut errors = Vec::new();
        check_limits(&mut limits, DEFAULT_MAX_NESTING_DEPTH + 1, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("V018"));
    }

    #[test]
    fn check_limits_nesting_only_reports_once() {
        let mut limits = LimitState::default();
        let mut errors = Vec::new();
        check_limits(&mut limits, DEFAULT_MAX_NESTING_DEPTH + 1, &mut errors);
        check_limits(&mut limits, DEFAULT_MAX_NESTING_DEPTH + 2, &mut errors);
        assert_eq!(errors.len(), 1, "nesting should only be reported once");
    }

    #[test]
    fn check_expr_depth_within_bounds() {
        let mut errors = Vec::new();
        assert!(check_expr_depth(100, &mut errors));
        assert!(errors.is_empty());
    }

    #[test]
    fn check_expr_depth_overflow() {
        let mut errors = Vec::new();
        assert!(!check_expr_depth(DEFAULT_MAX_EXPR_DEPTH + 1, &mut errors));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("V033"));
    }

    #[test]
    fn max_call_depth_within_bounds() {
        assert_eq!(max_call_depth("my_op", 10), Ok(10));
    }

    #[test]
    fn max_call_depth_exceeded() {
        let limit = max_call_depth("my_op", DEFAULT_MAX_CALL_DEPTH + 1)
            .expect_err("depth over limit must fail");
        // Contract (see `max_call_depth` docs): Err carries the OFFENDING
        // depth, not the limit.
        assert_eq!(limit, DEFAULT_MAX_CALL_DEPTH + 1);
    }
}
