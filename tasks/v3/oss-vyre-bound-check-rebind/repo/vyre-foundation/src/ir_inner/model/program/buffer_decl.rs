use std::ops::Range;
use std::sync::Arc;

use crate::ir_inner::model::types::{BufferAccess, DataType};

use super::{MemoryHints, MemoryKind};

/// Linear-type discipline for a buffer binding.
///
/// Vyre's IR is moving from an unrestricted-by-default world toward
/// a substructural type system: a buffer can be marked `Linear`
/// (must be used exactly once on each path through the Program),
/// `Affine` (used at most once  -  drops are fine), `Relevant`
/// (used at least once), or `Unrestricted` (the historical default).
/// The type-checker pass (P-1.0-V2.2) verifies these assertions
/// before lowering; backends that hit a violation reject the
/// program at validation time instead of producing wrong code.
///
/// `Unrestricted` is the safe default when authoring a `BufferDecl`
/// for back-compat  -  every existing program continues to type-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LinearType {
    /// Use exactly once on every path. Forbids both drop-without-use
    /// and double-use.
    Linear,
    /// Use at most once on every path. Allows drop-without-use,
    /// forbids double-use.
    Affine,
    /// Use at least once on every path. Forbids drop-without-use,
    /// allows double-use.
    Relevant,
    /// No discipline applied. Default for back-compat with the
    /// pre-V2.x IR.
    #[default]
    Unrestricted,
}

impl LinearType {
    /// Whether this discipline forbids dropping a buffer without
    /// using it (`Linear` or `Relevant`).
    #[must_use]
    #[inline]
    pub const fn forbids_drop(self) -> bool {
        matches!(self, Self::Linear | Self::Relevant)
    }

    /// Whether this discipline forbids using a buffer more than once
    /// (`Linear` or `Affine`).
    #[must_use]
    #[inline]
    pub const fn forbids_reuse(self) -> bool {
        matches!(self, Self::Linear | Self::Affine)
    }
}

/// Refinement predicate over a buffer's element count (P-1.0-V3.1).
///
/// Represents a small grammar of constraints a `BufferDecl` author
/// can attach. The validator (P-1.0-V3.2) checks each predicate
/// against the program's static count and the optimizer (P-1.0-V3.3)
/// uses verified predicates to prove loop-bound and alignment
/// invariants for vectorization.
///
/// `None` (the default) is "unconstrained"; existing programs keep
/// their current behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ShapePredicate {
    /// `count >= n`. Holds when the runtime element count is at
    /// least `n`. Used to prove non-empty workgroup buffers and
    /// minimum vectorization tile sizes.
    AtLeast(u32),
    /// `count <= n`. Holds when the count never exceeds `n`. Used
    /// to bound dispatch sizes and prevent oversized allocations.
    AtMost(u32),
    /// `count == n`. The strongest constraint; the count is fixed.
    Exactly(u32),
    /// `count % n == 0`. Used for alignment proofs (e.g. SIMD lanes).
    MultipleOf(u32),
    /// `count % modulus == remainder`. Invalid modular forms evaluate
    /// false, so static validation catches impossible declarations.
    ModEquals {
        /// Divisor used by the modular equality.
        modulus: u32,
        /// Required remainder. Must be less than `modulus` to match.
        remainder: u32,
    },
    /// `min <= count * scale + offset <= max`, evaluated with wide
    /// arithmetic for frontend-derived affine constraints.
    AffineRange {
        /// Multiplicative coefficient applied to `count`.
        scale: i64,
        /// Constant term added after scaling.
        offset: i64,
        /// Inclusive lower bound for the affine expression.
        min: i64,
        /// Inclusive upper bound for the affine expression.
        max: i64,
    },
    /// Conjunction of two predicates (`p1 && p2`). Both must hold.
    And(Box<ShapePredicate>, Box<ShapePredicate>),
    /// Disjunction of two predicates (`p1 || p2`). Either may hold.
    Or(Box<ShapePredicate>, Box<ShapePredicate>),
    /// Negation of a predicate.
    Not(Box<ShapePredicate>),
}

