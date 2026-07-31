// Expression nodes  -  produce values.
//
// Every expression evaluates to a typed value. Expressions are pure:
// they read state but do not modify it.

use crate::ir_inner::model::types::{DataType, SubgroupReduceOp};
use rustc_hash::FxHasher;
use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

/// Reference to the generator/macro that produced an AST region.
/// Used for source-mapping and DWARF-like debugging context.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GeneratorRef {
    /// The name of the generator (e.g., `vyre-nn::flash_attention`).
    pub name: String,
}

/// Interned identifier used by expression nodes.
///
/// `Ident` is cheap to clone and keeps expression trees from repeatedly
/// allocating owned `String` values for the same variable or buffer names.
#[derive(Clone, Eq, PartialEq)]
pub struct Ident {
    text: Arc<str>,
    hash: u64,
}

impl Ident {
    #[inline]
    fn prehash(text: &str) -> u64 {
        let mut hasher = FxHasher::default();
        text.hash(&mut hasher);
        hasher.finish()
    }

    #[must_use]
    #[inline]
    /// Construct an identifier from shared text while caching its hash once.
    pub fn new(text: Arc<str>) -> Self {
        let hash = Self::prehash(&text);
        Self { text, hash }
    }

    /// Clone the underlying interned string handle without copying UTF-8 bytes.
    #[must_use]
    #[inline]
    pub fn shared_text(&self) -> Arc<str> {
        Arc::clone(&self.text)
    }

    /// Return another identifier handle to the same interned text without
    /// reallocating text or recomputing the cached hash.
    #[must_use]
    #[inline]
    pub fn duplicate_handle(&self) -> Self {
        Self {
            text: Arc::clone(&self.text),
            hash: self.hash,
        }
    }

    /// Return the identifier text.
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Return the cached hash used by hash-map/set lookups.
    #[must_use]
    #[inline]
    pub fn cached_hash(&self) -> u64 {
        self.hash
    }
}

impl From<&str> for Ident {
    #[inline]
    fn from(value: &str) -> Self {
        Self::new(Arc::from(value))
    }
}

impl From<String> for Ident {
    #[inline]
    fn from(value: String) -> Self {
        Self::new(Arc::from(value))
    }
}

impl From<Arc<str>> for Ident {
    #[inline]
    fn from(value: Arc<str>) -> Self {
        Self::new(value)
    }
}

impl From<&String> for Ident {
    #[inline]
    fn from(value: &String) -> Self {
        Self::from(value.as_str())
    }
}

impl From<&Ident> for Ident {
    #[inline]
    fn from(value: &Ident) -> Self {
        value.clone()
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Ident").field(&self.as_str()).finish()
    }
}

impl Hash for Ident {
    /// Audit P-IDENT-BORROW (2026-04-29): hash via the underlying str so the
    /// `Hash` impl matches the `Borrow<str>` impl, preserving the
    /// `HashMap::get<Q: Borrow<K> + Hash + Eq>` invariant. The
    /// pre-fix `state.write_u64(self.hash)` produced a different u64 than
    /// `<str as Hash>::hash` for the same hasher (which writes bytes + a
    /// length terminator), so any `FxHashMap<Ident, V>::get(&str)` lookup
    /// silently missed the inserted entry. Callers that want the cached
    /// `FxHash` for a fast equality-check key call [`Ident::cached_hash`]
    /// directly.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl Deref for Ident {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for Ident {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Ident {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Ident {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for Ident {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Ident {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialOrd for Ident {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ident {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// An expression that produces a value.
///
/// # Examples
///
/// ```
/// use vyre::ir::Expr;
///
/// let lit = Expr::u32(42);
/// let var = Expr::var("x");
/// let add = Expr::add(lit, var);
/// ```
pub use crate::ir_inner::model::generated::Expr;

/// Public contract for downstream expression extension nodes.
///
/// Extension nodes are intentionally opaque to core. A downstream crate owns
/// the semantic payload and provides the stable metadata core needs for
/// validation, debug output, equality, and CSE identity. Backends that
/// understand the extension can downcast through their own wrapper type before
/// constructing target code; backends that do not understand it must reject it
/// with an actionable error.
pub trait ExprNode: fmt::Debug + Send + Sync + 'static {
    /// Stable extension namespace, for example `my_backend.tensor.shuffle`.
    fn extension_kind(&self) -> &'static str;

    /// Human-readable identity used in diagnostics and debug logs.
    fn debug_identity(&self) -> &str;

    /// Static result type produced by this expression.
    fn result_type(&self) -> Option<DataType>;

    /// Whether CSE may treat this extension as a pure, repeatable expression.
    fn cse_safe(&self) -> bool;

    /// Stable, content-addressed identity for equality and optimizer keys.
    fn stable_fingerprint(&self) -> [u8; 32];

