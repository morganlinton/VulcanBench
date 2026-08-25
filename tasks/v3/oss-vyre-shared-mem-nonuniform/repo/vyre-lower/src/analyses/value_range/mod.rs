//! Value-range analysis (phase 1).
//!
//! For each result-id, tracks the known integer range `[min, max]`
//! (inclusive) when statically derivable. Phase 1 is the minimum
//! viable: Lit-derived singletons (a Lit(U32(7)) op has range
//! `[7, 7]`) and trivial union via Min/Max BinOps.
//!
//! Future phases (not shipped):
//! - Add narrows from comparison-guarded branches.
//! - Add narrows from Add/Sub/Mul on known-bounded operands.
//! - Add SubgroupLocalId / LocalInvocationId range from
//!   dispatch.workgroup_size.
//! - F32 ranges (with NaN handling).
//!
//! Even phase 1 is useful: enables downstream rewrites to drop
//! bounds checks (`Lt(x, n)` with known x always < n) and to choose
//! efficient strength-reduce alternatives based on operand magnitude.

use rustc_hash::FxHashMap;

use serde::{Deserialize, Serialize};

use crate::{KernelBody, KernelDescriptor, KernelOpKind, LiteralValue};
use vyre_foundation::ir::BinOp;

/// Inclusive integer range. Represented as i64 internally so it can
/// hold both U32 and I32 bounds without overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntRange {
    pub min: i64,
    pub max: i64,
}

impl IntRange {
    /// Singleton range `[v, v]`.
    pub fn singleton(v: i64) -> Self {
        Self { min: v, max: v }
    }

    /// True iff this range contains exactly one value.
    pub fn is_singleton(&self) -> bool {
        self.min == self.max
    }

    /// Inclusive containment.
    pub fn contains(&self, v: i64) -> bool {
        v >= self.min && v <= self.max
    }

    /// Union of two ranges (smallest range that contains both).
    /// Useful for joining branch arms or Min/Max alternatives.
    pub fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ValueRangeReport {
    /// `result_id → IntRange` for ids in the TOP-LEVEL body whose
    /// range is statically derivable. Per-body id space  -  child
    /// bodies are walked separately via `analyze_body` if callers
    /// need it.
    pub ranges: FxHashMap<u32, IntRange>,
}

impl ValueRangeReport {
    pub fn known_count(&self) -> usize {
        self.ranges.len()
    }

    /// Range for `result_id`, or `None` if not known.
    pub fn get(&self, result_id: u32) -> Option<IntRange> {
        self.ranges.get(&result_id).copied()
    }

    /// True iff the value at `result_id` is provably equal to `target`.
    /// Returns `None` if the range isn't known (caller may want to
    /// treat that differently from a known-unequal).
    pub fn is_definitely(&self, result_id: u32, target: i64) -> Option<bool> {
        self.ranges
            .get(&result_id)
            .map(|r| r.is_singleton() && r.min == target)
    }

    /// True iff every value in `result_id`'s range is `< target`.
    /// `None` if range unknown.
    pub fn is_definitely_below(&self, result_id: u32, target: i64) -> Option<bool> {
        self.ranges.get(&result_id).map(|r| r.max < target)
    }

    /// True iff every value in `result_id`'s range is `>= target`.
    /// `None` if range unknown.
    pub fn is_definitely_at_least(&self, result_id: u32, target: i64) -> Option<bool> {
        self.ranges.get(&result_id).map(|r| r.min >= target)
    }