impl ShapePredicate {
    /// Evaluate the predicate against a concrete `count`. Returns
    /// `true` when the predicate holds. P-1.0-V3.2 uses this from
    /// the `validate()` pass; P-1.0-V3.3 calls it from optimizer
    /// passes that need a yes/no proof.
    #[must_use]
    pub fn holds(&self, count: u32) -> bool {
        match self {
            Self::AtLeast(n) => count >= *n,
            Self::AtMost(n) => count <= *n,
            Self::Exactly(n) => count == *n,
            Self::MultipleOf(n) => *n != 0 && count % *n == 0,
            Self::ModEquals { modulus, remainder } => {
                *modulus != 0 && *remainder < *modulus && count % *modulus == *remainder
            }
            Self::AffineRange {
                scale,
                offset,
                min,
                max,
            } => {
                let value = i128::from(count) * i128::from(*scale) + i128::from(*offset);
                value >= i128::from(*min) && value <= i128::from(*max)
            }
            Self::And(a, b) => a.holds(count) && b.holds(count),
            Self::Or(a, b) => a.holds(count) || b.holds(count),
            Self::Not(inner) => !inner.holds(count),
        }
    }

    /// Human-readable form for error messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::AtLeast(n) => format!("count >= {n}"),
            Self::AtMost(n) => format!("count <= {n}"),
            Self::Exactly(n) => format!("count == {n}"),
            Self::MultipleOf(n) => format!("count % {n} == 0"),
            Self::ModEquals { modulus, remainder } => format!("count % {modulus} == {remainder}"),
            Self::AffineRange {
                scale,
                offset,
                min,
                max,
            } => {
                format!("{min} <= count * {scale} + {offset} <= {max}")
            }
            Self::And(a, b) => format!("({}) && ({})", a.describe(), b.describe()),
            Self::Or(a, b) => format!("({}) || ({})", a.describe(), b.describe()),
            Self::Not(inner) => format!("!({})", inner.describe()),
        }
    }
}

/// A named buffer binding in a program.
///
/// # Examples
///
/// ```
/// use vyre::ir::{BufferDecl, BufferAccess, DataType};
///
/// let buf = BufferDecl::read("input", 0, DataType::U32);
/// assert_eq!(buf.name(), "input");
/// assert_eq!(buf.binding(), 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferDecl {
    /// Human-readable name. Referenced by `Expr::Load`, `Node::Store`, etc.
    pub name: Arc<str>,
    /// Binding slot: `@binding(N)`. All buffers are in `@group(0)`.
    /// Ignored for `BufferAccess::Workgroup`.
    pub binding: u32,
    /// Access mode.
    pub access: BufferAccess,
    /// Memory tier.
    pub kind: MemoryKind,
    /// Element data type.
    pub element: DataType,
    /// Number of elements.
    ///
    /// For `Workgroup` memory this is the static array length.
    /// For storage and uniform buffers this is `0` (runtime-sized).
    pub count: u32,
    /// Whether this buffer is the scalar expression output for composition inlining.
    pub is_output: bool,
    /// Whether the end-to-end pipeline reads this buffer after Program execution.
    ///
    /// Passes must treat this as an externally-visible sink even when the IR
    /// itself does not read the buffer again.
    pub pipeline_live_out: bool,
    /// Optional byte range to read back from this output buffer.
    ///
    /// `None` preserves the historical behavior and reads back the full
    /// declared output buffer.
    pub output_byte_range: Option<Range<usize>>,
    /// Non-binding backend optimization hints.
    pub hints: MemoryHints,
    /// When true, admits `DataType::Bytes` load/store despite V013.
    ///
    /// Bytes-producing or bytes-extraction ops (decode.base64,
    /// `compression.lz4_decompress`, `match.dfa_scan` position emission, etc.)
    /// opt into V013 relaxation per-buffer. Default false keeps scalar
    /// arithmetic protected from accidental bytes-blob reinterpretation.
    pub bytes_extraction: bool,
    /// Linear-type discipline for this buffer (P-1.0-V2.1).
    ///
    /// Defaults to `LinearType::Unrestricted` so existing programs
    /// continue to type-check. Authors opt in by calling
    /// [`BufferDecl::with_linear_type`]. The type-checker pass
    /// (`crate::validate::linear_type`) walks the IR and
    /// rejects programs that violate the declared discipline; backends
    /// that hit a violation surface it as a validation error before
    /// lowering.
    pub linear_type: LinearType,
    /// Optional shape-refinement predicate (P-1.0-V3.1).
    ///
    /// `None` is the default (no shape constraint, identical to the
    /// pre-V3.x IR). Authors opt in via
    /// [`BufferDecl::with_shape_predicate`]. The validator
    /// ([`crate::validate::shape_predicate::check_shape_predicates`])
    /// evaluates each predicate against the program's static `count`
    /// at `validate()` time and rejects programs whose static shape
    /// contradicts the declaration.
    pub shape_predicate: Option<ShapePredicate>,
}