    /// Validate extension-local invariants.
    ///
    /// # Errors
    ///
    /// The returned error must explain the bad invariant and include `Fix:`.
    fn validate_extension(&self) -> Result<(), String>;

    /// Downcast to Any to allow backend-specific dispatch from opaque payloads.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Serialize the extension payload into stable bytes used by the wire
    /// encoder's `Expr::Opaque` path (tag `0x80`). Default: empty payload  -
    /// suitable for extensions that carry no state beyond their type
    /// identity. Extensions with state must override this to emit the exact
    /// bytes `wire_payload`'s matching `OpaqueExprResolver` will consume.
    ///
    /// The payload contract is endian-fixed: any numeric field wider than
    /// one byte MUST be written with `to_le_bytes`, and the matching decoder
    /// MUST reconstruct it with `from_le_bytes`. Host-endian encodings such as
    /// `to_ne_bytes` are forbidden because the wire format must stay
    /// byte-identical across architectures.
    ///
    /// Extension authors are recommended (but not required, for API
    /// compatibility) to use [`crate::opaque_payload::LeBytesWriter`] when
    /// building payloads  -  it makes the right endianness the only choice at
    /// the type level.
    ///
    /// Literal extensions that encode regex payloads must also canonicalize
    /// inline flag prefixes before emitting bytes. For example, `(?mi)` and
    /// `(?im)` are the same semantic payload and MUST serialize to the same
    /// flag ordering.
    fn wire_payload(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl Expr {
    /// Load from buffer at index.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Expr;
    /// let _ = Expr::load("a", Expr::u32(0));
    /// ```
    #[must_use]
    #[inline]
    pub fn load(buffer: impl Into<Ident>, index: Self) -> Self {
        Self::Load {
            buffer: buffer.into(),
            index: Box::new(index),
        }
    }

    /// Buffer element count.
    ///
    /// # Examples
    ///
    /// ```
    /// use vyre::ir::Expr;
    /// let _ = Expr::buf_len("a");
    /// ```
    #[must_use]
    #[inline]
    pub fn buf_len(buffer: impl Into<Ident>) -> Self {
        Self::BufLen {
            buffer: buffer.into(),
        }
    }

    /// `global_invocation_id.x`
    #[must_use]
    #[inline]
    pub fn gid_x() -> Self {
        Self::InvocationId { axis: 0 }
    }

    /// `global_invocation_id.y`
    #[must_use]
    #[inline]
    pub fn gid_y() -> Self {
        Self::InvocationId { axis: 1 }
    }

    /// `global_invocation_id.z`
    #[must_use]
    #[inline]
    pub fn gid_z() -> Self {
        Self::InvocationId { axis: 2 }
    }

    /// `workgroup_id.x`
    #[must_use]
    #[inline]
    pub fn workgroup_x() -> Self {
        Self::WorkgroupId { axis: 0 }
    }

    /// `workgroup_id.y`
    #[must_use]
    #[inline]
    pub fn workgroup_y() -> Self {
        Self::WorkgroupId { axis: 1 }
    }

    /// `workgroup_id.z`
    #[must_use]
    #[inline]
    pub fn workgroup_z() -> Self {
        Self::WorkgroupId { axis: 2 }
    }

    /// `local_invocation_id.x`
    #[must_use]
    #[inline]
    pub fn local_x() -> Self {
        Self::LocalId { axis: 0 }
    }

    /// `subgroup_invocation_id` (lane index within subgroup).
    #[must_use]
    #[inline]
    pub fn subgroup_local_id() -> Self {
        Self::SubgroupLocalId
    }

    /// `subgroup_size` (number of lanes per subgroup).
    #[must_use]
    #[inline]
    pub fn subgroup_size() -> Self {
        Self::SubgroupSize
    }

    /// `local_invocation_id.y`
    #[must_use]
    #[inline]
    pub fn local_y() -> Self {
        Self::LocalId { axis: 1 }
    }

    /// `local_invocation_id.z`
    #[must_use]
    #[inline]
    pub fn local_z() -> Self {
        Self::LocalId { axis: 2 }
    }

    /// Substrate-neutral alias for [`workgroup_x`](Self::workgroup_x).
    ///
    /// "Parallel region" is the vocabulary used in vyre-core's public
    /// surface. Concrete drivers translate this concept into their own
    /// target vocabulary at the boundary.
    #[must_use]
    #[inline]
    pub fn parallel_region_x() -> Self {
        Self::WorkgroupId { axis: 0 }
    }

    /// Substrate-neutral alias for [`workgroup_y`](Self::workgroup_y).
    #[must_use]
    #[inline]
    pub fn parallel_region_y() -> Self {
        Self::WorkgroupId { axis: 1 }
    }

    /// Substrate-neutral alias for [`workgroup_z`](Self::workgroup_z).
    #[must_use]
    #[inline]
    pub fn parallel_region_z() -> Self {
        Self::WorkgroupId { axis: 2 }
    }

    /// Substrate-neutral alias for [`local_x`](Self::local_x).
    #[must_use]
    #[inline]
    pub fn invocation_local_x() -> Self {
        Self::LocalId { axis: 0 }
    }

    /// Substrate-neutral alias for [`local_y`](Self::local_y).
    #[must_use]
    #[inline]
    pub fn invocation_local_y() -> Self {
        Self::LocalId { axis: 1 }
    }

    /// Substrate-neutral alias for [`local_z`](Self::local_z).
    #[must_use]
    #[inline]
    pub fn invocation_local_z() -> Self {
        Self::LocalId { axis: 2 }
    }

    /// Conditional select.
    #[must_use]
    #[inline]
    pub fn select(cond: Self, true_val: Self, false_val: Self) -> Self {
        Self::Select {
            cond: Box::new(cond),
            true_val: Box::new(true_val),
            false_val: Box::new(false_val),
        }
    }

    /// Subgroup reduction across the active subgroup with the given operator.
    #[must_use]
    #[inline]
    pub fn subgroup_reduce(op: SubgroupReduceOp, value: Self) -> Self {
        Self::SubgroupReduce {
            op,
            value: Box::new(value),
        }
    }

    /// Subgroup sum reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_add(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Add, value)
    }

    /// Subgroup product reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_mul(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Mul, value)
    }

