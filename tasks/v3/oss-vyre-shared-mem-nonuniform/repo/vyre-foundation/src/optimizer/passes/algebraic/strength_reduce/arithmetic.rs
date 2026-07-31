use crate::ir::{BinOp, Expr, UnOp};
use crate::optimizer::passes::algebraic::const_fold::is_float_expr;

const MAX_SHIFT_ADD_CHAIN_COST: u32 = 4;

/// Maximum polynomial degree the Horner rewrite will fold. Real GPU kernels
/// evaluate low-degree polynomials; the cap bounds the emitted multiply chain.
const MAX_HORNER_DEGREE: u32 = 16;

/// Integer literal domain a polynomial folds in. u32 and i32 share
/// bit-identical wrapping add/mul, so the fold runs in u32 bit-space and only
/// the emitted literal constructor differs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IntKind {
    U32,
    I32,
}

impl IntKind {
    fn lit(self, bits: u32) -> Expr {
        match self {
            IntKind::U32 => Expr::u32(bits),
            IntKind::I32 => Expr::i32(bits as i32),
        }
    }
}

/// Rewrite an expanded single-variable integer polynomial into Horner form:
/// `a*x^n + ... + b*x + c` -> `(((a)*x + ...)*x + b)*x + c`.
///
/// Integer arithmetic in Vyre is two's-complement wrapping, so associativity
/// and distributivity hold modulo 2^32 for both u32 and i32. The fold runs in
/// u32 bit-space (identical wrapping for either domain) and emits literals in
/// whichever domain the polynomial's explicit literals established. It rejects
/// multi-variable, duplicate-degree, non-monomial, mixed-domain, literal-free,
/// and floating-point polynomials (FP reassociation changes rounding).
pub(super) fn horner_polynomial_int(expr: &Expr) -> Option<Expr> {
    let mut terms = Vec::with_capacity(8);
    collect_add_terms(expr, &mut terms);
    if terms.len() < 2 {
        return None;
    }

    // coeffs[d] holds the coefficient bits of x^d once a term of that degree is seen.
    let mut coeffs: Vec<Option<u32>> = Vec::new();
    let mut var: Option<&crate::ir::Ident> = None;
    let mut kind: Option<IntKind> = None;

    for term in terms {
        let (coeff, degree, term_var, lit_kind) = monomial_int_term(term)?;
        if degree > MAX_HORNER_DEGREE {
            return None;
        }
        if let Some(name) = term_var {
            match var {
                Some(existing) if existing != name => return None,
                _ => var = Some(name),
            }
        }
        if let Some(k) = lit_kind {
            match kind {
                Some(existing) if existing != k => return None,
                _ => kind = Some(k),
            }
        }
        let idx = degree as usize;
        if idx >= coeffs.len() {
            coeffs.resize(idx + 1, None);
        }
        // Duplicate degree is ambiguous here; let const-fold / canonicalize
        // merge like terms first instead of guessing how to combine them.
        if coeffs[idx].replace(coeff).is_some() {
            return None;
        }
    }

    // The domain must be pinned by an explicit literal so emitted coefficients
    // match the operand's integer type; a literal-free polynomial (e.g.
    // `x*x + x`) is ambiguous and could mistype a non-default-domain operand.
    let kind = kind?;
    let max_degree = coeffs.len() - 1;
    if max_degree < 2 {
        return None;
    }
    let var = var?;
    let x = Expr::var(var.as_str());

    // Horner fold from the highest coefficient down. A zero/absent coefficient
    // skips the add, so sparse polynomials emit no `+ 0` churn.
    let mut acc = kind.lit(coeffs[max_degree].unwrap_or(0));
    for degree in (0..max_degree).rev() {
        acc = Expr::mul(acc, x.clone());
        let coeff = coeffs[degree].unwrap_or(0);
        if coeff != 0 {
            acc = Expr::add(acc, kind.lit(coeff));
        }
    }
    Some(acc)
}

