use vyre_foundation::ir::Expr;

/// Preserve finite values and replace non-finite values with `replacement`.
#[must_use]
pub fn finite_or(value: Expr, replacement: Expr) -> Expr {
    Expr::select(Expr::is_finite(value.clone()), value, replacement)
}

/// Clamp non-positive or non-finite scale-like values to `f32::MIN_POSITIVE`.
#[must_use]
pub fn positive_finite_or_min(value: Expr) -> Expr {
    Expr::select(
        Expr::and(
            Expr::is_finite(value.clone()),
            Expr::gt(value.clone(), Expr::f32(f32::MIN_POSITIVE)),
        ),
        value,
        Expr::f32(f32::MIN_POSITIVE),
    )
}

/// Normalize subnormal results to zero so CPU and GPU backends agree.
#[must_use]
pub fn flush_tiny(value: Expr) -> Expr {
    Expr::select(
        Expr::le(Expr::abs(value.clone()), Expr::f32(f32::MIN_POSITIVE)),
        Expr::f32(0.0),
        value,
    )
}