    /// Subgroup minimum reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_min(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Min, value)
    }

    /// Subgroup maximum reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_max(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Max, value)
    }

    /// Subgroup bitwise-AND reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_and(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::And, value)
    }

    /// Subgroup bitwise-OR reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_or(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Or, value)
    }

    /// Subgroup bitwise-XOR reduction across the active subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_xor(value: Self) -> Self {
        Self::subgroup_reduce(SubgroupReduceOp::Xor, value)
    }

    /// Subgroup shuffle: broadcast `value` from the given lane id to
    /// every active lane in the subgroup.
    #[must_use]
    #[inline]
    pub fn subgroup_shuffle(value: Self, lane: Self) -> Self {
        Self::SubgroupShuffle {
            value: Box::new(value),
            lane: Box::new(lane),
        }
    }

    /// Subgroup ballot: gather the boolean predicate `cond` across
    /// the active subgroup into a single bitmask.
    #[must_use]
    #[inline]
    pub fn subgroup_ballot(cond: Self) -> Self {
        Self::SubgroupBallot {
            cond: Box::new(cond),
        }
    }

    /// Named variable reference.
    #[must_use]
    #[inline]
    pub fn var(name: impl Into<Ident>) -> Self {
        Self::Var(name.into())
    }

    /// Unsigned 32-bit literal.
    #[must_use]
    #[inline]
    pub fn u32(value: u32) -> Self {
        Self::LitU32(value)
    }

    /// Signed 32-bit literal.
    #[must_use]
    #[inline]
    pub fn i32(value: i32) -> Self {
        Self::LitI32(value)
    }

    /// 32-bit floating-point literal.
    #[must_use]
    #[inline]
    pub fn f32(value: f32) -> Self {
        Self::LitF32(value)
    }

    /// Boolean literal.
    #[must_use]
    #[inline]
    pub fn bool(value: bool) -> Self {
        Self::LitBool(value)
    }

    /// Operation call by stable operation ID.
    #[must_use]
    #[inline]
    pub fn call(op_id: impl Into<Ident>, args: Vec<Self>) -> Self {
        Self::Call {
            op_id: op_id.into(),
            args,
        }
    }

    /// Fused multiply-add `a * b + c` (f32).
    #[must_use]
    #[inline]
    pub fn fma(a: Self, b: Self, c: Self) -> Self {
        Self::Fma {
            a: Box::new(a),
            b: Box::new(b),
            c: Box::new(c),
        }
    }

    /// Cast a value to `target`.
    #[must_use]
    #[inline]
    pub fn cast(target: DataType, value: Self) -> Self {
        Self::Cast {
            target,
            value: Box::new(value),
        }
    }

    /// Wrap a downstream extension expression node.
    #[must_use]
    #[inline]
    pub fn opaque(node: impl ExprNode) -> Self {
        Self::Opaque(Arc::new(node))
    }

    /// Wrap a shared downstream extension expression node.
    #[must_use]
    #[inline]
    pub fn opaque_arc(node: Arc<dyn ExprNode>) -> Self {
        Self::Opaque(node)
    }
}

mod atomics;
mod builders;

#[cfg(test)]
mod tests {
    use super::Expr;

    #[test]
    fn expr_size_is_bounded() {
        let size = std::mem::size_of::<Expr>();
        assert!(
            size <= 128,
            "Expr grew to {size} bytes. Fix: box the largest variant before adding more fields."
        );
    }
}