/// Classify `expr` as a single monomial `coeff * x^degree` over at most one
/// variable. Returns `(coeff_bits, degree, var, lit_kind)`; `var == None` for a
/// bare constant and `lit_kind` names the integer domain of any explicit
/// literal (None when the term is literal-free). Coefficient arithmetic uses
/// u32 bit-space, which is wrapping-identical for u32 and i32. Any factor that
/// is not an integer literal or a variable rejects, as does mixing domains.
fn monomial_int_term(
    expr: &Expr,
) -> Option<(u32, u32, Option<&crate::ir::Ident>, Option<IntKind>)> {
    match expr {
        Expr::LitU32(value) => return Some((*value, 0, None, Some(IntKind::U32))),
        Expr::LitI32(value) => return Some((*value as u32, 0, None, Some(IntKind::I32))),
        Expr::Var(name) => return Some((1, 1, Some(name), None)),
        _ => {}
    }
    let mut factors = Vec::with_capacity(4);
    collect_mul_factors(expr, &mut factors);
    if factors.len() < 2 {
        return None;
    }
    let mut coeff: u32 = 1;
    let mut degree: u32 = 0;
    let mut var: Option<&crate::ir::Ident> = None;
    let mut kind: Option<IntKind> = None;
    for factor in factors {
        let factor_kind = match factor {
            Expr::LitU32(value) => {
                coeff = coeff.wrapping_mul(*value);
                IntKind::U32
            }
            Expr::LitI32(value) => {
                coeff = coeff.wrapping_mul(*value as u32);
                IntKind::I32
            }
            Expr::Var(name) => {
                match var {
                    Some(existing) if existing != name => return None,
                    _ => var = Some(name),
                }
                degree += 1;
                continue;
            }
            _ => return None,
        };
        match kind {
            Some(existing) if existing != factor_kind => return None,
            _ => kind = Some(factor_kind),
        }
    }
    Some((coeff, degree, var, kind))
}

fn collect_add_terms<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::BinOp {
        op: BinOp::Add,
        left,
        right,
    } = expr
    {
        collect_add_terms(left, out);
        collect_add_terms(right, out);
    } else {
        out.push(expr);
    }
}

fn collect_mul_factors<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::BinOp {
        op: BinOp::Mul,
        left,
        right,
    } = expr
    {
        collect_mul_factors(left, out);
        collect_mul_factors(right, out);
    } else {
        out.push(expr);
    }
}

/// Decompose `x * C` into shift-add/sub chains when `C = 2^hi +/- 2^lo`.
pub(super) fn shift_add_decompose(x: &Expr, constant: &Expr) -> Option<Expr> {
    let c = positive_u32_constant(constant)?;
    if c <= 1 || c.is_power_of_two() {
        return None;
    }
    if let Some(chain) = shift_add_chain(x, c) {
        return Some(chain);
    }
    for hi in (1u32..=16).rev() {
        let high = 1u32 << hi;
        if high > c {
            continue;
        }
        let remainder = c - high;
        if remainder == 0 {
            continue;
        }
        if remainder.is_power_of_two() {
            let lo = remainder.trailing_zeros();
            let lo_term = if lo == 0 {
                x.clone()
            } else {
                Expr::shl(x.clone(), Expr::u32(lo))
            };
            return Some(Expr::add(Expr::shl(x.clone(), Expr::u32(hi)), lo_term));
        }
    }
    for hi in (2u32..=16).rev() {
        let high = 1u32 << hi;
        if high <= c {
            continue;
        }
        let deficit = high - c;
        if deficit.is_power_of_two() && deficit < high {
            let lo = deficit.trailing_zeros();
            let lo_term = if lo == 0 {
                x.clone()
            } else {
                Expr::shl(x.clone(), Expr::u32(lo))
            };
            return Some(Expr::sub(Expr::shl(x.clone(), Expr::u32(hi)), lo_term));
        }
    }
    None
}