    /// If the range for `result_id` is a singleton, return that value.
    /// Useful for downstream rewrites that want to know "is this id
    /// known to be exactly some constant?". Returns `None` for both
    /// "range unknown" and "range non-singleton".
    pub fn as_constant(&self, result_id: u32) -> Option<i64> {
        self.ranges
            .get(&result_id)
            .filter(|r| r.is_singleton())
            .map(|r| r.min)
    }
}

#[must_use]
pub fn analyze(desc: &KernelDescriptor) -> ValueRangeReport {
    analyze_body(&desc.body)
}

#[must_use]
pub fn analyze_body(body: &KernelBody) -> ValueRangeReport {
    let mut ranges: FxHashMap<u32, IntRange> = FxHashMap::default();

    // Phase 1a: seed from Literal ops.
    for op in &body.ops {
        if matches!(op.kind, KernelOpKind::Literal) {
            if let (Some(rid), Some(&pool_idx)) = (op.result, op.operands.first()) {
                if let Some(lit) = body.literals.get(pool_idx as usize) {
                    let r = match lit {
                        LiteralValue::U32(v) => Some(IntRange::singleton(*v as i64)),
                        LiteralValue::I32(v) => Some(IntRange::singleton(*v as i64)),
                        LiteralValue::Bool(true) => Some(IntRange::singleton(1)),
                        LiteralValue::Bool(false) => Some(IntRange::singleton(0)),
                        _ => None,
                    };
                    if let Some(r) = r {
                        ranges.insert(rid, r);
                    }
                }
            }
        }
    }

    // Phase 1b: propagate through Min/Max BinOps where both operands
    // have known ranges. The result range is the union  -  Min(a, b)
    // could be either, so the result is in [min(a.min, b.min),
    // min(a.max, b.max)] for Min, but a tighter union is safe.
    for op in &body.ops {
        if let KernelOpKind::BinOpKind(bin_op) = &op.kind {
            if op.operands.len() < 2 {
                continue;
            }
            let lhs = ranges.get(&op.operands[0]).copied();
            let rhs = ranges.get(&op.operands[1]).copied();
            let Some(rid) = op.result else { continue };
            if let (Some(l), Some(r)) = (lhs, rhs) {
                let derived = match bin_op {
                    BinOp::Min => Some(IntRange {
                        min: l.min.min(r.min),
                        max: l.max.min(r.max),
                    }),
                    BinOp::Max => Some(IntRange {
                        min: l.min.max(r.min),
                        max: l.max.max(r.max),
                    }),
                    BinOp::Add | BinOp::WrappingAdd => {
                        // Result range is [l.min+r.min, l.max+r.max].
                        // Use checked_add to bail on overflow rather
                        // than silently wrap (which would produce a
                        // false-narrow range).
                        match (l.min.checked_add(r.min), l.max.checked_add(r.max)) {
                            (Some(min), Some(max)) => Some(IntRange { min, max }),
                            _ => None,
                        }
                    }
                    BinOp::Sub | BinOp::WrappingSub => {
                        // Result range is [l.min-r.max, l.max-r.min].
                        // Subtraction of a range flips the bounds.
                        match (l.min.checked_sub(r.max), l.max.checked_sub(r.min)) {
                            (Some(min), Some(max)) => Some(IntRange { min, max }),
                            _ => None,
                        }
                    }
                    BinOp::Mul => mul_range(l, r),
                    BinOp::BitAnd => {
                        // x & mask: result is in [0, max_possible].
                        // The max_possible is the smaller of the two
                        // operand maxes  -  neither operand can
                        // contribute bits the other doesn't have set.
                        // Conservative: refuse on negatives (sign bit
                        // makes the range non-trivial).
                        if l.min < 0 || r.min < 0 {
                            None
                        } else {
                            Some(IntRange {
                                min: 0,
                                max: l.max.min(r.max),
                            })
                        }
                    }
                    BinOp::BitOr => {
                        // x | y: each bit is ≥ either input's bit, so
                        // result.min ≥ max(l.min, r.min). Conservative
                        // upper bound: l.max | r.max (no bit can appear
                        // that wasn't in some operand's max). Refuse on
                        // negatives.
                        if l.min < 0 || r.min < 0 {
                            None
                        } else {
                            Some(IntRange {
                                min: l.min.max(r.min),
                                max: l.max | r.max,
                            })
                        }
                    }
                    BinOp::Shl if r.is_singleton() && r.min >= 0 && r.min < 32 => {
                        // x << k for known k: result range scales by 2^k.
                        // Use checked_shl to bail on overflow. l can be
                        // negative  -  Shl on negatives is well-defined
                        // arithmetic-shift in Rust (multiplies by 2^k).
                        let k = r.min as u32;
                        match (l.min.checked_shl(k), l.max.checked_shl(k)) {
                            (Some(min), Some(max)) => Some(IntRange { min, max }),
                            _ => None,
                        }
                    }
                    BinOp::Shr if r.is_singleton() && r.min >= 0 && r.min < 32 => {
                        // x >> k: arithmetic right shift on i64.
                        // Result range is [l.min >> k, l.max >> k]
                        // (shifting preserves order for the same shift).
                        let k = r.min as u32;
                        Some(IntRange {
                            min: l.min >> k,
                            max: l.max >> k,
                        })
                    }
                    _ => None,
                };
                if let Some(d) = derived {
                    ranges.insert(rid, d);
                }
            }
        }
    }

    ValueRangeReport { ranges }
}

/// Range of `l * r` accounting for sign  -  the result range is the
/// min/max of the four corner products (l.min*r.min, l.min*r.max,
/// l.max*r.min, l.max*r.max). Bails on overflow.
fn mul_range(l: IntRange, r: IntRange) -> Option<IntRange> {
    let corners = [
        l.min.checked_mul(r.min),
        l.min.checked_mul(r.max),
        l.max.checked_mul(r.min),
        l.max.checked_mul(r.max),
    ];
    let [Some(a), Some(b), Some(c), Some(d)] = corners else {
        return None;
    };
    let min = a.min(b).min(c).min(d);
    let max = a.max(b).max(c).max(d);
    Some(IntRange { min, max })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BindingLayout, Dispatch, KernelBody, KernelDescriptor, KernelOp, KernelOpKind, LiteralValue,
    };