impl BufferDecl {
    /// Create a storage buffer declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferDecl, BufferAccess, DataType};
    /// let _ = BufferDecl::storage("a", 0, BufferAccess::ReadOnly, DataType::U32);
    /// ```
    #[must_use]
    #[inline]
    pub fn storage(name: &str, binding: u32, access: BufferAccess, element: DataType) -> Self {
        let kind = match &access {
            BufferAccess::ReadOnly => MemoryKind::Readonly,
            BufferAccess::Uniform => MemoryKind::Uniform,
            BufferAccess::Workgroup => MemoryKind::Shared,
            _ => MemoryKind::Global,
        };
        Self {
            name: Arc::from(name),
            binding,
            access,
            kind,
            element,
            count: 0,
            is_output: false,
            pipeline_live_out: false,
            output_byte_range: None,
            hints: MemoryHints::default(),
            bytes_extraction: false,
            linear_type: LinearType::default(),
            shape_predicate: None,
        }
    }

    /// Shorthand for a read-only storage buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferDecl, DataType};
    /// let _ = BufferDecl::read("a", 0, DataType::U32);
    /// ```
    #[must_use]
    #[inline]
    pub fn read(name: &str, binding: u32, element: DataType) -> Self {
        Self::storage(name, binding, BufferAccess::ReadOnly, element)
    }

    /// Shorthand for a read-write storage buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferDecl, DataType};
    /// let _ = BufferDecl::read_write("a", 0, DataType::U32);
    /// ```
    #[must_use]
    #[inline]
    pub fn read_write(name: &str, binding: u32, element: DataType) -> Self {
        Self::storage(name, binding, BufferAccess::ReadWrite, element)
    }

    /// Shorthand for the read-write result buffer used by call inlining.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferDecl, DataType};
    /// let _ = BufferDecl::output("a", 0, DataType::U32);
    /// ```
    #[must_use]
    #[inline]
    pub fn output(name: &str, binding: u32, element: DataType) -> Self {
        Self {
            is_output: true,
            pipeline_live_out: true,
            ..Self::read_write(name, binding, element)
        }
    }

    /// Mark whether a caller/backend observes this buffer after Program execution.
    #[must_use]
    #[inline]
    pub fn with_pipeline_live_out(mut self, flag: bool) -> Self {
        self.pipeline_live_out = flag;
        self
    }

    /// Attach an output byte range for backends that can read back a slice.
    #[must_use]
    #[inline]
    pub fn with_output_byte_range(mut self, range: Range<usize>) -> Self {
        self.output_byte_range = Some(range);
        self
    }

