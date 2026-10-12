//! Scope binding metadata for the IR validator.
//!
//! During validation the compiler maintains a symbol table that maps
//! variable names to their declared types and mutability. `Binding` is
//! the per-variable record stored in that table.

/// Default validation limits re-exported for convenience.
///
/// These constants bound the size and depth of programs that the
/// validator will accept.
pub use super::depth::{DEFAULT_MAX_CALL_DEPTH, DEFAULT_MAX_NESTING_DEPTH, DEFAULT_MAX_NODE_COUNT};
use crate::ir_inner::model::expr::Ident;
use crate::ir_inner::model::types::DataType;
use crate::validate::{err, ValidationError};
use rustc_hash::FxHashSet;

/// Scope binding: type, mutability, and workgroup-uniformity.
///
/// The validator uses `Binding` to track every live variable: its
/// `DataType` (for type-checking expressions), whether it was
/// declared as mutable (for assignment validation), and whether
/// it holds a value that is *uniform* across every invocation in
/// the same workgroup. The uniformity bit feeds the relaxed
/// barrier-placement rule: a `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }` inside a `Node::Loop`
/// or `Node::If` is legal when the loop bounds (or `If` condition)
/// are uniform, because every invocation reaches the barrier
/// through the same iteration count and branch.
///
/// `ty_known` is `false` when the initialiser expression's type
/// cannot be determined statically (e.g. an `Expr::Call` whose op
/// signature is not visible to `expr_type`). When `ty_known` is
/// `false` the V045 assignment-type check is suppressed: comparing
/// an unknown type to the value's type would either fire a false
/// positive (fabricated sentinel ≠ real type) or silently pass a
/// genuinely wrong assignment (fabricated sentinel = real type by
/// coincidence).
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    /// Declared type of the variable.
    ///
    /// Only meaningful when `ty_known` is `true`. When `ty_known` is
    /// `false` this field is `DataType::U32` as a placeholder; callers
    /// that use `ty` for V045 checking must gate on `ty_known` first.
    pub(crate) ty: DataType,
    /// Whether the type in `ty` was determined from the initialiser.
    ///
    /// `false` means `expr_type` returned `None` and `ty` is a
    /// placeholder; V045 assignment-type checking must be skipped.
    pub(crate) ty_known: bool,
    /// Whether the variable can be reassigned.
    pub(crate) mutable: bool,
    /// Whether the variable is uniform across the workgroup.
    pub(crate) uniform: bool,
}

#[inline]
pub(crate) fn check_sibling_duplicate(
    name: &Ident,
    region_bindings: &mut FxHashSet<Ident>,
    allow_duplicate_siblings: bool,
    errors: &mut Vec<ValidationError>,
) -> bool {
    if region_bindings.insert(name.clone()) {
        return false;
    }
    if allow_duplicate_siblings {
        return false;
    }
    errors.push(err(format!(
        "V032: duplicate sibling let binding `{name}` in the same region. Fix: rename one binding or move one declaration into an inner Block/Region/Loop if a new scope is intended."
    )));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_binding_is_not_duplicate() {
        let mut region_bindings = FxHashSet::default();
        let mut errors = Vec::new();
        let dup =
            check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, false, &mut errors);
        assert!(!dup);
        assert!(errors.is_empty());
    }

    #[test]
    fn second_binding_is_duplicate() {
        let mut region_bindings = FxHashSet::default();
        let mut errors = Vec::new();
        check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, false, &mut errors);
        let dup =
            check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, false, &mut errors);
        assert!(dup);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message().contains("V032"));
    }

    #[test]
    fn different_names_not_duplicate() {
        let mut region_bindings = FxHashSet::default();
        let mut errors = Vec::new();
        check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, false, &mut errors);
        let dup =
            check_sibling_duplicate(&Ident::from("y"), &mut region_bindings, false, &mut errors);
        assert!(!dup);
        assert!(errors.is_empty());
    }

    #[test]
    fn duplicate_sibling_allowed_by_explicit_shadowing_mode() {
        let mut region_bindings = FxHashSet::default();
        let mut errors = Vec::new();
        check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, true, &mut errors);
        let dup =
            check_sibling_duplicate(&Ident::from("x"), &mut region_bindings, true, &mut errors);
        assert!(!dup);
        assert!(errors.is_empty());
    }
}
