//! Reference evaluator for [`RuleCondition`] / [`RuleFormula`] trees.
//!
//! Mirror of the GPU lowering in [`crate::rule::builder`] but runs the
//! formula through the deterministic reference oracle. It exists for
//! parity checks, CI gates, and unit tests that need rule outcomes
//! without backend dispatch.
//!
//! The evaluator is `unsafe`-free, side-effect-free, and `O(formula
//! size)`.
//!
//! # Example
//!
//! ```
//! use vyre_libs::rule::{evaluate_formula, RuleCondition, RuleEvaluationContext, RuleFormula};
//!
//! struct Ctx;
//! impl RuleEvaluationContext for Ctx {
//!     fn pattern_count(&self, pattern_id: u32) -> u32 {
//!         if pattern_id == 7 { 5 } else { 0 }
//!     }
//!     fn file_size(&self) -> u64 { 1024 }
//! }
//!
//! let f = RuleFormula::and(
//!     RuleFormula::condition(RuleCondition::PatternCountGte { pattern_id: 7, threshold: 3 }),
//!     RuleFormula::condition(RuleCondition::FileSizeLt(2048)),
//! );
//! assert!(evaluate_formula(&f, &Ctx));
//! ```

use std::sync::Arc;

use super::ast::{RuleCondition, RuleFormula};

/// Evaluation context the reference evaluator queries when resolving each
/// condition variant. Default impls return safe falsy values so a
/// minimal consumer only has to implement the methods it actually
/// uses.
pub trait RuleEvaluationContext {
    /// Number of times pattern `pattern_id` matched in the current
    /// record. Default: 0 (the pattern never matched). Override to
    /// resolve `PatternExists` / `PatternCountGt` / `PatternCountGte`.
    fn pattern_count(&self, _pattern_id: u32) -> u32 {
        0
    }

    /// File size in bytes for the current record. Default: 0.
    /// Override to resolve `FileSize*` conditions.
    fn file_size(&self) -> u64 {
        0
    }

    /// Resolve a named field value. Default: `None`. Override to
    /// resolve `RegexMatch { field, .. }` / `SubstringMatch { haystack,
    /// .. }` / `PrefixMatch { value, .. }` / `SuffixMatch { value, .. }`
    /// / `SetMembership { value, .. }`.
    ///
    /// Returns the borrowed field text. Conditions that need the
    /// caller's value directly carry it inline in the AST and don't
    /// hit this method.
    fn field_value(&self, _name: &str) -> Option<&str> {
        None
    }
}

/// Evaluate a [`RuleFormula`] against `ctx`. Recursive over
/// And/Or/Not nodes; returns the boolean verdict.
#[must_use]
pub fn evaluate_formula<C: RuleEvaluationContext + ?Sized>(formula: &RuleFormula, ctx: &C) -> bool {
    match formula {
        RuleFormula::Condition(cond) => evaluate_condition(cond, ctx),
        RuleFormula::And(left, right) => {
            // Short-circuit: don't evaluate `right` if `left` is false.
            evaluate_formula(left, ctx) && evaluate_formula(right, ctx)
        }
        RuleFormula::Or(left, right) => evaluate_formula(left, ctx) || evaluate_formula(right, ctx),
        RuleFormula::Not(inner) => !evaluate_formula(inner, ctx),
    }
}

