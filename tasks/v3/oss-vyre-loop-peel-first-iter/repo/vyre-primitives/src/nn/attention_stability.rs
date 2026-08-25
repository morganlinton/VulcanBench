use vyre_foundation::ir::Expr;

pub use super::f32_stability::{finite_or, flush_tiny};

/// Bound an attention score before it enters a softmax recurrence.
///
/// Finite-but-overflowed dot products become `-80.0` so `inf - inf`
/// never poisons a row, while explicit NaN inputs continue to propagate.
#[must_use]
pub fn bounded_score(value: Expr) -> Expr {
    Expr::select(
        Expr::is_nan(value.clone()),
        value.clone(),
        finite_or(value, Expr::f32(-80.0)),
    )
}

/// Clamp `exp` arguments to the stable attention range `[-80, 0]`.
#[must_use]
pub fn bounded_exp_arg(value: Expr) -> Expr {
    let value_is_nan = Expr::is_nan(value.clone());
    let finite = finite_or(value.clone(), Expr::f32(-80.0));
    let upper_bounded = Expr::select(
        Expr::gt(finite.clone(), Expr::f32(0.0)),
        Expr::f32(0.0),
        finite,
    );
    let clamped = Expr::select(
        Expr::lt(upper_bounded.clone(), Expr::f32(-80.0)),
        Expr::f32(-80.0),
        upper_bounded,
    );
    Expr::select(value_is_nan, value, clamped)
}

/// Keep a denominator positive without hiding NaN evidence.
#[must_use]
pub fn positive_denominator(value: Expr) -> Expr {
    let repaired = Expr::select(
        Expr::and(
            Expr::is_finite(value.clone()),
            Expr::gt(value.clone(), Expr::f32(f32::MIN_POSITIVE)),
        ),
        value.clone(),
        Expr::f32(f32::MIN_POSITIVE),
    );
    Expr::select(Expr::is_nan(value.clone()), value, repaired)
}