    /// Set the static element count for storage-style buffers.
    ///
    /// Set the element count. A count of `0` retains the IR's
    /// runtime-sized-buffer representation; validators reject zero-sized
    /// workgroup allocations before dispatch.
    #[must_use]
    #[inline]
    pub fn with_count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }

    /// Shorthand for a uniform buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferDecl, DataType};
    /// let _ = BufferDecl::uniform("a", 0, DataType::U32);
    /// ```
    #[must_use]
    #[inline]
    pub fn uniform(name: &str, binding: u32, element: DataType) -> Self {
        Self::storage(name, binding, BufferAccess::Uniform, element)
    }

    /// Shorthand for a workgroup-local shared array.
    ///
    /// `count` is the static number of elements visible to all invocations
    /// in the same workgroup.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::{BufferAccess, BufferDecl, DataType, MemoryKind};
    ///
    /// let scratch = BufferDecl::workgroup("scratch", 64, DataType::U32);
    ///
    /// assert_eq!(scratch.name(), "scratch");
    /// assert_eq!(scratch.access(), BufferAccess::Workgroup);
    /// assert_eq!(scratch.kind(), MemoryKind::Shared);
    /// assert_eq!(scratch.count(), 64);
    /// ```
    #[must_use]
    #[inline]
    pub fn workgroup(name: &str, count: u32, element: DataType) -> Self {
        Self {
            name: Arc::from(name),
            binding: 0,
            access: BufferAccess::Workgroup,
            kind: MemoryKind::Shared,
            element,
            count,
            is_output: false,
            pipeline_live_out: false,
            output_byte_range: None,
            hints: MemoryHints::default(),
            bytes_extraction: false,
            linear_type: LinearType::default(),
            shape_predicate: None,
        }
    }

    /// Mark this buffer as a bytes-extraction context so V013 admits Bytes load/store.
    #[must_use]
    #[inline]
    pub fn with_bytes_extraction(mut self, flag: bool) -> Self {
        self.bytes_extraction = flag;
        self
    }

    /// Set the linear-type discipline (P-1.0-V2.1).
    ///
    /// Defaults to [`LinearType::Unrestricted`] from the constructor;
    /// the type-checker pass enforces stricter disciplines when set.
    #[must_use]
    #[inline]
    pub fn with_linear_type(mut self, linear_type: LinearType) -> Self {
        self.linear_type = linear_type;
        self
    }

    /// Set the shape-refinement predicate (P-1.0-V3.1).
    ///
    /// Defaults to `None` (unconstrained); the validator
    /// ([`crate::validate::shape_predicate::check_shape_predicates`])
    /// rejects programs whose static `count` violates the predicate.
    #[must_use]
    #[inline]
    pub fn with_shape_predicate(mut self, predicate: ShapePredicate) -> Self {
        self.shape_predicate = Some(predicate);
        self
    }

    /// Override the memory tier.
    #[must_use]
    #[inline]
    pub fn with_kind(mut self, kind: MemoryKind) -> Self {
        self.kind = kind;
        self
    }

    /// Override memory optimization hints.
    #[must_use]
    #[inline]
    pub fn with_hints(mut self, hints: MemoryHints) -> Self {
        self.hints = hints;
        self
    }

    /// Buffer name.
    #[must_use]
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Binding slot.
    #[must_use]
    #[inline]
    pub fn binding(&self) -> u32 {
        self.binding
    }

    /// Buffer access mode.
    #[must_use]
    #[inline]
    pub fn access(&self) -> BufferAccess {
        self.access.clone()
    }

    /// Memory tier.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> MemoryKind {
        self.kind
    }

    /// Non-binding memory hints.
    #[must_use]
    #[inline]
    pub fn hints(&self) -> MemoryHints {
        self.hints
    }

    /// Element data type.
    #[must_use]
    #[inline]
    pub fn element(&self) -> DataType {
        self.element.clone()
    }

    /// Static element count for workgroup buffers.
    #[must_use]
    #[inline]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Static packed byte length for fixed-size buffers.
    ///
    /// Returns `Ok(None)` for runtime-sized buffer declarations (`count == 0`)
    /// and for fixed-count buffers whose element type is runtime-sized. Sub-byte
    /// element types use their packed bit width, so three `I4` elements occupy
    /// two bytes rather than three conservative one-byte lanes.
    ///
    /// # Errors
    ///
    /// Returns an actionable diagnostic when the packed byte count overflows.
    pub fn static_byte_len(&self) -> Result<Option<usize>, String> {
        let count = usize::try_from(self.count).map_err(|error| {
            format!(
                "buffer `{}` static element count {} cannot fit usize ({error}). Fix: split the buffer or reduce its element count.",
                self.name, self.count
            )
        })?;
        if count == 0 {
            return Ok(None);
        }
        self.element.packed_size_bytes(count).map_err(|error| {
            format!(
                "buffer `{}` static byte length could not be computed: {error}. Fix: use a fixed-width element type or split the buffer.",
                self.name
            )
        })
    }

    /// Return true when this buffer is the unique inlining result buffer.
    #[must_use]
    #[inline]
    pub fn is_output(&self) -> bool {
        self.is_output
    }

    /// Return true when the buffer must survive IR-local deadness analysis.
    #[must_use]
    #[inline]
    pub fn is_pipeline_live_out(&self) -> bool {
        self.pipeline_live_out
    }

    /// Byte range the consumer needs from this output buffer, if declared.
    #[must_use]
    #[inline]
    pub fn output_byte_range(&self) -> Option<Range<usize>> {
        self.output_byte_range.clone()
    }

    /// Linear-type discipline (P-1.0-V2.1).
    #[must_use]
    #[inline]
    pub fn linear_type(&self) -> LinearType {
        self.linear_type
    }

    /// Shape-refinement predicate (P-1.0-V3.1).
    #[must_use]
    #[inline]
    pub fn shape_predicate(&self) -> Option<&ShapePredicate> {
        self.shape_predicate.as_ref()
    }
}