/// Build a bounded non-adjacent-form shift/add/sub chain for integer `x * c`.
///
/// The old recognizer only handled constants of the form `2^a +/- 2^b`.
/// NAF covers the dense stride constants seen in indexing code (`11`, `13`,
/// `21`, `27`, `31`) while the cost gate avoids replacing one multiply with a
/// longer ALU chain.
pub(super) fn shift_add_chain(x: &Expr, c: u32) -> Option<Expr> {
    if c <= 1 || c.is_power_of_two() || operand_duplication_cost(x) > 1 {
        return None;
    }

    let terms = naf_terms(c);
    if terms.len() < 2 {
        return None;
    }
    let cost = shift_add_cost(&terms);
    if cost > MAX_SHIFT_ADD_CHAIN_COST {
        return None;
    }

    let mut terms = terms;
    terms.sort_unstable_by_key(|term| std::cmp::Reverse(term.shift));
    let mut iter = terms.into_iter();
    let first = iter.next()?;
    let mut acc = if first.sign > 0 {
        shifted_term(x, first.shift)
    } else {
        Expr::sub(Expr::u32(0), shifted_term(x, first.shift))
    };
    for term in iter {
        let rhs = shifted_term(x, term.shift);
        acc = if term.sign > 0 {
            Expr::add(acc, rhs)
        } else {
            Expr::sub(acc, rhs)
        };
    }
    Some(acc)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedShiftTerm {
    shift: u32,
    sign: i8,
}

fn naf_terms(mut n: u32) -> Vec<SignedShiftTerm> {
    let mut shift = 0u32;
    let mut terms = Vec::with_capacity(n.count_ones() as usize + 1);
    while n > 0 {
        if n & 1 == 0 {
            n >>= 1;
            shift += 1;
            continue;
        }
        let sign = if n & 3 == 1 || n == 1 { 1 } else { -1 };
        terms.push(SignedShiftTerm { shift, sign });
        if sign > 0 {
            n -= 1;
        } else {
            n = n.wrapping_add(1);
        }
        n >>= 1;
        shift += 1;
    }
    terms
}

fn shift_add_cost(terms: &[SignedShiftTerm]) -> u32 {
    let shifts =
        u32::try_from(terms.iter().filter(|term| term.shift != 0).count()).unwrap_or(u32::MAX);
    let combines = u32::try_from(terms.len().saturating_sub(1)).unwrap_or(u32::MAX);
    shifts + combines
}

fn shifted_term(x: &Expr, shift: u32) -> Expr {
    if shift == 0 {
        x.clone()
    } else {
        Expr::shl(x.clone(), Expr::u32(shift))
    }
}

fn operand_duplication_cost(expr: &Expr) -> u32 {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => 0,
        Expr::Load { .. } | Expr::BufLen { .. } => 1,
        _ => 2,
    }
}

fn positive_u32_constant(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::LitU32(v) => Some(*v),
        Expr::LitI32(v) if *v > 0 => u32::try_from(*v).ok(),
        _ => None,
    }
}

/// Fold `Div(LitF32(1.0), LitF32(c))` to `LitF32(1.0/c)` when `c` is
/// finite and non-zero. Division by zero and non-finite divisors stay
/// in the IR so validation/lowering keeps the same semantics as the
/// original program.
///
/// Returns `None` for `c == 0.0` or non-literal operands.
pub(super) fn reciprocal_constant_fold(left: &Expr, right: &Expr) -> Option<Expr> {
    if let (Expr::LitF32(one), Expr::LitF32(c)) = (left, right) {
        if one.to_bits() == 1.0f32.to_bits() && (c.to_bits() & 0x7FFF_FFFF) != 0 && c.is_finite() {
            return Some(Expr::f32(1.0 / c));
        }
    }
    None
}

pub(super) fn synthesize_fma_add(left: &Expr, right: &Expr) -> Option<Expr> {
    if let Some((a, b)) = mul_terms(left) {
        if is_float_expr(right) {
            return Some(Expr::fma(a, b, right.clone()));
        }
    }
    if let Some((a, b)) = mul_terms(right) {
        if is_float_expr(left) {
            return Some(Expr::fma(a, b, left.clone()));
        }
    }
    if let Some((a, b)) = negated_mul_terms(left) {
        if is_float_expr(right) {
            return Some(Expr::fma(Expr::negate(a), b, right.clone()));
        }
    }
    if let Some((a, b)) = negated_mul_terms(right) {
        if is_float_expr(left) {
            return Some(Expr::fma(Expr::negate(a), b, left.clone()));
        }
    }
    None
}

pub(super) fn synthesize_fma_sub(left: &Expr, right: &Expr) -> Option<Expr> {
    if let Some((a, b)) = mul_terms(left) {
        if is_float_expr(right) {
            return Some(Expr::fma(a, b, Expr::negate(right.clone())));
        }
    }
    if let Some((a, b)) = mul_terms(right) {
        if is_float_expr(left) {
            return Some(Expr::fma(Expr::negate(a), b, left.clone()));
        }
    }
    None
}

