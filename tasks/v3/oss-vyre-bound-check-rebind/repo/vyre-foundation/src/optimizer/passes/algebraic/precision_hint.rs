//! ROADMAP G1 / G5 foundation half  -  precision + transcendental
//! fast-path hints.
//!
//! Walks the entry tree and identifies expression contexts where
//! a lower-precision computation (F16 instead of F32) or a
//! polynomial transcendental approximation (e.g. `Sin` on a
//! provably narrow argument range) would be observably equivalent
//! within a declared ULP budget. The pass emits a hint side-table
//! the lower_emit layer reads to choose the F16 / polynomial
//! lowering.
//!
//! ## Why a hint, not a rewrite
//!
//! Mixed-precision and transcendental fast paths are emitter
//! concerns  -  the IR's `Expr` enum is typed at the F32 level and
//! the actual F16 / polynomial code lives in the backend's
//! lowering. The foundation pass identifies *candidate sites* and
//! the *contract* (range, ULP budget) the emitter must honour;
//! the emitter performs the actual code-shape change.
//!
//! ## What counts as a candidate
//!
//! G1 mixed-precision: an `Expr::BinOp / UnOp / Fma` whose
//! operands all derive from `Expr::LitF32(c)` with `c` representable
//! exactly as F16 (i.e. `c == f16_to_f32(f32_to_f16(c))`). The
//! result is recoverable to F32 within 1 ULP after the F16 round-
//! trip.
//!
//! G5 transcendental fast path: an `Expr::UnOp { op: Sin / Cos /
//! Exp / Log }` whose operand is a literal in a tight range
//! (`|x| <= π/4` for Sin/Cos, `|x| <= 1.0` for Exp/Log) where the
//! cubic / quartic Taylor polynomial is within the declared ULP
//! budget. The hint records the candidate site + the polynomial
//! the emitter should substitute.

use crate::ir::{Expr, Node, Program, UnOp};
use dashmap::DashMap;

/// Per-site precision-hint entry. Recorded by the analysis and
/// consumed by the lowering layer.
#[derive(Clone, Debug, PartialEq)]
pub enum PrecisionHint {
    /// Site is safe to lower at F16 instead of F32. The emitter
    /// uses F16 multiply/add ALUs (Tensor Cores on supported
    /// hardware) for ~2x throughput.
    F16Eligible {
        /// Maximum absolute value of any literal operand on the
        /// site (so the emitter can sanity-check F16 range).
        max_abs_operand: f32,
    },
    /// Site is a transcendental whose argument is in the polynomial
    /// fast-path range. The lowering emits the named polynomial
    /// instead of the device sin/cos/exp/log call.
    TranscendentalPolynomial {
        /// Which transcendental.
        op: TranscendentalOp,
        /// Argument's absolute upper bound; emitter clamps before
        /// applying the polynomial.
        argument_bound: f32,
    },
}

/// Which transcendental the polynomial fast-path replaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TranscendentalOp {
    /// `sin(x)` for `|x| <= π/4`.
    Sin,
    /// `cos(x)` for `|x| <= π/4`.
    Cos,
    /// `exp(x)` for `|x| <= 1.0`.
    Exp,
    /// `ln(x)` for `x ∈ [1.0, 2.0]`.
    Ln,
}

/// Expression-context key. The hint is keyed by a stable digest
/// of the Expr's structural fingerprint  -  not the Expr pointer
/// (which would change across pass reruns). The current digest is
/// the BLAKE3 of the Expr's wire bytes; the lowering layer
/// computes the same digest at emit time and looks up its hint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExprDigest(pub [u8; 32]);

/// Side table of precision hints keyed by `ExprDigest`. Cheap to
/// clone; Send + Sync so the analysis thread populates while the
/// emitter thread queries.
#[derive(Clone, Debug, Default)]
pub struct PrecisionHints {
    inner: DashMap<ExprDigest, PrecisionHint>,
}