#[cfg(test)]

mod linear_type_tests {
    use super::*;

    #[test]
    fn default_is_unrestricted() {
        let buf = BufferDecl::read("a", 0, DataType::U32);
        assert_eq!(buf.linear_type(), LinearType::Unrestricted);
        assert!(!LinearType::Unrestricted.forbids_drop());
        assert!(!LinearType::Unrestricted.forbids_reuse());
    }

    #[test]
    fn linear_forbids_both() {
        assert!(LinearType::Linear.forbids_drop());
        assert!(LinearType::Linear.forbids_reuse());
    }

    #[test]
    fn affine_forbids_only_reuse() {
        assert!(!LinearType::Affine.forbids_drop());
        assert!(LinearType::Affine.forbids_reuse());
    }

    #[test]
    fn relevant_forbids_only_drop() {
        assert!(LinearType::Relevant.forbids_drop());
        assert!(!LinearType::Relevant.forbids_reuse());
    }

    #[test]
    fn with_linear_type_is_round_trip() {
        for lt in [
            LinearType::Linear,
            LinearType::Affine,
            LinearType::Relevant,
            LinearType::Unrestricted,
        ] {
            let buf = BufferDecl::read("a", 0, DataType::U32).with_linear_type(lt);
            assert_eq!(buf.linear_type(), lt);
        }
    }

    #[test]
    fn workgroup_constructor_defaults_to_unrestricted() {
        let buf = BufferDecl::workgroup("scratch", 64, DataType::U32);
        assert_eq!(buf.linear_type(), LinearType::Unrestricted);
    }

    #[test]
    fn static_byte_len_uses_packed_subbyte_width() {
        let buf = BufferDecl::read("packed_i4", 0, DataType::I4).with_count(3);
        assert_eq!(
            buf.static_byte_len()
                .expect("Fix: packed I4 byte length must compute"),
            Some(2)
        );
    }

    #[test]
    fn static_byte_len_marks_runtime_sized_buffers_dynamic() {
        let zero_count = BufferDecl::read("dynamic_count", 0, DataType::U32);
        assert_eq!(
            zero_count
                .static_byte_len()
                .expect("Fix: zero-count buffer must be representable"),
            None
        );

        let dynamic_element = BufferDecl::read("tensor", 0, DataType::Tensor).with_count(4);
        assert_eq!(
            dynamic_element
                .static_byte_len()
                .expect("Fix: runtime-sized element must be representable"),
            None
        );
    }
}

#[cfg(test)]
mod shape_predicate_tests {
    use super::*;

    #[test]
    fn at_least_holds_when_count_meets_minimum() {
        let p = ShapePredicate::AtLeast(64);
        assert!(p.holds(64));
        assert!(p.holds(128));
        assert!(!p.holds(32));
    }