    fn build(ops: Vec<KernelOp>, lits: Vec<LiteralValue>) -> KernelDescriptor {
        KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops,
                child_bodies: vec![],
                literals: lits,
            },
        }
    }

    #[test]
    fn empty_kernel_no_ranges() {
        let r = analyze(&build(vec![], vec![]));
        assert!(r.ranges.is_empty());
        assert_eq!(r.known_count(), 0);
    }

    #[test]
    fn lit_u32_yields_singleton() {
        let desc = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![LiteralValue::U32(42)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&0], IntRange::singleton(42));
        assert!(r.ranges[&0].is_singleton());
    }

    #[test]
    fn lit_i32_negative_yields_correct_range() {
        let desc = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![LiteralValue::I32(-7)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&0], IntRange::singleton(-7));
    }

    #[test]
    fn bool_true_is_one_false_is_zero() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
            ],
            vec![LiteralValue::Bool(true), LiteralValue::Bool(false)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&0], IntRange::singleton(1));
        assert_eq!(r.ranges[&1], IntRange::singleton(0));
    }

    #[test]
    fn min_of_two_lits_propagates() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Min),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(5)],
        );
        let r = analyze(&desc);
        // Both operands are singletons; result range is min..=min.
        assert_eq!(r.ranges[&2], IntRange::singleton(3));
    }

    #[test]
    fn max_of_two_lits_propagates() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Max),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(5)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(5));
    }

    #[test]
    fn add_propagates_singleton_ranges() {
        // 3 + 5 → [8, 8]
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(3), LiteralValue::U32(5)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(8));
    }

    #[test]
    fn sub_flips_operand_bounds() {
        // l - r where l ∈ [a,b] and r ∈ [c,d] → [a-d, b-c]
        // For singletons: 10 - 3 = 7
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Sub),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::I32(10), LiteralValue::I32(3)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(7));
    }

    #[test]

    fn bitand_with_mask_bounds_to_zero_through_mask() {
        // x & 0xFF where x is unknown but BitAnd(x, 0xFF) bounds to [0, 0xFF].
        // Phase 1 only knows x's range when x is itself a literal,
        // so use lit-lit here.
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(0x12345678), LiteralValue::U32(0xFF)],
        );
        let r = analyze(&desc);
        // l.max = 0x12345678, r.max = 0xFF; min(...) = 0xFF.
        assert_eq!(r.ranges[&2], IntRange { min: 0, max: 0xFF });
    }

    #[test]
    fn shl_propagates_with_singleton_shift() {
        // 5 << 3 = 40
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(5), LiteralValue::U32(3)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(40));
    }

    #[test]
    fn shr_propagates_with_singleton_shift() {
        // 40 >> 3 = 5
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shr),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(40), LiteralValue::U32(3)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(5));
    }

    #[test]
    fn shl_with_huge_shift_not_propagated() {
        // shift ≥ 32: refuse (would be overflow on i64 too in extreme cases).
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Shl),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(1), LiteralValue::U32(64)],
        );
        let r = analyze(&desc);
        assert!(!r.ranges.contains_key(&2));
    }

    #[test]
    fn bitor_propagates_with_singletons() {
        // 0xF0 | 0x0F = 0xFF
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitOr),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::U32(0xF0), LiteralValue::U32(0x0F)],
        );
        let r = analyze(&desc);
        // l.max=0xF0, r.max=0x0F → max = 0xF0|0x0F = 0xFF.
        // l.min=0xF0, r.min=0x0F → min = max(0xF0, 0x0F) = 0xF0.
        assert_eq!(
            r.ranges[&2],
            IntRange {
                min: 0xF0,
                max: 0xFF
            }
        );
    }

    #[test]
    fn bitand_negative_operand_not_propagated() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::BitAnd),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::I32(-1), LiteralValue::I32(0xFF)],
        );
        let r = analyze(&desc);
        // Neg operand → BitAnd refused.
        assert!(!r.ranges.contains_key(&2));
    }

    #[test]
    fn mul_singletons() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Mul),
                    operands: vec![0, 1],
                    result: Some(2),
                },
            ],
            vec![LiteralValue::I32(7), LiteralValue::I32(-3)],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&2], IntRange::singleton(-21));
    }

    #[test]
    fn mul_range_corner_helper() {
        // [2, 5] * [3, 4] = corners 6, 8, 15, 20 → [6, 20].
        let r = mul_range(IntRange { min: 2, max: 5 }, IntRange { min: 3, max: 4 });
        assert_eq!(r, Some(IntRange { min: 6, max: 20 }));

        // [-2, 3] * [-1, 4] = corners 2, -8, -3, 12 → [-8, 12].
        let r = mul_range(IntRange { min: -2, max: 3 }, IntRange { min: -1, max: 4 });
        assert_eq!(r, Some(IntRange { min: -8, max: 12 }));
    }

    #[test]
    fn add_chains_propagate() {
        // (3 + 5) + 7 = 15
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![2],
                    result: Some(2),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![0, 1],
                    result: Some(3),
                },
                KernelOp {
                    kind: KernelOpKind::BinOpKind(BinOp::Add),
                    operands: vec![3, 2],
                    result: Some(4),
                },
            ],
            vec![
                LiteralValue::U32(3),
                LiteralValue::U32(5),
                LiteralValue::U32(7),
            ],
        );
        let r = analyze(&desc);
        assert_eq!(r.ranges[&4], IntRange::singleton(15));
    }

    #[test]
    fn non_lit_op_no_range() {
        // LocalInvocationId  -  can't statically bound in phase 1.
        let desc = build(
            vec![KernelOp {
                kind: KernelOpKind::LocalInvocationId,
                operands: vec![0],
                result: Some(0),
            }],
            vec![],
        );
        let r = analyze(&desc);
        assert!(!r.ranges.contains_key(&0));
    }

    #[test]
    fn as_constant_returns_value_for_singleton() {
        let desc = build(
            vec![KernelOp {
                kind: KernelOpKind::Literal,
                operands: vec![0],
                result: Some(0),
            }],
            vec![LiteralValue::U32(42)],
        );
        let r = analyze(&desc);
        assert_eq!(r.as_constant(0), Some(42));
        assert_eq!(r.as_constant(99), None); // unknown id
    }

    #[test]
    fn as_constant_returns_none_for_non_singleton() {
        // Build an Add of two ranges that produces a non-singleton.
        // Phase 1: lit + lit folds to singleton, so we can't easily
        // produce a non-singleton via the analyses. Test via direct
        // ValueRangeReport construction.
        let mut report = ValueRangeReport::default();
        report.ranges.insert(7, IntRange { min: 0, max: 10 });
        assert_eq!(report.as_constant(7), None);
    }

    #[test]
    fn report_accessors() {
        let desc = build(
            vec![
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![0],
                    result: Some(0),
                },
                KernelOp {
                    kind: KernelOpKind::Literal,
                    operands: vec![1],
                    result: Some(1),
                },
            ],
            vec![LiteralValue::U32(0), LiteralValue::U32(42)],
        );
        let r = analyze(&desc);
        // is_definitely
        assert_eq!(r.is_definitely(0, 0), Some(true));
        assert_eq!(r.is_definitely(0, 1), Some(false));
        assert_eq!(r.is_definitely(99, 0), None); // unknown id
                                                  // is_definitely_below
        assert_eq!(r.is_definitely_below(1, 100), Some(true));
        assert_eq!(r.is_definitely_below(1, 42), Some(false)); // 42 < 42 false
                                                               // is_definitely_at_least
        assert_eq!(r.is_definitely_at_least(1, 42), Some(true));
        assert_eq!(r.is_definitely_at_least(1, 43), Some(false));
        // get
        assert_eq!(r.get(0), Some(IntRange::singleton(0)));
        assert_eq!(r.get(99), None);
    }

    #[test]
    fn range_helpers() {
        let r = IntRange { min: 3, max: 7 };
        assert!(r.contains(5));
        assert!(r.contains(3));
        assert!(r.contains(7));
        assert!(!r.contains(2));
        assert!(!r.contains(8));
        assert!(!r.is_singleton());

        let s = IntRange::singleton(42);
        assert!(s.is_singleton());

        let u = IntRange { min: 0, max: 5 }.union(IntRange { min: 3, max: 10 });
        assert_eq!(u, IntRange { min: 0, max: 10 });
    }
}