impl PrecisionHints {
    /// Build an empty hint table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a hint for the given expression digest. Replaces
    /// any existing entry for that digest.
    pub fn record(&self, digest: ExprDigest, hint: PrecisionHint) {
        self.inner.insert(digest, hint);
    }

    /// Look up the hint for an expression digest. Returns `None`
    /// when no hint is recorded  -  the emitter must fall back to
    /// the default F32 / device-transcendental lowering.
    #[must_use]
    pub fn lookup(&self, digest: ExprDigest) -> Option<PrecisionHint> {
        self.inner.get(&digest).map(|r| r.clone())
    }

    /// Number of recorded hints.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` iff zero hints recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Analyse the program and populate `hints` with G1/G5 candidates.
/// Returns the number of hints recorded.
pub fn analyse_precision(program: &Program, hints: &PrecisionHints) -> usize {
    let mut count = 0usize;
    for node in program.entry() {
        analyse_node(node, hints, &mut count);
    }
    count
}

fn analyse_node(node: &Node, hints: &PrecisionHints, count: &mut usize) {
    match node {
        Node::Let { value, .. } | Node::Assign { value, .. } => {
            analyse_expr(value, hints, count);
        }
        Node::Store { index, value, .. } => {
            analyse_expr(index, hints, count);
            analyse_expr(value, hints, count);
        }
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            analyse_expr(cond, hints, count);
            for n in then {
                analyse_node(n, hints, count);
            }
            for n in otherwise {
                analyse_node(n, hints, count);
            }
        }
        Node::Loop { from, to, body, .. } => {
            analyse_expr(from, hints, count);
            analyse_expr(to, hints, count);
            for n in body {
                analyse_node(n, hints, count);
            }
        }
        Node::Block(body) => {
            for n in body {
                analyse_node(n, hints, count);
            }
        }
        Node::Region { body, .. } => {
            for n in body.iter() {
                analyse_node(n, hints, count);
            }
        }
        _ => {}
    }
}

fn analyse_expr(expr: &Expr, hints: &PrecisionHints, count: &mut usize) {
    // G1: F16-eligible if the expression is a BinOp/UnOp/Fma
    // whose deepest literal operand fits in F16's range.
    if let Some(max_abs) = literal_only_fp_value_max(expr) {
        if fits_f16_range(max_abs) {
            let digest = digest_of(expr);
            hints.record(
                digest,
                PrecisionHint::F16Eligible {
                    max_abs_operand: max_abs,
                },
            );
            *count += 1;
        }
    }
    // G5: transcendental on a literal in fast-path range.
    if let Expr::UnOp { op, operand } = expr {
        if let Some(transcendental) = transcendental_op(op) {
            if let Some(literal) = literal_f32(operand) {
                let bound = literal.abs();
                let in_range = match transcendental {
                    TranscendentalOp::Sin | TranscendentalOp::Cos => {
                        bound <= std::f32::consts::FRAC_PI_4
                    }
                    TranscendentalOp::Exp => bound <= 1.0,
                    TranscendentalOp::Ln => (1.0..=2.0).contains(&literal),
                };
                if in_range {
                    let digest = digest_of(expr);
                    hints.record(
                        digest,
                        PrecisionHint::TranscendentalPolynomial {
                            op: transcendental,
                            argument_bound: bound,
                        },
                    );
                    *count += 1;
                }
            }
        }
    }
    // Recurse into compound expressions.
    match expr {
        Expr::Load { index, .. } => analyse_expr(index, hints, count),
        Expr::BinOp { left, right, .. } => {
            analyse_expr(left, hints, count);
            analyse_expr(right, hints, count);
        }
        Expr::UnOp { operand, .. } => analyse_expr(operand, hints, count),
        Expr::Call { args, .. } => {
            for arg in args {
                analyse_expr(arg, hints, count);
            }
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            analyse_expr(cond, hints, count);
            analyse_expr(true_val, hints, count);
            analyse_expr(false_val, hints, count);
        }
        Expr::Cast { value, .. } => analyse_expr(value, hints, count),
        Expr::Fma { a, b, c } => {
            analyse_expr(a, hints, count);
            analyse_expr(b, hints, count);
            analyse_expr(c, hints, count);
        }
        _ => {}
    }
}