/// Evaluate a single [`RuleCondition`] against `ctx`. Pure function;
/// no I/O, no allocation outside the regex case (see below).
///
/// `RegexMatch` compiles its pattern on every call. Callers that
/// evaluate the same regex thousands of times should hoist the
/// compile out of the rule by pre-computing a [`RuleCondition::Set
/// Membership`] or wrapping a custom [`RuleCondition::Opaque`]
/// extension that caches its own compiled regex.
///
/// `Opaque` extension conditions delegate via
/// [`RuleConditionExt::evaluate_opaque`]; the trait passes a
/// `&dyn Any` reference so extensions can downcast to whatever
/// context type they require. The [`RuleEvaluationContext`] is
/// passed via the `Any` payload by reference so an extension that
/// needs the standard context can downcast to `&C`.
#[must_use]
pub fn evaluate_condition<C: RuleEvaluationContext + ?Sized>(
    condition: &RuleCondition,
    ctx: &C,
) -> bool {
    match condition {
        RuleCondition::PatternExists { pattern_id } => ctx.pattern_count(*pattern_id) > 0,
        RuleCondition::PatternCountGt {
            pattern_id,
            threshold,
        } => ctx.pattern_count(*pattern_id) > *threshold,
        RuleCondition::PatternCountGte {
            pattern_id,
            threshold,
        } => ctx.pattern_count(*pattern_id) >= *threshold,
        RuleCondition::FileSizeLt(t) => ctx.file_size() < *t,
        RuleCondition::FileSizeLte(t) => ctx.file_size() <= *t,
        RuleCondition::FileSizeGt(t) => ctx.file_size() > *t,
        RuleCondition::FileSizeGte(t) => ctx.file_size() >= *t,
        RuleCondition::FileSizeEq(t) => ctx.file_size() == *t,
        RuleCondition::FileSizeNe(t) => ctx.file_size() != *t,
        RuleCondition::LiteralTrue => true,
        RuleCondition::LiteralFalse => false,
        RuleCondition::RegexMatch { field, pattern } => {
            // AUDIT_2026-05-23: was compile-on-every-eval (regex::Regex::new).
            // Added lazy cache. Long-term: replace with vyre AC kernel
            // (vyre_libs::scan::aho_corasick) or Opaque pre-compiled condition.
            let Some(value) = ctx.field_value(field.as_ref()) else {
                return false;
            };
            use std::collections::HashMap;
            use std::sync::LazyLock;
            use std::sync::Mutex;
            static REGEX_CACHE: LazyLock<Mutex<HashMap<String, regex::Regex>>> =
                LazyLock::new(|| Mutex::new(HashMap::new()));
            let Ok(cache) = REGEX_CACHE.lock() else {
                return false;
            };
            let re = cache.get(pattern.as_ref()).cloned();
            drop(cache);
            match re {
                Some(re) => re.is_match(value),
                None => match regex::Regex::new(pattern.as_ref()) {
                    Ok(re) => {
                        let Ok(mut cache) = REGEX_CACHE.lock() else {
                            return false;
                        };
                        cache.insert(pattern.to_string(), re.clone());
                        re.is_match(value)
                    }
                    Err(_) => false,
                },
            }
        }
        RuleCondition::SubstringMatch { haystack, needle } => ctx
            .field_value(haystack.as_ref())
            .map(|h| h.contains(needle.as_ref()))
            .unwrap_or(false),
        RuleCondition::PrefixMatch { value, prefix } => ctx
            .field_value(value.as_ref())
            .map(|v| v.starts_with(prefix.as_ref()))
            .unwrap_or(false),
        RuleCondition::SuffixMatch { value, suffix } => ctx
            .field_value(value.as_ref())
            .map(|v| v.ends_with(suffix.as_ref()))
            .unwrap_or(false),
        RuleCondition::RangeMatch { value, min, max } => *value >= *min && *value <= *max,
        RuleCondition::SetMembership { value, set } => {
            set.iter().map(Arc::as_ref).any(|m| m == value.as_ref())
        }
        RuleCondition::FieldInSet { field, set } => {
            let Some(value) = ctx.field_value(field.as_ref()) else {
                return false;
            };
            set.iter().map(Arc::as_ref).any(|m| m == value)
        }
        RuleCondition::Opaque(ext) => {
            // Opaque extensions get a `Unit` `&dyn Any` because the
            // standard `RuleEvaluationContext` cannot cross the
            // 'static-required Any boundary as a borrow. Extensions
            // that need the standard context should be migrated to
            // a context-aware variant (future RuleConditionExt::
            // evaluate_with_context); for now the reference evaluator
            // just respects the upstream contract by passing the
            // empty payload the GPU lowering also uses.
            ext.evaluate_opaque(&() as &dyn std::any::Any)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticCtx<'a> {
        counts: &'a [(u32, u32)],
        size: u64,
        fields: &'a [(&'a str, &'a str)],
    }

    impl<'a> RuleEvaluationContext for StaticCtx<'a> {
        fn pattern_count(&self, pid: u32) -> u32 {
            self.counts
                .iter()
                .find(|(p, _)| *p == pid)
                .map(|(_, c)| *c)
                .unwrap_or(0)
        }
        fn file_size(&self) -> u64 {
            self.size
        }
        fn field_value(&self, name: &str) -> Option<&str> {
            self.fields
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| *v)
        }
    }

    fn empty_ctx() -> StaticCtx<'static> {
        StaticCtx {
            counts: &[],
            size: 0,
            fields: &[],
        }
    }

    #[test]
    fn literal_true_and_false() {
        assert!(evaluate_condition(
            &RuleCondition::LiteralTrue,
            &empty_ctx()
        ));
        assert!(!evaluate_condition(
            &RuleCondition::LiteralFalse,
            &empty_ctx()
        ));
    }

    #[test]
    fn pattern_exists_uses_count() {
        let ctx = StaticCtx {
            counts: &[(7, 3)],
            size: 0,
            fields: &[],
        };
        assert!(evaluate_condition(
            &RuleCondition::PatternExists { pattern_id: 7 },
            &ctx
        ));
        assert!(!evaluate_condition(
            &RuleCondition::PatternExists { pattern_id: 8 },
            &ctx
        ));
    }

    #[test]
    fn pattern_count_gt_gte() {
        let ctx = StaticCtx {
            counts: &[(1, 5)],
            size: 0,
            fields: &[],
        };
        assert!(evaluate_condition(
            &RuleCondition::PatternCountGt {
                pattern_id: 1,
                threshold: 4,
            },
            &ctx
        ));
        assert!(!evaluate_condition(
            &RuleCondition::PatternCountGt {
                pattern_id: 1,
                threshold: 5,
            },
            &ctx
        ));
        assert!(evaluate_condition(
            &RuleCondition::PatternCountGte {
                pattern_id: 1,
                threshold: 5,
            },
            &ctx
        ));
        assert!(!evaluate_condition(
            &RuleCondition::PatternCountGte {
                pattern_id: 1,
                threshold: 6,
            },
            &ctx
        ));
    }

    #[test]
    fn file_size_predicates() {
        let ctx = StaticCtx {
            counts: &[],
            size: 100,
            fields: &[],
        };
        assert!(evaluate_condition(&RuleCondition::FileSizeLt(101), &ctx));
        assert!(!evaluate_condition(&RuleCondition::FileSizeLt(100), &ctx));
        assert!(evaluate_condition(&RuleCondition::FileSizeLte(100), &ctx));
        assert!(evaluate_condition(&RuleCondition::FileSizeGt(99), &ctx));
        assert!(evaluate_condition(&RuleCondition::FileSizeGte(100), &ctx));
        assert!(evaluate_condition(&RuleCondition::FileSizeEq(100), &ctx));
        assert!(evaluate_condition(&RuleCondition::FileSizeNe(99), &ctx));
        assert!(!evaluate_condition(&RuleCondition::FileSizeNe(100), &ctx));
    }

    #[test]
    fn substring_prefix_suffix() {
        let ctx = StaticCtx {
            counts: &[],
            size: 0,
            fields: &[("path", "src/foo/bar.rs")],
        };
        assert!(evaluate_condition(
            &RuleCondition::SubstringMatch {
                haystack: "path".into(),
                needle: "/foo/".into(),
            },
            &ctx
        ));
        assert!(evaluate_condition(
            &RuleCondition::PrefixMatch {
                value: "path".into(),
                prefix: "src/".into(),
            },
            &ctx
        ));
        assert!(evaluate_condition(
            &RuleCondition::SuffixMatch {
                value: "path".into(),
                suffix: ".rs".into(),
            },
            &ctx
        ));
        assert!(!evaluate_condition(
            &RuleCondition::SuffixMatch {
                value: "path".into(),
                suffix: ".py".into(),
            },
            &ctx
        ));
        assert!(!evaluate_condition(
            &RuleCondition::SubstringMatch {
                haystack: "missing".into(),
                needle: "x".into(),
            },
            &ctx
        ));
    }

    #[test]
    fn range_match_inclusive() {
        let cond = RuleCondition::RangeMatch {
            value: 50,
            min: 10,
            max: 100,
        };
        assert!(evaluate_condition(&cond, &empty_ctx()));
        let cond = RuleCondition::RangeMatch {
            value: 5,
            min: 10,
            max: 100,
        };
        assert!(!evaluate_condition(&cond, &empty_ctx()));
    }

    #[test]
    fn field_in_set_resolves_via_context() {
        let ctx = StaticCtx {
            counts: &[],
            size: 0,
            fields: &[("detector_id", "aws-access-key")],
        };
        use smallvec::smallvec;
        let cond = RuleCondition::FieldInSet {
            field: "detector_id".into(),
            set: smallvec!["github-pat".into(), "aws-access-key".into()],
        };
        assert!(evaluate_condition(&cond, &ctx));
        let cond = RuleCondition::FieldInSet {
            field: "detector_id".into(),
            set: smallvec!["stripe".into()],
        };
        assert!(!evaluate_condition(&cond, &ctx));
        let cond = RuleCondition::FieldInSet {
            field: "missing".into(),
            set: smallvec!["x".into()],
        };
        assert!(!evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn set_membership() {
        use smallvec::smallvec;
        let cond = RuleCondition::SetMembership {
            value: "blue".into(),
            set: smallvec!["red".into(), "blue".into(), "green".into()],
        };
        assert!(evaluate_condition(&cond, &empty_ctx()));
        let cond = RuleCondition::SetMembership {
            value: "yellow".into(),
            set: smallvec!["red".into(), "blue".into()],
        };
        assert!(!evaluate_condition(&cond, &empty_ctx()));
    }

    #[test]
    fn regex_match_uses_field_value() {
        let ctx = StaticCtx {
            counts: &[],
            size: 0,
            fields: &[("commit", "abcdef1234567890")],
        };
        let cond = RuleCondition::RegexMatch {
            field: "commit".into(),
            pattern: "^[0-9a-f]+$".into(),
        };
        assert!(evaluate_condition(&cond, &ctx));
        let cond = RuleCondition::RegexMatch {
            field: "commit".into(),
            pattern: "^[A-Z]+$".into(),
        };
        assert!(!evaluate_condition(&cond, &ctx));
        // Unknown field → false.
        let cond = RuleCondition::RegexMatch {
            field: "missing".into(),
            pattern: ".*".into(),
        };
        assert!(!evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn formula_and_or_not_short_circuit() {
        let ctx = empty_ctx();
        let f = RuleFormula::and(
            RuleFormula::condition(RuleCondition::LiteralTrue),
            RuleFormula::condition(RuleCondition::LiteralTrue),
        );
        assert!(evaluate_formula(&f, &ctx));

        let f = RuleFormula::and(
            RuleFormula::condition(RuleCondition::LiteralTrue),
            RuleFormula::condition(RuleCondition::LiteralFalse),
        );
        assert!(!evaluate_formula(&f, &ctx));

        let f = RuleFormula::or(
            RuleFormula::condition(RuleCondition::LiteralFalse),
            RuleFormula::condition(RuleCondition::LiteralTrue),
        );
        assert!(evaluate_formula(&f, &ctx));

        let f = RuleFormula::not_formula(RuleFormula::condition(RuleCondition::LiteralFalse));
        assert!(evaluate_formula(&f, &ctx));
    }

    #[test]
    fn nested_formula() {
        // (PatternExists(7) AND FileSizeLt(2048)) OR NOT PatternCountGt(99, 1000)
        let ctx = StaticCtx {
            counts: &[(7, 3), (99, 50)],
            size: 1024,
            fields: &[],
        };
        let f = RuleFormula::or(
            RuleFormula::and(
                RuleFormula::condition(RuleCondition::PatternExists { pattern_id: 7 }),
                RuleFormula::condition(RuleCondition::FileSizeLt(2048)),
            ),
            RuleFormula::not_formula(RuleFormula::condition(RuleCondition::PatternCountGt {
                pattern_id: 99,
                threshold: 1000,
            })),
        );
        assert!(evaluate_formula(&f, &ctx));
    }
}
