// Typed rule conditions and formula trees.
// TAG RESERVATIONS: PatternExists=0x01, PatternCountGt=0x02,
// PatternCountGte=0x03, FileSizeLt=0x04, FileSizeLte=0x05,
// FileSizeGt=0x06, FileSizeGte=0x07, FileSizeEq=0x08, FileSizeNe=0x09,
// LiteralTrue=0x0A, LiteralFalse=0x0B, RegexMatch=0x0C,
// SubstringMatch=0x0D, PrefixMatch=0x0E, SuffixMatch=0x0F,
// RangeMatch=0x10, SetMembership=0x11, 0x12..=0x7F reserved,
// Opaque=0x80.

use std::sync::Arc;

use crate::rule::builder;
use vyre_foundation::extension::RuleConditionExt;
use vyre_foundation::ir::{BufferDecl, Program};

/// A typed rule leaf condition.
///
/// `pattern_id` indexes the `rule_bitmaps` and `rule_counts` buffers used by
/// [`RuleFormula::to_program`]. File-size thresholds are accepted as `u64`;
/// thresholds above the current scalar IR file-size range are folded to their
/// mathematically forced result.
///
/// # Examples
///
/// ```
/// use vyre_libs::rule::RuleCondition;
///
/// let condition = RuleCondition::PatternCountGte {
///     pattern_id: 7,
///     threshold: 2,
/// };
/// assert!(matches!(condition, RuleCondition::PatternCountGte { .. }));
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RuleCondition {
    /// True when the pattern has any match state.
    PatternExists {
        /// Pattern table index.
        pattern_id: u32,
    },
    /// True when the pattern count is strictly greater than `threshold`.
    PatternCountGt {
        /// Pattern table index.
        pattern_id: u32,
        /// Exclusive lower bound.
        threshold: u32,
    },
    /// True when the pattern count is greater than or equal to `threshold`.
    PatternCountGte {
        /// Pattern table index.
        pattern_id: u32,
        /// Inclusive lower bound.
        threshold: u32,
    },
    /// True when the file size is less than the threshold.
    FileSizeLt(u64),
    /// True when the file size is less than or equal to the threshold.
    FileSizeLte(u64),
    /// True when the file size is greater than the threshold.
    FileSizeGt(u64),
    /// True when the file size is greater than or equal to the threshold.
    FileSizeGte(u64),
    /// True when the file size equals the threshold.
    FileSizeEq(u64),
    /// True when the file size does not equal the threshold.
    FileSizeNe(u64),
    /// Constant true leaf.
    LiteralTrue,
    /// Constant false leaf.
    LiteralFalse,
    /// True when text matched by `field` satisfies `pattern`.
    RegexMatch {
        /// Source field name.
        field: Arc<str>,
        /// Regular expression pattern.
        pattern: Arc<str>,
    },
    /// True when `haystack` contains `needle`.
    SubstringMatch {
        /// Source text or field name.
        haystack: Arc<str>,
        /// Required substring.
        needle: Arc<str>,
    },
    /// True when `value` starts with `prefix`.
    PrefixMatch {
        /// Source text or field name.
        value: Arc<str>,
        /// Required prefix.
        prefix: Arc<str>,
    },
    /// True when `value` ends with `suffix`.
    SuffixMatch {
        /// Source text or field name.
        value: Arc<str>,
        /// Required suffix.
        suffix: Arc<str>,
    },
    /// True when `value` falls inside the inclusive numeric range.
    RangeMatch {
        /// Observed value.
        value: u64,
        /// Inclusive lower bound.
        min: u64,
        /// Inclusive upper bound.
        max: u64,
    },
    /// True when `value` is present in `set`.
    SetMembership {
        /// Candidate value.
        value: Arc<str>,
        /// Accepted set members.
        set: smallvec::SmallVec<[Arc<str>; 4]>,
    },
    /// True when the value of context field `field` is present in
    /// `set`. Differs from [`Self::SetMembership`]: this variant
    /// dereferences `field` against the evaluation context, while
    /// `SetMembership` compares a static `value` payload.
    /// Lets a rule express "detector_id is one of …" without
    /// emulating it via a regex alternation.
    FieldInSet {
        /// Context field name to look up (e.g. `"detector_id"`).
        field: Arc<str>,
        /// Accepted set members.
        set: smallvec::SmallVec<[Arc<str>; 4]>,
    },
    /// Extension-declared rule condition.
    ///
    /// Downstream crates supply an `Arc<dyn RuleConditionExt>` with its
    /// own evaluator + required-buffer contract. The core rule builder
    /// rejects opaque conditions because it cannot lower them truthfully;
    /// extension-aware builders can call [`RuleConditionExt::required_buffers`]
    /// when wiring the extension to concrete IR.
    Opaque(Arc<dyn RuleConditionExt>),
}