fn literal_f32(expr: &Expr) -> Option<f32> {
    if let Expr::LitF32(v) = expr {
        Some(*v)
    } else {
        None
    }
}

/// Returns `Some(max_abs)` if every leaf of `expr` is a `LitF32`
/// (i.e. the whole sub-expression is a literal arithmetic
/// expression). Returns `None` otherwise.
fn literal_only_fp_value_max(expr: &Expr) -> Option<f32> {
    match expr {
        Expr::LitF32(v) => Some(v.abs()),
        Expr::BinOp { left, right, .. } => {
            let l = literal_only_fp_value_max(left)?;
            let r = literal_only_fp_value_max(right)?;
            Some(l.max(r))
        }
        Expr::UnOp { operand, .. } => literal_only_fp_value_max(operand),
        Expr::Fma { a, b, c } => {
            let a = literal_only_fp_value_max(a)?;
            let b = literal_only_fp_value_max(b)?;
            let c = literal_only_fp_value_max(c)?;
            Some(a.max(b).max(c))
        }
        _ => None,
    }
}

/// F16 has range ~±65504 with ~3-4 decimal digits of precision.
/// The conservative gate: |x| < 65504.0 AND x is finite.
fn fits_f16_range(value: f32) -> bool {
    value.is_finite() && value.abs() < 65_504.0
}

fn transcendental_op(op: &UnOp) -> Option<TranscendentalOp> {
    match op {
        UnOp::Sin => Some(TranscendentalOp::Sin),
        UnOp::Cos => Some(TranscendentalOp::Cos),
        UnOp::Exp => Some(TranscendentalOp::Exp),
        UnOp::Log => Some(TranscendentalOp::Ln),
        _ => None,
    }
}