pub(super) fn power_of_two_shift(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::LitU32(value) if value.is_power_of_two() => Some(value.trailing_zeros()),
        Expr::LitI32(value)
            if *value > 0 && u32::try_from(*value).is_ok_and(u32::is_power_of_two) =>
        {
            u32::try_from(*value).ok().map(u32::trailing_zeros)
        }
        _ => None,
    }
}

fn mul_terms(expr: &Expr) -> Option<(Expr, Expr)> {
    match expr {
        Expr::BinOp {
            op: BinOp::Mul,
            left,
            right,
        } => Some((left.as_ref().clone(), right.as_ref().clone())),
        _ => None,
    }
}

fn negated_mul_terms(expr: &Expr) -> Option<(Expr, Expr)> {
    match expr {
        Expr::UnOp {
            op: UnOp::Negate,
            operand,
        } => mul_terms(operand),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════
// Granlund-Montgomery: integer division by constant → mulhi + shift
//
// Reference: Hacker's Delight (Henry S. Warren Jr.), Chapter 10.
// Also: "Division by Invariant Integers using Multiplication"
//       (Granlund & Montgomery, 1994, PLDI).
//
// For n / d where d is a known non-zero, non-power-of-two u32:
//
//   Case 1 (no fixup):   n / d = mulhi(n, magic) >> shift
//   Case 2 (with fixup): t = mulhi(n, magic)
//                         n / d = (t + ((n - t) >> 1)) >> (shift - 1)
//
// All math here is compile-time. Zero runtime cost.
// ═══════════════════════════════════════════════════════════════

/// Precomputed magic numbers for the Granlund-Montgomery transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DivMagic {
    /// The magic multiplier M. Passed to `mulhi(n, M)`.
    pub magic: u32,
    /// Post-multiply right-shift count.
    pub shift: u32,
    /// If true, use the fixup sequence:
    ///   `t = mulhi(n, M); result = (t + ((n - t) >> 1)) >> (shift - 1)`
    /// If false, use the simple sequence:
    ///   `result = mulhi(n, M) >> shift`
    pub needs_fixup: bool,
}

/// Compute Granlund-Montgomery magic numbers for unsigned 32-bit division.
///
/// Panics if `d` is 0 or 1 or a power of two (use `power_of_two_shift`
/// for those cases  -  they're even cheaper).
///
/// Algorithm D from Hacker's Delight, Chapter 10 (Henry S. Warren Jr.).
/// Uses u32 wrapping arithmetic matching Warren's original C code.
pub(super) fn compute_div_magic(d: u32) -> DivMagic {
    debug_assert!(
        d >= 2 && !d.is_power_of_two(),
        "d must be >= 2 and not a power of 2"
    );

    let mut needs_fixup = false;

    // nc = floor((2^32 - 1) / d) * d   -  largest multiple of d ≤ 2^32 - 1
    // Equivalent to C's: unsigned nc = -1 - (-d) % d;
    let nc = u32::MAX - (d.wrapping_neg() % d);
    let mut p: u32 = 31;

    let mut q1 = 0x8000_0000u32 / nc;
    let mut r1 = 0x8000_0000u32 - q1 * nc;
    let mut q2 = 0x7FFF_FFFFu32 / d;
    let mut r2 = 0x7FFF_FFFFu32 - q2 * d;

    loop {
        p += 1;

        if r1 >= nc - r1 {
            q1 = q1.wrapping_shl(1).wrapping_add(1);
            r1 = r1.wrapping_shl(1).wrapping_sub(nc);
        } else {
            q1 = q1.wrapping_shl(1);
            r1 = r1.wrapping_shl(1);
        }

        if r2.wrapping_add(1) >= d.wrapping_sub(r2) {
            if q2 >= 0x7FFF_FFFFu32 {
                needs_fixup = true;
            }
            q2 = q2.wrapping_shl(1).wrapping_add(1);
            r2 = r2.wrapping_shl(1).wrapping_add(1).wrapping_sub(d);
        } else {
            if q2 >= 0x8000_0000u32 {
                needs_fixup = true;
            }
            q2 = q2.wrapping_shl(1);
            r2 = r2.wrapping_shl(1).wrapping_add(1);
        }

        let delta = d.wrapping_sub(1).wrapping_sub(r2);
        if !(p < 64 && (q1 < delta || (q1 == delta && r1 == 0))) {
            break;
        }
    }

    DivMagic {
        magic: q2.wrapping_add(1),
        shift: p - 32,
        needs_fixup,
    }
}

/// Emit the Granlund-Montgomery sequence for `dividend / d`.
///
/// Returns `None` if `d` is 0, 1, or a power of two (handled elsewhere).
///
/// For non-fixup: `mulhi(n, M) >> s`  -  2 instructions, ~5 GPU cycles.
/// For fixup:     `t = mulhi(n, M); (t + ((n - t) >> 1)) >> (s - 1)`
///                 -  5 instructions, ~9 GPU cycles.
/// Original `Div`: 1 instruction but ~50-100 GPU cycles (software).
pub(super) fn granlund_montgomery_div(dividend: &Expr, d: u32) -> Option<Expr> {
    if d <= 1 || d.is_power_of_two() {
        return None;
    }

    let magic = compute_div_magic(d);
    let n = dividend.clone();

    if magic.needs_fixup {
        // Case 2: t = mulhi(n, M)
        //         result = (t + ((n - t) >> 1)) >> (s - 1)
        let t = Expr::mulhi(n.clone(), Expr::u32(magic.magic));
        let n_minus_t = Expr::sub(n, t.clone());
        let half = Expr::shr(n_minus_t, Expr::u32(1));
        let sum = Expr::add(t, half);
        let shift = magic.shift.saturating_sub(1);
        if shift == 0 {
            Some(sum)
        } else {
            Some(Expr::shr(sum, Expr::u32(shift)))
        }
    } else {
        // Case 1: result = mulhi(n, M) >> s
        let hi = Expr::mulhi(n, Expr::u32(magic.magic));
        if magic.shift == 0 {
            Some(hi)
        } else {
            Some(Expr::shr(hi, Expr::u32(magic.shift)))
        }
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    /// Exhaustive correctness test: verify the magic numbers produce
    /// the correct quotient for all u32 dividends in a representative
    /// sample, for every divisor 2..=1000.
    #[test]
    fn granlund_montgomery_correctness() {
        // Test a wide range of divisors.
        for d in 2u32..=1000 {
            if d.is_power_of_two() {
                continue;
            }
            let magic = compute_div_magic(d);

            // Test representative dividends: 0, 1, d-1, d, d+1,
            // small values, powers of 2, large values, MAX.
            let test_values: Vec<u32> = vec![
                0,
                1,
                2,
                d - 1,
                d,
                d + 1,
                d * 2,
                d * 3,
                1000,
                65535,
                65536,
                u32::MAX / d * d,     // largest exact multiple
                u32::MAX / d * d - 1, // just below
                u32::MAX,
                u32::MAX - 1,
            ];

            for &n in &test_values {
                let expected = n / d;
                let actual = apply_magic(n, &magic);
                assert_eq!(
                    actual, expected,
                    "n={n}, d={d}, magic={}, shift={}, fixup={}: got {actual}, expected {expected}",
                    magic.magic, magic.shift, magic.needs_fixup
                );
            }
        }
    }

    /// Verify extreme divisors.
    #[test]
    fn granlund_montgomery_extreme_divisors() {
        for d in [3, 7, 10, 127, 255, 1000, 65535, 0x7FFF_FFFF, u32::MAX - 1] {
            if d.is_power_of_two() || d <= 1 {
                continue;
            }
            let magic = compute_div_magic(d);
            for &n in &[0u32, 1, d - 1, d, d + 1, u32::MAX] {
                assert_eq!(apply_magic(n, &magic), n / d, "n={n}, d={d}");
            }
        }
    }

    /// CPU emulation of the magic-number division sequence.
    fn apply_magic(n: u32, magic: &DivMagic) -> u32 {
        let hi = ((n as u64).wrapping_mul(magic.magic as u64) >> 32) as u32;
        if !magic.needs_fixup {
            hi >> magic.shift
        } else {
            let t = hi;
            let half = (n.wrapping_sub(t)) >> 1;
            (t.wrapping_add(half)) >> magic.shift.saturating_sub(1)
        }
    }
}