    #[test]
    fn at_most_holds_when_count_within_bound() {
        let p = ShapePredicate::AtMost(64);
        assert!(p.holds(0));
        assert!(p.holds(64));
        assert!(!p.holds(65));
    }

    #[test]
    fn exactly_holds_only_for_match() {
        let p = ShapePredicate::Exactly(7);
        assert!(p.holds(7));
        assert!(!p.holds(6));
        assert!(!p.holds(8));
    }

    #[test]
    fn multiple_of_holds_for_aligned_count() {
        let p = ShapePredicate::MultipleOf(64);
        assert!(p.holds(0));
        assert!(p.holds(64));
        assert!(p.holds(128));
        assert!(!p.holds(63));
        assert!(!p.holds(65));
    }

    #[test]
    fn multiple_of_zero_never_holds() {
        let p = ShapePredicate::MultipleOf(0);
        assert!(!p.holds(0));
        assert!(!p.holds(64));
    }

    #[test]
    fn and_combines_two_predicates() {
        // count >= 64 && count % 32 == 0
        let p = ShapePredicate::And(
            Box::new(ShapePredicate::AtLeast(64)),
            Box::new(ShapePredicate::MultipleOf(32)),
        );
        assert!(p.holds(64));
        assert!(p.holds(96));
        assert!(!p.holds(32)); // satisfies MultipleOf but not AtLeast
        assert!(!p.holds(80)); // satisfies AtLeast but not MultipleOf
    }

    #[test]
    fn or_accepts_either_predicate() {
        let p = ShapePredicate::Or(
            Box::new(ShapePredicate::Exactly(8)),
            Box::new(ShapePredicate::Exactly(16)),
        );
        assert!(p.holds(8));
        assert!(p.holds(16));
        assert!(!p.holds(12));
    }

    #[test]
    fn not_inverts_predicate() {
        let p = ShapePredicate::Not(Box::new(ShapePredicate::AtMost(64)));
        assert!(!p.holds(64));
        assert!(p.holds(65));
    }

    #[test]
    fn mod_equals_requires_valid_modular_form() {
        assert!(ShapePredicate::ModEquals {
            modulus: 16,
            remainder: 4,
        }
        .holds(20));
        assert!(!ShapePredicate::ModEquals {
            modulus: 16,
            remainder: 4,
        }
        .holds(21));
        assert!(!ShapePredicate::ModEquals {
            modulus: 0,
            remainder: 0,
        }
        .holds(0));
        assert!(!ShapePredicate::ModEquals {
            modulus: 4,
            remainder: 4,
        }
        .holds(4));
    }

    #[test]
    fn affine_range_uses_wide_arithmetic() {
        let p = ShapePredicate::AffineRange {
            scale: 4,
            offset: -8,
            min: 24,
            max: 40,
        };
        assert!(!p.holds(7));
        assert!(p.holds(8));
        assert!(p.holds(12));
        assert!(!p.holds(13));
        assert!(!ShapePredicate::AffineRange {
            scale: i64::MAX,
            offset: i64::MAX,
            min: i64::MIN,
            max: i64::MAX,
        }
        .holds(u32::MAX));
    }

    #[test]
    fn buffer_decl_default_shape_predicate_is_none() {
        let buf = BufferDecl::read("a", 0, DataType::U32);
        assert_eq!(buf.shape_predicate(), None);
    }

    #[test]
    fn with_shape_predicate_round_trip() {
        let buf = BufferDecl::read("a", 0, DataType::U32)
            .with_shape_predicate(ShapePredicate::MultipleOf(32));
        assert_eq!(buf.shape_predicate(), Some(&ShapePredicate::MultipleOf(32)));
    }

    #[test]
    fn describe_renders_human_readable() {
        assert_eq!(
            ShapePredicate::And(
                Box::new(ShapePredicate::AtLeast(64)),
                Box::new(ShapePredicate::MultipleOf(32)),
            )
            .describe(),
            "(count >= 64) && (count % 32 == 0)"
        );
    }
}