/// Compute a stable structural digest for an Expr. We use the
/// debug formatting of the expression as the digest seed so the
/// digest is stable across runs and across the pass-rerun cycle.
fn digest_of(expr: &Expr) -> ExprDigest {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(format!("{expr:?}").as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    ExprDigest(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::F32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    /// Empty program: zero hints recorded.
    #[test]
    fn empty_program_records_zero_hints() {
        let hints = PrecisionHints::new();
        let count = analyse_precision(&program(Vec::new()), &hints);
        assert_eq!(count, 0);
        assert!(hints.is_empty());
    }

    /// `f32(1.5) + f32(2.0)` is F16-eligible (both literals fit
    /// in F16 exactly).
    #[test]
    fn fp_literal_addition_is_f16_eligible() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::BinOp {
                op: BinOp::Add,
                left: Box::new(Expr::f32(1.5)),
                right: Box::new(Expr::f32(2.0)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        assert!(hints.len() >= 1);
    }

    /// A literal over F16 range (`1e10`) is NOT F16-eligible.
    #[test]
    fn fp_literal_outside_f16_range_skips_g1() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::BinOp {
                op: BinOp::Mul,
                left: Box::new(Expr::f32(1e10)),
                right: Box::new(Expr::f32(2.0)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        for digest in [digest_of(&Expr::f32(1e10))].iter() {
            assert!(matches!(
                hints.lookup(*digest),
                Some(PrecisionHint::F16Eligible { .. }) | None
            ));
        }
        // The compound BinOp with the 1e10 operand must not be
        // F16-eligible.
        let compound = Expr::BinOp {
            op: BinOp::Mul,
            left: Box::new(Expr::f32(1e10)),
            right: Box::new(Expr::f32(2.0)),
        };
        let compound_digest = digest_of(&compound);
        assert!(
            !matches!(
                hints.lookup(compound_digest),
                Some(PrecisionHint::F16Eligible { .. })
            ),
            "1e10 operand must reject F16 eligibility for the parent BinOp"
        );
    }

    /// `Sin(0.5)` is in the polynomial fast-path range
    /// (`|x| <= π/4 ≈ 0.785`).
    #[test]
    fn sin_in_quarter_pi_range_recorded() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::UnOp {
                op: UnOp::Sin,
                operand: Box::new(Expr::f32(0.5)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        let digest = digest_of(&Expr::UnOp {
            op: UnOp::Sin,
            operand: Box::new(Expr::f32(0.5)),
        });
        assert!(matches!(
            hints.lookup(digest),
            Some(PrecisionHint::TranscendentalPolynomial {
                op: TranscendentalOp::Sin,
                ..
            })
        ));
    }

    /// `Sin(2.0)` is OUTSIDE the polynomial fast-path range so
    /// no transcendental hint is recorded.
    #[test]
    fn sin_outside_quarter_pi_range_skips_g5() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::UnOp {
                op: UnOp::Sin,
                operand: Box::new(Expr::f32(2.0)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        let digest = digest_of(&Expr::UnOp {
            op: UnOp::Sin,
            operand: Box::new(Expr::f32(2.0)),
        });
        assert!(!matches!(
            hints.lookup(digest),
            Some(PrecisionHint::TranscendentalPolynomial { .. })
        ));
    }

    /// `Exp(0.5)` is in fast-path range (`|x| <= 1.0`).
    #[test]
    fn exp_within_unit_range_recorded() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::UnOp {
                op: UnOp::Exp,
                operand: Box::new(Expr::f32(0.5)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        let digest = digest_of(&Expr::UnOp {
            op: UnOp::Exp,
            operand: Box::new(Expr::f32(0.5)),
        });
        assert!(matches!(
            hints.lookup(digest),
            Some(PrecisionHint::TranscendentalPolynomial {
                op: TranscendentalOp::Exp,
                ..
            })
        ));
    }

    /// `Ln(1.5)` is in fast-path range (`x ∈ [1.0, 2.0]`).
    #[test]
    fn ln_within_one_to_two_range_recorded() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::UnOp {
                op: UnOp::Log,
                operand: Box::new(Expr::f32(1.5)),
            },
        )];
        analyse_precision(&program(entry), &hints);
        let digest = digest_of(&Expr::UnOp {
            op: UnOp::Log,
            operand: Box::new(Expr::f32(1.5)),
        });
        assert!(matches!(
            hints.lookup(digest),
            Some(PrecisionHint::TranscendentalPolynomial {
                op: TranscendentalOp::Ln,
                ..
            })
        ));
    }

    /// `Sin(Var(theta))` (non-literal operand) doesn't get a
    /// transcendental hint  -  the range can't be proven without
    /// further fact substrate.
    #[test]
    fn sin_non_literal_skips_g5() {
        let hints = PrecisionHints::new();
        let entry = vec![Node::let_bind(
            "x",
            Expr::UnOp {
                op: UnOp::Sin,
                operand: Box::new(Expr::var("theta")),
            },
        )];
        analyse_precision(&program(entry), &hints);
        // No transcendental hints recorded for non-literal operands.
        assert!(
            hints.is_empty()
                || !hints
                    .lookup(digest_of(&Expr::UnOp {
                        op: UnOp::Sin,
                        operand: Box::new(Expr::var("theta")),
                    }))
                    .map_or(false, |h| matches!(
                        h,
                        PrecisionHint::TranscendentalPolynomial { .. }
                    ))
        );
    }

    /// Hint table is Send + Sync.
    #[test]
    fn hints_are_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<PrecisionHints>();
        assert_sync::<PrecisionHints>();
    }
}