impl PartialEq for RuleCondition {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PatternExists { pattern_id: a }, Self::PatternExists { pattern_id: b }) => {
                a == b
            }
            (
                Self::PatternCountGt {
                    pattern_id: a,
                    threshold: ta,
                },
                Self::PatternCountGt {
                    pattern_id: b,
                    threshold: tb,
                },
            ) => a == b && ta == tb,
            (
                Self::PatternCountGte {
                    pattern_id: a,
                    threshold: ta,
                },
                Self::PatternCountGte {
                    pattern_id: b,
                    threshold: tb,
                },
            ) => a == b && ta == tb,
            (Self::FileSizeLt(a), Self::FileSizeLt(b)) => a == b,
            (Self::FileSizeLte(a), Self::FileSizeLte(b)) => a == b,
            (Self::FileSizeGt(a), Self::FileSizeGt(b)) => a == b,
            (Self::FileSizeGte(a), Self::FileSizeGte(b)) => a == b,
            (Self::FileSizeEq(a), Self::FileSizeEq(b)) => a == b,
            (Self::FileSizeNe(a), Self::FileSizeNe(b)) => a == b,
            (Self::LiteralTrue, Self::LiteralTrue) => true,
            (Self::LiteralFalse, Self::LiteralFalse) => true,
            (
                Self::RegexMatch {
                    field: af,
                    pattern: ap,
                },
                Self::RegexMatch {
                    field: bf,
                    pattern: bp,
                },
            ) => af == bf && ap == bp,
            (
                Self::SubstringMatch {
                    haystack: ah,
                    needle: an,
                },
                Self::SubstringMatch {
                    haystack: bh,
                    needle: bn,
                },
            ) => ah == bh && an == bn,
            (
                Self::PrefixMatch {
                    value: av,
                    prefix: ap,
                },
                Self::PrefixMatch {
                    value: bv,
                    prefix: bp,
                },
            ) => av == bv && ap == bp,
            (
                Self::SuffixMatch {
                    value: av,
                    suffix: as_,
                },
                Self::SuffixMatch {
                    value: bv,
                    suffix: bs,
                },
            ) => av == bv && as_ == bs,
            (
                Self::RangeMatch {
                    value: av,
                    min: amin,
                    max: amax,
                },
                Self::RangeMatch {
                    value: bv,
                    min: bmin,
                    max: bmax,
                },
            ) => av == bv && amin == bmin && amax == bmax,
            (
                Self::SetMembership {
                    value: av,
                    set: aset,
                },
                Self::SetMembership {
                    value: bv,
                    set: bset,
                },
            ) => av == bv && aset == bset,
            (
                Self::FieldInSet {
                    field: af,
                    set: aset,
                },
                Self::FieldInSet {
                    field: bf,
                    set: bset,
                },
            ) => af == bf && aset == bset,
            (Self::Opaque(a), Self::Opaque(b)) => a.extension_id() == b.extension_id(),
            _ => false,
        }
    }
}

impl Eq for RuleCondition {}

impl RuleCondition {
    /// Return the buffer declarations this condition requires.
    ///
    /// Frozen conditions need only the six canonical rule buffers
    /// (`rule_ids`, `pattern_ids`, `rule_bitmaps`, `rule_counts`,
    /// `file_size`, `verdicts`). Extension conditions contribute extra
    /// buffers via [`RuleConditionExt::required_buffers`]  -  callers merge
    /// the results.
    #[must_use]
    pub fn required_extension_buffers(&self) -> Vec<BufferDecl> {
        match self {
            Self::Opaque(ext) => ext.required_buffers(),
            _ => Vec::new(),
        }
    }
}

/// A typed boolean rule formula tree.
///
/// Formula nodes compose typed conditions directly. They are not serialized
/// through an instruction stream and they do not require a runtime reducer.
///
/// # Examples
///
/// ```
/// use vyre_libs::rule::{RuleCondition, RuleFormula};
///
/// let formula = RuleFormula::and(
///     RuleFormula::condition(RuleCondition::PatternExists { pattern_id: 0 }),
///     RuleFormula::not(RuleFormula::condition(RuleCondition::LiteralFalse)),
/// );
/// let program = formula.to_program().expect("Fix: formula lowers");
/// assert!(program.has_buffer("verdicts"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuleFormula {
    /// Leaf condition.
    Condition(RuleCondition),
    /// Logical conjunction.
    And(Box<RuleFormula>, Box<RuleFormula>),
    /// Logical disjunction.
    Or(Box<RuleFormula>, Box<RuleFormula>),
    /// Logical negation.
    Not(Box<RuleFormula>),
}

impl RuleFormula {
    /// Create a leaf formula.
    #[must_use]
    pub fn condition(condition: RuleCondition) -> Self {
        Self::Condition(condition)
    }

    /// Create a conjunction.
    #[must_use]
    pub fn and(left: Self, right: Self) -> Self {
        Self::And(Box::new(left), Box::new(right))
    }

    /// Create a disjunction.
    #[must_use]
    pub fn or(left: Self, right: Self) -> Self {
        Self::Or(Box::new(left), Box::new(right))
    }

    /// Create a negation.
    #[must_use]
    pub fn not_formula(formula: Self) -> Self {
        Self::Not(Box::new(formula))
    }

    /// Create a negation.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn not(formula: Self) -> Self {
        Self::not_formula(formula)
    }

    /// Build a one-rule [`Program`] that stores the formula verdict at index 0.
    ///
    /// # Errors
    ///
    /// Returns [`builder::RuleBuildError`] when the formula contains a
    /// condition the core builder cannot lower truthfully.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre_libs::rule::{RuleCondition, RuleFormula};
    ///
    /// let program = RuleFormula::condition(RuleCondition::LiteralTrue)
    ///     .to_program()
    ///     .expect("Fix: literal rule lowers");
    /// assert!(program.has_buffer("rule_bitmaps"));
    /// assert!(program.has_buffer("verdicts"));
    /// ```
    #[must_use]
    pub fn to_program(&self) -> Result<Program, builder::RuleBuildError> {
        builder::build_rule_program(&[(self.clone(), 0)])
    }

    /// Try to build a one-rule [`Program`] that stores the formula verdict at
    /// index 0.
    ///
    /// # Errors
    ///
    /// Returns [`builder::RuleBuildError`] when the formula contains a
    /// condition the core builder cannot lower truthfully.
    pub fn try_to_program(&self) -> Result<Program, builder::RuleBuildError> {
        self.to_program()
    }
}
